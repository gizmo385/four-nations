//! Running a panel (`docs/design/balance-harness.md`, sections 5 and 8; #537): which runs the
//! targets need, the runs themselves on a few threads, and the records as JSON. This is the file
//! and thread side that `fn_sim` must not have: every run is `fn_sim::balance::run_record`, a
//! pure function of its spec, so the threads only decide how soon, never what.

use std::path::Path;
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};

use fn_sim::balance::{PEOPLE, RunSpec, run_record};
use fn_sim::{Defs, Nation};

use crate::config::Config;
use crate::verdict::Run;

/// One run to make.
#[derive(Debug, Clone, PartialEq)]
pub struct Job {
    pub style: String,
    pub preset: String,
    pub nation: Nation,
    pub seed: u64,
    pub minutes: f32,
    /// Ticks the player's steps run late by (`RunSpec::step_offset`): 0 for a panel, 1 for the
    /// jitter run.
    pub step_offset: u32,
}

/// What a caller narrows a panel to (`balance verdict --styles .. --presets ..`). `None` is
/// everything the targets name.
#[derive(Debug, Clone, Default)]
pub struct Filter {
    pub styles: Option<Vec<String>>,
    pub presets: Option<Vec<String>>,
    pub nations: Option<Vec<Nation>>,
    /// Runs this long instead of what the targets ask for (a smoke test or a quick look; targets
    /// that read later than this report no data).
    pub minutes: Option<f32>,
}

/// A (style, preset) and what is wanted of it: (minutes, seed limit: `None` = every seed).
type Combo = ((String, String), (f32, Option<usize>));

/// The runs the targets and pairs in `config` need over `seeds`, narrowed by `filter`: for each
/// (style, preset) they name, the longest `minutes` any of them asks for and as many seeds as the
/// most demanding of them wants. Sorted by (style, preset, nation, seed).
pub fn plan(config: &Config, seeds: &[u64], filter: &Filter) -> Vec<Job> {
    let mut combos: Vec<Combo> = Vec::new();
    let mut want = |style: &str, preset: &str, minutes: f32, first: Option<usize>| {
        let key = (style.to_owned(), preset.to_owned());
        match combos.iter_mut().find(|(k, _)| *k == key) {
            Some((_, (m, f))) => {
                *m = m.max(minutes);
                *f = match (*f, first) {
                    (Some(a), Some(b)) => Some(a.max(b)),
                    _ => None,
                };
            }
            None => combos.push((key, (minutes, first))),
        }
    };
    for t in &config.targets {
        want(&t.style, &t.preset, t.minutes, t.first_seeds);
    }
    for p in &config.pairs {
        want(&p.lower.style, &p.lower.preset, p.minutes, p.first_seeds);
        want(&p.higher.style, &p.higher.preset, p.minutes, p.first_seeds);
    }

    let keep = |list: &Option<Vec<String>>, id: &str| {
        list.as_ref().is_none_or(|l| l.iter().any(|x| x == id))
    };
    let mut jobs = Vec::new();
    for ((style, preset), (minutes, first)) in combos {
        if !keep(&filter.styles, &style) || !keep(&filter.presets, &preset) {
            continue;
        }
        let take = first.map_or(seeds.len(), |n| n.min(seeds.len()));
        for &nation in &Nation::ALL {
            if filter.nations.as_ref().is_some_and(|ns| !ns.contains(&nation)) {
                continue;
            }
            for &seed in &seeds[..take] {
                jobs.push(Job {
                    style: style.clone(),
                    preset: preset.clone(),
                    nation,
                    seed,
                    minutes: filter.minutes.unwrap_or(minutes),
                    step_offset: 0,
                });
            }
        }
    }
    jobs.sort_by(|a, b| sort_key(a).cmp(&sort_key(b)));
    jobs
}

fn sort_key(j: &Job) -> (&str, &str, &str, u64) {
    (&j.style, &j.preset, j.nation.as_str(), j.seed)
}

/// Plays one job: its style's player, the config's detector thresholds.
pub fn run_job(job: &Job, config: &Config, defs: &Defs) -> Run {
    let style = config.styles.get(&job.style).expect("jobs only name styles the config has");
    let mut spec = RunSpec::new(job.seed, job.nation, job.minutes, defs);
    spec.preset = Some(job.preset.clone());
    spec.player = style.player(PEOPLE);
    spec.detectors = config.detectors;
    spec.step_offset = job.step_offset;
    Run { style: job.style.clone(), preset: job.preset.clone(), record: run_record(&spec, defs) }
}

/// Plays every job on `threads` threads and returns the runs sorted by (style, preset, nation,
/// seed), so the result never depends on which thread finished first. `progress(done, total)` is
/// called as runs finish (from any thread).
pub fn run_all(
    jobs: &[Job],
    config: &Config,
    defs: &Defs,
    threads: usize,
    progress: &(dyn Fn(usize, usize) + Sync),
) -> Vec<Run> {
    let next = AtomicUsize::new(0);
    let done = AtomicUsize::new(0);
    let finished: Mutex<Vec<Run>> = Mutex::new(Vec::with_capacity(jobs.len()));
    std::thread::scope(|scope| {
        for _ in 0..threads.clamp(1, jobs.len().max(1)) {
            scope.spawn(|| {
                loop {
                    let i = next.fetch_add(1, Ordering::Relaxed);
                    let Some(job) = jobs.get(i) else { break };
                    let run = run_job(job, config, defs);
                    finished.lock().expect("a worker panicked").push(run);
                    progress(done.fetch_add(1, Ordering::Relaxed) + 1, jobs.len());
                }
            });
        }
    });
    let mut runs = finished.into_inner().expect("a worker panicked");
    runs.sort_by(|a, b| run_key(a).cmp(&run_key(b)));
    runs
}

fn run_key(r: &Run) -> (&str, &str, &str, u64) {
    (&r.style, &r.preset, r.record.nation.as_str(), r.record.seed)
}

#[derive(serde::Serialize)]
struct Entry<'a> {
    style: &'a str,
    preset: &'a str,
    record: &'a fn_sim::balance::RunRecord,
}

/// The runs as JSON: an array with one object per run (`style`, `preset`, `record`), one per
/// line, in the order given.
pub fn to_json(runs: &[Run]) -> String {
    let mut out = String::from("[\n");
    for (i, r) in runs.iter().enumerate() {
        let entry = Entry { style: &r.style, preset: &r.preset, record: &r.record };
        out.push_str("  ");
        out.push_str(&serde_json::to_string(&entry).expect("a record is plain data"));
        out.push_str(if i + 1 < runs.len() { ",\n" } else { "\n" });
    }
    out.push_str("]\n");
    out
}

/// Writes `runs` to `dir/LABEL.json`, making `dir` if needed, and returns the path.
pub fn write_json(runs: &[Run], dir: &Path, label: &str) -> std::io::Result<std::path::PathBuf> {
    std::fs::create_dir_all(dir)?;
    let path = dir.join(format!("{label}.json"));
    std::fs::write(&path, to_json(runs))?;
    Ok(path)
}

#[cfg(test)]
mod tests;
