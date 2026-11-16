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
    Ok(SpiritsDef {
        start: raw.start,
        unrest_per_draw: raw.unrest_per_draw,
        unrest_per_tree: raw.unrest_per_tree,
        calm_base: raw.calm_base,
        calm_per_trees: raw.calm_per_trees,
        trees_per_calm: raw.trees_per_calm,
        trees_calm_max: raw.trees_calm_max,
        trees,
    })
}
