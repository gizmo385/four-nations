//! Embark: surveying a generated map and reporting on a candidate site
//! (`docs/design/embark.md`, issue #288).
//!
//! `Survey` holds a generated map with its buried resources already hidden, exactly as
//! `World::bury` hides them, so a frontend can look at the map and ask for a report on any tile
//! before a world exists. `Survey::report` is pure and cheap (at most `(2*report_radius+1)^2`
//! tiles plus a walk capped at 40 steps), so it can run on every cursor move.
//!
//! Founding a world from a chosen site is `World::found` (`world/found.rs`, #289), which consumes
//! a `Survey` via `into_parts` instead of re-generating; the TUI screen is #290.

use std::collections::{BTreeMap, BTreeSet, VecDeque};

use serde::Deserialize;

use crate::defs::{BiomeId, Defs, DefsError, ResourceId, Terrain};
use crate::geom::Pos;
use crate::mapgen::{self, MapGenConfig, Tile, TileMap};
use crate::path::MoveRules;
use crate::regions::{RegionId, Regions};
use crate::rng;
use crate::seasons::ClimateDef;

/// Settlers a found world gets today (`fn_ui::STARTING_UNITS`). The report doesn't know the
/// count the player will found with, so it checks room for this many while surveying; only
/// `SiteProblem::NoRoom` reads it. `World::found` (`world/found.rs`) takes its own `units`
/// argument and debug-asserts it against this constant rather than being redesigned to share it,
/// since the two are expected to match in practice (#289).
pub(crate) const SETTLERS: u32 = 6;

/// How far a walk to water is searched before giving up (`SiteReport::water`).
const WATER_SEARCH_CAP: u32 = 40;

// ---- data/embark.toml ----

/// A map size preset (`[[map_size]]`), offered on #280's seed step.
#[derive(Debug, Clone, PartialEq)]
pub struct MapSizeDef {
    pub id: String,
    pub name: String,
    pub width: i32,
    pub height: i32,
    /// Overrides `MapGenConfig::continent_frequency`, if set.
    pub continent_frequency: Option<f64>,
}

/// One resource group the report counts (`[[group]]`): a count in the site box turned into a
/// level.
#[derive(Debug, Clone, PartialEq)]
pub struct GroupDef {
    pub id: String,
    pub name: String,
    /// Resources counted together.
    pub resources: Vec<ResourceId>,
    /// `[scarce_from, some_from, plenty_from]`, ascending. Below `scarce_from` is `none`.
    pub levels: [u32; 3],
    /// How much an empty or scarce box of this group counts against the outlook; 0 means never.
    pub outlook_weight: u32,
    /// True reads "?" instead of a level in a rough survey (gold, scrolls).
    pub rough_hidden: bool,
}

impl GroupDef {
    fn level_of(&self, count: u32) -> Level {
        let [scarce, some, plenty] = self.levels;
        if count >= plenty {
            Level::Plenty
        } else if count >= some {
            Level::Some
        } else if count >= scarce {
            Level::Scarce
        } else {
            Level::None
        }
    }
}

/// `[rough]`: how far a rough survey's estimate can stray from the truth.
#[derive(Debug, Clone, Copy, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RoughSettings {
    pub error_chance: f64,
}

/// `[outlook]`: how the one-word verdict is scored (`docs/design/embark.md`, "The site report").
#[derive(Debug, Clone, Copy, Default, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OutlookSettings {
    pub water_far: u32,
    pub water_weight: u32,
    pub fair_from: u32,
    pub hard_from: u32,
}

/// Everything in `data/embark.toml`, resolved and validated. Empty (no groups, no sizes) unless
/// loaded with `Defs::with_embark`, which `Defs::bundled` does.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct EmbarkDef {
    pub report_radius: i32,
    pub min_edge_distance: i32,
    pub min_room: u32,
    pub spawn_radius: i32,
    pub map_sizes: Vec<MapSizeDef>,
    pub rough: RoughSettings,
    pub groups: Vec<GroupDef>,
    pub outlook: OutlookSettings,
}

impl Default for RoughSettings {
    fn default() -> Self {
        Self { error_chance: 0.0 }
    }
}

