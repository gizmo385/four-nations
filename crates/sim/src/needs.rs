//! Unit needs: hunger, thirst, energy and health, and the death that follows from neglecting them.
//!
//! All numbers live in `data/needs.toml` (`NeedsSettings`, `FoodDef`). Rates there are per
//! in-game minute and converted to per-tick amounts here, so the tick rate can change without
//! rebalancing. This module only models the meters; *acting* on them (fetching food, sleeping) is
//! the job system's business (#4), which calls `Needs::eat`, `drink` and `tick(.., sleeping)`.
//!
//! Floating point is fine for determinism here: the same operations run in the same order on
//! every tick, and nothing depends on the platform's math library beyond `+ - * /`.

use serde::Deserialize;

use crate::defs::DefsError;

/// Seconds in an in-game minute.
const SECONDS_PER_MINUTE: f32 = 60.0;

#[derive(Debug, Clone, Copy, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NeedDef {
    pub decay_per_minute: f32,
    /// Below this level a unit wants to fix the need.
    pub low: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HealthDef {
    pub max: f32,
    /// Health lost per minute for each depleted need (hunger, thirst or energy at 0).
    pub depleted_damage_per_minute: f32,
    /// Health regained per minute while no need is below its `low` level.
    pub regen_per_minute: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GeneralSettings {
    /// The full level of hunger, thirst and energy.
    pub max: f32,
    /// In-game minutes between meals that raw food alone must be able to sustain.
    pub expected_meal_minutes: f32,
    /// Thirst restored by one drink of water.
    pub drink_thirst: f32,
    /// Energy restored per minute of sleep (energy doesn't decay while asleep).
    pub sleep_energy_per_minute: f32,
}

/// `[growth]` of `data/needs.toml` (issue #232, docs/design/endgame.md "Population growth"):
/// how often the settlement draws a newcomer, and what it costs in stores.
#[derive(Debug, Clone, Copy, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GrowthDef {
    /// In-game minutes between arrival checks.
    pub arrival_minutes: f32,
    /// Food items that must be in stores right now for one newcomer to arrive.
    pub food_per_newcomer: u32,
    /// Weight of the settlement's own nation when a newcomer's nation is drawn, once the Harmony
    /// Gate is complete (`docs/design/endgame.md`, "The transformation: the United City"; issue
    /// #240). Before completion every newcomer is the settlement's own nation regardless of this
    /// value: there is nobody else to draw from yet.
    pub own_weight: f32,
    /// Weight of each of the other three nations, same once-complete rule. The chance of each is
    /// its own weight over the sum of all four (`own_weight` once, this three times).
    pub other_weight: f32,
}

/// `[settings]`, `[hunger]`, `[thirst]`, `[energy]`, `[health]` and `[growth]` of
/// `data/needs.toml`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct NeedsSettings {
    pub general: GeneralSettings,
    pub hunger: NeedDef,
    pub thirst: NeedDef,
    pub energy: NeedDef,
    pub health: HealthDef,
    pub growth: GrowthDef,
}

