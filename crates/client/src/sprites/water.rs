//! Water and shores (#8): bitmask-autotiled water, biome-matched to the land it touches. The art
//! is `art/water.toml` (#146), drawn by `cargo xtask art` into `assets/original/water.png`.

use fn_sim::{Pos, Terrain, TileMap};

use super::{Cell, GroundRule, one, tile_hash};

/// The sheet the water art is on: three 4x4 shore sets (sand, snow, grass) stacked top to bottom,
/// then the fish spot. The cells are named in `art/water.toml`; a test below keeps them honest.
pub const SHEET: &str = "original/water.png";

/// A bitmask-autotiled water shore: 16 pieces of a water sheet (#8), one per combination of which
/// cardinal neighbors of a water tile are dry land. Laid out as a 4x4 block of 16px cells
/// (`art/water.toml`):
///
/// ```text
/// NW      N      NE      |strait
/// W       .      E       |strait
/// SW      S      SE      |strait
/// -------strait-------   alt
/// ```
///
/// where `.` is the open-water interior and `alt` is its second variant (another scatter of wave
/// marks, so open water doesn't read as a lattice). The right column and bottom row are the
/// straits: land on both sides with water flowing through, and their closed end caps.
pub struct WaterSet {
    interior: Cell,
    interior_alt: Cell,
    n: Cell,
    s: Cell,
    e: Cell,
    w: Cell,
    nw: Cell,
    ne: Cell,
    sw: Cell,
    se: Cell,
    v_top: Cell,
    v_mid: Cell,
    v_bot: Cell,
    h_left: Cell,
    h_mid: Cell,
    h_right: Cell,
}

impl WaterSet {
    /// `col`/`row` is the top-left (NW corner) cell of the 4x4 block.
    const fn new(sheet: &'static str, col: u32, row: u32) -> Self {
        WaterSet {
            nw: one(sheet, col, row),
            n: one(sheet, col + 1, row),
            ne: one(sheet, col + 2, row),
            v_top: one(sheet, col + 3, row),
            w: one(sheet, col, row + 1),
            interior: one(sheet, col + 1, row + 1),
            e: one(sheet, col + 2, row + 1),
            v_mid: one(sheet, col + 3, row + 1),
            sw: one(sheet, col, row + 2),
            s: one(sheet, col + 1, row + 2),
            se: one(sheet, col + 2, row + 2),
            v_bot: one(sheet, col + 3, row + 2),
            h_left: one(sheet, col, row + 3),
            h_mid: one(sheet, col + 1, row + 3),
            h_right: one(sheet, col + 2, row + 3),
            interior_alt: one(sheet, col + 3, row + 3),
        }
    }

    /// The piece for a water tile whose cardinal neighbors are land (`true`) or more water
    /// (`false`). A tile boxed in on all four sides has no single piece that reads right (and the
    /// sheet has no "isolated puddle" piece), so it falls back to the open-water interior.
    fn cell(&self, n: bool, s: bool, e: bool, w: bool) -> Cell {
        match (n, s, e, w) {
            (false, false, false, false) => self.interior,
            (true, false, false, false) => self.n,
            (false, true, false, false) => self.s,
            (false, false, true, false) => self.e,
            (false, false, false, true) => self.w,
            (true, false, false, true) => self.nw,
            (true, false, true, false) => self.ne,
            (false, true, false, true) => self.sw,
            (false, true, true, false) => self.se,
            (true, true, false, false) => self.v_mid,
            (false, false, true, true) => self.h_mid,
            (true, true, false, true) => self.h_left,
            (true, true, true, false) => self.h_right,
            (true, false, true, true) => self.v_top,
            (false, true, true, true) => self.v_bot,
            (true, true, true, true) => self.interior,
        }
    }
}

/// The floor cell for a water tile at `p`, bitmask-autotiled (#8) against its four cardinal
/// neighbors: a neighbor counts as "land" (picks an edge or corner piece) unless it's also
/// `Terrain::is_water` (sea, river, lake or a shallow ford all blend together), including off the
/// edge of the map, which stays open water rather than growing a false shore.
pub fn water_cell(map: &TileMap, p: Pos, set: &WaterSet) -> Cell {
    let land = |dx: i32, dy: i32| {
        map.get(Pos::new(p.x + dx, p.y + dy)).is_some_and(|t| !t.terrain.is_water())
    };
    set.cell(land(0, -1), land(0, 1), land(1, 0), land(-1, 0))
}

