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
//! study halls, row 2 the fallback. A nation's own buildings are on a sheet of their own
//! (`SHEET_AIR` is `art/buildings_air.toml`, `SHEET_EARTH` `art/buildings_earth.toml`), so
//! parallel art PRs don't share a file. A sheet's TOML is the `art/` file with its PNG's name.

use super::{Cell, big};

/// The sheet the buildings are on.
pub const SHEET: &str = "original/buildings.png";

/// The Air Nomad buildings' sheet (#176): orchard, windmill, wind turbine, bison stable.
pub const SHEET_AIR: &str = "original/buildings_air.png";

/// The Earth Kingdom buildings' sheet (#175): the earth wall.
pub const SHEET_EARTH: &str = "original/buildings_earth.png";

/// The Fire Nation's buildings' sheet (#177): forge, brick kiln, smelter, steam engine, lightning
/// station.
pub const SHEET_FIRE: &str = "original/buildings_fire.png";

/// The Water Tribe buildings' sheet (#178): the igloo.
pub const SHEET_WATER: &str = "original/buildings_water.png";

/// Every sheet, for the loader and the tests.
pub const SHEETS: &[&str] = &[SHEET, SHEET_AIR, SHEET_EARTH, SHEET_FIRE, SHEET_WATER];

/// One building's sprite: its frames, in the order they play (one for most buildings).
pub struct BuildingArt {
    /// The name in `art/buildings.toml` (the first frame's; later ones are `<name>_2`, ...), and
    /// what `BuildingDef::art_for` returns.
    pub name: &'static str,
    pub frames: &'static [Cell],
    /// Whether the frames are not an animation but 16 pieces of track, one for each set of
    /// neighbours it joins (`link_frame`), as the rail's are (#135).
    pub connects: bool,
}

/// The cells (column, row, in 16px cells) of the sprites in `art/buildings.toml`. A test checks
/// each against the sprite name.
pub const ART: &[BuildingArt] = &[
    BuildingArt { name: "bed", frames: &[big(SHEET, 0, 0)], connects: false },
    BuildingArt {
        name: "campfire",
        frames: &[big(SHEET, 2, 0), big(SHEET, 4, 0), big(SHEET, 6, 0)],
        connects: false,
    },
    BuildingArt { name: "meditation_pavilion", frames: &[big(SHEET, 0, 2)], connects: false },
    BuildingArt { name: "elders_lodge", frames: &[big(SHEET, 2, 2)], connects: false },
    BuildingArt { name: "scroll_library", frames: &[big(SHEET, 4, 2)], connects: false },
    BuildingArt { name: "academy", frames: &[big(SHEET, 6, 2)], connects: false },
    // Air Nomads (`SHEET_AIR`)
    BuildingArt { name: "orchard", frames: &[big(SHEET_AIR, 0, 0)], connects: false },
    BuildingArt {
        name: "windmill",
        frames: &[big(SHEET_AIR, 2, 0), big(SHEET_AIR, 4, 0), big(SHEET_AIR, 6, 0)],
        connects: false,
    },
    BuildingArt {
        name: "wind_turbine",
        frames: &[big(SHEET_AIR, 0, 2), big(SHEET_AIR, 2, 2), big(SHEET_AIR, 4, 2)],
        connects: false,
    },
    BuildingArt { name: "bison_stable", frames: &[big(SHEET_AIR, 6, 2)], connects: false },
    // Earth Kingdom (`SHEET_EARTH`)
    BuildingArt { name: "earth_wall", frames: &[big(SHEET_EARTH, 0, 0)], connects: false },
    BuildingArt { name: "rail", frames: &RAIL_FRAMES, connects: true },
    // Fire Nation (`SHEET_FIRE`)
    BuildingArt {
        name: "forge",
        frames: &[big(SHEET_FIRE, 0, 0), big(SHEET_FIRE, 2, 0), big(SHEET_FIRE, 4, 0)],
        connects: false,
    },
    BuildingArt {
        name: "brick_kiln",
        frames: &[big(SHEET_FIRE, 6, 0), big(SHEET_FIRE, 0, 2), big(SHEET_FIRE, 2, 2)],
        connects: false,
    },
    BuildingArt {
        name: "smelter",
        frames: &[big(SHEET_FIRE, 4, 2), big(SHEET_FIRE, 6, 2), big(SHEET_FIRE, 0, 4)],
        connects: false,
    },
    BuildingArt {
        name: "steam_engine",
        frames: &[big(SHEET_FIRE, 2, 4), big(SHEET_FIRE, 4, 4), big(SHEET_FIRE, 6, 4)],
        connects: false,
    },
    BuildingArt {
        name: "lightning_station",
        frames: &[big(SHEET_FIRE, 0, 6), big(SHEET_FIRE, 2, 6)],
        connects: false,
    },
    // Water Tribes (`SHEET_WATER`)
    BuildingArt { name: "igloo", frames: &[big(SHEET_WATER, 0, 0)], connects: false },
];

