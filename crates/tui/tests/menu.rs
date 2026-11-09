//! The game menu (`Esc`, #306, #312): opens only once nothing else is left for `Esc` to close,
//! pauses the clock while it's open, and reaches every save/load action — Save as…, Quicksave,
//! Load…, Quickload — plus Key help and Quit without a function key. The one reference golden
//! lives in `reference_goldens.rs` now (#455).

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

/// #416, owner decision 4 on #414: with seasons on, the side panel's header shows the season
/// instead of the elapsed clock, so the menu carries it in its own footer instead; with seasons
/// off, the footer stays as it was. The side panel's own header keeps the elapsed clock either
/// way with seasons off, so this checks the menu's footer line alone, not the whole frame.
#[test]
fn the_footer_carries_the_elapsed_clock_only_while_seasons_are_on() {
    let footer_line = |frame: &str| {
        frame
            .lines()
            .find(|l| l.contains("Enter select") && l.contains("Esc resume"))
            .unwrap_or_else(|| panic!("no menu footer in\n{frame}"))
            .to_owned()
    };

    let mut a = app(None);
    run(&mut a, "<tick 600> <esc>");
    let frame = text(&mut a, SIZE);
    assert!(!footer_line(&frame).contains("01:00"), "seasons off: {}", footer_line(&frame));

    let mut b = app(None);
    b.session.world.set_difficulty(Some("normal".to_owned()));
    run(&mut b, "<tick 600> <esc>");
    let frame = text(&mut b, SIZE);
    assert!(footer_line(&frame).contains("01:00"), "seasons on: {}", footer_line(&frame));
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
    run(&mut a, "s s s s s");
    assert_eq!(a.ui.menu_cursor, 6, "stops at the bottom (7 rows)");
    run(&mut a, "<down>");
    assert_eq!(a.ui.menu_cursor, 6);
    run(&mut a, "<up> w w w w w w");
    assert_eq!(a.ui.menu_cursor, 0, "stops at the top");
}

#[test]
fn quicksave_writes_the_slot_and_says_where() {
    let tmp = TempDir::new("quicksave");
    let mut a = app(Some(tmp.0.clone()));
    run(&mut a, "<tick 40> <esc> <down*2> <enter>");
    let path = fn_ui::saves::slot_path(&tmp.0, fn_ui::saves::QUICKSAVE);
    assert_eq!(a.ui.status, format!("Quicksaved to {} (tick 40)", path.display()));
    assert!(path.is_file());
    assert_eq!(a.ui.modal, None, "Quicksave closes the menu back to the game");
    assert!(!a.session.clock.paused, "and restores the pause from before it opened");
}

#[test]
fn quicksave_without_a_save_directory_says_so_in_the_notice() {
    let mut a = app(None);
    run(&mut a, "<esc> <down*2> <enter>");
    assert!(a.ui.status.starts_with("Save failed: this platform has no data directory"));
}

#[test]
fn load_opens_the_saves_screen_which_can_also_write_a_named_slot() {
    let tmp = TempDir::new("load");
    let mut a = app(Some(tmp.0.clone()));
    run(&mut a, "<esc> <down*3> <enter>");
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
    run(&mut a, "<esc> <down*5> <enter>");
    assert_eq!(a.ui.screen, Some(Screen::Help));
    assert_eq!(a.ui.modal, None);
}

#[test]
fn quit_asks_the_same_question_q_does_and_the_menu_stays_open_behind_it() {
    let mut a = app(None);
    run(&mut a, "<esc> <down*6> <enter>");
    assert_eq!(a.ui.modal, Some(Modal::Menu), "the menu stays open behind the prompt");
    let status = text(&mut a, SIZE).lines().last().unwrap().to_owned();
    assert!(status.starts_with(" QUIT? "), "{status}");

    // Declining leaves the menu where it was.
    run(&mut a, "n");
    assert_eq!(a.ui.modal, Some(Modal::Menu));
    assert!(!a.ui.quit);

    // Confirming answers as Q itself would (`App::ui.quit`, which `Shell` turns into its own
    // back-to-title/exit prompt, #280).
    run(&mut a, "<down*6> <enter> y");
    assert!(a.ui.quit);
}

// ---------------------------------------------------------------------------------------------
// Save as… (#312): the row list becomes a name prompt in place.
// ---------------------------------------------------------------------------------------------