impl NeedsSettings {
    /// The full level of hunger, thirst and energy.
    pub fn max(&self) -> f32 {
        self.general.max
    }
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FoodDef {
    pub id: String,
    pub name: String,
    /// An icon identifier for a future graphical frontend, as `ItemDef::icon`.
    pub icon: String,
    /// Hunger restored by one raw item. Never requires fire.
    pub hunger: f32,
    /// Hunger restored once cooked (needs `fire_making`); a bonus, so higher than `hunger`.
    pub cooked_hunger: Option<f32>,
    /// Id of the map resource that yields this food, if it has one yet.
    pub resource: Option<String>,
}

impl FoodDef {
    /// Hunger restored by one item; `cooked` only helps if this food can be cooked.
    pub fn hunger_restored(&self, cooked: bool) -> f32 {
        match (cooked, self.cooked_hunger) {
            (true, Some(c)) => c,
            _ => self.hunger,
        }
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct NeedsFile {
    settings: GeneralSettings,
    hunger: NeedDef,
    thirst: NeedDef,
    energy: NeedDef,
    health: HealthDef,
    growth: GrowthDef,
    food: Vec<FoodDef>,
}

impl NeedsFile {
    /// Checks the numbers and returns the settings and foods. `resource_ids` are the ids in
    /// `resources.toml`, for validating `FoodDef::resource`.
    pub(crate) fn resolve(
        self,
        resource_ids: &[&str],
    ) -> Result<(NeedsSettings, Vec<FoodDef>), DefsError> {
        let err = |msg: String| Err(DefsError(format!("needs.toml: {msg}")));
        let s = NeedsSettings {
            general: self.settings,
            hunger: self.hunger,
            thirst: self.thirst,
            energy: self.energy,
            health: self.health,
            growth: self.growth,
        };
        let g = s.general;
        for (what, v) in [
            ("settings.max", g.max),
            ("settings.expected_meal_minutes", g.expected_meal_minutes),
            ("settings.drink_thirst", g.drink_thirst),
            ("settings.sleep_energy_per_minute", g.sleep_energy_per_minute),
            ("health.max", s.health.max),
            ("health.depleted_damage_per_minute", s.health.depleted_damage_per_minute),
            ("health.regen_per_minute", s.health.regen_per_minute),
        ] {
            if !(v.is_finite() && v > 0.0) {
                return err(format!("{what} must be a positive number"));
            }
        }
        if g.drink_thirst > g.max {
            return err("settings.drink_thirst can't exceed settings.max".into());
        }
        if !(s.growth.arrival_minutes.is_finite() && s.growth.arrival_minutes > 0.0) {
            return err("growth.arrival_minutes must be a positive number".into());
        }
        if s.growth.food_per_newcomer == 0 {
            return err("growth.food_per_newcomer must be at least 1".into());
        }
        if !(s.growth.own_weight.is_finite() && s.growth.own_weight > 0.0) {
            return err("growth.own_weight must be a positive number".into());
        }
        if !(s.growth.other_weight.is_finite() && s.growth.other_weight > 0.0) {
            return err("growth.other_weight must be a positive number".into());
        }
        for (what, n) in [("hunger", s.hunger), ("thirst", s.thirst), ("energy", s.energy)] {
            if !(n.decay_per_minute.is_finite() && n.decay_per_minute > 0.0) {
                return err(format!("{what}.decay_per_minute must be a positive number"));
            }
            if !(0.0..=g.max).contains(&n.low) {
                return err(format!("{what}.low must be in [0, settings.max]"));
            }
        }
        if self.food.is_empty() {
            return err("need at least one [[food]]".into());
        }
        // Raw food must sustain a unit on its own: one item per expected meal interval has to
        // replace what hunger lost in that time. Cooking is only ever a bonus.
        let lost_between_meals = s.hunger.decay_per_minute * g.expected_meal_minutes;
        for (i, f) in self.food.iter().enumerate() {
            let id = &f.id;
            if self.food[..i].iter().any(|o| o.id == *id) {
                return err(format!("duplicate food {id}"));
            }
            if !(f.hunger.is_finite() && f.hunger > 0.0 && f.hunger <= g.max) {
                return err(format!("food {id}: hunger must be in (0, settings.max]"));
            }
            if f.hunger < lost_between_meals {
                return err(format!(
                    "food {id}: raw hunger {} can't sustain a unit (it loses {lost_between_meals} \
                     per {} min); raise it or lower hunger.decay_per_minute",
                    f.hunger, g.expected_meal_minutes
                ));
            }
            if let Some(c) = f.cooked_hunger
                && !(c.is_finite() && c > f.hunger && c <= g.max)
            {
                return err(format!("food {id}: cooked_hunger must be in (hunger, settings.max]"));
            }
            if let Some(r) = &f.resource
                && !resource_ids.contains(&r.as_str())
            {
                return err(format!("food {id}: unknown resource {r:?}"));
            }
        }
        Ok((s, self.food))
    }
}

/// Why a unit died: the first depleted need, in the order hunger, thirst, energy, or a harm
/// source that took health to 0 on its own (`crate::world::HarmKind::death_cause`, issue #387:
/// only reachable with the difficulty preset's `harm_kills` on, since harm otherwise floors
/// above 0, `data/harm.toml`'s `[settings] floor`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum DeathCause {
    Starvation,
    Dehydration,
    Exhaustion,
    /// Exposure: asleep, unsheltered, on cold land (`World::apply_exposure`).
    Exposure,
}

/// A unit's current meters. Each is in `[0, max]`; `health` has its own max.
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Needs {
    pub hunger: f32,
    pub thirst: f32,
    pub energy: f32,
    pub health: f32,
}

impl Needs {
    /// A unit that has just eaten, drunk and slept.
    pub fn full(s: &NeedsSettings) -> Self {
        Self { hunger: s.max(), thirst: s.max(), energy: s.max(), health: s.health.max }
    }

