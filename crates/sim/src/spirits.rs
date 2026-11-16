//! `[spirits]` in `data/jobs.toml` (docs/design/spirits.md, F6 of the factory endgame, #1030):
//! the numbers behind a United City's balance with the spirits. The run is
//! `world/spirits.rs`.

use serde::Deserialize;

use crate::defs::{DefsError, ResourceId};

/// The balance's numbers, all per minute. Every one has a default, so a `[spirits]` table may name
/// only what it changes, and data with none plays by the defaults.
#[derive(Debug, Clone, PartialEq)]
pub struct SpiritsDef {
    /// Where balance starts when the gate stands: 0 to 100, the ceiling.
    pub start: f32,
    /// Unrest for each point of draw on every running yard.
    pub unrest_per_draw: f32,
    /// Unrest for each tree felled.
    pub unrest_per_tree: f32,
    /// Calm every minute, whatever the city does.
    pub calm_base: f32,
    /// Calm for each `trees_per_calm` living trees on the map, at most `trees_calm_max`.
    pub calm_per_trees: f32,
    pub trees_per_calm: u32,
    pub trees_calm_max: f32,
    /// The resources that count as trees: living ones calm, felled ones unsettle.
    pub trees: Vec<ResourceId>,
    /// Calm for each Spirit Garden (F6c), or the city's nation's own figure where
    /// `garden_calm_by_nation` names it.
    pub calm_per_garden: f32,
    pub garden_calm_by_nation: std::collections::BTreeMap<crate::nation::Nation, f32>,
    /// No vine grows within this many tiles of a garden.
    pub garden_clear: u32,
    /// The vine resource (F6b), or `None` for no vines.
    pub vines: Option<ResourceId>,
    /// Below this balance a vine grows every `vine_seconds`; below `vine_fast_below`, twice as
    /// often.
    pub vine_below: f32,
    pub vine_fast_below: f32,
    pub vine_seconds: f32,
    /// The share of its speed a machine touching a vine keeps.
    pub vine_choke: f32,
    /// What an open Spirit Portal does (F6e).
    pub portal: PortalDef,
}

/// An open Spirit Portal (docs/design/spirits.md, "After it opens", F6e): its calm, the friendly
/// spirits that wander in, and their blessing. `[spirits.portal]`; every key has a default.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct PortalDef {
    /// Calm a minute while the portal is open and balance is above `calm_above`.
    pub calm: f32,
    pub calm_above: f32,
    /// One spirit for every `per` points of balance above `above`, at most `max`. One more
    /// comes each minute until there are that many.
    pub above: f32,
    pub per: f32,
    pub max: u32,
    /// Below this balance every spirit leaves, until balance recovers past `above` again.
    pub leave_below: f32,
    /// How far from the portal a spirit wanders, in tiles (Chebyshev).
    pub radius: u32,
    /// A spirit takes a step every this many seconds.
    pub step_seconds: f32,
    /// A farm plot or orchard beside a spirit grows `bless_boost` times as fast for
    /// `bless_seconds`.
    pub bless_boost: f32,
    pub bless_seconds: f32,
}

