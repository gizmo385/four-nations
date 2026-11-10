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

    // The strip's own sparkline and the game column's Settlement glance ride beside the pane.
    assert!(before_text.contains("AGENT earth  FOLLOW  turn 2"), "{before_text}");
    assert!(before_text.contains("◆ EARTH KINGDOM ◆"), "{before_text}");
    assert!(after_text.contains("AGENT earth  FOLLOW  turn 5"), "{after_text}");
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
fn watch_studio_ansi_frame_pins_the_highlight_and_diff_colours() {
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
    // Rev 2 of `keep-fed`, in truecolor: the Rhai classes and the diff gutter, cell by cell
    // (data/tui.toml's own `code` and `diff` colours).
    for (what, want) in [
        ("a keyword", "\x1b[0;38;2;198;120;221;49mlet"),
        ("a call", "\x1b[0;38;2;97;175;239;49mobserve"),
        ("a string", "\x1b[0;38;2;152;195;121;49m\"starting\""),
        ("an added line's gutter bar", "\x1b[0;38;2;120;200;110;49m▌"),
        (
            "an added line's comment, on the added-line background",
            "\x1b[0;38;2;110;118;129;48;2;24;40;30m// keep-fed v2",
        ),
        ("a line number", "\x1b[0;38;2;92;99;112;49m1"),
    ] {
        assert!(text.contains(want), "{what}: {want:?} in\n{text}");
    }
}

/// A script with more lines than the pane holds, at a wide terminal: the code stays one column
/// no wider than `code_max_width`, and the game column gets the rest of the width.
#[test]
fn watch_studio_keeps_code_to_one_column_and_widens_the_game() {
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
        .join(format!("fn_tui_watch_studio_one_col_test_{}_{n}.jsonl", std::process::id()));
    std::fs::File::create(&path).unwrap().write_all(text.as_bytes()).unwrap();

    let out = fn_tui()
        .args(["--watch", path.to_str().unwrap(), "--frame", "--size", "250x45"])
        .output()
        .unwrap();
    std::fs::remove_file(&path).ok();
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    let text = String::from_utf8(out.stdout).unwrap();
    // One column: line 1 near the top, and the lines past the pane's height scrolled off rather
    // than shown in a second column beside it.
    assert!(text.contains("let v0 = 0;"), "{text}");
    assert!(!text.contains("let v59 = 59;"), "{text}");
    let first = text.lines().find(|l| l.contains("1▌ let v0 = 0;")).expect("line 1");
    assert_eq!(first.matches("let v").count(), 1, "one column only:\n{first}");
    // The code pane's box is `code_max_width` (88) wide: its top-right corner is the 88th cell.
    let top: Vec<char> = text.lines().next().unwrap().chars().collect();
    assert_eq!(top[87], '┐', "the code pane ends at 88 columns:\n{}", text.lines().next().unwrap());
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
    assert!(text.contains("run_script keep-fed v1 · 3t → budget, 1 acts"), "{text}");
    assert!(text.contains("run_script keep-fed v2 · 5t   turn 1/5 · 1 acts"), "{text}");
    assert!(text.contains("acts ▃▁▁▃▁ 1"), "the strip's sparkline and its peak:\n{text}");
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
    assert!(text.contains("run_script keep-fed v1 · 3t → budget, 1 acts"), "{text}");
    assert!(!text.contains("turn 1/5"), "a woken entry drops its progress:\n{text}");
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
    assert!(text.contains("run_script play · 1t → woke: need: Thirsty first shown"), "{text}");
    assert!(text.contains("AGENT fire  FOLLOW  turn 1"), "{text}");
}

// -------------------------------------------------------------------------------------------
// The tab fallback and zoom (#501, docs/design/agent-studio.md "The tab fallback").
// -------------------------------------------------------------------------------------------

