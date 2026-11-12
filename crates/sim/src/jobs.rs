//! Items, harvest yields and job settings (`data/jobs.toml`), plus the plain types the job queue
//! is made of. The behaviour (claiming, pre-emption, hauling) lives in `world::work`.
//!
//! Items are one id space: the `[[item]]` materials in `jobs.toml` first, then every food from
//! `needs.toml` (so a harvested cabbage is both a food and something you can stockpile).

use serde::Deserialize;

use crate::bending::AbilityId;
use crate::defs::{DefsError, ResourceDef, ResourceId, Terrain};
use crate::geom::Pos;
use crate::nation::Modifiers;
use crate::needs::{FoodDef, NeedsSettings};
use crate::research::{TechDef, TechId};
use crate::world::UnitId;

// `Deserialize` is `save::ids`' remapping impl (#602).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, serde::Serialize)]
pub struct ItemId(pub u16);

#[derive(Debug, Clone, PartialEq)]
pub struct ItemDef {
    pub id: String,
    pub name: String,
    /// Index into `Defs::foods` if this item can be eaten.
    pub food: Option<usize>,
    /// Knowledge points reading this item adds to the research pool, if it can be read (issue
    /// #24). A readable item is carried to a study hall and used up there, never stockpiled.
    pub knowledge: Option<u32>,
    /// The item this one may pay for, one for one, in any building or endeavor cost (issue #371:
    /// `brick` stands for `stone`). Never a recipe input (`world/forge.rs` never substitutes).
    /// `None` for an ordinary item. Validated at load: not itself, no chains (the target can't
    /// itself stand in for something), and at most one item may stand in for a given target.
    pub stands_for: Option<ItemId>,
    /// Teachings reading this item offers (a teaching scroll's draft, docs/design/
    /// horizons-phase-5.md, "Searching one", #737). Like `knowledge`, it makes the item
    /// readable: carried to a study hall and used up there. At most one of the two; 1 to 4.
    pub reveals: Option<u32>,
}

impl ItemDef {
    /// Whether the item is read at a study hall (`knowledge` or `reveals`): never stockpiled,
    /// carried straight to the nearest hall and used up there.
    pub fn is_readable(&self) -> bool {
        self.knowledge.is_some() || self.reveals.is_some()
    }
}

/// A number of one kind of item.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Stack {
    pub item: ItemId,
    pub count: u32,
}

/// What working a map resource yields.
#[derive(Debug, Clone, PartialEq)]
pub struct HarvestDef {
    pub resource: ResourceId,
    /// What the designation is called ("Chop", "Quarry").
    pub verb: String,
    /// What the resource is called after a count, for a tool's preview ("7 trees to chop"):
    /// `noun` for one, `noun_many` for any other number. The resource's own name in lower case
    /// and that with an `s` when the data names neither.
    pub noun: String,
    pub noun_many: String,
    pub item: ItemId,
    pub count: u32,
    /// In-game seconds of work at a modifier of 1.0.
    pub work: f32,
    /// Nation modifier (`Modifiers::get`) that scales the work speed, if any.
    pub modifier: Option<String>,
    /// In-game seconds a renewable resource takes to ripen again after being worked. `None`
    /// means the resource is used up. See `World::growth_rate` for what speeds this up.
    pub regrow: Option<f32>,
    /// What one harvest yields instead of `count` when the resource is mined from inside a
    /// mountain: when a tunnel (`Terrain::Tunnel`) touches its tile (issue #132,
    /// `World::harvest_yield`). `None` means tunnels make no difference.
    pub tunnel_count: Option<u32>,
    /// The tech that must be known before this can be designated or foraged
    /// (`data/research.toml`, `Unlocks::jobs`, issue #22). `None` means every nation can work it
    /// from the start.
    pub unlocked_by: Option<TechId>,
    /// Skips the first frost of winter and grows at a flat `hardy_winter_growth` in every
    /// climate band instead of the season or climate factor (`docs/design/difficulty.md`,
    /// "Seasons", #295). Only the fish harvest has this today: ice fishing keeps working through
    /// winter everywhere.
    pub frost_hardy: bool,
    /// Strain this harvest adds to its tile per pick, before the difficulty preset's `strain`
    /// knob (`docs/design/difficulty.md`, "Strain: tiring land", #296): 0.0 (the default) means
    /// it never tires. Only wild renewable harvests (fruit, cabbage, fish) have one; one-shot
    /// harvests (trees, stone, gold) are used up instead, so strain would never be read for
    /// them.
    pub strain: f32,
    /// The chance that finishing one pick of this harvest injures the worker (felling, quarrying,
    /// mining; `docs/design/harm.md`, "Injury: heavy work", issue #388), in `[0, 1)`. 0.0 (the
    /// default) means this harvest never injures anyone. The chance actually rolled
    /// (`World::maybe_injure`) is this divided by the harvest's own `modifier` (earthworks for
    /// stone and ore), so Earth's earthworks lowers it and Air's weaker one raises it.
    pub risk: f32,
    /// How to uproot this resource for good instead of merely working it (the Clear designation,
    /// `docs/design/clearing.md`, issue #411/#424), when a plain harvest would leave it standing
    /// (`regrow` is `Some`). `None` for a resource whose harvest already removes it (every
    /// non-renewable: chopping a tree already clears it) and for a renewable with nothing to
    /// offer a clear (fish spots, on water where nobody builds).
    pub clear: Option<ClearDef>,
    /// Extra stacks that drop beside the main yield when the harvest completes, one ground stack
    /// per entry (`also`, docs/design/horizons-phase-5.md, "Searching one", #736). Not scaled by
    /// `tunnel_count` or anything else. Empty for every harvest but the great ruin's.
    pub also: Vec<Stack>,
}

