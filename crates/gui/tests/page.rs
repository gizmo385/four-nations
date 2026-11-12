//! The page frame and the Build screen (#881): the words are the terminal's, a click is the keys
//! it stands for, and the screen closes as it does in the terminal. Nothing here needs a GPU.

use eframe::egui::{self, Event, Key, Modifiers, PointerButton};
use egui_kittest::Harness;
use egui_kittest::kittest::{NodeT, Queryable};
use fn_gui::app::GuiApp;
use fn_gui::frame::{PageKind, Surface};
use fn_gui::page;
use fn_gui::screens::build::click_keys;
use fn_sim::Nation;
use fn_tui::shell::{Mode, Shell};
use fn_ui::Tool;
use fn_ui::build::{PickerRow, picker_rows};
use fn_ui::keyev::KeyCode;
use fn_ui::state::Modal;

const SIZE: (u16, u16) = (120, 40);

fn game(script: &str) -> Shell {
    let mut shell = Shell::new_game(fn_tui::new_session(7, Nation::Earth), SIZE, None);
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

fn state(app: &GuiApp) -> &fn_ui::app::App {
    match &app.shell().mode {
        Mode::Game(game) => game,
        Mode::Title(_) | Mode::Embark(_) => panic!("not in a game"),
    }
}

/// The list's row for the building `name` (its detail in the strip names it too, but is no
/// button).
fn row<'a>(h: &'a Harness<'_>, name: &str) -> egui_kittest::Node<'a> {
    h.query_all_by_role(egui::accesskit::Role::Button)
        .find(|n| {
            // The pointer's column, then the name padded to its column.
            let label = n.accesskit_node().label().unwrap_or_default();
            label.chars().skip(2).collect::<String>().starts_with(&format!("{name}  "))
        })
        .unwrap_or_else(|| panic!("no row for {name}"))
}

fn rows(app: &GuiApp) -> Vec<PickerRow> {
    picker_rows(&state(app).session)
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

/// `U` opens a native page: the window above the hint bar, no side panel, the title between two
/// seals, the terminal's rows and a detail strip for the highlighted building.
#[test]
fn build_is_a_page_with_the_terminals_rows_and_a_strip() {
    let mut app = GuiApp::with_shell(game("u"));
    let h = harness(&mut app);
    let wanted = h.query_all_by_label_contains("SETTLEMENT").count();
    assert_eq!(wanted, 0, "the side panel is hidden");
    h.get_by_label("◆ BUILD ◆");
    for head in ["BUILDING", "COST", "SIZE", "NEEDS"] {
        h.get_by_label_contains(head);
    }
    drop(h);
    assert_eq!(app.frame().surface, Some(Surface::Page(PageKind::Build)));
    assert!(app.frame().hint_text.starts_with("BUILD"), "{}", app.frame().hint_text);
    let rows = rows(&app);
    let mut h = harness(&mut app);
    for r in rows.iter().take(8) {
        row(&h, &r.name);
    }
    // The first building's detail: its size line and what stores hold.
    h.get_by_label_contains("Size 1x1.");
    h.run_steps(1);
}

/// A click moves the highlight (the keys that would), and a double-click picks: the screen
/// closes, the Build tool is active and holds that building.
#[test]
fn a_double_click_picks_a_building_and_leaves_the_build_tool_active() {
    let mut app = GuiApp::with_shell(game("u"));
    let rows = rows(&app);
    let target = 1;
    let other = 0;
    assert!(!rows[target].locked() && !rows[other].locked());

    // One click: the highlight moves, the screen stays.
    let mut h = harness(&mut app);
    row(&h, &rows[target].name).click();
    h.run_steps(3);
    drop(h);
    assert_eq!(state(&app).ui.build_cursor, target);
    assert_eq!(state(&app).ui.modal, Some(Modal::Build));
    assert_ne!(state(&app).ui.tool, Tool::Build);

    // Another row, by a double-click from there.
    let mut h = harness(&mut app);
    let centre = row(&h, &rows[other].name).rect().center();
    double_click(&mut h, centre);
    h.run_steps(3);
    drop(h);
    let ui = &state(&app).ui;
    assert_eq!(ui.modal, None);
    assert_eq!(ui.tool, Tool::Build);
    assert_eq!(ui.choice, Some(rows[other].id));
    assert_eq!(app.frame().surface, Some(Surface::Native));
}

/// The same pick by clicks and by keys leave the same game.
#[test]
fn a_click_and_the_keys_it_stands_for_leave_the_same_game() {
    let rows = picker_rows(&state_of(game("u")).session);
    let to = rows.iter().position(|r| !r.locked()).map(|_| 1).unwrap();
    assert!(!rows[to].locked());
    let mut by_hand = game("u");
    for key in click_keys(0, to, true) {
        by_hand.key(key);
    }
    let by_hand = state_of(by_hand);
    assert_eq!(by_hand.ui.choice, Some(rows[to].id));
    assert_eq!(by_hand.ui.tool, Tool::Build);
    assert_eq!(by_hand.ui.modal, None);
    // And back up.
    let keys = click_keys(to, 0, false);
    assert!(keys.iter().all(|k| k.code == KeyCode::Up) && keys.len() == to);
}

fn state_of(shell: Shell) -> fn_ui::app::App {
    match shell.mode {
        Mode::Game(game) => *game,
        Mode::Title(_) | Mode::Embark(_) => panic!("not in a game"),
    }
}

/// A locked building is not picked: the screen stays and the strip says what it needs.
#[test]
fn a_locked_building_stays_on_the_screen_and_the_strip_says_why() {
    let mut app = GuiApp::with_shell(game("u"));
    let locked = rows(&app).into_iter().find(|r| r.locked()).expect("a locked building");
    let mut h = harness(&mut app);
    row(&h, &locked.name).click();
    h.run_steps(3);
    h.get_by_label_contains("Research it first");
    h.key_press(Key::Enter);
    h.run_steps(3);
    drop(h);
    assert_eq!(state(&app).ui.modal, Some(Modal::Build));
    assert_ne!(state(&app).ui.tool, Tool::Build);
}

/// `Esc`, the key that opened the screen, and the close button all close it, and leave the tool
/// and the choice as they were.
#[test]
fn esc_the_opening_key_and_the_close_button_close_it() {
    for how in ["esc", "u", "button"] {
        let mut app = GuiApp::with_shell(game("u"));
        let before = (state(&app).ui.tool, state(&app).ui.choice);
        let mut h = harness(&mut app);
        match how {
            "esc" => h.key_press(Key::Escape),
            "u" => h.event(Event::Text("u".into())),
            _ => h.get_by_label(page::CLOSE).click(),
        }
        h.run_steps(3);
        drop(h);
        let ui = &state(&app).ui;
        assert_eq!(ui.modal, None, "{how}");
        assert_eq!((ui.tool, ui.choice), before, "{how}");
        assert_eq!(app.frame().surface, Some(Surface::Native), "{how}");
    }
}

/// Keys work as in the terminal: `S` moves the highlight down, and the strip follows it.
#[test]
fn keys_move_the_highlight_and_the_strip_follows() {
    let mut app = GuiApp::with_shell(game("u"));
    let rows = rows(&app);
    let mut h = harness(&mut app);
    h.event(Event::Text("s".into()));
    h.run_steps(3);
    h.get_by_label_contains(&format!("Size {}.", rows[1].size));
    drop(h);
    assert_eq!(state(&app).ui.build_cursor, 1);
}

/// The page covers the window above the hint bar, whatever the window's size, and a short window
/// still shows the title and the list.
#[test]
fn a_short_window_still_shows_the_frame() {
    let mut app = GuiApp::with_shell(game("u"));
    let mut h = Harness::builder()
        .with_size(egui::vec2(1024.0, 400.0))
        .with_context_setup(fn_gui::setup)
        .build_ui(|ui| app.show(ui));
    h.run_steps(3);
    h.get_by_label("◆ BUILD ◆");
    h.get_by_label_contains("BUILDING");
    h.get_by_label(page::CLOSE);
}

#[test]
fn the_geometry_keeps_the_body_a_row_and_the_strip_inside_the_frame() {
    let cell = egui::vec2(8.0, 17.0);
    let outer = egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(800.0, 340.0));
    let none = page::geometry(outer, cell, 0);
    assert_eq!(none.strip, None);
    assert_eq!(none.body, none.inner);
    let some = page::geometry(outer, cell, 5);
    let strip = some.strip.expect("a strip");
    assert!(some.body.height() >= cell.y, "{some:?}");
    assert!(some.body.bottom() < some.rule_y && some.rule_y < strip.top(), "{some:?}");
    assert_eq!(strip.bottom(), some.inner.bottom());
    // Asked for more rows than the page has: the strip shrinks, the body keeps a row.
    let greedy = page::geometry(outer, cell, 500);
    assert!(greedy.body.height() >= cell.y, "{greedy:?}");
    assert!(greedy.strip.is_some_and(|s| s.height() < 500.0 * cell.y));
}
