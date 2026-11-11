//! The Balloon Dock (`[building.balloon]`, docs/design/horizons-phase-2.md, "`cargo_balloons`"):
//! goods fly between the two docks of a pair (`pair.rs`).
//!
//! - **The goods route rule.** A unit taking a stack to stores (`Task::Deliver`) compares walking
//!   straight to the nearest stockpile with room against walking to a working dock, plus the
//!   flight at the unit's own walking speed, plus the partner dock's walk from its door to stores
//!   (`World::balloon_for`). Both docks work in both directions. The partner's walks come from the
//!   per-link-end cache `chute.rs` keeps (`foot_walks`), current until the map or the stockpile
//!   tiles change. A dock whose hold has no room for the stack is skipped, and a unit that
//!   arrives to find it full walks on instead: a full hold sends haulers walking. Build-site
//!   deliveries, refuels, meals and scrolls never use a dock: only `run_deliver` asks.
//! - **One balloon per pair.** It starts at the first dock finished (`assign_balloon`), and lives
//!   on a dock's record: parked there, or flying toward it (`Balloon::flight`). A parked balloon
//!   launches when its dock's hold has `load` items or the oldest has waited `wait_ticks`; if
//!   the goods are at the other dock it flies there empty first (`plan_launch`).
//! - **Fuel per flight.** Every launch burns `fuel_per_flight` from the launching dock's bunker,
//!   empty flights too. Short of that, the balloon stays (`BalloonStatus::OutOfFuel`). The
//!   bunker has no `per_minute`, so `fuel.rs` only refills it.
//! - **Landing.** The goods spill beside the dock's door as ground stacks, each with an ordinary
//!   Haul job, as at a chute's foot. A balloon already in the air lands even if the pair no
//!   longer works, so cargo is never lost.
//! - **Taken down.** A dock returns its hold, the cargo in flight toward it, and its bunker
//!   (`deconstruct.rs`).

use crate::building::BalloonDef;
use crate::flood::Flood;
use crate::jobs::{ItemId, Stack};
use crate::path::STRAIGHT;

use super::buildings::Kind;
use super::work::Go;
use super::*;

/// One stack waiting in a dock's hold, and the tick it arrived.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Held {
    pub stack: Stack,
    pub at: u64,
}

/// A balloon in the air: ticks until it lands, and what it carries (one stack per item).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Flight {
    pub left: u64,
    pub cargo: Vec<Stack>,
}

/// A pair's balloon, kept on the dock it is parked at or flying toward.
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Balloon {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub flight: Option<Flight>,
}

/// A Balloon Dock's state: its hold, oldest first, and the balloon if it is here.
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Dock {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub hold: Vec<Held>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub balloon: Option<Balloon>,
}

impl Dock {
    fn held(&self) -> u32 {
        self.hold.iter().map(|h| h.stack.count).sum()
    }
}

impl buildings::Building {
    /// Everything a taken-down dock gives back besides its bunker: its hold, and the cargo of a
    /// balloon that was flying toward it.
    pub(super) fn dock_goods(&self) -> Vec<Stack> {
        let Some(dock) = &self.dock else { return Vec::new() };
        let mut goods: Vec<Stack> = dock.hold.iter().map(|h| h.stack).collect();
        if let Some(Flight { cargo, .. }) = dock.balloon.as_ref().and_then(|b| b.flight.as_ref()) {
            goods.extend(cargo.iter().copied());
        }
        goods
    }
}

/// What a Balloon Dock's balloon is doing, for its cursor panel (`World::balloon_status`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BalloonStatus {
    /// The balloon is at the partner dock (or in the air toward it).
    Away,
    /// Parked here, with nothing to fly yet.
    Parked,
    /// In the air toward this dock, landing in this many ticks.
    Landing { ticks: u64 },
    /// Parked here with a flight to make and too little fuel to make it (`Out of wood`).
    OutOfFuel(ItemId),
}

