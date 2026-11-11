//! The full-screen frame (#652, docs/design/tui.md "Full screens"): every screen that takes over
//! the game (Nation, Research, Endeavor, Message log, Key help, Saves and Build) is drawn in one
//! frame that covers the side panel, takes every column and every row above the hint bar, closes
//! on `Esc` and on the key that opened it, says how far down it is on its bottom edge, and marks
//! its selection with a pointer and a quiet background rather than reverse video.

use std::path::PathBuf;

use fn_sim::Nation;
use fn_tui::app::App;
use fn_tui::color::{ColorDepth, Palette};
use fn_tui::frame::{render_app, to_text};
use fn_tui::page::select_bg;
use fn_tui::widgets::Marks;
use fn_tui::{keys, new_session};
use ratatui::buffer::Buffer;
use ratatui::style::Modifier;

/// The two sizes the acceptance criteria names: the floor and a roomy terminal.
const SIZES: [(u16, u16); 2] = [(80, 24), (120, 40)];

/// Each screen: the key that opens it, and the title on its frame.
const SCREENS: [(&str, &str); 7] = [
    ("n", "NATION"),
    ("t", "RESEARCH"),
    ("g", "ENDEAVOR"),
    ("l", "MESSAGE LOG"),
    ("?", "KEY HELP"),
    ("<f6>", "SAVES"),
    ("u", "BUILD"),
];

/// A fresh empty directory, removed when dropped, so `F6` has somewhere to look.
struct TempDir(PathBuf);

