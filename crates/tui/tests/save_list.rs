//! The saves screen (`F6`, #276): every named slot from `fn_ui::saves`, newest first, through a
//! temp save directory so the suite never touches the real one. Golden frames are rewritten with
//! `FN_BLESS=1 cargo test -p fn_tui` (review the diff).

use std::path::PathBuf;

use fn_sim::Nation;
use fn_tui::app::App;
use fn_tui::color::{ColorDepth, Palette};
use fn_tui::frame::{render_app, to_text};
use fn_tui::state::Screen;
use fn_tui::{keys, new_session};

const SIZE: (u16, u16) = (120, 40);

/// A fresh empty directory for one test, removed when dropped.
struct TempDir(PathBuf);

impl TempDir {
    fn new(name: &str) -> Self {
        let dir =
            std::env::temp_dir().join(format!("fn-tui-savelist-{}-{name}", std::process::id()));
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
    let mut app = App::new(new_session(7, Nation::Earth), SIZE);
    app.saves = dir;
    app
}

fn run(app: &mut App, script: &str) {
    app.play(&keys::parse(script).expect("a valid script"));
}

fn palette(app: &App, ascii: bool) -> Palette {
    Palette::new(&app.session.defs, app.session.world.nation, ColorDepth::TrueColor, ascii)
}

fn text(app: &mut App, size: (u16, u16)) -> String {
    let p = palette(app, false);
    to_text(&render_app(app, &p, size))
}

fn golden(name: &str, actual: &str) {
    let path = format!("{}/tests/golden/{name}.txt", env!("CARGO_MANIFEST_DIR"));
    if std::env::var_os("FN_BLESS").is_some() {
        std::fs::write(&path, actual).unwrap();
    }
    let expected = std::fs::read_to_string(&path).expect("golden missing; run with FN_BLESS=1");
    assert_eq!(actual, expected, "{name}: the frame changed; rebless with FN_BLESS=1 and review");
}

// ---------------------------------------------------------------------------------------------
// Opening and closing
// ---------------------------------------------------------------------------------------------

#[test]
fn f6_opens_the_screen_and_f6_or_esc_close_it_with_the_side_panel_staying() {
    let tmp = TempDir::new("open_close");
    let mut a = app(Some(tmp.0.clone()));
    run(&mut a, "<f6>");
    assert_eq!(a.ui.screen, Some(Screen::Saves));
    let frame = text(&mut a, SIZE);
    assert!(frame.contains("SAVES"), "{frame}");
    assert!(frame.contains("No saves yet"), "{frame}");
    // The side panel is all there, as with every other screen.
    assert!(frame.contains("EARTH KINGDOM"), "{frame}");
    assert!(frame.contains("SETTLEMENT"), "{frame}");

    run(&mut a, "<f6>");
    assert_eq!(a.ui.screen, None, "F6 again closes it");
    run(&mut a, "<f6> <esc>");
    assert_eq!(a.ui.screen, None, "Esc closes it too");
}

#[test]
fn opening_it_closes_whatever_other_screen_was_open() {
    let tmp = TempDir::new("switch");
    let mut a = app(Some(tmp.0.clone()));
    run(&mut a, "t");
    assert_eq!(a.ui.screen, Some(Screen::Research));
    run(&mut a, "<f6>");
    assert_eq!(a.ui.screen, Some(Screen::Saves));
    run(&mut a, "l");
    assert_eq!(a.ui.screen, Some(Screen::Log));
    run(&mut a, "<f6>");
    assert_eq!(a.ui.screen, Some(Screen::Saves));
}

#[test]
fn without_a_save_directory_f6_says_so_and_does_not_open_an_empty_screen() {
    let mut a = app(None);
    run(&mut a, "<f6>");
    assert_eq!(a.ui.screen, None);
    assert!(
        a.ui.status.starts_with("Save failed: this platform has no data directory"),
        "{}",
        a.ui.status
    );
}

#[test]
fn the_map_keys_wait_while_it_is_open() {
    let tmp = TempDir::new("waits");
    let mut a = app(Some(tmp.0.clone()));
    let (cursor, view) = (a.ui.cursor, a.ui.view);
    run(&mut a, "<tick 10> <f6> n h x p b <right*3> <down*2> <tab> f <enter>");
    assert_eq!(a.ui.cursor, cursor, "the map cursor did not move");
    assert_eq!(a.ui.view, view);
    assert_eq!(a.ui.tool, fn_ui::Tool::Select, "no tool was picked");
    assert_eq!(a.ui.selected, None, "Enter asked to load, not selected a unit");
    assert_eq!(a.ui.screen, Some(Screen::Saves));
    run(&mut a, "n"); // the load prompt swallows keys until answered; n keeps the game
    assert_eq!(a.ui.screen, Some(Screen::Saves));
}

#[test]
fn the_key_help_lists_the_saves_key() {
    let row = fn_tui::input::help_rows()
        .into_iter()
        .find(|r| r.what.starts_with("Saves"))
        .expect("a Saves row");
    assert_eq!(row.keys, "F6");
}

// ---------------------------------------------------------------------------------------------
// Moving the highlight
// ---------------------------------------------------------------------------------------------

#[test]
fn up_down_and_w_s_move_the_highlight_and_stop_at_the_edges() {
    let tmp = TempDir::new("move");
    let mut a = app(Some(tmp.0.clone()));
    run(&mut a, "<f6> n n n");
    assert_eq!(a.ui.save_slots.len(), 3);
    assert_eq!(a.ui.save_cursor, 0);
    run(&mut a, "<down>");
    assert_eq!(a.ui.save_cursor, 1);
    run(&mut a, "s");
    assert_eq!(a.ui.save_cursor, 2);
    run(&mut a, "<down>");
    assert_eq!(a.ui.save_cursor, 2, "stops at the bottom");
    run(&mut a, "<up> w");
    assert_eq!(a.ui.save_cursor, 0, "stops at the top");
    run(&mut a, "<up>");
    assert_eq!(a.ui.save_cursor, 0, "and stays there");
}

// ---------------------------------------------------------------------------------------------
// N: a new slot
// ---------------------------------------------------------------------------------------------

#[test]
fn n_writes_a_new_slot_with_no_prompt_and_the_list_grows() {
    let tmp = TempDir::new("new");
    let mut a = app(Some(tmp.0.clone()));
    run(&mut a, "<tick 40> <f6> n");
    let path = fn_ui::saves::slot_path(&tmp.0, "save-1");
    assert_eq!(a.ui.status, format!("Saved to {} (tick 40)", path.display()));
    assert!(path.is_file());
    assert_eq!(a.ui.save_slots.len(), 1);
    assert_eq!(a.ui.prompt, None, "N never asks: nothing is overwritten");

    run(&mut a, "n");
    let path = fn_ui::saves::slot_path(&tmp.0, "save-2");
    assert_eq!(a.ui.status, format!("Saved to {} (tick 40)", path.display()));
    assert_eq!(a.ui.save_slots.len(), 2);
}

#[test]
fn n_without_a_save_directory_says_so() {
    let mut a = app(None);
    a.ui.screen = Some(Screen::Saves); // as if it had opened before the directory went away
    run(&mut a, "n");
    assert!(a.ui.status.starts_with("Save failed: this platform has no data directory"));
}

// ---------------------------------------------------------------------------------------------
// Enter: load, after asking
// ---------------------------------------------------------------------------------------------

#[test]
fn enter_asks_and_loads_the_highlighted_slot_through_app_load() {
    let tmp = TempDir::new("load");
    let mut a = app(Some(tmp.0.clone()));
    run(&mut a, "<tick 40> <f6> n <tick 60>");
    assert_eq!(a.session.world.tick, 100);
    assert_eq!(a.ui.save_slots.len(), 1);

    run(&mut a, "<enter>");
    assert!(text(&mut a, SIZE).contains("LOAD?"), "the prompt shows");
    run(&mut a, "y");
    assert_eq!(a.session.world.tick, 40, "back to the saved tick");
    assert_eq!(a.ui.status, "Loaded save-1 (tick 40)");
    assert_eq!(a.ui.screen, None, "App::load resets the UI, closing the screen too");
}

#[test]
fn anything_but_y_keeps_the_game_on_a_load_prompt() {
    let tmp = TempDir::new("load_no");
    let mut a = app(Some(tmp.0.clone()));
    run(&mut a, "<tick 40> <f6> n <tick 60> <enter> n");
    assert_eq!(a.session.world.tick, 100, "n is not y, so nothing happened");
    assert_eq!(a.ui.screen, Some(Screen::Saves));
}

#[test]
fn a_failed_load_says_why_and_changes_nothing() {
    let tmp = TempDir::new("failed_load");
    std::fs::write(tmp.0.join("junk.ron"), "this is not a save").unwrap();
    let mut a = app(Some(tmp.0.clone()));
    run(&mut a, "<f6>");
    assert_eq!(a.ui.save_slots.len(), 1);
    run(&mut a, "<enter> y");
    assert!(a.ui.status.starts_with("Load failed:"), "{}", a.ui.status);
    assert_eq!(a.session.world.tick, 0);
}

#[test]
fn an_unloadable_slot_shows_its_reason_dimmed() {
    let tmp = TempDir::new("unloadable");
    std::fs::write(tmp.0.join("junk.ron"), "not a save").unwrap();
    let mut a = app(Some(tmp.0.clone()));
    run(&mut a, "<f6>");
    let frame = text(&mut a, SIZE);
    assert!(frame.contains("can't load:"), "{frame}");
}

// ---------------------------------------------------------------------------------------------
// Shift+S: save over, after asking
// ---------------------------------------------------------------------------------------------

#[test]
fn shift_s_asks_and_saves_over_the_highlighted_slot() {
    let tmp = TempDir::new("save_over");
    let mut a = app(Some(tmp.0.clone()));
    run(&mut a, "<tick 40> <f6> n <tick 60> S");
    assert!(text(&mut a, SIZE).contains("SAVE?"), "the prompt shows");
    run(&mut a, "y");
    let path = fn_ui::saves::slot_path(&tmp.0, "save-1");
    assert_eq!(a.ui.status, format!("Saved to {} (tick 100)", path.display()));
    assert_eq!(a.ui.screen, Some(Screen::Saves), "saving over a slot does not close the screen");

    // The slot now holds the later tick.
    run(&mut a, "<esc> <f9> y");
    assert!(a.ui.status.starts_with("Quickload failed"), "no quicksave, only save-1");
}

#[test]
fn anything_but_y_keeps_the_slot_on_a_save_over_prompt() {
    let tmp = TempDir::new("save_over_no");
    let mut a = app(Some(tmp.0.clone()));
    run(&mut a, "<tick 40> <f6> n <tick 60> S n");
    assert_eq!(a.ui.status, "", "n answered the prompt, not a fresh key");
    run(&mut a, "<f6> <f6>"); // re-reads save-1 from disk
    let frame = text(&mut a, SIZE);
    assert!(frame.contains("00:04"), "{frame}"); // tick 40, not 100
}

#[test]
fn shift_s_without_a_save_directory_says_so() {
    let mut a = app(None);
    a.ui.screen = Some(Screen::Saves);
    run(&mut a, "S y");
    assert_eq!(a.ui.prompt, None, "no slot to ask about, so there was nothing to confirm");
}

// ---------------------------------------------------------------------------------------------
// Golden frame
// ---------------------------------------------------------------------------------------------

#[test]
fn golden_saves_screen_with_a_loadable_and_an_unloadable_slot() {
    let tmp = TempDir::new("golden");
    let mut a = app(Some(tmp.0.clone()));
    run(&mut a, "<tick 40> <f6> n");
    std::fs::write(tmp.0.join("junk.ron"), "not a save").unwrap();
    run(&mut a, "<f6> <f6>"); // closes, then re-reads the directory (now with junk.ron too)
    golden("earth_seed7_saves", &text(&mut a, SIZE));
}
