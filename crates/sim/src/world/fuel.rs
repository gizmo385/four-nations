//! Fuel bunkers (`[building.fuel]`, docs/design/horizons-phase-1.md, "Fuel bunkers").
//!
//! A building with a `FuelDef` keeps a `Bunker` on its record: how many items of its fuel it
//! holds, and how far through burning the next one it is, in millionths of an item
//! (`FUEL_MICROS`), so burning is integer arithmetic and deterministic. It finishes empty.
//!
//! - **On only while fuelled.** The building's effect runs only while the bunker holds at least
//!   one item (`Building::fuel_lit`). A heat source with an empty bunker is out: it isn't among
//!   `Buildings::fires`, so `is_heated`, `is_cooking`, `sleep_factor_at` and `warm_sleep_route`
//!   (`heat.rs`) all skip it.
//! - **Burns only while in use.** Each tick, a fuelled bunker burns `micros_per_tick` if its
//!   effect says it is in use (`World::fuel_in_use`, one arm per effect): a heat source while any
//!   unit stands in its reach, a Hothouse while it warms a plot (`hothouse.rs`). Later effects
//!   (the Steam Engine) add their own arm.
//! - **Per flight instead.** A bunker with no `per_minute` never burns here: its building's own
//!   effect takes fuel when it acts (the Balloon Dock's launches, `balloon.rs`).
//! - **Refuelled from stores.** At or below half full, a `JobKind::Refuel` is opened. A unit
//!   fetches the shortfall (up to what it can carry) from the nearest stockpile or ground stack,
//!   the way a build site's bill arrives, and tips it into the bunker. Fuel in a bunker is out
//!   of stores, so it is never eaten.
//! - **Several fuels.** `items = [...]` in place of `item` lists what a bunker burns (#693, a
//!   beast's home burns any food). A Refuel haul brings whichever listed item stores hold the
//!   most of, ties broken by list order. A bunker may end up holding a mix; it remembers only
//!   the item last tipped in (`Bunker::item`), which is what taking the building down returns.
//! - **Tea burns always.** A Tea House's bunker burns whenever the house is finished and stocked,
//!   not only during a visit (`tea.rs`), since draw runs all the time.
//! - **A beast eats.** A bunker whose building houses a beast burns for as long as it does
//!   (`fuel_in_use`, `world/beast.rs`).
//! - **Returned in full.** Taking the building down (`deconstruct.rs`) drops the bunker's items
//!   by its anchor.

use crate::building::{BuildingDef, FUEL_MICROS};
use crate::jobs::{ItemId, JobKind, Stack};

use super::buildings::Kind;
use super::work::route_from;
use super::*;

/// A building's fuel bunker.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Bunker {
    /// Items of its fuel it holds.
    pub count: u32,
    /// How far through burning the next item it is, in `FUEL_MICROS` units.
    #[serde(default, skip_serializing_if = "is_zero")]
    pub burned: u64,
    /// The item last tipped in: what a bunker with several fuels gives back when its building
    /// comes down. `None` until something is, and in a save from before `items` existed, which
    /// reads as the building's first item.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub item: Option<ItemId>,
}

fn is_zero(n: &u64) -> bool {
    *n == 0
}

