//! Par and the controls beside a track's scores (#572, docs/design/agent-hillclimb.md, "Tracks:
//! what is comparable"): what a score is measured against. For the owner's tables only. No prompt
//! is built from anything in here.
//!
//! **Par** is `fn_agent par`: the score of the stock `autoplay::Economy` opening on the same
//! start at the same horizon. It is described everywhere as what a stock scripted opening
//! reaches on this start, reading the world directly (it scrolls out to 90 tiles, which a player
//! would have to discover), and never as a target a fair player should hit or a ceiling. A
//! par takes tens of seconds, so `fn_agent` keeps each one per build and start, in a folder
//! outside every run folder ([`cache_root`]); the pass here only runs the missing ones, in
//! parallel, and says how long it took and how many it had to compute.
//!
//! **Controls** are strategies with no model, played cleanly like any other and shown in the
//! same table: the do-nothing control ([`DO_NOTHING`], `loop { wait(10); }`, which is what a
//! colony earns if nobody ever gives an order), and any `--control GOVERNOR.rhai[@PLAN.json]`
//! the owner names, the same syntax `agent-compare` takes. They run through
//! `agent_eval::evaluate_many` and are kept per build and tier ([`control_path`]).
//!
//! **What par did not reach.** The scorecard counts 14 milestones and a 540-turn horizon cannot
//! always reach them all. [`References::ceiling_lines`] says which milestones par had not met by
//! the horizon on every seed and on some. That is a statement about the stock opening on those
//! seeds, not a proof that nobody could reach them, and it is worded that way.

use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::time::Instant;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::Result;
use crate::agent_eval::paired;
use crate::agent_eval::{self, Job, Role, Strategy, Table, Track};

fn say(msg: &str) {
    eprintln!("references: {msg}");
}

/// The governor of the do-nothing control: it ends every turn and gives no order.
pub(crate) const DO_NOTHING: &str = "loop { wait(10); }";

/// The label of the do-nothing control in tables.
pub(crate) const DO_NOTHING_LABEL: &str = "do-nothing";

/// What par is, in the owner's tables.
pub(crate) const PAR_WORDS: &str = "par is what a stock scripted opening reaches on this start, \
                                     reading the world directly (it knows where things are that a \
                                     player has to discover): not a target a fair player should \
                                     hit, and not a ceiling";

// -------------------------------------------------------------------------------------------
// The cache folder
// -------------------------------------------------------------------------------------------

/// The cache folder `fn_agent` keeps its pars in, resolved the same way
/// (`fn_agent::par::cache_root`): `$FN_AGENT_CACHE`, else `$XDG_CACHE_HOME/four-nations`, else
/// `$HOME/.cache/four-nations`. Never `$FN_AGENT_RUNS`: nothing here changes what a run browser
/// lists there (#585).
pub(crate) fn cache_root() -> Option<PathBuf> {
    let from = |var: &str| std::env::var_os(var).filter(|v| !v.is_empty()).map(PathBuf::from);
    from("FN_AGENT_CACHE")
        .or_else(|| from("XDG_CACHE_HOME").map(|p| p.join("four-nations")))
        .or_else(|| from("HOME").map(|p| p.join(".cache").join("four-nations")))
}

// -------------------------------------------------------------------------------------------
// Controls
// -------------------------------------------------------------------------------------------

/// A strategy played beside the one being measured, with no model.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Control {
    pub(crate) label: String,
    pub(crate) strategy: Strategy,
}

impl Control {
    pub(crate) fn do_nothing() -> Self {
        Self {
            label: DO_NOTHING_LABEL.to_owned(),
            strategy: Strategy { governor: DO_NOTHING.to_owned(), plan: None },
        }
    }

