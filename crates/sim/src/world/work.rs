//! What units do all day: the job queue, claiming, needs pre-empting jobs, and hauling.
//!
//! The shape, which follows the 2013 version's task queue:
//!
//! * The player **designates** work (`Command::DesignateHarvest`) and a stockpile zone
//!   (`Command::DesignateStockpile`). Each harvest designation becomes a `Job` in the queue.
//! * An **idle unit claims the nearest reachable job** (by path cost, ties by job id). It walks
//!   to a tile beside the target, works, and the yield drops on the ground as an item stack.
//!   Every stack lying outside a stockpile gets a **haul job**; a unit picks up as much as it can
//!   carry (`carry_capacity` modifier) and walks it to the nearest stockpile tile with room.
//! * **Needs pre-empt jobs.** When hunger, thirst or energy drops to its `low` level (see
//!   `data/needs.toml`) the unit sets its job aside, deals with the need, and goes back. The job
//!   stays claimed by that unit and keeps its progress, so "resume" is literal.
//! * **Work speed** is `1.0 / TICKS_PER_SECOND` work-seconds per tick times the modifier named by
//!   the harvest in `data/jobs.toml` (quarrying uses `earthworks`), read from the working unit's
//!   own effective modifiers (`World::modifiers_for`, #239), not necessarily the settlement's.
//!
//! **Renewable food** (#56): a harvest with a `regrow` time in `data/jobs.toml` (cabbages, fruit
//! trees, fishing spots) is not cleared when worked. The resource stays on the map, *resting*,
//! and `tick_growth` counts down the seconds it needs to ripen; until then it can't be worked,
//! foraged, or claimed. The rate is `World::growth_rate`, the one place something like wet
//! soil (docs/design/bending.md, "Water the Land") plugs in to make plants grow faster, and
//! where the difficulty preset's seasons slow or stop it in winter (`world/season.rs`, #295).
//!
//! **Standing harvest orders** (#81): `Command::DesignateRepeatingHarvest` marks a tile so that
//! every time its (renewable) resource ripens again, `tick_growth` queues a fresh `Harvest` job
//! for it without the player lifting a finger. The mark (`World::standing_harvest`) lives
//! independently of the `Job`: the job itself still disappears on completion like any one-shot
//! harvest, and reappears only once the resource is ripe again. `CancelHarvest` clears both the
//! job and the mark.
//!
//! **Bending** (`bending.rs`) rides on the same queue: a `JobKind::Bend` job is claimed by the
//! nearest unit whose chi covers the cost, pre-empted by needs like any job, and completes by
//! spending chi.
//!
//! **Study** (`study.rs`, #21) also rides on the same queue, but a seat never completes: it
//! produces knowledge for as long as someone holds it, and pre-emption just pauses that.
//!
//! **Attend** (`gate.rs`, #238), the Harmony Rite's own low-priority job, works the same way as a
//! seat: a unit claims one only with nothing more urgent reachable, and it never completes on its
//! own. The Avatar's own place in the rite (`Task::Rite`) is never a `Job` at all: `run_idle`
//! assigns it to him directly, and `consider_needs` never lets a need pre-empt it.
//!
//! Hungry units eat from the closest of: food they carry, food in a stockpile, food lying on the
//! ground, or a wild food resource they work for themselves. Raw food always works (owner rule,
//! #3): nobody starves because their nation lacks fire. A meal eaten within reach of a lit heat
//! source that cooks (a campfire, or Fire's Hearthflame, `heat.rs`'s `is_cooking`) is cooked and
//! restores the food's `cooked_hunger`; anywhere else -- including within reach of a shelter
//! like Earth's Earth Tent or Air's Air Shield, which keeps the cold away but cooks nothing
//! (#362) -- it is raw.
//!
//! **Sleep** (#58): an exhausted unit walks to the nearest free bed (`BuildingDef::bed`, within
//! its `seek_tiles`) and sleeps there, regaining energy faster. A bed holds one sleeper: the
//! unit's `bed` field claims it from the moment it sets off. With no bed in reach it goes to a
//! warm place if a heat source is near, else it sleeps on the ground where it stands.
//!
//! Everything is deterministic: units act in id order, searches break ties by position or id, and
//! the only randomness is the idle wander and pause (`world.units` stream).

use std::collections::HashMap;

use crate::defs::Terrain;
use crate::endeavor::TaskWho;
use crate::flood::Flood;
use crate::geom::Footprint;
use crate::harm::RemedyDef;
use crate::jobs::{HarvestDef, ItemId, JobKind};
use crate::path::{DIAGONAL, MoveRules, STRAIGHT};
use crate::research::MILLIS_PER_POINT;

use super::*;

use eligibility::{Bar, Check};
pub(super) use eligibility::{ClaimKey, IdleMemo, StandRule};

/// `claim_job`'s search result: the job taken (if any) and the route to it.
pub(super) type Picked = (Option<JobId>, Option<Vec<Pos>>);

/// How far from a drop point a stack may be placed when its own tile is taken, in tiles.
const GROUND_SEARCH_RADIUS: i32 = 8;

/// Where a hungry unit gets its food.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum FoodSource {
    /// Something it is already carrying.
    Carried,
    /// A stockpile tile.
    Stockpile(Pos),
    /// An item stack on the ground.
    Ground(Pos),
    /// A wild food resource on this tile, worked on the spot.
    Forage(Pos),
    /// The pantry of the outpost anchored on this tile, eaten from at its door
    /// (`world/outpost.rs`).
    Pantry(Pos),
}

/// What a unit is doing. Needs outrank jobs; `Unit::task` reports the one in effect.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum Task {
    /// Working a designated harvest.
    Harvest(JobId),
    /// Walking to pick up a stack for a haul job.
    Haul(JobId),
    /// Channeling a designated bend.
    Bend(JobId),
    /// Fetching a building's cost or working the site (`run_build` tells which from the site's
    /// state; the task itself doesn't need to know).
    Build(JobId),
    /// Holding a study hall's seat (`world/study.rs`, #21).
    Study(JobId),
    /// Doing one of the endeavor gate's tasks (`world/gate.rs`, #236).
    Endeavor(JobId),
    /// Holding a tile of the plaza during the Harmony Rite (`world/gate.rs`, #238).
    Attend(JobId),
    /// The Avatar's own place in the Harmony Rite: meditating on the plaza
    /// (`world/gate.rs`, #238). Never backed by a `Job`, so needs never pre-empt it once he
    /// holds it (`consider_needs`); only he is ever assigned it.
    Rite,
    /// Holding a production building's craft job (`world/forge.rs`, #124).
    Craft(JobId),
    /// Planting an empty farm plot (`world/farm.rs`, #198).
    Sow(JobId),
    /// Gathering a farm plot's ripe crop (`world/farm.rs`, #198).
    Reap(JobId),
    /// Uprooting or chopping down the resource on a tile for good
    /// (`docs/design/clearing.md`, issue #411/#424).
    Clear(JobId),
    /// Holding a Map Room's seat (`world/chart.rs`, #402).
    Chart(JobId),
    /// Taking a finished building down (`world/deconstruct.rs`, #342).
    Deconstruct(JobId),
    /// Carrying fuel to a building's bunker (`world/fuel.rs`).
    Refuel(JobId),
    /// Carrying goods to a caravan's camp (`world/trade.rs`, #437).
    Trade(JobId),
    /// Carrying a stack to a stockpile tile.
    Deliver,
    /// Carrying a stack to the chute anchored on `Pos`, to drop it down to the foot
    /// (`world/chute.rs`). Bound for stores, the way `Deliver` is.
    Chute(Pos),
    /// Carrying a stack to the Balloon Dock anchored on `Pos`, to wait in its hold
    /// (`world/balloon.rs`). Bound for stores, the way `Deliver` is.
    Balloon(Pos),
    Eat(FoodSource),
    Drink,
    Sleep,
    /// Resting at a healing building's anchor (`BuildingDef::healing`, issue #375), walking to
    /// one or inside it.
    Healing(Pos),
    /// Bathing at a bath's anchor (`BuildingDef::bath`, the Bathhouse, #741), walking to one or
    /// inside it.
    Bath(Pos),
    /// Fetching a remedy (herbs, to start) from the stockpile tile `Pos` and taking it like a
    /// meal (`data/harm.toml`'s `[[remedy]]`, issue #390, `World::plan_remedy`/`take_remedy`).
    Remedy(Pos),
}

/// Why a unit with nothing to do (`Unit::task` is `None`) has nothing to do (#337): read from
/// the same facts `World::idle_reason` already asks `World::check_job` (`eligibility.rs`) of
/// every unclaimed job, instead of a second copy of the assignment rules. A frontend falls back
/// to a bare "Idle" for `None`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IdleReason {
    /// Nothing at all is designated for anyone to do.
    NoWork,
    /// Something is designated, but none of it is on this unit's side of a river, wall, shore or
    /// door -- or every bit of it is a kind this unit never does at all (the Avatar on an
    /// ordinary job).
    Unreachable,
    /// The nearest open work this unit could do is a bend past its current chi.
    OutOfChi,
    /// The nearest open work this unit could do is hauling an item that every stockpile able to
    /// take it is already full of.
    StockpileFull,
    /// The nearest open work this unit could do is a harvest marked `risk`, and its own health
    /// is below `data/harm.toml`'s `[settings] fit_for_heavy` (`World::fit_for_heavy_work`,
    /// issue #388).
    TooHurt,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DesignateError {
    OutOfBounds,
    /// There is no resource with a harvest entry on that tile.
    NothingToHarvest,
    /// The resource needs a tech the nation doesn't know yet (fishing, say). The client can look
    /// up its name for a reason like "Requires Fishing" (issue #22).
    NotResearched(TechId),
    AlreadyDesignated,
    /// There is no designation on that tile to cancel (harvest or stockpile).
    NotDesignated,
    /// Stockpiles need a walkable tile with nothing on it.
    CannotStockpile,
    /// There is no farm plot on that tile (`Command::Sow`, #198).
    NoFarmPlot,
    /// The farm plot on that tile isn't empty: it's already sown, growing or ripe.
    PlotNotEmpty,
    /// The resource on that tile is renewable but has no `[harvest.clear]`
    /// (`docs/design/clearing.md`, issue #424): a fish spot, say.
    CannotClear,
    /// There is no finished building on that tile to take down (issue #342).
    NoBuilding,
    /// That building can't be taken down: the endeavor's gate (issue #342).
    CannotDeconstruct,
    /// That building is a landmark, which never comes down (`BuildingDef::landmark`, #723).
    Landmark,
}

#[derive(Debug, Clone, Copy)]
enum Need {
    Hunger = 0,
    Thirst = 1,
    Energy = 2,
}

impl Unit {
    /// What the unit is doing right now: a need it is meeting, else its job.
    pub fn task(&self) -> Option<Task> {
        self.need_task.or(self.job_task)
    }

    /// Asleep: it has chosen to sleep and has finished walking to its spot (a unit may first
    /// walk to a warm place, see `heat.rs`).
    pub fn is_sleeping(&self) -> bool {
        self.need_task == Some(Task::Sleep) && self.route.as_ref().is_none_or(|r| r.is_empty())
    }

    /// Healing inside a healing building right now (issue #375), as opposed to merely holding
    /// the task while walking there: it's actually stepped through the door
    /// (`World::enter_building`). `World::tick_needs` reads this to apply the building's boosted
    /// regen instead of the passive rate.
    pub fn is_healing(&self) -> bool {
        matches!(self.need_task, Some(Task::Healing(_))) && self.inside.is_some()
    }
}

/// The movement progress a step from `from` onto the adjacent tile `to` takes: 6 ticks at speed
/// 1.0 for a straight step on plain ground, more for a diagonal or a costly terrain, fewer along
/// a rail (#135). `rules` must be the ones the route was planned under (`World::rules_for`), so
/// the walk takes as long as the path search thought it would.
pub(super) fn step_needed(
    rules: &crate::path::MoveRules,
    map: &crate::mapgen::TileMap,
    from: Pos,
    to: Pos,
) -> f32 {
    let diagonal = to.x != from.x && to.y != from.y;
    let plain = if diagonal { DIAGONAL } else { STRAIGHT };
    let cost = rules.step_cost(&map[from], &map[to], diagonal).unwrap_or(plain);
    BASE_TICKS_PER_STEP * cost as f32 / STRAIGHT as f32
}

/// How a unit's trip to a stand tile stands this tick (`World::go_to`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Go {
    Arrived,
    Walking,
    Unreachable,
}

/// The remaining steps of a path, next step last.
pub(super) fn route_from(path: Vec<Pos>) -> Vec<Pos> {
    path.into_iter().skip(1).rev().collect()
}

