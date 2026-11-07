//! The Nation screen (`N`, #308, docs/design/side-panel.md "The Nation screen"): every settler
//! grouped by activity, with need meters or a flag (depending on the map region's width), every
//! store and the nation's traits. Golden frames are rewritten with `FN_BLESS=1 cargo test -p
//! fn_tui` (review the diff).

use fn_sim::world::Command;
use fn_sim::{Nation, Pos};
use fn_tui::app::App;
use fn_tui::color::{ColorDepth, Palette};
use fn_tui::frame::{render_app, to_text};
use fn_tui::state::Screen;
use fn_tui::{keys, new_session};
use fn_ui::Tool;

const SIZE: (u16, u16) = (120, 40);
const SMALL: (u16, u16) = (80, 24);
/// Wide enough that the old far-right-pinned meters (#325) would have drifted well away from the
/// names; the header and meters should still sit a fixed distance past them.
const WIDE: (u16, u16) = (200, 40);

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

fn golden(name: &str, actual: &str) {
    // FN_GOLDEN=skip (cargo xtask ci --fast --no-goldens): every golden comparison is a
    // no-op that passes, so agents can run the gate without caring whether goldens are stale.
    if std::env::var("FN_GOLDEN").ok().as_deref() == Some("skip") {
        return;
    }
    let path = format!("{}/tests/golden/{name}.txt", env!("CARGO_MANIFEST_DIR"));
    if std::env::var_os("FN_BLESS").is_some() {
        std::fs::write(&path, actual).unwrap();
    }
    let expected = std::fs::read_to_string(&path).expect("golden missing; run with FN_BLESS=1");
    assert_eq!(actual, expected, "{name}: the frame changed; rebless with FN_BLESS=1 and review");
}

/// A seed 7 Earth game with people at work (docs/design/side-panel.md "The Nation screen"):
/// harvest designated round the first settler, a stockpile a little further off for the haulers,
/// and enough ticks that the roster has more than one activity group.
fn working_game(size: (u16, u16)) -> App {
    let mut s = new_session(7, Nation::Earth);
    let from = s.world.units[0].pos;
    for dy in -6..=6 {
        for dx in -6..=6 {
            let _ = s.apply(Command::DesignateHarvest { pos: Pos::new(from.x + dx, from.y + dy) });
        }
    }
    for dy in 8..=11 {
        for dx in -2..=2 {
            let _ =
                s.apply(Command::DesignateStockpile { pos: Pos::new(from.x + dx, from.y + dy) });
        }
    }
    let mut app = App::new(s, size);
    for _ in 0..200 {
        app.play(&[keys::Step::Tick(200)]);
        let groups = fn_ui::view::activity_counts(&app.session).len();
        if groups >= 3 {
            break;
        }
    }
    assert!(
        fn_ui::view::activity_counts(&app.session).len() >= 3,
        "the settlers should have split into more than one activity"
    );
    app
}

#[test]
fn n_opens_the_screen_and_n_or_esc_close_it_with_the_side_panel_staying() {
    let mut a = app(7, Nation::Earth, SIZE);
    run(&mut a, "n");
    assert_eq!(a.ui.screen, Some(Screen::Nation));
    let frame = text(&mut a, SIZE);
    assert!(frame.contains("NATION"), "{frame}");
    assert!(frame.contains("IDLE"), "{frame}");
    assert!(frame.contains("STORES"), "{frame}");
    // The side panel is all there: its header, a section and the status line.
    assert!(frame.contains("EARTH KINGDOM"), "{frame}");
    assert!(frame.contains("SETTLEMENT"), "{frame}");
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

#[test]
fn golden_nation_wide_shows_meters() {
    let mut a = working_game(SIZE);
    run(&mut a, "n");
    golden("earth_seed7_nation", &text(&mut a, SIZE));
}

#[test]
fn golden_nation_narrow_drops_the_meters_for_a_flag() {
    let mut a = working_game(SMALL);
    run(&mut a, "n");
    golden("earth_seed7_nation_small", &text(&mut a, SMALL));
}

/// A very wide terminal (#325): the meter header and the meters themselves stay a fixed distance
/// past the name/task column instead of being pinned to the far right edge.
#[test]
fn golden_nation_wide_terminal_keeps_the_meters_near_the_names() {
    let mut a = working_game(WIDE);
    run(&mut a, "n");
    golden("earth_seed7_nation_wide", &text(&mut a, WIDE));
}
