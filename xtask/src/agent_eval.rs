//! `cargo xtask agent-eval --track ID --champion DIR [options]`
//!
//! The clean evaluation of a strategy on a track (docs/design/agent-hillclimb.md, "Across runs:
//! attempts and a champion" and "Measuring a difference"; #529, #571). CPU only: it calls no
//! model.
//!
//! `DIR/handoff.json` (the file `agent-run --carry` writes) holds the strategy: the script named
//! `governor` and the plan. For every seed of the track's tiers in `data/tracks.toml` (the
//! screen, the gate and the owner-only tier), this runs `fn_agent play --script governor --plan
//! plan --horizon N` once, and prints the score per seed, each tier's mean and the failure
//! counts (collapses, deaths, stopped scripts). The runs go in parallel, up to `--threads`, and
//! the table is the same whatever the thread count.
//!
//!   --track ID       a `[[track]]` id from the tracks file (required)
//!   --champion DIR   the directory holding `handoff.json` (required)
//!   --tiers LIST     which tiers to run, comma separated: screen, gate, owner (default: all
//!                    three)
//!   --threads N      runs in parallel (default: half the cores, at most 12, fewer if memory is
//!                    short; or $FN_EVAL_THREADS)
//!   --json           print the table as JSON on stdout instead of text
//!   --out DIR        keep each run's log as `DIR/seed-N.jsonl` (default: a scratch folder,
//!                    removed afterwards)
//!   --tracks FILE    a tracks file other than `data/tracks.toml` (for tests)
//!
//! A run whose script was stopped by an error or a runaway (`fn_agent play` exits nonzero and
//! reports `stopped_by`) is still a valid result: the colony coasted to the horizon and the score
//! is read there. It is recorded and flagged in the table, not treated as a failure. Anything
//! else going wrong (an unreadable handoff, no `governor`, `fn_agent` failing to start a run) is
//! an error and prints no table.
//!
//! `fn_agent` is built in release mode first, unless `$FN_AGENT_BIN` names a binary to use
//! (tests point it at a stub or at a `ci`-profile build).
//!
//! The evaluation lives in [`evaluate_many`], which the attempt loop (#530) and `agent-compare`
//! (#571) reuse; the statistics are in [`paired`].

use std::fmt::Write as _;
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::Result;
use crate::agent_run::{Handoff, NATIONS, cargo_build_release};

pub(crate) mod paired;

fn say(msg: &str) {
    eprintln!("agent-eval: {msg}");
}

fn help() -> String {
    include_str!("agent_eval.rs")
        .lines()
        .skip(1)
        .take_while(|l| l.starts_with("//!"))
        .map(|l| l.trim_start_matches("//!"))
        .collect::<Vec<_>>()
        .join("\n")
}

// -------------------------------------------------------------------------------------------
// data/tracks.toml
// -------------------------------------------------------------------------------------------

/// One `[[track]]`: a start family scored at a fixed horizon (see `data/tracks.toml`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Track {
    pub(crate) id: String,
    pub(crate) name: String,
    pub(crate) nation: String,
    pub(crate) difficulty: String,
    pub(crate) map_size: String,
    /// `"default"` or `"X,Y"`.
    pub(crate) site: String,
    pub(crate) prelude: Option<TrackPrelude>,
    pub(crate) horizon_turns: u32,
    pub(crate) play_seed: u64,
    /// The screen tier: the seeds the model tests on (`test_governor`) and the pairs of an
    /// attempt are first ranked on.
    pub(crate) screen_seeds: Vec<u64>,
    /// The gate tier: the seeds a candidate and the champion are compared on, paired, to decide
    /// the promotion. The model never names one.
    pub(crate) gate_seeds: Vec<u64>,
    /// The owner-only tier: reported to the owner, never selected on, never in a prompt. None is
    /// in the balance panel (1-32).
    pub(crate) owner_seeds: Vec<u64>,
    /// A score below this is a collapse: the failure count the gate reports and its second
    /// promotion condition reads.
    pub(crate) collapse_below: f64,
}

/// The seeds the balance harness tunes on (its panel). The gate and owner-only tiers avoid them,
/// so a result on them is not one the game was tuned against.
pub(crate) const BALANCE_PANEL: std::ops::RangeInclusive<u64> = 1..=32;

/// A track's optional prelude, as `data/scenarios.toml` writes one.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct TrackPrelude {
    /// `"economy"` or `"opening"`.
    pub(crate) opening: String,
    pub(crate) minutes: f64,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct TrackFile {
    #[serde(rename = "track")]
    tracks: Vec<Track>,
}

impl Track {
    /// What a track must satisfy beyond parsing: the checks that keep selection honest.
    fn validate(&self) -> Result {
        let id = &self.id;
        let bad = |why: &str| Err(format!("track {id:?}: {why}"));
        if id.is_empty() || id.chars().any(|c| !(c.is_ascii_alphanumeric() || c == '-' || c == '_'))
        {
            return bad("id wants letters, digits, '-' and '_' only");
        }
        if !NATIONS.contains(&self.nation.as_str()) {
            return bad(&format!("nation {:?} isn't one of {}", self.nation, NATIONS.join(", ")));
        }
        if !["small", "medium", "large"].contains(&self.map_size.as_str()) {
            return bad(&format!("map_size {:?} isn't small, medium or large", self.map_size));
        }
        if self.site_flag().is_err() {
            return bad(&format!("site wants \"default\" or \"X,Y\", got {:?}", self.site));
        }
        if let Some(p) = &self.prelude
            && (!["economy", "opening"].contains(&p.opening.as_str())
                || !p.minutes.is_finite()
                || p.minutes <= 0.0)
        {
            return bad("prelude wants opening = \"economy\" or \"opening\" and minutes > 0");
        }
        if self.horizon_turns == 0 {
            return bad("horizon_turns wants at least 1");
        }
        if self.screen_seeds.is_empty() {
            return bad("screen_seeds is empty: an attempt needs at least one seed to test on");
        }
        if self.gate_seeds.is_empty() {
            return bad("gate_seeds is empty: a promotion needs seeds to compare on");
        }
        if self.owner_seeds.is_empty() {
            return bad("owner_seeds is empty: the owner needs seeds to be told about");
        }
        if !self.collapse_below.is_finite() || self.collapse_below <= 0.0 {
            return bad("collapse_below wants a score above 0");
        }
        let tiers = [
            ("screen_seeds", &self.screen_seeds),
            ("gate_seeds", &self.gate_seeds),
            ("owner_seeds", &self.owner_seeds),
        ];
        let mut seen = std::collections::BTreeMap::new();
        for (name, seeds) in tiers {
            for s in seeds {
                if let Some(first) = seen.insert(*s, name) {
                    return bad(&format!(
                        "seed {s} appears twice across the tiers ({first} and {name}); they \
                         must not overlap"
                    ));
                }
            }
        }
        for (name, seeds) in &tiers[1..] {
            if let Some(s) = seeds.iter().find(|s| BALANCE_PANEL.contains(s)) {
                return bad(&format!(
                    "{name} has seed {s}, which is in the balance panel 1-32 the game is tuned on"
                ));
            }
            if seeds.contains(&self.play_seed) {
                return bad(&format!(
                    "play_seed is in {name}: the model plays it, so it is not a fresh seed"
                ));
            }
        }
        Ok(())
    }

