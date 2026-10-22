//! Items, harvest yields and job settings (`data/jobs.toml`), plus the plain types the job queue
//! is made of. The behaviour (claiming, pre-emption, hauling) lives in `world::work`.
//!
//! Items are one id space: the `[[item]]` materials in `jobs.toml` first, then every food from
//! `needs.toml` (so a harvested cabbage is both a food and something you can stockpile).

use serde::Deserialize;

use crate::defs::{DefsError, ResourceDef, ResourceId};
use crate::geom::Pos;
use crate::nation::Modifiers;
use crate::needs::{FoodDef, NeedsSettings};
use crate::world::UnitId;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ItemId(pub u16);

#[derive(Debug, Clone, PartialEq)]
pub struct ItemDef {
    pub id: String,
    pub name: String,
    /// Index into `Defs::foods` if this item can be eaten.
    pub food: Option<usize>,
}

/// A number of one kind of item.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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
            harvests.push(HarvestDef {
                resource: ResourceId(resource as u16),
                verb: h.verb,
                item: ItemId(item as u16),
                count: h.count,
                work: h.work,
                modifier: h.modifier,
            });
        }
        Ok((s, items, harvests))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct JobId(pub u32);

/// Player-designated work, waiting for a unit. Needs (eat, drink, sleep) are not jobs: units
/// handle those themselves.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JobKind {
    /// Work the resource on this tile until it yields its items (chop, quarry, gather, mine).
    Harvest { pos: Pos },
    /// Carry the item stack lying on this tile to a stockpile. Created automatically.
    Haul { pos: Pos },
}

#[derive(Debug, Clone, PartialEq)]
pub struct Job {
    pub id: JobId,
    pub kind: JobKind,
    /// The unit that has taken the job, which may have set it aside to eat or sleep.
    pub claimed_by: Option<UnitId>,
    /// Work done so far, in work-seconds. Kept when the unit is pulled away, so the job resumes.
    pub progress: f32,
}
