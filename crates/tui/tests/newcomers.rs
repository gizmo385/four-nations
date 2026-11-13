//! The From newcomers page of the Research screen (#762, docs/design/horizons-phase-6.md,
//! "Mockups"): `Tab` after the gate, a column for each other nation and one for the fusions,
//! borrowed prices and conditions with live counts in the detail strip, and the Waiting state in
//! the screen and the side panel. Frames are checked with `contains`, not goldens.

use fn_sim::Nation;
use fn_sim::research::TechId;
use fn_sim::world::{Command, UnitId};
use fn_tui::app::App;
use fn_tui::color::{ColorDepth, Palette};
use fn_tui::frame::{render_app, to_text};
use fn_tui::{keys, new_session};
use fn_ui::research::{ResearchPage, TechState, screen_page, screen_view};

const SIZE: (u16, u16) = (120, 40);

fn run(app: &mut App, script: &str) {
    app.play(&keys::parse(script).expect("a valid script"));
}

fn frame(app: &mut App, ascii: bool) -> String {
    let p = Palette::new(&app.session.defs, app.session.world.nation, ColorDepth::TrueColor, ascii);
    to_text(&render_app(app, &p, SIZE))
}

/// A Water city of 21 people: 4 Fire, 2 Earth and 3 Air residents, the rest Water's own. The gate
/// stands iff `united`.
fn water_city(united: bool) -> App {
    let mut a = App::new(new_session(7, Nation::Water), SIZE);
    learn(&mut a, &["pottery", "masonry", "farming"]);
    let world = &mut a.session.world;
    while world.units.len() < 21 {
        let mut u = world.units[0].clone();
        u.id = UnitId(1000 + world.units.len() as u32);
        world.units.push(u);
    }
    for (i, u) in world.units.iter_mut().enumerate() {
        u.nation = match i {
            0..4 => Some(Nation::Fire),
            4..6 => Some(Nation::Earth),
            6..9 => Some(Nation::Air),
            _ => None,
        };
    }
    if united {
        assert!(world.complete_endeavor(&a.session.defs));
    }
    // The roster is counted once a tick.
    a.session.tick();
    a
}

/// Sets the pool to `points` by way of a save: a world's research can't be edited from outside.
fn bank(app: &mut App, points: u64) {
    let defs = &app.session.defs;
    let text = app.session.world.save(defs).expect("saves");
    let at = text.find("pool_millis:").expect("a pool") + "pool_millis:".len();
    let len = text[at..].find(|c: char| !c.is_ascii_digit()).unwrap();
    let millis = points * fn_sim::research::MILLIS_PER_POINT;
    let text = format!("{}{millis}{}", &text[..at], &text[at + len..]);
    app.session.world = fn_sim::World::load(&text, defs).expect("loads");
}

use crate::schools::learn;

fn tech(app: &App, id: &str) -> TechId {
    app.session.defs.tech_id(id).unwrap_or_else(|| panic!("no tech {id}"))
}

/// Puts the cursor on `id` of the open page.
fn go_to(app: &mut App, id: &str) {
    let want = tech(app, id);
    let view = app.ui.research_view(&app.session);
    app.ui.tech = view
        .columns
        .iter()
        .enumerate()
        .find_map(|(c, col)| col.iter().position(|t| t.id == want).map(|r| (c, r)))
        .unwrap_or_else(|| panic!("{id} is not on this page"));
}

fn state(app: &App, page: ResearchPage, id: &str) -> TechState {
    let want = tech(app, id);
    screen_page(&app.session, page)
        .columns
        .iter()
        .flatten()
        .find(|c| c.id == want)
        .unwrap_or_else(|| panic!("{id} is not on the page"))
        .state
}

#[test]
fn before_the_gate_there_is_no_second_page_and_the_tree_is_as_it_was() {
    let mut a = water_city(false);
    run(&mut a, "t");
    let before = frame(&mut a, false);
    assert!(!before.contains("From newcomers"), "{before}");
    assert!(!before.contains("Tab switches"), "{before}");
    run(&mut a, "<tab>");
    assert_eq!(a.ui.research_page, ResearchPage::Tree);
    assert_eq!(frame(&mut a, false), before, "Tab does nothing without the page");
    assert!(!before.lines().last().unwrap().contains("Tab"), "{before}");
}