    /// The `--start` value: `Some("X,Y")` for a founded site, `None` for the default start.
    pub(crate) fn site_flag(&self) -> Result<Option<&str>> {
        if self.site == "default" {
            return Ok(None);
        }
        let ok = self.site.split_once(',').is_some_and(|(x, y)| {
            x.trim().parse::<i32>().is_ok() && y.trim().parse::<i32>().is_ok()
        });
        if ok { Ok(Some(&self.site)) } else { Err(format!("bad site {:?}", self.site)) }
    }

    /// The `fn_agent play` flags that set up this track's world on `seed` (everything but the
    /// script, plan, horizon and log).
    pub(crate) fn start_args(&self, seed: u64) -> Vec<String> {
        let mut a = vec![
            "--seed".to_owned(),
            seed.to_string(),
            "--nation".to_owned(),
            self.nation.clone(),
            "--difficulty".to_owned(),
            self.difficulty.clone(),
            "--map-size".to_owned(),
            self.map_size.clone(),
        ];
        if let Ok(Some(site)) = self.site_flag() {
            a.extend(["--start".to_owned(), site.to_owned()]);
        }
        if let Some(p) = &self.prelude {
            a.extend(["--prelude".to_owned(), format!("{}:{}", p.opening, p.minutes)]);
        }
        a
    }
}

/// Parses and validates a tracks file's text.
pub(crate) fn parse_tracks(src: &str) -> Result<Vec<Track>> {
    let file: TrackFile = toml::from_str(src).map_err(|e| format!("tracks file: {e}"))?;
    let mut ids = std::collections::BTreeSet::new();
    for t in &file.tracks {
        t.validate()?;
        if !ids.insert(t.id.clone()) {
            return Err(format!("track id {:?} appears twice", t.id));
        }
    }
    Ok(file.tracks)
}

/// The track named `id` in the tracks file at `path`, or an error that lists the ids there.
pub(crate) fn load_track(path: &Path, id: &str) -> Result<Track> {
    let src = fs::read_to_string(path)
        .map_err(|e| format!("can't read tracks file {}: {e}", path.display()))?;
    let tracks = parse_tracks(&src)?;
    let known: Vec<String> = tracks.iter().map(|t| t.id.clone()).collect();
    tracks.into_iter().find(|t| t.id == id).ok_or_else(|| {
        format!("unknown track {id:?} in {}; known tracks: {}", path.display(), known.join(", "))
    })
}

// -------------------------------------------------------------------------------------------
// The strategy (a champion's handoff)
// -------------------------------------------------------------------------------------------

/// The script name a strategy's governor is saved under.
const GOVERNOR: &str = "governor";

/// A strategy: the governor's code and the plan (an empty object when none was set).
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Strategy {
    pub(crate) governor: String,
    pub(crate) plan: Option<Value>,
}

/// Reads `DIR/handoff.json`; a clear error when it's missing or has no `governor` script.
pub(crate) fn read_strategy(dir: &Path) -> Result<Strategy> {
    let path = dir.join("handoff.json");
    let text =
        fs::read_to_string(&path).map_err(|e| format!("can't read {}: {e}", path.display()))?;
    let handoff: Handoff = serde_json::from_str(&text)
        .map_err(|e| format!("{} isn't a handoff: {e}", path.display()))?;
    let Some(entry) = handoff.scripts.get(GOVERNOR) else {
        let have: Vec<&str> = handoff.scripts.keys().map(String::as_str).collect();
        let have = if have.is_empty() { "no scripts".to_owned() } else { have.join(", ") };
        return Err(format!(
            "{} has no script named \"{GOVERNOR}\" (it has: {have}); a champion needs a governor",
            path.display()
        ));
    };
    if entry.code.trim().is_empty() {
        return Err(format!("the \"{GOVERNOR}\" script in {} is empty", path.display()));
    }
    Ok(Strategy { governor: entry.code.clone(), plan: handoff.plan })
}

// -------------------------------------------------------------------------------------------
// Running and the table
// -------------------------------------------------------------------------------------------

/// Which tier a seed belongs to (`data/tracks.toml`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum Role {
    /// What the model tests on, and what an attempt's pairs are first ranked on.
    Screen,
    /// What a promotion compares the candidate and the champion on.
    Gate,
    /// The owner's: reported, never selected on, never in a prompt.
    Owner,
}

impl Role {
    pub(crate) const ALL: [Role; 3] = [Role::Screen, Role::Gate, Role::Owner];

    pub(crate) fn word(self) -> &'static str {
        match self {
            Role::Screen => "screen",
            Role::Gate => "gate",
            Role::Owner => "owner",
        }
    }

    /// The tier named `word` (`screen`, `gate` or `owner`).
    pub(crate) fn parse(word: &str) -> Result<Role> {
        Role::ALL
            .into_iter()
            .find(|r| r.word() == word)
            .ok_or_else(|| format!("unknown tier {word:?}; the tiers are screen, gate and owner"))
    }
}

impl Track {
    /// The seeds of one tier.
    pub(crate) fn seeds(&self, role: Role) -> &[u64] {
        match role {
            Role::Screen => &self.screen_seeds,
            Role::Gate => &self.gate_seeds,
            Role::Owner => &self.owner_seeds,
        }
    }
}

