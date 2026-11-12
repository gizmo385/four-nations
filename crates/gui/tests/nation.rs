//! The Nation roster (#887): the words are the terminal's, a click on a settler is the keys
//! `Enter` stands for, and the needs are bars. Nothing here needs a GPU.

use eframe::egui;
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use fn_gui::app::GuiApp;
use fn_gui::frame::{PageKind, Surface};
use fn_gui::page;
use fn_gui::screens::nation::{click_keys, describe};
use fn_sim::Nation;
use fn_tui::shell::{Mode, Shell};
use fn_ui::keyev::KeyCode;
use fn_ui::view::{UnitView, nation_view};

const SIZE: (u16, u16) = (120, 40);

fn game(script: &str) -> Shell {
    let mut shell = Shell::new_game(fn_tui::new_session(7, Nation::Earth), SIZE, None);
    shell.play(&fn_ui::script::parse(script).unwrap());
    shell
}

/// A game with the first settler starving, on the Nation screen.
fn starving() -> Shell {
    let mut shell = game("");
    if let Mode::Game(app) = &mut shell.mode {
        app.session.world.units[0].needs.hunger = 0.0;
    }
    shell.play(&fn_ui::script::parse("n").unwrap());
    shell
}

fn harness(app: &mut GuiApp, width: f32) -> Harness<'_> {
    let mut h = Harness::builder()
        .with_size(egui::vec2(width, 960.0))
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

fn units(shell: &Shell) -> Vec<UnitView> {
    nation_view(&state(shell).session).groups.into_iter().flat_map(|(_, us)| us).collect()
}

/// `N` opens a native page: the title between two seals, the header and the grid's columns, one
/// button per settler with the terminal's words, the stores under them and the detail strip.
#[test]
fn the_roster_is_a_page_with_a_row_per_settler() {
    let mut app = GuiApp::with_shell(game("n"));
    let all = units(app.shell());
    assert!(all.len() > 1, "a settlement has people");
    let h = harness(&mut app, 1600.0);
    assert_eq!(h.query_all_by_label_contains("SETTLEMENT").count(), 0, "the side panel is hidden");
    h.get_by_label("◆ NATION ◆");
    for column in ["NAME", "DOING", "HUNGER", "THIRST", "ENERGY", "HEALTH", "BENDING"] {
        h.get_by_label_contains(column);
    }
    h.get_by_label_contains(&format!("IDLE {}", all.len()));
    for unit in &all {
        h.get_by_label(&describe(unit));
    }
    h.get_by_label("STORES");
    // The strip says how to go to the settler under the pointer.
    h.get_by_label_contains("Enter: go to them on the map");
    drop(h);
    assert_eq!(app.frame().surface, Some(Surface::Page(PageKind::Nation)));
}

/// The row's words say each need in percent, where the terminal has glyph meters (the starving
/// settler's task moves on within a frame, so the need is what is looked for).
#[test]
fn a_row_says_each_need_in_percent() {
    let shell = starving();
    let id = state(&shell).session.world.units[0].id;
    let first = units(&shell).into_iter().find(|u| u.id == id).expect("in the roster");
    let words = describe(&first);
    assert!(words.contains("Hunger 0%"), "{words}");
    assert!(words.contains("Health 100%"), "{words}");
    assert!(words.ends_with(&first.bending), "{words}");
    let mut app = GuiApp::with_shell(shell);
    let h = harness(&mut app, 1600.0);
    h.get_by_label_contains(&format!("{}, ", first.name));
    h.get_by_label_contains("Hunger 0%, Thirst 100%, Energy 100%, Health 100%, Student");
}

/// A click on a settler's row selects them as `Enter` does: the same state as the keys, the
/// screen closed, the settler selected.
#[test]
fn a_click_on_a_settler_selects_them_as_enter_does() {
    let all = units(&game("n"));
    let n = all.len() - 1;
    let target = all[n].clone();

    let mut app = GuiApp::with_shell(game("n"));
    let mut h = harness(&mut app, 1600.0);
    h.get_by_label(&describe(&target)).click();
    h.run_steps(3);
    drop(h);
    let clicked = state(app.shell());
    assert_eq!(clicked.ui.screen, None);
    assert_eq!(clicked.ui.selected, Some(target.id));
    assert_eq!(app.frame().surface, Some(Surface::Native));

    let by_hand = game(&format!("n <down*{n}> <enter>"));
    assert_eq!(state(&by_hand).ui.selected, clicked.ui.selected);
    assert_eq!(state(&by_hand).ui.cursor, clicked.ui.cursor);
}

/// The keys a click stands for: the arrows that put the pointer there, then `Enter`.
#[test]
fn the_keys_of_a_click_step_the_pointer_then_press_enter() {
    let codes = |at, to| click_keys(at, to).iter().map(|k| k.code).collect::<Vec<_>>();
    assert_eq!(codes(2, 2), [KeyCode::Enter]);
    assert_eq!(codes(0, 2), [KeyCode::Down, KeyCode::Down, KeyCode::Enter]);
    assert_eq!(codes(3, 2), [KeyCode::Up, KeyCode::Enter]);
}

/// The arrow keys still step the pointer.
#[test]
fn the_arrow_keys_step_the_pointer() {
    let mut app = GuiApp::with_shell(game("n"));
    let mut h = harness(&mut app, 1600.0);
    h.key_press(egui::Key::ArrowDown);
    h.run_steps(3);
    drop(h);
    assert_eq!(state(app.shell()).ui.nation_cursor, 1);
}

/// The close button is `Esc`.
#[test]
fn the_close_button_closes_the_screen() {
    let mut app = GuiApp::with_shell(game("n"));
    let mut h = harness(&mut app, 1600.0);
    h.get_by_label(page::CLOSE).click();
    h.run_steps(3);
    drop(h);
    assert_eq!(state(app.shell()).ui.screen, None);
}

/// Narrower than the meters need, a row flags what is wrong in words, as the terminal does, and
/// a click still selects.
#[test]
fn a_narrow_window_flags_trouble_in_words_and_still_selects() {
    let mut app = GuiApp::with_shell(starving());
    let mut h = harness(&mut app, 600.0);
    h.get_by_label_contains("Starving!").click();
    h.run_steps(3);
    drop(h);
    assert_eq!(state(app.shell()).ui.screen, None);
    assert!(state(app.shell()).ui.selected.is_some());
}
