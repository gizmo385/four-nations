//! The tech tree: definitions from `data/research.toml`, resolved and validated at load.
//!
//! See `docs/design/research.md`. Techs are referenced by `TechId`, their index in the file.
//! The file must list prereqs before the techs that need them, so `TechId` order is a valid
//! topological order and cycles are impossible by construction. Later code relies on that: it can
//! walk techs in id order and know every prereq has already been visited.
//!
//! Static definitions come first. `Research` at the bottom is the live state that `World` holds:
//! known techs, the knowledge pool and the queue. `effective_modifiers` and
//! `modifier_breakdown` apply completed techs' multipliers to a nation's modifiers.

use serde::Deserialize;

use crate::defs::{Defs, DefsError};
use crate::jobs::{ItemId, Stack};
use crate::nation::{ModifierMultipliers, Modifiers, Nation};
use crate::world::TICKS_PER_SECOND;

// `Deserialize` is `save::ids`' remapping impl (#602).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, serde::Serialize)]
pub struct TechId(pub u16);

/// Largest multiplier a single tech may apply. Keeps a typo like `12` instead of `1.2` from
/// loading.
pub const MAX_MULTIPLIER: f32 = 4.0;

/// The nation's base modifiers times every known tech's multipliers, applied in ascending
/// `TechId` order. `f32` multiplication isn't associative, so the order is fixed here rather than
/// left to completion order: any research order that reaches the same known set gives
/// bitwise-identical results.
pub fn effective_modifiers(
    nation: Nation,
    known: impl IntoIterator<Item = TechId>,
    defs: &Defs,
) -> Modifiers {
    let mut mods = defs.nation(nation).modifiers;
    for t in sorted(known) {
        mods.apply(&defs.tech(t).unlocks.modifiers);
    }
    mods
}

/// How `effective_modifiers` arrives at the modifier called `name`: the nation's base value, then
/// each known tech that multiplies it, in the same order as `effective_modifiers`. Each entry is
/// `(source name, factor)`; the product of the factors (left to right) is the effective value.
/// Empty if `name` isn't a modifier.
pub fn modifier_breakdown<'d>(
    name: &str,
    nation: Nation,
    known: impl IntoIterator<Item = TechId>,
    defs: &'d Defs,
) -> Vec<(&'d str, f32)> {
    let nation = defs.nation(nation);
    let Some(base) = nation.modifiers.get(name) else { return Vec::new() };
    let mut out = vec![(nation.name.as_str(), base)];
    for t in sorted(known) {
        let tech = defs.tech(t);
        if let Some((_, f)) = tech.unlocks.modifiers.iter().find(|(n, _)| *n == name) {
            out.push((tech.name.as_str(), f));
        }
    }
    out
}

/// `ids` in ascending order, without duplicates.
fn sorted(ids: impl IntoIterator<Item = TechId>) -> Vec<TechId> {
    let mut v: Vec<TechId> = ids.into_iter().collect();
    v.sort_unstable();
    v.dedup();
    v
}

/// Knowledge production rates. Converted to integer millis per tick by the research system.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ResearchSettings {
    /// Settlement-wide trickle that bootstraps the first techs.
    pub passive_knowledge_per_minute: f64,
    /// Per scholar at a study hall, before the `study` modifier.
    pub study_knowledge_per_minute: f64,
    /// Knowledge points the pool may hold with no research target queued: just enough to
    /// bootstrap a first pick without letting the passive trickle (or a seat left idle, #284)
    /// quietly bank toward several techs at once. Below the cheapest tech's cost.
    pub idle_knowledge_cap: f64,
    /// Only a tech costing at least this many points may have `cost_items` (docs/design/
    /// horizons-phase-4.md, "Item bills"), so the first tiers of the tree stay labour-only.
    #[serde(default = "default_cost_items_from")]
    pub cost_items_from: u32,
    /// The id of the building that takes a research bill's deliveries (`study_hall`). Required
    /// once any tech has `cost_items`; it must have seats. Resolved to `bill_hall_id` at load.
    #[serde(default)]
    pub bill_hall: Option<String>,
    /// `bill_hall` as a building id, filled in once the buildings load.
    #[serde(skip)]
    pub bill_hall_id: Option<crate::building::BuildingId>,
    /// `passive_knowledge_per_minute` as integer millis per tick, filled in at load.
    #[serde(skip)]
    pub passive_millis_per_tick: u64,
    /// `study_knowledge_per_minute` as integer millis per tick, filled in at load.
    #[serde(skip)]
    pub study_millis_per_tick: u64,
    /// `idle_knowledge_cap` in milli-points, filled in at load.
    #[serde(skip)]
    pub idle_knowledge_cap_millis: u64,
}

fn default_cost_items_from() -> u32 {
    1000
}

/// Largest count one `[[tech.cost_items]]` line may ask for.
pub const MAX_COST_ITEM_COUNT: u32 = 99;

/// One `[[tech.cost_items]]` line as written: an item id and a count.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct RawCostItem {
    item: String,
    count: u32,
}

/// Knowledge is stored in milli-points so rates stay integers.
pub const MILLIS_PER_POINT: u64 = 1000;

/// Converts a points-per-minute rate to whole millis per tick (rounded to nearest).
fn millis_per_tick(per_minute: f64) -> u64 {
    (per_minute * MILLIS_PER_POINT as f64 / (60.0 * f64::from(TICKS_PER_SECOND))).round() as u64
}

/// What learning a tech makes available: modifier multipliers, plus ids checked against the def
/// kind they name once it loads (`building::BuildingsFile::resolve`, `jobs::JobsFile::resolve`,
/// `bending::BendingFile::resolve`), which also records the unlocking tech on that def's
/// `unlocked_by` field. An item no tech names this way is available from the start (issue #22).
#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Unlocks {
    #[serde(default)]
    pub modifiers: ModifierMultipliers,
    /// Ids of buildings in `data/buildings.toml` this tech unlocks.
    #[serde(default)]
    pub buildings: Vec<String>,
    /// Ids of resources in `data/resources.toml` whose harvest designation (`data/jobs.toml`)
    /// this tech unlocks.
    #[serde(default)]
    pub jobs: Vec<String>,
    /// Ids of abilities in `data/bending.toml` this tech unlocks.
    #[serde(default)]
    pub abilities: Vec<String>,
    /// Properties of existing buildings this tech raises (`[[tech.unlocks.upgrades]]`,
    /// docs/design/horizons-phase-1.md): checked and recorded on `BuildingDef::seat_upgrades`
    /// once the buildings load.
    #[serde(default)]
    pub upgrades: Vec<Upgrade>,
    /// Named features the sim knows (`Feature`), docs/design/horizons-phase-1.md: each is
    /// unlocked by at most one tech, and systems ask `World::has_feature`, never a tech id.
    #[serde(default)]
    pub features: Vec<Feature>,
}

/// Largest `requires.scrolls_read` a tech may ask for (the smallest is 1).
pub const MAX_SCROLLS_REQUIRED: u32 = 20;

/// What a tech needs beyond its prereqs before it can be queued (`requires = { ... }` on a
/// `[[tech]]`, docs/design/horizons-phase-3.md, "Research conditions"). Every key is optional and
/// all that are set must hold. The table is `deny_unknown_fields`, so a key that doesn't exist
/// yet (phase 6 adds population, masters, residents) fails to load instead of being ignored.
///
/// Each condition is checked against [`Facts`] when a target is set (`Research::set_target`), not
/// again as research goes on, so a condition has to be one that only ever becomes true: a tally
/// never falls and a chapter stays met. A condition that can fall again needs a recheck on every
/// completion first.
///
/// To add a key: a field here, a check in `ResearchFile::resolve` (and in `check_chapters` if it
/// names other data), a field on `Facts` that `World::facts` fills, an arm in `Requires::unmet`
/// and a `Condition` variant with its `Display`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Requires {
    /// Scrolls read at a study hall so far (`Tallies::scrolls_read`), 1 to
    /// `MAX_SCROLLS_REQUIRED`.
    #[serde(default)]
    pub scrolls_read: Option<u32>,
    /// The id of a `[[chapter]]` in `data/endeavor.toml` that must be met (`World::chapter_met`).
    /// Checked against the endeavor once it loads (`Defs::with_endeavor`).
    #[serde(default)]
    pub chapter: Option<String>,
}

/// What the world knows that `Requires` asks about: filled by `World::facts`, so `research`
/// never reaches into the world. `Facts::default()` is a settlement that has done nothing yet.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Facts<'a> {
    /// Scrolls read so far, a count that never falls.
    pub scrolls_read: u32,
    /// Ids of the chapters met so far; a chapter stays met.
    pub chapters_met: Vec<&'a str>,
}

/// One unmet `Requires` key, as the player reads it: `Display` is `3 scrolls read` or the
/// chapter's name (`Known on the road`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Condition {
    /// At least this many scrolls read.
    ScrollsRead(u32),
    /// A chapter's display name.
    Chapter(String),
}