impl HarvestDef {
    /// The risk actually rolled (before the modifier divide, `World::maybe_injure`) when a
    /// `JobKind::Clear` on this resource finishes: the clear's own `risk` for a renewable with
    /// `[harvest.clear]`, else this harvest's own, since clearing a non-renewable is its harvest.
    pub fn clear_risk(&self) -> f32 {
        self.clear.map_or(self.risk, |c| c.risk)
    }
}

/// `HarvestDef::clear`: how a `JobKind::Clear` uproots a renewable resource instead of merely
/// picking it. Validated at load (`JobsFile::resolve`): only on a harvest with `regrow`, `work`
/// above 0, `item` known, `count` only with an `item`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ClearDef {
    /// In-game seconds of work, at a flat rate of 1.0 (no nation modifier: the data has none to
    /// name).
    pub work: f32,
    /// What the clear drops, beside the plant's own ripe yield if it has one (`item`/`count` of
    /// the harvest itself). `None` means the clear drops nothing of its own (a cabbage: pulling
    /// it up yields nothing but its crop, when ripe).
    pub item: Option<ItemId>,
    /// How many of `item` the clear drops. 0 when `item` is `None`.
    pub count: u32,
    /// The chance finishing the clear injures the worker, exactly as `HarvestDef::risk` (the
    /// existing `harm.injury` stream), but with no modifier of its own: uprooting isn't scaled by
    /// a nation's earthworks or the like.
    pub risk: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct JobSettings {
    /// Items one stockpile tile holds.
    pub stockpile_stack: u32,
    /// Items per carrying trip at a `carry_capacity` of 1.0.
    pub base_carry: u32,
    pub eat_seconds: f32,
    pub drink_seconds: f32,
    /// A sleeping unit wakes at this energy...
    pub sleep_until: f32,
    /// ...or earlier when hunger or thirst falls below this and it can act on it.
    pub sleep_interrupt_below: f32,
    /// Seconds before a unit with nothing reachable to do looks again.
    pub retry_seconds: f32,
    /// Seconds an idle unit stands still before it decides whether to wander (issue #110).
    pub idle_pause_seconds: f32,
    /// Chance, in (0, 1], that an idle unit wanders after a pause rather than standing again.
    pub idle_wander_chance: f32,
    /// A wander is 1 to this many random steps.
    pub idle_wander_steps: u8,
    /// Strain fades by this much per in-game minute, ripe or resting (`docs/design/difficulty.md`,
    /// "Strain: tiring land", #296), unscaled by any difficulty knob.
    pub strain_recovery_per_minute: f32,
    /// With the difficulty preset's `strain_kills`, a harvest that leaves its tile's strain at
    /// or above this kills the plant (#296).
    pub strain_kill_at: f32,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawItem {
    id: String,
    name: String,
    #[serde(default)]
    knowledge: Option<u32>,
    #[serde(default)]
    stands_for: Option<String>,
    #[serde(default)]
    reveals: Option<u32>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawHarvest {
    resource: String,
    verb: String,
    #[serde(default)]
    noun: Option<String>,
    #[serde(default)]
    noun_many: Option<String>,
    item: String,
    count: u32,
    work: f32,
    modifier: Option<String>,
    regrow: Option<f32>,
    tunnel_count: Option<u32>,
    #[serde(default)]
    frost_hardy: bool,
    #[serde(default)]
    strain: f32,
    #[serde(default)]
    risk: f32,
    #[serde(default)]
    clear: Option<RawClear>,
    #[serde(default)]
    also: Vec<RawStack>,
}

/// One `also = [{ item, count }]` entry of a harvest.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawStack {
    item: String,
    count: u32,
}

/// `[harvest.clear]` (`docs/design/clearing.md`, issue #411/#424): the Clear designation's own
/// numbers, for a renewable resource whose ordinary harvest leaves it standing.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawClear {
    work: f32,
    #[serde(default)]
    item: Option<String>,
    #[serde(default)]
    count: Option<u32>,
    #[serde(default)]
    risk: f32,
}

/// What `JobsFile::resolve` hands `Defs`.
pub(crate) type JobsResolved =
    (JobSettings, Vec<ItemDef>, Vec<HarvestDef>, Option<ShoreFishingDef>);

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct JobsFile {
    settings: JobSettings,
    item: Vec<RawItem>,
    harvest: Vec<RawHarvest>,
    #[serde(default)]
    shore_fishing: Option<RawShoreFishing>,
}

/// `[shore_fishing]` (docs/design/horizons-phase-4.md, `southern_ways`, #727): fishing from any
/// listed water tile, read only while the `shore_fishing` feature is known.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawShoreFishing {
    item: String,
    count: u32,
    work: f32,
    modifier: Option<String>,
    terrains: Vec<Terrain>,
    #[serde(default)]
    frost_hardy: bool,
}

