//! Bending definitions: the chi meter and the abilities, from `data/bending.toml`.
//!
//! See `docs/design/bending.md`. This module is only data: loading, validation and the plain
//! types. What an ability *does* is `world/bending.rs`, the one place that matches on `Effect`
//! (AGENTS.md rule 7: nations differ through data, and bending is the one thing that is allowed
//! to be unique per nation, so it lives in one obvious place).
//!
//! Abilities are referenced by `AbilityId`, their index in the file. Which nation can use one is
//! data (`AbilityDef::nation`); which tech unlocks it is `Unlocks::abilities` in
//! `data/research.toml`, resolved here into `AbilityDef::unlocked_by`. An ability no tech unlocks
//! is available from the start, which is what makes it a nation's signature ability.

use serde::Deserialize;

use crate::defs::{BiomeDef, BiomeId, DefsError, ResourceDef, ResourceId, Terrain};
use crate::jobs::HarvestDef;
use crate::nation::{Modifiers, Nation};
use crate::research::{TechDef, TechId};
use crate::world::TICKS_PER_SECOND;

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, serde::Serialize, serde::Deserialize,
)]
pub struct AbilityId(pub u16);

/// `[chi]` in `data/bending.toml`. Every unit has a meter from 0 to `max`.
#[derive(Debug, Clone, Copy, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ChiSettings {
    /// A full meter, which units start with. Also the cap on any ability's cost.
    pub max: f32,
    /// Chi regained per in-game minute while awake, before the `chi_regen` modifier.
    pub regen_per_minute: f32,
    /// The same while asleep.
    pub sleep_regen_per_minute: f32,
}

impl ChiSettings {
    /// `chi` after one tick of regeneration, clamped to `max`. `multiplier` is the unit's
    /// effective `chi_regen` modifier; `max` is usually `self.max`, but the Avatar's own meter
    /// (#237, `World::chi_max_for`) can hold more.
    pub fn regen(&self, chi: f32, sleeping: bool, multiplier: f32, max: f32) -> f32 {
        let per_minute = if sleeping { self.sleep_regen_per_minute } else { self.regen_per_minute };
        let per_tick = per_minute * multiplier / (60.0 * TICKS_PER_SECOND as f32);
        (chi + per_tick).min(max)
    }
}

/// What an ability does when it completes. One variant per kind in `bending.toml`; the match on
/// this enum is in `world/bending.rs` and nowhere else.
#[derive(Debug, Clone, PartialEq)]
pub enum Effect {
    /// Turn a tile of one terrain into another: Earth's "Raise Earth" (water into a ford) and
    /// "Bore Tunnel" (a mountain into a tunnel, #132).
    Reshape { from: Vec<Terrain>, to: Terrain },
    /// Wet a 3x3 patch near fresh water so wild food sprouts there (Water's "Water the Land").
    /// The magnitude (if any) scales `wet_minutes`.
    Irrigate {
        source_range: u32,
        wet_minutes: f32,
        sprouts: u32,
        sprout_resource: ResourceId,
        biomes: Vec<BiomeId>,
        fallow_minutes: f32,
    },
    /// A timed flame that cooks meals and warms sleep (Fire's "Hearthflame"). The magnitude
    /// scales `duration_minutes`.
    HeatSource { duration_minutes: f32, radius: u32, sleep_bonus: f32, seek_tiles: u32 },
    /// A warmth aura on the bender that follows them (Fire's "Inner Fire", #123): meals eaten
    /// within `radius` tiles of the carrier are cooked and sleep there is better, for
    /// `duration_minutes`. The magnitude scales `duration_minutes`. Unlike `HeatSource` it is
    /// never sought out, since it moves.
    HeatAura { duration_minutes: f32, radius: u32, sleep_bonus: f32 },
    /// Blow a ground stack into a stockpile (Air's "Wind Carry"). The magnitude scales `range`.
    WindCarry { range: u32 },
    /// Feel through the ground for what is buried within `radius` tiles (Chebyshev) of the target
    /// and uncover it (Earth's "Seismic Sense", #133): every buried resource there
    /// (`ResourceDef::buried`) comes onto the map, to be mined like any other. The magnitude
    /// scales `radius`.
    Reveal { radius: u32 },
    /// Only tests can define one: a valid target is walkable ground, and completing does nothing
    /// but spend the chi.
    #[cfg(test)]
    Probe,
}

