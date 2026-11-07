//! `fn_tui --watch LOG` (#319) through the real binary: `--frame --at TURN` needs no terminal,
//! so it's tested the same way `--frame` always is (`tests/binary.rs`), never by launching the
//! interactive loop.

use std::io::Write;
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicU32, Ordering};

use fn_sim::Nation;
use fn_tui::surface::{Host, LogLine};

fn fn_tui() -> Command {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_fn_tui"));
    cmd.stdin(Stdio::null()).env_remove("NO_COLOR").env_remove("COLORTERM");
    cmd
}

/// A short run log: a harvest designation, two turns, and a look, written as `fn_agent` (#317)
/// will write one -- a header line, then one line an event -- to a fresh temp file. Every call
/// gets its own file (a counter on top of the process id): `cargo test` runs this file's tests
/// in parallel threads of the one process, so a shared name would race.
fn write_log() -> std::path::PathBuf {
    static COUNTER: AtomicU32 = AtomicU32::new(0);
    let mut h = Host::new(7, Nation::Earth, None, None, None, false).unwrap().with_turn_seconds(5);
    h.press_keys(
        "h <right*3> <enter> <down*2> <enter>",
        Some("harvest".into()),
        Some("wood".into()),
    )
    .unwrap();
    h.end_turn(1);
    h.look(80, 60, 6, 4);
    h.end_turn(1);
    let text = h.log().to_jsonl(&h.header());

    let n = COUNTER.fetch_add(1, Ordering::Relaxed);
    let path = std::env::temp_dir()
        .join(format!("fn_tui_watch_binary_test_{}_{n}.jsonl", std::process::id()));
    let mut f = std::fs::File::create(&path).unwrap();
    f.write_all(text.as_bytes()).unwrap();
    path
}

#[test]
fn watch_frame_at_a_turn_shows_the_watched_session_not_the_key_hints() {
    let path = write_log();
    let out = fn_tui()
        .args(["--watch", path.to_str().unwrap(), "--frame", "--at", "1", "--size", "100x30"])
        .output()
        .unwrap();
    std::fs::remove_file(&path).ok();
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    let text = String::from_utf8(out.stdout).unwrap();
    assert_eq!(text.lines().count(), 30);
    // The agent strip replaces the bottom key-hint bar (docs/design/agent-play.md, "Watching").
    assert!(text.contains("AGENT earth"), "{text}");
    assert!(text.contains("turn 1"), "{text}");
    assert!(!text.contains("arrows/wasd move"), "the key hints should be gone:\n{text}");
}

/// Without `--at`, the frame is of the log as far as it currently reaches (both turns, here).
#[test]
fn watch_frame_without_at_reaches_the_end_of_the_log() {
    let path = write_log();
    let out = fn_tui().args(["--watch", path.to_str().unwrap(), "--frame"]).output().unwrap();
    std::fs::remove_file(&path).ok();
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    let text = String::from_utf8(out.stdout).unwrap();
    assert!(text.contains("turn 2"), "{text}");
}

/// `--at` needs `--watch --frame`; `--pace` needs `--watch`; `--watch` can't take `SEED NATION`
/// or `--fast` (#319's and #321's acceptance criteria: watch has its own pace, never the sim's
/// clock, and `--fast` is a different lane entirely).
#[test]
fn mistakes_say_what_is_wrong() {
    let out = fn_tui().args(["--at", "3"]).output().unwrap();
    assert!(!out.status.success());
    assert!(String::from_utf8_lossy(&out.stderr).contains("--at only means something"));

    let out = fn_tui().args(["--pace", "x10"]).output().unwrap();
    assert!(!out.status.success());
    assert!(String::from_utf8_lossy(&out.stderr).contains("--pace only means something"));

    let out = fn_tui().args(["7", "earth", "--watch", "run.jsonl"]).output().unwrap();
    assert!(!out.status.success());
    assert!(String::from_utf8_lossy(&out.stderr).contains("takes the place of SEED NATION"));

    let out = fn_tui().args(["--watch", "run.jsonl", "--fast"]).output().unwrap();
    assert!(!out.status.success());
    assert!(String::from_utf8_lossy(&out.stderr).contains("can't be combined"));
}

