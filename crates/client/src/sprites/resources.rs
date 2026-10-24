//! Resources that aren't trees: one-cell sprites, bottom-aligned to their tile.

use super::{ResourceRule, one};

/// The sheet the rocks and crops are on.
pub const SHEET: &str = "third_party/ninja-adventure/Backgrounds/Tilesets/TilesetNature.png";

/// The sheet the fish shadows are on.
pub const WATER_SHEET: &str = "third_party/ninja-adventure/Backgrounds/Tilesets/TilesetWater.png";

pub const RESOURCES: &[ResourceRule] = &[
    // Grey rock, and the same rock studded with orange ore.
    ResourceRule { resource: "stone", biome: None, cell: one(SHEET, 6, 14) },
    ResourceRule { resource: "gold_vein", biome: None, cell: one(SHEET, 4, 14) },
    // The pack has no cabbage; a head of leafy greens stands in.
    ResourceRule { resource: "cabbage", biome: None, cell: one(SHEET, 8, 11) },
    // Fish shadows on water, drawn over the whole tile.
    ResourceRule { resource: "fish_spot", biome: None, cell: one(WATER_SHEET, 11, 4) },
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_rest_are_one_cell() {
        for r in RESOURCES {
            assert!(!r.resource.ends_with("_tree"), "{}", r.resource);
            assert_eq!((r.cell.w, r.cell.h), (1, 1), "{}", r.resource);
        }
    }
}