/// What a parked balloon would do next, fuel aside.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Plan {
    /// Fly this dock's goods across.
    Loaded,
    /// Fly to the partner dock, where goods wait, with nothing aboard.
    Empty,
}

impl World {
    fn balloon_def(&self, dock: Pos, defs: &Defs) -> Option<BalloonDef> {
        defs.building(self.anchored(dock)?.id).balloon
    }

    /// The hold of the Balloon Dock anchored on `pos`: items waiting and how many it takes.
    pub fn dock_hold(&self, pos: Pos, defs: &Defs) -> Option<(u32, u32)> {
        let def = self.balloon_def(pos, defs)?;
        let held = self.anchored(pos)?.dock.as_ref().map_or(0, Dock::held);
        Some((held, def.hold))
    }

    /// Every stack waiting in the hold of the dock anchored on `pos`, oldest first.
    pub fn dock_stacks(&self, pos: Pos) -> impl Iterator<Item = Stack> + '_ {
        self.anchored(pos)
            .and_then(|b| b.dock.as_ref())
            .into_iter()
            .flat_map(|d| d.hold.iter().map(|h| h.stack))
    }

    /// How many more items the dock anchored on `pos` takes.
    fn dock_room(&self, pos: Pos, defs: &Defs) -> u32 {
        self.dock_hold(pos, defs).map_or(0, |(held, hold)| hold.saturating_sub(held))
    }

    /// The door tile of the finished building anchored on `pos`.
    fn door_of(&self, pos: Pos) -> Option<Pos> {
        Some(self.anchored(pos)?.footprint().entrance())
    }

    /// Ticks a flight over `distance` tiles takes.
    fn flight_ticks(def: &BalloonDef, distance: u32) -> u64 {
        let seconds = distance as f32 / def.tiles_per_second;
        ((seconds * TICKS_PER_SECOND as f32).ceil() as u64).max(1)
    }

    /// The dock unit `i` should take `stack` to instead of walking `straight` (its cost to the
    /// nearest stockpile with room, if any), and the path to stand at its door: the cheapest
    /// dock walk plus flight (at the unit's own speed) plus partner-door-to-stores walk that
    /// beats `straight`. Ties go to walking.
    pub(super) fn balloon_for(
        &mut self,
        i: usize,
        flood: &Flood,
        stack: Stack,
        straight: Option<u32>,
        defs: &Defs,
    ) -> Option<(Pos, Vec<Pos>)> {
        if !self.has_kind(Kind::Balloon) || Self::is_readable(stack.item, defs) {
            return None;
        }
        self.ensure_regions();
        let docks: Vec<Pos> = self.of_kind(Kind::Balloon).map(|b| b.anchor).collect();
        let speed = self.modifiers_for(&self.units[i], defs).move_speed
            * self.scooter_multiplier(self.units[i].id);
        let mut best: Option<(u32, Pos, Pos)> = None;
        for dock in docks {
            let Some(partner) = self.pair_partner(dock, defs) else { continue };
            let Some(def) = self.balloon_def(dock, defs) else { continue };
            if self.dock_room(dock, defs) < stack.count {
                continue;
            }
            let Some((walk, stand)) = self.stand_building(flood, dock, defs) else { continue };
            if straight.is_some_and(|s| walk >= s) {
                continue;
            }
            // The flight, in the flood's cost units: what the unit's own walk would cover in
            // the flight's ticks.
            let ticks = Self::flight_ticks(&def, dock.chebyshev(partner) as u32);
            let fly = (ticks as f32 * speed * STRAIGHT as f32 / BASE_TICKS_PER_STEP).round() as u32;
            let Some(door) = self.door_of(partner) else { continue };
            let walks = self.foot_walks(partner, door, i).to_vec();
            let down = walks.into_iter().find(|&(_, z)| self.room_at(z, stack.item, defs) > 0);
            let Some((down, _)) = down else { continue };
            let total = walk + fly + down;
            if straight.is_none_or(|s| total < s) && best.is_none_or(|(c, _, _)| total < c) {
                best = Some((total, dock, stand));
            }
        }
        let (_, dock, stand) = best?;
        Some((dock, flood.path_to(stand)?))
    }

    /// One tick of carrying a stack to the dock anchored on `dock` (`Task::Balloon`). At the door
    /// it goes into the hold. If the dock is gone, its pair no longer works, or its hold has no
    /// room for the stack, the unit carries it to stores on foot instead.
    pub(super) fn run_balloon(&mut self, i: usize, dock: Pos, defs: &Defs) {
        let Some(stack) = self.units[i].carrying else { return self.finish_job(i) };
        self.ensure_regions();
        if self.pair_partner(dock, defs).is_none() {
            return self.walk_instead(i);
        }
        match self.go_to(i, StandRule::Building(dock), defs) {
            Go::Walking => {}
            Go::Unreachable => self.walk_instead(i),
            Go::Arrived => {
                if self.dock_room(dock, defs) < stack.count {
                    return self.walk_instead(i);
                }
                self.units[i].carrying = None;
                self.units[i].route = None;
                let at = self.tick;
                let b = self.anchored_mut(dock).expect("a working dock");
                b.dock.get_or_insert_default().hold.push(Held { stack, at });
                self.finish_job(i);
            }
        }
    }

    /// What the parked balloon at `dock` would fly next, fuel aside: its own goods if they are
    /// ready, else an empty flight to the partner if the goods are ready there.
    fn plan_launch(&self, dock: Pos, partner: Pos, def: &BalloonDef) -> Option<Plan> {
        let ready = |pos: Pos| {
            let Some(d) = self.anchored(pos).and_then(|b| b.dock.as_ref()) else { return false };
            let Some(oldest) = d.hold.first() else { return false };
            d.held() >= def.load || self.tick.saturating_sub(oldest.at) >= def.wait_ticks
        };
        if ready(dock) {
            Some(Plan::Loaded)
        } else if ready(partner) {
            Some(Plan::Empty)
        } else {
            None
        }
    }

    /// Lands, assigns and launches balloons, one step each per tick.
    pub(super) fn tick_balloons(&mut self, defs: &Defs) {
        if !self.has_kind(Kind::Balloon) {
            return;
        }
        self.ensure_regions();
        let docks: Vec<Pos> = self.of_kind(Kind::Balloon).map(|b| b.anchor).collect();
        for &dock in &docks {
            self.fly(dock);
        }
        for &dock in &docks {
            self.assign_balloon(dock, defs);
        }
        for &dock in &docks {
            self.launch(dock, defs);
        }
    }

    /// One tick in the air for a balloon flying toward `dock`; on landing its goods spill beside
    /// the door as ground stacks, each with a Haul job.
    fn fly(&mut self, dock: Pos) {
        let Some(door) = self.door_of(dock) else { return };
        let Some(balloon) = self.anchored_mut(dock).and_then(|b| b.dock.as_mut()?.balloon.as_mut())
        else {
            return;
        };
        let Some(flight) = &mut balloon.flight else { return };
        flight.left = flight.left.saturating_sub(1);
        if flight.left > 0 {
            return;
        }
        let cargo = std::mem::take(&mut flight.cargo);
        balloon.flight = None;
        for stack in cargo {
            self.place_ground(door, stack);
        }
    }

    /// Gives the pair of `dock` its one balloon if it has none: at whichever dock was finished
    /// first. If both ends have one (a relink joined two pairs), the later dock's goes, its
    /// cargo spilling at its door.
    fn assign_balloon(&mut self, dock: Pos, defs: &Defs) {
        let Some(partner) = self.pair_partner(dock, defs) else { return };
        let key = |w: &World, p: Pos| w.map.get(p).and_then(|t| t.building).map(|b| b.key());
        if key(self, dock) >= key(self, partner) {
            return;
        }
        let has = |w: &World, p: Pos| {
            w.anchored(p).and_then(|b| b.dock.as_ref()).is_some_and(|d| d.balloon.is_some())
        };
        match (has(self, dock), has(self, partner)) {
            (false, false) => {
                let b = self.anchored_mut(dock).expect("a working dock");
                b.dock.get_or_insert_default().balloon = Some(Balloon::default());
            }
            (true, true) => {
                let door = self.door_of(partner).expect("a working dock");
                let gone = self.anchored_mut(partner).and_then(|b| b.dock.as_mut()?.balloon.take());
                for stack in gone.and_then(|b| b.flight).map(|f| f.cargo).unwrap_or_default() {
                    self.place_ground(door, stack);
                }
            }
            _ => {}
        }
    }

    /// Launches the balloon parked at `dock` if it has a flight to make and fuel for it.
    fn launch(&mut self, dock: Pos, defs: &Defs) {
        let Some(partner) = self.pair_partner(dock, defs) else { return };
        let Some(def) = self.balloon_def(dock, defs) else { return };
        let Some(d) = self.anchored(dock).and_then(|b| b.dock.as_ref()) else { return };
        if !d.balloon.as_ref().is_some_and(|b| b.flight.is_none()) {
            return;
        }
        let Some(plan) = self.plan_launch(dock, partner, &def) else { return };
        let Some(fuel) = self.anchored(dock).and_then(|b| b.fuel) else { return };
        if fuel.count < def.fuel_per_flight {
            return;
        }
        let b = self.anchored_mut(dock).expect("a working dock");
        b.fuel.as_mut().expect("checked above").count -= def.fuel_per_flight;
        let state = b.dock.as_mut().expect("checked above");
        state.balloon = None;
        let mut cargo: Vec<Stack> = Vec::new();
        if plan == Plan::Loaded {
            let mut room = def.load;
            while room > 0 && !state.hold.is_empty() {
                let held = &mut state.hold[0];
                let take = held.stack.count.min(room);
                held.stack.count -= take;
                room -= take;
                let item = held.stack.item;
                match cargo.iter_mut().find(|s| s.item == item) {
                    Some(s) => s.count += take,
                    None => cargo.push(Stack { item, count: take }),
                }
                if held.stack.count == 0 {
                    state.hold.remove(0);
                }
            }
        }
        let left = Self::flight_ticks(&def, dock.chebyshev(partner) as u32);
        let there = self.anchored_mut(partner).expect("a working dock");
        there.dock.get_or_insert_default().balloon =
            Some(Balloon { flight: Some(Flight { left, cargo }) });
    }

    /// What the balloon of the dock anchored on `pos` is doing, or `None` if no Balloon Dock is
    /// anchored there or its pair doesn't work (`pair_status` says why).
    pub fn balloon_status(&self, pos: Pos, defs: &Defs) -> Option<BalloonStatus> {
        let def = self.balloon_def(pos, defs)?;
        let partner = self.pair_partner(pos, defs)?;
        let balloon = self.anchored(pos)?.dock.as_ref().and_then(|d| d.balloon.as_ref());
        let Some(balloon) = balloon else { return Some(BalloonStatus::Away) };
        if let Some(flight) = &balloon.flight {
            return Some(BalloonStatus::Landing { ticks: flight.left });
        }
        let fuel = defs.building(self.anchored(pos)?.id).fuel.as_ref()?;
        let have = self.anchored(pos)?.fuel.map_or(0, |f| f.count);
        match self.plan_launch(pos, partner, &def) {
            Some(_) if have < def.fuel_per_flight => Some(BalloonStatus::OutOfFuel(fuel.item)),
            _ => Some(BalloonStatus::Parked),
        }
    }
}

#[cfg(test)]
mod tests;
