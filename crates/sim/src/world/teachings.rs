//! Lost teachings (docs/design/horizons-phase-5.md): the settlement's state for phase 5.
//!
//! #736 adds only `placed`, the flag that tells a pre-phase-5 save to get its great ruins on
//! load (`World::place_great_ruins`). The draft (kept, lost and pending offers) is #737's.

use serde::{Deserialize, Serialize};

use super::*;

/// The settlement's phase-5 state. Keyed by id, never by position (tech-stack.md, "Saves").
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Teachings {
    /// Whether this world's great ruins have been placed: `false` only in a save from before
    /// phase 5, which `World::load` upgrades.
    #[serde(default)]
    pub(super) placed: bool,
}

impl Teachings {
    /// A new world's: mapgen already placed its great ruins.
    pub(super) fn new() -> Self {
        Self { placed: true }
    }
}

impl World {
    /// Gives a pre-phase-5 save its great ruins (docs/design/horizons-phase-5.md, "Old saves"):
    /// runs `mapgen::pick_promoted` over the still-buried tiles of each promoted resource's
    /// `from` (in scan order, with the saved map for sizes and the save's own seed) and rewrites
    /// those buried entries. Ruins already uncovered are left alone, since the player has seen
    /// them as they are. Then sets `placed`, so this runs once. Deterministic: the same save
    /// always gets the same ruins.
    pub(crate) fn place_great_ruins(&mut self, defs: &Defs) {
        if self.teachings.placed {
            return;
        }
        for (i, r) in defs.resources.iter().enumerate() {
            let Some(promote) = &r.promote else { continue };
            let Some(from) = defs.resource_id(&promote.from) else { continue };
            let mut candidates: Vec<Pos> =
                self.buried.iter().filter(|&(_, &b)| b == from).map(|(&p, _)| p).collect();
            candidates.sort_by_key(|p| (p.y, p.x));
            for p in mapgen::pick_promoted(&self.map, &candidates, promote, self.seed, &r.id) {
                self.buried.insert(p, ResourceId(i as u16));
            }
        }
        self.teachings.placed = true;
    }

    /// Whether this world's great ruins have been placed (`Teachings::placed`).
    pub fn great_ruins_placed(&self) -> bool {
        self.teachings.placed
    }
}