/// Without a terminal and without `--frame`, `--watch` says so and exits instead of hanging
/// (the same shape the plain path's own "this needs a terminal" takes, `tests/binary.rs`).
#[test]
fn watch_without_a_terminal_or_frame_says_so() {
    let out = fn_tui().args(["--watch", "nonexistent.jsonl"]).output().unwrap();
    assert!(!out.status.success());
    assert!(String::from_utf8_lossy(&out.stderr).contains("needs a terminal"));
}

/// `--keys` plays free-look spectator keys before a `--watch --frame` (#330's acceptance
/// criteria): `V` switches to Free look, the strip says so, and moving the spectator's own
/// cursor away from the agent's doesn't change which turn or action the strip reports (the
/// replay is unaffected).
#[test]
fn watch_frame_keys_plays_free_look_spectator_keys() {
    let path = write_log();
    let follow = fn_tui()
        .args(["--watch", path.to_str().unwrap(), "--frame", "--at", "1", "--size", "100x30"])
        .output()
        .unwrap();
    let free = fn_tui()
        .args([
            "--watch",
            path.to_str().unwrap(),
            "--frame",
            "--at",
            "1",
            "--size",
            "100x30",
            "--keys",
            "v <right*5> <down*3> n",
        ])
        .output()
        .unwrap();
    std::fs::remove_file(&path).ok();
    assert!(follow.status.success(), "{}", String::from_utf8_lossy(&follow.stderr));
    assert!(free.status.success(), "{}", String::from_utf8_lossy(&free.stderr));
    let follow_text = String::from_utf8(follow.stdout).unwrap();
    let free_text = String::from_utf8(free.stdout).unwrap();

    assert!(follow_text.contains("FOLLOW"), "{follow_text}");
    assert!(free_text.contains("FREE"), "{free_text}");
    // Free look opened the Nation screen (`n`), which Follow never does.
    assert!(free_text.contains("NATION"), "{free_text}");
    assert!(!follow_text.contains("NATION"), "{follow_text}");
    // The replay itself -- the turn and the agent strip's own action -- is unaffected: both say
    // the same turn, whichever view is on.
    assert!(follow_text.contains("turn 1") && free_text.contains("turn 1"));
}

/// A run log wrapping one act and a turn in a scripted run (`Script` before, `Wake` after, the
/// shape `fn_agent`'s `run_script` actually writes, docs/design/agent-scripting.md's own
/// "Determinism, replay and watching"), to a fresh temp file.
fn write_scripted_log() -> std::path::PathBuf {
    static COUNTER: AtomicU32 = AtomicU32::new(0);
    let mut h = Host::new(7, Nation::Earth, None, None, None, false).unwrap().with_turn_seconds(5);
    h.press_keys(
        "h <right*3> <enter> <down*2> <enter>",
        Some("designate stockpile 3x3 at (62,40)".into()),
        Some("store by camp".into()),
    )
    .unwrap();
    h.end_turn(1);

    let mut lines = vec![LogLine::Header(h.header())];
    lines.push(LogLine::Script {
        name: "keep-fed".into(),
        version: 3,
        code: String::new(),
        turns: 45,
        wake_on: Vec::new(),
    });
    lines.extend(h.log().lines());
    lines.push(LogLine::Wake {
        reason: "event".into(),
        why: Some("death: Ping died of thirst".into()),
        turn: h.observe().turn,
        memory: serde_json::json!({}),
    });
    let text =
        lines.iter().map(|l| serde_json::to_string(l).unwrap()).collect::<Vec<_>>().join("\n")
            + "\n";

    let n = COUNTER.fetch_add(1, Ordering::Relaxed);
    let path = std::env::temp_dir()
        .join(format!("fn_tui_watch_scripted_binary_test_{}_{n}.jsonl", std::process::id()));
    let mut f = std::fs::File::create(&path).unwrap();
    f.write_all(text.as_bytes()).unwrap();
    path
}