/// Queries, and the designations behind `Command`s (`World::apply` is the way in from outside).
impl World {
    /// Designated work that isn't finished, oldest first.
    pub fn jobs(&self) -> &[Job] {
        &self.jobs
    }

    /// Tiles with a harvest designation that isn't finished (claimed or not), plus tiles that
    /// only have a standing order waiting on a resting resource to ripen (`is_repeating_harvest`
    /// tells the two apart).
    pub fn harvest_designations(&self) -> impl Iterator<Item = Pos> + '_ {
        self.jobs
            .iter()
            .filter_map(|j| match j.kind {
                JobKind::Harvest { pos } => Some(pos),
                JobKind::Haul { .. }
                | JobKind::Bend { .. }
                | JobKind::Build { .. }
                | JobKind::Study { .. }
                | JobKind::EndeavorTask { .. }
                | JobKind::Attend { .. } => None,
                JobKind::Craft { .. } => None,
                JobKind::Sow { .. } | JobKind::Reap { .. } => None,
                JobKind::Clear { .. } => None,
                JobKind::Chart { .. }
                | JobKind::Deconstruct { .. }
                | JobKind::Refuel { .. }
                | JobKind::Trade { .. } => None,
            })
            .chain(self.standing_harvest.iter().copied())
    }

    /// Is `pos` a repeating harvest (#81): one that re-queues itself every time the resource
    /// ripens again, instead of needing the player to re-designate it by hand?
    pub fn is_repeating_harvest(&self, pos: Pos) -> bool {
        self.standing_harvest.contains(&pos)
    }

    /// Tiles with a Clear mark that isn't finished (claimed or not): `designate_clear` queued a
    /// `JobKind::Clear` on them (docs/design/clearing.md, #411/#424/#426).
    pub fn clear_designations(&self) -> impl Iterator<Item = Pos> + '_ {
        self.jobs.iter().filter_map(|j| match j.kind {
            JobKind::Clear { pos } => Some(pos),
            JobKind::Harvest { .. }
            | JobKind::Haul { .. }
            | JobKind::Bend { .. }
            | JobKind::Build { .. }
            | JobKind::Study { .. }
            | JobKind::EndeavorTask { .. }
            | JobKind::Attend { .. }
            | JobKind::Craft { .. }
            | JobKind::Sow { .. }
            | JobKind::Reap { .. }
            | JobKind::Chart { .. }
            | JobKind::Deconstruct { .. }
            | JobKind::Refuel { .. }
            | JobKind::Trade { .. } => None,
        })
    }

    pub fn job(&self, id: JobId) -> Option<&Job> {
        self.jobs.binary_search_by_key(&id, |j| j.id).ok().map(|i| &self.jobs[i])
    }

    /// Whether no living unit could ever reach `pos` or stand beside it, under the walk rules
    /// every unit shares (`World::rules`): the same question `any_job_in_region` asks per unit,
    /// generalized over every one of them, so a frontend can flag a harvest or stockpile tile
    /// stranded on the wrong side of a river, wall or shore before a unit ever would have told
    /// it itself (#337, "inspect shows no ... unreachable status on a tile"). This is a weaker
    /// guarantee than `claim_job`'s own feasibility check: a tile can answer `false` here and
    /// still never be worked, for reasons this doesn't ask about (a resting resource, a chi cost,
    /// a full stockpile). `None` only in the narrow window before anything this map generation
    /// has needed the connectivity cache (`World::tick`'s own job claiming populates it the
    /// moment there's an unclaimed job and an idle unit to ask); a frontend that gets `None`
    /// should say nothing rather than guess.
    pub fn job_unreachable(&self, pos: Pos) -> Option<bool> {
        let regions = self.regions.as_ref()?;
        let in_region = |p: Pos| {
            regions
                .region(p)
                .is_some_and(|r| self.units.iter().any(|u| regions.region(u.pos) == Some(r)))
        };
        let walkable = self.map.get(pos).is_some_and(|t| self.rules.passable(t));
        let foot = Footprint::single(pos);
        let reachable =
            if walkable { foot.tiles().any(in_region) } else { foot.ring().any(in_region) };
        Some(!reachable)
    }

    /// Is the resource on `pos` a renewable one that has been worked and is still growing back?
    pub fn is_resting(&self, pos: Pos) -> bool {
        self.resting.contains_key(&pos)
    }

    /// Seconds left before the resting resource on `pos` ripens again, for the TUI's hover
    /// (`docs/design/difficulty.md`, "Readable simulation"). `None` when it isn't resting
    /// (`is_resting`, which an orchard's sapling also counts as).
    pub fn resting_left(&self, pos: Pos) -> Option<f32> {
        self.resting.get(&pos).copied()
    }

    /// Item stacks lying on the ground waiting for a hauler, by tile.
    pub fn ground_items(&self) -> impl Iterator<Item = (Pos, Stack)> + '_ {
        self.ground.iter().map(|(&p, &s)| (p, s))
    }

    pub fn stockpile_zones(&self) -> impl Iterator<Item = Pos> + '_ {
        self.zones.iter().copied()
    }

    /// What this stockpile tile holds.
    pub fn stored_at(&self, pos: Pos) -> Option<Stack> {
        self.stored.get(&pos).copied()
    }

    /// How many of `item` are in stockpiles.
    pub fn stored_total(&self, item: ItemId) -> u32 {
        self.stored.values().filter(|s| s.item == item).map(|s| s.count).sum()
    }

    /// Checks that `pos` has a resource the nation knows how to work, for either designate
    /// command.
    fn harvestable<'d>(&self, pos: Pos, defs: &'d Defs) -> Result<&'d HarvestDef, DesignateError> {
        self.map.get(pos).ok_or(DesignateError::OutOfBounds)?;
        let Some(h) = self.harvest_at(pos, defs) else {
            return Err(DesignateError::NothingToHarvest);
        };
        if let Some(t) = h.unlocked_by.filter(|&t| !self.research.is_known(t)) {
            return Err(DesignateError::NotResearched(t));
        }
        Ok(h)
    }

    /// What working `pos` is: its resource's harvest, or, with no resource on it, shore fishing
    /// (`[shore_fishing]`, #727) once `shore_fishing` is known and the tile is one of its water
    /// terrains with a walkable neighbour. `None` off the map or with nothing to work.
    pub fn harvest_at<'d>(&self, pos: Pos, defs: &'d Defs) -> Option<&'d HarvestDef> {
        let tile = self.map.get(pos)?;
        if let Some(r) = tile.resource {
            return defs.harvest(r);
        }
        self.shore_harvest(pos, defs)
    }

    /// Shore fishing on `pos` (`harvest_at`), ignoring any resource there.
    fn shore_harvest<'d>(&self, pos: Pos, defs: &'d Defs) -> Option<&'d HarvestDef> {
        let shore = defs.shore_fishing.as_ref()?;
        let tile = self.map.get(pos)?;
        if tile.resource.is_some() || !shore.terrains.contains(&tile.terrain) {
            return None;
        }
        if !self.has_feature(crate::research::Feature::ShoreFishing, defs) {
            return None;
        }
        let beside = [(1, 0), (-1, 0), (0, 1), (0, -1)]
            .into_iter()
            .any(|(dx, dy)| self.walkable(Pos::new(pos.x + dx, pos.y + dy)));
        beside.then_some(&shore.harvest)
    }

    /// Designates the resource on `pos` for harvesting (chop, quarry, gather, mine).
    pub(super) fn designate_harvest(
        &mut self,
        pos: Pos,
        defs: &Defs,
    ) -> Result<JobId, DesignateError> {
        self.harvestable(pos, defs)?;
        if self.jobs.iter().any(|j| j.kind == JobKind::Harvest { pos }) {
            return Err(DesignateError::AlreadyDesignated);
        }
        Ok(self.add_job(JobKind::Harvest { pos }))
    }

    /// Designates the resource on `pos` for harvesting, and marks it as a standing order (#81):
    /// every time it ripens again after being worked, a fresh job is queued without the player
    /// having to re-designate it. If the resource is currently resting, the mark is kept and the
    /// job is queued automatically once it ripens; if a job for this tile already exists (one-shot
    /// or otherwise), it is left alone and just gains the standing order.
    pub(super) fn designate_repeating_harvest(
        &mut self,
        pos: Pos,
        defs: &Defs,
    ) -> Result<(), DesignateError> {
        self.harvestable(pos, defs)?;
        self.standing_harvest.insert(pos);
        let resting = self.resting.contains_key(&pos);
        let queued = self.jobs.iter().any(|j| j.kind == JobKind::Harvest { pos });
        if !resting && !queued {
            self.add_job(JobKind::Harvest { pos });
        }
        Ok(())
    }

    /// Withdraws the harvest designation on `pos`, including any standing order on it. Work
    /// done on it is lost.
    pub(super) fn cancel_harvest(&mut self, pos: Pos) -> Result<(), DesignateError> {
        let before = self.jobs.len();
        self.jobs.retain(|j| j.kind != JobKind::Harvest { pos });
        let had_job = self.jobs.len() != before;
        let had_order = self.standing_harvest.remove(&pos);
        if !had_job && !had_order {
            return Err(DesignateError::NotDesignated);
        }
        Ok(())
    }

    /// Whether the resource on `pos`, if any, could be cleared right now: a known harvest (the
    /// tech gate included), and not a renewable with nothing to offer a clear
    /// (`HarvestDef::clear` is `None`, i.e. a fish spot). Shared by `designate_clear` (actually
    /// marking it) and build-over (`World::build_blocker`/`to_clear`, `docs/design/clearing.md`,
    /// issue #425): a footprint or door tile whose resource can't be cleared this way still
    /// blocks a placement.
    pub(super) fn clear_target<'d>(
        &self,
        pos: Pos,
        defs: &'d Defs,
    ) -> Result<&'d HarvestDef, DesignateError> {
        let h = self.harvestable(pos, defs)?;
        if h.regrow.is_some() && h.clear.is_none() {
            return Err(DesignateError::CannotClear);
        }
        Ok(h)
    }

    /// Marks the resource on `pos` for removal for good (`docs/design/clearing.md`, issue
    /// #411/#424): a non-renewable's Clear job is its ordinary harvest, which already clears it;
    /// a renewable needs a `[harvest.clear]` table, else it's refused (fish spots, on water
    /// where nobody builds). A resting plant can still be cleared: only `ripe_harvest` (an
    /// ordinary harvest) waits on that. Replaces any harvest designation and standing order on
    /// the tile; a unit mid-harvest drops it, as on `cancel_harvest` (the job disappearing under
    /// it, next tick it runs).
    pub(super) fn designate_clear(
        &mut self,
        pos: Pos,
        defs: &Defs,
    ) -> Result<JobId, DesignateError> {
        self.clear_target(pos, defs)?;
        if self.jobs.iter().any(|j| j.kind == JobKind::Clear { pos }) {
            return Err(DesignateError::AlreadyDesignated);
        }
        self.jobs.retain(|j| j.kind != JobKind::Harvest { pos });
        self.standing_harvest.remove(&pos);
        Ok(self.add_job(JobKind::Clear { pos }))
    }

    /// Withdraws the Clear designation on `pos`. Work already done on it is lost.
    pub(super) fn cancel_clear(&mut self, pos: Pos) -> Result<(), DesignateError> {
        let before = self.jobs.len();
        self.jobs.retain(|j| j.kind != JobKind::Clear { pos });
        if self.jobs.len() == before {
            return Err(DesignateError::NotDesignated);
        }
        Ok(())
    }

    /// Marks a tile as stockpile. Anything lying on the ground gets hauled to it (or another
    /// stockpile tile).
    pub(super) fn designate_stockpile(&mut self, pos: Pos) -> Result<(), DesignateError> {
        let tile = self.map.get(pos).ok_or(DesignateError::OutOfBounds)?;
        if !self.rules.passable(tile) {
            return Err(DesignateError::CannotStockpile);
        }
        self.zones.insert(pos);
        let stacks: Vec<Pos> = self.ground.keys().copied().collect();
        for p in stacks {
            self.ensure_haul_job(p);
        }
        self.wake_idle();
        Ok(())
    }

    /// Removes a tile's stockpile designation. What the tile held isn't lost and isn't hauled
    /// away: it is dropped on the ground on that tile (or the closest free one, if something
    /// else already lies there), where normal hauling can store it in another zone.
    pub(super) fn cancel_stockpile(&mut self, pos: Pos) -> Result<(), DesignateError> {
        if !self.zones.remove(&pos) {
            return Err(DesignateError::NotDesignated);
        }
        if let Some(stack) = self.stored.remove(&pos) {
            self.place_ground(pos, stack);
        }
        Ok(())
    }
}

