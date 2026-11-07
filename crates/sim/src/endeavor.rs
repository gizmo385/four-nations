//! The end-game endeavor's signs and stages: definitions from `data/endeavor.toml`, resolved and
//! validated at load (`docs/design/endgame.md`, "The road: ten signs" and "The project: four
//! stages"; issues #234 and #236).
//!
//! A sign is a condition on things the settlement does anyway: how many people it has, what it
//! has built and stored, lifetime tallies. The endeavor becomes Ready the first time every sign is
//! met at once (`world/endeavor.rs` holds that live state). Sign kinds are a closed enum, so a sign
//! the sim can't count can't be written: a sign whose system doesn't exist yet stays out of the
//! data until it does, the way `research_earth.rs` treats unlocks.
//!
//! Signs are referenced by their string id in saves (`World::endeavor_signs` and the events), so
//! reordering or retuning the list doesn't misread an old save.
//!
//! Once Ready, the settlement builds the gate (`EndeavorDef::building`) in stages, in file order.
//! A stage has its own cost and work, done by a settler as a construct job, and tasks: each task
//! needs its own materials hauled to the site, then one bend by a unit allowed to do it
//! (`TaskWho`: a nation's own benders, or the Avatar, who may do any of them), which spends chi
//! like an ability. Stages and tasks are saved by string id too (`world/gate.rs`).
//!
//! `[avatar]` (`docs/design/endgame.md`, "The Avatar"; issue #237) describes Aang: his own chi
//! and work modifiers (the best of the four nations'), the four signature abilities he may cast,
//! the foods he refuses, and the timings of his arrival, patience and return. The sim never
//! special-cases him by name; `world/avatar.rs` reads this data the same way `world/gate.rs` reads
//! a `[[stage]]`.

use serde::Deserialize;

use crate::bending::{AbilityDef, AbilityId, MasteryDef};
use crate::building::{BuildingDef, BuildingId, CostItem};
use crate::defs::DefsError;
use crate::jobs::{ItemDef, ItemId};
use crate::nation::{Modifiers, Nation};
use crate::research::TechDef;

/// The endeavor and its signs. Empty (no signs) when the data has no endeavor, as in
/// `Defs::from_toml` without `Defs::with_endeavor`: then the endeavor never becomes Ready.
#[derive(Debug, Clone, Default)]
pub struct EndeavorDef {
    /// What the endeavor is called in the UI ("The Harmony Gate").
    pub name: String,
    /// One line under the name on the Endeavor screen: what it is and what the signs are for.
    pub description: String,
    /// The signs, in file order: the order the UI lists them in.
    pub signs: Vec<SignDef>,
    /// The building the endeavor raises (`endeavor = true` in `data/buildings.toml`). Set exactly
    /// when there are stages.
    pub building: Option<BuildingId>,
    /// The stages of building it, in the order they are built. Empty when the data has none: then
    /// a Ready endeavor has no site to place.
    pub stages: Vec<StageDef>,
    /// Aang, the guest who appears once the foundation stage is done (`[avatar]`). `None` for
    /// data with no `[avatar]` section (most tests): then he never arrives.
    pub avatar: Option<AvatarDef>,
    /// A copy of `data/bending.toml`'s `[mastery]` master-bends numbers, kept only if a `masters`
    /// sign is actually in `signs`, so `master_threshold` doesn't need the whole bending data at
    /// every call (#360): the numbers themselves live in, and are validated by,
    /// `MasteryDef`/`bending.toml`; this is just the "is there a masters sign at all" gate the
    /// per-bend alert line (`fn_ui::alerts`) still needs.
    master_bends: Option<MasterBends>,
}

/// See `EndeavorDef::master_bends`.
#[derive(Debug, Clone, PartialEq)]
struct MasterBends {
    bends: u32,
    per_nation: [Option<u32>; 4],
}

impl EndeavorDef {
    /// The stage with this id, with its place in the list.
    pub fn stage(&self, id: &str) -> Option<(usize, &StageDef)> {
        self.stages.iter().enumerate().find(|(_, s)| s.id == id)
    }

    /// How many finished bends make a unit of `nation` a bending master
    /// (`data/bending.toml`'s `[mastery]`, #360), or `None` when the data has no `masters` sign
    /// at all. Every frontend reads this plain bends threshold through this one helper, never a
    /// sign's own field directly; `World::rank_of` is what actually judges mastery once forms
    /// are counted too.
    pub fn master_threshold(&self, nation: Nation) -> Option<u32> {
        self.master_bends.as_ref().map(|m| masters_threshold_for(m.bends, &m.per_nation, nation))
    }
}

/// A nation's own bends threshold: `bends_per_nation`'s override, or the flat `bends`. Shared by
/// `EndeavorDef::master_threshold` and anything else that reads a per-nation override the same
/// way `SignDef::target_for` reads `per_nation`.
fn masters_threshold_for(bends: u32, bends_per_nation: &[Option<u32>; 4], nation: Nation) -> u32 {
    let i = Nation::ALL.iter().position(|&n| n == nation).expect("every nation is in ALL");
    bends_per_nation[i].unwrap_or(bends)
}

/// One stage of building the gate (`[[stage]]`).
#[derive(Debug, Clone, PartialEq)]
pub struct StageDef {
    pub id: String,
    pub name: String,
    /// What a settler hauls to the site before the stage's work can start.
    pub cost: Vec<CostItem>,
    /// In-game seconds of construct work at a `construction` modifier of 1.0, done once `cost` is
    /// delivered. 0 when the stage is only tasks.
    pub work: f32,
    /// Bends that finish the stage, each once its own cost is delivered and the work is done.
    pub tasks: Vec<TaskDef>,
    /// The Harmony Rite's own numbers (`[stage.rite]`), set only on the stage that *is* the rite
    /// (`docs/design/endgame.md`, "The project: four stages"; issue #238). `None` for every other
    /// stage, including a data file that never defines one: then the gate finishes at the Arch.
    /// A rite stage has no `cost`, `work` or `tasks` of its own; its own module (`world/gate.rs`)
    /// tracks its progress differently, by attendance rather than hauling and channelling.
    pub rite: Option<RiteDef>,
}

/// The Harmony Rite's own numbers (`[stage.rite]`; issue #238): how long it takes and how much
/// of the settlement must attend for it to move at all.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RiteDef {
    /// Minutes of attendance needed to finish (the design's "10 minutes").
    pub minutes: f32,
    /// The share of the living population (the Avatar never counts) that must be on the plaza
    /// for progress to accrue, in `(0, 1]` (the design's "half the settlers").
    pub attendance_share: f32,
}

/// A bend that is part of a stage (`[[stage.task]]`): a pillar raised by its element, or an
/// "Avatar task" only he can claim (the Arch's beam lift; the rite, #238).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaskWho {
    /// A unit of this nation (home settler or not, #239) may do it. The Avatar may do it too.
    Nation(Nation),
    /// Only the Avatar may do it.
    Avatar,
}

/// A bend that is part of a stage (`[[stage.task]]`): a pillar raised by its element.
#[derive(Debug, Clone, PartialEq)]
pub struct TaskDef {
    pub id: String,
    pub name: String,
    /// Who may do it.
    pub element: TaskWho,
    /// What a settler hauls to the site before the task can be taken.
    pub cost: Vec<CostItem>,
    /// Chi the bend needs to start, and spends when it finishes.
    pub chi: f32,
    /// In-game seconds of channelling.
    pub work: f32,
}

