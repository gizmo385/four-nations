//! Harm settings shared by every source of damage beyond neglect (`data/harm.toml`,
//! `docs/design/harm.md`, issues #387 and #388): the floor harm respects unless the difficulty
//! preset's `harm_kills` is on, the health below which a unit is too hurt for heavy work
//! (`fit_for_heavy`, `World::fit_for_heavy_work`), exposure's own rate and injury's own damage
//! (`world/harm.rs`, `world/injury.rs`).
//!
//! Read by `Defs::with_harm`, the way `seasons.toml` is read by `with_seasons`: a separate step
//! from `from_toml`, so data without one (most tests) simply has `HarmDef::default`. Nothing acts
//! on it unless a difficulty preset's `harm` knob is above 0, and no preset exists without this
//! also being loaded (`Defs::bundled`).

use serde::Deserialize;

use crate::defs::DefsError;

/// `[settings]` of `data/harm.toml`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HarmSettings {
    /// With the difficulty preset's `harm_kills` off, harm never takes a unit's health below
    /// this (`World::apply_exposure`). Need damage (`Needs::tick_at_cold_healing`) isn't floored.
    pub floor: f32,
    /// Below this health, a unit claims no harvest marked `risk` (issue #388,
    /// `World::fit_for_heavy_work`).
    pub fit_for_heavy: f32,
}

/// `[exposure]` of `data/harm.toml`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ExposureDef {
    /// Health lost a minute, times the difficulty preset's `harm`, while a unit is asleep on a
    /// cold tile (`World::is_cold`) not sheltered by a bed whose own `sleep_bonus` beats the
    /// cold (`World::apply_exposure`).
    pub damage_per_minute: f32,
}

/// `[injury]` of `data/harm.toml` (issue #388).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct InjuryDef {
    /// Health lost on a hit, times the difficulty preset's `harm`, subject to `HarmSettings`'s
    /// floor (`World::maybe_injure`). The chance of a hit is `data/jobs.toml`'s `risk` on the
    /// harvest divided by the harvest's own `modifier`.
    pub damage: f32,
}

/// Everything in `data/harm.toml`, resolved and validated. A default (unloaded) value matches
/// the bundled starting values; nothing reads it unless a difficulty preset's `harm` knob is
/// above 0, and the bundled data is always loaded by `Defs::bundled` before any preset can be
/// chosen.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HarmDef {
    pub settings: HarmSettings,
    pub exposure: ExposureDef,
    pub injury: InjuryDef,
}

