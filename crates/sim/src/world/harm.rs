//! Harm: the rules every source of damage beyond neglect shares (`docs/design/harm.md`, issue
//! #387), the first source, exposure, and the third, sickness from sea water (issue #389; the
//! second, injury, is #388's own code).
//!
//! Three things every harm source shares:
//! - A damage scale, the difficulty preset's `harm` knob (`Knobs::harm`: 0 off, Normal 1.0, Hard
//!   1.5). 0 draws nothing at all, so a world with no difficulty preset, or Sandbox, plays
//!   exactly as it did before this module existed.
//! - A floor (`data/harm.toml`'s `[settings] floor`): with the preset's `harm_kills` off, harm
//!   never takes a unit's health below it. The need damage `Needs::tick_at_cold_healing` already
//!   applies is a different, older rule and isn't floored -- a death still needs an empty need
//!   unless `harm_kills` is on.
//! - No natural regen while harmed: `World::tick_needs` passes `regen_allowed: false` into
//!   `Needs::tick_at_cold_healing` while the unit's tile is cold or sick, on top of what this
//!   module does on its own.
//!
//! Exposure is a unit asleep on a cold tile (`World::is_cold`) not sheltered by a bed whose own
//! `sleep_bonus` beats the cold -- a heat source (a campfire, Hearthflame, Inner Fire, an igloo,
//! Earth Tent or Air Shield) already rules a tile out of `is_cold`, so this only ever matters
//! for a plain bed or no shelter at all. It logs `HarmEventKind::SleepingRough` once per sleep,
//! for the TUI follow-up (#393) to turn into an alert and a People flag.
//!
//! Sickness is a drink taken at a spot whose only water neighbours are the sea (`Terrain::Water`),
//! never a river, lake or ford: `World::maybe_catch_sickness` rolls it the tick a `Task::Drink`
//! completes, from its own stream (`rng::stream(seed, "harm.sickness")`, `World::sickness_rng`).
//! `World::plan_drink` steers a thirsty unit around it first, toward fresh water within
//! `data/harm.toml`'s `[sickness] fresh_detour` path tiles of the nearest drinking spot of any
//! kind, while the preset's `harm` is above 0.

use super::*;
use crate::needs::DeathCause;

/// What's harming a unit, for `Unit::last_harm` and `Death::weakened_by` (issue #387): exposure
/// (#387), injury (#388) and sickness (#389).
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum HarmKind {
    /// Asleep, unsheltered, on cold land (`World::apply_exposure`).
    Exposure,
    /// Hurt finishing a heavy pick: felling, quarrying or mining (`World::maybe_injure`, issue
    /// #388).
    Injury,
    /// A sea-water drink that rolled sick (`World::apply_sickness`, issue #389).
    Sickness,
}

impl HarmKind {
    /// The death cause this harm names when it alone -- not a depleted need -- takes a unit to 0
    /// health: only reachable with the difficulty preset's `harm_kills` on, since harm otherwise
    /// floors above 0 (`data/harm.toml`'s `[settings] floor`).
    fn death_cause(self) -> DeathCause {
        match self {
            Self::Exposure => DeathCause::Exposure,
            Self::Injury => DeathCause::Injury,
            Self::Sickness => DeathCause::Sickness,
        }
    }
}

/// A harm event, oldest first (`World::harm_events`), for the TUI's alerts (#393) to turn into a
/// line and a toast; this module only records what happened and to whom.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct HarmEvent {
    pub tick: u64,
    pub unit: UnitId,
    pub pos: Pos,
    pub kind: HarmEventKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum HarmEventKind {
    /// Lay down to sleep rough (cold, unsheltered) and started taking exposure damage. Fires
    /// once per sleep (`World::apply_exposure`), not every tick it continues.
    SleepingRough,
    /// Hurt finishing a heavy pick (`World::maybe_injure`, issue #388). Fires once per hit, not
    /// once per pick.
    Injured,
    /// A sea-water drink rolled sick (`World::maybe_catch_sickness`, issue #389).
    FellSick,
    /// Sickness ran out (`World::apply_sickness`'s own `sick_until`), issue #389.
    Recovered,
    /// A stored vial of spirit water saved a settler who would have died (`World::use_vial`,
    /// #739).
    Revived,
}

/// In-game minutes a death within this long of the last harm names that harm as
/// `Death::weakened_by`, even though a depleted need was the direct cause
/// (`docs/design/harm.md`, "a death names its weakener"). Picked to cover the doc's worst case
/// for a careful colony: an injured, then sick, settler who still has about 10 minutes of
/// hunger left once sickness runs out.
const WEAKENED_BY_MINUTES: f32 = 10.0;

