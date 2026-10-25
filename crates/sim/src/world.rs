//! The live game state and its fixed-step update.
//!
//! The sim advances in discrete ticks (`World::tick`). Clients render whatever state the world is
//! in and may interpolate between ticks, but must never mutate it except through commands.

use std::collections::{BTreeMap, BTreeSet};

use rand::RngExt;
use rand::seq::SliceRandom;

use crate::bending::AbilityId;
use crate::building::BuildingDef;
use crate::defs::Defs;
use crate::geom::Pos;
use crate::jobs::{Job, JobId, Stack};
use crate::mapgen::{self, MapGenConfig, TileMap};
use crate::nation::{Modifiers, Nation};
use crate::needs::{DeathCause, Needs};
use crate::path::MoveRules;
use crate::regions::Regions;
use crate::research::{self, Research, ResearchError, TechId};
use crate::rng::{self, SimRng};

mod bending;
mod build;
mod command;
mod heat;
mod stride;
mod study;
mod work;

use bending::Patch;
pub use bending::{BendChannel, BendError, BendEvent, BendOutcome, Moved};
pub use build::BuildError;
use build::Site;
pub use command::{Command, CommandError, Recorded};
pub use heat::{Flame, HeatSource};
pub use stride::{STRIDE_LOOKAHEAD, Stride};
pub use work::{DesignateError, FoodSource, Task};

/// Ticks per in-game second. Movement and needs are expressed per tick.
pub const TICKS_PER_SECOND: u32 = 10;

/// Ticks a baseline (move_speed 1.0) unit waits between steps.
const BASE_TICKS_PER_STEP: f32 = 6.0;

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, serde::Serialize, serde::Deserialize,
)]
pub struct UnitId(pub u32);

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Unit {
    pub id: UnitId,
    pub name: String,
    pub nation: Nation,
    pub pos: Pos,
    /// Which of the nation's sprite variants to draw. Purely cosmetic.
    pub variant: u8,
    /// Hunger, thirst, energy and health (data/needs.toml). At 0 health the unit dies.
    pub needs: Needs,
    /// Bending energy, from 0 to `data/bending.toml`'s `chi.max`. Regenerates every tick (faster
    /// asleep) and is spent when a bend completes. Never hurts the unit.
    pub chi: f32,
    /// What this unit is carrying to a stockpile (or has picked up to eat).
    pub carrying: Option<Stack>,
    /// A need it is acting on (eat, drink, sleep). Takes priority over `job_task`.
    need_task: Option<Task>,
    /// The job it has claimed. Set aside, not dropped, while a need is being met.
    job_task: Option<Task>,
    /// Steps left to the current task's destination, the next step last. `None` means the
    /// route needs planning (a new claim has just resumed, say).
    route: Option<Vec<Pos>>,
    /// Seconds into the current need task's work (eating, drinking).
    work: f32,
    /// The bed this unit is sleeping in or walking to, while its need task is `Task::Sleep`.
    /// Other units treat it as taken (`work.rs`, issue #58).
    #[serde(default)]
    bed: Option<Pos>,
    /// Tick before which a failed attempt to act on a need isn't repeated (hunger, thirst,
    /// energy).
    need_retry_at: [u64; 3],
    /// Tick before which an idle unit doesn't look for a job again.
    claim_retry_at: u64,
    /// Progress toward the next step; a step happens when this reaches `BASE_TICKS_PER_STEP`.
    move_progress: f32,
    /// Tick before which an idle unit stands still instead of deciding whether to wander
    /// (`data/jobs.toml`, `idle_pause_seconds`; issue #110).
    #[serde(default)]
    idle_until: u64,
    /// Steps left in the idle wander in progress; 0 when standing.
    #[serde(default)]
    wander_left: u8,
}

/// A unit that died, kept so the UI and tests can see what happened.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Death {
    pub id: UnitId,
    pub name: String,
    pub tick: u64,
    pub cause: DeathCause,
}

