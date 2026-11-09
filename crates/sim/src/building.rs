//! Buildings: constructible structures (`data/buildings.toml`, issue #64), each covering a
//! rectangle of tiles (`BuildingDef::footprint`, #207; docs/design/buildings.md).
//!
//! A `BuildingDef` just says what a building costs, how long it takes to build and whether units
//! may walk onto it once it's finished; everything about *designating* one, hauling its cost to
//! the site and the work that finishes it lives in `world/build.rs`. What a finished building
//! *does* is data on its def too:
//!
//! - `seats > 0` makes it a study hall: the moment one finishes construction `world/study.rs`
//!   opens that many `JobKind::Study` jobs on it (issue #21).
//! - `bed` makes it somewhere to sleep: a tired unit walks to the nearest free one and regains
//!   energy faster there (`world/work.rs`, issue #58). 1x1 only unless it also has a `capacity`
//!   (#227), in which case that many units sleep inside it at once, through the door, the same
//!   way a study hall's seats work.
//! - `rail` makes it a track (issue #135): a unit hauling a stack (a cart) goes faster along a
//!   line of them. The speed is data on the def; the tile carries a `Building::rail` flag so
//!   pathfinding never has to consult the defs (see `path.rs`, `MoveRules::with_rail`).
//! - `heat` makes it a permanent heat source (`world/heat.rs`, issue #58): meals eaten within
//!   its radius are cooked, and sleep within it is better. It is the same `HeatSource` Fire's
//!   Hearthflame lights for a while, so the cooking and warm-sleep code is shared.
//!
//! - `orchard` makes it a grove-keeper (issue #142): it plants and tends renewable trees around
//!   itself (`world/orchard.rs`). Every number is on `OrchardDef`.
//!
//! - `farm` makes it a farm plot (issue #198): empty until a `JobKind::Sow` job plants it, then
//!   it grows `grow_seconds` (scaled by `World::crop_growth_rate`, the hook irrigation and
//!   terraces will scale) before a `JobKind::Reap` job gathers its crop and it's empty again.
//!   The rules are in `world/farm.rs`; every number is on `FarmDef`.
//!
//! - `irrigation` makes it the irrigation channel (issue #126): farm plots within its `radius`
//!   grow `growth_boost` times as fast, the same way an orchard tends trees nearby. Every number
//!   is on `IrrigationDef`; the lookup is `World::irrigation_rate` in `world/farm.rs`.
//!
//! - `healing` makes it somewhere hurt settlers rest (issue #375, Water's Healing Hut): a unit
//!   whose health is below `seek_below` and whose needs are all comfortable walks to the nearest
//!   one with room and heals `regen_per_minute` times `modifier`, if named, until health reaches
//!   `stop_at` or a need pre-empts it (`world/work.rs`'s `Task::Healing`). Needs a `capacity`,
//!   the door it heals through, the same way a study hall's seats do. Every number is on
//!   `HealingDef`.
//!
//! - `endeavor` marks the end game's great work (the Harmony Gate, #236): it is never designated
//!   from the build picker. The Endeavor screen places it (`Command::BeginEndeavor`) and its
//!   stages in `data/endeavor.toml` carry its cost and work instead (`world/gate.rs`).
//!
//! - `beside` (issue #373) restricts *where* a building may go, on top of the usual per-tile
//!   checks: at least one tile of its footprint's ring (`Footprint::ring`) must be one of the
//!   named terrains, or `World::check_build` refuses it (`world/build.rs`'s `Blocker::Beside`),
//!   for buildings that work a vein from inside the rock (Earth's Mine Shaft and Deep Mine) or
//!   need open water alongside (a future Water building).
//!
//! - `power` makes a finished building a power source (issue #428, docs/design/late-tree.md):
//!   work inside any `uses_power` building within its reach runs faster. `PowerDef::radius` is
//!   scaled by the nation's `power_generation` modifier, the way `HeatDef::radius` is by
//!   `heating`; `World::power_at` (`world/power.rs`) is the lookup, and two sources in reach of
//!   one consumer don't stack. `uses_power = true` marks the consumer side: today the Workshop,
//!   the Forge, the Brick Kiln, the Smelter, the Mine Shaft, the Deep Mine and the study hall.
//!   Which buildings are sources is data, not this file's business (#255, #204, #200).
//!
//! - `chart` makes a finished building the Map Room (issue #402, docs/design/cartography.md):
//!   its `seats` become `JobKind::Chart` jobs instead of Study ones (`world/chart.rs`). A
//!   settler holding one slowly uncovers the nearest lost ruin the settlement hasn't found yet,
//!   among `finds` (buried resources, the same ids `Effect::Reveal` uses) within `range` tiles
//!   (Chebyshev) of the room, after `minutes` of seated work at a `study` modifier of 1.0.
//!
//! Which tech must be known before a building may be designated is `Unlocks::buildings` in
//! `data/research.toml`, resolved here into `BuildingDef::unlocked_by` (issue #22). A building no
//! tech names is available from the start.

use std::collections::HashMap;

use serde::Deserialize;

use crate::defs::{DefsError, ResourceDef, ResourceId, Terrain};
use crate::geom::{Footprint, Pos};
use crate::jobs::{ItemDef, ItemId};
use crate::nation::{Modifiers, Nation};
use crate::needs::NeedsSettings;
use crate::path::STRAIGHT;
use crate::research::{MAX_MULTIPLIER, TechDef, TechId};

// `Deserialize` is `save::ids`' remapping impl (#602).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, serde::Serialize)]
pub struct BuildingId(pub u16);

/// A building placed on a tile (`Tile::building`): baked onto the tile so pathfinding
/// (`path.rs`, `flood.rs`) never has to consult the defs to know whether it blocks movement.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Building {
    pub id: BuildingId,
    /// Whether units may walk onto this tile now that the building stands on it.
    pub passable: bool,
    /// Whether this is a rail tile (`BuildingDef::rail`, issue #135). Baked on like `passable`.
    /// Saves from before rail existed have none.
    #[serde(default)]
    pub rail: bool,
    /// This tile's offset from the building's anchor (its top-left tile, `[0, 0]`), `[x, y]`.
    /// Every tile of a footprint carries its own copy of the building (#207).
    #[serde(default, skip_serializing_if = "is_anchor")]
    pub part: [u8; 2],
    /// The footprint it was built with, `[w, h]`. Baked on like `passable`, never re-derived
    /// from `BuildingDef::footprint`, so a building keeps its size if the data changes later.
    /// Skipped when 1x1, so a world of 1x1 buildings saves as it did before footprints.
    #[serde(default = "one_by_one", skip_serializing_if = "is_one_by_one")]
    pub size: [u8; 2],
}

impl Building {
    /// A 1x1 building.
    pub fn new(id: BuildingId, passable: bool, rail: bool) -> Self {
        Self { id, passable, rail, part: [0, 0], size: ONE_BY_ONE }
    }

    /// The footprint this tile belongs to, given the tile's own position.
    pub fn footprint(&self, pos: Pos) -> Footprint {
        let anchor = Pos::new(pos.x - i32::from(self.part[0]), pos.y - i32::from(self.part[1]));
        Footprint::new(anchor, self.size[0], self.size[1])
    }
}