    /// `GOVERNOR.rhai[@PLAN.json]`, the syntax `agent-compare` takes for a fixed governor. The
    /// label is the governor's file name.
    pub(crate) fn from_spec(spec: &str) -> Result<Self> {
        let (file, plan_file) = match spec.split_once('@') {
            Some((f, p)) => (f, Some(p)),
            None => (spec, None),
        };
        let p = Path::new(file);
        if !p.is_file() {
            return Err(format!(
                "--control {spec}: {file} is not a file (a control is GOVERNOR.rhai[@PLAN.json])"
            ));
        }
        let governor =
            fs::read_to_string(p).map_err(|e| format!("can't read {}: {e}", p.display()))?;
        if governor.trim().is_empty() {
            return Err(format!("--control {spec}: {file} is empty"));
        }
        let plan = match plan_file {
            None => None,
            Some(pf) => {
                let text = fs::read_to_string(pf)
                    .map_err(|e| format!("--control {spec}: can't read the plan {pf}: {e}"))?;
                let v: Value = serde_json::from_str(&text)
                    .map_err(|e| format!("--control {spec}: {pf} isn't JSON: {e}"))?;
                if !v.is_object() {
                    return Err(format!("--control {spec}: {pf}: a plan is a JSON object"));
                }
                v.as_object().is_some_and(|m| !m.is_empty()).then_some(v)
            }
        };
        let label =
            p.file_stem().map_or("control".to_owned(), |n| n.to_string_lossy().into_owned());
        Ok(Self { label, strategy: Strategy { governor, plan } })
    }
}

/// The do-nothing control and the owner's own, with labels that don't repeat.
pub(crate) fn controls_for(specs: &[String], with_do_nothing: bool) -> Result<Vec<Control>> {
    let mut out = Vec::new();
    if with_do_nothing {
        out.push(Control::do_nothing());
    }
    for spec in specs {
        out.push(Control::from_spec(spec)?);
    }
    let mut labels: Vec<&str> = out.iter().map(|c| c.label.as_str()).collect();
    labels.sort_unstable();
    if let Some(w) = labels.windows(2).find(|w| w[0] == w[1]) {
        return Err(format!("two controls are called {:?}", w[0]));
    }
    Ok(out)
}

// -------------------------------------------------------------------------------------------
// What is measured
// -------------------------------------------------------------------------------------------

/// Par on one seed.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub(crate) struct ParSeed {
    pub(crate) seed: u64,
    /// The tier word (`screen`, `gate`, `owner`).
    pub(crate) tier: String,
    pub(crate) score: f64,
    pub(crate) progress: f64,
    pub(crate) care: f64,
    pub(crate) deaths: u32,
    /// How many milestones par had met by the horizon, of `total`.
    pub(crate) reached: u32,
    pub(crate) total: u32,
    /// The names of the milestones par had not met.
    pub(crate) unreached: Vec<String>,
}

/// A control on one seed.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub(crate) struct ControlSeed {
    pub(crate) seed: u64,
    pub(crate) tier: String,
    pub(crate) score: f64,
    pub(crate) progress: f64,
    pub(crate) care: f64,
    pub(crate) deaths: u32,
    /// The reason the script stopped, if it did.
    pub(crate) stopped: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub(crate) struct ControlResult {
    pub(crate) label: String,
    pub(crate) seeds: Vec<ControlSeed>,
}

/// Par and the controls for a track's tiers, as the owner's tables print them. Every field is
/// optional or may be empty, so a ledger written without them still reads, and a reader that does
/// not know them ignores them.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub(crate) struct References {
    /// The build they were measured on.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub(crate) build: String,
    /// The horizon in turns.
    #[serde(default)]
    pub(crate) horizon: u32,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub(crate) par: Vec<ParSeed>,
    /// Why par is missing, when it could not be made.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) par_error: Option<String>,
    /// Seconds the par pass took, and how many pars it had to compute (the rest were cached).
    /// Recorded for the owner to read in the ledger; the printed table leaves them out, so the
    /// same ledger prints the same bytes whether the pars were cached or not.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) par_secs: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) par_computed: Option<u32>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub(crate) controls: Vec<ControlResult>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) controls_error: Option<String>,
}

// -------------------------------------------------------------------------------------------
// A small parallel queue
// -------------------------------------------------------------------------------------------

