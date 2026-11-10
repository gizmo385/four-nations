//! The tile shape of save versions 1 and 2, frozen, and the upgrade to version 3 (#604,
//! `docs/design/buildings-as-records.md`, "Saves: v2 → v3").
//!
//! Up to version 2 every tile of a finished building carried a full copy of it (`Building`
//! below: its id, footprint size and its own offset from the anchor), and the building's state
//! lived in side tables keyed by anchor (`crafts`, `charts`, `orchards`, `farm_plots`, and the
//! building fires in `heat`). Version 3 keeps one record per building (`World::buildings`) and
//! has each tile point at it.
//!
//! The old shapes are read through deserialize-only fields on `World` (`v2_map`, `v2_crafts`
//! and the rest, named for the fields they used to be), never written. `World::load` parses
//! the body inside #602's id remap, so the `BuildingId`s here are already today's. Then
//! `World::upgrade_from_v2` turns them into records and empties every shim.

use crate::building::{BuildingId, one_by_one};
use crate::defs::{BiomeId, ResourceId, Terrain};

/// A v1 or v2 map tile: today's `Tile`, but carrying a whole `Building` copy.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Deserialize)]
pub(crate) struct Tile {
    pub elevation: u8,
    pub temperature: u8,
    pub moisture: u8,
    pub terrain: Terrain,
    pub biome: BiomeId,
    pub resource: Option<ResourceId>,
    pub building: Option<Building>,
}

/// A v1 or v2 building, copied onto every tile of its footprint.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Deserialize)]
pub(crate) struct Building {
    pub id: BuildingId,
    pub passable: bool,
    /// Saves from before rail (#135) have none.
    #[serde(default)]
    pub rail: bool,
    /// This tile's offset from the anchor, `[x, y]`; absent on the anchor itself.
    #[serde(default)]
    pub part: [u8; 2],
    /// The footprint, `[w, h]`; absent when 1x1.
    #[serde(default = "one_by_one")]
    pub size: [u8; 2],
}

impl Building {
    /// Is this the anchor tile (the top-left one) of its building?
    pub fn is_anchor(&self) -> bool {
        self.part == [0, 0]
    }
}
