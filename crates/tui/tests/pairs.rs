//! Placing a pair (#698, docs/design/horizons-phase-2.md, "Mockups"): the Build tool asks for the
//! partner right after the first end, `Enter` places the second end and links it, `O` on an end
//! moves the partner, and a `:` link line runs between the ends while one is under the cursor.
//! The cursor panel reads the dock's hold, fuel and balloon and the Waystation's pantry and beds.
//!
//! The goldens are the strip's own map cells, in Unicode and in `--ascii`, so mapgen and the rest
//! of the frame can change without touching them. Rewrite with
//! `FN_BLESS=1 cargo test -p fn_tui --test it pairs::` (review the diff).

use fn_sim::building::BuildingId;
use fn_sim::geom::Footprint;
use fn_sim::world::Command;
use fn_sim::{Nation, Pos, Terrain};
use fn_tui::app::App;
use fn_tui::color::{ColorDepth, Palette};
use fn_tui::frame::{render_app, to_text};
use fn_tui::{keys, layout, new_session};
use fn_ui::Tool;
use ratatui::buffer::Buffer;

const SIZE: (u16, u16) = (120, 40);
/// The strip of clear land the pair goes on.
const WIDE: i32 = 30;
const TALL: i32 = 5;

/// `nation`, seed 7, with a clear strip near the first settler and `building` unlocked. Returns
/// the app and the strip's top-left tile; the Build tool has `building` chosen, the cursor is on
/// the strip's left end.
fn app_with_strip(nation: Nation, building: &str) -> (App, BuildingId, Pos) {
    let mut s = new_session(7, nation);
    let id = s.defs.building_id(building).unwrap();
    s.defs.buildings[id.0 as usize].unlocked_by = None;
    let from = s.world.units[0].pos;
    let at = Pos::new(from.x + 3, from.y + 3);
    let strip = Footprint::new(at, WIDE as u8, TALL as u8);
    for p in strip.tiles() {
        let t = s.world.map.get_mut(p).unwrap();
        t.terrain = Terrain::Land;
        t.resource = None;
    }
    s.world.units.retain(|u| !strip.contains(u.pos));
    s.world.map_changed();
    let mut app = App::new(s, SIZE);
    app.ui.choice = Some(id);
    app.ui.set_tool(Tool::Build);
    app.ui.go_to(Pos::new(at.x + 2, at.y + 2));
    (app, id, at)
}

/// The strip's cells as drawn, one row per line, then the notice.
fn snapshot(app: &mut App, at: Pos, ascii: bool) -> String {
    let p = Palette::new(&app.session.defs, app.session.world.nation, ColorDepth::TrueColor, ascii);
    let buf: Buffer = render_app(app, &p, SIZE);
    let map = layout::regions(buf.area).unwrap().map;
    let mut glyphs = String::new();
    for dy in 0..TALL {
        for dx in 0..WIDE {
            let (x, y) = (at.x + dx - app.ui.view.x, at.y + dy - app.ui.view.y);
            assert!(x >= 0 && y >= 0 && x < i32::from(map.width) && y < i32::from(map.height));
            glyphs.push_str(buf[(map.x + x as u16, map.y + y as u16)].symbol());
        }
        glyphs.push('\n');
    }
    format!("{glyphs}\nnotice: {}\n", app.ui.notice)
}

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

fn play(app: &mut App, script: &str) {
    app.play(&keys::parse(script).unwrap());
}

/// The whole frame as text, for the panel's lines.
fn screen(app: &mut App) -> String {
    let p = Palette::new(&app.session.defs, app.session.world.nation, ColorDepth::TrueColor, true);
    to_text(&render_app(app, &p, SIZE))
}

fn sites(app: &App) -> usize {
    app.session.world.build_sites().count()
}

#[test]
fn placing_a_gate_asks_for_its_partner_and_places_it() {
    for ascii in [false, true] {
        let tag = if ascii { "_ascii" } else { "" };
        let (mut app, _, at) = app_with_strip(Nation::Air, "wind_gate");
        let a = Pos::new(at.x + 2, at.y + 2);
        play(&mut app, "<enter> <enter>");
        assert_eq!(app.ui.linking, Some(a), "the new site asks for its partner");
        assert!(app.ui.notice.contains("Pick the partner"), "{}", app.ui.notice);
        assert_eq!(sites(&app), 1);
        golden(&format!("pair_gate_asks{tag}"), &snapshot(&mut app, at, ascii));

        // Five tiles away is nearer than a pair may be: refused, nothing placed, still asking.
        play(&mut app, "<right*5>");
        assert!(app.ui.notice.contains("too close"), "{}", app.ui.notice);
        play(&mut app, "<enter>");
        assert_eq!((app.ui.linking, sites(&app)), (Some(a), 1));
        golden(&format!("pair_gate_refused{tag}"), &snapshot(&mut app, at, ascii));

        // Fifteen away is fine: the second site is placed and linked, and the line shows.
        play(&mut app, "<right*10>");
        assert!(app.ui.notice.contains("Pick the partner"), "{}", app.ui.notice);
        golden(&format!("pair_gate_line{tag}"), &snapshot(&mut app, at, ascii));
        play(&mut app, "<enter>");
        let b = Pos::new(a.x + 15, a.y);
        assert_eq!(app.ui.linking, None);
        assert_eq!(sites(&app), 2);
        assert_eq!(app.session.world.link_at(a), Some(b));
        assert_eq!(app.session.world.link_at(b), Some(a));
        assert!(app.ui.notice.contains("Partner placed at"), "{}", app.ui.notice);
    }
}