impl World {
    pub(super) fn add_job(&mut self, kind: JobKind) -> JobId {
        let id = JobId(self.next_job);
        self.next_job += 1;
        self.jobs.push(Job { id, kind, claimed_by: None, progress: 0.0 });
        self.wake_idle();
        id
    }

    pub(super) fn job_index(&self, id: JobId) -> Option<usize> {
        self.jobs.binary_search_by_key(&id, |j| j.id).ok()
    }

    /// New work exists, so idle units shouldn't wait out their retry timer.
    pub(super) fn wake_idle(&mut self) {
        for u in &mut self.units {
            u.claim_retry_at = 0;
        }
    }

    fn retry_ticks(&self, defs: &Defs) -> u64 {
        (defs.jobs.retry_seconds * TICKS_PER_SECOND as f32).ceil() as u64
    }

    pub(super) fn ensure_haul_job(&mut self, pos: Pos) {
        if !self.jobs.iter().any(|j| j.kind == JobKind::Haul { pos }) {
            self.add_job(JobKind::Haul { pos });
        }
    }

    /// Puts a stack on the ground at `near`, or the closest free tile (one with no other kind of
    /// item, and not a stockpile tile), and queues a haul for it.
    pub(super) fn place_ground(&mut self, near: Pos, stack: Stack) {
        for r in 0..=GROUND_SEARCH_RADIUS {
            for dy in -r..=r {
                for dx in -r..=r {
                    if dx.abs().max(dy.abs()) != r {
                        continue;
                    }
                    let p = Pos::new(near.x + dx, near.y + dy);
                    let Some(tile) = self.map.get(p) else { continue };
                    if !self.rules.passable(tile) || self.zones.contains(&p) {
                        continue;
                    }
                    match self.ground.get_mut(&p) {
                        None => {
                            self.ground.insert(p, stack);
                        }
                        Some(s) if s.item == stack.item => s.count += stack.count,
                        Some(_) => continue,
                    }
                    self.ensure_haul_job(p);
                    return;
                }
            }
        }
        // Nowhere within reach: the items are lost. Only possible on a map that is solid
        // obstacles for 8 tiles around, so not worth a fallback.
    }

    /// How many more of `item` a stockpile tile can take.
    pub(super) fn room_at(&self, zone: Pos, item: ItemId, defs: &Defs) -> u32 {
        let cap = defs.jobs.stockpile_stack;
        match self.stored.get(&zone) {
            None => cap,
            Some(s) if s.item == item => cap.saturating_sub(s.count),
            Some(_) => 0,
        }
    }

    /// Does the nation know what it takes to work this kind of resource?
    fn can_work(&self, h: &HarvestDef) -> bool {
        self.is_unlocked(h.unlocked_by)
    }

