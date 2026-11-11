//! Beasts in the world (`data/beasts.toml`, `beast.rs`, docs/design/horizons-phase-2.md, "Shared
//! sim work, 3. Beasts").
//!
//! A beast is a `Unit` with `Unit::beast` set, so it walks, claims jobs and carries through the
//! machinery settlers use. What makes it not a person is everything this module and its callers
//! leave out: no needs, chi, health or exposure (`tick_needs` skips it), no part of the
//! population, growth, the signs or the score (`Unit::is_settler`), and no job its row doesn't
//! name.
//!
//! - **Home.** Each tick starts with `tick_beasts`: a finished home building (`Kind::BeastHome`)
//!   with no beast gets one at its door, and a beast whose home is gone returns to the wild
//!   (`BeastEvent`). Doing it here, once, before any unit acts, keeps unit indexes steady for the
//!   whole tick, and covers a save from before the beast existed that already has its stable.
//! - **Work.** A hauling beast takes Haul and Refuel jobs whose trip is at least `min_trip`
//!   tiles (`beast_may_take`, asked from `job_gate`). With nothing to do it walks back to its
//!   home's door and stands there (`act_beast`); it never wanders.
//! - **Food.** A home with a `[building.fuel]` bunker burns it while it houses a beast
//!   (`fuel_in_use`). With the bunker empty the beast is `Hungry` (#689): it finishes the job it
//!   holds, then rests at home and claims nothing until the bunker is refilled. It never dies or
//!   leaves.

use crate::beast::{BeastId, BeastWork};
use crate::geom::Footprint;
use crate::jobs::JobKind;

use super::buildings::Kind;
use super::work::StandRule;
use super::*;

/// A beast came to be or went (`World::beast_events`), for a frontend's alert log.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct BeastEvent {
    pub tick: u64,
    /// The beast's name from `data/beasts.toml`: "Sky Bison".
    pub name: String,
    /// The home building's anchor.
    pub home: Pos,
    pub kind: BeastEventKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum BeastEventKind {
    /// Its home building came down: "The sky bison returns to the wild."
    ReturnedToWild,
}

impl BeastEvent {
    /// The log's line: `The sky bison returns to the wild.`
    pub fn describe(&self) -> String {
        match self.kind {
            BeastEventKind::ReturnedToWild => {
                format!("The {} returns to the wild.", self.name.to_lowercase())
            }
        }
    }
}

