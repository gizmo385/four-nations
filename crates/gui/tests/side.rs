//! The native side panel (#877): its words are the terminal's, its clicks are keys, and a click
//! and the key it stands for leave the same game. Like `frame.rs`, none of it needs a GPU.

use eframe::egui::{self, Event};
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use fn_gui::app::GuiApp;
use fn_gui::frame::{BoxKind, Surface};
use fn_gui::side;
use fn_sim::Nation;
use fn_tui::color::{ColorDepth, Palette};
use fn_tui::shell::{Mode, Shell};
use fn_ui::state::Screen;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;

const SIZE: (u16, u16) = (120, 40);

fn game(script: &str) -> Shell {
    let mut shell = Shell::new_game(fn_tui::new_session(7, Nation::Earth), SIZE, None);
    shell.play(&fn_ui::script::parse(script).unwrap());
    shell
}

/// A game that has run long enough for the research alert to open (a target nobody set).
fn game_with_an_alert() -> Shell {
    let mut shell = game("");
    for _ in 0..600 {
        shell.advance(0.5);
    }
    shell
}

fn harness(app: &mut GuiApp) -> Harness<'_> {
    let mut h = Harness::builder()
        .with_size(egui::vec2(1600.0, 960.0))
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

fn palette(app: &GuiApp) -> Palette {
    Palette::new(app.shell().defs(), Nation::Earth, ColorDepth::TrueColor, false)
}

/// The terminal's panel for the same game, as text, wide and tall enough that nothing wraps or
/// folds.
fn terminal_panel(app: &GuiApp) -> String {
    let game = state(app);
    let area = Rect::new(0, 0, 110, 60);
    let mut buf = Buffer::empty(area);
    fn_tui::panel::draw_panel(&game.session, &game.ui, &palette(app), area, &mut buf);
    fn_tui::frame::to_text(&buf)
}

fn typed(h: &mut Harness<'_>, keys: &str) {
    for c in keys.chars() {
        h.event(Event::Text(c.to_string()));
        h.run_steps(2);
    }
}

/// Every label the model has for a fixed session is a label of the window's panel and a piece of
/// the terminal's.
#[test]
fn the_panels_words_are_the_terminals() {
    let mut app = GuiApp::with_shell(game_with_an_alert());
    let model = side::model(state(&app), &palette(&app));
    let terminal = terminal_panel(&app);
    let h = harness(&mut app);

    let s = &model.settlement;
    let mut words = vec![model.title.clone(), model.footer.clone()];
    words.extend(model.header.iter().map(|r| r.left[0].0.clone()));
    let rows = [Some(&s.people), s.beasts.as_ref(), s.need_flags.as_ref(), Some(&s.food)]
        .into_iter()
        .flatten()
        .chain(s.winter.iter())
        .chain(Some(&s.research))
        .chain(s.research_bill.iter())
        .chain(&s.gate);
    for row in rows {
        words.push(row.left.iter().map(|(t, _)| t.as_str()).collect());
        words.extend(row.right.iter().map(|(t, _)| t.clone()));
    }
    words.extend(s.materials.iter().cloned());
    words.extend(model.alerts.entries.iter().map(|a| a.text.clone()));
    words.retain(|w| !w.trim().is_empty());
    assert!(words.len() > 8, "{words:?}");
    for word in &words {
        assert!(h.query_by_label(word.trim_end()).is_some(), "the window lacks {word:?}");
        assert!(terminal.contains(word.trim()), "the terminal lacks {word:?}\n{terminal}");
    }
    // The headings are the model's, and in its order.
    let headings = [&s.title, &model.alerts.title];
    for heading in headings {
        assert!(h.query_by_label(heading.as_str()).is_some(), "{heading}");
        assert!(terminal.contains(heading.as_str()), "{heading}");
    }
    assert!(terminal.find(&s.title) < terminal.find(&model.alerts.title));
}

/// Clicking a section's key does what the key does.
#[test]
fn clicking_t_opens_research() {
    let mut app = GuiApp::with_shell(game(""));
    let mut h = harness(&mut app);
    // A fresh settlement has no target, so the glance says so, with its key.
    h.get_by_label("[T]").click();
    h.run_steps(3);
    drop(h);
    assert_eq!(state(&app).ui.screen, Some(Screen::Research));
    assert_eq!(app.frame().surface, Some(Surface::Page(fn_gui::frame::PageKind::Research)));
}

#[test]
fn the_other_section_keys_open_their_screens() {
    for (label, screen) in
        [("[N]", Screen::Nation), ("[G]", Screen::Endeavor), ("[L]", Screen::Log)]
    {
        let mut app = GuiApp::with_shell(game(""));
        let mut h = harness(&mut app);
        h.get_by_label(label).click();
        h.run_steps(3);
        drop(h);
        assert_eq!(state(&app).ui.screen, Some(screen), "{label}");
    }
}

/// What the clock and the world hold: what two ways of pausing must agree on.
fn snapshot(app: &GuiApp) -> (bool, u32, String, usize) {
    let s = &state(app).session;
    let log = s.world.command_log();
    (s.clock.paused, s.clock.speed(), format!("{log:?}"), log.len())
}

/// A click on the header's pause button and a press of `Space` leave the same game: the same
/// clock and the same command log, after a designated rectangle gave the log something in it.
#[test]
fn clicking_pause_is_pressing_space() {
    let script = "h <left*6> <up*3> <enter> <right*5> <down*4> <enter> <esc>";
    let mut keyed = GuiApp::with_shell(game(script));
    let mut h = harness(&mut keyed);
    typed(&mut h, " ");
    drop(h);

    let mut clicked = GuiApp::with_shell(game(script));
    let mut h = harness(&mut clicked);
    // A running game offers Pause, a paused one Play.
    h.get_by_label("Pause").click();
    h.run_steps(2);
    drop(h);

    let (keyed, clicked) = (snapshot(&keyed), snapshot(&clicked));
    assert!(clicked.3 > 0, "the rectangle left commands in the log");
    assert_eq!(keyed, clicked);
    // And it did pause: a fresh game runs.
    assert!(clicked.0);
}

