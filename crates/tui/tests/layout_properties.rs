//! Layout properties for every TUI screen at a matrix of terminal sizes (#454): what the golden
//! frames are really guarding against, checked without depending on their exact content, so #455
//! can delete most of them. Each case below plays a short key script from a mid-game session
//! (some buildings, a selection, and -- for one of the two nations -- a live alert, #415) into
//! one of the game's screens, tools or modals, then four properties hold at every size in
//! [`SIZES`]:
//!
//! * no row is wider than the terminal it was drawn for;
//! * every heavy-framed box ([`fn_tui::widgets::Marks`]'s own `tl`/`tr`/`bl`/`br`) closes: the
//!   matching corners exist and the edge between them is the frame's own line, never something
//!   that spilled over it;
//! * the SETTLEMENT and HERE headings are always there (and ALERTS too, whenever an alert is
//!   live), spelled out in full -- a truncated heading would fail the literal match instead of
//!   quietly passing a shortened one;
//! * a cut line always ends with the panel's own ellipsis, never a bare word glued back onto one.
//!
//! The title screen and the embark screens (`fn_tui::shell::Shell`, not `fn_tui::app::App`) get
//! their own, smaller matrix below, for the same four properties.
//!
//! Whole-frame goldens are a handful of reference screens on a fixed hand-built world
//! (`reference_goldens.rs`, #455), kept for a reviewer to look at; this file and the
//! `contains`-style checks beside each feature are the regression net.

use std::path::PathBuf;

use fn_sim::building::BuildingId;
use fn_sim::embark::SurveyMode;
use fn_sim::geom::Footprint;
use fn_sim::world::{BendOutcome, Command};
use fn_sim::{Defs, Nation, Pos, Terrain};
use fn_tui::app::App;
use fn_tui::bar::bar_area;
use fn_tui::color::{ColorDepth, Palette};
use fn_tui::frame::{render_app, render_shell, to_text};
use fn_tui::keys;
use fn_tui::layout;
use fn_tui::shell::Shell;
use fn_tui::widgets::Marks;
use fn_ui::Session;
use fn_ui::tools::ability_for_nation;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;

/// Terminal sizes the acceptance criteria calls out by name: the floor, comfortably above it,
/// either side of the side panel's own width threshold (#423, [`layout::WIDE_TERMINAL`]), and
/// generous.
const SIZES: [(u16, u16); 6] = [(80, 24), (100, 30), (120, 40), (139, 40), (140, 40), (220, 60)];

/// A fresh empty directory, removed when dropped, so `<f6>`'s Saves screen has somewhere to
/// look (`panels.rs`'s/`save_list.rs`'s own `TempDir`): without one, `App` says there is no save
/// directory instead of opening the screen (`app.rs`'s `NO_SAVE_DIR`).
struct TempDir(PathBuf);

