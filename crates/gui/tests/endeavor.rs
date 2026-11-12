//! The Endeavor screen (#887): `fn_ui::endeavor::screen_view` in the page frame, a bar per sign,
//! and the gate's action as a button that presses its key. Nothing here needs a GPU.

use eframe::egui;
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use fn_gui::app::GuiApp;
use fn_gui::frame::{PageKind, Surface};
use fn_gui::page;
use fn_gui::screens::endeavor::{action_key, describe};
use fn_sim::Nation;
use fn_tui::shell::{Mode, Shell};
use fn_ui::endeavor::screen_view;
use fn_ui::gate::GateAction;
use fn_ui::keyev::KeyCode;

const SIZE: (u16, u16) = (120, 40);

fn game(script: &str) -> Shell {
    let mut shell = Shell::new_game(fn_tui::new_session(7, Nation::Earth), SIZE, None);
    shell.play(&fn_ui::script::parse(script).unwrap());
    shell
}

/// A game with the endeavor latched Ready, as if every sign had been met (a save round trip
/// with the state spliced in; meeting every sign takes a whole settlement).
fn ready(script: &str) -> Shell {
    let mut s = fn_tui::new_session(7, Nation::Earth);
    let text = s.world.save(&s.defs).unwrap().replacen("Dormant", "Ready(tick:0)", 1);
    s.world = fn_sim::World::load(&text, &s.defs).expect("round trip");
    let mut shell = Shell::new_game(s, SIZE, None);
    shell.play(&fn_ui::script::parse(script).unwrap());
    shell
}

fn harness(app: &mut GuiApp, height: f32) -> Harness<'_> {
    let mut h = Harness::builder()
        .with_size(egui::vec2(1600.0, height))
        .with_context_setup(fn_gui::setup)
        .build_ui(move |ui| app.show(ui));
    h.run_steps(3);
    h
}

fn state(shell: &Shell) -> &fn_ui::app::App {
    match &shell.mode {
        Mode::Game(game) => game,
        Mode::Title(_) | Mode::Embark(_) => panic!("not in a game"),
    }
}

/// `G` opens a native page: the title between two seals, the endeavor's name and count, its
/// stage, every group's heading and every sign with the terminal's words.
#[test]
fn endeavor_is_a_page_of_signs_under_their_headings() {
    let mut app = GuiApp::with_shell(game("g"));
    let view = screen_view(&state(app.shell()).session);
    let h = harness(&mut app, 2400.0);
    assert_eq!(h.query_all_by_label_contains("SETTLEMENT").count(), 0, "the side panel is hidden");
    h.get_by_label("◆ ENDEAVOR ◆");
    assert!(h.query_all_by_label_contains(&view.name).count() > 0);
    assert!(h.query_all_by_label_contains(&view.count).count() > 0);
    h.get_by_label(&view.stage);
    for group in &view.groups {
        h.get_by_label(&group.heading.to_uppercase());
        for sign in &group.signs {
            assert!(h.query_all_by_label(&describe(sign)).count() > 0, "{}", sign.name);
            // An unmet sign says what would help, under it.
            if let Some(hint) = &sign.hint {
                assert!(h.query_all_by_label_contains(hint).count() > 0, "{hint}");
            }
        }
    }
    drop(h);
    assert_eq!(app.frame().surface, Some(Surface::Page(PageKind::Endeavor)));
}

/// Before the endeavor is Ready there is nothing to do with the gate, so no button.
#[test]
fn a_dormant_endeavor_offers_no_gate_action() {
    let mut app = GuiApp::with_shell(game("g"));
    let h = harness(&mut app, 960.0);
    for action in [GateAction::Place, GateAction::Withdraw] {
        assert!(h.query_by_label(action.hint()).is_none(), "{action:?}");
    }
}

/// Once Ready the screen offers a site as a button, and clicking it is `Enter`: the Gate tool
/// is armed on the map, as the key does in the terminal.
#[test]
fn the_gate_button_is_enter() {
    let mut app = GuiApp::with_shell(ready("g"));
    let mut h = harness(&mut app, 960.0);
    h.get_by_label(GateAction::Place.hint()).click();
    h.run_steps(3);
    drop(h);
    let by_hand = ready("g <enter>");
    let (a, b) = (state(app.shell()), state(&by_hand));
    assert_eq!(a.ui.screen, b.ui.screen);
    assert_eq!(a.ui.tool, b.ui.tool);
    assert_eq!(a.ui.screen, None, "the screen closes to place the site");
}

#[test]
fn a_gate_action_is_the_key_it_names() {
    assert_eq!(action_key(GateAction::Place).code, KeyCode::Enter);
    assert_eq!(action_key(GateAction::Withdraw).code, KeyCode::Char('x'));
}

/// The arrow keys scroll the page: the shell's scroll moves, and a window too short for every
/// sign says where it is on the frame's edge.
#[test]
fn the_page_scrolls_with_the_keys_and_says_where_it_is() {
    let mut app = GuiApp::with_shell(game("g"));
    let mut h = harness(&mut app, 400.0);
    assert!(h.query_all_by_label_contains(" of ").count() > 0);
    h.key_press(egui::Key::ArrowDown);
    h.run_steps(3);
    drop(h);
    assert_eq!(state(app.shell()).ui.scroll, 1);
}

/// The close button is `Esc`.
#[test]
fn the_close_button_closes_the_screen() {
    let mut app = GuiApp::with_shell(game("g"));
    let mut h = harness(&mut app, 960.0);
    h.get_by_label(page::CLOSE).click();
    h.run_steps(3);
    drop(h);
    assert_eq!(state(app.shell()).ui.screen, None);
}