/// The sixteen pieces of track (#135), on `SHEET_EARTH` after the wall: sprite `1 + mask`, where
/// the mask has `LINK_N`, `LINK_E`, `LINK_S`, `LINK_W` for each neighbour the tile joins. The
/// sheet is four wide, so piece `n` is at column `n % 4 * 2`, row `n / 4 * 2`. The names in the
/// TOML are `rail`, `rail_2`, ... `rail_16`, which is what the `frames` naming test expects.
const RAIL_FRAMES: [Cell; 16] = {
    let mut frames = [big(SHEET_EARTH, 0, 0); 16];
    let mut mask = 0;
    while mask < 16 {
        let n = mask as u32 + 1;
        frames[mask] = big(SHEET_EARTH, n % 4 * 2, n / 4 * 2);
        mask += 1;
    }
    frames
};

/// The bits of a link mask: which of a tile's four neighbours it is joined to.
pub const LINK_N: u8 = 1;
pub const LINK_E: u8 = 2;
pub const LINK_S: u8 = 4;
pub const LINK_W: u8 = 8;

/// What a building with no art yet is drawn as.
pub const UNFINISHED: Cell = big(SHEET, 0, 4);

/// The buildings in `data/buildings.toml` that draw `UNFINISHED` until their art is made. Remove
/// an id here when its sprite is added to `ART`: a test fails for a building in neither place,
/// and for one in both.
#[cfg(test)]
pub const NO_ART_YET: &[&str] = &[
    // Earth Kingdom
    // Air Nomads
    // Fire Nation
    // Water Tribe
    // The endeavor: the Harmony Gate's art is `buildings_endeavor.png`, drawn by stage, which
    // the client issue after #236 wires up (#243).
    "harmony_gate",
    // Farm plots (#198): no client art, since the client is being removed (#283).
    "farm_plot",
];

/// Buildings whose art is drawn for a bigger footprint than the data gives them. A test fails for
/// a building listed here whose art fits its footprint. #214 turned the 2x2 sizes on and emptied
/// it (docs/design/buildings.md, "Footprint sizes").
#[cfg(test)]
pub const AWAITING_FOOTPRINT: &[&str] = &[];

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

/// Whether the sprite called `art` is a set of connecting pieces (the rail) rather than an
/// animation; if so its frame is `link_frame`.
pub fn connects(art: &str) -> bool {
    building_art(art).is_some_and(|a| a.connects)
}

/// The piece of a connecting sprite that joins the neighbours in `links` (`LINK_N | LINK_E ...`).
pub fn link_frame(frames: &'static [Cell], links: u8) -> Cell {
    frames[usize::from(links & 0b1111) % frames.len()]
}

/// How fast a multi-frame sprite (the campfire, the windmill, the turbine, the furnaces) plays, in frames a second.
pub const FRAMES_PER_SECOND: f32 = 6.0;

#[cfg(test)]
mod tests {
    use fn_sim::{Defs, Nation};

    use super::super::test_support::png_size;
    use super::*;

    /// The `art/` TOML of a sheet's PNG: `original/buildings_air.png` is `art/buildings_air.toml`.
    fn sheet_doc(sheet: &str) -> toml::Table {
        let stem = sheet.strip_prefix("original/").and_then(|s| s.strip_suffix(".png")).unwrap();
        let path = format!("{}/../../art/{stem}.toml", env!("CARGO_MANIFEST_DIR"));
        let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{path}: {e}"));
        text.parse().unwrap()
    }