impl World {
    /// The bunker of the building anchored on `pos`, with its def's size, if it has one: the
    /// cursor panel's `Fuel: 4/6 Fish` (`fn_ui`).
    pub fn bunker_at<'d>(
        &self,
        pos: Pos,
        defs: &'d Defs,
    ) -> Option<(Bunker, &'d crate::building::FuelDef)> {
        let b = self.building_record(pos)?;
        Some((b.fuel?, defs.building(b.id).fuel.as_ref()?))
    }

    /// Whether the effect of `b` (a building with `def`) is in use this tick, so its bunker
    /// burns. One arm per effect that can take fuel; an effect without an arm never burns.
    fn fuel_in_use(&self, b: &buildings::Building, def: &BuildingDef, defs: &Defs) -> bool {
        if let Some(heat) = b.heat {
            // A fire burns while anyone is close enough to feel it.
            return self.units.iter().any(|u| !u.is_beast() && heat.warms(u.pos));
        }
        if def.tea_house.is_some() {
            // A Tea House burns whenever it is finished and stocked: draw runs all the time.
            return true;
        }
        if def.hothouse.is_some() {
            // A Hothouse burns while it is warming a plot: winter, a crop in reach.
            return self.plots_warmed_by(b, defs).next().is_some();
        }
        // A beast eats for as long as it lives in the building, whatever it is doing.
        self.houses_beast(b.anchor)
    }

    /// Burns each fuelled bunker that is in use, then opens a `Refuel` job for each one at or
    /// below half full that has none.
    pub(super) fn tick_fuel(&mut self, defs: &Defs) {
        if !self.has_kind(Kind::Fuel) {
            return;
        }
        let mut burning: Vec<Pos> = Vec::new();
        let mut low: Vec<Pos> = Vec::new();
        for b in self.of_kind(Kind::Fuel) {
            let def = defs.building(b.id);
            let (Some(bunker), Some(fuel)) = (b.fuel, def.fuel.as_ref()) else { continue };
            // A bunker with no `per_minute` burns some other way (a balloon's flights).
            if bunker.count > 0 && fuel.micros_per_tick > 0 && self.fuel_in_use(b, def, defs) {
                burning.push(b.anchor);
            }
            if bunker.count * 2 <= fuel.bunker {
                low.push(b.anchor);
            }
        }
        for anchor in burning {
            let def = defs.building(self.anchored(anchor).expect("listed above").id);
            let per_tick = def.fuel.as_ref().expect("a fuel building").micros_per_tick;
            let bunker = self.anchored_mut(anchor).and_then(|b| b.fuel.as_mut()).expect("fuel");
            bunker.burned += per_tick;
            while bunker.burned >= FUEL_MICROS && bunker.count > 0 {
                bunker.burned -= FUEL_MICROS;
                bunker.count -= 1;
            }
            if bunker.count == 0 {
                bunker.burned = 0;
            }
        }
        for anchor in low {
            if !self.jobs.iter().any(|j| j.kind == JobKind::Refuel { pos: anchor }) {
                self.add_job(JobKind::Refuel { pos: anchor });
            }
        }
    }

    /// The fuel item to bring and how many the bunker anchored on `pos` is short of full, or
    /// `None` off a fuel building. With several fuels, the item is whichever stores hold the
    /// most of, ties broken by list order (`FuelDef::items`); with none in stores, the first.
    pub(super) fn fuel_shortfall(&self, pos: Pos, defs: &Defs) -> Option<(ItemId, u32)> {
        let b = self.anchored(pos)?;
        let fuel = defs.building(b.id).fuel.as_ref()?;
        let mut best = fuel.item;
        let mut most = 0;
        for &item in &fuel.items {
            let held = self.fuel_on_hand(item);
            if held > most {
                (best, most) = (item, held);
            }
        }
        Some((best, fuel.bunker.saturating_sub(b.fuel?.count)))
    }

    /// How many of `item` stores hold: stockpile tiles and ground stacks, the places a Refuel
    /// fetches from (`nearest_source`).
    fn fuel_on_hand(&self, item: ItemId) -> u32 {
        let count = |m: &BTreeMap<Pos, Stack>| -> u32 {
            m.values().filter(|s| s.item == item).map(|s| s.count).sum()
        };
        count(&self.stored) + count(&self.ground)
    }

    /// One tick of a unit refuelling the bunker anchored on `pos`: fetch up to the shortfall
    /// from the nearest store of the fuel, carry it to the building and tip it in. One trip
    /// finishes the job; `tick_fuel` opens another if the bunker is still low.
    pub(super) fn run_refuel(&mut self, i: usize, id: JobId, defs: &Defs) {
        let Some(idx) = self.job_index(id) else { return self.finish_job(i) };
        let JobKind::Refuel { pos } = self.jobs[idx].kind else { return self.finish_job(i) };
        if self.pantry_shortfall(pos, defs).is_some() {
            // An outpost's pantry, not a fuel bunker (`outpost.rs`).
            return self.run_restock(i, id, pos, defs);
        }
        if self.fuel_shortfall(pos, defs).is_none() && self.bill_hall(defs) == Some(pos) {
            // A research delivery to a bill hall (`bill.rs`).
            return self.run_bill(i, id, pos, defs);
        }
        let Some((item, short)) = self.fuel_shortfall(pos, defs) else {
            // The building is gone: moot. Anything in hand goes back to stores (`run_idle`).
            self.jobs.remove(idx);
            return self.finish_job(i);
        };
        // Whatever it already carries that the bunker burns is what it delivers.
        let burns = |s: &Stack| {
            defs.building(self.anchored(pos).expect("checked above").id)
                .fuel
                .as_ref()
                .is_some_and(|f| f.items.contains(&s.item))
        };
        if let Some(stack) = self.units[i].carrying.filter(burns) {
            if !self.go_or_abandon(i, id, StandRule::Building(pos), defs) {
                return;
            }
            let (_, short) = self.fuel_shortfall(pos, defs).expect("checked above");
            let put = stack.count.min(short);
            let bunker = self.anchored_mut(pos).and_then(|b| b.fuel.as_mut()).expect("fuel");
            bunker.count += put;
            if put > 0 {
                bunker.item = Some(stack.item);
            }
            let left = stack.count - put;
            self.units[i].carrying = (left > 0).then_some(Stack { item: stack.item, count: left });
            self.units[i].route = None;
            let idx = self.job_index(id).expect("still queued");
            self.jobs.remove(idx);
            return self.finish_job(i);
        }
        if short == 0 || self.units[i].carrying.is_some() {
            // Topped up some other way, or hands full of something else: done for now.
            self.jobs.remove(idx);
            return self.finish_job(i);
        }
        if !self.any_source_for_fuel(item) {
            return self.abandon_job(i, id, defs);
        }
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
        let take = self.take_for_build(i, source, short, defs);
        if take == 0 {
            return self.abandon_job(i, id, defs);
        }
        self.units[i].carrying = Some(Stack { item, count: take });
        self.units[i].route = None;
    }

    /// Whether the `Refuel` job for the building anchored on `pos` is still worth keeping: it
    /// has a bunker or a pantry.
    pub(super) fn refuel_live(&self, pos: Pos, defs: &Defs) -> bool {
        self.fuel_shortfall(pos, defs).is_some()
            || self.pantry_shortfall(pos, defs).is_some()
            || self.bill_live(pos, defs)
    }

    /// Whether a unit could usefully take the `Refuel` job for the building anchored on `pos`
    /// now: its bunker or pantry is short and some store holds what it keeps.
    pub(super) fn refuel_ready(&self, pos: Pos, defs: &Defs) -> bool {
        self.fuel_shortfall(pos, defs)
            .is_some_and(|(item, short)| short > 0 && self.any_source_for_fuel(item))
            || self
                .pantry_shortfall(pos, defs)
                .is_some_and(|short| short > 0 && self.any_source_for_pantry(pos, defs))
            || self.bill_ready(pos, defs)
    }

    /// Whether any stockpile tile or ground stack holds `item`: the cheap check before a flood.
    pub(super) fn any_source_for_fuel(&self, item: ItemId) -> bool {
        self.stored.values().any(|s| s.item == item) || self.ground.values().any(|s| s.item == item)
    }
}

#[cfg(test)]
mod tests;
