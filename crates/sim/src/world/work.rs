//! What units do all day: the job queue, claiming, needs pre-empting jobs, and hauling.
//!
//! The shape, which follows the 2013 version's task queue:
//!
//! * The player **designates** work (`designate_harvest`) and a stockpile zone
//!   (`designate_stockpile`). Each harvest designation becomes a `Job` in the queue.
//! * An **idle unit claims the nearest reachable job** (by path cost, ties by job id). It walks
//!   to a tile beside the target, works, and the yield drops on the ground as an item stack.
//!   Every stack lying outside a stockpile gets a **haul job**; a unit picks up as much as it can
//!   carry (`carry_capacity` modifier) and walks it to the nearest stockpile tile with room.
//! * **Needs pre-empt jobs.** When hunger, thirst or energy drops to its `low` level (see
//!   `data/needs.toml`) the unit sets its job aside, deals with the need, and goes back. The job
//!   stays claimed by that unit and keeps its progress, so "resume" is literal.
//! * **Work speed** is `1.0 / TICKS_PER_SECOND` work-seconds per tick times the nation modifier
//!   named by the harvest in `data/jobs.toml` (quarrying uses `earthworks`).
//!
//! Hungry units eat from the closest of: food they carry, food in a stockpile, food lying on the
//! ground, or a wild food resource they work for themselves. Raw food always works (owner rule,
//! #3): nobody starves because their nation lacks fire. Cooking would need a hearth, which
//! doesn't exist yet, so every meal is raw for now.
//!
//! Everything is deterministic: units act in id order, searches break ties by position or id, and
//! the only randomness is the idle wander (`world.units` stream).

use crate::flood::Flood;
use crate::jobs::{HarvestDef, ItemId, JobKind};
use crate::path::{DIAGONAL, NEIGHBORS, STRAIGHT};

use super::*;

/// How far from a drop point a stack may be placed when its own tile is taken, in tiles.
const GROUND_SEARCH_RADIUS: i32 = 8;

/// Where a hungry unit gets its food.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FoodSource {
    /// Something it is already carrying.
    Carried,
    /// A stockpile tile.
    Stockpile(Pos),
    /// An item stack on the ground.
    Ground(Pos),
    /// A wild food resource on this tile, worked on the spot.
    Forage(Pos),
}

/// What a unit is doing. Needs outrank jobs; `Unit::task` reports the one in effect.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Task {
    /// Working a designated harvest.
    Harvest(JobId),
    /// Walking to pick up a stack for a haul job.
    Haul(JobId),
    /// Carrying a stack to a stockpile tile.
    Deliver,
    Eat(FoodSource),
    Drink,
    Sleep,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DesignateError {
    OutOfBounds,
    /// There is no resource with a harvest entry on that tile.
    NothingToHarvest,
    AlreadyDesignated,
    /// Stockpiles need a walkable tile with nothing on it.
    CannotStockpile,
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

    pub fn is_sleeping(&self) -> bool {
        self.need_task == Some(Task::Sleep)
    }
}

/// The remaining steps of a path, next step last.
fn route_from(path: Vec<Pos>) -> Vec<Pos> {
    path.into_iter().skip(1).rev().collect()
}

/// Commands and queries. (A `Command` enum arrives with #5; until then these are the API.)
impl World {
    /// Designated work that isn't finished, oldest first.
    pub fn jobs(&self) -> &[Job] {
        &self.jobs
    }

