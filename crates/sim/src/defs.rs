//! Static game definitions loaded from the TOML files in `data/`.
//!
//! Definitions are referenced at runtime by small index newtypes (`BiomeId`, `ResourceId`,
//! `TechId`) so that map cells stay `Copy` and cheap. Validation happens once, at load time, and
//! is strict (unknown fields, out-of-range numbers and climate gaps are all errors) so that a
//! typo in the data fails `cargo test` instead of silently changing the game.

use serde::Deserialize;

use crate::beast::BeastDef;
use crate::bending::{AbilityDef, AbilityId, BendingFile, CanalDef, ChiSettings, MasteryDef};
use crate::building::{BuildingDef, BuildingId, BuildingsFile, DeconstructDef};
use crate::difficulty::DifficultyDef;
use crate::embark::EmbarkDef;
use crate::endeavor::{EndeavorDef, EndeavorFile};
use crate::harm::HarmDef;
use crate::jobs::{HarvestDef, ItemDef, ItemId, JobSettings, JobsFile};
use crate::nation::{Nation, NationDef};
use crate::needs::{FoodDef, NeedsFile, NeedsSettings};
use crate::path::MoveRules;
use crate::research::{self, ResearchFile, ResearchSettings, TechDef, TechId};
use crate::seasons::SeasonsDef;
use crate::trade::{TradeDef, TradeFile};

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
    /// A channel cut into the land by Water's Carve Canal bend (issue #695, docs/design/
    /// horizons-phase-2.md, "Shared sim work, 4. Canal"). Fresh water that counts as a river to
    /// every water rule (drinking, fishing, Water the Land's source), but not walkable, with no
    /// shore. Mapgen never makes one, so no generated map or golden file holds it.
    Canal,
}

impl Terrain {
    /// Any kind of water: sea, river, lake, ford (shallow water you can wade) or canal.
    pub fn is_water(self) -> bool {
        matches!(
            self,
            Terrain::Water | Terrain::River | Terrain::Lake | Terrain::Ford | Terrain::Canal
        )
    }

    /// Fresh water, the kind a unit drinks from and Water the Land draws on: a river, lake, ford
    /// or canal (not the sea).
    pub fn is_fresh(self) -> bool {
        matches!(self, Terrain::River | Terrain::Lake | Terrain::Ford | Terrain::Canal)
    }

    /// Standing water that gets a shoreline (`Coast`) around it. Rivers don't.
    pub fn has_shore(self) -> bool {
        matches!(self, Terrain::Water | Terrain::Lake)
    }

    pub fn is_walkable(self) -> bool {
        matches!(self, Terrain::Coast | Terrain::Land | Terrain::Ford | Terrain::Tunnel)
    }
}

// `Deserialize` is `save::ids`' remapping impl (#602).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize)]
pub struct BiomeId(pub u8);

// `Deserialize` is `save::ids`' remapping impl (#602).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize)]
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
    /// Taking a finished building down: its work and refund (data/buildings.toml's
    /// `[deconstruct]`, issue #342).
    pub deconstruct: DeconstructDef,
    /// The chi meter every unit has (data/bending.toml).
    pub chi: ChiSettings,
    /// Bending abilities, in file order; indexed by `AbilityId`.
    pub abilities: Vec<AbilityDef>,
    /// The route from novice to master (data/bending.toml's `[mastery]`, #360).
    pub mastery: MasteryDef,
    /// What a carved canal does for the farm plots beside it (data/bending.toml's `[canal]`,
    /// #695).
    pub canal: CanalDef,
    /// The end-game endeavor's signs and stages (data/endeavor.toml). Empty unless loaded with
    /// `with_endeavor`, which `bundled` does.
    pub endeavor: EndeavorDef,
    /// Embark's site box, map size presets and rough survey (data/embark.toml, #288). Empty
    /// unless loaded with `with_embark`, which `bundled` does.
    pub embark: EmbarkDef,
    /// Difficulty presets: the knobs that scale pressure and the stores `World::found` lays
    /// down (data/difficulty.toml, #294). Has no presets unless loaded with `with_difficulty`,
    /// which `bundled` does; `DifficultyDef::identity` still resolves either way.
    pub difficulty: DifficultyDef,
    /// Seasons, climate bands, frost and cold (data/seasons.toml, #295). Safe to read even
    /// unloaded (`SeasonsDef::default`); nothing acts on it unless a difficulty preset's
    /// `seasons` flag is on, and `bundled` always loads this alongside `difficulty`.
    pub seasons: SeasonsDef,
    /// Harm's shared settings and exposure's rate (data/harm.toml, #387). Safe to read even
    /// unloaded (`HarmDef::default`); nothing acts on it unless a difficulty preset's `harm`
    /// knob is above 0, and `bundled` always loads this alongside `difficulty`.
    pub harm: HarmDef,
    /// Caravans (data/trade.toml, #437). Empty unless loaded with `with_trade`, which `bundled`
    /// does: then no caravan ever comes.
    pub trade: TradeDef,
    /// Beasts, in file order; indexed by `BeastId` (data/beasts.toml, `beast.rs`). Empty unless
    /// loaded with `with_beasts`, which `bundled` does.
    pub beasts: Vec<BeastDef>,
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