/// Aang, the guest who appears once the foundation stage is done (`[avatar]`;
/// `docs/design/endgame.md`, "The Avatar"). The sim never special-cases him by name: everything
/// that treats him differently from a settler (`World::tick_avatar` and friends, `world/avatar.rs`)
/// reads this data.
#[derive(Debug, Clone, PartialEq)]
pub struct AvatarDef {
    /// A character id, for a frontend to pick his art (`NationDef::characters` is the pattern);
    /// the sim never interprets it.
    pub id: String,
    pub name: String,
    pub character: String,
    /// A full chi meter, which may be more than `ChiSettings::max` (every nation's own cap).
    pub chi_max: f32,
    /// His own work modifiers: the best of the four nations' (`docs/design/endgame.md`,
    /// "Numbers"). `modifiers.chi_regen` is his regen multiplier.
    pub modifiers: Modifiers,
    /// The signature abilities he may cast: any living unit's own nation would also let them
    /// cast one of these, but he is never restricted to one nation (`World::unit_can_bend`).
    pub abilities: Vec<AbilityId>,
    /// Foods he won't eat raw or cooked ("a canon touch and a real constraint for a fish-fed
    /// Water settlement").
    pub refuses: Vec<ItemId>,
    /// How long his hunger or energy may stay at or below `low` before he leaves.
    pub patience_minutes: f32,
    /// How long he stays away once he has left.
    pub return_minutes: f32,
    /// How long after the foundation stage is done before he arrives.
    pub arrive_after_minutes: f32,
    /// How long he stays for the celebration once the Harmony Rite completes, before he flies
    /// off for good (`docs/design/endgame.md`, "How long he stays": "one in-game day"; #238).
    pub farewell_minutes: f32,
}

/// Where a sign is listed. The UI groups signs under these, in this order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SignGroup {
    People,
    Buildings,
    Knowledge,
    Stores,
    Harvests,
    Bending,
}

/// What a sign counts.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SignKind {
    /// Living units.
    Population,
    /// Beds, against the population: met when there is a bed for everyone. Its target is the
    /// population, so it has none in the data.
    BedsForAll,
    /// Non-endeavor buildings this nation can ever unlock (`reachable_buildings`): no unlocking
    /// tech, or one unlocked by a shared tech or its own, minus `spare`. The target is computed
    /// per nation, the way `BranchComplete`'s is, so it has none in the data
    /// (`docs/design/materials.md`, section 6; #272, #354).
    EveryBuilding { spare: u32 },
    /// Techs of the settlement's own nation known, against all of them. Its target is the size of
    /// the branch, so it has none in the data.
    BranchComplete,
    /// These items in stockpiles, summed.
    Stored { items: Vec<ItemId> },
    /// A lifetime tally (`World::tallies`).
    Tally(Tally),
    /// Units at the bending master rank, against their own nation's thresholds
    /// (`World::rank_of`, `data/bending.toml`'s `[mastery]`, #360).
    Masters,
}

/// Which lifetime tally a `SignKind::Tally` reads. Crops harvested from farm plots joins this
/// list with the farm plots themselves (#198).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Tally {
    /// Items harvested, summed over these items.
    Harvested(Vec<ItemId>),
    ScrollsRead,
}

/// Non-endeavor buildings `nation` can ever unlock: no unlocking tech, or one unlocked by a
/// shared tech (`TechDef::nation` is `None`) or by the nation's own
/// (`SignKind::EveryBuilding`, `docs/design/materials.md` section 6, #354). Reads only `defs`,
/// not any particular settlement, so it adapts automatically as buildings are added to the data
/// (#357 and friends) rather than going stale the way a hand-set `per_nation` count would.
/// `techs` and `buildings` are `Defs::techs` and `Defs::buildings`; `buildings[i].unlocked_by`
/// indexes into `techs`.
pub fn reachable_buildings<'a>(
    nation: Nation,
    techs: &'a [TechDef],
    buildings: &'a [BuildingDef],
) -> impl Iterator<Item = BuildingId> + 'a {
    buildings.iter().enumerate().filter_map(move |(i, b)| {
        if b.endeavor {
            return None;
        }
        let reachable = match b.unlocked_by {
            None => true,
            Some(t) => match techs[t.0 as usize].nation {
                None => true,
                Some(n) => n == nation,
            },
        };
        reachable.then_some(BuildingId(i as u16))
    })
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SignDef {
    pub id: String,
    pub group: SignGroup,
    pub name: String,
    pub kind: SignKind,
    /// A short noun for a count ("people", "scrolls"), the side panel's compact Gate row (#416,
    /// docs/design/side-panel-v2.md "Zone 1": `Gate 8/9 · 3 scrolls [G]`), paired with how many
    /// are still missing.
    pub short: String,
    /// What the UI suggests while the sign isn't met ("Spare beds and food draw newcomers").
    pub hint: Option<String>,
    /// The shared target, 0 for the kinds whose target the sim works out (`target_for`).
    target: u32,
    /// Per-nation overrides of `target`, indexed like `Nation::ALL`.
    per_nation: [Option<u32>; 4],
}

