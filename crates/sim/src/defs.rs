//! Static game definitions loaded from the TOML files in `data/`.
//!
//! Definitions are referenced at runtime by small index newtypes (`BiomeId`, `ResourceId`,
//! `TechId`) so that map cells stay `Copy` and cheap. Validation happens once, at load time, and
//! is strict (unknown fields, out-of-range numbers and climate gaps are all errors) so that a
//! typo in the data fails `cargo test` instead of silently changing the game.

use serde::Deserialize;

use crate::bending::{AbilityDef, AbilityId, BendingFile, ChiSettings};
use crate::building::{BuildingDef, BuildingId, BuildingsFile};
use crate::embark::EmbarkDef;
use crate::endeavor::{EndeavorDef, EndeavorFile};
use crate::jobs::{HarvestDef, ItemDef, ItemId, JobSettings, JobsFile};
use crate::nation::{Nation, NationDef};
use crate::needs::{FoodDef, NeedsFile, NeedsSettings};
use crate::path::MoveRules;
use crate::research::{self, ResearchFile, ResearchSettings, TechDef, TechId};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Deserialize, serde::Serialize)]
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
    /// A passage through a mountain, bored by Earth's Bore Tunnel bend (issue #132,
    /// docs/design/research.md, "Tunneling"). Walkable, but not open ground: nothing is built or
    /// stockpiled on it. Minerals beside one are mined from inside the rock, which yields more.
    Tunnel,
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
        matches!(self, Terrain::Coast | Terrain::Land | Terrain::Ford | Terrain::Tunnel)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct BiomeId(pub u8);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
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
    /// The tile must touch (8 neighbours) one of these terrains; `None` means no condition. Keeps
    /// fish spots within reach of the shore.
    #[serde(default)]
    pub beside: Option<Vec<Terrain>>,
    /// Placed by mapgen but kept underground: a world takes it off the map when it is made
    /// (`World::buried`) and puts it back only when Earth's Seismic Sense uncovers it, or a Bore
    /// Tunnel strikes it (#133). Until then nothing shows it or can work it.
    #[serde(default)]
    pub buried: bool,
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
    /// Constructible structures, in file order; indexed by `BuildingId` (data/buildings.toml).
    pub buildings: Vec<BuildingDef>,
    /// The chi meter every unit has (data/bending.toml).
    pub chi: ChiSettings,
    /// Bending abilities, in file order; indexed by `AbilityId`.
    pub abilities: Vec<AbilityDef>,
    /// The end-game endeavor's signs and stages (data/endeavor.toml). Empty unless loaded with
    /// `with_endeavor`, which `bundled` does.
    pub endeavor: EndeavorDef,
    /// Embark's site box, map size presets and rough survey (data/embark.toml, #288). Empty
    /// unless loaded with `with_embark`, which `bundled` does.
    pub embark: EmbarkDef,
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
    /// A hash of every id table (biomes, resources, nations, techs, items, buildings, abilities) in
    /// order. Ids such as `TechId` and `ItemId` are indices into these tables, so a save is only
    /// meaningful against data whose hash matches. Balance numbers don't enter into it: retuning a
    /// rate or a cost keeps old saves loadable, while adding, removing or reordering an entry
    /// doesn't.
    pub fn id_hash(&self) -> u64 {
        let tables: [Vec<&str>; 7] = [
            self.biomes.iter().map(|d| d.id.as_str()).collect(),
            self.resources.iter().map(|d| d.id.as_str()).collect(),
            self.nations.iter().map(|d| d.name.as_str()).collect(),
            self.techs.iter().map(|d| d.id.as_str()).collect(),
            self.items.iter().map(|d| d.id.as_str()).collect(),
            self.buildings.iter().map(|d| d.id.as_str()).collect(),
            self.abilities.iter().map(|d| d.id.as_str()).collect(),
        ];
        // FNV-1a, as in `rng::stream`; stable across platforms and Rust versions.
        let mut h: u64 = 0xcbf2_9ce4_8422_2325;
        let mut feed = |b: u8| {
            h ^= u64::from(b);
            h = h.wrapping_mul(0x0100_0000_01b3);
        };
        for table in &tables {
            for id in table {
                id.bytes().for_each(&mut feed);
                feed(0);
            }
            feed(1);
        }
        h
    }

    /// The definitions shipped in `data/`, embedded at compile time.
    pub fn bundled() -> Self {
        Self::from_toml(
            include_str!("../../../data/biomes.toml"),
            include_str!("../../../data/resources.toml"),
            include_str!("../../../data/nations.toml"),
            include_str!("../../../data/research.toml"),
            include_str!("../../../data/needs.toml"),
            include_str!("../../../data/jobs.toml"),
            include_str!("../../../data/bending.toml"),
            include_str!("../../../data/buildings.toml"),
        )
        .and_then(|d| d.with_endeavor(include_str!("../../../data/endeavor.toml")))
        .and_then(|d| d.with_embark(include_str!("../../../data/embark.toml")))
        .expect("bundled data/*.toml must be valid; run `cargo test -p fn_sim`")
    }

    /// These definitions with the endeavor read from `endeavor` (data/endeavor.toml), replacing any
    /// endeavor they had. A separate step from `from_toml` because nothing else in the data
    /// depends on it, and data without one (most tests) simply has no signs.
    pub fn with_endeavor(mut self, endeavor: &str) -> Result<Self, DefsError> {
        self.endeavor = parse::<EndeavorFile>("endeavor.toml", endeavor)?.resolve(
            &self.items,
            &self.buildings,
            &self.abilities,
            self.chi.max,
        )?;
        Ok(self)
    }

    // One `data/*.toml` file per argument keeps every call site self-explanatory; a struct of
    // `&str`s would just move the long argument list one level out.
    #[allow(clippy::too_many_arguments)]
    pub fn from_toml(
        biomes: &str,
        resources: &str,
        nations: &str,
        research: &str,
        needs: &str,
        jobs: &str,
        bending: &str,
        buildings: &str,
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
            parse::<JobsFile>("jobs.toml", jobs)?.resolve(&resources, &foods, &needs, &techs)?;
        let buildings = parse::<BuildingsFile>("buildings.toml", buildings)?
            .resolve(&items, &techs, &resources)?;
        let biomes = parse::<BiomeFile>("biomes.toml", biomes)?.biome;
        let (chi, abilities) = parse::<BendingFile>("bending.toml", bending)?
            .resolve(&biomes, &resources, &harvests, &techs)?;
        let mut harvest_by_resource = vec![None; resources.len()];
        for (i, h) in harvests.iter().enumerate() {
            harvest_by_resource[h.resource.0 as usize] = Some(i);
        }
        let defs = Self {
            biomes,
            resources,
            nations,
            techs,
            research,
            needs,
            foods,
            items,
            harvests,
            jobs,
            buildings,
            chi,
            abilities,
            endeavor: EndeavorDef::default(),
            embark: EmbarkDef::default(),
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
        for b in &self.buildings {
            if let Some(o) = &b.orchard
                && self.harvest(o.plants).is_none_or(|h| h.regrow.is_none())
            {
                return Err(DefsError(format!(
                    "building {}: orchard plants {}, which must be a renewable harvest",
                    b.id,
                    self.resource(o.plants).id
                )));
            }
        }
        for n in Nation::ALL {
            match self.nations.iter().find(|d| d.id == n) {
                None => return Err(DefsError(format!("nations.toml is missing {n:?}"))),
                Some(d) if d.names.is_empty() => {
                    return Err(DefsError(format!("nation {n:?} needs at least one name")));
                }
                Some(d) if d.characters.is_empty() => {
                    return Err(DefsError(format!("nation {n:?} needs at least one character")));
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

    /// The techs `nation` can see and research, with their ids, in file order: the shared tier
    /// (no `nation`) plus that nation's own branch. Other nations' branches are never listed.
    pub fn techs_for(&self, nation: Nation) -> impl Iterator<Item = (TechId, &TechDef)> + '_ {
        self.techs
            .iter()
            .enumerate()
            .filter(move |(_, t)| t.nation.is_none_or(|n| n == nation))
            .map(|(i, t)| (TechId(i as u16), t))
    }

    pub fn ability_id(&self, id: &str) -> Option<AbilityId> {
        self.abilities.iter().position(|a| a.id == id).map(|i| AbilityId(i as u16))
    }

    pub fn ability(&self, id: AbilityId) -> &AbilityDef {
        &self.abilities[id.0 as usize]
    }

    pub fn building_id(&self, id: &str) -> Option<BuildingId> {
        self.buildings.iter().position(|b| b.id == id).map(|i| BuildingId(i as u16))
    }

    /// `walking` rules with rail switched on at the speed the rail building (if any) sets: how a
    /// unit hauling a stack moves (#135). Unchanged if no building is a rail.
    pub fn cart_rules(&self, walking: MoveRules) -> MoveRules {
        match self.buildings.iter().find_map(|b| b.rail) {
            Some(rail) => walking.with_rail(rail.step_cost()),
            None => walking,
        }
    }

    pub fn building(&self, id: BuildingId) -> &BuildingDef {
        &self.buildings[id.0 as usize]
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
    const BENDING: &str = include_str!("../../../data/bending.toml");
    const BUILDINGS: &str = include_str!("../../../data/buildings.toml");

    /// The bundled needs without the food-to-resource link, for tests with their own resources.
    fn unlinked_needs() -> String {
        NEEDS.lines().filter(|l| !l.starts_with("resource = ")).collect::<Vec<_>>().join("\n")
    }

    /// The bundled jobs without harvests, for tests with their own resources.
    fn bare_jobs() -> String {
        let end = JOBS.find("[[harvest]]").unwrap();
        format!("harvest = []\n{}", &JOBS[..end])
    }

    /// The bundled research tree without the job unlocks, for tests with their own resources
    /// and ability unlocks (so `fish_spot`, `stone` and the abilities don't dangle).
    fn bare_research() -> String {
        let mut src = RESEARCH.to_string();
        for job in ["fish_spot", "stone"] {
            let unlock = format!("[tech.unlocks]\njobs = [\"{job}\"]\n");
            assert!(src.contains(&unlock), "{job}'s unlocks.jobs moved; update this helper");
            src = src.replace(&unlock, "");
        }
        // The bare bending has no abilities, so a tech can't unlock one.
        for id in ["inner_fire", "bore_tunnel", "seismic_sense"] {
            let ability = format!("abilities = [\"{id}\"]\n");
            assert!(src.contains(&ability), "{id}'s unlock moved; update this helper");
            src = src.replace(&ability, "");
        }
        src
    }

    /// The bundled bending without abilities, for tests with their own biomes and resources.
    fn bare_bending() -> &'static str {
        &BENDING[..BENDING.find("\n[[ability]]").unwrap()]
    }

    /// The bundled buildings without the orchard's `[building.orchard]`, for tests whose resources
    /// don't include the fruit tree it plants.
    fn bare_buildings() -> String {
        let start = BUILDINGS.find("[building.orchard]").unwrap();
        let end = start
            + BUILDINGS[start..].find("auto_pick = true\n").unwrap()
            + "auto_pick = true\n".len();
        format!("{}{}", &BUILDINGS[..start], &BUILDINGS[end..])
    }

    fn with_biomes(biomes: &str) -> Result<Defs, DefsError> {
        // No resources, so they can't reference biomes missing from the test table.
        Defs::from_toml(
            biomes,
            "resource = []",
            NATIONS,
            &bare_research(),
            &unlinked_needs(),
            &bare_jobs(),
            bare_bending(),
            &bare_buildings(),
        )
    }

    fn with_resource(fields: &str) -> Result<Defs, DefsError> {
        let src = format!("[[resource]]\nid='x'\nname='X'\nglyph='x'\nchance=0.5\n{fields}");
        Defs::from_toml(
            BIOMES,
            &src,
            NATIONS,
            &bare_research(),
            &unlinked_needs(),
            &bare_jobs(),
            bare_bending(),
            &bare_buildings(),
        )
    }

    fn one_biome(t: &str, m: &str) -> String {
        format!("[[biome]]\nid='a'\nname='A'\nglyph='a'\ntemperature={t}\nmoisture={m}")
    }

    #[test]
    fn techs_for_is_the_shared_tier_plus_the_nation_s_own_branch() {
        let defs = Defs::bundled();
        let shared = defs.techs.iter().filter(|t| t.nation.is_none()).count();
        for nation in Nation::ALL {
            let own = defs.techs.iter().filter(|t| t.nation == Some(nation)).count();
            let seen: Vec<_> = defs.techs_for(nation).collect();
            assert_eq!(seen.len(), shared + own, "{nation:?}");
            assert!(seen.iter().all(|(_, t)| t.nation.is_none_or(|n| n == nation)));
            // File order, and the ids point at the same defs.
            assert!(seen.windows(2).all(|w| w[0].0 < w[1].0));
            assert!(seen.iter().all(|(id, t)| defs.tech(*id).id == t.id));
        }
        let healing = defs.tech_id("healing").unwrap();
        assert!(defs.techs_for(Nation::Water).any(|(id, _)| id == healing));
        assert!(defs.techs_for(Nation::Fire).all(|(id, _)| id != healing));
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
        assert!(with_resource("beside=['coast','land']").is_ok());
        assert!(with_resource("beside=['beach']").unwrap_err().0.contains("beach"));
        assert!(with_resource("chanse=0.1").unwrap_err().0.contains("chanse"));
    }

    #[test]
    fn a_nation_needs_characters() {
        let from = r#"characters = ["air_monk", "air_elder", "air_acolyte"]"#;
        assert!(NATIONS.contains(from));
        let nations = NATIONS.replacen(from, "characters = []", 1);
        let err = Defs::from_toml(
            BIOMES,
            include_str!("../../../data/resources.toml"),
            &nations,
            RESEARCH,
            NEEDS,
            JOBS,
            BENDING,
            BUILDINGS,
        )
        .unwrap_err();
        assert!(err.0.contains("character"), "{err}");
    }

    #[test]
    fn an_orchard_must_plant_a_renewable_resource() {
        let from = "plants = \"fruit_tree\"";
        assert!(BUILDINGS.contains(from));
        let with_plants = |what: &str| {
            Defs::from_toml(
                BIOMES,
                include_str!("../../../data/resources.toml"),
                NATIONS,
                RESEARCH,
                NEEDS,
                JOBS,
                BENDING,
                &BUILDINGS.replacen(from, &format!("plants = \"{what}\""), 1),
            )
        };
        // A tree that is chopped down for good can't be tended.
        let err = with_plants("oak_tree").unwrap_err();
        assert!(err.0.contains("renewable"), "{err}");
        assert!(with_plants("fruit_tree").is_ok() && with_plants("cabbage").is_ok());
    }

    fn with_needs(needs: &str) -> Result<Defs, DefsError> {
        Defs::from_toml(
            BIOMES,
            include_str!("../../../data/resources.toml"),
            NATIONS,
            RESEARCH,
            needs,
            JOBS,
            BENDING,
            BUILDINGS,
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
        assert!(bad("arrival_minutes = 15.0", "arrival_minutes = 0.0").contains("arrival_minutes"));
        assert!(
            bad("food_per_newcomer = 30", "food_per_newcomer = 0").contains("food_per_newcomer")
        );
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
            BENDING,
            BUILDINGS,
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
        assert!(bad("regrow = 300.0", "regrow = 0.0").contains("regrow"));
        assert!(bad("knowledge = 40", "knowledge = 0").contains("knowledge"));
        assert!(bad("knowledge = 40", "knowledge = -1").contains("knowledge"));
        // Two entries for one resource.
        let twice = format!(
            "{JOBS}\n[[harvest]]\nresource = \"stone\"\nverb = \"X\"\nitem = \"stone\"\ncount = 1\nwork = 1.0\n"
        );
        assert!(with_jobs(&twice).unwrap_err().0.contains("two harvest"));
    }

    fn with_buildings(buildings: &str) -> Result<Defs, DefsError> {
        Defs::from_toml(
            BIOMES,
            include_str!("../../../data/resources.toml"),
            NATIONS,
            RESEARCH,
            NEEDS,
            JOBS,
            BENDING,
            buildings,
        )
    }

    #[test]
    fn buildings_data_is_validated() {
        let bad = |from: &str, to: &str| {
            assert!(BUILDINGS.contains(from), "{from}");
            with_buildings(&BUILDINGS.replacen(from, to, 1)).unwrap_err().0
        };
        assert!(bad("item = \"wood\"", "item = \"plank\"").contains("plank"));
        assert!(bad("count = 5", "count = 0").contains("count"));
        assert!(bad("work = 20.0", "work = 0.0").contains("work"));
        // Two buildings with the same id.
        let dup = format!(
            "{BUILDINGS}\n[[building]]\nid = \"bed\"\nname = \"Bed\"\nverb = \"Build\"\nwork = 1.0\npassable = true\n"
        );
        assert!(with_buildings(&dup).unwrap_err().0.contains("duplicate"));
        // Two cost entries for the same item.
        let twice = BUILDINGS.replace(
            "[[building.cost]]\nitem = \"wood\"\ncount = 5",
            "[[building.cost]]\nitem = \"wood\"\ncount = 5\n[[building.cost]]\nitem = \"wood\"\ncount = 1",
        );
        assert!(with_buildings(&twice).unwrap_err().0.contains("costed twice"));
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
