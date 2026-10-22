//! Static game definitions loaded from the TOML files in `data/`.
//!
//! Definitions are referenced at runtime by small index newtypes (`BiomeId`, `ResourceId`,
//! `TechId`) so that map cells stay `Copy` and cheap. Validation happens once, at load time, and
//! is strict (unknown fields, out-of-range numbers and climate gaps are all errors) so that a
//! typo in the data fails `cargo test` instead of silently changing the game.

use serde::Deserialize;

use crate::jobs::{HarvestDef, ItemDef, ItemId, JobSettings, JobsFile};
use crate::nation::{Nation, NationDef};
use crate::needs::{FoodDef, NeedsFile, NeedsSettings};
use crate::research::{self, ResearchFile, ResearchSettings, TechDef, TechId};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Terrain {
    /// Sea: everything below sea level.
    Water,
    Coast,
    Land,
    Mountain,
    /// Fresh water flowing from the highlands to the sea. One tile wide.
    River,
    /// Fresh water standing in a filled depression.
    Lake,
    /// A shallow, walkable crossing where mapgen cut a river (issue #30). Replaces a single
    /// river tile so land on both banks stays connected.
    Ford,
}

impl Terrain {
    /// Any kind of water: sea, river, lake or ford (shallow water you can wade).
    pub fn is_water(self) -> bool {
        matches!(self, Terrain::Water | Terrain::River | Terrain::Lake | Terrain::Ford)
    }

    /// Standing water that gets a shoreline (`Coast`) around it. Rivers don't.
    pub fn has_shore(self) -> bool {
        matches!(self, Terrain::Water | Terrain::Lake)
    }

    pub fn is_walkable(self) -> bool {
        matches!(self, Terrain::Coast | Terrain::Land | Terrain::Ford)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct BiomeId(pub u8);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ResourceId(pub u16);

/// A biome and the climate it occupies. Mapgen picks the *first* biome (in file order) whose
/// inclusive `temperature` and `moisture` ranges contain a tile's climate, Whittaker-style.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BiomeDef {
    pub id: String,
    pub name: String,
    pub glyph: char,
    /// `[min, max]` within `[0, 1]`; 0 is polar cold, 1 is equatorial heat.
    pub temperature: [f64; 2],
    /// `[min, max]` within `[0, 1]`; 0 is arid, 1 is saturated.
    pub moisture: [f64; 2],
}

impl BiomeDef {
    pub fn contains(&self, temperature: f64, moisture: f64) -> bool {
        let [t0, t1] = self.temperature;
        let [m0, m1] = self.moisture;
        (t0..=t1).contains(&temperature) && (m0..=m1).contains(&moisture)
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ResourceDef {
    pub id: String,
    pub name: String,
    pub glyph: char,
    /// Base probability in `[0, 1]` that this resource spawns on an eligible tile.
    pub chance: f64,
    /// Terrains this may spawn on; `None` means any.
    pub terrains: Option<Vec<Terrain>>,
    /// Biome ids this may spawn in; `None` means any.
    pub biomes: Option<Vec<String>>,
    /// In `[0, 1]`. 0 rolls every tile independently; 1 makes the chance follow a noise field,
    /// so the resource grows in clumps (groves, outcrops) with gaps between. Default 0.
    #[serde(default)]
    pub clustering: f64,
    /// If true, the chance scales with tile moisture (x0.5 when arid, up to x1.5 when wet).
    #[serde(default)]
    pub moisture_scaled: bool,
    /// Minimum Chebyshev distance between two tiles of this resource, for spaced deposits.
    #[serde(default)]
    pub min_spacing: Option<u32>,
}

#[derive(Debug, Clone)]
pub struct Defs {
    pub biomes: Vec<BiomeDef>,
    pub resources: Vec<ResourceDef>,
    pub nations: Vec<NationDef>,
    /// The tech tree, in file order (a topological order); indexed by `TechId`.
    pub techs: Vec<TechDef>,
    pub research: ResearchSettings,
    /// Hunger, thirst, energy and health rates (data/needs.toml).
    pub needs: NeedsSettings,
    /// Edible foods, in file order.
    pub foods: Vec<FoodDef>,
    /// Everything that can be carried and stockpiled: the materials in data/jobs.toml, then
    /// every food. Indexed by `ItemId`.
    pub items: Vec<ItemDef>,
    /// What working each harvestable resource yields (data/jobs.toml).
    pub harvests: Vec<HarvestDef>,
    pub jobs: JobSettings,
    /// `harvests` index by `ResourceId`, so the lookup is O(1).
    harvest_by_resource: Vec<Option<usize>>,
}

#[derive(Debug, PartialEq)]
pub struct DefsError(pub String);

impl std::fmt::Display for DefsError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "invalid game data: {}", self.0)
    }
}

