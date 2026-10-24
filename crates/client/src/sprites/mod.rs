//! The one place that decides which sprite draws each terrain, biome and resource
//! (docs/design/art.md, docs/design/pixel-art.md). One module per art family, so art PRs for
//! different families touch different files:
//!
//! | Module | Owns |
//! |---|---|
//! | `floors` | the land and shore `GROUND` rows, `floor_at` |
//! | `water` | `WaterSet`, `water_cell`, `water_tile`, the water `GROUND` rows, `FISH_SPOT` |
//! | `cliffs` | `CliffSet`, `cliff_cell`, the mountain `GROUND` rows and boulders |
//! | `trees` | the 2x2 tree `RESOURCES` rows |
//! | `resources` | the other `RESOURCES` rows |
//! | `buildings` | building art name to its frames, and the fallback for a building with none |
//! | `characters` | character id to sheet and face paths |
//! | `fx` | bending's lasting effects: flames and wet ground (`art/fx.toml`) |
//!
//! A sheet is named by its asset path (a `&'static str`, a `pub const` in the family module that
//! uses it); `Art` loads each path once on first use, so a new sheet is only a new path constant.
//! A new biome or resource is one new row in a family's `GROUND` or `RESOURCES`; the tests below
//! fail until every terrain and biome in `data/` has a row.
//!
//! Cells are (column, row) in 16px cells of a sheet. Rows are matched top to bottom and the first
//! match wins, so put specific rows (a biome) before general ones (`biome: None`). Families list
//! disjoint terrains and resources, so the order between families doesn't matter.

pub mod buildings;
pub mod characters;
pub mod cliffs;
pub mod floors;
pub mod fx;
pub mod resources;
pub mod trees;
pub mod water;

use fn_sim::{Pos, Terrain};

pub use characters::{character_face_path, character_sheet_path};
pub use cliffs::{CliffSet, cliff_cell};
pub use water::{WaterSet, water_tile};

/// The tile size of the pack, and of the map.
pub const CELL: f32 = 16.0;

/// A rectangle of cells on a sheet: `w` by `h` cells with its top-left at (`col`, `row`).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Cell {
    /// The asset path of the sheet image, under `assets/`.
    pub sheet: &'static str,
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

const fn one(sheet: &'static str, col: u32, row: u32) -> Cell {
    Cell { sheet, col, row, w: 1, h: 1 }
}

/// A 2x2 sprite (the trees), drawn at native size.
const fn big(sheet: &'static str, col: u32, row: u32) -> Cell {
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
    /// All the variants of a floor, `base` first (empty for a rule with one fixed look). A tile
    /// shows one by weighted pick (`GroundRule::floor_at`), so flat floors don't look tiled.
    pub variants: &'static [Cell],
    pub decor: Option<Decor>,
    /// Water tiles (#8): when set, `base`/`variants` are ignored and the tile is drawn with
    /// `water_tile`, which picks a shore piece from this set by which neighbors are dry land.
    pub water: Option<&'static WaterSet>,
    /// Mountain tiles (#104): when set, a tile with lower ground beside it is drawn with the
    /// matching `cliff_cell` piece instead of `base`. `base` is then the set's flat top, so the
    /// interior of a range (and its boulders) blends into the rim.
    pub cliff: Option<&'static CliffSet>,
}

/// How a resource looks. Every sprite is bottom-aligned to its tile.
pub struct ResourceRule {
    pub resource: &'static str,
    /// `None` matches every biome.
    pub biome: Option<&'static str>,
    pub cell: Cell,
}

/// The `GROUND` row lists of each family that draws ground.
const GROUND_FAMILIES: [&[GroundRule]; 3] = [water::GROUND, floors::GROUND, cliffs::GROUND];

/// The `RESOURCES` row lists of each family that draws resources.
const RESOURCE_FAMILIES: [&[ResourceRule]; 2] = [trees::RESOURCES, resources::RESOURCES];

/// Every ground row, family by family.
pub fn ground_rules() -> impl Iterator<Item = &'static GroundRule> {
    GROUND_FAMILIES.into_iter().flatten()
}

/// Every resource row, family by family.
pub fn resource_rules() -> impl Iterator<Item = &'static ResourceRule> {
    RESOURCE_FAMILIES.into_iter().flatten()
}

pub fn ground_rule(terrain: Terrain, biome: &str) -> Option<&'static GroundRule> {
    ground_rules().find(|r| r.terrains.contains(&terrain) && r.biome.is_none_or(|b| b == biome))
}

pub fn resource_rule(resource: &str, biome: &str) -> Option<&'static ResourceRule> {
    resource_rules().find(|r| r.resource == resource && r.biome.is_none_or(|b| b == biome))
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

/// The down-facing idle frame: the top-left cell of a character's sheet, in pixels:
/// (min x, min y, max x, max y). Walk cycles and other directions come later.
pub const IDLE_DOWN: [f32; 4] = [0.0, 0.0, CELL, CELL];

/// Portraits are drawn at integer scale, so the 38px pixels stay crisp and square.
pub const FACE_SIZE: f32 = 38.0;

/// Helpers shared by the family modules' tests.
#[cfg(test)]
pub(crate) mod test_support {
    use fn_sim::defs::BiomeId;
    use fn_sim::{Grid, Pos, Terrain, Tile, TileMap};

    /// The pixel size of a PNG, from its IHDR chunk.
    pub fn png_size(path: &str) -> (u32, u32) {
        let bytes = std::fs::read(format!("{}/{path}", crate::ASSET_DIR))
            .unwrap_or_else(|e| panic!("{path}: {e}"));
        assert_eq!(&bytes[1..4], b"PNG", "{path}");
        let u32_at = |i: usize| u32::from_be_bytes(bytes[i..i + 4].try_into().unwrap());
        (u32_at(16), u32_at(20))
    }

    pub const TERRAINS: [Terrain; 7] = [
        Terrain::Water,
        Terrain::Coast,
        Terrain::Land,
        Terrain::Mountain,
        Terrain::River,
        Terrain::Lake,
        Terrain::Ford,
    ];

    /// A `w` by `h` map of one terrain, with `f` free to override single tiles.
    pub fn terrain_map(w: i32, h: i32, f: impl Fn(Pos) -> Terrain) -> TileMap {
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
}

#[cfg(test)]
mod tests {
    use super::test_support::TERRAINS;
    use super::*;
    use fn_sim::Defs;

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
    fn floors_are_never_big_sprites() {
        for r in ground_rules() {
            assert_eq!((r.base.w, r.base.h), (1, 1));
            assert!(r.variants.iter().all(|c| (c.w, c.h) == (1, 1)));
        }
    }

    /// Each terrain is drawn by exactly one family and each resource is in exactly one family's
    /// rows, so the order of the families never changes which rule wins.
    #[test]
    fn families_do_not_overlap() {
        for terrain in TERRAINS {
            let families =
                GROUND_FAMILIES.iter().filter(|f| f.iter().any(|r| r.terrains.contains(&terrain)));
            assert_eq!(families.count(), 1, "{terrain:?}");
        }
        for r in resource_rules() {
            let families =
                RESOURCE_FAMILIES.iter().filter(|f| f.iter().any(|o| o.resource == r.resource));
            assert_eq!(families.count(), 1, "{}", r.resource);
        }
    }
}
