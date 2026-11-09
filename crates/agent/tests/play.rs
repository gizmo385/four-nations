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

// ---------------------------------------------------------------------------------------------
// --horizon and --plan (#528, docs/design/agent-hillclimb.md, "Across runs: attempts and a
// champion"): a clean evaluation of a strategy to a fixed horizon.
// ---------------------------------------------------------------------------------------------

/// Runs `fn_agent play --horizon` on seed 7, Earth, with `extra` flags after the common ones;
/// returns the process output, the log's bytes and the log's path.
fn play_horizon(
    name: &str,
    script: &std::path::Path,
    horizon: u32,
    extra: &[&str],
) -> (std::process::Output, Vec<u8>, std::path::PathBuf) {
    let log = temp_path(&format!("{name}.jsonl"));
    let out = fn_agent()
        .args([
            "play",
            "--script",
            script.to_str().unwrap(),
            "--seed",
            "7",
            "--nation",
            "earth",
            "--horizon",
            &horizon.to_string(),
            "--log",
            log.to_str().unwrap(),
        ])
        .args(extra)
        .output()
        .unwrap();
    let bytes = std::fs::read(&log).unwrap_or_default();
    (out, bytes, log)
}

fn report(out: &std::process::Output) -> serde_json::Value {
    serde_json::from_slice(&out.stdout)
        .unwrap_or_else(|e| panic!("{e}: {}", String::from_utf8_lossy(&out.stderr)))
}

/// The log's `Turn` count and its `Wake` reasons, in order.
fn turns_and_wakes(log: &std::path::Path) -> (usize, Vec<String>) {
    let (_, lines) = watch::read_file(log).unwrap();
    let turns = lines.iter().filter(|l| matches!(l, fn_tui::surface::LogLine::Turn { .. })).count();
    let wakes = lines
        .iter()
        .filter_map(|l| match l {
            fn_tui::surface::LogLine::Wake { reason, .. } => Some(reason.clone()),
            _ => None,
        })
        .collect();
    (turns, wakes)
}

/// A script that returns at once still reaches the horizon, without spinning: each empty run is
/// followed by one forced turn, and every stop has its `wake` line.
#[test]
fn horizon_a_script_that_returns_at_once_reaches_the_horizon() {
    let script = temp_script("h_done.rhai", "1 + 1");
    let (out, _, log) = play_horizon("h_done", &script, 10, &[]);
    std::fs::remove_file(&script).ok();
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    let r = report(&out);
    assert_eq!(r["turns"], 10, "{r}");
    assert_eq!(r["restarts"], 9, "{r}");
    assert_eq!(r["stopped_by"], serde_json::Value::Null);
    assert_eq!(r["scorecard"]["horizon"], 10);
    assert_eq!(r["scorecard"]["played"], 10);
    assert_eq!(r["observe"]["turn"], 10);
    let (turns, wakes) = turns_and_wakes(&log);
    std::fs::remove_file(&log).ok();
    assert_eq!(turns, 10);
    assert_eq!(wakes, vec!["done"; 10]);
}

/// `wake()` restarts the script with its `memory` kept, and a script that waits runs again from
/// where it stopped, not from turn 0. The last run is cut by the horizon (`budget`).
#[test]
fn horizon_restarts_on_done_and_asked_keeping_memory() {
    let script = temp_script(
        "h_memory.rhai",
        "memory.runs = (memory.runs ?? 0) + 1; wait(3); if memory.runs == 2 { wake(\"again\"); }",
    );
    let (out, _, log) = play_horizon("h_memory", &script, 10, &[]);
    std::fs::remove_file(&script).ok();
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    let r = report(&out);
    // Turns 0-3, 3-6, 6-9 run to the end; the fourth run has one turn left and hits the budget.
    assert_eq!(r["restarts"], 3, "{r}");
    assert_eq!(r["reason"], "budget", "{r}");
    assert_eq!(r["memory"]["runs"], 4, "{r}");
    let (_, wakes) = turns_and_wakes(&log);
    std::fs::remove_file(&log).ok();
    assert_eq!(wakes, vec!["done", "asked", "done", "budget"]);
}

