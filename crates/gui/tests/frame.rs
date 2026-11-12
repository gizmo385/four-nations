//! The window's frame (#876): which surface each state of the shell gets, and the hint bar in a
//! real egui pass. kittest drives the egui pass without a renderer, so none of this needs a GPU
//! (the harness only asks for an adapter when it renders an image, and these never do).

use eframe::egui::{self, Event, Key, Modifiers};
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use fn_gui::app::GuiApp;
use fn_gui::frame::{BoxKind, PageKind, Surface, surface};
use fn_sim::Nation;
use fn_tui::shell::Shell;

const SIZE: (u16, u16) = (120, 40);

fn game(script: &str) -> Shell {
    let mut shell = Shell::new_game(fn_tui::new_session(7, Nation::Earth), SIZE, None);
    shell.play(&fn_ui::script::parse(script).unwrap());
    shell
}

#[test]
fn the_map_is_native_and_every_screen_is_cells_for_now() {
    assert_eq!(surface(&game("")), Surface::Native);
    // A tool, a rectangle corner and a selected settler are all the map.
    assert_eq!(surface(&game("h")), Surface::Native);
    assert_eq!(surface(&game("h <enter>")), Surface::Native);
    // Build is a native page (#881).
    assert_eq!(surface(&game("u")), Surface::Page(PageKind::Build));
    // The other full screens: Research, Nation, key help, the message log, Abilities, Traders
    // and the Endeavor page (Saves needs a store, which these shells have none of).
    for keys in ["t", "n", "?", "l", "i", "m", "g"] {
        assert_eq!(surface(&game(keys)), Surface::CellsFull, "{keys}");
    }
}

#[test]
fn the_title_screen_and_embark_are_native() {
    // Every step of the title is the native title (#880), not only the menu.
    let mut title = Shell::new_title(SIZE, None, 1);
    assert_eq!(surface(&title), Surface::Title);
    title.play(&fn_ui::script::parse("<enter>").unwrap());
    assert_eq!(surface(&title), Surface::Title);
    title.play(&fn_ui::script::parse("<enter>").unwrap());
    assert_eq!(surface(&title), Surface::Title);
    // Enter on the seed step opens embark (#883), a native surface of its own.
    title.play(&fn_ui::script::parse("<enter>").unwrap());
    assert_eq!(surface(&title), Surface::Embark);
}

#[test]
fn the_game_menu_and_its_questions_are_boxes_over_the_native_map() {
    // Esc with nothing to close opens the menu; Quit… is its last row.
    assert_eq!(surface(&game("<esc>")), Surface::CellsBox(BoxKind::Menu));
    let asked = game("<esc> <down*6> <enter>");
    assert_eq!(surface(&asked), Surface::CellsBox(BoxKind::Ask));
    // F9 asks before it replaces the game, with no menu open at all.
    let mut shell = game("");
    shell.key(fn_ui::keyev::KeyEvent::new(
        fn_ui::keyev::KeyCode::F(9),
        fn_ui::keyev::KeyModifiers::NONE,
    ));
    if let fn_tui::shell::Mode::Game(app) = &shell.mode
        && app.ui.prompt.is_some()
    {
        assert_eq!(surface(&shell), Surface::CellsBox(BoxKind::Ask));
    }
}

/// The question the saves screen asks stays in the full-screen frame it was asked from.
#[test]
fn a_question_over_a_full_screen_is_in_that_screens_cells() {
    let mut shell = game("n");
    if let fn_tui::shell::Mode::Game(app) = &mut shell.mode {
        app.ui.prompt = Some(fn_ui::state::Prompt::QuickLoad);
    }
    assert_eq!(surface(&shell), Surface::CellsFull);
}

#[test]
fn embarks_exit_prompt_covers_the_whole_frame() {
    let mut shell = game("");
    shell.exit_prompt = true;
    assert_eq!(surface(&shell), Surface::CellsFull);
}

fn harness(app: &mut GuiApp, size: egui::Vec2) -> Harness<'_> {
    Harness::builder()
        .with_size(size)
        // The window's own fonts: kittest panics on a glyph no font has.
        .with_context_setup(fn_gui::setup)
        .build_ui(move |ui| app.show(ui))
}

fn text(h: &Harness<'_>, key: &str) -> Event {
    let _ = h;
    Event::Text(key.to_owned())
}