/// The size of a building or site with no footprint in the data.
pub const ONE_BY_ONE: [u8; 2] = [1, 1];

pub(crate) fn one_by_one() -> [u8; 2] {
    ONE_BY_ONE
}

pub(crate) fn is_one_by_one(size: &[u8; 2]) -> bool {
    *size == ONE_BY_ONE
}

fn is_anchor(part: &[u8; 2]) -> bool {
    *part == [0, 0]
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

/// A building that is a track (`[building.rail]`, issue #135).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RailDef {
    /// How many times faster than bare ground a hauling unit goes from one rail tile to the next
    /// one beside it (a straight step). More than 1.0.
    pub speed: f32,
}

impl RailDef {
    /// The cost of that straight step in the units of `path.rs` (a plain straight step is
    /// `STRAIGHT`), rounded and never less than 1.
    pub fn step_cost(&self) -> u32 {
        ((STRAIGHT as f32 / self.speed).round() as u32).clamp(1, STRAIGHT)
    }
}

/// A building that plants and tends trees around itself (`[building.orchard]`, issue #142). The
/// rules are in `world/orchard.rs`; the design is in docs/design/research.md ("The orchard").
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct OrchardDef {
    /// The resource it plants. Must be renewable (its harvest has a `regrow`), so a planted tree
    /// bears again after being picked.
    pub plants: ResourceId,
    /// How far from the building, Chebyshev, it plants and tends.
    pub radius: u32,
    /// How many trees it keeps going at once. It plants again only while it tends fewer.
    pub max_trees: u32,
    /// In-game seconds between plantings while it tends fewer than `max_trees`.
    pub plant_seconds: f32,
    /// In-game seconds from planting to the first fruit.
    pub grow_seconds: f32,
    /// How much faster than normal a resting tree of `plants` within `radius` regrows (at least
    /// 1.0), whoever planted it.
    pub tend_boost: f32,
    /// How far apart a new tree stands from any other resource, in tiles (Chebyshev): at least 1
    /// (touching is allowed) and at most `radius`. 2 leaves a walkable lane between trees.
    pub spacing: u32,
    /// Whether every tree it plants carries a standing harvest order (#81) from the start, so
    /// the fruit is picked and hauled without the player designating it each time.
    pub auto_pick: bool,
}

/// One item and the count of it a building costs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CostItem {
    pub item: ItemId,
    pub count: u32,
}

/// What a production building turns raw items into (`[building.recipe]`, issue #124): the
/// forge's smith job, a brick kiln, a smelter. `world/forge.rs` runs it: a unit holding the
/// building's craft job fetches `inputs` for one batch, a trip at a time, then works inside for
/// `work` seconds (scaled by `modifier`, if named) and the batch yields `output`, dropped near
/// the building for hauling, before the cycle starts again. Unlike a building's own `cost`, a
/// recipe never finishes: the building keeps making batches for as long as its inputs keep
/// coming, up to `stock_limit` (issue #350). `inputs` may be empty (issue #373): a source
/// recipe, like Earth's Mine Shaft cutting stone out of the rock it's built against. With
/// nothing to fetch, a crafter goes straight to working inside once a batch is free to start
/// (`World::next_craft_shortfall` finds no shortfall), so it still needs a worker and still
/// takes `work` seconds; `stock_limit` still idles it once enough output is stored.
#[derive(Debug, Clone, PartialEq)]
pub struct RecipeDef {
    /// What one batch consumes. May be empty (issue #373: a source recipe); no item named twice.
    pub inputs: Vec<CostItem>,
    /// What one batch yields.
    pub output: CostItem,
    /// In-game seconds of work once a batch's inputs are all in, at `modifier` 1.0.
    pub work: f32,
    /// Nation modifier (`Modifiers::get`) that scales the work speed, if any (the Fire production
    /// buildings all use `heating`).
    pub modifier: Option<String>,
    /// A crafter starts a new batch only while fewer than this many of `output` are in stores
    /// (`World::stored_total`, issue #350: the Brick Kiln and the smelter burn their inputs for
    /// an output nothing uses otherwise). A batch already under way finishes regardless. 0 (the
    /// default) means no limit.
    pub stock_limit: u32,
}

/// A building that grows a crop once sown (`[building.farm]`, issue #198). The rules are in
/// `world/farm.rs`; the design is in docs/design/research.md ("Farm plots").
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FarmDef {
    /// What a harvest yields (cabbage to start).
    pub crop: ItemId,
    /// How many of `crop` one harvest yields.
    pub harvest_count: u32,
    /// In-game seconds of work to plant an empty plot (`JobKind::Sow`).
    pub sow_seconds: f32,
    /// In-game seconds from planting to ripe, at a `crop_growth` of 1.0.
    pub grow_seconds: f32,
    /// In-game seconds of work to gather a ripe plot (`JobKind::Reap`).
    pub reap_seconds: f32,
    /// How many times faster than `grow_seconds` the crop grows: the plot's own rate before
    /// `World::crop_growth_rate` multiplies in the irrigation channel's boost (#126) and, later,
    /// terraces' (#136), the same way an orchard's `tend_boost` speeds up a tree. 1.0 for the
    /// plain plot.
    pub crop_growth: f32,
}

/// A building that speeds up farm plots nearby (`[building.irrigation]`, issue #126): the
/// `irrigation` tech's permanent version of Water's Water the Land ability (docs/design/
/// bending.md). The rules are in `world/farm.rs`, read by `World::crop_growth_rate` the same way
/// `OrchardDef::tend_boost` speeds up a resting tree.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct IrrigationDef {
    /// How far from the channel, Chebyshev, a farm plot is sped up.
    pub radius: u32,
    /// How many times faster a farm plot within `radius` grows (at least 1.0). Two channels in
    /// reach of one plot don't stack; it takes the best boost, the same as an orchard's
    /// `tend_boost`.
    pub growth_boost: f32,
}

/// A building that speeds up work in buildings nearby (`[building.power]`, issue #428,
/// docs/design/late-tree.md): the Steam Engine and Lightning Station (#255) set this; the
/// Windmill, Wind Turbine and Tide Mill (#204, #200) are out of scope here. The lookup is
/// `World::power_at` in `world/power.rs`, the third reader of the "best thing in reach, no
/// stacking" pattern heat and irrigation already answer.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PowerDef {
    /// Tiles around it, Chebyshev, that count as in reach at a `power_generation` modifier of
    /// 1.0. Scaled by the nation's, the same way `HeatDef::radius` is by `heating`
    /// (`PowerDef::radius_for`).
    pub radius: u32,
    /// How many times faster work runs in a powered building: above 1.0, at most 4.0.
    pub boost: f32,
    /// `boost * 1000`, rounded at load, so `world/study.rs` can scale its integer millis-per-tick
    /// rate without floating point (issue #428: the sim stays deterministic across platforms).
    pub boost_millis: u32,
}

impl PowerDef {
    /// The reach at a nation's `power_generation` modifier: rounded, and never less than 1.
    pub fn radius_for(&self, power_generation: f32) -> u32 {
        ((self.radius as f32 * power_generation).round() as u32).max(1)
    }
}

