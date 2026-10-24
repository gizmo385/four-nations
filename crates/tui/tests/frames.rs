//! Frames through the public API: golden text frames (rewrite them with `FN_BLESS=1 cargo test -p
//! fn_tui` and review the diff) and the colour of particular cells.
//!
//! Colours are checked cell by cell here and not in colour goldens, which would churn on every
//! palette tweak.

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use fn_sim::world::Command;
use fn_sim::{Nation, Pos};
use fn_tui::app::App;
use fn_tui::color::{ColorDepth, Palette, Tint};
use fn_tui::frame::{render_app, to_ansi, to_text};
use fn_tui::layout;
use fn_tui::new_session;
use fn_ui::Session;
use ratatui::buffer::{Buffer, Cell};
use ratatui::style::{Color, Modifier};

const SIZE: (u16, u16) = (120, 40);

fn palette(session: &Session, depth: ColorDepth, ascii: bool) -> Palette {
    Palette::new(&session.defs, session.world.nation, depth, ascii)
}

fn frame(app: &mut App, depth: ColorDepth, ascii: bool, size: (u16, u16)) -> Buffer {
    let p = palette(&app.session, depth, ascii);
    render_app(app, &p, size)
}

fn golden(name: &str, actual: &str) {
    let path = format!("{}/tests/golden/{name}.txt", env!("CARGO_MANIFEST_DIR"));
    if std::env::var_os("FN_BLESS").is_some() {
        std::fs::write(&path, actual).unwrap();
    }
    let expected = std::fs::read_to_string(&path).expect("golden missing; run with FN_BLESS=1");
    assert_eq!(actual, expected, "{name}: the frame changed; see the module docs to rebless");
}

// ---------------------------------------------------------------------------------------------
// Golden frames
// ---------------------------------------------------------------------------------------------

/// One seed per nation, each from the first frame of a new game.
#[test]
fn golden_frame_per_nation() {
    for (seed, nation) in
        [(1, Nation::Air), (2, Nation::Water), (7, Nation::Earth), (3, Nation::Fire)]
    {
        let mut app = App::new(new_session(seed, nation), SIZE);
        let text = to_text(&frame(&mut app, ColorDepth::TrueColor, false, SIZE));
        golden(&format!("{}_seed{seed}", nation.as_str()), &text);
    }
}

#[test]
fn golden_ascii_frame() {
    let mut app = App::new(new_session(7, Nation::Earth), SIZE);
    let text = to_text(&frame(&mut app, ColorDepth::TrueColor, true, SIZE));
    assert!(text.is_ascii(), "the ASCII frame must be ASCII:\n{text}");
    golden("earth_seed7_ascii", &text);
}

/// Paused, with a few designations and the cursor moved: the status badge, the Cursor section on
/// a tree, and the same map at another scroll.
#[test]
fn golden_paused_with_designations_and_a_moved_cursor() {
    let mut app = App::new(new_session(7, Nation::Earth), SIZE);
    designate_around_the_settlers(&mut app.session);
    app.act(fn_tui::input::Action::TogglePause);
    app.act(fn_tui::input::Action::Move { dx: 1, dy: 1, tiles: 10 });
    golden("earth_seed7_paused", &to_text(&frame(&mut app, ColorDepth::TrueColor, false, SIZE)));
}

#[test]
fn golden_too_small_frame() {
    for size in [(79, 24), (80, 23), (60, 10)] {
        let mut app = App::new(new_session(7, Nation::Earth), size);
        let text = to_text(&frame(&mut app, ColorDepth::TrueColor, false, size));
        assert_eq!(text.lines().next(), Some("Terminal too small (80x24 needed)"));
        assert_eq!(text.lines().filter(|l| !l.is_empty()).count(), 1, "only the notice:\n{text}");
    }
    let mut app = App::new(new_session(7, Nation::Earth), (60, 20));
    golden("too_small", &to_text(&frame(&mut app, ColorDepth::TrueColor, false, (60, 20))));
}

#[test]
fn the_smallest_terminal_draws_the_whole_screen() {
    let mut app = App::new(new_session(7, Nation::Earth), (80, 24));
    let text = to_text(&frame(&mut app, ColorDepth::TrueColor, false, (80, 24)));
    assert!(!text.contains("too small"));
    assert!(text.contains("EARTH KINGDOM"), "{text}");
    assert!(text.contains("CURSOR"), "{text}");
    assert_eq!(text.lines().count(), 24);
}

