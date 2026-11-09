//! The reference governor (#564): a realistic governor, run on the real `fn_agent` binary from the
//! `water-3h` track's start for 330 turns of a clean evaluation, must finish with no stop.
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
//!
//! Why 330 turns and not the track's full 540: the turn the old budget ran away at (about 270)
//! is what this test has to get past, on a colony grown big enough (16 people by turn 300) that
//! its passes are as heavy as they get. Later turns cost the most to simulate and showed nothing
//! more, and the counting rule itself (the count starts again at every `wait`, however many
//! operations a run adds up to) is pinned by `script::runner`'s own unit tests. 330 turns takes
//! about half the CI time 540 did.
//!
//! It is still the one slow test in this crate, most of it in the sim (flood fills for the jobs
//! the governor designates) and in Rhai. `Cargo.toml` builds those crates with optimisations in
//! the `ci` profile for that reason (several times slower without).

use std::process::{Command, Stdio};

const GOVERNOR: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/reference/governor.rhai");
const PLAN: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/reference/plan.json");

/// Turns played: past the old runaway (see the module doc).
const HORIZON: u32 = 330;

/// The score this governor reaches on seed 7633 at 330 turns at the time of writing is about
/// 350. A script that only waits scores about 13 there, so the floor is set well under the
/// reference and well over that; a balance change that moves it a little is not a failure.
const SCORE_FLOOR: f64 = 150.0;

#[test]
fn the_reference_governor_plays_past_the_old_runaway_with_no_stop() {
    let log = std::env::temp_dir().join(format!("fn_agent_reference_{}.jsonl", std::process::id()));
    let out = Command::new(env!("CARGO_BIN_EXE_fn_agent"))
        .args(["play", "--script", GOVERNOR, "--plan", PLAN, "--horizon", &HORIZON.to_string()])
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
    assert_eq!(report["turns"], HORIZON, "{report}");
    let score = report["scorecard"]["score"].as_f64().expect("a score");
    assert!(score >= SCORE_FLOOR, "score {score} is under the floor {SCORE_FLOOR}: {report}");
}
