//! The binary as a user (or an agent) runs it, and the crate's place in the workspace.

use std::process::{Command, Stdio};

fn fn_tui() -> Command {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_fn_tui"));
    // A test run is never a terminal, whatever the harness's own environment says.
    cmd.stdin(Stdio::null()).env_remove("NO_COLOR").env_remove("COLORTERM");
    cmd
}

#[test]
fn frame_prints_one_frame_and_exits_without_a_tty() {
    let out = fn_tui().args(["7", "earth", "--frame", "--size", "100x30"]).output().unwrap();
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    let text = String::from_utf8(out.stdout).unwrap();
    assert_eq!(text.lines().count(), 30);
    assert!(text.lines().all(|l| l.chars().count() <= 100));
    assert!(text.contains("EARTH KINGDOM") && text.contains("Seed 7"), "{text}");
    assert!(!text.contains('\x1b'), "no escapes without --ansi");
}

#[test]
fn ticks_advance_the_clock_in_the_frame() {
    let frame = |ticks: &str| {
        let out = fn_tui().args(["7", "fire", "--frame", "--ticks", ticks]).output().unwrap();
        assert!(out.status.success());
        String::from_utf8(out.stdout).unwrap()
    };
    assert!(frame("0").contains("00:00"));
    assert!(frame("600").contains("01:00"), "600 ticks is a minute at 10 ticks a second");
}

#[test]
fn ansi_keeps_the_colour_escapes_and_color_never_removes_the_styles() {
    let out = fn_tui()
        .args(["7", "earth", "--frame", "--ansi", "--color", "truecolor"])
        .output()
        .unwrap();
    let text = String::from_utf8(out.stdout).unwrap();
    assert!(text.contains("\x1b[0;") && text.contains("38;2;"), "truecolor escapes");
    let out =
        fn_tui().args(["7", "earth", "--frame", "--ansi", "--color", "never"]).output().unwrap();
    let text = String::from_utf8(out.stdout).unwrap();
    assert!(!text.contains("38;") && !text.contains("48;2"), "no colour escapes");
}

#[test]
fn ascii_frames_are_ascii() {
    let out = fn_tui().args(["7", "water", "--frame", "--ascii"]).output().unwrap();
    let text = String::from_utf8(out.stdout).unwrap();
    assert!(text.is_ascii(), "{text}");
}

#[test]
fn a_small_size_prints_only_the_notice() {
    let out = fn_tui().args(["7", "air", "--frame", "--size", "79x24"]).output().unwrap();
    let text = String::from_utf8(out.stdout).unwrap();
    assert_eq!(text.trim_end(), "Terminal too small (80x24 needed)");
    assert_eq!(text.lines().count(), 24, "the frame is still the size that was asked for");
}

#[test]
fn interactive_mode_refuses_without_a_tty_and_points_to_frame() {
    let out = fn_tui().args(["7", "earth"]).output().unwrap();
    assert!(!out.status.success());
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(err.contains("--frame"), "{err}");
    assert!(out.stdout.is_empty(), "nothing on stdout, no escape codes for a pipe to choke on");
}

#[test]
fn bad_arguments_fail_with_usage() {
    let out = fn_tui().args(["7", "mars"]).output().unwrap();
    assert_eq!(out.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&out.stderr).contains("usage:"));
    let out = fn_tui().arg("--help").output().unwrap();
    assert!(out.status.success());
    assert!(String::from_utf8_lossy(&out.stdout).contains("--frame"));
}

/// The terminal frontend must stay free of the engine: it depends on `fn_sim` and `fn_ui` only
/// (docs/design/tui.md), so `cargo xtask ci sim` can gate it without compiling Bevy.
#[test]
fn bevy_is_not_in_the_dependency_tree() {
    let out = Command::new(env!("CARGO"))
        .args([
            "tree",
            "-p",
            "fn_tui",
            "--prefix",
            "none",
            "--edges",
            "normal,build,dev",
            "--offline",
        ])
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .output()
        .expect("cargo tree runs");
    let tree = String::from_utf8_lossy(&out.stdout);
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    assert!(tree.lines().any(|l| l.starts_with("fn_sim ")), "sanity: the tree lists fn_sim");
    let bevy: Vec<_> = tree.lines().filter(|l| l.starts_with("bevy")).collect();
    assert!(bevy.is_empty(), "fn_tui must not depend on Bevy: {bevy:?}");
    assert!(!tree.contains("fn_client"), "fn_tui must not depend on the Bevy client");
}
