//! Research horizons, phase 3 (#712, docs/design/horizons-phase-3.md, "Mockups"): `R` switching a
//! Workshop's recipe, the Archive and Tea House readouts under the cursor, the settlement panel's
//! copying row, `X` clearing the research target, and the Traders screen's tea line.

use fn_sim::geom::Footprint;
use fn_sim::{Nation, Pos};
use fn_tui::app::App;
use fn_tui::color::{ColorDepth, Palette};
use fn_tui::frame::{render_app, to_text};
use fn_tui::{keys, new_session};

const SIZE: (u16, u16) = (120, 40);

fn run(app: &mut App, script: &str) {
    app.play(&keys::parse(script).expect("a valid script"));
}

fn text(app: &mut App) -> String {
    let p = Palette::new(&app.session.defs, app.session.world.nation, ColorDepth::TrueColor, false);
    to_text(&render_app(app, &p, SIZE))
}

/// A game with `building` standing on the free 2x2 spot nearest the first settler, the cursor on
/// its anchor.
fn with(nation: Nation, building: &str) -> (App, Pos) {
    let mut s = new_session(7, nation);
    let id = s.defs.building_id(building).unwrap();
    let home = s.world.units[0].pos;
    let bed = s.defs.building_id("bed").unwrap();
    let at = s
        .world
        .map
        .iter()
        .map(|(p, _)| p)
        .filter(|&p| {
            [(0, 0), (1, 0), (0, 1), (1, 1)].iter().all(|&(dx, dy)| {
                let q = Pos::new(p.x + dx, p.y + dy);
                q != home && s.world.check_build(q, bed, &s.defs).is_ok()
            })
        })
        .min_by_key(|p| ((p.x - home.x).pow(2) + (p.y - home.y).pow(2), p.x, p.y))
        .expect("room near home");
    s.world.put_building(Footprint::new(at, 2, 2), id, &s.defs);
    let mut app = App::new(s, SIZE);
    app.ui.go_to(at);
    (app, at)
}

#[test]
fn r_switches_a_workshop_to_wares_once_the_tech_is_known_and_back() {
    let (mut a, at) = with(Nation::Earth, "workshop");
    let frame = text(&mut a);
    assert!(frame.contains("Making: Tools"), "{frame}");

    run(&mut a, "r");
    assert_eq!(a.ui.notice, "Can't switch recipe: needs Trade Wares");
    assert!(text(&mut a).contains("Making: Tools"));

    let id = a.session.defs.building_id("workshop").unwrap();
    a.session.defs.buildings[id.0 as usize].alt_recipes[0].unlocked_by = None;
    run(&mut a, "r");
    assert_eq!(a.ui.notice, "Workshop now makes Wares");
    assert_eq!(a.session.world.recipe_choice_at(at, &a.session.defs), Some(0));
    assert!(text(&mut a).contains("Making: Wares"));

    run(&mut a, "r");
    assert_eq!(a.session.world.recipe_choice_at(at, &a.session.defs), None);
    assert!(text(&mut a).contains("Making: Tools"));
}

#[test]
fn r_is_listed_in_the_key_help_and_does_nothing_off_a_workshop() {
    let rows = fn_ui::keys::help_rows();
    assert!(rows.iter().any(|r| r.keys == "R" && r.what.contains("Workshop")), "{rows:?}");
    let mut a = App::new(new_session(7, Nation::Earth), SIZE);
    run(&mut a, "r");
    assert_eq!(a.ui.notice, "Can't switch recipe: nothing to switch here");
}

#[test]
fn an_archive_reads_its_halls_copying_and_copies_under_the_cursor() {
    let (mut a, _) = with(Nation::Earth, "archive");
    let frame = text(&mut a);
    for want in ["Halls in reach: 0", "Copying: 0/150", "Copies made: 0"] {
        assert!(frame.contains(want), "{want:?} in\n{frame}");
    }
}

#[test]
fn a_tea_house_reads_closed_without_herbs() {
    let (mut a, _) = with(Nation::Water, "tea_house");
    let frame = text(&mut a);
    assert!(frame.contains("Closed: out of herbs"), "{frame}");
    assert!(frame.contains("Herbs: 0/10"), "{frame}");
}

#[test]
fn the_settlement_panel_shows_copying_once_an_archive_stands_with_no_target() {
    let (mut a, _) = with(Nation::Earth, "archive");
    let frame = text(&mut a);
    assert!(frame.contains("Copying scrolls: 0/150"), "{frame}");
    assert!(!frame.contains("Research: none set"), "{frame}");
    let shelter = a.session.defs.tech_id("shelter").unwrap();
    a.session.apply(fn_sim::world::Command::SetResearch { tech: shelter }).unwrap();
    let frame = text(&mut a);
    assert!(!frame.contains("Copying scrolls"), "{frame}");
}

#[test]
fn x_clears_the_target_in_the_research_screen_when_an_archive_stands() {
    let (mut a, _) = with(Nation::Earth, "archive");
    let shelter = a.session.defs.tech_id("shelter").unwrap();
    a.session.apply(fn_sim::world::Command::SetResearch { tech: shelter }).unwrap();
    run(&mut a, "t");
    let frame = text(&mut a);
    assert!(frame.lines().last().unwrap().contains("X clear target"), "{frame}");
    run(&mut a, "x");
    assert!(a.session.world.research().queue().is_empty());
    assert_eq!(a.ui.status, "Research target cleared");
    assert!(a.ui.screen.is_some(), "the screen stays open");
}

#[test]
fn x_in_the_research_screen_says_why_without_an_archive() {
    let mut a = App::new(new_session(7, Nation::Earth), SIZE);
    let shelter = a.session.defs.tech_id("shelter").unwrap();
    a.session.apply(fn_sim::world::Command::SetResearch { tech: shelter }).unwrap();
    run(&mut a, "t x");
    assert_eq!(a.ui.status, "Can't clear the target: needs an Archive");
    assert!(!a.session.world.research().queue().is_empty());
}

#[test]
fn the_research_screen_lists_the_conditions_a_tech_needs() {
    let mut a = App::new(new_session(7, Nation::Fire), (160, 50));
    run(&mut a, "t");
    let p = Palette::new(&a.session.defs, Nation::Fire, ColorDepth::TrueColor, false);
    let frame = to_text(&render_app(&mut a, &p, (160, 50)));
    assert!(frame.contains("Known on the road"), "{frame}");
    assert!(frame.contains("3 scrolls read"), "{frame}");
}
