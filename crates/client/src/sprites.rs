//! The one place that decides which Ninja Adventure sprite draws each terrain, biome and resource
//! (docs/design/art.md). A new biome or resource is one new row in `GROUND` or `RESOURCES`; the
//! tests below fail until every terrain and biome in `data/` has a row.
//!
//! Cells are (column, row) in 16px cells of a tileset sheet. Rows are matched top to bottom and
//! the first match wins, so put specific rows (a biome) before general ones (`biome: None`).

use fn_sim::{Pos, Terrain, TileMap};

/// The tile size of the pack, and of the map.
pub const CELL: f32 = 16.0;

/// A tileset sheet under `assets/third_party/ninja-adventure/Backgrounds/Tilesets/`.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Sheet {
    Floor,
    Water,
    Nature,
}

impl Sheet {
    pub const ALL: [Sheet; 3] = [Sheet::Floor, Sheet::Water, Sheet::Nature];

    pub fn path(self) -> &'static str {
        match self {
            Sheet::Floor => "third_party/ninja-adventure/Backgrounds/Tilesets/TilesetFloor.png",
            Sheet::Water => "third_party/ninja-adventure/Backgrounds/Tilesets/TilesetWater.png",
            Sheet::Nature => "third_party/ninja-adventure/Backgrounds/Tilesets/TilesetNature.png",
        }
    }
}

/// A rectangle of cells on a sheet: `w` by `h` cells with its top-left at (`col`, `row`).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Cell {
    pub sheet: Sheet,
    pub col: u32,
    pub row: u32,
    pub w: u32,
    pub h: u32,
}

impl Cell {
    /// The region of the sheet image, in pixels: (min x, min y, max x, max y).
    pub fn pixel_rect(&self) -> [f32; 4] {
        let (x, y) = (self.col as f32 * CELL, self.row as f32 * CELL);
        [x, y, x + self.w as f32 * CELL, y + self.h as f32 * CELL]
    }
}

const fn one(sheet: Sheet, col: u32, row: u32) -> Cell {
    Cell { sheet, col, row, w: 1, h: 1 }
}

/// A 2x2 sprite (the trees), drawn at native size.
const fn big(sheet: Sheet, col: u32, row: u32) -> Cell {
    Cell { sheet, col, row, w: 2, h: 2 }
}

/// Small sprites scattered over a floor (not sim resources), drawn on tiles with no resource.
pub struct Decor {
    pub cells: &'static [Cell],
    /// Chance that a tile gets one, as `num` in `den`.
    pub num: u32,
    pub den: u32,
}

/// How the ground layer of a tile looks.
pub struct GroundRule {
    pub terrains: &'static [Terrain],
    /// `None` matches every biome.
    pub biome: Option<&'static str>,
    pub base: Cell,
    /// One in four tiles shows one of these instead of `base`, so flat floors don't look tiled.
    pub variants: &'static [Cell],
    pub decor: Option<Decor>,
    /// Water tiles (#8): when set, `base`/`variants` are ignored and the tile is drawn with
    /// `water_cell`, which picks a shore piece from this set by which neighbors are dry land.
    pub water: Option<&'static WaterSet>,
}

/// A bitmask-autotiled water shore: 16 pieces of a `Sheet::Water` sub-sheet (#8), one per
/// combination of which cardinal neighbors of a water tile are dry land. Laid out as a 4x4 block
/// of 16px cells, read off the pack's own "pond" demo (`docs/design/art.md`):
///
/// ```text
/// NW   N    NE   |strait
/// W    .    E    |strait
/// SW   S    SE   |strait
/// -strait-  isolated (unused: art differs between variants, see `cell`)
/// ```
///
/// where `.` is the open-water interior and the right column and bottom-left row are 1-wide
/// straits (land on both sides, water flowing through) and their closed end caps.
pub struct WaterSet {
    interior: Cell,
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
    const fn new(sheet: Sheet, col: u32, row: u32) -> Self {
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
        }
    }

    /// The piece for a water tile whose cardinal neighbors are land (`true`) or more water
    /// (`false`). A tile boxed in on all four sides has no single piece that reads right (and the
    /// pack's "isolated puddle" tile isn't present for every variant), so it falls back to the
    /// open-water interior.
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

