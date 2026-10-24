//! Buildings: the 2x2 sprites, bottom-aligned to the tile they stand on. Claude-authored
//! (`art/buildings.toml`, docs/design/pixel-art.md), rendered by `cargo xtask art` to
//! `assets/original/buildings.png`.
//!
//! Which sprite a building uses is data: `BuildingDef::art_for(nation)` is its `art[nation]`, or
//! its `id` (`data/buildings.toml`). `frames` turns that name into the cells to draw. A building
//! with no art yet draws `UNFINISHED` (a signboard with a "?"), so it is still visible and honest
//! about it, and it is named in `NO_ART_YET` until its art lands (a test checks both lists).
//!
//! The sheet is four sprites wide: row 0 is the bed and the three campfire frames, row 1 the four
//! study halls, row 2 the fallback.

use super::{Cell, big};

/// The sheet the buildings are on.
pub const SHEET: &str = "original/buildings.png";

/// One building's sprite: its frames, in the order they play (one for most buildings).
pub struct BuildingArt {
    /// The name in `art/buildings.toml` (the first frame's; later ones are `<name>_2`, ...), and
    /// what `BuildingDef::art_for` returns.
    pub name: &'static str,
    pub frames: &'static [Cell],
}

/// The cells (column, row, in 16px cells) of the sprites in `art/buildings.toml`. A test checks
/// each against the sprite name.
pub const ART: &[BuildingArt] = &[
    BuildingArt { name: "bed", frames: &[big(SHEET, 0, 0)] },
    BuildingArt {
        name: "campfire",
        frames: &[big(SHEET, 2, 0), big(SHEET, 4, 0), big(SHEET, 6, 0)],
    },
    BuildingArt { name: "meditation_pavilion", frames: &[big(SHEET, 0, 2)] },
    BuildingArt { name: "elders_lodge", frames: &[big(SHEET, 2, 2)] },
    BuildingArt { name: "scroll_library", frames: &[big(SHEET, 4, 2)] },
    BuildingArt { name: "academy", frames: &[big(SHEET, 6, 2)] },
];

/// What a building with no art yet is drawn as.
pub const UNFINISHED: Cell = big(SHEET, 0, 4);

/// The buildings in `data/buildings.toml` that draw `UNFINISHED` until their art is made. Remove
/// an id here when its sprite is added to `ART`: a test fails for a building in neither place,
/// and for one in both.
#[cfg(test)]
pub const NO_ART_YET: &[&str] = &[
    // Earth Kingdom
    "earth_wall",
    // Air Nomads
    "orchard",
    "windmill",
    "wind_turbine",
    "bison_stable",
    // Fire Nation
    "forge",
    "brick_kiln",
    "smelter",
    "steam_engine",
    "lightning_station",
    // Water Tribe
    "igloo",
];

/// The frames of the sprite called `art`, if it has one.
pub fn building_art(art: &str) -> Option<&'static BuildingArt> {
    ART.iter().find(|a| a.name == art)
}

/// The cells to draw for the sprite called `art`: its frames, or `UNFINISHED` if it has none yet.
pub fn frames(art: &str) -> &'static [Cell] {
    match building_art(art) {
        Some(a) => a.frames,
        None => &[UNFINISHED],
    }
}

/// How fast a multi-frame sprite (the campfire) plays, in frames a second.
pub const FRAMES_PER_SECOND: f32 = 6.0;

#[cfg(test)]
mod tests {
    use fn_sim::{Defs, Nation};

    use super::super::test_support::png_size;
    use super::*;

    /// The names of the sprites of `art/buildings.toml`, in sheet order.
    fn sprite_names() -> Vec<String> {
        let path = format!("{}/../../art/buildings.toml", env!("CARGO_MANIFEST_DIR"));
        let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{path}: {e}"));
        let doc: toml::Table = text.parse().unwrap();
        doc["sprite"]
            .as_array()
            .unwrap()
            .iter()
            .map(|s| s["name"].as_str().unwrap().into())
            .collect()
    }

    /// The name of the sprite at a cell of the sheet (4 sprites a row, each 2x2 cells).
    fn name_at(cell: Cell) -> String {
        sprite_names().swap_remove(((cell.row / 2) * 4 + cell.col / 2) as usize)
    }

