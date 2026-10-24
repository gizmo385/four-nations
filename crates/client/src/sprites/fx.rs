//! Bending's lasting effects on the map (#154): the cells of `art/fx.toml`. Drawn at 1:1 over the
//! floor (docs/design/pixel-art.md). The sheet is three cells wide, one family per row:
//!
//! | Row | Cells | Meaning |
//! |---|---|---|
//! | 0 | `flame_0` to `flame_2` | a lit Hearthflame, cycled by [`flame_cell`] |
//! | 1 | `wet_0` to `wet_2` | a watered tile, picked by [`wet_cell`] |
//!
//! Cast animations (#166) add rows below, with their own names and a constant here. A test reads
//! `art/fx.toml` and checks each constant against the sprite name it claims.

use fn_sim::Pos;

use super::{Cell, one, tile_hash};

/// The sheet the effects are on (`assets/original/fx.png`, drawn by `cargo xtask art`).
pub const SHEET: &str = "original/fx.png";

/// The sheet's sprites in file order: cell `n` is column `n % COLUMNS`, row `n / COLUMNS`.
#[cfg(test)]
pub const COLUMNS: u32 = 3;

/// The frames of a lit flame, drawn on the flame's tile.
pub const FLAME: [Cell; 3] = [one(SHEET, 0, 0), one(SHEET, 1, 0), one(SHEET, 2, 0)];
/// The puddle variants of a wet tile.
pub const WET: [Cell; 3] = [one(SHEET, 0, 1), one(SHEET, 1, 1), one(SHEET, 2, 1)];

/// World ticks (`TICKS_PER_SECOND` a second) each flame frame shows.
pub const FLAME_FRAME_TICKS: u64 = 3;
/// The order the flame frames play in: the flame rises and falls back, so the loop has no jump.
const FLAME_SEQUENCE: [usize; 4] = [0, 1, 2, 1];

/// The flame frame to show at world tick `tick`. The session clock drives it, not wall time, so a
/// paused game holds a still flame. Each tile starts at a different point, so a row of flames
/// doesn't flicker in step.
pub fn flame_cell(tick: u64, p: Pos) -> Cell {
    let phase = tile_hash(p, 0xF1A3) as u64;
    let step = (tick / FLAME_FRAME_TICKS + phase) as usize % FLAME_SEQUENCE.len();
    FLAME[FLAME_SEQUENCE[step]]
}

/// The puddle sprite of the wet tile at `p`: a stable pick, so neighbours differ and a tile never
/// changes under the player's eye.
pub fn wet_cell(p: Pos) -> Cell {
    WET[tile_hash(p, 0x57E7) as usize % WET.len()]
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The names of `art/fx.toml`'s sprites in file order.
    fn names() -> Vec<String> {
        let path = format!("{}/../../art/fx.toml", env!("CARGO_MANIFEST_DIR"));
        let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{path}: {e}"));
        let doc: toml::Table = text.parse().unwrap();
        let sprites = doc["sprite"].as_array().unwrap();
        assert_eq!(doc["columns"].as_integer(), Some(i64::from(COLUMNS)));
        sprites.iter().map(|s| s["name"].as_str().unwrap().to_owned()).collect()
    }

    #[test]
    fn the_cell_constants_are_the_sprites_they_claim() {
        let names = names();
        let name_at = |c: Cell| names[(c.row * COLUMNS + c.col) as usize].as_str();
        for (i, c) in FLAME.iter().enumerate() {
            assert_eq!(name_at(*c), format!("flame_{i}"));
        }
        for (i, c) in WET.iter().enumerate() {
            assert_eq!(name_at(*c), format!("wet_{i}"));
        }
        assert_eq!(names.len(), FLAME.len() + WET.len(), "a sprite no constant points at");
    }

    #[test]
    fn the_sheet_is_as_wide_as_its_columns() {
        let (w, _) = super::super::test_support::png_size(SHEET);
        assert_eq!(w, COLUMNS * super::super::CELL as u32);
    }

    #[test]
    fn flames_cycle_through_every_frame_on_the_world_clock() {
        let p = Pos::new(3, 4);
        let shown: std::collections::BTreeSet<_> = (0..40).map(|t| flame_cell(t, p).col).collect();
        assert_eq!(shown.len(), FLAME.len(), "{shown:?}");
        // A frame lasts several ticks, and the same tick always gives the same frame.
        assert_eq!(flame_cell(0, p), flame_cell(FLAME_FRAME_TICKS - 1, p));
        assert_eq!(flame_cell(7, p), flame_cell(7, p));
    }

    #[test]
    fn wet_tiles_use_every_puddle_variant() {
        let shown: std::collections::BTreeSet<_> =
            (0..8).flat_map(|x| (0..8).map(move |y| wet_cell(Pos::new(x, y)).col)).collect();
        assert_eq!(shown.len(), WET.len());
    }
}