impl std::fmt::Display for Condition {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ScrollsRead(1) => f.write_str("1 scroll read"),
            Self::ScrollsRead(n) => write!(f, "{n} scrolls read"),
            Self::Chapter(name) => f.write_str(name),
        }
    }
}

impl Requires {
    /// Whether no key is set.
    pub fn is_empty(&self) -> bool {
        self.scrolls_read.is_none() && self.chapter.is_none()
    }

    /// The keys `facts` doesn't meet yet, in a fixed order (scrolls, then chapter). Empty once
    /// they are all met, and always for a tech with no `requires`. A chapter reads as its name
    /// from `defs.endeavor`, or its id if the endeavor isn't loaded.
    pub fn unmet(&self, facts: &Facts<'_>, defs: &Defs) -> Vec<Condition> {
        let mut out = Vec::new();
        if let Some(n) = self.scrolls_read.filter(|&n| facts.scrolls_read < n) {
            out.push(Condition::ScrollsRead(n));
        }
        if let Some(id) = self.chapter.as_deref().filter(|id| !facts.chapters_met.contains(id)) {
            let name = defs.endeavor.chapters.iter().find(|c| c.id == id).map_or(id, |c| &c.name);
            out.push(Condition::Chapter(name.to_owned()));
        }
        out
    }
}

/// A closed list of things a tech can switch on that are neither a building, a job, an ability
/// nor a modifier (`unlocks.features`). An unknown name fails to load. A feature no tech names
/// is available from the start, like every other unlock kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Feature {
    /// `World::almanac`: when the next frost (and later caravan) comes.
    Almanac,
    /// `Command::StandingSow`: a farm plot that re-sows itself whenever it's empty.
    StandingSow,
}

/// One `[[tech.unlocks.upgrades]]` entry: a building and exactly one property to raise: `seats`
/// (more study seats) or `recipe` (unlocks one of its `[[building.alt_recipe]]`, docs/design/
/// horizons-phase-3.md). The loader checks that exactly one is set. Any other key fails to load.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Upgrade {
    /// An id from `data/buildings.toml`.
    pub building: String,
    /// Added to the building's own `seats`: 1 to 8.
    #[serde(default)]
    pub seats: Option<u32>,
    /// The id of an alt recipe on the building that this tech unlocks.
    #[serde(default)]
    pub recipe: Option<String>,
}

/// A school: one permanent fork in a nation's tree (`[[school]]` in `data/research.toml`,
/// docs/design/horizons-phase-4.md, "Shared sim work, 1. Schools"). Indexed by `SchoolId`.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SchoolDef {
    pub id: String,
    pub nation: Nation,
    /// The fork's heading on the Research screen and in the confirm: `Which city?`.
    pub question: String,
}

/// A school's position in `Defs::schools`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct SchoolId(pub u16);

#[derive(Debug, Clone, PartialEq)]
pub struct TechDef {
    pub id: String,
    pub name: String,
    pub description: String,
    /// `None` for the shared civilization tier and for a `nations` tech; otherwise only this
    /// nation sees the tech.
    pub nation: Option<Nation>,
    /// Empty unless the data says `nations = [...]`: a tech for those nations only (at least
    /// two, never alongside `nation`), such as `herding`. Its prereqs are shared techs. Ask
    /// `open_to` and `is_shared` rather than reading this and `nation`.
    pub nations: Vec<Nation>,
    /// Knowledge points, always > 0.
    pub cost: u32,
    /// Every prereq is smaller than this tech's own id (file order is topological).
    pub prereqs: Vec<TechId>,
    pub unlocks: Unlocks,
    /// Counted by the Harmony Gate's signs (`core = true` in the data): `BranchComplete` counts
    /// only a nation's core techs, and `EveryBuilding` only buildings no tech or a core tech
    /// unlocks. Defaults to false, so a later tech can't lengthen the gate by forgetting a line
    /// (docs/design/horizons-phase-1.md, "The rule every phase-1 tech follows").
    pub core: bool,
    /// Conditions beyond `prereqs` (`Requires`); none for most techs.
    pub requires: Requires,
    /// The school this tech is one option of (`school = "..."`). Learning any other tech of the
    /// same school closes this one for the rest of the game (`Research::closed_by`).
    pub school: Option<SchoolId>,
    /// Items haulers must bring to a study hall before the tech completes
    /// (`[[tech.cost_items]]`, docs/design/horizons-phase-4.md, "Item bills"). Empty for most
    /// techs. Never scaled by the difficulty preset's `research_cost`.
    pub cost_items: Vec<Stack>,
}

impl TechDef {
    /// Whether `nation` sees this tech: it is shared, or it is the tech's own nation, or one of
    /// its `nations`.
    pub fn open_to(&self, nation: Nation) -> bool {
        match self.nation {
            Some(n) => n == nation,
            None => self.nations.is_empty() || self.nations.contains(&nation),
        }
    }

