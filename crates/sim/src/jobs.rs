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
    /// Index into `Defs::foods` if this item can be eaten.
    pub food: Option<usize>,
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
    /// The tech that must be known before this can be designated or foraged
    /// (`data/research.toml`, `Unlocks::jobs`, issue #22). `None` means every nation can work it
    /// from the start.
    pub unlocked_by: Option<TechId>,
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
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawItem {
    id: String,
    name: String,
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
        ] {
            if !(v.is_finite() && v > 0.0) {
                return err(format!("settings.{what} must be a positive number"));
            }
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
        let mut items: Vec<ItemDef> =
            self.item.into_iter().map(|i| ItemDef { id: i.id, name: i.name, food: None }).collect();
        items.extend(foods.iter().enumerate().map(|(n, f)| ItemDef {
            id: f.id.clone(),
            name: f.name.clone(),
            food: Some(n),
        }));

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
            harvests.push(HarvestDef {
                resource: ResourceId(resource as u16),
                verb: h.verb,
                item: ItemId(item as u16),
                count: h.count,
                work: h.work,
                modifier: h.modifier,
                regrow: h.regrow,
                unlocked_by: None,
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

    #[test]
    fn a_tech_records_the_job_it_unlocks() {
        let defs = Defs::bundled();
        let fishing = defs.tech_id("fishing").unwrap();
        let fish_spot = defs.resource_id("fish_spot").unwrap();
        assert_eq!(defs.harvest(fish_spot).unwrap().unlocked_by, Some(fishing));
        let stone = defs.resource_id("stone").unwrap();
        let stoneworking = defs.tech_id("stoneworking").unwrap();
        assert_eq!(defs.harvest(stone).unwrap().unlocked_by, Some(stoneworking));
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
}