// ---------------------------------------------------------------------------------------------
// Input
// ---------------------------------------------------------------------------------------------

fn press(app: &mut App, code: KeyCode, modifiers: KeyModifiers) {
    app.key(KeyEvent::new(code, modifiers));
}

fn text_after(keys: &[(KeyCode, KeyModifiers)]) -> String {
    let mut app = App::new(new_session(7, Nation::Earth), SIZE);
    for &(code, modifiers) in keys {
        press(&mut app, code, modifiers);
    }
    to_text(&frame(&mut app, ColorDepth::TrueColor, false, SIZE))
}

#[test]
fn wasd_and_the_arrows_give_the_same_frame() {
    let none = KeyModifiers::NONE;
    for (letter, arrow) in
        [('w', KeyCode::Up), ('a', KeyCode::Left), ('s', KeyCode::Down), ('d', KeyCode::Right)]
    {
        let by_letter = text_after(&[(KeyCode::Char(letter), none); 3]);
        let by_arrow = text_after(&[(arrow, none); 3]);
        assert_eq!(by_letter, by_arrow, "{letter} and {arrow:?}");
        assert_ne!(by_letter, text_after(&[]), "moving changes the Cursor section");
    }
}

#[test]
fn shift_wasd_and_shift_arrows_give_the_same_frame() {
    let shift = KeyModifiers::SHIFT;
    for (letter, arrow) in
        [('W', KeyCode::Up), ('A', KeyCode::Left), ('S', KeyCode::Down), ('D', KeyCode::Right)]
    {
        let by_letter = text_after(&[(KeyCode::Char(letter), shift)]);
        let by_arrow = text_after(&[(arrow, shift)]);
        assert_eq!(by_letter, by_arrow, "{letter} and Shift+{arrow:?}");
        // Ten tiles is not one: it matches ten plain presses.
        let ten_plain = text_after(&[(arrow, KeyModifiers::NONE); 10]);
        assert_eq!(by_arrow, ten_plain, "Shift+{arrow:?} is ten presses");
    }
}

#[test]
fn wasd_move_the_cursor_and_never_scroll_the_view_alone() {
    let mut app = App::new(new_session(7, Nation::Earth), SIZE);
    let (view, cursor) = (app.ui.view, app.ui.cursor);
    press(&mut app, KeyCode::Char('d'), KeyModifiers::NONE);
    assert_eq!(app.ui.cursor, Pos::new(cursor.x + 1, cursor.y));
    assert_eq!(app.ui.view, view, "the view only follows at the edges");
}

#[test]
fn space_pauses_and_the_dot_steps_one_tick() {
    let mut app = App::new(new_session(7, Nation::Earth), SIZE);
    let none = KeyModifiers::NONE;
    press(&mut app, KeyCode::Char('.'), none);
    assert_eq!(app.session.world.tick, 0, "stepping is for pausing");
    press(&mut app, KeyCode::Char(' '), none);
    assert!(app.session.clock.paused);
    app.advance(5.0);
    assert_eq!(app.session.world.tick, 0, "paused: no time passes");
    press(&mut app, KeyCode::Char('.'), none);
    press(&mut app, KeyCode::Char('.'), none);
    assert_eq!(app.session.world.tick, 2);
    let text = to_text(&frame(&mut app, ColorDepth::TrueColor, false, SIZE));
    assert!(text.contains("PAUSED") && text.contains("Paused"), "{text}");
    press(&mut app, KeyCode::Char(' '), none);
    app.advance(1.0);
    assert_eq!(app.session.world.tick, 2 + 10);
}

#[test]
fn plus_and_minus_change_the_speed() {
    let mut app = App::new(new_session(7, Nation::Earth), SIZE);
    let none = KeyModifiers::NONE;
    press(&mut app, KeyCode::Char('+'), none);
    press(&mut app, KeyCode::Char('+'), none);
    assert_eq!(app.session.clock.speed(), 3);
    assert!(to_text(&frame(&mut app, ColorDepth::TrueColor, false, SIZE)).contains("Running x3"));
    press(&mut app, KeyCode::Char('-'), none);
    assert_eq!(app.session.clock.speed(), 2);
}