/// Why a script stopped giving orders: `fn_agent play`'s `stopped_by`.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Stopped {
    pub(crate) reason: String,
    pub(crate) why: Option<String>,
    pub(crate) turn: u64,
}

/// One clean run's result.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Row {
    pub(crate) seed: u64,
    pub(crate) role: Role,
    pub(crate) score: f64,
    pub(crate) progress: f64,
    pub(crate) care: f64,
    /// Deaths in the run (the scorecard's `care_detail.deaths`; 0 when a report has none).
    pub(crate) deaths: u32,
    /// Set when the script was stopped by an error or a runaway: the score is still valid.
    pub(crate) stopped: Option<Stopped>,
}

/// A track's clean evaluation: a row per seed of each tier evaluated, in tier order (screen, gate,
/// owner) and in the track's seed order within a tier, whatever order the runs finished in.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Table {
    pub(crate) track: String,
    pub(crate) horizon: u32,
    /// The track's `collapse_below`, for the failure counts.
    pub(crate) collapse_below: f64,
    pub(crate) rows: Vec<Row>,
}

fn mean(xs: impl Iterator<Item = f64>) -> Option<f64> {
    let (sum, n) = xs.fold((0.0, 0u32), |(s, n), x| (s + x, n + 1));
    (n > 0).then(|| sum / f64::from(n))
}

impl Table {
    pub(crate) fn rows_of(&self, role: Role) -> impl Iterator<Item = &Row> {
        self.rows.iter().filter(move |r| r.role == role)
    }

    /// The mean score over one tier's seeds.
    pub(crate) fn mean_of(&self, role: Role) -> Option<f64> {
        mean(self.rows_of(role).map(|r| r.score))
    }

    /// The screen mean: what an attempt's governor and plan pairs are ranked on.
    pub(crate) fn screen_mean(&self) -> Option<f64> {
        self.mean_of(Role::Screen)
    }

    /// One tier's results as [`paired::Sample`]s, in seed order.
    pub(crate) fn samples(&self, role: Role) -> Vec<paired::Sample> {
        self.rows_of(role).map(paired::Sample::from).collect()
    }

    /// One tier's failure counts.
    pub(crate) fn failures(&self, role: Role) -> paired::Failures {
        paired::Failures::of(&self.samples(role), self.collapse_below)
    }

    pub(crate) fn stopped_count(&self) -> usize {
        self.rows.iter().filter(|r| r.stopped.is_some()).count()
    }

    pub(crate) fn text(&self) -> String {
        let mut out = String::new();
        let _ = writeln!(
            out,
            "agent-eval · {} · horizon {}                    (owner tier: owner only)",
            self.track, self.horizon
        );
        let _ = writeln!(
            out,
            " {:>8}  {:<7}  {:>7}  {:>8}  {:>5}  {:>6}  note",
            "seed", "tier", "score", "progress", "care", "deaths"
        );
        for r in &self.rows {
            let note = match &r.stopped {
                Some(s) => format!("stopped by {} at turn {}", s.reason, s.turn),
                None => String::new(),
            };
            let line = format!(
                " {:>8}  {:<7}  {:>7.1}  {:>8.3}  {:>5.3}  {:>6}  {note}",
                r.seed,
                r.role.word(),
                r.score,
                r.progress,
                r.care,
                r.deaths
            );
            let _ = writeln!(out, "{}", line.trim_end());
        }
        let _ = writeln!(out);
        for role in Role::ALL {
            let Some(m) = self.mean_of(role) else { continue };
            let n = self.rows_of(role).count();
            let sd = paired::sd(&self.rows_of(role).map(|r| r.score).collect::<Vec<_>>());
            let _ = writeln!(
                out,
                "{:<7} mean {m:>7.1}  sd {sd:>5.1}  {n} seed(s)   {}",
                role.word(),
                self.failures(role).text()
            );
        }
        let _ = writeln!(
            out,
            "collapse: a score below {}. A stopped script coasted to the horizon and is scored \
             as usual.",
            self.collapse_below
        );
        out
    }

    pub(crate) fn json(&self) -> Value {
        let row = |r: &Row| {
            json!({
                "seed": r.seed,
                "score": r.score,
                "progress": r.progress,
                "care": r.care,
                "deaths": r.deaths,
                "stopped_by": r.stopped.as_ref().map(|s| json!({
                    "reason": s.reason, "why": s.why, "turn": s.turn,
                })),
            })
        };
        let mut v = json!({
            "track": self.track,
            "horizon": self.horizon,
            "collapse_below": self.collapse_below,
            "stopped": self.stopped_count(),
        });
        for role in Role::ALL {
            let w = role.word();
            let f = self.failures(role);
            v[w] = json!(self.rows_of(role).map(row).collect::<Vec<_>>());
            v[format!("{w}_mean")] = json!(self.mean_of(role));
            v[format!("{w}_failures")] = json!({
                "collapses": f.collapses,
                "deaths": f.deaths,
                "death_seeds": f.death_seeds,
                "stopped": f.stopped,
            });
        }
        v
    }
}

/// Reads one `fn_agent play --horizon` report (its stdout JSON) into a [`Row`].
pub(crate) fn parse_report(seed: u64, role: Role, stdout: &str) -> Result<Row> {
    let v: Value = serde_json::from_str(stdout.trim())
        .map_err(|e| format!("seed {seed}: fn_agent's report isn't JSON: {e}"))?;
    let card = &v["scorecard"];
    let num = |key: &str| {
        card[key].as_f64().ok_or_else(|| format!("seed {seed}: the report has no scorecard.{key}"))
    };
    let stopped = match &v["stopped_by"] {
        Value::Null => None,
        s => Some(Stopped {
            reason: s["reason"].as_str().unwrap_or("error").to_owned(),
            why: s["why"].as_str().map(str::to_owned),
            turn: s["turn"].as_u64().unwrap_or(0),
        }),
    };
    let deaths =
        card["care_detail"]["deaths"].as_u64().map_or(0, |d| u32::try_from(d).unwrap_or(u32::MAX));
    Ok(Row {
        seed,
        role,
        score: num("score")?,
        progress: num("progress")?,
        care: num("care")?,
        deaths,
        stopped,
    })
}

