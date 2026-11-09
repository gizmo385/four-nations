//! `data/harness.toml` as a whole (`docs/design/balance-harness.md`, sections 3, 5 and 8; #537):
//! the panel's defaults, the detectors' thresholds, the targets and the sensitivity pairs. The
//! play styles in the same file are `style`'s.
//!
//! Strict: an unknown key, an unknown style, preset or metric, a target with no bound, a share
//! outside (0, 1] and a repeated id are all errors at load, naming the entry. `fn_sim` never
//! sees this file; the thresholds reach it as plain `Detectors` fields.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use fn_sim::Defs;
use fn_sim::balance::Detectors;
use serde::Deserialize;

use crate::metric::Metric;
use crate::style::Styles;

/// The bundled `data/harness.toml`.
const BUNDLED: &str = include_str!("../../../data/harness.toml");

/// The materials `SiteProfile::supply` counts (`fn_sim::balance`'s `MATERIAL_SIGNS`), which are
/// the only things `hard_when` can name. A test keeps this list in step with the sim's.
pub const SUPPLY_SIGNS: [&str; 2] = ["stone", "ore"];

/// Why `data/harness.toml` didn't load. One line, naming the entry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConfigError(String);

impl fmt::Display for ConfigError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for ConfigError {}

fn err<T>(msg: String) -> Result<T, ConfigError> {
    Err(ConfigError(msg))
}

// ---- As written ----

