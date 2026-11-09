//! The jitter run (`docs/design/balance-harness.md`, section 6; #538): every panel run played
//! twice, once on the opening's own grid and once with each of its steps one tick late
//! (`RunSpec::step_offset`). That changes nothing about balance, so how far each metric still
//! moves is the chaos floor: its **noise** is the median absolute difference between a run and its
//! twin. `noise_floor` computes it and `set_noise` writes it into `data/harness.toml`'s `[noise]`
//! table without touching anything else in the file.
//!
//! A style that plays nothing (`idle`) has no steps to delay, so its twin is the same run and it
//! stays out of the pool: counting it would only pull every median toward 0.

use std::collections::BTreeMap;

use crate::baseline::{Entry, Key, quantize};
use crate::config::Config;
use crate::regress::median;
use crate::runner::{Filter, Job, plan};
use crate::style::Play;

/// The jobs of the jitter run: the plan for `seeds` and `filter`, minus the styles that play
/// nothing. Offset 0; the caller makes the twins.
pub fn jobs(config: &Config, seeds: &[u64], filter: &Filter) -> Vec<Job> {
    plan(config, seeds, filter)
        .into_iter()
        .filter(|j| config.styles.get(&j.style).is_some_and(|s| s.play != Play::Idle))
        .collect()
}

/// Each metric's median absolute difference between `plain` runs and their `jittered` twins,
/// paired by key. A metric with no pair that has a reading on both sides has no entry. Two
/// infinities (the event never happened in either) are no difference.
pub fn noise_floor(plain: &[Entry], jittered: &[Entry]) -> BTreeMap<String, f64> {
    let by_key: BTreeMap<Key, &Entry> = plain.iter().map(|e| (e.key(), e)).collect();
    let mut diffs: BTreeMap<&str, Vec<f64>> = BTreeMap::new();
    for j in jittered {
        let Some(p) = by_key.get(&j.key()) else { continue };
        for (name, &jv) in &j.metrics {
            if let Some(&pv) = p.metrics.get(name) {
                diffs.entry(name).or_default().push(if pv == jv { 0.0 } else { (jv - pv).abs() });
            }
        }
    }
    diffs.into_iter().map(|(name, d)| (name.to_owned(), quantize(median(&d)))).collect()
}

/// The `[noise]` table's entries are `"metric" = value` lines. Rewrites `text` so that each metric
/// in `values` has its line with that value: an existing line keeps its key text, spacing and
/// trailing comment; a new one goes after the table's last entry (or right under its header); every
/// other line of the file, comments and layout, is left as it was. A `text` without the table gets
/// it at the end.
pub fn set_noise(text: &str, values: &BTreeMap<String, f64>) -> String {
    let mut lines: Vec<String> = text.split_inclusive('\n').map(str::to_owned).collect();
    if let Some(last) = lines.last_mut()
        && !last.ends_with('\n')
    {
        last.push('\n');
    }
    let header = match lines.iter().position(|l| l.trim_start().starts_with("[noise]")) {
        Some(i) => i,
        None => {
            lines.push("\n[noise]\n".to_owned());
            lines.len() - 1
        }
    };
    let end = lines[header + 1..]
        .iter()
        .position(|l| l.trim_start().starts_with('['))
        .map_or(lines.len(), |i| header + 1 + i);

    let mut last_entry = header;
    let mut written: Vec<&str> = Vec::new();
    for (i, slot) in lines.iter_mut().enumerate().take(end).skip(header + 1) {
        let Some((key, eq)) = entry_key(slot) else { continue };
        last_entry = i;
        if let Some((name, &v)) = values.get_key_value(key.as_str()) {
            let line = slot.as_str();
            let after = &line[eq + 1..];
            let (value_text, comment) = match after.find('#') {
                Some(h) => (&after[..h], &after[h..]),
                None => (after, ""),
            };
            let gap = &value_text[value_text.trim_end().len()..];
            let tail = if comment.is_empty() { "\n".to_owned() } else { format!("{gap}{comment}") };
            *slot = format!("{} {}{tail}", &line[..=eq], toml_float(v));
            written.push(name);
        }
    }
    let fresh: Vec<String> = values
        .iter()
        .filter(|(name, _)| !written.contains(&name.as_str()))
        .map(|(name, &v)| format!("\"{name}\" = {}\n", toml_float(v)))
        .collect();
    lines.splice(last_entry + 1..last_entry + 1, fresh);
    lines.concat()
}

/// The key of a `"quoted.key" = value` line and the byte index of its `=`, or `None` for a blank
/// line, a comment or anything else.
fn entry_key(line: &str) -> Option<(String, usize)> {
    let t = line.trim_start();
    let rest = t.strip_prefix('"')?;
    let close = rest.find('"')?;
    let key = rest[..close].to_owned();
    let indent = line.len() - t.len();
    let after_key = indent + 1 + close + 1;
    let eq = line[after_key..].find('=')? + after_key;
    line[after_key..eq].trim().is_empty().then_some((key, eq))
}

/// `1` as `1.0`, so the file reads as floats.
fn toml_float(v: f64) -> String {
    format!("{v:?}")
}

#[cfg(test)]
mod tests;
