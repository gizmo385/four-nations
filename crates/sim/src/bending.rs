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

use std::collections::BTreeMap;

use serde::Deserialize;

use crate::defs::{BiomeDef, BiomeId, DefsError, ResourceDef, ResourceId, Terrain};
use crate::jobs::{HarvestDef, ItemDef, ItemId};
use crate::nation::{Modifiers, Nation};
use crate::research::{TechDef, TechId};
use crate::world::TICKS_PER_SECOND;

/// A settler's bending rank (`docs/design/bending.md`, "The route: novice, adept, master",
/// #360): computed live from finished bends and forms (`World::rank_of`), never stored, so
/// retuning `[mastery]` or a save from before this landed is read consistently. Ordered
/// `Novice < Adept < Master` so a rank-gated ability (`AbilityDef::rank`) can be checked with
/// `>=`.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default, Deserialize, serde::Serialize,
)]
#[serde(rename_all = "snake_case")]
pub enum Rank {
    #[default]
    Novice,
    Adept,
    Master,
}

// `Deserialize` is `save::ids`' remapping impl (#602).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, serde::Serialize)]
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
    /// (#237, `World::chi_max_for`) can hold more. `sleep_scale` is the same sleep-rate
    /// multiplier energy regen uses while asleep (`World::tick_needs`'s `rest`: a bed's
    /// `sleep_bonus`, a heat source, the cold penalty), ignored while awake, so a comfier sleep
    /// is shorter but earns chi faster by the same factor and so restores the same total (#345).
    pub fn regen(
        &self,
        chi: f32,
        sleeping: bool,
        multiplier: f32,
        sleep_scale: f32,
        max: f32,
    ) -> f32 {
        let per_minute = if sleeping {
            self.sleep_regen_per_minute * sleep_scale
        } else {
            self.regen_per_minute
        };
        let per_tick = per_minute * multiplier / (60.0 * TICKS_PER_SECOND as f32);
        (chi + per_tick).min(max)
    }
}

/// `[mastery]` in `data/bending.toml`: the route from novice to master
/// (`docs/design/bending.md`, "The route: novice, adept, master", #360). A master is
/// `master_bends_for` finished bends in at least `master_forms` different abilities ("forms": an
/// ability a unit has finished at least one bend of, `Unit::forms`); an adept is
/// `adept_bends_for` bends (`adept_share` of the master count, rounded up) in at least
/// `adept_forms`. `World::rank_of` is the one place that reads these to answer a unit's rank.
#[derive(Debug, Clone, PartialEq)]
pub struct MasteryDef {
    /// The flat bends threshold for a master, overridden per nation by `master_bends_per_nation`
    /// (#344's numbers, moved here unchanged).
    pub master_bends: u32,
    /// Overrides `master_bends` for the nations named, indexed like `Nation::ALL`.
    pub master_bends_per_nation: [Option<u32>; 4],
    /// Distinct abilities a master must have finished at least one bend of.
    pub master_forms: u32,
    /// The share of a nation's master bends an adept needs, in `(0, 1]`.
    pub adept_share: f32,
    /// Distinct abilities an adept must have finished at least one bend of.
    pub adept_forms: u32,
    /// What a master pays for a bend, as a share of the ability's own `chi` in `(0, 1]` (0.8:
    /// 80%).
    pub master_chi: f32,
    /// What each rank is called, per nation, indexed like `Nation::ALL`.
    pub titles: [NationTitles; 4],
}

/// `[mastery.titles.<nation>]`: what a settler of that nation is called at each rank.
#[derive(Debug, Clone, PartialEq)]
pub struct NationTitles {
    pub novice: String,
    pub adept: String,
    pub master: String,
}

impl MasteryDef {
    /// `nation`'s own index into `Nation::ALL`, which every per-nation array here (and
    /// `endeavor.rs`'s `bends_per_nation`/`per_nation`) is ordered by.
    fn index(nation: Nation) -> usize {
        Nation::ALL.iter().position(|&n| n == nation).expect("every nation is in ALL")
    }