    pub fn is_dead(&self) -> bool {
        self.health <= 0.0
    }

    /// What is draining this unit's health right now, if any need is depleted.
    pub fn depleted(&self) -> Option<DeathCause> {
        if self.hunger <= 0.0 {
            Some(DeathCause::Starvation)
        } else if self.thirst <= 0.0 {
            Some(DeathCause::Dehydration)
        } else if self.energy <= 0.0 {
            Some(DeathCause::Exhaustion)
        } else {
            None
        }
    }

    /// True when any need is at or below its `low` level.
    pub fn any_low(&self, s: &NeedsSettings) -> bool {
        self.hunger <= s.hunger.low || self.thirst <= s.thirst.low || self.energy <= s.energy.low
    }

    /// Advances the meters by one sim tick (`TICKS_PER_SECOND` per in-game second).
    pub fn tick(&mut self, s: &NeedsSettings, sleeping: bool) {
        self.tick_scaled(s, sleeping, 1.0);
    }

    /// `tick` with energy regained asleep at `sleep_scale` times the base rate (a warm bed).
    pub fn tick_scaled(&mut self, s: &NeedsSettings, sleeping: bool, sleep_scale: f32) {
        self.tick_at_scaled(s, crate::world::TICKS_PER_SECOND, sleeping, sleep_scale);
    }

    /// `tick` at an explicit tick rate. Rates are per minute, so a minute's worth of ticks at any
    /// rate changes the meters by the same amount.
    pub fn tick_at(&mut self, s: &NeedsSettings, ticks_per_second: u32, sleeping: bool) {
        self.tick_at_scaled(s, ticks_per_second, sleeping, 1.0);
    }

    /// `tick_at` with a sleep-rate multiplier.
    pub fn tick_at_scaled(
        &mut self,
        s: &NeedsSettings,
        ticks_per_second: u32,
        sleeping: bool,
        sleep_scale: f32,
    ) {
        self.tick_at_full(s, ticks_per_second, sleeping, sleep_scale, 1.0);
    }

    /// `tick_at_scaled` with `decay_scale` also multiplying hunger, thirst and energy decay (not
    /// sleep's energy gain, nor health): the difficulty preset's `need_decay` knob
    /// (`docs/design/difficulty.md`, #294). 1.0 is `tick_at_scaled` exactly.
    pub fn tick_at_full(
        &mut self,
        s: &NeedsSettings,
        ticks_per_second: u32,
        sleeping: bool,
        sleep_scale: f32,
        decay_scale: f32,
    ) {
        self.tick_at_cold(s, ticks_per_second, sleeping, sleep_scale, decay_scale, 1.0, 1.0);
    }