/// How long one `fn_agent play --horizon` child may run before `evaluate_many` gives up on it (#564).
/// The script runner bounds a script's own work deterministically (`max_operations` per `wait`,
/// native calls included), so this is only the backstop for something that does not come back at
/// all. Generous: the reference governor takes about a minute and a half in a release build.
/// `$FN_AGENT_PLAY_TIMEOUT_SECS` overrides it (tests use a second).
const PLAY_TIMEOUT: Duration = Duration::from_secs(20 * 60);

fn play_timeout() -> Duration {
    std::env::var("FN_AGENT_PLAY_TIMEOUT_SECS")
        .ok()
        .and_then(|s| s.parse().ok())
        .map_or(PLAY_TIMEOUT, Duration::from_secs)
}

/// Runs `cmd` to completion for at most `limit` of wall-clock time, collecting its output. Past
/// the limit the child is killed and this is an error naming `what`: a failure of the harness,
/// reported as such and never as a score (the clock decides nothing a score reads, so a slow
/// machine can only turn a result into an error, never into a different result).
fn run_bounded(cmd: &mut Command, limit: Duration, what: &str, program: &Path) -> Result<Output> {
    cmd.stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::piped());
    let mut child = cmd.spawn().map_err(|e| format!("can't run {}: {e}", program.display()))?;
    // Drain both pipes on threads so a chatty child can't block on a full pipe.
    fn drain(mut pipe: impl Read + Send + 'static) -> std::thread::JoinHandle<Vec<u8>> {
        std::thread::spawn(move || {
            let mut buf = Vec::new();
            let _ = pipe.read_to_end(&mut buf);
            buf
        })
    }
    let stdout = drain(child.stdout.take().expect("piped"));
    let stderr = drain(child.stderr.take().expect("piped"));
    let started = Instant::now();
    let status = loop {
        if let Some(status) = child.try_wait().map_err(|e| e.to_string())? {
            break status;
        }
        if started.elapsed() >= limit {
            let _ = child.kill();
            let _ = child.wait();
            return Err(format!(
                "{what}: fn_agent play ran longer than {} s and was killed (a harness failure, \
                 not a score: the script runner bounds a script's own work, so something did \
                 not come back)",
                limit.as_secs()
            ));
        }
        std::thread::sleep(Duration::from_millis(25));
    };
    Ok(Output {
        status,
        stdout: stdout.join().unwrap_or_default(),
        stderr: stderr.join().unwrap_or_default(),
    })
}

// -------------------------------------------------------------------------------------------
// Running, in parallel (#571)
// -------------------------------------------------------------------------------------------

/// The most runs the default thread count runs at once. A governor run is one core and about
/// 30 MB (measured: 27 to 30 MB peak on a 540-turn water start), so memory allows far more than
/// this; the cap leaves cores for the rest of the machine, which on the desktop is other agents'
/// builds and on the laptop is the owner.
const MAX_DEFAULT_THREADS: usize = 12;

/// What one run is assumed to need in memory, for the default thread count: about three times
/// what was measured, so a machine short of memory runs fewer at a time instead of swapping.
const MEMORY_PER_RUN_KB: u64 = 100 * 1024;

/// The runs to do at once when nothing says otherwise: half the cores (24 on the desktop gives
/// 12, the owner's 8-core laptop 4), never more than [`MAX_DEFAULT_THREADS`], never more than
/// the available memory holds, and at least one.
pub(crate) fn default_threads() -> usize {
    let cores = std::thread::available_parallelism().map_or(1, usize::from);
    let by_memory =
        available_memory_kb().map_or(usize::MAX, |kb| (kb / MEMORY_PER_RUN_KB) as usize);
    threads_for(cores, by_memory)
}

fn threads_for(cores: usize, by_memory: usize) -> usize {
    (cores / 2).clamp(1, MAX_DEFAULT_THREADS).min(by_memory).max(1)
}

/// `MemAvailable` from `/proc/meminfo`, where there is one.
fn available_memory_kb() -> Option<u64> {
    let text = fs::read_to_string("/proc/meminfo").ok()?;
    let line = text.lines().find(|l| l.starts_with("MemAvailable:"))?;
    line.split_whitespace().nth(1)?.parse().ok()
}

/// The thread count to use: `flag` (a `--threads` value) first, then `$FN_EVAL_THREADS`, then
/// [`default_threads`]. An error for something that isn't a positive number.
pub(crate) fn resolve_threads(flag: Option<usize>) -> Result<usize> {
    if let Some(n) = flag {
        return if n == 0 { Err("--threads wants at least 1".to_owned()) } else { Ok(n) };
    }
    match std::env::var("FN_EVAL_THREADS") {
        Ok(v) => match v.trim().parse::<usize>() {
            Ok(n) if n > 0 => Ok(n),
            _ => Err(format!("$FN_EVAL_THREADS wants a positive number, got {v:?}")),
        },
        Err(_) => Ok(default_threads()),
    }
}

/// One strategy to evaluate, and the folder its scripts and logs go in.
pub(crate) struct Job<'a> {
    pub(crate) strategy: &'a Strategy,
    pub(crate) work: PathBuf,
}

/// The files a [`Job`]'s runs read: its governor and, if it has one, its plan.
struct Files {
    script: PathBuf,
    plan: Option<PathBuf>,
}

fn write_files(job: &Job) -> Result<Files> {
    fs::create_dir_all(&job.work)
        .map_err(|e| format!("can't create {}: {e}", job.work.display()))?;
    let script = job.work.join("governor.rhai");
    fs::write(&script, &job.strategy.governor).map_err(|e| e.to_string())?;
    let plan = match &job.strategy.plan {
        Some(p) => {
            let path = job.work.join("plan.json");
            fs::write(&path, p.to_string()).map_err(|e| e.to_string())?;
            Some(path)
        }
        None => None,
    };
    Ok(Files { script, plan })
}

/// One `fn_agent play --horizon` run: a seed of one tier for one job.
struct Run {
    job: usize,
    role: Role,
    seed: u64,
}

