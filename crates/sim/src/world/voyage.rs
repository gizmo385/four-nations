//! The Airship Dock's voyages (`[building.airship]`, #758, docs/design/horizons-phase-6.md,
//! "`airships`"): outbound trade on the player's own schedule, through the caravan deal lists
//! (#437, `data/trade.toml`).
//!
//! - **Planning.** `Command::PlanVoyage { dock, nation, deals }` picks one nation (the
//!   settlement's own included: kin) and up to the dock's `deals` of that nation's caravan deals,
//!   each up to its `times`: the United City's scaled set once the gate stands, fixed when the
//!   voyage is planned (`Plan::united`). The give side of all of them must fit the dock's `hold`.
//!   The plan lives on the dock's building record (`Building::voyage`), its deals in file order,
//!   not the order they were named in. `Command::CancelVoyage` drops it before it sails.
//! - **Loading.** Each deal is a slot, and `tick_voyages` keeps one `JobKind::Load` open per slot
//!   that still needs goods, once a second: a settler fetches up to a load from stores and tips
//!   it into the slot (`run_load`, the caravan camp's `run_trade` pattern). The goods are held on
//!   the plan until the airship sails; cancelling or taking the dock down drops them beside the
//!   door.
//! - **Crewing.** Once every slot is loaded and the bunker holds `fuel_per_voyage` (the usual
//!   Refuel haul fills it), `crew` `JobKind::Crew` jobs open, for settlers of any nation. A unit
//!   holding one walks to the door and waits there. When all the crew are standing on the door
//!   and none is off to eat or sleep, the airship sails: the fuel burns, the crew leave
//!   `World::units` for `World::voyages` and the goods leave the map.
//! - **Away.** `World::voyages` holds each airship that is out: its crew (in id order, their
//!   needs frozen because nothing ticks them: they can't die or leave), what it brings back and
//!   the landing tick, fixed at departure. Voyages are in dock order, one per dock. The crew
//!   still count as residents: the roster, `World::population`, `World::settlers` and the
//!   newcomer and celestial counts read them through `World::residents`. The invariants walk
//!   both lists (`check_voyages`), and a unit id is in one of them only.
//! - **Home.** On the landing tick the crew step off at the door and the get side spills beside
//!   the dock as ground stacks with ordinary Haul jobs, as at a caravan camp. Bought goods count
//!   toward the lifetime tally like a camp's (#834), and knowledge is added. If the dock is gone
//!   by then, the door tile it had stands for it.
//! - **Taken down.** Refused while the airship is away (`DesignateError::AirshipAway`). A dock
//!   taken down with a plan returns what was loaded (`Building::dock_goods`).
//! - **No randomness.** Nothing here draws: the deals are the list's own, and the landing tick
//!   is a function of the departure tick.

use serde::{Deserialize, Serialize};

use crate::building::{AirshipDef, MAX_VOYAGE_DEALS};
use crate::jobs::{ItemId, JobKind, Stack};
use crate::nation::Nation;
use crate::trade::{DealDef, Goods};

use super::buildings::Kind;
use super::work::{GROUND_SEARCH_RADIUS, route_from};
use super::*;

/// A voyage being loaded at a dock: where it is going and what it carries out.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Plan {
    /// Whose traders it sails to.
    pub nation: Nation,
    /// Whether the deals are the United City's scaled set (fixed when it was planned).
    #[serde(default)]
    pub united: bool,
    /// The deals it takes, in the caravan's file order.
    pub deals: Vec<PlannedDeal>,
}

/// One deal of a voyage and how much of its give side is on the dock.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlannedDeal {
    /// Its index in the caravan's `deals` (`CaravanDef::deals`).
    pub deal: u8,
    /// How many times it is taken.
    pub times: u8,
    /// Goods handed over toward the times.
    pub held: Vec<Stack>,
}