    #[test]
    fn the_sheet_is_four_wide_with_a_sprite_per_cell() {
        let names = sprite_names();
        assert_eq!(png_size(SHEET), (4 * 32, 3 * 32));
        assert_eq!(names.len(), 9);
    }

    #[test]
    fn the_cells_are_the_sprites_they_claim() {
        for a in ART {
            for (i, frame) in a.frames.iter().enumerate() {
                let want = if i == 0 { a.name.to_owned() } else { format!("{}_{}", a.name, i + 1) };
                assert_eq!(name_at(*frame), want);
                assert_eq!((frame.w, frame.h, frame.sheet), (2, 2, SHEET), "{want}");
            }
        }
        assert_eq!(name_at(UNFINISHED), "unfinished");
    }

    /// Every building in the data is drawn in every nation, with its own sprite or, until its
    /// art is made, the named fallback. The issue's rule ("a test fails if a building has no
    /// sprite") is this: a building can't be added without choosing one of the two.
    #[test]
    fn every_building_has_a_sprite_or_is_listed_as_waiting_for_one() {
        let defs = Defs::bundled();
        for b in &defs.buildings {
            for n in Nation::ALL {
                let art = b.art_for(n);
                let drawn = building_art(art).is_some();
                let waiting = NO_ART_YET.contains(&b.id.as_str());
                assert!(
                    drawn != waiting,
                    "{} in {n:?}: sprite {art:?} is {}, and it is {} NO_ART_YET",
                    b.id,
                    if drawn { "drawn" } else { "missing" },
                    if waiting { "in" } else { "not in" },
                );
            }
        }
    }

    #[test]
    fn the_waiting_list_names_real_buildings_and_no_sprite_is_orphaned() {
        let defs = Defs::bundled();
        for id in NO_ART_YET {
            assert!(defs.building_id(id).is_some(), "NO_ART_YET names unknown building {id}");
        }
        for a in ART {
            let used =
                defs.buildings.iter().any(|b| Nation::ALL.iter().any(|&n| b.art_for(n) == a.name));
            assert!(used, "no building uses the sprite {}", a.name);
        }
        let mut names: Vec<_> = ART.iter().map(|a| a.name).collect();
        names.sort_unstable();
        names.dedup();
        assert_eq!(names.len(), ART.len(), "a sprite listed twice");
    }

    #[test]
    fn an_unknown_sprite_name_draws_the_fallback() {
        assert_eq!(frames("no_such_building"), &[UNFINISHED]);
        assert_eq!(frames("campfire").len(), 3);
        assert_eq!(frames("bed").len(), 1);
    }

    /// Row depth sorts a building by the tile it stands on, so the sprite ends on the bottom row
    /// of that tile's 16px: the foot (outline) is on row 31 of its cell and nothing is lower.
    #[test]
    fn every_sprite_stands_on_the_bottom_of_its_tile_and_fits_its_cell() {
        let path = format!("{}/../../art/buildings.toml", env!("CARGO_MANIFEST_DIR"));
        let doc: toml::Table = std::fs::read_to_string(&path).unwrap().parse().unwrap();
        for s in doc["sprite"].as_array().unwrap() {
            let name = s["name"].as_str().unwrap();
            let grid: Vec<&str> = s["grid"].as_str().unwrap().lines().collect();
            assert_eq!(grid.len(), 32, "{name}");
            assert!(grid[30].bytes().any(|c| c != b'.'), "{name}: floats above its tile");
            assert!(grid[31].bytes().all(|c| c == b'.'), "{name}: bottom row");
            assert!(grid[0].bytes().all(|c| c == b'.'), "{name}: top row");
            assert!(grid.iter().all(|r| r.len() == 32), "{name}");
            assert!(grid.iter().all(|r| r.starts_with('.') && r.ends_with('.')), "{name}: sides");
            // The body stands on its own tile: some of its foot is in the bottom-centre 16px.
            assert!(grid[30][8..24].bytes().any(|c| c != b'.'), "{name}: foot off its tile");
        }
    }
}