#[test]
fn after_the_gate_tab_turns_to_the_page_and_back() {
    let mut a = water_city(true);
    run(&mut a, "t");
    let tree = frame(&mut a, false);
    assert!(tree.contains("[Your tree]") && tree.contains("From newcomers"), "{tree}");
    assert!(tree.contains("TIER 1"), "{tree}");
    assert!(!tree.contains("Steelworks"), "a fusion is on the other page:\n{tree}");
    assert!(tree.lines().last().unwrap().contains("Tab switch page"), "{tree}");

    run(&mut a, "<tab>");
    assert_eq!(a.ui.research_page, ResearchPage::Newcomers);
    let page = frame(&mut a, false);
    assert!(page.contains("Your tree") && page.contains("[From newcomers]"), "{page}");
    for head in ["FIRE NATION: 4 here", "EARTH KINGDOM: 2 here", "AIR NOMADS: 3 here", "FUSIONS"] {
        assert!(page.contains(head), "{head}\n{page}");
    }
    assert!(!page.contains("TIER 1"), "{page}");
    assert!(!page.contains("WATER TRIBES"), "the home nation has no column:\n{page}");
    for name in ["Brickmaking", "Earthen Walls", "Orchards", "Steelworks", "Aqueducts"] {
        assert!(page.contains(name) || name == "Aqueducts", "{name}\n{page}");
    }

    run(&mut a, "<tab>");
    assert_eq!(a.ui.research_page, ResearchPage::Tree);
    assert_eq!(frame(&mut a, false), tree);
    // Opening the screen again starts on the tree.
    run(&mut a, "<tab> t t");
    assert_eq!(a.ui.research_page, ResearchPage::Tree);
}

#[test]
fn the_page_lists_what_each_nation_teaches_with_the_borrowed_cost() {
    let mut a = water_city(true);
    run(&mut a, "t <tab>");
    let page = frame(&mut a, false);
    // Brickmaking costs 240 borrowed, Steam Engines 500, Cargo Balloons 1000.
    assert!(page.contains("Brickmaking") && page.contains("240 pts"), "{page}");
    assert!(page.contains("Steam Engines") && page.contains("500 pts"), "{page}");
    // Earth has two residents of the three it takes: its techs are locked, with the live count.
    assert!(page.contains("Needs 3 Earth residents"), "{page}");
    assert_eq!(state(&a, ResearchPage::Newcomers, "earthen_walls"), TechState::Locked);
    assert_eq!(state(&a, ResearchPage::Newcomers, "brickmaking"), TechState::Available);
    // The fusions name their nations.
    assert!(page.contains("Earth+Fire") || page.contains("Steelworks"), "{page}");
    // Cargo Balloons is the Fire column's fifth: below the fold now that every fusion row is
    // tall, so the page scrolls to it.
    go_to(&mut a, "cargo_balloons");
    let scrolled = frame(&mut a, false);
    assert!(scrolled.contains("Cargo Balloons") && scrolled.contains("1000 pts"), "{scrolled}");
}

#[test]
fn the_detail_strip_prices_a_borrowed_tech_and_names_who_gets_its_bonus() {
    let mut a = water_city(true);
    run(&mut a, "t <tab>");
    go_to(&mut a, "smelting");
    let page = frame(&mut a, false);
    assert!(page.contains("Available"), "{page}");
    assert!(page.contains("Borrowed from the Fire Nation at 2x cost."), "{page}");
    assert!(page.contains("No Forging needed."), "{page}");
    assert!(page.contains("Needs 3 Fire residents (4 live here)."), "{page}");
    assert!(page.contains("Smelter"), "{page}");
    assert!(page.contains("for Fire residents only"), "{page}");
}

