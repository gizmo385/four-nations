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
//! #226) and production stops until it comes back and re-enters. The job itself never completes:
//! a seat that's opened stays open for as long as the hall stands (taking it down, #342, closes
//! its seats with it), and a scholar who empties the queue simply leaves it open rather than
//! finishing it.

use crate::flood::Flood;
use crate::jobs::JobKind;

use super::*;

impl World {
    /// Opens `seats` new `Study` jobs at `pos`. Called once, when a study hall finishes
    /// construction; a no-op for an ordinary building (`seats == 0`).
    pub(super) fn open_study_seats(&mut self, pos: Pos, seats: u32) {
        for _ in 0..seats {
            self.add_job(JobKind::Study { pos });
        }
    }

    /// The seats of the building covering `pos`: its def's `seats` plus every upgrade a known
    /// tech adds (`BuildingDef::seat_upgrades`, `writing`'s +2, docs/design/horizons-phase-1.md).
    /// 0 off a building.
    pub fn seats_at(&self, pos: Pos, defs: &Defs) -> u32 {
        let Some(b) = self.building_record(pos) else { return 0 };
        self.seats_for(b.id, b.anchor, defs)
    }

    /// `seats_of` plus the best Archive in reach of a building `id` anchored on `anchor`
    /// (`BuildingDef::archive`, #709). Two Archives don't stack.
    pub(super) fn seats_for(&self, id: BuildingId, anchor: Pos, defs: &Defs) -> u32 {
        let def = defs.building(id);
        let (w, h) = def.footprint;
        let bonus = if def.seats > 0 {
            self.archive_bonus(id, crate::geom::Footprint::new(anchor, w, h), defs)
        } else {
            0
        };
        self.seats_of(def) + bonus
    }

    /// `seats_at` for a building def, for this settlement's research.
    pub(super) fn seats_of(&self, def: &BuildingDef) -> u32 {
        let extra: u32 = def
            .seat_upgrades
            .iter()
            .filter(|(t, _)| self.research.is_known(*t))
            .map(|&(_, n)| n)
            .sum();
        def.seats + extra
    }

    /// The fox spirits in each finished building `def` (`spirit_seats`, #739): its own plus the
    /// `spirit_library` upgrade once known, never more than its seats.
    pub fn foxes_of(&self, def: &BuildingDef) -> u32 {
        let up = def.spirit_upgrade.filter(|(t, _)| self.research.is_known(*t)).map_or(0, |u| u.1);
        (def.spirit_seats + up).min(self.seats_of(def))
    }

    /// The seats settlers take at a building `id` anchored on `anchor`: `seats_for` less its
    /// foxes, who fill seats rather than add them (#739).
    pub(super) fn settler_seats_for(&self, id: BuildingId, anchor: Pos, defs: &Defs) -> u32 {
        let def = defs.building(id);
        self.seats_for(id, anchor, defs).saturating_sub(self.foxes_of(def))
    }

    /// The fox spirits studying right now: every finished hall's `foxes_of`, while a seat is
    /// worth holding (`study_wanted`). Like a scholar with no target, a fox rests (#284).
    pub fn foxes_studying(&self, defs: &Defs) -> u32 {
        if !self.study_wanted(defs) {
            return 0;
        }
        self.buildings().map(|b| self.foxes_of(defs.building(b.id))).sum()
    }

    /// One tick of the fox spirits (#739): each studies exactly as a seated scholar does in its
    /// hall (`study_millis_at`), times the `study` modifier, through `add_toward_target`, so it
    /// feeds scribing when the queue is empty and an Archive stands. Foxes aren't units: they
    /// never eat, sleep, walk or leave, and nothing pre-empts them.
    pub(super) fn tick_foxes(&mut self, defs: &Defs) {
        if !self.study_wanted(defs) {
            return;
        }
        let halls: Vec<(Pos, u32)> = self
            .buildings()
            .map(|b| (b.anchor, self.foxes_of(defs.building(b.id))))
            .filter(|&(_, n)| n > 0)
            .collect();
        for (anchor, foxes) in halls {
            let rate = self.study_millis_at(anchor, defs) as f32 * self.modifiers().study;
            let research_cost = self.knobs(defs).research_cost;
            let scribing = self.scribing_on(defs);
            for _ in 0..foxes {
                self.research.add_toward_target(rate.round() as u64, defs, research_cost, scribing);
            }
        }
    }

