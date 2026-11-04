//! Rail draws as joined track (#279): a rail tile or site picks the box-drawing piece that joins
//! the rail tiles beside it (built or still a site), so a line of rail reads as one track and a
//! corner or a junction turns, instead of every tile wearing the same glyph.

use fn_sim::building::{Building, BuildingId};
use fn_sim::world::Command;
use fn_sim::{Nation, Pos, Terrain};
use fn_tui::app::App;
use fn_tui::color::{ColorDepth, Palette};
use fn_tui::frame::{render_app, to_text};
use fn_tui::new_session;
use ratatui::buffer::{Buffer, Cell};

const SIZE: (u16, u16) = (120, 40);

/// Earth, seed 7, with a clear 5x5 patch of land near the first settler, whose top-left tile is
/// returned, and rail's tech requirement lifted so a site can be designated without researching
/// it first (like `footprints.rs` lifts the bed's footprint).
fn app_with_clear_patch() -> (App, BuildingId, Pos) {
    let mut s = new_session(7, Nation::Earth);
    let rail = s.defs.building_id("rail").unwrap();
    s.defs.buildings[rail.0 as usize].unlocked_by = None;
    let from = s.world.units[0].pos;
    let at = Pos::new(from.x + 3, from.y + 3);
    for dy in 0..5 {
        for dx in 0..5 {
            let t = s.world.map.get_mut(Pos::new(at.x + dx, at.y + dy)).unwrap();
            t.terrain = Terrain::Land;
            t.resource = None;
        }
    }
    s.world.units.retain(|u| {
        !(u.pos.x >= at.x && u.pos.x < at.x + 5 && u.pos.y >= at.y && u.pos.y < at.y + 5)
    });
    let app = App::new(s, SIZE);
    (app, rail, at)
}

/// Bakes a finished, 1x1 rail tile at `pos`, as the sim does when its site completes.
fn bake_rail(app: &mut App, rail: BuildingId, pos: Pos) {
    let passable = app.session.defs.building(rail).passable;
    app.session.world.map.get_mut(pos).unwrap().building =
        Some(Building::new(rail, passable, true));
}

fn palette(app: &App, ascii: bool) -> Palette {
    Palette::new(&app.session.defs, app.session.world.nation, ColorDepth::TrueColor, ascii)
}

fn frame(app: &mut App, ascii: bool) -> Buffer {
    let p = palette(app, ascii);
    render_app(app, &p, SIZE)
}

fn cell_at<'b>(app: &App, buf: &'b Buffer, pos: Pos) -> &'b Cell {
    let map = fn_tui::layout::regions(buf.area).unwrap().map;
    let (x, y) = (pos.x - app.ui.view.x, pos.y - app.ui.view.y);
    assert!(x >= 0 && y >= 0 && x < i32::from(map.width) && y < i32::from(map.height), "{pos:?}");
    &buf[(map.x + x as u16, map.y + y as u16)]
}

fn golden(name: &str, actual: &str) {
    let path = format!("{}/tests/golden/{name}.txt", env!("CARGO_MANIFEST_DIR"));
    if std::env::var_os("FN_BLESS").is_some() {
        std::fs::write(&path, actual).unwrap();
    }
    let expected = std::fs::read_to_string(&path).expect("golden missing; run with FN_BLESS=1");
    assert_eq!(actual, expected, "{name}: the frame changed; rebless with FN_BLESS=1 and review");
}

/// A ring of rail with a straight run, two corners and a three-way junction (#279), laid out
/// relative to `at`:
///
/// ```text
/// R . .
/// R R R
/// R . R
/// ```
///
/// (`at.x, at.y`) is a dead-end straight piece (only its south neighbour is rail); the middle row
/// is a run with a junction where the vertical meets it; the bottom row closes the loop with two
/// more corners. Every mask from 1 to 15 that appears is exercised somewhere in this shape.
fn ring(at: Pos) -> Vec<Pos> {
    [(0, 0), (0, 1), (1, 1), (2, 1), (1, 2), (2, 2)]
        .into_iter()
        .map(|(dx, dy)| Pos::new(at.x + dx, at.y + dy))
        .collect()
}

/// The glyph a rail or rail site at `pos` draws, after rendering `app`.
fn glyph_at(app: &mut App, ascii: bool, pos: Pos) -> String {
    let buf = frame(app, ascii);
    cell_at(app, &buf, pos).symbol().to_owned()
}

#[test]
fn a_rail_line_joins_into_one_track_with_corners_and_a_junction() {
    let (mut app, rail, at) = app_with_clear_patch();
    for pos in ring(at) {
        bake_rail(&mut app, rail, pos);
    }

    // (0,0): only its south neighbour (0,1) is rail -> a straight vertical piece (no stub glyph
    // exists for a dead end).
    assert_eq!(glyph_at(&mut app, false, Pos::new(at.x, at.y)), "│");
    // (0,1): north (0,0) and east (1,1) are rail -> a corner turning the line east.
    assert_eq!(glyph_at(&mut app, false, Pos::new(at.x, at.y + 1)), "└");
    // (1,1): west (0,1), east (2,1) and south (1,2) are rail -> a three-way junction.
    assert_eq!(glyph_at(&mut app, false, Pos::new(at.x + 1, at.y + 1)), "┬");
    // (2,1): west (1,1) and south (2,2) are rail -> a corner turning the line south.
    assert_eq!(glyph_at(&mut app, false, Pos::new(at.x + 2, at.y + 1)), "┐");
    // (1,2): north (1,1) and east (2,2) are rail -> a corner turning the line east.
    assert_eq!(glyph_at(&mut app, false, Pos::new(at.x + 1, at.y + 2)), "└");
    // (2,2): north (2,1) and west (1,2) are rail -> a corner closing the loop.
    assert_eq!(glyph_at(&mut app, false, Pos::new(at.x + 2, at.y + 2)), "┘");

    // --ascii picks from `- | +` instead (#279): straight pieces keep `|`/`-`, every corner and
    // the junction become `+`.
    assert_eq!(glyph_at(&mut app, true, Pos::new(at.x, at.y)), "|");
    assert_eq!(glyph_at(&mut app, true, Pos::new(at.x, at.y + 1)), "+");
    assert_eq!(glyph_at(&mut app, true, Pos::new(at.x + 1, at.y + 1)), "+");

    golden("earth_seed7_rail", &to_text(&frame(&mut app, false)));
}

/// A lone rail tile, with no rail neighbour, keeps the rail building's own glyph (#279) rather
/// than drawing a box-drawing piece.
#[test]
fn a_lone_rail_tile_keeps_the_rail_buildings_own_glyph() {
    let (mut app, rail, at) = app_with_clear_patch();
    bake_rail(&mut app, rail, at);
    let p = palette(&app, false);
    let want = p.glyphs.building[&app.session.defs.building(rail).id].glyph(false);
    assert_eq!(glyph_at(&mut app, false, at), want.to_string());
}

/// A rail site joins the rail beside it the same as a finished tile does (#279): the sim need
/// not finish building it first for the track to read as one line.
#[test]
fn a_rail_site_joins_the_finished_rail_beside_it() {
    let (mut app, rail, at) = app_with_clear_patch();
    bake_rail(&mut app, rail, at);
    let site = Pos::new(at.x, at.y + 1);
    app.session.apply(Command::DesignateBuild { pos: site, building: rail }).unwrap();

    // The finished tile now has a rail neighbour that is only a site, and the site has a
    // finished rail neighbour: both draw the corner-free straight piece their one link gives.
    assert_eq!(glyph_at(&mut app, false, at), "│");
    assert_eq!(glyph_at(&mut app, false, site), "│");
}