    /// The names of the sprites of a sheet's TOML, in sheet order.
    fn sprite_names(sheet: &str) -> Vec<String> {
        sheet_doc(sheet)["sprite"]
            .as_array()
            .unwrap()
            .iter()
            .map(|s| s["name"].as_str().unwrap().into())
            .collect()
    }

    /// The name of the sprite at a cell of its sheet (4 sprites a row, each 2x2 cells).
    fn name_at(cell: Cell) -> String {
        sprite_names(cell.sheet).swap_remove(((cell.row / 2) * 4 + cell.col / 2) as usize)
    }

    #[test]
    fn each_sheet_is_four_wide_with_a_sprite_per_cell() {
        assert_eq!(png_size(SHEET), (4 * 32, 3 * 32));
        assert_eq!(sprite_names(SHEET).len(), 9);
        assert_eq!(png_size(SHEET_AIR), (4 * 32, 2 * 32));
        assert_eq!(sprite_names(SHEET_AIR).len(), 8);
        assert_eq!(png_size(SHEET_EARTH), (4 * 32, 5 * 32));
        assert_eq!(sprite_names(SHEET_EARTH).len(), 17);
        assert_eq!(png_size(SHEET_FIRE), (4 * 32, 4 * 32));
        assert_eq!(sprite_names(SHEET_FIRE).len(), 14);
        assert_eq!(png_size(SHEET_WATER), (4 * 32, 32));
        assert_eq!(sprite_names(SHEET_WATER).len(), 1);
    }