    /// Knowledge millis one seat at the hall anchored on `anchor` makes per tick, before the
    /// `study` modifier: the base rate times a powered hall's boost (#428) and any aura's study
    /// multiplier (#723), in integer millis so the result is exact on every platform.
    fn study_millis_at(&self, anchor: Pos, defs: &Defs) -> u64 {
        let def = self.building_at(anchor).map(|b| defs.building(b));
        let boost_millis = def
            .filter(|d| d.uses_power)
            .and_then(|_| self.footprint_at(anchor))
            .map_or(1000, |f| self.power_boost_millis(f, defs));
        let aura = self.footprint_at(anchor).map_or(1000, |f| self.aura_over(f, defs).study_millis);
        defs.research.study_millis_per_tick * boost_millis as u64 / 1000 * aura as u64 / 1000
    }

    /// Opens the seats each finished hall is missing against `seats_at`, and closes the ones it
    /// has too many of: Study jobs, or Chart jobs at a Map Room. Runs when a tech completes, when
    /// a hall or an Archive finishes or comes down, and after `load`, so learning `writing`,
    /// or loading a save that already knows it, gives every standing hall its new seats, and
    /// taking an Archive down takes its seats away again (#709). Closing goes unclaimed seats
    /// first, then the highest `JobId`, whose scholar steps out. A no-op when nothing changed.
    pub(crate) fn sync_seats(&mut self, defs: &Defs) {
        let halls: Vec<(Pos, bool, u32)> = self
            .buildings()
            .filter(|b| defs.building(b.id).seats > 0)
            .map(|b| {
                let d = defs.building(b.id);
                (b.anchor, d.chart.is_some(), self.settler_seats_for(b.id, b.anchor, defs))
            })
            .collect();
        for (anchor, chart, want) in halls {
            let mut have: Vec<(bool, JobId)> = self
                .jobs
                .iter()
                .filter(|j| match j.kind {
                    JobKind::Study { pos } | JobKind::Chart { pos } => pos == anchor,
                    _ => false,
                })
                .map(|j| (j.claimed_by.is_some(), j.id))
                .collect();
            let want = want as usize;
            if have.len() < want {
                let missing = (want - have.len()) as u32;
                if chart {
                    self.open_chart_seats(anchor, missing);
                } else {
                    self.open_study_seats(anchor, missing);
                }
            } else if have.len() > want {
                // Unclaimed first, then the highest id: sorted so the seats to close come last.
                have.sort_by_key(|&(claimed, id)| (claimed, std::cmp::Reverse(id)));
                let close: Vec<JobId> =
                    have.iter().take(have.len() - want).map(|&(_, id)| id).collect();
                self.close_seats(&close, defs);
            }
        }
    }

    /// Removes the seat jobs `ids`; each scholar holding one lets go of it and steps out.
    fn close_seats(&mut self, ids: &[JobId], defs: &Defs) {
        for i in 0..self.units.len() {
            let held = match self.units[i].job_task {
                Some(Task::Study(id) | Task::Chart(id)) if ids.contains(&id) => Some(id),
                _ => None,
            };
            if let Some(id) = held {
                self.abandon_job(i, id, defs);
            }
        }
        self.jobs.retain(|j| !ids.contains(&j.id));
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
        if !self.study_wanted(defs) {
            self.jobs[idx].claimed_by = None;
            return self.finish_job(i);
        }
        if self.units[i].inside.is_none() {
            if !self.go_or_abandon(i, id, StandRule::Entrance(anchor), defs) {
                return;
            }
            // At the door: step inside once there's room, else wait (retried every tick).
            if !self.enter_building(i, anchor, defs) {
                return;
            }
        }
        // A powered hall's seat produces `boost` times the knowledge (issue #428), and an aura
        // over the hall multiplies with it (#723).
        let rate = self.study_millis_at(anchor, defs) as f32 * self.modifiers().study;
        let research_cost = self.knobs(defs).research_cost;
        let scribing = self.scribing_on(defs);
        self.research.add_toward_target(rate.round() as u64, defs, research_cost, scribing);
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
