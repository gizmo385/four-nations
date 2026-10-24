//! Land and shore floors: one flat base per biome, with variants so it doesn't look tiled.

use fn_sim::{Pos, Terrain};

use super::{Cell, GroundRule, one, tile_hash};

/// The tileset sheet the floor art is on.
pub const SHEET: &str = "third_party/ninja-adventure/Backgrounds/Tilesets/TilesetFloor.png";

const SAND: Cell = one(SHEET, 1, 1);
const SNOW: Cell = one(SHEET, 1, 15);
const MUD: Cell = one(SHEET, 12, 15);
const LIGHT_GRASS: Cell = one(SHEET, 0, 12);
const DARK_GRASS: Cell = one(SHEET, 11, 12);

const SAND_VARIANTS: &[Cell] = &[one(SHEET, 0, 4), one(SHEET, 1, 4)];
const MUD_VARIANTS: &[Cell] = &[one(SHEET, 13, 19), one(SHEET, 14, 19)];
const LIGHT_GRASS_VARIANTS: &[Cell] =
    &[one(SHEET, 1, 12), one(SHEET, 2, 12), one(SHEET, 3, 12)];
const DARK_GRASS_VARIANTS: &[Cell] = &[one(SHEET, 12, 12), one(SHEET, 13, 12)];

const SHORES: &[Terrain] = &[Terrain::Coast, Terrain::Ford];
const LAND: &[Terrain] = &[Terrain::Land];

impl GroundRule {
    /// The floor cell for the tile at `p`.
    pub fn floor_at(&self, p: Pos) -> Cell {
        let h = tile_hash(p, 1);
        if self.variants.is_empty() || !h.is_multiple_of(4) {
            return self.base;
        }
        self.variants[(h >> 8) as usize % self.variants.len()]
    }
}

pub const GROUND: &[GroundRule] = &[
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

#[cfg(test)]
mod tests {
    use super::super::ground_rule;
    use super::*;

    #[test]
    fn variation_is_stable_and_mixed() {
        let rule = ground_rule(Terrain::Land, "plains").unwrap();
        let picks: Vec<Cell> = (0..400).map(|i| rule.floor_at(Pos::new(i % 20, i / 20))).collect();
        assert_eq!(picks[7], rule.floor_at(Pos::new(7, 0)));
        let plain = picks.iter().filter(|c| **c == rule.base).count();
        assert!((250..350).contains(&plain), "about three in four tiles are plain: {plain}");
    }
}
