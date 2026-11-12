//! Whole-world consistency checks (#600). #559, #562, #582 and #583 were one class of bug: a
//! settler's state disagreeing with the map or with another table. Each was found by a playtest,
//! not by CI, because the tests are example-based. `World::check_invariants` states what must
//! always hold, and the long runs (`balance::run`, the autoplay Economy tests, the bending runs
//! in `tests/invariants.rs`) call `World::assert_invariants` every `CHECK_EVERY` ticks.
//!
//! Compiled under `cfg(any(test, debug_assertions))`: `cfg(test)` alone would miss the
//! integration tests in `crates/sim/tests`, which link the library built as a dependency. The
//! `ci` profile inherits `dev`, so it has debug assertions on; a release build drops the checks
//! (and `assert_invariants` becomes a no-op in `world.rs`), so the shipped game pays nothing.
//!
//! Each table gets its own small function. The buildings' one is
//! `World::check_building_records` (`buildings.rs`, #618): tiles and records agree, each record's
//! state fits its def, and the indexes match the records. A check reports the first violation
//! it finds.
//!
//! Known violations on `main` that aren't fixed yet are listed in `ALLOWED`, each with its
//! follow-up issue; a violation whose text contains an allowed fragment is let through.

use super::*;
use crate::building::BuildingId;
use crate::jobs::JobKind;

/// How often (in ticks) `assert_invariants` actually checks. The check walks every unit, job and
/// table, which is cheap, but a long run is millions of ticks; every 600th (one in-game minute)
/// keeps the added time well under a tenth of the gate's.
pub const CHECK_EVERY: u64 = 600;

/// Violations known on `main`, as `(fragment of the message, follow-up issue)`.
const ALLOWED: &[(&str, &str)] = &[];

impl World {
    /// Checks that the world's tables agree with the map and with each other (module doc).
    /// `Err` names the first violation.
    pub fn check_invariants(&self, defs: &Defs) -> Result<(), String> {
        self.check_units_sorted_and_on_map()?;
        self.check_job_claims()?;
        self.check_unit_buildings(defs)?;
        self.check_stored_on_zones()?;
        self.check_haul_jobs_have_stacks()?;
        self.check_sites()?;
        self.check_building_records(defs)
    }

    /// `check_invariants` every `CHECK_EVERY` ticks, panicking with the tick on a violation not
    /// in `ALLOWED`. The one call a long run makes per tick.
    pub fn assert_invariants(&self, defs: &Defs) {
        if !self.tick.is_multiple_of(CHECK_EVERY) {
            return;
        }
        if let Err(e) = self.check_invariants(defs)
            && !ALLOWED.iter().any(|(fragment, _)| e.contains(fragment))
        {
            panic!("world invariant broken at tick {} (seed {}): {e}", self.tick, self.seed);
        }
    }

    /// The finished building whose anchor is `pos`, if there is one.
    fn anchored_building(&self, pos: Pos) -> Option<BuildingId> {
        self.anchored(pos).map(|b| b.id)
    }

    fn check_units_sorted_and_on_map(&self) -> Result<(), String> {
        for pair in self.units.windows(2) {
            if pair[0].id >= pair[1].id {
                return Err(format!(
                    "units not sorted by id: {:?} before {:?}",
                    pair[0].id, pair[1].id
                ));
            }
        }
        for u in &self.units {
            if !self.map.in_bounds(u.pos) {
                return Err(format!("unit {:?} stands off the map at {:?}", u.id, u.pos));
            }
        }
        Ok(())
    }