/// #482's acceptance criteria, end to end: the agent strip shows `SCRIPT name vN` while the
/// script's own act is running, and `woke: <why>` after its `Wake` line, until the (nonexistent,
/// here) next act.
#[test]
fn watch_frame_shows_the_running_script_then_what_it_woke_on() {
    let path = write_scripted_log();

    // `--at 1` stops right as the script's own turn finishes, before its `Wake` line: the strip
    // names the running script, not yet why it stopped.
    let running = fn_tui()
        .args(["--watch", path.to_str().unwrap(), "--frame", "--at", "1", "--size", "110x30"])
        .output()
        .unwrap();
    assert!(running.status.success(), "{}", String::from_utf8_lossy(&running.stderr));
    let running_text = String::from_utf8(running.stdout).unwrap();
    assert!(running_text.contains("SCRIPT keep-fed v3"), "{running_text}");
    assert!(!running_text.contains("woke:"), "{running_text}");

    // With no `--at`, the replay reaches the end of the log: the `Wake` line too.
    let woke = fn_tui()
        .args(["--watch", path.to_str().unwrap(), "--frame", "--size", "110x30"])
        .output()
        .unwrap();
    std::fs::remove_file(&path).ok();
    assert!(woke.status.success(), "{}", String::from_utf8_lossy(&woke.stderr));
    let woke_text = String::from_utf8(woke.stdout).unwrap();
    assert!(woke_text.contains("SCRIPT keep-fed v3"), "{woke_text}");
    assert!(woke_text.contains("woke: death: Ping died of thirst"), "{woke_text}");
}

/// #482's acceptance criteria: the message log (`L`) gets an Agent entry for the script's own
/// start and its wake, in the exact words the issue gives.
#[test]
fn watch_frame_message_log_has_an_entry_for_the_script_start_and_its_wake() {
    let path = write_scripted_log();
    let out = fn_tui()
        .args([
            "--watch",
            path.to_str().unwrap(),
            "--frame",
            "--size",
            "110x30",
            "--keys",
            "l", // opens the message log for the free-look spectator (#330), never the agent.
        ])
        .output()
        .unwrap();
    std::fs::remove_file(&path).ok();
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    let text = String::from_utf8(out.stdout).unwrap();
    assert!(text.contains("ran keep-fed v3, 45 turns"), "{text}");
    assert!(text.contains("woke after 1 turn: death: Ping died of thirst"), "{text}");
}

// -------------------------------------------------------------------------------------------
// The agent studio (#499, docs/design/agent-studio.md): the 160x45 layout and the code pane.
// -------------------------------------------------------------------------------------------

fn golden(name: &str, actual: &str) {
    // FN_GOLDEN=skip (cargo xtask ci --fast --no-goldens): every golden comparison is a
    // no-op that passes, so agents can run the gate without caring whether goldens are stale.
    if std::env::var("FN_GOLDEN").ok().as_deref() == Some("skip") {
        return;
    }
    let path = format!("{}/tests/golden/{name}.txt", env!("CARGO_MANIFEST_DIR"));
    if std::env::var_os("FN_BLESS").is_some() {
        std::fs::write(&path, actual).unwrap();
    }
    let expected = std::fs::read_to_string(&path).expect("golden missing; run with FN_BLESS=1");
    assert_eq!(actual, expected, "{name}: the frame changed; rebless with FN_BLESS=1 and review");
}

const KEEP_FED_V1: &str = "// keep-fed v1\nlet camp = observe().cursor;\nlog(\"starting\");\n";
const KEEP_FED_V2: &str = "// keep-fed v2: a wake of my own\nlet camp = observe().cursor;\nlog(\"starting\");\nwake(\"done\");\n";