fn run_one(fn_agent: &Path, track: &Track, files: &Files, work: &Path, run: &Run) -> Result<Row> {
    let seed = run.seed;
    let log = work.join(format!("seed-{seed}.jsonl"));
    let mut cmd = Command::new(fn_agent);
    cmd.arg("play").arg("--script").arg(&files.script);
    if let Some(plan) = &files.plan {
        cmd.arg("--plan").arg(plan);
    }
    cmd.args(["--horizon", &track.horizon_turns.to_string()])
        .args(track.start_args(seed))
        .arg("--log")
        .arg(&log);
    let out = run_bounded(&mut cmd, play_timeout(), &format!("seed {seed}"), fn_agent)?;
    let stdout = String::from_utf8_lossy(&out.stdout);
    let row = match parse_report(seed, run.role, &stdout) {
        Ok(row) => row,
        Err(parse_err) => {
            // No report at all: `play` refused to start or crashed. That's a real failure.
            let stderr = String::from_utf8_lossy(&out.stderr);
            return Err(format!(
                "seed {seed}: fn_agent play failed ({}): {}",
                out.status,
                if stderr.trim().is_empty() { parse_err } else { stderr.trim().to_owned() }
            ));
        }
    };
    // Nonzero exit with a report is `stopped_by`: still a scored result. Nonzero with
    // `stopped_by` null would be a mismatch worth failing on.
    if !out.status.success() && row.stopped.is_none() {
        return Err(format!(
            "seed {seed}: fn_agent play exited with {} but reported no stopped_by",
            out.status
        ));
    }
    Ok(row)
}

/// Evaluates every job on the seeds of `tiers`, up to `threads` runs at once, and returns a
/// table per job, in the order given. The runs of all the jobs share one queue, so a few slow
/// seeds don't leave threads idle while other jobs wait. Each table lists its tiers in the order
/// given and its seeds in the track's order, so the result is the same whatever the thread count
/// and whichever run finishes first. The first failure stops new runs from starting; the error
/// returned is that of the earliest run in queue order that failed.
pub(crate) fn evaluate_many(
    fn_agent: &Path,
    track: &Track,
    jobs: &[Job],
    tiers: &[Role],
    threads: usize,
) -> Result<Vec<Table>> {
    use std::sync::Mutex;
    use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

    let files: Vec<Files> = jobs.iter().map(write_files).collect::<Result<_>>()?;
    let mut runs = Vec::new();
    for job in 0..jobs.len() {
        for &role in tiers {
            for &seed in track.seeds(role) {
                runs.push(Run { job, role, seed });
            }
        }
    }
    let total = runs.len();
    let next = AtomicUsize::new(0);
    let failed = AtomicBool::new(false);
    let results: Mutex<Vec<Option<Result<Row>>>> = Mutex::new((0..total).map(|_| None).collect());
    let workers = threads.clamp(1, total.max(1));
    say(&format!(
        "{total} run(s) of {} turns on {workers} thread(s) ({})",
        track.horizon_turns, track.id
    ));
    std::thread::scope(|scope| {
        for _ in 0..workers {
            scope.spawn(|| {
                loop {
                    if failed.load(Ordering::Relaxed) {
                        return;
                    }
                    let i = next.fetch_add(1, Ordering::Relaxed);
                    let Some(run) = runs.get(i) else { return };
                    say(&format!(
                        "run {} of {total}: {} seed {} ({})",
                        i + 1,
                        run.role.word(),
                        run.seed,
                        jobs[run.job].work.display()
                    ));
                    let row = run_one(fn_agent, track, &files[run.job], &jobs[run.job].work, run);
                    if row.is_err() {
                        failed.store(true, Ordering::Relaxed);
                    }
                    results.lock().expect("no worker panics holding the lock")[i] = Some(row);
                }
            });
        }
    });
    let results = results.into_inner().expect("the workers are done");
    let mut tables: Vec<Table> = jobs
        .iter()
        .map(|_| Table {
            track: track.id.clone(),
            horizon: track.horizon_turns,
            collapse_below: track.collapse_below,
            rows: Vec::new(),
        })
        .collect();
    for (run, result) in runs.iter().zip(results) {
        match result {
            Some(Ok(row)) => tables[run.job].rows.push(row),
            Some(Err(e)) => return Err(e),
            // Never started: a run before it failed (and was returned above, being earlier in
            // the queue) or a later one did and stopped the queue.
            None => {}
        }
    }
    Ok(tables)
}

/// [`evaluate_many`] for one strategy and the tiers given.
pub(crate) fn evaluate_tiers(
    fn_agent: &Path,
    track: &Track,
    strategy: &Strategy,
    work: &Path,
    tiers: &[Role],
    threads: usize,
) -> Result<Table> {
    let job = Job { strategy, work: work.to_path_buf() };
    let mut tables = evaluate_many(fn_agent, track, &[job], tiers, threads)?;
    Ok(tables.remove(0))
}

// -------------------------------------------------------------------------------------------
// Entry point
// -------------------------------------------------------------------------------------------

/// The `fn_agent` to run: `$FN_AGENT_BIN` if set, else a release build of this checkout.
pub(crate) fn fn_agent_binary(repo: &Path) -> Result<PathBuf> {
    match std::env::var_os("FN_AGENT_BIN") {
        Some(bin) => Ok(PathBuf::from(bin)),
        None => {
            cargo_build_release(repo, &["fn_agent"])?;
            Ok(repo.join("target/release/fn_agent"))
        }
    }
}

/// `--tiers LIST`: the tiers named, comma separated, in the order of [`Role::ALL`].
pub(crate) fn parse_tiers(list: &str) -> Result<Vec<Role>> {
    let named = list.split(',').map(|w| Role::parse(w.trim())).collect::<Result<Vec<_>>>()?;
    Ok(Role::ALL.into_iter().filter(|r| named.contains(r)).collect())
}

#[derive(Debug, Default, PartialEq)]
struct Opts {
    track: Option<String>,
    champion: Option<PathBuf>,
    json: bool,
    out: Option<PathBuf>,
    tracks: Option<PathBuf>,
    threads: Option<usize>,
    tiers: Option<Vec<Role>>,
}

