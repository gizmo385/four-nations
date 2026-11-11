//! The saves screen (`F6`, #276): every named slot from `fn_ui::saves`, newest first, through a
//! temp save directory so the suite never touches the real one. All `contains`-style checks; the
//! reference goldens are in `reference_goldens.rs` (#455).

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

// ---------------------------------------------------------------------------------------------
// Opening and closing
// ---------------------------------------------------------------------------------------------

#[test]
fn f6_opens_the_screen_and_f6_or_esc_close_it_over_the_side_panel() {
    let tmp = TempDir::new("open_close");
    let mut a = app(Some(tmp.0.clone()));
    run(&mut a, "<f6>");
    assert_eq!(a.ui.screen, Some(Screen::Saves));
    let frame = text(&mut a, SIZE);
    assert!(frame.contains("SAVES"), "{frame}");
    assert!(frame.contains("No saves yet"), "{frame}");
    // A full screen covers the side panel (#652), as every other screen does.
    assert!(!frame.contains("EARTH KINGDOM"), "{frame}");
    assert!(!frame.contains("SETTLEMENT"), "{frame}");

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
    run(&mut a, "<tick 10> <f6> n <enter> h x p b <right*3> <down*2> <tab> f <enter>");
    assert_eq!(a.ui.cursor, cursor, "the map cursor did not move");
    assert_eq!(a.ui.view, view);
    assert_eq!(a.ui.tool, fn_ui::Tool::Select, "no tool was picked");
    assert_eq!(a.ui.selected, None, "Enter asked to load, not selected a unit");
    assert_eq!(a.ui.screen, Some(Screen::Saves));
    run(&mut a, "<esc>"); // the load box swallows keys until answered; Esc keeps the game
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
    run(&mut a, "<f6> n <enter> n <enter> n <enter>");
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
// N: a new slot, under a typed name (#704)
// ---------------------------------------------------------------------------------------------

/// The status line, the screen's last row.
fn bar(a: &mut App) -> String {
    text(a, SIZE).lines().last().unwrap().to_owned()
}

#[test]
fn n_then_enter_with_nothing_typed_writes_the_next_save_n() {
    let tmp = TempDir::new("new");
    let mut a = app(Some(tmp.0.clone()));
    run(&mut a, "<tick 40> <f6> n");
    assert_eq!(a.ui.save_name.as_deref(), Some(""), "N starts a name; nothing is written yet");
    assert!(fn_ui::saves::list(&tmp.0).unwrap().is_empty());
    let frame = text(&mut a, SIZE);
    assert!(frame.contains("Name: _ save-1"), "the suggestion shows with nothing typed:\n{frame}");

    run(&mut a, "<enter>");
    let path = fn_ui::saves::slot_path(&tmp.0, "save-1");
    assert_eq!(a.ui.status, format!("Saved to {} (tick 40)", path.display()));
    assert!(path.is_file());
    assert_eq!(a.ui.save_slots.len(), 1);
    assert_eq!((a.ui.save_name.clone(), a.ui.prompt.clone()), (None, None));

    run(&mut a, "n <enter>");
    let path = fn_ui::saves::slot_path(&tmp.0, "save-2");
    assert_eq!(a.ui.status, format!("Saved to {} (tick 40)", path.display()));
    assert_eq!(a.ui.save_slots.len(), 2);
}

#[test]
fn a_typed_name_names_the_slot() {
    let tmp = TempDir::new("named");
    let mut a = app(Some(tmp.0.clone()));
    run(&mut a, "<tick 40> <f6> n");
    // Every letter types, the hotkeys among them (`s` moves the highlight, `n` is the Nation
    // screen's), and F6 and Space do nothing while a name is open.
    run(&mut a, "B a s e <space> <f6> <backspace> _ 1 n");
    assert_eq!(a.ui.screen, Some(Screen::Saves));
    assert_eq!(a.ui.save_name.as_deref(), Some("Base_1n"));
    let frame = text(&mut a, SIZE);
    assert!(frame.contains("Name: Base_1n_"), "{frame}");
    run(&mut a, "<backspace> <enter>");
    let path = fn_ui::saves::slot_path(&tmp.0, "Base_1");
    assert_eq!(a.ui.status, format!("Saved to {} (tick 40)", path.display()));
    assert!(path.is_file());
    assert_eq!(a.ui.save_slots.iter().map(|s| s.name.as_str()).collect::<Vec<_>>(), ["Base_1"]);
    assert_eq!(a.ui.save_name, None);
    assert_eq!(a.ui.screen, Some(Screen::Saves), "the list stays open");
    assert!(text(&mut a, SIZE).contains("Base_1  Earth Kingdom"));
}

#[test]
fn esc_cancels_the_name_and_writes_nothing() {
    let tmp = TempDir::new("cancel");
    let mut a = app(Some(tmp.0.clone()));
    run(&mut a, "<f6> n a b <esc>");
    assert_eq!(a.ui.save_name, None);
    assert_eq!(a.ui.screen, Some(Screen::Saves), "Esc only closed the name, not the screen");
    assert!(fn_ui::saves::list(&tmp.0).unwrap().is_empty());
    assert!(!text(&mut a, SIZE).contains("Name:"));
    run(&mut a, "<esc>");
    assert_eq!(a.ui.screen, None, "a second Esc closes the screen as before");
}

#[test]
fn an_invalid_name_is_refused_with_the_typing_kept() {
    let tmp = TempDir::new("invalid");
    let mut a = app(Some(tmp.0.clone()));
    run(&mut a, "<f6> n a / b <enter>");
    assert!(a.ui.status.starts_with("Can't name a save \"a/b\""), "{}", a.ui.status);
    assert_eq!(a.ui.save_name.as_deref(), Some("a/b"), "still typing, so it can be fixed");
    assert!(fn_ui::saves::list(&tmp.0).unwrap().is_empty(), "nothing was written");
    let line = bar(&mut a);
    assert!(line.contains("Can't name a save"), "{line}");

    run(&mut a, "<backspace> <backspace> <backspace> o k <enter>");
    assert!(fn_ui::saves::slot_path(&tmp.0, "ok").is_file());

    // Typing stops at the longest name there can be.
    run(&mut a, "n");
    for _ in 0..fn_ui::saves::MAX_NAME + 5 {
        run(&mut a, "x");
    }
    assert_eq!(a.ui.save_name.as_ref().map(|n| n.len()), Some(fn_ui::saves::MAX_NAME));
    // A name that is only dots is no slot either: it never reaches the disk.
    run(&mut a, "<esc> n . . <enter>");
    assert_eq!(a.ui.save_name.as_deref(), Some(".."));
    assert_eq!(fn_ui::saves::list(&tmp.0).unwrap().len(), 1);
}

#[test]
fn a_name_already_taken_asks_before_writing_over_it() {
    let tmp = TempDir::new("taken");
    let mut a = app(Some(tmp.0.clone()));
    run(&mut a, "<tick 40> <f6> n b a s e <enter> <tick 60> n b a s e <enter>");
    assert_eq!(a.ui.prompt, Some(fn_tui::state::Prompt::SaveOver("base".to_owned())));
    assert!(text(&mut a, SIZE).contains("Save over the base slot?"));
    // Declining leaves the old save, with the saves screen still open.
    run(&mut a, "<esc> <f6> <f6>");
    assert!(text(&mut a, SIZE).contains("4:56 left"), "still tick 40");
    // Another case of the same name asks too (a case-blind file system would clobber it).
    run(&mut a, "n B A S E <enter>");
    assert_eq!(a.ui.prompt, Some(fn_tui::state::Prompt::SaveOver("base".to_owned())));
    run(&mut a, "<down> <enter>");
    let path = fn_ui::saves::slot_path(&tmp.0, "base");
    assert_eq!(a.ui.status, format!("Saved to {} (tick 100)", path.display()));
}

#[test]
fn the_hint_bar_lists_the_naming_keys() {
    let tmp = TempDir::new("hints");
    let mut a = app(Some(tmp.0.clone()));
    run(&mut a, "<f6>");
    let line = bar(&mut a);
    assert!(line.contains("N new"), "{line}");
    run(&mut a, "n");
    let line = bar(&mut a);
    assert!(line.contains("NAME"), "{line}");
    for item in fn_ui::keys::hints(fn_ui::keys::Mode::SaveName) {
        assert!(line.contains(&item.text()), "{} in {line}", item.text());
    }
}

#[test]
fn ctrl_c_still_quits_while_naming() {
    let tmp = TempDir::new("quit");
    let mut a = app(Some(tmp.0.clone()));
    run(&mut a, "<f6> n a <c-c>");
    assert!(a.ui.quit);
}

#[test]
fn n_without_a_save_directory_says_so() {
    let mut a = app(None);
    a.ui.screen = Some(Screen::Saves); // as if it had opened before the directory went away
    run(&mut a, "n");
    assert!(a.ui.status.starts_with("Save failed: this platform has no data directory"));
    assert_eq!(a.ui.save_name, None);
}

// ---------------------------------------------------------------------------------------------
// Enter: load, after asking
// ---------------------------------------------------------------------------------------------

#[test]
fn enter_asks_and_loads_the_highlighted_slot_through_app_load() {
    let tmp = TempDir::new("load");
    let mut a = app(Some(tmp.0.clone()));
    run(&mut a, "<tick 40> <f6> n <enter> <tick 60>");
    assert_eq!(a.session.world.tick, 100);
    assert_eq!(a.ui.save_slots.len(), 1);

    run(&mut a, "<enter>");
    assert!(text(&mut a, SIZE).contains("Load save-1 in place of this game?"), "the box shows");
    run(&mut a, "y");
    assert_eq!(a.session.world.tick, 100, "a bare y does nothing");
    run(&mut a, "<down> <enter>");
    assert_eq!(a.session.world.tick, 40, "back to the saved tick");
    assert_eq!(a.ui.status, "Loaded save-1 (tick 40)");
    assert_eq!(a.ui.screen, None, "App::load resets the UI, closing the screen too");
}

#[test]
fn esc_keeps_the_game_on_a_load_box() {
    let tmp = TempDir::new("load_no");
    let mut a = app(Some(tmp.0.clone()));
    run(&mut a, "<tick 40> <f6> n <enter> <tick 60> <enter> <esc>");
    assert_eq!(a.session.world.tick, 100, "Esc is the safe answer, so nothing happened");
    assert_eq!(a.ui.screen, Some(Screen::Saves));
}

#[test]
fn a_failed_load_says_why_and_changes_nothing() {
    let tmp = TempDir::new("failed_load");
    std::fs::write(tmp.0.join("junk.ron"), "this is not a save").unwrap();
    let mut a = app(Some(tmp.0.clone()));
    run(&mut a, "<f6>");
    assert_eq!(a.ui.save_slots.len(), 1);
    run(&mut a, "<enter> <down> <enter>");
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
    run(&mut a, "<tick 40> <f6> n <enter> <tick 60> S");
    assert!(text(&mut a, SIZE).contains("Save over the save-1 slot?"), "the box shows");
    run(&mut a, "y");
    assert_eq!(a.ui.status, "", "a bare y does nothing");
    run(&mut a, "<down> <enter>");
    let path = fn_ui::saves::slot_path(&tmp.0, "save-1");
    assert_eq!(a.ui.status, format!("Saved to {} (tick 100)", path.display()));
    assert_eq!(a.ui.screen, Some(Screen::Saves), "saving over a slot does not close the screen");

    // The slot now holds the later tick.
    run(&mut a, "<esc> <f9> <down> <enter>");
    assert!(a.ui.status.starts_with("Quickload failed"), "no quicksave, only save-1");
}

#[test]
fn esc_keeps_the_slot_on_a_save_over_box() {
    let tmp = TempDir::new("save_over_no");
    let mut a = app(Some(tmp.0.clone()));
    run(&mut a, "<tick 40> <f6> n <enter> <tick 60> S <esc>");
    assert_eq!(a.ui.status, "", "Esc answered the box, not a fresh key");
    run(&mut a, "<f6> <f6>"); // re-reads save-1 from disk
    let frame = text(&mut a, SIZE);
    assert!(frame.contains("4:56 left"), "{frame}"); // tick 40, not 100
}

#[test]
fn shift_s_without_a_save_directory_says_so() {
    let mut a = app(None);
    a.ui.screen = Some(Screen::Saves);
    run(&mut a, "S y");
    assert_eq!(a.ui.prompt, None, "no slot to ask about, so there was nothing to confirm");
}

// ---------------------------------------------------------------------------------------------
// A loadable and an unloadable slot, side by side
// ---------------------------------------------------------------------------------------------

/// The Saves screen lists an unloadable file with the reason it can't load beside a loadable
/// slot with its nation, seed and clock; the unloadable one sorts first and takes the pointer.
#[test]
fn the_saves_screen_lists_a_loadable_and_an_unloadable_slot() {
    let tmp = TempDir::new("slots");
    let mut a = app(Some(tmp.0.clone()));
    run(&mut a, "<tick 40> <f6> n <enter>");
    std::fs::write(tmp.0.join("junk.ron"), "not a save").unwrap();
    run(&mut a, "<f6> <f6>"); // closes, then re-reads the directory (now with junk.ron too)
    let frame = text(&mut a, SIZE);
    assert!(frame.contains("▸ junk  can't load: not a Four Nations save"), "{frame}");
    assert!(
        frame.contains("save-1  Earth Kingdom, seed 7, Day 1 - 4:56 left (tick 40)"),
        "{frame}"
    );
}
