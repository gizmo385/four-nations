//! Buildings: constructible, 1x1 structures (`data/buildings.toml`, issue #64).
//!
//! A `BuildingDef` just says what a building costs, how long it takes to build and whether units
//! may walk onto it once it's finished; everything about *designating* one, hauling its cost to
//! the site and the work that finishes it lives in `world/build.rs`. What a finished building
//! *does* (sleeping, cooking...) is mostly follow-up work (#58 beds/hearth), except seats: a
//! building with `seats > 0` is a study hall, and the moment one finishes construction
//! `world/study.rs` opens that many `JobKind::Study` jobs on it (issue #21).

use std::collections::HashMap;

use serde::Deserialize;

use crate::defs::DefsError;
use crate::jobs::{ItemDef, ItemId};
use crate::nation::Nation;

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
    /// How many units may study here at once. 0 unless this is a study hall (issue #21); a
    /// nonzero value is what tells `world/build.rs` to open that many `JobKind::Study` jobs when
    /// construction finishes.
    pub seats: u32,
    /// What each nation calls this building, if it varies (e.g. the study hall: Meditation
    /// Pavilion for Air). Empty when every nation uses the plain `name`. If set at all, every
    /// nation has an entry (`display_name` is the one place that matters).
    pub names: HashMap<Nation, String>,
}

impl BuildingDef {
    /// `names[nation]` if this building has a nation-specific name, else the shared `name`.
    pub fn display_name(&self, nation: Nation) -> &str {
        self.names.get(&nation).map(String::as_str).unwrap_or(&self.name)
    }
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
    #[serde(default)]
    seats: u32,
    #[serde(default)]
    names: HashMap<Nation, String>,
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
            if !b.names.is_empty() && b.names.len() != Nation::ALL.len() {
                return err(format!(
                    "building {}: names must name all four nations, or none",
                    b.id
                ));
            }
            for (nation, name) in &b.names {
                if name.trim().is_empty() {
                    return err(format!(
                        "building {}: {} name must not be empty",
                        b.id,
                        nation.as_str()
                    ));
                }
            }
            buildings.push(BuildingDef {
                id: b.id,
                name: b.name,
                verb: b.verb,
                cost,
                work: b.work,
                passable: b.passable,
                seats: b.seats,
                names: b.names,
            });
        }
        Ok(buildings)
    }
}

#[cfg(test)]
mod tests {
    use crate::defs::{Defs, DefsError};
    use crate::nation::Nation;

    use super::{BuildingDef, BuildingsFile};

    #[test]
    fn bundled_buildings_are_valid() {
        let defs = Defs::bundled();
        assert!(!defs.buildings.is_empty());
        let bed = defs.building_id("bed").expect("starter building");
        let def = defs.building(bed);
        assert!(def.work > 0.0);
        assert!(!def.cost.is_empty());
    }

    #[test]
    fn study_hall_has_seats_and_a_name_per_nation() {
        let defs = Defs::bundled();
        let hall = defs.building_id("study_hall").expect("study hall");
        let def = defs.building(hall);
        assert!(def.seats > 0);
        for n in Nation::ALL {
            assert_ne!(def.display_name(n), def.name, "{n:?} should have its own name");
        }
    }

    #[test]
    fn a_building_without_nation_names_falls_back_to_its_plain_name() {
        let defs = Defs::bundled();
        let bed = defs.building_id("bed").unwrap();
        let def = defs.building(bed);
        assert_eq!(def.seats, 0);
        for n in Nation::ALL {
            assert_eq!(def.display_name(n), def.name);
        }
    }

    fn resolve(toml: &str) -> Result<Vec<BuildingDef>, DefsError> {
        let items = Defs::bundled().items;
        toml::from_str::<BuildingsFile>(toml).unwrap().resolve(&items)
    }

    #[test]
    fn names_must_cover_every_nation_or_none() {
        let toml = r#"
            [[building]]
            id = "hut"
            name = "Hut"
            verb = "Build"
            work = 1.0
            passable = false
            [building.names]
            air = "Shelter"
        "#;
        let err = resolve(toml).unwrap_err();
        assert!(err.0.contains("all four nations"), "{err}");
    }

    #[test]
    fn a_blank_nation_name_is_rejected() {
        let toml = r#"
            [[building]]
            id = "hut"
            name = "Hut"
            verb = "Build"
            work = 1.0
            passable = false
            [building.names]
            air = "  "
            water = "W"
            earth = "E"
            fire = "F"
        "#;
        let err = resolve(toml).unwrap_err();
        assert!(err.0.contains("must not be empty"), "{err}");
    }
}