#[derive(Debug, Clone, PartialEq)]
pub struct AbilityDef {
    pub id: String,
    pub nation: Nation,
    pub name: String,
    /// What the designation tool is called ("Raise stones").
    pub verb: String,
    pub description: String,
    /// Chi spent when a bend completes, in `(0, ChiSettings::max]`.
    pub chi: f32,
    /// Channel time in in-game seconds at a modifier of 1.0.
    pub work: f32,
    /// Nation modifier (`Modifiers::get`) that divides the channel time, if any.
    pub speed_modifier: Option<String>,
    /// Nation modifier that scales the effect's one magnitude, if any. Each `Effect` variant
    /// documents which field that is.
    pub magnitude_modifier: Option<String>,
    pub effect: Effect,
    /// The tech that unlocks this ability, or `None` if it is available from the start.
    pub unlocked_by: Option<TechId>,
}

#[derive(Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
enum RawEffect {
    Reshape {
        from: Vec<Terrain>,
        to: Terrain,
    },
    Irrigate {
        source_range: u32,
        wet_minutes: f32,
        sprouts: u32,
        sprout_resource: String,
        biomes: Vec<String>,
        fallow_minutes: f32,
    },
    HeatSource {
        duration_minutes: f32,
        radius: u32,
        sleep_bonus: f32,
        seek_tiles: u32,
    },
    HeatAura {
        duration_minutes: f32,
        radius: u32,
        sleep_bonus: f32,
    },
    WindCarry {
        range: u32,
    },
    Reveal {
        radius: u32,
    },
    #[cfg(test)]
    Probe,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawAbility {
    id: String,
    nation: Nation,
    name: String,
    verb: String,
    #[serde(default)]
    description: String,
    chi: f32,
    work: f32,
    speed_modifier: Option<String>,
    magnitude_modifier: Option<String>,
    effect: RawEffect,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct BendingFile {
    chi: ChiSettings,
    #[serde(default)]
    ability: Vec<RawAbility>,
}

fn positive(v: f32) -> bool {
    v.is_finite() && v > 0.0
}

impl BendingFile {
    /// Checks the data, links names to ids and records which tech unlocks each ability.
    pub(crate) fn resolve(
        self,
        biomes: &[BiomeDef],
        resources: &[ResourceDef],
        harvests: &[HarvestDef],
        techs: &[TechDef],
    ) -> Result<(ChiSettings, Vec<AbilityDef>), DefsError> {
        let err = |msg: String| DefsError(format!("bending.toml: {msg}"));
        let c = self.chi;
        if !positive(c.max) {
            return Err(err("chi.max must be a positive number".into()));
        }
        for (what, v) in [
            ("regen_per_minute", c.regen_per_minute),
            ("sleep_regen_per_minute", c.sleep_regen_per_minute),
        ] {
            if !(v.is_finite() && v >= 0.0) {
                return Err(err(format!("chi.{what} must be a number >= 0")));
            }
        }
        if self.ability.len() > usize::from(u16::MAX) {
            return Err(err(format!("too many abilities: {}", self.ability.len())));
        }

        let modifier = |a: &str, what: &str, name: &Option<String>| match name {
            Some(m) if !Modifiers::NAMES.contains(&m.as_str()) => {
                Err(err(format!("ability {a}: unknown {what} {m:?}")))
            }
            _ => Ok(()),
        };
        let mut abilities: Vec<AbilityDef> = Vec::with_capacity(self.ability.len());
        for raw in self.ability {
            let a = raw.id.as_str();
            if abilities.iter().any(|d| d.id == a) {
                return Err(err(format!("duplicate ability {a}")));
            }
            if !(positive(raw.chi) && raw.chi <= c.max) {
                return Err(err(format!("ability {a}: chi must be in (0, chi.max]")));
            }
            if !(raw.work.is_finite() && raw.work >= 0.0) {
                return Err(err(format!("ability {a}: work must be a number >= 0")));
            }
            modifier(a, "speed_modifier", &raw.speed_modifier)?;
            modifier(a, "magnitude_modifier", &raw.magnitude_modifier)?;
            let effect = resolve_effect(a, raw.effect, biomes, resources, harvests)?;
            abilities.push(AbilityDef {
                id: raw.id,
                nation: raw.nation,
                name: raw.name,
                verb: raw.verb,
                description: raw.description,
                chi: raw.chi,
                work: raw.work,
                speed_modifier: raw.speed_modifier,
                magnitude_modifier: raw.magnitude_modifier,
                effect,
                unlocked_by: None,
            });
        }
        link_unlocks(&mut abilities, techs)?;
        Ok((c, abilities))
    }
}

fn resolve_effect(
    ability: &str,
    raw: RawEffect,
    biomes: &[BiomeDef],
    resources: &[ResourceDef],
    harvests: &[HarvestDef],
) -> Result<Effect, DefsError> {
    let err = |msg: &str| Err(DefsError(format!("bending.toml: ability {ability}: {msg}")));
    Ok(match raw {
        RawEffect::Reshape { from, to } => {
            if from.is_empty() {
                return err("reshape needs at least one `from` terrain");
            }
            if from.contains(&to) {
                return err("reshape `to` can't also be a `from` terrain");
            }
            Effect::Reshape { from, to }
        }
        RawEffect::Irrigate {
            source_range,
            wet_minutes,
            sprouts,
            sprout_resource,
            biomes: biome_ids,
            fallow_minutes,
        } => {
            if source_range == 0 || sprouts == 0 {
                return err("irrigate source_range and sprouts must be at least 1");
            }
            if !(positive(wet_minutes) && positive(fallow_minutes)) {
                return err("irrigate wet_minutes and fallow_minutes must be positive");
            }
            let Some(resource) = resources.iter().position(|r| r.id == sprout_resource) else {
                return err(&format!("unknown sprout_resource {sprout_resource:?}"));
            };
            let resource = ResourceId(resource as u16);
            if !harvests.iter().any(|h| h.resource == resource) {
                return err(&format!("sprout_resource {sprout_resource:?} has no harvest entry"));
            }
            if biome_ids.is_empty() {
                return err("irrigate needs at least one fertile biome");
            }
            let mut ids = Vec::with_capacity(biome_ids.len());
            for b in &biome_ids {
                let Some(i) = biomes.iter().position(|d| d.id == *b) else {
                    return err(&format!("unknown biome {b:?}"));
                };
                ids.push(BiomeId(i as u8));
            }
            Effect::Irrigate {
                source_range,
                wet_minutes,
                sprouts,
                sprout_resource: resource,
                biomes: ids,
                fallow_minutes,
            }
        }
        RawEffect::HeatSource { duration_minutes, radius, sleep_bonus, seek_tiles } => {
            if !positive(duration_minutes) || radius == 0 {
                return err("heat_source duration_minutes and radius must be positive");
            }
            if !(sleep_bonus.is_finite() && sleep_bonus >= 1.0) {
                return err("heat_source sleep_bonus must be at least 1.0");
            }
            Effect::HeatSource { duration_minutes, radius, sleep_bonus, seek_tiles }
        }
        RawEffect::HeatAura { duration_minutes, radius, sleep_bonus } => {
            if !positive(duration_minutes) || radius == 0 {
                return err("heat_aura duration_minutes and radius must be positive");
            }
            if !(sleep_bonus.is_finite() && sleep_bonus >= 1.0) {
                return err("heat_aura sleep_bonus must be at least 1.0");
            }
            Effect::HeatAura { duration_minutes, radius, sleep_bonus }
        }
        RawEffect::WindCarry { range } => {
            if range == 0 {
                return err("wind_carry range must be at least 1");
            }
            Effect::WindCarry { range }
        }
        RawEffect::Reveal { radius } => {
            if radius == 0 {
                return err("reveal radius must be at least 1");
            }
            Effect::Reveal { radius }
        }
        #[cfg(test)]
        RawEffect::Probe => Effect::Probe,
    })
}

/// Resolves every tech's `unlocks.abilities` into `AbilityDef::unlocked_by`: the ability must
/// exist, belong to the tech's own nation (so a shared tech can't unlock one), and be unlocked by
/// no other tech.
fn link_unlocks(abilities: &mut [AbilityDef], techs: &[TechDef]) -> Result<(), DefsError> {
    for (t, tech) in techs.iter().enumerate() {
        let err =
            |msg: String| Err(DefsError(format!("tech {}: unlocks.abilities: {msg}", tech.id)));
        for name in &tech.unlocks.abilities {
            let Some(ability) = abilities.iter_mut().find(|a| a.id == *name) else {
                return err(format!("unknown ability {name:?}"));
            };
            if tech.nation != Some(ability.nation) {
                return err(format!(
                    "{name:?} is a {} ability, so only a {} tech can unlock it",
                    ability.nation.as_str(),
                    ability.nation.as_str()
                ));
            }
            if let Some(other) = ability.unlocked_by {
                return if other == TechId(t as u16) {
                    err(format!("{name:?} listed twice"))
                } else {
                    err(format!(
                        "{name:?} is already unlocked by {:?}; an ability has one tech",
                        techs[other.0 as usize].id
                    ))
                };
            }
            ability.unlocked_by = Some(TechId(t as u16));
        }
    }
    Ok(())
}

#[cfg(test)]
pub(crate) mod testing;
#[cfg(test)]
mod tests;
