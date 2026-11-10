//! End-to-end coverage of `cargo xtask agent-eval` (#529) against the real `fn_agent`, on a tiny
//! track (a few turns, one seed in each tier) so it runs in seconds. No model is
//! involved. `fn_agent` is built once per test run with `cargo build -p fn_agent --profile ci`
//! and handed to `xtask` through `$FN_AGENT_BIN`, so the xtask binary never builds a release
//! `fn_agent` here.
//!
//! The scores themselves aren't pinned (balance moves them); the table's shape and the runs'
//! determinism are.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::OnceLock;

fn xtask_bin() -> &'static str {
    env!("CARGO_BIN_EXE_xtask")
}

/// The path of a `ci`-profile `fn_agent`, built on first use (cargo's own JSON names it).
fn fn_agent_bin() -> &'static Path {
    static BIN: OnceLock<PathBuf> = OnceLock::new();
    BIN.get_or_init(|| {
        let out = Command::new(env!("CARGO"))
            .args(["build", "-q", "-p", "fn_agent", "--profile", "ci", "--message-format=json"])
            .output()
            .expect("cargo runs");
        assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
        String::from_utf8_lossy(&out.stdout)
            .lines()
            .filter_map(|l| serde_json::from_str::<serde_json::Value>(l).ok())
            .filter(|m| m["reason"] == "compiler-artifact" && m["target"]["name"] == "fn_agent")
            .filter_map(|m| m["executable"].as_str().map(PathBuf::from))
            .next_back()
            .expect("cargo reported the fn_agent executable")
    })
}

const TRACKS: &str = r#"
[[track]]
id = "tiny"
name = "Tiny"
nation = "water"
difficulty = "normal"
map_size = "small"
site = "default"
horizon_turns = 4
play_seed = 7
screen_seeds = [7]
gate_seeds = [101]
owner_seeds = [201]
collapse_below = 1
"#;

struct Fixture {
    tmp: PathBuf,
    /// `$FN_AGENT_CACHE` for its evaluations.
    cache: PathBuf,
}

impl Fixture {
    /// Par and the controls come from a cache every test in this file shares (under cargo's
    /// target tmp dir, never the owner's): they're the same for every test on one build, and
    /// computing them is most of a first evaluation. Writes are atomic (a temporary file and a
    /// rename), so tests running at once can share it.
    fn new(name: &str) -> Self {
        let cache = Path::new(env!("CARGO_TARGET_TMPDIR")).join("agent_eval_cache");
        Self::with_cache(name, cache)
    }

    /// A fixture with a cache of its own, for a test of what the cache holds.
    fn own_cache(name: &str) -> Self {
        Self::with_cache(name, Self::dir(name).join("cache"))
    }

    fn dir(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!("fn_agent_eval_it_{name}_{}", std::process::id()))
    }

    fn with_cache(name: &str, cache: PathBuf) -> Self {
        let tmp = Self::dir(name);
        let _ = fs::remove_dir_all(&tmp);
        fs::create_dir_all(&tmp).unwrap();
        fs::write(tmp.join("tracks.toml"), TRACKS).unwrap();
        Self { tmp, cache }
    }

    /// A champion folder whose handoff holds `scripts` (name, code) and an optional plan.
    fn champion(&self, scripts: &[(&str, &str)], plan: Option<&str>) -> PathBuf {
        let dir = self.tmp.join("champion");
        fs::create_dir_all(&dir).unwrap();
        let scripts: serde_json::Map<String, serde_json::Value> = scripts
            .iter()
            .map(|(n, c)| ((*n).to_owned(), serde_json::json!({"version": 1, "code": c})))
            .collect();
        let mut handoff = serde_json::json!({"scripts": scripts, "notes": []});
        if let Some(plan) = plan {
            handoff["plan"] = serde_json::from_str(plan).unwrap();
        }
        fs::write(dir.join("handoff.json"), handoff.to_string()).unwrap();
        dir
    }

    fn eval(&self, champion: &Path, extra: &[&str]) -> Output {
        Command::new(xtask_bin())
            .arg("agent-eval")
            .args(["--track", "tiny", "--champion"])
            .arg(champion)
            .arg("--tracks")
            .arg(self.tmp.join("tracks.toml"))
            .args(extra)
            .env("FN_AGENT_BIN", fn_agent_bin())
            // Par and the controls are kept per build (#572): in the tests' cache, never the
            // owner's.
            .env("FN_AGENT_CACHE", &self.cache)
            .output()
            .unwrap()
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.tmp);
    }
}

