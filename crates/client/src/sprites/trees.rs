//! Trees: the 2x2 sprites, bottom-aligned to their tile. Claude-authored (`art/trees.toml`,
//! docs/design/pixel-art.md), rendered by `cargo xtask art` to `assets/original/trees.png`.
//!
//! The sheet has two rows of four 32x32 trees. Row 0 is the whole tree: canopy over a trunk in
//! the bottom-centre 16px, which is the tile the tree stands on. Row 1 is the same canopy with the
//! trunk removed (`canopy_of`). A trunk and its canopy cover disjoint pixels, so the fade of #77
//! (`canopy.rs`) draws them as two sprites: `trunks_of_sheet` blanks the canopy out of row 0 at
//! load time to get the trunk alone, and the canopy sprite on top of it is the part that fades.

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
/// footprint, so it can be drawn over the trunk at the same transform and faded on its own.
pub const fn canopy_of(tree: Cell) -> Cell {
    Cell { row: tree.row + CANOPY_ROW, ..tree }
}

/// Whether a resource's cell is one of the trees above (the resources with a canopy to fade).
pub fn is_tree(cell: Cell) -> bool {
    cell.sheet == SHEET
}

/// Where the canopy pixels are on a tree sprite, in pixels from the middle of its bottom edge
/// (x right, y up): (min x, min y, max x, max y). Every tree's canopy lies inside this box; it is
/// what hides a unit, so the fade doesn't count the transparent corners of the 32x32 sprite. A
/// test checks it against the art.
pub const CANOPY_EXTENT: [f32; 4] = [-15.0, 5.0, 15.0, 30.0];

/// The trunks alone, from the RGBA pixels of the tree sheet (`width` pixels wide): the sheet with
/// every pixel of row 0 blanked wherever the canopy copy, `CANOPY_ROW` cells below it, has one,
/// and everything below row 0 transparent. In the art the two are disjoint (a pixel is canopy or
/// trunk), so the trunk sprite with the canopy sprite over it is the whole tree.
pub fn trunks_of_sheet(rgba: &[u8], width: u32) -> Vec<u8> {
    let canopy = (CANOPY_ROW * super::CELL as u32 * width) as usize * 4;
    let mut out = vec![0; rgba.len()];
    for i in 0..canopy / 4 {
        let covered = rgba.get(canopy + i * 4 + 3).is_some_and(|&alpha| alpha != 0);
        if !covered {
            out[i * 4..i * 4 + 4].copy_from_slice(&rgba[i * 4..i * 4 + 4]);
        }
    }
    out
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

    /// The pixel columns and rows of a sprite's non-empty pixels, as (min x, min y, max x, max y)
    /// of the pixels themselves.
    fn pixel_box(grid: &[String]) -> (usize, usize, usize, usize) {
        let filled = |(_, c): (usize, char)| c != '.';
        let cols: Vec<usize> = grid
            .iter()
            .flat_map(|r| r.chars().enumerate().filter(|&p| filled(p)))
            .map(|p| p.0)
            .collect();
        let rows: Vec<usize> =
            (0..grid.len()).filter(|&y| grid[y].contains(|c| c != '.')).collect();
        (
            cols.iter().copied().min().unwrap(),
            rows[0],
            *cols.iter().max().unwrap(),
            rows[rows.len() - 1],
        )
    }

    /// The fade's idea of where a canopy is holds every tree's canopy, and is no looser than a
    /// pixel around all of them together (or a unit would be "under" empty air).
    #[test]
    fn the_canopy_extent_is_the_art() {
        let [x0, y0, x1, y1] = CANOPY_EXTENT;
        // The union of the canopies' boxes, in pixels from the middle of the bottom edge.
        let mut union = [f32::MAX, f32::MAX, f32::MIN, f32::MIN];
        for (name, grid) in sprites().iter().filter(|(n, _)| n.ends_with("_canopy")) {
            let (min_x, min_y, max_x, max_y) = pixel_box(grid);
            let (left, right) = (min_x as f32 - 16.0, max_x as f32 + 1.0 - 16.0);
            let (bottom, top) = (32.0 - (max_y as f32 + 1.0), 32.0 - min_y as f32);
            assert!(x0 <= left && right <= x1 && y0 <= bottom && top <= y1, "{name}");
            union =
                [union[0].min(left), union[1].min(bottom), union[2].max(right), union[3].max(top)];
        }
        assert!(union[0] - x0 <= 1.0 && x1 - union[2] <= 1.0, "extent too wide: {union:?}");
        assert!(union[1] - y0 <= 1.0 && y1 - union[3] <= 1.0, "extent too tall: {union:?}");
    }

    /// Blanking the canopy out of the whole trees leaves exactly the trunk pixels, and so the
    /// trunk and the canopy together are the whole tree.
    #[test]
    fn the_trunks_are_the_trees_minus_the_canopy() {
        let all = sprites();
        let rgba: Vec<u8> = (0..64)
            .flat_map(|y| (0..128).map(move |x| (x, y)))
            .flat_map(|(x, y)| {
                let grid = &all[(y / 32) * 4 + x / 32].1;
                match grid[y % 32].as_bytes()[x % 32] {
                    b'.' => [0, 0, 0, 0],
                    c => [c, 1, 2, 255],
                }
            })
            .collect();
        let trunks = trunks_of_sheet(&rgba, 128);
        assert_eq!(trunks.len(), rgba.len());
        for (i, tree) in all.iter().take(4).enumerate() {
            for y in 0..32 {
                for x in 0..32 {
                    let at = ((y * 128) + i * 32 + x) * 4;
                    let is_trunk = matches!(tree.1[y].as_bytes()[x], b'6'..=b'9');
                    assert_eq!(trunks[at + 3] != 0, is_trunk, "{} at {x},{y}", tree.0);
                    if is_trunk {
                        assert_eq!(trunks[at..at + 4], rgba[at..at + 4]);
                    }
                }
            }
        }
        // Row 1 (the canopy copies) is left transparent.
        assert!(trunks[128 * 32 * 4..].iter().all(|&b| b == 0));
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
