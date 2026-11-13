//! The corner notices in the terminal (#962): boxes over the map's top right, newest on top, with
//! key hints for their actions. The model is `fn_ui::notice`'s (its own tests); this is the
//! drawing, the keys, and that a game turns them on.
//!
//! Two goldens, the notice region alone (the map under and around it is mapgen's): a caravan's
//! camp, and four techs, two of them behind the "+N more" line. Rewrite with
//! `FN_BLESS=1 cargo test -p fn_tui --test it notices::` (review the diff).

use fn_sim::{Nation, Pos};
use fn_tui::app::App;
use fn_tui::color::{ColorDepth, Palette};
use fn_tui::frame::{render_app, to_text};
use fn_tui::notices::{MAX_BOXES, WIDTH};
use fn_tui::shell::{Mode, Shell};
use fn_tui::state::Modal;
use fn_tui::{keys, layout, new_session};
use fn_ui::alerts::Kind;
use fn_ui::notice::NoticeAction;
use fn_ui::{Session, Tool};
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;

const SIZE: (u16, u16) = (120, 40);
const SMALL: (u16, u16) = (80, 24);

fn golden(name: &str, actual: &str) {
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

fn palette(app: &App, ascii: bool) -> Palette {
    Palette::new(&app.session.defs, app.session.world.nation, ColorDepth::TrueColor, ascii)
}

fn frame(app: &mut App, size: (u16, u16), ascii: bool) -> Buffer {
    let p = palette(app, ascii);
    render_app(app, &p, size)
}

/// Earth, seed 7, notices on as a game in the terminal has them.
fn game(size: (u16, u16)) -> App {
    let mut app = App::new(new_session(7, Nation::Earth), size);
    app.ui.alerts.notices_mut().set_on(true);
    app
}

/// A game where the trade caravan makes camp within 100 ticks (the arrival pulled forward), played
/// until it has: the real notice a caravan raises, with the camp to jump to.
fn caravan_game() -> App {
    let mut s = Session::new(7, Nation::Earth, 64, 64);
    let src = include_str!("../../../data/trade.toml")
        .replacen("opens = \"known_on_the_road\"", "", 1)
        .replacen("offset_minutes = 4.0", "offset_minutes = 0.1", 1);
    s.defs = s.defs.clone().with_trade(&src).unwrap();
    let mut app = App::new(s, SIZE);
    app.ui.alerts.notices_mut().set_on(true);
    app.play(&keys::parse("<tick 100>").unwrap());
    assert!(app.session.world.caravan().is_some(), "the caravan made camp");
    app
}

/// A tech notice: the accent edge, "Enter · T research".
fn tech(app: &mut App, tick: u64, name: &str) {
    app.ui.alerts.notices_mut().push(
        (Kind::Tech, tick),
        (format!("Research done: {name}"), Vec::new()),
        vec![NoticeAction::OpenResearch],
        None,
    );
}

/// The map region of `buf`.
fn map(buf: &Buffer) -> Rect {
    layout::regions(buf.area).unwrap().map
}

/// The rows of `buf` from `rows` at the notices' columns on the map's right: what the corner
/// stack draws over.
fn corner(buf: &Buffer, rows: std::ops::Range<u16>) -> String {
    let map = map(buf);
    let width = WIDTH.min(map.width - 1);
    let x0 = map.right() - 1 - width;
    let mut out = String::new();
    for y in rows {
        let mut row = String::new();
        for x in x0..x0 + width {
            row.push_str(buf[(x, y)].symbol());
        }
        out.push_str(row.trim_end());
        out.push('\n');
    }
    out
}

#[test]
fn a_caravan_is_a_box_in_the_corner_with_its_keys() {
    let mut app = caravan_game();
    let buf = frame(&mut app, SIZE, false);
    let text = corner(&buf, 0..5);
    golden("notice_caravan", &text);
    assert!(text.contains("Fire Nation traders"), "{text}");
    assert!(text.contains("made camp"), "{text}");
    assert!(text.contains("Enter jump · M trade · Del ✕"), "{text}");
    // The edge wears the warning colour, the box is flush with the map's right side but one.
    let map = map(&buf);
    let p = palette(&app, false);
    let top_edge = &buf[(map.right() - 1 - 1 - 5, 0)];
    assert_eq!(top_edge.symbol(), "─");
    assert_eq!(top_edge.fg, p.token(p.theme.warn));
}

#[test]
fn the_ascii_frame_is_ascii() {
    let mut app = caravan_game();
    let buf = frame(&mut app, SIZE, true);
    let text = corner(&buf, 0..5);
    assert!(text.is_ascii(), "{text}");
    assert!(text.contains("Enter jump - M trade - Del x"), "{text}");
}

#[test]
fn two_boxes_then_a_more_line_newest_on_top() {
    let mut app = game(SIZE);
    for (tick, name) in [(1, "Metalbending"), (2, "Ice Construction"), (3, "Fire Making")] {
        tech(&mut app, tick, name);
    }
    tech(&mut app, 4, "Pottery");
    let buf = frame(&mut app, SIZE, false);
    // Two boxes of four rows (a bare notice: edge, title, hints, edge), then the more line.
    let text = corner(&buf, 0..9);
    golden("notice_overflow", &text);
    let rows: Vec<&str> = text.lines().collect();
    assert!(rows[1].contains("Research done: Pottery"), "newest on top\n{text}");
    assert!(rows[5].contains("Research done: Fire Making"), "{text}");
    assert!(rows[2].contains("Enter · T research · Del ✕"), "{text}");
    assert_eq!(rows[8].trim(), "+2 more · Del dismiss", "{text}");
    assert_eq!(MAX_BOXES, 2);
}

#[test]
fn del_takes_the_top_one_down_and_the_next_comes_up() {
    let mut app = game(SIZE);
    for (tick, name) in [(1, "Metalbending"), (2, "Ice Construction"), (3, "Fire Making")] {
        tech(&mut app, tick, name);
    }
    app.play(&keys::parse("<del>").unwrap());
    let text = to_text(&frame(&mut app, SIZE, false));
    assert!(!text.contains("Fire Making"), "the newest is gone\n{text}");
    assert!(text.contains("Ice Construction") && text.contains("Metalbending"), "{text}");
    assert!(!text.contains("more · Del dismiss"), "{text}");
}

#[test]
fn enter_runs_the_first_action_and_takes_the_notice_down() {
    let mut app = caravan_game();
    let camp = app.session.world.caravan().unwrap().camp;
    app.ui.go_to(Pos::new(0, 0));
    app.play(&keys::parse("<enter>").unwrap());
    assert!(!app.ui.alerts.notices().any(), "done with");
    assert_eq!(app.ui.cursor, camp, "the map is on the camp");
}

#[test]
fn a_small_terminal_gets_narrower_boxes_that_still_fit() {
    let mut app = game(SMALL);
    for (tick, name) in [(1, "Metalbending"), (2, "Ice Construction"), (3, "Fire Making")] {
        tech(&mut app, tick, name);
    }
    let buf = frame(&mut app, SMALL, false);
    let text = to_text(&buf);
    assert!(text.contains("Research done: Fire Making"), "{text}");
    assert!(text.contains("+1 more · Del dismiss"), "{text}");
    // The boxes end inside the map: no light box corner reaches the side panel.
    let map = map(&buf);
    for y in 0..map.height {
        for x in map.right()..buf.area.right() {
            let symbol = buf[(x, y)].symbol();
            assert!(!matches!(symbol, "┐" | "┘"), "({x}, {y})\n{text}");
        }
    }
}

#[test]
fn a_tool_banner_keeps_its_row_over_the_notices() {
    let mut app = caravan_game();
    app.play(&keys::parse("h").unwrap());
    assert_eq!(app.ui.tool, Tool::Harvest);
    let text = to_text(&frame(&mut app, SIZE, false));
    let rows: Vec<&str> = text.lines().collect();
    assert!(rows[0].contains("HARVEST"), "{text}");
    assert!(!rows[0].contains("Fire Nation traders"), "{text}");
    assert!(rows[2].contains("Fire Nation traders"), "under the banner\n{text}");
}

#[test]
fn a_screen_or_a_menu_covers_them() {
    let mut app = caravan_game();
    app.play(&keys::parse("t").unwrap());
    let text = to_text(&frame(&mut app, SIZE, false));
    assert!(!text.contains("Fire Nation traders"), "{text}");
    let mut menu = caravan_game();
    // The game menu is a box of its own over the map: the notices wait under it.
    menu.ui.modal = Some(Modal::Menu);
    let text = to_text(&frame(&mut menu, SIZE, false));
    assert!(!text.contains("Fire Nation traders"), "{text}");
}

#[test]
fn a_terminal_game_has_notices_on_and_keeps_them_across_a_new_map() {
    let shell = Shell::new_game(new_session(7, Nation::Earth), SIZE, None);
    let Mode::Game(mut app) = shell.mode else { panic!("a game") };
    assert!(app.ui.alerts.notices().is_on());
    // The game's first frame is not news.
    assert!(!app.ui.alerts.notices().any());
    app.restart(8, Nation::Water);
    assert!(app.ui.alerts.notices().is_on(), "a new map keeps them on");
    let world = app.session.world.clone();
    app.load(world);
    assert!(app.ui.alerts.notices().is_on(), "so does a load");
    assert!(!app.ui.alerts.notices().any(), "the past is not news");
}
