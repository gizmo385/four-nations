//! The compact layout (#979, docs/design/touch-pwa.md §5): below 1000×600 points the side panel
//! is a drawer over the map, closed by default, with a tab on the right edge and the time
//! controls floating at the map's top right.

use eframe::egui;
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use fn_gui::app::GuiApp;
use fn_sim::Nation;
use fn_tui::shell::{Mode, Shell};
use fn_ui::app::App;
use fn_ui::state::Modal;
use fn_ui::tools::Tool;

const PHONE: egui::Vec2 = egui::vec2(844.0, 390.0);

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

fn harness(app: &mut GuiApp, size: egui::Vec2) -> Harness<'_> {
    let mut h = Harness::builder()
        .with_size(size)
        .with_context_setup(fn_gui::setup)
        .build_ui(move |ui| app.show(ui));
    h.run_steps(3);
    h
}

#[test]
fn on_a_phone_the_panel_is_a_closed_drawer_with_a_tab_and_floating_time() {
    let mut app = GuiApp::with_shell(game(""));
    let mut h = harness(&mut app, PHONE);
    assert!(h.query_by_label("ALERTS").is_none(), "the drawer starts closed");
    assert!(h.query_by_label("Pause").is_some(), "pause floats on the map");
    assert!(h.query_by_label("Faster").is_some());
    h.get_by_label_contains("Open panel").click();
    h.run_steps(3);
    assert!(h.query_by_label("ALERTS").is_some(), "the tab opens it");
    assert!(h.query_by_label("Faster").is_none(), "the time controls are the panel's again");
    h.get_by_label("Close panel").click();
    h.run_steps(3);
    assert!(h.query_by_label("ALERTS").is_none(), "the chevron closes it");
    drop(h);
    assert!(!state(&app).session.clock.paused, "the drawer never paused the game");
}

#[test]
fn the_tab_counts_the_open_alerts() {
    let mut shell = game("");
    for _ in 0..600 {
        shell.advance(0.5);
    }
    let mut app = GuiApp::with_shell(shell);
    let open = state(&app).ui.alerts.open().len();
    assert!(open > 0, "the research alert is open");
    let h = harness(&mut app, PHONE);
    let want = if open == 1 {
        "Open panel: 1 alert".to_owned()
    } else {
        format!("Open panel: {open} alerts")
    };
    assert!(h.query_by_label(&want).is_some(), "{want}");
}

/// With Harvest armed, a tap on the map beside the open drawer closes it and does nothing else:
/// the cursor stays put and nothing is anchored.
#[test]
fn a_tap_on_the_map_closes_the_drawer_and_applies_nothing() {
    let mut app = GuiApp::with_shell(game("h"));
    app.frame_mut().controls.drawer_open = true;
    let cursor = state(&app).ui.cursor;
    let mut h = harness(&mut app, PHONE);
    assert!(h.query_by_label("ALERTS").is_some());
    let at = egui::pos2(200.0, 200.0);
    h.hover_at(at);
    h.run_steps(1);
    h.drag_at(at);
    h.run_steps(1);
    h.drop_at(at);
    h.run_steps(3);
    assert!(h.query_by_label("ALERTS").is_none(), "the tap closed the drawer");
    drop(h);
    let game = state(&app);
    assert_eq!(game.ui.tool, Tool::Harvest);
    assert_eq!(game.ui.cursor, cursor, "the tap never reached the map");
    assert!(game.ui.anchor.is_none());
    assert!(!app.frame().controls.drawer_open);
}

/// At 1000×600 and up the panel docks as before: no tab, no floating controls. (The harness
/// keeps an 8-point margin round the window's `Ui`, so 1016×616 is a 1000×600 window.)
#[test]
fn a_wide_window_docks_the_panel() {
    let mut app = GuiApp::with_shell(game(""));
    let h = harness(&mut app, egui::vec2(1016.0, 616.0));
    assert!(h.query_by_label_contains("SETTLEMENT").is_some(), "the docked panel");
    assert!(h.query_by_label_contains("Open panel").is_none());
    assert!(h.query_by_label("Faster").is_none());
}

/// On touch, a list's rows are a finger tall; with a mouse they stay a cell.
#[test]
fn on_touch_list_rows_are_a_finger_tall() {
    let row = |touch: bool| {
        let mut app = GuiApp::with_shell(game("u"));
        app.frame_mut().controls.force_touch = touch;
        let h = harness(&mut app, PHONE);
        h.get_by_label_contains("Scroll Library").rect().height()
    };
    let mouse = row(false);
    assert!(mouse < 44.0, "a cell with a mouse: {mouse}");
    assert!(row(true) >= 44.0, "a finger tall on touch: {}", row(true));
}

/// On touch a full screen's Close is a finger tall, off the window's top edge, and a tap a
/// little outside it still counts; with a mouse it is the cell-tall button on the frame's edge.
#[test]
fn on_touch_close_is_a_finger_sized_target() {
    let tap = |touch: bool, below: f32| {
        let mut app = GuiApp::with_shell(game("u"));
        app.frame_mut().controls.force_touch = touch;
        assert_eq!(state(&app).ui.modal, Some(Modal::Build), "the Build list is up");
        let mut h = harness(&mut app, PHONE);
        let close = h.get_by_label("Close [Esc]").rect();
        if touch {
            assert!(close.height() >= 44.0, "a finger tall: {close:?}");
            assert!(close.top() >= 8.0 + 4.0, "off the window's edge: {close:?}");
        }
        let at = egui::pos2(close.center().x, close.bottom() + below);
        h.hover_at(at);
        h.run_steps(1);
        h.drag_at(at);
        h.run_steps(1);
        h.drop_at(at);
        h.run_steps(3);
        drop(h);
        state(&app).ui.modal != Some(Modal::Build)
    };
    assert!(tap(true, -10.0), "a tap on it closes the page");
    assert!(tap(true, 5.0), "a tap just under it does too");
    assert!(tap(false, -4.0), "a click on the mouse's button closes it");
}
