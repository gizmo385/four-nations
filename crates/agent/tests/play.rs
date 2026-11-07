//! `fn_agent play` end-to-end, through the real binary (#483, docs/design/agent-scripting.md,
//! "Headless play"): no model, no MCP loop, just `--script FILE` run once against a fresh
//! `Host` and a run log written to `--log PATH`.
//!
//! `crates/agent/scripts/settle.rhai` -- the stock library this issue ships -- gets its own
//! `ci --fast` slice here (determinism across two runs) and its own soak test (the full year,
//! `#[ignore]`d the same way a heavier sweep stays out of `cargo xtask ci --fast`, the gate
//! `crates/sim/tests/balance.rs`'s own module doc explains is for the `balance` binary's job,
//! not a default `cargo test`).

use std::process::{Command, Stdio};

use fn_tui::surface::Host;
use fn_tui::watch;

const SETTLE: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/scripts/settle.rhai");

fn fn_agent() -> Command {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_fn_agent"));
    cmd.stdin(Stdio::null());
    cmd
}

/// A fresh temp path this process alone uses, so parallel test runs never collide.
fn temp_path(name: &str) -> std::path::PathBuf {
    std::env::temp_dir().join(format!("fn_agent_play_test_{}_{name}", std::process::id()))
}

/// Writes `code` to a fresh temp `.rhai` file and returns its path, for a test that needs an ad
/// hoc script rather than the stock library.
fn temp_script(name: &str, code: &str) -> std::path::PathBuf {
    let path = temp_path(name);
    std::fs::write(&path, code).unwrap();
    path
}

