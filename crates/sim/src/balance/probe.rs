//! The run-health probe (`docs/design/balance-harness.md`, section 3; #534).
//!
//! Once per in-game minute a run takes a [`Sample`] of its world, and a [`Watch`] reads the
//! samples and raises [`Flag`]s: evidence that a run says something about the scripted player or
//! the sim rather than about balance. The two halves are split so each can be tested on its own:
//! [`Probe::sample`] is the only part that reads a `World`, and [`Watch::observe`] is a pure state
//! machine over plain numbers, so a detector's unit test builds the samples by hand.
//!
//! A flag records the minute it was raised (when its condition had held long enough) and the
//! minute that condition began (`Raised::since`), and only the first time: the verdict asks
//! "did this fire before the deadline", not how often.

use std::collections::{BTreeMap, VecDeque};

use crate::autoplay::DEPOSIT_RADIUS;
use crate::bending::AbilityId;
use crate::defs::{Defs, ResourceId};
use crate::endeavor::SignKind;
use crate::geom::Pos;
use crate::jobs::{ItemId, JobKind};
use crate::world::{Death, IdleReason, Task, UnitId, World};

use super::autopsy::Autopsy;
use super::site::{MATERIAL_SIGNS, SiteProfile, sign_resources};
use super::ticks_to_minutes;

/// The sign a food shortfall is read from (`data/endeavor.toml`), for `Flag::Overgrow`.
const FOOD_SIGN: &str = "food";

/// Minutes of a settler's recent past an autopsy reports (`Autopsy::last_minutes`).
pub(super) const RECENT_MINUTES: usize = 10;

/// What a probe can raise. The first two are the sim's (a settler is cut off or stuck in a task
/// the rules should have ended); the rest are the scripted player's own failures.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Serialize, serde::Deserialize,
)]
#[serde(rename_all = "snake_case")]
pub enum Flag {
    /// A settler has been cut off from water, from a stockpile or from the settlement's anchor
    /// for minutes running (#384, #490). The doc also names an idle reason of `Unreachable`, but
    /// `World::idle_reason` falls back to that for any work a settler isn't blocked from by
    /// geometry too (a build waiting for wood, say), which is most idle minutes of a healthy
    /// run (a seed sweep flagged most runs on it, none with a death); a settler that can
    /// walk to water, its stockpile and home isn't stranded. The idle reason is still in every
    /// autopsy.
    Stranded,
    /// A settler has held the same need task for minutes while that need didn't rise (#386).
    StuckNeed,
    /// Most of the settlement is idle for half an hour with a stores sign unmet and its material
    /// in reach (#512).
    IdleWithWork,
    /// Stores haven't changed in half an hour, same conditions.
    StoresFrozen,
    /// Nothing is being researched while a tech is available (#397).
    ResearchIdle,
    /// A bender sits at full chi with no bend designated for half the time since the first bend
    /// was known (#443).
    ChiIdle,
    /// The player's commands were refused many times in a row.
    Refused,
    /// The population is well over the population sign while the food sign is unmet (#513).
    Overgrow,
}

/// Who a flag implicates: a flagged run is evidence about one of these, never about balance.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum FlagClass {
    /// The sim let a settler get stuck.
    SimLock,
    /// The scripted player left work undone.
    HarnessGap,
}

impl Flag {
    /// Every flag, in the doc's order.
    pub const ALL: [Flag; 8] = [
        Self::Stranded,
        Self::StuckNeed,
        Self::IdleWithWork,
        Self::StoresFrozen,
        Self::ResearchIdle,
        Self::ChiIdle,
        Self::Refused,
        Self::Overgrow,
    ];

    /// The flag's name as the record and the doc spell it.
    pub fn name(self) -> &'static str {
        match self {
            Self::Stranded => "stranded",
            Self::StuckNeed => "stuck_need",
            Self::IdleWithWork => "idle_with_work",
            Self::StoresFrozen => "stores_frozen",
            Self::ResearchIdle => "research_idle",
            Self::ChiIdle => "chi_idle",
            Self::Refused => "refused",
            Self::Overgrow => "overgrow",
        }
    }

    pub fn class(self) -> FlagClass {
        match self {
            Self::Stranded | Self::StuckNeed => FlagClass::SimLock,
            _ => FlagClass::HarnessGap,
        }
    }
}