/// Shore fishing (#727): a Fish order on any of `terrains` with a walkable neighbour, not only on
/// a fish spot, once `shore_fishing` is known (`World::harvest_at`). It never depletes and never
/// rests.
#[derive(Debug, Clone, PartialEq)]
pub struct ShoreFishingDef {
    /// The harvest it works as: `regrow` is `None` but nothing is used up, and `resource` is
    /// `ResourceId(u16::MAX)`, since no resource stands on the tile.
    pub harvest: HarvestDef,
    pub terrains: Vec<Terrain>,
}

impl JobsFile {
    /// Checks the data and links names to ids.
    pub(crate) fn resolve(
        self,
        resources: &[ResourceDef],
        foods: &[FoodDef],
        needs: &NeedsSettings,
        techs: &[TechDef],
    ) -> Result<JobsResolved, DefsError> {
        let err = |msg: String| Err(DefsError(format!("jobs.toml: {msg}")));
        let s = self.settings;
        for (what, v) in [
            ("eat_seconds", s.eat_seconds),
            ("drink_seconds", s.drink_seconds),
            ("retry_seconds", s.retry_seconds),
            ("idle_pause_seconds", s.idle_pause_seconds),
            ("strain_recovery_per_minute", s.strain_recovery_per_minute),
            ("strain_kill_at", s.strain_kill_at),
        ] {
            if !(v.is_finite() && v > 0.0) {
                return err(format!("settings.{what} must be a positive number"));
            }
        }
        if !(s.idle_wander_chance > 0.0 && s.idle_wander_chance <= 1.0) {
            return err("settings.idle_wander_chance must be in (0, 1]".into());
        }
        if s.idle_wander_steps == 0 {
            return err("settings.idle_wander_steps must be at least 1".into());
        }
        if s.stockpile_stack == 0 || s.base_carry == 0 {
            return err("settings.stockpile_stack and base_carry must be at least 1".into());
        }
        // Wake-up must be reachable, and above the level that sends a unit to bed, or it would
        // wake and fall asleep again at once.
        if !(needs.energy.low < s.sleep_until && s.sleep_until <= needs.max()) {
            return err("settings.sleep_until must be in (energy.low, needs max]".into());
        }
        if !(s.sleep_interrupt_below.is_finite()
            && (0.0..=needs.hunger.low.min(needs.thirst.low)).contains(&s.sleep_interrupt_below))
        {
            return err("settings.sleep_interrupt_below must be in [0, hunger/thirst low]".into());
        }

        let all_ids = self
            .item
            .iter()
            .map(|i| i.id.as_str())
            .chain(foods.iter().map(|f| f.id.as_str()))
            .collect::<Vec<_>>();
        for (i, id) in all_ids.iter().enumerate() {
            if all_ids[..i].contains(id) {
                return err(format!("duplicate item {id} (foods are items too)"));
            }
        }
        if let Some(i) = self.item.iter().find(|i| i.knowledge == Some(0)) {
            return err(format!("item {}: knowledge must be at least 1", i.id));
        }
        if let Some(i) = self.item.iter().find(|i| i.knowledge.is_some() && i.reveals.is_some()) {
            return err(format!("item {}: has both knowledge and reveals; at most one", i.id));
        }
        if let Some(i) = self.item.iter().find(|i| i.reveals.is_some_and(|n| !(1..=4).contains(&n)))
        {
            return err(format!("item {}: reveals must be 1 to 4", i.id));
        }
        // `stands_for` only ever names a non-food item (brick for stone), so it's resolved
        // against the raw list before it's consumed below.
        let raw_stands_for: Vec<Option<String>> =
            self.item.iter().map(|i| i.stands_for.clone()).collect();
        let mut items: Vec<ItemDef> = self
            .item
            .into_iter()
            .map(|i| ItemDef {
                id: i.id,
                name: i.name,
                food: None,
                knowledge: i.knowledge,
                stands_for: None,
                reveals: i.reveals,
            })
            .collect();
        items.extend(foods.iter().enumerate().map(|(n, f)| ItemDef {
            id: f.id.clone(),
            name: f.name.clone(),
            food: Some(n),
            knowledge: None,
            stands_for: None,
            reveals: None,
        }));
        for (idx, target) in raw_stands_for.iter().enumerate() {
            let Some(target_id) = target else { continue };
            if target_id == &items[idx].id {
                return err(format!("item {}: stands_for can't name itself", items[idx].id));
            }
            let Some(target_idx) = items.iter().position(|i| &i.id == target_id) else {
                return err(format!(
                    "item {}: stands_for names unknown item {target_id:?}",
                    items[idx].id
                ));
            };
            if raw_stands_for.get(target_idx).is_some_and(Option::is_some) {
                return err(format!(
                    "item {}: stands_for can't chain to {target_id:?}, which itself stands for \
                     something",
                    items[idx].id
                ));
            }
            items[idx].stands_for = Some(ItemId(target_idx as u16));
        }
        for (i, item) in items.iter().enumerate() {
            if let Some(target) = item.stands_for
                && items
                    .iter()
                    .enumerate()
                    .any(|(j, other)| j != i && other.stands_for == Some(target))
            {
                return err(format!(
                    "two items stand in for {}; only one may",
                    items[target.0 as usize].id
                ));
            }
        }

        let mut harvests: Vec<HarvestDef> = Vec::new();
        for h in self.harvest {
            let r = &h.resource;
            let Some(resource) = resources.iter().position(|d| d.id == *r) else {
                return err(format!("harvest of unknown resource {r:?}"));
            };
            if harvests.iter().any(|d| d.resource.0 as usize == resource) {
                return err(format!("resource {r} has two harvest entries"));
            }
            let Some(item) = items.iter().position(|i| i.id == h.item) else {
                return err(format!("harvest {r}: unknown item {:?}", h.item));
            };
            if h.count == 0 || !(h.work.is_finite() && h.work > 0.0) {
                return err(format!("harvest {r}: count and work must be positive"));
            }
            if let Some(m) = &h.modifier
                && !Modifiers::NAMES.contains(&m.as_str())
            {
                return err(format!("harvest {r}: unknown modifier {m:?}"));
            }
            if let Some(g) = h.regrow
                && !(g.is_finite() && g > 0.0)
            {
                return err(format!("harvest {r}: regrow must be a positive number of seconds"));
            }
            if h.tunnel_count.is_some_and(|t| t <= h.count) {
                return err(format!("harvest {r}: tunnel_count must be more than count"));
            }
            if h.frost_hardy && h.regrow.is_none() {
                return err(format!("harvest {r}: frost_hardy needs a regrow time"));
            }
            if !(h.strain.is_finite() && h.strain >= 0.0) {
                return err(format!("harvest {r}: strain must be at least 0"));
            }
            if h.strain > 0.0 && h.regrow.is_none() {
                return err(format!("harvest {r}: strain needs a regrow time"));
            }
            if !(h.risk.is_finite() && (0.0..1.0).contains(&h.risk)) {
                return err(format!("harvest {r}: risk must be in [0, 1)"));
            }
            let clear = match h.clear {
                None => None,
                Some(c) => {
                    if h.regrow.is_none() {
                        return err(format!("harvest {r}: clear needs a regrow time"));
                    }
                    if !(c.work.is_finite() && c.work > 0.0) {
                        return err(format!("harvest {r}: clear.work must be positive"));
                    }
                    if !(c.risk.is_finite() && (0.0..1.0).contains(&c.risk)) {
                        return err(format!("harvest {r}: clear.risk must be in [0, 1)"));
                    }
                    let item = match (&c.item, c.count) {
                        (None, Some(_)) => {
                            return err(format!("harvest {r}: clear.count needs an item"));
                        }
                        (Some(_), None) => {
                            return err(format!("harvest {r}: clear.item needs a count"));
                        }
                        (None, None) => None,
                        (Some(name), Some(_)) => {
                            let Some(i) = items.iter().position(|i| i.id == *name) else {
                                return err(format!("harvest {r}: clear: unknown item {name:?}"));
                            };
                            Some(ItemId(i as u16))
                        }
                    };
                    Some(ClearDef { work: c.work, item, count: c.count.unwrap_or(0), risk: c.risk })
                }
            };
            let mut also = Vec::new();
            for a in &h.also {
                let Some(i) = items.iter().position(|i| i.id == a.item) else {
                    return err(format!("harvest {r}: also: unknown item {:?}", a.item));
                };
                if a.count == 0 {
                    return err(format!("harvest {r}: also: count must be at least 1"));
                }
                also.push(Stack { item: ItemId(i as u16), count: a.count });
            }
            let noun = h.noun.unwrap_or_else(|| resources[resource].name.to_lowercase());
            let noun_many = h.noun_many.unwrap_or_else(|| format!("{noun}s"));
            harvests.push(HarvestDef {
                resource: ResourceId(resource as u16),
                verb: h.verb,
                noun,
                noun_many,
                item: ItemId(item as u16),
                count: h.count,
                work: h.work,
                modifier: h.modifier,
                regrow: h.regrow,
                tunnel_count: h.tunnel_count,
                unlocked_by: None,
                frost_hardy: h.frost_hardy,
                strain: h.strain,
                risk: h.risk,
                clear,
                also,
            });
        }
        link_unlocks(&mut harvests, resources, techs)?;
        let shore = match self.shore_fishing {
            None => None,
            Some(r) => {
                let e = |msg: &str| Err(DefsError(format!("jobs.toml: shore_fishing: {msg}")));
                let Some(item) = items.iter().position(|i| i.id == r.item) else {
                    return e(&format!("unknown item {:?}", r.item));
                };
                if r.count == 0 || !(r.work.is_finite() && r.work > 0.0) {
                    return e("count and work must be positive");
                }
                if let Some(m) = &r.modifier
                    && !Modifiers::NAMES.contains(&m.as_str())
                {
                    return e(&format!("unknown modifier {m:?}"));
                }
                if r.terrains.is_empty() || r.terrains.iter().any(|t| !t.is_water()) {
                    return e("terrains must list water and only water");
                }
                Some(ShoreFishingDef {
                    harvest: HarvestDef {
                        resource: ResourceId(u16::MAX),
                        verb: "Fish".into(),
                        noun: "shore tile".into(),
                        noun_many: "shore tiles".into(),
                        item: ItemId(item as u16),
                        count: r.count,
                        work: r.work,
                        modifier: r.modifier,
                        regrow: None,
                        tunnel_count: None,
                        unlocked_by: None,
                        frost_hardy: r.frost_hardy,
                        strain: 0.0,
                        risk: 0.0,
                        clear: None,
                        also: Vec::new(),
                    },
                    terrains: r.terrains,
                })
            }
        };
        Ok((s, items, harvests, shore))
    }
}

