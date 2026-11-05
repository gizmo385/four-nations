//! `fn_tui --watch LOG` (#319) through the real binary: `--frame --at TURN` needs no terminal,
//! so it's tested the same way `--frame` always is (`tests/binary.rs`), never by launching the
//! interactive loop.

use std::io::Write;
use std::process::{Command, Stdio};

use fn_sim::Nation;
use fn_tui::surface::Host;

fn fn_tui() -> Command {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_fn_tui"));
    cmd.stdin(Stdio::null()).env_remove("NO_COLOR").env_remove("COLORTERM");
    cmd
}

/// A short run log: a harvest designation, two turns, and a look, written as `fn_agent` (#317)
/// will write one -- a header line, then one line an event -- to a fresh temp file.
fn write_log() -> std::path::PathBuf {
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

    let path =
        std::env::temp_dir().join(format!("fn_tui_watch_binary_test_{}.jsonl", std::process::id()));
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
/// (#319's acceptance criteria).
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
}

/// Without a terminal and without `--frame`, `--watch` says so and exits instead of hanging
/// (the same shape the plain path's own "this needs a terminal" takes, `tests/binary.rs`).
#[test]
fn watch_without_a_terminal_or_frame_says_so() {
    let out = fn_tui().args(["--watch", "nonexistent.jsonl"]).output().unwrap();
    assert!(!out.status.success());
    assert!(String::from_utf8_lossy(&out.stderr).contains("needs a terminal"));
}