/// The stream sickness rolls draw from (issue #389). A saved world that predates sickness loads
/// with this seeded by 0 instead of the world seed, as `orchard_rng` does for #142; still
/// deterministic.
pub(super) fn default_sickness_rng() -> SimRng {
    rng::stream(0, "harm.sickness")
}

pub(super) fn new_sickness_rng(seed: u64) -> SimRng {
    rng::stream(seed, "harm.sickness")
}

impl World {
    /// The sleep bonus of the bed `u` lies in right now, 1.0 with none: half of exposure's test
    /// (a heat source is the other half, already folded into `cold`/`World::is_cold` by the
    /// caller). A sleeper inside a bed bigger than 1x1 lies on the entrance tile, not the
    /// building itself (#227), so this looks the bed up through `inside`, as `sleep_factor_for`
    /// does.
    pub(super) fn bed_sleep_bonus(&self, u: &Unit, defs: &Defs) -> f32 {
        let bed_tile = u.inside.unwrap_or(u.pos);
        self.building_at(bed_tile).and_then(|b| defs.building(b).bed).map_or(1.0, |b| b.sleep_bonus)
    }

    /// Applies this tick's exposure damage to `u` if it's exposed right now (asleep, on a cold
    /// tile, not in a bed whose own `sleep_bonus` beats the cold), and logs the "lay down rough"
    /// event once per sleep. A no-op with the difficulty preset's `harm` at 0 (Sandbox, or no
    /// preset at all): draws nothing, costs nothing, matches today exactly. `cold` is the
    /// caller's own `World::is_cold`-equivalent check for `u.pos`, computed once per unit
    /// alongside the needs tick it already does that for. Returns whether `u` was exposed (took
    /// damage) this tick, for `World::care`'s `cold_minutes`.
    pub(super) fn apply_exposure(&mut self, u: &mut Unit, cold: bool, defs: &Defs) -> bool {
        let knobs = self.knobs(defs);
        let (harm_scale, kills) = (knobs.harm, knobs.harm_kills);
        // A bath keeps the cold off for a while (#741).
        let warm = self.tick < u.warm_until;
        let exposed = harm_scale > 0.0
            && cold
            && !warm
            && u.is_sleeping()
            && self.bed_sleep_bonus(u, defs) <= 1.0;
        if !exposed {
            u.exposed_alerted = false;
            return false;
        }
        if !u.exposed_alerted {
            u.exposed_alerted = true;
            self.log_event(HarmEvent {
                tick: self.tick,
                unit: u.id,
                pos: u.pos,
                kind: HarmEventKind::SleepingRough,
            });
        }
        let per_tick =
            defs.harm.exposure.damage_per_minute * harm_scale / (60.0 * TICKS_PER_SECOND as f32);
        let floor = defs.harm.settings.floor;
        u.needs.health = if kills {
            (u.needs.health - per_tick).max(0.0)
        } else {
            (u.needs.health - per_tick).max(floor)
        };
        u.last_harm = Some((HarmKind::Exposure, self.tick));
        true
    }

    /// Whether `u` is sick right now (issue #389): used by `World::tick_needs` to hold off
    /// natural regen on top of what `apply_sickness` itself does.
    pub(super) fn is_sick(&self, u: &Unit) -> bool {
        u.sick_until.is_some_and(|until| self.tick < until)
    }

    /// Applies this tick's sickness damage to `u`, and clears it (logging
    /// `HarmEventKind::Recovered`) once `Unit::sick_until` is reached. A no-op for a unit that
    /// isn't sick, and with the difficulty preset's `harm` at 0 (Sandbox, or no preset at all):
    /// draws nothing, costs nothing, matches today exactly -- sickness can't be caught in the
    /// first place while `harm` is 0 (`World::maybe_catch_sickness`), so this only ever sees
    /// that case for a sickness caught before the preset changed under it.
    pub(super) fn apply_sickness(&mut self, u: &mut Unit, defs: &Defs) {
        let Some(until) = u.sick_until else { return };
        let knobs = self.knobs(defs);
        if knobs.harm <= 0.0 {
            return;
        }
        if self.tick >= until {
            u.sick_until = None;
            self.log_event(HarmEvent {
                tick: self.tick,
                unit: u.id,
                pos: u.pos,
                kind: HarmEventKind::Recovered,
            });
            return;
        }
        let per_tick =
            defs.harm.sickness.damage_per_minute * knobs.harm / (60.0 * TICKS_PER_SECOND as f32);
        let floor = defs.harm.settings.floor;
        u.needs.health = if knobs.harm_kills {
            (u.needs.health - per_tick).max(0.0)
        } else {
            (u.needs.health - per_tick).max(floor)
        };
        u.last_harm = Some((HarmKind::Sickness, self.tick));
    }

