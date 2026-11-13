//! Beasts and canals on the terminal (#699, docs/design/horizons-phase-2.md, "Mockups"): a beast
//! draws as its own glyph and shows in the Settlement panel and the inspector, a canal draws in
//! its own blue, and Carve Canal in the Bend tool says what it does and why it is refused.
//!
//! The goldens are the patch's own map cells, in Unicode and in `--ascii`, so the rest of the
//! frame can change without touching them. Rewrite with
//! `FN_BLESS=1 cargo test -p fn_tui --test it beasts_canals` (review the diff).

use fn_sim::geom::Footprint;
use fn_sim::{Nation, Pos, Terrain};
use fn_tui::app::App;
use fn_tui::color::{ColorDepth, Palette};
use fn_tui::frame::{render_app, to_text};
use fn_tui::map::{Overlays, Thing, cell_look, style_cell};
use fn_tui::{keys, new_session};
use fn_ui::{Session, Tool};
use ratatui::buffer::Buffer;
use ratatui::style::Modifier;

const SIZE: (u16, u16) = (120, 40);

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

fn play(app: &mut App, script: &str) {
    app.play(&keys::parse(script).unwrap());
}

/// The whole frame as text.
fn frame_text(app: &mut App) -> String {
    let p = palette(app, false);
    to_text(&render_app(app, &p, SIZE))
}

/// The `w` x `h` tiles from `at` as drawn on the map, one row a line.
fn cells(app: &mut App, at: Pos, w: i32, h: i32, ascii: bool) -> String {
    let p = palette(app, ascii);
    let buf: Buffer = render_app(app, &p, SIZE);
    let map = fn_tui::layout::regions(buf.area).unwrap().map;
    let mut out = String::new();
    for dy in 0..h {
        for dx in 0..w {
            let (x, y) = (at.x + dx - app.ui.view.x, at.y + dy - app.ui.view.y);
            assert!(x >= 0 && y >= 0 && x < i32::from(map.width) && y < i32::from(map.height));
            out.push_str(buf[(map.x + x as u16, map.y + y as u16)].symbol());
        }
        out.push('\n');
    }
    out
}

/// What the map draws on `pos`, as `cell_look` and `style_cell` decide it.
fn look_at(app: &App, ascii: bool, pos: Pos) -> (Thing, fn_tui::map::TileCell) {
    let p = palette(app, ascii);
    let over = Overlays::gather(&app.session, &app.ui);
    let look = cell_look(&app.session, &app.ui, &p, &over, pos).unwrap();
    let cell = style_cell(&look, &app.session.defs, &p);
    (look.thing, cell)
}

// ---------------------------------------------------------------------------------------------
// Beasts
// ---------------------------------------------------------------------------------------------

/// An Air game with a finished Bison Stable three tiles from the first settler, on cleared
/// ground, and the tick that brings its bison. Returns the app, the stable's anchor and the
/// bison's tile.
fn air_with_a_stable() -> (App, Pos, Pos) {
    let mut s = new_session(1, Nation::Air);
    let from = s.world.units[0].pos;
    let anchor = Pos::new(from.x + 3, from.y + 3);
    let stable = s.defs.building_id("bison_stable").unwrap();
    let foot = Footprint::new(anchor, 2, 2);
    for dy in -1..=3 {
        for dx in -1..=3 {
            let t = s.world.map.get_mut(Pos::new(anchor.x + dx, anchor.y + dy)).unwrap();
            t.terrain = Terrain::Land;
            t.resource = None;
        }
    }
    s.world.units.retain(|u| !foot.contains(u.pos));
    s.world.put_building(foot, stable, &s.defs);
    s.tick();
    let beast = s.world.beasts().next().expect("the stable's bison").pos;
    let mut app = App::new(s, SIZE);
    app.ui.go_to(beast);
    (app, anchor, beast)
}

/// Before any home is built there is no Beasts row; with a stable there is, and the bison
/// draws as its own glyph in its own colour, bold, in Unicode and in ASCII.
#[test]
fn a_beast_draws_as_its_own_glyph_and_the_settlement_panel_counts_it() {
    let before = new_session(1, Nation::Air);
    let mut app = App::new(before, SIZE);
    assert!(!frame_text(&mut app).contains("Beasts"), "no beast, no row");

    let (mut app, _, pos) = air_with_a_stable();
    let text = frame_text(&mut app);
    assert!(text.contains("Beasts 1"), "{text}");

    // Off the beast's tile, so the cursor's own colours don't stand in for its.
    app.ui.cursor = Pos::new(pos.x + 4, pos.y);
    for (ascii, glyph) in [(false, 'ß'), (true, 'b')] {
        let (thing, cell) = look_at(&app, ascii, pos);
        assert!(matches!(thing, Thing::Beast { selected: false, .. }), "{thing:?}");
        assert_eq!(cell.glyph, glyph);
        assert!(cell.style.add_modifier.contains(Modifier::BOLD));
        let want = palette(&app, ascii).glyphs.beast("sky_bison").fg;
        assert_eq!(cell.style.fg, Some(palette(&app, ascii).color(want)));
    }
}