#[test]
fn q_asks_first_and_only_y_quits() {
    let none = KeyModifiers::NONE;
    let mut app = App::new(new_session(7, Nation::Earth), SIZE);
    press(&mut app, KeyCode::Char('q'), none);
    assert!(!app.ui.quit, "q only asks");
    let text = to_text(&frame(&mut app, ColorDepth::TrueColor, false, SIZE));
    assert!(text.contains("QUIT?"), "{text}");
    // Any other key withdraws the question, and is not also acted on.
    let cursor = app.ui.cursor;
    press(&mut app, KeyCode::Char('d'), none);
    assert!(!app.ui.quit && app.ui.prompt.is_none());
    assert_eq!(app.ui.cursor, cursor);
    press(&mut app, KeyCode::Char('Q'), none);
    press(&mut app, KeyCode::Esc, none);
    assert!(!app.ui.quit && app.ui.prompt.is_none());
    press(&mut app, KeyCode::Char('q'), none);
    press(&mut app, KeyCode::Char('y'), none);
    assert!(app.ui.quit);
}

#[test]
fn a_terminal_resize_keeps_the_cursor_in_view() {
    let mut app = App::new(new_session(7, Nation::Earth), SIZE);
    app.resize(80, 24);
    assert!(app.ui.cursor_cell().is_some());
    let text = to_text(&frame(&mut app, ColorDepth::TrueColor, false, (80, 24)));
    assert!(text.contains("EARTH KINGDOM"));
}

// ---------------------------------------------------------------------------------------------
// Colours of particular cells
// ---------------------------------------------------------------------------------------------

/// The cell that shows tile `pos` in the frame `buf` drawn for `app`.
fn cell_at<'b>(app: &App, buf: &'b Buffer, pos: Pos) -> &'b Cell {
    let map = layout::regions(buf.area).unwrap().map;
    let (x, y) = (pos.x - app.ui.view.x, pos.y - app.ui.view.y);
    assert!(
        x >= 0 && y >= 0 && x < i32::from(map.width) && y < i32::from(map.height),
        "{pos:?} not in view"
    );
    &buf[(map.x + x as u16, map.y + y as u16)]
}

/// Designates a harvest on the nearest tree, a stockpile beside the first settler, and a build
/// site, so the map has each kind of tint.
fn designate_around_the_settlers(s: &mut Session) -> (Pos, Pos, Pos) {
    let from = s.world.units[0].pos;
    let near = |p: &Pos| (p.x - from.x).pow(2) + (p.y - from.y).pow(2);
    let mut tiles: Vec<_> = s.world.map.iter().map(|(p, t)| (p, *t)).collect();
    tiles.sort_by_key(|(p, _)| near(p));
    let occupied = |s: &Session, p: Pos| s.world.units.iter().any(|u| u.pos == p);

    // The nearest resource the nation can work already (some need research first).
    let candidates: Vec<Pos> = tiles
        .iter()
        .filter(|(p, t)| t.resource.is_some() && !occupied(s, *p))
        .map(|(p, _)| *p)
        .collect();
    let tree = candidates
        .into_iter()
        .find(|&pos| s.apply(Command::DesignateHarvest { pos }).is_ok())
        .expect("a harvestable resource");

    let free = |s: &Session, p: &Pos, t: &fn_sim::Tile| {
        t.terrain == fn_sim::Terrain::Land && t.resource.is_none() && !occupied(s, *p) && *p != tree
    };
    let land: Vec<Pos> =
        tiles.iter().filter(|(p, t)| free(s, p, t)).map(|(p, _)| *p).take(2).collect();
    let zone = land[0];
    s.apply(Command::DesignateStockpile { pos: zone }).unwrap();
    let site = land[1];
    let bed = s.defs.building_id("bed").unwrap();
    s.apply(Command::DesignateBuild { pos: site, building: bed }).unwrap();
    (tree, zone, site)
}

fn ground_bg(p: &Palette, session: &Session, pos: Pos) -> [u8; 3] {
    let tile = session.world.map.get(pos).unwrap();
    let look = match tile.terrain {
        fn_sim::Terrain::Land => &p.glyphs.biome[&session.defs.biome(tile.biome).id],
        other => p.glyphs.terrain(other).unwrap(),
    };
    look.bg.unwrap()
}

