//! Land and shore floors (#145): textured 16px tiles from `art/floors.toml`, four variants each,
//! so a plain is grass with blades and flowers and not one flat colour. The rules (variant 0 is
//! quiet, at most one variant has a loud mark, marks stay 1px off the edge) are in
//! docs/design/pixel-art.md, "Land tiles".

use fn_sim::{Pos, Terrain};

use super::{Cell, GroundRule, one, tile_hash};

/// The sheet the floor art is on (`art/floors.toml`, drawn by `cargo xtask art`): one floor per
/// row and its four variants across, in the order of the sprites in the TOML.
pub const SHEET: &str = "original/floors.png";

/// How many variants every floor has.
const VARIANTS: usize = 4;

/// The chance of each variant, as weights: the two quiet ones (0 and 1) carry 11 tiles in 16, and
/// the loudest (3, the flower or the puddle) one in eight, so no mark repeats into a pattern.
const WEIGHTS: [u32; VARIANTS] = [8, 3, 3, 2];

/// A floor's variants: row `row` of the sheet.
const fn floor(row: u32) -> [Cell; VARIANTS] {
    [one(SHEET, 0, row), one(SHEET, 1, row), one(SHEET, 2, row), one(SHEET, 3, row)]
}

/// The floors, by their row on the sheet. Each has a test that names its sprites in the TOML.
const GRASS: [Cell; VARIANTS] = floor(0);
const FOREST: [Cell; VARIANTS] = floor(1);
const SAND: [Cell; VARIANTS] = floor(2);
const SNOW: [Cell; VARIANTS] = floor(3);
const MUD: [Cell; VARIANTS] = floor(4);
/// The flat interior of a mountain. The mountain rows are the cliffs family's (#147).
#[allow(dead_code)]
pub const ROCK: [Cell; VARIANTS] = floor(5);

const SHORES: &[Terrain] = &[Terrain::Coast, Terrain::Ford];
const LAND: &[Terrain] = &[Terrain::Land];

impl GroundRule {
    /// The floor variant for the tile at `p`: a weighted pick by `tile_hash(p, 1)`, so a tile
    /// always looks the same, and so does a whole map from one seed. A rule with no variants is
    /// its `base`.
    pub fn floor_at(&self, p: Pos) -> Cell {
        if self.variants.is_empty() {
            return self.base;
        }
        let weights = &WEIGHTS[..self.variants.len().min(VARIANTS)];
        let mut roll = tile_hash(p, 1) % weights.iter().sum::<u32>();
        for (cell, weight) in self.variants.iter().zip(weights) {
            if roll < *weight {
                return *cell;
            }
            roll -= weight;
        }
        self.base
    }
}

pub const GROUND: &[GroundRule] = &[
    // Fords share the shore tile until autotiling can draw a crossing.
    GroundRule {
        terrains: SHORES,
        biome: Some("tundra"),
        base: SNOW[0],
        variants: &SNOW,
        decor: None,
        water: None,
        cliff: None,
    },
    GroundRule {
        terrains: SHORES,
        biome: None,
        base: SAND[0],
        variants: &SAND,
        decor: None,
        water: None,
        cliff: None,
    },
    GroundRule {
        terrains: LAND,
        biome: Some("desert"),
        base: SAND[0],
        variants: &SAND,
        decor: None,
        water: None,
        cliff: None,
    },
    GroundRule {
        terrains: LAND,
        biome: Some("tundra"),
        base: SNOW[0],
        variants: &SNOW,
        decor: None,
        water: None,
        cliff: None,
    },
    GroundRule {
        terrains: LAND,
        biome: Some("swamp"),
        base: MUD[0],
        variants: &MUD,
        decor: None,
        water: None,
        cliff: None,
    },
    GroundRule {
        terrains: LAND,
        biome: Some("forest"),
        base: FOREST[0],
        variants: &FOREST,
        decor: None,
        water: None,
        cliff: None,
    },
    GroundRule {
        terrains: LAND,
        biome: Some("taiga"),
        base: FOREST[0],
        variants: &FOREST,
        decor: None,
        water: None,
        cliff: None,
    },
    GroundRule {
        terrains: LAND,
        biome: None,
        base: GRASS[0],
        variants: &GRASS,
        decor: None,
        water: None,
        cliff: None,
    },
];

#[cfg(test)]
mod tests {
    use super::super::ground_rule;
    use super::*;