/// How a resource looks. Every sprite is bottom-aligned to its tile.
pub struct ResourceRule {
    pub resource: &'static str,
    /// `None` matches every biome.
    pub biome: Option<&'static str>,
    pub cell: Cell,
}

use Sheet::{Floor, Nature, Water};

const WATER: Cell = one(Water, 1, 1);

/// The pack's pre-coloured water shores (`docs/design/art.md`): sand for desert, snow for tundra
/// and grass for the rest, swamps included. Each is a 4x4 block of 16px cells on `TilesetWater`;
/// see `WaterSet` for the layout within a block. The sheet's fourth block, purple bog water on mud
/// at (13, 6), is left for a future bog terrain: swamp lakes and rivers drawn with it read as pink
/// (#109).
const SAND_WATER: WaterSet = WaterSet::new(Water, 0, 0);
const SNOW_WATER: WaterSet = WaterSet::new(Water, 13, 0);
const GRASS_WATER: WaterSet = WaterSet::new(Water, 0, 6);

const SAND: Cell = one(Floor, 1, 1);
const SNOW: Cell = one(Floor, 1, 15);
const DIRT: Cell = one(Floor, 1, 9);
const MUD: Cell = one(Floor, 12, 15);
const LIGHT_GRASS: Cell = one(Floor, 0, 12);
const DARK_GRASS: Cell = one(Floor, 11, 12);

const SAND_VARIANTS: &[Cell] = &[one(Floor, 0, 4), one(Floor, 1, 4)];
const MUD_VARIANTS: &[Cell] = &[one(Floor, 13, 19), one(Floor, 14, 19)];
const LIGHT_GRASS_VARIANTS: &[Cell] = &[one(Floor, 1, 12), one(Floor, 2, 12), one(Floor, 3, 12)];
const DARK_GRASS_VARIANTS: &[Cell] = &[one(Floor, 12, 12), one(Floor, 13, 12)];

const WATERS: &[Terrain] = &[Terrain::Water, Terrain::River, Terrain::Lake];
const SHORES: &[Terrain] = &[Terrain::Coast, Terrain::Ford];
const MOUNTAIN: &[Terrain] = &[Terrain::Mountain];
const LAND: &[Terrain] = &[Terrain::Land];

/// Mountains are a rock floor with grey boulders until autotiled cliffs land (#104).
const BOULDERS: Decor = Decor { cells: &[one(Nature, 18, 9)], num: 2, den: 3 };
const SNOWY_BOULDERS: Decor = Decor { cells: &[one(Nature, 4, 12)], num: 2, den: 3 };

pub const GROUND: &[GroundRule] = &[
    // Water (#8): bitmask-autotiled by `water_cell`, biome-matched to the land it touches.
    GroundRule {
        terrains: WATERS,
        biome: Some("tundra"),
        base: SNOW_WATER.interior,
        variants: &[],
        decor: None,
        water: Some(&SNOW_WATER),
    },
    GroundRule {
        terrains: WATERS,
        biome: Some("desert"),
        base: WATER,
        variants: &[],
        decor: None,
        water: Some(&SAND_WATER),
    },
    GroundRule {
        terrains: WATERS,
        biome: None,
        base: GRASS_WATER.interior,
        variants: &[],
        decor: None,
        water: Some(&GRASS_WATER),
    },
    // Fords share the shore tile until autotiling can draw a crossing.
    GroundRule {
        terrains: SHORES,
        biome: Some("tundra"),
        base: SNOW,
        variants: &[],
        decor: None,
        water: None,
    },
    GroundRule {
        terrains: SHORES,
        biome: None,
        base: SAND,
        variants: &[],
        decor: None,
        water: None,
    },
    GroundRule {
        terrains: MOUNTAIN,
        biome: Some("tundra"),
        base: SNOW,
        variants: &[],
        decor: Some(SNOWY_BOULDERS),
        water: None,
    },
    GroundRule {
        terrains: MOUNTAIN,
        biome: None,
        base: DIRT,
        variants: &[],
        decor: Some(BOULDERS),
        water: None,
    },
    GroundRule {
        terrains: LAND,
        biome: Some("desert"),
        base: SAND,
        variants: SAND_VARIANTS,
        decor: None,
        water: None,
    },
    GroundRule {
        terrains: LAND,
        biome: Some("tundra"),
        base: SNOW,
        variants: &[],
        decor: None,
        water: None,
    },
    GroundRule {
        terrains: LAND,
        biome: Some("swamp"),
        base: MUD,
        variants: MUD_VARIANTS,
        decor: None,
        water: None,
    },
    GroundRule {
        terrains: LAND,
        biome: Some("forest"),
        base: DARK_GRASS,
        variants: DARK_GRASS_VARIANTS,
        decor: None,
        water: None,
    },
    GroundRule {
        terrains: LAND,
        biome: Some("taiga"),
        base: DARK_GRASS,
        variants: DARK_GRASS_VARIANTS,
        decor: None,
        water: None,
    },
    GroundRule {
        terrains: LAND,
        biome: None,
        base: LIGHT_GRASS,
        variants: LIGHT_GRASS_VARIANTS,
        decor: None,
        water: None,
    },
];

