//! Semantic tests of the side panel and the cell-painted screens (#875): `egui_kittest`'s
//! `Harness` runs `GuiApp` frames with no renderer and no GPU, and finds what the window shows
//! through the AccessKit tree, the way a screen reader would. `frame.rs` does the same for the
//! hint bar and the frame's surfaces; this is the text that is painted as cells.
//!
//! Painted glyphs are not text to AccessKit, so [`fn_gui::cells::expose`] labels each row of a
//! cell-painted region with its text. A test finds a side-panel row by its words
//! (`Running x1`), and keeps working when a row moves, or when #877 turns the panel into egui
//! widgets that carry the same words.

use eframe::egui::{self, Event};
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use fn_gui::app::GuiApp;
use fn_sim::Nation;
use fn_tui::shell::Shell;

const SIZE: egui::Vec2 = egui::vec2(1600.0, 960.0);

fn game(script: &str) -> Shell {
    let mut shell = Shell::new_game(fn_tui::new_session(7, Nation::Earth), (120, 40), None);
    shell.play(&fn_ui::script::parse(script).unwrap());
    shell
}

/// A harness over `app`, a few frames in. `run()` would never return: the app asks for a repaint
/// every frame, so these use `run_steps`.
fn harness(app: &mut GuiApp) -> Harness<'_> {
    let mut h = Harness::builder()
        .with_size(SIZE)
        // The window's own fonts: kittest panics on a glyph no font has.
        .with_context_setup(fn_gui::setup)
        .build_ui(move |ui| app.show(ui));
    h.run_steps(3);
    h
}

/// Types `text` the way a keyboard does: egui hands printable keys over as `Text` events.
fn type_text(h: &mut Harness<'_>, text: &str) {
    h.event(Event::Text(text.to_owned()));
    h.run_steps(2);
}

/// How many nodes of the accessibility tree have a label (or text) containing `text`.
fn count(h: &Harness<'_>, text: &str) -> usize {
    h.query_all_by_label_contains(text).count()
}

#[test]
fn a_side_panel_row_is_in_the_accessibility_tree_and_a_key_changes_it() {
    let mut app = GuiApp::with_shell(game(""));
    let mut h = harness(&mut app);

    // The run state and speed: one row of the side panel, painted as cells.
    h.get_by_label_contains("Running x1");
    assert_eq!(count(&h, "Paused"), 0);

    // Space is the pause key: the same row now says so.
    type_text(&mut h, " ");
    h.get_by_label_contains("Paused x1");
    assert_eq!(count(&h, "Running x1"), 0);

    // And again, back.
    type_text(&mut h, " ");
    h.get_by_label_contains("Running x1");
    drop(h);
    assert!(!app.shell().paused());
}

#[test]
fn the_panels_other_rows_are_found_too() {
    let mut app = GuiApp::with_shell(game(""));
    let h = harness(&mut app);
    h.get_by_label_contains("EARTH KINGDOM");
    h.get_by_label_contains("SETTLEMENT");
    h.get_by_label_contains("People 6");
}

#[test]
fn the_map_widgets_glyphs_are_not_labelled() {
    // The map is a widget of its own while a game is on screen: its glyphs are not exposed, and
    // the panel's rows are only the panel's columns.
    let mut app = GuiApp::with_shell(game(""));
    let h = harness(&mut app);
    assert_eq!(count(&h, "≈"), 0, "water glyphs from the map are in the tree");
    assert_eq!(count(&h, "▲"), 0, "mountain glyphs from the map are in the tree");
}

#[test]
fn a_full_screen_and_a_box_are_exposed_too() {
    // The Nation screen is cells over the whole window.
    let mut app = GuiApp::with_shell(game("n"));
    let h = harness(&mut app);
    assert!(count(&h, "NATION") >= 1 || count(&h, "Nation") >= 1);
    drop(h);

    // The game menu is a cell box over the native map; Quit is its last row.
    let mut app = GuiApp::with_shell(game("<esc>"));
    let h = harness(&mut app);
    h.get_by_label_contains("Quit");
}