impl EmbarkDef {
    /// The preset with this id, if any.
    pub fn map_size(&self, id: &str) -> Option<&MapSizeDef> {
        self.map_sizes.iter().find(|s| s.id == id)
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct EmbarkFile {
    report_radius: i32,
    min_edge_distance: i32,
    min_room: u32,
    spawn_radius: i32,
    #[serde(rename = "map_size")]
    map_sizes: Vec<RawMapSize>,
    rough: RoughSettings,
    #[serde(rename = "group")]
    groups: Vec<RawGroup>,
    outlook: OutlookSettings,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawMapSize {
    id: String,
    name: String,
    width: i32,
    height: i32,
    #[serde(default)]
    continent_frequency: Option<f64>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawGroup {
    id: String,
    name: String,
    resources: Vec<String>,
    levels: [u32; 3],
    outlook_weight: u32,
    #[serde(default)]
    rough_hidden: bool,
}

impl EmbarkFile {
    fn resolve(self, resource_ids: &[&str]) -> Result<EmbarkDef, DefsError> {
        if self.report_radius <= 0 {
            return Err(DefsError("embark: report_radius must be positive".into()));
        }
        if self.map_sizes.is_empty() {
            return Err(DefsError("embark: need at least one [[map_size]]".into()));
        }
        let mut map_sizes = Vec::with_capacity(self.map_sizes.len());
        for raw in self.map_sizes {
            if map_sizes.iter().any(|s: &MapSizeDef| s.id == raw.id) {
                return Err(DefsError(format!("embark: map size {}: duplicate id", raw.id)));
            }
            if raw.width <= 0 || raw.height <= 0 {
                return Err(DefsError(format!("embark: map size {}: bad dimensions", raw.id)));
            }
            map_sizes.push(MapSizeDef {
                id: raw.id,
                name: raw.name,
                width: raw.width,
                height: raw.height,
                continent_frequency: raw.continent_frequency,
            });
        }
        if !(0.0..=1.0).contains(&self.rough.error_chance) {
            return Err(DefsError("embark: rough.error_chance must be in [0, 1]".into()));
        }
        let mut groups = Vec::with_capacity(self.groups.len());
        for raw in self.groups {
            if groups.iter().any(|g: &GroupDef| g.id == raw.id) {
                return Err(DefsError(format!("embark: group {}: duplicate id", raw.id)));
            }
            let [a, b, c] = raw.levels;
            if !(a <= b && b <= c) {
                return Err(DefsError(format!(
                    "embark: group {}: levels must ascend, got {:?}",
                    raw.id, raw.levels
                )));
            }
            let mut resources = Vec::with_capacity(raw.resources.len());
            for id in &raw.resources {
                let Some(i) = resource_ids.iter().position(|r| r == id) else {
                    return Err(DefsError(format!(
                        "embark: group {}: unknown resource {id:?}",
                        raw.id
                    )));
                };
                resources.push(ResourceId(i as u16));
            }
            groups.push(GroupDef {
                id: raw.id,
                name: raw.name,
                resources,
                levels: raw.levels,
                outlook_weight: raw.outlook_weight,
                rough_hidden: raw.rough_hidden,
            });
        }
        Ok(EmbarkDef {
            report_radius: self.report_radius,
            min_edge_distance: self.min_edge_distance,
            min_room: self.min_room,
            spawn_radius: self.spawn_radius,
            map_sizes,
            rough: self.rough,
            groups,
            outlook: self.outlook,
        })
    }
}

impl Defs {
    /// These definitions with the embark settings read from `embark` (data/embark.toml),
    /// replacing any it had. A separate step from `from_toml`, as `with_endeavor` is, so data
    /// without one (most tests) simply has no map sizes or groups.
    pub fn with_embark(mut self, embark: &str) -> Result<Self, DefsError> {
        let resource_ids: Vec<&str> = self.resources.iter().map(|r| r.id.as_str()).collect();
        self.embark = toml::from_str::<EmbarkFile>(embark)
            .map_err(|e| DefsError(format!("embark.toml: {e}")))?
            .resolve(&resource_ids)?;
        Ok(self)
    }
}

// ---- The site report ----

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SurveyMode {
    Exact,
    Rough,
}

/// Why a site can't be settled.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SiteProblem {
    /// The tile itself isn't open land (water, mountain, a ford).
    NotLand,
    /// Too close to the map edge (`EmbarkDef::min_edge_distance`).
    NearEdge,
    /// The landmass reachable from here is smaller than `EmbarkDef::min_room`.
    TooSmall,
    /// Fewer open tiles than settlers within `EmbarkDef::spawn_radius`.
    NoRoom,
}

/// A count in a site's box, turned into a level (`EmbarkDef::outlook`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Level {
    None,
    Scarce,
    Some,
    Plenty,
}

impl Level {
    /// This level moved `delta` steps (negative lowers it), clamped at `None` and `Plenty`.
    fn shifted(self, delta: i32) -> Self {
        let i = (self as i32 + delta).clamp(Level::None as i32, Level::Plenty as i32);
        [Level::None, Level::Scarce, Level::Some, Level::Plenty][i as usize]
    }
}

/// The share of a site's box that is a biome, or mountain, or water (`SiteReport::biomes`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LandKind {
    Biome(BiomeId),
    Mountain,
    Water,
}

/// What the report says about one resource group (`SiteReport::groups`).
#[derive(Debug, Clone, PartialEq)]
pub struct GroupCount {
    pub id: String,
    pub name: String,
    /// The count in the box; `None` in a rough survey, which reads only `level`.
    pub count: Option<u32>,
    pub level: Level,
    /// True for a `rough_hidden` group read in a rough survey: `level` is meaningless then.
    pub hidden: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Outlook {
    Easy,
    Fair,
    Hard,
}

/// What `Survey::report` says about one tile (`docs/design/embark.md`, "The site report").
#[derive(Debug, Clone, PartialEq)]
pub struct SiteReport {
    pub problem: Option<SiteProblem>,
    /// Share of the box per biome, plus mountain and water, largest first.
    pub biomes: Vec<(LandKind, u8)>,
    /// Open tiles reachable from the site, under the default `MoveRules`.
    pub room: u32,
    /// Whether that region is the largest landmass on the map.
    pub mainland: bool,
    /// The nearest tile a unit could drink from, by walking steps, and its kind. `None` if
    /// nothing is within the 40-step cap.
    pub water: Option<(Terrain, u32)>,
    pub groups: Vec<GroupCount>,
    /// The start tile's climate band (`World::climate_at`, #295), exact whether the survey is
    /// rough or not: terrain and climate are never what the scouts get wrong
    /// (`docs/design/difficulty.md`, "Embark").
    pub climate: ClimateDef,
    pub outlook: Outlook,
    /// The one or two facts that decided the outlook.
    pub reasons: Vec<String>,
    /// True when `mode` was `SurveyMode::Rough`.
    pub rough: bool,
}

impl SiteReport {
    fn refused(problem: SiteProblem, climate: ClimateDef, rough: bool) -> Self {
        Self {
            problem: Some(problem),
            biomes: Vec::new(),
            room: 0,
            mainland: false,
            water: None,
            groups: Vec::new(),
            climate,
            outlook: Outlook::Hard,
            reasons: Vec::new(),
            rough,
        }
    }
}

/// What `Survey::supply` found of one surveyed resource within one radius
/// (`docs/design/materials.md`, "What the map holds", `balance --supply`, #348).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DepositSupply {
    pub resource: ResourceId,
    /// Deposits that touch (8 neighbours) a walkable tile of the start's region, within the
    /// radius: directly, or (a buried resource only) by boring at most `bore_limit` tiles of
    /// tunnel to reach one.
    pub count: u32,
    /// Of `count`, how many were only reachable through a bore, so `worth` used
    /// `HarvestDef::tunnel_count` for them instead of `count`. Always 0 for a surface resource,
    /// which is already exposed and has nothing to bore.
    pub tunnelled: u32,
    /// Total yield: `HarvestDef::count` per deposit, except a tunnelled one, which uses
    /// `tunnel_count` (falling back to `count` if the harvest defines none). 0 for a resource
    /// with no `HarvestDef` at all.
    pub worth: u32,
}

/// One radius's results from `Survey::supply`, in the same order as the resources it was asked
/// for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RadiusSupply {
    pub radius: i32,
    pub deposits: Vec<DepositSupply>,
}

/// A generated map with its buried resources hidden, ready to be surveyed for a site
/// (`docs/design/embark.md`, "The sim surveys first, then founds").
#[derive(Debug, Clone)]
pub struct Survey {
    cfg: MapGenConfig,
    surface: TileMap,
    buried: BTreeMap<Pos, ResourceId>,
    regions: Regions,
}

/// Open land (not water, not mountain): what a site, or a settler's spot, must stand on.
fn is_open_land(t: Terrain) -> bool {
    matches!(t, Terrain::Land | Terrain::Coast)
}

/// Chebyshev distance from `p` to the nearest edge of a `width`x`height` grid.
fn edge_distance(width: i32, height: i32, p: Pos) -> i32 {
    p.x.min(p.y).min(width - 1 - p.x).min(height - 1 - p.y)
}

impl Survey {
    /// Generates `cfg`'s map and hides its buried resources, exactly as `World::bury` does.
    pub fn new(cfg: &MapGenConfig, defs: &Defs) -> Self {
        let mut surface = mapgen::generate(cfg, defs);
        let found: Vec<(Pos, ResourceId)> = surface
            .iter()
            .filter_map(|(p, t)| t.resource.filter(|&r| defs.resource(r).buried).map(|r| (p, r)))
            .collect();
        let mut buried = BTreeMap::new();
        for (p, r) in found {
            surface[p].resource = None;
            buried.insert(p, r);
        }
        let regions = Regions::compute(&surface, &MoveRules::default());
        Self { cfg: cfg.clone(), surface, buried, regions }
    }

    /// A `MapGenConfig` for `size`, `seed` plus the defaults with `size`'s dimensions and
    /// `continent_frequency` override, if any.
    pub fn config(seed: u64, size: &MapSizeDef) -> MapGenConfig {
        let defaults = MapGenConfig::default();
        MapGenConfig {
            seed,
            width: size.width,
            height: size.height,
            continent_frequency: size.continent_frequency.unwrap_or(defaults.continent_frequency),
            ..defaults
        }
    }

    /// The map a frontend draws: buried resources are already off it.
    pub fn map(&self) -> &TileMap {
        &self.surface
    }

    /// Consumes the survey for `World::found` (`world/found.rs`, #289): the map config, the
    /// surveyed map, the buried resources already hidden from it, and the region labelling
    /// `report` used to judge the site. Founding builds straight from these instead of a second
    /// `mapgen::generate` pass, so a map stays a function of its seed and size alone, and the
    /// regions a report approved are exactly the ones founding spawns into.
    pub(crate) fn into_parts(self) -> (MapGenConfig, TileMap, BTreeMap<Pos, ResourceId>, Regions) {
        (self.cfg, self.surface, self.buried, self.regions)
    }