    /// Whether every nation sees this tech: the shared civilization tier.
    pub fn is_shared(&self) -> bool {
        self.nation.is_none() && self.nations.is_empty()
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ResearchFile {
    settings: ResearchSettings,
    #[serde(default)]
    school: Vec<SchoolDef>,
    #[serde(default)]
    tech: Vec<RawTech>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawTech {
    id: String,
    name: String,
    description: String,
    #[serde(default)]
    nation: Option<Nation>,
    #[serde(default)]
    nations: Vec<Nation>,
    cost: u32,
    #[serde(default)]
    prereqs: Vec<String>,
    #[serde(default)]
    unlocks: Unlocks,
    #[serde(default)]
    core: bool,
    #[serde(default)]
    requires: Requires,
    #[serde(default)]
    school: Option<String>,
    #[serde(default)]
    cost_items: Vec<RawCostItem>,
}

/// Each tech's `cost_items` as written, by `TechId`, for `resolve_cost_items` once the items
/// are known.
pub(crate) type RawCostItems = Vec<(TechId, Vec<RawCostItem>)>;

fn nation_label(n: Option<Nation>) -> &'static str {
    n.map_or("shared", Nation::as_str)
}

impl ResearchFile {
    /// Validates the file and resolves string prereqs to `TechId`s and school names to
    /// `SchoolId`s. A school's rules that need the nations' starting techs are checked later, by
    /// `check_schools`.
    pub(crate) fn resolve(
        self,
    ) -> Result<(ResearchSettings, Vec<SchoolDef>, Vec<TechDef>, RawCostItems), DefsError> {
        let mut s = self.settings;
        for (what, v) in [
            ("passive_knowledge_per_minute", s.passive_knowledge_per_minute),
            ("study_knowledge_per_minute", s.study_knowledge_per_minute),
            ("idle_knowledge_cap", s.idle_knowledge_cap),
        ] {
            if !(v.is_finite() && v >= 0.0) {
                return Err(DefsError(format!(
                    "research settings: {what} must be finite and >= 0"
                )));
            }
        }
        s.passive_millis_per_tick = millis_per_tick(s.passive_knowledge_per_minute);
        s.study_millis_per_tick = millis_per_tick(s.study_knowledge_per_minute);
        s.idle_knowledge_cap_millis =
            (s.idle_knowledge_cap * MILLIS_PER_POINT as f64).round() as u64;
        if self.tech.len() > usize::from(u16::MAX) {
            return Err(DefsError(format!("too many techs: {}", self.tech.len())));
        }

        let schools = self.school;
        for (k, sc) in schools.iter().enumerate() {
            if schools[..k].iter().any(|o| o.id == sc.id) {
                return Err(DefsError(format!("school {}: duplicate id", sc.id)));
            }
            if let Some(o) = schools[..k].iter().find(|o| o.nation == sc.nation) {
                return Err(DefsError(format!(
                    "school {}: {} already has a school ({}); a nation has at most one",
                    sc.id,
                    sc.nation.as_str(),
                    o.id
                )));
            }
        }

        let mut raw_costs: RawCostItems = Vec::new();
        let mut techs: Vec<TechDef> = Vec::with_capacity(self.tech.len());
        let find = |techs: &[TechDef], id: &str| techs.iter().position(|t| t.id == id);
        for raw in self.tech {
            let err = |msg: String| DefsError(format!("tech {}: {msg}", raw.id));
            if find(&techs, &raw.id).is_some() {
                return Err(err("duplicate id".into()));
            }
            if raw.cost == 0 {
                return Err(err("cost must be > 0".into()));
            }
            if !raw.nations.is_empty() {
                if raw.nation.is_some() {
                    return Err(err("`nations` can't be combined with `nation`".into()));
                }
                if raw.nations.len() < 2 {
                    return Err(err(
                        "`nations` must list at least two nations (use `nation` for one)".into(),
                    ));
                }
                for (k, n) in raw.nations.iter().enumerate() {
                    if raw.nations[..k].contains(n) {
                        return Err(err(format!("nation {} listed twice", n.as_str())));
                    }
                }
            }
            if let Some(n) = raw.requires.scrolls_read
                && !(1..=MAX_SCROLLS_REQUIRED).contains(&n)
            {
                return Err(err(format!(
                    "requires.scrolls_read = {n} must be 1 to {MAX_SCROLLS_REQUIRED}"
                )));
            }
            for (field, m) in raw.unlocks.modifiers.iter() {
                if !(m.is_finite() && m > 0.0 && m <= MAX_MULTIPLIER) {
                    return Err(err(format!(
                        "multiplier {field} = {m} must be finite and in (0, {MAX_MULTIPLIER}]"
                    )));
                }
            }
            for (k, f) in raw.unlocks.features.iter().enumerate() {
                if raw.unlocks.features[..k].contains(f) {
                    return Err(err(format!("feature {f:?} listed twice")));
                }
                if let Some(other) = techs.iter().find(|t| t.unlocks.features.contains(f)) {
                    return Err(err(format!(
                        "feature {f:?} is already unlocked by {:?}; a feature has one tech",
                        other.id
                    )));
                }
            }
            let mut prereqs = Vec::with_capacity(raw.prereqs.len());
            for p in &raw.prereqs {
                // Only earlier techs are in `techs`, so a forward reference lands here too.
                let Some(i) = find(&techs, p) else {
                    return Err(err(format!(
                        "unknown prereq {p:?} (prereqs must be defined earlier in the file)"
                    )));
                };
                let id = TechId(i as u16);
                if prereqs.contains(&id) {
                    return Err(err(format!("prereq {p:?} listed twice")));
                }
                if !raw.nations.is_empty() && !techs[i].is_shared() {
                    return Err(err(format!(
                        "a `nations` tech can only require shared techs, and {p:?} isn't one"
                    )));
                }
                if !techs[i].nations.is_empty() {
                    return Err(err(format!(
                        "{p:?} is a `nations` tech, which no other tech can require yet"
                    )));
                }
                let pn = techs[i].nation;
                if pn.is_some() && pn != raw.nation {
                    return Err(err(format!(
                        "a {} tech can't require {p:?}, a {} tech",
                        nation_label(raw.nation),
                        nation_label(pn)
                    )));
                }
                if techs[i].school.is_some() {
                    return Err(err(format!(
                        "{p:?} is a school tech, which no other tech can require"
                    )));
                }
                prereqs.push(id);
            }
            let school = match &raw.school {
                None => None,
                Some(name) => {
                    let Some(k) = schools.iter().position(|sc| sc.id == *name) else {
                        return Err(err(format!("unknown school {name:?}")));
                    };
                    if raw.nation != Some(schools[k].nation) {
                        return Err(err(format!(
                            "school {name:?} is {}'s, and this is a {} tech",
                            schools[k].nation.as_str(),
                            nation_label(raw.nation)
                        )));
                    }
                    if raw.core {
                        return Err(err("a school tech can't be `core`".into()));
                    }
                    Some(SchoolId(k as u16))
                }
            };
            if !raw.cost_items.is_empty() {
                if raw.core {
                    return Err(err("a core tech can't have `cost_items`".into()));
                }
                if raw.cost < s.cost_items_from {
                    return Err(err(format!(
                        "`cost_items` needs a cost of at least {} (cost_items_from), not {}",
                        s.cost_items_from, raw.cost
                    )));
                }
                for (k, c) in raw.cost_items.iter().enumerate() {
                    if raw.cost_items[..k].iter().any(|o| o.item == c.item) {
                        return Err(err(format!("cost_items: {:?} listed twice", c.item)));
                    }
                    if !(1..=MAX_COST_ITEM_COUNT).contains(&c.count) {
                        return Err(err(format!(
                            "cost_items: {} x{} must be 1 to {MAX_COST_ITEM_COUNT}",
                            c.item, c.count
                        )));
                    }
                }
                if s.bill_hall.is_none() {
                    return Err(err("`cost_items` needs research settings `bill_hall`".into()));
                }
                raw_costs.push((TechId(techs.len() as u16), raw.cost_items));
            }
            techs.push(TechDef {
                id: raw.id,
                name: raw.name,
                description: raw.description,
                nation: raw.nation,
                nations: raw.nations,
                cost: raw.cost,
                prereqs,
                unlocks: raw.unlocks,
                core: raw.core,
                requires: raw.requires,
                school,
                cost_items: Vec::new(),
            });
        }
        for (k, sc) in schools.iter().enumerate() {
            let n = techs.iter().filter(|t| t.school == Some(SchoolId(k as u16))).count();
            if n == 0 {
                return Err(DefsError(format!("school {}: no tech names it", sc.id)));
            }
            if n != 2 {
                return Err(DefsError(format!(
                    "school {}: has {n} techs, and a school has exactly 2",
                    sc.id
                )));
            }
        }
        Ok((s, schools, techs, raw_costs))
    }
}

/// Resolves each tech's `cost_items` against the items, and `bill_hall` against the buildings
/// (it must have seats). Runs once both are loaded.
pub(crate) fn resolve_cost_items(
    settings: &mut ResearchSettings,
    techs: &mut [TechDef],
    raw: RawCostItems,
    items: &[crate::jobs::ItemDef],
    buildings: &[crate::building::BuildingDef],
) -> Result<(), DefsError> {
    if let Some(hall) = &settings.bill_hall {
        let Some(i) = buildings.iter().position(|b| b.id == *hall) else {
            return Err(DefsError(format!(
                "research settings: bill_hall: unknown building {hall:?}"
            )));
        };
        if buildings[i].seats == 0 {
            return Err(DefsError(format!(
                "research settings: bill_hall: {hall:?} has no seats to take deliveries at"
            )));
        }
        settings.bill_hall_id = Some(crate::building::BuildingId(i as u16));
    }
    for (tech, lines) in raw {
        let def = &mut techs[tech.0 as usize];
        for line in lines {
            let Some(i) = items.iter().position(|it| it.id == line.item) else {
                return Err(DefsError(format!(
                    "tech {}: cost_items: unknown item {:?}",
                    def.id, line.item
                )));
            };
            def.cost_items.push(Stack { item: ItemId(i as u16), count: line.count });
        }
    }
    Ok(())
}

/// Checks that no nation starts with a school tech: a fork is a choice made in play. Runs once
/// `starting_techs` are resolved.
pub(crate) fn check_schools(
    techs: &[TechDef],
    nations: &[crate::nation::NationDef],
) -> Result<(), DefsError> {
    for n in nations {
        if let Some(t) = n.starting_techs.iter().find(|t| techs[t.0 as usize].school.is_some()) {
            return Err(DefsError(format!(
                "nation {}: starting_techs: {:?} is a school tech, which no nation starts with",
                n.id.as_str(),
                techs[t.0 as usize].id
            )));
        }
    }
    Ok(())
}

/// Checks that every `requires.chapter` names a `[[chapter]]` in `chapters`. Chapters live in
/// `data/endeavor.toml`, which loads after the research file, so `Defs::with_endeavor` runs this
/// once it has them.
pub(crate) fn check_chapters(
    techs: &[TechDef],
    chapters: &[crate::endeavor::ChapterDef],
) -> Result<(), DefsError> {
    for t in techs {
        if let Some(c) = &t.requires.chapter
            && !chapters.iter().any(|ch| ch.id == *c)
        {
            return Err(DefsError(format!(
                "tech {}: requires.chapter: unknown chapter {c:?} (not a [[chapter]] in endeavor.toml)",
                t.id
            )));
        }
    }
    Ok(())
}

/// Resolves a nation's `starting_techs`: each must exist and be shared or the nation's own, and
/// every prereq of a starting tech must also be a starting tech.
pub(crate) fn resolve_starting_techs(
    nation: Nation,
    ids: &[String],
    techs: &[TechDef],
) -> Result<Vec<TechId>, DefsError> {
    let err = |msg: String| DefsError(format!("nation {}: starting_techs: {msg}", nation.as_str()));
    let mut out = Vec::with_capacity(ids.len());
    for id in ids {
        let Some(i) = techs.iter().position(|t| t.id == *id) else {
            return Err(err(format!("unknown tech {id:?}")));
        };
        let tech = TechId(i as u16);
        if out.contains(&tech) {
            return Err(err(format!("{id:?} listed twice")));
        }
        if !techs[i].open_to(nation) {
            return Err(match techs[i].nation {
                Some(n) => err(format!("{id:?} belongs to {}", n.as_str())),
                None => err(format!("{id:?} is for other nations only")),
            });
        }
        out.push(tech);
    }
    for &t in &out {
        let def = &techs[t.0 as usize];
        if let Some(p) = def.prereqs.iter().find(|p| !out.contains(p)) {
            return Err(err(format!(
                "{:?} requires {:?}, which must also be a starting tech",
                def.id, techs[p.0 as usize].id
            )));
        }
    }
    Ok(out)
}

/// A tech finishing, as logged in the world's event log (`World::tech_completions`, #605).
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct TechCompleted {
    pub tech: TechId,
    /// The world tick on which the tech completed.
    pub tick: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResearchError {
    /// No tech with this id exists.
    UnknownTech,
    /// The tech belongs to another nation's branch.
    WrongNation,
    AlreadyKnown,
    /// A tech the target would queue (the target itself or a prereq) has a `Requires` condition
    /// the settlement doesn't meet yet. Holds the first one, reads `needs 3 scrolls read` or
    /// `needs Known on the road`.
    NotYet(Condition),
    /// `Command::ClearResearch` with no finished Archive: clearing the target is only for
    /// starting to scribe (#709).
    NeedsArchive,
    /// The tech is one option of a school whose other option, `chosen`, is already known
    /// (docs/design/horizons-phase-4.md). Clients name it: `Closed: you chose Metal City`.
    Closed {
        chosen: TechId,
    },
}

impl std::fmt::Display for ResearchError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnknownTech => f.write_str("no such tech"),
            Self::WrongNation => f.write_str("that tech belongs to another nation"),
            Self::AlreadyKnown => f.write_str("that tech is already known"),
            Self::NotYet(c) => write!(f, "needs {c}"),
            Self::NeedsArchive => f.write_str("needs an Archive"),
            Self::Closed { .. } => f.write_str("the other path of this school was chosen"),
        }
    }
}