/// When a flag was raised.
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Raised {
    pub flag: Flag,
    /// The minute its condition had held long enough to raise it.
    pub minute: f32,
    /// The minute that condition began.
    pub since: f32,
}

/// The detectors' thresholds (`data/harness.toml` `[detectors]`, read by `fn_harness`; `fn_sim`
/// reads no files). The defaults are the doc's.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Detectors {
    /// `Stranded`: minutes running.
    pub stranded_minutes: u32,
    /// `StuckNeed`: minutes running.
    pub stuck_minutes: u32,
    /// `IdleWithWork`: the idle share of settlers at or above which a minute counts.
    pub idle_share: f32,
    /// `IdleWithWork`: minutes running.
    pub idle_minutes: u32,
    /// `StoresFrozen`: minutes unchanged.
    pub frozen_minutes: u32,
    /// `ResearchIdle`: minutes with nothing set.
    pub research_minutes: u32,
    /// `ChiIdle`: the share of the minutes since the first bend was known that must be idle.
    pub chi_idle_share: f32,
    /// `ChiIdle`: minutes of bending known before the share is judged.
    pub chi_idle_after: u32,
    /// `Refused`: commands refused in a row.
    pub refused_streak: u32,
    /// `Overgrow`: how far over the population sign, as a share of it.
    pub overgrow_share: f32,
}

impl Default for Detectors {
    fn default() -> Self {
        Self {
            stranded_minutes: 3,
            stuck_minutes: 3,
            idle_share: 0.6,
            idle_minutes: 30,
            frozen_minutes: 30,
            research_minutes: 10,
            chi_idle_share: 0.5,
            chi_idle_after: 20,
            refused_streak: 20,
            overgrow_share: 0.25,
        }
    }
}

/// What one settler was doing in a minute, for an autopsy's last ten and the stranded check.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Occupation {
    /// It had a task.
    Busy,
    /// Idle with nothing designated (`IdleReason::NoWork`).
    NoWork,
    /// Idle with work designated that it can't reach (`IdleReason::Unreachable`).
    Unreachable,
    OutOfChi,
    StockpileFull,
    TooHurt,
    /// Idle for no reason `World::idle_reason` names.
    Idle,
}

impl Occupation {
    /// What `unit` is doing in `world` right now (`None` if there is no such unit).
    pub fn read(world: &World, unit: UnitId, defs: &Defs) -> Option<Self> {
        let task = world.unit(unit)?.task();
        Some(Self::of(task, world.idle_reason(unit, defs)))
    }

    fn of(task: Option<Task>, reason: Option<IdleReason>) -> Self {
        match (task, reason) {
            (Some(_), _) => Self::Busy,
            (None, Some(IdleReason::NoWork)) => Self::NoWork,
            (None, Some(IdleReason::Unreachable)) => Self::Unreachable,
            (None, Some(IdleReason::OutOfChi)) => Self::OutOfChi,
            (None, Some(IdleReason::StockpileFull)) => Self::StockpileFull,
            (None, Some(IdleReason::TooHurt)) => Self::TooHurt,
            (None, None) => Self::Idle,
        }
    }
}

/// One minute's reading of a world: the numbers the detectors judge, nothing else. Built by
/// [`Probe::sample`], or by hand in a test.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Sample {
    pub minute: u32,
    /// Living settlers (the Avatar is a guest and counts for none of this).
    pub settlers: u32,
    /// Settlers with no task.
    pub idle: u32,
    /// The most minutes in a row any settler has been stranded, up to now.
    pub longest_stranded: u32,
    /// The most minutes in a row any settler has held a need task whose need didn't rise.
    pub longest_stuck: u32,
    /// A fingerprint of every stockpile total, so two equal ones mean nothing moved.
    pub stores: u64,
    /// A material sign is unmet, its material is in the site's reach and some of it still stands.
    pub material_unmet: bool,
    /// Nothing is set to research and a tech is available.
    pub research_waiting: bool,
    /// Someone can cast something.
    pub bending_known: bool,
    /// A bender is at full chi and no bend is designated.
    pub chi_idle: bool,
    /// Commands the world has refused since the last one it took.
    pub refused_streak: u32,
    pub population: u32,
    /// The population sign's target, 0 if the data has none.
    pub population_target: u32,
    pub food_met: bool,
}