/// A run log with two revisions of one saved script (`keep-fed`), each run in turn: rev 1 for
/// three turns, then rev 2 for two more -- the shape #499's acceptance criteria wants ("a log
/// generated in the test with two revisions of one script").
fn write_studio_log() -> std::path::PathBuf {
    static COUNTER: AtomicU32 = AtomicU32::new(0);
    let mut h = Host::new(7, Nation::Earth, None, None, None, false).unwrap().with_turn_seconds(5);

    h.end_turn(3);
    let run1_lines = h.log().lines();
    h.end_turn(2);
    let all_lines = h.log().lines();
    let run2_lines = all_lines[run1_lines.len()..].to_vec();

    let mut lines = vec![LogLine::Header(h.header())];
    lines.push(LogLine::Script {
        name: "keep-fed".into(),
        version: 1,
        code: KEEP_FED_V1.into(),
        turns: 3,
        wake_on: Vec::new(),
    });
    lines.extend(run1_lines);
    lines.push(LogLine::Wake {
        reason: "budget".into(),
        why: None,
        turn: 3,
        memory: serde_json::json!({}),
    });
    lines.push(LogLine::Script {
        name: "keep-fed".into(),
        version: 2,
        code: KEEP_FED_V2.into(),
        turns: 2,
        wake_on: Vec::new(),
    });
    lines.extend(run2_lines);
    lines.push(LogLine::Wake {
        reason: "done".into(),
        why: None,
        turn: 5,
        memory: serde_json::json!({}),
    });
    let text =
        lines.iter().map(|l| serde_json::to_string(l).unwrap()).collect::<Vec<_>>().join("\n")
            + "\n";

    let n = COUNTER.fetch_add(1, Ordering::Relaxed);
    let path = std::env::temp_dir()
        .join(format!("fn_tui_watch_studio_binary_test_{}_{n}.jsonl", std::process::id()));
    let mut f = std::fs::File::create(&path).unwrap();
    f.write_all(text.as_bytes()).unwrap();
    path
}

/// At 160x45 and up, `--watch --frame` draws the studio split: the code pane on the left (its
/// own revision header, Rhai highlighting and diff markers), the game column on the right at
/// its own clamped width, and the strip without `SCRIPT`/the action text.
#[test]
fn watch_studio_frame_shows_the_code_pane_beside_the_game() {
    let path = write_studio_log();

    // Before the second revision's run starts: the pane follows rev 1, still running, even
    // though the log already scanned both revisions (`M` is 2 right away).
    let before = fn_tui()
        .args(["--watch", path.to_str().unwrap(), "--frame", "--at", "2", "--size", "160x45"])
        .output()
        .unwrap();
    assert!(before.status.success(), "{}", String::from_utf8_lossy(&before.stderr));
    let before_text = String::from_utf8(before.stdout).unwrap();
    assert!(before_text.contains("keep-fed.rhai"), "{before_text}");
    assert!(before_text.contains("rev 1/2"), "{before_text}");
    assert!(before_text.contains("run 3t"), "{before_text}");
    assert!(before_text.contains("new"), "{before_text}");
    assert!(before_text.contains("let camp"), "{before_text}");
    // The game column's own strip drops SCRIPT and the action text in the studio.
    assert!(!before_text.contains("SCRIPT"), "{before_text}");

    // After the second revision's run (and its wake): the pane follows rev 2 instead, with its
    // own diff count against rev 1, and `woke 2t` once it's finished.
    let after = fn_tui()
        .args(["--watch", path.to_str().unwrap(), "--frame", "--size", "160x45"])
        .output()
        .unwrap();
    std::fs::remove_file(&path).ok();
    assert!(after.status.success(), "{}", String::from_utf8_lossy(&after.stderr));
    let after_text = String::from_utf8(after.stdout).unwrap();
    assert!(after_text.contains("rev 2/2"), "{after_text}");
    assert!(after_text.contains("woke 2t"), "{after_text}");
    assert!(after_text.contains("+2 −1") || after_text.contains("+2 -1"), "{after_text}");
    assert!(after_text.contains("wake(\"done\")"), "{after_text}");

    golden("watch_studio_160x45_rev1", &before_text);
    golden("watch_studio_160x45_rev2", &after_text);
}

/// Below the studio's own threshold, the frame is exactly today's: no code pane, the strip keeps
/// `SCRIPT`.
#[test]
fn below_the_studio_threshold_the_frame_is_unchanged() {
    let path = write_studio_log();
    let out = fn_tui()
        .args(["--watch", path.to_str().unwrap(), "--frame", "--at", "2", "--size", "159x45"])
        .output()
        .unwrap();
    std::fs::remove_file(&path).ok();
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    let text = String::from_utf8(out.stdout).unwrap();
    assert!(!text.contains("keep-fed.rhai"), "{text}");
    assert!(text.contains("SCRIPT keep-fed v1"), "{text}");
}