/// `f` over `items` on up to `threads` threads. The results are in the order of `items`
/// whichever finishes first; the first failure stops new work from starting, and the error
/// returned is that of the earliest item that failed.
pub(crate) fn parallel_map<T: Sync, R: Send>(
    items: &[T],
    threads: usize,
    f: impl Fn(&T) -> Result<R> + Sync,
) -> Result<Vec<R>> {
    let next = AtomicUsize::new(0);
    let failed = AtomicBool::new(false);
    let results: Mutex<Vec<Option<Result<R>>>> =
        Mutex::new((0..items.len()).map(|_| None).collect());
    let workers = threads.clamp(1, items.len().max(1));
    std::thread::scope(|scope| {
        for _ in 0..workers {
            scope.spawn(|| {
                loop {
                    if failed.load(Ordering::Relaxed) {
                        return;
                    }
                    let i = next.fetch_add(1, Ordering::Relaxed);
                    let Some(item) = items.get(i) else { return };
                    let r = f(item);
                    if r.is_err() {
                        failed.store(true, Ordering::Relaxed);
                    }
                    results.lock().expect("no worker panics holding the lock")[i] = Some(r);
                }
            });
        }
    });
    let mut out = Vec::new();
    for r in results.into_inner().expect("the workers are done") {
        match r {
            Some(Ok(v)) => out.push(v),
            Some(Err(e)) => return Err(e),
            None => {}
        }
    }
    Ok(out)
}

// -------------------------------------------------------------------------------------------
// Par
// -------------------------------------------------------------------------------------------

/// Reads one `fn_agent par --json` report.
pub(crate) fn parse_par(seed: u64, role: Role, stdout: &str) -> Result<(ParSeed, bool)> {
    let v: Value = serde_json::from_str(stdout.trim())
        .map_err(|e| format!("seed {seed}: fn_agent par's report isn't JSON: {e}"))?;
    let num = |key: &str| {
        v[key].as_f64().ok_or_else(|| format!("seed {seed}: the par report has no {key}"))
    };
    let ms = v["milestones"].as_array().ok_or_else(|| format!("seed {seed}: no milestones"))?;
    let unreached: Vec<String> = ms
        .iter()
        .filter(|m| m["met_turn"].is_null())
        .filter_map(|m| m["name"].as_str().map(str::to_owned))
        .collect();
    let seed_row = ParSeed {
        seed,
        tier: role.word().to_owned(),
        score: num("score")?,
        progress: num("progress")?,
        care: num("care")?,
        deaths: v["deaths"].as_u64().map_or(0, |d| u32::try_from(d).unwrap_or(u32::MAX)),
        reached: (ms.len() - unreached.len()) as u32,
        total: ms.len() as u32,
        unreached,
    };
    Ok((seed_row, v["cached"].as_bool().unwrap_or(false)))
}

/// Par on every seed of `tiers`, in parallel. The result is in tier and seed order. Returns the
/// time it took and how many pars `fn_agent` had to compute (the rest came from its cache).
pub(crate) fn par_many(
    fn_agent: &Path,
    track: &Track,
    tiers: &[Role],
    threads: usize,
) -> Result<(Vec<ParSeed>, u64, u32)> {
    let mut jobs: Vec<(Role, u64)> = Vec::new();
    for &role in tiers {
        for &seed in track.seeds(role) {
            jobs.push((role, seed));
        }
    }
    let started = Instant::now();
    let done = parallel_map(&jobs, threads, |&(role, seed)| {
        let mut cmd = Command::new(fn_agent);
        cmd.arg("par").args(track.start_args(seed)).args([
            "--horizon",
            &track.horizon_turns.to_string(),
            "--json",
        ]);
        let out = agent_eval::run_bounded(
            &mut cmd,
            agent_eval::play_timeout(),
            &format!("par for seed {seed}"),
            fn_agent,
        )?;
        if !out.status.success() {
            return Err(format!(
                "seed {seed}: fn_agent par failed ({}): {}",
                out.status,
                String::from_utf8_lossy(&out.stderr).trim()
            ));
        }
        parse_par(seed, role, &String::from_utf8_lossy(&out.stdout))
    })?;
    let computed = done.iter().filter(|(_, cached)| !cached).count() as u32;
    Ok((done.into_iter().map(|(p, _)| p).collect(), started.elapsed().as_secs(), computed))
}

// -------------------------------------------------------------------------------------------
// Controls, cached per build and tier
// -------------------------------------------------------------------------------------------

