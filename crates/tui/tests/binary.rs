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
    // The seed sits on the panel's bottom edge now (docs/design/side-panel.md "Headings"), not
    // a row of its own.
    assert!(text.contains("EARTH KINGDOM") && text.contains("seed 7"), "{text}");
    assert!(!text.contains('\x1b'), "no escapes without --ansi");
}

/// `--difficulty normal` reaches the plain `SEED NATION` path (#298) and turns on the side
/// panel's Season section (#299, docs/design/difficulty.md "TUI"); with no preset the section
/// stays hidden.
#[test]
fn difficulty_turns_on_the_season_section() {
    let out = fn_tui().args(["7", "earth", "--frame", "--difficulty", "normal"]).output().unwrap();
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    let text = String::from_utf8(out.stdout).unwrap();
    assert!(text.contains("Spring Y1"), "{text}");
    assert!(text.contains("Food 0 · 0 each"), "{text}");
    assert!(text.contains("Winter: mild"), "{text}");

    let out = fn_tui().args(["7", "earth", "--frame"]).output().unwrap();
    let text = String::from_utf8(out.stdout).unwrap();
    assert!(!text.contains("Spring Y1"), "no preset: seasons stay off:\n{text}");
    assert!(!text.contains("Winter:"), "no preset: no winter row either:\n{text}");
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
fn keys_play_a_script_before_the_frame() {
    let frame = |keys: &str| {
        let out = fn_tui().args(["7", "earth", "--frame", "--keys", keys]).output().unwrap();
        assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
        String::from_utf8(out.stdout).unwrap()
    };
    let text = frame("h <left*6> <up*3> <enter> <right*5> <down*4> <enter> <tick 600>");
    assert!(text.lines().last().unwrap().starts_with(" HARVEST "), "{text}");
    assert!(text.contains("done"), "{text}");
    assert!(text.contains("01:00"), "<tick 600> is a minute:\n{text}");
    // WASD scripts move the cursor like the arrows do.
    assert_eq!(frame("d d s"), frame("<right*2> <down>"));
    assert_ne!(frame("d d s"), frame(""));
}

#[test]
fn a_bad_key_script_fails_with_usage_before_anything_runs() {
    let out = fn_tui().args(["7", "earth", "--frame", "--keys", "h <bogus>"]).output().unwrap();
    assert_eq!(out.status.code(), Some(2));
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(err.contains("--keys:") && err.contains("<bogus>"), "{err}");
    assert!(out.stdout.is_empty());
    let out = fn_tui().args(["7", "earth", "--keys", "h"]).output().unwrap();
    assert_eq!(out.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&out.stderr).contains("only means something with --frame"));
}

#[test]
fn interactive_mode_refuses_without_a_tty_and_points_to_frame() {
    let out = fn_tui().args(["7", "earth"]).output().unwrap();
    assert!(!out.status.success());
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(err.contains("--frame"), "{err}");
    assert!(out.stdout.is_empty(), "nothing on stdout, no escape codes for a pipe to choke on");
}

/// With neither `SEED` nor `NATION`, `--frame` renders the title screen (#280) instead of
/// erroring, and a seed and nation together still skip it. `FN_SAVE_DIR` points at an empty
/// directory so a real save on the machine running the test can't add a `Continue` row.
#[test]
fn with_no_seed_frame_renders_the_title_screen_and_both_together_still_skip_it() {
    let dir = std::env::temp_dir().join(format!("fn-tui-title-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();

    let out = fn_tui().args(["--frame"]).env("FN_SAVE_DIR", &dir).output().unwrap();
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    let text = String::from_utf8(out.stdout).unwrap();
    assert!(text.contains("FOUR NATIONS") && text.contains("New game"), "{text}");
    assert!(!text.contains("Continue"), "nothing to continue in an empty save directory");

    let out = fn_tui().args(["7", "earth", "--frame"]).env("FN_SAVE_DIR", &dir).output().unwrap();
    assert!(out.status.success());
    let text = String::from_utf8(out.stdout).unwrap();
    assert!(text.contains("EARTH KINGDOM"), "a seed and a nation together skip the menu:\n{text}");

    let _ = std::fs::remove_dir_all(&dir);
}

/// `--embark` opens embark instead of the game (#290), `--size` picks the map preset, and
/// `--rough` reaches it too; `--start X,Y` founds a game straight away, and a refused site
/// fails with a message instead of silently falling back to anything.
#[test]
fn embark_and_start_from_the_command_line() {
    let out = fn_tui()
        .args(["7", "earth", "--embark", "--frame", "--size", "medium", "--rough", "--keys"])
        .arg("<enter>") // zoom in once, past the overview, so the outlook section shows
        .output()
        .unwrap();
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    let text = String::from_utf8(out.stdout).unwrap();
    assert!(text.contains("CHOOSE A SITE") && text.contains("(rough guess)"), "{text}");
    // Medium is wider than Small; the overview's default site sits further along x.
    assert!(!text.contains("(94, 64)"), "{text}");

    // The map's forced-sea border is always refused.
    let out = fn_tui().args(["7", "earth", "--start", "0,0", "--frame"]).output().unwrap();
    assert!(!out.status.success());
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(err.contains("can't start at (0, 0)") && err.contains("open water"), "{err}");

    // Small's own default site for this seed, valid by construction.
    let out = fn_tui().args(["7", "earth", "--start", "94,64", "--frame"]).output().unwrap();
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    let text = String::from_utf8(out.stdout).unwrap();
    assert!(text.contains("EARTH KINGDOM"), "a founded game, straight away:\n{text}");
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

/// The terminal frontend must stay free of a game engine: it depends on `fn_sim` and `fn_ui`
/// only (docs/design/tui.md).
#[test]
fn depends_on_the_sim_and_shared_ui_only() {
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
    assert!(tree.lines().any(|l| l.starts_with("fn_ui ")), "sanity: the tree lists fn_ui");
}