    #[test]
    fn the_cells_are_the_sprites_they_claim() {
        for a in ART {
            for (i, frame) in a.frames.iter().enumerate() {
                let want = if i == 0 { a.name.to_owned() } else { format!("{}_{}", a.name, i + 1) };
                assert_eq!(name_at(*frame), want);
                assert_eq!((frame.w, frame.h), (2, 2), "{want}");
                assert!(SHEETS.contains(&frame.sheet), "{want}: sheet {}", frame.sheet);
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

    /// The width in pixels of the opaque part of the sprite called `name` on `sheet`, from its
    /// grid in `art/` (`.` is clear), counting the 1px outline the art tool adds round it.
    fn opaque_width(sheet: &str, name: &str) -> u32 {
        let doc = sheet_doc(sheet);
        let sprite = doc["sprite"]
            .as_array()
            .unwrap()
            .iter()
            .find(|s| s["name"].as_str() == Some(name))
            .unwrap_or_else(|| panic!("{sheet}: no sprite {name}"));
        let outline = sprite
            .get("outline")
            .or_else(|| doc.get("outline"))
            .and_then(|o| o.as_bool())
            .unwrap_or(false);
        let grid = sprite["grid"].as_str().unwrap();
        let cols: Vec<usize> = grid
            .lines()
            .flat_map(|row| row.char_indices().filter(|&(_, c)| c != '.').map(|(x, _)| x))
            .collect();
        let (lo, hi) = (cols.iter().min().unwrap(), cols.iter().max().unwrap());
        (hi - lo + 1) as u32 + if outline { 2 } else { 0 }
    }

    /// A footprint `w` tiles wide takes a sprite `w * 16` px wide, give or take 2 px
    /// (docs/design/buildings.md, "Drawing"): no wider, or it overlaps its neighbours, and wider
    /// than the next size down, or it needs a smaller footprint.
    #[test]
    fn each_buildings_art_is_as_wide_as_its_footprint() {
        let defs = Defs::bundled();
        for b in &defs.buildings {
            let w = u32::from(b.footprint.0);
            let fits = |px: u32| px <= w * 16 + 2 && (w == 1 || px > (w - 1) * 16 + 2);
            let mut all_fit = true;
            for n in Nation::ALL {
                let Some(art) = building_art(b.art_for(n)) else { continue };
                let px = opaque_width(art.frames[0].sheet, art.name);
                all_fit &= fits(px);
                if !AWAITING_FOOTPRINT.contains(&b.id.as_str()) {
                    assert!(fits(px), "{} ({}): {px} px of art on {w} tiles", b.id, art.name);
                }
            }
            if AWAITING_FOOTPRINT.contains(&b.id.as_str()) {
                assert!(
                    !all_fit,
                    "{} fits its footprint now: take it off AWAITING_FOOTPRINT",
                    b.id
                );
            }
        }
        for id in AWAITING_FOOTPRINT {
            assert!(defs.building_id(id).is_some(), "AWAITING_FOOTPRINT names unknown {id}");
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

    /// Every piece of track draws the track its mask asks for: the cells are in mask order, a
    /// straight piece runs the way it joins, and an arm reaches the tile edge it faces (so two
    /// pieces side by side meet) while a side nothing joins stays shut.
    #[test]
    fn each_piece_of_track_reaches_exactly_the_edges_it_joins() {
        let doc = sheet_doc(SHEET_EARTH);
        let sprites = doc["sprite"].as_array().unwrap();
        let rail = building_art("rail").unwrap();
        assert!(rail.connects && rail.frames.len() == 16);
        for mask in 0..16u8 {
            let cell = link_frame(rail.frames, mask);
            let grid: Vec<&str> =
                sprites[(1 + mask) as usize]["grid"].as_str().unwrap().lines().collect();
            assert_eq!(
                name_at(cell),
                if mask == 0 { "rail".into() } else { format!("rail_{}", mask + 1) }
            );
            // The tile is the bottom-centre 16x16: columns 8..24, rows 16..32. An edge is open
            // where track reaches its middle two pixels.
            let open = |row: usize, col: usize| grid[row].as_bytes()[col] != b'.';
            let north = open(16, 15) && open(16, 16);
            let south = open(31, 15) && open(31, 16);
            let west = open(23, 8) && open(24, 8);
            let east = open(23, 23) && open(24, 23);
            let got = [north, east, south, west]
                .iter()
                .zip([LINK_N, LINK_E, LINK_S, LINK_W])
                .filter(|(open, _)| **open)
                .fold(0, |m, (_, bit)| m | bit);
            assert_eq!(got, mask, "rail mask {mask:04b} reaches edges {got:04b}");
        }
        assert!(connects("rail") && !connects("bed") && !connects("no_such_building"));
    }

    #[test]
    fn an_unknown_sprite_name_draws_the_fallback() {
        assert_eq!(frames("no_such_building"), &[UNFINISHED]);
        assert_eq!(frames("campfire").len(), 3);
        assert_eq!(frames("windmill").len(), 3);
        assert_eq!(frames("wind_turbine").len(), 3);
        assert_eq!(frames("bed").len(), 1);
    }

    /// Row depth sorts a building by the tile it stands on, so the sprite ends on the bottom row
    /// of that tile's 16px: the foot (outline) is on row 31 of its cell and nothing is lower.
    #[test]
    fn every_sprite_stands_on_the_bottom_of_its_tile_and_fits_its_cell() {
        for sheet in SHEETS {
            for s in sheet_doc(sheet)["sprite"].as_array().unwrap() {
                check_standing(s);
            }
        }
    }

    fn check_standing(s: &toml::Value) {
        {
            let name = s["name"].as_str().unwrap();
            let grid: Vec<&str> = s["grid"].as_str().unwrap().lines().collect();
            assert_eq!(grid.len(), 32, "{name}");
            // A piece of track is flat ground with no outline: it may stop short of the tile's
            // bottom row (it only reaches it where it joins the tile below), and fills the row
            // when it does, so that two pieces one above the other meet with no seam (#135).
            let flat = name == "rail" || name.starts_with("rail_");
            if !flat {
                assert!(grid[30].bytes().any(|c| c != b'.'), "{name}: floats above its tile");
                assert!(grid[31].bytes().all(|c| c == b'.'), "{name}: bottom row");
            }
            assert!(grid[0].bytes().all(|c| c == b'.'), "{name}: top row");
            assert!(grid.iter().all(|r| r.len() == 32), "{name}");
            assert!(grid.iter().all(|r| r.starts_with('.') && r.ends_with('.')), "{name}: sides");
            // The body stands on its own tile: some of its foot is in the bottom-centre 16px.
            if !flat {
                assert!(grid[30][8..24].bytes().any(|c| c != b'.'), "{name}: foot off its tile");
            }
        }
    }
}