#[test]
fn designations_tint_the_background_and_the_cursor_wins() {
    let mut app = App::new(new_session(7, Nation::Earth), SIZE);
    let (tree, zone, site) = designate_around_the_settlers(&mut app.session);
    let p = palette(&app.session, ColorDepth::TrueColor, false);
    // Put the cursor on the tree so the view covers all three, then off it.
    app.ui.cursor = site;
    let buf = frame(&mut app, ColorDepth::TrueColor, false, SIZE);
    for (pos, tint) in [(tree, Tint::Harvest), (zone, Tint::Stockpile), (site, Tint::Build)] {
        if pos == site {
            continue; // under the cursor, below
        }
        let want = p.tinted(ground_bg(&p, &app.session, pos), tint);
        assert_eq!(cell_at(&app, &buf, pos).bg, want, "{pos:?} should be {tint:?}");
    }
    // The cursor beats the build tint: black on the cursor colour.
    let cursor = cell_at(&app, &buf, site);
    assert_eq!(cursor.bg, p.cursor_bg());
    assert_eq!(cursor.fg, Color::Rgb(0, 0, 0));
    // The site draws as its building, dimmed.
    assert_eq!(cursor.symbol(), "Θ");
}

#[test]
fn a_unit_draws_in_its_nation_colour_in_bold() {
    for nation in Nation::ALL {
        let mut app = App::new(new_session(7, nation), SIZE);
        let p = palette(&app.session, ColorDepth::TrueColor, false);
        // Look at a unit that isn't under the cursor.
        let unit = app.session.world.units[1].pos;
        let buf = frame(&mut app, ColorDepth::TrueColor, false, SIZE);
        let cell = cell_at(&app, &buf, unit);
        assert_eq!(cell.symbol(), "☺", "{nation:?}");
        assert_eq!(cell.fg, p.unit(), "{nation:?}");
        assert!(cell.modifier.contains(Modifier::BOLD));
        let [r, g, b] = p.unit_rgb();
        assert_eq!(cell.fg, Color::Rgb(r, g, b));
    }
}

#[test]
fn ascii_units_are_ats() {
    let mut app = App::new(new_session(7, Nation::Earth), SIZE);
    let unit = app.session.world.units[1].pos;
    let buf = frame(&mut app, ColorDepth::TrueColor, true, SIZE);
    assert_eq!(cell_at(&app, &buf, unit).symbol(), "@");
}

#[test]
fn the_cursor_cell_is_black_on_yellow_and_the_terminal_cursor_sits_there() {
    let mut app = App::new(new_session(7, Nation::Earth), SIZE);
    let p = palette(&app.session, ColorDepth::TrueColor, false);
    let buf = frame(&mut app, ColorDepth::TrueColor, false, SIZE);
    let cell = cell_at(&app, &buf, app.ui.cursor);
    assert_eq!((cell.fg, cell.bg), (Color::Rgb(0, 0, 0), Color::Rgb(250, 220, 60)));
    assert_eq!(cell.symbol(), "☺", "the cursor keeps the glyph it covers");
    let at = fn_tui::draw::terminal_cursor(&app.ui, buf.area).unwrap();
    assert_eq!(&buf[(at.x, at.y)], cell);
    assert_eq!(p.cursor_bg(), cell.bg);
}

#[test]
fn without_colour_the_cursor_is_reversed_and_designations_are_underlined() {
    let mut app = App::new(new_session(7, Nation::Earth), SIZE);
    let (tree, _, _) = designate_around_the_settlers(&mut app.session);
    let buf = frame(&mut app, ColorDepth::Never, false, SIZE);
    let cursor = cell_at(&app, &buf, app.ui.cursor);
    assert!(cursor.modifier.contains(Modifier::REVERSED));
    let cell = cell_at(&app, &buf, tree);
    assert!(cell.modifier.contains(Modifier::UNDERLINED), "{cell:?}");
    for cell in buf.content() {
        assert!(matches!(cell.fg, Color::Reset) && matches!(cell.bg, Color::Reset), "{cell:?}");
    }
}