    /// `tick_at_full` with cold's own extra drain on top of `decay_scale`
    /// (`docs/design/difficulty.md`, "Seasons", #295): `cold_hunger` and `cold_energy` multiply
    /// hunger and (awake) energy decay respectively. Thirst is never scaled by cold, and neither
    /// is sleep's energy gain here: a cold sleeper's slower gain is folded into `sleep_scale`
    /// instead, by whoever computes it (`World::tick_needs`), since it depends on whether a bed
    /// or heat source beats the cold penalty. 1.0 for both is `tick_at_full` exactly.
    #[allow(clippy::too_many_arguments)]
    pub fn tick_at_cold(
        &mut self,
        s: &NeedsSettings,
        ticks_per_second: u32,
        sleeping: bool,
        sleep_scale: f32,
        decay_scale: f32,
        cold_hunger: f32,
        cold_energy: f32,
    ) {
        self.tick_at_cold_healing(
            s,
            ticks_per_second,
            sleeping,
            sleep_scale,
            decay_scale,
            cold_hunger,
            cold_energy,
            None,
            true,
        );
    }

    /// `tick_at_cold` with a healing building's boosted regen (`BuildingDef::healing`, issue
    /// #375), `Some((regen_per_minute, stop_at))`, in place of the passive `health.regen_per_minute`
    /// while health is below `stop_at`: `World::tick_needs` passes this for a unit whose
    /// `Task::Healing` has it resting inside one (`World::healing_bonus_for`). `None` is
    /// `tick_at_cold` exactly. Depleted needs still damage health first, same as ever: a need
    /// pre-empts healing in `run_need` long before this would matter, but the order here keeps
    /// the two rules from ever fighting over the same tick.
    ///
    /// `regen_allowed` gates both the passive and the healing-boosted branch: while cold, no one
    /// regenerates, awake or asleep (`docs/design/harm.md`, "Rules every source shares", issue
    /// #387); `World::tick_needs` passes `false` while the unit's tile is cold
    /// (`World::is_cold`), on top of whatever `apply_exposure` does separately. Depleted-need
    /// damage above is never gated by this: it isn't a form of harm this floors, and a death
    /// still needs an empty need unless the difficulty preset's `harm_kills` is on.
    #[allow(clippy::too_many_arguments)]
    pub fn tick_at_cold_healing(
        &mut self,
        s: &NeedsSettings,
        ticks_per_second: u32,
        sleeping: bool,
        sleep_scale: f32,
        decay_scale: f32,
        cold_hunger: f32,
        cold_energy: f32,
        healing: Option<(f32, f32)>,
        regen_allowed: bool,
    ) {
        let per_tick =
            |per_minute: f32| per_minute / (SECONDS_PER_MINUTE * ticks_per_second as f32);
        let max = s.max();
        self.hunger = (self.hunger
            - per_tick(s.hunger.decay_per_minute) * decay_scale * cold_hunger)
            .max(0.0);
        self.thirst = (self.thirst - per_tick(s.thirst.decay_per_minute) * decay_scale).max(0.0);
        if sleeping {
            let gain = per_tick(s.general.sleep_energy_per_minute) * sleep_scale;
            self.energy = (self.energy + gain).min(max);
        } else {
            self.energy = (self.energy
                - per_tick(s.energy.decay_per_minute) * decay_scale * cold_energy)
                .max(0.0);
        }
        let depleted =
            [self.hunger, self.thirst, self.energy].iter().filter(|&&v| v <= 0.0).count();
        if depleted > 0 {
            let dmg = per_tick(s.health.depleted_damage_per_minute) * depleted as f32;
            self.health = (self.health - dmg).max(0.0);
        } else if regen_allowed && !self.any_low(s) {
            match healing {
                Some((rate, stop_at)) if self.health < stop_at => {
                    self.health = (self.health + per_tick(rate)).min(stop_at);
                }
                _ => {
                    self.health =
                        (self.health + per_tick(s.health.regen_per_minute)).min(s.health.max);
                }
            }
        }
    }