/// The files [`Defs::bundled`] embeds, in the order it reads them: biomes, resources, nations,
/// research, needs, jobs, bending, buildings, endeavor, embark, difficulty, seasons, harm.
pub(crate) const BUNDLED_FILES: [&str; 13] = [
    include_str!("../../../data/biomes.toml"),
    include_str!("../../../data/resources.toml"),
    include_str!("../../../data/nations.toml"),
    include_str!("../../../data/research.toml"),
    include_str!("../../../data/needs.toml"),
    include_str!("../../../data/jobs.toml"),
    include_str!("../../../data/bending.toml"),
    include_str!("../../../data/buildings.toml"),
    include_str!("../../../data/endeavor.toml"),
    include_str!("../../../data/embark.toml"),
    include_str!("../../../data/difficulty.toml"),
    include_str!("../../../data/seasons.toml"),
    include_str!("../../../data/harm.toml"),
];

/// `data/beasts.toml` (#693), read after the 13 files above by [`Defs::with_beasts`]: a separate
/// step like `with_trade`, so `from_files` keeps its shape. `bundled` loads it; a test that edits
/// the 13 files and wants the beasts too adds `.with_beasts(BEASTS_FILE)`.
pub(crate) const BEASTS_FILE: &str = include_str!("../../../data/beasts.toml");

/// data/trade.toml (#437), read by [`Defs::bundled`] after the 13 files above. Kept apart from
/// them so `Defs::from_files` and its callers don't change; data loaded that way has no caravans.
pub(crate) const TRADE_FILE: &str = include_str!("../../../data/trade.toml");

impl Defs {
    /// A hash of every id table (biomes, resources, nations, techs, items, buildings, abilities) in
    /// order: `SaveIds::of(self).hash()`. Ids such as `TechId` are indices into these tables, so
    /// two builds with the same hash number their content the same way. Balance numbers don't
    /// enter into it. Saves no longer depend on it (#602: a save carries its own id tables and is
    /// remapped by name on load) and write it only as information. It is still a run log's
    /// `data_digest`, because replaying keystrokes only holds for the same id order.
    pub fn id_hash(&self) -> u64 {
        crate::save::SaveIds::of(self).hash()
    }

    /// The definitions shipped in `data/`, embedded at compile time.
    pub fn bundled() -> Self {
        Self::from_files(BUNDLED_FILES)
            .and_then(|d| d.with_trade(TRADE_FILE))
            .and_then(|d| d.with_beasts(BEASTS_FILE))
            .expect("bundled data/*.toml must be valid; run `cargo test -p fn_sim`")
    }

    /// These definitions with caravans read from `trade` (data/trade.toml, #437), replacing any
    /// they had. Read after the endeavor, since `[caravans].opens` names one of its chapters.
    pub fn with_trade(mut self, trade: &str) -> Result<Self, DefsError> {
        self.trade = parse::<TradeFile>("trade.toml", trade)?.resolve(
            &self.items,
            &self.endeavor.chapters,
            self.needs.growth.draw.bank,
        )?;
        Ok(self)
    }