pub const RESOURCES: &[ResourceRule] = &[
    ResourceRule { resource: "oak_tree", biome: None, cell: big(Nature, 0, 0) },
    ResourceRule { resource: "fir_tree", biome: Some("tundra"), cell: big(Nature, 8, 0) },
    ResourceRule { resource: "fir_tree", biome: None, cell: big(Nature, 2, 0) },
    // A cherry blossom stands in for a fruit tree: no tree in the pack is drawn with fruit.
    ResourceRule { resource: "fruit_tree", biome: None, cell: big(Nature, 14, 0) },
    // Grey rock, and the same rock studded with orange ore.
    ResourceRule { resource: "stone", biome: None, cell: one(Nature, 6, 14) },
    ResourceRule { resource: "gold_vein", biome: None, cell: one(Nature, 4, 14) },
    // The pack has no cabbage; a head of leafy greens stands in.
    ResourceRule { resource: "cabbage", biome: None, cell: one(Nature, 8, 11) },
    // Fish shadows on water, drawn over the whole tile.
    ResourceRule { resource: "fish_spot", biome: None, cell: one(Water, 11, 4) },
];

pub fn ground_rule(terrain: Terrain, biome: &str) -> Option<&'static GroundRule> {
    GROUND.iter().find(|r| r.terrains.contains(&terrain) && r.biome.is_none_or(|b| b == biome))
}

pub fn resource_rule(resource: &str, biome: &str) -> Option<&'static ResourceRule> {
    RESOURCES.iter().find(|r| r.resource == resource && r.biome.is_none_or(|b| b == biome))
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

/// A cheap, stable per-tile hash; `salt` gives independent rolls for the same tile.
fn tile_hash(p: Pos, salt: u32) -> u32 {
    let mut h = (p.x as u32).wrapping_mul(0x9E37_79B1) ^ (p.y as u32).wrapping_mul(0x85EB_CA77);
    h ^= salt.wrapping_mul(0xC2B2_AE3D);
    h ^= h >> 15;
    h = h.wrapping_mul(0x2C1B_3C6D);
    h ^= h >> 12;
    h = h.wrapping_mul(0x297A_2D39);
    h ^ (h >> 15)
}

impl GroundRule {
    /// The floor cell for the tile at `p`.
    pub fn floor_at(&self, p: Pos) -> Cell {
        let h = tile_hash(p, 1);
        if self.variants.is_empty() || !h.is_multiple_of(4) {
            return self.base;
        }
        self.variants[(h >> 8) as usize % self.variants.len()]
    }

    /// The decoration for the tile at `p`, if it rolls one.
    pub fn decor_at(&self, p: Pos) -> Option<Cell> {
        let d = self.decor.as_ref()?;
        let h = tile_hash(p, 2);
        (h % d.den < d.num).then(|| d.cells[(h >> 8) as usize % d.cells.len()])
    }
}