fn parse_args(args: &[&str]) -> Result<Opts> {
    let mut o = Opts::default();
    let mut it = args.iter();
    while let Some(&flag) = it.next() {
        let mut value = || it.next().copied().ok_or_else(|| format!("{flag} needs a value"));
        match flag {
            "--track" => o.track = Some(value()?.to_owned()),
            "--champion" => o.champion = Some(PathBuf::from(value()?)),
            "--out" => o.out = Some(PathBuf::from(value()?)),
            "--tracks" => o.tracks = Some(PathBuf::from(value()?)),
            "--threads" => {
                let v = value()?;
                o.threads =
                    Some(v.parse().map_err(|_| format!("--threads wants a number, got {v:?}"))?);
            }
            "--tiers" => o.tiers = Some(parse_tiers(value()?)?),
            "--json" => o.json = true,
            "-h" | "--help" => return Err(help()),
            other => return Err(format!("unknown flag {other:?}\n\n{}", help())),
        }
    }
    if o.track.is_none() || o.champion.is_none() {
        return Err(format!("--track and --champion are both required\n\n{}", help()));
    }
    Ok(o)
}

pub fn run(args: &[&str]) -> Result {
    let opts = parse_args(args)?;
    let repo = crate::root();
    let track_id = opts.track.as_deref().expect("checked in parse_args");
    let champion = opts.champion.as_deref().expect("checked in parse_args");

    let tracks_path = opts.tracks.clone().unwrap_or_else(|| repo.join("data/tracks.toml"));
    let track = load_track(&tracks_path, track_id)?;
    let strategy = read_strategy(champion)?;
    let threads = resolve_threads(opts.threads)?;
    let tiers = opts.tiers.clone().unwrap_or_else(|| Role::ALL.to_vec());
    let fn_agent = fn_agent_binary(&repo)?;

    let (work, scratch) = match &opts.out {
        Some(dir) => (dir.clone(), false),
        None => (std::env::temp_dir().join(format!("fn-agent-eval-{}", std::process::id())), true),
    };
    let result = evaluate_tiers(&fn_agent, &track, &strategy, &work, &tiers, threads);
    if scratch {
        let _ = fs::remove_dir_all(&work);
    }
    let table = result?;

    if opts.json {
        println!("{}", serde_json::to_string_pretty(&table.json()).map_err(|e| e.to_string())?);
    } else {
        print!("{}", table.text());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bundled() -> Vec<Track> {
        parse_tracks(include_str!("../../data/tracks.toml")).expect("bundled tracks.toml is valid")
    }

    #[test]
    fn bundled_tracks_parse_and_the_first_is_water_3h_with_its_three_tiers() {
        let tracks = bundled();
        let t = tracks.iter().find(|t| t.id == "water-3h").expect("water-3h");
        assert_eq!((t.nation.as_str(), t.horizon_turns, t.play_seed), ("water", 540, 7633));
        assert_eq!(t.screen_seeds, [7633, 7, 42]);
        assert_eq!(t.gate_seeds.len(), 12);
        assert_eq!(t.owner_seeds.len(), 30);
        assert!(t.collapse_below > 0.0);
        // The owner-only tier, and the gate, keep clear of the balance panel and of each other.
        for s in t.owner_seeds.iter().chain(&t.gate_seeds) {
            assert!(!BALANCE_PANEL.contains(s), "seed {s} is in the balance panel");
        }
        for s in &t.owner_seeds {
            assert!(!t.gate_seeds.contains(s) && !t.screen_seeds.contains(s), "seed {s}");
        }
    }

    /// Every track names a difficulty `data/difficulty.toml` has and a map size
    /// `data/embark.toml` has, so a typo can't survive until the first evaluation.
    #[test]
    fn bundled_tracks_name_real_difficulties_and_map_sizes() {
        let diff: toml::Table = include_str!("../../data/difficulty.toml").parse().unwrap();
        let ids: Vec<&str> =
            diff["preset"].as_array().unwrap().iter().filter_map(|p| p["id"].as_str()).collect();
        let embark = include_str!("../../data/embark.toml");
        for t in bundled() {
            assert!(ids.contains(&t.difficulty.as_str()), "{}: difficulty {}", t.id, t.difficulty);
            assert!(embark.contains(&format!("\"{}\"", t.map_size)), "{}: map_size", t.id);
        }
    }

    fn src(extra: &str) -> String {
        format!(
            "[[track]]\nid = \"t\"\nname = \"T\"\nnation = \"water\"\ndifficulty = \"normal\"\n\
             map_size = \"small\"\nsite = \"default\"\nhorizon_turns = 5\nplay_seed = 1\n\
             screen_seeds = [1, 2]\ngate_seeds = [101, 102]\nowner_seeds = [201]\n\
             collapse_below = 50\n{extra}"
        )
    }

    #[test]
    fn strict_keys_and_validation() {
        assert!(parse_tracks(&src("")).is_ok());
        assert!(parse_tracks(&src("bogus = 1\n")).unwrap_err().contains("bogus"));
        // The names from before the tiers are not keys any more.
        assert!(parse_tracks(&src("train_seeds = [1]\n")).unwrap_err().contains("train_seeds"));
        let bad = |from: &str, to: &str| parse_tracks(&src("").replace(from, to)).unwrap_err();
        assert!(bad("\"water\"", "\"wood\"").contains("nation"));
        assert!(bad("\"small\"", "\"huge\"").contains("map_size"));
        assert!(bad("\"default\"", "\"nope\"").contains("site"));
        assert!(bad("[1, 2]", "[]").contains("screen_seeds"));
        assert!(bad("[101, 102]", "[]").contains("gate_seeds"));
        assert!(bad("[201]", "[]").contains("owner_seeds"));
        assert!(bad("horizon_turns = 5", "horizon_turns = 0").contains("horizon"));
        assert!(bad("collapse_below = 50", "collapse_below = 0").contains("collapse_below"));
        let dup = format!("{}{}", src(""), src(""));
        assert!(parse_tracks(&dup).unwrap_err().contains("twice"));
    }

    /// The tiers may not overlap, the gate and owner-only tiers may not touch the balance panel
    /// 1-32, and the play seed is in the screen only.
    #[test]
    fn the_tiers_are_disjoint_and_fresh() {
        let bad = |from: &str, to: &str| parse_tracks(&src("").replace(from, to)).unwrap_err();
        let e = bad("[201]", "[101]");
        assert!(e.contains("seed 101") && e.contains("gate_seeds and owner_seeds"), "{e}");
        let e = bad("[101, 102]", "[2, 102]");
        assert!(e.contains("seed 2") && e.contains("screen_seeds and gate_seeds"), "{e}");
        let e = bad("[201]", "[32]");
        assert!(e.contains("owner_seeds") && e.contains("balance panel"), "{e}");
        assert!(bad("[201]", "[1000, 5]").contains("balance panel"));
        assert!(bad("[101, 102]", "[101, 5]").contains("gate_seeds has seed 5"));
        // 33 is the first seed outside the panel.
        assert!(parse_tracks(&src("").replace("[201]", "[33]")).is_ok());
        // The model plays play_seed, so it is a screen seed at most.
        let play = src("").replace("play_seed = 1", "play_seed = 201");
        assert!(parse_tracks(&play).unwrap_err().contains("play_seed"));
        let play = src("").replace("play_seed = 1", "play_seed = 101");
        assert!(parse_tracks(&play).unwrap_err().contains("play_seed"));
    }

    #[test]
    fn start_args_carry_the_whole_start() {
        let t = parse_tracks(&src("prelude = { opening = \"economy\", minutes = 60 }\n"))
            .unwrap()
            .remove(0);
        let a = t.start_args(9).join(" ");
        assert_eq!(
            a,
            "--seed 9 --nation water --difficulty normal --map-size small --prelude economy:60"
        );
        let mut t = t;
        t.site = "10,20".to_owned();
        t.prelude = None;
        assert!(t.start_args(9).join(" ").ends_with("--start 10,20"));
    }

    fn row(seed: u64, role: Role, score: f64, deaths: u32, stopped: Option<Stopped>) -> Row {
        Row { seed, role, score, progress: 0.5, care: 0.9, deaths, stopped }
    }

    #[test]
    fn table_means_failures_and_flags() {
        let stopped =
            Some(Stopped { reason: "error".to_owned(), why: Some("boom".to_owned()), turn: 3 });
        let t = Table {
            track: "t".to_owned(),
            horizon: 5,
            collapse_below: 60.0,
            rows: vec![
                row(1, Role::Screen, 100.0, 0, None),
                row(2, Role::Screen, 50.0, 2, stopped),
                row(101, Role::Gate, 80.0, 0, None),
                row(201, Role::Owner, 90.0, 1, None),
            ],
        };
        assert_eq!(t.screen_mean(), Some(75.0));
        assert_eq!(t.mean_of(Role::Gate), Some(80.0));
        assert_eq!(t.mean_of(Role::Owner), Some(90.0));
        let f = t.failures(Role::Screen);
        assert_eq!((f.collapses, f.deaths, f.death_seeds, f.stopped), (1, 2, 1, 1));
        let text = t.text();
        assert!(text.contains("screen  mean    75.0"), "{text}");
        assert!(text.contains("collapse 1/2 · deaths 2 on 1 seed(s) · stopped 1"), "{text}");
        assert!(text.contains("stopped by error at turn 3"), "{text}");
        assert!(text.contains("owner   mean    90.0"), "{text}");
        let j = t.json();
        assert_eq!(j["screen_mean"], 75.0);
        assert_eq!(j["stopped"], 1);
        assert_eq!(j["screen"][1]["stopped_by"]["why"], "boom");
        assert_eq!(j["gate"][0]["seed"], 101);
        assert_eq!(j["owner_failures"]["deaths"], 1);
    }

    #[test]
    fn parse_report_reads_score_deaths_and_stopped_by() {
        let ok = r#"{"scorecard":{"score":12.5,"progress":0.1,"care":0.9,
            "care_detail":{"deaths":2}},"stopped_by":null}"#;
        let r = parse_report(7, Role::Screen, ok).unwrap();
        assert_eq!((r.score, r.deaths, r.stopped), (12.5, 2, None));
        let stopped = r#"{"scorecard":{"score":1.0,"progress":0.0,"care":1.0},
            "stopped_by":{"reason":"runaway","why":null,"turn":4}}"#;
        let r = parse_report(7, Role::Screen, stopped).unwrap();
        assert_eq!((r.stopped.unwrap().reason.as_str(), r.deaths), ("runaway", 0));
        assert!(parse_report(7, Role::Screen, "not json").is_err());
        let e = parse_report(7, Role::Screen, r#"{"scorecard":{}}"#).unwrap_err();
        assert!(e.contains("score"), "{e}");
    }

    #[test]
    fn args_need_a_track_and_a_champion() {
        assert!(parse_args(&[]).is_err());
        assert!(parse_args(&["--track", "t"]).is_err());
        assert!(parse_args(&["--track", "t", "--champion", "d", "--bogus"]).is_err());
        let o = parse_args(&["--track", "t", "--champion", "d", "--json"]).unwrap();
        assert!(o.json && o.threads.is_none() && o.tiers.is_none());
        assert_eq!(o.champion, Some(PathBuf::from("d")));
        let o = parse_args(&[
            "--track",
            "t",
            "--champion",
            "d",
            "--threads",
            "3",
            "--tiers",
            "owner,screen",
        ])
        .unwrap();
        assert_eq!(o.threads, Some(3));
        assert_eq!(o.tiers, Some(vec![Role::Screen, Role::Owner]), "in tier order");
        assert!(parse_args(&["--track", "t", "--champion", "d", "--threads", "x"]).is_err());
        let e =
            parse_args(&["--track", "t", "--champion", "d", "--tiers", "held-out"]).unwrap_err();
        assert!(e.contains("held-out") && e.contains("screen, gate and owner"), "{e}");
    }

    #[test]
    fn the_default_thread_count_suits_the_desktop_and_the_laptop() {
        // 24 cores: half, capped at 12. 8 cores: 4. Tiny machines: at least 1.
        assert_eq!(threads_for(24, usize::MAX), 12);
        assert_eq!(threads_for(48, usize::MAX), 12, "capped");
        assert_eq!(threads_for(8, usize::MAX), 4);
        assert_eq!(threads_for(2, usize::MAX), 1);
        assert_eq!(threads_for(1, usize::MAX), 1);
        // Short of memory: fewer, but never none.
        assert_eq!(threads_for(24, 5), 5);
        assert_eq!(threads_for(24, 0), 1);
        assert!(default_threads() >= 1);
        assert_eq!(resolve_threads(Some(3)).unwrap(), 3, "a flag wins");
        assert!(resolve_threads(Some(0)).unwrap_err().contains("at least 1"));
    }

    /// A stub `fn_agent play` whose seeds finish in the opposite order to the queue (the larger
    /// the seed, the shorter its sleep), reporting `score = seed * 2` and a death on odd seeds.
    const STUB: &str = r#"#!/bin/sh
