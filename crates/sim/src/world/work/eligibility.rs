//! Which jobs a unit may take, and why not (#599): the one place that knows each `JobKind`.
//!
//! Four callers used to carry their own full `match` over every kind and had to agree by hand:
//! `claim_job` (take the nearest), `idle_reason` (say why nothing is taken), `any_job_in_region`
//! (a cheap pre-check before the flood) and `prune_jobs` (drop what is moot). They now ask here:
//!
//! * `job_is_live`: is the job still worth keeping in the queue? (`prune_jobs`)
//! * `avatar_may_take`: the Avatar's exclusions, written once.
//! * `job_gate`: for a job and a unit, everything that needs no flood: is the job valid, where
//!   must the unit stand (`StandRule`), and what bars this unit from it (`Bar`).
//! * `check_job`: the gate plus the flood: the final verdict `claim_job` and `idle_reason` use.
//!
//! Each of these matches on `JobKind` with no wildcard arm, so a new kind does not compile until
//! every question has an answer for it.

use super::*;

/// Where a unit stands to work a job: the per-kind stand-tile rule, shared by the flood lookup
/// (`World::stand_at`) and the region pre-check (`World::any_job_in_region`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::world) enum StandRule {
    /// On the tile if units can enter it, else beside it (`World::stand_tile`).
    Tile(Pos),
    /// Inside the building or site if it is walkable, else beside it (`World::stand_building`).
    Building(Pos),
    /// At the door of the building at this anchor, a seat inside it (`World::stand_entrance`,
    /// #227). The region pre-check only asks for a stand tile anywhere at the building.
    Entrance(Pos),
    /// On exactly this tile (a stack to pick up).
    Exact(Pos),
}

/// What bars a unit from a job it could otherwise stand beside.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Bar {
    /// Nothing: the unit may take it.
    Free,
    /// The unit may not, for this reason (`None`: a reason `IdleReason` has no name for, such as
    /// a bend the unit can't cast at all).
    Reason(Option<IdleReason>),
    /// Only if some stockpile able to take this item has room, which depends on what is
    /// reachable (`World::reachable_dest`) and so on the flood.
    Room(ItemId),
}

/// `World::job_gate`'s answer for a valid job.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct Gate {
    pub stand: StandRule,
    pub bar: Bar,
}

/// A job weighed against one unit and its flood.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Check {
    /// Not valid right now, or nowhere reachable to stand: it says nothing about the unit.
    Skip,
    /// Reachable, but this unit may not take it.
    Barred(Option<IdleReason>),
    /// The unit may take it: path cost to the stand tile, and the tile.
    Take(u32, Pos),
}

/// Whether the Avatar ever claims `kind` (`docs/design/endgame.md`, "The Avatar", #237, #238):
/// only Bend jobs and endeavor tasks are his; the rite itself is assigned directly (`run_idle`).
pub(super) fn avatar_may_take(kind: JobKind) -> bool {
    match kind {
        JobKind::Bend { .. } | JobKind::EndeavorTask { .. } => true,
        JobKind::Harvest { .. }
        | JobKind::Haul { .. }
        | JobKind::Build { .. }
        | JobKind::Study { .. }
        | JobKind::Attend { .. }
        | JobKind::Craft { .. }
        | JobKind::Sow { .. }
        | JobKind::Reap { .. }
        | JobKind::Clear { .. }
        | JobKind::Chart { .. }
        | JobKind::Deconstruct { .. }
        | JobKind::Refuel { .. } => false,
    }
}

/// A job's gate, its tier and the tiles its stand rule picks from.
type JobWeighing = (Gate, u8, Vec<Pos>);

/// Everything a claim attempt for one unit depends on, short of the flood itself (#574). The
/// attempt's answer is remembered with its key (`World::claim_memo`) and not recomputed while
/// the key is the same: the flood only reads the map's passability (`map_rev`) and the unit's tile,
/// and `check_job` only reads what `job_gate` and the stand and room lookups read, all of which
/// are listed here. The unit's own state (chi, health, cooldown, being the Avatar) and the
/// world's (research, plots, time-gated jobs) reach the verdict through `job_gate`, so the
/// gate itself is in the key rather than its inputs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(in crate::world) struct ClaimKey {
    pos: Pos,
    map_rev: u64,
    /// Each unclaimed job, its gate (`None`: gated out) and the tiles its stand rule picks from.
    jobs: Vec<(JobId, JobKind, Option<JobWeighing>)>,
    /// Per item a `Bar::Room` job asks about: the stockpile tiles with room for it, or, for a
    /// readable item, the tiles beside each study hall.
    rooms: Vec<(ItemId, Vec<Pos>)>,
}

