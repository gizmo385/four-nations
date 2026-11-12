//! The Research screen and the school box (#884): the words are the terminal's, a click is the
//! keys it stands for, and the box answers to `Enter` and `Esc`. Nothing here needs a GPU.

use eframe::egui::{self, Event, Key, Modifiers, PointerButton};
use egui_kittest::Harness;
use egui_kittest::kittest::{NodeT, Queryable};
use fn_gui::app::GuiApp;
use fn_gui::frame::{PageKind, Surface};
use fn_gui::page;
use fn_gui::screens::research::{Cursor, click_keys, step};
use fn_sim::Nation;
use fn_tui::shell::{Mode, Shell};
use fn_ui::research::{ScreenView, TechCard, TechState, screen_view};
use fn_ui::screens::research::{CONFIRM_TITLE, card_word};

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

fn state_of(shell: &Shell) -> &fn_ui::app::App {
    match &shell.mode {
        Mode::Game(game) => game,
        Mode::Title(_) | Mode::Embark(_) => panic!("not in a game"),
    }
}

/// The card `id`'s place in the tree.
fn place(view: &ScreenView, id: &str, shell: &Shell) -> Cursor {
    let want = shell.defs().tech_id(id).unwrap_or_else(|| panic!("no tech {id}"));
    view.columns
        .iter()
        .enumerate()
        .find_map(|(c, col)| col.iter().position(|t| t.id == want).map(|r| (c, r)))
        .unwrap_or_else(|| panic!("{id} is not in Earth's tree"))
}

