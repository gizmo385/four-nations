//! Harm settings shared by every source of damage beyond neglect (`data/harm.toml`,
//! `docs/design/harm.md`, issue #387): the floor harm respects unless the difficulty preset's
//! `harm_kills` is on, the health below which a unit is too hurt for heavy work (`fit_for_heavy`,
//! read by #388, not yet by anything here), exposure's own rate and sickness's own rate and
//! terrains (`world/harm.rs`, issue #389).
//!
//! Read by `Defs::with_harm`, the way `seasons.toml` is read by `with_seasons`: a separate step
//! from `from_toml`, so data without one (most tests) simply has `HarmDef::default`. Nothing acts
//! on it unless a difficulty preset's `harm` knob is above 0, and no preset exists without this
//! also being loaded (`Defs::bundled`).

use serde::Deserialize;

use crate::defs::{DefsError, Terrain};

/// `[settings]` of `data/harm.toml`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HarmSettings {
    /// With the difficulty preset's `harm_kills` off, harm never takes a unit's health below
    /// this (`World::apply_exposure`). Need damage (`Needs::tick_at_cold_healing`) isn't floored.
    pub floor: f32,
    /// Below this health, a unit claims no harvest marked `risk` (issue #388; unused until
    /// then).
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

/// One entry of `[[sickness.water]]` (issue #389): a drink taken at a spot whose only water
/// neighbours are terrains listed here rolls `chance` (the highest among those listed, if more
/// than one matches) from `rng::stream(seed, "harm.sickness")`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SicknessWaterDef {
    pub terrain: Terrain,
    pub chance: f32,
}

/// `[sickness]` of `data/harm.toml` (issue #389): a drink at a shore whose only water is sea.
#[derive(Debug, Clone, PartialEq)]
pub struct SicknessDef {
    /// In-game minutes a sickness lasts, with no natural regen the whole time
    /// (`World::apply_sickness`). Ends at `Unit::sick_until`.
    pub minutes: f32,
    /// Health lost a minute while sick, times the difficulty preset's `harm`.
    pub damage_per_minute: f32,
    /// Path tiles `World::plan_drink` will walk a thirsty unit past the nearest drinking spot of
    /// any kind to reach fresh water instead. Only while the preset's `harm` is above 0.
    pub fresh_detour: u32,
    /// Terrains sea-water sickness rolls against (`World::sickness_chance`).
    pub water: Vec<SicknessWaterDef>,
}

/// Everything in `data/harm.toml`, resolved and validated. A default (unloaded) value matches
/// the bundled starting values; nothing reads it unless a difficulty preset's `harm` knob is
/// above 0, and the bundled data is always loaded by `Defs::bundled` before any preset can be
/// chosen.
#[derive(Debug, Clone, PartialEq)]
pub struct HarmDef {
    pub settings: HarmSettings,
    pub exposure: ExposureDef,
    pub sickness: SicknessDef,
}