impl std::error::Error for ResearchError {}

/// A settlement's research state. Knowledge is one pool shared by the whole settlement; the
/// queue is spent from it in order, and leftover knowledge carries over.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Research {
    /// Indexed by `TechId`.
    known: Vec<bool>,
    pool_millis: u64,
    /// The next tech to finish is at the front. Always ascending by `TechId`, so every tech's
    /// unknown prereqs are ahead of it.
    queue: Vec<TechId>,
    /// A v1 to v3 save's completions (#605). Completions are logged in `World::events` now;
    /// these are merged into it on load, never written.
    #[serde(default, skip_serializing, rename = "log")]
    v3_log: Vec<TechCompleted>,
    /// Knowledge the pool's cap trimmed while scribing was on, in milli-points
    /// (docs/design/horizons-phase-3.md, "Scribing"; #709). `World::tick_scribing` turns it into
    /// scroll copies, `copy_points` x 1000 at a time. Kept when the last Archive comes down.
    #[serde(default)]
    scribe_millis: u64,
    /// What has been delivered toward queued techs' `cost_items` (docs/design/
    /// horizons-phase-4.md, "Item bills"). A bill exists once its first item arrives, and goes
    /// when its tech completes or leaves the queue.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    bills: Vec<Bill>,
    /// Items delivered toward a tech that left the queue, waiting for a finished bill hall to be
    /// dropped beside (`World::tick_bills`). Empty whenever a hall stands.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    refunds: Vec<Stack>,
}

/// Items delivered so far toward one queued tech's `cost_items`, in the def's line order.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Bill {
    pub tech: TechId,
    pub delivered: Vec<u32>,
}

impl Research {
    /// Seeds the known set from the nation's `starting_techs`, with an empty pool and queue.
    pub fn new(nation: Nation, defs: &Defs) -> Self {
        let mut known = vec![false; defs.techs.len()];
        for t in &defs.nation(nation).starting_techs {
            known[t.0 as usize] = true;
        }
        Self {
            known,
            pool_millis: 0,
            queue: Vec::new(),
            v3_log: Vec::new(),
            scribe_millis: 0,
            bills: Vec::new(),
            refunds: Vec::new(),
        }
    }

    /// Does this state cover exactly `defs`' techs, with a queue that names only real ones? False
    /// for a state read from a save made against other data.
    pub(crate) fn fits(&self, defs: &Defs) -> bool {
        self.known.len() == defs.techs.len()
            && self.queue.iter().all(|t| (t.0 as usize) < defs.techs.len())
            && self.bills.iter().all(|b| (b.tech.0 as usize) < defs.techs.len())
    }

    /// Rebuilds the positional state of a research state read from a save for today's tech
    /// table (#602): `known` is indexed by the save's own tech table, so it is remapped through
    /// `map` (today's index of each saved index, `None` where the tech is gone) to `defs.techs`.
    /// The queue's ids were already remapped as they were read; it is re-sorted, because its
    /// invariant is ascending `TechId` and file order keeps prereqs first. Fails with the saved
    /// index of a known tech today's data doesn't have, or `None` if `known` doesn't match the
    /// save's table at all.
    pub(crate) fn remap(
        &mut self,
        map: &[Option<usize>],
        defs: &Defs,
    ) -> Result<(), Option<usize>> {
        if self.known.len() != map.len() {
            return Err(None);
        }
        let mut known = vec![false; defs.techs.len()];
        for (saved, _) in self.known.iter().enumerate().filter(|(_, k)| **k) {
            known[map[saved].ok_or(Some(saved))?] = true;
        }
        self.known = known;
        self.queue.sort_unstable();
        Ok(())
    }

    pub fn is_known(&self, tech: TechId) -> bool {
        self.known.get(tech.0 as usize).copied().unwrap_or(false)
    }

