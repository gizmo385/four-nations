//! Buildings that cover several tiles (#212, docs/design/buildings.md, "TUI"): a finished
//! building or a site is a block of its glyph, and the Build tool (`U`) shows the footprint it
//! would place as a ghost, green where it fits and red where something is in the way, with a
//! drag showing one ghost per stamp. Every building in the data is 1x1 until #214, so these
//! tests make the bed a 2x2 building to exercise the general code.

use fn_sim::building::{Building, BuildingId};
use fn_sim::geom::Footprint;
use fn_sim::world::Command;
use fn_sim::{Nation, Pos, Terrain};
use fn_tui::app::App;
use fn_tui::color::{ColorDepth, Palette, Tint};
use fn_tui::frame::{render_app, to_text};
use fn_tui::{keys, layout, new_session};
use fn_ui::Tool;
use ratatui::buffer::{Buffer, Cell};
use ratatui::style::Modifier;

const SIZE: (u16, u16) = (120, 40);

/// Earth, seed 7, with the bed made a 2x2 building (and no longer a bed, which must be 1x1), and
/// a clear strip of land 6 tiles wide and 2 tall near the first settler, whose top-left tile is
/// returned. The settler's cursor starts on its own tile.
fn app_with_2x2_bed() -> (App, BuildingId, Pos) {
    let mut s = new_session(7, Nation::Earth);
    let bed = s.defs.building_id("bed").unwrap();
    let def = &mut s.defs.buildings[bed.0 as usize];
    def.footprint = (2, 2);
    def.bed = None;
    let from = s.world.units[0].pos;
    let at = Pos::new(from.x + 3, from.y + 3);
    let strip = Footprint::new(at, 6, 2);
    for p in strip.tiles() {
        let t = s.world.map.get_mut(p).unwrap();
        t.terrain = Terrain::Land;
        t.resource = None;
    }
    s.world.units.retain(|u| !strip.contains(u.pos));
    let mut app = App::new(s, SIZE);
    app.ui.cursor = from;
    (app, bed, at)
}

/// Bakes a finished `building` of `size` onto every tile of its footprint at `at`, as the sim
/// does when a site completes.
fn bake(app: &mut App, building: BuildingId, at: Pos, size: (u8, u8)) {
    for p in Footprint::new(at, size.0, size.1).tiles() {
        let part = [(p.x - at.x) as u8, (p.y - at.y) as u8];
        let b = Building { part, size: [size.0, size.1], ..Building::new(building, true, false) };
        app.session.world.map.get_mut(p).unwrap().building = Some(b);
    }
}

fn palette(app: &App, depth: ColorDepth, ascii: bool) -> Palette {
    Palette::new(&app.session.defs, app.session.world.nation, depth, ascii)
}

fn frame(app: &mut App, depth: ColorDepth, ascii: bool) -> Buffer {
    let p = palette(app, depth, ascii);
    render_app(app, &p, SIZE)
}

fn cell_at<'b>(app: &App, buf: &'b Buffer, pos: Pos) -> &'b Cell {
    let map = layout::regions(buf.area).unwrap().map;
    let (x, y) = (pos.x - app.ui.view.x, pos.y - app.ui.view.y);
    assert!(x >= 0 && y >= 0 && x < i32::from(map.width) && y < i32::from(map.height), "{pos:?}");
    &buf[(map.x + x as u16, map.y + y as u16)]
}

fn ground_bg(p: &Palette, app: &App, pos: Pos) -> [u8; 3] {
    let tile = app.session.world.map.get(pos).unwrap();
    match tile.terrain {
        Terrain::Land => p.glyphs.biome[&app.session.defs.biome(tile.biome).id].bg.unwrap(),
        other => p.glyphs.terrain(other).unwrap().bg.unwrap(),
    }
}

