//! The game menu (`Esc`, #306): opens only once nothing else is left for `Esc` to close, pauses
//! the clock while it's open, and reaches Save, Load, Key help and Quit without a function key.
//! Golden frames are rewritten with `FN_BLESS=1 cargo test -p fn_tui` (review the diff).

use std::path::PathBuf;

use fn_sim::Nation;
use fn_tui::app::App;
use fn_tui::color::{ColorDepth, Palette};
use fn_tui::frame::{render_app, to_text};
use fn_tui::state::{Modal, Screen};
use fn_tui::{keys, new_session};
use fn_ui::Tool;

const SIZE: (u16, u16) = (120, 40);

/// A fresh empty directory for one test, removed when dropped.
struct TempDir(PathBuf);

impl TempDir {
    fn new(name: &str) -> Self {
        let dir = std::env::temp_dir().join(format!("fn-tui-menu-{}-{name}", std::process::id()));
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

fn text(app: &mut App, size: (u16, u16)) -> String {
    let p = Palette::new(&app.session.defs, app.session.world.nation, ColorDepth::TrueColor, false);
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
// Opening: only once there's nothing else to close
// ---------------------------------------------------------------------------------------------

#[test]
fn esc_with_nothing_to_close_opens_the_menu_and_pauses() {
    let mut a = app(None);
    assert!(!a.session.clock.paused);
    run(&mut a, "<esc>");
    assert_eq!(a.ui.modal, Some(Modal::Menu));
    assert!(a.session.clock.paused, "the game pauses while the menu is open");
}

#[test]
fn esc_still_leaves_a_tool_and_closes_the_bar_before_the_menu() {
    let mut a = app(None);
    // A tool: Esc leaves it, not the menu.
    run(&mut a, "h <esc>");
    assert_eq!((a.ui.tool, a.ui.modal), (Tool::Select, None));

    // The selection bar: the first Esc closes it, not the menu.
    run(&mut a, "<tab> <esc>");
    assert_eq!(a.ui.selected, None);
    assert_eq!(a.ui.modal, None);

    // Only once both are already out of the way does Esc open the menu.
    run(&mut a, "<esc>");
    assert_eq!(a.ui.modal, Some(Modal::Menu));
}

#[test]
fn esc_still_closes_a_screen_or_the_build_modal_before_the_menu() {
    let mut a = app(None);
    run(&mut a, "l <esc>");
    assert_eq!((a.ui.screen, a.ui.modal), (None, None), "the log closed, no menu");

    run(&mut a, "u <esc>");
    assert_eq!((a.ui.modal, a.ui.tool), (None, Tool::Select), "the build modal closed, no menu");
}

#[test]
fn resume_or_esc_closes_the_menu_and_restores_the_pause() {
    let mut a = app(None);
    run(&mut a, "<esc>");
    assert_eq!(a.ui.modal, Some(Modal::Menu));
    run(&mut a, "<esc>");
    assert_eq!(a.ui.modal, None);
    assert!(!a.session.clock.paused, "wasn't paused before, isn't after");

    // Resume (the first row) does the same through Enter.
    run(&mut a, "<space>"); // pause by hand first
    assert!(a.session.clock.paused);
    run(&mut a, "<esc> <enter>");
    assert_eq!(a.ui.modal, None);
    assert!(a.session.clock.paused, "it was paused before the menu opened, so it stays");
}

#[test]
fn the_menu_captures_every_key_until_it_closes() {
    let mut a = app(None);
    let cursor = a.ui.cursor;
    run(&mut a, "<esc> h <right> <space>");
    assert_eq!(a.ui.modal, Some(Modal::Menu));
    assert_eq!(a.ui.cursor, cursor, "cursor keys don't reach the map while it's open");
    assert_eq!(a.ui.tool, Tool::Select, "h doesn't switch tools either");
}

// ---------------------------------------------------------------------------------------------
// The rows
// ---------------------------------------------------------------------------------------------

#[test]
fn arrows_and_w_s_move_the_highlight_and_stop_at_the_edges() {
    let mut a = app(None);
    run(&mut a, "<esc>");
    assert_eq!(a.ui.menu_cursor, 0);
    run(&mut a, "<down>");
    assert_eq!(a.ui.menu_cursor, 1);
    run(&mut a, "s s s");
    assert_eq!(a.ui.menu_cursor, 4, "stops at the bottom (5 rows)");
    run(&mut a, "<down>");
    assert_eq!(a.ui.menu_cursor, 4);
    run(&mut a, "<up> w w w w");
    assert_eq!(a.ui.menu_cursor, 0, "stops at the top");
}

#[test]
fn save_writes_the_quicksave_slot_and_says_where() {
    let tmp = TempDir::new("save");
    let mut a = app(Some(tmp.0.clone()));
    run(&mut a, "<tick 40> <esc> <down> <enter>");
    let path = fn_ui::saves::slot_path(&tmp.0, fn_ui::saves::QUICKSAVE);
    assert_eq!(a.ui.status, format!("Quicksaved to {} (tick 40)", path.display()));
    assert!(path.is_file());
    assert_eq!(a.ui.modal, None, "Save closes the menu back to the game");
    assert!(!a.session.clock.paused, "and restores the pause from before it opened");
}

#[test]
fn save_without_a_save_directory_says_so_in_the_notice() {
    let mut a = app(None);
    run(&mut a, "<esc> <down> <enter>");
    assert!(a.ui.status.starts_with("Save failed: this platform has no data directory"));
}

#[test]
fn load_opens_the_saves_screen_which_can_also_write_a_named_slot() {
    let tmp = TempDir::new("load");
    let mut a = app(Some(tmp.0.clone()));
    run(&mut a, "<esc> <down*2> <enter>");
    assert_eq!(a.ui.screen, Some(Screen::Saves));
    assert_eq!(a.ui.modal, None, "Load closes the menu, opening the screen instead");

    // The saves screen (#276) is where a named-slot save lives.
    run(&mut a, "n");
    let path = fn_ui::saves::slot_path(&tmp.0, "save-1");
    assert_eq!(a.ui.status, format!("Saved to {} (tick 0)", path.display()));
}

#[test]
fn help_opens_the_key_help_screen() {
    let mut a = app(None);
    run(&mut a, "<esc> <down*3> <enter>");
    assert_eq!(a.ui.screen, Some(Screen::Help));
    assert_eq!(a.ui.modal, None);
}

#[test]
fn quit_asks_the_same_question_q_does_and_the_menu_stays_open_behind_it() {
    let mut a = app(None);
    run(&mut a, "<esc> <down*4> <enter>");
    assert_eq!(a.ui.modal, Some(Modal::Menu), "the menu stays open behind the prompt");
    let status = text(&mut a, SIZE).lines().last().unwrap().to_owned();
    assert!(status.starts_with(" QUIT? "), "{status}");

    // Declining leaves the menu where it was.
    run(&mut a, "n");
    assert_eq!(a.ui.modal, Some(Modal::Menu));
    assert!(!a.ui.quit);

    // Confirming answers as Q itself would (`App::ui.quit`, which `Shell` turns into its own
    // back-to-title/exit prompt, #280).
    run(&mut a, "<down*4> <enter> y");
    assert!(a.ui.quit);
}

#[test]
fn golden_frame_of_the_game_menu() {
    let mut a = App::new(new_session(7, Nation::Earth), (80, 24));
    run(&mut a, "<esc>");
    assert_eq!(a.ui.modal, Some(Modal::Menu));
    golden("earth_seed7_menu", &text(&mut a, (80, 24)));
}
