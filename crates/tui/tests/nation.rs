//! The Nation screen (`N`, #308, docs/design/side-panel.md "The Nation screen"): every settler
//! grouped by activity, with need meters or a flag (depending on the screen's width), every
//! store and the nation's traits. The one reference golden lives in `reference_goldens.rs` now
//! (#455); the narrow-drops-to-a-flag and wide-meters-stay-near-the-names behaviour it used to
//! guard are unit-tested already in `crates/tui/src/nation.rs` (`a_narrow_row_flags_trouble_...`,
//! `a_wide_row_carries_a_flagged_meter_...` and `the_task_column_is_capped_on_a_wide_terminal`).

use fn_sim::Nation;
use fn_tui::app::App;
use fn_tui::color::{ColorDepth, Palette};
use fn_tui::frame::{render_app, to_text};
use fn_tui::state::Screen;
use fn_tui::{keys, new_session};
use fn_ui::Tool;

const SIZE: (u16, u16) = (120, 40);

fn app(seed: u64, nation: Nation, size: (u16, u16)) -> App {
    App::new(new_session(seed, nation), size)
}

fn run(app: &mut App, script: &str) {
    app.play(&keys::parse(script).expect("a valid script"));
}

fn text(app: &mut App, size: (u16, u16)) -> String {
    let p = Palette::new(&app.session.defs, app.session.world.nation, ColorDepth::TrueColor, false);
    to_text(&render_app(app, &p, size))
}

#[test]
fn n_opens_the_screen_and_n_or_esc_close_it_over_the_side_panel() {
    let mut a = app(7, Nation::Earth, SIZE);
    run(&mut a, "n");
    assert_eq!(a.ui.screen, Some(Screen::Nation));
    let frame = text(&mut a, SIZE);
    assert!(frame.contains("NATION"), "{frame}");
    assert!(frame.contains("IDLE"), "{frame}");
    assert!(frame.contains("STORES"), "{frame}");
    // A full screen covers the side panel (#652); the status line stays and says so.
    assert!(!frame.contains("EARTH KINGDOM"), "{frame}");
    assert!(!frame.contains("SETTLEMENT"), "{frame}");
    assert!(frame.lines().last().unwrap().contains("NATION"), "the status line says so");

    run(&mut a, "n");
    assert_eq!(a.ui.screen, None);
    run(&mut a, "n <esc>");
    assert_eq!(a.ui.screen, None);
    run(&mut a, "N");
    assert_eq!(a.ui.screen, Some(Screen::Nation));
}

#[test]
fn the_map_keys_wait_while_it_is_open() {
    let mut a = app(7, Nation::Earth, SIZE);
    let (cursor, view) = (a.ui.cursor, a.ui.view);
    run(&mut a, "n h x p b <down*2> <pgdn> <tab> f");
    assert_eq!(a.ui.cursor, cursor, "the map cursor did not move");
    assert_eq!(a.ui.view, view);
    assert_eq!(a.ui.tool, Tool::Select, "no tool was picked");
    assert_eq!(a.ui.selected, None, "nothing was selected yet");
    assert_eq!(a.ui.screen, Some(Screen::Nation));
    run(&mut a, "<esc> <right>");
    assert_ne!(a.ui.cursor, cursor);
}

/// `N`, a move and `Enter` leave that unit selected (#308's own acceptance test): the pointer's
/// settler becomes the selection, the map cursor moves onto them and the screen closes.
#[test]
fn n_a_move_and_enter_select_that_settler_and_close_the_screen() {
    let mut a = app(7, Nation::Earth, SIZE);
    run(&mut a, "n <down> <enter>");
    assert_eq!(a.ui.screen, None, "Enter closes the screen");
    let id = a.ui.selected.expect("a settler is selected");
    let unit = a.session.world.units.iter().find(|u| u.id == id).unwrap();
    assert_eq!(unit.name, a.session.world.units[1].name, "the second settler (one down from 0)");
    assert_eq!(a.ui.cursor, unit.pos, "the cursor moved onto them");
}

/// `PgDn` pages the pointer by more than one settler.
#[test]
fn page_keys_move_more_than_one_settler() {
    let mut a = app(7, Nation::Earth, SIZE);
    run(&mut a, "n <pgdn>");
    assert!(a.ui.nation_cursor > 1, "{}", a.ui.nation_cursor);
}