seed=0; log=""; prev=""
for a in "$@"; do
  [ "$prev" = "--seed" ] && seed="$a"
  [ "$prev" = "--log" ] && log="$a"
  prev="$a"
done
sleep "0.0$((9 - seed % 9))"
echo "stub log $seed" > "$log"
printf '{"scorecard":{"score":%d.0,"progress":0.5,"care":0.9,"care_detail":{"deaths":%d}},"stopped_by":null}\n' "$((seed * 2))" "$((seed % 2))"
"#;

    fn stub_fn_agent(dir: &Path) -> PathBuf {
        use std::os::unix::fs::PermissionsExt;
        fs::create_dir_all(dir).unwrap();
        let path = dir.join("fn_agent_stub");
        fs::write(&path, STUB).unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).unwrap();
        path
    }

    /// The table is the same for one thread and for many, rows in tier and seed order, though
    /// the runs finish in the reverse of that order (#571).
    #[test]
    fn evaluation_is_the_same_whatever_the_thread_count() {
        let tmp = std::env::temp_dir().join(format!("fn_eval_threads_{}", std::process::id()));
        let _ = fs::remove_dir_all(&tmp);
        let fn_agent = stub_fn_agent(&tmp);
        let track = parse_tracks(
            &src("")
                .replace("[1, 2]", "[1, 2, 3, 4, 5]")
                .replace("[101, 102]", "[101, 102, 103, 104, 105, 106]")
                .replace("[201]", "[201, 202, 203]"),
        )
        .unwrap()
        .remove(0);
        let strategy = Strategy { governor: "wait(1);".to_owned(), plan: None };
        let first =
            evaluate_tiers(&fn_agent, &track, &strategy, &tmp.join("w1"), &Role::ALL, 1).unwrap();
        let seeds: Vec<u64> = first.rows.iter().map(|r| r.seed).collect();
        assert_eq!(
            seeds,
            [1, 2, 3, 4, 5, 101, 102, 103, 104, 105, 106, 201, 202, 203],
            "tier order, then the track's seed order"
        );
        assert_eq!(first.rows[0].score, 2.0);
        assert_eq!(first.rows[5].deaths, 1, "seed 101 is odd");
        for threads in [2, 5, 14, 64] {
            let again =
                evaluate_tiers(&fn_agent, &track, &strategy, &tmp.join("w2"), &Role::ALL, threads)
                    .unwrap();
            assert_eq!(first, again, "{threads} threads");
            assert_eq!(first.text(), again.text());
        }
        // Only the tiers asked for are run, and every run's log is kept in the job's folder.
        let gate = evaluate_tiers(&fn_agent, &track, &strategy, &tmp.join("w3"), &[Role::Gate], 3)
            .unwrap();
        assert_eq!(gate.rows.len(), 6);
        assert!(tmp.join("w3/seed-106.jsonl").is_file() && !tmp.join("w3/seed-1.jsonl").exists());
        let _ = fs::remove_dir_all(&tmp);
    }

    /// Several strategies share one queue and come back in the order given.
    #[test]
    fn many_strategies_come_back_in_order() {
        let tmp = std::env::temp_dir().join(format!("fn_eval_many_{}", std::process::id()));
        let _ = fs::remove_dir_all(&tmp);
        let fn_agent = stub_fn_agent(&tmp);
        let track = parse_tracks(&src("")).unwrap().remove(0);
        let (a, b) = (
            Strategy { governor: "a".to_owned(), plan: None },
            Strategy { governor: "b".to_owned(), plan: Some(json!({"k": 1})) },
        );
        let jobs =
            [Job { strategy: &a, work: tmp.join("a") }, Job { strategy: &b, work: tmp.join("b") }];
        let tables = evaluate_many(&fn_agent, &track, &jobs, &[Role::Screen], 4).unwrap();
        assert_eq!(tables.len(), 2);
        assert!(tables.iter().all(|t| t.rows.len() == 2));
        assert_eq!(fs::read_to_string(tmp.join("a/governor.rhai")).unwrap(), "a");
        assert!(!tmp.join("a/plan.json").exists() && tmp.join("b/plan.json").is_file());
        let _ = fs::remove_dir_all(&tmp);
    }

    /// A run that fails stops the queue and the error is the earliest failing run's, so the
    /// message does not depend on which thread got there first.
    #[test]
    fn the_earliest_failure_is_the_one_reported() {
        use std::os::unix::fs::PermissionsExt;
        let tmp = std::env::temp_dir().join(format!("fn_eval_fail_{}", std::process::id()));
        let _ = fs::remove_dir_all(&tmp);
        fs::create_dir_all(&tmp).unwrap();
        let bad = tmp.join("fn_agent_bad");
        // Seeds 2 and 102 crash; the later seed is quicker.
        let script = "#!/bin/sh\nseed=0; prev=\"\"\nfor a in \"$@\"; do [ \"$prev\" = \"--seed\" ] && seed=\"$a\"; prev=\"$a\"; done\n\
                      case \"$seed\" in 2) sleep 0.3; echo boom2 >&2; exit 3;; 102) echo boom102 >&2; exit 3;; esac\n\
                      printf '{\"scorecard\":{\"score\":1.0,\"progress\":0.0,\"care\":1.0},\"stopped_by\":null}\\n'\n";
        fs::write(&bad, script).unwrap();
        fs::set_permissions(&bad, fs::Permissions::from_mode(0o755)).unwrap();
        let track = parse_tracks(&src("")).unwrap().remove(0);
        let strategy = Strategy { governor: "x".to_owned(), plan: None };
        let e = evaluate_tiers(&bad, &track, &strategy, &tmp.join("w"), &Role::ALL, 4).unwrap_err();
        assert!(e.contains("seed 2") && e.contains("boom2"), "{e}");
        let _ = fs::remove_dir_all(&tmp);
    }
}