impl TempDir {
    fn new(name: &str) -> Self {
        let dir =
            std::env::temp_dir().join(format!("fn-tui-layout-props-{}-{name}", std::process::id()));
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

// ---------------------------------------------------------------------------------------------
// The properties themselves
// ---------------------------------------------------------------------------------------------

/// No row is wider than the terminal it was drawn for.
fn assert_rows_fit(text: &str, size: (u16, u16), context: &str) {
    for (y, line) in text.lines().enumerate() {
        let width = line.chars().count();
        assert!(
            width <= usize::from(size.0),
            "{context}, row {y}: {width} columns on a {}-wide terminal: {line:?}",
            size.0
        );
    }
}

/// Every heavy-framed box ([`Marks::tl`]/`tr`/`bl`/`br`) inside `area` closes: the matching
/// corners exist and the edge between them, down both sides, is the frame's own vertical line
/// (or a section heading's `ls`/`rs` swap-in, docs/design/side-panel.md "Headings"), never
/// something a row's own text spilled over it.
fn assert_boxes_close(buf: &Buffer, area: Rect, marks: &Marks, context: &str) {
    let sym = |x: u16, y: u16| buf[(x, y)].symbol().chars().next();
    for y in area.top()..area.bottom() {
        for x in area.left()..area.right() {
            if sym(x, y) != Some(marks.tl) {
                continue;
            }
            let tr_x = (x + 1..area.right()).find(|&cx| sym(cx, y) == Some(marks.tr));
            let Some(tr_x) = tr_x else {
                panic!(
                    "{context}: a box opens at ({x},{y}) with no matching top-right corner on \
                     its row"
                );
            };
            let mut bl_y = None;
            for cy in y + 1..area.bottom() {
                let c = sym(x, cy);
                if c == Some(marks.bl) {
                    bl_y = Some(cy);
                    break;
                }
                assert!(
                    c == Some(marks.v) || c == Some(marks.ls),
                    "{context}: the box opened at ({x},{y})'s left edge is {c:?} at ({x},{cy}), \
                     not the frame's own line"
                );
            }
            let Some(bl_y) = bl_y else {
                panic!("{context}: a box opens at ({x},{y}) with no matching bottom-left corner");
            };
            for cy in y + 1..bl_y {
                let c = sym(tr_x, cy);
                assert!(
                    c == Some(marks.v) || c == Some(marks.rs),
                    "{context}: the box opened at ({x},{y})'s right edge is {c:?} at \
                     ({tr_x},{cy}), not the frame's own line"
                );
            }
            assert_eq!(
                sym(tr_x, bl_y),
                Some(marks.br),
                "{context}: the box opened at ({x},{y}) has no matching bottom-right corner"
            );
        }
    }
}

/// No ellipsis is immediately followed by another letter or digit: a cut line always ends
/// cleanly, never with a bare word glued back on after the cut (docs/design/side-panel.md, the
/// same rule `fn_tui::widgets::fit` and `wrap` already enforce for one row at a time).
fn assert_ellipsis_never_glued(text: &str, context: &str) {
    for (y, line) in text.lines().enumerate() {
        let chars: Vec<char> = line.chars().collect();
        for (i, &c) in chars.iter().enumerate() {
            if c != '…' {
                continue;
            }
            if let Some(&next) = chars.get(i + 1) {
                assert!(
                    !next.is_alphanumeric(),
                    "{context}, row {y}: an ellipsis at column {i} is glued to more text: \
                     {line:?}"
                );
            }
        }
    }
}

/// Every heading in `want` is in `text` verbatim: a truncated heading would fail this literal
/// match instead of quietly passing a `…`-shortened prefix.
fn assert_headings_present(text: &str, want: &[&str], context: &str) {
    for heading in want {
        assert!(text.contains(heading), "{context}: missing the {heading:?} heading:\n{text}");
    }
}

// ---------------------------------------------------------------------------------------------
// Building the two mid-game sessions the matrix plays from
// ---------------------------------------------------------------------------------------------

/// Stamps a finished `id` with its anchor at `at`: the way a construction site looks once it
/// finishes, without waiting out the ticks (`fn_tui::tests::frames`'s own `place_building`).
fn place_building(s: &mut Session, id: BuildingId, at: Pos) {
    let def = s.defs.building(id);
    let (w, h) = def.footprint;
    let foot = Footprint::new(at, w, h);
    s.world.units.retain(|u| !foot.contains(u.pos));
    s.world.put_building(foot, id, &s.defs);
}

/// A walkable, resource-free tile near `near` with no unit on it, for a building that must not
/// land on anything already designated.
fn free_tile_near(s: &Session, near: Pos) -> Pos {
    s.world
        .map
        .iter()
        .filter(|(p, t)| {
            t.terrain.is_walkable()
                && t.resource.is_none()
                && t.building.is_none()
                && (p.x - near.x).abs().max((p.y - near.y).abs()) <= 12
                && !s.world.units.iter().any(|u| u.pos == *p)
        })
        .map(|(p, _)| p)
        .next()
        .expect("a free tile near home")
}

/// Fire, seed 3 (the same scenario `panels.rs`'s own `bending_game` uses): a settler's own tile
/// turns to rock mid-channel, so one of a box of designated bends is doomed to fail and its toast
/// is still live by the time any fresh [`fn_tui::app::App`] is built on this session (#415's
/// alert, `fn_ui::alerts::Alerts::update` recomputes every toast at full strength on the first
/// call). A finished Bed stands in for "some buildings".
fn fire_mid_game() -> Session {
    // A small map (`frames.rs`'s own `session_with_a_test_source` does the same): plenty of
    // room for the bend box and a building, at a fraction of the default 192x128 map's own
    // generation and simulation cost.
    let mut s = Session::new(3, Nation::Fire, 64, 48);
    let ability = ability_for_nation(&s.defs, Nation::Fire).expect("Fire has an ability");
    let from = s.world.units[0].pos;
    for dy in -5..=5 {
        for dx in -5..=5 {
            let pos = Pos::new(from.x + dx, from.y + dy);
            let _ = s.apply(Command::Bend { ability, pos });
        }
    }
    s.world.map[from].terrain = Terrain::Mountain;
    s.world.map_changed();
    for _ in 0..200 {
        s.tick_n(100);
        if s.world.bend_events().any(|e| matches!(e.outcome, BendOutcome::Failed(_))) {
            break;
        }
    }
    assert!(
        s.world.bend_events().any(|e| matches!(e.outcome, BendOutcome::Failed(_))),
        "the settler's own tile turning to rock should have doomed one channel"
    );
    let bed = s.defs.building_id("bed").expect("every nation has a bed");
    let at = free_tile_near(&s, Pos::new(from.x + 8, from.y + 8));
    place_building(&mut s, bed, at);
    s
}

/// Earth, seed 7 (every other golden frame's own seed): a few harvest designations and a
/// finished Bed, no alert -- the matrix's second nation, so the layout has to hold for more than
/// one nation's own names and glyphs.
fn earth_mid_game() -> Session {
    let mut s = Session::new(7, Nation::Earth, 64, 48);
    let from = s.world.units[0].pos;
    let mut tiles: Vec<Pos> =
        s.world.map.iter().filter(|(_, t)| t.resource.is_some()).map(|(p, _)| p).collect();
    tiles.sort_by_key(|p| (p.x - from.x).pow(2) + (p.y - from.y).pow(2));
    for pos in tiles.into_iter().take(10) {
        let _ = s.apply(Command::DesignateHarvest { pos });
    }
    let bed = s.defs.building_id("bed").expect("every nation has a bed");
    let at = free_tile_near(&s, Pos::new(from.x + 8, from.y + 8));
    place_building(&mut s, bed, at);
    s
}

// ---------------------------------------------------------------------------------------------
// The game's own screens, tools and modals
// ---------------------------------------------------------------------------------------------

/// One way to reach a screen, a tool or a modal from a mid-game session, with the first settler
/// already selected (`<enter>` on the cursor, which starts on it): what else must be in the
/// frame besides the side panel's own `SETTLEMENT`/`HERE` (always checked) and, for the sessions
/// with a live alert, `ALERTS` (also always checked there).
struct Case {
    name: &'static str,
    script: &'static str,
    headings: &'static [&'static str],
}

const CASES: &[Case] = &[
    Case { name: "map_select", script: "<enter>", headings: &[] },
    Case { name: "map_harvest", script: "<enter> h", headings: &["HARVEST"] },
    Case { name: "map_build", script: "<enter> u <enter>", headings: &["BUILD"] },
    Case { name: "map_stockpile", script: "<enter> p", headings: &["STOCKPILE"] },
    Case { name: "map_bend", script: "<enter> B", headings: &["BEND"] },
    Case { name: "map_clear", script: "<enter> e", headings: &["CLEAR"] },
    Case { name: "map_cancel", script: "<enter> x", headings: &["CANCEL"] },
    Case { name: "build_modal", script: "<enter> u", headings: &["BUILD"] },
    Case { name: "research_screen", script: "<enter> t", headings: &["RESEARCH"] },
    Case { name: "nation_screen", script: "<enter> n", headings: &["NATION"] },
    Case { name: "endeavor_screen", script: "<enter> g", headings: &["ENDEAVOR"] },
    Case { name: "log_screen", script: "<enter> l", headings: &["MESSAGE LOG"] },
    Case { name: "help_screen", script: "<enter> ?", headings: &["KEY HELP"] },
    Case { name: "saves_screen", script: "<enter> <f6>", headings: &["SAVES"] },
    Case { name: "menu_modal", script: "<enter> <esc> <esc>", headings: &["MENU"] },
    Case {
        name: "menu_nations",
        script: "<enter> <esc> <esc> <down*5> <enter>",
        headings: &["SWITCH NATION"],
    },
    Case {
        name: "confirm_new_map",
        script: "<enter> <esc> <esc> <down*4> <enter>",
        headings: &["NEW MAP"],
    },
    Case {
        name: "confirm_switch_nation",
        script: "<enter> <esc> <esc> <down*5> <enter> <down> <enter>",
        headings: &["SWITCH NATION"],
    },
    Case {
        name: "confirm_quit",
        script: "<enter> <esc> <esc> <down*6> <enter>",
        headings: &["QUIT"],
    },
    Case { name: "confirm_quickload", script: "<enter> <f9>", headings: &["LOAD"] },
];

#[test]
fn every_app_screen_at_every_size_closes_its_boxes_fits_its_rows_and_keeps_its_headings() {
    let marks = Marks::new(false);
    let tmp = TempDir::new("saves");
    let sessions: [(&str, Session, bool); 2] =
        [("fire_mid_game", fire_mid_game(), true), ("earth_mid_game", earth_mid_game(), false)];

    for (label, session, has_alert) in &sessions {
        for case in CASES {
            for &size in &SIZES {
                let mut app = App::new(session.clone(), size);
                app.saves = Some(tmp.0.clone());
                app.play(&keys::parse(case.script).expect("a valid script"));
                let palette = Palette::new(
                    &app.session.defs,
                    app.session.world.nation,
                    ColorDepth::TrueColor,
                    false,
                );
                let buf = render_app(&mut app, &palette, size);
                let text = to_text(&buf);
                let context = format!("{label} {} at {size:?}", case.name);

                assert_rows_fit(&text, size, &context);
                assert_ellipsis_never_glued(&text, &context);

                let mut want: Vec<&str> = vec!["SETTLEMENT", "HERE"];
                if *has_alert {
                    want.push("ALERTS");
                }
                want.extend_from_slice(case.headings);
                assert_headings_present(&text, &want, &context);

                let regions = layout::regions(buf.area)
                    .unwrap_or_else(|| panic!("{context}: too small for a {size:?} terminal"));
                assert_boxes_close(&buf, regions.panel, &marks, &format!("{context}: panel"));
                if app.ui.screen.is_some() || app.ui.modal.is_some() {
                    assert_boxes_close(
                        &buf,
                        regions.map,
                        &marks,
                        &format!("{context}: screen/modal"),
                    );
                } else if app.ui.selected_unit(&app.session).is_some() {
                    assert_boxes_close(
                        &buf,
                        bar_area(regions.map),
                        &marks,
                        &format!("{context}: selection bar"),
                    );
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------------------------
// The title screen and the embark screens
// ---------------------------------------------------------------------------------------------

/// A page of the title screen or an embark screen, reached by playing `script` on a fresh
/// [`Shell`], with the headings that must be in its frame besides whatever the matrix already
/// checks for every page.
struct ShellCase {
    name: &'static str,
    shell: fn() -> Shell,
    script: &'static str,
    headings: &'static [&'static str],
}

fn title_shell() -> Shell {
    Shell::new_title((120, 40), None, 1)
}

fn embark_shell() -> Shell {
    let defs = Defs::bundled();
    let size = defs.embark.map_size("small").expect("the small map size exists").clone();
    Shell::new_embark(
        7,
        Nation::Earth,
        size,
        SurveyMode::Exact,
        Some("normal".into()),
        (120, 40),
        None,
    )
}

fn embark_rough_shell() -> Shell {
    let defs = Defs::bundled();
    let size = defs.embark.map_size("small").expect("the small map size exists").clone();
    Shell::new_embark(
        7,
        Nation::Earth,
        size,
        SurveyMode::Rough,
        Some("normal".into()),
        (120, 40),
        None,
    )
}

const SHELL_CASES: &[ShellCase] = &[
    ShellCase { name: "title_menu", shell: title_shell, script: "", headings: &["FOUR NATIONS"] },
    ShellCase {
        name: "title_nation_picker",
        shell: title_shell,
        script: "<enter>",
        headings: &["FOUR NATIONS"],
    },
    ShellCase {
        name: "title_seed_step",
        shell: title_shell,
        script: "<enter> <enter>",
        headings: &["FOUR NATIONS"],
    },
    ShellCase {
        name: "embark_overview",
        shell: embark_shell,
        script: "",
        headings: &["CHOOSE A SITE"],
    },
    ShellCase {
        name: "embark_site",
        shell: embark_shell,
        script: "<enter> <enter> <enter>",
        headings: &["CHOOSE A SITE"],
    },
    ShellCase {
        name: "embark_refused",
        shell: embark_shell,
        script: "<left*50>",
        headings: &["CHOOSE A SITE"],
    },
    ShellCase {
        name: "embark_rough",
        shell: embark_rough_shell,
        script: "<enter> <enter>",
        headings: &["CHOOSE A SITE"],
    },
];

#[test]
fn every_title_and_embark_page_at_every_size_closes_its_boxes_fits_its_rows_and_keeps_its_headings()
{
    let marks = Marks::new(false);
    for case in SHELL_CASES {
        // Built once per case, not once per size: an embark shell surveys a whole map
        // (`fn_sim::embark::Survey`), which costs far more than resizing and redrawing it.
        let mut shell = (case.shell)();
        shell.play(&keys::parse(case.script).expect("a valid script"));
        for &size in &SIZES {
            let palette = Palette::new(shell.defs(), shell.nation(), ColorDepth::TrueColor, false);
            let buf = render_shell(&mut shell, &palette, size);
            let text = to_text(&buf);
            let context = format!("{} at {size:?}", case.name);

            assert_rows_fit(&text, size, &context);
            assert_ellipsis_never_glued(&text, &context);
            assert_headings_present(&text, case.headings, &context);

            if case.name.starts_with("embark") {
                // The embark screens share the game's own map/panel split
                // (`fn_tui::layout::regions`); the "CHOOSE A SITE" box is the panel's.
                if let Some(regions) = layout::regions(buf.area) {
                    assert_boxes_close(&buf, regions.panel, &marks, &context);
                }
            } else {
                // The title screen draws one box, centred over the whole terminal.
                assert_boxes_close(&buf, buf.area, &marks, &context);
            }
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Hints live in the hint bar only (#651)
// ---------------------------------------------------------------------------------------------

/// What a bottom border may say besides its line: where a window sits ("12-48 of 61", "tiers
/// 1-4 of 5"), the clock, the panel's seed. Never a key.
fn border_words_are_not_keys(line: &str, marks: &Marks, context: &str) {
    let chars: Vec<char> = line.chars().collect();
    for (i, &c) in chars.iter().enumerate() {
        if c != marks.bl {
            continue;
        }
        let Some(j) = (i + 1..chars.len()).find(|&j| chars[j] == marks.br) else { continue };
        let inner: String = chars[i + 1..j].iter().filter(|&&c| c != marks.h).collect();
        for word in inner.split_whitespace() {
            let position = word.chars().all(|c| c.is_ascii_digit() || matches!(c, '-' | ':'));
            let noun = matches!(word, "of" | "tiers" | "settlers" | "slots" | "buildings" | "seed");
            assert!(
                position || noun,
                "{context}: a bottom border says {word:?}, which is not a scroll position: \
                 {line:?}"
            );
        }
    }
}

/// No screen, modal or bar prints key hints on its own bottom border: Nation, Research,
/// Endeavor, Log, Key help, Saves, Build, the game menu and the selected settler's bar. The hint
/// bar (the status line) is the only place keys are named.
#[test]
fn no_bottom_border_carries_key_hints() {
    let marks = Marks::new(false);
    let tmp = TempDir::new("hint-borders");
    let session = earth_mid_game();
    for case in CASES {
        for size in [(80, 24), (120, 40)] {
            let mut app = App::new(session.clone(), size);
            app.saves = Some(tmp.0.clone());
            app.play(&keys::parse(case.script).expect("a valid script"));
            let palette = Palette::new(
                &app.session.defs,
                app.session.world.nation,
                ColorDepth::TrueColor,
                false,
            );
            let text = to_text(&render_app(&mut app, &palette, size));
            let context = format!("{} at {size:?}", case.name);
            let rows: Vec<&str> = text.lines().collect();
            // The last row is the hint bar itself.
            for line in &rows[..rows.len() - 1] {
                border_words_are_not_keys(line, &marks, &context);
            }
            // And the one place that does name keys always ends in the tail.
            assert!(rows[rows.len() - 1].trim_end().ends_with("? keys"), "{context}");
        }
    }
}

/// A screen that scrolls says where it is on its freed bottom border: the key help, which is
/// longer than the 80x24 terminal's screen.
#[test]
fn a_scrolling_screen_shows_its_position_on_the_bottom_border() {
    let session = earth_mid_game();
    let size = (80, 24);
    let mut app = App::new(session, size);
    app.play(&keys::parse("?").unwrap());
    let palette =
        Palette::new(&app.session.defs, app.session.world.nation, ColorDepth::TrueColor, false);
    let text = to_text(&render_app(&mut app, &palette, size));
    let border = text.lines().find(|l| l.contains('┗')).unwrap();
    assert!(border.contains(" 1-") && border.contains(" of "), "{border}");
    // Scrolled down a page, the window moves.
    app.play(&keys::parse("<pgdn>").unwrap());
    let text = to_text(&render_app(&mut app, &palette, size));
    let border = text.lines().find(|l| l.contains('┗')).unwrap();
    assert!(!border.contains(" 1-") && border.contains(" of "), "{border}");
}
