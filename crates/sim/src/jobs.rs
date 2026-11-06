//! Items, harvest yields and job settings (`data/jobs.toml`), plus the plain types the job queue
//! is made of. The behaviour (claiming, pre-emption, hauling) lives in `world::work`.
//!
//! Items are one id space: the `[[item]]` materials in `jobs.toml` first, then every food from
//! `needs.toml` (so a harvested cabbage is both a food and something you can stockpile).

use serde::Deserialize;

use crate::bending::AbilityId;
use crate::defs::{DefsError, ResourceDef, ResourceId};
use crate::geom::Pos;
use crate::nation::Modifiers;
use crate::needs::{FoodDef, NeedsSettings};
use crate::research::{TechDef, TechId};
use crate::world::UnitId;

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, serde::Serialize, serde::Deserialize,
)]
pub struct ItemId(pub u16);

#[derive(Debug, Clone, PartialEq)]
pub struct ItemDef {
    pub id: String,
    pub name: String,
    /// An icon identifier for a future graphical frontend. The sim never reads it; it is data so
    /// which picture an item uses isn't a match in frontend code.
    pub icon: String,
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
    icon: String,
    #[serde(default)]
    knowledge: Option<u32>,
    #[serde(default)]
    stands_for: Option<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawHarvest {
    resource: String,
    verb: String,
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

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct JobsFile {
    settings: JobSettings,
    item: Vec<RawItem>,
    harvest: Vec<RawHarvest>,
}

impl JobsFile {
    /// Checks the data and links names to ids.
    pub(crate) fn resolve(
        self,
        resources: &[ResourceDef],
        foods: &[FoodDef],
        needs: &NeedsSettings,
        techs: &[TechDef],
    ) -> Result<(JobSettings, Vec<ItemDef>, Vec<HarvestDef>), DefsError> {
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
                icon: i.icon,
                food: None,
                knowledge: i.knowledge,
                stands_for: None,
            })
            .collect();
        items.extend(foods.iter().enumerate().map(|(n, f)| ItemDef {
            id: f.id.clone(),
            name: f.name.clone(),
            icon: f.icon.clone(),
            food: Some(n),
            knowledge: None,
            stands_for: None,
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
            harvests.push(HarvestDef {
                resource: ResourceId(resource as u16),
                verb: h.verb,
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
            });
        }
        link_unlocks(&mut harvests, resources, techs)?;
        Ok((s, items, harvests))
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
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Job {
    pub id: JobId,
    pub kind: JobKind,
    /// The unit that has taken the job, which may have set it aside to eat or sleep.
    pub claimed_by: Option<UnitId>,
    /// Work done so far, in work-seconds. Kept when the unit is pulled away, so the job resumes.
    pub progress: f32,
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
            "{JOBS}\n[[item]]\nid = \"super_brick\"\nicon = \"brick\"\nname = \"Super Brick\"\n\
             stands_for = \"brick\"\n"
        );
        let err = load_jobs(&chain).unwrap_err();
        assert!(err.0.contains("chain"), "{err}");

        let twice = format!(
            "{JOBS}\n[[item]]\nid = \"rubble\"\nicon = \"stone\"\nname = \"Rubble\"\n\
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
            "resource = \"oak_tree\"\nverb = \"Chop\"\nitem = \"wood\"\ncount = 4\nwork = 8.0\n\
             risk = 0.01\n",
            "resource = \"oak_tree\"\nverb = \"Chop\"\nitem = \"wood\"\ncount = 4\nwork = 8.0\n\
             risk = 0.01\n\n[harvest.clear]\nwork = 1.0\n",
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