/// Where each character's art lives. Which characters a nation uses is data
/// (`characters` in `data/nations.toml`); this is only how a character id becomes files.
const CHARACTER_DIR: &str = "third_party/ninja-adventure/Actor/Character";

/// Characters whose sprite sheet isn't called `SpriteSheet.png` upstream.
const SHEET_FILES: &[(&str, &str)] = &[("SamuraiRed", "redsamurai.png")];

/// The sprite sheet of a character: 4 columns of walk frames, 7 rows of 16px cells.
pub fn character_sheet_path(character: &str) -> String {
    let file = SHEET_FILES.iter().find(|(c, _)| *c == character).map_or("SpriteSheet.png", |f| f.1);
    format!("{CHARACTER_DIR}/{character}/{file}")
}

/// The 38x38 portrait of a character.
pub fn character_face_path(character: &str) -> String {
    format!("{CHARACTER_DIR}/{character}/Faceset.png")
}

/// The down-facing idle frame: the top-left cell of a character's sheet, in pixels:
/// (min x, min y, max x, max y). Walk cycles and other directions come later.
pub const IDLE_DOWN: [f32; 4] = [0.0, 0.0, CELL, CELL];

/// Portraits are drawn at integer scale, so the 38px pixels stay crisp and square.
pub const FACE_SIZE: f32 = 38.0;

#[cfg(test)]
mod tests {
    use super::*;
    use fn_sim::defs::BiomeId;
    use fn_sim::{Defs, Grid, Nation, Tile};

    /// The pixel size of a PNG, from its IHDR chunk.
    fn png_size(path: &str) -> (u32, u32) {
        let bytes = std::fs::read(format!("{}/{path}", crate::ASSET_DIR))
            .unwrap_or_else(|e| panic!("{path}: {e}"));
        assert_eq!(&bytes[1..4], b"PNG", "{path}");
        let u32_at = |i: usize| u32::from_be_bytes(bytes[i..i + 4].try_into().unwrap());
        (u32_at(16), u32_at(20))
    }

    #[test]
    fn every_nation_has_three_characters_with_art() {
        let defs = Defs::bundled();
        for nation in Nation::ALL {
            let characters = &defs.nation(nation).characters;
            assert_eq!(characters.len(), 3, "{nation:?}");
            for c in characters {
                assert_eq!(png_size(&character_sheet_path(c)), (64, 112), "{c} sheet");
                assert_eq!(png_size(&character_face_path(c)), (38, 38), "{c} face");
            }
        }
    }

    #[test]
    fn no_character_is_shared_between_nations() {
        let defs = Defs::bundled();
        let all: Vec<&String> =
            Nation::ALL.iter().flat_map(|n| defs.nation(*n).characters.iter()).collect();
        for (i, c) in all.iter().enumerate() {
            assert!(!all[..i].contains(c), "{c} is used twice");
        }
    }

    #[test]
    fn samurai_red_uses_its_odd_sheet_name() {
        assert!(character_sheet_path("SamuraiRed").ends_with("SamuraiRed/redsamurai.png"));
        assert!(character_sheet_path("Monk2").ends_with("Monk2/SpriteSheet.png"));
    }

    const TERRAINS: [Terrain; 7] = [
        Terrain::Water,
        Terrain::Coast,
        Terrain::Land,
        Terrain::Mountain,
        Terrain::River,
        Terrain::Lake,
        Terrain::Ford,
    ];

    #[test]
    fn every_terrain_in_every_biome_has_a_floor() {
        let defs = Defs::bundled();
        for biome in &defs.biomes {
            for terrain in TERRAINS {
                assert!(
                    ground_rule(terrain, &biome.id).is_some(),
                    "no ground sprite for {terrain:?} in {}",
                    biome.id
                );
            }
        }
    }

    #[test]
    fn every_resource_in_every_biome_has_a_sprite() {
        let defs = Defs::bundled();
        for res in &defs.resources {
            for biome in &defs.biomes {
                assert!(
                    resource_rule(&res.id, &biome.id).is_some(),
                    "no sprite for {} in {}",
                    res.id,
                    biome.id
                );
            }
        }
    }