/// An airship that is out: its crew, and what it brings back.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Away {
    /// The anchor of the dock it sailed from.
    pub dock: Pos,
    /// The dock's door when it sailed: where it lands.
    pub door: Pos,
    /// Whose traders it went to.
    pub nation: Nation,
    pub sailed: u64,
    /// The tick it lands.
    pub lands: u64,
    /// The residents away, in id order. Their state is frozen as they left it.
    pub crew: Vec<Unit>,
    /// What it brings back: stacks for the door.
    pub back: Vec<Stack>,
    /// Research points it brings back (Air's knowledge).
    pub knowledge: u32,
}

/// What an Airship Dock is doing, for its cursor panel and the Airship screen (#763).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VoyageStatus {
    /// Nothing planned.
    Docked,
    /// Loading: `held` of the `out` items are on the dock.
    Loading { held: u32, out: u32 },
    /// Loaded, waiting for fuel in the bunker.
    NeedsFuel { have: u32, need: u32 },
    /// Loaded and fuelled: `aboard` of the `crew` are standing at the door.
    Boarding { aboard: u8, crew: u8 },
    /// Away, landing in this many ticks.
    Away { ticks: u64 },
}

/// Why a voyage command was refused.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VoyageError {
    /// No Airship Dock is anchored there.
    NoDock,
    /// The airship is away.
    Away,
    /// A voyage is already planned: cancel it first.
    Planned,
    /// No voyage is planned there to cancel.
    NothingPlanned,
    /// That nation has no traders to sail to (no caravan data).
    NoCaravan,
    /// The plan names no deal.
    NoDeals,
    /// The plan names a deal the caravan doesn't have.
    NoSuchDeal,
    /// The plan names a deal twice.
    Duplicate,
    /// The plan has more deals than the dock takes.
    TooManyDeals { most: u8 },
    /// A deal is taken more times than it allows.
    TooManyTimes { most: u32 },
    /// The goods to carry out don't fit the hold.
    TooBig { out: u32, hold: u32 },
}

impl std::fmt::Display for VoyageError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NoDock => f.write_str("no Airship Dock here"),
            Self::Away => f.write_str("The airship is away"),
            Self::Planned => f.write_str("a voyage is already planned"),
            Self::NothingPlanned => f.write_str("no voyage is planned"),
            Self::NoCaravan => f.write_str("no traders to sail to"),
            Self::NoDeals => f.write_str("pick a deal to sail with"),
            Self::NoSuchDeal => f.write_str("no such deal"),
            Self::Duplicate => f.write_str("a deal can only be picked once"),
            Self::TooManyDeals { most } => write!(f, "at most {most} deals a voyage"),
            Self::TooManyTimes { most } => write!(f, "that deal is at most {most} times"),
            Self::TooBig { out, hold } => write!(f, "{out} items won't fit a hold of {hold}"),
        }
    }
}

/// How many of `goods` the stacks `held` cover.
fn held_of(held: &[Stack], goods: &Goods) -> u32 {
    held.iter().filter(|s| goods.accepts(s.item)).map(|s| s.count).sum()
}

/// The items a deal taken `times` times asks for on the give side.
fn give_count(deal: &DealDef, times: u8) -> u32 {
    deal.give.iter().map(|g| g.count).sum::<u32>() * u32::from(times)
}

impl buildings::Building {
    /// Everything a voyage had loaded here that a taken-down dock gives back.
    pub(super) fn voyage_goods(&self) -> Vec<Stack> {
        let Some(plan) = &self.voyage else { return Vec::new() };
        plan.deals.iter().flat_map(|d| d.held.iter().copied()).collect()
    }
}