fn stdout(out: &Output) -> String {
    String::from_utf8_lossy(&out.stdout).into_owned()
}

fn stderr(out: &Output) -> String {
    String::from_utf8_lossy(&out.stderr).into_owned()
}

/// The table has a row per seed of every tier, each tier's mean and failure counts, and two
/// evaluations of the same strategy print the same bytes, whatever the thread count (#571).
#[test]
fn the_table_has_its_shape_and_is_deterministic() {
    let fx = Fixture::new("shape");
    let champion = fx.champion(&[("governor", "wait(2);")], Some(r#"{"targets": {"wood": 10}}"#));
    let a = fx.eval(&champion, &[]);
    assert!(a.status.success(), "{}", stderr(&a));
    let text = stdout(&a);
    assert!(text.contains("tiny") && text.contains("horizon 4"), "{text}");
    for needle in ["7  screen", "101  gate", "201  owner", "screen  mean", "owner   mean"] {
        assert!(text.contains(needle), "missing {needle:?} in\n{text}");
    }
    let b = fx.eval(&champion, &["--threads", "1"]);
    assert_eq!(text, stdout(&b), "same strategy, same table, one thread or many");
    let c = fx.eval(&champion, &["--threads", "5"]);
    assert_eq!(text, stdout(&c), "same strategy, same table, five threads");
}

/// `--tiers` runs only the tiers named.
#[test]
fn tiers_can_be_picked() {
    let fx = Fixture::new("tiers");
    let champion = fx.champion(&[("governor", "wait(2);")], None);
    let out = fx.eval(&champion, &["--tiers", "gate"]);
    assert!(out.status.success(), "{}", stderr(&out));
    let text = stdout(&out);
    assert!(
        text.contains("101  gate") && !text.contains("screen") && !text.contains("owner  "),
        "{text}"
    );
    let bad = fx.eval(&champion, &["--tiers", "held-out"]);
    assert!(!bad.status.success() && stderr(&bad).contains("unknown tier"), "{}", stderr(&bad));
}

/// `--json` prints the same table as JSON, with the same determinism, and `--out` keeps the logs.
#[test]
fn json_is_the_same_table_and_out_keeps_the_logs() {
    let fx = Fixture::new("json");
    let champion = fx.champion(&[("governor", "wait(2);")], None);
    let out_dir = fx.tmp.join("logs");
    let a = fx.eval(&champion, &["--json", "--out", out_dir.to_str().unwrap()]);
    assert!(a.status.success(), "{}", stderr(&a));
    let j: serde_json::Value = serde_json::from_str(&stdout(&a)).unwrap();
    assert_eq!((j["track"].as_str(), j["horizon"].as_u64()), (Some("tiny"), Some(4)));
    let seeds = |panel: &str| -> Vec<u64> {
        j[panel].as_array().unwrap().iter().map(|r| r["seed"].as_u64().unwrap()).collect()
    };
    assert_eq!((seeds("screen"), seeds("gate"), seeds("owner")), (vec![7], vec![101], vec![201]));
    let scores: Vec<f64> =
        j["screen"].as_array().unwrap().iter().map(|r| r["score"].as_f64().unwrap()).collect();
    let mean = scores.iter().sum::<f64>() / scores.len() as f64;
    assert!((j["screen_mean"].as_f64().unwrap() - mean).abs() < 1e-9, "{j}");
    assert!(j["owner_mean"].is_number() && j["gate_failures"]["collapses"].is_number(), "{j}");
    assert!(j["screen"][0]["deaths"].is_number(), "{j}");
    for seed in [7, 101, 201] {
        assert!(out_dir.join(format!("seed-{seed}.jsonl")).is_file());
    }
    let b = fx.eval(&champion, &["--json"]);
    assert_eq!(stdout(&a), stdout(&b), "scores are deterministic across runs");
}

/// A script stopped by an error is still scored, and the table flags it instead of failing.
#[test]
fn a_stopped_script_is_scored_and_flagged() {
    let fx = Fixture::new("stopped");
    let champion = fx.champion(&[("governor", "wait(1); let x = 1 / 0;")], None);
    let out = fx.eval(&champion, &[]);
    assert!(out.status.success(), "{}", stderr(&out));
    let text = stdout(&out);
    assert!(text.contains("stopped by error at turn 1"), "{text}");
    assert!(text.contains("screen  mean"), "{text}");
    let j: serde_json::Value =
        serde_json::from_str(&stdout(&fx.eval(&champion, &["--json"]))).unwrap();
    assert_eq!(j["stopped"], 3, "{j}");
    assert_eq!(j["screen"][0]["stopped_by"]["reason"], "error");
    assert_eq!(j["screen_failures"]["stopped"], 1, "{j}");
}

/// A handoff with no `governor` script fails with a message that says so, and prints no table.
#[test]
fn a_handoff_without_a_governor_fails_clearly() {
    let fx = Fixture::new("nogovernor");
    let champion = fx.champion(&[("opener", "wait(1);")], None);
    let out = fx.eval(&champion, &[]);
    assert!(!out.status.success());
    assert!(stdout(&out).is_empty(), "{}", stdout(&out));
    let err = stderr(&out);
    assert!(err.contains("governor") && err.contains("opener"), "{err}");

    let empty = fx.champion(&[], None);
    let out = fx.eval(&empty, &[]);
    assert!(!out.status.success());
    assert!(stderr(&out).contains("governor"), "{}", stderr(&out));
}

/// A missing handoff and an unknown track each name what's wrong.
#[test]
fn a_missing_handoff_or_track_is_named() {
    let fx = Fixture::new("missing");
    let nowhere = fx.tmp.join("nowhere");
    let out = fx.eval(&nowhere, &[]);
    assert!(!out.status.success());
    assert!(stderr(&out).contains("handoff.json"), "{}", stderr(&out));

    let champion = fx.champion(&[("governor", "wait(1);")], None);
    let out = Command::new(xtask_bin())
        .args(["agent-eval", "--track", "nope", "--champion"])
        .arg(&champion)
        .arg("--tracks")
        .arg(fx.tmp.join("tracks.toml"))
        .output()
        .unwrap();
    assert!(!out.status.success());
    let err = stderr(&out);
    assert!(err.contains("nope") && err.contains("tiny"), "{err}");
}

/// A child run that never comes back is bounded (#564): `evaluate` kills it after
/// `$FN_AGENT_PLAY_TIMEOUT_SECS` and fails with an error that says so, rather than hanging the
/// whole evaluation or scoring it. A stub `fn_agent` that sleeps stands in for the stuck child.
#[test]
fn a_child_that_never_returns_is_killed_and_reported_not_scored() {
    use std::os::unix::fs::PermissionsExt;

    let fx = Fixture::new("hang");
    let stub = fx.tmp.join("fn_agent_hang");
    fs::write(&stub, "#!/bin/sh\nexec sleep 60\n").unwrap();
    fs::set_permissions(&stub, fs::Permissions::from_mode(0o755)).unwrap();
    let champion = fx.champion(&[("governor", "wait(1);")], None);
    let started = std::time::Instant::now();
    let out = Command::new(xtask_bin())
        .args(["agent-eval", "--track", "tiny", "--champion"])
        .arg(&champion)
        .arg("--tracks")
        .arg(fx.tmp.join("tracks.toml"))
        .env("FN_AGENT_BIN", &stub)
        .env("FN_AGENT_PLAY_TIMEOUT_SECS", "1")
        .output()
        .unwrap();
    assert!(started.elapsed().as_secs() < 30, "the stub's 60 s sleep was not cut short");
    assert!(!out.status.success());
    let err = stderr(&out);
    assert!(err.contains("seed 7") && err.contains("ran longer than 1 s"), "{err}");
    assert!(stdout(&out).is_empty(), "no table for a harness failure: {}", stdout(&out));
}

/// Par and the do-nothing control sit beside the scores (#572), described as what a stock
/// scripted opening reaches reading the world directly, and kept per build: a second evaluation
/// prints the same table from the cache.
#[test]
fn par_and_the_do_nothing_control_sit_beside_the_scores() {
    let fx = Fixture::own_cache("refs");
    let champion = fx.champion(&[("governor", "wait(2);")], None);
    let a = fx.eval(&champion, &[]);
    assert!(a.status.success(), "{}", stderr(&a));
    let text = stdout(&a);
    let head = text.lines().find(|l| l.contains("progress")).expect("a header");
    assert!(head.contains("par") && head.contains("do-nothing"), "{head}");
    for needle in [
        "references (build ",
        "par ",
        "what a stock scripted opening reaches on this start, reading the world directly",
        "not a target a fair player should hit, and not a ceiling",
        "par mean",
        "do-nothing mean",
    ] {
        assert!(text.contains(needle), "missing {needle:?} in\n{text}");
    }
    assert!(stderr(&a).contains("computed"), "{}", stderr(&a));
    // The cache holds one par per seed and one control file per tier, under the build.
    let cache = &fx.cache;
    let count = |sub: &str| {
        fs::read_dir(cache.join(sub))
            .unwrap()
            .flat_map(|b| fs::read_dir(b.unwrap().path()).unwrap())
            .count()
    };
    assert_eq!((count("par"), count("controls")), (3, 3));
    // A second evaluation reads them back: the same bytes, nothing computed.
    let b = fx.eval(&champion, &["--threads", "1"]);
    assert_eq!(text, stdout(&b));
    assert!(stderr(&b).contains("0 computed"), "{}", stderr(&b));
}

/// `--control GOV@PLAN` is the syntax `agent-compare` takes; `--no-par` and `--no-controls` leave
/// the references out, and then the table is the one from before.
#[test]
fn a_fixed_governor_control_and_the_switches() {
    let fx = Fixture::new("control");
    let champion = fx.champion(&[("governor", "wait(2);")], None);
    fs::write(fx.tmp.join("busy.rhai"), "wait(3);").unwrap();
    fs::write(fx.tmp.join("busy.json"), r#"{"targets": {"wood": 10}}"#).unwrap();
    let spec =
        format!("{}@{}", fx.tmp.join("busy.rhai").display(), fx.tmp.join("busy.json").display());
    let out = fx.eval(&champion, &["--no-par", "--control", &spec, "--json"]);
    assert!(out.status.success(), "{}", stderr(&out));
    let j: serde_json::Value = serde_json::from_str(&stdout(&out)).unwrap();
    let labels: Vec<&str> = j["references"]["controls"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| c["label"].as_str().unwrap())
        .collect();
    assert_eq!(labels, ["do-nothing", "busy"], "{j}");
    assert!(j["references"].get("par").is_none(), "--no-par: {j}");
    assert_eq!(j["references"]["controls"][1]["seeds"].as_array().unwrap().len(), 3);

    let plain = fx.eval(&champion, &["--no-par", "--no-controls"]);
    assert!(plain.status.success(), "{}", stderr(&plain));
    let text = stdout(&plain);
    assert!(!text.contains("references") && !text.contains("par"), "{text}");
    let bad = fx.eval(&champion, &["--control", "nowhere.rhai"]);
    assert!(!bad.status.success() && stderr(&bad).contains("nowhere.rhai"), "{}", stderr(&bad));
}

/// Par is for one start: a track's seeds give pars of their own, and the same track on another
/// build or horizon would be another file. This one pins that a par equals what `fn_agent par`
/// prints for the same flags (the table's number is that number).
#[test]
fn the_tables_par_is_what_fn_agent_par_prints() {
    let fx = Fixture::new("parnum");
    let champion = fx.champion(&[("governor", "wait(2);")], None);
    let out = fx.eval(&champion, &["--no-controls", "--json", "--tiers", "screen"]);
    assert!(out.status.success(), "{}", stderr(&out));
    let j: serde_json::Value = serde_json::from_str(&stdout(&out)).unwrap();
    let table_par = j["references"]["par"][0]["score"].as_f64().unwrap();
    let direct = Command::new(fn_agent_bin())
        .args(["par", "--seed", "7", "--nation", "water", "--difficulty", "normal"])
        .args(["--map-size", "small", "--horizon", "4", "--json"])
        .env("FN_AGENT_CACHE", fx.tmp.join("other-cache"))
        .output()
        .unwrap();
    assert!(direct.status.success(), "{}", stderr(&direct));
    let v: serde_json::Value = serde_json::from_str(&stdout(&direct)).unwrap();
    assert_eq!(v["cached"], false);
    assert_eq!(v["score"].as_f64().unwrap(), table_par);
}