    /// The harvest for the resource on `pos`, if it can be worked right now: it has a harvest
    /// entry, the nation knows how, and it isn't resting after the last time.
    pub(super) fn ripe_harvest<'d>(&self, pos: Pos, defs: &'d Defs) -> Option<&'d HarvestDef> {
        let h = self.harvest_at(pos, defs)?;
        (self.can_work(h) && !self.resting.contains_key(&pos)).then_some(h)
    }

    /// The harvest for the resource on `pos`, if a `JobKind::Clear` on it can be worked: it has
    /// a harvest entry and the nation knows how, same as `ripe_harvest` except a resting plant
    /// doesn't block a clear (`docs/design/clearing.md`: "a picked fruit tree can be uprooted at
    /// once"). Whether it can actually be *cleared* at all (a renewable needs
    /// `HarvestDef::clear`) is checked at designate time (`designate_clear`); `prune_jobs` drops
    /// the job if the resource itself goes some other way, so this only asks the work question.
    fn clearable_harvest<'d>(&self, pos: Pos, defs: &'d Defs) -> Option<&'d HarvestDef> {
        let h = defs.harvest(self.map.get(pos)?.resource?)?;
        self.can_work(h).then_some(h)
    }

    /// Seconds of growth a resting resource on `pos` gains per second: the boost of any orchard
    /// tending it (`tend_rate`, #142), times the difficulty preset's season factor
    /// (`World::season_growth_factor`, `world/season.rs`, #295): identity (1.0) with `seasons`
    /// off, so this is exactly `tend_rate` for every save from before #295.
    ///
    /// This is also the hook for faster growth from other sources: Water's "Water the Land"
    /// (docs/design/bending.md, #69) leaves tiles wet for a while, and plants on wet tiles (and
    /// later, farm plots) should ripen faster by a data-driven multiplier. Wet tiles now exist
    /// (`World::wet_tiles`, #69), but no multiplier is defined yet: Water the Land's payoff is
    /// sprouts, not faster regrowth. When one is, scale the result here too; nothing else in the
    /// regrowth code changes.
    fn growth_rate(&self, pos: Pos, defs: &Defs) -> f32 {
        let frost_hardy = self
            .map
            .get(pos)
            .and_then(|t| t.resource)
            .and_then(|r| defs.harvest(r))
            .is_some_and(|h| h.frost_hardy);
        self.tend_rate(pos, defs) * self.season_growth_factor(pos, defs, frost_hardy)
    }

    /// Advances every resting resource, ripening those that are done. A resource that was
    /// cleared some other way stops resting.
    pub(super) fn tick_growth(&mut self, defs: &Defs) {
        if self.resting.is_empty() {
            return;
        }
        // A sapling ripens (or is lost) when it stops resting, below.
        let step = 1.0 / TICKS_PER_SECOND as f32;
        let spots: Vec<Pos> = self.resting.keys().copied().collect();
        // Resources that ripened just now (as opposed to having been cleared some other way
        // while resting), collected so a standing order (#81) can re-queue their job once
        // `self.resting` is done borrowing.
        let mut ripened: Vec<Pos> = Vec::new();
        for pos in spots {
            let rate = self.growth_rate(pos, defs);
            let left = self.resting.get_mut(&pos).expect("just listed");
            *left -= rate * step;
            if *left <= 0.0 {
                self.resting.remove(&pos);
                if self.map[pos].resource.is_some() {
                    ripened.push(pos);
                }
            } else if self.map[pos].resource.is_none() {
                self.resting.remove(&pos);
            }
        }
        self.saplings.retain(|p| self.resting.contains_key(p));
        for pos in ripened {
            if self.standing_harvest.contains(&pos)
                && !self.jobs.iter().any(|j| j.kind == JobKind::Harvest { pos })
            {
                self.add_job(JobKind::Harvest { pos });
            }
        }
    }

    /// How many items working the resource on `pos` yields: `HarvestDef::tunnel_count` when a
    /// tunnel touches the tile (any of its 8 neighbours), so the vein is mined from inside the
    /// mountain (issue #132, docs/design/research.md, "Tunneling"), else `count`. Nothing asks
    /// which nation dug the tunnel or is mining: only Earth can bore one, through its tech.
    pub fn harvest_yield(&self, pos: Pos, h: &HarvestDef) -> u32 {
        let tunnel = || {
            (-1..=1)
                .flat_map(|dy| (-1..=1).map(move |dx| Pos::new(pos.x + dx, pos.y + dy)))
                .filter_map(|q| self.map.get(q))
                .any(|t| t.terrain == Terrain::Tunnel)
        };
        match h.tunnel_count {
            Some(n) if tunnel() => n,
            _ => h.count,
        }
    }

    /// The work on `pos` is done: a renewable resource goes back to growing, anything else is
    /// used up and its tile cleared. Returns its yield (`harvest_yield`), which counts in full
    /// toward the harvest tally. The resting time is multiplied by the difficulty preset's
    /// `regrow` knob (1.0 is identity, #294) and, for a wild renewable harvest, by `1 + strain`
    /// (`harvest_strain`, #296); strain_kills may clear the resource outright instead of resting
    /// it.
    ///
    /// The harvest's `also` stacks (the great ruin's teaching scroll, #736) drop here, each as
    /// its own ground stack, so every way a harvest completes drops them. The caller drops the
    /// main yield.
    pub(super) fn work_done(&mut self, pos: Pos, h: &HarvestDef, defs: &Defs) -> u32 {
        let count = self.harvest_yield(pos, h);
        self.tallies.count_harvest(h.item, count);
        for &extra in &h.also {
            self.tallies.count_harvest(extra.item, extra.count);
            self.place_ground(pos, extra);
        }
        match h.regrow {
            Some(seconds) => match self.harvest_strain(pos, h, defs) {
                Some(strain_mult) => {
                    self.resting.insert(pos, seconds * self.knobs(defs).regrow * strain_mult);
                }
                None => {
                    self.map[pos].resource = None;
                    self.map_changed();
                }
            },
            None => {
                self.map[pos].resource = None;
                self.map_changed();
            }
        }
        count
    }

    /// A `JobKind::Clear` on `pos` just finished (`run_clear`), uprooting or felling the
    /// resource there for good (`docs/design/clearing.md`, issue #411/#424). `h` is the
    /// resource's own harvest.
    ///
    /// Non-renewable (`h.clear` is `None`): the clear *is* the harvest -- `work_done` already
    /// clears the tile and counts the tally -- so it drops exactly what chopping it would.
    /// Renewable with `HarvestDef::clear` (the only other way a Clear job exists, by
    /// `designate_clear`): drops the plant's ordinary ripe yield too, if it wasn't resting, then
    /// the clear's own item (if any), and removes the resource along with everything that
    /// tracked it on this tile.
    fn clear_done(&mut self, i: usize, pos: Pos, h: &HarvestDef, defs: &Defs) {
        let Some(c) = h.clear else {
            let count = self.work_done(pos, h, defs);
            self.place_ground(pos, Stack { item: h.item, count });
            self.maybe_injure(i, pos, h, defs);
            return;
        };
        if !self.resting.contains_key(&pos) {
            let count = self.harvest_yield(pos, h);
            self.tallies.count_harvest(h.item, count);
            self.place_ground(pos, Stack { item: h.item, count });
        }
        if let Some(item) = c.item {
            self.tallies.count_harvest(item, c.count);
            self.place_ground(pos, Stack { item, count: c.count });
        }
        self.map[pos].resource = None;
        self.resting.remove(&pos);
        self.saplings.remove(&pos);
        self.strain.remove(&pos);
        self.standing_harvest.remove(&pos);
        self.map_changed();
        self.maybe_injure_at(i, pos, c.risk, None, defs);
    }

    pub(super) fn carry_capacity(&self, i: usize, defs: &Defs) -> u32 {
        // A hauling beast carries its row's `carry`, not scaled by `carry_capacity` (#693).
        if let Some(b) = self.units[i].beast {
            return match defs.beast(b.id).work {
                crate::beast::BeastWork::Haul { carry, .. } => carry,
                crate::beast::BeastWork::Bend { .. } => 0,
            };
        }
        let m = self.modifiers_for(&self.units[i], defs).carry_capacity;
        ((defs.jobs.base_carry as f32 * m).round() as u32).max(1)
    }

    /// Work-seconds per second for this unit on this harvest: the modifier it names, from the
    /// unit's own effective modifiers (`modifiers_for`, #239), not necessarily the settlement's.
    fn work_rate(&self, i: usize, h: &HarvestDef, defs: &Defs) -> f32 {
        let mods = self.modifiers_for(&self.units[i], defs);
        h.modifier.as_deref().and_then(|m| mods.get(m)).unwrap_or(1.0)
    }

    /// Frees what a dead unit held: its claimed jobs go back in the queue (keeping progress) and
    /// what it carried drops where it fell.
    pub(super) fn release(&mut self, u: &Unit) {
        for j in &mut self.jobs {
            if j.claimed_by == Some(u.id) {
                j.claimed_by = None;
            }
        }
        if let Some(stack) = u.carrying {
            self.place_ground(u.pos, stack);
        }
        self.wake_idle();
    }

    /// Drops unclaimed jobs whose target has gone: a tree someone else cleared, a stack that was
    /// eaten.
    fn prune_jobs(&mut self, defs: &Defs) {
        // `job_is_live` reads the world, so the queue is lifted out while it is filtered.
        let mut jobs = std::mem::take(&mut self.jobs);
        jobs.retain(|j| j.claimed_by.is_some() || self.job_is_live(j.kind, defs));
        self.jobs = jobs;
        // A standing order (#81) outlives its job (which disappears between harvests while the
        // resource rests), but not the resource itself: once that's gone for good there is
        // nothing left to re-queue for.
        let gone: Vec<Pos> = self
            .standing_harvest
            .iter()
            .copied()
            .filter(|&pos| self.harvest_at(pos, defs).is_none())
            .collect();
        for pos in gone {
            self.standing_harvest.remove(&pos);
        }
    }

    /// The cheapest reachable tile to stand on to work something covering `foot`: one of its
    /// own tiles if it is `walkable`, else one of the ring around it. Ties break on the tile.
    pub(super) fn stand(
        &self,
        flood: &Flood,
        foot: Footprint,
        walkable: bool,
    ) -> Option<(u32, Pos)> {
        let cost = |q: Pos| flood.cost(q).map(|c| (c, q));
        if walkable {
            foot.tiles().filter_map(cost).min()
        } else {
            foot.ring().filter_map(cost).min()
        }
    }

    /// `stand` for one tile: the tile itself if units can enter it, else one of its neighbours
    /// (a tree can't be walked onto, but can be chopped from beside it).
    pub(super) fn stand_tile(&self, flood: &Flood, pos: Pos) -> Option<(u32, Pos)> {
        let walkable = self.map.get(pos).is_some_and(|t| self.rules.passable(t));
        self.stand(flood, Footprint::single(pos), walkable)
    }

    /// `stand` for the building or site on `pos` (`World::work_area`): inside a walkable one,
    /// beside a solid one.
    pub(super) fn stand_building(
        &self,
        flood: &Flood,
        pos: Pos,
        defs: &Defs,
    ) -> Option<(u32, Pos)> {
        let (foot, walkable) = self.work_area(pos, defs);
        self.stand(flood, foot, walkable)
    }

    /// Plans unit `i`'s route to stand for `rule`, unless it already has one (#603): the one
    /// place a runner turns "go there" into a route, so a stale-route fix (#562) lands once.
    /// False if there is nowhere reachable to stand. An existing route is trusted, with no flood.
    pub(super) fn plan_route(&mut self, i: usize, rule: StandRule, defs: &Defs) -> bool {
        if self.units[i].route.is_some() {
            return true;
        }
        let flood = self.flood_from(i);
        match self.stand_at(&flood, rule, defs).and_then(|(_, s)| flood.path_to(s)) {
            Some(path) => {
                self.units[i].route = Some(route_from(path));
                true
            }
            None => false,
        }
    }

    /// One tick of getting to `rule`'s stand tile: plan if there is no route, then `walk`.
    pub(super) fn go_to(&mut self, i: usize, rule: StandRule, defs: &Defs) -> Go {
        if !self.plan_route(i, rule, defs) {
            Go::Unreachable
        } else if self.walk(i, defs) {
            Go::Arrived
        } else {
            Go::Walking
        }
    }

    /// `go_to` for a claimed job: true once arrived, false while still walking, and an
    /// unreachable destination abandons the job (also false).
    pub(super) fn go_or_abandon(
        &mut self,
        i: usize,
        id: JobId,
        rule: StandRule,
        defs: &Defs,
    ) -> bool {
        match self.go_to(i, rule, defs) {
            Go::Arrived => true,
            Go::Walking => false,
            Go::Unreachable => {
                self.abandon_job(i, id, defs);
                false
            }
        }
    }

    /// Mid-walk to a stand tile chosen earlier (a fetch from a source): takes the step and says
    /// so, so the runner skips the flood that re-picking the source would cost (#574, #603).
    /// False once the route is spent or missing, which is when the runner looks again.
    pub(super) fn keep_walking(&mut self, i: usize, defs: &Defs) -> bool {
        if self.units[i].route.as_ref().is_some_and(|r| !r.is_empty()) {
            self.walk(i, defs);
            true
        } else {
            false
        }
    }

    pub(super) fn flood_from(&self, i: usize) -> Flood {
        let rules = self.rules_for(&self.units[i]);
        Flood::over(self.step_grids.get(&self.map, self.map_rev, rules), self.units[i].pos)
    }

    /// How `u` moves right now. Planning (`flood_from`), walking (`walk`) and the client's view
    /// of it (`stride`) all ask here, so they cannot disagree.
    pub(super) fn rules_for(&self, u: &Unit) -> &MoveRules {
        rules_for(&self.rules, &self.cart_rules, u)
    }

    pub(super) fn finish_need(&mut self, i: usize) {
        // Whatever bed or healing building held the entrance for it (#227, a bed bigger than
        // 1x1; #375, the Healing Hut) is done with, so a unit left inside steps back out: waking
        // normally, reaching `stop_at`, or a hungrier need cutting it short (`run_need`'s
        // `Task::Sleep` and `Task::Healing` call this directly, without going through
        // `consider_needs`'s own leave-on-pre-empt).
        self.leave_building(i);
        let u = &mut self.units[i];
        u.need_task = None;
        u.route = None;
        u.work = 0.0;
        u.bed = None;
        u.healing = None;
        u.bath = None;
    }

    pub(super) fn finish_job(&mut self, i: usize) {
        // Whatever job held the entrance for it (#226; #227 is the first to claim one) is over,
        // so a unit left inside steps back out.
        self.leave_building(i);
        let u = &mut self.units[i];
        u.job_task = None;
        u.route = None;
    }

    /// Lets go of a claimed job without finishing it (it became unreachable).
    pub(super) fn abandon_job(&mut self, i: usize, id: JobId, defs: &Defs) {
        if let Some(j) = self.job_index(id) {
            self.jobs[j].claimed_by = None;
        }
        self.finish_job(i);
        self.units[i].claim_retry_at = self.tick + self.retry_ticks(defs);
    }

    /// One tick of walking along the route. True once the unit is at the end of it (or the
    /// route is empty to begin with); arriving costs the tick the last step landed on.
    pub(super) fn walk(&mut self, i: usize, defs: &Defs) -> bool {
        // A gate may make the route shorter (`glide.rs`), and its hop is a step to a tile that
        // isn't a neighbour.
        self.consider_gate(i, defs);
        let u = &self.units[i];
        if let Some(&next) = u.route.as_ref().and_then(|r| r.last())
            && u.pos.chebyshev(next) > 1
        {
            self.begin_hop(i, next, defs);
            return false;
        }
        let speed = self.unit_speed(&self.units[i], defs);
        let rules = rules_for(&self.rules, &self.cart_rules, &self.units[i]);
        let u = &mut self.units[i];
        let Some(route) = u.route.as_mut() else { return false };
        let Some(&next) = route.last() else {
            u.move_progress = 0.0;
            return true;
        };
        u.move_progress += speed;
        let needed = step_needed(rules, &self.map, u.pos, next);
        if u.move_progress >= needed {
            u.move_progress -= needed;
            u.pos = next;
            route.pop();
        }
        false
    }

    /// One unit's turn: meet a need if one is low, else carry on with its job, else find one.
    pub(super) fn act(&mut self, i: usize, defs: &Defs) {
        // A hop through a gate isn't interrupted by a need, or anything else (`glide.rs`).
        if self.units[i].glider.hopping() {
            return self.glide_tick(i);
        }
        self.consider_needs(i, defs);
        let u = &self.units[i];
        match (u.need_task, u.job_task) {
            (Some(task), _) => self.run_need(i, task, defs),
            (None, Some(task)) => self.run_job(i, task, defs),
            (None, None) => self.run_idle(i, defs),
        }
    }

    // ---- needs ----

    /// Starts a need task if a need is at or below its `low` level and there is a way to meet
    /// it, most urgent (lowest relative to `low`) first. A need that can't be met right now is
    /// retried after a short wait, and meanwhile the job goes on.
    fn consider_needs(&mut self, i: usize, defs: &Defs) {
        let u = &self.units[i];
        if u.need_task.is_some() {
            return;
        }
        if u.job_task == Some(Task::Rite) {
            // His needs never pre-empt the Harmony Rite once he has begun it
            // (`docs/design/endgame.md`, "What can go wrong"; #238).
            return;
        }
        let n = &defs.needs;
        let mut urgent: Vec<(f32, Need)> = [
            (Need::Thirst, u.needs.thirst, n.thirst.low),
            (Need::Hunger, u.needs.hunger, n.hunger.low),
            (Need::Energy, u.needs.energy, n.energy.low),
        ]
        .into_iter()
        .filter(|&(need, level, low)| level <= low && self.tick >= u.need_retry_at[need as usize])
        .map(|(need, level, low)| (level / low.max(f32::EPSILON), need))
        .collect();
        urgent.sort_by(|a, b| a.0.total_cmp(&b.0));
        if !urgent.is_empty() && self.units[i].inside.is_some() {
            // An urgent need pre-empts whatever job held the entrance for it (#226): it has to
            // be met from outside, so the unit steps out before any route below is planned, in
            // case the door is blocked and it comes out somewhere else.
            self.leave_building(i);
        }
        for (_, need) in urgent {
            match self.plan_need(i, need, defs) {
                Some((task, route, bed)) => {
                    let u = &mut self.units[i];
                    u.need_task = Some(task);
                    u.route = Some(route);
                    u.work = 0.0;
                    u.bed = bed;
                    if let Task::Bath(anchor) = task {
                        u.bath = Some(anchor);
                    }
                    return;
                }
                None => {
                    self.units[i].need_retry_at[need as usize] = self.tick + self.retry_ticks(defs);
                }
            }
        }
        // Nothing urgent (issue #375): a hurt or sick unit fetches a remedy from stores if one
        // fits (issue #390), else rests at the nearest healing building with room; either is the
        // lowest priority there is, and both only ever start once every real need is comfortably
        // above `low` (`Needs::any_low`), the same gate the passive regen uses, so a need that's
        // merely on retry cooldown (the loop above skipped it, but it's still `<= low`) rules
        // both out. A remedy is tried first: it's a short errand, not a trip to rest, so it
        // doesn't compete with the healing building for the same unit.
        if self.units[i].needs.any_low(n) {
            return;
        }
        if self.tick >= self.units[i].remedy_retry_at {
            match self.plan_remedy(i, defs) {
                Some((task, route)) => {
                    let u = &mut self.units[i];
                    u.need_task = Some(task);
                    u.route = Some(route);
                    u.work = 0.0;
                    return;
                }
                None => {
                    self.units[i].remedy_retry_at = self.tick + self.retry_ticks(defs);
                }
            }
        }
        if self.tick >= self.units[i].healing_retry_at {
            match self.plan_healing(i, defs) {
                Some((route, anchor)) => {
                    let u = &mut self.units[i];
                    u.need_task = Some(Task::Healing(anchor));
                    u.route = Some(route);
                    u.work = 0.0;
                    u.healing = Some(anchor);
                }
                None => {
                    self.units[i].healing_retry_at = self.tick + self.retry_ticks(defs);
                }
            }
        }
    }

    /// The task for a need, the route to where it happens, and the bed it claims (sleeping only).
    fn plan_need(
        &self,
        i: usize,
        need: Need,
        defs: &Defs,
    ) -> Option<(Task, Vec<Pos>, Option<Pos>)> {
        let plain = |(task, route)| (task, route, None);
        match need {
            // A tired unit with a bath in reach bathes instead of lying down (#741).
            Need::Energy => Some(match self.plan_bath(i, defs) {
                Some((route, anchor)) => (Task::Bath(anchor), route, None),
                None => self.plan_sleep(i, defs),
            }),
            Need::Thirst => self.plan_drink(i, defs).map(plain),
            Need::Hunger => self.plan_food(i, defs).map(plain),
        }
    }

    /// Where to sleep: the nearest free bed within its `seek_tiles`, else near a lit heat
    /// source if there is one in reach, else right here on the ground.
    fn plan_sleep(&self, i: usize, defs: &Defs) -> (Task, Vec<Pos>, Option<Pos>) {
        if let Some((route, bed)) = self.bed_route(i, defs) {
            return (Task::Sleep, route, Some(bed));
        }
        (Task::Sleep, self.warm_sleep_route(i).unwrap_or_default(), None)
    }

    /// Whether fewer than `limit` other units have already claimed the bed keyed by `key`
    /// (`Unit::bed`: a 1x1 bed's own tile, or a bigger one's anchor), counting from the moment
    /// they set off, not just once they're lying in it.
    fn bed_has_room(&self, key: Pos, me: usize, limit: u32) -> bool {
        let claimed =
            self.units.iter().enumerate().filter(|&(k, u)| k != me && u.bed == Some(key)).count();
        (claimed as u32) < limit
    }

    /// The route to the nearest free bed unit `i` can reach within the bed's `seek_tiles` (by
    /// walking cost, ties by position), and the key it reserves (`Unit::bed`). A 1x1 bed is
    /// walked onto directly and holds one sleeper, keyed by its own tile, unchanged since #58.
    /// One bigger than 1x1 needs a `capacity` (#227): that many places inside it, reached (and
    /// reserved) through its door like a study seat, keyed by its anchor.
    pub(super) fn bed_route(&self, i: usize, defs: &Defs) -> Option<(Vec<Pos>, Pos)> {
        if !defs.buildings.iter().any(|b| b.bed.is_some()) {
            return None;
        }
        // Every bed's stand tile and the most it can cost to reach (#293: a flood only needs to
        // grow far enough to confirm the nearest one of these, not the whole reachable region).
        // A stand tile shared by more than one bed (vanishingly rare -- two footprints would
        // have to touch at exactly that tile) keeps all of them, in `self.buildings()`'s anchor
        // order, same as a plain scan would see them in.
        let mut targets: HashMap<Pos, Vec<(u32, Pos, u32)>> = HashMap::new();
        for b in self.buildings() {
            let (foot, id) = (b.footprint(), b.id);
            let Some(bed) = defs.building(id).bed else { continue };
            let big = foot.w > 1 || foot.h > 1;
            let (stand, limit) =
                if big { (foot.entrance(), defs.building(id).capacity) } else { (foot.anchor, 1) };
            if limit == 0 {
                continue;
            }
            targets.entry(stand).or_default().push((bed.seek_tiles * STRAIGHT, foot.anchor, limit));
        }
        let rules = self.rules_for(&self.units[i]);
        let (flood, found) =
            Flood::nearest(&self.map, self.units[i].pos, rules, &targets, |_, c, beds| {
                beds.iter()
                    .find(|&&(max_cost, anchor, limit)| {
                        c <= max_cost && self.bed_has_room(anchor, i, limit)
                    })
                    .map(|&(_, anchor, _)| ((), anchor))
            });
        let (stand, key) = found?;
        Some((route_from(flood.path_to(stand)?), key))
    }

    /// How much faster than the base rate sleeping unit `u` regains energy: the warmth of the
    /// heat source it is near (`sleep_factor_at`) times the bed it is lying in, if any. A
    /// sleeper inside a bed bigger than 1x1 (#227) lies on the entrance tile, not the building
    /// itself, so this looks the bed up through `inside`, the same as a study seat does.
    pub(super) fn sleep_factor_for(&self, u: &Unit, defs: &Defs) -> f32 {
        self.sleep_factor_at(u.pos) * self.bed_sleep_bonus(u, defs)
    }

    /// Where to heal (issue #375): the route to the nearest healing building with room whose
    /// `seek_below` this unit's health is under, and the anchor it reserves (`Unit::healing`).
    /// `None` with no reachable one -- no healing building in the data at all, none this
    /// healthy would seek out, or every reachable one already full. Unlike `bed_route` there is
    /// no `seek_tiles` limit: the data names none, so any reachable one counts.
    pub(super) fn plan_healing(&self, i: usize, defs: &Defs) -> Option<(Vec<Pos>, Pos)> {
        if !defs.buildings.iter().any(|b| b.healing.is_some()) {
            return None;
        }
        let health = self.units[i].needs.health;
        // Every healing building's door, and the capacity and threshold of each (parallel to
        // `bed_route`'s `targets`, keyed the same way: a door shared by more than one building
        // keeps all of them, in `self.buildings()`'s anchor order).
        let mut targets: HashMap<Pos, Vec<(Pos, u32, f32)>> = HashMap::new();
        for b in self.buildings() {
            let (foot, id) = (b.footprint(), b.id);
            let Some(h) = defs.building(id).healing.as_ref() else { continue };
            let capacity = defs.building(id).capacity;
            if capacity == 0 || health >= h.seek_below {
                continue;
            }
            targets.entry(foot.entrance()).or_default().push((foot.anchor, capacity, h.seek_below));
        }
        if targets.is_empty() {
            return None;
        }
        let rules = self.rules_for(&self.units[i]);
        let (flood, found) =
            Flood::nearest(&self.map, self.units[i].pos, rules, &targets, |_, _, huts| {
                huts.iter()
                    .find(|&&(anchor, limit, _)| self.healing_has_room(anchor, i, limit))
                    .map(|&(anchor, ..)| ((), anchor))
            });
        let (stand, key) = found?;
        Some((route_from(flood.path_to(stand)?), key))
    }

    /// Where to bathe (#741): the route to the nearest finished bath with room within its
    /// `seek_tiles`, for a unit whose energy is under its `seek_energy_below` and that is off its
    /// cooldown, and the anchor it reserves (`Unit::bath`). `None` otherwise.
    pub(super) fn plan_bath(&self, i: usize, defs: &Defs) -> Option<(Vec<Pos>, Pos)> {
        if !defs.buildings.iter().any(|b| b.bath.is_some()) {
            return None;
        }
        let u = &self.units[i];
        if self.tick < u.bath_ready_at {
            return None;
        }
        let mut targets: HashMap<Pos, Vec<(u32, Pos, u32)>> = HashMap::new();
        for b in self.buildings() {
            let def = defs.building(b.id);
            let Some(bath) = def.bath else { continue };
            if def.capacity == 0 || u.needs.energy >= bath.seek_energy_below {
                continue;
            }
            let foot = b.footprint();
            targets.entry(foot.entrance()).or_default().push((
                bath.seek_tiles * STRAIGHT,
                foot.anchor,
                def.capacity,
            ));
        }
        if targets.is_empty() {
            return None;
        }
        let rules = self.rules_for(u);
        let (flood, found) = Flood::nearest(&self.map, u.pos, rules, &targets, |_, c, baths| {
            baths
                .iter()
                .find(|&&(max_cost, anchor, limit)| {
                    c <= max_cost && self.bath_has_room(anchor, i, limit)
                })
                .map(|&(_, anchor, _)| ((), anchor))
        });
        let (stand, key) = found?;
        Some((route_from(flood.path_to(stand)?), key))
    }

    /// Whether fewer than `limit` other units have claimed the bath anchored on `key`.
    fn bath_has_room(&self, key: Pos, me: usize, limit: u32) -> bool {
        let claimed =
            self.units.iter().enumerate().filter(|&(k, u)| k != me && u.bath == Some(key)).count();
        (claimed as u32) < limit
    }

    /// Whether fewer than `limit` other units have already claimed the healing building keyed
    /// by `key` (`Unit::healing`: its anchor), counting from the moment they set off, not just
    /// once they're inside. Mirrors `bed_has_room`.
    fn healing_has_room(&self, key: Pos, me: usize, limit: u32) -> bool {
        let claimed = self
            .units
            .iter()
            .enumerate()
            .filter(|&(k, u)| k != me && u.healing == Some(key))
            .count();
        (claimed as u32) < limit
    }

    /// The healing boost unit `u` gets from `data/needs.toml`'s passive rate right now (issue
    /// #375): `(regen_per_minute, stop_at)`, scaled by the healing building's `modifier` if it
    /// names one. `None` unless it is actually `Unit::is_healing` (walking there doesn't count,
    /// and nor does a building that vanished from under it). Read by `World::tick_needs`.
    pub(super) fn healing_bonus_for(&self, u: &Unit, defs: &Defs) -> Option<(f32, f32)> {
        if !u.is_healing() {
            return None;
        }
        let anchor = u.inside?;
        let healing = self.building_at(anchor).map(|id| defs.building(id))?.healing.as_ref()?;
        let mods = self.modifiers_for(u, defs);
        let rate = healing.regen_per_minute
            * healing.modifier.as_deref().and_then(|m| mods.get(m)).unwrap_or(1.0);
        Some((rate, healing.stop_at))
    }

    /// The remedy unit `i` qualifies for right now (issue #390, docs/design/harm.md "Healing: a
    /// job for each feature"): sick with one that `cures_sickness`, or hurt below one's own
    /// `seek_below`, whichever `data/harm.toml` lists first. `None` with the difficulty preset's
    /// `harm` at 0 (nothing qualifies, matching every other harm rule) or nothing fits.
    fn remedy_for<'d>(&self, i: usize, defs: &'d Defs) -> Option<&'d RemedyDef> {
        if self.knobs(defs).harm <= 0.0 {
            return None;
        }
        let u = &self.units[i];
        let (sick, health) = (self.is_sick(u), u.needs.health);
        defs.harm.remedies.iter().find(|r| (r.cures_sickness && sick) || health < r.seek_below)
    }

    /// Where to fetch a remedy (issue #390): the nearest stockpile tile holding one unit `i`
    /// qualifies for (`remedy_for`), and the route there. `None` with nothing to qualify for, or
    /// none in stores anywhere reachable.
    fn plan_remedy(&self, i: usize, defs: &Defs) -> Option<(Task, Vec<Pos>)> {
        let remedy = self.remedy_for(i, defs)?;
        let rules = self.rules_for(&self.units[i]);
        let (flood, found) = Flood::seek(&self.map, self.units[i].pos, rules, |p, _| {
            self.stored.get(&p).filter(|s| s.item == remedy.item).map(|_| ((), ()))
        });
        let (stand, ()) = found?;
        Some((Task::Remedy(stand), route_from(flood.path_to(stand)?)))
    }

    /// The nearest tile beside water: `Flood::seek` (#293) checks the handful of tiles it
    /// actually visits on the way, nearest-cost-first, instead of a second pass over the whole
    /// map once a full flood is in hand -- every tile that pass would skip fails this check too.
    ///
    /// While the difficulty preset's `harm` is above 0 (`docs/design/harm.md`, "Sickness: sea
    /// water", issue #389), this prefers a spot beside fresh water (a river, lake, ford or canal) over
    /// one beside any water at all, as long as that spot is within `data/harm.toml`'s
    /// `[sickness] fresh_detour` path tiles of the nearest spot of any kind; otherwise, or with
    /// `harm` at 0, it keeps the nearest spot of any kind, sea or not, exactly as before #389.
    fn plan_drink(&self, i: usize, defs: &Defs) -> Option<(Task, Vec<Pos>)> {
        let rules = self.rules_for(&self.units[i]);
        let pos = self.units[i].pos;
        // A barrel's door is a spot to drink, and fresh (`outpost.rs`).
        let barrels = self.barrel_door_list(defs);
        let (any_flood, any_found) = Flood::seek(&self.map, pos, rules, |p, _| {
            if !self.rules.passable(&self.map[p]) {
                return None;
            }
            (barrels.contains(&p)
                || self.map.neighbors(p, true).any(|q| self.map[q].terrain.is_water()))
            .then_some(((), ()))
        });
        let (any_stand, ()) = any_found?;
        if self.knobs(defs).harm > 0.0 {
            let is_fresh = Terrain::is_fresh;
            let (fresh_flood, fresh_found) = Flood::seek(&self.map, pos, rules, |p, _| {
                if !self.rules.passable(&self.map[p]) {
                    return None;
                }
                (barrels.contains(&p)
                    || self.map.neighbors(p, true).any(|q| is_fresh(self.map[q].terrain)))
                .then_some(((), ()))
            });
            if let Some((fresh_stand, ())) = fresh_found {
                let any_cost = any_flood.cost(any_stand).unwrap_or(0);
                let budget = any_cost + defs.harm.sickness.fresh_detour * STRAIGHT;
                if fresh_flood.cost(fresh_stand).is_some_and(|c| c <= budget) {
                    return Some((Task::Drink, route_from(fresh_flood.path_to(fresh_stand)?)));
                }
            }
        }
        Some((Task::Drink, route_from(any_flood.path_to(any_stand)?)))
    }

    /// Whether unit `i` won't eat `item` (the Avatar's `[avatar].refuses`, #237; "a canon touch
    /// and a real constraint for a fish-fed Water settlement"). Always false for anyone else.
    pub(super) fn refuses_food(&self, i: usize, item: ItemId, defs: &Defs) -> bool {
        self.units[i].is_avatar()
            && defs.endeavor.avatar.as_ref().is_some_and(|a| a.refuses.contains(&item))
    }

    /// The rank (lowest first: stockpile, pantry, ground, forage) of the best unclaimed food
    /// source on `pos` that unit `me` will eat, if any. Matches `plan_food`'s old per-category checks, just
    /// asked of one tile at a time instead of every entry of `self.stored`/`self.ground`/the map
    /// up front.
    fn food_rank_at(&self, pos: Pos, defs: &Defs, me: usize) -> Option<u8> {
        let eats =
            |item: ItemId| defs.food_of(item).is_some() && !self.refuses_food(me, item, defs);
        if self.stored.get(&pos).is_some_and(|s| eats(s.item)) {
            return Some(0);
        }
        // Rank 1 is a pantry's door, which `plan_food` checks itself.
        if self.ground.get(&pos).is_some_and(|s| eats(s.item)) {
            return Some(2);
        }
        let h = self.ripe_harvest(pos, defs)?;
        (eats(h.item) && !self.forage_taken(pos, me)).then_some(3)
    }

    /// The closest food this unit will eat: carried (free), then stockpile, ground and wild
    /// resources by walking cost, preferring them in that order on a tie. `Flood::seek` (#293)
    /// checks `food_rank_at` only on tiles it actually visits (a stand tile itself, or a neighbour
    /// standing beside an unwalkable source, exactly as `stand_tile` would resolve one),
    /// nearest-cost-first, instead of a second pass over the whole map.
    fn plan_food(&self, i: usize, defs: &Defs) -> Option<(Task, Vec<Pos>)> {
        let u = &self.units[i];
        if u.carrying
            .is_some_and(|s| defs.food_of(s.item).is_some() && !self.refuses_food(i, s.item, defs))
        {
            return Some((Task::Eat(FoodSource::Carried), Vec::new()));
        }
        let rules = self.rules_for(&self.units[i]);
        // The door of every outpost pantry holding something this unit will eat (`outpost.rs`).
        let pantries = self.pantry_doors(defs, |item| {
            defs.food_of(item).is_some() && !self.refuses_food(i, item, defs)
        });
        let (flood, found) = Flood::seek(&self.map, self.units[i].pos, rules, |q, _| {
            let mut best: Option<(u8, Pos)> = None;
            let mut consider = |rank: u8, source: Pos| {
                let key = (rank, source);
                if best.is_none_or(|b| key < b) {
                    best = Some(key);
                }
            };
            // q itself, if a unit could actually stand there to work it.
            if self.rules.passable(&self.map[q]) {
                if let Some(rank) = self.food_rank_at(q, defs, i) {
                    consider(rank, q);
                }
                if pantries.iter().any(|&(door, _)| door == q) {
                    consider(1, q);
                }
            }
            // An unwalkable source (ordinarily a wild resource) worked from beside it: one of
            // q's neighbours, the same ring `stand_tile` would search.
            for h in self.map.neighbors(q, true) {
                if !self.rules.passable(&self.map[h])
                    && let Some(rank) = self.food_rank_at(h, defs, i)
                {
                    consider(rank, h);
                }
            }
            best.map(|key| (key, key))
        });
        let (stand, (rank, source)) = found?;
        let source = match rank {
            0 => FoodSource::Stockpile(source),
            1 => {
                let anchor = pantries.iter().find(|&&(door, _)| door == source).map(|&(_, a)| a)?;
                FoodSource::Pantry(anchor)
            }
            2 => FoodSource::Ground(source),
            _ => FoodSource::Forage(source),
        };
        Some((Task::Eat(source), route_from(flood.path_to(stand)?)))
    }

    /// True if the resource on `pos` is spoken for: designated for harvest or clear
    /// (`docs/design/clearing.md`: hungry units never forage a Clear job, so an uprooted tree's
    /// bonus yield isn't eaten out from under the unit doing the clearing), or another unit is
    /// already on its way to forage it.
    fn forage_taken(&self, pos: Pos, me: usize) -> bool {
        self.jobs
            .iter()
            .any(|j| matches!(j.kind, JobKind::Harvest { pos: p } | JobKind::Clear { pos: p } if p == pos))
            || self
                .units
                .iter()
                .enumerate()
                .any(|(k, u)| k != me && u.need_task == Some(Task::Eat(FoodSource::Forage(pos))))
    }

    fn run_need(&mut self, i: usize, task: Task, defs: &Defs) {
        match task {
            Task::Sleep => {
                let (u, s) = (&self.units[i], &defs.jobs);
                let ready = |need: Need, level: f32| {
                    level < s.sleep_interrupt_below && self.tick >= u.need_retry_at[need as usize]
                };
                if u.needs.energy >= s.sleep_until
                    || ready(Need::Hunger, u.needs.hunger)
                    || ready(Need::Thirst, u.needs.thirst)
                {
                    return self.finish_need(i);
                }
                if self.units[i].route.as_ref().is_some_and(|r| !r.is_empty()) {
                    self.walk(i, defs);
                }
                // At the door of a bed bigger than 1x1 (#227), the tick it arrives or any tick
                // after while it waits for room: step inside. A no-op once already inside, or
                // for a 1x1 bed, whose own tile -- where the unit already stands -- isn't its
                // door.
                if let Some(anchor) = self.units[i].bed
                    && self.units[i].inside.is_none()
                {
                    self.enter_building(i, anchor, defs);
                }
            }
            Task::Drink | Task::Eat(_) | Task::Remedy(_) => {
                // `consider_needs` always plans these with a route, and `walk` reports arriving
                // only on one. With none left, something cleared it under the task (#386, #563),
                // and nothing would ever set it again: let the need go and plan it afresh
                // rather than hold the task, and the unit, until the meter runs out.
                if self.units[i].route.is_none() {
                    return self.finish_need(i);
                }
                if self.walk(i, defs) {
                    self.consume(i, task, defs);
                }
            }
            Task::Healing(anchor) => {
                let Some(healing) = self
                    .building_at(anchor)
                    .map(|id| defs.building(id))
                    .and_then(|d| d.healing.as_ref())
                else {
                    // The building is gone: taken down (#342, though `remove_building` already
                    // ends this need) or a data change. Moot.
                    return self.finish_need(i);
                };
                let u = &self.units[i];
                if u.needs.health >= healing.stop_at || u.needs.any_low(&defs.needs) {
                    return self.finish_need(i);
                }
                if self.units[i].route.as_ref().is_some_and(|r| !r.is_empty()) {
                    self.walk(i, defs);
                }
                // At the door, the tick it arrives or any tick after while it waits for room:
                // step inside. A no-op once already inside.
                if self.units[i].inside.is_none() {
                    self.enter_building(i, anchor, defs);
                }
            }
            Task::Bath(anchor) => {
                let Some(bath) = self.building_at(anchor).and_then(|id| defs.building(id).bath)
                else {
                    return self.finish_need(i);
                };
                if self.units[i].route.as_ref().is_some_and(|r| !r.is_empty()) {
                    self.walk(i, defs);
                    return;
                }
                if self.units[i].inside.is_none() && !self.enter_building(i, anchor, defs) {
                    return;
                }
                // Inside: soak for `minutes`, then come out rested and warm.
                let u = &mut self.units[i];
                u.work += 1.0 / TICKS_PER_SECOND as f32;
                if u.work >= bath.minutes * 60.0 {
                    u.needs.energy = defs.needs.max();
                    u.warm_until = self.tick + minutes_to_ticks_min1(bath.warm_minutes);
                    u.bath_ready_at = self.tick + minutes_to_ticks_min1(bath.cooldown_minutes);
                    self.finish_need(i);
                }
            }
            // Job tasks never sit in the need slot.
            _ => self.finish_need(i),
        }
    }

    /// The item an eating unit would get from `source`, if it is still there.
    fn food_item(&self, i: usize, source: FoodSource, defs: &Defs) -> Option<ItemId> {
        let item = match source {
            FoodSource::Carried => self.units[i].carrying?.item,
            FoodSource::Stockpile(p) => self.stored.get(&p)?.item,
            FoodSource::Ground(p) => self.ground.get(&p)?.item,
            FoodSource::Forage(p) => self.ripe_harvest(p, defs)?.item,
            FoodSource::Pantry(a) => self.pantry_item(i, a, defs)?,
        };
        defs.food_of(item).map(|_| item)
    }

    /// At the spot: spend time eating or drinking, and when it's done, take the effect.
    fn consume(&mut self, i: usize, task: Task, defs: &Defs) {
        let s = &defs.jobs;
        let required = match task {
            Task::Drink => s.drink_seconds,
            Task::Eat(source) => {
                if self.food_item(i, source, defs).is_none() {
                    // Someone got there first. Look again next tick.
                    return self.finish_need(i);
                }
                let forage = match source {
                    FoodSource::Forage(p) => {
                        // `harvest_at`, not the tile's resource: a shore tile forages fish with
                        // no resource on it (Southern Ways, #727).
                        let h = self.harvest_at(p, defs).expect("food_item found it");
                        h.work / self.work_rate(i, h, defs)
                    }
                    _ => 0.0,
                };
                s.eat_seconds + forage
            }
            Task::Remedy(pos) => {
                if self.stored_at(pos).is_none() {
                    // Someone got there first. Look again next tick.
                    return self.finish_need(i);
                }
                s.eat_seconds
            }
            _ => return self.finish_need(i),
        };
        self.units[i].work += 1.0 / TICKS_PER_SECOND as f32;
        if self.units[i].work < required {
            return;
        }
        match task {
            Task::Drink => {
                self.units[i].needs.drink(&defs.needs);
                // A drink at a spot whose only water is the sea may make the drinker sick
                // (`docs/design/harm.md`, "Sickness: sea water", issue #389).
                self.maybe_catch_sickness(i, defs);
            }
            Task::Eat(source) => {
                let item = self.food_item(i, source, defs).expect("checked above");
                self.take_food(i, source, item, defs);
                // Cooked if the unit is eating within reach of a lit heat source that cooks
                // (#362): a shelter (Earth Tent, Air Shield) never counts.
                let food = defs.food_of(item).expect("food_item checked");
                let cooked = self.is_cooking(self.units[i].pos);
                self.units[i].needs.eat(food, cooked, &defs.needs);
            }
            Task::Remedy(pos) => self.take_remedy(i, pos, defs),
            _ => {}
        }
        self.finish_need(i);
    }

    /// Takes one remedy from the stockpile tile `pos` (issue #390): raises unit `i`'s health by
    /// the matching `RemedyDef::health`, and ends its sickness if the remedy `cures_sickness`.
    /// A no-op (beyond the removal) if `pos` no longer holds a known remedy item -- unreachable
    /// in practice, since `consume` already checked it's there the same tick.
    fn take_remedy(&mut self, i: usize, pos: Pos, defs: &Defs) {
        let Some(item) = self.stored_at(pos).map(|s| s.item) else { return };
        let Some(remedy) = defs.harm.remedies.iter().find(|r| r.item == item).copied() else {
            return;
        };
        let mut s = self.stored.remove(&pos);
        if let Some(stack) = &mut s {
            stack.count -= 1;
            if stack.count == 0 {
                s = None;
            }
        }
        if let Some(s) = s {
            self.stored.insert(pos, s);
        }
        let u = &mut self.units[i];
        u.needs.heal(remedy.health, &defs.needs);
        if remedy.cures_sickness {
            u.sick_until = None;
        }
    }

    /// Removes one `item` from `source`; for a wild resource, clears it and drops the rest of
    /// its yield for hauling.
    pub(super) fn take_food(&mut self, i: usize, source: FoodSource, item: ItemId, defs: &Defs) {
        fn take_one(stack: &mut Option<Stack>) {
            if let Some(s) = stack {
                s.count -= 1;
                if s.count == 0 {
                    *stack = None;
                }
            }
        }
        match source {
            FoodSource::Carried => take_one(&mut self.units[i].carrying),
            FoodSource::Stockpile(p) => {
                let mut s = self.stored.remove(&p);
                take_one(&mut s);
                if let Some(s) = s {
                    self.stored.insert(p, s);
                }
            }
            FoodSource::Ground(p) => {
                let mut s = self.ground.remove(&p);
                take_one(&mut s);
                match s {
                    Some(s) => {
                        self.ground.insert(p, s);
                    }
                    // The last item was eaten: its haul job has nothing left to carry (#600).
                    None => self.take_haul_job(p),
                }
            }
            FoodSource::Pantry(a) => self.take_from_pantry(a, item),
            FoodSource::Forage(p) => {
                let h = self.harvest_at(p, defs).expect("checked");
                let count = self.work_done(p, h, defs);
                if count > 1 {
                    self.place_ground(p, Stack { item, count: count - 1 });
                }
            }
        }
    }

    // ---- jobs ----

    fn run_idle(&mut self, i: usize, defs: &Defs) {
        // Something still in hand (the job that was delivering it is gone): finish the delivery.
        if self.units[i].carrying.is_some() {
            let u = &mut self.units[i];
            u.job_task = Some(Task::Deliver);
            u.route = None;
        } else if self.units[i].is_avatar() && self.gate_on_rite_stage(defs) {
            // The Harmony Rite outranks everything else of his (`docs/design/endgame.md`, "The
            // Avatar"'s priority list puts Avatar tasks first): never a queued `Job`, so there's
            // nothing to claim, just his own place to take (`world/gate.rs`, #238).
            self.units[i].job_task = Some(Task::Rite);
        } else if self.claim_job(i, defs) {
            self.units[i].wander_left = 0;
        } else {
            self.wander(i, defs);
        }
    }

    /// Takes the nearest reachable open job, if any, preferring a lower-priority tier only when
    /// nothing in a higher one is reachable (`job_priority`, issue #284). Returns whether it did.
    pub(super) fn claim_job(&mut self, i: usize, defs: &Defs) -> bool {
        if self.tick < self.units[i].claim_retry_at {
            return false;
        }
        self.prune_jobs(defs);
        if self.jobs.iter().all(|j| j.claimed_by.is_some()) {
            return false;
        }
        if !self.any_job_in_region(i, defs) {
            // Nothing open is on this unit's side of a river, wall or shore: skip the flood.
            self.units[i].claim_retry_at = self.tick + self.retry_ticks(defs);
            return false;
        }
        // The verdict is a function of the key (`ClaimKey`): asked again with nothing changed it
        // is the same job and the same route, so the flood is skipped (#574). That is the common
        // case for a settler that keeps claiming a build site and abandoning it for want of
        // materials, and for one that finds nothing at all.
        let unit_id = self.units[i].id;
        let key = self.claim_key(i, defs);
        let (job_id, route) = match self.claim_memo.get(&unit_id).filter(|(k, _)| *k == key) {
            Some((_, outcome)) => outcome.clone(),
            None => {
                let outcome = self.pick_job(i, defs);
                self.claim_memo.insert(unit_id, (key, outcome.clone()));
                outcome
            }
        };
        let Some(job_id) = job_id else {
            self.units[i].claim_retry_at = self.tick + self.retry_ticks(defs);
            return false;
        };
        let idx = self.job_index(job_id).expect("an unclaimed job in the key");
        let job = &mut self.jobs[idx];
        job.claimed_by = Some(unit_id);
        let task = match job.kind {
            JobKind::Harvest { .. } => Task::Harvest(job.id),
            JobKind::Haul { .. } => Task::Haul(job.id),
            JobKind::Bend { .. } => Task::Bend(job.id),
            JobKind::Build { .. } => Task::Build(job.id),
            JobKind::Study { .. } => Task::Study(job.id),
            JobKind::EndeavorTask { .. } => Task::Endeavor(job.id),
            JobKind::Attend { .. } => Task::Attend(job.id),
            JobKind::Craft { .. } => Task::Craft(job.id),
            JobKind::Sow { .. } => Task::Sow(job.id),
            JobKind::Reap { .. } => Task::Reap(job.id),
            JobKind::Clear { .. } => Task::Clear(job.id),
            JobKind::Chart { .. } => Task::Chart(job.id),
            JobKind::Deconstruct { .. } => Task::Deconstruct(job.id),
            JobKind::Refuel { .. } => Task::Refuel(job.id),
            JobKind::Trade { .. } => Task::Trade(job.id),
        };
        let u = &mut self.units[i];
        u.job_task = Some(task);
        u.route = route;
        true
    }

    /// The nearest unclaimed job in the best tier that unit `i` can take, and the route to it
    /// (`claim_job`, which remembers the answer). Floods from the unit.
    fn pick_job(&self, i: usize, defs: &Defs) -> Picked {
        let flood = self.flood_from(i);
        let is_avatar = self.units[i].is_avatar();
        let priority = |kind: JobKind| {
            if is_avatar { self.avatar_job_priority(kind, defs) } else { job_priority(kind) }
        };
        // The best tier first, so that once something is found, a job in a worse tier is never
        // weighed at all and one in the same tier only has its stand looked for as far as the
        // best cost so far (`check_job`'s `limit`): the flood stops growing there. The answer is
        // the least `(priority, cost, id)` whatever order the jobs are weighed in.
        // Within a tier, nearest by straight-line distance first, so the bound tightens early.
        let from = self.units[i].pos;
        let near = |kind: JobKind| job_pos(kind).map_or(i32::MAX, |p| from.chebyshev(p));
        let mut open: Vec<(u8, i32, &Job)> = self
            .jobs
            .iter()
            .filter(|j| j.claimed_by.is_none())
            // The Avatar only ever takes a bend or a gate task (#237).
            .filter(|j| {
                !is_avatar || matches!(j.kind, JobKind::Bend { .. } | JobKind::EndeavorTask { .. })
            })
            .map(|j| (priority(j.kind), near(j.kind), j))
            .collect();
        open.sort_by_key(|&(p, d, _)| (p, d));
        let mut room_cache: Vec<(ItemId, bool)> = Vec::new();
        let mut best: Option<((u8, u32, JobId), Pos)> = None;
        // A bending beast ranks its jobs by the walk from its home, not from where it stands,
        // so a dragged line is bored outward from the den (`World::beast_home_flood`, #697).
        let home_flood = self.beast_home_flood(i, defs);
        for (priority, _, job) in open {
            let limit = match best {
                Some(((p, _, _), _)) if p < priority => break,
                Some(_) if home_flood.is_some() => None,
                Some(((_, cost, _), _)) => Some(cost),
                None => None,
            };
            let found = match self.check_job(i, job.kind, &flood, &mut room_cache, limit, defs) {
                Check::Take(cost, stand) => {
                    let ranked = home_flood
                        .as_ref()
                        .map_or(cost, |home| home.cost(stand).unwrap_or(u32::MAX));
                    Some((ranked, stand))
                }
                Check::Skip | Check::Barred(_) => None,
            };
            if let Some((cost, stand)) = found {
                let key = (priority, cost, job.id);
                if best.is_none_or(|b| key < b.0) {
                    best = Some((key, stand));
                }
            }
        }
        match best {
            Some(((_, _, id), stand)) => (Some(id), flood.path_to(stand).map(route_from)),
            None => (None, None),
        }
    }

    /// Why `unit` is idle (#337), for a frontend's selection bar: `None` once it isn't (it has a
    /// `Task`) or nothing designated explains it (the odd tick a claimable job hasn't been
    /// picked up yet, a chained bend not yet reshaped). Otherwise weighs every unclaimed job
    /// exactly as `claim_job` does (`World::check_job`) -- ripe, in stage, on this unit's side of
    /// a wall -- and the first one still blocked even though a stand tile is in reach names why: a bend
    /// past the unit's current chi (`OutOfChi`; a recast too fresh to restoke, #359, is folded
    /// into this too, since `unit_can_bend` doesn't separate the two), a haul with no
    /// stockpile room anywhere for its item (`StockpileFull`), or a harvest marked `risk` with
    /// the unit's own health below `fit_for_heavy` (`TooHurt`, issue #388). Short of that,
    /// `Unreachable` covers both a genuinely stranded designation and the Avatar standing idle
    /// beside jobs that are never his (`docs/design/endgame.md`, "The Avatar").
    pub fn idle_reason(&self, unit: UnitId, defs: &Defs) -> Option<IdleReason> {
        let i = self.units.iter().position(|u| u.id == unit)?;
        if self.units[i].task().is_some() {
            return None;
        }
        let unclaimed: Vec<&Job> = self.jobs.iter().filter(|j| j.claimed_by.is_none()).collect();
        if unclaimed.is_empty() {
            return Some(IdleReason::NoWork);
        }
        // The verdict is a function of the key, so asking again under the same key (the balance
        // probe asks once a minute per idle settler) gets the last answer (#574).
        let key = self.claim_key(i, defs);
        if let Some(reason) = self.idle_memo.get(unit, &key) {
            return reason;
        }
        let reason = self.idle_reason_for(i, &unclaimed, defs);
        self.idle_memo.put(unit, key, reason);
        reason
    }

    /// `idle_reason`'s weighing of every unclaimed job, for a unit with no task.
    fn idle_reason_for(&self, i: usize, unclaimed: &[&Job], defs: &Defs) -> Option<IdleReason> {
        let flood = self.flood_from(i);
        let mut rooms: Vec<(ItemId, bool)> = Vec::new();
        let (mut chi_blocked, mut room_blocked, mut too_hurt) = (false, false, false);
        for job in unclaimed {
            match self.check_job(i, job.kind, &flood, &mut rooms, None, defs) {
                Check::Take(..) => return None,
                Check::Barred(Some(IdleReason::StockpileFull)) => room_blocked = true,
                Check::Barred(Some(IdleReason::OutOfChi)) => chi_blocked = true,
                Check::Barred(Some(IdleReason::TooHurt)) => too_hurt = true,
                Check::Barred(_) | Check::Skip => {}
            }
        }
        Some(if room_blocked {
            IdleReason::StockpileFull
        } else if chi_blocked {
            IdleReason::OutOfChi
        } else if too_hurt {
            IdleReason::TooHurt
        } else {
            IdleReason::Unreachable
        })
    }

    /// A cheap, conservative check against the connectivity cache: false only if no unclaimed
    /// job could possibly be reached from unit `i`'s tile, so claiming can skip the flood. True
    /// means "maybe" (the flood still decides), and it is always true when the unit stands on a
    /// tile it couldn't walk onto, which the cache has no region for.
    fn any_job_in_region(&mut self, i: usize, defs: &Defs) -> bool {
        let from = self.units[i].pos;
        self.regions();
        let Some(regions) = self.regions.as_ref() else { return true };
        let Some(here) = regions.region(from) else { return true };
        let in_here = |p: Pos| regions.region(p) == Some(here);
        // Inside a walkable area, else beside it (see `stand`).
        let can_stand_in = |(foot, walkable): (Footprint, bool)| {
            if walkable { foot.tiles().any(in_here) } else { foot.ring().any(in_here) }
        };
        let can_stand = |pos: Pos| {
            let walkable = self.map.get(pos).is_some_and(|t| self.rules.passable(t));
            can_stand_in((Footprint::single(pos), walkable))
        };
        let can_stand_building = |pos: Pos| can_stand_in(self.work_area(pos, defs));
        let can_stand_rule = |rule: StandRule| match rule {
            StandRule::Tile(pos) => can_stand(pos),
            // A seat's door is only known to the flood; somewhere to stand at the building is
            // the conservative question.
            StandRule::Building(pos) | StandRule::Entrance(pos) => can_stand_building(pos),
            StandRule::Exact(pos) => in_here(pos),
            StandRule::Beside(pos) => can_stand_in((Footprint::single(pos), false)),
        };
        self.jobs.iter().filter(|j| j.claimed_by.is_none()).any(|job| {
            let Some(gate) = self.job_gate(i, job.kind, defs) else { return false };
            can_stand_rule(gate.stand)
                && match gate.bar {
                    Bar::Free => true,
                    Bar::Reason(_) => false,
                    Bar::Room(item) => {
                        if Self::is_readable(item, defs) {
                            self.study_halls().any(can_stand_building)
                        } else {
                            self.zones
                                .iter()
                                .any(|&z| in_here(z) && self.room_at(z, item, defs) > 0)
                        }
                    }
                }
        })
    }

    /// Is `item` one that is read at a study hall instead of stockpiled (issue #24)?
    pub(super) fn is_readable(item: ItemId, defs: &Defs) -> bool {
        defs.item(item).is_readable()
    }

    /// Anchors of the study halls that stand, one per hall, in `Pos` order (`Kind::Study`:
    /// seats, and no chart). Every reader asks "any" or "nearest", so the order doesn't matter.
    pub(super) fn study_halls(&self) -> impl Iterator<Item = Pos> + '_ {
        self.of_kind(Kind::Study).map(|b| b.anchor)
    }

    /// Is `pos` on or beside a study hall? Where a unit that walked a scroll to the hall ends up.
    fn at_study_hall(&self, pos: Pos) -> bool {
        self.study_halls()
            .any(|h| self.footprint_at(h).unwrap_or(Footprint::single(h)).distance(pos) <= 1)
    }

    /// The anchor of a study hall `pos` is on or beside (`at_study_hall`), the first in `Pos`
    /// order if several: where a teaching scroll read here is shelved (#737).
    fn study_hall_at(&self, pos: Pos) -> Option<Pos> {
        self.study_halls()
            .find(|&h| self.footprint_at(h).unwrap_or(Footprint::single(h)).distance(pos) <= 1)
    }

    /// Where a unit with `item` in hand takes it: a study hall for a readable item (to stand
    /// beside it, with its cost), a stockpile tile with room for anything else.
    pub(super) fn reachable_dest(
        &self,
        flood: &Flood,
        item: ItemId,
        defs: &Defs,
    ) -> Option<(u32, Pos)> {
        if Self::is_readable(item, defs) {
            self.study_halls().filter_map(|h| self.stand_building(flood, h, defs)).min()
        } else {
            self.reachable_room(flood, item, defs)
        }
    }

    /// The nearest stockpile tile with room for `item`, with its cost.
    fn reachable_room(&self, flood: &Flood, item: ItemId, defs: &Defs) -> Option<(u32, Pos)> {
        self.zones
            .iter()
            .filter(|&&z| self.room_at(z, item, defs) > 0)
            .filter_map(|&z| flood.cost(z).map(|c| (c, z)))
            .min()
    }

    pub(super) fn run_job(&mut self, i: usize, task: Task, defs: &Defs) {
        match task {
            Task::Harvest(id) => self.run_harvest(i, id, defs),
            Task::Haul(id) => self.run_haul(i, id, defs),
            Task::Bend(id) => self.run_bend(i, id, defs),
            Task::Build(id) => self.run_build(i, id, defs),
            Task::Study(id) => self.run_study(i, id, defs),
            Task::Endeavor(id) => self.run_gate_task(i, id, defs),
            Task::Attend(id) => self.run_attend(i, id, defs),
            Task::Rite => self.run_rite(i, defs),
            Task::Craft(id) => self.run_craft(i, id, defs),
            Task::Sow(id) => self.run_sow(i, id, defs),
            Task::Reap(id) => self.run_reap(i, id, defs),
            Task::Clear(id) => self.run_clear(i, id, defs),
            Task::Chart(id) => self.run_chart(i, id, defs),
            Task::Deconstruct(id) => self.run_deconstruct(i, id, defs),
            Task::Refuel(id) => self.run_refuel(i, id, defs),
            Task::Trade(id) => self.run_trade(i, id, defs),
            Task::Deliver => self.run_deliver(i, defs),
            Task::Chute(head) => self.run_chute(i, head, defs),
            Task::Balloon(dock) => self.run_balloon(i, dock, defs),
            // Need tasks never sit in the job slot.
            _ => self.finish_job(i),
        }
    }

    fn run_harvest(&mut self, i: usize, id: JobId, defs: &Defs) {
        let Some(idx) = self.job_index(id) else { return self.finish_job(i) };
        let JobKind::Harvest { pos } = self.jobs[idx].kind else { return self.finish_job(i) };
        let Some(h) = self.harvest_at(pos, defs) else {
            // The resource is gone, so the job is moot.
            self.jobs.remove(idx);
            return self.finish_job(i);
        };
        if self.resting.contains_key(&pos) {
            // Not ripe (it can't have been when the job was claimed): wait for it.
            return self.abandon_job(i, id, defs);
        }
        if !self.go_or_abandon(i, id, StandRule::Tile(pos), defs) {
            return;
        }
        let rate = self.work_rate(i, h, defs);
        let job = &mut self.jobs[idx];
        job.progress += rate / TICKS_PER_SECOND as f32;
        if job.progress < h.work {
            return;
        }
        self.jobs.remove(idx);
        if self.map[pos].resource.is_none() {
            // Shore fishing (#727): nothing is used up or rests, so a standing order queues
            // the next catch at once.
            self.tallies.count_harvest(h.item, h.count);
            self.tallies.count_school("shore_fish", h.count);
            self.place_ground(pos, Stack { item: h.item, count: h.count });
            if self.standing_harvest.contains(&pos) {
                self.add_job(JobKind::Harvest { pos });
            }
            return self.finish_job(i);
        }
        let count = self.work_done(pos, h, defs);
        self.place_ground(pos, Stack { item: h.item, count });
        self.maybe_injure(i, pos, h, defs);
        self.finish_job(i);
    }

    /// One tick of a unit clearing a tile (`docs/design/clearing.md`, issue #411/#424): walk to
    /// it, work the clear's own `work` seconds (the harvest's, for a non-renewable; `clear.work`
    /// for a renewable), then remove the resource for good (`clear_done`). Unlike `run_harvest`,
    /// a resting resource never abandons the job: it can be cleared right away.
    fn run_clear(&mut self, i: usize, id: JobId, defs: &Defs) {
        let Some(idx) = self.job_index(id) else { return self.finish_job(i) };
        let JobKind::Clear { pos } = self.jobs[idx].kind else { return self.finish_job(i) };
        let Some(h) = self.map[pos].resource.and_then(|r| defs.harvest(r)) else {
            // The resource is gone, so the job is moot.
            self.jobs.remove(idx);
            return self.finish_job(i);
        };
        if !self.go_or_abandon(i, id, StandRule::Tile(pos), defs) {
            return;
        }
        // A renewable's clear has no modifier of its own (`data/jobs.toml`'s `[harvest.clear]`
        // names none); a non-renewable's clear is its harvest, at the harvest's own rate.
        let (work, rate) = match h.clear {
            Some(c) => (c.work, 1.0),
            None => (h.work, self.work_rate(i, h, defs)),
        };
        let job = &mut self.jobs[idx];
        job.progress += rate / TICKS_PER_SECOND as f32;
        if job.progress < work {
            return;
        }
        self.jobs.remove(idx);
        self.clear_done(i, pos, h, defs);
        self.finish_job(i);
    }

    fn run_haul(&mut self, i: usize, id: JobId, defs: &Defs) {
        let Some(idx) = self.job_index(id) else { return self.finish_job(i) };
        let JobKind::Haul { pos } = self.jobs[idx].kind else { return self.finish_job(i) };
        let Some(&stack) = self.ground.get(&pos) else {
            self.jobs.remove(idx);
            return self.finish_job(i);
        };
        if !self.go_or_abandon(i, id, StandRule::Exact(pos), defs) {
            return;
        }
        let take = stack.count.min(self.carry_capacity(i, defs));
        if take == stack.count {
            self.ground.remove(&pos);
            self.jobs.remove(idx);
        } else {
            self.ground.get_mut(&pos).expect("just read").count -= take;
            self.jobs[idx].claimed_by = None;
            self.wake_idle();
        }
        let u = &mut self.units[i];
        u.carrying = Some(Stack { item: stack.item, count: take });
        u.job_task = Some(Task::Deliver);
        u.route = None;
    }

    fn run_deliver(&mut self, i: usize, defs: &Defs) {
        let Some(stack) = self.units[i].carrying else { return self.finish_job(i) };
        if self.units[i].route.is_none() {
            let flood = self.flood_from(i);
            let dest = self.reachable_dest(&flood, stack.item, defs);
            // A chute whose head plus foot walk beats walking straight to stores (`chute.rs`).
            if let Some((head, path)) =
                self.chute_for(i, &flood, stack.item, dest.map(|(c, _)| c), defs)
            {
                let u = &mut self.units[i];
                u.job_task = Some(Task::Chute(head));
                u.route = Some(route_from(path));
                return;
            }
            // Or a Balloon Dock whose flight beats it the same way (`balloon.rs`).
            if let Some((dock, path)) =
                self.balloon_for(i, &flood, stack, dest.map(|(c, _)| c), defs)
            {
                let u = &mut self.units[i];
                u.job_task = Some(Task::Balloon(dock));
                u.route = Some(route_from(path));
                return;
            }
            let route = dest.and_then(|(_, z)| flood.path_to(z)).map(route_from);
            match route {
                Some(route) => self.units[i].route = Some(route),
                None => {
                    // No stockpile with room that it can reach: put it down for later.
                    self.units[i].carrying = None;
                    let pos = self.units[i].pos;
                    self.place_ground(pos, stack);
                    return self.finish_job(i);
                }
            }
        }
        if !self.walk(i, defs) {
            return;
        }
        let pos = self.units[i].pos;
        if let Some(reveals) = defs.item(stack.item).reveals {
            // A teaching scroll (#737): read at the hall, each one opens a draft. It adds no
            // knowledge and doesn't count toward the scrolls sign.
            if let Some(hall) = self.study_hall_at(pos) {
                for _ in 0..stack.count {
                    self.read_teaching_scroll(hall, reveals, defs);
                }
                self.units[i].carrying = None;
                self.finish_job(i);
            } else {
                self.units[i].route = None;
            }
            return;
        }
        if let Some(points) = defs.item(stack.item).knowledge {
            // A readable item is used up at the hall, all of it, and never stockpiled. If the
            // hall went away on the way, plan again (no hall left: it is put down).
            if self.at_study_hall(pos) {
                // A bill short of it takes it first: no knowledge, and not read (#715).
                let billed = self.research.pay_bill(stack.item, stack.count, defs);
                let read = stack.count - billed;
                let millis = u64::from(points) * u64::from(read) * MILLIS_PER_POINT;
                self.research.add_knowledge(millis);
                self.tallies.count_scrolls(read);
                self.units[i].carrying = None;
                self.finish_job(i);
            } else {
                self.units[i].route = None;
            }
            return;
        }
        let put = if self.zones.contains(&pos) {
            stack.count.min(self.room_at(pos, stack.item, defs))
        } else {
            0
        };
        if put > 0 {
            self.stored
                .entry(pos)
                .and_modify(|s| s.count += put)
                .or_insert(Stack { item: stack.item, count: put });
        }
        let rest = stack.count - put;
        let u = &mut self.units[i];
        if rest == 0 {
            u.carrying = None;
            self.finish_job(i);
        } else {
            // That tile filled up on the way (or this isn't a stockpile tile any more).
            u.carrying = Some(Stack { count: rest, ..stack });
            u.route = None;
        }
    }

    /// `job_priority` for the Avatar (`claim_job` only calls this once `kind` is already known to
    /// be a `Bend` or an `EndeavorTask`, the only two he ever claims, #237): his own priority list
    /// is Avatar tasks first, then any nation's pillar task, then bend jobs of any ability.
    fn avatar_job_priority(&self, kind: JobKind, defs: &Defs) -> u8 {
        match kind {
            JobKind::EndeavorTask { task } => match self.gate_task(task, defs) {
                Some((def, _)) if def.element == TaskWho::Avatar => 0,
                _ => 1,
            },
            JobKind::Bend { .. } => 2,
            _ => unreachable!("claim_job excludes every other kind for the Avatar"),
        }
    }
}