    pub fn job(&self, id: JobId) -> Option<&Job> {
        self.jobs.binary_search_by_key(&id, |j| j.id).ok().map(|i| &self.jobs[i])
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

    /// Designates the resource on `pos` for harvesting (chop, quarry, gather, mine).
    pub fn designate_harvest(&mut self, pos: Pos, defs: &Defs) -> Result<JobId, DesignateError> {
        let tile = self.map.get(pos).ok_or(DesignateError::OutOfBounds)?;
        if !tile.resource.is_some_and(|r| defs.harvest(r).is_some()) {
            return Err(DesignateError::NothingToHarvest);
        }
        if self.jobs.iter().any(|j| j.kind == JobKind::Harvest { pos }) {
            return Err(DesignateError::AlreadyDesignated);
        }
        Ok(self.add_job(JobKind::Harvest { pos }))
    }

    /// Withdraws the harvest designation on `pos`. Work done on it is lost. Returns whether there
    /// was one.
    pub fn cancel_harvest(&mut self, pos: Pos) -> bool {
        let before = self.jobs.len();
        self.jobs.retain(|j| j.kind != JobKind::Harvest { pos });
        self.jobs.len() != before
    }

    /// Marks a tile as stockpile. Anything lying on the ground gets hauled to it (or another
    /// stockpile tile).
    pub fn designate_stockpile(&mut self, pos: Pos) -> Result<(), DesignateError> {
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
}

impl World {
    fn add_job(&mut self, kind: JobKind) -> JobId {
        let id = JobId(self.next_job);
        self.next_job += 1;
        self.jobs.push(Job { id, kind, claimed_by: None, progress: 0.0 });
        self.wake_idle();
        id
    }

    fn job_index(&self, id: JobId) -> Option<usize> {
        self.jobs.binary_search_by_key(&id, |j| j.id).ok()
    }

    /// New work exists, so idle units shouldn't wait out their retry timer.
    fn wake_idle(&mut self) {
        for u in &mut self.units {
            u.claim_retry_at = 0;
        }
    }

    fn retry_ticks(&self, defs: &Defs) -> u64 {
        (defs.jobs.retry_seconds * TICKS_PER_SECOND as f32).ceil() as u64
    }

    fn ensure_haul_job(&mut self, pos: Pos) {
        if !self.jobs.iter().any(|j| j.kind == JobKind::Haul { pos }) {
            self.add_job(JobKind::Haul { pos });
        }
    }

    /// Puts a stack on the ground at `near`, or the closest free tile (one with no other kind of
    /// item, and not a stockpile tile), and queues a haul for it.
    fn place_ground(&mut self, near: Pos, stack: Stack) {
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
    fn room_at(&self, zone: Pos, item: ItemId, defs: &Defs) -> u32 {
        let cap = defs.jobs.stockpile_stack;
        match self.stored.get(&zone) {
            None => cap,
            Some(s) if s.item == item => cap.saturating_sub(s.count),
            Some(_) => 0,
        }
    }

    fn carry_capacity(&self, i: usize, defs: &Defs) -> u32 {
        let m = defs.nation(self.units[i].nation).modifiers.carry_capacity;
        ((defs.jobs.base_carry as f32 * m).round() as u32).max(1)
    }

    /// Work-seconds per second for this unit on this harvest: the nation modifier it names.
    fn work_rate(&self, i: usize, h: &HarvestDef, defs: &Defs) -> f32 {
        let mods = &defs.nation(self.units[i].nation).modifiers;
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
        let (map, ground) = (&self.map, &self.ground);
        self.jobs.retain(|j| {
            j.claimed_by.is_some()
                || match j.kind {
                    JobKind::Harvest { pos } => {
                        map[pos].resource.is_some_and(|r| defs.harvest(r).is_some())
                    }
                    JobKind::Haul { pos } => ground.contains_key(&pos),
                }
        });
    }

    /// The cheapest tile to stand on to work `pos`: the tile itself if units can enter it, else
    /// one of its neighbours (a tree can't be walked onto, but can be chopped from beside it).
    fn stand(&self, flood: &Flood, pos: Pos) -> Option<(u32, Pos)> {
        if self.map.get(pos).is_some_and(|t| self.rules.passable(t)) {
            return flood.cost(pos).map(|c| (c, pos));
        }
        NEIGHBORS
            .iter()
            .filter_map(|&(dx, dy)| {
                let q = Pos::new(pos.x + dx, pos.y + dy);
                flood.cost(q).map(|c| (c, q))
            })
            .min()
    }

    fn flood_from(&self, i: usize) -> Flood {
        Flood::new(&self.map, self.units[i].pos, &self.rules)
    }

    fn finish_need(&mut self, i: usize) {
        let u = &mut self.units[i];
        u.need_task = None;
        u.route = None;
        u.work = 0.0;
    }

    fn finish_job(&mut self, i: usize) {
        let u = &mut self.units[i];
        u.job_task = None;
        u.route = None;
    }

    /// Lets go of a claimed job without finishing it (it became unreachable).
    fn abandon_job(&mut self, i: usize, id: JobId, defs: &Defs) {
        if let Some(j) = self.job_index(id) {
            self.jobs[j].claimed_by = None;
        }
        self.finish_job(i);
        self.units[i].claim_retry_at = self.tick + self.retry_ticks(defs);
    }

    /// One tick of walking along the route. True once the unit is at the end of it (or the
    /// route is empty to begin with); arriving costs the tick the last step landed on.
    fn walk(&mut self, i: usize, defs: &Defs) -> bool {
        let speed = defs.nation(self.units[i].nation).modifiers.move_speed;
        let u = &mut self.units[i];
        let Some(route) = u.route.as_mut() else { return false };
        let Some(&next) = route.last() else {
            u.move_progress = 0.0;
            return true;
        };
        u.move_progress += speed;
        let diagonal = next.x != u.pos.x && next.y != u.pos.y;
        let step = if diagonal { DIAGONAL } else { STRAIGHT };
        let tile_cost = self.rules.tile_cost(&self.map[next]).unwrap_or(1);
        let needed = BASE_TICKS_PER_STEP * (step * tile_cost) as f32 / STRAIGHT as f32;
        if u.move_progress >= needed {
            u.move_progress -= needed;
            u.pos = next;
            route.pop();
        }
        false
    }

    /// One unit's turn: meet a need if one is low, else carry on with its job, else find one.
    pub(super) fn act(&mut self, i: usize, defs: &Defs) {
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
        for (_, need) in urgent {
            match self.plan_need(i, need, defs) {
                Some((task, route)) => {
                    let u = &mut self.units[i];
                    u.need_task = Some(task);
                    u.route = Some(route);
                    u.work = 0.0;
                    return;
                }
                None => {
                    self.units[i].need_retry_at[need as usize] = self.tick + self.retry_ticks(defs);
                }
            }
        }
    }

    fn plan_need(&self, i: usize, need: Need, defs: &Defs) -> Option<(Task, Vec<Pos>)> {
        match need {
            Need::Energy => Some((Task::Sleep, Vec::new())),
            Need::Thirst => self.plan_drink(i),
            Need::Hunger => self.plan_food(i, defs),
        }
    }

    /// The nearest tile beside water.
    fn plan_drink(&self, i: usize) -> Option<(Task, Vec<Pos>)> {
        let flood = self.flood_from(i);
        let mut best: Option<(u32, Pos)> = None;
        for (p, tile) in self.map.iter() {
            let Some(c) = flood.cost(p) else { continue };
            if !self.rules.passable(tile) || best.is_some_and(|b| (c, p) >= b) {
                continue;
            }
            if self.map.neighbors(p, true).any(|q| self.map[q].terrain.is_water()) {
                best = Some((c, p));
            }
        }
        let (_, stand) = best?;
        Some((Task::Drink, route_from(flood.path_to(stand)?)))
    }

    /// The closest food: carried (free), then stockpile, ground and wild resources by walking
    /// cost, preferring them in that order on a tie.
    fn plan_food(&self, i: usize, defs: &Defs) -> Option<(Task, Vec<Pos>)> {
        let u = &self.units[i];
        if u.carrying.is_some_and(|s| defs.food_of(s.item).is_some()) {
            return Some((Task::Eat(FoodSource::Carried), Vec::new()));
        }
        let flood = self.flood_from(i);
        let mut best: Option<((u32, u8, Pos), FoodSource, Pos)> = None;
        let mut consider = |rank: u8, pos: Pos, source: FoodSource, stand: Option<(u32, Pos)>| {
            let Some((cost, stand)) = stand else { return };
            let key = (cost, rank, pos);
            if best.as_ref().is_none_or(|b| key < b.0) {
                best = Some((key, source, stand));
            }
        };
        for (&pos, s) in &self.stored {
            if defs.food_of(s.item).is_some() {
                consider(0, pos, FoodSource::Stockpile(pos), self.stand(&flood, pos));
            }
        }
        for (&pos, s) in &self.ground {
            if defs.food_of(s.item).is_some() {
                consider(1, pos, FoodSource::Ground(pos), self.stand(&flood, pos));
            }
        }
        for (pos, tile) in self.map.iter() {
            let Some(h) = tile.resource.and_then(|r| defs.harvest(r)) else { continue };
            if defs.food_of(h.item).is_none() || self.forage_taken(pos, i) {
                continue;
            }
            consider(2, pos, FoodSource::Forage(pos), self.stand(&flood, pos));
        }
        let (_, source, stand) = best?;
        Some((Task::Eat(source), route_from(flood.path_to(stand)?)))
    }

    /// True if the resource on `pos` is spoken for: designated for harvest, or another unit is
    /// already on its way to forage it.
    fn forage_taken(&self, pos: Pos, me: usize) -> bool {
        self.jobs.iter().any(|j| j.kind == JobKind::Harvest { pos })
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
                    self.finish_need(i);
                }
            }
            Task::Drink | Task::Eat(_) => {
                if self.walk(i, defs) {
                    self.consume(i, task, defs);
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
            FoodSource::Forage(p) => defs.harvest(self.map.get(p)?.resource?)?.item,
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
                        let h = self.map[p].resource.and_then(|r| defs.harvest(r)).unwrap();
                        h.work / self.work_rate(i, h, defs)
                    }
                    _ => 0.0,
                };
                s.eat_seconds + forage
            }
            _ => return self.finish_need(i),
        };
        self.units[i].work += 1.0 / TICKS_PER_SECOND as f32;
        if self.units[i].work < required {
            return;
        }
        match task {
            Task::Drink => self.units[i].needs.drink(&defs.needs),
            Task::Eat(source) => {
                let item = self.food_item(i, source, defs).expect("checked above");
                self.take_food(i, source, item, defs);
                // Always raw: cooking needs a hearth, which doesn't exist yet.
                let food = defs.food_of(item).expect("food_item checked");
                self.units[i].needs.eat(food, false, &defs.needs);
            }
            _ => {}
        }
        self.finish_need(i);
    }

    /// Removes one `item` from `source`; for a wild resource, clears it and drops the rest of
    /// its yield for hauling.
    fn take_food(&mut self, i: usize, source: FoodSource, item: ItemId, defs: &Defs) {
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
                if let Some(s) = s {
                    self.ground.insert(p, s);
                }
            }
            FoodSource::Forage(p) => {
                let tile = &mut self.map[p];
                let h = defs.harvest(tile.resource.take().expect("checked")).expect("checked");
                if h.count > 1 {
                    self.place_ground(p, Stack { item, count: h.count - 1 });
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
        } else if !self.claim_job(i, defs) {
            self.wander(i, defs);
        }
    }

    /// Takes the nearest reachable open job, if any. Returns whether it did.
    fn claim_job(&mut self, i: usize, defs: &Defs) -> bool {
        if self.tick < self.units[i].claim_retry_at {
            return false;
        }
        self.prune_jobs(defs);
        if self.jobs.iter().all(|j| j.claimed_by.is_some()) {
            return false;
        }
        let flood = self.flood_from(i);
        // Whether a stockpile tile with room for each item can be reached, asked once per item.
        let mut room_cache: Vec<(ItemId, bool)> = Vec::new();
        let mut best: Option<((u32, JobId), usize, Pos)> = None;
        for (idx, job) in self.jobs.iter().enumerate() {
            if job.claimed_by.is_some() {
                continue;
            }
            let found = match job.kind {
                JobKind::Harvest { pos } => self.stand(&flood, pos),
                JobKind::Haul { pos } => {
                    let item = self.ground[&pos].item;
                    let room = match room_cache.iter().find(|(it, _)| *it == item) {
                        Some(&(_, r)) => r,
                        None => {
                            let r = self.reachable_room(&flood, item, defs).is_some();
                            room_cache.push((item, r));
                            r
                        }
                    };
                    room.then(|| flood.cost(pos).map(|c| (c, pos))).flatten()
                }
            };
            if let Some((cost, stand)) = found {
                let key = (cost, job.id);
                if best.is_none_or(|b| key < b.0) {
                    best = Some((key, idx, stand));
                }
            }
        }
        let Some((_, idx, stand)) = best else {
            self.units[i].claim_retry_at = self.tick + self.retry_ticks(defs);
            return false;
        };
        let unit_id = self.units[i].id;
        let job = &mut self.jobs[idx];
        job.claimed_by = Some(unit_id);
        let task = match job.kind {
            JobKind::Harvest { .. } => Task::Harvest(job.id),
            JobKind::Haul { .. } => Task::Haul(job.id),
        };
        let u = &mut self.units[i];
        u.job_task = Some(task);
        u.route = flood.path_to(stand).map(route_from);
        true
    }

    /// The nearest stockpile tile with room for `item`, with its cost.
    fn reachable_room(&self, flood: &Flood, item: ItemId, defs: &Defs) -> Option<(u32, Pos)> {
        self.zones
            .iter()
            .filter(|&&z| self.room_at(z, item, defs) > 0)
            .filter_map(|&z| flood.cost(z).map(|c| (c, z)))
            .min()
    }

    fn run_job(&mut self, i: usize, task: Task, defs: &Defs) {
        match task {
            Task::Harvest(id) => self.run_harvest(i, id, defs),
            Task::Haul(id) => self.run_haul(i, id, defs),
            Task::Deliver => self.run_deliver(i, defs),
            // Need tasks never sit in the job slot.
            _ => self.finish_job(i),
        }
    }

    fn run_harvest(&mut self, i: usize, id: JobId, defs: &Defs) {
        let Some(idx) = self.job_index(id) else { return self.finish_job(i) };
        let JobKind::Harvest { pos } = self.jobs[idx].kind else { return self.finish_job(i) };
        let Some(h) = self.map[pos].resource.and_then(|r| defs.harvest(r)) else {
            // The resource is gone, so the job is moot.
            self.jobs.remove(idx);
            return self.finish_job(i);
        };
        if self.units[i].route.is_none() {
            // Resuming after a need, or the claim's route was lost: plan from here.
            let flood = self.flood_from(i);
            match self.stand(&flood, pos).and_then(|(_, s)| flood.path_to(s)) {
                Some(path) => self.units[i].route = Some(route_from(path)),
                None => return self.abandon_job(i, id, defs),
            }
        }
        if !self.walk(i, defs) {
            return;
        }
        let rate = self.work_rate(i, h, defs);
        let job = &mut self.jobs[idx];
        job.progress += rate / TICKS_PER_SECOND as f32;
        if job.progress < h.work {
            return;
        }
        self.jobs.remove(idx);
        self.map[pos].resource = None;
        self.place_ground(pos, Stack { item: h.item, count: h.count });
        self.finish_job(i);
    }

    fn run_haul(&mut self, i: usize, id: JobId, defs: &Defs) {
        let Some(idx) = self.job_index(id) else { return self.finish_job(i) };
        let JobKind::Haul { pos } = self.jobs[idx].kind else { return self.finish_job(i) };
        let Some(&stack) = self.ground.get(&pos) else {
            self.jobs.remove(idx);
            return self.finish_job(i);
        };
        if self.units[i].route.is_none() {
            let flood = self.flood_from(i);
            match flood.path_to(pos) {
                Some(path) => self.units[i].route = Some(route_from(path)),
                None => return self.abandon_job(i, id, defs),
            }
        }
        if !self.walk(i, defs) {
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
            let route = self
                .reachable_room(&flood, stack.item, defs)
                .and_then(|(_, z)| flood.path_to(z))
                .map(route_from);
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
}

#[cfg(test)]
mod tests;