/// What decides the controls' scores besides the build and the run: `data/score.toml` as this
/// xtask was built with, and the profile `fn_agent` is told to use (`FN_SCORE_PROFILE`, set from
/// the track before anything is spawned).
static SCORE_DATA_KEY: std::sync::LazyLock<String> = std::sync::LazyLock::new(|| {
    let profile = std::env::var(agent_eval::SCORE_PROFILE_ENV).unwrap_or_default();
    format!("{:016x}:{profile}", fnv(include_str!("../../data/score.toml").as_bytes()))
});

/// FNV-1a over `bytes`: a stable hash for a file name (std's hasher is free to change).
fn fnv(bytes: &[u8]) -> u64 {
    bytes
        .iter()
        .fold(0xcbf2_9ce4_8422_2325, |h, &b| (h ^ u64::from(b)).wrapping_mul(0x0100_0000_01b3))
}

/// The file a control's results for one tier are kept in: its label, the tier and a hash of
/// everything that decides them besides the build (the start, the horizon, the tier's seeds, the
/// governor and the plan), under the build's folder.
pub(crate) fn control_path(
    root: &Path,
    build: &str,
    track: &Track,
    tier: Role,
    control: &Control,
) -> PathBuf {
    let plan = control.strategy.plan.as_ref().map_or(String::new(), Value::to_string);
    // The score data too: a control's results are scores, so another weight or profile is
    // another file even on the same build id (CI pins one for every commit).
    let what = format!(
        "{}|{}|{}|{}|{}|{:?}|{}|{:?}|{}|{}",
        SCORE_DATA_KEY.as_str(),
        track.nation,
        track.difficulty,
        track.map_size,
        track.site,
        track.prelude,
        track.horizon_turns,
        track.seeds(tier),
        control.strategy.governor,
        plan
    );
    let label: String = control
        .label
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() || c == '-' { c } else { '_' })
        .collect();
    root.join("controls").join(build).join(format!(
        "{}-{label}-{}-{:016x}.json",
        track.id,
        tier.word(),
        fnv(what.as_bytes())
    ))
}

fn read_control(path: &Path, tier: Role, seeds: &[u64]) -> Option<Vec<ControlSeed>> {
    let rows: Vec<ControlSeed> = serde_json::from_str(&fs::read_to_string(path).ok()?).ok()?;
    let same = rows.len() == seeds.len()
        && rows.iter().zip(seeds).all(|(r, s)| r.seed == *s && r.tier == tier.word());
    same.then_some(rows)
}

fn write_control(path: &Path, rows: &[ControlSeed]) {
    let Some(dir) = path.parent() else { return };
    let Ok(text) = serde_json::to_string_pretty(rows) else { return };
    if fs::create_dir_all(dir).is_err() {
        return;
    }
    let name = path.file_name().map_or(String::new(), |n| n.to_string_lossy().into_owned());
    let tmp = dir.join(format!(".{}.{name}.tmp", std::process::id()));
    if fs::write(&tmp, text).is_ok() && fs::rename(&tmp, path).is_err() {
        let _ = fs::remove_file(&tmp);
    }
}

fn control_seeds(table: &Table, tier: Role) -> Vec<ControlSeed> {
    table
        .rows_of(tier)
        .map(|r| ControlSeed {
            seed: r.seed,
            tier: tier.word().to_owned(),
            score: r.score,
            progress: r.progress,
            care: r.care,
            deaths: r.deaths,
            stopped: r.stopped.as_ref().map(|s| s.reason.clone()),
        })
        .collect()
}

/// Where the controls' results are kept and their logs go.
pub(crate) struct Place<'a> {
    /// The folder to keep results in (`None`: keep nothing). A build named `unknown` is never
    /// cached.
    pub(crate) cache: Option<&'a Path>,
    /// The build `fn_agent` is: results are kept per build.
    pub(crate) build: &'a str,
    /// A scratch folder for the logs, which the caller removes.
    pub(crate) work: &'a Path,
}