/// Resolves every tech's `unlocks.jobs` into `HarvestDef::unlocked_by`: each entry names a
/// resource (`data/resources.toml`) that has a harvest entry, and may be unlocked by no other
/// tech.
fn link_unlocks(
    harvests: &mut [HarvestDef],
    resources: &[ResourceDef],
    techs: &[TechDef],
) -> Result<(), DefsError> {
    for (t, tech) in techs.iter().enumerate() {
        let err = |msg: String| Err(DefsError(format!("tech {}: unlocks.jobs: {msg}", tech.id)));
        for name in &tech.unlocks.jobs {
            let Some(r) = resources.iter().position(|r| r.id == *name) else {
                return err(format!("unknown resource {name:?}"));
            };
            let resource = ResourceId(r as u16);
            let Some(h) = harvests.iter_mut().find(|h| h.resource == resource) else {
                return err(format!("resource {name:?} has no harvest entry"));
            };
            if let Some(other) = h.unlocked_by {
                return if other == TechId(t as u16) {
                    err(format!("{name:?} listed twice"))
                } else {
                    err(format!(
                        "{name:?} is already unlocked by {:?}; a job has one tech",
                        techs[other.0 as usize].id
                    ))
                };
            }
            h.unlocked_by = Some(TechId(t as u16));
        }
    }
    Ok(())
}

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, serde::Serialize, serde::Deserialize,
)]
pub struct JobId(pub u32);