impl Default for PortalDef {
    fn default() -> Self {
        Self {
            calm: 2.0,
            calm_above: 50.0,
            above: 50.0,
            per: 10.0,
            max: 5,
            leave_below: 30.0,
            radius: 12,
            step_seconds: 2.0,
            bless_boost: 1.25,
            bless_seconds: 60.0,
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct RawSpirits {
    #[serde(default = "start")]
    start: f32,
    #[serde(default = "unrest_per_draw")]
    unrest_per_draw: f32,
    #[serde(default = "one")]
    unrest_per_tree: f32,
    #[serde(default = "one")]
    calm_base: f32,
    #[serde(default = "calm_per_trees")]
    calm_per_trees: f32,
    #[serde(default = "trees_per_calm")]
    trees_per_calm: u32,
    #[serde(default = "trees_calm_max")]
    trees_calm_max: f32,
    #[serde(default)]
    trees: Vec<String>,
    #[serde(default = "calm_per_garden")]
    calm_per_garden: f32,
    #[serde(default)]
    garden_calm_by_nation: std::collections::BTreeMap<crate::nation::Nation, f32>,
    #[serde(default = "garden_clear")]
    garden_clear: u32,
    #[serde(default)]
    vines: Option<String>,
    #[serde(default = "vine_below")]
    vine_below: f32,
    #[serde(default = "vine_fast_below")]
    vine_fast_below: f32,
    #[serde(default = "vine_seconds")]
    vine_seconds: f32,
    #[serde(default = "vine_choke")]
    vine_choke: f32,
    #[serde(default)]
    portal: PortalDef,
}

fn calm_per_garden() -> f32 {
    1.5
}
fn garden_clear() -> u32 {
    4
}
fn vine_below() -> f32 {
    40.0
}
fn vine_fast_below() -> f32 {
    15.0
}
fn vine_seconds() -> f32 {
    30.0
}
fn vine_choke() -> f32 {
    0.5
}

fn start() -> f32 {
    100.0
}
fn unrest_per_draw() -> f32 {
    0.25
}
fn one() -> f32 {
    1.0
}
fn calm_per_trees() -> f32 {
    0.5
}
fn trees_per_calm() -> u32 {
    25
}
fn trees_calm_max() -> f32 {
    3.0
}

impl Default for SpiritsDef {
    fn default() -> Self {
        Self {
            start: start(),
            unrest_per_draw: unrest_per_draw(),
            unrest_per_tree: one(),
            calm_base: one(),
            calm_per_trees: calm_per_trees(),
            trees_per_calm: trees_per_calm(),
            trees_calm_max: trees_calm_max(),
            trees: Vec::new(),
            calm_per_garden: calm_per_garden(),
            garden_calm_by_nation: Default::default(),
            garden_clear: garden_clear(),
            vines: None,
            vine_below: vine_below(),
            vine_fast_below: vine_fast_below(),
            vine_seconds: vine_seconds(),
            vine_choke: vine_choke(),
            portal: PortalDef::default(),
        }
    }
}

/// `[spirits]` checked, its trees resolved against `data/resources.toml`.
pub(crate) fn resolve(
    raw: Option<RawSpirits>,
    resource: impl Fn(&str) -> Option<ResourceId>,
) -> Result<SpiritsDef, DefsError> {
    let Some(raw) = raw else { return Ok(SpiritsDef::default()) };
    let err = |msg: String| Err(DefsError(format!("jobs.toml: [spirits]: {msg}")));
    if !(0.0..=100.0).contains(&raw.start) {
        return err(format!("start = {} must be 0 to 100", raw.start));
    }
    for (key, v) in [
        ("unrest_per_draw", raw.unrest_per_draw),
        ("unrest_per_tree", raw.unrest_per_tree),
        ("calm_base", raw.calm_base),
        ("calm_per_trees", raw.calm_per_trees),
        ("trees_calm_max", raw.trees_calm_max),
        ("calm_per_garden", raw.calm_per_garden),
    ] {
        if !(v.is_finite() && (0.0..=20.0).contains(&v)) {
            return err(format!("{key} = {v} must be 0 to 20"));
        }
    }
    if raw.trees_per_calm == 0 {
        return err("trees_per_calm must be at least 1".to_owned());
    }
    let mut trees = Vec::new();
    for id in &raw.trees {
        let Some(r) = resource(id) else { return err(format!("unknown tree resource {id:?}")) };
        trees.push(r);
    }
    let vines = match &raw.vines {
        None => None,
        Some(id) => match resource(id) {
            Some(r) => Some(r),
            None => return err(format!("unknown vine resource {id:?}")),
        },
    };
    if !(0.0..=100.0).contains(&raw.vine_below) || raw.vine_fast_below > raw.vine_below {
        return err("vine_fast_below must be at most vine_below, both 0 to 100".to_owned());
    }
    if !(raw.vine_seconds.is_finite() && raw.vine_seconds >= 1.0) {
        return err(format!("vine_seconds = {} must be at least 1", raw.vine_seconds));
    }
    if !(raw.vine_choke > 0.0 && raw.vine_choke <= 1.0) {
        return err(format!("vine_choke = {} must be above 0, at most 1", raw.vine_choke));
    }
    Ok(SpiritsDef {
        start: raw.start,
        unrest_per_draw: raw.unrest_per_draw,
        unrest_per_tree: raw.unrest_per_tree,
        calm_base: raw.calm_base,
        calm_per_trees: raw.calm_per_trees,
        trees_per_calm: raw.trees_per_calm,
        trees_calm_max: raw.trees_calm_max,
        trees,
        calm_per_garden: raw.calm_per_garden,
        garden_calm_by_nation: raw.garden_calm_by_nation,
        garden_clear: raw.garden_clear,
        vines,
        vine_below: raw.vine_below,
        vine_fast_below: raw.vine_fast_below,
        vine_seconds: raw.vine_seconds,
        vine_choke: raw.vine_choke,
        portal: match check_portal(raw.portal) {
            Ok(p) => p,
            Err(m) => return err(m),
        },
    })
}

fn check_portal(p: PortalDef) -> Result<PortalDef, String> {
    let fail = |m: String| Err(format!("portal: {m}"));
    for (key, v) in [("calm", p.calm), ("calm_above", p.calm_above), ("above", p.above)] {
        if !(v.is_finite() && (0.0..=100.0).contains(&v)) {
            return fail(format!("{key} = {v} must be 0 to 100"));
        }
    }
    if !(p.leave_below.is_finite() && (0.0..=p.above).contains(&p.leave_below)) {
        return fail(format!("leave_below = {} must be 0 to `above`", p.leave_below));
    }
    for (key, v) in
        [("per", p.per), ("step_seconds", p.step_seconds), ("bless_seconds", p.bless_seconds)]
    {
        if !(v.is_finite() && v >= 0.5) {
            return fail(format!("{key} = {v} must be at least 0.5"));
        }
    }
    if !(p.bless_boost.is_finite() && (1.0..=4.0).contains(&p.bless_boost)) {
        return fail(format!("bless_boost = {} must be 1 to 4", p.bless_boost));
    }
    if p.max > 20 || p.radius == 0 || p.radius > 40 {
        return fail("max must be at most 20, and radius 1 to 40".to_owned());
    }
    Ok(p)
}
