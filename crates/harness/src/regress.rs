//! Regression: two run sets paired seed by seed (`docs/design/balance-harness.md`, section 6;
//! #538).
//!
//! Runs are paired by (style, preset, nation, seed). For each metric a target or pair reads, on
//! each (style, preset) it reads it on, the pairs with a reading on both sides give a delta
//! (later minus earlier) and the table says how many runs moved earlier, later or not at all and
//! what the median delta is. A metric has **moved** when the median delta is past its noise floor
//! (`[noise]` in `data/harness.toml`, from the jitter run) and at least twice as many runs moved
//! the median's way as the other way: a sign test, with no dependencies.
//!
//! Everything here is pure: two sets in, a [`Comparison`] and its text out.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;

use crate::baseline::{Entry, Key};
use crate::config::Config;

/// A metric read on one style and preset: one row of the comparison.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Group {
    pub metric: String,
    pub style: String,
    pub preset: String,
}

/// Every (metric, style, preset) the targets and pairs of `config` read, in the order they first
/// appear.
pub fn groups(config: &Config) -> Vec<Group> {
    let mut out: Vec<Group> = Vec::new();
    let mut add = |metric: String, style: &str, preset: &str| {
        let g = Group { metric, style: style.to_owned(), preset: preset.to_owned() };
        if !out.contains(&g) {
            out.push(g);
        }
    };
    for t in &config.targets {
        add(t.metric.name(), &t.style, &t.preset);
    }
    for p in &config.pairs {
        add(p.metric.name(), &p.lower.style, &p.lower.preset);
        add(p.metric.name(), &p.higher.style, &p.higher.preset);
    }
    out
}

/// One metric on one style and preset, over the runs both sets have a reading for.
#[derive(Debug, Clone, PartialEq)]
pub struct Row {
    pub group: Group,
    /// Pairs with a reading on both sides.
    pub paired: u32,
    /// Runs whose reading is lower now, higher now, and the same.
    pub earlier: u32,
    pub later: u32,
    pub same: u32,
    /// The median of (now - then); 0 with no pairs.
    pub median: f64,
    pub noise: f64,
    pub moved: bool,
}

/// What `--against` found.
#[derive(Debug, Clone, PartialEq)]
pub struct Comparison {
    pub rows: Vec<Row>,
    /// Runs in both sets with the same length.
    pub paired_runs: u32,
    /// Runs only the baseline has / only this set has.
    pub only_then: u32,
    pub only_now: u32,
    /// Runs in both whose lengths differ, which are not paired.
    pub length_differs: u32,
    /// Paired runs that raised a flag the baseline's didn't, and ones that no longer raise one.
    pub newly_flagged: u32,
    pub unflagged: u32,
}

impl Comparison {
    /// The rows that moved.
    pub fn moved(&self) -> impl Iterator<Item = &Row> {
        self.rows.iter().filter(|r| r.moved)
    }
}

/// The change from `then` to `now`. Two infinities (neither run met the event) are the same.
fn delta(then: f64, now: f64) -> f64 {
    if then == now { 0.0 } else { now - then }
}

/// The median of `values` (order irrelevant); the mean of the middle two when there is an even
/// number, with opposite infinities cancelling to 0. 0 for none.
pub fn median(values: &[f64]) -> f64 {
    if values.is_empty() {
        return 0.0;
    }
    let mut v = values.to_vec();
    v.sort_by(f64::total_cmp);
    let n = v.len();
    if n % 2 == 1 {
        v[n / 2]
    } else {
        let sum = v[n / 2 - 1] + v[n / 2];
        if sum.is_nan() {
            0.0
        } else if v[n / 2 - 1] == v[n / 2] {
            v[n / 2]
        } else {
            sum / 2.0
        }
    }
}

/// The moved rule (module doc): the median past `noise`, and at least twice as many runs moved the
/// median's way as the other way.
pub fn is_moved(median: f64, earlier: u32, later: u32, noise: f64) -> bool {
    if median.abs() <= noise {
        return false;
    }
    let (with, against) = if median > 0.0 { (later, earlier) } else { (earlier, later) };
    with >= 2 * against
}