    /// The sickness chance a drink at `pos` rolls, or `None` if it shouldn't roll at all: `pos`
    /// must have at least one water neighbour, and *every* water neighbour must be a terrain
    /// `data/harm.toml`'s `[[sickness.water]]` lists (the sea; a river, lake or ford beside the
    /// same spot rules it safe). The highest chance among the terrains present, when more than
    /// one matches.
    pub(super) fn sickness_chance(&self, pos: Pos, defs: &Defs) -> Option<f32> {
        // A barrel at the door is fresh water, whatever the shore beside it is (`outpost.rs`).
        if self.is_barrel_door(pos, defs) {
            return None;
        }
        // So is a flowing aqueduct beside it (`aqueduct.rs`, #755).
        if self.flow(defs).beside(&self.map, pos) {
            return None;
        }
        let mut chance: Option<f32> = None;
        for q in self.map.neighbors(pos, true) {
            let terrain = self.map[q].terrain;
            if !terrain.is_water() {
                continue;
            }
            let entry = defs.harm.sickness.water.iter().find(|w| w.terrain == terrain)?;
            chance = Some(chance.map_or(entry.chance, |c: f32| c.max(entry.chance)));
        }
        chance
    }

    /// Rolls sickness for unit `i`'s drink just finished at its current position
    /// (`World::consume`'s `Task::Drink`), from `World::sickness_rng`. A no-op with the
    /// difficulty preset's `harm` at 0 (`World::plan_drink` and this both gate on it, so a
    /// `difficulty == None` world draws nothing and matches today exactly), for a unit already
    /// sick (sickness doesn't stack or restart its clock), or at a spot with no unlisted-safe
    /// water neighbour that still isn't entirely sea (`World::sickness_chance`).
    pub(super) fn maybe_catch_sickness(&mut self, i: usize, defs: &Defs) {
        if self.knobs(defs).harm <= 0.0 || self.units[i].sick_until.is_some() {
            return;
        }
        let Some(chance) = self.sickness_chance(self.units[i].pos, defs) else {
            return;
        };
        if self.sickness_rng.random::<f32>() >= chance {
            return;
        }
        let ticks = (defs.harm.sickness.minutes * 60.0 * TICKS_PER_SECOND as f32).round() as u64;
        let u = &mut self.units[i];
        u.sick_until = Some(self.tick + ticks);
        let (unit, pos) = (u.id, u.pos);
        self.log_event(HarmEvent { tick: self.tick, unit, pos, kind: HarmEventKind::FellSick });
    }

    /// What `Death::weakened_by` should say for a unit whose `last_harm` was `last_harm`, dying
    /// of a depleted need (`cause`) at `now`: the harm, if it struck within
    /// `WEAKENED_BY_MINUTES` of `now`, else `None`. `cause` already being a `HarmKind`'s own
    /// (`DeathCause::Exposure`, say) always answers `None`: that death already names its cause
    /// directly, so repeating it as "weakened by" would be redundant.
    pub(super) fn weakened_by(
        cause: DeathCause,
        last_harm: Option<(HarmKind, u64)>,
        now: u64,
    ) -> Option<HarmKind> {
        if !matches!(
            cause,
            DeathCause::Starvation | DeathCause::Dehydration | DeathCause::Exhaustion
        ) {
            return None;
        }
        let (kind, when) = last_harm?;
        let window = (WEAKENED_BY_MINUTES * 60.0 * TICKS_PER_SECOND as f32) as u64;
        (now.saturating_sub(when) <= window).then_some(kind)
    }

    /// The cause a dead unit's death names: the first depleted need (`Needs::depleted`'s own
    /// priority), or, with none, the harm that took it to 0 directly -- only reachable with the
    /// difficulty preset's `harm_kills` on, since harm otherwise floors above 0.
    pub(super) fn death_cause(u: &Unit) -> DeathCause {
        u.needs.depleted().unwrap_or_else(|| {
            u.last_harm.map(|(k, _)| k.death_cause()).unwrap_or(DeathCause::Starvation)
        })
    }
}

#[cfg(test)]
mod tests;
