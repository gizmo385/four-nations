//! The window's frame (#876): which surface each state of the shell gets, and the hint bar in a
//! real egui pass. kittest drives the egui pass without a renderer, so none of this needs a GPU
//! (the harness only asks for an adapter when it renders an image, and these never do).

use eframe::egui::{self, Event, Key, Modifiers};
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use fn_gui::app::GuiApp;
use fn_gui::frame::{BoxKind, PageKind, Surface, modal, surface};
use fn_sim::Nation;
use fn_tui::shell::Shell;

const SIZE: (u16, u16) = (120, 40);

fn game(script: &str) -> Shell {
    let mut shell = Shell::new_game(fn_tui::new_session(7, Nation::Earth), SIZE, None);
    shell.play(&fn_ui::script::parse(script).unwrap());
    shell
}

#[test]
fn the_map_and_every_full_screen_are_native() {
    assert_eq!(surface(&game("")), Surface::Native);
    // A tool, a rectangle corner and a selected settler are all the map.
    assert_eq!(surface(&game("h")), Surface::Native);
    assert_eq!(surface(&game("h <enter>")), Surface::Native);
    // Build is a native page (#881).
    assert_eq!(surface(&game("u")), Surface::Page(PageKind::Build));
    // So is Research (#884), box and all.
    assert_eq!(surface(&game("t")), Surface::Page(PageKind::Research));
    // So are Abilities and Traders (#886), the Nation roster and the Endeavor page (#887).
    assert_eq!(surface(&game("i")), Surface::Page(PageKind::Abilities));
    assert_eq!(surface(&game("m")), Surface::Page(PageKind::Traders));
    assert_eq!(surface(&game("n")), Surface::Page(PageKind::Nation));
    assert_eq!(surface(&game("g")), Surface::Page(PageKind::Endeavor));
    // And the message log and the key help (#888).
    assert_eq!(surface(&game("l")), Surface::Page(PageKind::Log));
    assert_eq!(surface(&game("?")), Surface::Page(PageKind::Help));
    // Saves needs a store, which these shells have none of; its page is tested in saves.rs.
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
    // No box with nothing open.
    assert_eq!(modal(&game("")), None);
    // Esc with nothing to close opens the menu; Quit… is its last row.
    let menu = game("<esc>");
    assert_eq!(surface(&menu), Surface::Native, "the map stays under the box");
    assert_eq!(modal(&menu), Some(BoxKind::Menu));
    let asked = game("<esc> <down*6> <enter>");
    assert_eq!(surface(&asked), Surface::Native);
    assert_eq!(modal(&asked), Some(BoxKind::Ask), "the question hides the menu behind it");
    // F9 asks before it replaces the game, with no menu open at all.
    let mut shell = game("");
    shell.key(fn_ui::keyev::KeyEvent::new(
        fn_ui::keyev::KeyCode::F(9),
        fn_ui::keyev::KeyModifiers::NONE,
    ));
    if let fn_tui::shell::Mode::Game(app) = &shell.mode
        && app.ui.prompt.is_some()
    {
        assert_eq!(modal(&shell), Some(BoxKind::Ask));
    }
}

/// A question asked from a full screen (the saves screen's) or a page is a box over it, and the
/// screen stays under it.
#[test]
fn a_question_over_a_screen_or_a_page_is_a_box_over_it() {
    for (keys, base) in [
        ("?", Surface::Page(PageKind::Help)),
        ("u", Surface::Page(PageKind::Build)),
        ("n", Surface::Page(PageKind::Nation)),
    ] {
        let mut shell = game(keys);
        if let fn_tui::shell::Mode::Game(app) = &mut shell.mode {
            app.ui.prompt = Some(fn_ui::state::Prompt::QuickLoad);
        }
        assert_eq!(surface(&shell), base, "{keys}");
        assert_eq!(modal(&shell), Some(BoxKind::Ask), "{keys}");
    }
}

#[test]
fn embarks_exit_prompt_is_a_box_over_embark() {
    let defs = fn_sim::Defs::bundled();
    let mut shell = Shell::new_embark(
        7,
        Nation::Earth,
        defs.embark.map_sizes[0].clone(),
        fn_sim::embark::SurveyMode::Exact,
        None,
        SIZE,
        None,
    );
    assert_eq!(modal(&shell), None);
    shell.exit_prompt = true;
    assert_eq!(surface(&shell), Surface::Embark);
    assert_eq!(modal(&shell), Some(BoxKind::Exit));
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
    assert_eq!(app.frame().surface, Some(Surface::Native));
    assert_eq!(app.frame().modal, Some(BoxKind::Menu));
    assert!(app.frame().hint_text.starts_with("MENU"), "{}", app.frame().hint_text);
}

/// What is still the terminal's (the LOST TEACHING box, #743) is cells over the whole window:
/// no hint bar of its own, the terminal's row. Every full screen is a native page now (#888).
#[test]
fn a_full_screen_is_the_whole_window_in_cells() {
    let mut shell = game("");
    if let fn_tui::shell::Mode::Game(app) = &mut shell.mode {
        app.ui.modal = Some(fn_ui::state::Modal::Teaching);
    }
    let mut app = GuiApp::with_shell(shell);
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

/// Watching a replay (#996): the side panel's keys and the map's clicks reach nothing. The frame
/// hands back no keys, and the game the frame shows is left as it was.
#[test]
fn a_watch_only_frame_takes_no_clicks() {
    use fn_gui::frame::Frame;
    use fn_tui::color::{ColorDepth, Palette};

    let mut shell = game("h");
    let palette = Palette::new(shell.defs(), Nation::Earth, ColorDepth::TrueColor, false);
    let mut frame = Frame::watching();
    let (cursor, log) = match &shell.mode {
        fn_tui::shell::Mode::Game(app) => (app.ui.cursor, app.session.world.command_log().len()),
        _ => unreachable!("a game"),
    };
    let mut h = Harness::builder()
        .with_size(egui::vec2(1400.0, 800.0))
        .with_context_setup(fn_gui::setup)
        .build_ui(|ui| frame.show(ui, &mut shell, &palette));
    h.run_steps(2);
    h.get_by_label("[T]").click();
    h.run_steps(2);
    let middle = egui::pos2(500.0, 400.0);
    for pressed in [true, false] {
        h.event(Event::PointerButton {
            pos: middle,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: Modifiers::NONE,
        });
        h.run_steps(1);
    }
    drop(h);
    assert!(frame.take_keys().is_empty());
    assert_eq!(frame.surface, Some(Surface::Native));
    let fn_tui::shell::Mode::Game(app) = &shell.mode else { unreachable!("a game") };
    assert_eq!((app.ui.cursor, app.session.world.command_log().len()), (cursor, log));
}