/// `World::idle_reason`'s answers, kept per unit with the `ClaimKey` they were given for (#574).
/// It takes `&self`, hence the lock; a clone starts empty, and it is never saved.
#[derive(Default)]
pub(in crate::world) struct IdleMemo(
    std::sync::Mutex<BTreeMap<UnitId, (ClaimKey, Option<IdleReason>)>>,
);

impl IdleMemo {
    pub(super) fn get(&self, unit: UnitId, key: &ClaimKey) -> Option<Option<IdleReason>> {
        let memo = self.0.lock().unwrap_or_else(|e| e.into_inner());
        memo.get(&unit).filter(|(k, _)| k == key).map(|&(_, reason)| reason)
    }

    pub(super) fn put(&self, unit: UnitId, key: ClaimKey, reason: Option<IdleReason>) {
        self.0.lock().unwrap_or_else(|e| e.into_inner()).insert(unit, (key, reason));
    }
}

impl Clone for IdleMemo {
    fn clone(&self) -> Self {
        Self::default()
    }
}

impl std::fmt::Debug for IdleMemo {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("IdleMemo")
    }
}

impl World {
    /// The tiles `rule` picks its stand tile from, before the flood narrows them to the
    /// reachable cheapest (what `stand_at` reads from the map).
    fn stand_candidates(&self, rule: StandRule, defs: &Defs) -> Vec<Pos> {
        let area = |foot: Footprint, walkable: bool| -> Vec<Pos> {
            if walkable { foot.tiles().collect() } else { foot.ring().collect() }
        };
        match rule {
            StandRule::Tile(pos) => {
                let walkable = self.map.get(pos).is_some_and(|t| self.rules.passable(t));
                area(Footprint::single(pos), walkable)
            }
            StandRule::Building(pos) => {
                let (foot, walkable) = self.work_area(pos, defs);
                area(foot, walkable)
            }
            StandRule::Entrance(pos) => {
                self.footprint_at(pos).map(|f| f.entrance()).into_iter().collect()
            }
            StandRule::Exact(pos) => vec![pos],
        }
    }

    /// See `ClaimKey`.
    pub(super) fn claim_key(&self, i: usize, defs: &Defs) -> ClaimKey {
        let is_avatar = self.units[i].is_avatar();
        let mut rooms: Vec<(ItemId, Vec<Pos>)> = Vec::new();
        let jobs = self
            .jobs
            .iter()
            .filter(|j| j.claimed_by.is_none())
            .map(|j| {
                let gate = self.job_gate(i, j.kind, defs).map(|g| {
                    if let Bar::Room(item) = g.bar
                        && !rooms.iter().any(|(it, _)| *it == item)
                    {
                        let tiles = if Self::is_readable(item, defs) {
                            self.study_halls()
                                .flat_map(|h| {
                                    let (foot, walkable) = self.work_area(h, defs);
                                    if walkable {
                                        foot.tiles().collect::<Vec<_>>()
                                    } else {
                                        foot.ring().collect()
                                    }
                                })
                                .collect()
                        } else {
                            self.zones
                                .iter()
                                .copied()
                                .filter(|&z| self.room_at(z, item, defs) > 0)
                                .collect()
                        };
                        rooms.push((item, tiles));
                    }
                    // The tier decides between jobs the unit can take (`claim_job`).
                    let tier = if is_avatar {
                        self.avatar_job_priority(j.kind, defs)
                    } else {
                        job_priority(j.kind)
                    };
                    (g, tier, self.stand_candidates(g.stand, defs))
                });
                (j.id, j.kind, gate)
            })
            .collect();
        ClaimKey { pos: self.units[i].pos, map_rev: self.map_rev, jobs, rooms }
    }