fn golden(name: &str, actual: &str) {
    let path = format!("{}/tests/golden/{name}.txt", env!("CARGO_MANIFEST_DIR"));
    if std::env::var_os("FN_BLESS").is_some() {
        std::fs::write(&path, actual).unwrap();
    }
    let expected = std::fs::read_to_string(&path).expect("golden missing; run with FN_BLESS=1");
    assert_eq!(actual, expected, "{name}: the frame changed; rebless with FN_BLESS=1 and review");
}

/// The side panel's CURSOR section, below its heading (which names the tile).
fn cursor_section(app: &mut App) -> String {
    let text = to_text(&frame(app, ColorDepth::TrueColor, false));
    let panel = layout::regions(ratatui::layout::Rect::new(0, 0, SIZE.0, SIZE.1)).unwrap().panel;
    let rows: Vec<String> = text
        .lines()
        .map(|l| l.chars().skip(usize::from(panel.x)).collect::<String>().trim_end().to_owned())
        .collect();
    let start = rows.iter().position(|r| r.contains("CURSOR")).expect("a CURSOR section");
    rows[start + 1..].join("\n")
}

#[test]
fn a_2x2_building_and_a_2x2_site_are_blocks_of_their_glyph() {
    let (mut app, bed, at) = app_with_2x2_bed();
    bake(&mut app, bed, at, (2, 2));
    let site = Pos::new(at.x + 3, at.y);
    app.session.apply(Command::DesignateBuild { pos: site, building: bed }).unwrap();
    golden("earth_seed7_footprints", &to_text(&frame(&mut app, ColorDepth::TrueColor, false)));

    for ascii in [false, true] {
        let p = palette(&app, ColorDepth::TrueColor, ascii);
        let look = &p.glyphs.building["bed"];
        let buf = frame(&mut app, ColorDepth::TrueColor, ascii);
        for pos in Footprint::new(at, 2, 2).tiles() {
            let cell = cell_at(&app, &buf, pos);
            assert_eq!(cell.symbol(), look.glyph(ascii).to_string(), "built {pos:?}");
            assert_eq!(cell.fg, p.color(look.fg), "built {pos:?}");
        }
        for pos in Footprint::new(site, 2, 2).tiles() {
            let cell = cell_at(&app, &buf, pos);
            assert_eq!(cell.symbol(), look.glyph(ascii).to_string(), "site {pos:?}");
            assert_ne!(cell.fg, p.color(look.fg), "a site is dimmed: {pos:?}");
        }
        // Beside the block, the ground.
        assert_ne!(
            cell_at(&app, &buf, Pos::new(at.x + 2, at.y)).symbol(),
            look.glyph(ascii).to_string()
        );
    }
}

#[test]
fn the_cursor_section_says_the_same_from_every_tile_of_a_footprint() {
    let (mut app, bed, at) = app_with_2x2_bed();
    bake(&mut app, bed, at, (2, 2));
    let site = Pos::new(at.x + 3, at.y);
    app.session.apply(Command::DesignateBuild { pos: site, building: bed }).unwrap();
    for anchor in [at, site] {
        app.ui.cursor = anchor;
        let want = cursor_section(&mut app);
        assert!(want.contains("Bed"), "{want}");
        for pos in Footprint::new(anchor, 2, 2).tiles() {
            app.ui.cursor = pos;
            assert_eq!(cursor_section(&mut app), want, "{pos:?}");
        }
    }
}

#[test]
fn u_picks_the_build_tool_and_its_ghost_is_green_where_the_building_fits() {
    let (mut app, bed, at) = app_with_2x2_bed();
    app.play(&keys::parse("u").unwrap());
    assert_eq!(app.ui.tool, Tool::Build);
    assert_eq!(app.ui.choice, Some(bed), "the first building Earth can place");
    // A 2x2 grows down and right from the cursor.
    app.ui.cursor = at;
    let p = palette(&app, ColorDepth::TrueColor, false);
    let buf = frame(&mut app, ColorDepth::TrueColor, false);
    for pos in Footprint::new(at, 2, 2).tiles() {
        let cell = cell_at(&app, &buf, pos);
        assert_eq!(cell.bg, p.ghost(ground_bg(&p, &app, pos), true), "{pos:?}");
        assert_eq!(cell.symbol(), p.glyphs.building["bed"].glyph(false).to_string(), "{pos:?}");
    }
    let text = to_text(&buf);
    let status = text.lines().last().unwrap();
    assert!(status.contains("build Bed"), "{status}");
    golden("earth_seed7_ghost", &text);
}

