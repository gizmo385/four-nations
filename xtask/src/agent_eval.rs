//! `cargo xtask agent-eval --track ID --champion DIR [--json] [--out DIR] [--tracks FILE]`
//!
//! The clean evaluation of a strategy on a track (docs/design/agent-hillclimb.md, "Across runs:
//! attempts and a champion" and "Fairness"; #529). CPU only: it calls no model.
//!
//! `DIR/handoff.json` (the file `agent-run --carry` writes) holds the strategy: the script named
//! `governor` and the plan. For every train seed, then every held-out seed, of the track in
//! `data/tracks.toml`, this runs `fn_agent play --script governor --plan plan --horizon N` once
//! (one at a time, which matters on a small laptop), and prints the score per seed, the train
//! mean and the held-out scores. Selection uses the train mean only; the held-out scores are for
//! the owner.
//!
//!   --track ID       a `[[track]]` id from the tracks file (required)
//!   --champion DIR   the directory holding `handoff.json` (required)
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
//! The evaluation lives in [`evaluate`], which the attempt loop (#530) reuses.

use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use serde::Deserialize;
use serde_json::{Value, json};

use crate::Result;
use crate::agent_run::{Handoff, NATIONS, cargo_build_release};

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
#[derive(Debug, Clone, PartialEq, Deserialize)]
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
    pub(crate) train_seeds: Vec<u64>,
    pub(crate) holdout_seeds: Vec<u64>,
}

/// A track's optional prelude, as `data/scenarios.toml` writes one.
#[derive(Debug, Clone, PartialEq, Deserialize)]
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
        if self.train_seeds.is_empty() {
            return bad("train_seeds is empty: selection needs at least one seed");
        }
        let mut seen = std::collections::BTreeSet::new();
        for s in self.train_seeds.iter().chain(&self.holdout_seeds) {
            if !seen.insert(*s) {
                return bad(&format!(
                    "seed {s} appears twice across train_seeds and holdout_seeds"
                ));
            }
        }
        if self.holdout_seeds.contains(&self.play_seed) {
            return bad("play_seed is in holdout_seeds: the model plays it, so it isn't held out");
        }
        Ok(())
    }

    /// The `--start` value: `Some("X,Y")` for a founded site, `None` for the default start.
    fn site_flag(&self) -> Result<Option<&str>> {
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

/// Which panel a seed belongs to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Role {
    Train,
    Holdout,
}

impl Role {
    fn word(self) -> &'static str {
        match self {
            Role::Train => "train",
            Role::Holdout => "held-out",
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
    /// Set when the script was stopped by an error or a runaway: the score is still valid.
    pub(crate) stopped: Option<Stopped>,
}

/// A track's clean evaluation: a row per train seed, then a row per held-out seed.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Table {
    pub(crate) track: String,
    pub(crate) horizon: u32,
    pub(crate) rows: Vec<Row>,
}

fn mean(xs: impl Iterator<Item = f64>) -> Option<f64> {
    let (sum, n) = xs.fold((0.0, 0u32), |(s, n), x| (s + x, n + 1));
    (n > 0).then(|| sum / f64::from(n))
}

impl Table {
    fn rows_of(&self, role: Role) -> impl Iterator<Item = &Row> {
        self.rows.iter().filter(move |r| r.role == role)
    }

    /// The selection number: the mean score over the train seeds.
    pub(crate) fn train_mean(&self) -> Option<f64> {
        mean(self.rows_of(Role::Train).map(|r| r.score))
    }

    /// The mean over the held-out seeds (for the owner; never selected on).
    pub(crate) fn holdout_mean(&self) -> Option<f64> {
        mean(self.rows_of(Role::Holdout).map(|r| r.score))
    }

    pub(crate) fn stopped_count(&self) -> usize {
        self.rows.iter().filter(|r| r.stopped.is_some()).count()
    }