/// An erroring script stops the orders, not the clock: the rest of the horizon coasts, the first
/// error is `stopped_by`, and the exit is nonzero.
#[test]
fn horizon_an_erroring_script_coasts_to_the_horizon() {
    let script = temp_script("h_error.rhai", "wait(2); let x = 1 / 0;");
    let (out, _, log) = play_horizon("h_error", &script, 8, &[]);
    std::fs::remove_file(&script).ok();
    assert!(!out.status.success());
    let r = report(&out);
    assert_eq!(r["turns"], 8, "{r}");
    assert_eq!(r["restarts"], 0, "{r}");
    assert_eq!(r["stopped_by"]["reason"], "error", "{r}");
    assert_eq!(r["stopped_by"]["turn"], 2, "{r}");
    assert_eq!(r["scorecard"]["played"], 8, "{r}");
    let (turns, wakes) = turns_and_wakes(&log);
    std::fs::remove_file(&log).ok();
    assert_eq!(turns, 8);
    assert_eq!(wakes, vec!["error"]);
}

/// A loop with no `wait` is a `runaway`; it coasts the same way.
#[test]
fn horizon_a_runaway_script_coasts_to_the_horizon() {
    let script = temp_script("h_runaway.rhai", "loop { let x = 1; }");
    let (out, _, log) = play_horizon("h_runaway", &script, 5, &[]);
    std::fs::remove_file(&script).ok();
    std::fs::remove_file(&log).ok();
    assert!(!out.status.success());
    let r = report(&out);
    assert_eq!(r["turns"], 5, "{r}");
    assert_eq!(r["stopped_by"]["reason"], "runaway", "{r}");
    assert_eq!(r["stopped_by"]["turn"], 0, "{r}");
}

/// A loop of native calls with no `wait` is a `runaway` too, and quickly (#564): Rhai counts an
/// `observe()` as one operation, but the budget charges what the host does for it.
#[test]
fn horizon_a_native_call_loop_with_no_wait_is_a_runaway() {
    let script = temp_script("h_native.rhai", "loop { observe(); }");
    let (out, _, log) = play_horizon("h_native", &script, 5, &[]);
    std::fs::remove_file(&script).ok();
    std::fs::remove_file(&log).ok();
    assert!(!out.status.success());
    let r = report(&out);
    assert_eq!(r["stopped_by"]["reason"], "runaway", "{r}");
    assert_eq!(r["stopped_by"]["turn"], 0, "{r}");
    let why = r["stopped_by"]["why"].as_str().unwrap_or_default();
    assert!(why.contains("Stopped at line 1"), "{r}");
}

/// A loop with a `wait` in it never runs away, however much work the whole run does (#564): 20
/// passes of about 60,000 operations is more than the whole budget, and the run is one script
/// to the horizon, with no restart.
#[test]
fn horizon_a_loop_that_waits_runs_the_whole_horizon_in_one_run() {
    let script =
        temp_script("h_waits.rhai", "let x = 0; loop { for i in 0..20000 { x += 1; } wait(1); }");
    let (out, _, log) = play_horizon("h_waits", &script, 20, &[]);
    std::fs::remove_file(&script).ok();
    std::fs::remove_file(&log).ok();
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    let r = report(&out);
    assert_eq!(r["stopped_by"], serde_json::Value::Null, "{r}");
    assert_eq!(r["restarts"], 0, "{r}");
    assert_eq!(r["turns_run"], 20, "{r}");
}

/// Event wakes are ignored: the run that `--wake-on need` stopped early in the `--turns` test
/// above goes on to the horizon.
#[test]
fn horizon_ignores_event_wakes() {
    let script = temp_script("h_events.rhai", "loop { wait(1); }");
    let (out, _, log) = play_horizon("h_events", &script, 6, &["--turn", "600"]);
    std::fs::remove_file(&script).ok();
    std::fs::remove_file(&log).ok();
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    let r = report(&out);
    assert_eq!(r["reason"], "budget", "{r}");
    assert_eq!(r["turns_run"], 6, "{r}");
    assert_eq!(r["restarts"], 0, "{r}");
}