/// What the renderer draws for the water tile at `p`: `water_cell`, except that open water
/// (the interior piece) is one of two scatters of wave marks, picked by a stable per-tile hash so
/// a lake doesn't read as a lattice. Shore pieces have one look each.
pub fn water_tile(map: &TileMap, p: Pos, set: &WaterSet) -> Cell {
    let cell = water_cell(map, p, set);
    if cell == set.interior && tile_hash(p, 2) & 1 == 1 { set.interior_alt } else { cell }
}
/// The fish shadows and ripples (#56), drawn over a water tile on a transparent background.
pub const FISH_SPOT: Cell = one(SHEET, 0, 12);

/// The shore sets, each bordered in the base tone of the floor it meets (`art/palette.toml`):
/// sand for desert, snow for tundra and grass for the rest, swamps included. Each is a 4x4 block
/// of 16px cells on `SHEET`; see `WaterSet` for the layout within a block. Water is blue in every
/// set. Swamp lakes and rivers once read as pink with the pack's purple bog water (#109), so a
/// future bog terrain would get a set of its own.
const SAND_WATER: WaterSet = WaterSet::new(SHEET, 0, 0);
const SNOW_WATER: WaterSet = WaterSet::new(SHEET, 0, 4);
const GRASS_WATER: WaterSet = WaterSet::new(SHEET, 0, 8);

const WATERS: &[Terrain] = &[Terrain::Water, Terrain::River, Terrain::Lake];

pub const GROUND: &[GroundRule] = &[
    GroundRule {
        terrains: WATERS,
        biome: Some("tundra"),
        base: SNOW_WATER.interior,
        variants: &[],
        decor: None,
        water: Some(&SNOW_WATER),
        cliff: None,
    },
    GroundRule {
        terrains: WATERS,
        biome: Some("desert"),
        base: SAND_WATER.interior,
        variants: &[],
        decor: None,
        water: Some(&SAND_WATER),
        cliff: None,
    },
    GroundRule {
        terrains: WATERS,
        biome: None,
        base: GRASS_WATER.interior,
        variants: &[],
        decor: None,
        water: Some(&GRASS_WATER),
        cliff: None,
    },
];

#[cfg(test)]
mod tests {
    use super::super::ground_rule;
    use super::super::test_support::terrain_map;
    use super::*;
    use fn_sim::Defs;

    /// Every `WATERS` row (not `SHORES`/`LAND`/`MOUNTAIN`) is autotiled rather than flat.
    #[test]
    fn every_biome_s_water_is_autotiled() {
        let defs = Defs::bundled();
        for biome in &defs.biomes {
            let rule = ground_rule(Terrain::Water, &biome.id).unwrap();
            assert!(rule.water.is_some(), "{} water isn't autotiled", biome.id);
        }
    }

    #[test]
    fn swamp_water_is_blue_not_bog_purple() {
        for terrain in WATERS {
            let rule = ground_rule(*terrain, "swamp").unwrap();
            assert!(
                rule.water.is_some_and(
                    |set| set.interior == GRASS_WATER.interior && set.nw == GRASS_WATER.nw
                ),
                "{terrain:?} in a swamp should use the grass-edged blue water"
            );
        }
    }

    /// A 3x3 map whose center is water, with `n`/`s`/`e`/`w` marking which cardinal neighbor is
    /// land; the corners are irrelevant (only cardinals feed the bitmask) so they're left water.
    fn water_neighbor_map(n: bool, s: bool, e: bool, w: bool) -> TileMap {
        let land = |b: bool| if b { Terrain::Land } else { Terrain::Water };
        terrain_map(3, 3, |p| match (p.x, p.y) {
            (1, 0) => land(n),
            (1, 2) => land(s),
            (2, 1) => land(e),
            (0, 1) => land(w),
            _ => Terrain::Water,
        })
    }

    #[test]
    fn water_cell_picks_every_piece_from_its_land_neighbors() {
        let set = &SAND_WATER;
        let cases = [
            ((false, false, false, false), set.interior),
            ((true, false, false, false), set.n),
            ((false, true, false, false), set.s),
            ((false, false, true, false), set.e),
            ((false, false, false, true), set.w),
            ((true, false, false, true), set.nw),
            ((true, false, true, false), set.ne),
            ((false, true, false, true), set.sw),
            ((false, true, true, false), set.se),
            ((true, true, false, false), set.v_mid),
            ((false, false, true, true), set.h_mid),
            ((true, true, false, true), set.h_left),
            ((true, true, true, false), set.h_right),
            ((true, false, true, true), set.v_top),
            ((false, true, true, true), set.v_bot),
            ((true, true, true, true), set.interior),
        ];
        for ((n, s, e, w), expected) in cases {
            let map = water_neighbor_map(n, s, e, w);
            let got = water_cell(&map, Pos::new(1, 1), set);
            assert_eq!(got, expected, "n={n} s={s} e={e} w={w}");
        }
    }

