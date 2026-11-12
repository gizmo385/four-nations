//! The schools' readouts (#730, docs/design/horizons-phase-4.md): the build entries' effect
//! lines, the inspect lines, the sheltered tint while a ring wall is in hand, the power tint
//! along a live rail run, and `O` on a machine in an automating yard.

use fn_sim::Nation;
use fn_sim::geom::{Footprint, Pos};
use fn_tui::app::App;
use fn_tui::color::{ColorDepth, Palette};
use fn_tui::map::{Mark, Overlays, cell_look};
use fn_tui::{keys, new_session};
use fn_ui::Tool;
use fn_ui::inspect::{Line, tile_lines};

use crate::schools::learn;

fn app(nation: Nation) -> App {
    App::new(new_session(7, nation), (120, 40))
}

fn effects(app: &App, id: &str) -> Vec<String> {
    let b = app.session.defs.building_id(id).unwrap();
    fn_ui::build::building_detail(&app.session, b).effects
}

fn details(app: &App, at: Pos) -> Vec<String> {
    tile_lines(&app.session, Tool::Select, at)
        .unwrap()
        .into_iter()
        .filter_map(|l| match l {
            Line::Detail(d) => Some(d),
            _ => None,
        })
        .collect()
}

fn put(app: &mut App, id: &str, at: Pos) {
    let defs = app.session.defs.clone();
    let b = defs.building_id(id).unwrap();
    let (w, h) = defs.building(b).footprint;
    app.session.world.put_building(Footprint::new(at, w, h), b, &defs);
}

/// A square ring of Earth Wall from `lo` to `hi` on both axes, offset from the first settler.
fn ring(app: &mut App, lo: i32, hi: i32) -> Pos {
    let home = app.session.world.units[0].pos;
    for i in lo..=hi {
        for p in [(i, lo), (i, hi), (lo, i), (hi, i)] {
            put(app, "earth_wall", Pos::new(home.x + p.0, home.y + p.1));
        }
    }
    Pos::new(home.x + (lo + hi) / 2, home.y + (lo + hi) / 2)
}

#[test]
fn build_entries_say_what_each_school_gives() {
    let mut a = app(Nation::Earth);
    assert!(!effects(&a, "earth_wall").iter().any(|e| e.contains("ring")), "not before the tech");
    learn(&mut a, &["agrarian_ring"]);
    assert!(effects(&a, "earth_wall").contains(&"A closed ring shelters what's inside".into()));
    assert!(effects(&a, "ring_gate").contains(&"People pass, and it closes a ring".into()));
    let rail = effects(&a, "metal_rail");
    assert!(rail.contains(&"Carries power to buildings beside it".into()), "{rail:?}");
    assert!(rail.contains(&"Can be laid in tunnels".into()), "{rail:?}");

    let mut f = app(Nation::Fire);
    learn(&mut f, &["sun_warriors", "great_works"]);
    let fire = effects(&f, "campfire");
    assert!(fire.contains(&"Flames heal 2x and restore chi 1.5x nearby".into()), "{fire:?}");
    assert!(effects(&f, "hothouse").contains(&"Needs no fuel".into()));
    let station = effects(&f, "lightning_station");
    assert!(
        station
            .contains(&"Machines within 4 tiles run with no crafter, as fast as a crafter".into()),
        "{station:?}"
    );
}

#[test]
fn a_tile_inside_a_ring_says_so_and_the_wall_tool_tints_it() {
    let mut a = app(Nation::Earth);
    learn(&mut a, &["earthen_walls", "agrarian_ring"]);
    let inside = ring(&mut a, 3, 9);
    let lines = details(&a, inside);
    assert!(lines.contains(&"Sheltered: inside a ring of 25 tiles".to_owned()), "{lines:?}");

    let palette =
        Palette::new(&a.session.defs, a.session.world.nation, ColorDepth::TrueColor, false);
    let marks = |a: &App| {
        let over = Overlays::gather(&a.session, &a.ui);
        cell_look(&a.session, &a.ui, &palette, &over, inside).unwrap().marks
    };
    assert!(!marks(&a).contains(&Mark::Sheltered), "only with a wall in hand");
    a.ui.choice = a.session.defs.building_id("earth_wall");
    a.ui.set_tool(Tool::Build);
    assert!(marks(&a).contains(&Mark::Sheltered));
}

#[test]
fn a_consumer_on_a_rail_run_reads_powered_by_rail() {
    let mut a = app(Nation::Earth);
    learn(&mut a, &["metal_city"]);
    let home = a.session.world.units[0].pos;
    let mill = Pos::new(home.x - 12, home.y + 4);
    put(&mut a, "hot_spring_mill", mill);
    for x in (mill.x + 2)..(mill.x + 22) {
        put(&mut a, "metal_rail", Pos::new(x, mill.y));
    }
    let forge = Pos::new(mill.x + 18, mill.y + 1);
    put(&mut a, "forge", forge);
    let lines = details(&a, forge);
    let want = format!("Powered x1.5 by rail from Hot Spring Mill ({}, {})", mill.x, mill.y);
    assert!(lines.contains(&want), "{lines:?}");
}

#[test]
fn o_puts_a_machine_in_a_yard_on_automation_and_back() {
    let mut a = app(Nation::Fire);
    learn(&mut a, &["great_works"]);
    let home = a.session.world.units[0].pos;
    let station = Pos::new(home.x + 6, home.y + 6);
    put(&mut a, "lightning_station", station);
    let forge = Pos::new(station.x + 4, station.y);
    put(&mut a, "forge", forge);
    let line = |a: &App| fn_ui::inspect::automation_line(&a.session.world, &a.session.defs, forge);
    assert_eq!(line(&a).as_deref(), Some("Automated: waiting for inputs"));
    a.ui.cursor = forge;
    a.play(&keys::parse("o").unwrap());
    assert_eq!(line(&a).as_deref(), Some("Automation off: O to turn it on"));
    assert!(a.ui.notice.contains("back to a crafter"), "{}", a.ui.notice);
    a.play(&keys::parse("o").unwrap());
    assert_eq!(line(&a).as_deref(), Some("Automated: waiting for inputs"));
}