/// The beast under the cursor is named in the inspector with its home, and selecting it swaps
/// that for its own rows: no meters, rank or chi.
#[test]
fn a_beast_is_inspectable_under_the_cursor_and_selected() {
    let (mut app, _, pos) = air_with_a_stable();
    app.ui.cursor = pos;
    let text = frame_text(&mut app);
    assert!(text.contains("SKY BISON"), "{text}");
    assert!(text.contains("Home: Bison Stable"), "{text}");

    let id = app.session.world.beasts().next().unwrap().id;
    app.ui.select_unit(&app.session, id);
    let text = frame_text(&mut app);
    assert!(text.contains("SKY BISON"), "{text}");
    assert!(text.contains("Beast"), "{text}");
    assert!(text.contains("Home: Bison Stable"), "{text}");
    assert!(text.contains("Standing on"), "{text}");
    for gone in ["Hunger", "Thirst", "Energy", "Chi "] {
        assert!(!text.contains(gone), "a beast has no {gone}:\n{text}");
    }
    // Selected, the beast is in reverse video, and the map still draws it.
    let (thing, cell) = look_at(&app, false, pos);
    assert!(matches!(thing, Thing::Beast { selected: true, .. }), "{thing:?}");
    assert!(cell.style.add_modifier.contains(Modifier::REVERSED));
}

/// A settler standing on the beast's tile draws over it, unless the beast is the selected one.
#[test]
fn a_settler_draws_over_a_beast_unless_the_beast_is_selected() {
    let (mut app, _, pos) = air_with_a_stable();
    app.session.world.units[0].pos = pos;
    assert!(matches!(look_at(&app, false, pos).0, Thing::Unit { .. }));
    let id = app.session.world.beasts().next().unwrap().id;
    app.ui.select(Some(id));
    assert!(matches!(look_at(&app, false, pos).0, Thing::Beast { selected: true, .. }));
}

/// The legend lists the beast with the people, by its row's name.
#[test]
fn the_legend_names_the_beast() {
    let (app, _, _) = air_with_a_stable();
    let p = palette(&app, false);
    let found = fn_tui::legend::entries(&app.session, &app.ui, &p);
    let bison = found.iter().find(|e| e.name == "Sky Bison").expect("a bison in view");
    assert_eq!((bison.group, bison.glyph), (fn_tui::legend::LegendGroup::People, 'ß'));
}

/// The stable and its bison at the door, as the player sees them.
#[test]
fn the_stable_and_its_bison_golden() {
    for ascii in [false, true] {
        let (mut app, anchor, _) = air_with_a_stable();
        let at = Pos::new(anchor.x - 1, anchor.y - 1);
        let tag = if ascii { "_ascii" } else { "" };
        golden(&format!("beast_stable{tag}"), &cells(&mut app, at, 5, 5, ascii));
    }
}

// ---------------------------------------------------------------------------------------------
// Canals
// ---------------------------------------------------------------------------------------------

/// Overwrites the `w` x `h` box at `at` with `rows` (`~` sea, `=` river, `C` canal, `.` land),
/// ringed by one tile of sea, with nothing on any tile: a patch of the map the test controls.
fn carve(s: &mut Session, at: Pos, rows: &[&str]) {
    let (w, h) = (rows[0].len() as i32, rows.len() as i32);
    for dy in -1..=h {
        for dx in -1..=w {
            let pos = Pos::new(at.x + dx, at.y + dy);
            let t = s.world.map.get_mut(pos).unwrap();
            let ch = if (0..w).contains(&dx) && (0..h).contains(&dy) {
                rows[dy as usize].as_bytes()[dx as usize] as char
            } else {
                '~'
            };
            t.terrain = match ch {
                '=' => Terrain::River,
                'C' => Terrain::Canal,
                '.' => Terrain::Land,
                _ => Terrain::Water,
            };
            t.resource = None;
        }
    }
    s.world.units.retain(|u| {
        u.pos.x < at.x - 1 || u.pos.x > at.x + w || u.pos.y < at.y - 1 || u.pos.y > at.y + h
    });
}

const PATCH: Pos = Pos::new(4, 4);

