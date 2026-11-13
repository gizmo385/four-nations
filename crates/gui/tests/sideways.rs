//! The turn-sideways screen (#975, docs/design/touch-pwa.md §1): an upright phone shows it, and
//! a running game pauses under it until the phone turns back.

use eframe::egui;
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use fn_gui::app::GuiApp;
use fn_gui::sideways::Sideways;
use fn_sim::Nation;
use fn_tui::shell::{Mode, Shell};

fn game(script: &str) -> Shell {
    let mut shell = Shell::new_game(fn_tui::new_session(7, Nation::Earth), (120, 40), None);
    shell.play(&fn_ui::script::parse(script).unwrap());
    shell
}

fn paused(shell: &Shell) -> bool {
    match &shell.mode {
        Mode::Game(app) => app.session.clock.paused,
        _ => panic!("not in a game"),
    }
}

#[test]
fn turning_upright_pauses_a_running_game_and_turning_back_resumes_it() {
    let mut shell = game("");
    let mut s = Sideways::default();
    assert!(!s.update(false, &mut shell));
    assert!(!paused(&shell));
    assert!(s.update(true, &mut shell));
    assert!(paused(&shell), "the screen paused it");
    assert!(s.update(true, &mut shell));
    assert!(paused(&shell));
    assert!(!s.update(false, &mut shell));
    assert!(!paused(&shell), "turning back starts it again");
}

#[test]
fn a_game_the_player_paused_stays_paused() {
    let mut shell = game("<space>");
    assert!(paused(&shell));
    let mut s = Sideways::default();
    s.update(true, &mut shell);
    s.update(false, &mut shell);
    assert!(paused(&shell), "the screen didn't pause it, so it doesn't start it");
}

#[test]
fn the_title_screen_shows_it_with_nothing_to_pause() {
    let mut shell = Shell::new_title((120, 40), None, 42);
    let mut s = Sideways::default();
    assert!(s.update(true, &mut shell));
    assert!(!s.update(false, &mut shell));
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
fn an_upright_window_shows_the_screen_and_a_wide_one_the_game() {
    let mut app = GuiApp::with_shell(game("")).with_upright_screen(true);
    let h = harness(&mut app, egui::vec2(390.0, 844.0));
    assert!(h.query_by_label_contains("Turn your phone sideways").is_some());
    assert!(h.query_by_label_contains("paused until you turn back").is_some());
    drop(h);
    assert!(paused(app.shell()));
    let h = harness(&mut app, egui::vec2(844.0, 390.0));
    assert!(h.query_by_label_contains("Turn your phone sideways").is_none());
    drop(h);
    assert!(!paused(app.shell()));
}

#[test]
fn a_desktop_window_is_never_told_to_turn() {
    let mut app = GuiApp::with_shell(game(""));
    let h = harness(&mut app, egui::vec2(390.0, 844.0));
    assert!(h.query_by_label_contains("Turn your phone sideways").is_none());
}

#[test]
fn the_title_screen_says_only_to_turn() {
    let mut app =
        GuiApp::with_shell(Shell::new_title((120, 40), None, 42)).with_upright_screen(true);
    let h = harness(&mut app, egui::vec2(390.0, 844.0));
    assert!(h.query_by_label_contains("Turn your phone sideways").is_some());
    assert!(h.query_by_label_contains("paused").is_none());
}
