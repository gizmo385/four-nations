//! The Study job: knowledge production at a study hall's seats (`data/buildings.toml`,
//! `data/research.toml`, issue #21).
//!
//! A study hall's `seats` becomes that many `JobKind::Study` jobs the moment it finishes
//! construction (`world/build.rs`), one per seat. It's the lowest-priority job there is
//! (`work::job_priority`, #284): a unit claims a free seat only once no harvest, haul, bend,
//! build or endeavor task is reachable, and only while something is queued to research
//! (`Research::queue`); with nothing queued, no seat is claimed at all. A unit that already holds
//! a seat walks to the hall's door, steps inside (`World::enter_building`, #226 -- a seat is a
//! place inside, so the hall's `capacity`, which defaults to its `seats`, naturally caps how many
//! study at once) and, every tick it's there, adds
//! `study_knowledge_per_minute × the nation's effective `study` modifier` to the research pool
//! (`Research::add_toward_target`), which caps the pool at the cost of what's being researched
//! so the trickle can't bank toward several techs at once (`Research::cap_millis`). Needs pre-empt a
//! seat exactly like any other job: a hungry scholar steps back out to eat (`consider_needs`,
//! #226) and production stops until it comes back and re-enters. The job itself never completes;
//! there's nothing to demolish yet, so a seat that's opened stays open for the rest of the game,
//! and a scholar who empties the queue simply leaves it open rather than finishing it.

use crate::flood::Flood;
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

    /// The door of the building anchored on `anchor`, with its flood cost: where a unit claiming
    /// or holding a seat there walks to, since seats are places inside (#227). `None` if the
    /// building is gone or its door isn't reachable.
    pub(super) fn stand_entrance(&self, flood: &Flood, anchor: Pos) -> Option<(u32, Pos)> {
        let door = self.footprint_at(anchor)?.entrance();
        flood.cost(door).map(|c| (c, door))
    }

    /// One tick of a unit holding a study seat: leave it if the research queue has emptied since
    /// it was claimed (#284), else walk to the hall's door, step inside (once there's room: the
    /// hall's `capacity`, #226) and add knowledge every tick it's there. The job never finishes
    /// on its own; leaving just frees the seat for later.
    pub(super) fn run_study(&mut self, i: usize, id: JobId, defs: &Defs) {
        let Some(idx) = self.job_index(id) else { return self.finish_job(i) };
        let JobKind::Study { pos: anchor } = self.jobs[idx].kind else { return self.finish_job(i) };
        if self.research.queue().is_empty() {
            self.jobs[idx].claimed_by = None;
            return self.finish_job(i);
        }
        if self.units[i].inside.is_none() {
            if self.units[i].route.is_none() {
                let flood = self.flood_from(i);
                match self.stand_entrance(&flood, anchor).and_then(|(_, s)| flood.path_to(s)) {
                    Some(path) => self.units[i].route = Some(route_from(path)),
                    None => return self.abandon_job(i, id, defs),
                }
            }
            if !self.walk(i, defs) {
                return;
            }
            // At the door: step inside once there's room, else wait (retried every tick).
            if !self.enter_building(i, anchor, defs) {
                return;
            }
        }
        // A powered hall's seat produces `boost` times the knowledge (issue #428), kept in
        // integer millis so the boost's effect is exact on every platform: `boost_millis` is
        // `boost * 1000`, rounded at load.
        let def = self.building_at(anchor).map(|b| defs.building(b));
        let boost_millis = def
            .filter(|d| d.uses_power)
            .and_then(|_| self.footprint_at(anchor))
            .map_or(1000, |f| self.power_boost_millis(f, defs));
        let millis = defs.research.study_millis_per_tick * boost_millis as u64 / 1000;
        let rate = millis as f32 * self.modifiers().study;
        let research_cost = self.knobs(defs).research_cost;
        self.research.add_toward_target(rate.round() as u64, defs, research_cost);
    }

    /// How many units are seated at a study hall and producing knowledge right now, as opposed
    /// to merely holding the job while they walk to one: `run_study` only adds to the pool once
    /// `inside` is set. What the research screen's effective rate (#338) adds on top of the
    /// passive trickle.
    pub fn study_seats_filled(&self) -> usize {
        self.units
            .iter()
            .filter(|u| matches!(u.task(), Some(Task::Study(_))) && u.inside().is_some())
            .count()
    }
}

#[cfg(test)]
mod tests;