    /// Known techs in `TechId` order.
    pub fn known_techs(&self) -> impl Iterator<Item = TechId> + '_ {
        self.known.iter().enumerate().filter(|(_, k)| **k).map(|(i, _)| TechId(i as u16))
    }

    /// Banked knowledge in milli-points (1000 per point).
    pub fn pool_millis(&self) -> u64 {
        self.pool_millis
    }

    /// The research queue, front first.
    pub fn queue(&self) -> &[TechId] {
        &self.queue
    }

    /// Knowledge waiting to become scroll copies, in milli-points (#709).
    pub fn scribe_millis(&self) -> u64 {
        self.scribe_millis
    }

    /// Spends `millis` of `scribe_millis` on a copy. Only `World::tick_scribing` calls it, having
    /// checked there is that much.
    pub(crate) fn spend_scribe(&mut self, millis: u64) {
        self.scribe_millis = self.scribe_millis.saturating_sub(millis);
    }

    /// Empties the queue (`Command::ClearResearch`). What the pool holds above
    /// `idle_knowledge_cap` moves to `scribe_millis`, so nothing banked toward the dropped
    /// target is lost (#709). The caller has checked there is an Archive.
    pub(crate) fn clear_target(&mut self, defs: &Defs) {
        self.queue.clear();
        self.drop_stale_bills(defs);
        let cap = defs.research.idle_knowledge_cap_millis;
        let over = self.pool_millis.saturating_sub(cap);
        self.pool_millis -= over;
        self.scribe_millis += over;
    }

    /// Whether a pre-v4 save's completions are still waiting for `World::upgrade_from_v3`.
    pub(crate) fn has_v3_log(&self) -> bool {
        !self.v3_log.is_empty()
    }

    /// A pre-v4 save's completions, leaving none behind.
    pub(crate) fn take_v3_log(&mut self) -> Vec<TechCompleted> {
        std::mem::take(&mut self.v3_log)
    }

    /// Replaces the queue with `tech` plus its unknown prereqs, in ascending `TechId` order.
    /// Knowledge already banked is kept. Refuses with `ResearchError::NotYet`, leaving the queue
    /// as it was, if any tech it would queue has a `Requires` condition `facts` doesn't meet.
    /// Conditions are checked here only, once, because every one so far only ever becomes true.
    pub fn set_target(
        &mut self,
        tech: TechId,
        nation: Nation,
        facts: &Facts<'_>,
        defs: &Defs,
    ) -> Result<(), ResearchError> {
        let def = defs.techs.get(tech.0 as usize).ok_or(ResearchError::UnknownTech)?;
        if !def.open_to(nation) {
            return Err(ResearchError::WrongNation);
        }
        if self.is_known(tech) {
            return Err(ResearchError::AlreadyKnown);
        }
        // No tech requires a school tech (load refuses it), so only the target itself can be
        // closed.
        if let Some(chosen) = self.closed_by(tech, defs) {
            return Err(ResearchError::Closed { chosen });
        }
        // Prereqs always have smaller ids, so one descending pass finds the whole closure.
        let mut needed = vec![false; tech.0 as usize + 1];
        needed[tech.0 as usize] = true;
        for i in (0..=tech.0 as usize).rev() {
            if needed[i] && !self.known[i] {
                for p in &defs.techs[i].prereqs {
                    needed[p.0 as usize] = true;
                }
            }
        }
        let queue: Vec<TechId> = (0..needed.len())
            .filter(|&i| needed[i] && !self.known[i])
            .map(|i| TechId(i as u16))
            .collect();
        for &t in &queue {
            if let Some(c) = defs.tech(t).requires.unmet(facts, defs).into_iter().next() {
                return Err(ResearchError::NotYet(c));
            }
        }
        self.queue = queue;
        self.drop_stale_bills(defs);
        Ok(())
    }

    /// Moves what was delivered toward any tech no longer queued to `refunds`.
    fn drop_stale_bills(&mut self, defs: &Defs) {
        let queue = &self.queue;
        let (keep, stale): (Vec<Bill>, Vec<Bill>) =
            std::mem::take(&mut self.bills).into_iter().partition(|b| queue.contains(&b.tech));
        self.bills = keep;
        for bill in stale {
            for (line, &n) in defs.tech(bill.tech).cost_items.iter().zip(&bill.delivered) {
                if n > 0 {
                    self.refunds.push(Stack { item: line.item, count: n });
                }
            }
        }
    }

    /// The bill of a queued tech: its `cost_items` with what has been delivered of each, as
    /// `(item, delivered, needed)`, in line order. Empty for a tech with no `cost_items`.
    pub fn bill(&self, tech: TechId, defs: &Defs) -> Vec<(ItemId, u32, u32)> {
        let delivered = self.bills.iter().find(|b| b.tech == tech).map(|b| &b.delivered);
        defs.tech(tech)
            .cost_items
            .iter()
            .enumerate()
            .map(|(k, line)| {
                let got = delivered.and_then(|d| d.get(k)).copied().unwrap_or(0);
                (line.item, got.min(line.count), line.count)
            })
            .collect()
    }

    /// Whether `tech`'s bill is in: true for a tech with no `cost_items`.
    pub fn bill_full(&self, tech: TechId, defs: &Defs) -> bool {
        self.bill(tech, defs).iter().all(|&(_, got, need)| got >= need)
    }

    /// What queued techs still owe, front first: `(tech, item, still owed)` for each short line.
    pub fn bills_owed<'a>(
        &'a self,
        defs: &'a Defs,
    ) -> impl Iterator<Item = (TechId, ItemId, u32)> + 'a {
        self.queue.iter().flat_map(move |&t| {
            self.bill(t, defs)
                .into_iter()
                .filter(|&(_, got, need)| got < need)
                .map(move |(item, got, need)| (t, item, need - got))
        })
    }

    /// Puts up to `count` of `item` on the first queued bill short of it, never more than it
    /// owes. Returns how many it took; the rest stays with the carrier.
    pub(crate) fn pay_bill(&mut self, item: ItemId, count: u32, defs: &Defs) -> u32 {
        let Some((tech, _, owed)) = self.bills_owed(defs).find(|&(_, i, _)| i == item) else {
            return 0;
        };
        let put = count.min(owed);
        let lines = &defs.tech(tech).cost_items;
        let k = lines.iter().position(|l| l.item == item).expect("owed, so listed");
        let bill = match self.bills.iter().position(|b| b.tech == tech) {
            Some(b) => &mut self.bills[b],
            None => {
                self.bills.push(Bill { tech, delivered: vec![0; lines.len()] });
                self.bills.last_mut().expect("just pushed")
            }
        };
        if bill.delivered.len() < lines.len() {
            bill.delivered.resize(lines.len(), 0);
        }
        bill.delivered[k] += put;
        put
    }

    /// Whether the front tech is waiting only on its bill: the pool already covers its scaled
    /// cost and some line is short. Study seats are then not worth claiming (#284's rule), so
    /// scholars become hands for the hauling.
    pub fn waiting_on_bill(&self, defs: &Defs, research_cost: f32) -> bool {
        self.queue.first().is_some_and(|&front| {
            self.pool_millis >= scaled_cost_millis(defs.tech(front).cost, research_cost)
                && !self.bill_full(front, defs)
        })
    }

    /// Items waiting to be dropped beside a bill hall, leaving none behind.
    pub(crate) fn take_refunds(&mut self) -> Vec<Stack> {
        std::mem::take(&mut self.refunds)
    }

    /// Whether any refund is waiting for a bill hall.
    pub(crate) fn has_refunds(&self) -> bool {
        !self.refunds.is_empty()
    }

    /// The known school mate that closes `tech`, if any: `None` for a tech outside a school, and
    /// for a school tech while no other option of its school is learned. Closed is derived from
    /// `known` alone, so nothing about it is saved. Queuing a school tech closes nothing.
    pub fn closed_by(&self, tech: TechId, defs: &Defs) -> Option<TechId> {
        defs.school_mates(tech).find(|&m| self.is_known(m))
    }

    /// Adds knowledge straight to the pool, outside the passive trickle `tick` adds on its own:
    /// an ancient scroll, read and used up at a study hall (`world/work.rs`, #24). A one-off
    /// reward, so unlike `add_toward_target` it isn't capped. Techs it can now afford complete
    /// on the next `tick`, not immediately, so knowledge earned partway through a tick can't
    /// retroactively unlock something earlier in that same tick.
    pub fn add_knowledge(&mut self, millis: u64) {
        self.pool_millis += millis;
    }

    /// The most the pool may hold right now: `research_cost` times the cost of the tech at the
    /// front of the queue (the one it is currently paying toward), or the unscaled
    /// `idle_knowledge_cap_millis` with nothing queued (a flat buffer, not a cost, so the
    /// difficulty preset's `research_cost` knob doesn't touch it). Used by `add_toward_target` so
    /// a steady, no-effort source can't bank ahead of what is actually being worked toward
    /// (#284).
    fn cap_millis(&self, defs: &Defs, research_cost: f32) -> u64 {
        match self.queue.first() {
            Some(&front) => scaled_cost_millis(defs.tech(front).cost, research_cost),
            None => defs.research.idle_knowledge_cap_millis,
        }
    }

    /// Adds knowledge from a source that runs on its own with no cost to the player: the passive
    /// "oral tradition" trickle (`tick`) and a seated scholar (`world/study.rs`, #21). Clamped at
    /// `cap_millis` so hours of it can't quietly bank toward several techs at once, the bug
    /// #284 fixed (an ancient scroll, `add_knowledge`, is a deliberate one-off reward and isn't
    /// capped this way). `research_cost` is the difficulty preset's knob (1.0 is identity, #294).
    /// With `scribing` on (`World::scribing_on`, #709) what the clamp trims goes to
    /// `scribe_millis` instead of being lost; the pool's own arithmetic is the same either way.
    pub(crate) fn add_toward_target(
        &mut self,
        millis: u64,
        defs: &Defs,
        research_cost: f32,
        scribing: bool,
    ) {
        let total = self.pool_millis + millis;
        let cap = self.cap_millis(defs, research_cost);
        self.pool_millis = total.min(cap);
        if scribing {
            self.scribe_millis += total - self.pool_millis;
        }
    }

    /// Marks `tech` known at once, skipping the queue and its cost. For tests that need a nation
    /// to have a tech without waiting out the research.
    #[cfg(test)]
    pub(crate) fn grant(&mut self, tech: TechId) {
        self.known[tech.0 as usize] = true;
    }

    /// Adds knowledge straight to the pool with no cap, for tests that want to skip ahead of
    /// what natural production could bank (`add_toward_target`'s cap is what #284 is about, not
    /// these tests).
    #[cfg(test)]
    pub(crate) fn bank_for_test(&mut self, millis: u64) {
        self.pool_millis += millis;
    }

    /// Adds one tick of passive knowledge, then completes queued techs the pool can pay for,
    /// returning them in order (the world logs them, `World::tech_completions`). `tick` is the
    /// world tick being simulated, recorded on completions. `research_cost` is the difficulty
    /// preset's knob, multiplying every tech's cost (1.0 is identity, #294).
    #[must_use = "the completions belong in the world's event log"]
    pub fn tick(&mut self, tick: u64, defs: &Defs, research_cost: f32) -> Vec<TechCompleted> {
        self.tick_scribing(tick, defs, research_cost, false)
    }

    /// `tick`, with the passive trickle's overflow scribed when `scribing` is on (#709).
    #[must_use = "the completions belong in the world's event log"]
    pub(crate) fn tick_scribing(
        &mut self,
        tick: u64,
        defs: &Defs,
        research_cost: f32,
        scribing: bool,
    ) -> Vec<TechCompleted> {
        let mut done = Vec::new();
        self.add_toward_target(
            defs.research.passive_millis_per_tick,
            defs,
            research_cost,
            scribing,
        );
        while let Some(&front) = self.queue.first() {
            let cost = scaled_cost_millis(defs.tech(front).cost, research_cost);
            if cost > self.pool_millis || !self.bill_full(front, defs) {
                break;
            }
            self.pool_millis -= cost;
            self.queue.remove(0);
            self.bills.retain(|b| b.tech != front);
            self.known[front.0 as usize] = true;
            done.push(TechCompleted { tech: front, tick });
        }
        done
    }
}

/// `cost` points in milli-points, times the difficulty preset's `research_cost` knob (1.0 is
/// identity). `f64` only to multiply by a non-integer factor; exact for the small integer costs
/// `data/research.toml` validates, so `research_cost == 1.0` reproduces the unscaled value
/// bit-for-bit.
fn scaled_cost_millis(cost: u32, research_cost: f32) -> u64 {
    (f64::from(cost) * f64::from(research_cost) * MILLIS_PER_POINT as f64).round() as u64
}

#[cfg(test)]
mod tests {
    use crate::defs::{Defs, DefsError};
    use crate::nation::Nation;

