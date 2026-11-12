//! The abilities screen (#886): the words are the terminal's, a click is the keys it stands for,
//! and a double-click is `Enter`, which picks the ability for the Bend tool. Nothing here needs
//! a GPU.

use eframe::egui::{self, Event, Modifiers, PointerButton};
use egui_kittest::Harness;
use egui_kittest::kittest::{NodeT, Queryable};
use fn_gui::app::GuiApp;
use fn_gui::frame::{PageKind, Surface};
use fn_gui::page;
use fn_gui::screens::abilities::click_keys;
use fn_sim::Nation;
use fn_tui::shell::{Mode, Shell};
use fn_ui::bend::{AbilityRow, abilities};
use fn_ui::keyev::KeyCode;
use fn_ui::state::Screen;
use fn_ui::tools::Tool;

const SIZE: (u16, u16) = (120, 40);

fn game(nation: Nation, script: &str) -> Shell {
    let mut shell = Shell::new_game(fn_tui::new_session(7, nation), SIZE, None);
    shell.play(&fn_ui::script::parse(script).unwrap());
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

fn state(shell: &Shell) -> &fn_ui::app::App {
    match &shell.mode {
        Mode::Game(game) => game,
        Mode::Title(_) | Mode::Embark(_) => panic!("not in a game"),
    }
}

/// The ability's row, a button with its name and the rest of its columns.
fn row<'a>(h: &'a Harness<'_>, row: &AbilityRow) -> egui_kittest::Node<'a> {
    h.query_all_by_role(egui::accesskit::Role::Button)
        .find(|n| n.accesskit_node().label().unwrap_or_default().contains(&row.name))
        .unwrap_or_else(|| panic!("no row for {}", row.name))
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

/// What the keyboard arrives at: the pointer, the screen, the tool and its ability, and the
/// command log.
fn snapshot(shell: &Shell) -> (usize, Option<Screen>, Tool, Option<u16>, String) {
    let app = state(shell);
    let log = app.session.world.command_log();
    let commands = format!("{:?}", log.iter().map(|r| &r.command).collect::<Vec<_>>());
    let choice = app.session.bend_choice.map(|id| id.0);
    (app.ui.ability_cursor, app.ui.screen, app.ui.tool, choice, commands)
}

/// `I` opens a native page: the title between two seals, a header over three columns, every
/// ability of the nation with what it needs or does, and the highlighted one in the strip.
#[test]
fn abilities_is_a_page_with_a_list_and_a_detail_strip() {
    for nation in Nation::ALL {
        let mut app = GuiApp::with_shell(game(nation, "i"));
        let rows = abilities(&state(app.shell()).session);
        let opening = rows[0].what.split_whitespace().take(3).collect::<Vec<_>>().join(" ");
        let h = harness(&mut app);
        assert_eq!(h.query_all_by_label_contains("SETTLEMENT").count(), 0, "the side panel");
        h.get_by_label("◆ ABILITIES ◆");
        h.get_by_label_contains("WHAT IT DOES");
        for r in &rows {
            row(&h, r);
        }
        // A locked row says what it needs in the last column.
        let locked = rows.iter().find(|r| !r.usable()).unwrap();
        let label = row(&h, locked).accesskit_node().label().unwrap_or_default();
        assert!(label.contains(locked.unusable.as_deref().unwrap()), "{nation:?}: {label}");
        // The strip holds the highlighted ability.
        // (The list's row has the words too, so look at the labels, not the buttons.)
        let strip = |words: &str| {
            h.query_all_by_role(egui::accesskit::Role::Label)
                .any(|n| n.accesskit_node().value().unwrap_or_default().contains(words))
        };
        assert!(strip(&opening), "{nation:?}: {opening}");
        assert!(strip("The Bend tool channels it now."), "{nation:?}");
        drop(h);
        assert_eq!(app.frame().surface, Some(Surface::Page(PageKind::Abilities)));
        assert!(app.frame().hint_text.starts_with("ABILITIES"), "{}", app.frame().hint_text);
    }
}

/// A click moves the highlight: the very keys the arrows are. On a locked ability the strip
/// says why `Enter` does nothing.
#[test]
fn a_click_moves_the_highlight_as_the_arrow_keys_do() {
    let mut app = GuiApp::with_shell(game(Nation::Fire, "i"));
    let rows = abilities(&state(app.shell()).session);
    let to = rows.iter().position(|r| !r.usable()).unwrap();
    assert!(to > 0);
    let mut h = harness(&mut app);
    row(&h, &rows[to]).click();
    h.run_steps(3);
    let why = rows[to].unusable.clone().unwrap();
    h.get_by_label_contains(&format!("{why}: Enter picks nothing until then."));
    drop(h);
    assert_eq!(state(app.shell()).ui.ability_cursor, to);
    assert_eq!(snapshot(app.shell()), snapshot(&game(Nation::Fire, &format!("i <down*{to}>"))));
}

/// A double-click on a row is `Enter` there: on the one the Bend tool channels it picks it, the
/// Bend tool is active and the screen closes, with the same state and command log as the keys.
#[test]
fn a_double_click_picks_the_ability_as_enter_does() {
    let mut app = GuiApp::with_shell(game(Nation::Fire, "i"));
    let rows = abilities(&state(app.shell()).session);
    let mut h = harness(&mut app);
    let centre = row(&h, &rows[0]).rect().center();
    double_click(&mut h, centre);
    h.run_steps(3);
    drop(h);
    let ui = &state(app.shell()).ui;
    assert_eq!((ui.screen, ui.tool), (None, Tool::Bend));
    assert_eq!(snapshot(app.shell()), snapshot(&game(Nation::Fire, "i <enter>")));
    assert_eq!(app.frame().surface, Some(Surface::Native));
}

/// A double-click on a locked ability moves the highlight and does nothing else: the screen
/// stays open, the tool is as it was, as `Enter` there.
#[test]
fn a_double_click_on_a_locked_ability_picks_nothing() {
    let mut app = GuiApp::with_shell(game(Nation::Fire, "i"));
    let rows = abilities(&state(app.shell()).session);
    let to = rows.iter().position(|r| !r.usable()).unwrap();
    let before = state(app.shell()).ui.tool;
    let mut h = harness(&mut app);
    let centre = row(&h, &rows[to]).rect().center();
    double_click(&mut h, centre);
    h.run_steps(3);
    drop(h);
    let ui = &state(app.shell()).ui;
    assert_eq!((ui.screen, ui.tool, ui.ability_cursor), (Some(Screen::Abilities), before, to));
    let by_hand = game(Nation::Fire, &format!("i <down*{to}> <enter>"));
    assert_eq!(snapshot(app.shell()), snapshot(&by_hand));
}

/// The close button is `Esc`: the screen closes and the tool is as it was.
#[test]
fn the_close_button_is_esc() {
    let mut app = GuiApp::with_shell(game(Nation::Earth, "i"));
    let mut h = harness(&mut app);
    h.get_by_label(page::CLOSE).click();
    h.run_steps(3);
    drop(h);
    assert_eq!(snapshot(app.shell()), snapshot(&game(Nation::Earth, "i <esc>")));
    assert_eq!(state(app.shell()).ui.screen, None);
}

/// The keyboard works on the page as in the terminal: the arrows move the highlight.
#[test]
fn the_keys_work_on_the_page() {
    let mut app = GuiApp::with_shell(game(Nation::Fire, "i"));
    let mut h = harness(&mut app);
    h.key_press(egui::Key::ArrowDown);
    h.run_steps(2);
    drop(h);
    assert_eq!(state(app.shell()).ui.ability_cursor, 1);
}

/// A short window scrolls the list and says where on the frame's edge.
#[test]
fn a_short_window_scrolls_the_list_and_says_where() {
    let mut app = GuiApp::with_shell(game(Nation::Earth, "i"));
    let total = abilities(&state(app.shell()).session).len();
    let mut h = Harness::builder()
        .with_size(egui::vec2(640.0, 240.0))
        .with_context_setup(fn_gui::setup)
        .build_ui(|ui| app.show(ui));
    h.run_steps(3);
    h.get_by_label_contains(&format!("of {total}"));
}

#[test]
fn the_keys_of_a_click_are_the_arrows_then_enter() {
    assert!(click_keys(2, 2, false).is_empty());
    let down: Vec<_> = click_keys(1, 4, true).iter().map(|k| k.code).collect();
    assert_eq!(down, [KeyCode::Down, KeyCode::Down, KeyCode::Down, KeyCode::Enter]);
    let up: Vec<_> = click_keys(3, 2, false).iter().map(|k| k.code).collect();
    assert_eq!(up, [KeyCode::Up]);
}