impl World {
    /// Every beast, in id order.
    pub fn beasts(&self) -> impl Iterator<Item = &Unit> + '_ {
        self.units.iter().filter(|u| u.is_beast())
    }

    /// The beast housed by the building anchored on `home`, if it has one.
    pub fn beast_housed_at(&self, home: Pos) -> Option<&Unit> {
        self.beasts().find(|u| u.beast.is_some_and(|b| b.home == home))
    }

    /// People in the settlement: every unit but the beasts. The Avatar counts, as he always has
    /// in the headcounts that are not about growth (`Unit::is_settler` leaves him out too).
    pub fn population(&self) -> usize {
        self.units.iter().filter(|u| !u.is_beast()).count()
    }

    /// Whether beast `u` is hungry: its home has a fuel bunker and it is empty (#689). False for
    /// a beast whose home needs no food, and for any unit that is not a beast.
    pub fn beast_hungry(&self, u: &Unit, defs: &Defs) -> bool {
        let Some(beast) = u.beast else { return false };
        self.anchored(beast.home)
            .is_some_and(|b| defs.building(b.id).fuel.is_some() && !b.fuel_lit())
    }

    /// The row `u` is, if it is a beast.
    pub fn beast_def<'d>(&self, u: &Unit, defs: &'d Defs) -> Option<&'d crate::beast::BeastDef> {
        u.beast.map(|b| defs.beast(b.id))
    }

    /// Puts a beast at the door of each finished home building that lacks one, and sends back to
    /// the wild any whose home is gone (module docs).
    pub(super) fn tick_beasts(&mut self, defs: &Defs) {
        if defs.beasts.is_empty() {
            return;
        }
        let gone: Vec<Pos> = self
            .beasts()
            .filter_map(|u| u.beast)
            .filter(|b| {
                self.anchored(b.home)
                    .is_none_or(|rec| defs.beast_for(self.nation, rec.id) != Some(b.id))
            })
            .map(|b| b.home)
            .collect();
        for home in gone {
            self.beast_returns_to_the_wild(home, defs);
        }
        let missing: Vec<(Pos, BeastId, Footprint)> = self
            .of_kind(Kind::BeastHome)
            .filter(|b| self.beast_housed_at(b.anchor).is_none())
            .filter_map(|b| Some((b.anchor, defs.beast_for(self.nation, b.id)?, b.footprint())))
            .collect();
        for (home, beast, foot) in missing {
            self.spawn_beast(home, beast, foot, defs);
        }
    }

    fn spawn_beast(&mut self, home: Pos, beast: BeastId, foot: Footprint, defs: &Defs) {
        // The door, or the nearest walkable tile of the ring if a tree or the shore holds it.
        let door = foot.entrance();
        let pos =
            std::iter::once(door).chain(foot.ring()).find(|&p| self.walkable(p)).unwrap_or(door);
        let id = UnitId(self.next_unit);
        self.next_unit += 1;
        let unit = Unit {
            id,
            name: defs.beast(beast).name.clone(),
            nation: None,
            pos,
            // A full meter nothing ever drains: `tick_needs` skips a beast.
            needs: Needs::full(&defs.needs),
            chi: 0.0,
            carrying: None,
            need_task: None,
            job_task: None,
            route: None,
            work: 0.0,
            bed: None,
            healing: None,
            need_retry_at: [0; 3],
            healing_retry_at: 0,
            claim_retry_at: 0,
            move_progress: 0.0,
            idle_until: 0,
            wander_left: 0,
            bends: 0,
            bends_by: BTreeMap::new(),
            rank_credited: Rank::Novice,
            inside: None,
            guest: None,
            beast: Some(Beast { id: beast, home }),
            glider: Glider::default(),
            unhappy_since: None,
            last_harm: None,
            exposed_alerted: false,
            sick_until: None,
            remedy_retry_at: 0,
        };
        // `units` stays sorted by id, and the Avatar's id is the largest there is.
        let at = self.units.partition_point(|u| u.id < id);
        self.units.insert(at, unit);
    }

    fn beast_returns_to_the_wild(&mut self, home: Pos, defs: &Defs) {
        let Some(at) = self.units.iter().position(|u| u.beast.is_some_and(|b| b.home == home))
        else {
            return;
        };
        let unit = self.units.remove(at);
        let beast = unit.beast.expect("found by its beast");
        self.release(&unit);
        self.log_event(BeastEvent {
            tick: self.tick,
            name: defs.beast(beast.id).name.clone(),
            home,
            kind: BeastEventKind::ReturnedToWild,
        });
    }

    /// One tick of beast `i`: carry on with the job it holds, else find one it may take, else
    /// head home and stand at the door. No needs, so nothing else interrupts it.
    pub(super) fn act_beast(&mut self, i: usize, defs: &Defs) {
        if let Some(task) = self.units[i].job_task {
            return self.run_job(i, task, defs);
        }
        if self.units[i].carrying.is_some() {
            // Something still in hand (its job went away): finish the delivery.
            let u = &mut self.units[i];
            u.job_task = Some(Task::Deliver);
            u.route = None;
            return;
        }
        let hungry = self.beast_hungry(&self.units[i], defs);
        if !hungry && self.claim_job(i, defs) {
            return;
        }
        self.beast_rests(i, defs);
    }

    /// Walks beast `i` back to its home's door, and stands there once it arrives.
    fn beast_rests(&mut self, i: usize, defs: &Defs) {
        let Some(beast) = self.units[i].beast else { return };
        let Some(foot) = self.footprint_at(beast.home) else { return };
        if self.units[i].pos == foot.entrance() {
            self.units[i].route = None;
            return;
        }
        if self.units[i].route.is_none() && self.tick < self.units[i].idle_until {
            return;
        }
        match self.go_to(i, StandRule::Entrance(beast.home), defs) {
            super::work::Go::Walking => {}
            super::work::Go::Arrived => self.units[i].route = None,
            // Nowhere to stand that it can reach: look again in a few seconds.
            super::work::Go::Unreachable => self.rest_idle(i, defs),
        }
    }

    /// Whether beast `u` may take a job of `kind` at all: only the kinds its row names. A
    /// hauling beast takes Haul and Refuel, and only when the trip is at least `min_trip` tiles
    /// (Chebyshev, pickup to drop). A bending beast takes nothing yet (its work is #697's). A
    /// hungry beast is kept off jobs by `act_beast`, which doesn't ask at all.
    pub(super) fn beast_may_take(&self, u: &Unit, kind: JobKind, defs: &Defs) -> bool {
        let Some(beast) = u.beast else { return true };
        let BeastWork::Haul { min_trip, .. } = defs.beast(beast.id).work else { return false };
        let trip = match kind {
            JobKind::Haul { pos } => self.haul_trip(pos, defs),
            JobKind::Refuel { pos } => self.refuel_trip(pos, defs),
            _ => return false,
        };
        trip.is_some_and(|t| t >= min_trip as i32)
    }

    /// How far the stack on `pos` goes to be stored: the Chebyshev distance to the nearest place
    /// that would take it (a stockpile tile with room, or a study hall for a scroll). `None`
    /// when nowhere would.
    fn haul_trip(&self, pos: Pos, defs: &Defs) -> Option<i32> {
        let item = self.ground.get(&pos)?.item;
        if Self::is_readable(item, defs) {
            self.study_halls().map(|h| pos.chebyshev(h)).min()
        } else {
            self.zones
                .iter()
                .filter(|&&z| self.room_at(z, item, defs) > 0)
                .map(|&z| pos.chebyshev(z))
                .min()
        }
    }

    /// How far the fuel for the bunker anchored on `pos` has to come: the Chebyshev distance
    /// from the nearest stack of it, in stores or on the ground, to the building. `None` when
    /// nothing is on hand.
    fn refuel_trip(&self, pos: Pos, defs: &Defs) -> Option<i32> {
        let (item, _) = self.fuel_shortfall(pos, defs)?;
        self.stored
            .iter()
            .chain(self.ground.iter())
            .filter(|(_, s)| s.item == item)
            .map(|(&p, _)| p.chebyshev(pos))
            .min()
    }

    /// Whether a beast lives in the building anchored on `anchor` (`fuel_in_use`'s arm).
    pub(super) fn houses_beast(&self, anchor: Pos) -> bool {
        self.beast_housed_at(anchor).is_some()
    }
}

#[cfg(test)]
mod tests;