    /// The valid site nearest the map centre.
    pub fn default_site(&self, defs: &Defs) -> Pos {
        let centre = Pos::new(self.surface.width() / 2, self.surface.height() / 2);
        let mut candidates: Vec<Pos> = self.surface.positions().collect();
        candidates.sort_by_key(|p| {
            let d2 = (p.x - centre.x).pow(2) + (p.y - centre.y).pow(2);
            (d2, *p)
        });
        candidates.into_iter().find(|&p| self.problem_at(p, defs).is_none()).unwrap_or(centre)
    }

    /// Why `at` can't be settled, or `None` if it can. Exact: refusals never read randomness.
    fn problem_at(&self, at: Pos, defs: &Defs) -> Option<SiteProblem> {
        let embark = &defs.embark;
        let Some(tile) = self.surface.get(at) else { return Some(SiteProblem::NotLand) };
        // Open land only: not water, not a mountain, not a ford, and nothing (a tree, a stone
        // outcrop) sitting on it blocking the region it would be in.
        let Some(region) = (is_open_land(tile.terrain)).then(|| self.regions.region(at)).flatten()
        else {
            return Some(SiteProblem::NotLand);
        };
        let edge = edge_distance(self.surface.width(), self.surface.height(), at);
        if edge < embark.min_edge_distance {
            return Some(SiteProblem::NearEdge);
        }
        if self.regions.size(region) < embark.min_room as usize {
            return Some(SiteProblem::TooSmall);
        }
        let open = self
            .box_tiles(at, embark.spawn_radius)
            .filter(|(_, t)| t.terrain.is_walkable() && t.resource.is_none())
            .count();
        if open < SETTLERS as usize {
            return Some(SiteProblem::NoRoom);
        }
        None
    }