impl Default for HarmDef {
    fn default() -> Self {
        Self {
            settings: HarmSettings { floor: 10.0, fit_for_heavy: 80.0 },
            exposure: ExposureDef { damage_per_minute: 4.0 },
            sickness: SicknessDef {
                minutes: 12.0,
                damage_per_minute: 3.0,
                fresh_detour: 15,
                water: vec![SicknessWaterDef { terrain: Terrain::Water, chance: 0.05 }],
            },
        }
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct HarmFile {
    settings: RawSettings,
    exposure: RawExposure,
    sickness: RawSickness,
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
struct RawSickness {
    minutes: f32,
    damage_per_minute: f32,
    fresh_detour: u32,
    water: Vec<RawSicknessWater>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawSicknessWater {
    terrain: Terrain,
    chance: f32,
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
        let k = &self.sickness;
        if !(k.minutes.is_finite() && k.minutes > 0.0) {
            return err("sickness.minutes must be a positive number".into());
        }
        if !(k.damage_per_minute.is_finite() && k.damage_per_minute > 0.0) {
            return err("sickness.damage_per_minute must be a positive number".into());
        }
        if k.water.is_empty() {
            return err("sickness.water must list at least one terrain".into());
        }
        let mut water = Vec::with_capacity(k.water.len());
        for w in &k.water {
            if !w.terrain.is_water() {
                return err(format!("sickness.water: {:?} is not a water terrain", w.terrain));
            }
            if !(w.chance.is_finite() && w.chance > 0.0 && w.chance <= 1.0) {
                return err(format!("sickness.water: {:?} chance must be in (0, 1]", w.terrain));
            }
            water.push(SicknessWaterDef { terrain: w.terrain, chance: w.chance });
        }
        Ok(HarmDef {
            settings: HarmSettings { floor: s.floor, fit_for_heavy: s.fit_for_heavy },
            exposure: ExposureDef { damage_per_minute: e.damage_per_minute },
            sickness: SicknessDef {
                minutes: k.minutes,
                damage_per_minute: k.damage_per_minute,
                fresh_detour: k.fresh_detour,
                water,
            },
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
        assert_eq!(defs.harm.sickness.minutes, 12.0);
        assert_eq!(defs.harm.sickness.damage_per_minute, 3.0);
        assert_eq!(defs.harm.sickness.fresh_detour, 15);
        assert_eq!(defs.harm.sickness.water.len(), 1);
        assert_eq!(defs.harm.sickness.water[0].terrain, crate::defs::Terrain::Water);
        assert_eq!(defs.harm.sickness.water[0].chance, 0.05);
    }

    #[test]
    fn bundled_harm_data_is_valid() {
        let defs = Defs::bundled();
        assert_eq!(defs.harm.settings.floor, 10.0);
        assert_eq!(defs.harm.settings.fit_for_heavy, 80.0);
        assert_eq!(defs.harm.exposure.damage_per_minute, 4.0);
        assert_eq!(defs.harm.sickness.minutes, 12.0);
        assert_eq!(defs.harm.sickness.damage_per_minute, 3.0);
        assert_eq!(defs.harm.sickness.fresh_detour, 15);
        assert_eq!(defs.harm.sickness.water[0].terrain, crate::defs::Terrain::Water);
        assert_eq!(defs.harm.sickness.water[0].chance, 0.05);
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
    fn non_positive_sickness_minutes_is_rejected() {
        let bad = HARM.replacen("minutes = 12.0", "minutes = 0.0", 1);
        let err = defs_without_harm().with_harm(&bad).unwrap_err();
        assert!(err.0.contains("sickness.minutes"), "{err}");
    }

    #[test]
    fn non_positive_sickness_damage_is_rejected() {
        let bad = HARM.replacen("damage_per_minute = 3.0", "damage_per_minute = 0.0", 1);
        let err = defs_without_harm().with_harm(&bad).unwrap_err();
        assert!(err.0.contains("sickness.damage_per_minute"), "{err}");
    }

    #[test]
    fn sickness_water_must_be_a_water_terrain() {
        let bad = HARM.replacen(r#"terrain = "water""#, r#"terrain = "land""#, 1);
        let err = defs_without_harm().with_harm(&bad).unwrap_err();
        assert!(err.0.contains("water terrain"), "{err}");
    }

    #[test]
    fn sickness_water_chance_must_be_in_range() {
        let too_high = HARM.replacen("chance = 0.05", "chance = 1.5", 1);
        let err = defs_without_harm().with_harm(&too_high).unwrap_err();
        assert!(err.0.contains("chance"), "{err}");

        let zero = HARM.replacen("chance = 0.05", "chance = 0.0", 1);
        let err = defs_without_harm().with_harm(&zero).unwrap_err();
        assert!(err.0.contains("chance"), "{err}");
    }

    #[test]
    fn sickness_water_must_not_be_empty() {
        let bad = HARM
            .replace("[[sickness.water]]\nterrain = \"water\"\nchance = 0.05\n", "water = []\n");
        let err = defs_without_harm().with_harm(&bad).unwrap_err();
        assert!(err.0.contains("sickness.water"), "{err}");
    }
}