/// The one place rail is decided (#135). Every unit runs along rail, loaded or not (owner,
/// 2026-10-25: rail is a road, not only a freight line). To make it a freight line again, return
/// `cart` only when `_u.carrying.is_some()` and `_on_foot` otherwise.
pub(super) fn rules_for<'a>(
    _on_foot: &'a MoveRules,
    cart: &'a MoveRules,
    _u: &Unit,
) -> &'a MoveRules {
    cart
}

/// `claim_job`'s tiers, lowest first: a study seat (#284) is filler for hands with nothing more
/// pressing to do, so it only wins a tie-free pick when no harvest, haul, bend, build or
/// endeavor task is reachable at all. Attending the Harmony Rite (#238) is lower still: "nothing
/// more urgent" (`docs/design/endgame.md`), so it loses to a study seat too. Lower is picked
/// first.
/// Where `kind`'s work is, for ordering (`pick_job`); `None` for an endeavor task, whose place
/// is the gate's.
fn job_pos(kind: JobKind) -> Option<Pos> {
    match kind {
        JobKind::Harvest { pos }
        | JobKind::Haul { pos }
        | JobKind::Bend { pos, .. }
        | JobKind::Build { pos }
        | JobKind::Study { pos }
        | JobKind::Attend { pos }
        | JobKind::Craft { pos }
        | JobKind::Sow { pos }
        | JobKind::Reap { pos }
        | JobKind::Clear { pos }
        | JobKind::Chart { pos }
        | JobKind::Deconstruct { pos }
        | JobKind::Refuel { pos }
        | JobKind::Trade { pos, .. } => Some(pos),
        JobKind::EndeavorTask { .. } => None,
    }
}