    /// The names of `art/water.toml`'s sprites, in file order (the order of the sheet's cells).
    fn sprite_names() -> Vec<String> {
        let path = format!("{}/../../art/water.toml", env!("CARGO_MANIFEST_DIR"));
        let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{path}: {e}"));
        let doc: toml::Table = text.parse().unwrap();
        doc["sprite"]
            .as_array()
            .unwrap()
            .iter()
            .map(|s| s["name"].as_str().unwrap().to_owned())
            .collect()
    }

    /// The name `art/water.toml` gives the sprite at `cell` (the sheet is 4 cells wide).
    fn name_at(names: &[String], cell: Cell) -> &str {
        assert_eq!(cell.sheet, SHEET);
        assert_eq!((cell.w, cell.h), (1, 1));
        &names[(cell.row * 4 + cell.col) as usize]
    }

    #[test]
    fn every_piece_is_the_sprite_it_claims_to_be() {
        let names = sprite_names();
        for (prefix, set) in [("sand", &SAND_WATER), ("snow", &SNOW_WATER), ("grass", &GRASS_WATER)]
        {
            let pieces = [
                ("nw", set.nw),
                ("n", set.n),
                ("ne", set.ne),
                ("v_top", set.v_top),
                ("w", set.w),
                ("interior", set.interior),
                ("e", set.e),
                ("v_mid", set.v_mid),
                ("sw", set.sw),
                ("s", set.s),
                ("se", set.se),
                ("v_bot", set.v_bot),
                ("h_left", set.h_left),
                ("h_mid", set.h_mid),
                ("h_right", set.h_right),
                ("interior_alt", set.interior_alt),
            ];
            for (piece, cell) in pieces {
                assert_eq!(name_at(&names, cell), format!("{prefix}_{piece}"));
            }
        }
    }

    /// The fish spot is a cell of the new sheet, so `TilesetWater.png` isn't needed any more.
    #[test]
    fn the_fish_spot_is_a_cell_of_the_water_sheet() {
        let names = sprite_names();
        assert_eq!(name_at(&names, FISH_SPOT), "fish_spot");
        let (w, h) = super::super::test_support::png_size(SHEET);
        let [_, _, x1, y1] = FISH_SPOT.pixel_rect();
        assert!(x1 <= w as f32 && y1 <= h as f32, "the fish spot is outside {SHEET}");
        let rule = super::super::resource_rule("fish_spot", "plains").unwrap();
        assert_eq!(rule.cell, FISH_SPOT);
    }

    /// Open water is one of two wave scatters, stably per tile, and mixes both across a lake.
    #[test]
    fn open_water_mixes_two_stable_variants_and_shores_have_one() {
        let map = terrain_map(20, 20, |_| Terrain::Water);
        let picks: Vec<Cell> = map.iter().map(|(p, _)| water_tile(&map, p, &SAND_WATER)).collect();
        let alt = picks.iter().filter(|c| **c == SAND_WATER.interior_alt).count();
        let plain = picks.iter().filter(|c| **c == SAND_WATER.interior).count();
        assert_eq!(alt + plain, picks.len());
        assert!(alt > 100 && plain > 100, "alt {alt}, plain {plain}");
        assert_eq!(picks[7], water_tile(&map, Pos::new(7, 0), &SAND_WATER));

        let shore = water_neighbor_map(true, false, false, false);
        assert_eq!(water_tile(&shore, Pos::new(1, 1), &SAND_WATER), SAND_WATER.n);
    }

    /// A ford is still water as far as the shore goes: it doesn't grow a false edge next to it.
    #[test]
    fn a_ford_neighbor_does_not_count_as_land() {
        let map = terrain_map(3, 1, |p| if p.x == 0 { Terrain::Water } else { Terrain::Ford });
        assert_eq!(water_cell(&map, Pos::new(0, 0), &SAND_WATER), SAND_WATER.interior);
    }

    /// Off the edge of the map stays open water rather than growing a false shore.
    #[test]
    fn off_map_does_not_count_as_land() {
        let map = terrain_map(1, 1, |_| Terrain::Water);
        assert_eq!(water_cell(&map, Pos::new(0, 0), &SAND_WATER), SAND_WATER.interior);
    }
}
