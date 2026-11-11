//! Outposts (`[building.outpost]`, the Waystation, docs/design/horizons-phase-2.md,
//! "`waystations`: the Waystation", and #688).
//!
//! A far outpost is not a second base: it holds no general store, so far yields keep flowing
//! home. It has three things, all data on `OutpostDef`:
//!
//! - **A pantry.** A bunker of food (`Pantry`, on the building's record) that hungry units eat
//!   from at the building's door. `plan_food` counts it with the stockpiles in the need flood
//!   (`FoodSource::Pantry`), after a stockpile on a tie in cost. Its food is out of stores, so
//!   it is never counted in `stored_total`, and `deconstruct.rs` drops it by the anchor.
//! - **A refill.** At or below half full a `JobKind::Refuel` is opened, the same job a fuel
//!   bunker uses. A unit fetches the shortfall (up to what it can carry) of one of the pantry's
//!   items from the nearest stockpile or ground stack and tips it in (`run_restock`). A beast
//!   takes this long trip when there is one (#693).
//! - **A barrel.** A drinking place at the door that counts as fresh water: `plan_drink` treats
//!   the door as a spot beside water, and a drink there never rolls sickness
//!   (`sickness_chance`).
//!
//! The beds are the building's own `bed` and `capacity` (#227). Where it may stand is checked
//! once, at placement (`far_blocker`): every footprint tile at least `far_from_stores` tiles,
//! Chebyshev, from any stockpile tile. A stockpile drawn beside it later is the player's choice.

use crate::building::OutpostDef;
use crate::flood::Flood;
use crate::jobs::{ItemId, JobKind, Stack};

use super::build::Source;
use super::buildings::{Building, Kind};
use super::work::route_from;
use super::*;

/// The food an outpost's pantry holds, one stack per item in the order first stocked.
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Pantry {
    pub stacks: Vec<Stack>,
}

impl Pantry {
    /// Every item held.
    pub fn total(&self) -> u32 {
        self.stacks.iter().map(|s| s.count).sum()
    }

    /// How many of `item` it holds.
    pub fn count_of(&self, item: ItemId) -> u32 {
        self.stacks.iter().filter(|s| s.item == item).map(|s| s.count).sum()
    }

    fn add(&mut self, item: ItemId, count: u32) {
        match self.stacks.iter_mut().find(|s| s.item == item) {
            Some(s) => s.count += count,
            None => self.stacks.push(Stack { item, count }),
        }
    }

    fn take_one(&mut self, item: ItemId) {
        if let Some(i) = self.stacks.iter().position(|s| s.item == item) {
            self.stacks[i].count -= 1;
            if self.stacks[i].count == 0 {
                self.stacks.remove(i);
            }
        }
    }
}

impl Building {
    /// Its pantry, if it has one.
    pub fn pantry(&self) -> Option<&Pantry> {
        self.pantry.as_ref()
    }
}

impl World {
    /// The pantry of the building anchored on `pos` and its size, if it has one: the cursor
    /// panel's `Pantry: 9/12` (`fn_ui`).
    pub fn pantry_at(&self, pos: Pos, defs: &Defs) -> Option<(&Pantry, u32)> {
        let b = self.building_record(pos)?;
        let size = defs.building(b.id).outpost.as_ref()?.pantry;
        (size > 0).then_some((b.pantry.as_ref()?, size))
    }

    /// Whether `pos` is the door of a finished building with a barrel.
    pub fn is_barrel_door(&self, pos: Pos, defs: &Defs) -> bool {
        self.barrel_doors(defs).any(|d| d == pos)
    }

    /// The doors of every building with a barrel.
    fn barrel_doors<'a>(&'a self, defs: &'a Defs) -> impl Iterator<Item = Pos> + 'a {
        self.of_kind(Kind::Outpost)
            .filter(|b| defs.building(b.id).outpost.as_ref().is_some_and(|o| o.barrel))
            .map(|b| b.footprint().entrance())
    }

    /// The door and anchor of every pantry holding something unit `me` will eat, in anchor
    /// order (`plan_food`).
    pub(super) fn pantry_doors(
        &self,
        defs: &Defs,
        eats: impl Fn(ItemId) -> bool,
    ) -> Vec<(Pos, Pos)> {
        self.of_kind(Kind::Outpost)
            .filter(|b| b.pantry.as_ref().is_some_and(|p| p.stacks.iter().any(|s| eats(s.item))))
            .filter(|b| defs.building(b.id).outpost.as_ref().is_some_and(|o| o.pantry > 0))
            .map(|b| (b.footprint().entrance(), b.anchor))
            .collect()
    }

    /// The doors `plan_drink` counts as a spot beside water (empty with no barrel standing).
    pub(super) fn barrel_door_list(&self, defs: &Defs) -> Vec<Pos> {
        self.barrel_doors(defs).collect()
    }

    /// The item unit `i` would eat from the pantry of the building anchored on `anchor`: the
    /// first of its `pantry_items` in stock that the unit will eat.
    pub(super) fn pantry_item(&self, i: usize, anchor: Pos, defs: &Defs) -> Option<ItemId> {
        let b = self.anchored(anchor)?;
        let pantry = b.pantry.as_ref()?;
        let o = defs.building(b.id).outpost.as_ref()?;
        o.pantry_items.iter().copied().find(|&item| {
            pantry.count_of(item) > 0
                && defs.food_of(item).is_some()
                && !self.refuses_food(i, item, defs)
        })
    }