/// The controls' clean scores on `tiers`: from the cache where this build has played that
/// control on that tier already, else played now (`evaluate_many`, one queue per tier) and kept.
pub(crate) fn controls_many(
    fn_agent: &Path,
    track: &Track,
    controls: &[Control],
    tiers: &[Role],
    threads: usize,
    place: &Place,
) -> Result<Vec<ControlResult>> {
    let Place { build, work, .. } = *place;
    let cache = place.cache.filter(|_| build != "unknown");
    let mut results: Vec<ControlResult> = controls
        .iter()
        .map(|c| ControlResult { label: c.label.clone(), seeds: Vec::new() })
        .collect();
    for &tier in tiers {
        let mut missing: Vec<usize> = Vec::new();
        for (i, c) in controls.iter().enumerate() {
            let hit = cache.and_then(|root| {
                read_control(&control_path(root, build, track, tier, c), tier, track.seeds(tier))
            });
            match hit {
                Some(rows) => results[i].seeds.extend(rows),
                None => missing.push(i),
            }
        }
        if missing.is_empty() {
            continue;
        }
        let jobs: Vec<Job> = missing
            .iter()
            .map(|&i| Job {
                strategy: &controls[i].strategy,
                work: work.join(format!("{}-{}", tier.word(), controls[i].label)),
            })
            .collect();
        let tables = agent_eval::evaluate_many(fn_agent, track, &jobs, &[tier], threads)?;
        for (&i, table) in missing.iter().zip(&tables) {
            let rows = control_seeds(table, tier);
            if let Some(root) = cache {
                write_control(&control_path(root, build, track, tier, &controls[i]), &rows);
            }
            results[i].seeds.extend(rows);
        }
    }
    Ok(results)
}

// -------------------------------------------------------------------------------------------
// Gathering, and the owner's text
// -------------------------------------------------------------------------------------------

/// What to gather.
pub(crate) struct Want<'a> {
    pub(crate) par: bool,
    pub(crate) controls: &'a [Control],
}

/// Par and the controls on `tiers`. A failure to make par, or a control, is kept in the result
/// as a note and does not fail the caller: these are context beside a score.
pub(crate) fn gather(
    fn_agent: &Path,
    track: &Track,
    tiers: &[Role],
    want: &Want,
    threads: usize,
    build: &str,
) -> References {
    let mut refs = References {
        build: build.to_owned(),
        horizon: track.horizon_turns,
        ..References::default()
    };
    if want.par {
        let n: usize = tiers.iter().map(|&t| track.seeds(t).len()).sum();
        say(&format!(
            "par for {n} seed(s) on {threads} thread(s): a par is computed once per build and kept \
             ({}), so only the first can take tens of seconds",
            cache_root().map_or("no cache folder".to_owned(), |p| p.display().to_string())
        ));
        match par_many(fn_agent, track, tiers, threads) {
            Ok((par, secs, computed)) => {
                say(&format!(
                    "par: {secs} s, {computed} computed and {} cached",
                    n as u32 - computed
                ));
                refs.par = par;
                refs.par_secs = Some(secs);
                refs.par_computed = Some(computed);
            }
            Err(e) => {
                say(&format!("no par: {e}"));
                refs.par_error = Some(e);
            }
        }
    }
    if !want.controls.is_empty() {
        let work =
            std::env::temp_dir().join(format!("fn-agent-controls-{}-{build}", std::process::id()));
        let cache = cache_root();
        let place = Place { cache: cache.as_deref(), build, work: &work };
        let result = controls_many(fn_agent, track, want.controls, tiers, threads, &place);
        let _ = fs::remove_dir_all(&work);
        match result {
            Ok(c) => refs.controls = c,
            Err(e) => {
                say(&format!("no controls: {e}"));
                refs.controls_error = Some(e);
            }
        }
    }
    refs
}

fn mean_of(xs: impl Iterator<Item = f64>) -> Option<f64> {
    let v: Vec<f64> = xs.collect();
    (!v.is_empty()).then(|| paired::mean(&v))
}

/// A tier's scores in a cell: the seeds one by one when there are few, else the mean.
fn tier_cell(scores: &[f64]) -> String {
    match scores.len() {
        0 => "-".to_owned(),
        1..=4 => scores.iter().map(|s| format!("{s:.0}")).collect::<Vec<_>>().join("/"),
        n => format!("mean {:.0} ({n} seeds)", paired::mean(scores)),
    }
}

impl References {
    pub(crate) fn is_empty(&self) -> bool {
        self.par.is_empty()
            && self.controls.is_empty()
            && self.par_error.is_none()
            && self.controls_error.is_none()
    }

    pub(crate) fn par_of(&self, seed: u64) -> Option<&ParSeed> {
        self.par.iter().find(|p| p.seed == seed)
    }

