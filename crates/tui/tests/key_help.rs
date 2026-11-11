//! The key help (#662): its keys fit one screen at 120x40 in two columns, fall back to one column
//! that says how far down it is below 100 columns, and `Tab` switches to the Legend, the map's
//! glyphs with their names, coloured as on the map.

use fn_sim::Nation;
use fn_tui::app::App;
use fn_tui::color::{ColorDepth, Palette};
use fn_tui::frame::{render_app, to_text};
use fn_tui::state::{HelpTab, Screen};
use fn_tui::{keys, new_session};
use ratatui::buffer::Buffer;

fn app(size: (u16, u16)) -> App {
    App::new(new_session(7, Nation::Earth), size)
}

fn run(app: &mut App, script: &str) {
    app.play(&keys::parse(script).expect("a valid script"));
}

fn palette(app: &App, ascii: bool) -> Palette {
    Palette::new(&app.session.defs, Nation::Earth, ColorDepth::TrueColor, ascii)
}

fn frame(app: &mut App, ascii: bool, size: (u16, u16)) -> Buffer {
    let p = palette(app, ascii);
    render_app(app, &p, size)
}

fn text(app: &mut App, size: (u16, u16)) -> String {
    to_text(&frame(app, false, size))
}

/// All of the keys are on the one 120x40 screen: the last row of each column shows, nothing
/// scrolls, and the bottom edge has no position on it.
#[test]
fn the_keys_fit_one_screen_at_120_by_40() {
    let size = (120, 40);
    let mut a = app(size);
    run(&mut a, "?");
    let seen = text(&mut a, size);
    for row in fn_ui::keys::help_rows() {
        assert!(seen.contains(&row.what), "{:?} is on screen:\n{seen}", row.what);
    }
    // The last row of the right column, and of the left.
    assert!(seen.contains("Drop the corner, then leave"), "{seen}");
    assert!(seen.contains("Sow a farm plot"), "{seen}");
    let bottom = seen.lines().nth(38).unwrap();
    assert!(bottom.starts_with('┗') && !bottom.contains(" of "), "{bottom}");
    // Nothing to scroll: PgDn stays at the top.
    run(&mut a, "<pgdn>");
    assert_eq!(text(&mut a, size), seen);
    // Two columns: Time sits on the first group's row, to the right.
    let line = seen.lines().find(|l| l.contains("CURSOR AND VIEW")).unwrap();
    assert!(line.contains("TIME"), "{line}");
}

/// Under 100 columns it is one column, and a page too short for it says where it is.
#[test]
fn below_100_columns_it_is_one_column_that_shows_its_scroll_position() {
    let size = (99, 30);
    let mut a = app(size);
    run(&mut a, "?");
    let seen = text(&mut a, size);
    let line = seen.lines().find(|l| l.contains("CURSOR AND VIEW")).unwrap();
    assert!(!line.contains("TIME"), "{line}");
    let bottom = seen.lines().nth(28).unwrap();
    assert!(bottom.contains(" 1-") && bottom.contains(" of "), "{bottom}");
    run(&mut a, "<pgdn*20>");
    let seen = text(&mut a, size);
    assert!(seen.contains("Drop the corner, then leave"), "the last row is reachable:\n{seen}");
}

/// `Tab` flips between the tabs, the strip says which is open, and the screen reopens on Keys.
#[test]
fn tab_switches_between_keys_and_legend() {
    let size = (120, 40);
    let mut a = app(size);
    run(&mut a, "?");
    assert_eq!(a.ui.help_tab, HelpTab::Keys);
    assert!(text(&mut a, size).contains("[Keys]  Legend"));
    run(&mut a, "<tab>");
    assert_eq!(a.ui.help_tab, HelpTab::Legend);
    assert!(text(&mut a, size).contains("Keys  [Legend]"));
    run(&mut a, "<tab>");
    assert_eq!(a.ui.help_tab, HelpTab::Keys);
    // Shift+Tab goes the same way; the tab is not kept when the screen closes.
    run(&mut a, "<s-tab>");
    assert_eq!(a.ui.help_tab, HelpTab::Legend);
    run(&mut a, "? ?");
    assert_eq!(a.ui.help_tab, HelpTab::Keys, "reopens on the keys");
    // Tab never selects a settler behind the screen.
    assert_eq!(a.ui.selected, None);
}

/// The legend names each kind of thing on the map, grouped, with the glyph the map draws in the
/// colour the map draws it.
#[test]
fn the_legend_lists_the_maps_glyphs_by_group_coloured_as_on_the_map() {
    let size = (120, 40);
    let mut a = app(size);
    let map = frame(&mut a, false, size);
    run(&mut a, "? <tab>");
    let legend = frame(&mut a, false, size);
    let seen = to_text(&legend);
    for want in ["PEOPLE", "Settler", "PLANTS", "TERRAIN", "MARKS", "Cursor"] {
        assert!(seen.contains(want), "{want:?} in\n{seen}");
    }
    // Each glyph in the legend is drawn as the map draws it: the same colours.
    let find = |buf: &Buffer, symbol: &str, in_legend: bool| {
        let rows = if in_legend { 3..buf.area.height } else { 0..buf.area.height };
        let mut found = None;
        for y in rows {
            for x in 0..buf.area.width {
                if buf[(x, y)].symbol() == symbol && (!in_legend || x < 20) {
                    found = Some((buf[(x, y)].fg, buf[(x, y)].modifier));
                }
            }
        }
        found
    };
    let on_map = find(&map, "☺", false).expect("a settler on the map");
    let in_legend = find(&legend, "☺", true).expect("a settler in the legend");
    assert_eq!(on_map.0, in_legend.0, "the settler's colour");
}

/// `--ascii` draws the legend's glyphs as the ASCII map does.
#[test]
fn the_legend_honours_ascii() {
    let size = (120, 40);
    let mut a = app(size);
    run(&mut a, "? <tab>");
    let seen = to_text(&frame(&mut a, true, size));
    assert!(seen.contains("@ Settler"), "{seen}");
    assert!(!seen.contains('☺'), "{seen}");
}

/// The game menu's Key help row opens the same screen as `?`.
#[test]
fn the_game_menus_key_help_row_opens_the_same_screen() {
    let size = (120, 40);
    let mut a = app(size);
    run(&mut a, "?");
    let direct = text(&mut a, size);
    let mut b = app(size);
    run(&mut b, "<esc> <down*3> <enter>");
    assert_eq!(b.ui.screen, Some(Screen::Help));
    assert_eq!(b.ui.help_tab, HelpTab::Keys);
    assert_eq!(text(&mut b, size), direct);
}
