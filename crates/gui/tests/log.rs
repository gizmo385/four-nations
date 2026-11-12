//! The message log (#888): the lines are the terminal's, a click is the keys it stands for, and
//! `Enter` (or a double-click) goes to the line. Nothing here needs a GPU.

use eframe::egui::{self, Event, Modifiers, PointerButton};
use egui_kittest::Harness;
use egui_kittest::kittest::{NodeT, Queryable};
use fn_gui::app::GuiApp;
use fn_gui::frame::{PageKind, Surface};
use fn_gui::page;
use fn_gui::screens::log::click_keys;
use fn_sim::world::Command;
use fn_sim::{Nation, Pos};
use fn_tui::shell::{Mode, Shell};
use fn_ui::Session;
use fn_ui::alerts::Category;
use fn_ui::keyev::KeyCode;
use fn_ui::screens::msglog::lines;
use fn_ui::state::Screen;
use fn_ui::tools::ability_for_nation;

const SIZE: (u16, u16) = (120, 40);

/// A Fire game where the settlers have bent a good many times, so the log has runs to fold,
/// then a few minutes on.
fn bending_shell() -> Shell {
    let mut s = Session::new(3, Nation::Fire, 48, 32);
    let ability = ability_for_nation(&s.defs, Nation::Fire).unwrap();
    let from = s.world.units[0].pos;
    for dy in -5..=5 {
        for dx in -5..=5 {
            let pos = Pos::new(from.x + dx, from.y + dy);
            let _ = s.apply(Command::Bend { ability, pos });
        }
    }
    let mut shell = Shell::new_game(s, SIZE, None);
    for _ in 0..400 {
        shell.advance(0.25);
    }
    // The newest lines: a few the agent wrote, each with a time of its own.
    for n in 1..=24 {
        if let Mode::Game(app) = &mut shell.mode {
            let tick = app.session.world.tick;
            app.ui.alerts.push_agent(tick, format!("Agent: scouting the {n}th ridge"));
        }
        for _ in 0..8 {
            shell.advance(0.25);
        }
    }
    shell.play(&fn_ui::script::parse("l").unwrap());
    shell
}

fn harness(app: &mut GuiApp) -> Harness<'_> {
    let mut h = Harness::builder()
        .with_size(egui::vec2(1600.0, 960.0))
        // Each queued event is a frame of its own, so short ones: two clicks are a double-click
        // only if less than 0.3 s apart.
        .with_step_dt(0.02)
        .with_context_setup(fn_gui::setup)
        .build_ui(move |ui| app.show(ui));
    h.run_steps(2);
    h
}

fn state(app: &GuiApp) -> &fn_ui::app::App {
    state_of(app.shell())
}

fn state_of(shell: &Shell) -> &fn_ui::app::App {
    match &shell.mode {
        Mode::Game(game) => game,
        Mode::Title(_) | Mode::Embark(_) => panic!("not in a game"),
    }
}

/// Two quick presses at `centre`: a double-click for egui.
fn double_click(h: &mut Harness<'_>, centre: egui::Pos2) {
    h.event(Event::PointerMoved(centre));
    for _ in 0..2 {
        for pressed in [true, false] {
            h.event(Event::PointerButton {
                pos: centre,
                button: PointerButton::Primary,
                pressed,
                modifiers: Modifiers::NONE,
            });
        }
    }
    h.step();
}

/// The line's row, a button whose words are the stamp and the line's text.
fn row<'a>(h: &'a Harness<'_>, text: &str) -> egui_kittest::Node<'a> {
    h.query_all_by_role(egui::accesskit::Role::Button)
        .find(|n| {
            let label = n.accesskit_node().label().unwrap_or_default();
            label.ends_with(text) && label.contains(" left ")
        })
        .unwrap_or_else(|| panic!("no row for {text}"))
}

type Snapshot = (Option<(u64, bool)>, Option<Category>, Vec<u64>, Option<Screen>);

/// What the keyboard arrives at: the cursor, the filter, the open folds and the screen.
fn snapshot(shell: &Shell) -> Snapshot {
    let ui = &state_of(shell).ui;
    (ui.log_cursor.map(|k| (k.id, k.sub)), ui.log_filter, ui.log_open.clone(), ui.screen)
}

/// `L` opens a native page: the title, the filter chips, a season's heading, and the newest
/// line first, in the terminal's words.
#[test]
fn the_log_is_a_page_of_chips_headings_and_lines() {
    let mut app = GuiApp::with_shell(bending_shell());
    let all = lines(&state(&app).ui);
    assert!(all.len() > 3, "the game has logged: {}", all.len());
    // The heading where the stretch of game time begins (days or seasons, as the game has).
    let stamped = fn_ui::screens::msglog::stamps(&state(&app).session, &all);
    let h = harness(&mut app);
    assert_eq!(h.query_all_by_label_contains("SETTLEMENT").count(), 0, "the side panel is hidden");
    h.get_by_label("◆ MESSAGE LOG ◆");
    for chip in ["All", "1 Alerts", "2 People", "3 Knowledge", "4 Bending", "5 Work"] {
        h.get_by_label(chip);
    }
    h.get_by_label(stamped[0].heading.as_deref().expect("the first line has a heading"));
    row(&h, &all[0].text);
    drop(h);
    assert_eq!(app.frame().surface, Some(Surface::Page(PageKind::Log)));
}