    pub(crate) fn control_of(&self, label: &str, seed: u64) -> Option<&ControlSeed> {
        self.controls.iter().find(|c| c.label == label)?.seeds.iter().find(|s| s.seed == seed)
    }

    /// The mean of par on one tier's seeds.
    pub(crate) fn par_mean(&self, tier: Role) -> Option<f64> {
        mean_of(self.par.iter().filter(|p| p.tier == tier.word()).map(|p| p.score))
    }

    /// The milestones par had not met by the horizon, with on how many of the seeds: those on
    /// every seed first. `(name, seeds missed, seeds with a par)`.
    pub(crate) fn unreached(&self) -> Vec<(String, usize, usize)> {
        let total = self.par.len();
        let mut names: Vec<String> = Vec::new();
        for p in &self.par {
            for n in &p.unreached {
                if !names.contains(n) {
                    names.push(n.clone());
                }
            }
        }
        let mut out: Vec<(String, usize, usize)> = names
            .into_iter()
            .map(|n| {
                let c = self.par.iter().filter(|p| p.unreached.contains(&n)).count();
                (n, c, total)
            })
            .collect();
        out.sort_by_key(|(_, c, _)| std::cmp::Reverse(*c));
        out
    }

    /// What par reached by the horizon, as the practical ceiling's honest statement: which
    /// milestones it had not met on any seed and on some. Empty without a par.
    pub(crate) fn ceiling_lines(&self) -> Vec<String> {
        if self.par.is_empty() {
            return Vec::new();
        }
        let missed = self.unreached();
        let total = self.par.len();
        let none: Vec<&str> =
            missed.iter().filter(|(_, c, _)| *c == total).map(|(n, _, _)| n.as_str()).collect();
        let some: Vec<String> = missed
            .iter()
            .filter(|(_, c, _)| *c < total)
            .map(|(n, c, _)| format!("{n} ({c}/{total})"))
            .collect();
        let mut lines = Vec::new();
        let mut head = format!("by turn {}, par had not met", self.horizon);
        if none.is_empty() && some.is_empty() {
            let _ = write!(head, " no milestone on any of {total} seed(s)");
        } else {
            if !none.is_empty() {
                let _ = write!(head, " on any of {total} seed(s): {}", none.join(", "));
            }
            if !some.is_empty() {
                let sep = if none.is_empty() { ":" } else { ";" };
                let _ = write!(head, "{sep} on some: {}", some.join(", "));
            }
        }
        lines.push(head);
        lines.push(
            "that is what the stock opening reached, not a proof the rest cannot be reached by the \
             horizon; no score changes"
                .to_owned(),
        );
        lines
    }