impl std::error::Error for DefsError {}

#[derive(Deserialize)]
struct BiomeFile {
    biome: Vec<BiomeDef>,
}

#[derive(Deserialize)]
struct ResourceFile {
    resource: Vec<ResourceDef>,
}

#[derive(Deserialize)]
struct NationFile {
    nation: Vec<NationDef>,
}

fn parse<T: for<'de> Deserialize<'de>>(name: &str, src: &str) -> Result<T, DefsError> {
    toml::from_str(src).map_err(|e| DefsError(format!("{name}: {e}")))
}

/// Maps a quantized climate value (`Tile::temperature`, `Tile::moisture`) to `[0, 1]`.
pub fn climate_unit(v: u8) -> f64 {
    f64::from(v) / 255.0
}

impl Defs {
    /// The definitions shipped in `data/`, embedded at compile time.
    pub fn bundled() -> Self {
        Self::from_toml(
            include_str!("../../../data/biomes.toml"),
            include_str!("../../../data/resources.toml"),
            include_str!("../../../data/nations.toml"),
            include_str!("../../../data/research.toml"),
            include_str!("../../../data/needs.toml"),
            include_str!("../../../data/jobs.toml"),
        )
        .expect("bundled data/*.toml must be valid; run `cargo test -p fn_sim`")
    }

    pub fn from_toml(
        biomes: &str,
        resources: &str,
        nations: &str,
        research: &str,
        needs: &str,
        jobs: &str,
    ) -> Result<Self, DefsError> {
        let (research, techs) = parse::<ResearchFile>("research.toml", research)?.resolve()?;
        let mut nations = parse::<NationFile>("nations.toml", nations)?.nation;
        for n in &mut nations {
            n.starting_techs =
                research::resolve_starting_techs(n.id, &n.starting_tech_ids, &techs)?;
        }
        let resources = parse::<ResourceFile>("resources.toml", resources)?.resource;
        let resource_ids: Vec<&str> = resources.iter().map(|r| r.id.as_str()).collect();
        let (needs, foods) = parse::<NeedsFile>("needs.toml", needs)?.resolve(&resource_ids)?;
        let (jobs, items, harvests) =
            parse::<JobsFile>("jobs.toml", jobs)?.resolve(&resources, &foods, &needs)?;
        let mut harvest_by_resource = vec![None; resources.len()];
        for (i, h) in harvests.iter().enumerate() {
            harvest_by_resource[h.resource.0 as usize] = Some(i);
        }
        let defs = Self {
            biomes: parse::<BiomeFile>("biomes.toml", biomes)?.biome,
            resources,
            nations,
            techs,
            research,
            needs,
            foods,
            items,
            harvests,
            jobs,
            harvest_by_resource,
        };
        defs.validate()?;
        Ok(defs)
    }

