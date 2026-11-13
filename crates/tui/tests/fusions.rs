//! Celestial days and the fusion readouts (#763, docs/design/horizons-phase-6.md, "Mockups"): the
//! Almanac's celestial lines, the aqueduct's irrigation tint, the Sky Lift's link line, the
//! Steam Engine's bank lines, the Airship screen and its confirm box, joint bends in the bend
//! picker, and the rain tint. Frames are checked with `contains`, not goldens.

use fn_sim::geom::{Footprint, Pos};
use fn_sim::world::{Command, UnitId};
use fn_sim::{Nation, Terrain};
use fn_tui::app::App;
use fn_tui::color::{ColorDepth, Palette};
use fn_tui::frame::{render_app, to_text};
use fn_tui::map::{Mark, Overlays, cell_look};
use fn_tui::state::Screen;
use fn_tui::{keys, new_session};
use fn_ui::Tool;
use fn_ui::inspect::{Line, tile_lines};

use crate::schools::learn;

const SIZE: (u16, u16) = (120, 40);

fn app(nation: Nation) -> App {
    App::new(new_session(7, nation), SIZE)
}

fn play(app: &mut App, script: &str) {
    app.play(&keys::parse(script).expect("a valid script"));
}

fn frame(app: &mut App) -> String {
    let p = Palette::new(&app.session.defs, app.session.world.nation, ColorDepth::TrueColor, true);
    to_text(&render_app(app, &p, SIZE))
}