impl Default for HarmDef {
    fn default() -> Self {
        Self {
            settings: HarmSettings { floor: 10.0, fit_for_heavy: 80.0 },
            exposure: ExposureDef { damage_per_minute: 4.0 },
            injury: InjuryDef { damage: 40.0 },
        }
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct HarmFile {
    settings: RawSettings,
    exposure: RawExposure,
    injury: RawInjury,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawSettings {
    floor: f32,
    fit_for_heavy: f32,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawExposure {
    damage_per_minute: f32,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawInjury {
    damage: f32,
}

impl HarmFile {
    fn resolve(self) -> Result<HarmDef, DefsError> {
        let err = |msg: String| Err(DefsError(format!("harm.toml: {msg}")));
        let s = &self.settings;
        if !(s.floor.is_finite() && s.floor >= 0.0) {
            return err("settings.floor must be a non-negative number".into());
        }
        if !(s.fit_for_heavy.is_finite() && s.fit_for_heavy > 0.0) {
            return err("settings.fit_for_heavy must be a positive number".into());
        }
        if s.floor >= s.fit_for_heavy {
            return err("settings.floor must be less than settings.fit_for_heavy".into());
        }
        let e = &self.exposure;
        if !(e.damage_per_minute.is_finite() && e.damage_per_minute > 0.0) {
            return err("exposure.damage_per_minute must be a positive number".into());
        }
        let j = &self.injury;
        if !(j.damage.is_finite() && j.damage > 0.0) {
            return err("injury.damage must be a positive number".into());
        }
        Ok(HarmDef {
            settings: HarmSettings { floor: s.floor, fit_for_heavy: s.fit_for_heavy },
            exposure: ExposureDef { damage_per_minute: e.damage_per_minute },
            injury: InjuryDef { damage: j.damage },
        })
    }
}

impl crate::defs::Defs {
    /// These definitions with harm settings read from `harm` (`data/harm.toml`), replacing any
    /// they had. A separate step from `from_toml`, as `with_seasons` is, so data without one
    /// (most tests) simply has `HarmDef::default`, and nothing acts on it unless a difficulty
    /// preset's `harm` knob is above 0.
    pub fn with_harm(mut self, harm: &str) -> Result<Self, DefsError> {
        self.harm = toml::from_str::<HarmFile>(harm)
            .map_err(|e| DefsError(format!("harm.toml: {e}")))?
            .resolve()?;
        Ok(self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::defs::Defs;

    const HARM: &str = include_str!("../../../data/harm.toml");

    fn defs_without_harm() -> Defs {
        Defs::from_toml(
            include_str!("../../../data/biomes.toml"),
            include_str!("../../../data/resources.toml"),
            include_str!("../../../data/nations.toml"),
            include_str!("../../../data/research.toml"),
            include_str!("../../../data/needs.toml"),
            include_str!("../../../data/jobs.toml"),
            include_str!("../../../data/bending.toml"),
            include_str!("../../../data/buildings.toml"),
        )
        .unwrap()
    }

    #[test]
    fn unloaded_defs_fall_back_to_a_safe_default() {
        let defs = defs_without_harm();
        assert_eq!(defs.harm, HarmDef::default());
        assert_eq!(defs.harm.settings.floor, 10.0);
        assert_eq!(defs.harm.settings.fit_for_heavy, 80.0);
        assert_eq!(defs.harm.exposure.damage_per_minute, 4.0);
        assert_eq!(defs.harm.injury.damage, 40.0);
    }

    #[test]
    fn bundled_harm_data_is_valid() {
        let defs = Defs::bundled();
        assert_eq!(defs.harm.settings.floor, 10.0);
        assert_eq!(defs.harm.settings.fit_for_heavy, 80.0);
        assert_eq!(defs.harm.exposure.damage_per_minute, 4.0);
        assert_eq!(defs.harm.injury.damage, 40.0);
    }

    #[test]
    fn floor_must_be_below_fit_for_heavy() {
        let bad = HARM.replacen("floor = 10.0", "floor = 90.0", 1);
        let err = defs_without_harm().with_harm(&bad).unwrap_err();
        assert!(err.0.contains("floor"), "{err}");
    }

    #[test]
    fn negative_floor_is_rejected() {
        let bad = HARM.replacen("floor = 10.0", "floor = -1.0", 1);
        let err = defs_without_harm().with_harm(&bad).unwrap_err();
        assert!(err.0.contains("floor"), "{err}");
    }

    #[test]
    fn non_positive_exposure_damage_is_rejected() {
        let bad = HARM.replacen("damage_per_minute = 4.0", "damage_per_minute = 0.0", 1);
        let err = defs_without_harm().with_harm(&bad).unwrap_err();
        assert!(err.0.contains("damage_per_minute"), "{err}");
    }

    #[test]
    fn unknown_field_is_rejected() {
        let bad = HARM.replacen("floor = 10.0", "flooor = 10.0", 1);
        let err = defs_without_harm().with_harm(&bad).unwrap_err();
        assert!(err.0.contains("flooor"), "{err}");
    }

    #[test]
    fn non_positive_injury_damage_is_rejected() {
        let bad = HARM.replacen("damage = 40.0", "damage = 0.0", 1);
        let err = defs_without_harm().with_harm(&bad).unwrap_err();
        assert!(err.0.contains("injury.damage"), "{err}");
    }
}