/// Where `count` starting units go: walkable tiles, spiralling out from the map center.
fn spawn_points(map: &TileMap, count: usize) -> Vec<Pos> {
    let center = Pos::new(map.width() / 2, map.height() / 2);
    let mut candidates: Vec<Pos> = map
        .iter()
        .filter(|(_, t)| t.terrain.is_walkable() && t.resource.is_none())
        .map(|(p, _)| p)
        .collect();
    candidates.sort_by_key(|p| ((p.x - center.x).pow(2) + (p.y - center.y).pow(2), *p));
    candidates.truncate(count);
    candidates
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct World {
    pub seed: u64,
    pub tick: u64,
    pub map: TileMap,
    pub nation: Nation,
    /// Living units, ordered by id. Dead units are removed (see `deaths`), so a unit's index
    /// here can change; use `UnitId` to refer to one.
    pub units: Vec<Unit>,
    research: Research,
    /// The nation's modifiers with every known tech applied. Recomputed when a tech completes;
    /// read it through `modifiers()`.
    modifiers: Modifiers,
    /// Units that have died, oldest first.
    pub deaths: Vec<Death>,
    /// Designated work that isn't finished, in creation order.
    jobs: Vec<Job>,
    next_job: u32,
    /// Commands applied so far (`World::apply`), for replay.
    command_log: Vec<Recorded>,
    /// Renewable resources that have been worked and are growing back, with the seconds of
    /// growth each still needs. They stay on the map but can't be worked until they ripen.
    resting: BTreeMap<Pos, f32>,
    /// Harvest tiles with a standing order (#81): when the resource here next ripens, a new
    /// `JobKind::Harvest` is queued automatically instead of waiting for the player to
    /// re-designate it. Cleared by `CancelHarvest`.
    standing_harvest: BTreeSet<Pos>,
    /// Item stacks lying on the ground, one stack per tile, waiting to be hauled.
    ground: BTreeMap<Pos, Stack>,
    /// Tiles designated as stockpile.
    zones: BTreeSet<Pos>,
    /// What the stockpile tiles hold, one kind of item per tile.
    stored: BTreeMap<Pos, Stack>,
    /// Build designations that aren't finished, by tile (`Command::DesignateBuild`, #64).
    sites: BTreeMap<Pos, Site>,
    /// How units walk. Every unit uses these for now; a nation that can cross water would get
    /// its own.
    rules: MoveRules,
    /// How a unit hauling a stack moves: `rules` plus rail (#135). Built from the building defs
    /// when the world is made, so a save from before rail existed has the plain `rules` here.
    #[serde(default)]
    cart_rules: MoveRules,
    /// Which tiles can reach which under `rules`, or `None` when the map has changed since it
    /// was computed. Recomputed on demand; see `regions.rs`.
    #[serde(skip)]
    regions: Option<Regions>,
    /// Every bend that has finished or failed, oldest first.
    bend_events: Vec<BendEvent>,
    /// Watered patches that haven't dried yet, in `(dries_at, centre)` order (Water the Land).
    patches: Vec<Patch>,
    /// Tiles that sprouted lately and the tick their fallow time ends. Expired entries are
    /// dropped as time passes.
    fallow: BTreeMap<Pos, u64>,
    /// Lit heat sources by tile: Hearthflames today, campfires later (`heat.rs`).
    heat: BTreeMap<Pos, HeatSource>,
    /// Picks which tiles of a dried patch sprout (`bending.sprout`).
    sprout_rng: SimRng,
    rng: SimRng,
}

impl World {
    pub fn new(cfg: &MapGenConfig, nation: Nation, starting_units: u32, defs: &Defs) -> Self {
        let map = mapgen::generate(cfg, defs);
        let spawns = spawn_points(&map, starting_units as usize);
        Self::with_map(cfg.seed, map, nation, &spawns, defs)
    }

    /// A world on a given map, with one unit at each of `spawns`. For tests and tools that need a
    /// hand-built map; `new` is the usual way in.
    pub fn with_map(seed: u64, map: TileMap, nation: Nation, spawns: &[Pos], defs: &Defs) -> Self {
        let mut world = Self {
            seed,
            tick: 0,
            map,
            nation,
            units: Vec::new(),
            research: Research::new(nation, defs),
            modifiers: defs.nation(nation).modifiers,
            deaths: Vec::new(),
            jobs: Vec::new(),
            next_job: 0,
            command_log: Vec::new(),
            resting: BTreeMap::new(),
            standing_harvest: BTreeSet::new(),
            ground: BTreeMap::new(),
            zones: BTreeSet::new(),
            stored: BTreeMap::new(),
            sites: BTreeMap::new(),
            rules: MoveRules::default(),
            cart_rules: defs.cart_rules(MoveRules::default()),
            regions: None,
            bend_events: Vec::new(),
            patches: Vec::new(),
            fallow: BTreeMap::new(),
            heat: BTreeMap::new(),
            sprout_rng: rng::stream(seed, "bending.sprout"),
            rng: rng::stream(seed, "world.units"),
        };
        world.refresh_modifiers(defs);
        world.spawn_units(spawns, defs);
        world
    }

    /// Checks that a world read from a save is consistent enough to run: the things that would
    /// panic later (grid shape, research tables, unit placement), not every invariant.
    pub(crate) fn validate(&self, defs: &Defs) -> Result<(), String> {
        if !self.map.is_well_formed() {
            return Err("the map's size doesn't match its tiles".into());
        }
        if !self.research.fits(defs) {
            return Err("the research state doesn't match the tech tree".into());
        }
        if !self.units.windows(2).all(|w| w[0].id < w[1].id) {
            return Err("units are out of order".into());
        }
        if let Some(u) = self.units.iter().find(|u| !self.map.in_bounds(u.pos)) {
            return Err(format!("{} is off the map", u.name));
        }
        Ok(())
    }

    /// The modifiers systems must read: the nation's base times every completed tech's
    /// multipliers. Never read `NationDef::modifiers` directly, or techs do nothing.
    pub fn modifiers(&self) -> &Modifiers {
        &self.modifiers
    }

    /// How the effective value of the modifier `name` is built up, for tooltips: the nation's
    /// base, then each known tech that multiplies it, as `(source name, factor)`. Empty for an
    /// unknown name.
    pub fn modifier_breakdown<'d>(&self, name: &str, defs: &'d Defs) -> Vec<(&'d str, f32)> {
        research::modifier_breakdown(name, self.nation, self.research.known_techs(), defs)
    }

    fn refresh_modifiers(&mut self, defs: &Defs) {
        self.modifiers =
            research::effective_modifiers(self.nation, self.research.known_techs(), defs);
        self.refresh_heat_radii(defs);
    }

    pub fn research(&self) -> &Research {
        &self.research
    }

    /// Is something whose `unlocked_by` is `unlocked_by` available to this settlement right now:
    /// no tech locks it (`None`), or the nation knows the one that does (`Unlocks::buildings`,
    /// `Unlocks::jobs`, issue #22).
    pub fn is_unlocked(&self, unlocked_by: Option<TechId>) -> bool {
        unlocked_by.is_none_or(|t| self.research.is_known(t))
    }

    /// Sets the research target: the queue becomes `tech` plus its unknown prereqs. Banked
    /// knowledge is kept. Reached through `Command::SetResearch`.
    fn set_research(&mut self, tech: TechId, defs: &Defs) -> Result<(), ResearchError> {
        self.research.set_target(tech, self.nation, defs)
    }

    fn spawn_units(&mut self, spawns: &[Pos], defs: &Defs) {
        let nation = defs.nation(self.nation);
        let mut names = nation.names.clone();
        names.shuffle(&mut rng::stream(self.seed, "world.names"));
        for (i, &pos) in spawns.iter().enumerate() {
            let variant = self.rng.random_range(0..nation.characters.len() as u8);
            // Reuse the pool with a numeral suffix if there are more units than names.
            let name = match i / names.len() {
                0 => names[i % names.len()].clone(),
                n => format!("{} {}", names[i % names.len()], n + 1),
            };
            self.units.push(Unit {
                id: UnitId(i as u32),
                name,
                nation: self.nation,
                pos,
                variant,
                needs: Needs::full(&defs.needs),
                chi: defs.chi.max,
                carrying: None,
                need_task: None,
                job_task: None,
                route: None,
                work: 0.0,
                bed: None,
                need_retry_at: [0; 3],
                claim_retry_at: 0,
                move_progress: 0.0,
                idle_until: 0,
                wander_left: 0,
            });
        }
    }

    /// The living unit with this id, if it hasn't died.
    pub fn unit(&self, id: UnitId) -> Option<&Unit> {
        self.units.binary_search_by_key(&id, |u| u.id).ok().map(|i| &self.units[i])
    }

    /// Tells the world that `map` was edited, so cached connectivity can't be trusted. The world
    /// does this itself for changes it makes (clearing a resource); only code that writes to
    /// `map` directly needs to call it.
    pub fn map_changed(&mut self) {
        self.regions = None;
    }

    /// The connected regions of the current map, recomputed first if the map has changed.
    fn regions(&mut self) -> &Regions {
        let (map, rules) = (&self.map, &self.rules);
        self.regions.get_or_insert_with(|| Regions::compute(map, rules))
    }

    fn walkable(&self, p: Pos) -> bool {
        self.map.get(p).is_some_and(|t| t.terrain.is_walkable() && t.resource.is_none())
    }

    /// Advances the world by one tick: research, units' needs and chi, resource regrowth and
    /// drying patches and burning-out flames run (units whose health reaches 0 die and are removed, in id order), then
    /// each survivor acts, in id order: it meets a need if one is low, else carries on with its
    /// job, else claims one, else wanders. See `work.rs`.
    pub fn tick(&mut self, defs: &Defs) {
        self.tick += 1;
        let completed = self.research.completions().len();
        self.research.tick(self.tick, defs);
        if self.research.completions().len() != completed {
            self.refresh_modifiers(defs);
        }
        self.tick_needs(defs);
        self.tick_growth(defs);
        self.tick_irrigation(defs);
        self.tick_heat();
        for i in 0..self.units.len() {
            self.act(i, defs);
        }
    }

    /// What a unit with nothing to do does (issue #110). It stands still for `idle_pause_seconds`;
    /// then, with chance `idle_wander_chance`, it wanders 1 to `idle_wander_steps` random steps
    /// (at a speed set by the `move_speed` modifier) before standing again. All draws come from
    /// the `world.units` stream.
    fn wander(&mut self, i: usize, defs: &Defs) {
        if self.tick < self.units[i].idle_until {
            return;
        }
        let s = defs.jobs;
        if self.units[i].wander_left == 0 {
            if self.rng.random::<f32>() >= s.idle_wander_chance {
                self.rest_idle(i, defs);
                return;
            }
            self.units[i].wander_left = self.rng.random_range(1..=s.idle_wander_steps);
            self.units[i].move_progress = 0.0;
        }
        let speed = self.modifiers.move_speed;
        self.units[i].move_progress += speed;
        if self.units[i].move_progress < BASE_TICKS_PER_STEP {
            return;
        }
        self.units[i].move_progress -= BASE_TICKS_PER_STEP;
        let pos = self.units[i].pos;
        let options: Vec<Pos> =
            self.map.neighbors(pos, false).filter(|&q| self.walkable(q)).collect();
        if options.is_empty() {
            self.units[i].wander_left = 0;
        } else {
            self.units[i].pos = options[self.rng.random_range(0..options.len())];
            self.units[i].wander_left -= 1;
        }
        if self.units[i].wander_left == 0 {
            self.rest_idle(i, defs);
        }
    }

    /// Starts an idle pause: the unit stands still for `idle_pause_seconds`.
    fn rest_idle(&mut self, i: usize, defs: &Defs) {
        let pause = (defs.jobs.idle_pause_seconds * TICKS_PER_SECOND as f32).ceil() as u64;
        self.units[i].wander_left = 0;
        self.units[i].idle_until = self.tick + pause;
    }
}

