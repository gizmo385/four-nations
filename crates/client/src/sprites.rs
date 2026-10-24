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
    Relief,
}

impl Sheet {
    pub const ALL: [Sheet; 4] = [Sheet::Floor, Sheet::Water, Sheet::Nature, Sheet::Relief];

    pub fn path(self) -> &'static str {
        match self {
            Sheet::Floor => "third_party/ninja-adventure/Backgrounds/Tilesets/TilesetFloor.png",
            Sheet::Water => "third_party/ninja-adventure/Backgrounds/Tilesets/TilesetWater.png",
            Sheet::Nature => "third_party/ninja-adventure/Backgrounds/Tilesets/TilesetNature.png",
            Sheet::Relief => "third_party/ninja-adventure/Backgrounds/Tilesets/TilesetRelief.png",
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
    /// Mountain tiles (#104): when set, a tile with lower ground beside it is drawn with the
    /// matching `cliff_cell` piece instead of `base`. `base` is then the set's flat top, so the
    /// interior of a range (and its boulders) blends into the rim.
    pub cliff: Option<&'static CliffSet>,
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

/// Autotiled mountain cliffs (#104): a raised plateau from `TilesetRelief`, 1 cell per piece. A
/// piece is picked by which cardinal neighbors are lower ground (any non-mountain tile). The sheet
/// has a 3x3 block for a plateau and a 1-wide, 3-tall column for a ridge:
///
/// ```text
/// strip_top |NW   N    NE
/// strip_mid |W    top  E
/// strip_bot |SW   S    SE
/// ```
///
/// Every piece is one tile and the cliff face is painted inside the south pieces (the lower two
/// thirds of `S`, `SW`, `SE` and `strip_bot`), so it never overlaps the row below and needs no
/// depth banding of its own, unlike a tree.
///
/// The sheet has no inner-corner pieces, no end caps for a horizontal ridge and no face-only
/// pieces for a plateau two cells thick, so a tile whose only lower neighbor is diagonal reads as
/// interior, and a ridge one tile tall (lower ground north and south) draws its south face
/// without a north rim.
pub struct CliffSet {
    top: Cell,
    n: Cell,
    s: Cell,
    e: Cell,
    w: Cell,
    nw: Cell,
    ne: Cell,
    sw: Cell,
    se: Cell,
    strip_top: Cell,
    strip_mid: Cell,
    strip_bot: Cell,
}

impl CliffSet {
    /// `col`/`row` is the top-left cell of the 1-wide column; the 3x3 block starts one right.
    const fn new(sheet: Sheet, col: u32, row: u32) -> Self {
        CliffSet {
            strip_top: one(sheet, col, row),
            strip_mid: one(sheet, col, row + 1),
            strip_bot: one(sheet, col, row + 2),
            nw: one(sheet, col + 1, row),
            n: one(sheet, col + 2, row),
            ne: one(sheet, col + 3, row),
            w: one(sheet, col + 1, row + 1),
            top: one(sheet, col + 2, row + 1),
            e: one(sheet, col + 3, row + 1),
            sw: one(sheet, col + 1, row + 2),
            s: one(sheet, col + 2, row + 2),
            se: one(sheet, col + 3, row + 2),
        }
    }

    /// The piece for a mountain tile whose cardinal neighbors are lower ground (`true`) or more
    /// mountain (`false`); `None` for a tile with no lower neighbor, which stays flat floor.
    fn cell(&self, n: bool, s: bool, e: bool, w: bool) -> Option<Cell> {
        Some(match (n, s, e, w) {
            (false, false, false, false) => return None,
            // Lower ground on both sides: a ridge one tile wide, capped where it ends. A lone
            // tile (lower on all four sides) takes the base of a ridge, face included.
            (_, _, true, true) => match (n, s) {
                (false, false) => self.strip_mid,
                (true, false) => self.strip_top,
                (_, true) => self.strip_bot,
            },
            // Anything facing south wears the face: with lower ground north as well (a ridge one
            // tile tall) the north rim is dropped, there is no piece for it.
            (_, true, false, true) => self.sw,
            (_, true, true, false) => self.se,
            (_, true, false, false) => self.s,
            (true, false, false, true) => self.nw,
            (true, false, true, false) => self.ne,
            (true, false, false, false) => self.n,
            (false, false, true, false) => self.e,
            (false, false, false, true) => self.w,
        })
    }
}

/// How a resource looks. Every sprite is bottom-aligned to its tile.
pub struct ResourceRule {
    pub resource: &'static str,
    /// `None` matches every biome.
    pub biome: Option<&'static str>,
    pub cell: Cell,
}

use Sheet::{Floor, Nature, Relief, Water};

const WATER: Cell = one(Water, 1, 1);

/// The pack's pre-coloured water shores (`docs/design/art.md`): sand for desert, snow for tundra
/// and grass for the rest, swamps included. Each is a 4x4 block of 16px cells on `TilesetWater`;
/// see `WaterSet` for the layout within a block. The sheet's fourth block, purple bog water on mud
/// at (13, 6), is left for a future bog terrain: swamp lakes and rivers drawn with it read as pink
/// (#109).
const SAND_WATER: WaterSet = WaterSet::new(Water, 0, 0);
const SNOW_WATER: WaterSet = WaterSet::new(Water, 13, 0);
const GRASS_WATER: WaterSet = WaterSet::new(Water, 0, 6);

/// The pack's two plateaus (`docs/design/art.md`): a grey-faced cliff with a snow top for tundra,
/// and a brown-faced cliff with a sand top for every other biome.
const SNOW_CLIFF: CliffSet = CliffSet::new(Relief, 0, 0);
const SAND_CLIFF: CliffSet = CliffSet::new(Relief, 0, 5);

const SAND: Cell = one(Floor, 1, 1);
const SNOW: Cell = one(Floor, 1, 15);
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

/// Boulders on a range's flat interior; its rim is a cliff (#104).
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
        cliff: None,
    },
    GroundRule {
        terrains: WATERS,
        biome: Some("desert"),
        base: WATER,
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
    // Fords share the shore tile until autotiling can draw a crossing.
    GroundRule {
        terrains: SHORES,
        biome: Some("tundra"),
        base: SNOW,
        variants: &[],
        decor: None,
        water: None,
        cliff: None,
    },
    GroundRule {
        terrains: SHORES,
        biome: None,
        base: SAND,
        variants: &[],
        decor: None,
        water: None,
        cliff: None,
    },
    GroundRule {
        terrains: MOUNTAIN,
        biome: Some("tundra"),
        base: SNOW_CLIFF.top,
        variants: &[],
        decor: Some(SNOWY_BOULDERS),
        water: None,
        cliff: Some(&SNOW_CLIFF),
    },
    GroundRule {
        terrains: MOUNTAIN,
        biome: None,
        base: SAND_CLIFF.top,
        variants: &[],
        decor: Some(BOULDERS),
        water: None,
        cliff: Some(&SAND_CLIFF),
    },
    GroundRule {
        terrains: LAND,
        biome: Some("desert"),
        base: SAND,
        variants: SAND_VARIANTS,
        decor: None,
        water: None,
        cliff: None,
    },
    GroundRule {
        terrains: LAND,
        biome: Some("tundra"),
        base: SNOW,
        variants: &[],
        decor: None,
        water: None,
        cliff: None,
    },
    GroundRule {
        terrains: LAND,
        biome: Some("swamp"),
        base: MUD,
        variants: MUD_VARIANTS,
        decor: None,
        water: None,
        cliff: None,
    },
    GroundRule {
        terrains: LAND,
        biome: Some("forest"),
        base: DARK_GRASS,
        variants: DARK_GRASS_VARIANTS,
        decor: None,
        water: None,
        cliff: None,
    },
    GroundRule {
        terrains: LAND,
        biome: Some("taiga"),
        base: DARK_GRASS,
        variants: DARK_GRASS_VARIANTS,
        decor: None,
        water: None,
        cliff: None,
    },
    GroundRule {
        terrains: LAND,
        biome: None,
        base: LIGHT_GRASS,
        variants: LIGHT_GRASS_VARIANTS,
        decor: None,
        water: None,
        cliff: None,
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

/// The cliff piece for a mountain tile at `p` (#104), bitmask-autotiled against its four cardinal
/// neighbors: a neighbor is "lower" unless it is mountain too. Off the edge of the map doesn't
/// count, so a range running off the map doesn't grow a false cliff there. `None` for a tile
/// with no lower neighbor, which is the rule's flat `base`.
pub fn cliff_cell(map: &TileMap, p: Pos, set: &CliffSet) -> Option<Cell> {
    let lower = |dx: i32, dy: i32| {
        map.get(Pos::new(p.x + dx, p.y + dy)).is_some_and(|t| t.terrain != Terrain::Mountain)
    };
    set.cell(lower(0, -1), lower(0, 1), lower(1, 0), lower(-1, 0))
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

    /// The decoration for the tile at `p`, if it rolls one. A mountain's cliff rim carries none
    /// (#104): boulders sit on the flat interior, not across a cliff face.
    pub fn decor_on(&self, map: &TileMap, p: Pos) -> Option<Cell> {
        let on_cliff = self.cliff.is_some_and(|set| cliff_cell(map, p, set).is_some());
        if on_cliff { None } else { self.decor_at(p) }
    }

    /// The decoration for the tile at `p`, if it rolls one, wherever it is on the map.
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

    // -------------------------------------------------------------------------------------
    // #104: mountain cliffs

    /// A `w` by `h` map of one terrain, with `f` free to override single tiles.
    fn terrain_map(w: i32, h: i32, f: impl Fn(Pos) -> Terrain) -> TileMap {
        Grid::from_fn(w, h, |p| Tile {
            elevation: 0,
            temperature: 0,
            moisture: 0,
            terrain: f(p),
            biome: BiomeId(0),
            resource: None,
            building: None,
        })
    }

    /// A 3x3 map whose center is a mountain and whose cardinal neighbors are lower ground
    /// (`true`, land) or more mountain (`false`). Corners are mountain.
    fn mountain_neighbor_map(n: bool, s: bool, e: bool, w: bool) -> TileMap {
        let at = |lower: bool| if lower { Terrain::Land } else { Terrain::Mountain };
        terrain_map(3, 3, |p| match (p.x, p.y) {
            (1, 0) => at(n),
            (1, 2) => at(s),
            (2, 1) => at(e),
            (0, 1) => at(w),
            _ => Terrain::Mountain,
        })
    }

    #[test]
    fn every_mountain_is_a_cliff_on_the_relief_sheet() {
        let defs = Defs::bundled();
        for biome in &defs.biomes {
            let rule = ground_rule(Terrain::Mountain, &biome.id).unwrap();
            let set = rule.cliff.unwrap_or_else(|| panic!("{} mountain has no cliff", biome.id));
            assert_eq!(rule.base, set.top, "{}: interior blends into the rim", biome.id);
            assert!(rule.decor.is_some(), "{}: interior keeps its boulders", biome.id);
        }
        for terrain in TERRAINS.iter().filter(|t| **t != Terrain::Mountain) {
            let rule = ground_rule(*terrain, "plains").unwrap();
            assert!(rule.cliff.is_none(), "{terrain:?}");
        }
    }

    #[test]
    fn tundra_cliffs_are_snowy_and_the_rest_are_sandy() {
        let snow = ground_rule(Terrain::Mountain, "tundra").unwrap().cliff.unwrap();
        let sand = ground_rule(Terrain::Mountain, "plains").unwrap().cliff.unwrap();
        assert_eq!(snow.top, one(Relief, 2, 1));
        assert_eq!(sand.top, one(Relief, 2, 6));
        assert_eq!(ground_rule(Terrain::Mountain, "desert").unwrap().cliff.unwrap().top, sand.top);
    }

    #[test]
    fn cliff_pieces_stay_inside_the_relief_sheet() {
        let (w, h) = png_size(Sheet::Relief.path());
        for set in [&SNOW_CLIFF, &SAND_CLIFF] {
            let pieces = [
                set.top,
                set.n,
                set.s,
                set.e,
                set.w,
                set.nw,
                set.ne,
                set.sw,
                set.se,
                set.strip_top,
                set.strip_mid,
                set.strip_bot,
            ];
            for c in pieces {
                assert_eq!((c.sheet, c.w, c.h), (Relief, 1, 1));
                let [x0, y0, x1, y1] = c.pixel_rect();
                assert!(x0 >= 0.0 && y0 >= 0.0 && x1 <= w as f32 && y1 <= h as f32, "{c:?}");
            }
        }
    }

    #[test]
    fn cliff_cell_picks_every_piece_from_its_lower_neighbors() {
        let set = &SAND_CLIFF;
        let cases = [
            ((true, false, false, false), Some(set.n)),
            ((false, true, false, false), Some(set.s)),
            ((false, false, true, false), Some(set.e)),
            ((false, false, false, true), Some(set.w)),
            ((true, false, false, true), Some(set.nw)),
            ((true, false, true, false), Some(set.ne)),
            ((false, true, false, true), Some(set.sw)),
            ((false, true, true, false), Some(set.se)),
            // A ridge one tile wide, running north to south.
            ((false, false, true, true), Some(set.strip_mid)),
            ((true, false, true, true), Some(set.strip_top)),
            ((false, true, true, true), Some(set.strip_bot)),
            ((true, true, true, true), Some(set.strip_bot)),
            // A ridge one tile tall keeps its south face.
            ((true, true, false, false), Some(set.s)),
            ((true, true, false, true), Some(set.sw)),
            ((true, true, true, false), Some(set.se)),
        ];
        for ((n, s, e, w), expected) in cases {
            let map = mountain_neighbor_map(n, s, e, w);
            assert_eq!(cliff_cell(&map, Pos::new(1, 1), set), expected, "n={n} s={s} e={e} w={w}");
        }
    }

    /// Mountain on every side, even with lower ground on the diagonals, stays a flat floor.
    #[test]
    fn interior_mountain_tiles_have_no_cliff() {
        let map = mountain_neighbor_map(false, false, false, false);
        assert_eq!(cliff_cell(&map, Pos::new(1, 1), &SAND_CLIFF), None);
        let diagonals =
            terrain_map(
                3,
                3,
                |p| {
                    if p.x != 1 && p.y != 1 { Terrain::Land } else { Terrain::Mountain }
                },
            );
        assert_eq!(cliff_cell(&diagonals, Pos::new(1, 1), &SAND_CLIFF), None);
    }

    /// Water, coast and ford all count as lower ground.
    #[test]
    fn any_non_mountain_neighbor_is_lower_ground() {
        for lower in [Terrain::Water, Terrain::Coast, Terrain::River, Terrain::Lake, Terrain::Ford]
        {
            let map = terrain_map(1, 2, |p| if p.y == 0 { lower } else { Terrain::Mountain });
            assert_eq!(cliff_cell(&map, Pos::new(0, 1), &SAND_CLIFF), Some(SAND_CLIFF.n));
        }
    }

    /// A range running off the map doesn't grow a false cliff along the edge.
    #[test]
    fn off_map_is_not_lower_ground() {
        let map = terrain_map(1, 1, |_| Terrain::Mountain);
        assert_eq!(cliff_cell(&map, Pos::new(0, 0), &SAND_CLIFF), None);
    }

    #[test]
    fn boulders_sit_on_the_interior_not_the_cliff() {
        let rule = ground_rule(Terrain::Mountain, "plains").unwrap();
        // A tile that rolls a boulder, in the middle of a tall column of map.
        let p = (1..40).map(|y| Pos::new(1, y)).find(|p| rule.decor_at(*p).is_some()).unwrap();
        let range = terrain_map(3, 42, |_| Terrain::Mountain);
        assert_eq!(rule.decor_on(&range, p), rule.decor_at(p));
        let peak = terrain_map(3, 42, |q| if q == p { Terrain::Mountain } else { Terrain::Land });
        assert_eq!(rule.decor_on(&peak, p), None, "a lone peak is all cliff");
    }
}
