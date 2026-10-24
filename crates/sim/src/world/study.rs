//! The Study job: knowledge production at a study hall's seats (`data/buildings.toml`,
//! `data/research.toml`, issue #21).
//!
//! A study hall's `seats` becomes that many `JobKind::Study` jobs the moment it finishes
//! construction (`world/build.rs`), one per seat. An idle unit claims a free seat exactly like
//! any other job, walks to the hall and, every tick it's there, adds
//! `study_knowledge_per_minute × the nation's effective `study` modifier` to the research pool
//! (`Research::add_knowledge`). Needs pre-empt a seat exactly like any other job: a hungry
//! scholar leaves to eat and production stops until it comes back. Unlike a harvest or a build,
//! the job itself never completes; there's nothing to demolish yet, so a seat that's opened stays
//! open for the rest of the game.

use crate::jobs::JobKind;

use super::work::route_from;
use super::*;

impl World {
    /// Opens `seats` new `Study` jobs at `pos`. Called once, when a study hall finishes
    /// construction; a no-op for an ordinary building (`seats == 0`).
    pub(super) fn open_study_seats(&mut self, pos: Pos, seats: u32) {
        for _ in 0..seats {
            self.add_job(JobKind::Study { pos });
        }
    }

    /// One tick of a unit holding a study seat: walk to the hall, then add knowledge every tick
    /// it's there. The job never finishes on its own.
    pub(super) fn run_study(&mut self, i: usize, id: JobId, defs: &Defs) {
        let Some(idx) = self.job_index(id) else { return self.finish_job(i) };
        let JobKind::Study { pos } = self.jobs[idx].kind else { return self.finish_job(i) };
        if self.units[i].route.is_none() {
            let flood = self.flood_from(i);
            match self.stand(&flood, pos).and_then(|(_, s)| flood.path_to(s)) {
                Some(path) => self.units[i].route = Some(route_from(path)),
                None => return self.abandon_job(i, id, defs),
            }
        }
        if !self.walk(i, defs) {
            return;
        }
        let rate = defs.research.study_millis_per_tick as f32 * self.modifiers().study;
        self.research.add_knowledge(rate.round() as u64);
    }
}

#[cfg(test)]
mod tests;