    use super::TechId;

    const BIOMES: &str = include_str!("../../../data/biomes.toml");
    const RESOURCES: &str = include_str!("../../../data/resources.toml");
    const NEEDS: &str = include_str!("../../../data/needs.toml");
    const JOBS: &str = include_str!("../../../data/jobs.toml");
    const NATIONS: &str = include_str!("../../../data/nations.toml");
    const RESEARCH: &str = include_str!("../../../data/research.toml");
    const BENDING: &str = include_str!("../../../data/bending.toml");
    const BUILDINGS: &str = include_str!("../../../data/buildings.toml");

    const SETTINGS: &str = "[settings]\npassive_knowledge_per_minute = 3.0\n\
         study_knowledge_per_minute = 9.0\nidle_knowledge_cap = 10.0\n";

    /// The bundled starting techs, which every test tree below must define.
    const BASICS: &str = "
        [[tech]]
        id = 'fire_making'
        name = 'F'
        description = ''
        cost = 25
        [[tech]]
        id = 'fishing'
        name = 'F'
        description = ''
        cost = 25
        [[tech]]
        id = 'stoneworking'
        name = 'S'
        description = ''
        cost = 25
        [[tech]]
        id = 'orchards'
        name = 'O'
        description = ''
        nation = 'air'
        cost = 25
    ";

    fn load(research: &str, nations: &str) -> Result<Defs, DefsError> {
        Defs::from_toml(BIOMES, RESOURCES, nations, research, NEEDS, JOBS, BENDING, BUILDINGS)
    }

    /// Loads the bundled nations with `BASICS` plus `techs`.
    fn with_techs(techs: &str) -> Result<Defs, DefsError> {
        load(&format!("{SETTINGS}{BASICS}{techs}"), NATIONS)
    }

    fn tech(id: &str, extra: &str) -> String {
        format!("\n[[tech]]\nid = '{id}'\nname = '{id}'\ndescription = ''\n{extra}\n")
    }

    fn assert_err(r: Result<Defs, DefsError>, needle: &str) {
        let err = r.expect_err("should be rejected");
        assert!(err.0.contains(needle), "expected {needle:?} in: {err}");
    }

    #[test]
    fn bundled_tree_loads() {
        let defs = Defs::bundled();
        assert!(defs.techs.len() >= 11 + 6 * 3 + 7);
        let writing = defs.tech(defs.tech_id("writing").unwrap());
        assert_eq!(writing.nation, None);
        assert_eq!(writing.unlocks.modifiers.study, Some(1.25));
        assert_eq!(writing.prereqs, [defs.tech_id("pottery").unwrap()]);
        let walls = defs.tech(defs.tech_id("earthen_walls").unwrap());
        assert_eq!(walls.nation, Some(Nation::Earth));
        assert_eq!(walls.cost, 120);
        assert!(defs.tech_id("bloodbending").is_none());
        assert_eq!(defs.research.study_knowledge_per_minute, 9.0);
    }

    #[test]
    fn bundled_tree_has_twelve_shared_techs_and_a_branch_per_nation() {
        let defs = Defs::bundled();
        // Core techs only: the optional phase-1 techs (docs/design/horizons-phase-1.md) sit
        // outside the tree this pins.
        let count = |n| defs.techs.iter().filter(|t| t.nation == n && t.core).count();
        // 12, not 11: #383 added the shared `prospecting` tech, gating gold mining the way
        // `stoneworking` already gates stone.
        assert_eq!(count(None), 12);
        for n in [Nation::Earth, Nation::Fire, Nation::Water] {
            assert_eq!(count(Some(n)), 6, "{n:?}");
        }
        // Air's branch also holds its starting tech, `orchards` (owner decision, research.md).
        assert_eq!(count(Some(Nation::Air)), 7);
    }

    #[test]
    fn bundled_starting_techs() {
        let defs = Defs::bundled();
        for (n, id) in [
            (Nation::Fire, "fire_making"),
            (Nation::Water, "fishing"),
            (Nation::Earth, "stoneworking"),
            (Nation::Air, "orchards"),
        ] {
            assert_eq!(defs.nation(n).starting_techs, [defs.tech_id(id).unwrap()], "{n:?}");
        }
    }

    #[test]
    fn every_nation_starts_with_unit_study() {
        let defs = Defs::bundled();
        for n in Nation::ALL {
            assert_eq!(defs.nation(n).modifiers.study, 1.0, "{n:?}");
        }
    }

    #[test]
    fn prereqs_resolve_in_topological_order() {
        let defs =
            with_techs(&tech("a", "cost = 1\nprereqs = ['fire_making', 'fishing']")).unwrap();
        let a = defs.tech_id("a").unwrap();
        let expect = [defs.tech_id("fire_making").unwrap(), defs.tech_id("fishing").unwrap()];
        assert_eq!(defs.tech(a).prereqs, expect);
        assert!(defs.tech(a).prereqs.iter().all(|&p| p < a));
        assert_eq!(defs.tech(TechId(0)).id, "fire_making");
    }

    #[test]
    fn duplicate_id_is_rejected() {
        assert_err(with_techs(&tech("fishing", "cost = 1")), "duplicate id");
    }

    #[test]
    fn unknown_prereq_is_rejected() {
        assert_err(with_techs(&tech("a", "cost = 1\nprereqs = ['alchemy']")), "alchemy");
    }

    #[test]
    fn prereq_after_its_dependant_is_rejected() {
        let src = tech("a", "cost = 1\nprereqs = ['b']") + &tech("b", "cost = 1");
        assert_err(with_techs(&src), "defined earlier");
    }

    #[test]
    fn self_prereq_is_rejected() {
        assert_err(with_techs(&tech("a", "cost = 1\nprereqs = ['a']")), "unknown prereq");
    }

    #[test]
    fn duplicate_prereq_is_rejected() {
        let src = tech("a", "cost = 1\nprereqs = ['fishing', 'fishing']");
        assert_err(with_techs(&src), "twice");
    }

    #[test]
    fn shared_tech_requiring_a_nation_tech_is_rejected() {
        assert_err(with_techs(&tech("a", "cost = 1\nprereqs = ['orchards']")), "shared tech");
    }

    #[test]
    fn nation_tech_requiring_another_nations_tech_is_rejected() {
        let src = tech("a", "nation = 'fire'\ncost = 1\nprereqs = ['orchards']");
        assert_err(with_techs(&src), "air tech");
    }

    #[test]
    fn nation_tech_may_require_shared_and_own_techs() {
        let src = tech("a", "nation = 'air'\ncost = 1\nprereqs = ['fishing', 'orchards']");
        assert!(with_techs(&src).is_ok());
    }

    #[test]
    fn a_nations_tech_names_two_or_more_nations_and_not_nation() {
        let ok = tech("a", "nations = ['earth', 'fire']\ncost = 1\nprereqs = ['fishing']");
        let defs = with_techs(&ok).unwrap();
        let a = defs.tech(defs.tech_id("a").unwrap());
        assert_eq!((a.nation, a.nations.clone()), (None, vec![Nation::Earth, Nation::Fire]));
        assert!(!a.is_shared());
        for n in Nation::ALL {
            assert_eq!(a.open_to(n), matches!(n, Nation::Earth | Nation::Fire), "{n:?}");
            assert_eq!(defs.techs_for(n).any(|(_, t)| t.id == "a"), a.open_to(n), "{n:?}");
        }
        let one = tech("a", "nations = ['earth']\ncost = 1");
        assert_err(with_techs(&one), "at least two");
        let both = tech("a", "nation = 'earth'\nnations = ['earth', 'fire']\ncost = 1");
        assert_err(with_techs(&both), "can't be combined");
        let twice = tech("a", "nations = ['fire', 'fire']\ncost = 1");
        assert_err(with_techs(&twice), "listed twice");
        let unknown = tech("a", "nations = ['fire', 'mars']\ncost = 1");
        assert!(with_techs(&unknown).is_err());
    }

    #[test]
    fn a_nations_tech_requires_only_shared_techs() {
        let own = tech("a", "nation = 'air'\ncost = 1\nprereqs = ['fishing']");
        let src = own + &tech("b", "nations = ['air', 'fire']\ncost = 1\nprereqs = ['a']");
        assert_err(with_techs(&src), "only require shared");
        let air = tech("b", "nations = ['air', 'fire']\ncost = 1\nprereqs = ['orchards']");
        assert_err(with_techs(&air), "only require shared");
        let shared = tech("b", "nations = ['air', 'fire']\ncost = 1\nprereqs = ['fishing']");
        assert!(with_techs(&shared).is_ok());
    }

    #[test]
    fn no_tech_requires_a_nations_tech() {
        let base = tech("a", "nations = ['earth', 'fire']\ncost = 1");
        for extra in ["", "nation = 'earth'\n"] {
            let src = base.clone() + &tech("b", &format!("{extra}cost = 1\nprereqs = ['a']"));
            assert_err(with_techs(&src), "`nations` tech");
        }
    }