/// `--plan FILE` is what `plan()` returns, and the log carries it as a `plan` line. Two runs of
/// the same script, plan and seed write the same log and report the same score; the stdout
/// scorecard is what `fn_agent score LOG --horizon N --json` prints for the log.
#[test]
fn horizon_with_a_plan_is_deterministic_and_matches_fn_agent_score() {
    let script =
        temp_script("h_plan.rhai", "memory.goal = plan().goal; for i in 0..plan().n { wait(1); }");
    let plan = temp_script("h_plan.json", r#"{"goal": "stay alive", "n": 4}"#);
    let plan_arg = plan.to_str().unwrap();
    let (out1, log1, path1) = play_horizon("h_plan_a", &script, 30, &["--plan", plan_arg]);
    let (out2, log2, path2) = play_horizon("h_plan_b", &script, 30, &["--plan", plan_arg]);
    std::fs::remove_file(&script).ok();
    std::fs::remove_file(&plan).ok();
    assert!(out1.status.success(), "{}", String::from_utf8_lossy(&out1.stderr));
    assert_eq!(out1.stdout, out2.stdout, "the report should be identical both times");

    let digest = |bytes: &[u8]| -> u64 {
        use std::hash::{Hash, Hasher};
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        bytes.hash(&mut hasher);
        hasher.finish()
    };
    assert_eq!(digest(&log1), digest(&log2), "the run log digest should be stable");

    let r = report(&out1);
    assert_eq!(r["memory"]["goal"], "stay alive", "{r}");
    // Runs of 4 turns: 0-4 ... 24-28 (7 runs), then a last one cut to 2 turns by the horizon.
    assert_eq!(r["restarts"], 7, "{r}");
    let (_, lines) = watch::read_file(&path1).unwrap();
    assert!(
        lines.iter().any(|l| matches!(l, fn_tui::surface::LogLine::Plan { plan }
            if plan["goal"] == "stay alive")),
        "the log should carry the plan"
    );

    let scored = fn_agent()
        .args(["score", path1.to_str().unwrap(), "--horizon", "30", "--json"])
        .output()
        .unwrap();
    assert!(scored.status.success(), "{}", String::from_utf8_lossy(&scored.stderr));
    let scored: serde_json::Value = serde_json::from_slice(&scored.stdout).unwrap();
    assert_eq!(r["scorecard"], scored);
    assert_eq!(r["scorecard"]["horizon"], 30);
    std::fs::remove_file(&path1).ok();
    std::fs::remove_file(&path2).ok();
}

/// `--plan` also works with `--turns`, and a plan that isn't a JSON object is refused before
/// anything runs.
#[test]
fn plan_flag_works_with_turns_and_refuses_a_bad_file() {
    let script = temp_script("p_turns.rhai", "memory.n = plan().n; 1");
    let plan = temp_script("p_turns.json", r#"{"n": 3}"#);
    let log = temp_path("p_turns.jsonl");
    let args = |plan: &std::path::Path| {
        vec![
            "play".to_owned(),
            "--script".to_owned(),
            script.to_str().unwrap().to_owned(),
            "--seed".to_owned(),
            "7".to_owned(),
            "--nation".to_owned(),
            "earth".to_owned(),
            "--turns".to_owned(),
            "5".to_owned(),
            "--plan".to_owned(),
            plan.to_str().unwrap().to_owned(),
            "--log".to_owned(),
            log.to_str().unwrap().to_owned(),
        ]
    };
    let out = fn_agent().args(args(&plan)).output().unwrap();
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    assert_eq!(report(&out)["memory"]["n"], 3);

    let bad = temp_script("p_bad.json", "[1, 2]");
    let out = fn_agent().args(args(&bad)).output().unwrap();
    assert!(!out.status.success());
    assert!(String::from_utf8_lossy(&out.stderr).contains("--plan"), "{:?}", out.stderr);
    for p in [&script, &plan, &bad, &log] {
        std::fs::remove_file(p).ok();
    }
}

/// `--horizon` and `--turns` are different modes; asking for both is a usage error.
#[test]
fn horizon_with_turns_is_a_usage_error() {
    let script = temp_script("h_both.rhai", "1");
    let (out, _, log) = play_horizon("h_both", &script, 5, &["--turns", "5"]);
    std::fs::remove_file(&script).ok();
    std::fs::remove_file(&log).ok();
    assert!(!out.status.success());
    assert!(String::from_utf8_lossy(&out.stderr).contains("--horizon"));
}