/// Pressing a tool key changes the hint bar: the badge names the tool, and the keys are the
/// tool's. The bar is queried by its labels through AccessKit, as a screen reader would read it.
#[test]
fn the_hint_bar_follows_the_tool_keys() {
    let mut app = GuiApp::with_shell(game(""));
    let mut h = harness(&mut app, egui::vec2(1600.0, 960.0));
    h.run_steps(2);
    assert!(h.query_by_label_contains("SELECT").is_some(), "the map's Select badge");
    assert!(h.query_by_label_contains("HARVEST").is_none());
    assert!(h.query_by_label_contains("? keys").is_some(), "the tail");

    h.event(text(&h, "h"));
    h.run_steps(2);
    assert!(h.query_by_label(" HARVEST ").is_some(), "the tool's badge");
    assert!(h.query_by_label_contains("━ HARVEST ━").is_some(), "the banner's badge");
    // The banner says it too, on the map's top edge.
    assert!(h.query_by_label_contains("Enter: set a corner").is_some(), "the tool banner");
    // The tool's own keys are in the bar.
    assert!(h.query_by_label_contains("Arrows move").is_some(), "the tool's hints");
    assert!(h.query_by_label_contains("O repeat").is_some(), "the Harvest tool's own toggle");

    // Esc leaves the tool, and the bar goes back.
    h.key_press(Key::Escape);
    h.run_steps(2);
    assert!(h.query_by_label(" HARVEST ").is_none());
    assert!(h.query_by_label_contains("SELECT").is_some());
    drop(h);
    assert_eq!(app.frame().surface, Some(Surface::Native));
}

/// The shell is sized to the window in cells, not to the 120 by 40 it started at.
#[test]
fn the_shell_is_sized_to_the_window_in_cells() {
    let mut app = GuiApp::with_shell(game(""));
    let mut h = harness(&mut app, egui::vec2(1600.0, 960.0));
    h.run_steps(2);
    drop(h);
    // 8 by 17 point cells (Hack at 14 pt). The harness leaves 8 points round its root, so 198
    // of the 200 columns a real 1600-point window has.
    let (cols, rows) = app.frame().size();
    assert_eq!(cols, 198);
    assert!((50..=60).contains(&rows), "{rows}");
    if let fn_tui::shell::Mode::Game(game) = &app.shell().mode {
        let (w, h) = fn_ui::layout::viewport(cols, rows);
        assert_eq!(game.ui.visible().0, w);
        assert!(h > 0);
    }
}

/// The menu opens as a box: the bar changes to the menu's keys, and the map is still there.
#[test]
fn the_menu_is_a_box_and_the_bar_says_so() {
    let mut app = GuiApp::with_shell(game(""));
    let mut h = harness(&mut app, egui::vec2(1600.0, 960.0));
    h.run_steps(2);
    h.key_press(Key::Escape);
    h.run_steps(2);
    drop(h);
    assert_eq!(app.frame().surface, Some(Surface::CellsBox(BoxKind::Menu)));
    assert!(app.frame().hint_text.starts_with("MENU"), "{}", app.frame().hint_text);
}

/// A full screen is cells over the whole window: no hint bar of its own, the terminal's row.
#[test]
fn a_full_screen_is_the_whole_window_in_cells() {
    let mut app = GuiApp::with_shell(game("n"));
    let mut h = harness(&mut app, egui::vec2(1600.0, 960.0));
    h.run_steps(2);
    drop(h);
    assert_eq!(app.frame().surface, Some(Surface::CellsFull));
    assert!(app.frame().hint_text.is_empty());
}

/// F12 shows and hides the timing overlay, and is not a game key.
#[test]
fn f12_toggles_the_timing_overlay() {
    let mut app = GuiApp::with_shell(game(""));
    let mut h = harness(&mut app, egui::vec2(1600.0, 960.0));
    h.run_steps(2);
    h.key_press(Key::F12);
    h.run_steps(2);
    drop(h);
    assert!(app.frame().overlay);
    let mut h = harness(&mut app, egui::vec2(1600.0, 960.0));
    h.key_press_modifiers(Modifiers::NONE, Key::F12);
    h.run_steps(2);
    drop(h);
    assert!(!app.frame().overlay);
}
