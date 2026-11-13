//! The phone's notch and home bar (#974, docs/design/touch-pwa.md §1): with safe-area insets
//! set, every widget is laid out inside them.

use eframe::egui;
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use fn_gui::app::GuiApp;
use fn_gui::frame::SafeArea;
use fn_sim::Nation;
use fn_tui::shell::Shell;

/// An iPhone held sideways with the notch on the left.
const NOTCH: SafeArea = SafeArea { top: 0.0, right: 47.0, bottom: 21.0, left: 47.0 };

#[test]
fn the_rail_action_bar_and_panel_stay_inside_the_insets() {
    let size = egui::vec2(844.0, 390.0);
    let shell = Shell::new_game(fn_tui::new_session(7, Nation::Earth), (120, 40), None);
    let mut app = GuiApp::with_shell(shell);
    app.frame_mut().controls.rail_on = true;
    app.frame_mut().safe = NOTCH;
    let mut h = Harness::builder()
        .with_size(size)
        .with_context_setup(fn_gui::setup)
        .build_ui(move |ui| app.show(ui));
    h.run_steps(3);
    let select = h.get_by_label("Select").rect();
    assert!(select.min.x >= NOTCH.left, "the rail clears the notch: {select:?}");
    let more = h.get_by_label("More").rect();
    assert!(more.max.y <= size.y - NOTCH.bottom, "the bar clears the home bar: {more:?}");
    let pause = h.get_by_label("Pause").rect();
    assert!(pause.max.x <= size.x - NOTCH.right, "the panel clears the right edge: {pause:?}");
}

/// The insets are drawn as the game's background and nothing is laid out in them: the title's
/// rows sit inside, centred on what is left.
#[test]
fn the_title_screen_stays_inside_the_insets() {
    let size = egui::vec2(844.0, 390.0);
    let mut app = GuiApp::with_shell(Shell::new_title((120, 40), None, 42));
    app.frame_mut().safe = SafeArea { left: 120.0, ..NOTCH };
    let mut h = Harness::builder()
        .with_size(size)
        .with_context_setup(fn_gui::setup)
        .build_ui(move |ui| app.show(ui));
    h.run_steps(3);
    let row = h.get_by_label("New game").rect();
    assert!(row.min.x >= 120.0, "{row:?}");
    let middle = (120.0 + size.x - NOTCH.right) / 2.0;
    assert!((row.center().x - middle).abs() < 2.0, "centred inside the insets: {row:?}");
}