    #[test]
    fn a_nations_tech_is_refused_to_an_unlisted_nation() {
        let src = tech("a", "nations = ['earth', 'fire']\ncost = 1");
        let defs = with_techs(&src).unwrap();
        let a = defs.tech_id("a").unwrap();
        for n in Nation::ALL {
            let r =
                super::Research::new(n, &defs).set_target(a, n, &super::Facts::default(), &defs);
            let listed = matches!(n, Nation::Earth | Nation::Fire);
            assert_eq!(r.is_ok(), listed, "{n:?}");
            if !listed {
                assert_eq!(r, Err(super::ResearchError::WrongNation));
            }
        }
    }

    #[test]
    fn zero_cost_is_rejected() {
        assert_err(with_techs(&tech("a", "cost = 0")), "cost must be > 0");
    }

    #[test]
    fn effective_modifiers_ignore_the_order_techs_are_listed_in() {
        use super::{effective_modifiers, modifier_breakdown};
        let defs = Defs::bundled();
        let id = |s: &str| defs.tech_id(s).unwrap();
        // Three techs that all multiply `construction`.
        let known = ["carpentry", "masonry", "earthen_walls"].map(id);
        let bits = |k: &[TechId]| {
            let m = effective_modifiers(Nation::Earth, k.iter().copied(), &defs);
            m.construction.to_bits()
        };
        let want = bits(&known);
        let mut k = known;
        for _ in 0..known.len() {
            k.rotate_left(1);
            assert_eq!(bits(&k), want);
            k.reverse();
            assert_eq!(bits(&k), want);
        }
        // The breakdown multiplies out (left to right) to the effective value.
        let parts = modifier_breakdown("construction", Nation::Earth, known, &defs);
        let names: Vec<_> = parts.iter().map(|p| p.0).collect();
        assert_eq!(names, ["Earth Kingdom", "Carpentry", "Masonry", "Earthen Walls"]);
        assert_eq!(parts.iter().fold(1.0f32, |acc, p| acc * p.1).to_bits(), want);
    }

    #[test]
    fn bad_multipliers_are_rejected() {
        for m in ["0.0", "-1.0", "4.01", "inf", "nan"] {
            let src =
                tech("a", &format!("cost = 1\n[tech.unlocks]\nmodifiers = {{ heating = {m} }}"));
            assert_err(with_techs(&src), "multiplier heating");
        }
        let ok = tech("a", "cost = 1\n[tech.unlocks]\nmodifiers = { heating = 4.0, study = 0.5 }");
        assert!(with_techs(&ok).is_ok());
    }

    #[test]
    fn unknown_modifier_name_is_rejected() {
        let src = tech("a", "cost = 1\n[tech.unlocks]\nmodifiers = { contstruction = 1.2 }");
        assert_err(with_techs(&src), "contstruction");
    }

    #[test]
    fn unknown_tech_fields_are_rejected() {
        assert_err(with_techs(&tech("a", "cost = 1\ncots = 2")), "cots");
    }

    #[test]
    fn bad_settings_are_rejected() {
        let src = RESEARCH
            .replace("passive_knowledge_per_minute = 3.0", "passive_knowledge_per_minute = -1.0");
        assert_ne!(src, RESEARCH);
        assert_err(load(&src, NATIONS), "passive_knowledge_per_minute");
    }

    fn with_air_start(ids: &str) -> Result<Defs, DefsError> {
        let nations =
            NATIONS.replace("starting_techs = [\"orchards\"]", &format!("starting_techs = {ids}"));
        assert_ne!(nations, NATIONS);
        let extra = tech("gliders", "nation = 'air'\ncost = 1\nprereqs = ['orchards']")
            + &tech("forging", "nation = 'fire'\ncost = 1");
        load(&format!("{SETTINGS}{BASICS}{extra}"), &nations)
    }

    #[test]
    fn starting_techs_may_be_shared_or_own() {
        let defs = with_air_start("['orchards', 'gliders', 'fishing']").unwrap();
        assert_eq!(defs.nation(Nation::Air).starting_techs.len(), 3);
    }

    #[test]
    fn starting_tech_of_another_nation_is_rejected() {
        assert_err(with_air_start("['orchards', 'forging']"), "belongs to fire");
    }

    #[test]
    fn starting_techs_must_be_closed_under_prereqs() {
        assert_err(with_air_start("['gliders']"), "must also be a starting tech");
    }

    #[test]
    fn unknown_starting_tech_is_rejected() {
        assert_err(with_air_start("['levitation']"), "levitation");
    }

    #[test]
    fn features_are_a_closed_list_with_one_tech_each() {
        let with = |extra: &str| {
            let mut files = crate::defs::BUNDLED_FILES;
            let research = format!(
                "{}\n[[tech]]\nid = \"t\"\nname = \"T\"\ndescription = \"\"\ncost = 10\n\
                 [tech.unlocks]\n{extra}\n",
                files[3]
            );
            files[3] = &research;
            Defs::from_files(files).map(|_| ())
        };
        assert!(with("features = [\"weather\"]").is_err(), "unknown name");
        let e = with("features = [\"almanac\"]").unwrap_err().0;
        assert!(e.contains("already unlocked by \"calendar\""), "{e}");
        let defs = Defs::bundled();
        let calendar = defs.tech_id("calendar").unwrap();
        assert_eq!(defs.feature_tech(super::Feature::Almanac), Some(calendar));
        assert!(!defs.tech(calendar).core, "an optional horizons tech");
    }

    // Research conditions (`requires`, docs/design/horizons-phase-3.md).

    use super::{Condition, Facts, Research, ResearchError};

    /// The bundled files plus one extra tech, `gated`, with `extra` as its keys.
    fn with_gated(extra: &str) -> Result<Defs, DefsError> {
        let mut files = crate::defs::BUNDLED_FILES;
        let research = format!(
            "{}\n[[tech]]\nid = \"gated\"\nname = \"Gated\"\ndescription = \"\"\ncost = 10\n{extra}\n",
            files[3]
        );
        files[3] = &research;
        Defs::from_files(files)
    }

    const BOTH: &str = "requires = { scrolls_read = 3, chapter = \"known_on_the_road\" }";

    #[test]
    fn requires_takes_either_key_or_both() {
        let none = with_gated("").unwrap();
        assert!(none.tech(none.tech_id("gated").unwrap()).requires.is_empty());
        for (extra, scrolls, chapter) in [
            ("requires = { scrolls_read = 3 }", Some(3), None),
            ("requires = { scrolls_read = 1 }", Some(1), None),
            ("requires = { chapter = \"known_on_the_road\" }", None, Some("known_on_the_road")),
            (BOTH, Some(3), Some("known_on_the_road")),
            ("requires = { scrolls_read = 20 }", Some(20), None),
        ] {
            let defs = with_gated(extra).unwrap();
            let r = &defs.tech(defs.tech_id("gated").unwrap()).requires;
            assert_eq!((r.scrolls_read, r.chapter.as_deref()), (scrolls, chapter), "{extra}");
        }
    }

    #[test]
    fn requires_is_checked_strictly() {
        for bad in ["0", "21", "1000"] {
            let e = with_gated(&format!("requires = {{ scrolls_read = {bad} }}")).unwrap_err();
            assert!(e.0.contains("scrolls_read") && e.0.contains("1 to 20"), "{bad}: {e}");
        }
        let e = with_gated("requires = { scrolls_read = -1 }").unwrap_err();
        assert!(e.0.contains("scrolls_read"), "{e}");
        let e = with_gated("requires = { chapter = \"nowhere\" }").unwrap_err();
        assert!(e.0.contains("gated") && e.0.contains("unknown chapter \"nowhere\""), "{e}");
        let e = with_gated("requires = { population = 5 }").unwrap_err();
        assert!(e.0.contains("population"), "an unknown key fails to load: {e}");
    }

    #[test]
    fn a_refused_target_leaves_the_queue_alone_and_names_the_condition() {
        let defs = with_gated(BOTH).unwrap();
        let gated = defs.tech_id("gated").unwrap();
        let mut r = Research::new(Nation::Fire, &defs);
        let shelter = defs.tech_id("shelter").unwrap();
        r.set_target(shelter, Nation::Fire, &Facts::default(), &defs).unwrap();
        let queue = r.queue().to_vec();

        let refused = r.set_target(gated, Nation::Fire, &Facts::default(), &defs);
        assert_eq!(refused, Err(ResearchError::NotYet(Condition::ScrollsRead(3))));
        assert_eq!(refused.unwrap_err().to_string(), "needs 3 scrolls read");
        assert_eq!(r.queue(), queue, "a refusal changes nothing");

        // The scrolls alone aren't enough, and then the error names the chapter.
        let scrolls = Facts { scrolls_read: 3, ..Facts::default() };
        let refused = r.set_target(gated, Nation::Fire, &scrolls, &defs).unwrap_err();
        assert_eq!(refused.to_string(), "needs Known on the road");
        assert_eq!(r.queue(), queue);
        let near = Facts { scrolls_read: 2, chapters_met: vec!["known_on_the_road"] };
        let refused = r.set_target(gated, Nation::Fire, &near, &defs).unwrap_err();
        assert_eq!(refused.to_string(), "needs 3 scrolls read");
    }