    /// Every claimed job names a living unit holding that job as its `job_task`, and every
    /// `job_task` that names a job finds it claimed by that unit.
    fn check_job_claims(&self) -> Result<(), String> {
        for pair in self.jobs.windows(2) {
            if pair[0].id >= pair[1].id {
                return Err(format!(
                    "jobs not sorted by id: {:?} before {:?}",
                    pair[0].id, pair[1].id
                ));
            }
        }
        for job in &self.jobs {
            if job.partner.is_some() && !job.joint {
                return Err(format!("job {:?} has a partner but is not a joint bend", job.id));
            }
            for uid in job.holders() {
                let Some(unit) = self.units.iter().find(|u| u.id == uid) else {
                    return Err(format!(
                        "job {:?} is claimed by {uid:?}, who is not alive",
                        job.id
                    ));
                };
                if unit.job_task.and_then(task_job) != Some(job.id) {
                    return Err(format!(
                        "job {:?} is claimed by {uid:?}, whose job_task is {:?}",
                        job.id, unit.job_task
                    ));
                }
            }
        }
        for unit in &self.units {
            let Some(id) = unit.job_task.and_then(task_job) else { continue };
            match self.jobs.iter().find(|j| j.id == id) {
                None => {
                    return Err(format!(
                        "unit {:?} holds {:?}, but job {id:?} does not exist",
                        unit.id, unit.job_task
                    ));
                }
                Some(j) if !j.held_by(unit.id) => {
                    return Err(format!(
                        "unit {:?} holds job {id:?}, which is claimed by {:?}",
                        unit.id, j.claimed_by
                    ));
                }
                Some(_) => {}
            }
        }
        Ok(())
    }

    /// A unit's `bed`, `healing` and `inside` name a standing building (by anchor) of the right
    /// kind: a bed, a healing building, and any building at all.
    fn check_unit_buildings(&self, defs: &Defs) -> Result<(), String> {
        for u in &self.units {
            let kind_ok = |at: Option<Pos>, what: &str, ok: fn(&BuildingDef) -> bool| {
                let Some(pos) = at else { return Ok(()) };
                match self.anchored_building(pos) {
                    Some(id) if ok(defs.building(id)) => Ok(()),
                    found => Err(format!(
                        "unit {:?} has {what} {pos:?}, but the tile holds {found:?}",
                        u.id
                    )),
                }
            };
            kind_ok(u.bed, "bed", |d| d.bed.is_some())?;
            kind_ok(u.healing, "healing", |d| d.healing.is_some())?;
            kind_ok(u.bath, "bath", |d| d.bath.is_some())?;
            kind_ok(u.inside, "inside", |_| true)?;
        }
        Ok(())
    }

    fn check_stored_on_zones(&self) -> Result<(), String> {
        match self.stored.keys().find(|p| !self.zones.contains(p)) {
            Some(p) => Err(format!("stored stack at {p:?}, which is not a stockpile tile")),
            None => Ok(()),
        }
    }

    fn check_haul_jobs_have_stacks(&self) -> Result<(), String> {
        for job in &self.jobs {
            if let JobKind::Haul { pos } = job.kind
                && !self.ground.contains_key(&pos)
            {
                return Err(format!("haul job {:?} at {pos:?} has no ground stack", job.id));
            }
        }
        Ok(())
    }

    /// A build site sits on the map, and no finished building already stands on it.
    fn check_sites(&self) -> Result<(), String> {
        for (&pos, site) in &self.sites {
            for p in site.footprint(pos).tiles() {
                match self.map.get(p) {
                    None => return Err(format!("site {pos:?} extends off the map at {p:?}")),
                    Some(t) if t.building.is_some() => {
                        return Err(format!("site {pos:?} has a finished building on {p:?}"));
                    }
                    Some(_) => {}
                }
            }
        }
        Ok(())
    }
}

/// The job a task holds, if it holds one.
fn task_job(task: Task) -> Option<JobId> {
    match task {
        Task::Harvest(id)
        | Task::Haul(id)
        | Task::Bend(id)
        | Task::Build(id)
        | Task::Study(id)
        | Task::Endeavor(id)
        | Task::Attend(id)
        | Task::Craft(id)
        | Task::Sow(id)
        | Task::Reap(id)
        | Task::Clear(id)
        | Task::Chart(id)
        | Task::Deconstruct(id)
        | Task::Refuel(id)
        | Task::Trade(id) => Some(id),
        Task::Rite
        | Task::Deliver
        | Task::Chute(_)
        | Task::Balloon(_)
        | Task::Eat(_)
        | Task::Drink
        | Task::Sleep
        | Task::Healing(_)
        | Task::Bath(_)
        | Task::Remedy(_) => None,
    }
}

#[cfg(test)]
mod tests;