/// Pairs `now` with `then` and reads off every group that has at least one run in `now`, with
/// each metric's noise from `config`.
pub fn compare(then: &[Entry], now: &[Entry], config: &Config) -> Comparison {
    let by_key: BTreeMap<Key, &Entry> = then.iter().map(|e| (e.key(), e)).collect();
    let mut pairs: Vec<(&Entry, &Entry)> = Vec::new();
    let (mut only_now, mut length_differs) = (0, 0);
    let mut matched: BTreeSet<Key> = BTreeSet::new();
    for n in now {
        match by_key.get(&n.key()) {
            None => only_now += 1,
            Some(t) => {
                matched.insert(n.key());
                if t.minutes == n.minutes {
                    pairs.push((t, n));
                } else {
                    length_differs += 1;
                }
            }
        }
    }
    let only_then = by_key.keys().filter(|k| !matched.contains(*k)).count() as u32;

    let (mut newly_flagged, mut unflagged) = (0, 0);
    for (t, n) in &pairs {
        newly_flagged += u32::from(n.flags.iter().any(|f| !t.flags.contains(f)));
        unflagged += u32::from(t.flags.iter().any(|f| !n.flags.contains(f)));
    }

    let mut rows = Vec::new();
    for g in groups(config) {
        if !now.iter().any(|n| n.style == g.style && n.preset == g.preset) {
            continue;
        }
        let deltas: Vec<f64> = pairs
            .iter()
            .filter(|(_, n)| n.style == g.style && n.preset == g.preset)
            .filter_map(|(t, n)| {
                Some(delta(*t.metrics.get(&g.metric)?, *n.metrics.get(&g.metric)?))
            })
            .collect();
        let earlier = deltas.iter().filter(|&&d| d < 0.0).count() as u32;
        let later = deltas.iter().filter(|&&d| d > 0.0).count() as u32;
        let med = median(&deltas);
        let noise = config.noise_of(&g.metric);
        rows.push(Row {
            group: g,
            paired: deltas.len() as u32,
            earlier,
            later,
            same: deltas.len() as u32 - earlier - later,
            median: med,
            noise,
            moved: is_moved(med, earlier, later, noise),
        });
    }
    Comparison {
        rows,
        paired_runs: pairs.len() as u32,
        only_then,
        only_now,
        length_differs,
        newly_flagged,
        unflagged,
    }
}

/// The warning to print when the baseline was made under other data than the run it is compared
/// with, or `None` when they agree. A warning and not a refusal: comparing across a change is the
/// point. `then` is the baseline's digest, `None` for a file that carries none.
pub fn stale_note(then: Option<&str>, now: &str) -> Option<String> {
    match then {
        Some(d) if d == now => None,
        Some(d) => Some(format!(
            "the baseline was made under other data ({d}, now {now}): its pairs include every \
             change to data/*.toml and the play styles since. A change to the sim's code does not \
             show in either digest."
        )),
        None => Some(format!(
            "this file carries no data digest, so it can't be told from the current data ({now})."
        )),
    }
}

fn signed(x: f64) -> String {
    if x == 0.0 { "0".to_owned() } else { format!("{x:+.2}") }
}

/// The comparison as text, under a heading naming `source`. `note` is [`stale_note`]'s.
pub fn render(c: &Comparison, source: &str, note: Option<&str>) -> String {
    let mut out = String::new();
    let _ = writeln!(
        out,
        "against      {source}: {} runs paired, {} only there, {} only here{}",
        c.paired_runs,
        c.only_then,
        c.only_now,
        if c.length_differs > 0 {
            format!(", {} of another length (not paired)", c.length_differs)
        } else {
            String::new()
        }
    );
    if let Some(note) = note {
        let _ = writeln!(out, "  WARNING    {note}");
    }
    for r in &c.rows {
        let mark = if r.moved { "MOVED" } else { "" };
        let place = format!("{}/{}", r.group.style, r.group.preset);
        if r.paired == 0 {
            let _ =
                writeln!(out, "  {mark:<6} {:<28} {place:<20} no paired readings", r.group.metric);
            continue;
        }
        let _ = writeln!(
            out,
            "  {mark:<6} {:<28} {place:<20} n {:>3}  earlier {:>3}  later {:>3}  same {:>3}  \
             median {:>7}  noise {}",
            r.group.metric,
            r.paired,
            r.earlier,
            r.later,
            r.same,
            signed(r.median),
            r.noise
        );
    }
    let moved: Vec<String> = c
        .moved()
        .map(|r| format!("{} ({}/{})", r.group.metric, r.group.style, r.group.preset))
        .collect();
    let _ = writeln!(
        out,
        "  {} of {} readings moved{}{}",
        moved.len(),
        c.rows.len(),
        if moved.is_empty() { String::new() } else { format!(": {}", moved.join(", ")) },
        if c.newly_flagged + c.unflagged > 0 {
            format!("; flags: {} runs newly flagged, {} no longer", c.newly_flagged, c.unflagged)
        } else {
            String::new()
        }
    );
    out
}

#[cfg(test)]
mod tests;