/// An `--ansi` frame of the studio, to pin the highlight and diff colours
/// (docs/design/agent-studio.md, "Testing").
#[test]
fn watch_studio_ansi_frame_is_golden() {
    let path = write_studio_log();
    let out = fn_tui()
        .args([
            "--watch",
            path.to_str().unwrap(),
            "--frame",
            "--size",
            "160x45",
            "--ansi",
            "--color",
            "truecolor",
        ])
        .output()
        .unwrap();
    std::fs::remove_file(&path).ok();
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    let text = String::from_utf8(out.stdout).unwrap();
    golden("watch_studio_160x45_ansi", &text);
}

/// A script with more lines than a single column holds, at a terminal wide enough for the code
/// pane to split into two columns of at least `two_column_min` each (newspaper order).
#[test]
fn watch_studio_splits_into_two_columns_when_the_script_does_not_fit() {
    static COUNTER: AtomicU32 = AtomicU32::new(0);
    let mut h = Host::new(7, Nation::Earth, None, None, None, false).unwrap().with_turn_seconds(5);
    h.end_turn(1);
    let mut code = String::new();
    for i in 0..60 {
        code.push_str(&format!("let v{i} = {i};\n"));
    }
    let mut lines = vec![LogLine::Header(h.header())];
    lines.push(LogLine::Script {
        name: "long".into(),
        version: 1,
        code: code.clone(),
        turns: 1,
        wake_on: Vec::new(),
    });
    lines.extend(h.log().lines());
    let text =
        lines.iter().map(|l| serde_json::to_string(l).unwrap()).collect::<Vec<_>>().join("\n")
            + "\n";
    let n = COUNTER.fetch_add(1, Ordering::Relaxed);
    let path = std::env::temp_dir()
        .join(format!("fn_tui_watch_studio_two_col_test_{}_{n}.jsonl", std::process::id()));
    std::fs::File::create(&path).unwrap().write_all(text.as_bytes()).unwrap();

    let out = fn_tui()
        .args(["--watch", path.to_str().unwrap(), "--frame", "--size", "250x45"])
        .output()
        .unwrap();
    std::fs::remove_file(&path).ok();
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    let text = String::from_utf8(out.stdout).unwrap();
    // Both halves of the script show somewhere in the pane: line 1 near the top of the left
    // column, line 60 only reachable through the second column.
    assert!(text.contains("let v0 = 0;"), "{text}");
    assert!(text.contains("let v59 = 59;"), "{text}");
    golden("watch_studio_250x45_two_columns", &text);
}

// -------------------------------------------------------------------------------------------
// The agent feed and the acts sparkline (#500, docs/design/agent-studio.md).
// -------------------------------------------------------------------------------------------

fn write_lines(lines: &[LogLine], prefix: &str) -> std::path::PathBuf {
    static COUNTER: AtomicU32 = AtomicU32::new(0);
    let text =
        lines.iter().map(|l| serde_json::to_string(l).unwrap()).collect::<Vec<_>>().join("\n")
            + "\n";
    let n = COUNTER.fetch_add(1, Ordering::Relaxed);
    let path =
        std::env::temp_dir().join(format!("fn_tui_{prefix}_{}_{n}.jsonl", std::process::id()));
    std::fs::File::create(&path).unwrap().write_all(text.as_bytes()).unwrap();
    path
}

fn call(tool: &str, summary: &str) -> LogLine {
    LogLine::Call { tool: tool.to_owned(), summary: summary.to_owned() }
}

