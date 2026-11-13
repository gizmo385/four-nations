//! The corner notices (#945): a caravan making camp stays in the map's corner until dismissed,
//! its Jump button centres the camera on the camp, and a toast's own button does what it says.
//! Like the rest of the window's tests, none of it needs a GPU.

use eframe::egui;
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use fn_gui::app::GuiApp;
use fn_sim::Nation;
use fn_tui::shell::{Mode, Shell};
use fn_ui::alerts::Kind;
use fn_ui::notice::{Group, NoticeAction};
use fn_ui::state::Screen;

const SIZE: (u16, u16) = (120, 40);

/// A game in which the Fire Nation traders have just made camp (their first visit is moved up
/// to a tenth of a minute in, as the alert tests do), with the window not yet drawn.
fn caravan_game() -> Shell {
    let mut shell = Shell::new_game(fn_tui::new_session(7, Nation::Earth), SIZE, None);
    let Mode::Game(app) = &mut shell.mode else { panic!("a game") };
    let src = include_str!("../../../data/trade.toml")
        .replacen("opens = \"known_on_the_road\"", "", 1)
        .replacen("offset_minutes = 4.0", "offset_minutes = 0.1", 1);
    app.session.defs = app.session.defs.clone().with_trade(&src).unwrap();
    // What the window does each frame; the shell is settled by hand here, as it is paused.
    app.ui.alerts.notices_mut().set_on(true);
    for _ in 0..100 {
        app.session.tick();
    }
    app.settle(0.0);
    assert!(app.session.world.caravan().is_some(), "the caravan is camped");
    // Paused, so the window's frames add no time of their own.
    app.session.clock.paused = true;
    shell
}

fn harness(app: &mut GuiApp) -> Harness<'_> {
    let mut h = Harness::builder()
        .with_size(egui::vec2(1600.0, 960.0))
        .with_context_setup(fn_gui::setup)
        .build_ui(move |ui| app.show(ui));
    h.run_steps(3);
    h
}

fn state(app: &GuiApp) -> &fn_ui::app::App {
    match &app.shell().mode {
        Mode::Game(game) => game,
        Mode::Title(_) | Mode::Embark(_) => panic!("not in a game"),
    }
}

#[test]
fn a_caravan_notice_appears_jumps_the_camera_and_is_dismissed() {
    let mut app = GuiApp::with_shell(caravan_game());
    let camp = state(&app).session.world.caravan().expect("camped").camp;
    let h = harness(&mut app);
    assert!(h.query_by_label("◆ Fire Nation traders").is_some(), "the notice is up");
    assert!(h.query_by_label("Jump to caravan").is_some());
    assert!(h.query_by_label("made camp").is_some());
    assert!(h.query_by_label("Trade [M]").is_some());
    drop(h);
    assert_eq!(state(&app).ui.alerts.notices().shown().count(), 1);
    assert!(state(&app).ui.alerts.toasts().is_empty(), "a notice, not a toast as well");

    let before = app.frame().camera().expect("the map has a camera").centre;
    let mut h = harness(&mut app);
    h.get_by_label("Jump to caravan").click();
    h.run_steps(3);
    // The notice stays: acting on it is not dismissing it.
    assert!(h.query_by_label("◆ Fire Nation traders").is_some());
    drop(h);
    let after = app.frame().camera().expect("the map has a camera").centre;
    assert_ne!(before, after, "the camera moved");
    assert_eq!(after, egui::vec2(camp.x as f32 + 0.5, camp.y as f32 + 0.5), "to the camp");
    assert_eq!(state(&app).ui.cursor, camp);

    let mut h = harness(&mut app);
    h.get_by_label("✕").click();
    h.run_steps(3);
    assert!(h.query_by_label("◆ Fire Nation traders").is_none(), "dismissed");
    drop(h);
    assert!(!state(&app).ui.alerts.notices().any());
}

/// `Esc` takes the newest notice down before it opens the menu.
#[test]
fn esc_dismisses_the_newest_notice_before_the_menu() {
    let mut app = GuiApp::with_shell(caravan_game());
    let mut h = harness(&mut app);
    assert!(h.query_by_label("◆ Fire Nation traders").is_some());
    h.key_press(egui::Key::Escape);
    h.run_steps(3);
    assert!(h.query_by_label("◆ Fire Nation traders").is_none(), "Esc closed it");
    assert!(h.query_by_label("Resume").is_none(), "and did not open the menu");
    h.key_press(egui::Key::Escape);
    h.run_steps(3);
    assert!(h.query_by_label("Resume").is_some(), "the next Esc is the menu's");
}

