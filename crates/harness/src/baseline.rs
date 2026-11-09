//! The committed baseline and the run sets `--against` compares (`docs/design/balance-harness.md`,
//! section 6; #538).
//!
//! A [`Set`] is one panel's runs boiled down to what a comparison reads: for each run its key
//! (style, preset, nation, seed), its length, the flags it raised and a reading of every metric
//! `data/harness.toml` names. That is the shape of `crates/harness/baseline.json`, written by
//! `balance verdict --bless`. It is not the full record on purpose: a record is about 6 KB (its
//! season rows and timeline are most of that), so 700 of them would be 4 MB of JSON whose every
//! line changes with every sim tweak. The summary is about 0.7 KB a run, and a diff of it shows
//! which readings moved, which is the question a balance PR asks. `balance verdict` still writes
//! the full records to `target/balance/LABEL.json`, and [`Set::parse`] reads those too.
//!
//! The file is a pure function of the runs: entries are sorted by key, metrics by name, numbers are
//! rounded to [`QUANTUM`], and nothing about the machine, the thread count or the clock is in it.

use std::collections::BTreeMap;

use fn_sim::balance::RunRecord;
use fn_sim::{Defs, Nation};
use serde::{Deserialize, Serialize};

use crate::config::Config;
use crate::metric::Metric;
use crate::verdict::Run;

/// The file format's version.
pub const FORMAT: u32 = 1;

/// Where the committed baseline lives, from this crate's directory.
pub const BASELINE_FILE: &str = "baseline.json";

/// Readings are rounded to a millionth, so a value prints short (`100.099998`, not
/// `100.0999984741211`) and a fresh reading equals the stored one exactly when the run is the same.
const QUANTUM: f64 = 1_000_000.0;

/// How an infinite reading (an event that never happened) is written, JSON having no infinity.
const NEVER: &str = "never";

/// `x` rounded to [`QUANTUM`]; infinity stays infinity.
pub fn quantize(x: f64) -> f64 {
    if x.is_finite() { (x * QUANTUM).round() / QUANTUM } else { x }
}

/// One run, boiled down: the key, how long it ran, its flags and its metric readings.
#[derive(Debug, Clone, PartialEq)]
pub struct Entry {
    pub style: String,
    pub preset: String,
    pub nation: Nation,
    pub seed: u64,
    pub minutes: f32,
    /// The names of the flags the run raised, in `Flag::ALL`'s order.
    pub flags: Vec<String>,
    /// Metric name to reading. A metric the run can't say (it ended before the minute asked about)
    /// has no entry; an event that never happened reads as infinity.
    pub metrics: BTreeMap<String, f64>,
}

/// What pairs two runs: style, preset, nation, seed.
pub type Key<'a> = (&'a str, &'a str, &'a str, u64);

impl Entry {
    pub fn key(&self) -> Key<'_> {
        (&self.style, &self.preset, self.nation.as_str(), self.seed)
    }

    /// The entry of a finished run, reading every metric in `metrics`.
    pub fn of(run: &Run, metrics: &[(String, Metric)], defs: &Defs) -> Entry {
        Entry::from_record(&run.style, &run.preset, &run.record, metrics, defs)
    }

    fn from_record(
        style: &str,
        preset: &str,
        rec: &RunRecord,
        metrics: &[(String, Metric)],
        defs: &Defs,
    ) -> Entry {
        Entry {
            style: style.to_owned(),
            preset: preset.to_owned(),
            nation: rec.nation,
            seed: rec.seed,
            minutes: rec.minutes,
            flags: rec.flags.iter().map(|r| r.flag.name().to_owned()).collect(),
            metrics: metrics
                .iter()
                .filter_map(|(name, m)| m.read(rec, defs).map(|v| (name.clone(), quantize(v))))
                .collect(),
        }
    }
}

/// Every distinct metric the targets and pairs of `config` read, as (name, metric), by name.
pub fn metric_list(config: &Config) -> Vec<(String, Metric)> {
    let mut all: BTreeMap<String, Metric> = BTreeMap::new();
    for m in config.targets.iter().map(|t| &t.metric).chain(config.pairs.iter().map(|p| &p.metric))
    {
        all.entry(m.name()).or_insert_with(|| m.clone());
    }
    all.into_iter().collect()
}

/// A hash of what shapes a run besides the code: the game data (`Defs::bundled_digest`) and the
/// harness's own play styles and detector thresholds. Targets and noise floors are not in it (they
/// judge runs, they don't change them), so re-jittering doesn't make a baseline stale. 16 hex
/// digits (FNV-1a, stable across platforms).
pub fn data_digest(config: &Config) -> String {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    let text = format!("{:?}|{:?}", config.styles, config.detectors);
    for b in Defs::bundled_digest().to_le_bytes().into_iter().chain(text.bytes()) {
        h ^= u64::from(b);
        h = h.wrapping_mul(0x0100_0000_01b3);
    }
    format!("{h:016x}")
}