/// A run log with real `call` lines (#497) ahead of a `save_script`/`run_script` pair, twice:
/// `keep-fed` saved as v1 then run for 3 turns (waking on budget), then saved as v2 and run
/// again for up to 5 turns, this time waking on an event mid-run -- the shape #500's acceptance
/// criteria wants ("a generated log with `call` lines, mid-run and just after a wake").
fn write_feed_log() -> std::path::PathBuf {
    let mut h = Host::new(7, Nation::Earth, None, None, None, false).unwrap().with_turn_seconds(5);
    h.press_keys(
        "h <right*3> <enter> <down*2> <enter>",
        Some("harvest".into()),
        Some("wood".into()),
    )
    .unwrap();
    h.end_turn(3);
    let run1_lines = h.log().lines();
    h.press_keys("p <right*2> <enter> <down*2> <enter>", Some("build Bed".into()), None).unwrap();
    h.end_turn(1);
    let all_lines = h.log().lines();
    let run2_lines = all_lines[run1_lines.len()..].to_vec();

    let mut lines = vec![LogLine::Header(h.header())];
    lines.push(call("save_script", ""));
    lines.push(LogLine::Script {
        name: "keep-fed".into(),
        version: 1,
        code: KEEP_FED_V1.into(),
        turns: 0,
        wake_on: Vec::new(),
    });
    lines.push(call("run_script", "keep-fed 3t"));
    lines.push(LogLine::Script {
        name: "keep-fed".into(),
        version: 1,
        code: KEEP_FED_V1.into(),
        turns: 3,
        wake_on: Vec::new(),
    });
    lines.extend(run1_lines);
    lines.push(LogLine::Wake {
        reason: "budget".into(),
        why: None,
        turn: 3,
        memory: serde_json::json!({}),
    });
    lines.push(call("save_script", ""));
    lines.push(LogLine::Script {
        name: "keep-fed".into(),
        version: 2,
        code: KEEP_FED_V2.into(),
        turns: 0,
        wake_on: Vec::new(),
    });
    lines.push(call("run_script", "keep-fed 5t"));
    lines.push(LogLine::Script {
        name: "keep-fed".into(),
        version: 2,
        code: KEEP_FED_V2.into(),
        turns: 5,
        wake_on: Vec::new(),
    });
    lines.extend(run2_lines);
    lines.push(LogLine::Wake {
        reason: "event".into(),
        why: Some("death: Ping died of thirst".into()),
        turn: 4,
        memory: serde_json::json!({}),
    });
    write_lines(&lines, "feed")
}

/// Mid the second run, the feed's own box shows the running entry's progress (`turn i/N · k
/// acts`) and its latest act, and the strip ends with the acts sparkline.
#[test]
fn watch_studio_feed_shows_a_running_entry_and_the_sparkline() {
    let path = write_feed_log();
    let out = fn_tui()
        .args(["--watch", path.to_str().unwrap(), "--frame", "--at", "4", "--size", "160x45"])
        .output()
        .unwrap();
    std::fs::remove_file(&path).ok();
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    let text = String::from_utf8(out.stdout).unwrap();
    assert!(text.contains("AGENT"), "{text}");
    assert!(text.contains("calls") && text.contains("runs"), "{text}");
    assert!(text.contains("save_script keep-fed v1"), "{text}");
    assert!(text.contains("run_script keep-fed v1"), "{text}");
    assert!(text.contains("budget"), "{text}");
    assert!(text.contains("run_script keep-fed v2"), "{text}");
    assert!(text.contains("acts "), "{text}");
    golden("watch_studio_feed_160x45_running", &text);
}

/// Right after the second run's `Wake`, its entry reads a single `→ woke: ...` line, not the
/// progress line, and the strip is no longer waiting on anything.
#[test]
fn watch_studio_feed_shows_the_wake_outcome() {
    let path = write_feed_log();
    let out = fn_tui()
        .args(["--watch", path.to_str().unwrap(), "--frame", "--size", "160x45"])
        .output()
        .unwrap();
    std::fs::remove_file(&path).ok();
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    let text = String::from_utf8(out.stdout).unwrap();
    assert!(text.contains("run_script keep-fed v2"), "{text}");
    assert!(text.contains("woke: death: Ping died of thirst"), "{text}");
    golden("watch_studio_feed_160x45_woken", &text);
}