/// A run of consecutive minutes a condition held.
#[derive(Debug, Clone, Copy, Default)]
struct Run {
    len: u32,
    since: u32,
}

impl Run {
    /// Extends the run if `holds` (starting it at `minute`), else ends it. `back` moves the start
    /// earlier, for a condition that is only seen one minute after it began.
    fn step(&mut self, holds: bool, minute: u32, back: u32) {
        if !holds {
            *self = Self::default();
        } else if self.len == 0 {
            *self = Self { len: 1, since: minute.saturating_sub(back) };
        } else {
            self.len += 1;
        }
    }
}

/// The detectors' state: reads one [`Sample`] per minute, in order, and records each flag the
/// first time it is raised.
#[derive(Debug, Clone)]
pub struct Watch {
    det: Detectors,
    idle: Run,
    frozen: Run,
    research: Run,
    last_stores: Option<u64>,
    bending_minutes: u32,
    chi_idle_minutes: u32,
    chi_idle_since: Option<u32>,
    raised: BTreeMap<Flag, Raised>,
}

impl Watch {
    pub fn new(det: Detectors) -> Self {
        Self {
            det,
            idle: Run::default(),
            frozen: Run::default(),
            research: Run::default(),
            last_stores: None,
            bending_minutes: 0,
            chi_idle_minutes: 0,
            chi_idle_since: None,
            raised: BTreeMap::new(),
        }
    }

    /// Reads the next minute's sample.
    pub fn observe(&mut self, s: &Sample) {
        let d = self.det;
        if d.stranded_minutes > 0 && s.longest_stranded >= d.stranded_minutes {
            self.raise(Flag::Stranded, s.minute, s.minute + 1 - s.longest_stranded);
        }
        if d.stuck_minutes > 0 && s.longest_stuck >= d.stuck_minutes {
            self.raise(Flag::StuckNeed, s.minute, s.minute + 1 - s.longest_stuck);
        }

        let mostly_idle = s.settlers > 0 && s.idle as f32 >= d.idle_share * s.settlers as f32;
        self.idle.step(mostly_idle && s.material_unmet, s.minute, 0);
        if self.idle.len >= d.idle_minutes.max(1) {
            self.raise(Flag::IdleWithWork, s.minute, self.idle.since);
        }

        let unchanged = self.last_stores == Some(s.stores);
        self.last_stores = Some(s.stores);
        self.frozen.step(unchanged && s.material_unmet, s.minute, 1);
        if self.frozen.len >= d.frozen_minutes.max(1) {
            self.raise(Flag::StoresFrozen, s.minute, self.frozen.since);
        }

        self.research.step(s.research_waiting, s.minute, 0);
        if self.research.len >= d.research_minutes.max(1) {
            self.raise(Flag::ResearchIdle, s.minute, self.research.since);
        }

        if s.bending_known {
            self.bending_minutes += 1;
            if s.chi_idle {
                self.chi_idle_minutes += 1;
                self.chi_idle_since.get_or_insert(s.minute);
            }
            if self.bending_minutes >= d.chi_idle_after.max(1)
                && self.chi_idle_minutes as f32 >= d.chi_idle_share * self.bending_minutes as f32
                && let Some(since) = self.chi_idle_since
            {
                self.raise(Flag::ChiIdle, s.minute, since);
            }
        }

        if d.refused_streak > 0 && s.refused_streak >= d.refused_streak {
            self.raise(Flag::Refused, s.minute, s.minute);
        }

        let over = s.population as f32 > (1.0 + d.overgrow_share) * s.population_target as f32;
        if s.population_target > 0 && over && !s.food_met {
            self.raise(Flag::Overgrow, s.minute, s.minute);
        }
    }

    fn raise(&mut self, flag: Flag, minute: u32, since: u32) {
        self.raised.entry(flag).or_insert(Raised {
            flag,
            minute: minute as f32,
            since: since as f32,
        });
    }

    /// Every flag raised so far, in `Flag::ALL`'s order.
    pub fn raised(&self) -> Vec<Raised> {
        self.raised.values().copied().collect()
    }

    /// When `flag` was first raised, if it has been.
    pub fn first(&self, flag: Flag) -> Option<Raised> {
        self.raised.get(&flag).copied()
    }
}

