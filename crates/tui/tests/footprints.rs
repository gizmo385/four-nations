//! Buildings that cover several tiles (#212, docs/design/buildings.md, "TUI"): a finished building
//! or a site is a block of its glyph, and the Build tool (`U`) shows the footprint it would place
//! as a ghost, green where it fits and red where something is in the way. Every building in the
//! data is 1x1 until #214, so these tests make the bed a 2x2 building.

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use fn_sim::building::{Building, BuildingId};
use fn_sim::geom::Footprint;
use fn_sim::world::Command;
use fn_sim::{Nation, Pos, Terrain};
use fn_tui::app::App;
use fn_tui::color::{ColorDepth, Palette};
use fn_tui::frame::{render_app, to_text};
use fn_tui::{layout, new_session};
use fn_ui::Tool;
use ratatui::buffer::{Buffer, Cell};
use ratatui::style::{Color, Modifier};

const SIZE: (u16, u16) = (120, 40);

/// Earth, seed 7, with the bed made a 2x2 building (and no longer a bed, which must be 1x1), and
/// a clear strip of land 6 tiles wide and 2 tall to the lower right of the first settler, whose
/// top-left tile is returned.
fn app_with_2x2_bed() -> (App, BuildingId, Pos) {
    let mut s = new_session(7, Nation::Earth);
    let bed = s.defs.building_id("bed").unwrap();
    let def = &mut s.defs.buildings[bed.0 as usize];
    def.footprint = (2, 2);
    def.bed = None;
    let from = s.world.units[0].pos;
    let at = Pos::new(from.x + 3, from.y + 3);
    for p in Footprint::new(at, 6, 2).tiles() {
        let t = s.world.map.get_mut(p).unwrap();
        t.terrain = Terrain::Land;
        t.resource = None;
    }
    s.world.units.retain(|u| !Footprint::new(at, 6, 2).contains(u.pos));
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
    assert!(x >= 0 && y >= 0 && x < i32::from(map.width) && y < i32::from(map.height));
    &buf[(map.x + x as u16, map.y + y as u16)]
}

fn ground_bg(p: &Palette, app: &App, pos: Pos) -> [u8; 3] {
    let tile = app.session.world.map.get(pos).unwrap();
    p.glyphs.biome[&app.session.defs.biome(tile.biome).id].bg.unwrap()
}

fn golden(name: &str, actual: &str) {
    let path = format!("{}/tests/golden/{name}.txt", env!("CARGO_MANIFEST_DIR"));
    if std::env::var_os("FN_BLESS").is_some() {
        std::fs::write(&path, actual).unwrap();
    }
    let expected = std::fs::read_to_string(&path).expect("golden missing; run with FN_BLESS=1");
    assert_eq!(actual, expected, "{name}: the frame changed; rebless with FN_BLESS=1 and review");
}

fn press(app: &mut App, c: char) {
    app.key(KeyEvent::new(KeyCode::Char(c), KeyModifiers::NONE));
}

/// The side panel's CURSOR section below its heading (which names the tile).
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
    press(&mut app, 'u');
    assert_eq!(app.ui.tool, Tool::Build);
    assert_eq!(app.ui.building, Some(bed), "the first building Earth can place");
    // A 2x2 grows down and right from the cursor.
    app.ui.cursor = at;
    let p = palette(&app, ColorDepth::TrueColor, false);
    let buf = frame(&mut app, ColorDepth::TrueColor, false);
    for pos in Footprint::new(at, 2, 2).tiles() {
        let cell = cell_at(&app, &buf, pos);
        assert_eq!(cell.bg, p.ghost(ground_bg(&p, &app, pos), true), "{pos:?}");
        assert_eq!(cell.symbol(), "Θ", "{pos:?}");
    }
    let text = to_text(&buf);
    let status = text.lines().last().unwrap();
    assert!(status.contains("build Bed  (U: next building)"), "{status}");
    golden("earth_seed7_ghost", &text);
}

#[test]
fn a_blocked_tile_is_red_and_the_status_line_says_why() {
    let (mut app, _, at) = app_with_2x2_bed();
    let corner = Pos::new(at.x + 1, at.y + 1);
    app.session.apply(Command::DesignateStockpile { pos: corner }).unwrap();
    press(&mut app, 'u');
    app.ui.cursor = at;
    let p = palette(&app, ColorDepth::TrueColor, false);
    let buf = frame(&mut app, ColorDepth::TrueColor, false);
    for pos in Footprint::new(at, 2, 2).tiles() {
        let want = p.ghost(ground_bg(&p, &app, pos), pos != corner);
        assert_eq!(cell_at(&app, &buf, pos).bg, want, "{pos:?}");
    }
    let text = to_text(&buf);
    let status = text.lines().last().unwrap();
    assert!(status.contains("Bed: can't build here: on a stockpile"), "{status}");

    // Without colour the ghost is reversed: bold where it fits, and where it doesn't, dim, with
    // the glyph of what is in the way.
    let buf = frame(&mut app, ColorDepth::Never, false);
    let free = cell_at(&app, &buf, Pos::new(at.x + 1, at.y));
    assert!(free.modifier.contains(Modifier::REVERSED | Modifier::BOLD));
    assert_eq!(free.symbol(), "Θ");
    let blocked = cell_at(&app, &buf, corner);
    assert!(blocked.modifier.contains(Modifier::REVERSED | Modifier::DIM));
    assert_ne!(blocked.symbol(), "Θ");
}

#[test]
fn a_drag_shows_a_ghost_per_stamp_and_enter_places_them() {
    let (mut app, bed, at) = app_with_2x2_bed();
    press(&mut app, 'u');
    app.ui.cursor = at;
    app.key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
    // Five tiles across fit two 2x2 stamps; the fifth column is left out.
    app.ui.cursor = Pos::new(at.x + 4, at.y + 1);
    let p = palette(&app, ColorDepth::TrueColor, false);
    let buf = frame(&mut app, ColorDepth::TrueColor, false);
    for anchor in [at, Pos::new(at.x + 2, at.y)] {
        for pos in Footprint::new(anchor, 2, 2).tiles() {
            if pos == app.ui.cursor {
                continue;
            }
            let cell = cell_at(&app, &buf, pos);
            assert_eq!(cell.bg, p.ghost(ground_bg(&p, &app, pos), true), "{pos:?}");
            assert_eq!(cell.symbol(), "Θ", "{pos:?}");
        }
    }
    let leftover = cell_at(&app, &buf, Pos::new(at.x + 4, at.y));
    assert_ne!(leftover.symbol(), "Θ", "no stamp fits the fifth column");
    assert_ne!(leftover.bg, Color::Reset);

    app.key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
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
fn u_again_moves_on_to_the_next_building_and_other_tools_keep_it() {
    let (mut app, bed, _) = app_with_2x2_bed();
    press(&mut app, 'u');
    assert_eq!(app.ui.building, Some(bed));
    press(&mut app, 'u');
    let next = app.ui.building;
    assert!(next.is_some());
    press(&mut app, 'h');
    assert_eq!(app.ui.tool, Tool::Harvest);
    assert!(app.ui.ghosts(&app.session).is_empty(), "no ghost outside the Build tool");
    press(&mut app, 'u');
    assert_eq!(app.ui.building, next, "U returns to the building last picked");
}