/// Player-designated work, waiting for a unit. Needs (eat, drink, sleep) are not jobs: units
/// handle those themselves.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum JobKind {
    /// Work the resource on this tile until it yields its items (chop, quarry, gather, mine).
    Harvest { pos: Pos },
    /// Carry the item stack lying on this tile to a stockpile. Created automatically.
    Haul { pos: Pos },
    /// Channel this ability on this tile (`data/bending.toml`). Needs a unit with enough chi.
    Bend { ability: AbilityId, pos: Pos },
    /// Haul a building's cost to this tile and construct it (`data/buildings.toml`, #64).
    Build { pos: Pos },
    /// Work a study hall's seat, adding knowledge to the research pool each tick it's worked
    /// (`data/buildings.toml`, `data/research.toml`, #21). Opened in sets of `seats` when a study
    /// hall finishes construction (`world/study.rs`); never completes on its own.
    Study { pos: Pos },
    /// Do one of the endeavor gate's tasks: a bend by a unit of the task's element, on the gate's
    /// site (`world/gate.rs`, #236). `task` is its place in `GateSite::tasks`, which saves name
    /// by id. Queued automatically once the task's materials are in.
    EndeavorTask { task: u8 },
    /// Stand on this tile of the gate's plaza during the Harmony Rite (`world/gate.rs`, #238): a
    /// low-priority job for a settler with nothing more urgent, so `tick_rite` can count how
    /// many are attending. Opened on every tile of the plaza the moment the rite stage begins,
    /// and closed with it; never completes on its own, like `Study`.
    Attend { pos: Pos },
    /// Work a production building's seat (the forge's smith job, a brick kiln, a smelter;
    /// `data/buildings.toml`, `BuildingDef::recipe`, #124): fetch the recipe's inputs, then work
    /// inside to turn them into its output. Opened in sets of `capacity` when such a building
    /// finishes construction (`world/forge.rs`), mirroring a study hall's seats; never completes
    /// on its own, it just starts its next batch.
    Craft { pos: Pos },
    /// Plant an empty farm plot (`data/buildings.toml`, `[building.farm]`, issue #198), opened
    /// by `Command::Sow`.
    Sow { pos: Pos },
    /// Gather a farm plot's ripe crop (issue #198), queued automatically once it ripens
    /// (`world/farm.rs`).
    Reap { pos: Pos },
    /// Remove the resource on this tile for good: uproot a renewable plant (`HarvestDef::clear`)
    /// or work a non-renewable's ordinary harvest, which already clears it
    /// (`docs/design/clearing.md`, issue #411/#424). Opened by `Command::DesignateClear`.
    Clear { pos: Pos },
    /// Work a Map Room's seat (`data/buildings.toml`, `BuildingDef::chart`, issue #402,
    /// docs/design/cartography.md): each finished chart uncovers the nearest lost ruin the
    /// settlement hasn't found yet, among the ones `pos` is still in range of. Opened in sets of
    /// `seats` when a Map Room finishes construction (`world/chart.rs`), mirroring a study
    /// hall's `Study` job; never completes on its own.
    Chart { pos: Pos },
    /// Take down the finished building anchored on this tile (issue #342,
    /// `world/deconstruct.rs`): a unit works it from beside (or inside) the building, and when
    /// the work is done the whole footprint clears at once and part of its cost drops by the
    /// anchor (`[deconstruct]` in data/buildings.toml). Opened by `Command::Deconstruct`.
    Deconstruct { pos: Pos },
    /// Carry fuel from stores to the fuel bunker of the building anchored on this tile
    /// (`BuildingDef::fuel`, `world/fuel.rs`): opened automatically once the bunker is at or
    /// below half full, and done when one trip's worth is in.
    Refuel { pos: Pos },
    /// Carry goods from stores to the caravan camp anchored on this tile for offer `slot` of
    /// the visit (`world/trade.rs`, #437): opened while a taken deal or request still needs
    /// goods, and done when one trip's worth is in.
    Trade { pos: Pos, slot: u8 },
    /// Carry goods from stores to the Airship Dock anchored on this tile for deal `slot` of its
    /// planned voyage (`world/voyage.rs`, #758): opened while the slot still needs goods, and
    /// done when one trip's worth is in.
    Load { pos: Pos, slot: u8 },
    /// A place in the crew of the Airship Dock anchored on this tile (`world/voyage.rs`, #758):
    /// walk to the door and wait there. Opened in sets of `crew` once the voyage is loaded and
    /// fuelled; the airship sails when the whole crew is at the door.
    Crew { pos: Pos },
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Job {
    pub id: JobId,
    pub kind: JobKind,
    /// The unit that has taken the job, which may have set it aside to eat or sleep.
    pub claimed_by: Option<UnitId>,
    /// Work done so far, in work-seconds. Kept when the unit is pulled away, so the job resumes.
    pub progress: f32,
    /// A joint bend (`AbilityDef::nations`, #759) has two slots, one per nation, each claimed
    /// separately. `claimed_by` is the first nation's slot and `partner` the second's. A job that
    /// isn't joint never has a `partner`.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub joint: bool,
    /// The second slot's caster, for a `joint` job.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub partner: Option<UnitId>,
}

