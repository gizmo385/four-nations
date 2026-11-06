//! Harm: the rules every source of damage beyond neglect shares (`docs/design/harm.md`, issue
//! #387), and the first source, exposure.
//!
//! Three things every harm source will share (today just exposure; injury and sickness are
//! later issues' own code):
//! - A damage scale, the difficulty preset's `harm` knob (`Knobs::harm`: 0 off, Normal 1.0, Hard
//!   1.5). 0 draws nothing at all, so a world with no difficulty preset, or Sandbox, plays
//!   exactly as it did before this module existed.
//! - A floor (`data/harm.toml`'s `[settings] floor`): with the preset's `harm_kills` off, harm
//!   never takes a unit's health below it. The need damage `Needs::tick_at_cold_healing` already
//!   applies is a different, older rule and isn't floored -- a death still needs an empty need
//!   unless `harm_kills` is on.
//! - No natural regen while harmed: `World::tick_needs` passes `regen_allowed: false` into
//!   `Needs::tick_at_cold_healing` while the unit's tile is cold, on top of what this module
//!   does on its own.
//!
//! Exposure is a unit asleep on a cold tile (`World::is_cold`) not sheltered by a bed whose own
//! `sleep_bonus` beats the cold -- a heat source (a campfire, Hearthflame, Inner Fire, an igloo,
//! Earth Tent or Air Shield) already rules a tile out of `is_cold`, so this only ever matters
//! for a plain bed or no shelter at all. It logs `HarmEventKind::SleepingRough` once per sleep,
//! for the TUI follow-up (#393) to turn into an alert and a People flag.

use super::*;
use crate::needs::DeathCause;

/// What's harming a unit, for `Unit::last_harm` and `Death::weakened_by` (issue #387). More
/// variants arrive with sickness (#389).
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum HarmKind {
    /// Asleep, unsheltered, on cold land (`World::apply_exposure`).
    Exposure,
    /// Hurt finishing a heavy pick: felling, quarrying or mining (`World::maybe_injure`, issue
    /// #388).
    Injury,
}

impl HarmKind {
    /// The death cause this harm names when it alone -- not a depleted need -- takes a unit to 0
    /// health: only reachable with the difficulty preset's `harm_kills` on, since harm otherwise
    /// floors above 0 (`data/harm.toml`'s `[settings] floor`).
    fn death_cause(self) -> DeathCause {
        match self {
            Self::Exposure => DeathCause::Exposure,
            Self::Injury => DeathCause::Injury,
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
}

/// In-game minutes a death within this long of the last harm names that harm as
/// `Death::weakened_by`, even though a depleted need was the direct cause
/// (`docs/design/harm.md`, "a death names its weakener"). Picked to cover the doc's worst case
/// for a careful colony: an injured, then sick, settler who still has about 10 minutes of
/// hunger left once sickness runs out.
const WEAKENED_BY_MINUTES: f32 = 10.0;

impl World {
    /// Harm events, oldest first.
    pub fn harm_events(&self) -> &[HarmEvent] {
        &self.harm_events
    }

    /// The sleep bonus of the bed `u` lies in right now, 1.0 with none: half of exposure's test
    /// (a heat source is the other half, already folded into `cold`/`World::is_cold` by the
    /// caller). A sleeper inside a bed bigger than 1x1 lies on the entrance tile, not the
    /// building itself (#227), so this looks the bed up through `inside`, as `sleep_factor_for`
    /// does.
    pub(super) fn bed_sleep_bonus(&self, u: &Unit, defs: &Defs) -> f32 {
        let bed_tile = u.inside.unwrap_or(u.pos);
        self.map[bed_tile]
            .building
            .and_then(|b| defs.building(b.id).bed)
            .map_or(1.0, |b| b.sleep_bonus)
    }

    /// Applies this tick's exposure damage to `u` if it's exposed right now (asleep, on a cold
    /// tile, not in a bed whose own `sleep_bonus` beats the cold), and logs the "lay down rough"
    /// event once per sleep. A no-op with the difficulty preset's `harm` at 0 (Sandbox, or no
    /// preset at all): draws nothing, costs nothing, matches today exactly. `cold` is the
    /// caller's own `World::is_cold`-equivalent check for `u.pos`, computed once per unit
    /// alongside the needs tick it already does that for.
    pub(super) fn apply_exposure(&mut self, u: &mut Unit, cold: bool, defs: &Defs) {
        let knobs = self.knobs(defs);
        let (harm_scale, kills) = (knobs.harm, knobs.harm_kills);
        let exposed =
            harm_scale > 0.0 && cold && u.is_sleeping() && self.bed_sleep_bonus(u, defs) <= 1.0;
        if !exposed {
            u.exposed_alerted = false;
            return;
        }
        if !u.exposed_alerted {
            u.exposed_alerted = true;
            self.harm_events.push(HarmEvent {
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