    /// Whether an unclaimed job of `kind` should stay in the queue: its target hasn't gone.
    pub(super) fn job_is_live(&self, kind: JobKind, defs: &Defs) -> bool {
        match kind {
            JobKind::Harvest { pos } => {
                self.map[pos].resource.is_some_and(|r| defs.harvest(r).is_some())
            }
            JobKind::Haul { pos } => self.ground.contains_key(&pos),
            // Left for a unit to find out: a bend that can't happen is reported as a failed
            // event, not silently dropped.
            JobKind::Bend { .. } => true,
            JobKind::Build { pos } => self.sites.contains_key(&pos),
            // A seat opened for a study hall stays open for as long as the building stands. The
            // same goes for a production building's craft job (#124) and a Map Room's seat
            // (#402). Taking it down (#342, `remove_building`) removes them along with it.
            JobKind::Study { pos } | JobKind::Craft { pos } | JobKind::Chart { pos } => {
                self.map[pos].building.is_some()
            }
            JobKind::Deconstruct { pos } => self.anchored(pos).is_some(),
            JobKind::Refuel { pos } => self.fuel_shortfall(pos, defs).is_some(),
            // The gate drops its own task and Attend jobs when it moves on (`gate.rs`).
            JobKind::EndeavorTask { .. } | JobKind::Attend { .. } => true,
            // Moot if the plot is gone (taken down, #342) or moved past the stage this job was
            // for.
            JobKind::Sow { pos } => self.plot_stage(pos) == Some(PlotStage::Empty),
            JobKind::Reap { pos } => self.plot_stage(pos) == Some(PlotStage::Ripe),
            JobKind::Clear { pos } => {
                self.map[pos].resource.is_some_and(|r| defs.harvest(r).is_some())
            }
        }
    }

    /// Everything about `kind` for unit `i` that needs no flood, or `None` if the unit's side of
    /// the question is "not this job at all": the Avatar on an ordinary job, or a job whose
    /// target is not ready (unripe, wrong stage, nothing to study toward, ...).
    pub(super) fn job_gate(&self, i: usize, kind: JobKind, defs: &Defs) -> Option<Gate> {
        if self.units[i].is_avatar() && !avatar_may_take(kind) {
            return None;
        }
        let free = |stand| Some(Gate { stand, bar: Bar::Free });
        let hurt = |risk: f32| {
            if risk > 0.0 && !self.fit_for_heavy_work(&self.units[i], defs) {
                Bar::Reason(Some(IdleReason::TooHurt))
            } else {
                Bar::Free
            }
        };
        match kind {
            JobKind::Harvest { pos } => {
                let h = self.ripe_harvest(pos, defs)?;
                Some(Gate { stand: StandRule::Tile(pos), bar: hurt(h.risk) })
            }
            JobKind::Clear { pos } => {
                let h = self.clearable_harvest(pos, defs)?;
                Some(Gate { stand: StandRule::Tile(pos), bar: hurt(h.clear_risk()) })
            }
            JobKind::Sow { pos } => (self.plot_stage(pos) == Some(PlotStage::Empty))
                .then(|| free(StandRule::Building(pos)))
                .flatten(),
            JobKind::Deconstruct { pos } => {
                self.anchored(pos).and_then(|_| free(StandRule::Building(pos)))
            }
            // Only while the bunker is short and some store holds its fuel, so nobody claims a
            // trip with nothing to fetch (the stuck-build case of #322).
            JobKind::Refuel { pos } => self
                .fuel_shortfall(pos, defs)
                .filter(|&(item, short)| short > 0 && self.any_source_for_fuel(item))
                .and_then(|_| free(StandRule::Building(pos))),
            JobKind::Reap { pos } => (self.plot_stage(pos) == Some(PlotStage::Ripe))
                .then(|| free(StandRule::Building(pos)))
                .flatten(),
            JobKind::Haul { pos } => {
                let item = self.ground.get(&pos)?.item;
                Some(Gate { stand: StandRule::Exact(pos), bar: Bar::Room(item) })
            }
            // A tile chained off a still-pending neighbour (#304) can't be claimed until that
            // neighbour has actually been reshaped: `bend_ready`.
            JobKind::Bend { ability, pos } => {
                if !self.bend_ready(ability, pos, defs) {
                    return None;
                }
                let bar = if self.unit_can_bend(i, ability, defs) {
                    Bar::Free
                } else if self.can_cast(&self.units[i], ability, defs)
                    && self.units[i].chi < defs.ability(ability).chi
                {
                    // A recast too fresh to restoke (#359) is folded in here too, since
                    // `unit_can_bend` doesn't separate the two.
                    Bar::Reason(Some(IdleReason::OutOfChi))
                } else {
                    Bar::Reason(None)
                };
                Some(Gate { stand: StandRule::Tile(pos), bar })
            }
            // Reachable as soon as the site itself is and nothing it still needs cleared
            // (`site_ready`, `docs/design/clearing.md`, issue #425), whether or not its
            // materials are yet: a unit will wait on those, as a resting harvest does.
            JobKind::Build { pos } => {
                self.site_ready(pos, defs).then(|| free(StandRule::Building(pos))).flatten()
            }
            // Not reachable at all with nothing queued to study toward: nobody should claim a
            // seat that would only idle there (#284). A seat is a place inside (#227), so the
            // stand is the door, not the ring around the building.
            JobKind::Study { pos } => (!self.research.queue().is_empty())
                .then(|| free(StandRule::Entrance(pos)))
                .flatten(),
            // Not reachable at all with nothing left to chart within range (#402), the same way
            // an empty research queue leaves a study seat unclaimed.
            JobKind::Chart { pos } => self
                .chart_def_at(pos, defs)
                .filter(|def| self.chart_has_target(pos, def))
                .and_then(|_| free(StandRule::Entrance(pos))),
            JobKind::EndeavorTask { task } => self
                .gate_task(task, defs)
                .filter(|(def, _)| self.unit_can_raise(i, def))
                .and_then(|(_, anchor)| free(StandRule::Building(anchor))),
            JobKind::Attend { pos } => free(StandRule::Tile(pos)),
            // A seat inside, like a study hall's (#227). Always reachable once the building
            // stands; a crafter waits on its inputs the way a resting harvest is waited on.
            JobKind::Craft { pos } => free(StandRule::Entrance(pos)),
        }
    }

