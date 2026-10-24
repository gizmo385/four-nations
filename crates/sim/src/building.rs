//! Buildings: constructible, 1x1 structures (`data/buildings.toml`, issue #64).
//!
//! A `BuildingDef` just says what a building costs, how long it takes to build and whether units
//! may walk onto it once it's finished; everything about *designating* one, hauling its cost to
//! the site and the work that finishes it lives in `world/build.rs`. What a finished building
//! *does* is data on its def too:
//!
//! - `seats > 0` makes it a study hall: the moment one finishes construction `world/study.rs`
//!   opens that many `JobKind::Study` jobs on it (issue #21).
//! - `bed` makes it somewhere to sleep: a tired unit walks to the nearest free one and regains
//!   energy faster there (`world/work.rs`, issue #58).
//! - `heat` makes it a permanent heat source (`world/heat.rs`, issue #58): meals eaten within
//!   its radius are cooked, and sleep within it is better. It is the same `HeatSource` Fire's
//!   Hearthflame lights for a while, so the cooking and warm-sleep code is shared.
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

/// A building units sleep in (`[building.bed]`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BedDef {
    /// Energy regained asleep here, times the base rate (at least 1.0). Stacks with warmth.
    pub sleep_bonus: f32,
    /// A tired unit walks to a free bed within this many path tiles; further than that, it
    /// sleeps where it is.
    pub seek_tiles: u32,
}

/// A building that keeps a fire (`[building.heat]`). Unlike Hearthflame it never goes out.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HeatDef {
    /// Tiles around it, Chebyshev, that count as near it at a `heating` modifier of 1.0. The
    /// nation's `heating` modifier scales it (`HeatDef::radius_for`).
    pub radius: u32,
    /// Energy regained asleep nearby, times the base rate (at least 1.0).
    pub sleep_bonus: f32,
    /// A unit about to sleep walks to the warmth if it is within this many path tiles.
    pub seek_tiles: u32,
}

impl HeatDef {
    /// The radius at a nation's `heating` modifier: rounded, and never less than 1.
    pub fn radius_for(&self, heating: f32) -> u32 {
        ((self.radius as f32 * heating).round() as u32).max(1)
    }
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
    /// Set if units sleep here (issue #58).
    pub bed: Option<BedDef>,
    /// Set if this keeps a permanent fire: it cooks meals and warms sleep nearby (issue #58).
    pub heat: Option<HeatDef>,
    /// What each nation calls this building, if it varies (e.g. the study hall: Meditation
    /// Pavilion for Air). Empty when every nation uses the plain `name`. If set at all, every
    /// nation has an entry (`display_name` is the one place that matters).
    pub names: HashMap<Nation, String>,
    /// Which sprite of `art/buildings.toml` each nation draws this building with, if it varies
    /// (the study hall looks different in each nation). Empty when every nation uses the sprite
    /// named for the building's `id`. If set at all, every nation has an entry. The sim never
    /// reads it; the client does (`BuildingDef::art_for`).
    pub art: HashMap<Nation, String>,
    /// The tech that must be known before this building may be designated (`data/research.toml`,
    /// `Unlocks::buildings`, issue #22). `None` means every nation can build it from the start.
    pub unlocked_by: Option<TechId>,
}