/// A chip is its digit: the same filter the key sets, and the same cursor on the top line.
#[test]
fn a_filter_chip_is_its_digit() {
    let mut app = GuiApp::with_shell(bending_shell());
    let mut h = harness(&mut app);
    h.get_by_label("4 Bending").click();
    h.run_steps(3);
    drop(h);
    let mut by_hand = bending_shell();
    by_hand.play(&fn_ui::script::parse("4").unwrap());
    assert_eq!(state(&app).ui.log_filter, Some(Category::Bending));
    assert_eq!(snapshot(app.shell()), snapshot(&by_hand));

    // Its own chip again shows everything, and so does All.
    let mut h = harness(&mut app);
    h.get_by_label("All").click();
    h.run_steps(3);
    drop(h);
    assert_eq!(state(&app).ui.log_filter, None);
}

/// A click puts the cursor on the line, as the arrows do; a double-click goes to it, as
/// `Enter` does.
#[test]
fn a_click_moves_the_cursor_and_a_double_click_goes_to_the_line() {
    let mut app = GuiApp::with_shell(bending_shell());
    let all = lines(&state(&app).ui);
    // The first line about a place: Enter on it closes the log, as it goes there.
    let to = all.iter().position(|l| l.place.is_some()).expect("a bend has a place");
    assert!(to > 0, "the agent's lines are newest");
    let mut h = harness(&mut app);
    row(&h, &all[to].text).click();
    h.run_steps(3);
    drop(h);
    let mut by_hand = bending_shell();
    by_hand.play(&fn_ui::script::parse(&format!("<down*{to}>")).unwrap());
    assert_eq!(snapshot(app.shell()), snapshot(&by_hand));
    assert_eq!(state(&app).ui.log_cursor, Some(all[to].key));

    // Enter on the line leaves the screen if the line is about a place, or stays on it.
    let (mut app, mut by_hand) = (GuiApp::with_shell(bending_shell()), bending_shell());
    let mut h = harness(&mut app);
    let centre = row(&h, &all[to].text).rect().center();
    double_click(&mut h, centre);
    h.run_steps(3);
    drop(h);
    by_hand.play(&fn_ui::script::parse(&format!("<down*{to}> <enter>")).unwrap());
    assert_eq!(snapshot(app.shell()), snapshot(&by_hand));
    assert_eq!(state(&app).ui.screen, None, "the line is about a place, so the log closed");
}

/// A fold's `+` unfolds it as `Right` does, and its `-` folds it again.
#[test]
fn a_folds_sign_unfolds_and_folds_it() {
    let mut app = GuiApp::with_shell(bending_shell());
    let all = lines(&state(&app).ui);
    let at = all.iter().position(|l| l.is_fold()).expect("bending made a fold");
    let mut h = harness(&mut app);
    let sign = format!("Unfold {} lines", all[at].count);
    h.get_by_label(&sign).click();
    h.run_steps(3);
    drop(h);
    let mut by_hand = bending_shell();
    by_hand.play(&fn_ui::script::parse(&format!("<down*{at}> <right>")).unwrap());
    assert_eq!(snapshot(app.shell()), snapshot(&by_hand));
    assert!(state(&app).ui.log_open.contains(&all[at].key.id));

    let mut h = harness(&mut app);
    h.get_by_label(&format!("Fold {} lines", all[at].count)).click();
    h.run_steps(3);
    drop(h);
    by_hand.play(&fn_ui::script::parse("<left>").unwrap());
    assert_eq!(snapshot(app.shell()), snapshot(&by_hand));
    assert!(state(&app).ui.log_open.is_empty());
}

/// The keyboard keeps working on the native page, and the close button is `Esc`.
#[test]
fn the_arrows_still_move_the_cursor_and_close_is_esc() {
    let mut app = GuiApp::with_shell(bending_shell());
    let mut h = harness(&mut app);
    h.key_press(egui::Key::ArrowDown);
    h.run_steps(3);
    h.get_by_label(page::CLOSE).click();
    h.run_steps(3);
    drop(h);
    assert_eq!(state(&app).ui.screen, None);
    assert_eq!(app.frame().surface, Some(Surface::Native));
}

/// A short window shows a stretch of the lines and the frame's edge says which.
#[test]
fn a_short_window_scrolls_and_the_edge_says_where() {
    let mut app = GuiApp::with_shell(bending_shell());
    let total = lines(&state(&app).ui).len();
    let mut h = Harness::builder()
        .with_size(egui::vec2(900.0, 360.0))
        .with_context_setup(fn_gui::setup)
        .build_ui(|ui| app.show(ui));
    h.run_steps(3);
    h.get_by_label_contains(&format!("of {total}"));
}

/// The click keys are the arrows, up or down, and `Enter` last.
#[test]
fn click_keys_are_arrows_then_enter() {
    let codes = |keys: Vec<fn_ui::keyev::KeyEvent>| keys.iter().map(|k| k.code).collect::<Vec<_>>();
    assert_eq!(codes(click_keys(1, 3, false)), [KeyCode::Down, KeyCode::Down]);
    assert_eq!(codes(click_keys(3, 2, true)), [KeyCode::Up, KeyCode::Enter]);
    assert!(click_keys(2, 2, false).is_empty());
}

/// With nothing logged the page says so, in the terminal's words.
#[test]
fn an_empty_log_says_nothing_has_happened() {
    let mut shell = Shell::new_game(fn_tui::new_session(7, Nation::Earth), SIZE, None);
    shell.play(&fn_ui::script::parse("l").unwrap());
    let mut app = GuiApp::with_shell(shell);
    if !lines(&state(&app).ui).is_empty() {
        return;
    }
    let h = harness(&mut app);
    h.get_by_label_contains("Nothing has happened yet");
}