    /// The cheapest reachable stand tile for `rule`, with its path cost.
    pub(super) fn stand_at(
        &self,
        flood: &Flood,
        rule: StandRule,
        defs: &Defs,
    ) -> Option<(u32, Pos)> {
        match rule {
            StandRule::Tile(pos) => self.stand_tile(flood, pos),
            StandRule::Building(pos) => self.stand_building(flood, pos, defs),
            StandRule::Entrance(pos) => self.stand_entrance(flood, pos),
            StandRule::Exact(pos) => flood.cost(pos).map(|c| (c, pos)),
        }
    }

    /// Unit `i` weighed against `kind`: the gate, then the flood. `rooms` caches, per item,
    /// whether a stockpile tile with room for it can be reached (asked once per item). With a
    /// `limit`, a stand tile costing more than it counts as out of reach (`Flood::within`): for
    /// `pick_job`, which only wants a job cheaper than the best it has.
    pub(super) fn check_job(
        &self,
        i: usize,
        kind: JobKind,
        flood: &Flood,
        rooms: &mut Vec<(ItemId, bool)>,
        limit: Option<u32>,
        defs: &Defs,
    ) -> Check {
        let Some(gate) = self.job_gate(i, kind, defs) else { return Check::Skip };
        let stand = match limit {
            Some(limit) => flood.within(limit, || self.stand_at(flood, gate.stand, defs)),
            None => self.stand_at(flood, gate.stand, defs),
        };
        let Some((cost, stand)) = stand else {
            return Check::Skip;
        };
        match gate.bar {
            Bar::Free => Check::Take(cost, stand),
            Bar::Reason(why) => Check::Barred(why),
            Bar::Room(item) => {
                let room = match rooms.iter().find(|(it, _)| *it == item) {
                    Some(&(_, r)) => r,
                    None => {
                        let r = self.reachable_dest(flood, item, defs).is_some();
                        rooms.push((item, r));
                        r
                    }
                };
                if room {
                    Check::Take(cost, stand)
                } else {
                    Check::Barred(Some(IdleReason::StockpileFull))
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every kind answers `avatar_may_take` (the matches here have no wildcard arm, so a new
    /// variant fails to compile until it has a rule); the Avatar's list is exactly the bend and
    /// endeavor kinds.
    #[test]
    fn avatar_takes_only_bends_and_endeavor_tasks() {
        let p = Pos::new(0, 0);
        let ability = AbilityId(0);
        let all = [
            JobKind::Harvest { pos: p },
            JobKind::Haul { pos: p },
            JobKind::Bend { ability, pos: p },
            JobKind::Build { pos: p },
            JobKind::Study { pos: p },
            JobKind::EndeavorTask { task: 0 },
            JobKind::Attend { pos: p },
            JobKind::Craft { pos: p },
            JobKind::Sow { pos: p },
            JobKind::Reap { pos: p },
            JobKind::Clear { pos: p },
            JobKind::Chart { pos: p },
            JobKind::Deconstruct { pos: p },
            JobKind::Refuel { pos: p },
        ];
        for kind in all {
            let expected = matches!(kind, JobKind::Bend { .. } | JobKind::EndeavorTask { .. });
            assert_eq!(avatar_may_take(kind), expected, "{kind:?}");
        }
    }
}
