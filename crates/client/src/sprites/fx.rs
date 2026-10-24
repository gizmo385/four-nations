//! Bending's effects on the map: the cells of `art/fx.toml`. Drawn at 1:1 over the floor
//! (docs/design/pixel-art.md). The sheet is three cells wide, one family per row:
//!
//! | Row | Cells | Meaning |
//! |---|---|---|
//! | 0 | `flame_0` to `flame_2` | a lit Hearthflame, cycled by [`flame_cell`] (#154) |
//! | 1 | `wet_0` to `wet_2` | a watered tile, picked by [`wet_cell`] (#154) |
//! | 2 | `earth_0` to `earth_2` | Raise Earth's cast: cracks, spikes, stones (#166) |
//! | 3 | `dust_0` to `dust_2` | the dust that settles after it |
//! | 4 | `drop_0` to `drop_2` | Water the Land's stream: one droplet, centred |
//! | 5 | `splash_0` to `splash_2` | the stream landing: ring, crown, ripple |
//! | 6 | `spark_0` to `spark_2` | Hearthflame's cast: sparks, flare, embers |
//! | 7 | `gust_0` to `gust_2` | Wind Carry's gust: a curl of wind, turning |
//! | 8 | `swirl_0` to `swirl_2` | the whirl that lifts the stack |
//!
//! The cast rows (2 to 8, #166) are played by `cast.rs`. A test reads `art/fx.toml` and checks
//! each constant against the sprite name it claims.

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

/// Raise Earth's cast: cracks open, rock spikes rise, stepping stones stand.
pub const EARTH: [Cell; 3] = [one(SHEET, 0, 2), one(SHEET, 1, 2), one(SHEET, 2, 2)];
/// The dust of the stones settling (dense to thin).
pub const DUST: [Cell; 3] = [one(SHEET, 0, 3), one(SHEET, 1, 3), one(SHEET, 2, 3)];
/// A droplet of Water the Land's stream (big, small, spray).
pub const DROP: [Cell; 3] = [one(SHEET, 0, 4), one(SHEET, 1, 4), one(SHEET, 2, 4)];
/// The stream landing on a tile: ring, crown, settling ripple.
pub const SPLASH: [Cell; 3] = [one(SHEET, 0, 5), one(SHEET, 1, 5), one(SHEET, 2, 5)];
/// Hearthflame kindling: sparks gather, a flare, embers rise.
pub const SPARK: [Cell; 3] = [one(SHEET, 0, 6), one(SHEET, 1, 6), one(SHEET, 2, 6)];
/// A curl of Wind Carry's gust, three turns of the spiral.
pub const GUST: [Cell; 3] = [one(SHEET, 0, 7), one(SHEET, 1, 7), one(SHEET, 2, 7)];
/// The whirl that lifts the stack, three turns.
pub const SWIRL: [Cell; 3] = [one(SHEET, 0, 8), one(SHEET, 1, 8), one(SHEET, 2, 8)];

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
        let cast = [
            ("earth", EARTH),
            ("dust", DUST),
            ("drop", DROP),
            ("splash", SPLASH),
            ("spark", SPARK),
            ("gust", GUST),
            ("swirl", SWIRL),
        ];
        for (family, cells) in cast {
            for (i, c) in cells.iter().enumerate() {
                assert_eq!(name_at(*c), format!("{family}_{i}"));
            }
        }
        assert_eq!(names.len(), FLAME.len() + WET.len() + 7 * 3, "a sprite no constant points at");
    }

    #[test]
    fn the_sheet_is_as_wide_as_its_columns_and_has_nine_rows() {
        let (w, h) = super::super::test_support::png_size(SHEET);
        assert_eq!(w, COLUMNS * super::super::CELL as u32);
        assert_eq!(h, 9 * super::super::CELL as u32);
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
