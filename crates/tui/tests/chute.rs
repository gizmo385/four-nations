//! Picking a chute's foot (#683): placing a chute site asks for the foot with a reach tint and a
//! refusal on tiles it can't go on, `O` on a chute re-picks it, and a `:` line runs from head to
//! foot while the head is under the cursor.
//!
//! Each step is a `--keys` script played on a hand-carved slope beside the first settler, and the
//! goldens are the slope's own map cells plus the Tool section's notice, in Unicode and in
//! `--ascii`, so mapgen and the rest of the frame can change without touching them. Rewrite with
//! `FN_BLESS=1 cargo test -p fn_tui --test it chute::` (review the diff).

use fn_sim::building::BuildingId;
use fn_sim::{Nation, Pos, Terrain};
use fn_tui::app::App;
use fn_tui::color::{ColorDepth, Palette};
use fn_tui::frame::render_app;
use fn_tui::map::{Mark, Overlays, cell_look};
use fn_tui::{keys, new_session};
use fn_ui::Tool;
use ratatui::buffer::Buffer;

const SIZE: (u16, u16) = (120, 40);
/// The slope: a rock column on the left, then land falling 5 elevation a tile eastward.
const WIDE: i32 = 20;
const TALL: i32 = 5;

/// Earth, seed 7, with the slope carved near the first settler and the chute's tech lifted.
/// Returns the app, the chute and the slope's top-left tile (the rock column).
fn app_with_slope() -> (App, BuildingId, Pos) {
    let mut s = new_session(7, Nation::Earth);
    let chute = s.defs.building_id("chute").unwrap();
    s.defs.buildings[chute.0 as usize].unlocked_by = None;
    let from = s.world.units[0].pos;
    let at = Pos::new(from.x + 3, from.y + 3);
    for dy in 0..TALL {
        for dx in 0..WIDE {
            let t = s.world.map.get_mut(Pos::new(at.x + dx, at.y + dy)).unwrap();
            t.terrain = if dx == 0 { Terrain::Mountain } else { Terrain::Land };
            t.elevation = (200 - 5 * dx) as u8;
            t.resource = None;
        }
    }
    s.world.units.retain(|u| {
        !(u.pos.x >= at.x && u.pos.x < at.x + WIDE && u.pos.y >= at.y && u.pos.y < at.y + TALL)
    });
    let mut app = App::new(s, SIZE);
    app.ui.go_to(Pos::new(at.x + 1, at.y + 2));
    (app, chute, at)
}

/// The slope's cells as drawn, one row per line; then the same rows with `~` where the foot
/// picker's reach tint lands (`Mark::FootReach`) and `.` elsewhere; then the notice.
fn snapshot(app: &mut App, at: Pos, ascii: bool) -> String {
    let p = Palette::new(&app.session.defs, app.session.world.nation, ColorDepth::TrueColor, ascii);
    let buf: Buffer = render_app(app, &p, SIZE);
    let map = fn_tui::layout::regions(buf.area).unwrap().map;
    let over = Overlays::gather(&app.session, &app.ui);
    let (mut glyphs, mut reach) = (String::new(), String::new());
    for dy in 0..TALL {
        for dx in 0..WIDE {
            let pos = Pos::new(at.x + dx, at.y + dy);
            let (x, y) = (pos.x - app.ui.view.x, pos.y - app.ui.view.y);
            assert!(x >= 0 && y >= 0 && x < i32::from(map.width) && y < i32::from(map.height));
            glyphs.push_str(buf[(map.x + x as u16, map.y + y as u16)].symbol());
            let look = cell_look(&app.session, &app.ui, &p, &over, pos).unwrap();
            reach.push(if look.marks.contains(&Mark::FootReach) { '~' } else { '.' });
        }
        glyphs.push('\n');
        reach.push('\n');
    }
    format!("{glyphs}\n{reach}\nnotice: {}\n", app.ui.notice)
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

/// The Build tool with the chute chosen, as the Build modal leaves it.
fn build_chute(app: &mut App, chute: BuildingId) {
    app.ui.choice = Some(chute);
    app.ui.set_tool(Tool::Build);
}

#[test]
fn placing_a_chute_asks_for_its_foot() {
    for ascii in [false, true] {
        let tag = if ascii { "_ascii" } else { "" };
        let (mut app, chute, at) = app_with_slope();
        let head = Pos::new(at.x + 1, at.y + 2);
        build_chute(&mut app, chute);
        play(&mut app, "<enter> <enter>");
        assert_eq!(app.ui.linking, Some(head), "the new site asks for its foot");
        golden(&format!("chute_asks{tag}"), &snapshot(&mut app, at, ascii));

        // One tile east is only 5 lower: refused, and the picker stays open.
        play(&mut app, "<right> <enter>");
        assert!(app.ui.notice.contains("not far enough downhill"), "{}", app.ui.notice);
        assert_eq!(app.ui.linking, Some(head));
        golden(&format!("chute_refused{tag}"), &snapshot(&mut app, at, ascii));

        play(&mut app, "<right*9> <enter>");
        assert_eq!(app.ui.linking, None);
        assert_eq!(app.session.world.link_at(head), Some(Pos::new(head.x + 10, head.y)));
        // Back on the head in Select mode, the line shows.
        play(&mut app, "<esc> <left*10>");
        assert_eq!(app.ui.tool, Tool::Select);
        golden(&format!("chute_line{tag}"), &snapshot(&mut app, at, ascii));
    }
}

#[test]
fn o_on_a_chute_re_picks_its_foot() {
    let (mut app, chute, at) = app_with_slope();
    let head = Pos::new(at.x + 1, at.y + 2);
    build_chute(&mut app, chute);
    play(&mut app, "<enter> <enter> <esc> <esc>");
    assert_eq!((app.ui.linking, app.ui.tool), (None, Tool::Select));
    assert_eq!(app.session.world.link_at(head), None, "Esc leaves it unset");
    play(&mut app, "o");
    assert_eq!(app.ui.linking, Some(head));
    play(&mut app, "<right*5> <down> <enter>");
    assert_eq!(app.session.world.link_at(head), Some(Pos::new(head.x + 5, head.y + 1)));
    assert_eq!(app.ui.notice, format!("Chute foot set at ({}, {})", head.x + 5, head.y + 1));
    // `O` off a chute does nothing in Select mode.
    play(&mut app, "o");
    assert_eq!(app.ui.linking, None);
}