    #[test]
    fn a_target_is_accepted_once_its_conditions_are_met() {
        let defs = with_gated(BOTH).unwrap();
        let gated = defs.tech_id("gated").unwrap();
        let mut r = Research::new(Nation::Fire, &defs);
        let met = Facts { scrolls_read: 3, chapters_met: vec!["known_on_the_road"] };
        r.set_target(gated, Nation::Fire, &met, &defs).unwrap();
        assert_eq!(r.queue(), [gated]);
        // More than enough counts too.
        let lots = Facts { scrolls_read: 9, ..met };
        r.set_target(gated, Nation::Fire, &lots, &defs).unwrap();
    }

    #[test]
    fn a_gated_prereq_refuses_the_techs_that_need_it() {
        let src = tech("gate", "cost = 1\nrequires = { scrolls_read = 1 }")
            + &tech("after", "cost = 1\nprereqs = ['gate']");
        let defs = with_techs(&src).unwrap();
        let after = defs.tech_id("after").unwrap();
        let mut r = Research::new(Nation::Fire, &defs);
        let e = r.set_target(after, Nation::Fire, &Facts::default(), &defs).unwrap_err();
        assert_eq!(e.to_string(), "needs 1 scroll read");
        let one = Facts { scrolls_read: 1, ..Facts::default() };
        r.set_target(after, Nation::Fire, &one, &defs).unwrap();
        assert_eq!(r.queue().len(), 2);
    }

    #[test]
    fn a_known_gated_tech_no_longer_blocks_its_dependants() {
        let src = tech("gate", "cost = 1\nrequires = { scrolls_read = 1 }")
            + &tech("after", "cost = 1\nprereqs = ['gate']");
        let defs = with_techs(&src).unwrap();
        let (gate, after) = (defs.tech_id("gate").unwrap(), defs.tech_id("after").unwrap());
        let mut r = Research::new(Nation::Fire, &defs);
        r.grant(gate);
        r.set_target(after, Nation::Fire, &Facts::default(), &defs).unwrap();
        assert_eq!(r.queue(), [after]);
    }

    // Schools (docs/design/horizons-phase-4.md, "Shared sim work, 1. Schools").

    /// The bundled files plus `extra` appended to the research file.
    fn with_research(extra: &str) -> Result<Defs, DefsError> {
        let mut files = crate::defs::BUNDLED_FILES;
        let research = format!("{}\n{extra}\n", files[3]);
        files[3] = &research;
        Defs::from_files(files)
    }

    fn school_tech(id: &str, nation: &str, school: &str, extra: &str) -> String {
        format!(
            "[[tech]]\nid = \"{id}\"\nname = \"{id}\"\ndescription = \"\"\nnation = \"{nation}\"\n\
             cost = 1000\nprereqs = [\"forging\"]\nschool = \"{school}\"\n{extra}\n"
        )
    }

    const FIRE_SCHOOL: &str =
        "[[school]]\nid = \"fire_purpose\"\nnation = \"fire\"\nquestion = \"What is fire for?\"\n";

    /// A valid fire school, `sun` and `works`, plus `extra`.
    fn fire_fork(extra: &str) -> Result<Defs, DefsError> {
        with_research(&format!(
            "{FIRE_SCHOOL}{}{}{extra}",
            school_tech("sun", "fire", "fire_purpose", ""),
            school_tech("works", "fire", "fire_purpose", "")
        ))
    }

    #[test]
    fn a_school_loads_with_its_two_techs() {
        let defs = fire_fork("").unwrap();
        let (sun, works) = (defs.tech_id("sun").unwrap(), defs.tech_id("works").unwrap());
        let school = defs.tech(sun).school.unwrap();
        assert_eq!(defs.school(school).question, "What is fire for?");
        assert_eq!(defs.tech(works).school, Some(school));
        assert_eq!(defs.school_mates(sun).collect::<Vec<_>>(), [works]);
        assert_eq!(defs.school_mates(works).collect::<Vec<_>>(), [sun]);
        assert_eq!(defs.school_techs(Nation::Fire), [sun, works]);
        assert!(defs.school_techs(Nation::Air).is_empty());
        let forging = defs.tech_id("forging").unwrap();
        assert_eq!(defs.school_mates(forging).count(), 0);
    }

    #[test]
    fn school_validation_errors() {
        let one = |extra: &str| {
            with_research(&format!(
                "{FIRE_SCHOOL}{}{extra}",
                school_tech("sun", "fire", "fire_purpose", "")
            ))
        };
        assert_err(one(""), "has 1 techs");
        assert_err(
            fire_fork(&school_tech("third", "fire", "fire_purpose", "")),
            "has 3 techs, and a school has exactly 2",
        );
        assert_err(with_research(FIRE_SCHOOL), "no tech names it");
        assert_err(one(&school_tech("x", "fire", "nowhere", "")), "unknown school \"nowhere\"");
        // A tech of another nation, or a shared one, in fire's school.
        let water = "[[tech]]\nid = \"w\"\nname = \"w\"\ndescription = \"\"\nnation = \"water\"\n\
                     cost = 1000\nschool = \"fire_purpose\"\n";
        assert_err(one(water), "school \"fire_purpose\" is fire's, and this is a water tech");
        let shared = "[[tech]]\nid = \"s\"\nname = \"s\"\ndescription = \"\"\ncost = 1000\n\
                      school = \"fire_purpose\"\n";
        assert_err(one(shared), "this is a shared tech");
        assert_err(
            one(&school_tech("core_one", "fire", "fire_purpose", "core = true")),
            "a school tech can't be `core`",
        );
        let second = "[[school]]\nid = \"again\"\nnation = \"fire\"\nquestion = \"?\"\n";
        assert_err(fire_fork(second), "fire already has a school (fire_purpose)");
        let dup = "[[school]]\nid = \"fire_purpose\"\nnation = \"air\"\nquestion = \"?\"\n";
        assert_err(fire_fork(dup), "school fire_purpose: duplicate id");
        let needs_sun = "[[tech]]\nid = \"after\"\nname = \"a\"\ndescription = \"\"\n\
                         nation = \"fire\"\ncost = 10\nprereqs = [\"sun\"]\n";
        assert_err(
            fire_fork(needs_sun),
            "\"sun\" is a school tech, which no other tech can require",
        );
        let unknown = "[[school]]\nid = \"x\"\nnation = \"air\"\nquestion = \"?\"\ncolour = 1\n";
        assert_err(fire_fork(unknown), "colour");
    }

    #[test]
    fn no_nation_starts_with_a_school_tech() {
        let mut files = crate::defs::BUNDLED_FILES;
        let research = format!(
            "{}\n{FIRE_SCHOOL}{}{}",
            files[3],
            school_tech("sun", "fire", "fire_purpose", "").replace("prereqs = [\"forging\"]\n", ""),
            school_tech("works", "fire", "fire_purpose", "")
        );
        files[3] = &research;
        let nations = files[2].replacen(
            "starting_techs = [\"fire_making\"]",
            "starting_techs = [\"fire_making\", \"sun\"]",
            1,
        );
        assert_ne!(nations, files[2], "the fixture found fire's starting techs");
        files[2] = &nations;
        assert_err(
            Defs::from_files(files),
            "\"sun\" is a school tech, which no nation starts with",
        );
    }

    #[test]
    fn learning_one_option_closes_the_other_and_switching_is_free_until_then() {
        let defs = fire_fork("").unwrap();
        let (sun, works) = (defs.tech_id("sun").unwrap(), defs.tech_id("works").unwrap());
        let facts = Facts::default();
        let mut r = Research::new(Nation::Fire, &defs);
        assert_eq!(r.closed_by(sun, &defs), None);
        r.set_target(sun, Nation::Fire, &facts, &defs).unwrap();
        // Queuing closes nothing: the player may switch.
        assert_eq!(r.closed_by(works, &defs), None);
        r.set_target(works, Nation::Fire, &facts, &defs).unwrap();
        assert_eq!(r.queue().last(), Some(&works));
        r.set_target(sun, Nation::Fire, &facts, &defs).unwrap();
        r.grant(sun);
        assert_eq!(r.closed_by(works, &defs), Some(sun));
        assert_eq!(r.closed_by(sun, &defs), None, "a known tech isn't closed");
        let queue = r.queue().to_vec();
        let e = r.set_target(works, Nation::Fire, &facts, &defs).unwrap_err();
        assert_eq!(e, ResearchError::Closed { chosen: sun });
        assert_eq!(e.to_string(), "the other path of this school was chosen");
        assert_eq!(r.queue(), queue, "a refusal leaves the queue alone");
    }
}