/// Below the studio's own threshold (160x45), the frame is three tabs instead of the split: the
/// game tab (today's view, #501's "the default") with no `--keys`, the code pane full screen on
/// `--keys 1`, and the feed full screen on `--keys 3`. Each strip ends with the sparkline and
/// the `1[2]3` tab indicator (#501's acceptance criteria).
#[test]
fn watch_tab_fallback_frames_show_one_pane_each() {
    let path = write_feed_log();

    let game = fn_tui()
        .args(["--watch", path.to_str().unwrap(), "--frame", "--at", "4", "--size", "110x34"])
        .output()
        .unwrap();
    assert!(game.status.success(), "{}", String::from_utf8_lossy(&game.stderr));
    let game_text = String::from_utf8(game.stdout).unwrap();
    assert!(game_text.contains("1[2]3"), "{game_text}: the game tab is the default");
    assert!(game_text.contains("◆ EARTH KINGDOM ◆"), "the game tab draws the map:\n{game_text}");
    assert!(!game_text.contains("keep-fed.rhai"), "no code pane in the game tab:\n{game_text}");
    assert!(game_text.contains("SCRIPT keep-fed v2"), "{game_text}");

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
    assert!(code_text.contains("rev 2/2 ── run 5t ── +2 −1"), "{code_text}");
    assert!(code_text.contains("1▌ // keep-fed v2: a wake of my own"), "{code_text}");
    assert!(!code_text.contains("◆ EARTH KINGDOM ◆"), "the code tab is full screen:\n{code_text}");

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
    assert!(feed_text.contains("4 calls · 2 runs"), "{feed_text}");
    assert!(feed_text.contains("save_script keep-fed v2  +2 −1"), "{feed_text}");
    assert!(feed_text.contains("run_script keep-fed v2 · 5t   turn 1/5 · 1 acts"), "{feed_text}");
    assert!(!feed_text.contains("◆ EARTH KINGDOM ◆"), "the feed tab is full screen:\n{feed_text}");
}

/// At or above the threshold, the same digit zooms its pane full screen instead of switching a
/// tab (#501's acceptance criteria: "there is one model, not two").
#[test]
fn watch_studio_zoomed_frame_is_the_pane_full_screen() {
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
    assert!(text.contains("rev 1/2 ── run 3t ── new"), "{text}");
    assert!(text.contains("1▌ // keep-fed v1"), "{text}");
    assert!(!text.contains("◆ EARTH KINGDOM ◆"), "the zoomed pane takes the whole screen:\n{text}");
}

