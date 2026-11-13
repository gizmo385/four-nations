//! The native inspector (#882): the side panel's last section answers "why" for the settler
//! selected, or for the tile under the cursor. Like `side.rs`, none of it needs a GPU.

use eframe::egui::{self, Key};
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use fn_gui::app::GuiApp;
use fn_sim::Nation;
use fn_tui::shell::{Mode, Shell};
use fn_ui::view::settler_view;

const SIZE: (u16, u16) = (120, 40);

fn game() -> Shell {
    Shell::new_game(fn_tui::new_session(7, Nation::Earth), SIZE, None)
}

fn harness(app: &mut GuiApp, size: egui::Vec2) -> Harness<'_> {
    let mut h = Harness::builder()
        .with_size(size)
        .with_context_setup(fn_gui::setup)
        .build_ui(move |ui| app.show(ui));
    h.run_steps(2);
    h
}

fn state(app: &GuiApp) -> &fn_ui::app::App {
    match &app.shell().mode {
        Mode::Game(game) => game,
        Mode::Title(_) | Mode::Embark(_) => panic!("not in a game"),
    }
}

fn tab(h: &mut Harness<'_>) {
    h.key_press(Key::Tab);
    h.run_steps(3);
}

/// After `Tab` the inspector names the settler and says what it is doing, in the heading's own
/// case, with its rank at the end of the heading.
#[test]
fn after_tab_the_inspector_shows_the_settlers_name_and_task() {
    let mut app = GuiApp::with_shell(game());
    let mut h = harness(&mut app, egui::vec2(1600.0, 960.0));
    tab(&mut h);
    drop(h);
    let game = state(&app);
    let unit = game.ui.selected_unit(&game.session).expect("Tab selects a settler");
    let view = settler_view(&game.session, unit);

    let h = harness(&mut app, egui::vec2(1600.0, 960.0));
    assert!(h.query_by_label(&view.name).is_some(), "the name {:?}", view.name);
    assert!(h.query_by_label(&view.task).is_some(), "the task {:?}", view.task);
    assert!(h.query_by_label(&view.rank_title).is_some(), "the rank {:?}", view.rank_title);
    assert!(h.query_by_label("Carrying nothing").is_some());
    // The inspector closes the panel: under the sections, above the seed line, in the column.
    let (alerts, name, seed) = (
        h.get_by_label("ALERTS").rect(),
        h.get_by_label(&view.name).rect(),
        h.get_by_label("seed 7").rect(),
    );
    assert!(alerts.max.y <= name.min.y && name.max.y <= seed.min.y, "{alerts:?} {name:?} {seed:?}");
    assert!(name.min.x >= 1600.0 - fn_gui::frame::PANEL_RANGE.1, "{name:?}");
}

/// The meters are bars; one that is not ok says "low" in words as well, and chi never does.
#[test]
fn meters_are_bars_and_a_low_one_says_so() {
    let mut shell = game();
    let Mode::Game(game) = &mut shell.mode else { panic!("not in a game") };
    // Every settler starts with its energy gone, so whichever Tab picks has one low meter.
    for unit in &mut game.session.world.units {
        unit.needs.energy = 0.0;
    }
    let mut app = GuiApp::with_shell(shell);
    let mut h = harness(&mut app, egui::vec2(1600.0, 960.0));
    tab(&mut h);
    for label in ["Health", "Hunger", "Thirst", "Energy", "Chi"] {
        assert!(h.query_by_label(label).is_some(), "{label}");
    }
    assert_eq!(h.query_all_by_label("low").count(), 1, "only energy is low");
    let bars = h.query_all_by_role(egui::accesskit::Role::ProgressIndicator).count();
    assert_eq!(bars, 5, "four needs and chi, each a bar");
}