fn job_priority(kind: JobKind) -> u8 {
    match kind {
        JobKind::Study { .. } | JobKind::Chart { .. } => 1,
        JobKind::Attend { .. } => 2,
        JobKind::Harvest { .. }
        | JobKind::Haul { .. }
        | JobKind::Bend { .. }
        | JobKind::Build { .. }
        | JobKind::Craft { .. }
        | JobKind::EndeavorTask { .. }
        | JobKind::Sow { .. }
        | JobKind::Reap { .. }
        | JobKind::Clear { .. }
        | JobKind::Deconstruct { .. }
        | JobKind::Refuel { .. }
        | JobKind::Trade { .. } => 0,
    }
}

mod eligibility;
#[cfg(test)]
mod tests;

/// `StepGrid`s for the map as of `map_rev`, one per `MoveRules` asked for (walking and hauling),
/// shared by every `flood_from` until the map changes. Not saved, and a clone starts empty.
#[derive(Default)]
pub(in crate::world) struct StepGrids(
    std::sync::Mutex<Vec<(u64, MoveRules, std::sync::Arc<crate::flood::StepGrid>)>>,
);

impl StepGrids {
    pub(super) fn get(
        &self,
        map: &crate::mapgen::TileMap,
        rev: u64,
        rules: &MoveRules,
    ) -> std::sync::Arc<crate::flood::StepGrid> {
        let mut grids = self.0.lock().unwrap_or_else(|e| e.into_inner());
        grids.retain(|(r, _, _)| *r == rev);
        if let Some((_, _, g)) = grids.iter().find(|(_, r, _)| r == rules) {
            return std::sync::Arc::clone(g);
        }
        let g = std::sync::Arc::new(crate::flood::StepGrid::new(map, rules));
        grids.push((rev, rules.clone(), std::sync::Arc::clone(&g)));
        g
    }
}

impl Clone for StepGrids {
    fn clone(&self) -> Self {
        Self::default()
    }
}

impl std::fmt::Debug for StepGrids {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("StepGrids")
    }
}