#[test]
fn quantised_frames_only_use_the_depth_they_were_asked_for() {
    let mut app = App::new(new_session(7, Nation::Earth), SIZE);
    let buf = frame(&mut app, ColorDepth::Indexed256, false, SIZE);
    for c in buf.content() {
        assert!(!matches!(c.fg, Color::Rgb(..)) && !matches!(c.bg, Color::Rgb(..)), "{c:?}");
    }
    let buf = frame(&mut app, ColorDepth::Ansi16, false, SIZE);
    for c in buf.content() {
        assert!(
            !matches!(c.fg, Color::Rgb(..) | Color::Indexed(_))
                && !matches!(c.bg, Color::Rgb(..) | Color::Indexed(_)),
            "{c:?}"
        );
    }
    assert!(to_ansi(&buf).contains("\x1b["));
}

#[test]
fn items_on_the_ground_and_in_the_stockpile_show_as_item_glyphs() {
    let mut s = new_session(7, Nation::Earth);
    // Harvest every tree and rock near the settlers, and give them a stockpile to fill.
    let from = s.world.units[0].pos;
    let near: Vec<Pos> = s
        .world
        .map
        .iter()
        .filter(|(p, t)| {
            (p.x - from.x).abs() < 12
                && (p.y - from.y).abs() < 8
                && t.resource.is_some_and(|r| s.defs.harvest(r).is_some())
        })
        .map(|(p, _)| p)
        .collect();
    for p in near {
        let _ = s.apply(Command::DesignateHarvest { pos: p });
    }
    let zone = Pos::new(from.x, from.y + 2);
    for dx in 0..3 {
        let _ = s.apply(Command::DesignateStockpile { pos: Pos::new(zone.x + dx, zone.y) });
    }
    let mut seen = (false, false);
    for _ in 0..40 {
        s.tick_n(100);
        seen.0 |= s.world.ground_items().next().is_some();
        seen.1 |= s.world.stored_at(zone).is_some();
        if seen == (true, true) {
            break;
        }
    }
    assert!(seen.0, "something should have been dropped on the ground");

    let mut app = App::new(s, SIZE);
    app.ui.cursor = from;
    let p = palette(&app.session, ColorDepth::TrueColor, false);
    let buf = frame(&mut app, ColorDepth::TrueColor, false, SIZE);
    let units: Vec<Pos> = app.session.world.units.iter().map(|u| u.pos).collect();
    let mut checked = 0;
    for (pos, stack) in app.session.world.ground_items() {
        let (x, y) = (pos.x - app.ui.view.x, pos.y - app.ui.view.y);
        if units.contains(&pos) || pos == app.ui.cursor || x < 0 || y < 0 || x >= 88 || y >= 39 {
            continue;
        }
        let item = &app.session.defs.item(stack.item).id;
        let want = p.glyphs.item[item];
        let cell = cell_at(&app, &buf, pos);
        assert_eq!(cell.symbol(), want.glyph.to_string(), "{pos:?} holds {item}");
        assert_eq!(cell.fg, Color::Rgb(want.fg[0], want.fg[1], want.fg[2]));
        checked += 1;
    }
    assert!(checked > 0, "the scenario should put an item on view");
}

#[test]
fn a_lit_flame_draws_over_the_resource_and_warms_its_reach_under_the_cursor() {
    let mut s = Session::new(1, Nation::Fire, 48, 32);
    let pos = s.world.units[0].pos;
    let ability = fn_ui::tools::ability_for_nation(&s.defs, Nation::Fire).unwrap();
    s.apply(Command::Bend { ability, pos }).unwrap();
    for _ in 0..20_000 {
        s.tick();
        if s.world.is_lit(pos) && !s.world.units.iter().any(|u| u.pos == pos) {
            break;
        }
    }
    assert!(s.world.is_lit(pos), "the flame should finish lighting");
    assert!(!s.world.units.iter().any(|u| u.pos == pos), "no unit stands on the flame");
    let radius = s.world.flames()[0].radius as i32;

    let mut app = App::new(s, SIZE);
    let p = palette(&app.session, ColorDepth::TrueColor, false);
    app.ui.cursor = Pos::new(pos.x + 5, pos.y); // elsewhere
    let buf = frame(&mut app, ColorDepth::TrueColor, false, SIZE);
    let flame = cell_at(&app, &buf, pos);
    assert_eq!(flame.symbol(), "☼");
    let look = p.glyphs.overlay.flame;
    assert_eq!(flame.fg, Color::Rgb(look.fg[0], look.fg[1], look.fg[2]));

    // With the cursor on the flame, the tiles it warms get the warm background; others don't.
    app.ui.cursor = pos;
    let buf = frame(&mut app, ColorDepth::TrueColor, false, SIZE);
    let warm = Pos::new(pos.x + radius, pos.y);
    let cold = Pos::new(pos.x + radius + 1, pos.y);
    let warm_cell = cell_at(&app, &buf, warm);
    let cold_cell = cell_at(&app, &buf, cold);
    assert_eq!(warm_cell.bg, p.tinted(ground_bg(&p, &app.session, warm), Tint::Warm));
    assert_eq!(cold_cell.bg, p.color(ground_bg(&p, &app.session, cold)));
}