/// With nothing selected the inspector describes the tile under the cursor, and its heading ends
/// with the cursor's position.
#[test]
fn with_nothing_selected_it_is_the_tile_under_the_cursor() {
    let mut app = GuiApp::with_shell(game());
    let at = {
        let game = state(&app);
        assert!(game.ui.selected.is_none());
        format!("({}, {})", game.ui.cursor.x, game.ui.cursor.y)
    };
    let h = harness(&mut app, egui::vec2(1600.0, 960.0));
    assert!(h.query_by_label(&at).is_some(), "the cursor {at}");
    assert!(h.query_all_by_role(egui::accesskit::Role::ProgressIndicator).next().is_none());
}

/// A short window scrolls the inspector instead of trimming it: every row is still there, and the
/// seed line stays at the panel's foot.
#[test]
fn a_short_panel_scrolls_the_inspector_instead_of_trimming_it() {
    let mut app = GuiApp::with_shell(game());
    let mut h = harness(&mut app, egui::vec2(1200.0, 260.0));
    tab(&mut h);
    for label in ["Health", "Hunger", "Thirst", "Energy", "Chi"] {
        assert!(h.query_by_label(label).is_some(), "{label}");
    }
    assert!(h.query_by_label_contains("Standing on").is_some(), "the last row is not trimmed");
    let seed = h.get_by_label("seed 7").rect();
    assert!(seed.max.y <= 260.0, "the seed line is in the window: {seed:?}");
}

/// A game with a bed standing on a free tile, the cursor on it, and `marked` or not for take-down.
fn game_with_bed(marked: bool) -> (Shell, fn_sim::Pos) {
    let mut shell = game();
    let Mode::Game(game) = &mut shell.mode else { panic!("not in a game") };
    let defs = game.session.defs.clone();
    let bed = defs.building_id("bed").unwrap();
    let at = game
        .session
        .world
        .map
        .iter()
        .find(|(p, t)| {
            t.terrain == fn_sim::Terrain::Land
                && t.resource.is_none()
                && !game.session.world.units.iter().any(|u| u.pos == *p)
                && game.session.world.check_build(*p, bed, &defs).is_ok()
        })
        .map(|(p, _)| p)
        .expect("a free land tile");
    game.session.world.put_building(fn_sim::geom::Footprint::single(at), bed, &defs);
    if marked {
        game.session.apply(fn_sim::world::Command::Deconstruct { pos: at }).unwrap();
    }
    game.ui.cursor = at;
    (shell, at)
}

/// A building marked to be taken down says so and has a "Keep it" button; clicking it takes the
/// mark back and the building stays (#967).
#[test]
fn keep_it_is_a_button_on_a_marked_building_and_clicking_it_keeps_the_building() {
    let (shell, at) = game_with_bed(true);
    let mut app = GuiApp::with_shell(shell);
    let mut h = harness(&mut app, egui::vec2(1600.0, 960.0));
    assert!(h.query_by_label("Marked to be taken down").is_some());
    assert!(h.query_by_label_contains("Press K").is_none(), "a button, not the key's hint");
    h.get_by_label("Keep it").click();
    h.run_steps(4);
    drop(h);
    let world = &state(&app).session.world;
    assert!(!world.is_marked_for_deconstruct(at), "the mark is gone");
    assert_eq!(world.deconstruct_designations().count(), 0);
    assert!(world.building_at(at).is_some(), "the building stays");

    let h = harness(&mut app, egui::vec2(1600.0, 960.0));
    assert!(h.query_by_label("Keep it").is_none());
    assert!(h.query_by_label("Marked to be taken down").is_none());
}

/// An unmarked building has no button and no marked line.
#[test]
fn an_unmarked_building_has_no_keep_it_button() {
    let (shell, _) = game_with_bed(false);
    let mut app = GuiApp::with_shell(shell);
    let h = harness(&mut app, egui::vec2(1600.0, 960.0));
    assert!(h.query_by_label("Keep it").is_none());
    assert!(h.query_by_label("Marked to be taken down").is_none());
}