impl World {
    /// The settlement's own people, the crew away on a voyage included (`World::voyages`): what
    /// every headcount that is about residents reads, so a voyage never changes it.
    pub fn residents(&self) -> impl Iterator<Item = &Unit> + '_ {
        self.units
            .iter()
            .chain(self.voyages.iter().flat_map(|v| v.crew.iter()))
            .filter(|u| u.is_settler())
    }

    /// Every airship that is away, in dock order.
    pub fn voyages(&self) -> &[Away] {
        &self.voyages
    }

    /// The airship away from the dock anchored on `dock`, if it is out.
    pub fn voyage_away(&self, dock: Pos) -> Option<&Away> {
        self.voyages.iter().find(|v| v.dock == dock)
    }

    /// The voyage being loaded at the dock anchored on `dock`.
    pub fn voyage_plan(&self, dock: Pos) -> Option<&Plan> {
        self.anchored(dock)?.voyage.as_ref()
    }

    fn airship_def(&self, dock: Pos, defs: &Defs) -> Option<AirshipDef> {
        defs.building(self.anchored(dock)?.id).airship
    }

    /// The door tile of the finished building anchored on `dock`.
    fn dock_door(&self, dock: Pos) -> Option<Pos> {
        Some(self.anchored(dock)?.footprint().entrance())
    }

    /// What the Airship Dock anchored on `dock` is doing, or `None` off an airship dock.
    pub fn voyage_status(&self, dock: Pos, defs: &Defs) -> Option<VoyageStatus> {
        let def = self.airship_def(dock, defs)?;
        if let Some(away) = self.voyage_away(dock) {
            return Some(VoyageStatus::Away { ticks: away.lands.saturating_sub(self.tick) });
        }
        let Some(plan) = self.voyage_plan(dock) else { return Some(VoyageStatus::Docked) };
        if !self.voyage_loaded(dock, defs) {
            let caravan = defs.trade.caravan(plan.nation, plan.united);
            let out = plan
                .deals
                .iter()
                .filter_map(|p| Some(give_count(caravan?.deals.get(usize::from(p.deal))?, p.times)))
                .sum();
            let held = plan.deals.iter().flat_map(|p| &p.held).map(|s| s.count).sum();
            return Some(VoyageStatus::Loading { held, out });
        }
        let have = self.anchored(dock)?.bunker().map_or(0, |b| b.count);
        if have < def.fuel_per_voyage {
            return Some(VoyageStatus::NeedsFuel { have, need: def.fuel_per_voyage });
        }
        Some(VoyageStatus::Boarding { aboard: self.crew_aboard(dock).len() as u8, crew: def.crew })
    }

    /// `Command::PlanVoyage`.
    pub(super) fn plan_voyage(
        &mut self,
        dock: Pos,
        nation: Nation,
        deals: [(u8, u8); MAX_VOYAGE_DEALS],
        defs: &Defs,
    ) -> Result<(), VoyageError> {
        let def = self.airship_def(dock, defs).ok_or(VoyageError::NoDock)?;
        if self.voyage_away(dock).is_some() {
            return Err(VoyageError::Away);
        }
        if self.voyage_plan(dock).is_some() {
            return Err(VoyageError::Planned);
        }
        let united = self.united();
        let caravan = defs.trade.caravan(nation, united).ok_or(VoyageError::NoCaravan)?;
        let mut picked: Vec<PlannedDeal> = Vec::new();
        let mut out = 0;
        for &(deal, times) in deals.iter().filter(|&&(_, times)| times > 0) {
            let d = caravan.deals.get(usize::from(deal)).ok_or(VoyageError::NoSuchDeal)?;
            if u32::from(times) > d.times {
                return Err(VoyageError::TooManyTimes { most: d.times });
            }
            if picked.iter().any(|p| p.deal == deal) {
                return Err(VoyageError::Duplicate);
            }
            out += give_count(d, times);
            picked.push(PlannedDeal { deal, times, held: Vec::new() });
        }
        if picked.is_empty() {
            return Err(VoyageError::NoDeals);
        }
        if picked.len() > usize::from(def.deals) {
            return Err(VoyageError::TooManyDeals { most: def.deals });
        }
        if out > def.hold {
            return Err(VoyageError::TooBig { out, hold: def.hold });
        }
        picked.sort_by_key(|p| p.deal);
        let b = self.anchored_mut(dock).expect("found above");
        b.voyage = Some(Plan { nation, united, deals: picked });
        self.open_voyage_jobs(dock, defs);
        Ok(())
    }

    /// `Command::CancelVoyage`: the plan goes, what was loaded drops by the door and the dock's
    /// Load and Crew jobs go with it.
    pub(super) fn cancel_voyage(&mut self, dock: Pos, defs: &Defs) -> Result<(), VoyageError> {
        self.airship_def(dock, defs).ok_or(VoyageError::NoDock)?;
        if self.voyage_away(dock).is_some() {
            return Err(VoyageError::Away);
        }
        let door = self.dock_door(dock).expect("found above");
        let plan = self
            .anchored_mut(dock)
            .and_then(|b| b.voyage.take())
            .ok_or(VoyageError::NothingPlanned)?;
        for stack in plan.deals.into_iter().flat_map(|d| d.held) {
            self.place_ground(door, stack);
        }
        self.drop_voyage_jobs(dock);
        Ok(())
    }

    /// Removes the Load and Crew jobs of the dock anchored on `dock`; a unit holding one lets go
    /// of it (anything in hand goes back to stores, `run_idle`).
    fn drop_voyage_jobs(&mut self, dock: Pos) {
        let at_dock = |kind: JobKind| match kind {
            JobKind::Load { pos, .. } | JobKind::Crew { pos } => pos == dock,
            _ => false,
        };
        let gone: Vec<JobId> = self.jobs.iter().filter(|j| at_dock(j.kind)).map(|j| j.id).collect();
        for i in 0..self.units.len() {
            if matches!(self.units[i].job_task, Some(Task::Load(id) | Task::Crew(id)) if gone.contains(&id))
            {
                self.finish_job(i);
            }
        }
        self.jobs.retain(|j| !gone.contains(&j.id));
    }

    /// The goods slot `slot` of the voyage at `dock` still needs, each with how many.
    fn voyage_needs<'d>(
        &self,
        dock: Pos,
        slot: u8,
        defs: &'d Defs,
    ) -> Option<Vec<(&'d Goods, u32)>> {
        let plan = self.voyage_plan(dock)?;
        let entry = plan.deals.get(usize::from(slot))?;
        let deal =
            defs.trade.caravan(plan.nation, plan.united)?.deals.get(usize::from(entry.deal))?;
        Some(
            deal.give
                .iter()
                .map(|g| {
                    let want = g.count * u32::from(entry.times);
                    (g, want.saturating_sub(held_of(&entry.held, g)))
                })
                .collect(),
        )
    }

    /// Whether a `Load` job for `slot` at `dock` is still wanted: the voyage is planned and the
    /// slot still needs goods.
    pub(super) fn voyage_load_wanted(&self, dock: Pos, slot: u8, defs: &Defs) -> bool {
        self.voyage_needs(dock, slot, defs).is_some_and(|n| n.iter().any(|&(_, c)| c > 0))
    }

    /// The first item slot `slot` still needs that some store holds, with how many: what a `Load`
    /// job fetches next. Goods in data order, then items in id order.
    pub(super) fn voyage_source(&self, dock: Pos, slot: u8, defs: &Defs) -> Option<(ItemId, u32)> {
        let needs = self.voyage_needs(dock, slot, defs)?;
        needs.into_iter().filter(|&(_, n)| n > 0).find_map(|(g, n)| {
            g.items.iter().copied().find(|&i| self.any_source_for_fuel(i)).map(|i| (i, n))
        })
    }

    /// Whether the voyage planned at `dock` has every deal's goods on the dock.
    fn voyage_loaded(&self, dock: Pos, defs: &Defs) -> bool {
        let Some(plan) = self.voyage_plan(dock) else { return false };
        (0..plan.deals.len() as u8).all(|slot| !self.voyage_load_wanted(dock, slot, defs))
    }

    /// Whether the voyage at `dock` is loaded and the bunker holds the fuel: a Crew job is
    /// wanted (and the airship may sail).
    pub(super) fn crew_wanted(&self, dock: Pos, defs: &Defs) -> bool {
        let Some(def) = self.airship_def(dock, defs) else { return false };
        self.voyage_loaded(dock, defs)
            && self
                .anchored(dock)
                .and_then(|b| b.bunker())
                .is_some_and(|b| b.count >= def.fuel_per_voyage)
    }

    /// Indexes into `units`, in id order, of the settlers holding a Crew job at `dock` that are
    /// standing on its door and not off to meet a need.
    fn crew_aboard(&self, dock: Pos) -> Vec<usize> {
        let Some(door) = self.dock_door(dock) else { return Vec::new() };
        let mut aboard: Vec<usize> = self
            .jobs
            .iter()
            .filter(|j| j.progress >= 1.0 && j.kind == JobKind::Crew { pos: dock })
            .filter_map(|j| self.units.binary_search_by_key(&j.claimed_by?, |u| u.id).ok())
            .filter(|&i| {
                let u = &self.units[i];
                u.need_task.is_none() && u.inside.is_none() && u.pos == door
            })
            .collect();
        aboard.sort_unstable();
        aboard
    }

    /// Opens, for the dock at `dock`, a Load job for every slot that needs goods and has none,
    /// and (once loaded and fuelled) a Crew job for every place in the crew not yet open.
    fn open_voyage_jobs(&mut self, dock: Pos, defs: &Defs) {
        let Some(def) = self.airship_def(dock, defs) else { return };
        let Some(plan) = self.voyage_plan(dock) else { return };
        for slot in 0..plan.deals.len() as u8 {
            let kind = JobKind::Load { pos: dock, slot };
            if self.voyage_load_wanted(dock, slot, defs)
                && !self.jobs.iter().any(|j| j.kind == kind)
            {
                self.add_job(kind);
            }
        }
        if self.crew_wanted(dock, defs) {
            let open = self.jobs.iter().filter(|j| j.kind == JobKind::Crew { pos: dock }).count();
            for _ in open..usize::from(def.crew) {
                self.add_job(JobKind::Crew { pos: dock });
            }
        }
    }

    /// Lands every airship whose time has come, then (once a second) keeps each dock's jobs
    /// open and sails any dock whose crew is all aboard.
    pub(super) fn tick_voyages(&mut self, defs: &Defs) {
        while let Some(k) = self.voyages.iter().position(|v| v.lands <= self.tick) {
            self.land(k);
        }
        if !self.has_kind(Kind::Airship) {
            return;
        }
        let docks: Vec<Pos> = self.of_kind(Kind::Airship).map(|b| b.anchor).collect();
        let on_second = self.tick.is_multiple_of(u64::from(TICKS_PER_SECOND));
        for dock in docks {
            if self.voyage_plan(dock).is_none() {
                continue;
            }
            if on_second {
                self.open_voyage_jobs(dock, defs);
            }
            self.try_sail(dock, defs);
        }
    }

    /// Sails the voyage at `dock` if it is loaded, fuelled and the whole crew stands on the
    /// door.
    fn try_sail(&mut self, dock: Pos, defs: &Defs) {
        let Some(def) = self.airship_def(dock, defs) else { return };
        if !self.crew_wanted(dock, defs) {
            return;
        }
        let aboard = self.crew_aboard(dock);
        if aboard.len() < usize::from(def.crew) {
            return;
        }
        let door = self.dock_door(dock).expect("a dock with a plan stands");
        let (plan, nation) = {
            let b = self.anchored_mut(dock).expect("a dock with a plan stands");
            let plan = b.voyage.take().expect("checked by crew_wanted");
            if let Some(bunker) = b.fuel.as_mut() {
                bunker.count -= def.fuel_per_voyage;
                if bunker.count == 0 {
                    bunker.burned = 0;
                }
            }
            let nation = plan.nation;
            (plan, nation)
        };
        // What it brings back: each deal's get side, as many times as it was taken, one stack per
        // item in the order first named, and the knowledge in points.
        let caravan = defs.trade.caravan(plan.nation, plan.united).expect("planned against it");
        let mut back: Vec<Stack> = Vec::new();
        let mut knowledge = 0;
        for p in &plan.deals {
            let deal = &caravan.deals[usize::from(p.deal)];
            for &(item, count) in &deal.get {
                let count = count * u32::from(p.times);
                match back.iter_mut().find(|s| s.item == item) {
                    Some(s) => s.count += count,
                    None => back.push(Stack { item, count }),
                }
            }
            knowledge += deal.knowledge * u32::from(p.times);
        }
        // The crew leave the map, last index first so the rest keep theirs, then back in id order.
        let taken: Vec<JobId> = self
            .jobs
            .iter()
            .filter(|j| j.kind == JobKind::Crew { pos: dock })
            .map(|j| j.id)
            .collect();
        let mut crew: Vec<Unit> = Vec::new();
        for &i in aboard.iter().take(usize::from(def.crew)).rev() {
            if let Some(stack) = self.units[i].carrying.take() {
                self.place_ground(door, stack);
            }
            let mut unit = self.units.remove(i);
            unit.job_task = None;
            unit.route = None;
            unit.move_progress = 0.0;
            self.claim_memo.remove(&unit.id);
            crew.push(unit);
        }
        crew.reverse();
        self.jobs.retain(|j| !taken.contains(&j.id));
        // Anyone else still holding one of those jobs lets go of it.
        for i in 0..self.units.len() {
            if matches!(self.units[i].job_task, Some(Task::Crew(id)) if taken.contains(&id)) {
                self.finish_job(i);
            }
        }
        let away = Away {
            dock,
            door,
            nation,
            sailed: self.tick,
            lands: self.tick + def.away_ticks,
            crew,
            back,
            knowledge,
        };
        let at = self.voyages.partition_point(|v| v.dock < dock);
        self.voyages.insert(at, away);
    }

    /// Lands the airship `voyages[k]`: the crew step off at the door and the goods spill beside it.
    fn land(&mut self, k: usize) {
        let away = self.voyages.remove(k);
        let at = self.landing_tile(away.door);
        for mut unit in away.crew {
            unit.pos = at;
            unit.claim_retry_at = self.tick;
            let i = self.units.partition_point(|u| u.id < unit.id);
            self.units.insert(i, unit);
        }
        for stack in away.back {
            self.place_ground(away.door, stack);
            // Goods bought count toward the lifetime tally like mined ones, as at a camp (#834).
            self.tallies.count_harvest(stack.item, stack.count);
        }
        if away.knowledge > 0 {
            self.research.add_knowledge(u64::from(away.knowledge) * 1000);
        }
    }

    /// Where the crew step off: the door, or if something now stands there, the nearest tile
    /// units can walk onto.
    fn landing_tile(&self, door: Pos) -> Pos {
        let walkable = |p: Pos| self.map.get(p).is_some_and(|t| self.rules.passable(t));
        if walkable(door) {
            return door;
        }
        for r in 1..=GROUND_SEARCH_RADIUS {
            for dy in -r..=r {
                for dx in -r..=r {
                    let p = Pos::new(door.x + dx, door.y + dy);
                    if dx.abs().max(dy.abs()) == r && walkable(p) {
                        return p;
                    }
                }
            }
        }
        door
    }

    /// One tick of a unit carrying goods to the dock for slot `slot`: fetch up to a load of the
    /// first good still needed from the nearest store, carry it to the door and hand it over. One
    /// trip finishes the job; `tick_voyages` opens another while the slot still needs more.
    pub(super) fn run_load(&mut self, i: usize, id: JobId, defs: &Defs) {
        let Some(idx) = self.job_index(id) else { return self.finish_job(i) };
        let JobKind::Load { pos, slot } = self.jobs[idx].kind else { return self.finish_job(i) };
        if !self.voyage_load_wanted(pos, slot, defs) {
            // Cancelled, or loaded some other way: anything in hand goes back to stores.
            self.jobs.remove(idx);
            return self.finish_job(i);
        }
        let needs = self.voyage_needs(pos, slot, defs).expect("wanted, so planned");
        let carried = self.units[i].carrying;
        if let Some(stack) = carried
            && let Some(&(_, need)) = needs.iter().find(|(g, n)| *n > 0 && g.accepts(stack.item))
        {
            if !self.go_or_abandon(i, id, StandRule::Entrance(pos), defs) {
                return;
            }
            let put = stack.count.min(need);
            let b = self.anchored_mut(pos).expect("wanted, so here");
            let entry =
                &mut b.voyage.as_mut().expect("wanted, so planned").deals[usize::from(slot)];
            match entry.held.iter_mut().find(|s| s.item == stack.item) {
                Some(s) => s.count += put,
                None => entry.held.push(Stack { item: stack.item, count: put }),
            }
            let left = stack.count - put;
            self.units[i].carrying = (left > 0).then_some(Stack { item: stack.item, count: left });
            self.units[i].route = None;
            let idx = self.job_index(id).expect("still queued");
            self.jobs.remove(idx);
            return self.finish_job(i);
        }
        if carried.is_some() {
            // Hands full of something the dock doesn't take: done for now.
            self.jobs.remove(idx);
            return self.finish_job(i);
        }
        let Some((item, need)) = self.voyage_source(pos, slot, defs) else {
            return self.abandon_job(i, id, defs);
        };
        if self.keep_walking(i, defs) {
            return;
        }
        let flood = self.flood_from(i);
        let Some((source, stand)) = self.nearest_source(&flood, item) else {
            return self.abandon_job(i, id, defs);
        };
        if self.units[i].route.is_none() {
            match flood.path_to(stand) {
                Some(path) => self.units[i].route = Some(route_from(path)),
                None => return self.abandon_job(i, id, defs),
            }
        }
        if !self.walk(i, defs) {
            return;
        }
        let take = self.take_for_build(i, source, need, defs);
        if take == 0 {
            return self.abandon_job(i, id, defs);
        }
        self.units[i].carrying = Some(Stack { item, count: take });
        self.units[i].route = None;
    }

    /// One tick of a unit holding a crew place: walk to the door and wait there. The airship
    /// sails (`try_sail`) once the whole crew stands on it. `progress` 1.0 marks it arrived.
    pub(super) fn run_crew(&mut self, i: usize, id: JobId, defs: &Defs) {
        let Some(idx) = self.job_index(id) else { return self.finish_job(i) };
        let JobKind::Crew { pos } = self.jobs[idx].kind else { return self.finish_job(i) };
        if !self.crew_wanted(pos, defs) {
            self.jobs[idx].claimed_by = None;
            self.jobs[idx].progress = 0.0;
            return self.finish_job(i);
        }
        if self.go_or_abandon(i, id, StandRule::Entrance(pos), defs) {
            self.jobs[idx].progress = 1.0;
        }
    }

    /// Whether a Crew job at `dock` is still wanted, for the job queue's pruning.
    pub(super) fn crew_job_live(&self, dock: Pos, defs: &Defs) -> bool {
        self.crew_wanted(dock, defs)
    }

    /// Checks the voyages (`check_invariants`): in dock order with one per dock, each with its
    /// crew in id order, settlers all, holding no job or need, and with ids no living unit has.
    pub(super) fn check_voyages(&self) -> Result<(), String> {
        for pair in self.voyages.windows(2) {
            if pair[0].dock >= pair[1].dock {
                return Err(format!(
                    "voyages not in dock order: {:?} before {:?}",
                    pair[0].dock, pair[1].dock
                ));
            }
        }
        for v in &self.voyages {
            if v.crew.is_empty() {
                return Err(format!("the airship from {:?} is away with no crew", v.dock));
            }
            for pair in v.crew.windows(2) {
                if pair[0].id >= pair[1].id {
                    return Err(format!(
                        "crew not sorted by id: {:?} before {:?}",
                        pair[0].id, pair[1].id
                    ));
                }
            }
            for u in &v.crew {
                if !u.is_settler() {
                    return Err(format!("{:?} is away on a voyage but is no settler", u.id));
                }
                if self.units.iter().any(|o| o.id == u.id) {
                    return Err(format!("{:?} is both on the map and away on a voyage", u.id));
                }
                if u.job_task.is_some() || u.need_task.is_some() || u.inside.is_some() {
                    return Err(format!("{:?} is away on a voyage holding a task", u.id));
                }
            }
            if v.lands <= v.sailed {
                return Err(format!("the airship from {:?} lands before it sails", v.dock));
            }
        }
        let mut ids: Vec<UnitId> =
            self.voyages.iter().flat_map(|v| v.crew.iter().map(|u| u.id)).collect();
        ids.sort_unstable();
        if ids.windows(2).any(|w| w[0] == w[1]) {
            return Err("a unit is away on two voyages".into());
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests;