#[test]
fn a_locked_borrowed_tech_says_what_it_needs_with_the_live_count() {
    let mut a = water_city(true);
    run(&mut a, "t <tab>");
    go_to(&mut a, "earth_rail");
    let page = frame(&mut a, false);
    assert!(page.contains("Needs Earthen Walls and 3 Earth residents (2 live here)."), "{page}");
    // Enter on it says why, in the status line.
    run(&mut a, "<enter>");
    let after = frame(&mut a, false);
    assert!(a.session.world.research().queue().is_empty());
    assert!(after.contains("Can't set target: needs 3 Earth residents (2 live here)"), "{after}");
}

#[test]
fn enter_on_a_borrowed_tech_sets_it_as_the_target() {
    let mut a = water_city(true);
    run(&mut a, "t <tab>");
    go_to(&mut a, "brickmaking");
    run(&mut a, "<enter>");
    let queue = a.session.world.research().queue().to_vec();
    assert_eq!(queue.last(), Some(&tech(&a, "brickmaking")));
    assert_eq!(state(&a, ResearchPage::Newcomers, "brickmaking"), TechState::Queued);
    assert!(frame(&mut a, false).contains("Target"));
}

#[test]
fn a_fusion_lists_every_condition_with_its_live_count() {
    let mut a = water_city(true);
    run(&mut a, "t <tab>");
    go_to(&mut a, "steelworks");
    let page = frame(&mut a, false);
    assert!(page.contains("Locked"), "{page}");
    assert!(page.contains("Fusion of Earth and Fire."), "{page}");
    assert!(page.contains("3 Earth residents (2 live here)"), "{page}");
    assert!(page.contains("3 Fire residents (4 live"), "{page}");
    // #955: a fusion has no masters condition.
    assert!(!page.contains("master"), "{page}");
    assert!(page.contains("1000 pts + 8"), "the bill:\n{page}");
}

#[test]
fn the_gui_less_view_keeps_the_fusions_off_the_tree() {
    let a = water_city(true);
    let tree = screen_view(&a.session);
    let steelworks = tech(&a, "steelworks");
    assert!(tree.columns.iter().flatten().all(|c| c.id != steelworks));
    let page = screen_page(&a.session, ResearchPage::Newcomers);
    assert_eq!(page.columns.len(), page.titles.len());
    assert!(page.columns.iter().flatten().any(|c| c.id == steelworks));
}

/// A tech whose points are in and whose teachers left waits at the front of the queue: Waiting on
/// the Research screen, with its progress and what it needs, and under the Research row of the
/// side panel.
#[test]
fn a_tech_waiting_on_a_fallen_condition_reads_waiting_everywhere() {
    let mut a = water_city(true);
    let brick = tech(&a, "brickmaking");
    a.session.apply(Command::SetResearch { tech: brick }).unwrap();
    // The points are in; two of the four Fire residents leave, leaving two of the three needed.
    bank(&mut a, 1000);
    for u in a.session.world.units.iter_mut().filter(|u| u.nation == Some(Nation::Fire)).take(2) {
        u.nation = Some(Nation::Earth);
    }
    a.session.tick();
    assert!(a.session.world.research().queue().contains(&brick), "it waits, it is not learned");
    assert_eq!(state(&a, ResearchPage::Newcomers, "brickmaking"), TechState::Waiting);

    // The side panel.
    let panel = frame(&mut a, false);
    assert!(panel.contains("Brickmaking █"), "{panel}");
    assert!(panel.contains("Waiting: 3 Fire residents"), "{panel}");
    assert!(panel.contains("(2 live here)"), "{panel}");

    // The screen: the page and the detail strip.
    run(&mut a, "t <tab>");
    go_to(&mut a, "brickmaking");
    let page = frame(&mut a, false);
    assert!(page.contains("Waiting"), "{page}");
    assert!(page.contains("240/240 pts"), "{page}");
    assert!(page.contains("Waiting: needs 3 Fire residents (2 live here)"), "{page}");

    // One more arrives: it finishes by itself.
    let u = a.session.world.units.iter_mut().find(|u| u.nation == Some(Nation::Earth)).unwrap();
    u.nation = Some(Nation::Fire);
    a.session.tick();
    assert!(a.session.world.research().is_known(brick));
    assert_eq!(state(&a, ResearchPage::Newcomers, "brickmaking"), TechState::Known);
    assert!(frame(&mut a, false).contains("Known, borrowed"));
}