impl TempDir {
    fn new(name: &str) -> Self {
        let dir =
            std::env::temp_dir().join(format!("fn-tui-full-screen-{}-{name}", std::process::id()));
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

fn app(size: (u16, u16), saves: &TempDir) -> App {
    let mut app = App::new(new_session(7, Nation::Earth), size);
    app.saves = Some(saves.0.clone());
    app
}

fn run(app: &mut App, script: &str) {
    app.play(&keys::parse(script).expect("a valid script"));
}

fn palette(app: &App) -> Palette {
    Palette::new(&app.session.defs, app.session.world.nation, ColorDepth::TrueColor, false)
}

fn render(app: &mut App, size: (u16, u16)) -> Buffer {
    let p = palette(app);
    render_app(app, &p, size)
}

/// Every screen's frame takes the full width and every row above the hint bar, with its title on
/// the top edge, and the side panel isn't drawn under it.
#[test]
fn every_screen_covers_the_whole_page_at_eighty_by_twenty_four_and_one_twenty_by_forty() {
    let marks = Marks::new(false);
    let tmp = TempDir::new("covers");
    for size in SIZES {
        for (key, title) in SCREENS {
            let mut a = app(size, &tmp);
            run(&mut a, key);
            assert!(a.ui.full_screen(), "{key} at {size:?} opened a full screen");
            let buf = render(&mut a, size);
            let text = to_text(&buf);
            let lines: Vec<&str> = text.lines().collect();
            let (w, h) = (usize::from(size.0), usize::from(size.1));
            assert_eq!(lines.len(), h);
            let top: Vec<char> = lines[0].chars().collect();
            let bottom: Vec<char> = lines[h - 2].chars().collect();
            let context = format!("{title} at {size:?}:\n{text}");
            assert_eq!(top.len(), w, "{context}");
            assert_eq!(bottom.len(), w, "{context}");
            assert_eq!((top[0], top[w - 1]), (marks.tl, marks.tr), "{context}");
            assert_eq!((bottom[0], bottom[w - 1]), (marks.bl, marks.br), "{context}");
            assert!(lines[0].contains(&format!(" ◆ {title} ◆ ")), "{context}");
            for line in &lines[1..h - 2] {
                let row: Vec<char> = line.chars().collect();
                assert_eq!(row.len(), w, "every row reaches the right edge: {context}");
                assert!(matches!(row[0], '│' | '├'), "{context}");
                assert!(matches!(row[w - 1], '│' | '┤'), "{context}");
            }
            // Nothing of the side panel: not its heading, not its nation title.
            assert!(!text.contains("SETTLEMENT"), "{context}");
            assert!(!text.contains("EARTH KINGDOM ◆"), "{context}");
            // The hint bar is still the last row.
            assert!(lines[h - 1].trim_end().ends_with("? keys"), "{context}");
        }
    }
}

/// Every screen closes on `Esc` and on the key that opened it, and the map comes back.
#[test]
fn every_screen_closes_on_esc_and_on_its_own_key() {
    let tmp = TempDir::new("closes");
    for (key, title) in SCREENS {
        for close in ["<esc>", key] {
            let mut a = app((120, 40), &tmp);
            run(&mut a, key);
            assert!(a.ui.full_screen(), "{title}: {key} opens it");
            run(&mut a, close);
            assert!(!a.ui.full_screen(), "{title}: {close} closes it");
            assert!(a.ui.screen.is_none() && a.ui.modal.is_none(), "{title}: {close}");
            let text = to_text(&render(&mut a, (120, 40)));
            assert!(text.contains("SETTLEMENT"), "{title}: the side panel is back:\n{text}");
        }
    }
    // `U` in either case closes the Build screen, and leaves the tool as it was.
    let mut a = app((120, 40), &tmp);
    run(&mut a, "u U");
    assert!(a.ui.modal.is_none());
    assert_eq!(a.ui.tool, fn_ui::Tool::Select, "closing picks nothing");
}

/// A screen with more rows than fit says where it is on its bottom edge, and nothing else.
#[test]
fn a_long_screen_shows_its_position_on_the_bottom_edge() {
    let tmp = TempDir::new("position");
    let mut a = app((80, 24), &tmp);
    run(&mut a, "?");
    let text = to_text(&render(&mut a, (80, 24)));
    let bottom = text.lines().nth(22).unwrap();
    let total = fn_tui::screens::line_count(fn_tui::state::Screen::Help, &a.session, 76);
    // The tab strip and a blank row take two of the 21 rows.
    assert!(total > 19, "the help is longer than a page at 80x24");
    assert!(bottom.ends_with(&format!(" 1-19 of {total} ─┛")), "{bottom}");
    run(&mut a, "<pgdn>");
    let text = to_text(&render(&mut a, (80, 24)));
    let bottom = text.lines().nth(22).unwrap();
    let first = 20.min(total - 18);
    assert!(bottom.contains(&format!(" {first}-{} of {total} ", first + 18)), "{bottom}");
}

/// The Nation screen's selected row: the pointer, a quiet background under every cell, brighter
/// (bold) text, and no reverse video, so its meters keep their own ok and warning colours.
#[test]
fn the_selected_nation_row_wears_a_quiet_background_and_keeps_its_meter_colours() {
    let tmp = TempDir::new("nation_row");
    let size = (120, 40);
    let mut a = app(size, &tmp);
    a.session.world.units[0].needs.hunger = 0.0;
    run(&mut a, "n");
    let p = palette(&a);
    let buf = render_app(&mut a, &p, size);
    let text = to_text(&buf);
    let name = &a.session.world.units[0].name;
    let y = text
        .lines()
        .position(|l| l.contains(&format!("▸ {name}")))
        .unwrap_or_else(|| panic!("{name} under the pointer:\n{text}")) as u16;
    let select = select_bg(&p).expect("a colour palette has a selection background");
    // Every cell between the padding columns is on the quiet background, bold, never reversed.
    for x in 2..size.0 - 2 {
        let cell = &buf[(x, y)];
        assert_eq!(cell.bg, select, "({x},{y}) {:?}", cell.symbol());
        assert!(cell.modifier.contains(Modifier::BOLD), "({x},{y})");
        assert!(!cell.modifier.contains(Modifier::REVERSED), "({x},{y})");
    }
    // The meters keep their own colours: the empty hunger meter's flag in `bad` or `warn`,
    // a full one in `ok`.
    let theme = &p.theme;
    let row: Vec<&str> = (0..size.0).map(|x| buf[(x, y)].symbol()).collect();
    let flag = row.iter().position(|s| *s == "!").expect("hunger is flagged") as u16;
    let fg = buf[(flag, y)].fg;
    assert!(fg == p.token(theme.bad) || fg == p.token(theme.warn), "{fg:?}");
    let full = row.iter().rposition(|s| *s == "█").expect("a full meter") as u16;
    assert_eq!(buf[(full, y)].fg, p.token(theme.accent_text), "chi in the accent");
    let ok = (0..size.0).find(|&x| buf[(x, y)].fg == p.token(theme.ok));
    assert!(ok.is_some(), "an ok meter keeps its colour");
    // The row under it is not selected.
    assert_ne!(buf[(4, y + 1)].bg, select);
}

/// A screen runs or pauses the clock as before (#652 only moved where it draws): the clock keys
/// still work under every screen, and the Build screen leaves the clock alone.
#[test]
fn the_clock_runs_under_a_screen_as_before() {
    let tmp = TempDir::new("clock");
    for (key, title) in SCREENS {
        let mut a = app((120, 40), &tmp);
        let paused = a.session.clock.paused;
        run(&mut a, key);
        assert_eq!(a.session.clock.paused, paused, "{title}: opening leaves the clock alone");
    }
    for (key, title) in SCREENS.iter().filter(|(k, _)| *k != "u") {
        let mut a = app((120, 40), &tmp);
        let paused = a.session.clock.paused;
        run(&mut a, &format!("{key} <space>"));
        assert_ne!(a.session.clock.paused, paused, "{title}: Space still pauses or runs");
    }
}