#[test]
fn a_blocked_tile_is_red_and_mono_keeps_what_is_in_the_way() {
    let (mut app, _, at) = app_with_2x2_bed();
    let corner = Pos::new(at.x + 1, at.y + 1);
    app.session.apply(Command::DesignateStockpile { pos: corner }).unwrap();
    app.play(&keys::parse("u").unwrap());
    app.ui.cursor = at;
    let p = palette(&app, ColorDepth::TrueColor, false);
    let buf = frame(&mut app, ColorDepth::TrueColor, false);
    for pos in Footprint::new(at, 2, 2).tiles() {
        let want = p.ghost(ground_bg(&p, &app, pos), pos != corner);
        assert_eq!(cell_at(&app, &buf, pos).bg, want, "{pos:?}");
    }

    // Without colour the ghost is reversed: bold where it fits, and where it doesn't, dim, with
    // the glyph of what is in the way (here, nothing, so the ground glyph).
    let buf = frame(&mut app, ColorDepth::Never, false);
    let free = cell_at(&app, &buf, Pos::new(at.x + 1, at.y));
    assert!(free.modifier.contains(Modifier::REVERSED | Modifier::BOLD));
    let blocked = cell_at(&app, &buf, corner);
    assert!(blocked.modifier.contains(Modifier::REVERSED | Modifier::DIM));
}

#[test]
fn a_drag_shows_a_ghost_per_stamp_and_enter_places_them() {
    let (mut app, bed, at) = app_with_2x2_bed();
    app.play(&keys::parse("u").unwrap());
    app.ui.cursor = at;
    app.play(&keys::parse("<enter>").unwrap());
    // The strip is 6 wide: two 2x2 stamps fit across it, the fifth and sixth columns left out.
    app.ui.cursor = Pos::new(at.x + 4, at.y + 1);
    let p = palette(&app, ColorDepth::TrueColor, false);
    let buf = frame(&mut app, ColorDepth::TrueColor, false);
    for anchor in [at, Pos::new(at.x + 2, at.y)] {
        for pos in Footprint::new(anchor, 2, 2).tiles() {
            let cell = cell_at(&app, &buf, pos);
            assert_eq!(cell.bg, p.ghost(ground_bg(&p, &app, pos), true), "{pos:?}");
        }
    }
    // No stamp covers the fifth column: it's still inside the dragged rectangle, so it wears the
    // drag's plain hue instead of a ghost.
    let leftover_pos = Pos::new(at.x + 4, at.y);
    let leftover = cell_at(&app, &buf, leftover_pos);
    assert_eq!(leftover.bg, p.dragged(ground_bg(&p, &app, leftover_pos), Tint::Build));

    app.play(&keys::parse("<enter>").unwrap());
    let sites: Vec<Pos> = app
        .session
        .world
        .build_sites()
        .filter(|(_, b, _)| *b == bed)
        .map(|(f, ..)| f.anchor)
        .collect();
    assert_eq!(sites, vec![at, Pos::new(at.x + 2, at.y)]);
    assert!(app.ui.notice.contains("2 done"), "{}", app.ui.notice);
}

#[test]
fn no_ghost_outside_the_build_tool_or_with_nothing_chosen() {
    let (mut app, _, at) = app_with_2x2_bed();
    assert!(app.ui.ghosts(&app.session).is_empty(), "Select mode, nothing chosen yet");
    // The cursor stays away from `at`, so its own highlight doesn't stand in for the ghost's.
    let buf = frame(&mut app, ColorDepth::TrueColor, false);
    let p = palette(&app, ColorDepth::TrueColor, false);
    assert_eq!(cell_at(&app, &buf, at).bg, p.color(ground_bg(&p, &app, at)));
}