    /// The sprite names of `art/floors.toml` in file order.
    fn sprite_names() -> Vec<String> {
        let path = format!("{}/../../art/floors.toml", env!("CARGO_MANIFEST_DIR"));
        let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{path}: {e}"));
        let doc: toml::Table = text.parse().unwrap();
        doc["sprite"]
            .as_array()
            .unwrap()
            .iter()
            .map(|s| s["name"].as_str().unwrap().into())
            .collect()
    }

    #[test]
    fn every_floor_cell_is_the_sprite_it_claims() {
        let names = sprite_names();
        assert_eq!(names.len(), 6 * VARIANTS);
        let floors = [
            ("grass", GRASS),
            ("forest_floor", FOREST),
            ("sand", SAND),
            ("snow", SNOW),
            ("mud", MUD),
            ("rock", ROCK),
        ];
        for (prefix, cells) in floors {
            for (v, cell) in cells.iter().enumerate() {
                assert_eq!(cell.sheet, SHEET);
                // `columns = 4` in the TOML: sprite n is at column n % 4, row n / 4.
                let index = (cell.row as usize) * VARIANTS + cell.col as usize;
                assert_eq!(names[index], format!("{prefix}{v}"));
                assert_eq!(cell.col as usize, v);
            }
        }
    }

    #[test]
    fn the_sheet_is_four_variants_by_six_floors() {
        assert_eq!(super::super::test_support::png_size(SHEET), (4 * 16, 6 * 16));
    }

    #[test]
    fn variant_zero_is_the_base_and_every_floor_has_four() {
        for rule in GROUND {
            assert_eq!(rule.variants.len(), VARIANTS);
            assert_eq!(rule.variants[0], rule.base);
        }
    }

    #[test]
    fn the_pick_is_pinned() {
        let rule = ground_rule(Terrain::Land, "plains").unwrap();
        let picks = |at: &dyn Fn(i32) -> Pos| -> Vec<usize> {
            (0..24)
                .map(|i| rule.variants.iter().position(|c| *c == rule.floor_at(at(i))).unwrap())
                .collect()
        };
        assert_eq!(
            picks(&|x| Pos::new(x, 0)),
            [0, 0, 0, 1, 2, 1, 0, 0, 0, 0, 0, 1, 1, 1, 0, 0, 0, 3, 0, 1, 0, 0, 2, 0]
        );
        assert_eq!(
            picks(&|y| Pos::new(5, y)),
            [1, 3, 2, 0, 2, 0, 0, 0, 2, 0, 3, 0, 1, 0, 0, 0, 0, 1, 3, 3, 0, 2, 0, 1]
        );
    }

    #[test]
    fn every_variant_appears_in_roughly_its_weight() {
        for (terrain, biome) in [
            (Terrain::Land, "plains"),
            (Terrain::Land, "forest"),
            (Terrain::Land, "desert"),
            (Terrain::Land, "tundra"),
            (Terrain::Land, "swamp"),
            (Terrain::Coast, "plains"),
            (Terrain::Ford, "tundra"),
        ] {
            let rule = ground_rule(terrain, biome).unwrap();
            let mut seen = [0u32; VARIANTS];
            for y in 0..64 {
                for x in 0..64 {
                    let cell = rule.floor_at(Pos::new(x, y));
                    seen[rule.variants.iter().position(|c| *c == cell).unwrap()] += 1;
                }
            }
            let total: u32 = WEIGHTS.iter().sum();
            for (v, n) in seen.iter().enumerate() {
                let expect = 64 * 64 * WEIGHTS[v] / total;
                assert!(
                    n.abs_diff(expect) < expect / 5,
                    "{terrain:?} {biome}: variant {v} on {n} of 4096 tiles, expected about {expect}"
                );
            }
        }
    }

    #[test]
    fn the_shore_and_the_land_of_a_biome_share_a_floor() {
        let land = ground_rule(Terrain::Land, "desert").unwrap();
        assert_eq!(ground_rule(Terrain::Coast, "desert").unwrap().variants, land.variants);
        assert_eq!(ground_rule(Terrain::Ford, "swamp").unwrap().variants, land.variants);
        let tundra = ground_rule(Terrain::Land, "tundra").unwrap();
        assert_eq!(ground_rule(Terrain::Coast, "tundra").unwrap().variants, tundra.variants);
    }
}