    fn validate(&self) -> Result<(), DefsError> {
        if self.biomes.is_empty() || self.biomes.len() > u8::MAX as usize {
            return Err(DefsError(format!("need 1..=255 biomes, got {}", self.biomes.len())));
        }
        for b in &self.biomes {
            for (what, [lo, hi]) in [("temperature", b.temperature), ("moisture", b.moisture)] {
                if !(0.0 <= lo && lo <= hi && hi <= 1.0) {
                    return Err(DefsError(format!(
                        "biome {}: {what} must be [min, max] with 0 <= min <= max <= 1",
                        b.id
                    )));
                }
            }
        }
        // Every climate a tile can have must map to a biome: check the whole quantized grid.
        for t in 0..=u8::MAX {
            for m in 0..=u8::MAX {
                if self.try_classify_biome(t, m).is_none() {
                    return Err(DefsError(format!(
                        "no biome covers temperature {:.3}, moisture {:.3}; add a catch-all",
                        climate_unit(t),
                        climate_unit(m)
                    )));
                }
            }
        }
        for r in &self.resources {
            if !(0.0..=1.0).contains(&r.chance) {
                return Err(DefsError(format!("resource {}: chance must be in [0, 1]", r.id)));
            }
            if !(0.0..=1.0).contains(&r.clustering) {
                return Err(DefsError(format!("resource {}: clustering must be in [0, 1]", r.id)));
            }
            if r.min_spacing == Some(0) {
                return Err(DefsError(format!("resource {}: min_spacing must be >= 1", r.id)));
            }
            for b in r.biomes.iter().flatten() {
                if self.biome_id(b).is_none() {
                    return Err(DefsError(format!("resource {}: unknown biome {b:?}", r.id)));
                }
            }
        }
        for n in Nation::ALL {
            match self.nations.iter().find(|d| d.id == n) {
                None => return Err(DefsError(format!("nations.toml is missing {n:?}"))),
                Some(d) if d.names.is_empty() => {
                    return Err(DefsError(format!("nation {n:?} needs at least one name")));
                }
                Some(_) => {}
            }
        }
        Ok(())
    }

    fn try_classify_biome(&self, temperature: u8, moisture: u8) -> Option<BiomeId> {
        let (t, m) = (climate_unit(temperature), climate_unit(moisture));
        self.biomes.iter().position(|b| b.contains(t, m)).map(|i| BiomeId(i as u8))
    }

    /// The biome for a quantized climate: the first biome in `biomes.toml` whose ranges contain
    /// it. Total, because validation checks that the table covers every climate.
    pub fn classify_biome(&self, temperature: u8, moisture: u8) -> BiomeId {
        self.try_classify_biome(temperature, moisture).expect("coverage validated at load")
    }

    pub fn biome_id(&self, id: &str) -> Option<BiomeId> {
        self.biomes.iter().position(|b| b.id == id).map(|i| BiomeId(i as u8))
    }

    pub fn resource_id(&self, id: &str) -> Option<ResourceId> {
        self.resources.iter().position(|r| r.id == id).map(|i| ResourceId(i as u16))
    }

    pub fn food(&self, id: &str) -> Option<&FoodDef> {
        self.foods.iter().find(|f| f.id == id)
    }

    pub fn item_id(&self, id: &str) -> Option<ItemId> {
        self.items.iter().position(|i| i.id == id).map(|i| ItemId(i as u16))
    }

    pub fn item(&self, id: ItemId) -> &ItemDef {
        &self.items[id.0 as usize]
    }

    /// The food an item is, if it can be eaten.
    pub fn food_of(&self, id: ItemId) -> Option<&FoodDef> {
        self.item(id).food.map(|f| &self.foods[f])
    }

    /// What working this resource yields, if it can be worked.
    pub fn harvest(&self, id: ResourceId) -> Option<&HarvestDef> {
        self.harvest_by_resource[id.0 as usize].map(|i| &self.harvests[i])
    }

    pub fn tech_id(&self, id: &str) -> Option<TechId> {
        self.techs.iter().position(|t| t.id == id).map(|i| TechId(i as u16))
    }

    pub fn biome(&self, id: BiomeId) -> &BiomeDef {
        &self.biomes[id.0 as usize]
    }

    pub fn resource(&self, id: ResourceId) -> &ResourceDef {
        &self.resources[id.0 as usize]
    }

    pub fn tech(&self, id: TechId) -> &TechDef {
        &self.techs[id.0 as usize]
    }