    /// In-bounds tiles of the `(2*radius+1)^2` box centred on `at`, row-major.
    fn box_tiles(&self, at: Pos, radius: i32) -> impl Iterator<Item = (Pos, &Tile)> + '_ {
        (at.y - radius..=at.y + radius)
            .flat_map(move |y| (at.x - radius..=at.x + radius).map(move |x| Pos::new(x, y)))
            .filter_map(move |p| self.surface.get(p).map(|t| (p, t)))
    }

    /// Whether `region` is (tied for) the largest on the map.
    fn is_mainland(&self, region: RegionId) -> bool {
        let max = (0..self.regions.count())
            .map(|i| self.regions.size(RegionId(i as u32)))
            .max()
            .unwrap_or(0);
        self.regions.size(region) == max
    }

    /// `at`'s climate band (`World::climate_at`, #295), by the survey's own mapgen temperature;
    /// always exact, never jittered by a rough survey. Falls back to the coldest band if `at` is
    /// off the map, which `report` never shows (a refused site draws no climate line).
    fn climate_at(&self, at: Pos, defs: &Defs) -> ClimateDef {
        self.surface.get(at).map_or_else(
            || defs.seasons.climates.first().expect("validated: at least one climate band").clone(),
            |t| defs.seasons.climate_for(t.temperature).clone(),
        )
    }

    /// The nearest tile a unit could drink from, walking from `at` under the default
    /// `MoveRules` (8-connected, no corner cutting), capped at `WATER_SEARCH_CAP` steps. Terrain
    /// is always accurate, rough survey or not (`docs/design/embark.md`, "Rough survey").
    fn nearest_water(&self, at: Pos) -> Option<(Terrain, u32)> {
        self.nearest_water_where(at, Terrain::is_water)
    }

    /// `nearest_water`, but only counting water `wanted` accepts.
    fn nearest_water_where(&self, at: Pos, wanted: fn(Terrain) -> bool) -> Option<(Terrain, u32)> {
        let rules = MoveRules::default();
        let mut visited = BTreeSet::from([at]);
        let mut queue = VecDeque::from([(at, 0u32)]);
        while let Some((p, dist)) = queue.pop_front() {
            if wanted(self.surface[p].terrain) {
                return Some((self.surface[p].terrain, dist));
            }
            for q in self.surface.neighbors(p, true) {
                if wanted(self.surface[q].terrain) {
                    return Some((self.surface[q].terrain, dist));
                }
            }
            if dist >= WATER_SEARCH_CAP {
                continue;
            }
            for q in self.surface.neighbors(p, true) {
                if visited.insert(q) && rules.passable(&self.surface[q]) {
                    queue.push_back((q, dist + 1));
                }
            }
        }
        None
    }

    /// Walking steps from `at` to the nearest fresh water (a river, lake or ford: what a settler
    /// drinks without risk of sickness, `docs/design/harm.md`), capped at 40. `None` if there is
    /// none within the cap, or `at` isn't on the map.
    pub fn fresh_water_steps(&self, at: Pos) -> Option<u32> {
        if !self.surface.in_bounds(at) {
            return None;
        }
        self.nearest_water_where(at, |t| {
            matches!(t, Terrain::River | Terrain::Lake | Terrain::Ford)
        })
        .map(|(_, steps)| steps)
    }

    /// How many tiles within `radius` of `at` (Chebyshev) hold one of `resources` and touch
    /// ground connected to `at`: the survey's own count for a group, for any list of resources
    /// and radius. Zero if `at` is on no region.
    pub fn count_resources(&self, at: Pos, radius: i32, resources: &[ResourceId]) -> u32 {
        let Some(region) = self.regions.region(at) else { return 0 };
        self.count_in(at, radius, region, resources)
    }

    fn count_in(&self, at: Pos, radius: i32, region: RegionId, resources: &[ResourceId]) -> u32 {
        self.box_tiles(at, radius)
            .filter(|&(p, t)| {
                t.resource.is_some_and(|r| resources.contains(&r))
                    && self
                        .surface
                        .neighbors(p, true)
                        .any(|n| self.regions.region(n) == Some(region))
            })
            .count() as u32
    }

    /// Open ground a settler could walk to from `at` within `radius` tiles of it (Chebyshev),
    /// counting `at`: how much room there is to move, build and be cut off from. Walks the
    /// ordinary `MoveRules`, so a pond or a cliff line reduces it as it does a real settler's
    /// reach.
    pub fn open_tiles_near(&self, at: Pos, radius: i32) -> u32 {
        let rules = MoveRules::default();
        if !self.surface.in_bounds(at) || !rules.passable(&self.surface[at]) {
            return 0;
        }
        let mut seen = BTreeSet::from([at]);
        let mut queue = VecDeque::from([at]);
        while let Some(p) = queue.pop_front() {
            for q in self.surface.neighbors(p, true) {
                if q.chebyshev(at) <= radius && rules.passable(&self.surface[q]) && seen.insert(q) {
                    queue.push_back(q);
                }
            }
        }
        seen.len() as u32
    }

    /// The share of `at`'s box per biome, plus mountain and water, largest first (stable on
    /// ties, so biome file order then mountain then water decides them).
    fn biome_shares(&self, at: Pos, radius: i32, defs: &Defs) -> Vec<(LandKind, u8)> {
        let mountain_i = defs.biomes.len();
        let water_i = defs.biomes.len() + 1;
        let mut counts = vec![0u32; defs.biomes.len() + 2];
        let mut total = 0u32;
        for (_, t) in self.box_tiles(at, radius) {
            total += 1;
            let i = if t.terrain == Terrain::Mountain {
                mountain_i
            } else if t.terrain.is_water() {
                water_i
            } else {
                t.biome.0 as usize
            };
            counts[i] += 1;
        }
        if total == 0 {
            return Vec::new();
        }
        let mut shares: Vec<(LandKind, u32)> = counts
            .iter()
            .enumerate()
            .filter(|&(_, &c)| c > 0)
            .map(|(i, &c)| {
                let kind = if i == mountain_i {
                    LandKind::Mountain
                } else if i == water_i {
                    LandKind::Water
                } else {
                    LandKind::Biome(BiomeId(i as u8))
                };
                (kind, c)
            })
            .collect();
        shares.sort_by_key(|&(_, c)| std::cmp::Reverse(c));
        shares
            .into_iter()
            .map(|(k, c)| (k, ((f64::from(c) * 100.0 / f64::from(total)).round() as u8)))
            .collect()
    }

    /// `g`'s true count in `at`'s box: buried resources are already off `surface`, so they never
    /// count. A deposit only counts once it touches (8 neighbours) a walkable tile of `region`
    /// (`at`'s own region, same rule `deposit_supply` uses for the supply survey, #396): a vein
    /// or grove the box can see across water or a sealed-off pocket of land isn't actually
    /// reachable from the site, so the report shouldn't count it as if it were.
    fn count_group(&self, at: Pos, radius: i32, region: RegionId, g: &GroupDef) -> u32 {
        self.count_in(at, radius, region, &g.resources)
    }

    /// A rough survey's estimate of `g`'s level at `at`: `true_level` moved one step with
    /// probability `error_chance`, keyed on (seed, site, group) so a site always reads the same
    /// however the cursor got there.
    fn estimate_level(&self, at: Pos, g: &GroupDef, true_level: Level, error_chance: f64) -> Level {
        let mut rng =
            rng::stream(self.cfg.seed, &format!("embark.rough.{}.{}.{}", at.x, at.y, g.id));
        use rand::RngExt;
        let roll: f64 = rng.random();
        if roll >= error_chance {
            return true_level;
        }
        let up: bool = rng.random();
        true_level.shifted(if up { 1 } else { -1 })
    }

    /// The report on `at`: land, room, water, resources and an outlook
    /// (`docs/design/embark.md`, "The site report"). Pure; `SurveyMode::Exact` reads no RNG.
    ///
    /// `preset` is the chosen difficulty preset's id (`World::difficulty`), or `None`; it decides
    /// whether a winter term joins the outlook (`docs/design/difficulty.md`, "Embark"). An id the
    /// data doesn't have resolves to identity, the same way `World::knobs` treats one.
    pub fn report(
        &self,
        at: Pos,
        mode: SurveyMode,
        preset: Option<&str>,
        defs: &Defs,
    ) -> SiteReport {
        let embark = &defs.embark;
        let rough = mode == SurveyMode::Rough;
        let climate = self.climate_at(at, defs);
        if let Some(problem) = self.problem_at(at, defs) {
            return SiteReport::refused(problem, climate, rough);
        }
        let biomes = self.biome_shares(at, embark.report_radius, defs);
        let region = self.regions.region(at).expect("checked by problem_at");
        let room = self.regions.size(region) as u32;
        let mainland = self.is_mainland(region);
        let water = self.nearest_water(at);

        let mut groups = Vec::with_capacity(embark.groups.len());
        let mut points: i32 = 0;
        let mut negatives = Vec::new();
        let mut positives = Vec::new();
        for g in &embark.groups {
            let count = self.count_group(at, embark.report_radius, region, g);
            let true_level = g.level_of(count);
            let (level, display_count, hidden) = match mode {
                SurveyMode::Exact => (true_level, Some(count), false),
                SurveyMode::Rough if g.rough_hidden => (Level::None, None, true),
                SurveyMode::Rough => {
                    (self.estimate_level(at, g, true_level, embark.rough.error_chance), None, false)
                }
            };
            if !hidden && g.outlook_weight > 0 {
                points += outlook_points(level) * g.outlook_weight as i32;
                match level {
                    Level::None => negatives.push(format!("no {} nearby", g.name.to_lowercase())),
                    Level::Scarce => negatives.push(format!("{} is scarce", g.name)),
                    Level::Plenty => positives.push(format!("{} plentiful", g.name)),
                    Level::Some => {}
                }
            }
            groups.push(GroupCount {
                id: g.id.clone(),
                name: g.name.clone(),
                count: display_count,
                level,
                hidden,
            });
        }
        points += water_points(water, embark.outlook.water_far, embark.outlook.water_weight);
        match water {
            None => negatives.push("no water within reach".into()),
            Some((_, steps)) if steps > embark.outlook.water_far => {
                negatives.push(format!("water {steps} steps away"));
            }
            _ => {}
        }

        // The winter term (`docs/design/difficulty.md`, "Embark"): this is how a difficulty
        // preset reaches the outlook, instead of shifting every verdict. Identity, and every
        // preset with `seasons` off (Sandbox), adds nothing.
        let knobs =
            preset.and_then(|id| defs.difficulty.preset(id)).unwrap_or(&defs.difficulty.identity);
        if knobs.seasons {
            let winter_points = (f64::from(climate.outlook_points)
                * f64::from(knobs.winter_severity))
            .round() as i32;
            points += winter_points;
            if winter_points > 0 {
                negatives.push("cold winters".to_owned());
            }
        }

        let outlook = classify_outlook(points, embark.outlook.fair_from, embark.outlook.hard_from);
        // The one or two facts that decided it: lead with what's actually wrong, if anything;
        // otherwise a highlight plus the walk to water, which is always worth knowing.
        let mut reasons: Vec<String> = if !negatives.is_empty() {
            negatives
        } else {
            let mut good: Vec<String> = positives.into_iter().take(1).collect();
            if let Some((_, steps)) = water {
                good.push(format!("water {steps} steps away"));
            }
            good
        };
        reasons.truncate(2);

        SiteReport {
            problem: None,
            biomes,
            room,
            mainland,
            water,
            groups,
            climate,
            outlook,
            reasons,
            rough,
        }
    }
}

/// What `Survey::deposit_supply` needs that doesn't change across the resources it's called for
/// (`Survey::supply`'s own loop): bundled so the function stays under clippy's argument limit.
struct SupplyCtx<'a> {
    at: Pos,
    region: Option<RegionId>,
    /// `Survey::bore_distances`, computed once for `region` and shared by every buried resource
    /// `supply` is asked about; `None` if `at` has no region (shouldn't happen for a valid site,
    /// but `supply` stays total rather than panicking).
    bore: Option<BTreeMap<Pos, u32>>,
    bore_limit: u32,
    defs: &'a Defs,
}