#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawPanel {
    seeds: Option<String>,
    threads: Option<usize>,
    minutes: Option<f32>,
    min_runs: Option<u32>,
    share: Option<f64>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawDetectors {
    stranded_minutes: Option<u32>,
    stuck_minutes: Option<u32>,
    idle_share: Option<f32>,
    idle_minutes: Option<u32>,
    frozen_minutes: Option<u32>,
    research_minutes: Option<u32>,
    chi_idle_share: Option<f32>,
    chi_idle_after: Option<u32>,
    refused_streak: Option<u32>,
    overgrow_share: Option<f32>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawHardWhen {
    supply: String,
    below: f64,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawHardExpect {
    at_most: Option<f64>,
    at_least: Option<f64>,
    share: Option<f64>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawTarget {
    id: String,
    doc: String,
    style: Option<String>,
    preset: Option<String>,
    metric: String,
    at_most: Option<f64>,
    at_least: Option<f64>,
    share: Option<f64>,
    min_runs: Option<u32>,
    minutes: Option<f32>,
    first_seeds: Option<usize>,
    nations: Option<Vec<String>>,
    hard_when: Option<RawHardWhen>,
    hard_expect: Option<RawHardExpect>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawSide {
    style: String,
    preset: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawPair {
    id: String,
    doc: String,
    metric: String,
    lower: RawSide,
    higher: RawSide,
    share: Option<f64>,
    minutes: Option<f32>,
    first_seeds: Option<usize>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawConfig {
    #[serde(default)]
    panel: RawPanel,
    #[serde(default)]
    detectors: RawDetectors,
    // `[[style]]` is `Styles::parse`'s; here it is only let through.
    #[serde(default)]
    #[allow(dead_code)]
    style: Vec<toml::Value>,
    #[serde(default)]
    target: Vec<RawTarget>,
    #[serde(default)]
    pair: Vec<RawPair>,
    #[serde(default)]
    noise: BTreeMap<String, f64>,
}

// ---- Resolved ----

/// What a run must show to pass: either bound, or both (a band).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Bar {
    pub at_most: Option<f64>,
    pub at_least: Option<f64>,
}

impl Bar {
    /// Whether `value` meets it. An event that never happened reads as infinity, so it passes
    /// `at_least` ("no death in three hours") and fails `at_most`.
    pub fn met(&self, value: f64) -> bool {
        self.at_most.is_none_or(|m| value <= m) && self.at_least.is_none_or(|m| value >= m)
    }
}

impl fmt::Display for Bar {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match (self.at_least, self.at_most) {
            (Some(lo), Some(hi)) => write!(f, "{lo}..{hi}"),
            (Some(lo), None) => write!(f, ">= {lo}"),
            (None, Some(hi)) => write!(f, "<= {hi}"),
            (None, None) => f.write_str("any"),
        }
    }
}

/// A site is hard for a target when the material in reach is below `below` times the sign's need
/// (#543).
#[derive(Debug, Clone, PartialEq)]
pub struct HardWhen {
    /// A material sign id (`SUPPLY_SIGNS`).
    pub supply: String,
    pub below: f64,
}

/// The softer bar hard sites are judged against (#546), and the share of them that must meet it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HardExpect {
    pub bar: Bar,
    pub share: f64,
}

/// One design target (`[[target]]`), resolved.
#[derive(Debug, Clone, PartialEq)]
pub struct Target {
    pub id: String,
    /// The design doc section it comes from, `file.md#anchor`.
    pub doc: String,
    pub style: String,
    pub preset: String,
    pub metric: Metric,
    pub bar: Bar,
    /// The share of counted runs that must meet the bar.
    pub share: f64,
    /// Fewer counted runs than this is "no data".
    pub min_runs: u32,
    /// How long its runs are.
    pub minutes: f32,
    /// Only the first this-many seeds of the panel run for it.
    pub first_seeds: Option<usize>,
    /// Only these nations (every nation if empty).
    pub nations: Vec<String>,
    pub hard_when: Option<HardWhen>,
    pub hard_expect: Option<HardExpect>,
}

/// One side of a sensitivity pair: a style on a preset.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Side {
    pub style: String,
    pub preset: String,
}

/// A pair of runs that must separate (`[[pair]]`): `higher` should read above `lower` on the
/// metric on at least `share` of paired runs, or the comparison can't fail (#486).
#[derive(Debug, Clone, PartialEq)]
pub struct Pair {
    pub id: String,
    pub doc: String,
    pub metric: Metric,
    pub lower: Side,
    pub higher: Side,
    pub share: f64,
    pub minutes: f32,
    /// Only the first this-many seeds of the panel run for it.
    pub first_seeds: Option<usize>,
}

/// The panel's defaults (`[panel]`).
#[derive(Debug, Clone, PartialEq)]
pub struct Panel {
    /// A seed list in `--seeds`' syntax.
    pub seeds: String,
    pub threads: usize,
    pub minutes: f32,
    pub min_runs: u32,
    pub share: f64,
}

/// Everything in `data/harness.toml`.
#[derive(Debug, Clone, PartialEq)]
pub struct Config {
    pub panel: Panel,
    pub detectors: Detectors,
    pub styles: Styles,
    pub targets: Vec<Target>,
    pub pairs: Vec<Pair>,
    /// Each metric's noise floor, by metric name (`[noise]`, written by `balance jitter`, #538).
    /// A metric with no entry has none: any median delta past 0 counts.
    pub noise: BTreeMap<String, f64>,
}

fn share_ok(what: &str, share: f64) -> Result<f64, ConfigError> {
    if share > 0.0 && share <= 1.0 {
        Ok(share)
    } else {
        err(format!("{what}: share {share} is not in (0, 1]"))
    }
}

fn bar_of(what: &str, at_most: Option<f64>, at_least: Option<f64>) -> Result<Bar, ConfigError> {
    if at_most.is_none() && at_least.is_none() {
        return err(format!("{what}: needs `at_most` or `at_least`"));
    }
    if let (Some(lo), Some(hi)) = (at_least, at_most)
        && lo > hi
    {
        return err(format!("{what}: at_least {lo} is above at_most {hi}"));
    }
    Ok(Bar { at_most, at_least })
}

impl Config {
    /// The bundled `data/harness.toml`, checked against the bundled game data. Panics if it
    /// doesn't load (a test keeps that from happening).
    pub fn bundled(defs: &Defs) -> Config {
        Config::parse(BUNDLED, defs).unwrap_or_else(|e| panic!("bundled data/harness.toml: {e}"))
    }

    /// Parses `text`, checking every style, preset, metric and sign it names against `defs`.
    pub fn parse(text: &str, defs: &Defs) -> Result<Config, ConfigError> {
        let styles = Styles::parse(text).map_err(|e| ConfigError(e.to_string()))?;
        let raw: RawConfig = toml::from_str(text).map_err(|e| ConfigError(e.to_string()))?;

        let p = &raw.panel;
        let panel = Panel {
            seeds: p.seeds.clone().unwrap_or_else(|| "1-32".to_owned()),
            threads: p.threads.unwrap_or(4),
            minutes: p.minutes.unwrap_or(180.0),
            min_runs: p.min_runs.unwrap_or(8),
            share: share_ok("[panel]", p.share.unwrap_or(0.85))?,
        };
        if panel.threads == 0 {
            return err("[panel]: threads must be at least 1".to_owned());
        }
        if parse_seeds(&panel.seeds).is_none_or(|s| s.is_empty()) {
            return err(format!(
                "[panel]: seeds {:?} is not a seed list like \"1-32\"",
                panel.seeds
            ));
        }

        let d = &raw.detectors;
        let def = Detectors::default();
        let detectors = Detectors {
            stranded_minutes: d.stranded_minutes.unwrap_or(def.stranded_minutes),
            stuck_minutes: d.stuck_minutes.unwrap_or(def.stuck_minutes),
            idle_share: d.idle_share.unwrap_or(def.idle_share),
            idle_minutes: d.idle_minutes.unwrap_or(def.idle_minutes),
            frozen_minutes: d.frozen_minutes.unwrap_or(def.frozen_minutes),
            research_minutes: d.research_minutes.unwrap_or(def.research_minutes),
            chi_idle_share: d.chi_idle_share.unwrap_or(def.chi_idle_share),
            chi_idle_after: d.chi_idle_after.unwrap_or(def.chi_idle_after),
            refused_streak: d.refused_streak.unwrap_or(def.refused_streak),
            overgrow_share: d.overgrow_share.unwrap_or(def.overgrow_share),
        };

        let check_side = |what: &str, style: &str, preset: &str| -> Result<(), ConfigError> {
            if styles.get(style).is_none() {
                return err(format!("{what}: unknown style {style:?}"));
            }
            if defs.difficulty.preset(preset).is_none() {
                return err(format!("{what}: unknown preset {preset:?}"));
            }
            Ok(())
        };

        let mut ids = BTreeSet::new();
        let mut targets = Vec::new();
        for t in &raw.target {
            let what = format!("target {:?}", t.id);
            if !ids.insert(t.id.clone()) {
                return err(format!("{what}: id used twice"));
            }
            if !t.doc.contains(".md#") {
                return err(format!("{what}: `doc` must be `file.md#anchor`, got {:?}", t.doc));
            }
            let style = t.style.clone().unwrap_or_else(|| "steady".to_owned());
            let preset = t.preset.clone().unwrap_or_else(|| "normal".to_owned());
            check_side(&what, &style, &preset)?;
            let metric =
                Metric::parse(&t.metric, defs).map_err(|e| ConfigError(format!("{what}: {e}")))?;
            let bar = bar_of(&what, t.at_most, t.at_least)?;
            let nations = t.nations.clone().unwrap_or_default();
            for n in &nations {
                if !fn_sim::Nation::ALL.iter().any(|x| x.as_str() == n) {
                    return err(format!("{what}: unknown nation {n:?}"));
                }
            }
            if t.first_seeds == Some(0) {
                return err(format!("{what}: first_seeds must be at least 1"));
            }
            let hard_when = match &t.hard_when {
                None => None,
                Some(h) => {
                    if !SUPPLY_SIGNS.contains(&h.supply.as_str()) {
                        return err(format!(
                            "{what}: hard_when.supply {:?} is not one of {SUPPLY_SIGNS:?}",
                            h.supply
                        ));
                    }
                    if !defs.endeavor.signs.iter().any(|s| s.id == h.supply) {
                        return err(format!("{what}: hard_when.supply {:?} is no sign", h.supply));
                    }
                    if h.below <= 0.0 {
                        return err(format!("{what}: hard_when.below must be above 0"));
                    }
                    Some(HardWhen { supply: h.supply.clone(), below: h.below })
                }
            };
            let hard_expect = match &t.hard_expect {
                None => None,
                Some(h) => {
                    if hard_when.is_none() {
                        return err(format!("{what}: hard_expect without hard_when"));
                    }
                    let bar = bar_of(&format!("{what} hard_expect"), h.at_most, h.at_least)?;
                    let share = share_ok(&format!("{what} hard_expect"), h.share.unwrap_or(0.5))?;
                    Some(HardExpect { bar, share })
                }
            };
            targets.push(Target {
                id: t.id.clone(),
                doc: t.doc.clone(),
                style,
                preset,
                metric,
                bar,
                share: share_ok(&what, t.share.unwrap_or(panel.share))?,
                min_runs: t.min_runs.unwrap_or(panel.min_runs),
                minutes: t.minutes.unwrap_or(panel.minutes),
                first_seeds: t.first_seeds,
                nations,
                hard_when,
                hard_expect,
            });
        }

        let mut pairs = Vec::new();
        for p in &raw.pair {
            let what = format!("pair {:?}", p.id);
            if !ids.insert(p.id.clone()) {
                return err(format!("{what}: id used twice"));
            }
            if !p.doc.contains(".md#") {
                return err(format!("{what}: `doc` must be `file.md#anchor`, got {:?}", p.doc));
            }
            check_side(&what, &p.lower.style, &p.lower.preset)?;
            check_side(&what, &p.higher.style, &p.higher.preset)?;
            if p.first_seeds == Some(0) {
                return err(format!("{what}: first_seeds must be at least 1"));
            }
            if p.lower.style == p.higher.style && p.lower.preset == p.higher.preset {
                return err(format!("{what}: both sides are the same run"));
            }
            pairs.push(Pair {
                id: p.id.clone(),
                doc: p.doc.clone(),
                metric: Metric::parse(&p.metric, defs)
                    .map_err(|e| ConfigError(format!("{what}: {e}")))?,
                lower: Side { style: p.lower.style.clone(), preset: p.lower.preset.clone() },
                higher: Side { style: p.higher.style.clone(), preset: p.higher.preset.clone() },
                share: share_ok(&what, p.share.unwrap_or(0.25))?,
                minutes: p.minutes.unwrap_or(panel.minutes),
                first_seeds: p.first_seeds,
            });
        }

        for (name, &floor) in &raw.noise {
            let what = format!("[noise] {name:?}");
            Metric::parse(name, defs).map_err(|e| ConfigError(format!("{what}: {e}")))?;
            if floor.is_nan() || floor < 0.0 {
                return err(format!("{what}: noise must be 0 or more, got {floor}"));
            }
        }

        Ok(Config { panel, detectors, styles, targets, pairs, noise: raw.noise })
    }

    /// The noise floor of the metric called `name` (see [`Config::noise`]).
    pub fn noise_of(&self, name: &str) -> f64 {
        self.noise.get(name).copied().unwrap_or(0.0)
    }

    /// The target with this id.
    pub fn target(&self, id: &str) -> Option<&Target> {
        self.targets.iter().find(|t| t.id == id)
    }
}

/// A seed list: comma-separated numbers and `A-B` ranges (`1-8`, `7,42`, `1-4,9`). `None` if any
/// part isn't one. Order is kept and repeats are dropped.
pub fn parse_seeds(s: &str) -> Option<Vec<u64>> {
    let mut out: Vec<u64> = Vec::new();
    for part in s.split(',') {
        let (a, b) = match part.split_once('-') {
            Some((a, b)) => (a.trim().parse::<u64>().ok()?, b.trim().parse::<u64>().ok()?),
            None => {
                let n = part.trim().parse::<u64>().ok()?;
                (n, n)
            }
        };
        if a > b {
            return None;
        }
        for seed in a..=b {
            if !out.contains(&seed) {
                out.push(seed);
            }
        }
    }
    Some(out)
}

#[cfg(test)]
mod tests;