    #[test]
    fn trees_are_two_by_two_and_the_rest_one_cell() {
        for r in RESOURCES {
            let tree = r.resource.ends_with("_tree");
            assert_eq!((r.cell.w, r.cell.h), if tree { (2, 2) } else { (1, 1) }, "{}", r.resource);
        }
    }

    #[test]
    fn floors_are_never_big_sprites() {
        for r in GROUND {
            assert_eq!((r.base.w, r.base.h), (1, 1));
            assert!(r.variants.iter().all(|c| (c.w, c.h) == (1, 1)));
        }
    }

    #[test]
    fn variation_is_stable_and_mixed() {
        let rule = ground_rule(Terrain::Land, "plains").unwrap();
        let picks: Vec<Cell> = (0..400).map(|i| rule.floor_at(Pos::new(i % 20, i / 20))).collect();
        assert_eq!(picks[7], rule.floor_at(Pos::new(7, 0)));
        let plain = picks.iter().filter(|c| **c == rule.base).count();
        assert!((250..350).contains(&plain), "about three in four tiles are plain: {plain}");
    }

    #[test]
    fn about_two_in_three_mountain_tiles_get_a_boulder() {
        let rule = ground_rule(Terrain::Mountain, "plains").unwrap();
        let n = (0..900).filter(|i| rule.decor_at(Pos::new(i % 30, i / 30)).is_some()).count();
        assert!((540..660).contains(&n), "{n}");
    }

    // -------------------------------------------------------------------------------------
    // #8: water autotiling

    /// Every `WATERS` row (not `SHORES`/`LAND`/`MOUNTAIN`) is autotiled rather than flat.
    #[test]
    fn every_biome_s_water_is_autotiled() {
        let defs = Defs::bundled();
        for biome in &defs.biomes {
            let rule = ground_rule(Terrain::Water, &biome.id).unwrap();
            assert!(rule.water.is_some(), "{} water isn't autotiled", biome.id);
        }
    }

    /// A 3x3 map whose center is water, with `n`/`s`/`e`/`w` marking which cardinal neighbor is
    /// land; the corners are irrelevant (only cardinals feed the bitmask) so they're left water.
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

    fn water_neighbor_map(n: bool, s: bool, e: bool, w: bool) -> TileMap {
        let ch = |land: bool| if land { '.' } else { '~' };
        let rows = [format!("~{}~", ch(n)), format!("{}~{}", ch(w), ch(e)), format!("~{}~", ch(s))];
        Grid::from_fn(3, 3, |p| Tile {
            elevation: 0,
            temperature: 0,
            moisture: 0,
            terrain: if rows[p.y as usize].as_bytes()[p.x as usize] == b'.' {
                Terrain::Land
            } else {
                Terrain::Water
            },
            biome: BiomeId(0),
            resource: None,
            building: None,
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

    /// A ford is still water as far as the shore goes: it doesn't grow a false edge next to it.
    #[test]
    fn a_ford_neighbor_does_not_count_as_land() {
        let map = Grid::from_fn(3, 1, |p| Tile {
            elevation: 0,
            temperature: 0,
            moisture: 0,
            terrain: if p.x == 0 { Terrain::Water } else { Terrain::Ford },
            biome: BiomeId(0),
            resource: None,
            building: None,
        });
        assert_eq!(water_cell(&map, Pos::new(0, 0), &SAND_WATER), SAND_WATER.interior);
    }

    /// Off the edge of the map stays open water rather than growing a false shore.
    #[test]
    fn off_map_does_not_count_as_land() {
        let map = Grid::from_fn(1, 1, |_| Tile {
            elevation: 0,
            temperature: 0,
            moisture: 0,
            terrain: Terrain::Water,
            biome: BiomeId(0),
            resource: None,
            building: None,
        });
        assert_eq!(water_cell(&map, Pos::new(0, 0), &SAND_WATER), SAND_WATER.interior);
    }
}