/// A log with no `call` lines at all (`fn_agent play`, which wraps its whole run in one
/// `Script`/`Wake` pair and writes no `call` lines of its own) gets the exact same entry, opened
/// by the structural lines themselves: `run_script play · Nt → woke: ...` (docs/design/
/// agent-studio.md's own example).
#[test]
fn watch_studio_feed_from_a_log_with_no_call_lines_is_synthetic() {
    let mut h = Host::new(7, Nation::Fire, None, None, None, false).unwrap().with_turn_seconds(5);
    h.press_keys("h <right*3> <enter> <down*2> <enter>", Some("harvest".into()), None).unwrap();
    h.end_turn(1);

    let mut lines = vec![LogLine::Header(h.header())];
    lines.push(LogLine::Script {
        name: "play".into(),
        version: 0,
        code: "designate_near(\"harvest\", \"Wild Cabbage\", observe().cursor, 15, true);".into(),
        turns: 1,
        wake_on: Vec::new(),
    });
    lines.extend(h.log().lines());
    lines.push(LogLine::Wake {
        reason: "event".into(),
        why: Some("need: Thirsty first shown".into()),
        turn: 1,
        memory: serde_json::json!({}),
    });
    let path = write_lines(&lines, "feed_play");

    let out = fn_tui()
        .args(["--watch", path.to_str().unwrap(), "--frame", "--size", "160x45"])
        .output()
        .unwrap();
    std::fs::remove_file(&path).ok();
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    let text = String::from_utf8(out.stdout).unwrap();
    assert!(text.contains("run_script play"), "{text}");
    assert!(text.contains("woke: need: Thirsty first shown"), "{text}");
    // Exactly one call and one run here: both read singular (#501's triage comment).
    assert!(text.contains("1 call ") && text.contains("1 run "), "{text}");
    golden("watch_studio_feed_160x45_play_log", &text);
}

// -------------------------------------------------------------------------------------------
// The tab fallback and zoom (#501, docs/design/agent-studio.md "The tab fallback").
// -------------------------------------------------------------------------------------------

/// Below the studio's own threshold (160x45), the frame is three tabs instead of the split: the
/// game tab (today's view, #501's "the default") with no `--keys`, the code pane full screen on
/// `--keys 1`, and the feed full screen on `--keys 3`. Each strip ends with the sparkline and
/// the `1[2]3` tab indicator (#501's acceptance criteria).
#[test]
fn watch_tab_fallback_frames_are_golden() {
    let path = write_feed_log();

    let game = fn_tui()
        .args(["--watch", path.to_str().unwrap(), "--frame", "--at", "4", "--size", "110x34"])
        .output()
        .unwrap();
    assert!(game.status.success(), "{}", String::from_utf8_lossy(&game.stderr));
    let game_text = String::from_utf8(game.stdout).unwrap();
    assert!(game_text.contains("1[2]3"), "{game_text}: the game tab is the default");
    golden("watch_tabs_110x34_game", &game_text);

    let code = fn_tui()
        .args([
            "--watch",
            path.to_str().unwrap(),
            "--frame",
            "--at",
            "4",
            "--size",
            "110x34",
            "--keys",
            "1",
        ])
        .output()
        .unwrap();
    assert!(code.status.success(), "{}", String::from_utf8_lossy(&code.stderr));
    let code_text = String::from_utf8(code.stdout).unwrap();
    assert!(code_text.contains("keep-fed.rhai"), "{code_text}");
    assert!(code_text.contains("[1]23"), "{code_text}");
    golden("watch_tabs_110x34_code", &code_text);

    let feed = fn_tui()
        .args([
            "--watch",
            path.to_str().unwrap(),
            "--frame",
            "--at",
            "4",
            "--size",
            "110x34",
            "--keys",
            "3",
        ])
        .output()
        .unwrap();
    std::fs::remove_file(&path).ok();
    assert!(feed.status.success(), "{}", String::from_utf8_lossy(&feed.stderr));
    let feed_text = String::from_utf8(feed.stdout).unwrap();
    assert!(feed_text.contains("AGENT ── "), "{feed_text}");
    assert!(feed_text.contains("12[3]"), "{feed_text}");
    golden("watch_tabs_110x34_feed", &feed_text);
}

/// At or above the threshold, the same digit zooms its pane full screen instead of switching a
/// tab (#501's acceptance criteria: "there is one model, not two").
#[test]
fn watch_studio_zoomed_frame_is_golden() {
    let path = write_studio_log();
    let out = fn_tui()
        .args([
            "--watch",
            path.to_str().unwrap(),
            "--frame",
            "--at",
            "2",
            "--size",
            "160x45",
            "--keys",
            "1",
        ])
        .output()
        .unwrap();
    std::fs::remove_file(&path).ok();
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    let text = String::from_utf8(out.stdout).unwrap();
    assert!(text.contains("keep-fed.rhai"), "{text}");
    assert!(text.contains("[1]23"), "{text}");
    golden("watch_studio_160x45_zoomed_code", &text);
}
