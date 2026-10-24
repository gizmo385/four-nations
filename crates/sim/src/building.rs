//! Buildings: constructible, 1x1 structures (`data/buildings.toml`, issue #64).
//!
//! A `BuildingDef` just says what a building costs, how long it takes to build and whether units
//! may walk onto it once it's finished; everything about *designating* one, hauling its cost to
//! the site and the work that finishes it lives in `world/build.rs`. What a finished building
//! *does* (sleeping, cooking...) is mostly follow-up work (#58 beds/hearth), except seats: a
//! building with `seats > 0` is a study hall, and the moment one finishes construction
//! `world/study.rs` opens that many `JobKind::Study` jobs on it (issue #21).
//!
//! Which tech must be known before a building may be designated is `Unlocks::buildings` in
//! `data/research.toml`, resolved here into `BuildingDef::unlocked_by` (issue #22). A building no
//! tech names is available from the start.

use std::collections::HashMap;

use serde::Deserialize;

use crate::defs::DefsError;
use crate::jobs::{ItemDef, ItemId};
use crate::nation::Nation;
use crate::research::{TechDef, TechId};

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, serde::Serialize, serde::Deserialize,
)]
pub struct BuildingId(pub u16);

/// A building placed on a tile (`Tile::building`): baked onto the tile so pathfinding
/// (`path.rs`, `flood.rs`) never has to consult the defs to know whether it blocks movement.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
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
    /// The tech that must be known before this building may be designated (`data/research.toml`,
    /// `Unlocks::buildings`, issue #22). `None` means every nation can build it from the start.
    pub unlocked_by: Option<TechId>,
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
    /// Checks the data, links item names to ids, and records which tech unlocks each building.
    pub(crate) fn resolve(
        self,
        items: &[ItemDef],
        techs: &[TechDef],
    ) -> Result<Vec<BuildingDef>, DefsError> {
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
                unlocked_by: None,
            });
        }
        link_unlocks(&mut buildings, techs)?;
        Ok(buildings)
    }
}

/// Resolves every tech's `unlocks.buildings` into `BuildingDef::unlocked_by`: the building must
/// exist and be unlocked by no other tech. Unlike abilities, a building isn't owned by one
/// nation, so any tech (shared or a nation's own) may unlock any building.
fn link_unlocks(buildings: &mut [BuildingDef], techs: &[TechDef]) -> Result<(), DefsError> {
    for (t, tech) in techs.iter().enumerate() {
        let err =
            |msg: String| Err(DefsError(format!("tech {}: unlocks.buildings: {msg}", tech.id)));
        for name in &tech.unlocks.buildings {
            let Some(b) = buildings.iter_mut().find(|b| b.id == *name) else {
                return err(format!("unknown building {name:?}"));
            };
            if let Some(other) = b.unlocked_by {
                return if other == TechId(t as u16) {
                    err(format!("{name:?} listed twice"))
                } else {
                    err(format!(
                        "{name:?} is already unlocked by {:?}; a building has one tech",
                        techs[other.0 as usize].id
                    ))
                };
            }
            b.unlocked_by = Some(TechId(t as u16));
        }
    }
    Ok(())
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
        let defs = Defs::bundled();
        toml::from_str::<BuildingsFile>(toml).unwrap().resolve(&defs.items, &defs.techs)
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

    const BIOMES: &str = include_str!("../../../data/biomes.toml");
    const RESOURCES: &str = include_str!("../../../data/resources.toml");
    const NATIONS: &str = include_str!("../../../data/nations.toml");
    const RESEARCH: &str = include_str!("../../../data/research.toml");
    const NEEDS: &str = include_str!("../../../data/needs.toml");
    const JOBS: &str = include_str!("../../../data/jobs.toml");
    const BENDING: &str = include_str!("../../../data/bending.toml");
    const BUILDINGS: &str = include_str!("../../../data/buildings.toml");

    /// The bundled data, with `tech` appended to `data/research.toml`.
    fn load(tech: &str) -> Result<Defs, DefsError> {
        Defs::from_toml(
            BIOMES,
            RESOURCES,
            NATIONS,
            &format!("{RESEARCH}\n{tech}"),
            NEEDS,
            JOBS,
            BENDING,
            BUILDINGS,
        )
    }

    /// A tech named `id` that unlocks `buildings` (a TOML array literal, e.g. `"['bed']"`).
    fn tech(id: &str, buildings: &str) -> String {
        format!(
            "[[tech]]\nid = '{id}'\nname = 'T'\ndescription = ''\ncost = 1\n\
             [tech.unlocks]\nbuildings = {buildings}\n"
        )
    }

    #[test]
    fn a_tech_records_the_building_it_unlocks() {
        let defs = Defs::bundled();
        let fire_making = defs.tech_id("fire_making").unwrap();
        let campfire = defs.building_id("campfire").expect("added by issue #22");
        assert_eq!(defs.building(campfire).unlocked_by, Some(fire_making));
        let bed = defs.building_id("bed").unwrap();
        assert_eq!(defs.building(bed).unlocked_by, None);
    }

    #[test]
    fn a_tech_may_only_unlock_buildings_that_exist() {
        let err = load(&tech("a", "['nope']")).unwrap_err();
        assert!(err.0.contains("unknown building \"nope\""), "{err}");
    }

    #[test]
    fn a_building_has_at_most_one_tech() {
        let two = load(&format!("{}{}", tech("a", "['bed']"), tech("b", "['bed']")));
        assert!(two.unwrap_err().0.contains("one tech"));
        let twice = load(&tech("a", "['bed', 'bed']"));
        assert!(twice.unwrap_err().0.contains("listed twice"));
    }
}
