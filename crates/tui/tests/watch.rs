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