    /// The finished bends that make a unit of `nation` a master: `master_bends_per_nation`'s
    /// override, or the flat `master_bends`.
    pub fn master_bends_for(&self, nation: Nation) -> u32 {
        self.master_bends_per_nation[Self::index(nation)].unwrap_or(self.master_bends)
    }

    /// The finished bends that make a unit of `nation` an adept: `adept_share` of its own
    /// master threshold, rounded up so an adept is always strictly easier than a master.
    pub fn adept_bends_for(&self, nation: Nation) -> u32 {
        (self.master_bends_for(nation) as f32 * self.adept_share).ceil() as u32
    }

    /// What a unit of `nation` at `rank` is called.
    pub fn title(&self, nation: Nation, rank: Rank) -> &str {
        let t = &self.titles[Self::index(nation)];
        match rank {
            Rank::Novice => &t.novice,
            Rank::Adept => &t.adept,
            Rank::Master => &t.master,
        }
    }
}

#[derive(Deserialize, Default)]
#[serde(deny_unknown_fields)]
struct PerNationBends {
    air: Option<u32>,
    water: Option<u32>,
    earth: Option<u32>,
    fire: Option<u32>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawTitles {
    novice: String,
    adept: String,
    master: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawMastery {
    master_bends: u32,
    #[serde(default)]
    master_bends_per_nation: PerNationBends,
    master_forms: u32,
    adept_share: f32,
    adept_forms: u32,
    master_chi: f32,
    titles: BTreeMap<String, RawTitles>,
}

impl RawMastery {
    fn resolve(self, abilities: &[AbilityDef]) -> Result<MasteryDef, DefsError> {
        let err = |msg: String| DefsError(format!("bending.toml: mastery: {msg}"));
        if self.master_bends == 0 {
            return Err(err("master_bends must be at least 1".into()));
        }
        let p = self.master_bends_per_nation;
        let master_bends_per_nation = [p.air, p.water, p.earth, p.fire];
        if master_bends_per_nation.contains(&Some(0)) {
            return Err(err("master_bends_per_nation values must be at least 1".into()));
        }
        if self.master_forms == 0 {
            return Err(err("master_forms must be at least 1".into()));
        }
        if self.adept_forms == 0 || self.adept_forms > self.master_forms {
            return Err(
                err("adept_forms must be at least 1, and no more than master_forms".into()),
            );
        }
        if !is_share(self.adept_share) {
            return Err(err("adept_share must be a share in (0, 1]".into()));
        }
        if !is_share(self.master_chi) {
            return Err(err("master_chi must be a share in (0, 1]".into()));
        }
        // Skipped when the data defines no abilities at all: tests of other tables (biomes,
        // resources) reuse the bundled `[chi]`/`[mastery]` with none, and bending itself is moot
        // without any.
        if !abilities.is_empty() {
            for n in Nation::ALL {
                let count = abilities.iter().filter(|a| a.of_nation(n)).count() as u32;
                if self.master_forms > count {
                    return Err(err(format!(
                        "master_forms ({}) is more than {} has abilities ({count})",
                        self.master_forms,
                        n.as_str()
                    )));
                }
            }
        }
        let mut titles = self.titles;
        let mut resolved = Vec::with_capacity(Nation::ALL.len());
        for n in Nation::ALL {
            let Some(t) = titles.remove(n.as_str()) else {
                return Err(err(format!("titles is missing {:?}", n.as_str())));
            };
            resolved.push(NationTitles { novice: t.novice, adept: t.adept, master: t.master });
        }
        if let Some(unknown) = titles.keys().next() {
            return Err(err(format!("titles: unknown nation {unknown:?}")));
        }
        let titles: [NationTitles; 4] =
            resolved.try_into().expect("one title per nation, just checked");
        Ok(MasteryDef {
            master_bends: self.master_bends,
            master_bends_per_nation,
            master_forms: self.master_forms,
            adept_share: self.adept_share,
            adept_forms: self.adept_forms,
            master_chi: self.master_chi,
            titles,
        })
    }
}

/// What an ability does when it completes. One variant per kind in `bending.toml`; the match on
/// this enum is in `world/bending.rs` and nowhere else.
#[derive(Debug, Clone, PartialEq)]
pub enum Effect {
    /// Turn a tile of one terrain into another: Earth's "Raise Earth" (water into a ford),
    /// "Bore Tunnel" (a mountain into a tunnel, #132) and Water's "Draw Water" (dry land into a
    /// pool, #364). `revert_minutes`, if set, is the magnitude when set (scaled the same way as
    /// `HeatSource::duration_minutes`): the tile reverts to whatever it was before after that
    /// long, instead of staying `to` forever. A target that is already walkable (Draw Water's dry
    /// land) needs no bank of passable ground beside it, since the bender can simply stand on it;
    /// one that isn't (Raise Earth's water, Bore Tunnel's rock) does.
    ///
    /// `source` (Carve Canal, #695), when not empty, replaces the bank rule: the target must be
    /// 4-adjacent to one of those terrains, or chained off a pending designation of the same
    /// bend. And when the target is walkable but `to` isn't (a canal cut into land), the bender
    /// stands beside it, the cut waits until no unit stands on the tile, and a cut that would
    /// split the land in two is refused.
    Reshape { from: Vec<Terrain>, to: Terrain, revert_minutes: Option<f32>, source: Vec<Terrain> },
    /// Cuts `count` of `item` from a tile of one of `from` terrains beside passable ground
    /// (Water's "Cut Ice", #364): the target itself (river, lake or sea) isn't walkable, so the
    /// yield drops on the tile the bender actually stood on to reach it, not on the target.
    DrawItem { from: Vec<Terrain>, item: ItemId, count: u32 },
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
    /// A timed source of warmth (Fire's "Hearthflame", Earth's "Earth Tent", Air's "Air
    /// Shield", #362). The magnitude scales `duration_minutes`. `cooks` (default true) says
    /// whether a meal eaten within `radius` counts as cooked; a source with `cooks = false` is a
    /// shelter, not a flame: it still keeps the cold away and gives its `sleep_bonus`, but
    /// nothing kindles there to cook on. `restoke_below` (in `(0, 1]`, default 1.0) is the
    /// restoke rule (#359): designating a lit source with more than that share of a fresh burn
    /// still left is refused, so restoking only ever shortens the wasted tail instead of
    /// resetting it outright.
    HeatSource {
        duration_minutes: f32,
        radius: u32,
        sleep_bonus: f32,
        seek_tiles: u32,
        restoke_below: f32,
        cooks: bool,
    },
    /// A warmth aura on the bender that follows them (Fire's "Inner Fire", #123): meals eaten
    /// within `radius` tiles of the carrier are cooked and sleep there is better, for
    /// `duration_minutes`. The magnitude scales `duration_minutes`. Unlike `HeatSource` it is
    /// never sought out, since it moves. `restoke_below` is the same restoke rule (#359), applied
    /// to the carrier's own aura instead of a tile.
    HeatAura { duration_minutes: f32, radius: u32, sleep_bonus: f32, restoke_below: f32 },
    /// Blow a ground stack into a stockpile (Air's "Wind Carry"). The magnitude scales `range`.
    WindCarry { range: u32 },
    /// A burst of speed on the caster (Air's "Air Scooter", #143): for `duration_minutes`, the
    /// caster's `move_speed` is multiplied by `multiplier` on top of everything else. Ends with
    /// its timer, not with distance covered. The magnitude scales `duration_minutes`, by
    /// `move_speed` itself: a nation already fast on its feet rides the burst a little longer.
    Scooter { duration_minutes: f32, multiplier: f32 },
    /// Uncover what is buried within `radius` tiles (Chebyshev) of the target: every buried
    /// resource (`ResourceDef::buried`) of a kind named in `finds` comes onto the map, to be
    /// worked like any other. Earth's "Seismic Sense" (#133) feels gold seams in the rock; Air's
    /// "Glide" (#197) spots lost ruins from above. Each sense's `finds` names only its own, so
    /// one nation's buried find stays hidden from the other's reveal. The magnitude scales
    /// `radius`.
    Reveal { radius: u32, finds: Vec<ResourceId> },
    /// Turns a ground stack of `ore` into `metal`, in place (Earth's "Metalbend", #134): metal
    /// from ore with no smelter, trip or building, at the cost of chi and a channel instead.
    /// `ratio` is how many `ore` one unit of `metal` costs. The whole stack converts in one bend;
    /// any remainder that doesn't divide evenly by `ratio` stays behind as ore.
    Smelt { ore: ItemId, metal: ItemId, ratio: u32 },
    /// Adds work to an ordinary build site (Fire's "Weld" and Air's "Lift", #363): not the
    /// endeavor's gate, which `gate.rs` owns, and not one whose work is already done. The
    /// magnitude scales `work_seconds`, banked straight onto the site's own progress, capped at
    /// the building's `work`. Work banked before the site's materials arrive is kept: the site
    /// finishes as soon as both are complete, through `run_build`'s own check, with no special
    /// case here for an unsupplied site.
    Build { work_seconds: f32 },
    /// Clears the strain from every tile within `radius` (Chebyshev) of the target (Water's
    /// "Regrow", `plantbending`, #740; difficulty.md, "Strain: tiring land"). The target must
    /// be open ground with strain on at least one tile in reach. No magnitude.
    Unstrain { radius: u32 },
    /// A cloud over the fields (Call Rain, `cloudbending`, #759): on completion a `Rain` joins
    /// `World::rains`, and every farm plot within `radius` tiles (Chebyshev) of the target grows
    /// `growth_boost` times faster, under the best-boost rule (`World::irrigation_rate`), for
    /// `minutes`. Casting over a patch already raining from the same target replaces that rain.
    /// The magnitude scales `minutes`.
    Rain { radius: u32, growth_boost: f32, minutes: f32 },
    /// Only tests can define one: a valid target is walkable ground, and completing does nothing
    /// but spend the chi.
    #[cfg(test)]
    Probe,
}

#[derive(Debug, Clone, PartialEq)]
pub struct AbilityDef {
    pub id: String,
    /// The nation that can use it. For a joint bend (`nations`) the first of its two nations;
    /// ask `of_nation` and `is_joint` before reading this as "the" nation.
    pub nation: Nation,
    /// A joint bend's two nations (#759), in file order: one caster of each channels it together.
    /// `None` for every other ability.
    pub nations: Option<[Nation; 2]>,
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
    /// The rank (`Adept` or `Master`, never `Novice`) a caster needs, checked by `can_cast`
    /// next to `unlocked_by` (#360). `None` for most abilities, including every signature
    /// (which needs neither a tech nor a rank).
    pub rank: Option<Rank>,
}

/// A `Reshape` that names a `source` (Carve Canal, #695), as a frontend reads it to say what the
/// bend does without matching on [`Effect`] itself (`AbilityDef::cut`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Cut<'a> {
    /// The terrains it can cut into.
    pub from: &'a [Terrain],
    /// What the cut tile becomes.
    pub to: Terrain,
    /// What the target must touch.
    pub source: &'a [Terrain],
    /// It never reverts: no `revert_minutes`.
    pub permanent: bool,
}

impl AbilityDef {
    /// Whether this is a joint bend: two casters of two nations channel it together (#759).
    pub fn is_joint(&self) -> bool {
        self.nations.is_some()
    }