fn put(app: &mut App, id: &str, at: Pos) {
    let defs = app.session.defs.clone();
    let b = defs.building_id(id).unwrap();
    let (w, h) = defs.building(b).footprint;
    app.session.world.put_building(Footprint::new(at, w, h), b, &defs);
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

fn effects(app: &App, id: &str) -> Vec<String> {
    let b = app.session.defs.building_id(id).unwrap();
    fn_ui::build::building_detail(&app.session, b).effects
}

/// Clears a `w` x `h` patch of open land at `at`, with nobody standing on it.
fn clear(app: &mut App, at: Pos, w: i32, h: i32) {
    let world = &mut app.session.world;
    for y in at.y..at.y + h {
        for x in at.x..at.x + w {
            let t = &mut world.map[Pos::new(x, y)];
            t.terrain = Terrain::Land;
            t.resource = None;
        }
    }
    world.units.retain(|u| {
        !(at.x - 1..=at.x + w).contains(&u.pos.x) || u.pos.y < at.y - 1 || u.pos.y > at.y + h
    });
    world.map_changed();
}

/// A spot of open ground well away from the first settler's own tile.
fn site(app: &App) -> Pos {
    let home = app.session.world.units[0].pos;
    Pos::new((home.x + 8).min(app.session.world.map.width() - 40), home.y.max(8))
}

fn palette(app: &App) -> Palette {
    Palette::new(&app.session.defs, app.session.world.nation, ColorDepth::TrueColor, false)
}

fn marks(app: &App, at: Pos) -> Vec<Mark> {
    let over = Overlays::gather(&app.session, &app.ui);
    cell_look(&app.session, &app.ui, &palette(app), &over, at).unwrap().marks
}

/// A Water city with `fire` Fire residents and `air` Air residents, the rest Water's own.
fn water_city(fire: usize, air: usize) -> App {
    let mut a = app(Nation::Water);
    let world = &mut a.session.world;
    while world.units.len() < 12 {
        let mut u = world.units[0].clone();
        u.id = UnitId(1000 + world.units.len() as u32);
        world.units.push(u);
    }
    for (i, u) in world.units.iter_mut().enumerate() {
        u.nation = match i {
            i if i < fire => Some(Nation::Fire),
            i if i < fire + air => Some(Nation::Air),
            _ => None,
        };
    }
    a
}

#[test]
fn the_almanac_lists_the_celestial_days_once_star_charts_is_known() {
    let mut a = water_city(3, 0);
    learn(&mut a, &["calendar"]);
    play(&mut a, "n");
    let before = frame(&mut a);
    assert!(before.contains("ALMANAC") && !before.contains("benders:"), "{before}");
    play(&mut a, "<esc>");

    learn(&mut a, &["calendar", "star_charts"]);
    let lines = fn_ui::view::almanac_lines(&a.session);
    let water = a.session.world.units.iter().filter(|u| u.nation.is_none()).count();
    assert!(
        lines.iter().any(|l| l.starts_with("The harvest moon in ")
            && l.ends_with(&format!(". Water benders: {water}"))),
        "{lines:?}"
    );
    assert!(
        lines
            .iter()
            .any(|l| l.starts_with("The summer solstice in ") && l.ends_with(". Fire benders: 3")),
        "{lines:?}"
    );
    assert!(lines.iter().all(|l| !l.contains("Air benders")), "no Air resident: {lines:?}");
    // Soonest first: the solstice (summer) before the moon (autumn).
    let at = |name: &str| lines.iter().position(|l| l.starts_with(name)).unwrap();
    assert!(at("The summer solstice") < at("The harvest moon"), "{lines:?}");
    play(&mut a, "n");
    let shown = frame(&mut a);
    assert!(shown.contains("The harvest moon in ") && shown.contains("Fire benders: 3"), "{shown}");
    play(&mut a, "<esc>");

    // On the day itself the running day comes first, with its time left.
    let defs = a.session.defs.clone();
    let day = defs.seasons.celestial.days.iter().find(|d| d.id == "summer_solstice").unwrap();
    let start = defs.seasons.celestial_start(day);
    a.session.world.tick = start + 1;
    let lines = fn_ui::view::almanac_lines(&a.session);
    // After the frost line, before the others.
    assert!(
        lines[1].starts_with("The summer solstice: ")
            && lines[1].ends_with(" left. Fire benders: 3"),
        "{lines:?}"
    );
    assert!(lines[2].starts_with("The harvest moon in "), "{lines:?}");
}

#[test]
fn an_aqueduct_under_the_cursor_tints_the_plots_it_irrigates() {
    let mut a = app(Nation::Earth);
    let at = site(&a);
    clear(&mut a, at, 14, 9);
    for y in at.y..at.y + 9 {
        a.session.world.map[Pos::new(at.x, y)].terrain = Terrain::River;
    }
    a.session.world.map_changed();
    for x in at.x + 1..at.x + 6 {
        put(&mut a, "aqueduct", Pos::new(x, at.y + 4));
    }
    // Flowing, it reads so, and the reach shows only while the cursor is on it.
    let end = Pos::new(at.x + 5, at.y + 4);
    let lines = details(&a, end);
    assert!(lines.iter().any(|l| l.starts_with("Flowing: 5 tiles")), "{lines:?}");
    let near = Pos::new(at.x + 3, at.y + 6);
    let far = Pos::new(at.x + 3, at.y + 7);
    a.ui.go_to(Pos::new(at.x + 12, at.y));
    assert!(!marks(&a, near).contains(&Mark::Irrigated));
    a.ui.go_to(end);
    assert!(marks(&a, near).contains(&Mark::Irrigated), "two tiles from the run");
    assert!(!marks(&a, far).contains(&Mark::Irrigated), "three tiles is out of reach");
    // A dry run waters nothing: take the river away from its source.
    for y in at.y..at.y + 9 {
        a.session.world.map[Pos::new(at.x, y)].terrain = Terrain::Land;
    }
    a.session.world.map_changed();
    assert!(!marks(&a, near).contains(&Mark::Irrigated), "a dry aqueduct irrigates nothing");
}

#[test]
fn a_sky_lift_shows_its_partner_link_line_and_riders() {
    let mut a = app(Nation::Earth);
    let at = site(&a);
    clear(&mut a, at, 30, 5);
    let (one, two) = (Pos::new(at.x + 1, at.y + 1), Pos::new(at.x + 21, at.y + 1));
    put(&mut a, "sky_lift", one);
    put(&mut a, "sky_lift", two);
    a.session.apply(Command::SetLink { pos: one, to: two }).unwrap();
    let lines = details(&a, one);
    let partner = format!("Partner: ({}, {}), 20 tiles", two.x, two.y);
    assert!(lines.contains(&partner), "{lines:?}");
    assert!(lines.contains(&"Riders: 0".to_owned()), "{lines:?}");
    a.ui.go_to(one);
    let over = Overlays::gather(&a.session, &a.ui);
    let look =
        cell_look(&a.session, &a.ui, &palette(&a), &over, Pos::new(at.x + 11, at.y + 1)).unwrap();
    assert_eq!(look.thing, fn_tui::map::Thing::Link, "the `:` line runs between the ends");
    assert!(
        effects(&a, "sky_lift").contains(
            &"Carries people and their loads to its partner up to 32 tiles away".to_owned()
        )
    );
}

#[test]
fn a_steam_engine_on_the_bank_burns_nothing_and_says_so() {
    let mut a = app(Nation::Fire);
    let before = effects(&a, "steam_engine");
    assert!(before.iter().all(|e| !e.contains("On the bank")), "{before:?}");
    learn(&mut a, &["steam_engines", "steam_power"]);
    let want = "On the bank: burns nothing, reaches 4 tiles";
    let power = effects(&a, "steam_engine");
    let reach = a.session.world.modifiers().power_generation;
    let expect =
        format!("On the bank: burns nothing, reaches {} tiles", (4.0 * reach).round() as u32);
    assert!(power.contains(&expect), "{power:?} (the plain {want} is at modifier 1.0)");

    let at = site(&a);
    clear(&mut a, at, 14, 8);
    for y in at.y..at.y + 8 {
        a.session.world.map[Pos::new(at.x, y)].terrain = Terrain::River;
    }
    a.session.world.map_changed();
    let bank = Pos::new(at.x + 1, at.y + 1);
    let inland = Pos::new(at.x + 8, at.y + 1);
    put(&mut a, "steam_engine", bank);
    put(&mut a, "steam_engine", inland);
    assert!(
        details(&a, bank).contains(&"On the bank: no fuel".to_owned()),
        "{:?}",
        details(&a, bank)
    );
    assert!(details(&a, inland).iter().all(|l| !l.contains("On the bank")));
    // Its reach is the larger of its own and the bank's.
    let reach_of = |a: &App, p: Pos| a.session.world.power_def(p, &a.session.defs).unwrap().radius;
    assert_eq!((reach_of(&a, bank), reach_of(&a, inland)), (4, 3));
}

/// A Fire game with a finished Airship Dock on open ground, the cursor on it.
fn dock_app() -> (App, Pos) {
    let mut a = app(Nation::Fire);
    let at = site(&a);
    clear(&mut a, at, 8, 8);
    let dock = Pos::new(at.x + 2, at.y + 2);
    put(&mut a, "airship_dock", dock);
    a.ui.go_to(Pos::new(dock.x + 1, dock.y + 1));
    (a, dock)
}

#[test]
fn enter_on_a_dock_opens_the_airship_screen_and_the_box_sends_a_voyage() {
    let (mut a, dock) = dock_app();
    assert!(details(&a, dock).contains(&"Docked".to_owned()), "{:?}", details(&a, dock));
    play(&mut a, "<enter>");
    assert_eq!(a.ui.screen, Some(Screen::Airship));
    let shown = frame(&mut a);
    for want in [
        "AIRSHIP",
        "Airship Dock at (",
        "Docked",
        "Crew: 2 residents",
        "SAIL TO",
        "Earth Kingdom",
        "Fire Nation (kin)",
        "DEALS",
        "50 wood for 8 gold ore",
        "In stores",
        "HOLD",
        "Pick a deal to sail with",
        "Up/Down choose",
        "Left/Right nation or deal",
        "+/- times",
        "Enter load and sail",
    ] {
        assert!(shown.contains(want), "{want:?} missing from\n{shown}");
    }
    // Nothing picked: Enter refuses and no box opens.
    play(&mut a, "<enter>");
    assert!(a.ui.status.contains("pick a deal to sail with"), "{}", a.ui.status);
    assert!(!a.ui.airship.as_ref().unwrap().confirm);

    // The deals column, the wood deal, once.
    play(&mut a, "<right> <down> +");
    let shown = frame(&mut a);
    assert!(shown.contains("Out: 50 wood") && shown.contains("50/60"), "{shown}");
    assert!(shown.contains("Back in about 15 min with 8 gold ore"), "{shown}");
    // Twice would not fit the hold.
    play(&mut a, "+");
    assert!(a.ui.status.contains("won't fit a hold of 60"), "{}", a.ui.status);

    play(&mut a, "<enter>");
    let boxed = frame(&mut a);
    for want in [
        "SEND THE AIRSHIP",
        "Load 50 wood and sail to the Earth Kingdom?",
        "Haulers bring the goods to the dock",
        "Load and sail [Enter]",
        "Back [Esc]",
    ] {
        assert!(boxed.contains(want), "{want:?} missing from\n{boxed}");
    }
    assert!(boxed.lines().last().unwrap().contains("Esc back"), "{boxed}");
    // Esc backs out of the box and leaves the screen open.
    play(&mut a, "<esc>");
    assert_eq!(a.ui.screen, Some(Screen::Airship));
    assert!(!a.ui.airship.as_ref().unwrap().confirm);
    play(&mut a, "<enter> <enter>");
    assert!(a.ui.status.contains("Loading the airship for the Earth Kingdom"), "{}", a.ui.status);
    let shown = frame(&mut a);
    assert!(shown.contains("Loading for Earth Kingdom: 0/50"), "{shown}");
    assert!(details(&a, dock).contains(&"Loading for Earth Kingdom: 0/50".to_owned()));

    // X cancels what is being loaded.
    play(&mut a, "x");
    assert!(frame(&mut a).contains("Docked"));
    play(&mut a, "<esc>");
    assert_eq!(a.ui.screen, None);
}

#[test]
fn enter_off_a_dock_still_selects_a_settler() {
    let mut a = app(Nation::Fire);
    let home = a.session.world.units[0].pos;
    a.ui.go_to(home);
    play(&mut a, "<enter>");
    assert_eq!(a.ui.screen, None);
    assert!(a.ui.selected_unit(&a.session).is_some());
}

#[test]
fn the_bend_picker_counts_both_nations_for_a_joint_bend() {
    let mut a = water_city(0, 2);
    learn(&mut a, &["cloudbending"]);
    let rows = fn_ui::bend::abilities(&a.session);
    let rain = rows.iter().find(|r| r.name == "Call Rain").expect("Call Rain is offered");
    assert!(rain.casters.starts_with("Water ") && rain.casters.contains(" + Air "), "{rain:?}");
    assert!(rain.casters.ends_with("adepts") || rain.casters.ends_with("adept"), "{rain:?}");
    assert_eq!(
        rain.effects,
        [
            "Plots in a 13x13 patch grow 1.5x for a season",
            "Needs a waterbender and an airbender together"
        ]
    );
    // The plain bends keep their plain counts.
    let water = rows.iter().find(|r| r.name == "Water the Land").unwrap();
    assert!(water.casters.ends_with("benders") && !water.casters.contains('+'), "{water:?}");
    // The Abilities screen shows the picker's row.
    play(&mut a, "i");
    let shown = frame(&mut a);
    assert!(shown.contains("Call Rain") && shown.contains(" + Air "), "{shown}");
}

#[test]
fn the_bend_tool_tints_the_patch_call_rain_would_cover() {
    let mut a = water_city(0, 2);
    learn(&mut a, &["cloudbending"]);
    let rain =
        fn_ui::bend::abilities(&a.session).into_iter().find(|r| r.name == "Call Rain").unwrap();
    a.session.bend_choice = Some(rain.id);
    let at = site(&a);
    a.ui.go_to(Pos::new(at.x + 8, at.y + 8));
    let c = a.ui.cursor;
    assert!(!marks(&a, c).contains(&Mark::Rain), "not with the Select tool");
    a.ui.set_tool(Tool::Bend);
    assert!(marks(&a, c).contains(&Mark::Rain));
    assert!(marks(&a, Pos::new(c.x + 6, c.y - 6)).contains(&Mark::Rain), "the 13x13 corner");
    assert!(!marks(&a, Pos::new(c.x + 7, c.y)).contains(&Mark::Rain), "one past it");
}

#[test]
fn the_airship_screen_and_its_box_fit_the_smallest_frame() {
    for size in [(80, 24), (100, 30), (160, 50)] {
        let (mut a, _) = dock_app();
        play(&mut a, "<enter> <right> <down> + <enter>");
        assert!(a.ui.airship.as_ref().unwrap().confirm);
        let p = Palette::new(&a.session.defs, a.session.world.nation, ColorDepth::TrueColor, true);
        let text = to_text(&render_app(&mut a, &p, size));
        assert!(text.contains("SEND THE AIRSHIP") && text.contains("AIRSHIP"), "{size:?}\n{text}");
        assert!(text.lines().all(|l| l.chars().count() <= usize::from(size.0)), "{size:?}");
    }
}