/// A building that charts the nearest lost ruin (`[building.chart]`, issue #402,
/// docs/design/cartography.md): the Map Room. `world/chart.rs` runs it.
#[derive(Debug, Clone, PartialEq)]
pub struct ChartDef {
    /// Buried resources (`ResourceDef::buried`) a finished chart may uncover.
    pub finds: Vec<ResourceId>,
    /// In-game minutes of seated work per chart, at a `study` modifier of 1.0.
    pub minutes: f32,
    /// Tiles around the room, Chebyshev, a chart may uncover within.
    pub range: u32,
}

/// A building units heal inside (`[building.healing]`, issue #375, narrowing #125, Water's
/// Healing Hut). A settler whose health is below `seek_below` and whose needs are all
/// comfortable (none at or below `low`, `Needs::any_low`) walks to the nearest one with a free
/// place and rests there (`world/work.rs`'s `Task::Healing`, entered through the door like a
/// study seat, so it needs a `capacity`), healing `regen_per_minute` times `modifier` until
/// health reaches `stop_at` or a need pre-empts it. Data-driven on purpose, so a later source of
/// harm (#385) can tune or add to what a healing building treats without reworking this.
#[derive(Debug, Clone, PartialEq)]
pub struct HealingDef {
    /// Health below which a hurt unit seeks this building out.
    pub seek_below: f32,
    /// Health regained per minute while resting here, before `modifier`. Always above
    /// `data/needs.toml`'s passive `health.regen_per_minute`, or the building would heal no
    /// faster than standing around.
    pub regen_per_minute: f32,
    /// Health this building heals up to, at or below `data/needs.toml`'s `health.max`; the unit
    /// leaves once it gets there (passive regen can still carry it the rest of the way).
    pub stop_at: f32,
    /// Nation modifier (`Modifiers::get`) that scales `regen_per_minute`, if any (Water's
    /// `water_work`).
    pub modifier: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct BuildingDef {
    pub id: String,
    pub name: String,
    /// What the designation is called in the UI ("Build").
    pub verb: String,
    /// What a construct job hauls to the site before work can start. Empty for an `endeavor`
    /// building, whose stages carry the cost.
    pub cost: Vec<CostItem>,
    /// In-game seconds of work at a `construction` modifier of 1.0. 0 for an `endeavor` building,
    /// whose stages carry the work.
    pub work: f32,
    /// Whether units may walk onto the tile once the building is finished.
    pub passable: bool,
    /// The tiles it covers, `(w, h)`, each 1 to 4 (#207). `(1, 1)` unless the data says
    /// otherwise.
    pub footprint: (u8, u8),
    /// How many units may study here at once. 0 unless this is a study hall (issue #21); a
    /// nonzero value is what tells `world/build.rs` to open that many `JobKind::Study` jobs when
    /// construction finishes.
    pub seats: u32,
    /// How many units may be inside at once (`docs/design/buildings.md`, "Units inside
    /// buildings", #226): defaults to `seats` (so a study hall needs no data of its own), else 0.
    /// Only a building with a capacity above 0 may be entered (`World::enter_building`). A `bed`
    /// bigger than 1x1 needs a capacity too: that many sleeping places inside it, rather than one
    /// sleeper lying on its single tile (#227).
    pub capacity: u32,
    /// Set if units sleep here (issue #58). On a 1x1 building the sleeper lies right on its
    /// tile; on a bigger one with a `capacity` (#227), each of that many places is a sleeper
    /// inside, the same as a study hall's seats.
    pub bed: Option<BedDef>,
    /// Set if this keeps a permanent fire: it cooks meals and warms sleep nearby (issue #58).
    pub heat: Option<HeatDef>,
    /// Set if this is a track: hauling units run faster along a line of them (issue #135).
    pub rail: Option<RailDef>,
    /// Set if this plants and tends trees around itself (issue #142).
    pub orchard: Option<OrchardDef>,
    /// Set if this is a production building: the forge, a brick kiln, a smelter (issue #124).
    /// Needs a `capacity` above 0, the number of crafters it seats, each through its door like a
    /// study hall's seats.
    pub recipe: Option<RecipeDef>,
    /// Set if this is a farm plot: empty until sown, then grows a crop to harvest (issue #198).
    pub farm: Option<FarmDef>,
    /// Set if this speeds up farm plots nearby: the irrigation channel (issue #126).
    pub irrigation: Option<IrrigationDef>,
    /// Set if units heal here: Water's Healing Hut (issue #375). Needs a `capacity` above 0,
    /// the number of places a hurt settler heals in at once, each through its door.
    pub healing: Option<HealingDef>,
    /// Set if this is a power source: work inside a `uses_power` building within its reach runs
    /// faster (issue #428, `world/power.rs`).
    pub power: Option<PowerDef>,
    /// Set if this is the Map Room (issue #402): its `seats` open `JobKind::Chart` jobs instead
    /// of Study ones (`world/chart.rs`).
    pub chart: Option<ChartDef>,
    /// Whether this building's recipe batches (`world/forge.rs`) or study seats
    /// (`world/study.rs`) run faster while a power source is in reach (issue #428). Has no
    /// effect without a `recipe` or `seats`.
    pub uses_power: bool,
    /// What each nation calls this building, if it varies (e.g. the study hall: Meditation
    /// Pavilion for Air). Empty when every nation uses the plain `name`. If set at all, every
    /// nation has an entry (`display_name` is the one place that matters).
    pub names: HashMap<Nation, String>,
    /// The tech that must be known before this building may be designated (`data/research.toml`,
    /// `Unlocks::buildings`, issue #22). `None` means every nation can build it from the start.
    pub unlocked_by: Option<TechId>,
    /// The end game's great work (#236): hidden from the build picker and refused by
    /// `Command::DesignateBuild`. `data/endeavor.toml` names it and holds its stages.
    pub endeavor: bool,
    /// Terrains at least one tile of the footprint's ring must touch (issue #373), or
    /// `World::check_build` refuses the placement. `None` means no such rule. Earth's Mine Shaft
    /// and Deep Mine need `["mountain", "tunnel"]`, the rock they work from inside.
    pub beside: Option<Vec<Terrain>>,
}

impl BuildingDef {
    /// The word naming what `beside` requires touching, for the placement ghost's refusal
    /// ("must touch rock", issue #373). `None` when this building has no `beside` rule.
    pub fn beside_word(&self) -> Option<&'static str> {
        let terrains = self.beside.as_ref()?;
        Some(if terrains.iter().all(|t| matches!(t, Terrain::Mountain | Terrain::Tunnel)) {
            "rock"
        } else if terrains.iter().all(|t| t.is_water()) {
            "water"
        } else {
            "the right ground"
        })
    }

    /// `names[nation]` if this building has a nation-specific name, else the shared `name`.
    pub fn display_name(&self, nation: Nation) -> &str {
        self.names.get(&nation).map(String::as_str).unwrap_or(&self.name)
    }
}

