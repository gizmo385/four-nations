//! `F5` and `F9` (#216): the quicksave through the TUI's own path, by key script, in-process and
//! through the binary's `--frame --keys`.

use std::path::PathBuf;
use std::process::{Command, Stdio};

use fn_tui::app::App;
use fn_tui::color::{ColorDepth, Palette};
use fn_tui::frame::{render_app, to_text};
use fn_tui::{keys, new_session};
use fn_ui::Tool;

const SIZE: (u16, u16) = (120, 40);

/// A fresh empty directory for one test, removed when dropped.
struct TempDir(PathBuf);

impl TempDir {
    fn new(name: &str) -> Self {
        let dir = std::env::temp_dir().join(format!("fn-tui-saves-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        TempDir(dir)
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn app(dir: Option<PathBuf>) -> App {
    let mut app = App::new(new_session(7, fn_sim::Nation::Earth), SIZE);
    app.saves = dir;
    app
}

fn play(app: &mut App, script: &str) {
    app.play(&keys::parse(script).unwrap());
}

fn frame(app: &mut App) -> String {
    let p = Palette::new(&app.session.defs, app.session.world.nation, ColorDepth::Never, false);
    to_text(&render_app(app, &p, SIZE))
}

#[test]
fn f5_saves_and_f9_after_a_yes_brings_the_game_back() {
    let tmp = TempDir::new("roundtrip");
    let mut app = app(Some(tmp.0.clone()));
    play(&mut app, "<tick 40> <f5>");
    let path = fn_ui::saves::slot_path(&tmp.0, fn_ui::saves::QUICKSAVE);
    assert_eq!(app.ui.status, format!("Quicksaved to {} (tick 40)", path.display()));
    let saved = app.session.world.save(&app.session.defs).unwrap();
    assert!(tmp.0.join("quicksave.ron").is_file());

    // Play on, pick a tool and open the log; F9 only asks, and Esc keeps the game.
    play(&mut app, "<tick 100> h <f9> <esc>");
    assert_eq!(app.session.world.tick, 140);
    play(&mut app, "<f9>");
    assert!(frame(&mut app).contains("Load the quicksave in place of this game?"), "the box shows");
    play(&mut app, "y");
    assert_eq!(app.session.world.tick, 140, "a bare y does nothing");
    play(&mut app, "<down> <enter>");
    assert_eq!(app.ui.status, "Quickloaded (tick 40)");
    assert_eq!(app.session.world.save(&app.session.defs).unwrap(), saved);
    assert_eq!(app.ui.tool, Tool::Select, "the tool starts over");
    assert!(frame(&mut app).contains("Quickloaded (tick 40)"), "the status line says so");
    play(&mut app, "<right>");
    assert!(app.ui.status.is_empty(), "until the next key");

    // The loaded game goes on as the saved one would have.
    play(&mut app, "<tick 10>");
    assert_eq!(app.session.world.tick, 50);
}

#[test]
fn a_failed_load_says_why_and_keeps_the_game() {
    let tmp = TempDir::new("failed");
    let mut app = app(Some(tmp.0.clone()));
    play(&mut app, "<tick 5> <f9> <down> <enter>");
    assert!(app.ui.status.starts_with("Quickload failed: no quicksave yet"), "{}", app.ui.status);
    assert_eq!(app.session.world.tick, 5);

    std::fs::write(tmp.0.join("quicksave.ron"), "garbage").unwrap();
    play(&mut app, "<f9> <down> <enter>");
    assert!(
        app.ui.status.starts_with("Quickload failed: not a Four Nations save"),
        "{}",
        app.ui.status
    );
    assert_eq!(app.session.world.tick, 5);
}

#[test]
fn without_a_save_directory_both_keys_say_so() {
    let mut app = app(None);
    play(&mut app, "<f5>");
    assert!(app.ui.status.starts_with("Save failed: this platform has no data directory"));
    play(&mut app, "<f9> <down> <enter>");
    assert!(app.ui.status.starts_with("Save failed"));
}

#[test]
fn the_binary_saves_and_loads_from_a_key_script() {
    let tmp = TempDir::new("binary");
    let out = Command::new(env!("CARGO_BIN_EXE_fn_tui"))
        .args(["7", "earth", "--frame", "--keys", "<tick 40> <f5> <tick 600> <f9> <down> <enter>"])
        .env("FN_SAVE_DIR", &tmp.0)
        .env_remove("NO_COLOR")
        .env_remove("COLORTERM")
        .stdin(Stdio::null())
        .output()
        .unwrap();
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    let text = String::from_utf8(out.stdout).unwrap();
    assert!(text.contains("Quickloaded (tick 40)"), "{text}");
    assert!(text.contains("00:04"), "the clock is back at the save's 4 seconds: {text}");
    assert!(tmp.0.join("quicksave.ron").is_file());
}
