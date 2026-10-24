//! Trees: the 2x2 sprites, bottom-aligned to their tile.

use super::{ResourceRule, big};

/// The sheet the trees are on.
pub const SHEET: &str = "third_party/ninja-adventure/Backgrounds/Tilesets/TilesetNature.png";

pub const RESOURCES: &[ResourceRule] = &[
    ResourceRule { resource: "oak_tree", biome: None, cell: big(SHEET, 0, 0) },
    ResourceRule { resource: "fir_tree", biome: Some("tundra"), cell: big(SHEET, 8, 0) },
    ResourceRule { resource: "fir_tree", biome: None, cell: big(SHEET, 2, 0) },
    // A cherry blossom stands in for a fruit tree: no tree in the pack is drawn with fruit.
    ResourceRule { resource: "fruit_tree", biome: None, cell: big(SHEET, 14, 0) },
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn trees_are_two_by_two() {
        for r in RESOURCES {
            assert!(r.resource.ends_with("_tree"), "{}", r.resource);
            assert_eq!((r.cell.w, r.cell.h), (2, 2), "{}", r.resource);
        }
    }
}