impl Survey {
    /// The supply survey (`docs/design/materials.md`, "What the map holds" and "Measuring it",
    /// #348): for `at`'s region, at each of `radii` (ascending, Chebyshev tiles from `at`), how
    /// many of each of `resources` are reachable and what they're worth under `defs`'s bundled
    /// `data/jobs.toml` yields.
    ///
    /// A deposit counts once it touches (8 neighbours) a walkable tile of the start's region.
    /// A buried resource (`gold_seam`) additionally counts up to `bore_limit` Bore Tunnel casts
    /// deep in the rock, chained one orthogonal tile at a time off that region exactly as
    /// `world::bending`'s chain rule requires, since nothing stands beside it otherwise; those
    /// are valued at `tunnel_count` instead of `count` (`DepositSupply::tunnelled`). A surface
    /// resource (`stone`, `gold_vein`) is never tunnel-aware: it's already exposed, so a
    /// deposit that touches nothing walkable simply isn't counted. Only Earth has Bore Tunnel
    /// and Seismic Sense, so a buried resource's worth is only real for it in practice; this
    /// method itself is nation-agnostic, the same way the map is.
    ///
    /// `resources` names what to survey, not the code: a new deposit kind (#134's metal ore, or
    /// #349's foothill boulders) is a caller's list, never a hard-coded id here. A resource with
    /// no `HarvestDef` reports zero reachability rather than panicking, so a caller can survey a
    /// find like `ancient_scroll` without special-casing it.
    pub fn supply(
        &self,
        at: Pos,
        resources: &[ResourceId],
        radii: &[i32],
        bore_limit: u32,
        defs: &Defs,
    ) -> Vec<RadiusSupply> {
        let region = self.regions.region(at);
        let ctx = SupplyCtx {
            at,
            region,
            bore: region.map(|r| self.bore_distances(r)),
            bore_limit,
            defs,
        };
        radii
            .iter()
            .map(|&radius| RadiusSupply {
                radius,
                deposits: resources
                    .iter()
                    .map(|&id| self.deposit_supply(&ctx, id, radius))
                    .collect(),
            })
            .collect()
    }

    /// `resource`'s reachable count and worth within `radius`, for `Survey::supply`.
    fn deposit_supply(&self, ctx: &SupplyCtx, resource: ResourceId, radius: i32) -> DepositSupply {
        let mut out = DepositSupply { resource, count: 0, tunnelled: 0, worth: 0 };
        let (Some(harvest), Some(region)) = (ctx.defs.harvest(resource), ctx.region) else {
            return out;
        };
        let buried = ctx.defs.resource(resource).buried;
        let positions: Vec<Pos> = if buried {
            self.buried.iter().filter(|&(_, &r)| r == resource).map(|(&p, _)| p).collect()
        } else {
            self.surface
                .iter()
                .filter(|(_, t)| t.resource == Some(resource))
                .map(|(p, _)| p)
                .collect()
        };
        for p in positions {
            if p.chebyshev(ctx.at) > radius {
                continue;
            }
            let direct =
                self.surface.neighbors(p, true).any(|n| self.regions.region(n) == Some(region));
            if direct {
                out.count += 1;
                out.worth += harvest.count;
                continue;
            }
            if !buried {
                continue; // exposed already: no walkable neighbour means truly unreachable
            }
            let nearest_bore = ctx.bore.as_ref().and_then(|dist| {
                self.surface
                    .neighbors(p, true)
                    .filter(|&n| self.borable(n))
                    .filter_map(|n| dist.get(&n).copied())
                    .min()
            });
            if let Some(bores) = nearest_bore
                && bores <= ctx.bore_limit
            {
                out.count += 1;
                out.tunnelled += 1;
                out.worth += harvest.tunnel_count.unwrap_or(harvest.count);
            }
        }
        out
    }

    /// Whether `p` is an empty `Terrain::Mountain` tile Bore Tunnel could target: not water or
    /// open land, nothing sitting on it, and not hiding a buried resource either, the same
    /// "something is in the way" rule `world::bending::validate_effect` applies to a real cast.
    /// A buried resource's own tile has no `resource` on `surface` (`Survey::new` hides it, as
    /// `World::bury` does), so that has to be checked against `buried` separately.
    fn borable(&self, p: Pos) -> bool {
        self.surface.get(p).is_some_and(|t| t.terrain == Terrain::Mountain && t.resource.is_none())
            && !self.buried.contains_key(&p)
    }

    /// For every tile `borable` allows, how many Bore Tunnel casts, chained one orthogonal tile
    /// at a time off `region`'s own walkable ground, it would take to turn that tile into a
    /// tunnel (`world::bending`'s `has_bank`/chain rule for `Effect::Reshape`, #132). A
    /// multi-source BFS from every such tile that already has a walkable neighbour in `region`
    /// (distance 1, one bore).
    fn bore_distances(&self, region: RegionId) -> BTreeMap<Pos, u32> {
        let mut dist: BTreeMap<Pos, u32> = BTreeMap::new();
        let mut queue: VecDeque<Pos> = VecDeque::new();
        for (p, _) in self.surface.iter().filter(|&(p, _)| self.borable(p)) {
            if self.surface.neighbors(p, false).any(|n| self.regions.region(n) == Some(region)) {
                dist.insert(p, 1);
                queue.push_back(p);
            }
        }
        while let Some(p) = queue.pop_front() {
            let d = dist[&p];
            for n in self.surface.neighbors(p, false) {
                if self.borable(n) {
                    dist.entry(n).or_insert_with(|| {
                        queue.push_back(n);
                        d + 1
                    });
                }
            }
        }
        dist
    }
}

/// Outlook points a level contributes on its own (before the group's weight): `none` is worth
/// twice `scarce`, and `some`/`plenty` never cost anything (`docs/design/embark.md`, "The site
/// report").
fn outlook_points(level: Level) -> i32 {
    match level {
        Level::None => 2,
        Level::Scarce => 1,
        Level::Some | Level::Plenty => 0,
    }
}

/// Outlook points the walk to water contributes: `water_weight` beyond `water_far`, doubled when
/// there is none within the cap.
fn water_points(water: Option<(Terrain, u32)>, water_far: u32, water_weight: u32) -> i32 {
    match water {
        None => 2 * water_weight as i32,
        Some((_, steps)) if steps > water_far => water_weight as i32,
        Some(_) => 0,
    }
}