#[test]
fn save_as_types_a_name_writes_it_and_closes_the_menu() {
    let tmp = TempDir::new("save_as");
    let mut a = app(Some(tmp.0.clone()));
    run(&mut a, "<tick 10> <esc> <down> <enter>");
    assert!(a.ui.menu_name.is_some(), "the row list became the name prompt");
    let status = text(&mut a, SIZE).lines().last().unwrap().to_owned();
    assert!(status.starts_with(" SAVE AS "), "{status}");
    run(&mut a, "m y - s a v e <enter>");
    assert_eq!(a.ui.modal, None, "a successful save as closes the menu back to the game");
    let path = fn_ui::saves::slot_path(&tmp.0, "my-save");
    assert!(path.is_file());
    assert_eq!(a.ui.status, format!("Saved to {} (tick 10)", path.display()));
}

#[test]
fn esc_in_the_name_prompt_goes_back_to_the_menu_without_saving() {
    let tmp = TempDir::new("save_as_esc");
    let mut a = app(Some(tmp.0.clone()));
    run(&mut a, "<esc> <down> <enter> m y <esc>");
    assert_eq!(a.ui.modal, Some(Modal::Menu), "back to the row list, not resumed");
    assert_eq!(a.ui.menu_name, None);
    assert!(fn_ui::saves::list(&tmp.0).unwrap().is_empty());
}

#[test]
fn save_as_with_no_typed_name_falls_back_to_new_name() {
    let tmp = TempDir::new("save_as_blank");
    let mut a = app(Some(tmp.0.clone()));
    run(&mut a, "<esc> <down> <enter> <enter>");
    let path = fn_ui::saves::slot_path(&tmp.0, "save-1");
    assert!(path.is_file(), "an empty name falls back to new_name's save-1");
    assert_eq!(a.ui.modal, None);
}

#[test]
fn save_as_over_an_existing_name_asks_first_like_shift_s() {
    let tmp = TempDir::new("save_as_over");
    let mut a = app(Some(tmp.0.clone()));
    run(&mut a, "<esc> <down*2> <enter>"); // Quicksave writes the `quicksave` slot.
    assert!(fn_ui::saves::slot_path(&tmp.0, fn_ui::saves::QUICKSAVE).is_file());

    run(&mut a, "<esc> <down> <enter> q u i c k s a v e <enter>");
    assert_eq!(a.ui.modal, Some(Modal::Menu), "the menu stays open behind the prompt");
    let status = text(&mut a, SIZE).lines().last().unwrap().to_owned();
    assert!(status.to_lowercase().contains("save"), "{status}");

    // Declining leaves it as it was, cursor still on Save as….
    run(&mut a, "n");
    assert_eq!(a.ui.modal, Some(Modal::Menu));

    // Confirming writes it over and closes the menu, as a plain new name does.
    run(&mut a, "<enter> q u i c k s a v e <enter> y");
    assert_eq!(a.ui.modal, None);
}

#[test]
fn save_as_without_a_save_directory_says_so_and_leaves_the_menu_as_it_was() {
    let mut a = app(None);
    run(&mut a, "<esc> <down> <enter>");
    assert!(a.ui.status.starts_with("Save failed: this platform has no data directory"));
    assert_eq!(a.ui.menu_name, None, "never entered the name prompt");
    assert_eq!(a.ui.modal, Some(Modal::Menu));
}

// ---------------------------------------------------------------------------------------------
// Quickload (#312): the same y/n `F9` asks, greyed out (and said) with nothing to load.
// ---------------------------------------------------------------------------------------------

#[test]
fn quickload_asks_first_like_f9_then_loads_and_closes_the_menu() {
    let tmp = TempDir::new("quickload");
    let mut a = app(Some(tmp.0.clone()));
    run(&mut a, "<tick 40> <esc> <down*2> <enter>"); // Quicksave at tick 40.
    assert_eq!(a.ui.modal, None);

    run(&mut a, "<tick 20> <esc> <down*4> <enter>"); // Quickload row.
    assert_eq!(a.ui.modal, Some(Modal::Menu), "the menu stays open behind the prompt");
    run(&mut a, "y");
    assert_eq!(a.session.world.tick, 40, "the quicksave replaced the game");
    assert_eq!(a.ui.modal, None, "and the menu closed along with it");
}

#[test]
fn quickload_is_greyed_out_and_fails_gracefully_with_no_quicksave_yet() {
    let tmp = TempDir::new("quickload_none");
    let mut a = app(Some(tmp.0.clone()));
    run(&mut a, "<esc>");
    assert!(!a.ui.has_quicksave, "nothing saved yet");
    let frame = text(&mut a, SIZE);
    assert!(frame.contains("Quickload"), "the row stays, dimmed: {frame}");

    run(&mut a, "<down*4> <enter> y");
    assert!(a.ui.status.contains("no quicksave yet"), "{}", a.ui.status);
    assert_eq!(a.ui.modal, None, "the menu still closes after the failed attempt");
}