    /// Definitions read from the text of the 13 `data/*.toml` files, in the order
    /// [`Defs::bundled`] reads them: biomes, resources, nations, research, needs, jobs, bending,
    /// buildings, endeavor, embark, difficulty, seasons, harm. `bundled` is this on the
    /// embedded files; tests use it on edited copies of them.
    pub fn from_files(files: [&str; 13]) -> Result<Self, DefsError> {
        let [
            biomes,
            resources,
            nations,
            research,
            needs,
            jobs,
            bending,
            buildings,
            endeavor,
            embark,
            difficulty,
            seasons,
            harm,
        ] = files;
        Self::from_toml(biomes, resources, nations, research, needs, jobs, bending, buildings)
            .and_then(|d| d.with_endeavor(endeavor))
            .and_then(|d| d.with_embark(embark))
            .and_then(|d| d.with_difficulty(difficulty))
            .and_then(|d| d.with_seasons(seasons))
            .and_then(|d| d.with_harm(harm))
    }

    /// A hash of the text of every file [`Defs::bundled`] reads, so two runs of the game (or of
    /// the balance harness) can say whether they played the same numbers. Unlike [`Defs::id_hash`]
    /// it moves with any edit, a retuned rate included. FNV-1a, stable across platforms and Rust
    /// versions. It cannot see a change to the code, only to `data/`.
    pub fn bundled_digest() -> u64 {
        let mut h: u64 = 0xcbf2_9ce4_8422_2325;
        for file in BUNDLED_FILES.into_iter().chain([TRADE_FILE, BEASTS_FILE]) {
            for b in file.bytes().chain(std::iter::once(0)) {
                h ^= u64::from(b);
                h = h.wrapping_mul(0x0100_0000_01b3);
            }
        }
        h
    }

