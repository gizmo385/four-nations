//! The game menu (`Esc`, #306, #655): opens only once nothing else is left for `Esc` to close,
//! pauses the clock while it's open, and holds Resume, Save…, Load…, Key help, New map…, Switch
//! nation… and Quit…. The last three are asked in a centred box (`fn_tui::ask`) and have no key
//! of their own. The one reference golden lives in `reference_goldens.rs` now (#455).

use std::path::PathBuf;

use fn_sim::Nation;
use fn_tui::app::App;
use fn_tui::color::{ColorDepth, Palette};
use fn_tui::frame::{render_app, to_text};
use fn_tui::state::{Modal, Prompt, Screen};
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
    app.saves = dir.map(fn_ui::saves::DiskStore::shared);
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

/// #654: the menu's footer carries the game's one clock, the same words as the side panel's: the
/// season, the year and the time left with seasons on, the day and the time left with them off.
/// Never minutes since the start.
#[test]
fn the_footer_carries_the_one_clock() {
    let footer_line = |frame: &str| {
        // The menu's own bottom edge: the first row with a bottom-left corner (the side panel's
        // is the last row of the screen).
        frame
            .lines()
            .find(|l| l.contains('┗'))
            .unwrap_or_else(|| panic!("no menu footer in\n{frame}"))
            .to_owned()
    };

    let mut a = app(None);
    run(&mut a, "<tick 600> <esc>");
    let frame = text(&mut a, SIZE);
    assert!(footer_line(&frame).contains("Day 1 · 4:00 left"), "{}", footer_line(&frame));

    let mut b = app(None);
    b.session.world.set_difficulty(Some("normal".to_owned()));
    run(&mut b, "<tick 600> <esc>");
    let frame = text(&mut b, SIZE);
    assert!(
        footer_line(&frame).contains("Spring Y1 · 14:00 left"),
        "seasons on: {}",
        footer_line(&frame)
    );
    assert!(frame.contains("Spring Y1 · 14:00 left"), "the panel says the same:\n{frame}");
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

/// The menu's rows in order: Resume 0, Save… 1, Load… 2, Key help 3, New map… 4,
/// Switch nation… 5, Quit… 6.
#[test]
fn the_menu_lists_the_rows_the_issue_names_with_the_game_enders_set_apart() {
    let mut a = app(None);
    run(&mut a, "<esc>");
    let frame = text(&mut a, SIZE);
    let mut last = 0;
    for row in ["Resume", "Save…", "Load…", "Key help", "New map…", "Switch nation…", "Quit…"]
    {
        let at = frame.find(row).unwrap_or_else(|| panic!("no {row} row in\n{frame}"));
        assert!(at > last, "{row} is out of order");
        last = at;
    }
    for gone in ["Save as", "Quicksave", "Quickload"] {
        assert!(!frame.contains(gone), "{gone} left the menu");
    }
    let line = |needle: &str| frame.lines().position(|l| l.contains(needle)).unwrap();
    assert_eq!(line("New map…") - line("Key help"), 2, "a blank line sets the three apart");
    assert_eq!(line("Save…") - line("Resume"), 1);
}

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
fn f5_still_quicksaves_from_the_map_and_says_where() {
    let tmp = TempDir::new("quicksave");
    let mut a = app(Some(tmp.0.clone()));
    run(&mut a, "<tick 40> <f5>");
    let path = fn_ui::saves::slot_path(&tmp.0, fn_ui::saves::QUICKSAVE);
    assert_eq!(a.ui.status, format!("Quicksaved to {} (tick 40)", path.display()));
    assert!(path.is_file());
}

#[test]
fn save_and_load_both_open_the_saves_screen_which_writes_a_named_slot() {
    for rows in ["<down>", "<down*2>"] {
        let tmp = TempDir::new("saves_screen");
        let mut a = app(Some(tmp.0.clone()));
        run(&mut a, &format!("<esc> {rows} <enter>"));
        assert_eq!(a.ui.screen, Some(Screen::Saves), "{rows}");
        assert_eq!(a.ui.modal, None, "the menu closes, opening the screen instead");

        // The saves screen (#276) is where a named-slot save lives (#704).
        run(&mut a, "n m a p <enter>");
        let path = fn_ui::saves::slot_path(&tmp.0, "map");
        assert_eq!(a.ui.status, format!("Saved to {} (tick 0)", path.display()));
    }
}

#[test]
fn save_without_a_save_directory_says_so() {
    let mut a = app(None);
    run(&mut a, "<esc> <down> <enter>");
    assert!(a.ui.status.contains("data directory"), "{}", a.ui.status);
}

#[test]
fn help_opens_the_key_help_screen() {
    let mut a = app(None);
    run(&mut a, "<esc> <down*3> <enter>");
    assert_eq!(a.ui.screen, Some(Screen::Help));
    assert_eq!(a.ui.modal, None);
}

// ---------------------------------------------------------------------------------------------
// The freed keys (#655): 1-4 and Q do nothing on the map and in a tool. R no longer starts a new
// map either; in Select mode it switches a Workshop's recipe (#712), and does nothing in a tool.
// ---------------------------------------------------------------------------------------------

#[test]
fn r_1_to_4_and_q_do_nothing_on_the_map_or_in_a_tool() {
    for prelude in ["", "h", "x", "b", "u <esc>"] {
        let mut a = app(None);
        run(&mut a, prelude);
        let before = (a.ui.clone(), text(&mut a, SIZE));
        run(&mut a, "1 2 3 4 q Q");
        assert_eq!((a.ui.clone(), text(&mut a, SIZE)), before, "after {prelude:?}");
        assert_eq!(a.ui.prompt, None, "no prompt opens");
        assert!(!a.ui.quit && !a.reopen_embark);
        assert_eq!(a.session.world.nation, Nation::Earth);
        // `R` opens no prompt and leaves the game alone; off a Workshop it only says so.
        run(&mut a, "r R");
        assert_eq!(a.ui.prompt, None, "no prompt opens");
        assert!(!a.ui.quit && !a.reopen_embark);
        assert_eq!(a.session.world.nation, Nation::Earth);
        if a.ui.tool == Tool::Select {
            assert_eq!(a.ui.notice, "Can't switch recipe: nothing to switch here");
        } else {
            assert_eq!((a.ui.clone(), text(&mut a, SIZE)), before, "after {prelude:?}");
        }
    }
}

// ---------------------------------------------------------------------------------------------
// New map…, Switch nation… and Quit… ask in a centred box
// ---------------------------------------------------------------------------------------------

#[test]
fn new_map_asks_in_a_centred_box_with_the_cursor_on_the_safe_answer() {
    let mut a = app(None);
    run(&mut a, "<esc> <down*4> <enter>");
    assert_eq!(a.ui.prompt, Some(Prompt::NewMap));
    assert_eq!(a.ui.ask_cursor, 0);
    let frame = text(&mut a, SIZE);
    assert!(frame.contains("NEW MAP"), "{frame}");
    assert!(frame.contains("Start a new map (seed 8)?"), "{frame}");
    assert!(frame.contains("This game has not been saved."), "{frame}");
    assert!(frame.contains("▸ Keep playing"), "{frame}");
    assert!(frame.contains("Start over without saving"), "{frame}");
    assert!(!frame.contains("Press y"), "the hint bar no longer asks");
    // The menu is hidden behind it, and the hint bar shows the box's keys.
    assert!(!frame.contains("Switch nation…"), "{frame}");
    let bar = frame.lines().last().unwrap();
    assert!(bar.contains("CONFIRM") && bar.contains("Esc cancel"), "{bar}");
}

#[test]
fn a_bare_y_does_nothing_and_esc_picks_the_safe_answer() {
    let mut a = app(None);
    run(&mut a, "<esc> <down*4> <enter> y Y n");
    assert_eq!(a.ui.prompt, Some(Prompt::NewMap), "still asking");
    assert!(!a.reopen_embark);

    // Esc keeps playing: the box and the menu close and the clock runs again.
    run(&mut a, "<esc>");
    assert_eq!(a.ui.prompt, None);
    assert_eq!(a.ui.modal, None);
    assert!(!a.reopen_embark);
    assert!(!a.session.clock.paused);
}

#[test]
fn the_answers_are_rows_the_cursor_moves_over() {
    let mut a = app(None);
    run(&mut a, "<esc> <down*4> <enter>");
    run(&mut a, "<down>");
    assert_eq!(a.ui.ask_cursor, 1);
    run(&mut a, "s s s");
    assert_eq!(a.ui.ask_cursor, 1, "two rows with nowhere to save to: stops at the last");
    run(&mut a, "<up> w");
    assert_eq!(a.ui.ask_cursor, 0);
    // Without a save directory there is no "Save, then" row.
    assert!(!text(&mut a, SIZE).contains("Save, then"));
}

#[test]
fn the_last_row_starts_over_without_saving() {
    let mut a = app(None);
    run(&mut a, "<esc> <down*4> <enter> <down> <enter>");
    assert!(a.reopen_embark, "the shell reopens embark on the next seed");
}

#[test]
fn switch_nation_lists_the_nations_then_asks() {
    let mut a = app(None);
    run(&mut a, "<esc> <down*5> <enter>");
    assert_eq!(a.ui.prompt, None);
    let frame = text(&mut a, SIZE);
    assert!(frame.contains("SWITCH NATION"), "{frame}");
    for name in ["Air Nomads", "Water Tribes", "Earth Kingdom", "Fire Nation"] {
        assert!(frame.contains(name), "no {name} in\n{frame}");
    }
    assert!(frame.contains("playing"), "the current nation is marked");
    let bar = frame.lines().last().unwrap();
    assert!(bar.contains("SWITCH") && bar.contains("Esc back"), "{bar}");

    // Esc goes back to the rows, on Switch nation….
    run(&mut a, "<esc>");
    assert_eq!(a.ui.modal, Some(Modal::Menu));
    assert_eq!(a.ui.menu_cursor, 5);

    // Choosing the second nation asks, naming it.
    run(&mut a, "<enter> <down> <enter>");
    assert_eq!(a.ui.prompt, Some(Prompt::Nation(Nation::Water)));
    let frame = text(&mut a, SIZE);
    assert!(frame.contains("Start over as the Water Tribes?"), "{frame}");

    // Going on restarts the game as that nation.
    run(&mut a, "<down> <enter>");
    assert_eq!(a.session.world.nation, Nation::Water);
    assert_eq!((a.ui.prompt.clone(), a.ui.modal), (None, None));
}

#[test]
fn quit_asks_in_a_box_and_declining_resumes_the_game() {
    let mut a = app(None);
    run(&mut a, "<esc> <down*6> <enter>");
    assert_eq!(a.ui.prompt, Some(Prompt::Quit));
    let frame = text(&mut a, SIZE);
    assert!(frame.contains("Leave this game?"), "{frame}");

    run(&mut a, "y q <enter>");
    assert!(!a.ui.quit, "y and q do nothing, and Enter on Keep playing keeps playing");
    assert_eq!((a.ui.prompt.clone(), a.ui.modal), (None, None));

    // The box asks where to go too (#703): the title screen is `ui.to_title`, and only its last
    // answer is `ui.quit`. `Shell` acts on both without asking again.
    run(&mut a, "<esc> <down*6> <enter> <down> <enter>");
    assert!(a.ui.to_title && !a.ui.quit);
    let mut b = app(None);
    run(&mut b, "<esc> <down*6> <enter> <down*2> <enter>");
    assert!(b.ui.quit && !b.ui.to_title);
}

#[test]
fn ctrl_c_still_quits_at_once_even_with_a_question_open() {
    let mut a = app(None);
    run(&mut a, "<esc> <down*6> <enter> <c-c>");
    assert!(a.ui.quit);
}

// ---------------------------------------------------------------------------------------------
// "Save, then …" and how long ago the game was saved
// ---------------------------------------------------------------------------------------------

#[test]
fn the_box_says_how_long_ago_the_game_was_saved_in_game_time() {
    let tmp = TempDir::new("ago");
    let mut a = app(Some(tmp.0.clone()));
    run(&mut a, "<tick 10> <f5> <tick 7200> <esc> <down*6> <enter>");
    let frame = text(&mut a, SIZE);
    assert!(frame.contains("This game was last saved 12 minutes ago."), "{frame}");
    assert!(frame.contains("Anything since then is lost."), "{frame}");
}

#[test]
fn save_then_quit_writes_a_new_slot_and_only_then_quits() {
    let tmp = TempDir::new("save_then");
    let mut a = app(Some(tmp.0.clone()));
    run(&mut a, "<tick 30> <esc> <down*6> <enter>");
    let frame = text(&mut a, SIZE);
    assert!(frame.contains("Save, then go to the title screen"), "{frame}");
    assert!(frame.contains("Go to the title screen without saving"), "{frame}");
    assert!(fn_ui::saves::list(&fn_ui::saves::DiskStore::new(&tmp.0)).unwrap().is_empty());

    run(&mut a, "<down> <enter>");
    let slots = fn_ui::saves::list(&fn_ui::saves::DiskStore::new(&tmp.0)).unwrap();
    assert_eq!(slots.len(), 1, "one new slot");
    assert_eq!(slots[0].name, "save-1");
    assert!(a.ui.to_title && !a.ui.quit);
}

#[test]
fn a_failed_save_keeps_the_box_open_and_does_not_go_on() {
    let tmp = TempDir::new("save_then_fails");
    // A file where the directory should be: every write fails.
    let blocked = tmp.0.join("not-a-dir");
    std::fs::write(&blocked, "x").unwrap();
    let mut a = app(Some(blocked));
    run(&mut a, "<esc> <down*6> <enter> <down> <enter>");
    assert!(!a.ui.quit && !a.ui.to_title);
    assert_eq!(a.ui.prompt, Some(Prompt::Quit), "still asking");
    assert!(a.ui.status.contains("failed"), "{}", a.ui.status);
}

// ---------------------------------------------------------------------------------------------
// Quickload (F9) and the saves screen ask in the same box
// ---------------------------------------------------------------------------------------------

#[test]
fn f9_asks_in_the_box_then_loads() {
    let tmp = TempDir::new("quickload");
    let mut a = app(Some(tmp.0.clone()));
    run(&mut a, "<tick 40> <f5> <tick 20> <f9>");
    assert_eq!(a.ui.prompt, Some(Prompt::QuickLoad));
    let frame = text(&mut a, SIZE);
    assert!(frame.contains("Load the quicksave in place of this game?"), "{frame}");
    assert!(!frame.contains("Save, then"), "loading offers no save-first row: {frame}");
    run(&mut a, "y");
    assert_eq!(a.session.world.tick, 60, "y alone loads nothing");
    run(&mut a, "<down> <enter>");
    assert_eq!(a.session.world.tick, 40, "the quicksave replaced the game");
}

#[test]
fn saving_over_a_slot_asks_in_the_box_too() {
    let tmp = TempDir::new("save_over");
    let mut a = app(Some(tmp.0.clone()));
    run(&mut a, "<f5> <f6> S");
    assert_eq!(a.ui.prompt, Some(Prompt::SaveOver("quicksave".to_owned())));
    let frame = text(&mut a, SIZE);
    assert!(frame.contains("Save over the quicksave slot?"), "{frame}");
    run(&mut a, "<esc>");
    assert_eq!((a.ui.prompt.clone(), a.ui.screen), (None, Some(Screen::Saves)));
}