    /// The block under a table: par and each control, a cell per tier, then par's words and what
    /// it had not reached. `tiers` are the tiers to show, in order.
    pub(crate) fn text(&self, tiers: &[Role]) -> String {
        let mut out = String::new();
        let mut rows: Vec<(String, Vec<String>)> = Vec::new();
        if !self.par.is_empty() {
            rows.push((
                "par".to_owned(),
                tiers
                    .iter()
                    .map(|t| {
                        let s: Vec<f64> = self
                            .par
                            .iter()
                            .filter(|p| p.tier == t.word())
                            .map(|p| p.score)
                            .collect();
                        tier_cell(&s)
                    })
                    .collect(),
            ));
        }
        for c in &self.controls {
            rows.push((
                c.label.clone(),
                tiers
                    .iter()
                    .map(|t| {
                        let s: Vec<f64> = c
                            .seeds
                            .iter()
                            .filter(|x| x.tier == t.word())
                            .map(|x| x.score)
                            .collect();
                        tier_cell(&s)
                    })
                    .collect(),
            ));
        }
        if !rows.is_empty() {
            let note = if self.build.is_empty() {
                String::new()
            } else {
                format!(" (build {})", self.build)
            };
            let _ = writeln!(out, " references{note}");
            let w = rows.iter().map(|(l, _)| l.chars().count()).max().unwrap_or(0);
            for (label, cells) in &rows {
                let cols: Vec<String> =
                    tiers.iter().zip(cells).map(|(t, c)| format!("{} {c}", t.word())).collect();
                let _ = writeln!(out, "   {label:<w$}  {}", cols.join(" \u{b7} "));
            }
        }
        if !self.par.is_empty() {
            let _ = writeln!(out, " {PAR_WORDS}.");
            for line in self.ceiling_lines() {
                let _ = writeln!(out, " {line}.");
            }
        }
        if let Some(e) = &self.par_error {
            let _ = writeln!(out, " no par: {e}");
        }
        if let Some(e) = &self.controls_error {
            let _ = writeln!(out, " no controls: {e}");
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn par_seed(seed: u64, tier: &str, score: f64, unreached: &[&str]) -> ParSeed {
        ParSeed {
            seed,
            tier: tier.into(),
            score,
            progress: score / 1000.0,
            care: 1.0,
            deaths: 0,
            reached: 14 - unreached.len() as u32,
            total: 14,
            unreached: unreached.iter().map(|s| (*s).to_owned()).collect(),
        }
    }

    fn control_seed(seed: u64, tier: &str, score: f64) -> ControlSeed {
        ControlSeed {
            seed,
            tier: tier.into(),
            score,
            progress: 0.0,
            care: 1.0,
            deaths: 0,
            stopped: None,
        }
    }

    fn refs() -> References {
        References {
            build: "abc1234".into(),
            horizon: 540,
            par: vec![
                par_seed(1, "screen", 517.0, &["Ready", "Foundation", "Pillars", "Arch", "Rite"]),
                par_seed(
                    2,
                    "screen",
                    276.0,
                    &["Ready", "Foundation", "Pillars", "Arch", "Rite", "Population"],
                ),
                par_seed(101, "gate", 480.0, &["Ready", "Foundation", "Pillars", "Arch", "Rite"]),
                par_seed(102, "gate", 500.0, &["Ready", "Foundation", "Pillars", "Arch", "Rite"]),
                par_seed(103, "gate", 520.0, &["Ready", "Foundation", "Pillars", "Arch", "Rite"]),
                par_seed(104, "gate", 460.0, &["Ready", "Foundation", "Pillars", "Arch", "Rite"]),
                par_seed(105, "gate", 540.0, &["Ready", "Foundation", "Pillars", "Arch", "Rite"]),
            ],
            par_secs: Some(31),
            par_computed: Some(2),
            controls: vec![ControlResult {
                label: "do-nothing".into(),
                seeds: vec![
                    control_seed(1, "screen", 13.0),
                    control_seed(2, "screen", 12.0),
                    control_seed(101, "gate", 14.0),
                ],
            }],
            ..References::default()
        }
    }

    #[test]
    fn the_text_gives_par_and_the_controls_per_tier_in_the_owners_words() {
        let t = refs().text(&[Role::Screen, Role::Gate]);
        assert!(t.contains("references (build abc1234)"), "{t}");
        assert!(t.contains("par         screen 517/276 \u{b7} gate mean 500 (5 seeds)"), "{t}");
        assert!(t.contains("do-nothing  screen 13/12 \u{b7} gate 14"), "{t}");
        assert!(
            t.contains(
                "what a stock scripted opening reaches on this start, reading the world directly"
            ),
            "{t}"
        );
        assert!(t.contains("not a target a fair player should hit, and not a ceiling"), "{t}");
        assert!(!t.contains("took"), "the time is on stderr and in the ledger, not in the table");
    }

    /// The practical ceiling is worded as what par reached: never "can't be reached".
    #[test]
    fn the_ceiling_says_what_par_had_not_met_and_not_what_cannot_be_reached() {
        let lines = refs().ceiling_lines();
        assert_eq!(
            lines[0],
            "by turn 540, par had not met on any of 7 seed(s): Ready, Foundation, Pillars, Arch, Rite; on some: Population (1/7)"
        );
        assert!(lines[1].contains("not a proof"), "{lines:?}");
        for l in &lines {
            assert!(!l.contains("cannot be reached.") && !l.contains("unreachable"), "{l}");
        }
        assert!(References::default().ceiling_lines().is_empty());
        let mut all = refs();
        for p in &mut all.par {
            p.unreached.clear();
        }
        assert!(all.ceiling_lines()[0].contains("no milestone on any of 7 seed(s)"));
    }

    #[test]
    fn an_empty_or_failed_set_says_so() {
        assert!(References::default().is_empty());
        let r = References { par_error: Some("seed 5: boom".into()), ..References::default() };
        assert!(!r.is_empty());
        assert!(r.text(&[Role::Screen]).contains("no par: seed 5: boom"));
    }

    #[test]
    fn it_round_trips_and_reads_back_when_fields_are_missing() {
        let r = refs();
        let back: References = serde_json::from_str(&serde_json::to_string(&r).unwrap()).unwrap();
        assert_eq!(back, r);
        let bare: References = serde_json::from_str("{}").unwrap();
        assert_eq!(bare, References::default());
    }

    #[test]
    fn a_par_report_parses_into_a_row() {
        let json = r#"{"build":"b","horizon":540,"score":517.4,"progress":0.52,"care":1.0,
            "deaths":0,"cached":true,"milestones":[
            {"id":"a","name":"Population","kind":"sign","met_turn":120,"met_now":true,"credit":0.8},
            {"id":"ready","name":"Ready","kind":"ready","met_turn":null,"met_now":false,"credit":0.0}]}"#;
        let (row, cached) = parse_par(7, Role::Gate, json).unwrap();
        assert!(cached);
        assert_eq!((row.seed, row.tier.as_str(), row.reached, row.total), (7, "gate", 1, 2));
        assert_eq!(row.unreached, ["Ready"]);
        assert!(parse_par(7, Role::Gate, "nope").unwrap_err().contains("seed 7"));
        assert!(parse_par(7, Role::Gate, r#"{"milestones":[]}"#).unwrap_err().contains("score"));
    }

    #[test]
    fn parallel_map_keeps_order_and_reports_the_earliest_failure() {
        let items: Vec<u64> = (0..20).collect();
        for threads in [1, 3, 64] {
            let got = parallel_map(&items, threads, |&i| {
                std::thread::sleep(std::time::Duration::from_millis(20 - i));
                Ok(i * 2)
            })
            .unwrap();
            assert_eq!(got, (0..20).map(|i| i * 2).collect::<Vec<_>>());
        }
        let e = parallel_map(&items, 4, |&i| {
            if i == 3 || i == 9 {
                std::thread::sleep(std::time::Duration::from_millis(if i == 3 { 100 } else { 0 }));
                Err(format!("item {i}"))
            } else {
                Ok(i)
            }
        })
        .unwrap_err();
        assert_eq!(e, "item 3", "the earliest in queue order, not the first to finish");
        assert!(parallel_map::<u64, u64>(&[], 4, |&i| Ok(i)).unwrap().is_empty());
    }

    #[test]
    fn controls_parse_the_agent_compare_syntax() {
        let dir = std::env::temp_dir().join(format!("fn-refs-ctl-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("gov.rhai"), "wait(5);").unwrap();
        fs::write(dir.join("plan.json"), r#"{"k":1}"#).unwrap();
        fs::write(dir.join("empty.json"), "{}").unwrap();
        fs::write(dir.join("list.json"), "[]").unwrap();
        let gov = dir.join("gov.rhai").display().to_string();
        let c = Control::from_spec(&gov).unwrap();
        assert_eq!((c.label.as_str(), c.strategy.plan.clone()), ("gov", None));
        let c = Control::from_spec(&format!("{gov}@{}", dir.join("plan.json").display())).unwrap();
        assert_eq!(c.strategy.plan, Some(serde_json::json!({"k": 1})));
        let c = Control::from_spec(&format!("{gov}@{}", dir.join("empty.json").display())).unwrap();
        assert_eq!(c.strategy.plan, None, "an empty plan is no plan, as in agent-compare");
        assert!(Control::from_spec(&format!("{gov}@{}", dir.join("list.json").display())).is_err());
        assert!(Control::from_spec(&dir.join("nope.rhai").display().to_string()).is_err());
        let both = controls_for(std::slice::from_ref(&gov), true).unwrap();
        assert_eq!(both[0].label, "do-nothing");
        assert_eq!(both[0].strategy.governor, "loop { wait(10); }");
        assert!(controls_for(&[gov.clone(), gov], false).unwrap_err().contains("two controls"));
        let _ = fs::remove_dir_all(&dir);
    }
}
