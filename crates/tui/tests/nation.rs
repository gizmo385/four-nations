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
use fn_tui::shell::{Mode, Shell};
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

/// The `earth-year-two` scenario (#660): a settlement a year in, so its settlers are busy.
fn year_two(size: (u16, u16)) -> App {
    let args = fn_tui::cli::parse(
        ["--scenario", "earth-year-two", "--frame"].into_iter().map(str::to_owned),
    )
    .unwrap();
    let shell = Shell::for_args(&args, size, None, 0).unwrap();
    let Mode::Game(game) = shell.mode else { panic!("a scenario starts a game") };
    let mut game = *game;
    game.resize(size.0, size.1);
    game
}

/// The roster's rows of a frame: from the column header down to the first blank row.
fn roster_rows(frame: &str) -> Vec<&str> {
    let mut rows = frame.lines().skip_while(|l| !l.contains("NAME"));
    rows.next();
    rows.take_while(|l| !l.trim_matches(|c| c == '│' || c == ' ').is_empty()).collect()
}

/// At 120 wide no task in `earth-year-two` is cut (#660), every group has its full word, four
/// meters and one BENDING column stand where Chi, Bends and Forms were, and the detail strip
/// under the rule holds the selected settler.
#[test]
fn the_roster_shows_every_task_whole_with_four_meters_and_bending_in_words() {
    let mut a = year_two(SIZE);
    run(&mut a, "n");
    let frame = text(&mut a, SIZE);
    let rows = roster_rows(&frame);
    assert!(rows.len() > 4, "a busy settlement has several rows:\n{frame}");
    assert!(!rows.iter().any(|r| r.contains('…')), "no task is cut:\n{frame}");
    let header = frame.lines().find(|l| l.contains("NAME")).unwrap();
    for word in ["DOING", "HUNGER", "THIRST", "ENERGY", "HEALTH", "BENDING"] {
        assert!(header.contains(word), "{header}");
    }
    for word in ["Chi", "CHI", "Bends", "Forms", "FORMS"] {
        assert!(!header.contains(word), "{header}");
    }
    assert!(frame.contains(", 0 bends") || frame.contains(" bends"), "{frame}");
    assert!(!frame.contains("0/8"), "bending is in words:\n{frame}");
    // Group headings are full words: no "HAUL 3", "EAT 7".
    for heading in ["│ HAULING ", "│ HARVESTING ", "│ EATING AND DRINKING "] {
        assert!(frame.contains(heading), "{heading}\n{frame}");
    }
    for short in ["HAUL ", "EAT ", "HARVEST "] {
        assert!(!frame.contains(&format!("│ {short}")), "{short}\n{frame}");
    }
    // The strip is under a rule joined to the frame, and names the selected settler.
    let rule = frame.lines().position(|l| l.starts_with("├─")).expect("a rule");
    let strip = frame.lines().skip(rule + 1).take(4).collect::<Vec<_>>().join("\n");
    let who = a.session.world.units.iter().find(|u| Some(u.id) == nation_unit(&a)).unwrap();
    assert!(strip.contains(&who.name), "{strip}");
    assert!(strip.contains("Enter: go to them on the map"), "{strip}");
    assert!(strip.contains("Carrying"), "{strip}");
    assert!(strip.contains("To adept:") || strip.contains("Master:"), "{strip}");
}

/// The settler the Nation screen's pointer is on.
fn nation_unit(app: &App) -> Option<fn_sim::world::UnitId> {
    let view = fn_ui::view::nation_view(&app.session);
    fn_tui::nation::roster(&view).get(app.ui.nation_cursor).copied()
}

/// A task that cannot fit is cut at a word and the strip shows it whole: at 80 columns the roster
/// has the least room.
#[test]
fn a_task_that_cannot_fit_is_in_full_in_the_detail_strip() {
    let size = (80, 40);
    let mut a = year_two(size);
    run(&mut a, "n");
    let view = fn_ui::view::nation_view(&a.session);
    let longest = fn_tui::nation::roster(&view)
        .iter()
        .filter_map(|id| view.groups.iter().flat_map(|(_, us)| us).find(|u| u.id == *id))
        .max_by_key(|u| u.task.chars().count())
        .unwrap()
        .clone();
    let at = fn_tui::nation::roster(&view).iter().position(|&id| id == longest.id).unwrap();
    a.ui.nation_cursor = at;
    let frame = text(&mut a, size);
    let rule = frame.lines().position(|l| l.starts_with("├─")).expect("a rule");
    let strip: String = frame.lines().skip(rule + 1).take(3).collect::<Vec<_>>().join(" ");
    let squashed = strip.split_whitespace().collect::<Vec<_>>().join(" ");
    assert!(
        squashed.contains(longest.task.trim_end_matches('.')),
        "the strip holds {:?} whole:\n{frame}",
        longest.task
    );
}

/// At 80 columns the BENDING column is dropped before any meter.
#[test]
fn at_80_columns_bending_is_dropped_and_the_four_meters_stay() {
    let size = (80, 40);
    let mut a = year_two(size);
    run(&mut a, "n");
    let frame = text(&mut a, size);
    assert!(!frame.contains("BENDING"), "{frame}");
    let header = frame.lines().find(|l| l.contains("NAME")).unwrap();
    for word in ["HUNGER", "THIRST", "ENERGY", "HEALTH"] {
        assert!(header.contains(word), "{header}");
    }
}

/// `Enter` on a row closes the screen with the map cursor on that settler and them selected.
#[test]
fn enter_on_a_later_row_selects_that_settler_and_puts_the_cursor_on_them() {
    let mut a = year_two(SIZE);
    run(&mut a, "n <down*3>");
    let id = nation_unit(&a).expect("a settler under the pointer");
    run(&mut a, "<enter>");
    assert_eq!(a.ui.screen, None);
    assert_eq!(a.ui.selected, Some(id));
    let unit = a.session.world.units.iter().find(|u| u.id == id).unwrap();
    assert_eq!(a.ui.cursor, unit.pos);
}

/// STORES marks an item no stockpile can take more of: the sandbox preset starts with exactly
/// one full tile of wood, and tiles of other things round it.
#[test]
fn stores_marks_a_resource_whose_stockpiles_are_full() {
    let defs = fn_sim::Defs::bundled();
    let size = defs.embark.map_size("small").unwrap();
    let survey = fn_sim::embark::Survey::new(&fn_sim::embark::Survey::config(7, size), &defs);
    let start = survey.default_site(&defs);
    let session =
        fn_ui::Session::found(survey, Nation::Earth, start, Some("sandbox".to_owned())).unwrap();
    let mut a = App::new(session, SIZE);
    run(&mut a, "n");
    let frame = text(&mut a, SIZE);
    let stores = frame.lines().find(|l| l.contains("Wood")).expect("a stores line");
    assert!(stores.contains("Wood 20 full"), "{stores}");
    assert!(!stores.contains("Fruit 30 full") && !stores.contains("Stone 10 full"), "{stores}");
}