/// A notice stack is newest on top and keeps to its cap; "Open research" opens the screen.
#[test]
fn notices_stack_and_open_research() {
    let mut shell = caravan_game();
    let Mode::Game(game) = &mut shell.mode else { panic!("a game") };
    let notices = game.ui.alerts.notices_mut();
    for n in 0..fn_ui::notice::MAX_NOTICES + 1 {
        notices.push(
            (Kind::Tech, 1000 + n as u64),
            (format!("Research done: Tech {n}"), vec![]),
            vec![NoticeAction::OpenResearch],
            None,
        );
    }
    notices.push(
        (Kind::Trade, 2000),
        ("Late traders".into(), vec![]),
        vec![],
        Some(Group::Caravan),
    );
    assert_eq!(notices.shown().count(), fn_ui::notice::MAX_NOTICES, "the cap");
    assert!(notices.hidden() > 0, "the rest wait behind");
    let top = notices.shown().next().map(|n| n.title.clone());
    assert_eq!(top.as_deref(), Some("Late traders"), "newest on top");

    let mut app = GuiApp::with_shell(shell);
    let mut h = harness(&mut app);
    h.get_all_by_label("Open research").next().expect("a button").click();
    h.run_steps(3);
    drop(h);
    assert_eq!(state(&app).ui.screen, Some(Screen::Research));
}

/// An ordinary alert with an obvious target has a button too: no research target is set, so
/// "Open research" is there, and the click on the alert's words is still the message log.
#[test]
fn an_alert_offers_its_own_button() {
    let mut shell = Shell::new_game(fn_tui::new_session(7, Nation::Earth), SIZE, None);
    for _ in 0..600 {
        shell.advance(0.5);
    }
    let mut app = GuiApp::with_shell(shell);
    let mut h = harness(&mut app);
    h.get_by_label("Open research").click();
    h.run_steps(3);
    drop(h);
    assert_eq!(state(&app).ui.screen, Some(Screen::Research));
}

/// Past the cap a "+N more" row has "Dismiss all".
#[test]
fn the_rest_wait_behind_a_more_row_that_dismisses_all() {
    let mut shell = caravan_game();
    let Mode::Game(game) = &mut shell.mode else { panic!("a game") };
    for n in 0..fn_ui::notice::MAX_NOTICES + 2 {
        let face = (format!("Research done: Tech {n}"), vec![]);
        game.ui.alerts.notices_mut().push((Kind::Tech, n as u64), face, vec![], None);
    }
    let mut app = GuiApp::with_shell(shell);
    let mut h = harness(&mut app);
    assert!(h.query_by_label("+3 more").is_some(), "the caravan and five more, past four");
    h.get_by_label("Dismiss all").click();
    h.run_steps(3);
    drop(h);
    assert!(!state(&app).ui.alerts.notices().any());
}

/// In Select mode `Enter` runs the top notice's first action and `Del` dismisses the top one.
#[test]
fn enter_runs_the_top_notices_first_action_and_del_dismisses_it() {
    let mut app = GuiApp::with_shell(caravan_game());
    let camp = state(&app).session.world.caravan().expect("camped").camp;
    let mut h = harness(&mut app);
    h.key_press(egui::Key::Enter);
    h.run_steps(3);
    drop(h);
    assert_eq!(state(&app).ui.cursor, camp, "Enter jumped to the caravan");
    assert!(!state(&app).ui.alerts.notices().any(), "and the notice is done with");

    let mut shell = caravan_game();
    let Mode::Game(game) = &mut shell.mode else { panic!("a game") };
    let face = ("Research done: Mining".to_owned(), vec![]);
    game.ui.alerts.notices_mut().push((Kind::Tech, 9), face, vec![], None);
    let mut app = GuiApp::with_shell(shell);
    let mut h = harness(&mut app);
    h.key_press(egui::Key::Delete);
    h.run_steps(3);
    assert!(h.query_by_label("◆ Research done: Mining").is_none(), "Del took the top one down");
    assert!(h.query_by_label("◆ Fire Nation traders").is_some(), "and left the one under it");
}