#[test]
fn a_dock_pair_shows_its_line_on_either_end_and_the_cursor_panel_reads_them() {
    for ascii in [false, true] {
        let tag = if ascii { "_ascii" } else { "" };
        let (mut app, dock, at) = app_with_strip(Nation::Fire, "balloon_dock");
        let a = Pos::new(at.x + 2, at.y + 2);
        play(&mut app, "<enter> <enter>");
        assert_eq!(app.ui.linking, Some(app.session.world.footprint_at(a).unwrap().anchor));
        play(&mut app, "<right*20> <enter>");
        assert_eq!(app.ui.linking, None);
        assert_eq!(sites(&app), 2, "{}", app.ui.notice);
        // Back on the first end in Select mode, the line runs to the other.
        play(&mut app, "<esc> <left*20>");
        assert_eq!(app.ui.tool, Tool::Select);
        golden(&format!("pair_dock_line{tag}"), &snapshot(&mut app, at, ascii));

        // Finished, the cursor panel reads the partner, hold, fuel and balloon.
        let (a, b) = {
            let anchor = app.session.world.footprint_at(a).unwrap().anchor;
            (anchor, app.session.world.link_at(anchor).unwrap())
        };
        for p in [a, b] {
            let foot = app.session.world.footprint_at(p).unwrap();
            app.session.apply(Command::CancelBuild { pos: p }).unwrap();
            app.session.world.put_building(foot, dock, &app.session.defs);
        }
        app.session.apply(Command::SetLink { pos: a, to: b }).unwrap();
        let text = screen(&mut app);
        for want in
            ["BALLOON DOCK", "Partner: (", "20 tiles", "Hold: 0/", "Out of wood", "Balloon: "]
        {
            assert!(text.contains(want), "{want:?} missing from\n{text}");
        }
    }
}

#[test]
fn o_on_an_end_moves_the_partner() {
    let (mut app, _, at) = app_with_strip(Nation::Air, "wind_gate");
    let a = Pos::new(at.x + 2, at.y + 2);
    play(&mut app, "<enter> <enter> <right*15> <enter>");
    let b = Pos::new(a.x + 15, a.y);
    assert_eq!(app.session.world.link_at(a), Some(b));
    // Select mode, back on `a`: `O` asks again, and a new tile moves the partner there.
    play(&mut app, "<esc> <left*15> o");
    assert_eq!(app.ui.linking, Some(a));
    play(&mut app, "<right*5> <down> <right*10> <enter>");
    let c = Pos::new(a.x + 15, a.y + 1);
    assert_eq!(app.session.world.link_at(a), Some(c));
    assert_eq!(app.session.world.link_at(c), Some(a));
    assert_eq!(app.session.world.link_at(b), None, "the old partner is let go");
    // Pointing it at the old partner's own site links to that end instead of placing another.
    play(&mut app, "<left*15> <up> o <right*15> <enter>");
    assert_eq!(app.session.world.link_at(a), Some(b));
    assert_eq!(app.session.world.link_at(c), None);
    assert_eq!(sites(&app), 3, "no new site: it linked to the one that was there");
}

#[test]
fn esc_leaves_the_partner_unset() {
    let (mut app, _, at) = app_with_strip(Nation::Air, "wind_gate");
    play(&mut app, "<enter> <enter> <esc>");
    assert_eq!(app.ui.linking, None);
    assert_eq!(app.ui.notice, "Partner left unset");
    assert_eq!(app.session.world.link_at(Pos::new(at.x + 2, at.y + 2)), None);
    assert_eq!(sites(&app), 1);
}

#[test]
fn the_cursor_panel_reads_a_stable_and_a_waystation() {
    let (mut app, _, at) = app_with_strip(Nation::Earth, "stable");
    let defs = app.session.defs.clone();
    let stable = defs.building_id("stable").unwrap();
    let station = defs.building_id("waystation").unwrap();
    let (a, b) = (Pos::new(at.x + 2, at.y + 2), Pos::new(at.x + 10, at.y + 2));
    app.session.world.put_building(Footprint::new(a, 2, 2), stable, &defs);
    app.session.world.put_building(Footprint::new(b, 2, 2), station, &defs);
    app.session.tick();
    app.ui.set_tool(Tool::Select);
    app.ui.go_to(a);
    let text = screen(&mut app);
    for want in ["STABLE", "Food: 0/8", "Hungry"] {
        assert!(text.contains(want), "{want:?} missing from\n{text}");
    }
    app.ui.go_to(b);
    let text = screen(&mut app);
    for want in ["WAYSTATION", "Pantry empty", "Beds: 0/2 taken"] {
        assert!(text.contains(want), "{want:?} missing from\n{text}");
    }
}