impl BuildingDef {
    /// The name of the sprite this building is drawn with in `nation`: `art[nation]` if the
    /// building has per-nation art, else its `id`.
    pub fn art_for(&self, nation: Nation) -> &str {
        self.art.get(&nation).map(String::as_str).unwrap_or(&self.id)
    }

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
struct RawBed {
    sleep_bonus: f32,
    seek_tiles: u32,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawHeat {
    radius: u32,
    sleep_bonus: f32,
    seek_tiles: u32,
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
    bed: Option<RawBed>,
    heat: Option<RawHeat>,
    #[serde(default)]
    names: HashMap<Nation, String>,
    #[serde(default)]
    art: HashMap<Nation, String>,
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
            if !b.art.is_empty() && b.art.len() != Nation::ALL.len() {
                return err(format!("building {}: art must name all four nations, or none", b.id));
            }
            for (nation, art) in &b.art {
                if art.trim().is_empty() {
                    return err(format!(
                        "building {}: {} art must not be empty",
                        b.id,
                        nation.as_str()
                    ));
                }
            }
            let bonus_ok = |x: f32| x.is_finite() && x >= 1.0;
            if let Some(bed) = &b.bed
                && !(bonus_ok(bed.sleep_bonus) && bed.seek_tiles > 0)
            {
                return err(format!(
                    "building {}: bed needs sleep_bonus >= 1.0 and seek_tiles > 0",
                    b.id
                ));
            }
            if let Some(heat) = &b.heat
                && !(bonus_ok(heat.sleep_bonus) && heat.radius > 0 && heat.seek_tiles > 0)
            {
                return err(format!(
                    "building {}: heat needs radius > 0, sleep_bonus >= 1.0 and seek_tiles > 0",
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
                bed: b.bed.map(|d| BedDef { sleep_bonus: d.sleep_bonus, seek_tiles: d.seek_tiles }),
                heat: b.heat.map(|d| HeatDef {
                    radius: d.radius,
                    sleep_bonus: d.sleep_bonus,
                    seek_tiles: d.seek_tiles,
                }),
                names: b.names,
                art: b.art,
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

    #[test]
    fn beds_and_campfires_are_data() {
        let defs = Defs::bundled();
        let bed = defs.building(defs.building_id("bed").unwrap());
        let bedding = bed.bed.expect("a bed is a bed");
        assert!(bed.passable, "a unit lies on its bed");
        assert!(bedding.sleep_bonus > 1.0 && bedding.seek_tiles > 0);
        assert!(bed.heat.is_none());
        let fire = defs.building(defs.building_id("campfire").unwrap());
        let heat = fire.heat.expect("a campfire keeps a fire");
        assert!(fire.bed.is_none() && heat.sleep_bonus > 1.0);
        // The nation's `heating` modifier scales the reach, never below one tile.
        assert_eq!(heat.radius_for(1.0), heat.radius);
        assert!(heat.radius_for(2.5) > heat.radius);
        assert_eq!(heat.radius_for(0.01), 1);
    }

    #[test]
    fn bed_and_heat_numbers_are_validated() {
        let one = |extra: &str| {
            format!(
                "[[building]]\nid = \"hut\"\nname = \"Hut\"\nverb = \"Build\"\nwork = 1.0\n\
                 passable = true\n{extra}\n\
                 [[building]]\nid = \"campfire\"\nname = \"Campfire\"\nverb = \"Build\"\n\
                 work = 1.0\npassable = true\n"
            )
        };
        resolve(&one("[building.bed]\nsleep_bonus = 1.2\nseek_tiles = 5")).unwrap();
        let weak = resolve(&one("[building.bed]\nsleep_bonus = 0.5\nseek_tiles = 5"));
        assert!(weak.unwrap_err().0.contains("bed needs"));
        let none = resolve(&one("[building.bed]\nsleep_bonus = 1.2\nseek_tiles = 0"));
        assert!(none.unwrap_err().0.contains("bed needs"));
        let tiny = resolve(&one("[building.heat]\nradius = 0\nsleep_bonus = 1.2\nseek_tiles = 5"));
        assert!(tiny.unwrap_err().0.contains("heat needs"));
    }

    fn resolve(toml: &str) -> Result<Vec<BuildingDef>, DefsError> {
        let defs = Defs::bundled();
        // The bundled techs unlock the bundled buildings, which these small fixtures don't
        // define, so resolve them against a tree that unlocks no buildings.
        let mut techs = defs.techs.clone();
        for t in &mut techs {
            t.unlocks.buildings.clear();
        }
        toml::from_str::<BuildingsFile>(toml).unwrap().resolve(&defs.items, &techs)
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

    #[test]
    fn art_defaults_to_the_id_and_may_vary_per_nation() {
        let defs = Defs::bundled();
        let bed = defs.building(defs.building_id("bed").unwrap());
        let hall = defs.building(defs.building_id("study_hall").unwrap());
        for n in Nation::ALL {
            assert_eq!(bed.art_for(n), "bed");
            assert_ne!(hall.art_for(n), "study_hall", "{n:?} has its own study hall");
        }
        let mut looks: Vec<_> = Nation::ALL.iter().map(|&n| hall.art_for(n)).collect();
        looks.sort_unstable();
        looks.dedup();
        assert_eq!(looks.len(), 4, "one look per nation");
    }

    #[test]
    fn art_must_cover_every_nation_or_none() {
        let toml = r#"
            [[building]]
            id = "hut"
            name = "Hut"
            verb = "Build"
            work = 1.0
            passable = false
            [building.art]
            air = "hut_air"
        "#;
        let err = resolve(toml).unwrap_err();
        assert!(err.0.contains("art must name all four nations"), "{err}");
        let blank = toml.replace(
            "air = \"hut_air\"",
            "air = \" \"\nwater = \"w\"\nearth = \"e\"\nfire = \"f\"",
        );
        assert!(resolve(&blank).unwrap_err().0.contains("art must not be empty"));
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
