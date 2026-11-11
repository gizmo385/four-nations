//! A walking-cost field per link end (docs/design/horizons-phase-2.md, "Shared sim work, 2.
//! Routes through links"): the cost of the cheapest walk from a pair end (a Wind Gate, a Balloon
//! Dock) to every tile, for a route that compares walking straight there with going by a link.
//!
//! - **One field per end.** `World::link_field(end, rules)` floods from `end` once and keeps every
//!   tile's cost. A route that asks again, for the same end, gets the kept field, so comparing a
//!   link costs a lookup, not a flood. The caller picks the ends it needs and whichever of the
//!   two directions it wants: the walk from an end is the field read at the destination, the walk
//!   to an end is the field read at where the unit stands. The two differ only by the cost of
//!   the ground at the two ends of the walk, which is why a field serves both.
//! - **Keyed on the map revision.** A field belongs to the `StepGrid` it was flooded over, which
//!   `World` keeps per map revision and `MoveRules` (`work::StepGrids`). Any map change, or a
//!   different rule set, gives a new grid, so every field built before it is stale and the next
//!   ask floods again. Nothing else invalidates a field: moving a building's link changes no
//!   walking cost. Not saved, and a clone starts empty.
//!
//! Nothing here knows what a link is, so people's gates (`glide.rs`) and goods' docks share it.

use std::collections::BTreeMap;
use std::collections::btree_map::Entry;
use std::sync::Arc;

use crate::flood::{Flood, StepGrid};

use super::work::rules_for;
use super::*;

/// No path: a tile the end can't reach.
const NONE: u32 = u32::MAX;

/// What a field holds for each tile of the map, row by row.
#[derive(Debug)]
pub(super) struct LinkField {
    /// The grid it was flooded over: current while it is the one `World` hands out now.
    grid: Arc<StepGrid>,
    width: i32,
    cost: Vec<u32>,
}

impl LinkField {
    /// The cost of the cheapest walk between the field's end and `p` (the units of `Flood::cost`),
    /// or `None` if there is none.
    pub(super) fn cost(&self, p: Pos) -> Option<u32> {
        if p.x < 0 || p.y < 0 || p.x >= self.width {
            return None;
        }
        let at = self.cost.get((p.y * self.width + p.x) as usize).copied()?;
        (at != NONE).then_some(at)
    }
}

/// Every end's field, by anchor.
#[derive(Debug, Default)]
pub(super) struct LinkFields {
    fields: BTreeMap<Pos, LinkField>,
    /// How many fields were flooded since this was made: for a test to see a cache hit or miss.
    #[cfg(test)]
    pub(super) floods: u32,
}

impl Clone for LinkFields {
    /// A clone starts empty, as `StepGrids` does: the fields are only a cache.
    fn clone(&self) -> Self {
        Self::default()
    }
}

impl LinkFields {
    /// The field of `end` over `grid`, flooded now if it has none or its own is from an older
    /// grid.
    fn get(&mut self, end: Pos, grid: Arc<StepGrid>, map: &TileMap) -> &LinkField {
        // Older grids' fields are stale for good, and would keep their grids alive.
        self.fields.retain(|_, f| Arc::ptr_eq(&f.grid, &grid));
        match self.fields.entry(end) {
            Entry::Occupied(held) => held.into_mut(),
            Entry::Vacant(room) => {
                #[cfg(test)]
                {
                    self.floods += 1;
                }
                let flood = Flood::over(Arc::clone(&grid), end);
                let cost = map.iter().map(|(p, _)| flood.cost(p).unwrap_or(NONE)).collect();
                room.insert(LinkField { grid, width: map.width(), cost })
            }
        }
    }
}

impl World {
    /// The walking-cost field of the link end anchored on `end`, under the rules unit `i` walks by
    /// (`rules_for`), cached until the map changes.
    pub(super) fn link_field(&mut self, end: Pos, i: usize) -> &LinkField {
        let rules = rules_for(&self.rules, &self.cart_rules, &self.units[i]);
        let grid = self.step_grids.get(&self.map, self.map_rev, rules);
        self.link_fields.get(end, grid, &self.map)
    }
}
