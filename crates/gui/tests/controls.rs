//! The touch controls (#978, docs/design/touch-pwa.md §4): the tool rail, the action bar and
//! the More sheet. Each button is the keys it stands for.

use eframe::egui;
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use fn_gui::app::GuiApp;
use fn_gui::controls::{self, actions, menu_keys, rail_keys, speed_keys};
use fn_sim::Nation;
use fn_tui::shell::{Mode, Shell};
use fn_ui::app::App;
use fn_ui::keyev::{KeyCode, KeyEvent, KeyModifiers};
use fn_ui::state::Modal;
use fn_ui::tools::Tool;

fn game(script: &str) -> Shell {
    let mut shell = Shell::new_game(fn_tui::new_session(7, Nation::Earth), (120, 40), None);
    shell.play(&fn_ui::script::parse(script).unwrap());
    shell
}

fn state(app: &GuiApp) -> &App {
    match &app.shell().mode {
        Mode::Game(game) => game,
        _ => panic!("not in a game"),
    }
}

fn app_state(script: &str) -> App {
    let mut app = App::new(fn_tui::new_session(7, Nation::Earth), (120, 40));
    app.play(&fn_ui::script::parse(script).unwrap());
    app
}

fn esc() -> KeyEvent {
    KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE)
}

fn ch(c: char) -> KeyEvent {
    KeyEvent::new(KeyCode::Char(c), KeyModifiers::NONE)
}

/// A game with the rail switched on (as on a touch screen).
fn with_rail(script: &str) -> GuiApp {
    let mut app = GuiApp::with_shell(game(script));
    app.frame_mut().controls.rail_on = true;
    app
}

fn harness(app: &mut GuiApp, size: egui::Vec2) -> Harness<'_> {
    let mut h = Harness::builder()
        .with_size(size)
        .with_context_setup(fn_gui::setup)
        .build_ui(move |ui| app.show(ui));
    h.run_steps(3);
    h
}

#[test]
fn a_rail_button_is_its_tools_letter_and_select_is_the_escs_back() {
    let plain = app_state("");
    assert_eq!(rail_keys(&plain, Tool::Harvest), vec![ch('h')]);
    assert_eq!(rail_keys(&plain, Tool::Build), vec![ch('u')]);
    assert!(rail_keys(&plain, Tool::Select).is_empty(), "no Esc: it would open the menu");
    let tool = app_state("h");
    assert_eq!(rail_keys(&tool, Tool::Select), vec![esc()]);
    let corner = app_state("h <enter>");
    assert!(corner.ui.anchor.is_some());
    assert_eq!(rail_keys(&corner, Tool::Select), vec![esc(), esc()], "drop the corner, leave");
}

#[test]
fn the_action_bar_offers_the_modes_buttons() {
    let labels = |app: &App| actions(app).iter().map(|a| a.label).collect::<Vec<_>>();
    assert_eq!(labels(&app_state("")), ["‹", "›", "Research", "Nation", "Log"]);
    assert_eq!(labels(&app_state("h")), ["Repeat", "Done"]);
    assert_eq!(labels(&app_state("p")), ["Done"]);
    let done = actions(&app_state("h")).pop().unwrap();
    assert!(done.primary && done.keys == vec![esc()]);
}

#[test]
fn a_speed_is_the_steps_to_it_and_a_start() {
    let running = app_state("");
    assert_eq!(running.session.clock.speed(), 1);
    assert_eq!(speed_keys(&running, Some(3)), vec![ch('+'), ch('+')]);
    assert_eq!(speed_keys(&running, Some(1)), Vec::<KeyEvent>::new());
    assert_eq!(speed_keys(&running, None), vec![ch(' ')], "pause");
    let paused = app_state("<space> + + +");
    assert!(paused.session.clock.paused);
    assert_eq!(speed_keys(&paused, Some(2)), vec![ch('-'), ch('-'), ch(' ')]);
    assert!(speed_keys(&paused, None).is_empty(), "already paused");
}

#[test]
fn the_game_menu_is_reached_from_any_mode() {
    for script in ["", "h", "h <enter>", "<tab>"] {
        let mut app = app_state(script);
        for k in menu_keys(&app) {
            app.key(k);
        }
        assert_eq!(app.ui.modal, Some(Modal::Menu), "from {script:?}");
    }
}

/// Phone size, rail on: tapping Harvest arms it, Done leaves it.
#[test]
fn tapping_the_rail_arms_a_tool_and_done_leaves_it() {
    let size = egui::vec2(844.0, 390.0);
    let mut app = with_rail("");
    let mut h = harness(&mut app, size);
    assert!(h.query_by_label("More").is_some(), "the rail's More");
    h.get_by_label("Harvest").click();
    h.run_steps(3);
    drop(h);
    assert_eq!(state(&app).ui.tool, Tool::Harvest);
    let mut h = harness(&mut app, size);
    h.get_by_label("Done").click();
    h.run_steps(3);
    drop(h);
    assert_eq!(state(&app).ui.tool, Tool::Select);
}

/// The More sheet's x3 sets the speed; its Pause pauses.
#[test]
fn the_more_sheets_speed_row_sets_the_clock() {
    let size = egui::vec2(844.0, 390.0);
    let mut app = with_rail("");
    let mut h = harness(&mut app, size);
    h.get_by_label("More").click();
    h.run_steps(3);
    h.get_by_label("Speed: x3").click();
    h.run_steps(3);
    drop(h);
    assert_eq!(state(&app).session.clock.speed(), 3);
    let mut h = harness(&mut app, size);
    h.get_by_label("Speed: pause").click();
    h.run_steps(3);
    drop(h);
    assert!(state(&app).session.clock.paused);
}

/// Without a touch screen the rail is off; the corner button opens More, whose switch turns
/// the rail on.
#[test]
fn on_desktop_the_rail_is_switched_on_from_more() {
    let size = egui::vec2(1600.0, 960.0);
    let mut app = GuiApp::with_shell(game(""));
    let mut h = harness(&mut app, size);
    assert!(h.query_by_label("Harvest").is_none(), "no rail by default");
    h.get_by_label("More").click();
    h.run_steps(3);
    h.get_by_label("Tool rail").click();
    h.run_steps(3);
    drop(h);
    assert!(app.frame().controls.rail_on);
    let h = harness(&mut app, size);
    assert!(h.query_by_label("Harvest").is_some(), "the rail");
    assert!(controls::wide(egui::Rect::from_min_size(egui::Pos2::ZERO, size)));
}