fn classify_outlook(points: i32, fair_from: u32, hard_from: u32) -> Outlook {
    if points >= hard_from as i32 {
        Outlook::Hard
    } else if points >= fair_from as i32 {
        Outlook::Fair
    } else {
        Outlook::Easy
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::defs::BiomeId;
    use crate::geom::Grid;
    use crate::nation::Nation;

    const EMBARK: &str = include_str!("../../../data/embark.toml");

    fn defs_without_embark() -> Defs {
        Defs::from_toml(
            include_str!("../../../data/biomes.toml"),
            include_str!("../../../data/resources.toml"),
            include_str!("../../../data/nations.toml"),
            include_str!("../../../data/research.toml"),
            include_str!("../../../data/needs.toml"),
            include_str!("../../../data/jobs.toml"),
            include_str!("../../../data/bending.toml"),
            include_str!("../../../data/buildings.toml"),
        )
        .unwrap()
    }

    // ---- data/embark.toml ----

    #[test]
    fn unloaded_defs_have_no_embark_settings() {
        let defs = defs_without_embark();
        assert!(defs.embark.map_sizes.is_empty());
        assert!(defs.embark.groups.is_empty());
    }

    #[test]
    fn bundled_embark_data_is_valid() {
        let defs = Defs::bundled();
        assert_eq!(defs.embark.map_size("small").unwrap().width, 192);
        assert!(defs.embark.map_size("medium").is_some());
        assert!(defs.embark.map_size("large").is_some());
        assert!(defs.embark.groups.iter().any(|g| g.id == "wood"));
    }

    #[test]
    fn unknown_resource_in_a_group_is_rejected() {
        let bad = EMBARK.replacen("\"oak_tree\"", "\"oak\"", 1);
        let err = defs_without_embark().with_embark(&bad).unwrap_err();
        assert!(err.0.contains("oak"), "{err}");
    }

    #[test]
    fn group_levels_must_ascend() {
        let bad = EMBARK.replacen("levels = [1, 10, 30]", "levels = [10, 1, 30]", 1);
        let err = defs_without_embark().with_embark(&bad).unwrap_err();
        assert!(err.0.contains("ascend"), "{err}");
    }

    #[test]
    fn duplicate_group_or_map_size_ids_are_rejected() {
        let dup_group = format!(
            "{EMBARK}\n[[group]]\nid = \"wood\"\nname = \"W\"\nresources = []\nlevels = [1, 2, 3]\noutlook_weight = 0\n"
        );
        assert!(defs_without_embark().with_embark(&dup_group).unwrap_err().0.contains("duplicate"));
        let dup_size = format!(
            "{EMBARK}\n[[map_size]]\nid = \"small\"\nname = \"S\"\nwidth = 10\nheight = 10\n"
        );
        assert!(defs_without_embark().with_embark(&dup_size).unwrap_err().0.contains("duplicate"));
    }

    #[test]
    fn config_builds_a_mapgen_config_from_a_preset() {
        let defs = Defs::bundled();
        let small = defs.embark.map_size("small").unwrap();
        let cfg = Survey::config(7, small);
        assert_eq!((cfg.width, cfg.height, cfg.seed), (192, 128, 7));
        assert_eq!(cfg.continent_frequency, MapGenConfig::default().continent_frequency);

        let medium = defs.embark.map_size("medium").unwrap();
        let cfg = Survey::config(7, medium);
        assert_eq!((cfg.width, cfg.height), (288, 192));
        assert_eq!(cfg.continent_frequency, medium.continent_frequency.unwrap());
        assert_ne!(cfg.continent_frequency, MapGenConfig::default().continent_frequency);
    }

    // ---- Survey ----

    /// Builds a tiny map from ASCII rows for `problem_at` tests: `.` land, `,` coast, `~` water,
    /// `^` mountain, `T` a land tile with a resource on it (blocks movement, like a tree).
    fn tiny_map(rows: &[&str]) -> TileMap {
        Grid::from_fn(rows[0].len() as i32, rows.len() as i32, |p| {
            let c = rows[p.y as usize].as_bytes()[p.x as usize] as char;
            let terrain = match c {
                '.' | 'T' => Terrain::Land,
                ',' => Terrain::Coast,
                '~' => Terrain::Water,
                '^' => Terrain::Mountain,
                _ => panic!("bad map char {c:?}"),
            };
            Tile {
                elevation: 0,
                temperature: 128,
                moisture: 128,
                terrain,
                biome: BiomeId(0),
                resource: (c == 'T').then_some(ResourceId(0)),
                building: None,
            }
        })
    }

    fn survey_of(map: TileMap) -> Survey {
        let regions = Regions::compute(&map, &MoveRules::default());
        let cfg = MapGenConfig { width: map.width(), height: map.height(), ..Default::default() };
        Survey { cfg, surface: map, buried: BTreeMap::new(), regions }
    }

    /// `EmbarkDef` with just the fields `problem_at` reads; `report_radius` wide enough for the
    /// small test maps, no groups.
    fn test_embark(min_edge_distance: i32, min_room: u32, spawn_radius: i32) -> EmbarkDef {
        EmbarkDef {
            report_radius: 2,
            min_edge_distance,
            min_room,
            spawn_radius,
            ..Default::default()
        }
    }

    fn with_embark_def(embark: EmbarkDef) -> Defs {
        let mut defs = Defs::bundled();
        defs.embark = embark;
        defs
    }

    #[test]
    fn not_land_on_water_mountain_or_a_resource_blocked_tile() {
        let survey = survey_of(tiny_map(&[
            ".....", //
            ".~^T.", //
            ".....",
        ]));
        // spawn_radius 2 reaches the 7 open tiles around the corner, enough for `SETTLERS`.
        let defs = with_embark_def(test_embark(0, 1, 2));
        assert_eq!(survey.problem_at(Pos::new(1, 1), &defs), Some(SiteProblem::NotLand));
        assert_eq!(survey.problem_at(Pos::new(2, 1), &defs), Some(SiteProblem::NotLand));
        assert_eq!(survey.problem_at(Pos::new(3, 1), &defs), Some(SiteProblem::NotLand));
        assert_eq!(survey.problem_at(Pos::new(0, 0), &defs), None);
    }

    #[test]
    fn near_edge_when_closer_than_min_edge_distance() {
        let rows: Vec<String> = (0..11).map(|_| ".".repeat(11)).collect();
        let rows: Vec<&str> = rows.iter().map(String::as_str).collect();
        let survey = survey_of(tiny_map(&rows));
        let defs = with_embark_def(test_embark(3, 1, 2));
        assert_eq!(survey.problem_at(Pos::new(2, 5), &defs), Some(SiteProblem::NearEdge));
        assert_eq!(survey.problem_at(Pos::new(3, 5), &defs), None);
    }

    #[test]
    fn too_small_when_the_landmass_is_smaller_than_min_room() {
        // A 3x3 open island (9 tiles, enough room for `SETTLERS`), ringed by water.
        let survey = survey_of(tiny_map(&[
            "~~~~~", //
            "~...~", //
            "~...~", //
            "~...~", //
            "~~~~~",
        ]));
        let defs = with_embark_def(test_embark(0, 100, 1));
        assert_eq!(survey.problem_at(Pos::new(2, 2), &defs), Some(SiteProblem::TooSmall));
        let defs = with_embark_def(test_embark(0, 9, 1));
        assert_eq!(survey.problem_at(Pos::new(2, 2), &defs), None);
    }

    #[test]
    fn no_room_when_too_few_open_tiles_near_the_start() {
        // A 5x5 open field, but every tile within one step of the centre except the centre
        // itself is blocked by a resource: 1 open tile where `SETTLERS` (6) need to fit.
        let survey = survey_of(tiny_map(&[
            ".....", //
            ".TTT.", //
            ".T.T.", //
            ".TTT.", //
            ".....",
        ]));
        let defs = with_embark_def(test_embark(0, 1, 1));
        assert_eq!(survey.problem_at(Pos::new(2, 2), &defs), Some(SiteProblem::NoRoom));
        // Radius 2 reaches past the ring of trees to the open border: 17 open tiles.
        let defs = with_embark_def(test_embark(0, 1, 2));
        assert_eq!(survey.problem_at(Pos::new(2, 2), &defs), None);
    }

    #[test]
    fn default_site_is_valid_and_nearest_the_centre() {
        let defs = Defs::bundled();
        for seed in [1, 7, 42] {
            let cfg = Survey::config(seed, defs.embark.map_size("small").unwrap());
            let survey = Survey::new(&cfg, &defs);
            let site = survey.default_site(&defs);
            let report = survey.report(site, SurveyMode::Exact, None, &defs);
            assert_eq!(report.problem, None, "seed {seed}: default site {site:?} is refused");
        }
    }

    #[test]
    fn survey_hides_buried_resources_like_world_bury() {
        let defs = Defs::bundled();
        let cfg = MapGenConfig { seed: 3, width: 64, height: 64, ..Default::default() };
        let raw = mapgen::generate(&cfg, &defs);
        let world = crate::world::World::with_map(cfg.seed, raw, Nation::Earth, &[], &defs);
        let survey = Survey::new(&cfg, &defs);
        assert_eq!(survey.map(), &world.map);
        assert!(
            survey.map().iter().all(|(_, t)| t.resource.is_none_or(|r| !defs.resource(r).buried)),
            "no buried resource should ever show on the survey map"
        );
    }

    #[test]
    fn report_never_shows_a_buried_resource_in_its_counts() {
        // Seed 7's default-size map has a gold seam at (103, 89) and nothing else buried nearby;
        // a report centred close enough to have it in the box must not count it.
        let defs = Defs::bundled();
        let cfg = Survey::config(7, defs.embark.map_size("small").unwrap());
        let survey = Survey::new(&cfg, &defs);
        let at = Pos::new(98, 79);
        assert!(at.chebyshev(Pos::new(103, 89)) <= defs.embark.report_radius);
        let report = survey.report(at, SurveyMode::Exact, None, &defs);
        let gold = report.groups.iter().find(|g| g.id == "gold").unwrap();
        // The surfaced gold veins in the box (verified against the raw, pre-hidden map below;
        // #349's denser veins put three here, up from one); the buried seam at (103, 89) must
        // not add to that count.
        let raw = mapgen::generate(&cfg, &defs);
        let vein = defs.resource_id("gold_vein").unwrap();
        let seam = defs.resource_id("gold_seam").unwrap();
        let in_box = |id: ResourceId| {
            raw.iter().filter(|(p, t)| t.resource == Some(id) && p.chebyshev(at) <= 10).count()
        };
        assert_eq!(in_box(vein), 3, "test assumption: three surfaced veins in the box");
        assert_eq!(in_box(seam), 1, "test assumption: one buried seam in the box");
        assert_eq!(gold.count, Some(3), "the buried seam must not be counted");
    }

    // ---- The supply survey (#348) ----

    #[test]
    fn supply_is_deterministic() {
        let defs = Defs::bundled();
        let cfg = Survey::config(7, defs.embark.map_size("small").unwrap());
        let survey = Survey::new(&cfg, &defs);
        let start = survey.default_site(&defs);
        let resources =
            [defs.resource_id("stone").unwrap(), defs.resource_id("gold_seam").unwrap()];
        let a = survey.supply(start, &resources, &[20, 40], 2, &defs);
        let b = survey.supply(start, &resources, &[20, 40], 2, &defs);
        assert_eq!(a, b);
    }

    #[test]
    fn supply_pins_seed_7_small_counts() {
        // A regression pin (#348's acceptance criteria): if a data or sim change moves these
        // numbers, `docs/design/materials.md`'s own trial needs a second look too.
        // #349 re-pinned these after richer outcrops, denser veins and foothill boulders
        // (`boulder` is counted alongside `stone`, per the doc's "outcrops + boulders").
        let defs = Defs::bundled();
        let cfg = Survey::config(7, defs.embark.map_size("small").unwrap());
        let survey = Survey::new(&cfg, &defs);
        let start = survey.default_site(&defs);
        let stone = defs.resource_id("stone").unwrap();
        let boulder = defs.resource_id("boulder").unwrap();
        let vein = defs.resource_id("gold_vein").unwrap();
        let seam = defs.resource_id("gold_seam").unwrap();
        let report = survey.supply(start, &[stone, boulder, vein, seam], &[20, 40], 2, &defs);

        assert_eq!(report[0].radius, 20);
        assert_eq!(
            report[0].deposits,
            vec![
                DepositSupply { resource: stone, count: 3, tunnelled: 0, worth: 30 },
                DepositSupply { resource: boulder, count: 11, tunnelled: 0, worth: 66 },
                DepositSupply { resource: vein, count: 2, tunnelled: 0, worth: 6 },
                DepositSupply { resource: seam, count: 1, tunnelled: 0, worth: 3 },
            ]
        );
        assert_eq!(report[1].radius, 40);
        assert_eq!(
            report[1].deposits,
            vec![
                DepositSupply { resource: stone, count: 25, tunnelled: 0, worth: 250 },
                DepositSupply { resource: boulder, count: 30, tunnelled: 0, worth: 180 },
                DepositSupply { resource: vein, count: 16, tunnelled: 0, worth: 48 },
                DepositSupply { resource: seam, count: 6, tunnelled: 3, worth: 24 },
            ]
        );
    }

    #[test]
    fn supply_count_never_shrinks_as_the_radius_grows() {
        let defs = Defs::bundled();
        let stone = defs.resource_id("stone").unwrap();
        for seed in 1..=8u64 {
            let cfg = Survey::config(seed, defs.embark.map_size("small").unwrap());
            let survey = Survey::new(&cfg, &defs);
            let start = survey.default_site(&defs);
            let report = survey.supply(start, &[stone], &[20, 40], 2, &defs);
            assert!(
                report[0].deposits[0].count <= report[1].deposits[0].count,
                "seed {seed}: {report:?}"
            );
        }
    }

    #[test]
    fn supply_is_generic_over_a_buried_find_with_no_tunnel_count() {
        // lost_ruins is buried like gold_seam but its harvest (data/jobs.toml) defines no
        // `tunnel_count`: nothing here is specific to gold_seam's id or its numbers.
        let defs = Defs::bundled();
        let ruins = defs.resource_id("lost_ruins").unwrap();
        let harvest = defs.harvest(ruins).unwrap();
        assert!(harvest.tunnel_count.is_none());
        for seed in 1..=8u64 {
            let cfg = Survey::config(seed, defs.embark.map_size("small").unwrap());
            let survey = Survey::new(&cfg, &defs);
            let start = survey.default_site(&defs);
            let report = survey.supply(start, &[ruins], &[60], 2, &defs);
            let d = &report[0].deposits[0];
            assert_eq!(d.worth, d.count * harvest.count, "seed {seed}: tunnelled {d:?}");
        }
    }

    #[test]
    fn supply_counts_a_buried_seam_only_within_its_bore_limit() {
        // A one-row map: open ground at x=0..=1, then a chain of empty mountain tiles, then a
        // buried seam at x=5 with no open neighbour of its own. Each mountain tile's bore
        // distance grows by one per step away from the open ground (x=2 is 1 bore from the
        // bank, x=3 is 2, x=4 is 3), so the seam -- reachable only by boring the x=4 tile next
        // to it -- needs exactly 3 bores. `x=6` dangles past the seam with no bank-connected
        // mountain tile of its own (its only neighbour is the seam, which can't be bored
        // through), so it never gets a distance at all: boring in from the far side isn't a way
        // around the limit.
        let defs = Defs::bundled();
        let seam = defs.resource_id("gold_seam").unwrap();
        let harvest = defs.harvest(seam).unwrap();
        let mut surface = Grid::from_fn(7, 1, |p| Tile {
            elevation: 0,
            temperature: 128,
            moisture: 128,
            terrain: if p.x <= 1 { Terrain::Land } else { Terrain::Mountain },
            biome: BiomeId(0),
            resource: None,
            building: None,
        });
        let seam_pos = Pos::new(5, 0);
        surface[seam_pos].resource = None; // hidden, as `Survey::new` leaves a buried tile
        let buried = BTreeMap::from([(seam_pos, seam)]);
        let regions = Regions::compute(&surface, &MoveRules::default());
        let cfg = MapGenConfig { width: 7, height: 1, ..Default::default() };
        let survey = Survey { cfg, surface, buried, regions };
        let start = Pos::new(0, 0);

        let too_tight = survey.supply(start, &[seam], &[10], 2, &defs);
        assert_eq!(
            too_tight[0].deposits[0],
            DepositSupply { resource: seam, count: 0, tunnelled: 0, worth: 0 }
        );

        let enough = survey.supply(start, &[seam], &[10], 3, &defs);
        assert_eq!(
            enough[0].deposits[0],
            DepositSupply {
                resource: seam,
                count: 1,
                tunnelled: 1,
                worth: harvest.tunnel_count.unwrap(),
            }
        );
    }

    // ---- The site report ----

    #[test]
    fn level_shift_clamps_at_none_and_plenty() {
        assert_eq!(Level::None.shifted(-1), Level::None);
        assert_eq!(Level::Plenty.shifted(1), Level::Plenty);
        assert_eq!(Level::Scarce.shifted(1), Level::Some);
        assert_eq!(Level::Some.shifted(-1), Level::Scarce);
    }

    #[test]
    fn outlook_classification_follows_the_thresholds() {
        assert_eq!(classify_outlook(0, 1, 4), Outlook::Easy);
        assert_eq!(classify_outlook(1, 1, 4), Outlook::Fair);
        assert_eq!(classify_outlook(3, 1, 4), Outlook::Fair);
        assert_eq!(classify_outlook(4, 1, 4), Outlook::Hard);
    }

    #[test]
    fn water_points_double_when_nothing_is_within_the_cap() {
        assert_eq!(water_points(None, 15, 2), 4);
        assert_eq!(water_points(Some((Terrain::Water, 20)), 15, 2), 2);
        assert_eq!(water_points(Some((Terrain::Water, 5)), 15, 2), 0);
    }

    #[test]
    fn a_refused_site_s_report_carries_the_problem_and_nothing_else() {
        let survey = survey_of(tiny_map(&[".~."]));
        let defs = with_embark_def(test_embark(0, 1, 0));
        let r = survey.report(Pos::new(1, 0), SurveyMode::Exact, None, &defs);
        assert_eq!(r.problem, Some(SiteProblem::NotLand));
        assert!(r.groups.is_empty() && r.biomes.is_empty() && r.water.is_none());
    }

    #[test]
    fn rough_survey_hides_gold_and_scrolls() {
        let defs = Defs::bundled();
        let cfg = Survey::config(7, defs.embark.map_size("small").unwrap());
        let survey = Survey::new(&cfg, &defs);
        let site = survey.default_site(&defs);
        let report = survey.report(site, SurveyMode::Rough, None, &defs);
        assert!(report.rough);
        for g in &report.groups {
            if g.id == "gold" || g.id == "scrolls" {
                assert!(g.hidden, "{} should be hidden in a rough survey", g.id);
                assert_eq!(g.count, None);
            } else {
                assert!(!g.hidden);
            }
        }
    }

    #[test]
    fn rough_survey_is_deterministic_and_sometimes_wrong() {
        let defs = Defs::bundled();
        let small = defs.embark.map_size("small").unwrap();
        let mut any_difference = false;
        for seed in 0..20 {
            let cfg = Survey::config(seed, small);
            let survey = Survey::new(&cfg, &defs);
            let site = survey.default_site(&defs);
            let a = survey.report(site, SurveyMode::Rough, None, &defs);
            let b = survey.report(site, SurveyMode::Rough, None, &defs);
            assert_eq!(a, b, "seed {seed}: two calls at the same site must agree");

            // Visiting other sites first (a different "cursor path") must not change the
            // reading at `site`.
            let other = Pos::new(site.x.saturating_sub(5), site.y);
            let _ = survey.report(other, SurveyMode::Rough, None, &defs);
            let _ = survey.report(
                Pos::new(site.x, site.y.saturating_add(3)),
                SurveyMode::Rough,
                None,
                &defs,
            );
            let c = survey.report(site, SurveyMode::Rough, None, &defs);
            assert_eq!(a, c, "seed {seed}: unrelated reports in between must not change it");

            let exact = survey.report(site, SurveyMode::Exact, None, &defs);
            let levels_differ = a
                .groups
                .iter()
                .zip(exact.groups.iter())
                .any(|(r, e)| !r.hidden && r.level != e.level);
            any_difference |= levels_differ;
        }
        assert!(any_difference, "expected at least one seed to read a jittered level");
    }

    #[test]
    fn rough_survey_never_jitters_the_walk_to_water() {
        let defs = Defs::bundled();
        let cfg = Survey::config(7, defs.embark.map_size("small").unwrap());
        let survey = Survey::new(&cfg, &defs);
        let site = survey.default_site(&defs);
        let exact = survey.report(site, SurveyMode::Exact, None, &defs);
        let rough = survey.report(site, SurveyMode::Rough, None, &defs);
        assert_eq!(exact.water, rough.water);
    }

    // ---- Climate and the winter term (#297, docs/design/difficulty.md "Embark") ----

    #[test]
    fn rough_survey_never_jitters_the_climate_band() {
        let defs = Defs::bundled();
        let cfg = Survey::config(7, defs.embark.map_size("small").unwrap());
        let survey = Survey::new(&cfg, &defs);
        let site = survey.default_site(&defs);
        let exact = survey.report(site, SurveyMode::Exact, Some("normal"), &defs);
        let rough = survey.report(site, SurveyMode::Rough, Some("normal"), &defs);
        assert_eq!(exact.climate, rough.climate);
    }

    /// A 5x5 all-land map, every tile at mapgen temperature 0 (polar), with one water tile at a
    /// corner close enough that the walk to it never adds outlook points: isolates the winter
    /// term from everything else `report` scores.
    fn polar_survey() -> Survey {
        let map = Grid::from_fn(5, 5, |p| {
            let (terrain, temperature) =
                if p == Pos::new(0, 0) { (Terrain::Water, 0) } else { (Terrain::Land, 0) };
            Tile {
                elevation: 0,
                temperature,
                moisture: 128,
                terrain,
                biome: BiomeId(0),
                resource: None,
                building: None,
            }
        });
        survey_of(map)
    }

    /// `fair_from`/`hard_from` set so the winter term alone (no groups, water close enough to
    /// cost nothing) decides the outlook: Normal's rounded term (2) clears `fair_from` but not
    /// `hard_from`; Hard's (3, `winter_severity` 1.3) clears both.
    fn polar_embark() -> Defs {
        with_embark_def(EmbarkDef {
            report_radius: 2,
            min_edge_distance: 0,
            min_room: 1,
            spawn_radius: 2,
            outlook: OutlookSettings { water_far: 15, water_weight: 2, fair_from: 2, hard_from: 3 },
            ..Default::default()
        })
    }

    #[test]
    fn a_polar_site_rates_harder_under_hard_than_normal_and_sandbox_ignores_it() {
        let defs = polar_embark();
        let survey = polar_survey();
        let site = Pos::new(2, 2);

        let none = survey.report(site, SurveyMode::Exact, None, &defs);
        assert_eq!(none.climate.id, "polar");
        assert_eq!(none.outlook, Outlook::Easy, "{none:?}");
        assert!(!none.reasons.iter().any(|r| r == "cold winters"));

        let sandbox = survey.report(site, SurveyMode::Exact, Some("sandbox"), &defs);
        assert_eq!(sandbox.outlook, none.outlook, "sandbox has no seasons: ignores climate");

        let normal = survey.report(site, SurveyMode::Exact, Some("normal"), &defs);
        assert_eq!(normal.outlook, Outlook::Fair, "{normal:?}");
        assert!(normal.reasons.iter().any(|r| r == "cold winters"));

        let hard = survey.report(site, SurveyMode::Exact, Some("hard"), &defs);
        assert_eq!(hard.outlook, Outlook::Hard, "{hard:?}");
        assert!(hard.reasons.iter().any(|r| r == "cold winters"));
    }

    #[test]
    fn an_unknown_preset_id_resolves_to_identity_like_world_knobs_does() {
        let defs = polar_embark();
        let survey = polar_survey();
        let site = Pos::new(2, 2);
        let none = survey.report(site, SurveyMode::Exact, None, &defs);
        let unknown = survey.report(site, SurveyMode::Exact, Some("extreme"), &defs);
        assert_eq!(none.outlook, unknown.outlook);
    }
}