/// What the probe remembers of one settler between minutes.
#[derive(Debug, Clone)]
pub(super) struct Track {
    pub(super) pos: Pos,
    hunger: f32,
    thirst: f32,
    pub(super) last_ate: u32,
    pub(super) last_drank: u32,
    /// What it was doing in each of the last `RECENT_MINUTES` minutes, oldest first.
    pub(super) recent: VecDeque<Occupation>,
    /// Minutes running it has been idle with only unreachable work.
    pub(super) unreachable: u32,
    /// Minutes running it has been stranded (`Probe::sample`).
    stranded: u32,
    /// The need task it held last minute and the need's level then.
    need: Option<(Task, f32)>,
    stuck: u32,
}

/// A material sign the probe watches (`MATERIAL_SIGNS`).
#[derive(Debug, Clone)]
struct Material {
    sign: String,
    /// The deposits that feed it.
    resources: Vec<ResourceId>,
    /// Whether the site profile says any is in reach.
    in_reach: bool,
}

/// Reads a world once a minute. Keeps what the detectors and autopsies need between minutes
/// (each settler's recent past); everything else it asks the world afresh.
#[derive(Debug, Clone)]
pub struct Probe {
    materials: Vec<Material>,
    abilities: Vec<AbilityId>,
    items: Vec<ItemId>,
    tracks: BTreeMap<UnitId, Track>,
    anchor: Pos,
    /// Deaths already given an autopsy.
    autopsied: usize,
}

impl Probe {
    pub fn new(world: &World, defs: &Defs, site: &SiteProfile) -> Self {
        let materials = MATERIAL_SIGNS
            .iter()
            .map(|&sign| Material {
                sign: sign.to_owned(),
                resources: sign_resources(defs, sign),
                in_reach: site.supply.get(sign).is_some_and(|&n| n > 0),
            })
            .collect();
        Self {
            materials,
            abilities: (0..defs.abilities.len()).map(|i| AbilityId(i as u16)).collect(),
            items: (0..defs.items.len()).map(|i| ItemId(i as u16)).collect(),
            tracks: BTreeMap::new(),
            anchor: world.settlement_anchor(),
            autopsied: 0,
        }
    }

