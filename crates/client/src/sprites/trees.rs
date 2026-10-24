//! Trees: the 2x2 sprites, bottom-aligned to their tile. Claude-authored (`art/trees.toml`,
//! docs/design/pixel-art.md), rendered by `cargo xtask art` to `assets/original/trees.png`.
//!
//! The sheet has two rows of four 32x32 trees. Row 0 is the whole tree: canopy over a trunk in
//! the bottom-centre 16px, which is the tile the tree stands on. Row 1 is the same canopy with the
//! trunk removed, so the fade of #77 can draw the trunk at full strength and fade only the canopy
//! over it (`canopy_of`).

use super::{Cell, ResourceRule, big};

/// The sheet the trees are on.
pub const SHEET: &str = "original/trees.png";

/// The cells (column, row, in 16px cells) of the sprites in `art/trees.toml`. A test checks each
/// against the sprite name.
const OAK: u32 = 0;
const FIR: u32 = 2;
const FIR_SNOW: u32 = 4;
const FRUIT: u32 = 6;

/// The row of the canopy-only copies: the whole trees are in row 0.
const CANOPY_ROW: u32 = 2;

pub const RESOURCES: &[ResourceRule] = &[
    ResourceRule { resource: "oak_tree", biome: None, cell: big(SHEET, OAK, 0) },
    ResourceRule { resource: "fir_tree", biome: Some("tundra"), cell: big(SHEET, FIR_SNOW, 0) },
    ResourceRule { resource: "fir_tree", biome: None, cell: big(SHEET, FIR, 0) },
    ResourceRule { resource: "fruit_tree", biome: None, cell: big(SHEET, FRUIT, 0) },
];

/// The canopy-only copy of a whole tree's cell: the same tree with its trunk removed, on the same
/// footprint, so it can be drawn over the whole tree at the same transform. (For #77; nothing
/// draws it yet.)
#[allow(dead_code)]
pub const fn canopy_of(tree: Cell) -> Cell {
    Cell { row: tree.row + CANOPY_ROW, ..tree }
}

#[cfg(test)]
mod tests {
    use super::super::test_support::png_size;
    use super::*;

    /// A sprite of `art/trees.toml`: its name and its 32 rows of 32 characters.
    fn sprites() -> Vec<(String, Vec<String>)> {
        let path = format!("{}/../../art/trees.toml", env!("CARGO_MANIFEST_DIR"));
        let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{path}: {e}"));
        let doc: toml::Table = text.parse().unwrap();
        doc["sprite"]
            .as_array()
            .unwrap()
            .iter()
            .map(|s| {
                let name = s["name"].as_str().unwrap().to_owned();
                let grid = s["grid"].as_str().unwrap().lines().map(str::to_owned).collect();
                (name, grid)
            })
            .collect()
    }

    /// The sprite at a cell of the sheet (a 32px sprite is two 16px cells wide and tall).
    fn at(cell: Cell) -> (String, Vec<String>) {
        let all = sprites();
        let columns = 4;
        let index = (cell.row / 2) * columns + cell.col / 2;
        all.into_iter().nth(index as usize).unwrap()
    }

    #[test]
    fn trees_are_two_by_two() {
        for r in RESOURCES {
            assert!(r.resource.ends_with("_tree"), "{}", r.resource);
            assert_eq!((r.cell.w, r.cell.h), (2, 2), "{}", r.resource);
            assert_eq!(r.cell.sheet, SHEET);
        }
    }

    #[test]
    fn the_sheet_is_two_rows_of_four_trees() {
        assert_eq!(png_size(SHEET), (4 * 32, 2 * 32));
        assert_eq!(sprites().len(), 8);
    }

    #[test]
    fn the_cells_are_the_sprites_they_claim() {
        for (col, name) in
            [(OAK, "oak"), (FIR, "fir"), (FIR_SNOW, "fir_snow"), (FRUIT, "fruit_tree")]
        {
            assert_eq!(at(big(SHEET, col, 0)).0, name);
            assert_eq!(at(canopy_of(big(SHEET, col, 0))).0, format!("{name}_canopy"));
        }
        // The tundra fir is the snowy one; everywhere else is the plain fir.
        let fir = |biome| super::super::resource_rule("fir_tree", biome).unwrap().cell;
        assert_eq!(at(fir("tundra")).0, "fir_snow");
        assert_eq!(at(fir("taiga")).0, "fir");
    }

    #[test]
    fn every_tree_has_a_trunk_in_the_bottom_centre_tile() {
        for (name, grid) in sprites().iter().filter(|(n, _)| !n.ends_with("_canopy")) {
            assert_eq!(grid.len(), 32, "{name}");
            let mut trunk = 0;
            for (y, row) in grid.iter().enumerate() {
                assert_eq!(row.len(), 32, "{name}");
                for (x, ch) in row.bytes().enumerate() {
                    if matches!(ch, b'6'..=b'9') {
                        trunk += 1;
                        // Inside the tile the tree stands on: x 8..24, y 16..32.
                        assert!((8..24).contains(&x) && y >= 16, "{name}: trunk at {x},{y}");
                    }
                }
            }
            assert!(trunk >= 12, "{name}: only {trunk} trunk pixels");
            // The foot stands on the bottom of the tile, so a tree on a row never floats.
            assert!(grid[30].bytes().any(|c| matches!(c, b'6'..=b'9')), "{name}: no foot");
        }
    }

    /// Row 1 of the sheet is row 0 minus the trunk, so a fade of the canopy never leaves a ghost
    /// of the trunk or loses a leaf.
    #[test]
    fn the_canopy_is_the_tree_without_its_trunk() {
        let all = sprites();
        for i in 0..4 {
            let (tree, canopy) = (&all[i], &all[i + 4]);
            assert_eq!(canopy.0, format!("{}_canopy", tree.0));
            let blanked: Vec<String> = tree
                .1
                .iter()
                .map(|r| {
                    r.chars().map(|c| if ('6'..='9').contains(&c) { '.' } else { c }).collect()
                })
                .collect();
            assert_eq!(canopy.1, blanked, "{}", canopy.0);
        }
    }

    #[test]
    fn the_fruit_tree_visibly_carries_fruit() {
        let (_, grid) = &sprites()[FRUIT as usize / 2];
        let fruit =
            grid.iter().flat_map(|r| r.bytes()).filter(|c| matches!(c, b'r' | b'R')).count();
        assert!(fruit >= 12, "{fruit} fruit pixels");
        // No other tree is drawn with red.
        for (name, grid) in sprites().iter().filter(|(n, _)| !n.starts_with("fruit")) {
            assert!(!grid.iter().any(|r| r.contains(['r', 'R', 'k'])), "{name}");
        }
    }

    /// Row depth sorts a tree by the tile it stands on, so the sprite must end at the bottom of
    /// that tile: nothing below the foot, and the canopy never reaches the sprite's top edge,
    /// where its outline would be cut.
    #[test]
    fn the_sprites_stay_inside_their_cell() {
        for (name, grid) in sprites() {
            assert!(grid[0].bytes().all(|c| c == b'.'), "{name}: top row");
            assert!(grid[31].bytes().all(|c| c == b'.'), "{name}: bottom row");
            assert!(grid.iter().all(|r| r.starts_with('.') && r.ends_with('.')), "{name}: sides");
        }
    }
}