    pub fn nation(&self, n: Nation) -> &NationDef {
        self.nations.iter().find(|d| d.id == n).expect("validated at load")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const BIOMES: &str = include_str!("../../../data/biomes.toml");
    const NATIONS: &str = include_str!("../../../data/nations.toml");
    const RESEARCH: &str = include_str!("../../../data/research.toml");
    const NEEDS: &str = include_str!("../../../data/needs.toml");
    const JOBS: &str = include_str!("../../../data/jobs.toml");

    /// The bundled needs without the food-to-resource link, for tests with their own resources.
    fn unlinked_needs() -> String {
        NEEDS.replace("resource = \"cabbage\"", "")
    }

    /// The bundled jobs without harvests, for tests with their own resources.
    fn bare_jobs() -> String {
        let end = JOBS.find("[[harvest]]").unwrap();
        format!("harvest = []\n{}", &JOBS[..end])
    }

    fn with_biomes(biomes: &str) -> Result<Defs, DefsError> {
        // No resources, so they can't reference biomes missing from the test table.
        Defs::from_toml(biomes, "resource = []", NATIONS, RESEARCH, &unlinked_needs(), &bare_jobs())
    }

    fn with_resource(fields: &str) -> Result<Defs, DefsError> {
        let src = format!("[[resource]]\nid='x'\nname='X'\nglyph='x'\nchance=0.5\n{fields}");
        Defs::from_toml(BIOMES, &src, NATIONS, RESEARCH, &unlinked_needs(), &bare_jobs())
    }

    fn one_biome(t: &str, m: &str) -> String {
        format!("[[biome]]\nid='a'\nname='A'\nglyph='a'\ntemperature={t}\nmoisture={m}")
    }

    #[test]
    fn bundled_data_is_valid() {
        let defs = Defs::bundled();
        assert!(defs.biome_id("plains").is_some());
        assert!(defs.resource_id("gold_vein").is_some());
    }

    #[test]
    fn unknown_biome_reference_is_rejected() {
        let err = with_resource("biomes=['mangrove']").unwrap_err();
        assert!(err.0.contains("mangrove"), "{err}");
    }

    #[test]
    fn bundled_biome_table_is_climate_driven() {
        let defs = Defs::bundled();
        let at = |t: f64, m: f64| {
            let id = defs.classify_biome((t * 255.0) as u8, (m * 255.0) as u8);
            defs.biome(id).id.as_str()
        };
        assert_eq!(at(0.05, 0.5), "tundra");
        assert_eq!(at(0.05, 0.95), "tundra");
        assert_eq!(at(0.85, 0.1), "desert");
        assert_eq!(at(0.5, 0.75), "forest");
        assert_eq!(at(0.5, 0.35), "plains");
    }

    #[test]
    fn first_matching_biome_wins() {
        let toml = format!(
            "{}\n{}",
            one_biome("[0.0, 0.5]", "[0.0, 1.0]"),
            one_biome("[0.0, 1.0]", "[0.0, 1.0]")
        )
        .replacen("id='a'", "id='b'", 1);
        let defs = with_biomes(&toml).unwrap();
        assert_eq!(defs.classify_biome(0, 0), BiomeId(0));
        assert_eq!(defs.classify_biome(255, 0), BiomeId(1));
    }

    #[test]
    fn biome_gaps_are_rejected() {
        let err = with_biomes(&one_biome("[0.0, 0.5]", "[0.0, 1.0]")).unwrap_err();
        assert!(err.0.contains("no biome covers"), "{err}");
    }

    #[test]
    fn bad_biome_ranges_are_rejected() {
        let err = with_biomes(&one_biome("[0.6, 0.5]", "[0.0, 1.0]")).unwrap_err();
        assert!(err.0.contains("temperature"), "{err}");
        let err = with_biomes(&one_biome("[0.0, 1.0]", "[0.0, 1.5]")).unwrap_err();
        assert!(err.0.contains("moisture"), "{err}");
    }

    #[test]
    fn biomes_need_climate_ranges() {
        let err = with_biomes("[[biome]]\nid='a'\nname='A'\nglyph='a'").unwrap_err();
        assert!(err.0.contains("temperature"), "{err}");
    }

    #[test]
    fn resource_fields_are_validated() {
        assert!(with_resource("clustering=0.5\nmin_spacing=3\nmoisture_scaled=true").is_ok());
        assert!(with_resource("clustering=1.5").unwrap_err().0.contains("clustering"));
        assert!(with_resource("min_spacing=0").unwrap_err().0.contains("min_spacing"));
        assert!(with_resource("chanse=0.1").unwrap_err().0.contains("chanse"));
    }

    fn with_needs(needs: &str) -> Result<Defs, DefsError> {
        Defs::from_toml(
            BIOMES,
            include_str!("../../../data/resources.toml"),
            NATIONS,
            RESEARCH,
            needs,
            JOBS,
        )
    }

    #[test]
    fn needs_data_is_validated() {
        let bad = |from: &str, to: &str| {
            assert!(NEEDS.contains(from), "{from}");
            with_needs(&NEEDS.replacen(from, to, 1)).unwrap_err().0
        };
        assert!(bad("decay_per_minute = 4.0", "decay_per_minute = 0.0").contains("hunger"));
        assert!(bad("low = 30.0", "low = 130.0").contains("energy.low"));
        assert!(bad("cooked_hunger = 70.0", "cooked_hunger = 20.0").contains("cooked_hunger"));
        assert!(bad("resource = \"cabbage\"", "resource = \"nope\"").contains("nope"));
        assert!(bad("id = \"fruit\"", "id = \"fish\"").contains("duplicate"));
        assert!(bad("drink_thirst = 50.0", "drnk = 50.0").contains("drnk"));
    }

    #[test]
    fn raw_food_that_cannot_sustain_a_unit_is_rejected() {
        // Fish at 20 hunger can't replace the 24 a unit loses between meals, however good it
        // tastes cooked: cooking must never be required.
        let err = with_needs(&NEEDS.replacen("hunger = 40.0", "hunger = 20.0", 1)).unwrap_err();
        assert!(err.0.contains("sustain"), "{err}");
    }

    fn with_jobs(jobs: &str) -> Result<Defs, DefsError> {
        Defs::from_toml(
            BIOMES,
            include_str!("../../../data/resources.toml"),
            NATIONS,
            RESEARCH,
            NEEDS,
            jobs,
        )
    }

    #[test]
    fn jobs_data_is_validated() {
        let bad = |from: &str, to: &str| {
            assert!(JOBS.contains(from), "{from}");
            with_jobs(&JOBS.replacen(from, to, 1)).unwrap_err().0
        };
        assert!(bad("resource = \"oak_tree\"", "resource = \"oak\"").contains("oak"));
        assert!(bad("item = \"wood\"", "item = \"plank\"").contains("plank"));
        assert!(bad("modifier = \"earthworks\"", "modifier = \"earthwork\"").contains("earthwork"));
        assert!(bad("count = 4", "count = 0").contains("count"));
        assert!(bad("id = \"wood\"", "id = \"fish\"").contains("duplicate"));
        assert!(bad("sleep_until = 90.0", "sleep_until = 20.0").contains("sleep_until"));
        assert!(bad("base_carry = 5", "base_carry = 0").contains("base_carry"));
        assert!(bad("eat_seconds = 3.0", "eat_secs = 3.0").contains("eat_secs"));
        // Two entries for one resource.
        let twice = format!(
            "{JOBS}\n[[harvest]]\nresource = \"stone\"\nverb = \"X\"\nitem = \"stone\"\ncount = 1\nwork = 1.0\n"
        );
        assert!(with_jobs(&twice).unwrap_err().0.contains("two harvest"));
    }

    #[test]
    fn every_harvest_modifier_and_food_item_resolves() {
        let defs = Defs::bundled();
        let quarry = defs.harvest(defs.resource_id("stone").unwrap()).unwrap();
        assert_eq!(quarry.modifier.as_deref(), Some("earthworks"));
        assert_eq!(defs.item(quarry.item).id, "stone");
        let cabbage = defs.item_id("cabbage").unwrap();
        assert_eq!(defs.food_of(cabbage).unwrap().id, "cabbage");
        assert!(defs.food_of(defs.item_id("wood").unwrap()).is_none());
        // Resources without a harvest entry simply can't be designated.
        assert!(defs.harvests.len() <= defs.resources.len());
    }
}