    /// Takes the sample for the minute `world` is at. Call once per minute, before the player
    /// acts; it reads the world and changes only its own memory.
    pub fn sample(&mut self, world: &World, defs: &Defs) -> Sample {
        let minute = (ticks_to_minutes(world.tick) + 0.5) as u32;
        let settlers: Vec<&crate::world::Unit> =
            world.units.iter().filter(|u| u.is_settler()).collect();
        self.tracks.retain(|id, _| settlers.iter().any(|u| u.id == *id));

        let (mut idle, mut longest_stranded, mut longest_stuck) = (0, 0, 0);
        for u in &settlers {
            let task = u.task();
            let reason = world.idle_reason(u.id, defs);
            let occupation = Occupation::of(task, reason);
            if task.is_none() {
                idle += 1;
            }
            let reach = world.reach(u.id);
            let cut_off =
                reach.is_some_and(|r| !r.water || r.stockpile == Some(false) || !r.anchor);

            let t = self.tracks.entry(u.id).or_insert_with(|| Track {
                pos: u.pos,
                hunger: u.needs.hunger,
                thirst: u.needs.thirst,
                last_ate: minute,
                last_drank: minute,
                recent: VecDeque::new(),
                unreachable: 0,
                stranded: 0,
                need: None,
                stuck: 0,
            });
            t.pos = u.pos;
            if u.needs.hunger > t.hunger {
                t.last_ate = minute;
            }
            if u.needs.thirst > t.thirst {
                t.last_drank = minute;
            }
            (t.hunger, t.thirst) = (u.needs.hunger, u.needs.thirst);
            if t.recent.len() == RECENT_MINUTES {
                t.recent.pop_front();
            }
            t.recent.push_back(occupation);
            let unreachable = occupation == Occupation::Unreachable;
            t.unreachable = if unreachable { t.unreachable + 1 } else { 0 };
            t.stranded = if cut_off { t.stranded + 1 } else { 0 };

            // A need task and the level of the need it serves: eating raises hunger, drinking
            // thirst, sleeping energy. Holding the same one while the level doesn't rise means
            // the meal, drink or sleep isn't happening.
            let need = match task {
                Some(task @ Task::Eat(_)) => Some((task, u.needs.hunger)),
                Some(task @ Task::Drink) => Some((task, u.needs.thirst)),
                Some(task @ Task::Sleep) => Some((task, u.needs.energy)),
                _ => None,
            };
            t.stuck = match (need, t.need) {
                (Some((now, level)), Some((before, was))) if now == before && level <= was => {
                    t.stuck + 1
                }
                _ => 0,
            };
            t.need = need;
            longest_stranded = longest_stranded.max(t.stranded);
            longest_stuck = longest_stuck.max(t.stuck);
        }

        let signs = world.endeavor_signs(defs);
        let progress = |id: &str| signs.iter().find(|p| p.id == id);
        let population_target = defs
            .endeavor
            .signs
            .iter()
            .zip(&signs)
            .find(|(s, _)| s.kind == SignKind::Population)
            .map_or(0, |(_, p)| p.target);
        let material_unmet = self.materials.iter().any(|m| {
            m.in_reach
                && progress(&m.sign).is_some_and(|p| !p.met)
                && self.deposits_standing(world, &m.resources) > 0
        });

        let queue_empty = world.research().queue().is_empty();
        let research_waiting = queue_empty
            && defs.techs_for(world.nation).any(|(id, t)| {
                // A school option closed by its sibling can never be set (horizons-phase-4.md).
                !world.research().is_known(id)
                    && world.research().closed_by(id, defs).is_none()
                    // A teaching can be set only once kept from a draft (horizons-phase-5.md).
                    && (t.teaching.is_none() || world.kept_teachings().any(|k| k == id))
                    && t.visible_when(world.united())
                    && t.prereqs.iter().all(|&p| world.research().is_known(p))
            });

        let casters = |full: bool| {
            settlers.iter().any(|u| {
                (!full || u.chi >= defs.chi.max - 0.01)
                    && self.abilities.iter().any(|&a| world.can_cast(u, a, defs))
            })
        };
        let bending_known = casters(false);
        let bend_designated = world.jobs().iter().any(|j| matches!(j.kind, JobKind::Bend { .. }));

        Sample {
            minute,
            settlers: settlers.len() as u32,
            idle,
            longest_stranded,
            longest_stuck,
            stores: self.stores_fingerprint(world),
            material_unmet,
            research_waiting,
            bending_known,
            chi_idle: bending_known && !bend_designated && casters(true),
            refused_streak: world.refusals().streak(),
            population: settlers.len() as u32,
            population_target,
            food_met: progress(FOOD_SIGN).is_some_and(|p| p.met),
        }
    }

    /// Resource tiles of `resources` standing within `DEPOSIT_RADIUS` of the anchor.
    fn deposits_standing(&self, world: &World, resources: &[ResourceId]) -> u32 {
        let r = DEPOSIT_RADIUS;
        let (w, h) = (world.map.width(), world.map.height());
        let mut n = 0;
        for y in (self.anchor.y - r).max(0)..=(self.anchor.y + r).min(h - 1) {
            for x in (self.anchor.x - r).max(0)..=(self.anchor.x + r).min(w - 1) {
                if world.map[Pos::new(x, y)].resource.is_some_and(|res| resources.contains(&res)) {
                    n += 1;
                }
            }
        }
        n
    }

    /// FNV-1a over every item's stockpile total: equal fingerprints, equal stores.
    fn stores_fingerprint(&self, world: &World) -> u64 {
        let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
        for &item in &self.items {
            for byte in world.stored_total(item).to_le_bytes() {
                hash = (hash ^ u64::from(byte)).wrapping_mul(0x0100_0000_01b3);
            }
        }
        hash
    }

    /// An autopsy for every death since the last call, oldest first. Call it right after a tick
    /// that added deaths, so the world is as the settler left it.
    pub fn autopsies(&mut self, world: &World, defs: &Defs) -> Vec<Autopsy> {
        let new: Vec<&Death> = world.deaths().skip(self.autopsied).collect();
        self.autopsied = world.deaths().count();
        new.into_iter()
            .map(|death| {
                let track = self.tracks.remove(&death.id);
                Autopsy::of(death, track.as_ref(), self.anchor, world, defs)
            })
            .collect()
    }
}

#[cfg(test)]
mod tests;