impl Job {
    /// Whether slot `slot` (0 for `claimed_by`, 1 for `partner`) can still be taken.
    pub fn slot_open(&self, slot: usize) -> bool {
        match slot {
            0 => self.claimed_by.is_none(),
            _ => self.joint && self.partner.is_none(),
        }
    }

    /// Whether any slot of the job can still be taken.
    pub fn is_open(&self) -> bool {
        self.slot_open(0) || self.slot_open(1)
    }

    /// Whether `unit` holds one of the job's slots.
    pub fn held_by(&self, unit: UnitId) -> bool {
        self.claimed_by == Some(unit) || (self.joint && self.partner == Some(unit))
    }

    /// Every unit holding a slot, the first slot's first.
    pub fn holders(&self) -> impl Iterator<Item = UnitId> {
        self.claimed_by.into_iter().chain(self.partner.filter(|_| self.joint))
    }

    /// Frees whichever slot `unit` holds, keeping the progress.
    pub fn release(&mut self, unit: UnitId) {
        if self.claimed_by == Some(unit) {
            self.claimed_by = None;
        }
        if self.partner == Some(unit) {
            self.partner = None;
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::defs::{Defs, DefsError};

    const BIOMES: &str = include_str!("../../../data/biomes.toml");
    const RESOURCES: &str = include_str!("../../../data/resources.toml");
    const NATIONS: &str = include_str!("../../../data/nations.toml");
    const RESEARCH: &str = include_str!("../../../data/research.toml");
    const NEEDS: &str = include_str!("../../../data/needs.toml");
    const JOBS: &str = include_str!("../../../data/jobs.toml");
    const BENDING: &str = include_str!("../../../data/bending.toml");
    const BUILDINGS: &str = include_str!("../../../data/buildings.toml");

    /// The bundled data, with `tech` appended to `data/research.toml` and `resource` appended to
    /// `data/resources.toml` (so a test can name a resource with no harvest entry).
    fn load(tech: &str, resource: &str) -> Result<Defs, DefsError> {
        Defs::from_toml(
            BIOMES,
            &format!("{RESOURCES}\n{resource}"),
            NATIONS,
            &format!("{RESEARCH}\n{tech}"),
            NEEDS,
            JOBS,
            BENDING,
            BUILDINGS,
        )
    }

    /// A tech named `id` that unlocks `jobs` (a TOML array literal, e.g. `"['stone']"`).
    fn tech(id: &str, jobs: &str) -> String {
        format!(
            "[[tech]]\nid = '{id}'\nname = 'T'\ndescription = ''\ncost = 1\n\
             [tech.unlocks]\njobs = {jobs}\n"
        )
    }

    /// The bundled data, with `jobs` in place of `data/jobs.toml`.
    fn load_jobs(jobs: &str) -> Result<Defs, DefsError> {
        Defs::from_toml(BIOMES, RESOURCES, NATIONS, RESEARCH, NEEDS, jobs, BENDING, BUILDINGS)
    }

    #[test]
    fn a_tech_records_the_job_it_unlocks() {
        let defs = Defs::bundled();
        let fishing = defs.tech_id("fishing").unwrap();
        let fish_spot = defs.resource_id("fish_spot").unwrap();
        assert_eq!(defs.harvest(fish_spot).unwrap().unlocked_by, Some(fishing));
        let stone = defs.resource_id("stone").unwrap();
        let stoneworking = defs.tech_id("stoneworking").unwrap();
        assert_eq!(defs.harvest(stone).unwrap().unlocked_by, Some(stoneworking));
        // Gold mining is prospecting's, the same way (#383): both its surface vein and the
        // buried seam Seismic Sense uncovers.
        let prospecting = defs.tech_id("prospecting").unwrap();
        for resource in ["gold_vein", "gold_seam"] {
            let id = defs.resource_id(resource).unwrap();
            assert_eq!(defs.harvest(id).unwrap().unlocked_by, Some(prospecting), "{resource}");
        }
        // Chopping needs no tech.
        let oak = defs.resource_id("oak_tree").unwrap();
        assert_eq!(defs.harvest(oak).unwrap().unlocked_by, None);
    }

    #[test]
    fn a_tech_may_only_unlock_jobs_whose_resource_exists() {
        let err = load(&tech("a", "['nope']"), "").unwrap_err();
        assert!(err.0.contains("unknown resource \"nope\""), "{err}");
    }

    #[test]
    fn a_tech_may_only_unlock_a_resource_with_a_harvest_entry() {
        let driftwood = "[[resource]]\nid = 'driftwood'\nname = 'Driftwood'\nglyph = 'd'\n\
                          chance = 0.1\n";
        let err = load(&tech("a", "['driftwood']"), driftwood).unwrap_err();
        assert!(err.0.contains("no harvest entry"), "{err}");
    }

    #[test]
    fn a_job_has_at_most_one_tech() {
        // `stone` is already `stoneworking`'s, so a second tech can't take it.
        let taken = load(&tech("a", "['stone']"), "");
        assert!(taken.unwrap_err().0.contains("one tech"));
        // Chopping is free in the bundled data, so these tests claim it themselves.
        let two = load(&format!("{}{}", tech("a", "['oak_tree']"), tech("b", "['oak_tree']")), "");
        assert!(two.unwrap_err().0.contains("one tech"));
        let twice = load(&tech("a", "['oak_tree', 'oak_tree']"), "");
        assert!(twice.unwrap_err().0.contains("listed twice"));
    }

    /// `stands_for` (issue #371: brick pays for stone): not itself, must name a known item, no
    /// chains, and at most one item may stand for a given target.
    #[test]
    fn stands_for_is_validated() {
        let self_ref = JOBS.replace("stands_for = \"stone\"", "stands_for = \"brick\"");
        let err = load_jobs(&self_ref).unwrap_err();
        assert!(err.0.contains("can't name itself"), "{err}");

        let unknown = JOBS.replace("stands_for = \"stone\"", "stands_for = \"mithril\"");
        let err = load_jobs(&unknown).unwrap_err();
        assert!(err.0.contains("unknown item"), "{err}");

        let chain = format!(
            "{JOBS}\n[[item]]\nid = \"super_brick\"\nname = \"Super Brick\"\n\
             stands_for = \"brick\"\n"
        );
        let err = load_jobs(&chain).unwrap_err();
        assert!(err.0.contains("chain"), "{err}");

        let twice = format!(
            "{JOBS}\n[[item]]\nid = \"rubble\"\nname = \"Rubble\"\n\
             stands_for = \"stone\"\n"
        );
        let err = load_jobs(&twice).unwrap_err();
        assert!(err.0.contains("only one may"), "{err}");

        // Valid: the bundled data's own brick -> stone.
        assert!(load_jobs(JOBS).is_ok());
    }

    /// `risk` (issue #388): bundled values, and the `[0, 1)` bound.
    #[test]
    fn risk_is_validated() {
        let defs = load_jobs(JOBS).unwrap();
        for (resource, risk) in [
            ("oak_tree", 0.01),
            ("fir_tree", 0.01),
            ("stone", 0.02),
            ("boulder", 0.02),
            ("gold_vein", 0.03),
            ("gold_seam", 0.03),
        ] {
            let id = defs.resource_id(resource).unwrap();
            assert_eq!(defs.harvest(id).unwrap().risk, risk, "{resource}");
        }
        // Recipes carry no risk.
        let ancient_scroll = defs.resource_id("ancient_scroll").unwrap();
        assert_eq!(defs.harvest(ancient_scroll).unwrap().risk, 0.0);

        let too_high =
            JOBS.replacen("tunnel_count = 14\nrisk = 0.02", "tunnel_count = 14\nrisk = 1.0", 1);
        let err = load_jobs(&too_high).unwrap_err();
        assert!(err.0.contains("risk"), "{err}");

        let negative =
            JOBS.replacen("tunnel_count = 14\nrisk = 0.02", "tunnel_count = 14\nrisk = -0.1", 1);
        let err = load_jobs(&negative).unwrap_err();
        assert!(err.0.contains("risk"), "{err}");
    }

    /// What a tool's preview calls a resource (#657): the data's own noun where it names one,
    /// else the resource's name in lower case and with an "s".
    #[test]
    fn a_harvest_names_what_it_counts_as() {
        let defs = load_jobs(JOBS).unwrap();
        let noun = |resource: &str| {
            let h = defs.harvest(defs.resource_id(resource).unwrap()).unwrap();
            (h.noun.clone(), h.noun_many.clone())
        };
        assert_eq!(noun("oak_tree"), ("tree".to_owned(), "trees".to_owned()));
        assert_eq!(noun("gold_vein"), ("rock".to_owned(), "rock".to_owned()));
        assert_eq!(noun("fish_spot"), ("fishing spot".to_owned(), "fishing spots".to_owned()));
    }

    /// `[harvest.clear]` (the Clear designation, `docs/design/clearing.md`, issue #411/#424):
    /// the bundled numbers, and validation (only on a harvest with `regrow`, `work` above 0,
    /// `item` known, `count` only with an `item`).
    #[test]
    fn clear_is_validated() {
        let defs = load_jobs(JOBS).unwrap();
        let wood = defs.item_id("wood").unwrap();

        let cabbage = defs.resource_id("cabbage").unwrap();
        let c = defs.harvest(cabbage).unwrap().clear.unwrap();
        assert_eq!((c.work, c.item, c.count, c.risk), (1.0, None, 0, 0.0));

        let fruit_tree = defs.resource_id("fruit_tree").unwrap();
        let c = defs.harvest(fruit_tree).unwrap().clear.unwrap();
        assert_eq!((c.work, c.item, c.count, c.risk), (6.0, Some(wood), 2, 0.01));

        // Non-renewables and the fish spot have no `[harvest.clear]` at all.
        for resource in ["oak_tree", "stone", "ancient_scroll", "fish_spot"] {
            let id = defs.resource_id(resource).unwrap();
            assert_eq!(defs.harvest(id).unwrap().clear, None, "{resource}");
        }

        // Only allowed on a harvest with `regrow`.
        let no_regrow = JOBS.replacen(
            "resource = \"oak_tree\"\nverb = \"Chop\"\nnoun = \"tree\"\nitem = \"wood\"\n\
             count = 3\nwork = 8.0\nrisk = 0.01\n",
            "resource = \"oak_tree\"\nverb = \"Chop\"\nnoun = \"tree\"\nitem = \"wood\"\n\
             count = 3\nwork = 8.0\nrisk = 0.01\n\n[harvest.clear]\nwork = 1.0\n",
            1,
        );
        let err = load_jobs(&no_regrow).unwrap_err();
        assert!(err.0.contains("clear needs a regrow time"), "{err}");

        // `work` must be positive.
        let bad_work =
            JOBS.replacen("[harvest.clear]\nwork = 1.0\n", "[harvest.clear]\nwork = 0.0\n", 1);
        let err = load_jobs(&bad_work).unwrap_err();
        assert!(err.0.contains("clear.work"), "{err}");

        // `count` needs an `item`.
        let count_no_item = JOBS.replacen(
            "[harvest.clear]\nwork = 1.0\n",
            "[harvest.clear]\nwork = 1.0\ncount = 1\n",
            1,
        );
        let err = load_jobs(&count_no_item).unwrap_err();
        assert!(err.0.contains("clear.count needs an item"), "{err}");

        // `item` needs a `count`.
        let item_no_count = JOBS.replacen(
            "[harvest.clear]\nwork = 1.0\n",
            "[harvest.clear]\nwork = 1.0\nitem = \"wood\"\n",
            1,
        );
        let err = load_jobs(&item_no_count).unwrap_err();
        assert!(err.0.contains("clear.item needs a count"), "{err}");

        // `item` must be a known item.
        let unknown_item = JOBS.replacen(
            "item = \"wood\"\ncount = 2\nrisk = 0.01",
            "item = \"mithril\"\ncount = 2\nrisk = 0.01",
            1,
        );
        let err = load_jobs(&unknown_item).unwrap_err();
        assert!(err.0.contains("clear: unknown item"), "{err}");

        // `risk` must be in [0, 1).
        let bad_risk = JOBS.replacen(
            "item = \"wood\"\ncount = 2\nrisk = 0.01",
            "item = \"wood\"\ncount = 2\nrisk = 1.0",
            1,
        );
        let err = load_jobs(&bad_risk).unwrap_err();
        assert!(err.0.contains("clear.risk"), "{err}");
    }
}

/// A tech's upgrade to an item (`[[tech.unlocks.upgrades]]` with `item = ...`, docs/design/
/// horizons-phase-4.md, "New upgrade properties"): once `tech` is known, `item` pays for
/// `stands_for` one for one, as brick pays for stone (`World::stands_for`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ItemUpgrade {
    pub tech: crate::research::TechId,
    pub item: ItemId,
    pub stands_for: ItemId,
}

/// Checks each item upgrade: the item exists and has no `stands_for` of its own, it names
/// exactly one property (`stands_for`), that names another item which isn't itself a stand-in,
/// and each item is upgraded by at most one tech.
pub(crate) fn resolve_item_upgrades(
    techs: &[crate::research::TechDef],
    items: &[ItemDef],
) -> Result<Vec<ItemUpgrade>, DefsError> {
    let mut out: Vec<ItemUpgrade> = Vec::new();
    for (t, tech) in techs.iter().enumerate() {
        let err =
            |msg: String| Err(DefsError(format!("tech {}: unlocks.upgrades: {msg}", tech.id)));
        for up in &tech.unlocks.upgrades {
            let Some(name) = &up.item else { continue };
            let Some(i) = items.iter().position(|it| it.id == *name) else {
                return err(format!("unknown item {name:?}"));
            };
            if up.seats.is_some() || up.recipe.is_some() {
                return err(format!("item {name:?} can only take `stands_for`"));
            }
            let Some(target) = &up.stands_for else {
                return err(format!("item {name:?} must name one property: stands_for"));
            };
            if items[i].stands_for.is_some() {
                return err(format!("item {name:?} already stands for something"));
            }
            let Some(k) = items.iter().position(|it| it.id == *target) else {
                return err(format!("stands_for names unknown item {target:?}"));
            };
            if k == i {
                return err(format!("item {name:?} can't stand for itself"));
            }
            if items[k].stands_for.is_some() {
                return err(format!("{target:?} itself stands for something"));
            }
            if let Some(o) = out.iter().find(|o| o.item.0 as usize == i) {
                return err(format!(
                    "item {name:?} is already upgraded by {:?}; a property has one tech",
                    techs[o.tech.0 as usize].id
                ));
            }
            out.push(ItemUpgrade {
                tech: crate::research::TechId(t as u16),
                item: ItemId(i as u16),
                stands_for: ItemId(k as u16),
            });
        }
    }
    Ok(out)
}