/// The longest side a footprint may have, in tiles.
const MAX_SIDE: u8 = 4;

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
struct RawRail {
    speed: f32,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawOrchard {
    plants: String,
    radius: u32,
    max_trees: u32,
    plant_seconds: f32,
    grow_seconds: f32,
    tend_boost: f32,
    spacing: u32,
    auto_pick: bool,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawRecipeInput {
    item: String,
    count: u32,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawRecipe {
    input: Vec<RawRecipeInput>,
    output: String,
    output_count: u32,
    work: f32,
    modifier: Option<String>,
    #[serde(default)]
    stock_limit: u32,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawFarm {
    crop: String,
    harvest_count: u32,
    sow_seconds: f32,
    grow_seconds: f32,
    reap_seconds: f32,
    crop_growth: f32,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawIrrigation {
    radius: u32,
    growth_boost: f32,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawHealing {
    seek_below: f32,
    regen_per_minute: f32,
    stop_at: f32,
    modifier: Option<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawPower {
    radius: u32,
    boost: f32,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawChart {
    finds: Vec<String>,
    minutes: f32,
    range: u32,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawBuilding {
    id: String,
    name: String,
    verb: String,
    #[serde(default)]
    cost: Vec<RawCost>,
    #[serde(default)]
    work: Option<f32>,
    passable: bool,
    #[serde(default = "one_by_one")]
    footprint: [u8; 2],
    #[serde(default)]
    seats: u32,
    /// Defaults to `seats` when absent (#226): most buildings with a capacity are study halls,
    /// where capacity and seats are the same number.
    #[serde(default)]
    capacity: Option<u32>,
    bed: Option<RawBed>,
    heat: Option<RawHeat>,
    rail: Option<RawRail>,
    orchard: Option<RawOrchard>,
    recipe: Option<RawRecipe>,
    farm: Option<RawFarm>,
    irrigation: Option<RawIrrigation>,
    healing: Option<RawHealing>,
    power: Option<RawPower>,
    chart: Option<RawChart>,
    #[serde(default)]
    uses_power: bool,
    #[serde(default)]
    names: HashMap<Nation, String>,
    #[serde(default)]
    endeavor: bool,
    /// `beside = ["mountain", "tunnel"]` (issue #373): the footprint's ring must touch one of
    /// these, or the placement is refused. Absent means no such rule.
    #[serde(default)]
    beside: Option<Vec<Terrain>>,
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
        resources: &[ResourceDef],
        needs: &NeedsSettings,
    ) -> Result<Vec<BuildingDef>, DefsError> {
        let err = |msg: String| Err(DefsError(format!("buildings.toml: {msg}")));
        let mut buildings: Vec<BuildingDef> = Vec::new();
        for b in self.building {
            if buildings.iter().any(|d| d.id == b.id) {
                return err(format!("duplicate building {}", b.id));
            }
            let work = match (b.endeavor, b.work) {
                (false, Some(w)) if w.is_finite() && w > 0.0 => w,
                (false, _) => {
                    return err(format!("building {}: work must be a positive number", b.id));
                }
                (true, None) if b.cost.is_empty() => 0.0,
                (true, _) => {
                    return err(format!(
                        "building {}: an endeavor building's cost and work are its stages', in \
                         endeavor.toml",
                        b.id
                    ));
                }
            };
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
            let [w, h] = b.footprint;
            if !(1..=MAX_SIDE).contains(&w) || !(1..=MAX_SIDE).contains(&h) {
                return err(format!(
                    "building {}: footprint sides must be 1 to {MAX_SIDE} tiles, not [{w}, {h}]",
                    b.id
                ));
            }
            let capacity = b.capacity.unwrap_or(b.seats);
            if b.bed.is_some() && b.footprint != ONE_BY_ONE && capacity == 0 {
                return err(format!(
                    "building {}: a bed bigger than 1x1 needs a capacity, the number of places \
                     inside it (one sleeper otherwise lies on its own 1x1 tile)",
                    b.id
                ));
            }
            if b.recipe.is_some() && capacity == 0 {
                return err(format!(
                    "building {}: a recipe needs a capacity, the door a crafter works through",
                    b.id
                ));
            }
            if b.healing.is_some() && capacity == 0 {
                return err(format!(
                    "building {}: healing needs a capacity, the door a unit heals through",
                    b.id
                ));
            }
            if b.chart.is_some() && capacity == 0 {
                return err(format!(
                    "building {}: chart needs a capacity, the door a scholar charts through",
                    b.id
                ));
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
            if let Some(rail) = &b.rail {
                // Faster than walking, and no faster than the cost grid can express.
                if !(rail.speed.is_finite() && rail.speed > 1.0 && rail.speed <= STRAIGHT as f32) {
                    return err(format!(
                        "building {}: rail speed must be above 1.0 and at most {STRAIGHT}",
                        b.id
                    ));
                }
                if !b.passable {
                    return err(format!("building {}: a rail must be passable", b.id));
                }
                if buildings.iter().any(|d| d.rail.is_some()) {
                    return err(format!(
                        "building {}: only one building may be a rail (the world has one \
                         haul-speed rule)",
                        b.id
                    ));
                }
            }
            if let Some(beside) = &b.beside
                && beside.is_empty()
            {
                return err(format!("building {}: beside must name at least one terrain", b.id));
            }
            let orchard = match &b.orchard {
                None => None,
                Some(o) => Some(resolve_orchard(&b.id, o, resources)?),
            };
            let recipe = match &b.recipe {
                None => None,
                Some(r) => Some(resolve_recipe(&b.id, r, items)?),
            };
            let farm = match &b.farm {
                None => None,
                Some(f) => Some(resolve_farm(&b.id, f, items)?),
            };
            let irrigation = match &b.irrigation {
                None => None,
                Some(i) => Some(resolve_irrigation(&b.id, i)?),
            };
            let healing = match &b.healing {
                None => None,
                Some(h) => Some(resolve_healing(&b.id, h, needs)?),
            };
            let power = match &b.power {
                None => None,
                Some(p) => Some(resolve_power(&b.id, p)?),
            };
            let chart = match &b.chart {
                None => None,
                Some(c) => Some(resolve_chart(&b.id, c, resources)?),
            };
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
                work,
                passable: b.passable,
                footprint: (w, h),
                seats: b.seats,
                capacity,
                bed: b.bed.map(|d| BedDef { sleep_bonus: d.sleep_bonus, seek_tiles: d.seek_tiles }),
                heat: b.heat.map(|d| HeatDef {
                    radius: d.radius,
                    sleep_bonus: d.sleep_bonus,
                    seek_tiles: d.seek_tiles,
                }),
                rail: b.rail.map(|d| RailDef { speed: d.speed }),
                orchard,
                recipe,
                farm,
                irrigation,
                healing,
                power,
                chart,
                uses_power: b.uses_power,
                names: b.names,
                unlocked_by: None,
                endeavor: b.endeavor,
                beside: b.beside,
            });
        }
        link_unlocks(&mut buildings, techs)?;
        Ok(buildings)
    }
}

/// Checks a `[building.orchard]` and links its resource. That the resource is renewable is
/// checked once the harvests are known (`Defs::validate`).
fn resolve_orchard(
    building: &str,
    o: &RawOrchard,
    resources: &[ResourceDef],
) -> Result<OrchardDef, DefsError> {
    let err = |msg: &str| Err(DefsError(format!("buildings.toml: building {building}: {msg}")));
    let Some(plants) = resources.iter().position(|r| r.id == o.plants) else {
        return err(&format!("orchard plants unknown resource {:?}", o.plants));
    };
    let seconds_ok = |x: f32| x.is_finite() && x > 0.0;
    if o.radius == 0 || o.max_trees == 0 {
        return err("orchard needs radius > 0 and max_trees > 0");
    }
    if !(seconds_ok(o.plant_seconds) && seconds_ok(o.grow_seconds)) {
        return err("orchard plant_seconds and grow_seconds must be positive numbers");
    }
    if !(o.tend_boost.is_finite() && o.tend_boost >= 1.0) {
        return err("orchard tend_boost must be at least 1.0");
    }
    if o.spacing == 0 || o.spacing > o.radius {
        return err("orchard spacing must be between 1 and its radius");
    }
    Ok(OrchardDef {
        plants: ResourceId(plants as u16),
        radius: o.radius,
        max_trees: o.max_trees,
        plant_seconds: o.plant_seconds,
        grow_seconds: o.grow_seconds,
        tend_boost: o.tend_boost,
        spacing: o.spacing,
        auto_pick: o.auto_pick,
    })
}

/// Checks a `[building.recipe]` and links its items (issue #124).
fn resolve_recipe(
    building: &str,
    r: &RawRecipe,
    items: &[ItemDef],
) -> Result<RecipeDef, DefsError> {
    let err =
        |msg: &str| Err(DefsError(format!("buildings.toml: building {building}: recipe {msg}")));
    // `input = []` is allowed (issue #373): a source recipe, like the Mine Shaft's stone cut
    // from the rock it's built against, with nothing to fetch.
    let mut inputs = Vec::new();
    for i in &r.input {
        let Some(item) = items.iter().position(|d| d.id == i.item) else {
            return err(&format!("unknown input item {:?}", i.item));
        };
        if i.count == 0 {
            return err("input count must be positive");
        }
        if inputs.iter().any(|existing: &CostItem| existing.item.0 as usize == item) {
            return err(&format!("input item {:?} listed twice", i.item));
        }
        inputs.push(CostItem { item: ItemId(item as u16), count: i.count });
    }
    let Some(output) = items.iter().position(|d| d.id == r.output) else {
        return err(&format!("unknown output item {:?}", r.output));
    };
    if r.output_count == 0 {
        return err("output_count must be positive");
    }
    if !(r.work.is_finite() && r.work > 0.0) {
        return err("work must be a positive number");
    }
    if let Some(m) = &r.modifier
        && !Modifiers::NAMES.contains(&m.as_str())
    {
        return err(&format!("unknown modifier {m:?}"));
    }
    Ok(RecipeDef {
        inputs,
        output: CostItem { item: ItemId(output as u16), count: r.output_count },
        work: r.work,
        modifier: r.modifier.clone(),
        stock_limit: r.stock_limit,
    })
}

/// Checks a `[building.farm]` and links its crop item.
fn resolve_farm(building: &str, f: &RawFarm, items: &[ItemDef]) -> Result<FarmDef, DefsError> {
    let err = |msg: &str| Err(DefsError(format!("buildings.toml: building {building}: {msg}")));
    let Some(crop) = items.iter().position(|i| i.id == f.crop) else {
        return err(&format!("farm crop is unknown item {:?}", f.crop));
    };
    if f.harvest_count == 0 {
        return err("farm harvest_count must be at least 1");
    }
    let seconds_ok = |x: f32| x.is_finite() && x > 0.0;
    if !(seconds_ok(f.sow_seconds) && seconds_ok(f.grow_seconds) && seconds_ok(f.reap_seconds)) {
        return err("farm sow_seconds, grow_seconds and reap_seconds must be positive numbers");
    }
    if !(f.crop_growth.is_finite() && f.crop_growth > 0.0) {
        return err("farm crop_growth must be a positive number");
    }
    Ok(FarmDef {
        crop: ItemId(crop as u16),
        harvest_count: f.harvest_count,
        sow_seconds: f.sow_seconds,
        grow_seconds: f.grow_seconds,
        reap_seconds: f.reap_seconds,
        crop_growth: f.crop_growth,
    })
}

/// Checks a `[building.irrigation]`.
fn resolve_irrigation(building: &str, i: &RawIrrigation) -> Result<IrrigationDef, DefsError> {
    let err = |msg: &str| Err(DefsError(format!("buildings.toml: building {building}: {msg}")));
    if i.radius == 0 {
        return err("irrigation needs radius > 0");
    }
    if !(i.growth_boost.is_finite() && i.growth_boost >= 1.0) {
        return err("irrigation growth_boost must be at least 1.0");
    }
    Ok(IrrigationDef { radius: i.radius, growth_boost: i.growth_boost })
}

/// Checks a `[building.power]` (issue #428): `radius` must be at least 1 tile, and `boost` must
/// be above 1.0 (otherwise it wouldn't be a power source at all) and at most
/// `research::MAX_MULTIPLIER`, the same ceiling a tech's own modifier multiplier has.
fn resolve_power(building: &str, p: &RawPower) -> Result<PowerDef, DefsError> {
    let err = |msg: &str| Err(DefsError(format!("buildings.toml: building {building}: {msg}")));
    if p.radius == 0 {
        return err("power needs radius >= 1");
    }
    if !(p.boost.is_finite() && p.boost > 1.0 && p.boost <= MAX_MULTIPLIER) {
        return err(&format!("power boost must be in (1.0, {MAX_MULTIPLIER}]"));
    }
    Ok(PowerDef {
        radius: p.radius,
        boost: p.boost,
        boost_millis: (p.boost * 1000.0).round() as u32,
    })
}

/// Checks a `[building.chart]` (issue #402): every `finds` id must be a resource that is
/// actually `buried` (the same rule `Effect::Reveal` uses), and `minutes` and `range` must be
/// positive.
fn resolve_chart(
    building: &str,
    c: &RawChart,
    resources: &[ResourceDef],
) -> Result<ChartDef, DefsError> {
    let err =
        |msg: &str| Err(DefsError(format!("buildings.toml: building {building}: chart {msg}")));
    if c.finds.is_empty() {
        return err("needs at least one resource to find");
    }
    let mut finds = Vec::with_capacity(c.finds.len());
    for n in &c.finds {
        let Some(i) = resources.iter().position(|r| r.id == *n) else {
            return err(&format!("unknown resource {n:?}"));
        };
        if !resources[i].buried {
            return err(&format!("finds {n:?}, which is never buried"));
        }
        finds.push(ResourceId(i as u16));
    }
    if !(c.minutes.is_finite() && c.minutes > 0.0) {
        return err("minutes must be a positive number");
    }
    if c.range == 0 {
        return err("range must be at least 1");
    }
    Ok(ChartDef { finds, minutes: c.minutes, range: c.range })
}

/// Checks a `[building.healing]` (issue #375): `seek_below` and `stop_at` must sit inside the
/// health meter in that order, `regen_per_minute` must beat the passive rate everyone already
/// gets (`needs.health.regen_per_minute`), and `modifier`, if named, must be a real nation
/// modifier.
fn resolve_healing(
    building: &str,
    h: &RawHealing,
    needs: &NeedsSettings,
) -> Result<HealingDef, DefsError> {
    let err = |msg: &str| Err(DefsError(format!("buildings.toml: building {building}: {msg}")));
    let max = needs.health.max;
    if !(h.seek_below.is_finite() && h.seek_below > 0.0 && h.seek_below <= max) {
        return err(&format!("healing seek_below must be in (0, {max}]"));
    }
    if !(h.stop_at.is_finite() && h.stop_at > h.seek_below && h.stop_at <= max) {
        return err("healing stop_at must be above seek_below and at most health.max");
    }
    if !(h.regen_per_minute.is_finite() && h.regen_per_minute > needs.health.regen_per_minute) {
        return err(&format!(
            "healing regen_per_minute must be above needs.toml's base regen ({})",
            needs.health.regen_per_minute
        ));
    }
    if let Some(m) = &h.modifier
        && !Modifiers::NAMES.contains(&m.as_str())
    {
        return err(&format!("healing has an unknown modifier {m:?}"));
    }
    Ok(HealingDef {
        seek_below: h.seek_below,
        regen_per_minute: h.regen_per_minute,
        stop_at: h.stop_at,
        modifier: h.modifier.clone(),
    })
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
            if b.endeavor {
                return err(format!("{name:?} is the endeavor's; the signs open it, not a tech"));
            }
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
    use crate::defs::{Defs, DefsError, Terrain};
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
        // Capacity defaults to seats (#226): places inside are the hall's seats.
        assert_eq!(def.capacity, def.seats);
        for n in Nation::ALL {
            assert_ne!(def.display_name(n), def.name, "{n:?} should have its own name");
        }
    }

    #[test]
    fn capacity_defaults_to_seats_but_may_be_set_on_its_own() {
        let one = |extra: &str| {
            format!(
                "[[building]]\nid = \"hall\"\nname = \"Hall\"\nverb = \"Build\"\nwork = 1.0\n\
                 passable = false\nseats = 2\n{extra}\n"
            )
        };
        assert_eq!(resolve(&one("")).unwrap()[0].capacity, 2, "defaults to seats");
        assert_eq!(resolve(&one("capacity = 5")).unwrap()[0].capacity, 5, "an explicit value wins");
        assert_eq!(resolve(&one("capacity = 0")).unwrap()[0].capacity, 0);
        // A building with no seats at all has no capacity either, by default.
        let defs = Defs::bundled();
        let bed = defs.building(defs.building_id("bed").unwrap());
        assert_eq!(bed.capacity, 0);
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

    #[test]
    fn footprints_default_to_one_tile_and_are_validated() {
        let defs = Defs::bundled();
        // #214 turns sizes on for these; the rest stay 1x1.
        let big = [
            "study_hall",
            "orchard",
            "windmill",
            "bison_stable",
            "forge",
            "brick_kiln",
            "smelter",
            "steam_engine",
            "lightning_station",
        ];
        for id in big {
            let def = defs.building(defs.building_id(id).unwrap());
            assert_eq!(def.footprint, (2, 2), "{id}");
        }
        for id in ["bed", "campfire", "earth_wall", "rail", "wind_turbine", "igloo"] {
            let def = defs.building(defs.building_id(id).unwrap());
            assert_eq!(def.footprint, (1, 1), "{id}");
        }
        // The endeavor's gate (#236) is the one building already full size.
        let gate = defs.building(defs.building_id("harmony_gate").unwrap());
        assert_eq!(gate.footprint, (4, 4));
        let one = |extra: &str| {
            format!(
                "[[building]]\nid = \"hall\"\nname = \"Hall\"\nverb = \"Build\"\nwork = 1.0\n\
                 passable = false\n{extra}\n"
            )
        };
        assert_eq!(resolve(&one("footprint = [2, 3]")).unwrap()[0].footprint, (2, 3));
        assert_eq!(resolve(&one("footprint = [4, 1]")).unwrap()[0].footprint, (4, 1));
        for bad in ["[0, 1]", "[1, 0]", "[5, 2]", "[2, 5]"] {
            let r = resolve(&one(&format!("footprint = {bad}")));
            assert!(r.unwrap_err().0.contains("footprint sides"), "{bad}");
        }
        let bed = "[building.bed]\nsleep_bonus = 1.2\nseek_tiles = 5";
        resolve(&one(&format!("footprint = [1, 1]\n{bed}"))).unwrap();
        // Bigger than 1x1 needs a capacity (#227): the number of sleeping places inside it.
        let big_bed = resolve(&one(&format!("footprint = [2, 1]\n{bed}")));
        assert!(big_bed.unwrap_err().0.contains("needs a capacity"));
        let big_bed_with_capacity =
            resolve(&one(&format!("footprint = [2, 1]\ncapacity = 2\n{bed}"))).unwrap();
        assert_eq!(big_bed_with_capacity[0].capacity, 2);
        assert_eq!(big_bed_with_capacity[0].footprint, (2, 1));
    }

    #[test]
    fn rail_is_data_and_its_numbers_are_validated() {
        let defs = Defs::bundled();
        let rail = defs.building(defs.building_id("rail").expect("issue #135"));
        let speed = rail.rail.expect("rail is a rail").speed;
        assert!(speed > 1.0 && rail.passable, "a cart must be able to use it");
        assert_eq!(rail.rail.unwrap().step_cost(), 5, "speed 2.0 halves a straight step");
        assert_eq!(rail.unlocked_by, defs.tech_id("earth_rail"));
        // Only the rail is a rail, and every other building leaves rail rules alone.
        assert_eq!(defs.buildings.iter().filter(|b| b.rail.is_some()).count(), 1);
        assert_eq!(defs.cart_rules(crate::MoveRules::default()).rail_step(), 5);

        let one = |extra: &str, passable: bool| {
            format!(
                "[[building]]\nid = \"track\"\nname = \"Track\"\nverb = \"Lay\"\nwork = 1.0\n\
                 passable = {passable}\n{extra}\n"
            )
        };
        resolve(&one("[building.rail]\nspeed = 2.0", true)).unwrap();
        for bad in ["0.5", "1.0", "11.0", "nan"] {
            let r = resolve(&one(&format!("[building.rail]\nspeed = {bad}"), true));
            assert!(r.unwrap_err().0.contains("rail speed"), "{bad}");
        }
        let wall = resolve(&one("[building.rail]\nspeed = 2.0", false));
        assert!(wall.unwrap_err().0.contains("must be passable"));
        let two = format!(
            "{}{}",
            one("[building.rail]\nspeed = 2.0", true),
            one("[building.rail]\nspeed = 3.0", true).replace("track", "track2")
        );
        assert!(resolve(&two).unwrap_err().0.contains("only one building"));
    }

    #[test]
    fn the_orchard_is_data() {
        let defs = Defs::bundled();
        let def = defs.building(defs.building_id("orchard").unwrap());
        let o = def.orchard.expect("an orchard plants");
        assert_eq!(defs.resource(o.plants).id, "fruit_tree");
        assert!(o.max_trees > 0 && o.radius > 0 && o.tend_boost >= 1.0);
        assert!(o.plant_seconds > 0.0 && o.grow_seconds > 0.0);
        assert!(def.passable, "units walk through an orchard");
        // Only the orchard and the Fishing Dock (#374) plant anything.
        assert_eq!(defs.buildings.iter().filter(|b| b.orchard.is_some()).count(), 2);
    }

    /// #374: Water's orchard, on the water. The same mechanism as the Orchard, planting
    /// `fish_spot` instead of `fruit_tree`, and solid (units don't walk through a dock the way
    /// they do an orchard's canopy).
    #[test]
    fn the_fishing_dock_is_data() {
        let defs = Defs::bundled();
        let def = defs.building(defs.building_id("fishing_dock").unwrap());
        let o = def.orchard.expect("a fishing dock plants");
        assert_eq!(defs.resource(o.plants).id, "fish_spot");
        assert_eq!(o.max_trees, 3);
        assert!(o.radius > 0 && o.tend_boost > 1.0);
        assert!(o.plant_seconds > 0.0 && o.grow_seconds > 0.0);
        assert!(!def.passable, "a dock is solid");
        assert_eq!(
            def.beside.as_deref(),
            Some(&[Terrain::Water, Terrain::River, Terrain::Lake][..])
        );
        assert_eq!(def.beside_word(), Some("water"));
    }

    #[test]
    fn orchard_numbers_are_validated() {
        let one = |extra: &str| {
            format!(
                "[[building]]\nid = \"grove\"\nname = \"Grove\"\nverb = \"Plant\"\nwork = 1.0\n\
                 passable = true\n[building.orchard]\nplants = \"fruit_tree\"\nradius = 4\n\
                 max_trees = 6\nplant_seconds = 90.0\ngrow_seconds = 480.0\ntend_boost = 1.5\n\
                 spacing = 2\nauto_pick = true\n{extra}"
            )
        };
        resolve(&one("")).unwrap();
        let bad = |from: &str, to: &str| resolve(&one("").replacen(from, to, 1)).unwrap_err().0;
        assert!(bad("radius = 4", "radius = 0").contains("radius > 0"));
        assert!(bad("max_trees = 6", "max_trees = 0").contains("max_trees > 0"));
        assert!(bad("plant_seconds = 90.0", "plant_seconds = 0.0").contains("positive"));
        assert!(bad("grow_seconds = 480.0", "grow_seconds = -1.0").contains("positive"));
        assert!(bad("tend_boost = 1.5", "tend_boost = 0.5").contains("tend_boost"));
        assert!(bad("spacing = 2", "spacing = 0").contains("spacing"));
        assert!(bad("spacing = 2", "spacing = 5").contains("spacing"));
        assert!(bad("\"fruit_tree\"", "\"nothing\"").contains("unknown resource"));
    }

    #[test]
    fn the_farm_plot_is_data() {
        let defs = Defs::bundled();
        let def = defs.building(defs.building_id("farm_plot").unwrap());
        let f = def.farm.expect("a farm plot grows a crop");
        assert_eq!(defs.item(f.crop).id, "cabbage");
        assert!(f.harvest_count > 0);
        assert!(f.sow_seconds > 0.0 && f.grow_seconds > 0.0 && f.reap_seconds > 0.0);
        assert_eq!(f.crop_growth, 1.0);
        assert!(def.passable, "units walk through a farm plot");
        // The farm plot and the Herb Garden (issue #390) grow a crop; nothing else does.
        assert_eq!(defs.buildings.iter().filter(|b| b.farm.is_some()).count(), 2);
    }

    #[test]
    fn the_herb_garden_is_data() {
        let defs = Defs::bundled();
        let def = defs.building(defs.building_id("herb_garden").unwrap());
        let f = def.farm.expect("the Herb Garden grows herbs");
        assert_eq!(defs.item(f.crop).id, "herbs");
        assert!(f.harvest_count > 0);
        assert!(f.sow_seconds > 0.0 && f.grow_seconds > 0.0 && f.reap_seconds > 0.0);
        assert!(def.passable, "units walk through an herb garden, like a farm plot");
    }

    #[test]
    fn farm_numbers_are_validated() {
        let one = |extra: &str| {
            format!(
                "[[building]]\nid = \"plot\"\nname = \"Plot\"\nverb = \"Till\"\nwork = 1.0\n\
                 passable = true\n[building.farm]\ncrop = \"cabbage\"\nharvest_count = 3\n\
                 sow_seconds = 6.0\ngrow_seconds = 300.0\nreap_seconds = 5.0\n\
                 crop_growth = 1.0\n{extra}"
            )
        };
        resolve(&one("")).unwrap();
        let bad = |from: &str, to: &str| resolve(&one("").replacen(from, to, 1)).unwrap_err().0;
        assert!(bad("harvest_count = 3", "harvest_count = 0").contains("harvest_count"));
        assert!(bad("sow_seconds = 6.0", "sow_seconds = 0.0").contains("positive"));
        assert!(bad("grow_seconds = 300.0", "grow_seconds = -1.0").contains("positive"));
        assert!(bad("reap_seconds = 5.0", "reap_seconds = 0.0").contains("positive"));
        assert!(bad("crop_growth = 1.0", "crop_growth = 0.0").contains("crop_growth"));
        assert!(bad("crop_growth = 1.0", "crop_growth = -1.0").contains("crop_growth"));
        assert!(bad("\"cabbage\"", "\"nothing\"").contains("unknown item"));
    }

    #[test]
    fn the_irrigation_channel_is_data() {
        let defs = Defs::bundled();
        let def = defs.building(defs.building_id("irrigation_channel").unwrap());
        let i = def.irrigation.expect("the channel speeds up farm plots");
        assert!(i.radius > 0);
        assert!(i.growth_boost >= 1.0);
        assert!(def.passable, "units walk through a channel");
        // Only the channel speeds up plots.
        assert_eq!(defs.buildings.iter().filter(|b| b.irrigation.is_some()).count(), 1);
    }

    #[test]
    fn irrigation_numbers_are_validated() {
        let one = |extra: &str| {
            format!(
                "[[building]]\nid = \"channel\"\nname = \"Channel\"\nverb = \"Dig\"\nwork = 1.0\n\
                 passable = true\n[building.irrigation]\nradius = 3\ngrowth_boost = 1.5\n{extra}"
            )
        };
        resolve(&one("")).unwrap();
        let bad = |from: &str, to: &str| resolve(&one("").replacen(from, to, 1)).unwrap_err().0;
        assert!(bad("radius = 3", "radius = 0").contains("radius > 0"));
        assert!(bad("growth_boost = 1.5", "growth_boost = 0.5").contains("growth_boost"));
    }

    #[test]
    fn uses_power_is_set_on_every_consumer_and_fire_has_its_two_sources() {
        let defs = Defs::bundled();
        for id in
            ["workshop", "forge", "brick_kiln", "smelter", "mine_shaft", "deep_mine", "study_hall"]
        {
            let def = defs.building(defs.building_id(id).unwrap());
            assert!(def.uses_power, "{id} should run faster when powered");
        }
        // Fire's two sources (#255, docs/design/late-tree.md, "Power"); Water's Tide Mill (#200)
        // and Air's Wind Turbine (#204) are checked below.
        let steam_engine = defs.building(defs.building_id("steam_engine").unwrap());
        let power = steam_engine.power.expect("the Steam Engine is a power source");
        assert_eq!((power.radius, power.boost_millis), (3, 1500));

        let lightning_station = defs.building(defs.building_id("lightning_station").unwrap());
        let power = lightning_station.power.expect("the Lightning Station is a power source");
        assert_eq!((power.radius, power.boost_millis), (4, 2000));

        assert_eq!(defs.buildings.iter().filter(|b| b.power.is_some()).count(), 4);
    }

    #[test]
    fn the_tide_mill_is_waters_power_source_and_must_touch_water() {
        let defs = Defs::bundled();
        let tide_mill = defs.building(defs.building_id("tide_mill").unwrap());
        let power = tide_mill.power.expect("the Tide Mill is a power source");
        assert_eq!((power.radius, power.boost_millis), (4, 1500));
        assert_eq!(tide_mill.beside_word(), Some("water"));
        let tide_mills = defs.tech_id("tide_mills").unwrap();
        assert_eq!(tide_mill.unlocked_by, Some(tide_mills));
    }

    /// The Wind Turbine (issue #204, docs/design/late-tree.md, "The sources"): Air's second
    /// power source, builds anywhere (no `beside` rule, unlike the Tide Mill).
    #[test]
    fn the_wind_turbine_is_airs_power_source_and_builds_anywhere() {
        let defs = Defs::bundled();
        let wind_turbine = defs.building(defs.building_id("wind_turbine").unwrap());
        let power = wind_turbine.power.expect("the Wind Turbine is a power source");
        assert_eq!((power.radius, power.boost_millis), (3, 1500));
        assert_eq!(wind_turbine.beside_word(), None);
        let wind_turbines = defs.tech_id("wind_turbines").unwrap();
        assert_eq!(wind_turbine.unlocked_by, Some(wind_turbines));
    }

    #[test]
    fn power_numbers_are_validated() {
        let one = |extra: &str| {
            format!(
                "[[building]]\nid = \"mill\"\nname = \"Mill\"\nverb = \"Build\"\nwork = 1.0\n\
                 passable = false\n[building.power]\nradius = 3\nboost = 1.5\n{extra}"
            )
        };
        let def = &resolve(&one("")).unwrap()[0];
        let power = def.power.expect("a mill is a power source");
        assert_eq!(power.boost_millis, 1500);
        let bad = |from: &str, to: &str| resolve(&one("").replacen(from, to, 1)).unwrap_err().0;
        assert!(bad("radius = 3", "radius = 0").contains("radius >= 1"));
        assert!(
            bad("boost = 1.5", "boost = 1.0").contains("boost must be"),
            "boost must be above 1.0"
        );
        assert!(bad("boost = 1.5", "boost = 0.5").contains("boost must be"));
        assert!(bad("boost = 1.5", "boost = 5.0").contains("boost must be"));
        resolve(&one("").replacen("boost = 1.5", "boost = 4.0", 1)).unwrap();
    }

    #[test]
    fn the_healing_hut_is_data() {
        let defs = Defs::bundled();
        let def = defs.building(defs.building_id("healing_hut").unwrap());
        let h = def.healing.as_ref().expect("the hut heals hurt settlers");
        assert!(h.seek_below > 0.0 && h.seek_below <= defs.needs.health.max);
        assert!(h.stop_at > h.seek_below && h.stop_at <= defs.needs.health.max);
        assert!(h.regen_per_minute > defs.needs.health.regen_per_minute);
        assert_eq!(h.modifier.as_deref(), Some("water_work"));
        assert!(def.capacity > 0, "healing needs a door, like a study seat");
        // Only the hut heals.
        assert_eq!(defs.buildings.iter().filter(|b| b.healing.is_some()).count(), 1);
    }

    #[test]
    fn healing_numbers_are_validated() {
        let one = |extra: &str| {
            format!(
                "[[building]]\nid = \"hut\"\nname = \"Hut\"\nverb = \"Build\"\nwork = 1.0\n\
                 passable = false\ncapacity = 2\n[building.healing]\nseek_below = 60.0\n\
                 regen_per_minute = 8.0\nstop_at = 90.0\nmodifier = \"water_work\"\n{extra}"
            )
        };
        resolve(&one("")).unwrap();
        let bad = |from: &str, to: &str| resolve(&one("").replacen(from, to, 1)).unwrap_err().0;
        assert!(bad("seek_below = 60.0", "seek_below = 0.0").contains("seek_below"));
        assert!(bad("seek_below = 60.0", "seek_below = 150.0").contains("seek_below"));
        assert!(bad("stop_at = 90.0", "stop_at = 40.0").contains("stop_at"));
        assert!(bad("stop_at = 90.0", "stop_at = 150.0").contains("stop_at"));
        assert!(
            bad("regen_per_minute = 8.0", "regen_per_minute = 1.0").contains("regen_per_minute")
        );
        assert!(bad("modifier = \"water_work\"", "modifier = \"nope\"").contains("modifier"));
        let no_capacity = one("").replacen("capacity = 2\n", "", 1);
        assert!(resolve(&no_capacity).unwrap_err().0.contains("healing needs a capacity"));
    }

    fn resolve(toml: &str) -> Result<Vec<BuildingDef>, DefsError> {
        let defs = Defs::bundled();
        // The bundled techs unlock the bundled buildings, which these small fixtures don't
        // define, so resolve them against a tree that unlocks no buildings.
        let mut techs = defs.techs.clone();
        for t in &mut techs {
            t.unlocks.buildings.clear();
        }
        toml::from_str::<BuildingsFile>(toml).unwrap().resolve(
            &defs.items,
            &techs,
            &defs.resources,
            &defs.needs,
        )
    }

    #[test]
    fn an_endeavor_building_has_no_cost_or_work_of_its_own() {
        let gate = |extra: &str| {
            format!(
                "[[building]]\nid = \"gate\"\nname = \"Gate\"\nverb = \"Raise\"\n\
                 passable = true\nendeavor = true\n{extra}\n"
            )
        };
        let def = &resolve(&gate("")).unwrap()[0];
        assert!(def.endeavor);
        assert_eq!(def.work, 0.0);
        assert!(resolve(&gate("work = 5.0")).unwrap_err().0.contains("stages"));
        let costed = gate("[[building.cost]]\nitem = \"stone\"\ncount = 1");
        assert!(resolve(&costed).unwrap_err().0.contains("stages"));
        // An ordinary building still needs its work.
        let plain = "[[building]]\nid = \"hut\"\nname = \"Hut\"\nverb = \"Build\"\n\
                     passable = false\n";
        assert!(resolve(plain).unwrap_err().0.contains("work must be a positive number"));
        let bundled = Defs::bundled();
        let gate = bundled.building(bundled.building_id("harmony_gate").unwrap());
        assert!(gate.endeavor && gate.passable && gate.cost.is_empty());
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