/// A renewable resource that has been worked and is growing back draws dimmed.
#[test]
fn a_resource_growing_back_is_dim_grey() {
    let mut s = new_session(7, Nation::Earth);
    let from = s.world.units[0].pos;
    let mut renewable: Vec<Pos> = s
        .world
        .map
        .iter()
        .filter(|(_, t)| {
            t.resource.and_then(|r| s.defs.harvest(r)).is_some_and(|h| h.regrow.is_some())
        })
        .map(|(p, _)| p)
        .collect();
    renewable.sort_by_key(|p| (p.x - from.x).pow(2) + (p.y - from.y).pow(2));
    let target = renewable
        .into_iter()
        .find(|&pos| s.apply(Command::DesignateHarvest { pos }).is_ok())
        .expect("a renewable resource the nation can work");
    for _ in 0..100 {
        s.tick_n(100);
        if s.world.is_resting(target) {
            break;
        }
    }
    assert!(s.world.is_resting(target), "the resource should have been worked");

    let mut app = App::new(s, SIZE);
    app.ui.cursor = target;
    app.ui.cursor.x = (target.x + 4).min(app.session.world.map.width() - 1);
    let p = palette(&app.session, ColorDepth::TrueColor, false);
    let buf = frame(&mut app, ColorDepth::TrueColor, false, SIZE);
    let cell = cell_at(&app, &buf, target);
    let dim = fn_tui::color::to_u8(p.theme.dim);
    let res = app.session.world.map.get(target).unwrap().resource.unwrap();
    let id = &app.session.defs.resource(res).id;
    if !app.session.world.units.iter().any(|u| u.pos == target) {
        assert_eq!(cell.symbol(), p.glyphs.resource[id].glyph.to_string());
        assert_eq!(cell.fg, Color::Rgb(dim[0], dim[1], dim[2]));
    }
}

/// A wet patch (Water the Land) is a dark mud background under whatever else is on the tile.
#[test]
fn a_wet_patch_is_a_mud_background() {
    let mut s = Session::new(2, Nation::Water, 48, 32);
    let ability = fn_ui::tools::ability_for_nation(&s.defs, Nation::Water).unwrap();
    let from = s.world.units[0].pos;
    // Water the nearest tile the sim accepts.
    let mut tiles: Vec<Pos> = s.world.map.iter().map(|(p, _)| p).collect();
    tiles.sort_by_key(|p| (p.x - from.x).pow(2) + (p.y - from.y).pow(2));
    let target = tiles
        .into_iter()
        .find(|&pos| s.apply(Command::Bend { ability, pos }).is_ok())
        .expect("a tile to water");
    for _ in 0..200 {
        s.tick_n(100);
        if !s.world.wet_tiles().is_empty() {
            break;
        }
    }
    let wet = s.world.wet_tiles();
    assert!(!wet.is_empty(), "the bend should finish and wet some tiles ({target:?})");

    let mut app = App::new(s, SIZE);
    let p = palette(&app.session, ColorDepth::TrueColor, false);
    let (pos, _) = wet[0];
    // The cursor beside it, so the cursor's own colour doesn't hide the patch.
    app.ui.cursor = Pos::new(pos.x, if pos.y > 0 { pos.y - 1 } else { 1 });
    let buf = frame(&mut app, ColorDepth::TrueColor, false, SIZE);
    let cell = cell_at(&app, &buf, pos);
    assert_eq!(cell.bg, p.tinted(ground_bg(&p, &app.session, pos), Tint::Wet));
    assert_ne!(cell.bg, p.color(ground_bg(&p, &app.session, pos)));
}
