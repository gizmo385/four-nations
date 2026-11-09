//! The reference governor (#564): a realistic governor, run on the real `fn_agent` binary for a
//! full clean evaluation (540 turns, the `water-3h` track's start), must finish with no stop.
//!
//! `reference/governor.rhai` is the campaign champion of that track with the waits it was
//! scattered with taken out, written as one `loop` with a single `wait` per pass: the shape a
//! model writes. It does about 45,000 Rhai operations and up to 40 `act`s in its heaviest pass.
//! The operation budget used to count over the whole run (500,000 operations), so this governor
//! ran away at about turn 270 of 540 (the operation counts of its passes, summed; see
//! `docs/design/agent-scripting.md`, "The operation budget", for what it counts now). A change
//! to the budget, to what a native call costs, or to the runner that stops a healthy governor
//! fails this test, which no short script can show. The fixture lives under `tests/` and is
//! never shipped to an agent.
//!
//! It is the one slow test in this crate: a release build takes about 85 s for this seed, most of
//! it in the sim (flood fills for the jobs the governor designates) and in Rhai. `Cargo.toml`
//! builds those crates with optimisations in the `ci` profile for that reason, which brings the
//! test to about 90 s in the fast gate (over 5 minutes without).

use std::process::{Command, Stdio};

const GOVERNOR: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/reference/governor.rhai");
const PLAN: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/reference/plan.json");

/// The score this governor reaches on seed 7633 at the time of writing is about 418. A governor
/// that stops (or never ran) coasts to a score below 60, so the floor is set well under the
/// reference and well over that; a balance change that moves it a little is not a failure.
const SCORE_FLOOR: f64 = 250.0;

#[test]
fn the_reference_governor_plays_all_540_turns_with_no_stop() {
    let log = std::env::temp_dir().join(format!("fn_agent_reference_{}.jsonl", std::process::id()));
    let out = Command::new(env!("CARGO_BIN_EXE_fn_agent"))
        .args(["play", "--script", GOVERNOR, "--plan", PLAN, "--horizon", "540"])
        .args(["--seed", "7633", "--nation", "water", "--difficulty", "normal"])
        .args(["--map-size", "small", "--log"])
        .arg(&log)
        .stdin(Stdio::null())
        .output()
        .unwrap();
    std::fs::remove_file(&log).ok();
    let stderr = String::from_utf8_lossy(&out.stderr);
    let report: serde_json::Value =
        serde_json::from_slice(&out.stdout).unwrap_or_else(|e| panic!("no report ({e}): {stderr}"));

    assert_eq!(report["stopped_by"], serde_json::Value::Null, "the governor stopped: {report}");
    assert!(out.status.success(), "{stderr}");
    assert_eq!(report["restarts"], 0, "one run to the horizon, the loop never returns: {report}");
    assert_eq!(report["turns"], 540, "{report}");
    let score = report["scorecard"]["score"].as_f64().expect("a score");
    assert!(score >= SCORE_FLOOR, "score {score} is under the floor {SCORE_FLOOR}: {report}");
}