    /// Eats one item of `food`, raw or cooked.
    pub fn eat(&mut self, food: &FoodDef, cooked: bool, s: &NeedsSettings) {
        self.hunger = (self.hunger + food.hunger_restored(cooked)).min(s.max());
    }

    /// Drinks once.
    pub fn drink(&mut self, s: &NeedsSettings) {
        self.thirst = (self.thirst + s.general.drink_thirst).min(s.max());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::defs::Defs;
    use crate::world::TICKS_PER_SECOND;

    fn minute_ticks() -> u32 {
        TICKS_PER_SECOND * 60
    }

    #[test]
    fn a_unit_with_no_food_or_water_eventually_dies() {
        let defs = Defs::bundled();
        let s = &defs.needs;
        let mut n = Needs::full(s);
        let mut ticks = 0u64;
        while !n.is_dead() {
            n.tick(s, false);
            ticks += 1;
            assert!(ticks < 10 * 60 * u64::from(minute_ticks()), "never died");
        }
        // Not instant: it lasted at least until thirst ran out.
        let thirst_minutes = s.max() / s.thirst.decay_per_minute;
        assert!(ticks as f32 / minute_ticks() as f32 > thirst_minutes);
        assert!(n.depleted().is_some());
    }

    #[test]
    fn rates_are_per_minute_whatever_the_tick_rate() {
        let defs = Defs::bundled();
        let s = &defs.needs;
        for tps in [1, 10, 25, 60] {
            let mut n = Needs::full(s);
            for _ in 0..tps * 60 {
                n.tick_at(s, tps, false);
            }
            assert!((s.max() - n.hunger - s.hunger.decay_per_minute).abs() < 0.05, "tps {tps}");
            assert!((s.max() - n.thirst - s.thirst.decay_per_minute).abs() < 0.05, "tps {tps}");
            assert!((s.max() - n.energy - s.energy.decay_per_minute).abs() < 0.05, "tps {tps}");
            assert_eq!(n.health, s.health.max);
        }
    }

    #[test]
    fn decay_scale_speeds_up_hunger_thirst_and_energy_but_not_sleep_or_health() {
        // The difficulty preset's `need_decay` knob (#294): a run at 1.1 (Hard's default) loses
        // hunger, thirst and awake energy faster than identity (1.0), but sleep's energy gain
        // and health are untouched either way.
        let defs = Defs::bundled();
        let s = &defs.needs;
        let mut identity = Needs::full(s);
        let mut scaled = Needs::full(s);
        for _ in 0..minute_ticks() {
            identity.tick_at_full(s, TICKS_PER_SECOND, false, 1.0, 1.0);
            scaled.tick_at_full(s, TICKS_PER_SECOND, false, 1.0, 1.1);
        }
        assert!(scaled.hunger < identity.hunger);
        assert!(scaled.thirst < identity.thirst);
        assert!(scaled.energy < identity.energy);

        let mut identity = Needs { hunger: 50.0, thirst: 50.0, energy: 10.0, health: 100.0 };
        let mut scaled = identity;
        for _ in 0..minute_ticks() {
            identity.tick_at_full(s, TICKS_PER_SECOND, true, 1.0, 1.0);
            scaled.tick_at_full(s, TICKS_PER_SECOND, true, 1.0, 1.1);
        }
        assert_eq!(identity.energy, scaled.energy, "sleep's gain isn't scaled");
        assert_eq!(identity.health, scaled.health, "health isn't scaled directly");
    }

    #[test]
    fn cold_scales_hunger_and_awake_energy_but_never_thirst_or_sleep_gain() {
        // Cold's own extra drain (#295), on top of (not instead of) the difficulty's
        // `need_decay` knob: `tick_at_cold`'s `cold_hunger`/`cold_energy` speed up hunger and
        // awake energy decay, but thirst is untouched, and so is a sleeper's energy gain (that's
        // `sleep_scale`'s business, computed by the caller).
        let defs = Defs::bundled();
        let s = &defs.needs;
        let mut identity = Needs::full(s);
        let mut cold = Needs::full(s);
        for _ in 0..minute_ticks() {
            identity.tick_at_cold(s, TICKS_PER_SECOND, false, 1.0, 1.0, 1.0, 1.0);
            cold.tick_at_cold(s, TICKS_PER_SECOND, false, 1.0, 1.0, 1.25, 1.5);
        }
        assert!(cold.hunger < identity.hunger);
        assert_eq!(cold.thirst, identity.thirst);
        assert!(cold.energy < identity.energy);

        let mut identity = Needs { hunger: 50.0, thirst: 50.0, energy: 10.0, health: 100.0 };
        let mut cold = identity;
        for _ in 0..minute_ticks() {
            identity.tick_at_cold(s, TICKS_PER_SECOND, true, 1.0, 1.0, 1.0, 1.0);
            cold.tick_at_cold(s, TICKS_PER_SECOND, true, 0.5, 1.0, 1.25, 1.5);
        }
        assert!(cold.energy < identity.energy, "a cold sleeper's slower gain is sleep_scale's");
    }

    #[test]
    fn needs_never_leave_their_range() {
        let defs = Defs::bundled();
        let s = &defs.needs;
        let mut n = Needs::full(s);
        n.energy = 1.0;
        for _ in 0..minute_ticks() * 10 {
            n.tick(s, true);
        }
        assert_eq!(n.energy, s.max());
        for f in &defs.foods {
            n.eat(f, true, s);
        }
        (0..3).for_each(|_| n.drink(s));
        assert_eq!((n.hunger, n.thirst), (s.max(), s.max()));
    }

    #[test]
    fn sleeping_restores_energy_and_depleted_needs_cost_health() {
        let defs = Defs::bundled();
        let s = &defs.needs;
        let mut n = Needs { hunger: 50.0, thirst: 50.0, energy: 10.0, health: 100.0 };
        for _ in 0..minute_ticks() {
            n.tick(s, true);
        }
        assert!(n.energy > 10.0 + s.general.sleep_energy_per_minute - 0.1);
        let mut n = Needs { hunger: 0.0, thirst: 0.0, energy: 50.0, health: 100.0 };
        for _ in 0..minute_ticks() {
            n.tick(s, false);
        }
        // Two depleted needs drain twice as fast.
        let expected = 100.0 - 2.0 * s.health.depleted_damage_per_minute;
        assert!((n.health - expected).abs() < 0.01, "{}", n.health);
    }

    #[test]
    fn health_regenerates_only_when_comfortable() {
        let defs = Defs::bundled();
        let s = &defs.needs;
        let mut n = Needs::full(s);
        n.health = 50.0;
        n.tick(s, false);
        assert!(n.health > 50.0);
        let mut n = Needs::full(s);
        n.health = 50.0;
        n.hunger = s.hunger.low - 1.0;
        n.tick(s, false);
        assert_eq!(n.health, 50.0);
    }

    #[test]
    fn a_healing_boost_regens_faster_than_passive_and_stops_at_the_cap() {
        // A Healing Hut's boost (issue #375): faster than the passive rate, and capped below
        // `health.max` rather than at it.
        let defs = Defs::bundled();
        let s = &defs.needs;
        let mut boosted = Needs::full(s);
        boosted.health = 50.0;
        let mut passive = boosted;
        boosted.tick_at_cold_healing(s, 1, false, 1.0, 1.0, 1.0, 1.0, Some((16.0, 90.0)), true);
        passive.tick_at_cold_healing(s, 1, false, 1.0, 1.0, 1.0, 1.0, None, true);
        assert!(boosted.health > passive.health, "{} vs {}", boosted.health, passive.health);

        // One tick right at the cap never overshoots it: `World::run_need` (#375) is what
        // actually stops the unit (and so the boost) once health reaches `stop_at`, but the
        // tick itself must never push past it even taken alone.
        let mut n = Needs::full(s);
        n.health = 89.9;
        n.tick_at_cold_healing(s, 60, false, 1.0, 1.0, 1.0, 1.0, Some((16.0, 90.0)), true);
        assert!(n.health <= 90.0, "{}", n.health);
        n.health = 90.0;
        n.tick_at_cold_healing(s, 60, false, 1.0, 1.0, 1.0, 1.0, Some((16.0, 90.0)), true);
        assert_eq!(n.health, 90.0 + s.health.regen_per_minute / 60.0 / 60.0);

        // With no boost in play, `tick_at_cold_healing(.., None)` matches `tick_at_cold` exactly.
        let mut a = Needs::full(s);
        a.health = 50.0;
        let mut b = a;
        a.tick_at_cold(s, 1, false, 1.0, 1.0, 1.0, 1.0);
        b.tick_at_cold_healing(s, 1, false, 1.0, 1.0, 1.0, 1.0, None, true);
        assert_eq!(a, b);
    }

    #[test]
    fn regen_allowed_false_blocks_regen_even_when_comfortable() {
        // `World::tick_needs` passes `false` while the unit's tile is cold (issue #387, "no
        // natural regen while cold"), whether or not a healing boost is also in play.
        let defs = Defs::bundled();
        let s = &defs.needs;
        let mut blocked = Needs::full(s);
        blocked.health = 50.0;
        let before = blocked.health;
        blocked.tick_at_cold_healing(s, 1, false, 1.0, 1.0, 1.0, 1.0, None, false);
        assert_eq!(blocked.health, before, "no passive regen while cold");

        let mut blocked_boosted = Needs::full(s);
        blocked_boosted.health = 50.0;
        let before = blocked_boosted.health;
        blocked_boosted.tick_at_cold_healing(
            s,
            1,
            false,
            1.0,
            1.0,
            1.0,
            1.0,
            Some((16.0, 90.0)),
            false,
        );
        assert_eq!(blocked_boosted.health, before, "no healing boost either, while cold");

        // A depleted need still damages health even with regen blocked: the floor/regen rule
        // doesn't touch this path at all.
        let mut starving = Needs { hunger: 0.0, thirst: 50.0, energy: 50.0, health: 50.0 };
        starving.tick_at_cold_healing(s, 1, false, 1.0, 1.0, 1.0, 1.0, None, false);
        assert!(starving.health < 50.0);
    }

    #[test]
    fn raw_food_alone_sustains_a_unit() {
        // Owner requirement (#3): nobody starves for lack of fire. Eat each raw food at the
        // expected meal pace, with thirst and energy kept up, for three in-game days.
        let defs = Defs::bundled();
        let s = &defs.needs;
        let meal_ticks = (s.general.expected_meal_minutes * minute_ticks() as f32) as u32;
        for food in &defs.foods {
            let mut n = Needs::full(s);
            n.hunger = s.hunger.low; // start hungry
            for tick in 0..minute_ticks() * 60 * 24 * 3 {
                if tick % meal_ticks == 0 {
                    n.eat(food, false, s);
                }
                n.thirst = s.max();
                n.energy = s.max();
                n.tick(s, false);
                assert!(n.hunger > 0.0, "{} left a unit starving at tick {tick}", food.id);
                assert_eq!(n.health, s.health.max, "{} cost a unit health", food.id);
            }
        }
    }

    #[test]
    fn cooking_is_only_a_bonus() {
        let defs = Defs::bundled();
        for f in &defs.foods {
            assert!(f.hunger > 0.0);
            assert!(f.hunger_restored(true) >= f.hunger_restored(false), "{}", f.id);
        }
        assert!(defs.foods.iter().any(|f| f.cooked_hunger.is_some()));
    }
}