    /// These definitions with the endeavor read from `endeavor` (data/endeavor.toml), replacing any
    /// endeavor they had. A separate step from `from_toml` because nothing else in the data
    /// depends on it, and data without one (most tests) simply has no signs.
    pub fn with_endeavor(mut self, endeavor: &str) -> Result<Self, DefsError> {
        self.endeavor = parse::<EndeavorFile>("endeavor.toml", endeavor)?.resolve(
            &self.items,
            &self.buildings,
            &self.techs,
            &self.abilities,
            self.chi.max,
            &self.mastery,
        )?;
        // A tech's `requires.chapter` can only be checked now that the chapters are known.
        research::check_chapters(&self.techs, &self.endeavor.chapters)?;
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
        let buildings = parse::<BuildingsFile>("buildings.toml", buildings)?;
        let deconstruct = buildings.deconstruct()?;
        let buildings = buildings.resolve(&items, &techs, &resources, &needs)?;
        let biomes = parse::<BiomeFile>("biomes.toml", biomes)?.biome;
        let (chi, abilities, mastery, canal) = parse::<BendingFile>("bending.toml", bending)?
            .resolve(&biomes, &resources, &harvests, &items, &techs)?;
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
            deconstruct,
            chi,
            abilities,
            mastery,
            canal,
            endeavor: EndeavorDef::default(),
            embark: EmbarkDef::default(),
            difficulty: DifficultyDef::default(),
            seasons: SeasonsDef::default(),
            harm: HarmDef::default(),
            trade: TradeDef::default(),
            beasts: Vec::new(),
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
            for &r in b.pickers.iter().flat_map(|p| &p.resources) {
                if self.harvest(r).is_none() {
                    return Err(DefsError(format!(
                        "building {}: pickers pick {}, which has no harvest",
                        b.id,
                        self.resource(r).id
                    )));
                }
            }
            for &item in b.outpost.iter().flat_map(|o| &o.pantry_items) {
                if self.food_of(item).is_none() {
                    return Err(DefsError(format!(
                        "building {}: outpost pantry keeps {}, which is not a food",
                        b.id,
                        self.item(item).id
                    )));
                }
            }
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

    /// The item `item` may pay for, one for one, in a building or endeavor cost (issue #371:
    /// `ItemDef::stands_for`, brick for stone). Never a recipe input.
    pub fn stands_for(&self, item: ItemId) -> Option<ItemId> {
        self.item(item).stands_for
    }

    /// The item, if any, whose `stands_for` names `target` (the reverse of `stands_for`: stone
    /// -> brick). At most one item may stand for a given target (validated at load), so this is
    /// unambiguous.
    pub fn stand_in_for(&self, target: ItemId) -> Option<ItemId> {
        self.items.iter().position(|i| i.stands_for == Some(target)).map(|i| ItemId(i as u16))
    }

    /// Whether `nation` could ever raise a building whose recipe makes `item` (not a harvest):
    /// the brick kiln for brick, say. Used so a stand-in's cost hint ("or brick") only shows for
    /// a nation that can actually make the stand-in (issue #371).
    pub fn nation_can_make(&self, nation: Nation, item: ItemId) -> bool {
        self.buildings.iter().any(|b| {
            b.recipe.as_ref().is_some_and(|r| r.output.item == item)
                && b.unlocked_by.is_none_or(|t| self.tech(t).open_to(nation))
        })
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
    /// The tech that unlocks `feature` (`Unlocks::features`), or `None` if no tech does, which
    /// makes it available from the start.
    pub fn feature_tech(&self, feature: crate::research::Feature) -> Option<TechId> {
        self.techs
            .iter()
            .position(|t| t.unlocks.features.contains(&feature))
            .map(|i| TechId(i as u16))
    }

    pub fn techs_for(&self, nation: Nation) -> impl Iterator<Item = (TechId, &TechDef)> + '_ {
        self.techs
            .iter()
            .enumerate()
            .filter(move |(_, t)| t.open_to(nation))
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

    /// `walking` rules with every track switched on at its own speed (`TrackDef`): how units
    /// move (#135, docs/design/horizons-phase-1.md). Unchanged if no building is a track.
    pub fn cart_rules(&self, walking: MoveRules) -> MoveRules {
        walking.with_tracks(self.track_steps())
    }

    /// Each track's straight-step cost, indexed by `TrackDef::index - 1`.
    pub fn track_steps(&self) -> Vec<u32> {
        self.buildings.iter().filter_map(|b| b.track).map(|t| t.step_cost()).collect()
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

    /// The digest is a function of the shipped data text alone, so it is the same every time.
    #[test]
    fn the_bundled_digest_is_stable() {
        assert_eq!(Defs::bundled_digest(), Defs::bundled_digest());
        assert_ne!(Defs::bundled_digest(), 0xcbf2_9ce4_8422_2325, "it hashed nothing");
    }

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
        for (job, unlock) in [
            ("fish_spot", "[tech.unlocks]\njobs = [\"fish_spot\"]\n".to_owned()),
            // boulder added alongside stone by #349.
            ("stone", "[tech.unlocks]\njobs = [\"stone\", \"boulder\"]\n".to_owned()),
            // gold_vein and gold_seam added by #383 (prospecting).
            ("gold_vein", "[tech.unlocks]\njobs = [\"gold_vein\", \"gold_seam\"]\n".to_owned()),
        ] {
            assert!(src.contains(&unlock), "{job}'s unlocks.jobs moved; update this helper");
            src = src.replace(&unlock, "");
        }
        // The bare bending has no abilities, so a tech can't unlock one.
        for id in [
            "inner_fire",
            "bore_tunnel",
            "seismic_sense",
            "glide",
            "metalbend",
            "air_scooter",
            "earth_tent",
            "air_shield",
            "weld",
            "lift",
            "draw_water",
            "cut_ice",
            "carve_canal",
        ] {
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

    /// The bundled buildings without any `[building.orchard]` section (the Orchard's and the
    /// Fishing Dock's, issue #374) or `[building.chart]` section (the Map Room's, issue #402),
    /// for tests whose resources don't include the fruit tree, fish spot or lost ruins any of
    /// those name.
    fn bare_buildings() -> String {
        let mut text = BUILDINGS.to_owned();
        while let Some(start) = text.find("[building.orchard]") {
            let end = start
                + text[start..].find("auto_pick = true\n").unwrap()
                + "auto_pick = true\n".len();
            text.replace_range(start..end, "");
        }
        while let Some(start) = text.find("[building.pickers]") {
            let rest = &text[start..];
            let next = rest[1..].find("\n[").expect("pickers is followed by another table");
            text.replace_range(start..start + 1 + next, "");
        }
        while let Some(start) = text.find("[building.chart]") {
            let rest = &text[start..];
            let next = rest[1..].find("\n[").expect("chart is followed by another table");
            text.replace_range(start..start + 1 + next, "");
        }
        text
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
        let shared = defs.techs.iter().filter(|t| t.is_shared()).count();
        for nation in Nation::ALL {
            let own = defs
                .techs
                .iter()
                .filter(|t| t.nation == Some(nation) || t.nations.contains(&nation))
                .count();
            let seen: Vec<_> = defs.techs_for(nation).collect();
            assert_eq!(seen.len(), shared + own, "{nation:?}");
            assert!(seen.iter().all(|(_, t)| t.open_to(nation)));
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
        assert!(bad("arrival_minutes = 10.0", "arrival_minutes = 0.0").contains("arrival_minutes"));
        assert!(
            bad("food_per_newcomer = 30", "food_per_newcomer = 0").contains("food_per_newcomer")
        );
        assert!(bad("own_weight = 0.4", "own_weight = 0.0").contains("own_weight"));
        assert!(bad("other_weight = 0.2", "other_weight = -1.0").contains("other_weight"));
        // `[growth.draw]` (#435): strict, and range-checked.
        assert!(bad("max = 1.4", "max = 0.5").contains("growth.draw.base"));
        assert!(bad("bank = 2.0", "bank = 0.5").contains("growth.draw.bank"));
        assert!(bad("food_kind_min = 10", "food_kind_min = 0").contains("food_kind_min"));
        assert!(bad("min_gap_minutes = 2.0", "min_gap_minutes = -1.0").contains("min_gap"));
        assert!(bad("spare_beds_max = 2", "spare_beds = 2").contains("spare_beds"));
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
        assert!(bad("count = 3", "count = 0").contains("count"));
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
    fn a_pair_section_is_parsed_and_validated() {
        let with_pair = |pair: &str| {
            let extra = format!(
                "\n[[building]]\nid = \"twin\"\nname = \"Twin\"\nverb = \"Build\"\nwork = 1.0\npassable = true\n{pair}[[building.cost]]\nitem = \"wood\"\ncount = 1\n"
            );
            with_buildings(&format!("{BUILDINGS}{extra}"))
        };
        let defs = with_pair("[building.pair]\nreach = 48\nmin_tiles = 12\n").unwrap();
        let twin = defs.building(defs.building_id("twin").unwrap());
        assert_eq!(twin.pair, Some(crate::building::PairDef { reach: 48, min_tiles: 12 }));
        assert_eq!(defs.building(defs.building_id("bed").unwrap()).pair, None);
        let err = |pair: &str| with_pair(pair).unwrap_err().0;
        assert!(err("[building.pair]\nreach = 0\nmin_tiles = 0\n").contains("pair reach"));
        assert!(err("[building.pair]\nreach = 8\nmin_tiles = 9\n").contains("pair reach"));
        assert!(err("[building.pair]\nreach = 8\n").contains("min_tiles"));
        assert!(
            err("[building.pair]\nreach = 8\nmin_tiles = 2\nfar = 1\n").contains("far"),
            "unknown fields are refused"
        );
        let both =
            "[building.pair]\nreach = 8\nmin_tiles = 2\n[building.chute]\nreach = 8\ndrop = 1\n";
        assert!(err(both).contains("chute"));
    }

    #[test]
    fn a_glide_section_is_parsed_and_validated() {
        let with = |sections: &str| {
            let extra = format!(
                "\n[[building]]\nid = \"twin\"\nname = \"Twin\"\nverb = \"Build\"\nwork = 1.0\npassable = true\n{sections}[[building.cost]]\nitem = \"wood\"\ncount = 1\n"
            );
            with_buildings(&format!("{BUILDINGS}{extra}"))
        };
        let pair = "[building.pair]\nreach = 48\nmin_tiles = 12\n";
        let defs =
            with(&format!("{pair}[building.glide]\nseconds = 2.0\nseconds_per_tile = 0.1\n"))
                .unwrap();
        let twin = defs.building(defs.building_id("twin").unwrap());
        let glide = twin.glide.unwrap();
        assert_eq!((glide.seconds, glide.seconds_per_tile), (2.0, 0.1));
        // 2 s plus 0.1 s a tile, at 10 ticks a second.
        assert_eq!(glide.ticks(30), 50);
        assert_eq!(defs.building(defs.building_id("bed").unwrap()).glide, None);
        let err = |s: &str| with(s).unwrap_err().0;
        assert!(err("[building.glide]\nseconds = 2.0\nseconds_per_tile = 0.1\n").contains("pair"));
        assert!(
            err(&format!("{pair}[building.glide]\nseconds = -1.0\nseconds_per_tile = 0.1\n"))
                .contains("glide")
        );
        assert!(
            err(&format!("{pair}[building.glide]\nseconds = 2.0\n")).contains("seconds_per_tile")
        );
        let bundled = Defs::bundled();
        let gate = bundled.building(bundled.building_id("wind_gate").unwrap());
        assert!(gate.glide.is_some() && gate.pair.is_some() && gate.passable);
    }

    #[test]
    fn a_balloon_section_is_parsed_and_validated() {
        let with = |body: &str| {
            let extra = format!(
                "\n[[building]]\nid = \"twin\"\nname = \"Twin\"\nverb = \"Build\"\nwork = 1.0\npassable = true\n{body}[[building.cost]]\nitem = \"wood\"\ncount = 1\n"
            );
            with_buildings(&format!("{BUILDINGS}{extra}"))
        };
        let pair = "[building.pair]\nreach = 48\nmin_tiles = 12\n";
        let balloon = |tweak: &str| {
            format!(
                "{pair}[building.balloon]\nload = 20\nhold = 40\nwait_seconds = 60.0\ntiles_per_second = 1.5\nfuel_per_flight = 2\n{tweak}"
            )
        };
        let fuel = "[building.fuel]\nitem = \"wood\"\nbunker = 10\n";
        let defs = with(&format!("{}{fuel}", balloon(""))).unwrap();
        let twin = defs.building(defs.building_id("twin").unwrap());
        let b = twin.balloon.expect("a balloon");
        assert_eq!((b.load, b.hold, b.wait_ticks, b.fuel_per_flight), (20, 40, 600, 2));
        assert_eq!(
            twin.fuel.as_ref().unwrap().per_minute,
            None,
            "per_minute is optional with a balloon"
        );
        let err = |body: String| with(&body).unwrap_err().0;
        assert!(err(format!("{}{fuel}", balloon("far = 1\n"))).contains("far"));
        assert!(err(format!("[building.balloon]\nload = 1\nhold = 1\nwait_seconds = 1.0\ntiles_per_second = 1.0\nfuel_per_flight = 1\n{fuel}")).contains("needs a [building.pair]"));
        assert!(err(balloon("")).contains("needs a [building.fuel]"));
        let bad = |from: &str, to: &str| {
            let body = balloon("").replace(from, to);
            err(format!("{body}{fuel}"))
        };
        assert!(bad("load = 20", "load = 41").contains("load must be"));
        assert!(bad("load = 20", "load = 0").contains("load must be"));
        assert!(bad("fuel_per_flight = 2", "fuel_per_flight = 11").contains("fuel_per_flight"));
        assert!(bad("fuel_per_flight = 2", "fuel_per_flight = 0").contains("fuel_per_flight"));
        assert!(bad("wait_seconds = 60.0", "wait_seconds = 0.0").contains("wait_seconds"));
        assert!(
            bad("tiles_per_second = 1.5", "tiles_per_second = 0.0").contains("tiles_per_second")
        );
        // A fuel bunker with no per_minute and no balloon would never burn.
        let lone = err(fuel.to_string());
        assert!(lone.contains("fuel needs per_minute"), "{lone}");
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
