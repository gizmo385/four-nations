//! Everything scrolls in a short window (#973, docs/design/touch-pwa.md §5): a phone held
//! sideways is about 844×390 points. Lists scroll when their content is dragged, by a finger
//! from the first touch on and by a mouse, and a box too tall for the window scrolls its body.
//! Like `boxes.rs`, none of it needs a GPU.

use eframe::egui;
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use fn_gui::app::GuiApp;
use fn_sim::Nation;
use fn_tui::shell::Shell;

fn game(script: &str) -> Shell {
    let mut shell = Shell::new_game(fn_tui::new_session(7, Nation::Earth), (120, 40), None);
    shell.play(&fn_ui::script::parse(script).unwrap());
    shell
}

fn harness(app: &mut GuiApp, size: egui::Vec2) -> Harness<'_> {
    let mut h = Harness::builder()
        .with_size(size)
        .with_context_setup(fn_gui::setup)
        .build_ui(move |ui| app.show(ui));
    h.run_steps(3);
    h
}

/// The top of the first node whose label contains `text`.
fn top_of(h: &Harness<'_>, text: &str) -> f32 {
    h.get_by_label_contains(text).rect().min.y
}

/// A press at `from`, a move to `to` in steps, and a release: a drag, with the mouse.
fn drag(h: &mut Harness<'_>, from: egui::Pos2, to: egui::Pos2) {
    h.hover_at(from);
    h.run_steps(1);
    h.drag_at(from);
    h.run_steps(1);
    for i in 1..=8 {
        h.hover_at(from + (to - from) * (i as f32 / 8.0));
        h.run_steps(1);
    }
    h.drop_at(to);
    h.run_steps(3);
}

/// The Build list at phone size: a drag on its rows (with a mouse, so no touch has been seen)
/// scrolls it, and a later row comes into view.
#[test]
fn dragging_the_build_list_scrolls_it() {
    let mut app = GuiApp::with_shell(game("u"));
    let mut h = harness(&mut app, egui::vec2(844.0, 390.0));
    let before = top_of(&h, "Scroll Library");
    let at = egui::pos2(200.0, before + 40.0);
    drag(&mut h, at, at - egui::vec2(0.0, 80.0));
    let after = top_of(&h, "Scroll Library");
    assert!(after < before - 40.0, "the list moved up with the drag: {before} -> {after}");
}

/// The side panel's sections scroll by drag too (in the drawer, as the window is compact).
#[test]
fn dragging_the_side_panel_scrolls_it() {
    let mut app = GuiApp::with_shell(game(""));
    app.frame_mut().controls.drawer_open = true;
    let mut h = harness(&mut app, egui::vec2(844.0, 240.0));
    let before = top_of(&h, "ALERTS");
    let at = egui::pos2(700.0, 120.0);
    drag(&mut h, at, at - egui::vec2(0.0, 60.0));
    let after = top_of(&h, "ALERTS");
    assert!(after < before - 20.0, "the sections moved up with the drag: {before} -> {after}");
}

/// The game menu in a window shorter than the menu: its last row starts below the window, and
/// scrolling the box's body brings it inside.
#[test]
fn a_box_taller_than_the_window_scrolls_its_body() {
    let height = 150.0;
    let mut app = GuiApp::with_shell(game("<esc>").with_quit(false));
    let mut h = harness(&mut app, egui::vec2(844.0, height));
    let before = top_of(&h, "Title screen…");
    assert!(before > height, "the last row is out of sight at first: {before}");
    let resume = top_of(&h, "Resume");
    let at = egui::pos2(422.0, resume + 10.0);
    drag(&mut h, at, at - egui::vec2(0.0, 100.0));
    let after = top_of(&h, "Title screen…");
    assert!(after < height - 10.0, "the last row scrolled into the window: {after}");
}
