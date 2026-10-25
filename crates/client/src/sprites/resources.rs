//! Resources that aren't trees: one-cell sprites, bottom-aligned to their tile. The rocks and the
//! cabbage are Claude-authored (`art/resources.toml`, docs/design/pixel-art.md). The fish spot is
//! drawn on the water sheet (`art/water.toml`, #146), so it comes from `sprites::water`.

use super::{ResourceRule, one};

/// The sheet the rocks and crops are on, rendered by `cargo xtask art`.
pub const SHEET: &str = "original/resources.png";

// The columns of `SHEET`, in the order of the sprites in `art/resources.toml`.
const STONE: u32 = 0;
const CABBAGE: u32 = 1;
const GOLD_VEIN: u32 = 2;
const ANCIENT_SCROLL: u32 = 3;
const SAPLING_COL: u32 = 4;
const GOLD_SEAM: u32 = 5;

/// A fruit tree an orchard has just planted, drawn in place of the fruit tree until it first
/// ripens (`World::is_sapling`, #142). A resource's own sprite is its rule; this belongs to a
/// state of one.
pub const SAPLING: super::Cell = one(SHEET, SAPLING_COL, 0);

pub const RESOURCES: &[ResourceRule] = &[
    // Grey rock, and the same rock with gold in it: one silhouette.
    ResourceRule { resource: "stone", biome: None, cell: one(SHEET, STONE, 0) },
    ResourceRule { resource: "gold_vein", biome: None, cell: one(SHEET, GOLD_VEIN, 0) },
    // The same rock split open on a thick band of gold: a seam Seismic Sense uncovered (#133).
    ResourceRule { resource: "gold_seam", biome: None, cell: one(SHEET, GOLD_SEAM, 0) },
    ResourceRule { resource: "cabbage", biome: None, cell: one(SHEET, CABBAGE, 0) },
    // A rolled-up scroll lying where the ruins were (#24).
    ResourceRule { resource: "ancient_scroll", biome: None, cell: one(SHEET, ANCIENT_SCROLL, 0) },
    // Fish shadows and ripples, on a transparent background over the water tile (#146).
    ResourceRule { resource: "fish_spot", biome: None, cell: super::water::FISH_SPOT },
];

#[cfg(test)]
mod tests {
    use super::*;
    use fn_sim::{Defs, Terrain};

    /// The sprite names of `art/resources.toml` in file order.
    fn sprite_names() -> Vec<String> {
        let path = format!("{}/../../art/resources.toml", env!("CARGO_MANIFEST_DIR"));
        let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{path}: {e}"));
        let doc: toml::Table = text.parse().unwrap();
        let sprites = doc["sprite"].as_array().unwrap();
        sprites.iter().map(|s| s["name"].as_str().unwrap().to_owned()).collect()
    }

    #[test]
    fn the_rest_are_one_cell() {
        for r in RESOURCES {
            assert!(!r.resource.ends_with("_tree"), "{}", r.resource);
            assert_eq!((r.cell.w, r.cell.h), (1, 1), "{}", r.resource);
        }
    }

    #[test]
    fn the_cells_are_the_sprites_they_claim() {
        let names = sprite_names();
        assert_eq!(names[STONE as usize], "stone");
        assert_eq!(names[GOLD_VEIN as usize], "gold_vein");
        assert_eq!(names[CABBAGE as usize], "cabbage");
        assert_eq!(names[ANCIENT_SCROLL as usize], "ancient_scroll");
        assert_eq!(names[SAPLING_COL as usize], "sapling");
        assert_eq!(names[GOLD_SEAM as usize], "gold_seam");
    }

    /// Every land resource that isn't a tree has a sprite on our sheet, named like the resource.
    #[test]
    fn every_land_resource_has_an_authored_sprite() {
        let names = sprite_names();
        let on_water = |t: &Terrain| matches!(t, Terrain::Water | Terrain::Lake | Terrain::River);
        for res in &Defs::bundled().resources {
            let wet = res.terrains.as_ref().is_some_and(|ts| ts.iter().any(on_water));
            if res.id.ends_with("_tree") || wet {
                continue;
            }
            let rule = RESOURCES
                .iter()
                .find(|r| r.resource == res.id)
                .unwrap_or_else(|| panic!("no sprite for land resource {}", res.id));
            assert_eq!(rule.cell.sheet, SHEET, "{}", res.id);
            assert_eq!(names[rule.cell.col as usize], res.id);
        }
    }
}