/// A script that returns right away is `"done"`, and `play` exits 0.
#[test]
fn a_script_that_returns_is_done_and_exits_zero() {
    let script = temp_script("done.rhai", "1 + 41");
    let log = temp_path("done.jsonl");
    let out = fn_agent()
        .args([
            "play",
            "--script",
            script.to_str().unwrap(),
            "--seed",
            "7",
            "--nation",
            "earth",
            "--turns",
            "5",
            "--log",
            log.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    std::fs::remove_file(&script).ok();
    std::fs::remove_file(&log).ok();
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    let report: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(report["reason"], "done");
    assert_eq!(report["value"], 42);
}

/// A script that errors is `"error"`, and `play` exits nonzero -- the shell-loop signal the
/// acceptance criteria asks for (docs/design/agent-scripting.md, "Headless play").
#[test]
fn a_script_that_errors_exits_nonzero() {
    let script = temp_script("error.rhai", "let x = 1 / 0;");
    let log = temp_path("error.jsonl");
    let out = fn_agent()
        .args([
            "play",
            "--script",
            script.to_str().unwrap(),
            "--seed",
            "7",
            "--nation",
            "earth",
            "--turns",
            "5",
            "--log",
            log.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    std::fs::remove_file(&script).ok();
    std::fs::remove_file(&log).ok();
    assert!(!out.status.success());
    let report: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(report["reason"], "error");
}

/// A `--wake-on` event landing during a `wait` stops the script `"event"`, still exits
/// nonzero, and the run log carries the `Script` and `Wake` lines that bracket it, in order.
#[test]
fn a_wake_on_event_exits_nonzero_and_logs_script_and_wake_lines() {
    let script = temp_script("wake.rhai", "loop { wait(1); }");
    let log = temp_path("wake.jsonl");
    let out = fn_agent()
        .args([
            "play",
            "--script",
            script.to_str().unwrap(),
            "--seed",
            "7",
            "--nation",
            "earth",
            "--turn",
            "600",
            "--turns",
            "50",
            "--wake-on",
            "need",
            "--log",
            log.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    std::fs::remove_file(&script).ok();
    let report: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(report["reason"], "event", "{report}");
    assert!(!out.status.success());

    let (_, lines) = watch::read_file(&log).unwrap();
    std::fs::remove_file(&log).ok();
    let types: Vec<&str> = lines
        .iter()
        .map(|l| match l {
            fn_tui::surface::LogLine::Script { .. } => "script",
            fn_tui::surface::LogLine::Turn { .. } => "turn",
            fn_tui::surface::LogLine::Wake { .. } => "wake",
            fn_tui::surface::LogLine::End { .. } => "end",
            _ => "other",
        })
        .collect();
    assert_eq!(types.first(), Some(&"script"));
    assert_eq!(types.last(), Some(&"end"));
    assert!(types.contains(&"wake"), "{types:?}");
    let wake_at = types.iter().position(|t| *t == "wake").unwrap();
    assert!(types[..wake_at].contains(&"turn"), "turns ran before the wake: {types:?}");
}

/// `play`'s own usage mistakes (a missing `--script`, `--turns` or `--log`) are a plain error
/// on stderr and a nonzero exit, before anything is read or written.
#[test]
fn missing_required_flags_is_a_usage_error() {
    let out = fn_agent().args(["play", "--seed", "7", "--nation", "earth"]).output().unwrap();
    assert!(!out.status.success());
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(err.contains("--script") || err.contains("--turns") || err.contains("--log"), "{err}");
}

/// `crates/agent/scripts/settle.rhai`'s own `ci --fast` slice (#483's acceptance criteria): a
/// 30-turn run of the stock library on seed 7, Earth, Normal, run twice, gives the same run log
/// both times -- the same determinism `fn_agent::script::runner`'s own tests already check for
/// one `run_script` call, now end to end through the real binary and a real file.
#[test]
fn settle_rhai_thirty_turns_is_deterministic_across_two_runs() {
    let run = |tag: &str| -> (std::process::Output, Vec<u8>) {
        let log = temp_path(&format!("settle_det_{tag}.jsonl"));
        let out = fn_agent()
            .args([
                "play",
                "--script",
                SETTLE,
                "--seed",
                "7",
                "--nation",
                "earth",
                "--difficulty",
                "normal",
                "--turns",
                "30",
                "--wake-on",
                "death",
                "--log",
                log.to_str().unwrap(),
            ])
            .output()
            .unwrap();
        let bytes = std::fs::read(&log).unwrap();
        std::fs::remove_file(&log).ok();
        (out, bytes)
    };

    let (out1, log1) = run("a");
    let (out2, log2) = run("b");
    assert!(out1.status.success(), "{}", String::from_utf8_lossy(&out1.stderr));
    assert!(out2.status.success(), "{}", String::from_utf8_lossy(&out2.stderr));
    assert_eq!(out1.stdout, out2.stdout, "the wake report should be identical both times");

    let digest = |bytes: &[u8]| -> u64 {
        use std::hash::{Hash, Hasher};
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        bytes.hash(&mut hasher);
        hasher.finish()
    };
    assert_eq!(digest(&log1), digest(&log2), "the run log digest should be stable across runs");

    // Also proves the slice actually ran something and didn't silently error out.
    let report: serde_json::Value = serde_json::from_slice(&out1.stdout).unwrap();
    assert_eq!(report["reason"], "budget", "{report}");
    assert_eq!(report["turns_run"], 30);
}

/// The soak test (outside `ci --fast`, like `crates/sim/tests/balance.rs`'s own CI soak stays
/// cheap enough to run inside it while a much larger sweep belongs to the standalone `balance`
/// binary instead): `settle.rhai` plays the whole first game year (`--turns 180`, the one-year
/// cap `data/agent.toml`'s own `max_turns` sets) on seed 7, Earth, Normal, and nobody starves.
#[test]
#[ignore = "a full game year; run explicitly with `cargo test -p fn_agent -- --ignored`"]
fn settle_rhai_plays_a_full_year_with_no_death() {
    let log = temp_path("settle_soak.jsonl");
    let out = fn_agent()
        .args([
            "play",
            "--script",
            SETTLE,
            "--seed",
            "7",
            "--nation",
            "earth",
            "--difficulty",
            "normal",
            "--turns",
            "180",
            "--wake-on",
            "death",
            "--log",
            log.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    std::fs::remove_file(&log).ok();
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    let report: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(report["reason"], "budget", "a death would have stopped it \"event\": {report}");
    assert_eq!(report["turns_run"], 180);
    let people = report["since_start"]["people"].as_str().unwrap();
    let (before, after) = people.split_once(" -> ").unwrap();
    assert!(
        after.parse::<u32>().unwrap() >= before.parse::<u32>().unwrap(),
        "nobody should have starved over the year: {people}"
    );
}

/// `fn_agent keys` (which reads the exact same `RunLogHeader`/`LogLine` schema `--watch` does)
/// still prints a working command line for a log `play` wrote -- proving `play`'s own log is
/// the same shape `serve`'s is, not a special case `keys` or `--watch` need to know about.
#[test]
fn fn_agent_keys_reads_a_play_log() {
    let script = temp_script("keys.rhai", r#"keys("<right>"); wait(1);"#);
    let log = temp_path("keys.jsonl");
    let play_out = fn_agent()
        .args([
            "play",
            "--script",
            script.to_str().unwrap(),
            "--seed",
            "7",
            "--nation",
            "fire",
            "--turns",
            "3",
            "--log",
            log.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    std::fs::remove_file(&script).ok();
    assert!(play_out.status.success(), "{}", String::from_utf8_lossy(&play_out.stderr));

    let keys_out = fn_agent().args(["keys", log.to_str().unwrap()]).output().unwrap();
    std::fs::remove_file(&log).ok();
    assert!(keys_out.status.success(), "{}", String::from_utf8_lossy(&keys_out.stderr));
    let line = String::from_utf8(keys_out.stdout).unwrap();
    assert!(line.contains("7 fire"), "{line}");
    assert!(line.contains("--frame --keys"), "{line}");
}

/// `fn_tui --watch` replays a `play` log to the same final frame the live `Host` running the
/// same steps would reach (#483's own acceptance criteria, pending `fn_agent verify` (#318)):
/// an `act`, a `wait`, exactly as the script asked for.
#[test]
fn watch_replays_a_play_log_to_the_same_final_frame() {
    const SIZE: (u16, u16) = (100, 40);
    let script = temp_script(
        "watch.rhai",
        r#"act([#{step: "cursor", x: 5, y: 5}], "scout the east"); wait(1);"#,
    );
    let log = temp_path("watch.jsonl");
    let out = fn_agent()
        .args([
            "play",
            "--script",
            script.to_str().unwrap(),
            "--seed",
            "7",
            "--nation",
            "fire",
            "--turn",
            "10",
            "--turns",
            "1",
            "--log",
            log.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    std::fs::remove_file(&script).ok();
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));

    // The reference frame: the same steps, run directly against a fresh `Host`.
    let nation = fn_tui::cli::parse_nation("fire").unwrap();
    let mut reference =
        Host::new(7, nation, None, None, None, false).unwrap().with_turn_seconds(10);
    let steps: Vec<fn_tui::surface::ActionStep> =
        serde_json::from_value(serde_json::json!([{"step": "cursor", "x": 5, "y": 5}])).unwrap();
    reference.act(&steps, Some("scout the east".into()));
    reference.end_turn(1);
    let want = reference.frame(SIZE.0, SIZE.1);

    let (header, lines) = watch::read_file(&log).unwrap();
    std::fs::remove_file(&log).ok();
    let mut watcher = watch::Watcher::new(&header).unwrap();
    watcher.feed(lines);
    watcher.fast_forward_to_turn(1);

    let palette = fn_tui::color::Palette::new(
        watcher.defs(),
        watcher.nation(),
        fn_tui::color::ColorDepth::TrueColor,
        true,
    );
    let got = fn_tui::frame::to_text(&watcher.render(&palette, SIZE));

    // The watcher's bottom row is the agent strip, not the key hints; compare everything above
    // it, the same trim `crates/agent/tests/watch_parity.rs` already uses.
    let trim_strip =
        |s: &str| s.lines().take(usize::from(SIZE.1) - 1).collect::<Vec<_>>().join("\n");
    assert_eq!(trim_strip(&got), trim_strip(&want));
}