impl SignDef {
    /// The target `nation` must reach, or `None` for a kind whose target isn't in the data
    /// (`BedsForAll`, `BranchComplete`).
    pub fn target_for(&self, nation: Nation) -> Option<u32> {
        let i = Nation::ALL.iter().position(|&n| n == nation).expect("every nation is in ALL");
        (self.target > 0).then(|| self.per_nation[i].unwrap_or(self.target))
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct EndeavorFile {
    endeavor: RawEndeavor,
    #[serde(default)]
    sign: Vec<RawSign>,
    #[serde(default)]
    stage: Vec<RawStage>,
    #[serde(default)]
    avatar: Option<RawAvatar>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawEndeavor {
    name: String,
    #[serde(default)]
    description: String,
    #[serde(default)]
    building: Option<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawCost {
    item: String,
    count: u32,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawStage {
    id: String,
    name: String,
    #[serde(default)]
    cost: Vec<RawCost>,
    #[serde(default)]
    work: f32,
    #[serde(default)]
    task: Vec<RawTask>,
    /// Set only on the stage that is the Harmony Rite (`[stage.rite]`, #238).
    #[serde(default)]
    rite: Option<RawRite>,
}

/// `[stage.rite]`: the Harmony Rite's own numbers.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawRite {
    minutes: f32,
    attendance_share: f32,
}

impl RawRite {
    fn resolve(self, err: &dyn Fn(&str) -> DefsError) -> Result<RiteDef, DefsError> {
        if !(self.minutes.is_finite() && self.minutes > 0.0) {
            return Err(err("rite.minutes must be a positive number"));
        }
        if !(self.attendance_share.is_finite()
            && self.attendance_share > 0.0
            && self.attendance_share <= 1.0)
        {
            return Err(err("rite.attendance_share must be in (0, 1]"));
        }
        Ok(RiteDef { minutes: self.minutes, attendance_share: self.attendance_share })
    }
}

/// `[[stage.task]]`'s `element`: a nation, or `"avatar"` for a task only he can claim.
#[derive(Deserialize, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
enum RawElement {
    Air,
    Water,
    Earth,
    Fire,
    Avatar,
}

impl From<RawElement> for TaskWho {
    fn from(e: RawElement) -> Self {
        match e {
            RawElement::Air => TaskWho::Nation(Nation::Air),
            RawElement::Water => TaskWho::Nation(Nation::Water),
            RawElement::Earth => TaskWho::Nation(Nation::Earth),
            RawElement::Fire => TaskWho::Nation(Nation::Fire),
            RawElement::Avatar => TaskWho::Avatar,
        }
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawTask {
    id: String,
    name: String,
    element: RawElement,
    #[serde(default)]
    cost: Vec<RawCost>,
    chi: f32,
    work: f32,
}

#[derive(Deserialize, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
enum RawKind {
    Population,
    BedsForAll,
    EveryBuilding,
    BranchComplete,
    Stored,
    Tally,
    Masters,
}

#[derive(Deserialize, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
enum RawTally {
    Harvested,
    ScrollsRead,
}

#[derive(Deserialize, Default)]
#[serde(deny_unknown_fields)]
struct PerNation {
    air: Option<u32>,
    water: Option<u32>,
    earth: Option<u32>,
    fire: Option<u32>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawSign {
    id: String,
    group: SignGroup,
    name: String,
    kind: RawKind,
    /// The side panel's compact Gate row (#416): a short noun for a count ("people", "scrolls").
    short: String,
    #[serde(default)]
    target: Option<u32>,
    #[serde(default)]
    per_nation: Option<PerNation>,
    #[serde(default)]
    hint: Option<String>,
    /// `stored` and `tally = "harvested"`: item ids.
    #[serde(default)]
    items: Option<Vec<String>>,
    /// `stored`: also count every edible item.
    #[serde(default)]
    food: Option<bool>,
    /// `tally`: which tally.
    #[serde(default)]
    tally: Option<RawTally>,
    /// `every_building`: kinds the player may skip; defaults to 0.
    #[serde(default)]
    spare: Option<u32>,
}

impl EndeavorFile {
    /// Validates the file and resolves item, building, tech and ability ids. `items` is
    /// `Defs::items`, `buildings` `Defs::buildings`, `techs` `Defs::techs` and `abilities`
    /// `Defs::abilities`.
    pub(crate) fn resolve(
        self,
        items: &[ItemDef],
        buildings: &[BuildingDef],
        techs: &[TechDef],
        abilities: &[AbilityDef],
        chi_max: f32,
        mastery: &MasteryDef,
    ) -> Result<EndeavorDef, DefsError> {
        if self.endeavor.name.trim().is_empty() {
            return Err(DefsError("endeavor: name must not be empty".into()));
        }
        let mut signs: Vec<SignDef> = Vec::with_capacity(self.sign.len());
        for raw in self.sign {
            let sign = raw.resolve(items)?;
            if signs.iter().any(|s| s.id == sign.id) {
                return Err(DefsError(format!("sign {}: duplicate id", sign.id)));
            }
            if let SignKind::EveryBuilding { spare } = sign.kind {
                // Every nation's reachable count, so `spare` can never zero out even the
                // smallest nation's target (`docs/design/materials.md` section 6; #354).
                let smallest = Nation::ALL
                    .iter()
                    .map(|&n| reachable_buildings(n, techs, buildings).count() as u32)
                    .min()
                    .expect("Nation::ALL is non-empty");
                if spare >= smallest {
                    return Err(DefsError(format!(
                        "sign {}: spare must be below the smallest nation's count ({smallest})",
                        sign.id
                    )));
                }
            }
            signs.push(sign);
        }
        let mut stages: Vec<StageDef> = Vec::with_capacity(self.stage.len());
        for raw in self.stage {
            let stage = raw.resolve(items, chi_max)?;
            if stages.iter().any(|s| s.id == stage.id) {
                return Err(DefsError(format!("stage {}: duplicate id", stage.id)));
            }
            let tasks = stages.iter().flat_map(|s| &s.tasks).chain(&stage.tasks);
            let mut ids: Vec<&str> = tasks.map(|t| t.id.as_str()).collect();
            ids.sort_unstable();
            if let Some(w) = ids.windows(2).find(|w| w[0] == w[1]) {
                return Err(DefsError(format!("task {}: duplicate id", w[0])));
            }
            stages.push(stage);
        }
        let rite_stages = stages.iter().filter(|s| s.rite.is_some()).count();
        if rite_stages > 1 {
            return Err(DefsError("endeavor: only one stage may be the Harmony Rite".into()));
        }
        if rite_stages == 1 && !stages.last().is_some_and(|s| s.rite.is_some()) {
            return Err(DefsError("endeavor: the Harmony Rite must be the last stage".into()));
        }
        let RawEndeavor { name, description, building } = self.endeavor;
        let building = match (building, stages.is_empty()) {
            (None, true) => None,
            (None, false) => {
                return Err(DefsError("endeavor: stages need a building to raise".into()));
            }
            (Some(_), true) => {
                return Err(DefsError("endeavor: a building needs at least one [[stage]]".into()));
            }
            (Some(b), false) => {
                let Some(n) = buildings.iter().position(|d| d.id == b) else {
                    return Err(DefsError(format!("endeavor: unknown building {b:?}")));
                };
                if !buildings[n].endeavor {
                    return Err(DefsError(format!(
                        "endeavor: building {b:?} needs endeavor = true in buildings.toml"
                    )));
                }
                Some(BuildingId(n as u16))
            }
        };
        let avatar = self.avatar.map(|a| a.resolve(items, abilities)).transpose()?;
        let needs_avatar = rite_stages > 0
            || stages.iter().flat_map(|s| &s.tasks).any(|t| matches!(t.element, TaskWho::Avatar));
        if needs_avatar && avatar.is_none() {
            return Err(DefsError(
                "endeavor: a task for the Avatar, or the Harmony Rite, needs an [avatar] section"
                    .into(),
            ));
        }
        let master_bends =
            signs.iter().any(|s| s.kind == SignKind::Masters).then_some(MasterBends {
                bends: mastery.master_bends,
                per_nation: mastery.master_bends_per_nation,
            });
        Ok(EndeavorDef { name, description, signs, building, stages, avatar, master_bends })
    }
}

/// Links a cost list's item ids: each item known, counted at least once, and listed once.
fn resolve_cost(
    raw: Vec<RawCost>,
    items: &[ItemDef],
    err: &dyn Fn(&str) -> DefsError,
) -> Result<Vec<CostItem>, DefsError> {
    let mut cost: Vec<CostItem> = Vec::with_capacity(raw.len());
    for c in raw {
        let Some(n) = items.iter().position(|i| i.id == c.item) else {
            return Err(err(&format!("unknown item {:?}", c.item)));
        };
        let item = ItemId(n as u16);
        if c.count == 0 {
            return Err(err("cost count must be positive"));
        }
        if cost.iter().any(|existing| existing.item == item) {
            return Err(err(&format!("item {:?} costed twice", c.item)));
        }
        cost.push(CostItem { item, count: c.count });
    }
    Ok(cost)
}

impl RawStage {
    fn resolve(self, items: &[ItemDef], chi_max: f32) -> Result<StageDef, DefsError> {
        let id = self.id;
        let err = |msg: &str| DefsError(format!("stage {id}: {msg}"));
        if id.trim().is_empty() {
            return Err(DefsError("stage: id must not be empty".into()));
        }
        if self.name.trim().is_empty() {
            return Err(err("name must not be empty"));
        }
        if !(self.work.is_finite() && self.work >= 0.0) {
            return Err(err("work must be a number of seconds, 0 or more"));
        }
        let cost = resolve_cost(self.cost, items, &err)?;
        let rite = self.rite.map(|r| r.resolve(&err)).transpose()?;
        if rite.is_some() {
            // The rite's own progress is attendance, not hauling or channelling (`world/gate.rs`,
            // #238): its stage carries none of the usual cost, work or tasks.
            if !cost.is_empty() || self.work != 0.0 || !self.task.is_empty() {
                return Err(err("the Harmony Rite has no cost, work or task of its own"));
            }
        } else {
            if !cost.is_empty() && self.work == 0.0 {
                return Err(err("a stage with a cost needs work to spend it on"));
            }
            if self.work == 0.0 && self.task.is_empty() {
                return Err(err("needs work or at least one task"));
            }
        }
        // Jobs name a task by its place in the stage, in a byte.
        if self.task.len() > usize::from(u8::MAX) {
            return Err(err("too many tasks"));
        }
        let tasks =
            self.task.into_iter().map(|t| t.resolve(items, chi_max)).collect::<Result<_, _>>()?;
        Ok(StageDef { name: self.name, cost, work: self.work, tasks, rite, id })
    }
}

impl RawTask {
    fn resolve(self, items: &[ItemDef], chi_max: f32) -> Result<TaskDef, DefsError> {
        let id = self.id;
        let err = |msg: &str| DefsError(format!("task {id}: {msg}"));
        if id.trim().is_empty() {
            return Err(DefsError("task: id must not be empty".into()));
        }
        if self.name.trim().is_empty() {
            return Err(err("name must not be empty"));
        }
        // A nation's own meter never holds more than `chi.max`, so a dearer task could never be
        // taken by its own benders (the Avatar's own meter is bigger, but this bound still holds
        // for him too, just more loosely).
        if !(self.chi > 0.0 && self.chi <= chi_max) {
            return Err(err(&format!("chi must be in (0, {chi_max}] (chi.max)")));
        }
        if !(self.work.is_finite() && self.work > 0.0) {
            return Err(err("work must be a positive number of seconds"));
        }
        let cost = resolve_cost(self.cost, items, &err)?;
        Ok(TaskDef {
            name: self.name,
            element: self.element.into(),
            cost,
            chi: self.chi,
            work: self.work,
            id,
        })
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawAvatar {
    id: String,
    name: String,
    character: String,
    chi_max: f32,
    modifiers: Modifiers,
    abilities: Vec<String>,
    #[serde(default)]
    refuses: Vec<String>,
    patience_minutes: f32,
    return_minutes: f32,
    arrive_after_minutes: f32,
    farewell_minutes: f32,
}

impl RawAvatar {
    fn resolve(self, items: &[ItemDef], abilities: &[AbilityDef]) -> Result<AvatarDef, DefsError> {
        let err = |msg: &str| DefsError(format!("avatar: {msg}"));
        if self.id.trim().is_empty() {
            return Err(err("id must not be empty"));
        }
        if self.name.trim().is_empty() {
            return Err(err("name must not be empty"));
        }
        if self.character.trim().is_empty() {
            return Err(err("character must not be empty"));
        }
        if !(self.chi_max.is_finite() && self.chi_max > 0.0) {
            return Err(err("chi_max must be a positive number"));
        }
        for (what, v) in [
            ("patience_minutes", self.patience_minutes),
            ("return_minutes", self.return_minutes),
            ("arrive_after_minutes", self.arrive_after_minutes),
            ("farewell_minutes", self.farewell_minutes),
        ] {
            if !(v.is_finite() && v > 0.0) {
                return Err(err(&format!("{what} must be a positive number")));
            }
        }
        if self.abilities.is_empty() {
            return Err(err("needs at least one ability"));
        }
        let mut resolved_abilities = Vec::with_capacity(self.abilities.len());
        for a in &self.abilities {
            let Some(n) = abilities.iter().position(|d| d.id == *a) else {
                return Err(err(&format!("unknown ability {a:?}")));
            };
            let id = AbilityId(n as u16);
            if resolved_abilities.contains(&id) {
                return Err(err(&format!("ability {a:?} listed twice")));
            }
            resolved_abilities.push(id);
        }
        let mut refuses = Vec::with_capacity(self.refuses.len());
        for r in &self.refuses {
            let Some(n) = items.iter().position(|i| i.id == *r) else {
                return Err(err(&format!("unknown item {r:?}")));
            };
            if items[n].food.is_none() {
                return Err(err(&format!("{r:?} isn't a food")));
            }
            let id = ItemId(n as u16);
            if refuses.contains(&id) {
                return Err(err(&format!("{r:?} listed twice")));
            }
            refuses.push(id);
        }
        Ok(AvatarDef {
            id: self.id,
            name: self.name,
            character: self.character,
            chi_max: self.chi_max,
            modifiers: self.modifiers,
            abilities: resolved_abilities,
            refuses,
            patience_minutes: self.patience_minutes,
            return_minutes: self.return_minutes,
            arrive_after_minutes: self.arrive_after_minutes,
            farewell_minutes: self.farewell_minutes,
        })
    }
}

impl RawSign {
    fn resolve(self, items: &[ItemDef]) -> Result<SignDef, DefsError> {
        let id = self.id;
        let err = |msg: &str| DefsError(format!("sign {id}: {msg}"));
        if id.trim().is_empty() {
            return Err(DefsError("sign: id must not be empty".into()));
        }
        if self.name.trim().is_empty() {
            return Err(err("name must not be empty"));
        }
        if self.short.trim().is_empty() {
            return Err(err("short must not be empty"));
        }
        // Each optional field belongs to some kinds only; anywhere else it is a mistake.
        let allowed = |field: bool, ok: bool, name: &str| {
            if field && !ok {
                Err(err(&format!("{name} doesn't apply to this kind")))
            } else {
                Ok(())
            }
        };
        let k = self.kind;
        let computed =
            matches!(k, RawKind::BedsForAll | RawKind::BranchComplete | RawKind::EveryBuilding);
        allowed(self.target.is_some(), !computed, "target")?;
        allowed(self.per_nation.is_some(), !computed, "per_nation")?;
        allowed(self.food.is_some(), k == RawKind::Stored, "food")?;
        allowed(self.tally.is_some(), k == RawKind::Tally, "tally")?;
        allowed(self.spare.is_some(), k == RawKind::EveryBuilding, "spare")?;
        let takes_items = k == RawKind::Stored
            || (k == RawKind::Tally && self.tally == Some(RawTally::Harvested));
        allowed(self.items.is_some(), takes_items, "items")?;

        let target = match self.target {
            Some(0) => return Err(err("target must be at least 1")),
            Some(t) => t,
            None if computed => 0,
            None => return Err(err("needs a target")),
        };
        let p = self.per_nation.unwrap_or_default();
        let per_nation = [p.air, p.water, p.earth, p.fire];
        if per_nation.contains(&Some(0)) {
            return Err(err("per_nation targets must be at least 1"));
        }

        let resolve_items = |ids: &[String]| -> Result<Vec<ItemId>, DefsError> {
            ids.iter()
                .map(|i| {
                    items
                        .iter()
                        .position(|d| d.id == *i)
                        .map(|n| ItemId(n as u16))
                        .ok_or_else(|| err(&format!("unknown item {i:?}")))
                })
                .collect()
        };
        let mut listed = resolve_items(self.items.as_deref().unwrap_or_default())?;
        let kind = match k {
            RawKind::Population => SignKind::Population,
            RawKind::BedsForAll => SignKind::BedsForAll,
            RawKind::EveryBuilding => SignKind::EveryBuilding { spare: self.spare.unwrap_or(0) },
            RawKind::BranchComplete => SignKind::BranchComplete,
            RawKind::Stored => {
                if self.food == Some(true) {
                    let food = items.iter().enumerate().filter(|(_, d)| d.food.is_some());
                    listed.extend(food.map(|(n, _)| ItemId(n as u16)));
                }
                listed.sort();
                listed.dedup();
                if listed.is_empty() {
                    return Err(err("stored needs items or food = true"));
                }
                SignKind::Stored { items: listed }
            }
            RawKind::Tally => match self.tally {
                None => return Err(err("tally needs tally = \"harvested\" or \"scrolls_read\"")),
                Some(RawTally::ScrollsRead) => SignKind::Tally(Tally::ScrollsRead),
                Some(RawTally::Harvested) => {
                    listed.sort();
                    listed.dedup();
                    if listed.is_empty() {
                        return Err(err("a harvested tally needs items"));
                    }
                    SignKind::Tally(Tally::Harvested(listed))
                }
            },
            RawKind::Masters => SignKind::Masters,
        };
        Ok(SignDef {
            group: self.group,
            name: self.name,
            kind,
            short: self.short,
            hint: self.hint.filter(|h| !h.trim().is_empty()),
            target,
            per_nation,
            id,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::defs::Defs;

    const HEADER: &str = "[endeavor]\nname = \"The Harmony Gate\"\n";

    /// The bundled data with `endeavor.toml` replaced by `src`.
    fn load_file(src: &str) -> Result<EndeavorDef, DefsError> {
        Ok(Defs::bundled().with_endeavor(src)?.endeavor)
    }

    /// The bundled data with these signs as the whole list.
    fn load(signs: &str) -> Result<EndeavorDef, DefsError> {
        load_file(&format!("{HEADER}{signs}"))
    }

    fn error(signs: &str) -> String {
        load(signs).expect_err("should be rejected").0
    }

    fn file_error(src: &str) -> String {
        load_file(src).expect_err("should be rejected").0
    }

    #[test]
    fn the_bundled_endeavor_loads_with_every_kind_it_can_count() {
        let e = Defs::bundled().endeavor;
        assert_eq!(e.name, "The Harmony Gate");
        assert!(e.description.contains("Avatar"));
        assert!(!e.signs.is_empty());
        let has = |f: fn(&SignKind) -> bool| e.signs.iter().any(|s| f(&s.kind));
        assert!(has(|k| *k == SignKind::Population));
        assert!(has(|k| *k == SignKind::BedsForAll));
        assert!(has(|k| *k == SignKind::BranchComplete));
        assert!(has(|k| matches!(k, SignKind::EveryBuilding { .. })));
        assert!(has(|k| matches!(k, SignKind::Stored { .. })));
        assert!(has(|k| matches!(k, SignKind::Tally(Tally::Harvested(_)))));
        assert!(has(|k| matches!(k, SignKind::Tally(Tally::ScrollsRead))));
        assert!(has(|k| *k == SignKind::Masters));
    }

    #[test]
    fn a_sign_resolves_items_targets_and_per_nation_overrides() {
        let e = load(
            r#"
            [[sign]]
            id = "stone"
            group = "stores"
            name = "Stone in stores"
            kind = "stored"
            short = "stone"
            items = ["stone"]
            target = 200
            per_nation = { air = 150 }
            hint = "Quarry stone"

            [[sign]]
            id = "beds"
            group = "people"
            name = "Everyone has a bed"
            kind = "beds_for_all"
            short = "beds"
            "#,
        )
        .unwrap();
        let defs = Defs::bundled();
        let stone = &e.signs[0];
        assert_eq!(stone.kind, SignKind::Stored { items: vec![defs.item_id("stone").unwrap()] });
        assert_eq!(stone.target_for(Nation::Air), Some(150));
        assert_eq!(stone.target_for(Nation::Earth), Some(200));
        assert_eq!(stone.hint.as_deref(), Some("Quarry stone"));
        assert_eq!(e.signs[1].target_for(Nation::Fire), None);
    }

    /// `master_threshold` reads `data/bending.toml`'s bundled `[mastery]` numbers for a `masters`
    /// sign with no bends fields of its own (#360), and `None` when the data has no such sign at
    /// all (`docs/design/endgame.md`, "Mastery pacing").
    #[test]
    fn master_threshold_reads_the_bundled_mastery_or_none_without_a_masters_sign() {
        let e = load(
            "[[sign]]\nid = \"masters\"\ngroup = \"bending\"\nname = \"M\"\nshort = \"m\"\n\
             kind = \"masters\"\ntarget = 3\n",
        )
        .unwrap();
        assert_eq!(e.master_threshold(Nation::Fire), Some(10));
        assert_eq!(e.master_threshold(Nation::Earth), Some(8));
        assert_eq!(e.master_threshold(Nation::Water), Some(8));
        assert_eq!(e.master_threshold(Nation::Air), Some(16));
        assert_eq!(load("").unwrap().master_threshold(Nation::Fire), None);
    }

    #[test]
    fn food_true_stores_every_edible_item() {
        let e = load(
            "[[sign]]\nid = \"f\"\ngroup = \"stores\"\nname = \"F\"\nshort = \"f\"\n\
             kind = \"stored\"\nfood = true\ntarget = 5\n",
        )
        .unwrap();
        let defs = Defs::bundled();
        let SignKind::Stored { items } = &e.signs[0].kind else { panic!() };
        let food: Vec<ItemId> = (0..defs.items.len())
            .map(|i| ItemId(i as u16))
            .filter(|&i| defs.food_of(i).is_some())
            .collect();
        assert!(!food.is_empty());
        assert_eq!(items, &food);
    }

    #[test]
    fn an_empty_or_missing_list_of_signs_is_allowed() {
        assert!(load("").unwrap().signs.is_empty());
        // Data built without an endeavor file has no signs at all.
        assert!(crate::bending::testing::probe_defs(1.0, "").endeavor.signs.is_empty());
    }

    fn sign(fields: &str) -> String {
        format!("[[sign]]\nid = \"s\"\ngroup = \"people\"\nname = \"S\"\nshort = \"s\"\n{fields}\n")
    }

    #[test]
    fn unknown_keys_and_kinds_are_rejected() {
        assert!(error(&sign("kind = \"population\"\ntarget = 3\ncolour = 1")).contains("colour"));
        assert!(error(&sign("kind = \"crops\"\ntarget = 3")).contains("crops"));
        assert!(
            error(&sign("kind = \"tally\"\ntally = \"crops_harvested\"\ntarget = 3"))
                .contains("crops_harvested")
        );
        assert!(file_error("[endeavor]\nname = \"G\"\nextra = 1\n").contains("extra"));
        assert!(file_error("[endeavor]\nname = \"G\"\n[[step]]\nid = \"x\"\n").contains("step"));
        assert!(
            error("[[sign]]\nid = \"s\"\ngroup = \"war\"\nname = \"S\"\nshort = \"s\"\nkind = \"population\"\ntarget = 1\n")
                .contains("war")
        );
    }

    #[test]
    fn unresolved_item_ids_are_rejected() {
        let e = error(&sign("kind = \"stored\"\nitems = [\"mithril\"]\ntarget = 3"));
        assert!(e.contains("unknown item \"mithril\""), "{e}");
        let e =
            error(&sign("kind = \"tally\"\ntally = \"harvested\"\nitems = [\"x\"]\ntarget = 3"));
        assert!(e.contains("unknown item \"x\""), "{e}");
    }

    #[test]
    fn targets_are_required_positive_and_only_where_the_data_sets_them() {
        assert!(error(&sign("kind = \"population\"")).contains("needs a target"));
        assert!(error(&sign("kind = \"population\"\ntarget = 0")).contains("at least 1"));
        assert!(
            error(&sign("kind = \"beds_for_all\"\ntarget = 4")).contains("target doesn't apply")
        );
        assert!(
            error(&sign("kind = \"branch_complete\"\nper_nation = { air = 1 }"))
                .contains("per_nation")
        );
        let e = error(&sign("kind = \"population\"\ntarget = 4\nper_nation = { fire = 0 }"));
        assert!(e.contains("per_nation targets must be at least 1"), "{e}");
        assert!(
            error(&sign("kind = \"population\"\ntarget = 4\nper_nation = { mars = 2 }"))
                .contains("mars")
        );
    }

    #[test]
    fn kind_specific_fields_are_checked() {
        assert!(error(&sign("kind = \"stored\"\ntarget = 3")).contains("items or food"));
        assert!(
            error(&sign("kind = \"population\"\ntarget = 3\nitems = [\"stone\"]"))
                .contains("items doesn't apply")
        );
        assert!(
            error(&sign("kind = \"population\"\ntarget = 3\nfood = true"))
                .contains("food doesn't apply")
        );
        assert!(error(&sign("kind = \"tally\"\ntarget = 3")).contains("tally needs"));
        assert!(
            error(&sign("kind = \"tally\"\ntally = \"harvested\"\ntarget = 3"))
                .contains("needs items")
        );
        let e = error(&sign(
            "kind = \"tally\"\ntally = \"scrolls_read\"\nitems = [\"stone\"]\ntarget = 3",
        ));
        assert!(e.contains("items doesn't apply"), "{e}");
        // `bends`/`bends_per_nation` moved to `data/bending.toml`'s `[mastery]` (#360): a sign
        // naming either, on any kind, is simply an unknown field now.
        assert!(error(&sign("kind = \"masters\"\ntarget = 3\nbends = 10")).contains("bends"));
        assert!(load(&sign("kind = \"masters\"\ntarget = 3")).is_ok(), "masters needs no fields");
        assert!(
            error(&sign("kind = \"population\"\ntarget = 3\ntally = \"scrolls_read\""))
                .contains("tally doesn't apply")
        );
        assert!(
            error(&sign("kind = \"population\"\ntarget = 3\nspare = 1"))
                .contains("spare doesn't apply")
        );
    }

    /// The bundled data's reachable count, per nation, before `spare` (today: Air 11, Water 11,
    /// Earth 11, Fire 12; `docs/design/materials.md` section 6, raised by the shared Workshop
    /// (#369) and Map Room (#402), the Mine Shaft and Deep Mine for Earth (#373), the Healing
    /// Hut and Fishing Dock for Water (#375, #374), and the shared Herb Garden (#390)).
    fn reachable_counts() -> [(Nation, u32); 4] {
        let defs = Defs::bundled();
        Nation::ALL
            .map(|n| (n, reachable_buildings(n, &defs.techs, &defs.buildings).count() as u32))
    }

    #[test]
    fn every_building_is_computed_per_nation_with_no_target_in_the_data() {
        let e = load(
            "[[sign]]\nid = \"b\"\ngroup = \"buildings\"\nname = \"B\"\nshort = \"b\"\n\
             kind = \"every_building\"\nspare = 1\n",
        )
        .unwrap();
        assert_eq!(e.signs[0].kind, SignKind::EveryBuilding { spare: 1 });
        // No target in the data: target_for is always None, the way BranchComplete's is; the
        // sim works out the per-nation number itself (`world/endeavor.rs`).
        assert_eq!(e.signs[0].target_for(Nation::Air), None);
        // Today's bundled counts, so a content change that moves these is visible in review.
        assert_eq!(
            reachable_counts(),
            [(Nation::Air, 11), (Nation::Water, 11), (Nation::Earth, 11), (Nation::Fire, 12)]
        );
    }

    #[test]
    fn every_building_defaults_spare_to_zero_and_target_and_per_nation_dont_apply() {
        let e = load(
            "[[sign]]\nid = \"b\"\ngroup = \"buildings\"\nname = \"B\"\nshort = \"b\"\nkind = \"every_building\"\n",
        )
        .unwrap();
        assert_eq!(e.signs[0].kind, SignKind::EveryBuilding { spare: 0 });
        assert!(
            error(&sign("kind = \"every_building\"\ntarget = 3")).contains("target doesn't apply")
        );
        assert!(
            error(&sign("kind = \"every_building\"\nper_nation = { air = 3 }"))
                .contains("per_nation doesn't apply")
        );
    }

    #[test]
    fn every_building_spare_must_be_below_the_smallest_nations_count() {
        // Air, Water and Earth are tied for fewest kinds today, 11 (the shared Workshop, #369,
        // Map Room, #402 and Herb Garden, #390; the Mine Shaft and Deep Mine for Earth, #373;
        // the Healing Hut and Fishing Dock for Water, #375, #374); spare = 11 would zero every
        // one of their targets.
        let e = error(&sign("kind = \"every_building\"\nspare = 11"));
        assert!(e.contains("spare must be below the smallest nation's count (11)"), "{e}");
        assert!(load(&sign("kind = \"every_building\"\nspare = 10")).is_ok());
    }

    #[test]
    fn ids_and_names_must_be_present_and_ids_unique() {
        let two = format!(
            "{}{}",
            sign("kind = \"population\"\ntarget = 1"),
            sign("kind = \"population\"\ntarget = 2")
        );
        assert!(error(&two).contains("duplicate id"));
        assert!(error("[[sign]]\nid = \" \"\ngroup = \"people\"\nname = \"S\"\nshort = \"s\"\nkind = \"population\"\ntarget = 1\n").contains("id must not be empty"));
        assert!(error("[[sign]]\nid = \"s\"\ngroup = \"people\"\nname = \"\"\nshort = \"s\"\nkind = \"population\"\ntarget = 1\n").contains("name must not be empty"));
        assert!(file_error("[endeavor]\nname = \"\"\n").contains("name must not be empty"));
    }

    const GATE: &str = "[endeavor]\nname = \"G\"\nbuilding = \"harmony_gate\"\n";

    /// The bundled data with the gate and these stages as the endeavor.
    fn stages(src: &str) -> Result<EndeavorDef, DefsError> {
        load_file(&format!("{GATE}{src}"))
    }

    fn stage_error(src: &str) -> String {
        stages(src).expect_err("should be rejected").0
    }

    const PILLAR: &str = "[[stage.task]]\nid = \"t\"\nname = \"T\"\nelement = \"earth\"\n\
                          chi = 50.0\nwork = 10.0\n";

    #[test]
    fn the_bundled_gate_has_its_stages_and_a_pillar_per_element() {
        let defs = Defs::bundled();
        let e = &defs.endeavor;
        assert_eq!(e.building, defs.building_id("harmony_gate"));
        let ids: Vec<&str> = e.stages.iter().map(|s| s.id.as_str()).collect();
        assert_eq!(ids, ["foundation", "pillars", "arch", "rite"]);
        let foundation = &e.stages[0];
        assert!(
            foundation.work > 0.0 && !foundation.cost.is_empty() && foundation.tasks.is_empty()
        );
        let (n, pillars) = e.stage("pillars").unwrap();
        assert_eq!(n, 1);
        assert_eq!(pillars.work, 0.0);
        let elements: Vec<Nation> = pillars
            .tasks
            .iter()
            .map(|t| match t.element {
                TaskWho::Nation(n) => n,
                TaskWho::Avatar => panic!("a pillar task is for a nation, not the Avatar"),
            })
            .collect();
        assert_eq!(elements, Nation::ALL);
        assert!(pillars.tasks.iter().all(|t| t.chi <= defs.chi.max && !t.cost.is_empty()));
        // The Arch's beam lift needs the Avatar (docs/design/endgame.md, "The project").
        let (_, arch) = e.stage("arch").unwrap();
        assert!(arch.tasks.iter().any(|t| t.element == TaskWho::Avatar));
        // The Harmony Rite is the last stage, and has none of the usual cost, work or tasks.
        let (last, rite) = e.stage("rite").unwrap();
        assert_eq!(last, e.stages.len() - 1);
        assert!(rite.cost.is_empty() && rite.work == 0.0 && rite.tasks.is_empty());
        let r = rite.rite.expect("the rite stage carries [stage.rite]");
        assert!(r.minutes > 0.0 && r.attendance_share > 0.0 && r.attendance_share <= 1.0);
        assert!(e.avatar.is_some());
    }

    #[test]
    fn a_stage_resolves_its_cost_and_tasks() {
        let e = stages(&format!(
            "[[stage]]\nid = \"s\"\nname = \"S\"\ncost = [{{ item = \"stone\", count = 4 }}]\n\
             work = 30.0\n{PILLAR}cost = [{{ item = \"wood\", count = 2 }}]\n"
        ))
        .unwrap();
        let defs = Defs::bundled();
        let s = &e.stages[0];
        assert_eq!(s.cost, [CostItem { item: defs.item_id("stone").unwrap(), count: 4 }]);
        assert_eq!(s.work, 30.0);
        let t = &s.tasks[0];
        assert_eq!((t.element, t.chi, t.work), (TaskWho::Nation(Nation::Earth), 50.0, 10.0));
        assert_eq!(t.cost, [CostItem { item: defs.item_id("wood").unwrap(), count: 2 }]);
    }

    #[test]
    fn the_gate_and_its_stages_come_together() {
        let stage = "[[stage]]\nid = \"s\"\nname = \"S\"\nwork = 5.0\n";
        let e = load_file(&format!("[endeavor]\nname = \"G\"\n{stage}")).unwrap_err();
        assert!(e.0.contains("need a building"), "{e}");
        assert!(stage_error("").contains("at least one [[stage]]"));
        let e = load_file(&format!("[endeavor]\nname = \"G\"\nbuilding = \"bed\"\n{stage}"))
            .unwrap_err();
        assert!(e.0.contains("endeavor = true"), "{e}");
        let e = load_file(&format!("[endeavor]\nname = \"G\"\nbuilding = \"ziggurat\"\n{stage}"))
            .unwrap_err();
        assert!(e.0.contains("unknown building"), "{e}");
        // No gate and no stages is a world that never builds one.
        let none = load("").unwrap();
        assert!(none.building.is_none() && none.stages.is_empty());
    }

    #[test]
    fn stages_and_tasks_are_checked() {
        let s = |body: &str| format!("[[stage]]\nid = \"s\"\nname = \"S\"\n{body}\n");
        assert!(stage_error(&s("")).contains("needs work or at least one task"));
        assert!(stage_error(&s("work = -1.0")).contains("work must be"));
        let e = stage_error(&s("cost = [{ item = \"stone\", count = 1 }]\n"));
        assert!(e.contains("needs work"), "{e}");
        assert!(
            stage_error(&s("work = 1.0\ncost = [{ item = \"mithril\", count = 1 }]"))
                .contains("unknown item")
        );
        assert!(
            stage_error(&s("work = 1.0\ncost = [{ item = \"stone\", count = 0 }]"))
                .contains("positive")
        );
        let twice =
            "work = 1.0\ncost = [{ item = \"stone\", count = 1 }, { item = \"stone\", count = 2 }]";
        assert!(stage_error(&s(twice)).contains("costed twice"));
        assert!(stage_error(&s("work = 1.0\nhue = 3")).contains("hue"));
        assert!(
            stage_error(&format!("{}{}", s("work = 1.0"), s("work = 2.0")))
                .contains("duplicate id")
        );
        let task = |body: &str| s(&format!("[[stage.task]]\nid = \"t\"\nname = \"T\"\n{body}"));
        let e = stage_error(&task("element = \"earth\"\nchi = 500.0\nwork = 1.0"));
        assert!(e.contains("chi.max"), "{e}");
        assert!(stage_error(&task("element = \"earth\"\nchi = 0.0\nwork = 1.0")).contains("chi"));
        assert!(stage_error(&task("element = \"earth\"\nchi = 5.0\nwork = 0.0")).contains("work"));
        assert!(stage_error(&task("element = \"metal\"\nchi = 5.0\nwork = 1.0")).contains("metal"));
        let two = format!("{}[[stage]]\nid = \"s2\"\nname = \"S2\"\n{PILLAR}", s(PILLAR));
        assert!(stage_error(&two).contains("task t: duplicate id"));
    }

    #[test]
    fn a_task_for_the_avatar_needs_an_avatar_section() {
        let task = "[[stage.task]]\nid = \"t\"\nname = \"T\"\nelement = \"avatar\"\n\
                     chi = 50.0\nwork = 1.0\n";
        let e = stage_error(&format!("[[stage]]\nid = \"s\"\nname = \"S\"\n{task}"));
        assert!(e.contains("needs an [avatar] section"), "{e}");
    }

    const RITE: &str = "[[stage]]\nid = \"rite\"\nname = \"R\"\n[stage.rite]\nminutes = 10.0\nattendance_share = 0.5\n";

    /// A valid `[avatar]` section on its own (no `[endeavor]` header), to compose with `stages()`.
    const AVATAR_SECTION: &str = "[avatar]\nid = \"a\"\nname = \"A\"\ncharacter = \"c\"\n\
        chi_max = 200.0\npatience_minutes = 1.0\nreturn_minutes = 1.0\n\
        arrive_after_minutes = 1.0\nfarewell_minutes = 1.0\nabilities = [\"raise_earth\"]\n\
        [avatar.modifiers]\nmove_speed = 1.0\ncarry_capacity = 1.0\nearthworks = 1.0\n\
        heating = 1.0\nwater_work = 1.0\npower_generation = 1.0\nconstruction = 1.0\n\
        study = 1.0\nchi_regen = 1.0\n";

    #[test]
    fn a_rite_stage_needs_an_avatar_section_and_carries_no_other_fields() {
        let e = stage_error(RITE);
        assert!(e.contains("needs an [avatar] section"), "{e}");
        let with_cost = "[[stage]]\nid = \"rite\"\nname = \"R\"\n\
             cost = [{ item = \"stone\", count = 1 }]\n[stage.rite]\nminutes = 10.0\n\
             attendance_share = 0.5\n";
        assert!(stage_error(with_cost).contains("no cost, work or task"));
        let with_work = "[[stage]]\nid = \"rite\"\nname = \"R\"\nwork = 1.0\n\
             [stage.rite]\nminutes = 10.0\nattendance_share = 0.5\n";
        assert!(stage_error(with_work).contains("no cost, work or task"));
        let with_task = format!(
            "[[stage]]\nid = \"rite\"\nname = \"R\"\n{PILLAR}\
             [stage.rite]\nminutes = 10.0\nattendance_share = 0.5\n"
        );
        assert!(stage_error(&with_task).contains("no cost, work or task"));
    }

    #[test]
    fn a_rite_stages_own_numbers_are_checked() {
        let rite = |minutes: &str, share: &str| {
            format!(
                "[[stage]]\nid = \"rite\"\nname = \"R\"\n[stage.rite]\nminutes = {minutes}\n\
                 attendance_share = {share}\n"
            )
        };
        assert!(stage_error(&rite("0.0", "0.5")).contains("minutes must be"));
        assert!(stage_error(&rite("-1.0", "0.5")).contains("minutes must be"));
        assert!(stage_error(&rite("10.0", "0.0")).contains("attendance_share"));
        assert!(stage_error(&rite("10.0", "1.5")).contains("attendance_share"));
        // Exactly 1.0 (everyone) is allowed.
        let e = format!("{}{}", rite("10.0", "1.0"), AVATAR_SECTION);
        assert!(stages(&e).is_ok());
    }

    #[test]
    fn only_one_stage_may_be_the_rite_and_it_must_be_last() {
        let second_rite = "[[stage]]\nid = \"rite2\"\nname = \"R2\"\n\
             [stage.rite]\nminutes = 10.0\nattendance_share = 0.5\n";
        let two = format!("{RITE}{second_rite}");
        let e = stage_error(&two);
        assert!(e.contains("only one stage"), "{e}");
        let after = format!("{RITE}[[stage]]\nid = \"after\"\nname = \"After\"\nwork = 1.0\n");
        let e = stage_error(&after);
        assert!(e.contains("must be the last stage"), "{e}");
    }

    #[test]
    fn the_bundled_avatar_has_his_abilities_refuses_and_modifiers() {
        let defs = Defs::bundled();
        let a = defs.endeavor.avatar.as_ref().expect("bundled data has an avatar");
        assert_eq!(a.name, "Aang");
        assert_eq!(a.chi_max, 200.0);
        assert_eq!(a.modifiers.chi_regen, 2.0);
        assert_eq!(a.modifiers.move_speed, 1.6);
        let signature = ["raise_earth", "water_the_land", "hearthflame", "wind_carry"];
        for id in signature {
            assert!(a.abilities.contains(&defs.ability_id(id).unwrap()), "{id}");
        }
        assert_eq!(a.abilities.len(), signature.len());
        assert_eq!(a.refuses, [defs.item_id("fish").unwrap()]);
        assert!(a.patience_minutes > 0.0 && a.return_minutes > 0.0 && a.arrive_after_minutes > 0.0);
        assert!(a.farewell_minutes > 0.0);
    }

    /// A valid `[avatar]` section, with `chi_max`, `patience_minutes`, `abilities` and `refuses`
    /// as given, so each test only names the field it is checking.
    fn avatar_src(chi_max: &str, patience: &str, abilities: &str, refuses: &str) -> String {
        format!(
            "[endeavor]\nname = \"G\"\n[avatar]\nid = \"a\"\nname = \"A\"\ncharacter = \"c\"\n\
             chi_max = {chi_max}\npatience_minutes = {patience}\nreturn_minutes = 1.0\n\
             arrive_after_minutes = 1.0\nfarewell_minutes = 1.0\n\
             abilities = {abilities}\nrefuses = {refuses}\n\
             [avatar.modifiers]\nmove_speed = 1.0\ncarry_capacity = 1.0\nearthworks = 1.0\n\
             heating = 1.0\nwater_work = 1.0\npower_generation = 1.0\nconstruction = 1.0\n\
             study = 1.0\nchi_regen = 1.0\n"
        )
    }

    fn avatar_ok() -> String {
        avatar_src("200.0", "20.0", "[\"raise_earth\"]", "[\"fish\"]")
    }

    #[test]
    fn a_minimal_avatar_section_loads() {
        let e = load_file(&avatar_ok()).unwrap();
        assert!(e.avatar.is_some());
    }

    #[test]
    fn the_avatar_sections_numbers_and_ids_are_checked() {
        let e = |src: String| load_file(&src).expect_err("should be rejected").0;
        assert!(e(avatar_src("-1.0", "20.0", "[\"raise_earth\"]", "[]")).contains("chi_max"));
        assert!(e(avatar_src("200.0", "0.0", "[\"raise_earth\"]", "[]")).contains("patience"));
        assert!(e(avatar_src("200.0", "20.0", "[]", "[]")).contains("at least one ability"));
        assert!(e(avatar_src("200.0", "20.0", "[\"nonsense\"]", "[]")).contains("unknown ability"));
        let twice = e(avatar_src("200.0", "20.0", "[\"raise_earth\", \"raise_earth\"]", "[]"));
        assert!(twice.contains("listed twice"), "{twice}");
        assert!(
            e(avatar_src("200.0", "20.0", "[\"raise_earth\"]", "[\"nonsense\"]"))
                .contains("unknown item")
        );
        let not_food = e(avatar_src("200.0", "20.0", "[\"raise_earth\"]", "[\"stone\"]"));
        assert!(not_food.contains("isn't a food"), "{not_food}");
    }

    #[test]
    fn the_avatars_id_name_and_character_must_not_be_empty() {
        let base = "[endeavor]\nname = \"G\"\n[avatar]\nid = \"{id}\"\nname = \"{name}\"\n\
                     character = \"{character}\"\nchi_max = 200.0\npatience_minutes = 1.0\n\
                     return_minutes = 1.0\narrive_after_minutes = 1.0\nfarewell_minutes = 1.0\n\
                     abilities = [\"raise_earth\"]\n[avatar.modifiers]\nmove_speed = 1.0\n\
                     carry_capacity = 1.0\nearthworks = 1.0\nheating = 1.0\nwater_work = 1.0\n\
                     power_generation = 1.0\nconstruction = 1.0\nstudy = 1.0\nchi_regen = 1.0\n";
        let src = |id: &str, name: &str, character: &str| {
            base.replace("{id}", id).replace("{name}", name).replace("{character}", character)
        };
        assert!(load_file(&src(" ", "A", "c")).expect_err("should be rejected").0.contains("id"));
        assert!(load_file(&src("a", " ", "c")).expect_err("should be rejected").0.contains("name"));
        assert!(
            load_file(&src("a", "A", " ")).expect_err("should be rejected").0.contains("character")
        );
        assert!(load_file(&src("a", "A", "c")).is_ok());
    }
}