    /// Whether a unit of `nation` can channel this ability: it is the ability's own nation, or
    /// one of a joint bend's two.
    pub fn of_nation(&self, nation: Nation) -> bool {
        match self.nations {
            Some(pair) => pair.contains(&nation),
            None => self.nation == nation,
        }
    }

    /// The cut this ability makes, if its effect is a `Reshape` with a `source`: `None` for every
    /// other ability.
    pub fn cut(&self) -> Option<Cut<'_>> {
        match &self.effect {
            Effect::Reshape { from, to, revert_minutes, source } if !source.is_empty() => {
                Some(Cut { from, to: *to, source, permanent: revert_minutes.is_none() })
            }
            _ => None,
        }
    }
}

#[derive(Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
enum RawEffect {
    Reshape {
        from: Vec<Terrain>,
        to: Terrain,
        #[serde(default)]
        revert_minutes: Option<f32>,
        #[serde(default)]
        source: Option<Vec<Terrain>>,
    },
    DrawItem {
        from: Vec<Terrain>,
        item: String,
        count: u32,
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
        #[serde(default = "full_restoke")]
        restoke_below: f32,
        #[serde(default = "always_cooks")]
        cooks: bool,
    },
    HeatAura {
        duration_minutes: f32,
        radius: u32,
        sleep_bonus: f32,
        #[serde(default = "full_restoke")]
        restoke_below: f32,
    },
    WindCarry {
        range: u32,
    },
    Scooter {
        duration_minutes: f32,
        multiplier: f32,
    },
    Reveal {
        radius: u32,
        finds: Vec<String>,
    },
    Smelt {
        ore: String,
        metal: String,
        ratio: u32,
    },
    Build {
        work_seconds: f32,
    },
    Unstrain {
        radius: u32,
    },
    Rain {
        radius: u32,
        growth_boost: f32,
        minutes: f32,
    },
    #[cfg(test)]
    Probe,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawAbility {
    id: String,
    #[serde(default)]
    nation: Option<Nation>,
    #[serde(default)]
    nations: Option<Vec<Nation>>,
    name: String,
    verb: String,
    #[serde(default)]
    description: String,
    chi: f32,
    work: f32,
    speed_modifier: Option<String>,
    magnitude_modifier: Option<String>,
    #[serde(default)]
    rank: Option<Rank>,
    effect: RawEffect,
}

/// `[canal]` in `data/bending.toml` (#695): what a carved canal tile does for the fields beside
/// it. Read by `World::irrigation_rate` next to the Irrigation Channel's own rule: the best boost
/// in reach applies, and a channel and a canal don't stack.
#[derive(Debug, Clone, Copy, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CanalDef {
    /// How far from each canal tile, Chebyshev, a farm plot is sped up: at least 1.
    pub irrigation_radius: u32,
    /// How many times faster a farm plot in reach grows: at least 1.0.
    pub growth_boost: f32,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct BendingFile {
    chi: ChiSettings,
    canal: CanalDef,
    #[serde(default)]
    ability: Vec<RawAbility>,
    mastery: RawMastery,
}

fn positive(v: f32) -> bool {
    v.is_finite() && v > 0.0
}

/// The default `restoke_below` for `heat_source` and `heat_aura`: a full share of the duration,
/// i.e. today's "always restoke" behaviour (#359).
fn full_restoke() -> f32 {
    1.0
}

/// The default `cooks` for `heat_source`: true, today's only behaviour before #362 split the
/// shelters (Earth Tent, Air Shield) off of it.
fn always_cooks() -> bool {
    true
}

/// Whether `v` is a share of a duration in `(0, 1]`, as `restoke_below` must be.
fn is_share(v: f32) -> bool {
    v.is_finite() && v > 0.0 && v <= 1.0
}

impl BendingFile {
    /// Checks the data, links names to ids and records which tech unlocks each ability.
    pub(crate) fn resolve(
        self,
        biomes: &[BiomeDef],
        resources: &[ResourceDef],
        harvests: &[HarvestDef],
        items: &[ItemDef],
        techs: &[TechDef],
    ) -> Result<(ChiSettings, Vec<AbilityDef>, MasteryDef, CanalDef), DefsError> {
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
        let canal = self.canal;
        if canal.irrigation_radius == 0 {
            return Err(err("canal.irrigation_radius must be at least 1".into()));
        }
        if !(canal.growth_boost.is_finite() && canal.growth_boost >= 1.0) {
            return Err(err("canal.growth_boost must be at least 1.0".into()));
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
            if raw.rank == Some(Rank::Novice) {
                return Err(err(format!("ability {a}: rank must be adept or master")));
            }
            let (nation, nations) = match (raw.nation, raw.nations) {
                (Some(n), None) => (n, None),
                (None, Some(pair)) => {
                    let [x, y] = pair[..] else {
                        return Err(err(format!(
                            "ability {a}: `nations` must list exactly two nations, not {}",
                            pair.len()
                        )));
                    };
                    if x == y {
                        return Err(err(format!(
                            "ability {a}: `nations` lists {} twice",
                            x.as_str()
                        )));
                    }
                    if raw.speed_modifier.is_some() {
                        return Err(err(format!(
                            "ability {a}: a joint bend has no speed_modifier (two casters \
                             would have two)"
                        )));
                    }
                    (x, Some([x, y]))
                }
                _ => {
                    return Err(err(format!(
                        "ability {a}: needs exactly one of `nation` and `nations`"
                    )));
                }
            };
            let effect = resolve_effect(a, raw.effect, biomes, resources, harvests, items)?;
            abilities.push(AbilityDef {
                id: raw.id,
                nation,
                nations,
                name: raw.name,
                verb: raw.verb,
                description: raw.description,
                chi: raw.chi,
                work: raw.work,
                speed_modifier: raw.speed_modifier,
                magnitude_modifier: raw.magnitude_modifier,
                effect,
                unlocked_by: None,
                rank: raw.rank,
            });
        }
        link_unlocks(&mut abilities, techs)?;
        let mastery = self.mastery.resolve(&abilities)?;
        Ok((c, abilities, mastery, canal))
    }
}

fn resolve_effect(
    ability: &str,
    raw: RawEffect,
    biomes: &[BiomeDef],
    resources: &[ResourceDef],
    harvests: &[HarvestDef],
    items: &[ItemDef],
) -> Result<Effect, DefsError> {
    let err = |msg: &str| Err(DefsError(format!("bending.toml: ability {ability}: {msg}")));
    Ok(match raw {
        RawEffect::Reshape { from, to, revert_minutes, source } => {
            if from.is_empty() {
                return err("reshape needs at least one `from` terrain");
            }
            if from.contains(&to) {
                return err("reshape `to` can't also be a `from` terrain");
            }
            if revert_minutes.is_some_and(|m| !positive(m)) {
                return err("reshape revert_minutes must be positive");
            }
            if source.as_ref().is_some_and(|s| s.is_empty()) {
                return err("reshape `source` needs at least one terrain");
            }
            let source = source.unwrap_or_default();
            Effect::Reshape { from, to, revert_minutes, source }
        }
        RawEffect::DrawItem { from, item, count } => {
            if from.is_empty() {
                return err("draw_item needs at least one `from` terrain");
            }
            if count == 0 {
                return err("draw_item count must be at least 1");
            }
            let Some(idx) = items.iter().position(|i| i.id == item) else {
                return err(&format!("unknown item {item:?}"));
            };
            Effect::DrawItem { from, item: ItemId(idx as u16), count }
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
        RawEffect::HeatSource {
            duration_minutes,
            radius,
            sleep_bonus,
            seek_tiles,
            restoke_below,
            cooks,
        } => {
            if !positive(duration_minutes) || radius == 0 {
                return err("heat_source duration_minutes and radius must be positive");
            }
            if !(sleep_bonus.is_finite() && sleep_bonus >= 1.0) {
                return err("heat_source sleep_bonus must be at least 1.0");
            }
            if !is_share(restoke_below) {
                return err("heat_source restoke_below must be a share in (0, 1]");
            }
            Effect::HeatSource {
                duration_minutes,
                radius,
                sleep_bonus,
                seek_tiles,
                restoke_below,
                cooks,
            }
        }
        RawEffect::HeatAura { duration_minutes, radius, sleep_bonus, restoke_below } => {
            if !positive(duration_minutes) || radius == 0 {
                return err("heat_aura duration_minutes and radius must be positive");
            }
            if !(sleep_bonus.is_finite() && sleep_bonus >= 1.0) {
                return err("heat_aura sleep_bonus must be at least 1.0");
            }
            if !is_share(restoke_below) {
                return err("heat_aura restoke_below must be a share in (0, 1]");
            }
            Effect::HeatAura { duration_minutes, radius, sleep_bonus, restoke_below }
        }
        RawEffect::WindCarry { range } => {
            if range == 0 {
                return err("wind_carry range must be at least 1");
            }
            Effect::WindCarry { range }
        }
        RawEffect::Scooter { duration_minutes, multiplier } => {
            if !positive(duration_minutes) {
                return err("scooter duration_minutes must be positive");
            }
            if !(multiplier.is_finite() && multiplier > 1.0) {
                return err("scooter multiplier must be greater than 1.0");
            }
            Effect::Scooter { duration_minutes, multiplier }
        }
        RawEffect::Reveal { radius, finds: names } => {
            if radius == 0 || names.is_empty() {
                return err("reveal needs a radius of at least 1 and something to find");
            }
            let mut finds = Vec::with_capacity(names.len());
            for n in &names {
                let Some(i) = resources.iter().position(|r| r.id == *n) else {
                    return err(&format!("reveal finds unknown resource {n:?}"));
                };
                if !resources[i].buried {
                    return err(&format!("reveal finds {n:?}, which is never buried"));
                }
                finds.push(ResourceId(i as u16));
            }
            Effect::Reveal { radius, finds }
        }
        RawEffect::Unstrain { radius } => {
            if !(1..=3).contains(&radius) {
                return err("unstrain radius must be 1 to 3");
            }
            Effect::Unstrain { radius }
        }
        RawEffect::Rain { radius, growth_boost, minutes } => {
            if !(1..=12).contains(&radius) {
                return err("rain radius must be 1 to 12");
            }
            if !(growth_boost.is_finite() && growth_boost >= 1.0) {
                return err("rain growth_boost must be at least 1.0");
            }
            if !positive(minutes) {
                return err("rain minutes must be positive");
            }
            Effect::Rain { radius, growth_boost, minutes }
        }
        RawEffect::Smelt { ore, metal, ratio } => {
            if ratio == 0 {
                return err("smelt ratio must be at least 1");
            }
            if ore == metal {
                return err("smelt ore and metal must be different items");
            }
            let Some(ore_idx) = items.iter().position(|i| i.id == ore) else {
                return err(&format!("unknown ore item {ore:?}"));
            };
            let Some(metal_idx) = items.iter().position(|i| i.id == metal) else {
                return err(&format!("unknown metal item {metal:?}"));
            };
            Effect::Smelt { ore: ItemId(ore_idx as u16), metal: ItemId(metal_idx as u16), ratio }
        }
        RawEffect::Build { work_seconds } => {
            if !positive(work_seconds) {
                return err("build work_seconds must be positive");
            }
            Effect::Build { work_seconds }
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
            if let Some(pair) = ability.nations {
                // A joint bend (#759) comes from a fusion of its own two nations, in either order.
                let same =
                    tech.fusion.is_some_and(|f| f.contains(&pair[0]) && f.contains(&pair[1]));
                if !same {
                    return err(format!(
                        "{name:?} is a {}-and-{} joint bend, so only a fusion of those two \
                         nations can unlock it",
                        pair[0].as_str(),
                        pair[1].as_str()
                    ));
                }
            } else if tech.nation != Some(ability.nation) {
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
    // `nations` is allowed only on an ability that a fusion of the same two nations unlocks.
    if let Some(a) = abilities.iter().find(|a| a.is_joint() && a.unlocked_by.is_none()) {
        return Err(DefsError(format!(
            "bending.toml: ability {}: a joint bend (`nations`) must be unlocked by a fusion tech",
            a.id
        )));
    }
    Ok(())
}

#[cfg(test)]
pub(crate) mod testing;
#[cfg(test)]
mod tests;