    /// Takes one `item` out of the pantry anchored on `anchor`.
    pub(super) fn take_from_pantry(&mut self, anchor: Pos, item: ItemId) {
        if let Some(p) = self.anchored_mut(anchor).and_then(|b| b.pantry.as_mut()) {
            p.take_one(item);
        }
    }

    /// Why `foot` is too near the stockpiles for `def`, if it has a `far_from_stores` rule: the
    /// first footprint tile (row-major) within that many tiles of a stockpile tile, with the
    /// blocker saying how far it should be.
    pub(super) fn far_blocker(
        &self,
        foot: crate::geom::Footprint,
        def: &BuildingDef,
    ) -> Option<(Pos, super::build::Blocker)> {
        let far = def.outpost.as_ref().map(|o| o.far_from_stores).filter(|&f| f > 0)?;
        foot.tiles()
            .find(|&t| self.zones.iter().any(|&z| z.chebyshev(t) < far as i32))
            .map(|t| (t, super::build::Blocker::NearStores(far)))
    }

    /// Opens a `Refuel` job for each pantry at or below half full that has none.
    pub(super) fn tick_outposts(&mut self, defs: &Defs) {
        if !self.has_kind(Kind::Outpost) {
            return;
        }
        let low: Vec<Pos> = self
            .of_kind(Kind::Outpost)
            .filter(|b| {
                let size = defs.building(b.id).outpost.as_ref().map_or(0, |o| o.pantry);
                size > 0 && b.pantry.as_ref().map_or(0, Pantry::total) * 2 <= size
            })
            .map(|b| b.anchor)
            .collect();
        for anchor in low {
            if !self.jobs.iter().any(|j| j.kind == JobKind::Refuel { pos: anchor }) {
                self.add_job(JobKind::Refuel { pos: anchor });
            }
        }
    }

    /// How many items the pantry anchored on `pos` is short of full, or `None` off a building
    /// with a pantry.
    pub(super) fn pantry_shortfall(&self, pos: Pos, defs: &Defs) -> Option<u32> {
        let b = self.anchored(pos)?;
        let o: &OutpostDef = defs.building(b.id).outpost.as_ref().filter(|o| o.pantry > 0)?;
        Some(o.pantry.saturating_sub(b.pantry.as_ref()?.total()))
    }

    /// Whether some stockpile tile or ground stack holds an item the pantry anchored on `pos`
    /// keeps: the cheap check before a flood.
    pub(super) fn any_source_for_pantry(&self, pos: Pos, defs: &Defs) -> bool {
        let Some(o) = self.anchored(pos).and_then(|b| defs.building(b.id).outpost.as_ref()) else {
            return false;
        };
        let keeps = |s: &Stack| o.pantry_items.contains(&s.item);
        self.stored.values().any(keeps) || self.ground.values().any(keeps)
    }

    /// The nearest stockpile tile or ground stack holding any of `items` (stockpiles first on a
    /// tie), with the tile to stand on and the item found.
    fn nearest_source_of(&self, flood: &Flood, items: &[ItemId]) -> Option<(Source, Pos, ItemId)> {
        type Best = ((u32, u8, Pos), Source, Pos, ItemId);
        let mut best: Option<Best> = None;
        let mut consider = |rank: u8, pos: Pos, source: Source, item: ItemId| {
            let Some((cost, stand)) = self.stand_tile(flood, pos) else { return };
            let key = (cost, rank, pos);
            if best.as_ref().is_none_or(|b| key < b.0) {
                best = Some((key, source, stand, item));
            }
        };
        for (&pos, s) in &self.stored {
            if items.contains(&s.item) {
                consider(0, pos, Source::Stockpile(pos), s.item);
            }
        }
        for (&pos, s) in &self.ground {
            if items.contains(&s.item) {
                consider(1, pos, Source::Ground(pos), s.item);
            }
        }
        best.map(|(_, source, stand, item)| (source, stand, item))
    }

    /// One tick of a unit restocking the pantry anchored on `pos`: fetch up to the shortfall of
    /// the nearest item it keeps, carry it to the building and tip it in. One trip finishes the
    /// job; `tick_outposts` opens another if the pantry is still at or below half.
    pub(super) fn run_restock(&mut self, i: usize, id: JobId, pos: Pos, defs: &Defs) {
        let Some(short) = self.pantry_shortfall(pos, defs) else {
            let idx = self.job_index(id).expect("still queued");
            self.jobs.remove(idx);
            return self.finish_job(i);
        };
        let items: Vec<ItemId> = {
            let b = self.anchored(pos).expect("shortfall found it");
            defs.building(b.id).outpost.as_ref().expect("a pantry").pantry_items.clone()
        };
        if let Some(stack) = self.units[i].carrying.filter(|s| items.contains(&s.item)) {
            if !self.go_or_abandon(i, id, StandRule::Building(pos), defs) {
                return;
            }
            let short = self.pantry_shortfall(pos, defs).expect("checked above");
            let put = stack.count.min(short);
            if let Some(p) = self.anchored_mut(pos).and_then(|b| b.pantry.as_mut()) {
                p.add(stack.item, put);
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
            let idx = self.job_index(id).expect("still queued");
            self.jobs.remove(idx);
            return self.finish_job(i);
        }
        if !self.any_source_for_pantry(pos, defs) {
            return self.abandon_job(i, id, defs);
        }
        if self.keep_walking(i, defs) {
            return;
        }
        let flood = self.flood_from(i);
        let Some((source, stand, item)) = self.nearest_source_of(&flood, &items) else {
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
}

#[cfg(test)]
mod tests;