impl World {
    fn tick_needs(&mut self, defs: &Defs) {
        let tick = self.tick;
        let mut dead = Vec::new();
        let chi_regen = self.modifiers.chi_regen;
        for mut u in std::mem::take(&mut self.units) {
            let rest = if u.is_sleeping() { self.sleep_factor_for(&u, defs) } else { 1.0 };
            u.needs.tick_scaled(&defs.needs, u.is_sleeping(), rest);
            u.chi = defs.chi.regen(u.chi, u.is_sleeping(), chi_regen);
            if u.needs.is_dead() {
                dead.push(u);
            } else {
                self.units.push(u);
            }
        }
        for u in dead {
            let cause = u.needs.depleted().unwrap_or(DeathCause::Starvation);
            self.deaths.push(Death { id: u.id, name: u.name.clone(), tick, cause });
            self.release(&u);
        }
    }
}

#[cfg(test)]
mod idle_tests;

#[cfg(test)]
mod rest_tests;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::research::TechCompleted;

    fn world(nation: Nation) -> (World, Defs) {
        let defs = Defs::bundled();
        let cfg = MapGenConfig { seed: 7, ..Default::default() };
        (World::new(&cfg, nation, 4, &defs), defs)
    }

    #[test]
    fn units_spawn_on_walkable_tiles() {
        let (w, _) = world(Nation::Earth);
        assert_eq!(w.units.len(), 4);
        assert!(w.units.iter().all(|u| w.walkable(u.pos)));
    }

    #[test]
    fn units_get_unique_deterministic_names() {
        let (a, _) = world(Nation::Fire);
        let (b, _) = world(Nation::Fire);
        let names: Vec<_> = a.units.iter().map(|u| u.name.clone()).collect();
        assert_eq!(names, b.units.iter().map(|u| u.name.clone()).collect::<Vec<_>>());
        let unique: std::collections::BTreeSet<_> = names.iter().collect();
        assert_eq!(unique.len(), names.len());
    }

    #[test]
    fn units_stay_on_walkable_tiles() {
        let (mut w, defs) = world(Nation::Water);
        for _ in 0..500 {
            w.tick(&defs);
        }
        assert!(w.units.iter().all(|u| w.walkable(u.pos)));
    }

    #[test]
    fn airbenders_cover_more_ground_than_earthbenders() {
        // Count steps taken during a long wander: air's move_speed modifier is higher than earth's.
        let steps = |nation| {
            let (mut w, defs) = world(nation);
            for u in &mut w.units {
                u.wander_left = 200;
            }
            let mut moved = 0;
            for _ in 0..300 {
                let before: Vec<Pos> = w.units.iter().map(|u| u.pos).collect();
                w.tick(&defs);
                moved += w.units.iter().zip(&before).filter(|(u, b)| u.pos != **b).count();
            }
            moved
        };
        assert!(steps(Nation::Air) > steps(Nation::Earth));
    }

    fn tech(defs: &Defs, id: &str) -> TechId {
        defs.tech_id(id).unwrap()
    }

    fn spent_millis(w: &World, defs: &Defs) -> u64 {
        w.research().completions().iter().map(|c| u64::from(defs.tech(c.tech).cost) * 1000).sum()
    }

    #[test]
    fn research_starts_with_the_nations_starting_techs() {
        let (w, defs) = world(Nation::Fire);
        let known: Vec<_> = w.research().known_techs().collect();
        assert_eq!(known, [tech(&defs, "fire_making")]);
        assert!(w.research().queue().is_empty());
        assert_eq!(w.research().pool_millis(), 0);
    }

    #[test]
    fn passive_rate_is_converted_to_millis_per_tick() {
        // 3 and 9 points a minute at 10 ticks a second.
        assert_eq!(Defs::bundled().research.passive_millis_per_tick, 5);
        assert_eq!(Defs::bundled().research.study_millis_per_tick, 15);
    }

    #[test]
    fn fire_making_completes_when_the_pacing_math_says() {
        let (mut w, defs) = world(Nation::Earth);
        let fm = tech(&defs, "fire_making");
        w.set_research(fm, &defs).unwrap();
        // 25 points at 3 a minute is 500 seconds, so tick 5000.
        for _ in 0..4999 {
            w.tick(&defs);
        }
        assert!(!w.research().is_known(fm));
        w.tick(&defs);
        assert!(w.research().is_known(fm));
        assert_eq!(w.research().completions(), [TechCompleted { tech: fm, tick: 5000 }]);
        assert!(w.research().queue().is_empty());
        assert_eq!(w.research().pool_millis(), 0);
    }

    #[test]
    fn deep_tech_queues_its_unknown_prereq_chain() {
        let (mut w, defs) = world(Nation::Earth);
        let target = tech(&defs, "earthen_walls");
        w.set_research(target, &defs).unwrap();
        let mut expected: Vec<TechId> =
            ["shelter", "carpentry", "masonry", "earth_shaping", "earthen_walls"]
                .iter()
                .map(|n| tech(&defs, n))
                .collect();
        expected.sort();
        // stoneworking is a starting tech, so it isn't queued.
        assert_eq!(w.research().queue(), expected);
        assert_eq!(w.research().queue().last(), Some(&target));
    }

    #[test]
    fn set_research_rejects_other_nations_and_known_techs() {
        let (mut w, defs) = world(Nation::Earth);
        assert_eq!(
            w.set_research(tech(&defs, "lightning_generation"), &defs),
            Err(ResearchError::WrongNation)
        );
        assert_eq!(
            w.set_research(tech(&defs, "stoneworking"), &defs),
            Err(ResearchError::AlreadyKnown)
        );
        assert_eq!(w.set_research(TechId(u16::MAX), &defs), Err(ResearchError::UnknownTech));
        assert!(w.research().queue().is_empty());
    }

    #[test]
    fn knowledge_banks_while_idle_and_carries_over() {
        let (mut w, defs) = world(Nation::Earth);
        for _ in 0..6000 {
            w.tick(&defs);
        }
        assert_eq!(w.research().pool_millis(), 30_000);
        w.set_research(tech(&defs, "shelter"), &defs).unwrap();
        w.tick(&defs);
        // 30_005 in the pool, minus shelter's 25_000.
        assert_eq!(w.research().pool_millis(), 5_005);
        assert_eq!(w.research().completions().len(), 1);
    }

    #[test]
    fn several_techs_can_complete_in_one_tick() {
        let (mut w, defs) = world(Nation::Earth);
        // 100 points banked pays for shelter (25) and carpentry (60) together.
        for _ in 0..20_000 {
            w.tick(&defs);
        }
        w.set_research(tech(&defs, "carpentry"), &defs).unwrap();
        w.tick(&defs);
        let ticks: Vec<_> = w.research().completions().iter().map(|c| c.tick).collect();
        assert_eq!(ticks, [20_001, 20_001]);
        assert!(w.research().is_known(tech(&defs, "carpentry")));
    }

    #[test]
    fn changing_target_loses_no_knowledge() {
        let (mut w, defs) = world(Nation::Earth);
        w.set_research(tech(&defs, "fire_making"), &defs).unwrap();
        for _ in 0..3000 {
            w.tick(&defs);
        }
        let before = w.research().pool_millis();
        w.set_research(tech(&defs, "masonry"), &defs).unwrap();
        assert_eq!(w.research().pool_millis(), before);
        for _ in 0..9000 {
            w.tick(&defs);
        }
        // Everything earned is either still banked or was spent on a completed tech.
        assert_eq!(w.research().pool_millis() + spent_millis(&w, &defs), 5 * 12_000);
        assert!(!w.research().completions().is_empty());
    }

    #[test]
    fn research_is_deterministic() {
        let run = || {
            let (mut w, defs) = world(Nation::Water);
            w.set_research(tech(&defs, "healing"), &defs).unwrap();
            for t in 0..20_000 {
                if t == 7_000 {
                    w.set_research(tech(&defs, "water_travel"), &defs).unwrap();
                }
                w.tick(&defs);
            }
            w.research().clone()
        };
        assert_eq!(run(), run());
    }

    /// All of `m`'s modifiers as raw bits, so tests can demand bitwise equality.
    fn bits(m: &Modifiers) -> Vec<u32> {
        Modifiers::NAMES.iter().map(|n| m.get(n).unwrap().to_bits()).collect()
    }

    #[test]
    fn modifiers_start_as_the_nations_base() {
        let (w, defs) = world(Nation::Earth);
        assert_eq!(w.modifiers(), &defs.nation(Nation::Earth).modifiers);
    }

    #[test]
    fn completing_a_tech_changes_the_relevant_modifier() {
        let (mut w, defs) = world(Nation::Earth);
        let base = defs.nation(Nation::Earth).modifiers;
        // 100 points banked pays for shelter (25) and carpentry (60) in one tick.
        for _ in 0..20_000 {
            w.tick(&defs);
        }
        assert_eq!(w.modifiers(), &base);
        w.set_research(tech(&defs, "carpentry"), &defs).unwrap();
        w.tick(&defs);
        assert_eq!(w.modifiers().construction, base.construction * 1.1);
        assert_eq!(w.modifiers(), &Modifiers { construction: base.construction * 1.1, ..base });
        assert_eq!(
            w.modifier_breakdown("construction", &defs),
            [("Earth Kingdom", base.construction), ("Carpentry", 1.1)]
        );
        // A modifier no known tech touches has just the nation's entry.
        assert_eq!(w.modifier_breakdown("move_speed", &defs), [("Earth Kingdom", base.move_speed)]);
        assert!(w.modifier_breakdown("nonsense", &defs).is_empty());
    }

    #[test]
    fn research_order_does_not_change_the_modifiers() {
        let (mut w, defs) = world(Nation::Earth);
        // Enough banked for shelter, carpentry, masonry and earth_shaping (265 points).
        for _ in 0..54_000 {
            w.tick(&defs);
        }
        let reach = |first: &str, then: &str| {
            let mut w = w.clone();
            for t in [first, then] {
                w.set_research(tech(&defs, t), &defs).unwrap();
                w.tick(&defs);
            }
            let order: Vec<_> = w.research().completions().iter().map(|c| c.tech).collect();
            (order, w)
        };
        let (order_a, a) = reach("masonry", "earth_shaping");
        let (order_b, b) = reach("earth_shaping", "masonry");
        assert_ne!(order_a, order_b, "the two runs should complete techs in different orders");
        assert!(a.research().known_techs().eq(b.research().known_techs()));
        assert_eq!(bits(a.modifiers()), bits(b.modifiers()));
        // And both match a recomputation from the known set alone.
        let fresh = research::effective_modifiers(Nation::Earth, a.research().known_techs(), &defs);
        assert_eq!(bits(a.modifiers()), bits(&fresh));
    }

    #[test]
    fn units_without_food_die_and_are_removed_deterministically() {
        let run = || {
            let (mut w, defs) = world(Nation::Earth);
            // No wild food anywhere: the units can drink but never eat.
            let food: Vec<Pos> = w
                .map
                .iter()
                .filter(|(_, t)| {
                    t.resource.is_some_and(|r| {
                        defs.harvest(r).is_some_and(|h| defs.food_of(h.item).is_some())
                    })
                })
                .map(|(p, _)| p)
                .collect();
            for p in food {
                w.map[p].resource = None;
            }
            w.map_changed();
            let mut ticks = 0;
            while !w.units.is_empty() {
                w.tick(&defs);
                ticks += 1;
                assert!(ticks < 100_000, "units never died");
            }
            w
        };
        let (a, b) = (run(), run());
        assert_eq!(a.deaths.len(), 4);
        assert_eq!(a.deaths, b.deaths);
        assert!(a.deaths.windows(2).all(|w| w[0].tick <= w[1].tick));
    }

    #[test]
    fn dead_units_leave_lookup_and_the_rest_carry_on() {
        let (mut w, defs) = world(Nation::Fire);
        // Make only unit 1 fatally ill; the others are fine.
        w.units[1].needs.hunger = 0.0;
        w.units[1].needs.health = 0.01;
        w.tick(&defs);
        assert_eq!(w.units.len(), 3);
        assert!(w.unit(UnitId(1)).is_none());
        assert_eq!(w.unit(UnitId(2)).map(|u| u.id), Some(UnitId(2)));
        assert_eq!(w.deaths.len(), 1);
        assert_eq!(w.deaths[0].id, UnitId(1));
        assert_eq!(w.deaths[0].cause, DeathCause::Starvation);
    }
}