    pub(crate) fn text(&self) -> String {
        let mut out = String::new();
        let _ = writeln!(
            out,
            "agent-eval · {} · horizon {}                    (held-out: owner only)",
            self.track, self.horizon
        );
        let _ = writeln!(
            out,
            " {:>8}  {:<8}  {:>7}  {:>8}  {:>5}  note",
            "seed", "panel", "score", "progress", "care"
        );
        for r in &self.rows {
            let note = match &r.stopped {
                Some(s) => format!("stopped by {} at turn {}", s.reason, s.turn),
                None => String::new(),
            };
            let line = format!(
                " {:>8}  {:<8}  {:>7.1}  {:>8.3}  {:>5.3}  {note}",
                r.seed,
                r.role.word(),
                r.score,
                r.progress,
                r.care
            );
            let _ = writeln!(out, "{}", line.trim_end());
        }
        let _ = writeln!(out);
        let list = |role| {
            self.rows_of(role).map(|r| format!("{}: {:.1}", r.seed, r.score)).collect::<Vec<_>>()
        };
        if let Some(m) = self.train_mean() {
            let _ = writeln!(out, "train mean   {m:.1}   ({})", list(Role::Train).join(", "));
        }
        if let Some(m) = self.holdout_mean() {
            let _ =
                writeln!(out, "held-out     {}   (mean {m:.1})", list(Role::Holdout).join(", "));
        }
        let n = self.stopped_count();
        if n > 0 {
            let _ = writeln!(
                out,
                "{n} run(s) had their script stopped by an error or a runaway; they coasted to \
                 the horizon and are scored as usual."
            );
        }
        out
    }

