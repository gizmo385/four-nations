//! Buildings: constructible, 1x1 structures (`data/buildings.toml`, issue #64).
//!
//! A `BuildingDef` just says what a building costs, how long it takes to build and whether units
//! may walk onto it once it's finished; everything about *designating* one, hauling its cost to
//! the site and the work that finishes it lives in `world/build.rs`. What a finished building
//! *does* (sleeping, cooking...) is follow-up work (#58 beds/hearth): nothing here assumes it.

use serde::Deserialize;

use crate::defs::DefsError;
use crate::jobs::{ItemDef, ItemId};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct BuildingId(pub u16);

/// A building placed on a tile (`Tile::building`): baked onto the tile so pathfinding
/// (`path.rs`, `flood.rs`) never has to consult the defs to know whether it blocks movement.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Building {
    pub id: BuildingId,
    /// Whether units may walk onto this tile now that the building stands on it.
    pub passable: bool,
}

/// One item and the count of it a building costs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CostItem {
    pub item: ItemId,
    pub count: u32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct BuildingDef {
    pub id: String,
    pub name: String,
    /// What the designation is called in the UI ("Build").
    pub verb: String,
    /// What a construct job hauls to the site before work can start.
    pub cost: Vec<CostItem>,
    /// In-game seconds of work at a `construction` modifier of 1.0.
    pub work: f32,
    /// Whether units may walk onto the tile once the building is finished.
    pub passable: bool,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawCost {
    item: String,
    count: u32,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawBuilding {
    id: String,
    name: String,
    verb: String,
    #[serde(default)]
    cost: Vec<RawCost>,
    work: f32,
    passable: bool,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct BuildingsFile {
    building: Vec<RawBuilding>,
}

impl BuildingsFile {
    /// Checks the data and links item names to ids.
    pub(crate) fn resolve(self, items: &[ItemDef]) -> Result<Vec<BuildingDef>, DefsError> {
        let err = |msg: String| Err(DefsError(format!("buildings.toml: {msg}")));
        let mut buildings: Vec<BuildingDef> = Vec::new();
        for b in self.building {
            if buildings.iter().any(|d| d.id == b.id) {
                return err(format!("duplicate building {}", b.id));
            }
            if !(b.work.is_finite() && b.work > 0.0) {
                return err(format!("building {}: work must be a positive number", b.id));
            }
            let mut cost = Vec::new();
            for c in b.cost {
                let Some(item) = items.iter().position(|i| i.id == c.item) else {
                    return err(format!("building {}: unknown item {:?}", b.id, c.item));
                };
                if c.count == 0 {
                    return err(format!("building {}: cost count must be positive", b.id));
                }
                if cost.iter().any(|existing: &CostItem| existing.item.0 as usize == item) {
                    return err(format!("building {}: item {:?} costed twice", b.id, c.item));
                }
                cost.push(CostItem { item: ItemId(item as u16), count: c.count });
            }
            buildings.push(BuildingDef {
                id: b.id,
                name: b.name,
                verb: b.verb,
                cost,
                work: b.work,
                passable: b.passable,
            });
        }
        Ok(buildings)
    }
}

#[cfg(test)]
mod tests {
    use crate::defs::Defs;

    #[test]
    fn bundled_buildings_are_valid() {
        let defs = Defs::bundled();
        assert!(!defs.buildings.is_empty());
        let bed = defs.building_id("bed").expect("starter building");
        let def = defs.building(bed);
        assert!(def.work > 0.0);
        assert!(!def.cost.is_empty());
    }
}
