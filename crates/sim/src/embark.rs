//! Embark: surveying a generated map and reporting on a candidate site
//! (`docs/design/embark.md`, issue #288).
//!
//! `Survey` holds a generated map with its buried resources already hidden, exactly as
//! `World::bury` hides them, so a frontend can look at the map and ask for a report on any tile
//! before a world exists. `Survey::report` is pure and cheap (at most `(2*report_radius+1)^2`
//! tiles plus a walk capped at 40 steps), so it can run on every cursor move.
//!
//! Founding a world from a chosen site (`World::found`) is #289; the TUI screen is #290.

use std::collections::{BTreeMap, BTreeSet, VecDeque};

use serde::Deserialize;

use crate::defs::{BiomeId, Defs, DefsError, ResourceId, Terrain};
use crate::geom::Pos;
use crate::mapgen::{self, MapGenConfig, Tile, TileMap};
use crate::path::MoveRules;
use crate::regions::{RegionId, Regions};
use crate::rng;

/// Settlers a found world gets today (`fn_ui::STARTING_UNITS`). The report doesn't know the
/// count the player will found with (that is chosen later, #289), so it checks room for this
/// many while surveying. Only `SiteProblem::NoRoom` reads it.
const SETTLERS: u32 = 6;

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
    pub outlook: Outlook,
    /// The one or two facts that decided the outlook.
    pub reasons: Vec<String>,
    /// True when `mode` was `SurveyMode::Rough`.
    pub rough: bool,
}

impl SiteReport {
    fn refused(problem: SiteProblem, rough: bool) -> Self {
        Self {
            problem: Some(problem),
            biomes: Vec::new(),
            room: 0,
            mainland: false,
            water: None,
            groups: Vec::new(),
            outlook: Outlook::Hard,
            reasons: Vec::new(),
            rough,
        }
    }
}

/// A generated map with its buried resources hidden, ready to be surveyed for a site
/// (`docs/design/embark.md`, "The sim surveys first, then founds").
#[derive(Debug, Clone)]
pub struct Survey {
    cfg: MapGenConfig,
    surface: TileMap,
    #[allow(dead_code)] // kept for parity with `World::buried`; not read yet (founding is #289)
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

    /// The nearest tile a unit could drink from, walking from `at` under the default
    /// `MoveRules` (8-connected, no corner cutting), capped at `WATER_SEARCH_CAP` steps. Terrain
    /// is always accurate, rough survey or not (`docs/design/embark.md`, "Rough survey").
    fn nearest_water(&self, at: Pos) -> Option<(Terrain, u32)> {
        let rules = MoveRules::default();
        let mut visited = BTreeSet::from([at]);
        let mut queue = VecDeque::from([(at, 0u32)]);
        while let Some((p, dist)) = queue.pop_front() {
            if self.surface[p].terrain.is_water() {
                return Some((self.surface[p].terrain, dist));
            }
            for q in self.surface.neighbors(p, true) {
                if self.surface[q].terrain.is_water() {
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
    /// count.
    fn count_group(&self, at: Pos, radius: i32, g: &GroupDef) -> u32 {
        self.box_tiles(at, radius)
            .filter(|(_, t)| t.resource.is_some_and(|r| g.resources.contains(&r)))
            .count() as u32
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
    pub fn report(&self, at: Pos, mode: SurveyMode, defs: &Defs) -> SiteReport {
        let embark = &defs.embark;
        let rough = mode == SurveyMode::Rough;
        if let Some(problem) = self.problem_at(at, defs) {
            return SiteReport::refused(problem, rough);
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
            let count = self.count_group(at, embark.report_radius, g);
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

        SiteReport { problem: None, biomes, room, mainland, water, groups, outlook, reasons, rough }
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
            let report = survey.report(site, SurveyMode::Exact, &defs);
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
        let report = survey.report(at, SurveyMode::Exact, &defs);
        let gold = report.groups.iter().find(|g| g.id == "gold").unwrap();
        // Exactly the one surfaced gold vein in the box (verified against the raw, pre-hidden
        // map below); the buried seam at (103, 89) must not add to it.
        let raw = mapgen::generate(&cfg, &defs);
        let vein = defs.resource_id("gold_vein").unwrap();
        let seam = defs.resource_id("gold_seam").unwrap();
        let in_box = |id: ResourceId| {
            raw.iter().filter(|(p, t)| t.resource == Some(id) && p.chebyshev(at) <= 10).count()
        };
        assert_eq!(in_box(vein), 1, "test assumption: one surfaced vein in the box");
        assert_eq!(in_box(seam), 1, "test assumption: one buried seam in the box");
        assert_eq!(gold.count, Some(1), "the buried seam must not be counted");
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
        let r = survey.report(Pos::new(1, 0), SurveyMode::Exact, &defs);
        assert_eq!(r.problem, Some(SiteProblem::NotLand));
        assert!(r.groups.is_empty() && r.biomes.is_empty() && r.water.is_none());
    }

    #[test]
    fn rough_survey_hides_gold_and_scrolls() {
        let defs = Defs::bundled();
        let cfg = Survey::config(7, defs.embark.map_size("small").unwrap());
        let survey = Survey::new(&cfg, &defs);
        let site = survey.default_site(&defs);
        let report = survey.report(site, SurveyMode::Rough, &defs);
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
            let a = survey.report(site, SurveyMode::Rough, &defs);
            let b = survey.report(site, SurveyMode::Rough, &defs);
            assert_eq!(a, b, "seed {seed}: two calls at the same site must agree");

            // Visiting other sites first (a different "cursor path") must not change the
            // reading at `site`.
            let other = Pos::new(site.x.saturating_sub(5), site.y);
            let _ = survey.report(other, SurveyMode::Rough, &defs);
            let _ =
                survey.report(Pos::new(site.x, site.y.saturating_add(3)), SurveyMode::Rough, &defs);
            let c = survey.report(site, SurveyMode::Rough, &defs);
            assert_eq!(a, c, "seed {seed}: unrelated reports in between must not change it");

            let exact = survey.report(site, SurveyMode::Exact, &defs);
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
        let exact = survey.report(site, SurveyMode::Exact, &defs);
        let rough = survey.report(site, SurveyMode::Rough, &defs);
        assert_eq!(exact.water, rough.water);
    }
}