/// The card's button: a card's words as the tree gives them, name then state, however its
/// lines wrapped.
fn card<'a>(h: &'a Harness<'_>, card: &TechCard) -> egui_kittest::Node<'a> {
    let want = format!("{} {}", card.name, card_word(card));
    h.query_all_by_role(egui::accesskit::Role::Button)
        .find(|n| {
            let label = n.accesskit_node().label().unwrap_or_default();
            label.split_whitespace().collect::<Vec<_>>().join(" ").contains(&want)
        })
        .unwrap_or_else(|| panic!("no card for {}", card.name))
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

/// `<key*n>` for a script, or nothing for zero (the script has no zero repeat).
fn repeat(key: &str, n: usize) -> String {
    if n == 0 { String::new() } else { format!("<{key}*{n}> ") }
}

/// What the keyboard arrives at: the cursor, the box, the target and the command log.
fn snapshot(shell: &Shell) -> (Cursor, bool, Vec<String>, String) {
    let app = state_of(shell);
    let queue = app.session.world.research().queue().iter().map(|t| format!("{t:?}")).collect();
    // The commands, not when they ran: the window's clock runs on a little between frames.
    let log = app.session.world.command_log();
    let commands = format!("{:?}", log.iter().map(|r| &r.command).collect::<Vec<_>>());
    (app.ui.tech, app.ui.school_confirm.is_some(), queue, commands)
}

/// `T` opens a native page: the title between two seals, a column of cards per tier, the
/// terminal's words on each card, and the cursor's card in the detail strip.
#[test]
fn research_is_a_page_of_tiers_and_cards_with_a_strip() {
    let mut app = GuiApp::with_shell(game("t"));
    let view = screen_view(&state(&app).session);
    let cursor = state(&app).ui.tech;
    let description = state(&app).session.defs.tech(view.columns[cursor.0][cursor.1].id);
    // The description's first words: the strip wraps it, so only its first row has them all.
    let opening = description.description.split_whitespace().take(3).collect::<Vec<_>>().join(" ");
    let h = harness(&mut app);
    assert_eq!(h.query_all_by_label_contains("SETTLEMENT").count(), 0, "the side panel is hidden");
    h.get_by_label("◆ RESEARCH ◆");
    for c in 0..view.columns.len() {
        h.get_by_label(&format!("TIER {}", c + 1));
    }
    // Every tier's first card is on the page, with its state word and cost.
    for column in &view.columns {
        card(&h, &column[0]);
    }
    // The strip says what the card under the cursor is.
    h.get_by_label_contains(&opening);
    drop(h);
    assert_eq!(app.frame().surface, Some(Surface::Page(PageKind::Research)));
    assert!(app.frame().hint_text.starts_with("RESEARCH"), "{}", app.frame().hint_text);
}

/// A narrow window shows fewer tiers, with an arrow for the rest and where it stands on the
/// frame's edge, as the terminal does.
#[test]
fn a_narrow_window_windows_the_tiers_and_the_arrows_step_the_cursor() {
    let mut app = GuiApp::with_shell(game("t <up*20> <left*20>"));
    let mut h = Harness::builder()
        .with_size(egui::vec2(640.0, 800.0))
        .with_context_setup(fn_gui::setup)
        .build_ui(|ui| app.show(ui));
    h.run_steps(3);
    h.get_by_label("TIER 1");
    assert!(h.query_by_label("TIER 5").is_none());
    h.get_by_label_contains("tiers 1-");
    h.get_by_label("Later tiers").click();
    h.run_steps(3);
    drop(h);
    assert_eq!(state(&app).ui.tech.0, 1);
}

/// A click moves the cursor: the very keys the arrows are.
#[test]
fn a_click_moves_the_cursor_as_the_arrow_keys_do() {
    let mut app = GuiApp::with_shell(game("t <up*20> <left*20>"));
    let view = screen_view(&state(&app).session);
    let to = (2, 1);
    let mut h = harness(&mut app);
    card(&h, &view.columns[to.0][to.1]).click();
    h.run_steps(3);
    drop(h);
    assert_eq!(state(&app).ui.tech, to);

    let mut by_hand = game("t <up*20> <left*20> <right*2> <down>");
    assert_eq!(snapshot(app.shell()), snapshot(&by_hand));
    // The close button is `Esc`, and the screen closes.
    by_hand.play(&fn_ui::script::parse("<esc>").unwrap());
    let mut h = harness(&mut app);
    h.get_by_label(page::CLOSE).click();
    h.run_steps(3);
    drop(h);
    assert_eq!(state_of(app.shell()).ui.screen, None);
    assert_eq!(app.frame().surface, Some(Surface::Native));
}

/// A double-click on a card that needs no confirming sets the target, and leaves the game the
/// keyboard's `Enter` on that card does: the cursor and the same command log.
#[test]
fn a_double_click_sets_the_target_as_the_keyboard_does() {
    let probe = game("t");
    let view = screen_view(&state_of(&probe).session);
    let (c, r) = view
        .columns
        .iter()
        .enumerate()
        .find_map(|(c, col)| {
            col.iter()
                .position(|t| t.state == TechState::Available && t.school.is_none())
                .map(|r| (c, r))
        })
        .expect("an available tech");
    let target = &view.columns[c][r];

    let mut app = GuiApp::with_shell(game("t"));
    let mut h = harness(&mut app);
    let centre = card(&h, target).rect().center();
    double_click(&mut h, centre);
    h.run_steps(3);
    drop(h);
    let clicked = snapshot(app.shell());
    assert_eq!(clicked.2, [format!("{:?}", target.id)], "the target is set");
    assert!(!clicked.3.is_empty() && clicked.3.contains("SetResearch"), "{}", clicked.3);

    let by_hand =
        game(&format!("t <up*20> <left*20> {}{}<enter>", repeat("right", c), repeat("down", r)));
    assert_eq!(clicked, snapshot(&by_hand));
}

/// The keys a click stands for land on the card from every card, whatever the columns' lengths
/// (a step sideways keeps the row only where the next column has that many cards).
#[test]
fn the_keys_of_a_click_land_on_the_card_from_every_card() {
    let shell = game("t");
    let view = screen_view(&state_of(&shell).session);
    let all: Vec<Cursor> = view
        .columns
        .iter()
        .enumerate()
        .flat_map(|(c, col)| (0..col.len()).map(move |r| (c, r)))
        .collect();
    for &from in &all {
        for &to in &all {
            let keys = click_keys(&view, from, to, false);
            let end = keys.iter().fold(from, |at, k| {
                use fn_ui::keyev::KeyCode::{Down, Left, Right, Up};
                let (dx, dy) = match k.code {
                    Left => (-1, 0),
                    Right => (1, 0),
                    Up => (0, -1),
                    Down => (0, 1),
                    other => panic!("{other:?}"),
                };
                step(&view, at, dx, dy)
            });
            assert_eq!(end, to, "from {from:?}");
        }
    }
    // On the card itself there is nothing to press but Enter, when picking.
    assert!(click_keys(&view, (1, 1), (1, 1), false).is_empty());
    assert_eq!(click_keys(&view, (1, 1), (1, 1), true).len(), 1);
}

/// Opens the CHOOSE A SCHOOL box on Earth's Metal City, with the clicks a player would make, and
/// returns the app and the card.
fn school_box() -> (GuiApp, TechCard) {
    let shell = game("t <up*20> <left*20>");
    let view = screen_view(&state_of(&shell).session);
    let (c, r) = place(&view, "metal_city", &shell);
    let target = view.columns[c][r].clone();
    let mut app = GuiApp::with_shell(shell);
    let mut h = harness(&mut app);
    // Down to the card by keys (which scroll it into view), then a double-click on it.
    for _ in 0..c {
        h.key_press(Key::ArrowRight);
    }
    for _ in 0..r {
        h.key_press(Key::ArrowDown);
    }
    h.run_steps(3);
    let centre = card(&h, &target).rect().center();
    double_click(&mut h, centre);
    h.run_steps(3);
    drop(h);
    (app, target)
}

/// A double-click on a school option asks first: the box, with the fork's words, and nothing is
/// set until the box is answered.
#[test]
fn a_school_option_asks_in_the_box_before_it_sets_the_target() {
    let (mut app, target) = school_box();
    assert!(state(&app).ui.school_confirm.is_some());
    assert!(state(&app).session.world.research().queue().is_empty());
    let name = &target.name;
    let h = harness(&mut app);
    h.get_by_label(CONFIRM_TITLE);
    h.get_by_label_contains("for the rest of this game");
    h.get_by_label(&format!("Choose {name} [Enter]"));
    h.get_by_label("Back [Esc]");
    drop(h);
    assert_eq!(app.frame().hint_text.split_whitespace().next(), Some("CHOOSE"));
}

/// The box's "Choose" button is `Enter`: it sets the target and closes the box, as the keys.
#[test]
fn the_choose_button_is_enter() {
    let (mut app, target) = school_box();
    let mut h = harness(&mut app);
    h.get_by_label(&format!("Choose {} [Enter]", target.name)).click();
    h.run_steps(3);
    drop(h);
    assert!(state(&app).ui.school_confirm.is_none());
    assert_eq!(state(&app).session.world.research().queue().last(), Some(&target.id));

    let (mut app2, _) = school_box();
    let mut h = harness(&mut app2);
    h.key_press(Key::Enter);
    h.run_steps(3);
    drop(h);
    assert_eq!(snapshot(app.shell()), snapshot(app2.shell()));
}

/// "Back", the backdrop and `Esc` cancel: the box closes and the target is not set.
#[test]
fn back_the_backdrop_and_esc_cancel() {
    for how in ["button", "backdrop", "esc"] {
        let (mut app, _) = school_box();
        let at = state(&app).ui.tech;
        let mut h = harness(&mut app);
        match how {
            "button" => h.get_by_label("Back [Esc]").click(),
            "backdrop" => {
                h.event(Event::PointerMoved(egui::pos2(5.0, 5.0)));
                for pressed in [true, false] {
                    h.event(Event::PointerButton {
                        pos: egui::pos2(5.0, 5.0),
                        button: PointerButton::Primary,
                        pressed,
                        modifiers: Modifiers::NONE,
                    });
                }
            }
            _ => h.key_press(Key::Escape),
        }
        h.run_steps(3);
        drop(h);
        let ui = &state(&app).ui;
        assert!(ui.school_confirm.is_none(), "{how}");
        assert!(state(&app).session.world.research().queue().is_empty(), "{how}");
        assert_eq!(ui.tech, at, "{how}");
        assert_eq!(ui.screen, Some(fn_ui::state::Screen::Research), "{how}");
    }
}

/// The box fits a small window: its two answers are on the page.
#[test]
fn the_box_fits_a_small_window() {
    let shell = game("t <up*20> <left*20>");
    let view = screen_view(&state_of(&shell).session);
    let (c, r) = place(&view, "metal_city", &shell);
    let mut shell = shell;
    let keys = format!("{}{}<enter>", repeat("right", c), repeat("down", r));
    shell.play(&fn_ui::script::parse(&keys).unwrap());
    let mut app = GuiApp::with_shell(shell);
    let mut h = Harness::builder()
        .with_size(egui::vec2(800.0, 500.0))
        .with_context_setup(fn_gui::setup)
        .build_ui(|ui| app.show(ui));
    h.run_steps(3);
    h.get_by_label(CONFIRM_TITLE);
    h.get_by_label("Back [Esc]");
}

/// Once the gate stands the page has tabs: a click on "From newcomers" is `Tab`, and the page of
/// the other nations' techs and the fusions takes the place of the tiers.
#[test]
fn after_the_gate_the_tabs_turn_to_the_newcomers_page() {
    let mut session = fn_tui::new_session(7, Nation::Water);
    assert!(session.world.complete_endeavor(&session.defs));
    let mut shell = Shell::new_game(session, SIZE, None);
    shell.play(&fn_ui::script::parse("t").unwrap());
    let mut app = GuiApp::with_shell(shell);
    let mut h = harness(&mut app);
    h.get_by_label("Your tree");
    h.get_by_label("TIER 1");
    h.get_by_label("From newcomers").click();
    h.run_steps(3);
    h.get_by_label_contains("FUSIONS");
    h.get_by_label_contains("FIRE NATION");
    assert!(h.query_by_label("TIER 1").is_none());
    h.get_by_label_contains("Steelworks");
    drop(h);
    assert_eq!(state(&app).ui.research_page, fn_ui::research::ResearchPage::Newcomers);
}