/// `+` and `-` in the header are the speed keys.
#[test]
fn the_speed_buttons_are_plus_and_minus() {
    let mut keyed = GuiApp::with_shell(game(""));
    let mut h = harness(&mut keyed);
    typed(&mut h, "++-");
    drop(h);
    let mut clicked = GuiApp::with_shell(game(""));
    let mut h = harness(&mut clicked);
    for label in ["+", "+", "-"] {
        h.get_by_label(label).click();
        h.run_steps(2);
    }
    drop(h);
    assert_eq!(snapshot(&keyed), snapshot(&clicked));
    assert_ne!(snapshot(&clicked).1, snapshot(&GuiApp::with_shell(game(""))).1);
}

/// An open alert goes to its line in the message log. This one, a research target nobody set,
/// names no place, so the log stays open on its line, as `L` and the arrows leave it.
#[test]
fn clicking_an_alert_is_the_logs_keys_on_its_line() {
    let mut app = GuiApp::with_shell(game_with_an_alert());
    let open = state(&app).ui.alerts.open();
    assert_eq!(open.len(), 1, "{open:?}");
    let keys = side::alert_keys(&state(&app).ui, &open[0]);
    assert!(!keys.is_empty());

    // The same keys by hand.
    let mut by_hand = game_with_an_alert();
    for key in keys {
        by_hand.key(key);
    }
    let Mode::Game(by_hand) = &by_hand.mode else { panic!("a game") };

    let mut h = harness(&mut app);
    h.get_by_label_contains("No research target is set").click();
    h.run_steps(2);
    drop(h);
    let clicked = state(&app);
    assert_eq!(clicked.ui.screen, Some(Screen::Log));
    assert_eq!(clicked.ui.log_cursor, by_hand.ui.log_cursor);
    assert_eq!(clicked.ui.screen, by_hand.ui.screen);
}

/// With a box over the map the panel still shows, but a click on it does nothing.
#[test]
fn a_box_takes_the_panels_clicks() {
    let mut app = GuiApp::with_shell(game("<esc>"));
    let mut h = harness(&mut app);
    h.get_by_label("[T]").click();
    h.run_steps(2);
    drop(h);
    assert_eq!(app.frame().surface, Some(Surface::CellsBox(BoxKind::Menu)));
    assert_eq!(state(&app).ui.screen, None);
}

/// The tool section follows the tool, in the tool's own words, and the list (`/`) is there
/// too; a click on a row of it picks the tool.
#[test]
fn the_tool_section_and_the_tool_list() {
    let mut app = GuiApp::with_shell(game(""));
    let mut h = harness(&mut app);
    assert!(h.query_by_label("HARVEST").is_none(), "no tool section in Select mode");
    typed(&mut h, "h");
    assert!(h.query_by_label("HARVEST").is_some(), "the tool's heading");
    assert!(h.query_by_label("[H]").is_some());
    assert!(h.query_by_label("Standing order: off").is_some(), "a toggle");
    typed(&mut h, "o");
    assert!(h.query_by_label("Standing order: on").is_some());
    drop(h);

    let mut app = GuiApp::with_shell(game("/"));
    let mut h = harness(&mut app);
    assert!(h.query_by_label("DESIGNATE").is_some(), "the list's heading");
    assert!(h.query_by_label("[/]").is_some());
    h.get_by_label_contains("Stockpile").click();
    h.run_steps(2);
    drop(h);
    assert_eq!(state(&app).ui.tool, fn_ui::Tool::Stockpile);
    assert!(state(&app).ui.tool_list.is_none(), "picking closes the list");
}

/// At the smallest window the panel's widgets stay inside its own column, in the panel's order.
#[test]
fn the_panels_widgets_stay_in_their_column_in_order() {
    let mut app = GuiApp::with_shell(game_with_an_alert());
    let mut h = Harness::builder()
        .with_size(egui::vec2(1024.0, 640.0))
        .with_context_setup(fn_gui::setup)
        .build_ui(move |ui| app.show(ui));
    h.run_steps(2);
    let rect = |label: &str| h.get_by_label(label).rect();
    let (settlement, alerts) = (rect("SETTLEMENT"), rect("ALERTS"));
    let column = 1024.0 - fn_gui::frame::PANEL_RANGE.1;
    for label in ["Pause", "+", "-", "[N]", "[T]", "SETTLEMENT", "ALERTS", "seed 7"] {
        let r = rect(label);
        assert!(r.min.x >= column && r.max.x <= 1024.0, "{label} at {r:?}");
        assert!(r.min.y >= 0.0 && r.max.y <= 640.0, "{label} at {r:?}");
    }
    assert!(rect("Pause").max.y <= settlement.min.y, "the header is above the glance");
    assert!(rect("[N]").center().x > settlement.max.x, "the key is at the heading's end");
    assert!(settlement.max.y <= alerts.min.y, "the glance is above the alerts");
    assert!(alerts.max.y <= rect("seed 7").min.y);
}

/// The inspector keeps its place under the native sections: the seed line closes the panel.
#[test]
fn the_inspector_slot_sits_under_the_sections() {
    let mut app = GuiApp::with_shell(game(""));
    let h = harness(&mut app);
    assert!(h.query_by_label("seed 7").is_some());
    assert!(h.query_by_label("SETTLEMENT").is_some());
}