    pub(crate) fn json(&self) -> Value {
        let row = |r: &Row| {
            json!({
                "seed": r.seed,
                "score": r.score,
                "progress": r.progress,
                "care": r.care,
                "stopped_by": r.stopped.as_ref().map(|s| json!({
                    "reason": s.reason, "why": s.why, "turn": s.turn,
                })),
            })
        };
        json!({
            "track": self.track,
            "horizon": self.horizon,
            "train": self.rows_of(Role::Train).map(row).collect::<Vec<_>>(),
            "train_mean": self.train_mean(),
            "holdout": self.rows_of(Role::Holdout).map(row).collect::<Vec<_>>(),
            "holdout_mean": self.holdout_mean(),
            "stopped": self.stopped_count(),
        })
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
    Ok(Row {
        seed,
        role,
        score: num("score")?,
        progress: num("progress")?,
        care: num("care")?,
        stopped,
    })
}

/// Runs `fn_agent play --horizon` once per train seed, then once per held-out seed, one at a
/// time, with `strategy` on `track`. `fn_agent` is the binary to run; `work` is a folder for the
/// script, the plan and each seed's log (`seed-N.jsonl`). The reusable core (#530).
pub(crate) fn evaluate(
    fn_agent: &Path,
    track: &Track,
    strategy: &Strategy,
    work: &Path,
) -> Result<Table> {
    fs::create_dir_all(work).map_err(|e| format!("can't create {}: {e}", work.display()))?;
    let script = work.join("governor.rhai");
    fs::write(&script, &strategy.governor).map_err(|e| e.to_string())?;
    let plan = match &strategy.plan {
        Some(p) => {
            let path = work.join("plan.json");
            fs::write(&path, p.to_string()).map_err(|e| e.to_string())?;
            Some(path)
        }
        None => None,
    };

    let mut rows = Vec::new();
    let panels = [(Role::Train, &track.train_seeds), (Role::Holdout, &track.holdout_seeds)];
    for (role, seeds) in panels {
        for &seed in seeds {
            say(&format!(
                "{} seed {seed} ({}), {} turns",
                role.word(),
                track.id,
                track.horizon_turns
            ));
            let log = work.join(format!("seed-{seed}.jsonl"));
            let mut cmd = Command::new(fn_agent);
            cmd.arg("play").arg("--script").arg(&script);
            if let Some(plan) = &plan {
                cmd.arg("--plan").arg(plan);
            }
            cmd.args(["--horizon", &track.horizon_turns.to_string()])
                .args(track.start_args(seed))
                .arg("--log")
                .arg(&log);
            let out = cmd.output().map_err(|e| format!("can't run {}: {e}", fn_agent.display()))?;
            let stdout = String::from_utf8_lossy(&out.stdout);
            let row = match parse_report(seed, role, &stdout) {
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
            rows.push(row);
        }
    }
    Ok(Table { track: track.id.clone(), horizon: track.horizon_turns, rows })
}

// -------------------------------------------------------------------------------------------
// Entry point
// -------------------------------------------------------------------------------------------

#[derive(Debug, Default, PartialEq)]
struct Opts {
    track: Option<String>,
    champion: Option<PathBuf>,
    json: bool,
    out: Option<PathBuf>,
    tracks: Option<PathBuf>,
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

    let fn_agent = match std::env::var_os("FN_AGENT_BIN") {
        Some(bin) => PathBuf::from(bin),
        None => {
            cargo_build_release(&repo, &["fn_agent"])?;
            repo.join("target/release/fn_agent")
        }
    };

    let (work, scratch) = match &opts.out {
        Some(dir) => (dir.clone(), false),
        None => (std::env::temp_dir().join(format!("fn-agent-eval-{}", std::process::id())), true),
    };
    let result = evaluate(&fn_agent, &track, &strategy, &work);
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
    fn bundled_tracks_parse_and_the_first_is_water_3h() {
        let tracks = bundled();
        let t = tracks.iter().find(|t| t.id == "water-3h").expect("water-3h");
        assert_eq!((t.nation.as_str(), t.horizon_turns, t.play_seed), ("water", 540, 7633));
        assert_eq!(t.train_seeds, [7633, 7, 42]);
        assert_eq!(t.holdout_seeds, [1, 5, 8]);
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
             train_seeds = [1, 2]\nholdout_seeds = [3]\n{extra}"
        )
    }

    #[test]
    fn strict_keys_and_validation() {
        assert!(parse_tracks(&src("")).is_ok());
        assert!(parse_tracks(&src("bogus = 1\n")).unwrap_err().contains("bogus"));
        let bad = |from: &str, to: &str| parse_tracks(&src("").replace(from, to)).unwrap_err();
        assert!(bad("\"water\"", "\"wood\"").contains("nation"));
        assert!(bad("\"small\"", "\"huge\"").contains("map_size"));
        assert!(bad("\"default\"", "\"nope\"").contains("site"));
        assert!(bad("[1, 2]", "[]").contains("train_seeds"));
        assert!(bad("[3]", "[2]").contains("twice"));
        assert!(bad("[3]", "[1]").contains("twice"));
        assert!(bad("horizon_turns = 5", "horizon_turns = 0").contains("horizon"));
        let dup = format!("{}{}", src(""), src(""));
        assert!(parse_tracks(&dup).unwrap_err().contains("twice"));
        // A held-out seed that is the play seed is caught even without being a train seed.
        let held_play = src("").replace("train_seeds = [1, 2]", "train_seeds = [2]");
        let held_play = held_play.replace("[3]", "[1]");
        assert!(parse_tracks(&held_play).unwrap_err().contains("play_seed"));
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

    #[test]
    fn table_means_and_flags() {
        let row = |seed, role, score, stopped| Row {
            seed,
            role,
            score,
            progress: 0.5,
            care: 0.9,
            stopped,
        };
        let stopped =
            Some(Stopped { reason: "error".to_owned(), why: Some("boom".to_owned()), turn: 3 });
        let t = Table {
            track: "t".to_owned(),
            horizon: 5,
            rows: vec![
                row(1, Role::Train, 100.0, None),
                row(2, Role::Train, 50.0, stopped),
                row(3, Role::Holdout, 80.0, None),
            ],
        };
        assert_eq!(t.train_mean(), Some(75.0));
        assert_eq!(t.holdout_mean(), Some(80.0));
        let text = t.text();
        assert!(text.contains("train mean   75.0"), "{text}");
        assert!(text.contains("stopped by error at turn 3"), "{text}");
        assert!(text.contains("3: 80.0"), "{text}");
        let j = t.json();
        assert_eq!(j["train_mean"], 75.0);
        assert_eq!(j["stopped"], 1);
        assert_eq!(j["train"][1]["stopped_by"]["why"], "boom");
        assert_eq!(j["holdout"][0]["seed"], 3);
    }

    #[test]
    fn parse_report_reads_score_and_stopped_by() {
        let ok = r#"{"scorecard":{"score":12.5,"progress":0.1,"care":0.9},"stopped_by":null}"#;
        let r = parse_report(7, Role::Train, ok).unwrap();
        assert_eq!((r.score, r.stopped), (12.5, None));
        let stopped = r#"{"scorecard":{"score":1.0,"progress":0.0,"care":1.0},
            "stopped_by":{"reason":"runaway","why":null,"turn":4}}"#;
        let r = parse_report(7, Role::Train, stopped).unwrap();
        assert_eq!(r.stopped.unwrap().reason, "runaway");
        assert!(parse_report(7, Role::Train, "not json").is_err());
        assert!(parse_report(7, Role::Train, r#"{"scorecard":{}}"#).unwrap_err().contains("score"));
    }

    #[test]
    fn args_need_a_track_and_a_champion() {
        assert!(parse_args(&[]).is_err());
        assert!(parse_args(&["--track", "t"]).is_err());
        assert!(parse_args(&["--track", "t", "--champion", "d", "--bogus"]).is_err());
        let o = parse_args(&["--track", "t", "--champion", "d", "--json"]).unwrap();
        assert!(o.json);
        assert_eq!(o.champion, Some(PathBuf::from("d")));
    }
}
