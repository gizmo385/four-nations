//! The game menu, the confirm boxes and embark's exit prompt as modals (#879): their words are
//! `fn_ui`'s, and a click is the keys it stands for. Like `frame.rs`, none of it needs a GPU.

use eframe::egui;
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use fn_gui::app::GuiApp;
use fn_gui::boxes::click_keys;
use fn_gui::frame::{BoxKind, PageKind, Surface};
use fn_sim::Nation;
use fn_tui::shell::{Mode, Shell};
use fn_ui::ask::ExitAnswer;
use fn_ui::keyev::{KeyCode, KeyEvent, KeyModifiers};
use fn_ui::menu::{MenuPage, MenuRow};
use fn_ui::state::{Modal, Prompt};

const SIZE: (u16, u16) = (120, 40);

fn game(script: &str) -> Shell {
    let mut shell = Shell::new_game(fn_tui::new_session(7, Nation::Earth), SIZE, None);
    shell.play(&fn_ui::script::parse(script).unwrap());
    shell
}

fn embark() -> Shell {
    let defs = fn_sim::Defs::bundled();
    Shell::new_embark(
        7,
        Nation::Earth,
        defs.embark.map_sizes[0].clone(),
        fn_sim::embark::SurveyMode::Exact,
        None,
        SIZE,
        None,
    )
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

fn paused(app: &GuiApp) -> bool {
    state(app).session.clock.paused
}

/// A click at `at`, the way a pointer makes one: move, press, release.
fn click_at(h: &mut Harness<'_>, at: egui::Pos2) {
    h.hover_at(at);
    h.run_steps(1);
    h.drag_at(at);
    h.run_steps(1);
    h.drop_at(at);
    h.run_steps(2);
}

/// The acceptance test: `Esc` opens the menu (and stops the clock), and clicking Resume closes it
/// and puts the clock as it was, whichever way it was.
#[test]
fn esc_opens_the_menu_and_resume_closes_it_and_restores_the_clock() {
    for was_paused in [false, true] {
        let mut shell = game("");
        let Mode::Game(game) = &mut shell.mode else { panic!("a game") };
        game.session.clock.paused = was_paused;
        let mut app = GuiApp::with_shell(shell);
        let mut h = harness(&mut app);
        assert!(h.query_by_label("Resume").is_none(), "no menu yet");

        h.key_press(egui::Key::Escape);
        h.run_steps(2);
        assert!(h.query_by_label("Resume").is_some(), "the menu's first row");
        drop(h);
        assert_eq!(app.frame().modal, Some(BoxKind::Menu));
        assert_eq!(state(&app).ui.modal, Some(Modal::Menu));
        assert!(paused(&app), "the menu pauses the clock");

        let mut h = harness(&mut app);
        h.get_by_label("Resume").click();
        h.run_steps(2);
        assert!(h.query_by_label("Resume").is_none(), "the box is gone");
        drop(h);
        assert_eq!(app.frame().modal, None);
        assert_eq!(state(&app).ui.modal, None);
        assert_eq!(paused(&app), was_paused, "the clock is as it was");
    }
}

/// Every row of the menu is a button with `fn_ui`'s label.
#[test]
fn the_menu_lists_fn_uis_rows() {
    let mut app = GuiApp::with_shell(game("<esc>"));
    let h = harness(&mut app);
    for row in MenuRow::ALL {
        assert!(h.query_by_label(row.label_for(true)).is_some(), "{row:?}");
    }
    let marks = fn_tui::widgets::Marks::new(false);
    let heading = format!("{0} {1} {0}", marks.seal, MenuPage::Rows.title());
    assert!(h.query_by_label(&heading).is_some(), "the title between two seals");
}

/// The browser's menu has no Quit: the row is the way back to the title screen and says so.
#[test]
fn without_a_quit_the_last_row_goes_to_the_title_screen() {
    let mut shell = game("").with_quit(false);
    shell.play(&fn_ui::script::parse("<esc>").unwrap());
    let mut app = GuiApp::with_shell(shell);
    let h = harness(&mut app);
    assert!(h.query_by_label("Quit…").is_none());
    assert!(h.query_by_label("Title screen…").is_some());
}

/// A click and the keys it stands for leave the same game, and the keys all still work.
#[test]
fn a_click_on_a_row_is_the_keys_that_reach_it() {
    // New map… is the fifth row (index 4).
    let mut by_hand = game("<esc> <down*4> <enter>");
    let mut app = GuiApp::with_shell(game("<esc>"));
    let mut h = harness(&mut app);
    h.get_by_label("New map…").click();
    h.run_steps(2);
    drop(h);
    assert_eq!(app.frame().modal, Some(BoxKind::Ask));
    let (clicked, typed) = (
        state(&app),
        match &mut by_hand.mode {
            Mode::Game(g) => g,
            _ => panic!("a game"),
        },
    );
    assert_eq!(clicked.ui.prompt, Some(Prompt::NewMap));
    assert_eq!(clicked.ui.prompt, typed.ui.prompt);
    assert_eq!(clicked.ui.ask_cursor, typed.ui.ask_cursor);
}

#[test]
fn the_keys_for_a_click_step_to_the_row_then_pick_it() {
    let press = |code| KeyEvent::new(code, KeyModifiers::NONE);
    assert_eq!(
        click_keys(1, 3),
        [press(KeyCode::Down), press(KeyCode::Down), press(KeyCode::Enter)]
    );
    assert_eq!(click_keys(2, 1), [press(KeyCode::Up), press(KeyCode::Enter)]);
    assert_eq!(click_keys(0, 0), [press(KeyCode::Enter)]);
}

/// Keyboard stays whole under the modal: arrows move the highlight, Enter picks, Esc answers the
/// safe way, and none of it reaches a button twice.
#[test]
fn the_keyboard_still_drives_the_boxes() {
    let mut app = GuiApp::with_shell(game("<esc>"));
    let mut h = harness(&mut app);
    for _ in 0..6 {
        h.key_press(egui::Key::ArrowDown);
        h.run_steps(1);
    }
    h.key_press(egui::Key::Enter);
    h.run_steps(2);
    assert!(h.query_by_label("Keep playing").is_some(), "Quit…'s box");
    h.key_press(egui::Key::Escape);
    h.run_steps(2);
    assert!(h.query_by_label("Keep playing").is_none());
    drop(h);
    assert_eq!(state(&app).ui.prompt, None);
    assert_eq!(state(&app).ui.modal, None, "Esc there is the safe answer, which closes the menu");
}

/// The confirm box: the question, what follows from it and the answers, from `fn_ui`; "Keep
/// playing" closes the box and the menu behind it.
#[test]
fn a_confirm_box_says_what_fn_ui_says_and_keep_playing_closes_it() {
    let mut app = GuiApp::with_shell(game("<esc> <down*6> <enter>"));
    let mut h = harness(&mut app);
    h.run_steps(1);
    assert!(h.query_by_label("Leave this game?").is_some());
    assert!(h.query_by_label("This game has not been saved.").is_some());
    assert!(h.query_by_label("Go to the title screen without saving").is_some());
    h.get_by_label("Keep playing").click();
    h.run_steps(2);
    assert!(h.query_by_label("Leave this game?").is_none());
    drop(h);
    assert_eq!(app.frame().modal, None);
    assert_eq!(state(&app).ui.prompt, None);
    assert_eq!(state(&app).ui.modal, None);
    assert!(!paused(&app), "the clock runs again");
}

/// A click outside the box is `Esc`: Resume on the menu, back on the nation list.
#[test]
fn a_click_outside_the_box_is_escape() {
    let mut app = GuiApp::with_shell(game("<esc>"));
    let mut h = harness(&mut app);
    h.get_by_label("Switch nation…").click();
    h.run_steps(2);
    assert!(h.query_by_label_contains(fn_ui::menu::PLAYING).is_some(), "the nation list");
    click_at(&mut h, egui::pos2(6.0, 6.0));
    assert!(h.query_by_label("Resume").is_some(), "back on the rows, still open");
    click_at(&mut h, egui::pos2(6.0, 6.0));
    assert!(h.query_by_label("Resume").is_none(), "Esc on the rows resumes");
    drop(h);
    assert_eq!(state(&app).ui.modal, None);
}

/// Switch nation… turns the box into the nation list, the nation being played noted; a click on
/// a nation asks before switching.
#[test]
fn the_nation_list_notes_the_one_playing_and_a_click_asks() {
    let mut app = GuiApp::with_shell(game("<esc>"));
    let mut h = harness(&mut app);
    h.get_by_label("Switch nation…").click();
    h.run_steps(2);
    drop(h);
    assert_eq!(state(&app).ui.menu_page, MenuPage::Nations);
    let rows = fn_ui::menu::nation_rows(&state(&app).session);
    let mut h = harness(&mut app);
    for (name, playing) in &rows {
        let label =
            if *playing { format!("{name}  {}", fn_ui::menu::PLAYING) } else { name.clone() };
        assert!(h.query_by_label(&label).is_some(), "{label}");
    }
    let other = rows.iter().position(|(_, playing)| !playing).unwrap();
    h.get_by_label(&rows[other].0).click();
    h.run_steps(2);
    drop(h);
    assert_eq!(state(&app).ui.prompt, Some(Prompt::Nation(Nation::ALL[other])));
    assert_eq!(app.frame().modal, Some(BoxKind::Ask));
}

/// Over a page, a prompt is a box over the page (and no longer a box over the map).
#[test]
fn a_question_over_a_page_keeps_the_page() {
    let mut shell = game("u");
    let Mode::Game(game) = &mut shell.mode else { panic!("a game") };
    game.ui.prompt = Some(Prompt::QuickLoad);
    let mut app = GuiApp::with_shell(shell);
    let h = harness(&mut app);
    assert!(h.query_by_label_contains("Load the quicksave").is_some());
    assert!(h.query_by_label("Close [Esc]").is_some(), "the page is still under it");
    drop(h);
    assert_eq!(app.frame().surface, Some(Surface::Page(PageKind::Build)));
    assert_eq!(app.frame().modal, Some(BoxKind::Ask));
    let mut h = harness(&mut app);
    h.get_by_label("Keep playing").click();
    h.run_steps(2);
    drop(h);
    assert_eq!(state(&app).ui.prompt, None);
    assert_eq!(app.frame().modal, None);
}

/// Embark's exit prompt: three buttons, each the key it is labelled with.
#[test]
fn the_exit_prompt_is_three_buttons_for_its_keys() {
    for answer in ExitAnswer::ALL {
        let mut shell = embark();
        shell.exit_prompt = true;
        let mut app = GuiApp::with_shell(shell);
        let mut h = harness(&mut app);
        assert!(h.query_by_label(fn_ui::ask::EXIT_QUESTION).is_some());
        h.get_by_label_contains(answer.label()).click();
        h.run_steps(2);
        drop(h);
        let shell = app.shell();
        match answer {
            ExitAnswer::Title => {
                assert!(matches!(shell.mode, Mode::Title(_)));
                assert!(!shell.exit_prompt);
            }
            ExitAnswer::Quit => assert!(shell.quit),
            ExitAnswer::Stay => {
                assert!(matches!(shell.mode, Mode::Embark(_)));
                assert!(!shell.exit_prompt);
                assert_eq!(app.frame().modal, None);
            }
        }
    }
}
