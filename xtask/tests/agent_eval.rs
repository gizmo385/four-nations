//! End-to-end coverage of `cargo xtask agent-eval` (#529) against the real `fn_agent`, on a tiny
//! track (a few turns, two train seeds and one held-out seed) so it runs in seconds. No model is
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
train_seeds = [7, 42]
holdout_seeds = [1]
"#;

struct Fixture {
    tmp: PathBuf,
}

impl Fixture {
    fn new(name: &str) -> Self {
        let tmp =
            std::env::temp_dir().join(format!("fn_agent_eval_it_{name}_{}", std::process::id()));
        let _ = fs::remove_dir_all(&tmp);
        fs::create_dir_all(&tmp).unwrap();
        fs::write(tmp.join("tracks.toml"), TRACKS).unwrap();
        Self { tmp }
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

/// The table has a row per train and held-out seed, the train mean and the held-out scores, and
/// two evaluations of the same strategy print the same bytes.
#[test]
fn the_table_has_its_shape_and_is_deterministic() {
    let fx = Fixture::new("shape");
    let champion = fx.champion(&[("governor", "wait(2);")], Some(r#"{"targets": {"wood": 10}}"#));
    let a = fx.eval(&champion, &[]);
    assert!(a.status.success(), "{}", stderr(&a));
    let text = stdout(&a);
    assert!(text.contains("tiny") && text.contains("horizon 4"), "{text}");
    for needle in ["7  train", "42  train", "1  held-out", "train mean", "held-out"] {
        assert!(text.contains(needle), "missing {needle:?} in\n{text}");
    }
    let b = fx.eval(&champion, &[]);
    assert_eq!(text, stdout(&b), "same strategy, same table");
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
    assert_eq!((seeds("train"), seeds("holdout")), (vec![7, 42], vec![1]));
    let scores: Vec<f64> =
        j["train"].as_array().unwrap().iter().map(|r| r["score"].as_f64().unwrap()).collect();
    let mean = scores.iter().sum::<f64>() / scores.len() as f64;
    assert!((j["train_mean"].as_f64().unwrap() - mean).abs() < 1e-9, "{j}");
    assert!(j["holdout_mean"].is_number(), "{j}");
    for seed in [7, 42, 1] {
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
    assert!(text.contains("train mean"), "{text}");
    let j: serde_json::Value =
        serde_json::from_str(&stdout(&fx.eval(&champion, &["--json"]))).unwrap();
    assert_eq!(j["stopped"], 3, "{j}");
    assert_eq!(j["train"][0]["stopped_by"]["reason"], "error");
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