/// `--record` (#503) through the real binary: no terminal (stdin and stdout are not ttys here),
/// the same log gives a byte-identical file whatever the environment says about colour, the
/// header's size is `--size`'s, and the studio's code pane is in the frames.
#[test]
fn record_writes_a_deterministic_asciicast() {
    let path = write_studio_log();
    let dir = std::env::temp_dir().join(format!("fn_tui_record_test_{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let (a, b) = (dir.join("a.cast"), dir.join("b.cast"));
    let record = |out: &std::path::Path, no_color: bool| {
        let mut cmd = fn_tui();
        cmd.args(["--watch", path.to_str().unwrap(), "--record", out.to_str().unwrap()])
            .args(["--size", "160x45", "--pace", "x40", "--fps", "10"]);
        if no_color {
            cmd.env("NO_COLOR", "1");
        }
        let out = cmd.output().unwrap();
        assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
        assert!(out.stdout.is_empty(), "the recorder prints nothing");
    };
    record(&a, false);
    record(&b, true);
    let (a_text, b_text) =
        (std::fs::read_to_string(&a).unwrap(), std::fs::read_to_string(&b).unwrap());
    std::fs::remove_dir_all(&dir).ok();
    std::fs::remove_file(&path).ok();
    assert_eq!(a_text, b_text, "NO_COLOR and the like must not change the file");

    let header: serde_json::Value = serde_json::from_str(a_text.lines().next().unwrap()).unwrap();
    assert_eq!((header["width"].as_u64(), header["height"].as_u64()), (Some(160), Some(45)));
    assert!(a_text.contains("keep-fed.rhai"), "the studio's code pane is recorded");
    assert!(a_text.lines().count() > 3);
}

#[test]
fn record_says_what_is_wrong_with_the_log() {
    let out = fn_tui()
        .args(["--watch", "/nonexistent/run.jsonl", "--record", "/nonexistent/out.cast"])
        .output()
        .unwrap();
    assert!(!out.status.success());
    assert!(String::from_utf8_lossy(&out.stderr).contains("can't read the run log"));
}

// -------------------------------------------------------------------------------------------
// A playtest's remarks (#576): `say` lines in the feed, the header's label, and a log that reads
// the same with or without them.
// -------------------------------------------------------------------------------------------

fn say(text: &str) -> LogLine {
    LogLine::Say { text: text.to_owned() }
}

const PROBLEM: &str = "game problem: the briefing promises wood from a harvest, but the stock \
                       stays at 0 after two turns";

/// The lines of a short playtest: remarks around a harvest and two turns, one carrying a
/// `game problem:` line. With `remarks` false, the same log without them.
fn playtest_lines(remarks: bool) -> Vec<LogLine> {
    let mut h = Host::new(7, Nation::Earth, None, None, None, false)
        .unwrap()
        .with_turn_seconds(5)
        .with_playtest();
    h.press_keys(
        "h <right*3> <enter> <down*2> <enter>",
        Some("harvest".into()),
        Some("wood".into()),
    )
    .unwrap();
    h.end_turn(2);
    let body = h.log().lines();

    let mut lines = vec![LogLine::Header(h.header())];
    if remarks {
        lines.push(say("I expect the harvest to be done within a turn or two."));
    }
    lines.push(call("act", "1 step"));
    lines.extend(body);
    if remarks {
        lines.push(say(&format!("Nothing changed in the wood count.\n{PROBLEM}")));
    }
    lines
}

fn write_playtest_log() -> std::path::PathBuf {
    write_lines(&playtest_lines(true), "playtest")
}

fn frame_of(path: &std::path::Path, extra: &[&str]) -> String {
    let out =
        fn_tui().args(["--watch", path.to_str().unwrap(), "--frame"]).args(extra).output().unwrap();
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    String::from_utf8(out.stdout).unwrap()
}

/// In the studio the feed shows what the model said, labelled as saying and not as thinking,
/// with the `game problem:` line marked; and the strip says it is a playtest.
#[test]
fn watch_studio_feed_shows_remarks_and_the_playtest_label() {
    let path = write_playtest_log();
    let text = frame_of(&path, &["--size", "160x45"]);
    std::fs::remove_file(&path).ok();
    assert!(text.contains("says: I expect the harvest to be done"), "{text}");
    assert!(text.contains("says: Nothing changed in the wood count."), "{text}");
    assert!(text.contains("! game problem: the briefing promises wood"), "{text}");
    assert!(text.contains("PLAYTEST"), "the strip names it:\n{text}");
    assert!(!text.to_lowercase().contains("thinking"), "{text}");
    // Remarks are not calls: there is the one `act` call.
    assert!(text.contains("1 call "), "{text}");
}

/// Replayed to a turn before the later remark, the feed has only what came before it.
#[test]
fn watch_studio_feed_only_shows_remarks_replayed_so_far() {
    let path = write_playtest_log();
    let text = frame_of(&path, &["--size", "160x45", "--at", "1"]);
    std::fs::remove_file(&path).ok();
    assert!(text.contains("says: I expect the harvest"), "{text}");
    assert!(!text.contains("game problem:"), "{text}");
}

/// The feed tab (below the studio's threshold) and `--ascii` carry the remarks and the marker
/// too, wrapped to fit.
#[test]
fn watch_tab_and_ascii_frames_show_remarks() {
    let path = write_playtest_log();
    for size in ["110x34", "80x24", "60x20"] {
        let text = frame_of(&path, &["--size", size, "--keys", "3", "--ascii"]);
        assert!(text.contains("says: I expect"), "{size}:\n{text}");
        assert!(text.contains("! game problem: the"), "{size}:\n{text}");
        let width: usize = size.split('x').next().unwrap().parse().unwrap();
        assert!(text.lines().all(|l| l.chars().count() <= width), "{size}:\n{text}");
        assert!(text.contains("PLAYTEST") || size == "60x20", "{size}:\n{text}");
    }
    std::fs::remove_file(&path).ok();
}

/// An ordinary run's header and strip are untouched: no label, and no `playtest` key in the line.
#[test]
fn a_run_that_is_not_a_playtest_says_nothing_of_it() {
    let path = write_log();
    let text = frame_of(&path, &["--size", "160x45"]);
    let log = std::fs::read_to_string(&path).unwrap();
    std::fs::remove_file(&path).ok();
    assert!(!text.contains("PLAYTEST"), "{text}");
    assert!(!log.contains("playtest"), "{log}");
}

/// A log with and without its remarks gives the same world and the same score: replay and
/// scoring ignore a `say` line.
#[test]
fn remarks_change_neither_the_world_nor_the_score() {
    let with = playtest_lines(true);
    let without = playtest_lines(false);
    assert!(with.len() > without.len());
    let (head_with, body_with) = with.split_first().unwrap();
    let (head_without, body_without) = without.split_first().unwrap();
    let (LogLine::Header(hw), LogLine::Header(ho)) = (head_with, head_without) else {
        panic!("a header first")
    };
    assert!(hw.playtest && ho.playtest);

    let a = Host::resume(hw, body_with).unwrap();
    let b = Host::resume(ho, body_without).unwrap();
    assert_eq!(a.tick(), b.tick());
    assert_eq!(
        serde_json::to_string(&a.observe()).unwrap(),
        serde_json::to_string(&b.observe()).unwrap()
    );
    assert_eq!(a.header(), b.header(), "a resumed host is still a playtest");

    let sa = fn_tui::runs::score_lines(hw, body_with).unwrap();
    let sb = fn_tui::runs::score_lines(ho, body_without).unwrap();
    assert_eq!(sa, sb);

    // And the keys script a replay would press.
    assert_eq!(
        fn_tui::watch::to_keys_script(hw, body_with),
        fn_tui::watch::to_keys_script(ho, body_without)
    );
}

/// The header carries `playtest` only for a playtest, and a header without the key reads as one
/// that isn't (every log written before #576).
#[test]
fn the_header_marks_a_playtest_and_old_headers_still_read() {
    let lines = playtest_lines(false);
    let LogLine::Header(h) = &lines[0] else { panic!("a header first") };
    let text = serde_json::to_string(&lines[0]).unwrap();
    assert!(text.contains(r#""playtest":true"#), "{text}");

    let plain = fn_tui::surface::RunLogHeader { playtest: false, ..h.clone() };
    let text = serde_json::to_string(&LogLine::Header(plain.clone())).unwrap();
    assert!(!text.contains("playtest"), "{text}");
    let back: LogLine = serde_json::from_str(&text).unwrap();
    assert_eq!(back, LogLine::Header(plain));
}

/// What reading a log does with a line kind it has no variant for (#576): the whole read fails and
/// names the line. That is what a build from before `say` does with a playtest's log, so a log
/// with remarks in it needs this build to be watched, scored or resumed; and what this build does
/// with `say` lines is the opposite, which is why they are a variant rather than skipped text.
#[test]
fn an_unknown_line_kind_fails_the_read_and_a_say_line_does_not() {
    let lines = playtest_lines(true);
    let text =
        lines.iter().map(|l| serde_json::to_string(l).unwrap()).collect::<Vec<_>>().join("\n");
    let (header, parsed) = fn_tui::watch::parse(&format!("{text}\n")).unwrap();
    assert!(header.playtest);
    assert_eq!(parsed.iter().filter(|l| matches!(l, LogLine::Say { .. })).count(), 2);

    let future = format!("{text}\n{{\"type\":\"a_kind_from_the_future\",\"x\":1}}\n");
    let err = fn_tui::watch::parse(&future).unwrap_err();
    assert!(err.contains("unknown variant") && err.contains("a_kind_from_the_future"), "{err}");
}