/// A set of runs for a comparison.
#[derive(Debug, Clone, PartialEq)]
pub struct Set {
    /// The data digest the runs were made under (`data_digest`); `None` for a file of full records,
    /// which doesn't carry one.
    pub digest: Option<String>,
    /// Sorted by key.
    pub entries: Vec<Entry>,
}

// ---- As written ----

/// A reading in the file: a number, or `"never"`.
#[derive(Debug, Serialize, Deserialize)]
#[serde(untagged)]
enum Stored {
    Number(f64),
    Word(String),
}

#[derive(Debug, Serialize, Deserialize)]
struct StoredEntry {
    style: String,
    preset: String,
    nation: Nation,
    seed: u64,
    minutes: f32,
    flags: Vec<String>,
    metrics: BTreeMap<String, Stored>,
}

#[derive(Debug, Deserialize)]
struct StoredFile {
    format: u32,
    data: String,
    runs: Vec<StoredEntry>,
}

/// One element of the full records `balance verdict` writes (`runner::to_json`).
#[derive(Debug, Deserialize)]
struct FullEntry {
    style: String,
    preset: String,
    record: RunRecord,
}

impl StoredEntry {
    fn of(e: &Entry) -> StoredEntry {
        StoredEntry {
            style: e.style.clone(),
            preset: e.preset.clone(),
            nation: e.nation,
            seed: e.seed,
            minutes: e.minutes,
            flags: e.flags.clone(),
            metrics: e
                .metrics
                .iter()
                .map(|(k, &v)| {
                    let stored = if v.is_finite() {
                        Stored::Number(v)
                    } else {
                        Stored::Word(NEVER.to_owned())
                    };
                    (k.clone(), stored)
                })
                .collect(),
        }
    }

    fn into_entry(self) -> Result<Entry, String> {
        let mut metrics = BTreeMap::new();
        for (name, v) in self.metrics {
            let value = match v {
                Stored::Number(x) => x,
                Stored::Word(w) if w == NEVER => f64::INFINITY,
                Stored::Word(w) => return Err(format!("metric {name:?}: {w:?} is not a number")),
            };
            metrics.insert(name, value);
        }
        Ok(Entry {
            style: self.style,
            preset: self.preset,
            nation: self.nation,
            seed: self.seed,
            minutes: self.minutes,
            flags: self.flags,
            metrics,
        })
    }
}

impl Set {
    /// The runs of a panel, read against every metric `config` names, under the current data.
    pub fn from_runs(runs: &[Run], config: &Config, defs: &Defs) -> Set {
        let metrics = metric_list(config);
        let mut entries: Vec<Entry> = runs.iter().map(|r| Entry::of(r, &metrics, defs)).collect();
        entries.sort_by(|a, b| a.key().cmp(&b.key()));
        Set { digest: Some(data_digest(config)), entries }
    }

    /// The set as the baseline file: one line a run, so a diff shows runs, not a wall of JSON.
    pub fn to_json(&self) -> String {
        let mut out = format!(
            "{{\n\"format\": {FORMAT},\n\"data\": \"{}\",\n\"runs\": [\n",
            self.digest.as_deref().unwrap_or("unknown")
        );
        for (i, e) in self.entries.iter().enumerate() {
            out.push_str(&serde_json::to_string(&StoredEntry::of(e)).expect("plain data"));
            out.push_str(if i + 1 < self.entries.len() { ",\n" } else { "\n" });
        }
        out.push_str("]\n}\n");
        out
    }

    /// Reads a baseline file, or a file of full records (`target/balance/LABEL.json`, whose
    /// metrics are read afresh against `config`).
    pub fn parse(text: &str, config: &Config, defs: &Defs) -> Result<Set, String> {
        if text.trim_start().starts_with('[') {
            let full: Vec<FullEntry> = serde_json::from_str(text)
                .map_err(|e| format!("not a list of run records: {e}"))?;
            let metrics = metric_list(config);
            let mut entries: Vec<Entry> = full
                .iter()
                .map(|f| Entry::from_record(&f.style, &f.preset, &f.record, &metrics, defs))
                .collect();
            entries.sort_by(|a, b| a.key().cmp(&b.key()));
            return Ok(Set { digest: None, entries });
        }
        let file: StoredFile =
            serde_json::from_str(text).map_err(|e| format!("not a baseline file: {e}"))?;
        if file.format != FORMAT {
            return Err(format!("baseline format {} (this build reads {FORMAT})", file.format));
        }
        let mut entries =
            file.runs.into_iter().map(StoredEntry::into_entry).collect::<Result<Vec<_>, _>>()?;
        entries.sort_by(|a, b| a.key().cmp(&b.key()));
        let digest = (file.data != "unknown").then_some(file.data);
        Ok(Set { digest, entries })
    }
}

#[cfg(test)]
mod tests;
