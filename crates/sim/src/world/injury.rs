//! Injury: heavy work (issue #388, `docs/design/harm.md` "Injury: heavy work").
//!
//! A harvest marked `risk` in `data/jobs.toml` (felling, quarrying, mining) has a chance of
//! hurting the worker when a pick finishes: `risk` divided by the harvest's own `modifier`
//! (earthworks for stone and ore, 1.0 with none), so Earth's earthworks lowers the rate and
//! Air's weaker one raises it -- there's no `if nation` anywhere in this module. The roll is
//! drawn once per finished pick, in unit-id order on the completing tick (`World::act` already
//! walks units in that order), from its own stream (`rng::stream(seed, "harm.injury")`), and
//! only while the difficulty preset's `harm` knob is above 0: a world with no difficulty preset,
//! or Sandbox, draws nothing, matching #387's rule for every harm source.
//!
//! A unit below `data/harm.toml`'s `[settings] fit_for_heavy` doesn't claim a harvest with a
//! `risk` at all (`claim_job`, `idle_reason`, both in `world/work.rs`), which is what makes an
//! injury (or any other health loss) take a worker off heavy labour until it heals.

use super::*;
use crate::jobs::HarvestDef;

/// The stream injury rolls draw from. A save that predates #388 loads this seeded by 0 instead
/// of the world seed, as `orchard::default_rng` does for #142; still deterministic.
pub(super) fn default_rng() -> SimRng {
    rng::stream(0, "harm.injury")
}

pub(super) fn new_rng(seed: u64) -> SimRng {
    rng::stream(seed, "harm.injury")
}

impl World {
    /// Whether `u` is fit to take on a harvest marked `risk`: not below `[settings]
    /// fit_for_heavy`. Only gates claiming while harm is actually in play (the difficulty
    /// preset's `harm` above 0), so a world with no difficulty preset, or Sandbox, claims
    /// exactly as it did before #388.
    pub(super) fn fit_for_heavy_work(&self, u: &Unit, defs: &Defs) -> bool {
        self.knobs(defs).harm <= 0.0 || u.needs.health >= defs.harm.settings.fit_for_heavy
    }

    /// Rolls once for an injury on the pick of `h` that unit `i` just finished at `pos`
    /// (`run_harvest`): drawn only while harm is in play and this harvest has a `risk`, at a
    /// chance of `risk` divided by the harvest's own `modifier` (the unit's own effective
    /// modifiers, `modifiers_for`, #239, not necessarily the settlement's). On a hit the unit
    /// loses `[injury] damage` times the preset's `harm`, subject to #387's floor (or killed
    /// outright with `harm_kills`), logs a `HarmEvent` and sets `last_harm`.
    pub(super) fn maybe_injure(&mut self, i: usize, pos: Pos, h: &HarvestDef, defs: &Defs) {
        self.maybe_injure_at(i, pos, h.risk, h.modifier.as_deref(), defs);
    }

    /// `maybe_injure`, generalized to a risk and (optional) nation modifier that aren't
    /// necessarily a harvest's own: a `JobKind::Clear` on a renewable resource rolls its own
    /// `ClearDef::risk`, with no modifier to scale it (`run_clear`'s `clear_done`, issue #424).
    pub(super) fn maybe_injure_at(
        &mut self,
        i: usize,
        pos: Pos,
        risk: f32,
        modifier: Option<&str>,
        defs: &Defs,
    ) {
        let harm_scale = self.knobs(defs).harm;
        if harm_scale <= 0.0 || risk <= 0.0 {
            return;
        }
        let modifier =
            modifier.and_then(|m| self.modifiers_for(&self.units[i], defs).get(m)).unwrap_or(1.0);
        let chance = (risk / modifier).clamp(0.0, 1.0);
        if self.injury_rng.random::<f32>() >= chance {
            return;
        }
        let kills = self.knobs(defs).harm_kills;
        let floor = defs.harm.settings.floor;
        let damage = defs.harm.injury.damage * harm_scale;
        let u = &mut self.units[i];
        u.needs.health = if kills {
            (u.needs.health - damage).max(0.0)
        } else {
            (u.needs.health - damage).max(floor)
        };
        u.last_harm = Some((HarmKind::Injury, self.tick));
        let unit = u.id;
        self.log_event(HarmEvent { tick: self.tick, unit, pos, kind: HarmEventKind::Injured });
    }
}

#[cfg(test)]
mod tests;