/// A Water game with Carve Canal usable (its tech lifted) and `rows` carved at [`PATCH`].
fn water_with(rows: &[&str]) -> App {
    let mut s = new_session(2, Nation::Water);
    let carve_canal = s.defs.ability_id("carve_canal").expect("Carve Canal");
    s.defs.abilities[carve_canal.0 as usize].unlocked_by = None;
    s.bend_choice = Some(carve_canal);
    carve(&mut s, PATCH, rows);
    let mut app = App::new(s, SIZE);
    app.ui.go_to(Pos::new(PATCH.x + 3, PATCH.y + 1));
    app
}

/// A canal carved east from the river: the river and the canal share a glyph in ASCII and tell
/// apart in Unicode by their own colours.
#[test]
fn a_canal_draws_in_its_own_blue_and_shares_the_rivers_ascii_glyph() {
    let rows = ["=CCCC.", "......"];
    for ascii in [false, true] {
        let mut app = water_with(&rows);
        let tag = if ascii { "_ascii" } else { "" };
        golden(&format!("canal_cut{tag}"), &cells(&mut app, PATCH, 6, 2, ascii));
    }
    let app = water_with(&rows);
    let (river, canal) = (Pos::new(PATCH.x, PATCH.y), Pos::new(PATCH.x + 2, PATCH.y));
    let (_, river_cell) = look_at(&app, false, river);
    let (_, canal_cell) = look_at(&app, false, canal);
    assert_eq!(river_cell.glyph, canal_cell.glyph, "the same wave");
    assert_ne!(river_cell.style.fg, canal_cell.style.fg);
    assert_ne!(river_cell.style.bg, canal_cell.style.bg);
    assert_eq!(look_at(&app, true, canal).1.glyph, '=');
}

/// Hovering a canal says what it does and that it is fresh water.
#[test]
fn hovering_a_canal_says_what_it_does() {
    let mut app = water_with(&["=CCCC.", "......"]);
    app.ui.cursor = Pos::new(PATCH.x + 2, PATCH.y);
    let text = frame_text(&mut app);
    assert!(text.contains("CANAL"), "{text}");
    assert!(text.contains("Irrigates plots within 2"), "{text}");
    assert!(text.contains("Fresh water"), "{text}");
}

/// The Bend tool says what Carve Canal does, from its data, and the abilities screen does too.
#[test]
fn the_bend_tool_and_the_abilities_screen_say_what_carve_canal_does() {
    let mut app = water_with(&["=.....", "......"]);
    play(&mut app, "b <enter>");
    assert_eq!(app.ui.tool, Tool::Bend);
    let text = frame_text(&mut app);
    for line in
        ["Turns land beside fresh", "water into a canal", "Irrigates plots within 2", "Permanent"]
    {
        assert!(text.contains(line), "{line} in\n{text}");
    }
    play(&mut app, "<esc> i");
    let text = frame_text(&mut app);
    assert!(text.contains("Carve Canal"), "{text}");
    assert!(text.contains("Turns land beside fresh water into a canal"), "{text}");
    assert!(text.contains("Permanent"), "{text}");
}

/// A tile with no fresh water beside it is refused in the Tool section, in a capital: the sim's
/// own words are lowercase.
#[test]
fn carve_canal_says_why_a_tile_has_no_fresh_water() {
    let mut app = water_with(&["=.....", "......"]);
    app.ui.cursor = Pos::new(PATCH.x + 3, PATCH.y + 1);
    play(&mut app, "b <enter> <enter>");
    let text = frame_text(&mut app);
    assert!(text.contains("Needs fresh water beside it"), "{text}");
    assert!(!text.contains("needs fresh water beside it"), "{text}");
    // Beside the river it is accepted.
    app.ui.cursor = Pos::new(PATCH.x + 1, PATCH.y);
    play(&mut app, "<esc>");
    play(&mut app, "b <enter> <enter>");
    let text = frame_text(&mut app);
    assert!(!text.contains("Needs fresh water"), "{text}");
}

/// A cut that would leave a one-tile causeway in two is refused, in a capital.
#[test]
fn carve_canal_says_why_a_cut_would_split_the_land() {
    let mut app = water_with(&["=======", "~.....~", "~~~~~~~"]);
    app.ui.cursor = Pos::new(PATCH.x + 3, PATCH.y + 1);
    play(&mut app, "b <enter> <enter>");
    let text = frame_text(&mut app);
    assert!(text.contains("Would cut the land in two"), "{text}");
    // The end of the causeway is safe.
    play(&mut app, "<esc> <left*2>");
    play(&mut app, "b <enter> <enter>");
    let text = frame_text(&mut app);
    assert!(!text.contains("Would cut the land in two"), "{text}");
}
