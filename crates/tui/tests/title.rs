//! The title screen through the public API (#280): the one reference golden (the fresh menu)
//! lives in `reference_goldens.rs` now (#455); this keeps the `contains`-style checks for the
//! nation picker and the seed step, and the shell's own switch between the title screen, a game,
//! and back.

use fn_sim::Nation;
use fn_tui::color::{ColorDepth, Palette};
use fn_tui::frame::{render_shell, to_text};
use fn_tui::keyev::{KeyCode, KeyEvent, KeyModifiers};
use fn_tui::shell::Shell;

const SIZE: (u16, u16) = (120, 40);

fn frame(shell: &mut Shell) -> String {
    let p = Palette::new(shell.defs(), shell.nation(), ColorDepth::TrueColor, false);
    to_text(&render_shell(shell, &p, SIZE))
}

fn press(shell: &mut Shell, code: KeyCode) {
    shell.key(KeyEvent::new(code, KeyModifiers::NONE));
}

// ---------------------------------------------------------------------------------------------
// The nation picker and the seed step
// ---------------------------------------------------------------------------------------------

/// The nation picker, on the first nation (Air Nomads): its own blurb under the roster, the
/// pointer on its row.
#[test]
fn the_nation_picker_shows_the_pointed_nations_blurb() {
    let mut shell = Shell::new_title(SIZE, None, 1);
    press(&mut shell, KeyCode::Enter);
    let text = frame(&mut shell);
    assert!(text.contains("▸ 1. Air Nomads"), "{text}");
    assert!(text.contains("Nomadic monks. Fast and light; can power turbines and mills"), "{text}");
}

/// The seed step's Size and Survey rows (#290), with the highlighted row's hint below the three,
/// on the Survey row.
#[test]
fn the_seed_step_shows_the_highlighted_rows_hint() {
    let mut shell = Shell::new_title(SIZE, None, 7);
    let to_seed = fn_tui::keys::parse("<enter> <enter> <down> <down>").unwrap();
    shell.play(&to_seed);
    let text = frame(&mut shell);
    assert!(text.contains("Seed: 7") && text.contains("Size: Small"), "{text}");
    assert!(text.contains("▸ Survey: Exact"), "the pointer is on Survey:\n{text}");
    assert!(
        text.contains("Exact: you'll see exactly what's at every site."),
        "its own hint is below it:\n{text}"
    );
}

// ---------------------------------------------------------------------------------------------
// The shell's own switching
// ---------------------------------------------------------------------------------------------

/// New game, through the nation picker, to the seed step, where `Enter` now opens embark
/// (#290) instead of the game straight away; `e` `y` settles at its default site from there.
#[test]
fn a_full_script_starts_a_game_from_the_title_screen_and_plays_it() {
    let mut shell = Shell::new_title(SIZE, None, 7);
    let script =
        fn_tui::keys::parse("<enter> <right> <right> <enter> <enter> e y <tick 600>").unwrap();
    shell.play(&script);
    let text = frame(&mut shell);
    assert_eq!(shell.nation(), Nation::Earth);
    assert!(matches!(shell.mode, fn_tui::shell::Mode::Game(_)));
    assert!(text.contains("EARTH KINGDOM"), "{text}");
    // The title screen's own default preset has seasons on: the panel and the Nation screen's
    // header line both say the season and the time left (#654), a minute into spring.
    assert!(text.contains("Spring Y1 · 14:00 left"), "<tick 600> is a minute:\n{text}");
    press(&mut shell, KeyCode::Char('n'));
    let nation_text = frame(&mut shell);
    assert!(nation_text.contains("Spring Y1 · 14:00 left"), "{nation_text}");
}

/// `Enter` on the seed step opens embark instead of starting the game (#290); `Esc` at its
/// overview goes back to the seed step with the seed kept.
#[test]
fn enter_on_the_seed_step_opens_embark_and_esc_at_the_overview_goes_back() {
    let mut shell = Shell::new_title(SIZE, None, 7);
    let to_seed = fn_tui::keys::parse("<enter> <enter>").unwrap(); // New game, Air
    shell.play(&to_seed);
    press(&mut shell, KeyCode::Enter); // the seed step: opens embark
    assert!(matches!(shell.mode, fn_tui::shell::Mode::Embark(_)));
    let text = frame(&mut shell);
    assert!(text.contains("CHOOSE A SITE"), "{text}");

    press(&mut shell, KeyCode::Esc);
    match &shell.mode {
        fn_tui::shell::Mode::Title(title) => {
            assert!(
                matches!(title.page, fn_tui::title::Page::Seed { nation: Nation::Air, .. }),
                "{:?}",
                title.page
            );
        }
        other => panic!("expected the seed step, got {other:?}"),
    }
}

/// New map…, confirmed in a game, reopens embark on the next seed instead of restarting in place
/// (#290); the nation switch, with no chosen start, still restarts in place as before.
#[test]
fn new_map_in_a_game_reopens_embark_on_the_next_seed() {
    let mut shell = Shell::new_game(fn_tui::new_session(7, Nation::Earth), SIZE, None);
    // The menu's New map…, then the box's second answer (no save directory).
    shell.play(&fn_tui::keys::parse("<esc> <down*4> <enter> <down> <enter>").unwrap());
    match &shell.mode {
        fn_tui::shell::Mode::Embark(screen) => {
            assert_eq!(screen.seed(), 8);
            assert_eq!(screen.nation, Nation::Earth);
        }
        other => panic!("expected embark, got {other:?}"),
    }
}

/// The game menu's Quit… (the last row), as far as its box.
const ASK_QUIT: &str = "<esc> <down*6> <enter>";

/// A game on the default site, at the title screen's keys.
fn new_game() -> Shell {
    let mut shell = Shell::new_title(SIZE, None, 1);
    // New game, Air, the seed step's embark (#290), then settle at its default site.
    shell.play(&fn_tui::keys::parse("<enter> <enter> <enter> e y").unwrap());
    assert!(matches!(shell.mode, fn_tui::shell::Mode::Game(_)));
    shell
}

/// Quitting from a game asks once (#703): the Quit… box answers both whether to leave and where
/// to. There is no second prompt after it.
#[test]
fn quitting_a_game_asks_once_in_one_box() {
    let mut shell = new_game();
    shell.play(&fn_tui::keys::parse(ASK_QUIT).unwrap());
    let text = frame(&mut shell);
    assert!(text.contains("Leave this game?"), "{text}");
    for row in ["Keep playing", "Go to the title screen without saving", "Quit the game"] {
        assert!(text.contains(row), "{row}:\n{text}");
    }
    assert!(!text.contains("QUIT?") && !text.contains("title screen, or exit"), "{text}");

    // Each answer ends it: no further box, whichever way it goes.
    press(&mut shell, KeyCode::Esc);
    assert!(!shell.exit_prompt && !shell.quit, "Esc keeps playing");
    assert!(matches!(shell.mode, fn_tui::shell::Mode::Game(_)));

    shell.play(&fn_tui::keys::parse(&format!("{ASK_QUIT} <down> <enter>")).unwrap());
    assert!(!shell.exit_prompt && !shell.quit);
    assert!(matches!(shell.mode, fn_tui::shell::Mode::Title(_)), "back at the title screen");
    assert!(!frame(&mut shell).contains("Leave this game?"));

    press(&mut shell, KeyCode::Down);
    press(&mut shell, KeyCode::Down);
    // The menu is New game, Load, Quit with no saves: the third row is Quit.
    press(&mut shell, KeyCode::Enter);
    assert!(shell.quit, "the title screen's own Quit stops the process without asking again");
}

#[test]
fn the_quit_boxs_last_answer_exits_the_process_at_once() {
    let mut shell = new_game();
    shell.play(&fn_tui::keys::parse(&format!("{ASK_QUIT} <down*2> <enter>")).unwrap());
    assert!(shell.quit);
    assert!(!shell.exit_prompt, "no second question");
}

#[test]
fn ctrl_c_in_a_game_exits_without_asking() {
    let mut shell = new_game();
    shell.key(KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL));
    assert!(shell.quit && !shell.exit_prompt);
}

/// `Q` in embark still asks where to go (it has no game to save); its keys are in the hint bar's
/// `EXIT` mode, not printed in the box.
#[test]
fn embark_q_asks_with_its_keys_in_the_hint_bar() {
    let mut shell = Shell::new_title(SIZE, None, 1);
    shell.play(&fn_tui::keys::parse("<enter> <enter> <enter>").unwrap());
    assert!(matches!(shell.mode, fn_tui::shell::Mode::Embark(_)));
    press(&mut shell, KeyCode::Char('q'));
    assert!(shell.exit_prompt && !shell.quit);
    let text = frame(&mut shell);
    let bar = text.lines().last().unwrap();
    for item in fn_ui::keys::hints(fn_ui::keys::Mode::Exit) {
        assert!(bar.contains(&item.text()), "{} in the hint bar: {bar}", item.text());
        assert_eq!(text.matches(&item.text()).count(), 1, "only there:\n{text}");
    }
    assert!(bar.trim_start().starts_with("EXIT"), "{bar}");

    press(&mut shell, KeyCode::Enter);
    assert!(shell.quit);
}

#[test]
fn embark_q_then_t_goes_to_the_title_screen() {
    let mut shell = Shell::new_title(SIZE, None, 1);
    shell.play(&fn_tui::keys::parse("<enter> <enter> <enter> q t").unwrap());
    assert!(matches!(shell.mode, fn_tui::shell::Mode::Title(_)));
    assert!(!shell.exit_prompt && !shell.quit);
}

#[test]
fn seed_and_nation_together_skip_the_menu_straight_into_a_game() {
    let mut shell = Shell::new_game(fn_tui::new_session(7, Nation::Fire), SIZE, None);
    let text = frame(&mut shell);
    assert!(text.contains("FIRE NATION"), "{text}");
    assert!(!text.contains("FOUR NATIONS"), "no title screen shows at all:\n{text}");
}

/// A host that can't quit (#866, the browser): no Quit anywhere, and every way back to the title
/// screen still works.
#[test]
fn without_quit_nothing_offers_or_does_it() {
    // The title screen: New game, Load, and that is all (no saves, no runs).
    let mut shell = Shell::new_title(SIZE, None, 1).with_quit(false);
    let text = frame(&mut shell);
    assert!(text.contains("New game") && text.contains("Load"), "{text}");
    assert!(!text.contains("Quit") && !text.contains("Replays"), "{text}");

    // Embark's Q goes straight back to the title screen, with no question.
    shell.play(&fn_tui::keys::parse("<enter> <enter> <enter>").unwrap());
    assert!(matches!(shell.mode, fn_tui::shell::Mode::Embark(_)));
    press(&mut shell, KeyCode::Char('q'));
    assert!(!shell.exit_prompt && !shell.quit);
    assert!(matches!(shell.mode, fn_tui::shell::Mode::Title(_)));
    assert!(!frame(&mut shell).contains("Quit"));

    // A game started from it: Ctrl+C does nothing, and the menu's last row is the title screen.
    shell.play(&fn_tui::keys::parse("<enter> <enter> <enter> e y").unwrap());
    assert!(matches!(shell.mode, fn_tui::shell::Mode::Game(_)));
    shell.key(KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL));
    assert!(!shell.quit, "Ctrl+C is not a quit here");
    shell.play(&fn_tui::keys::parse(ASK_QUIT).unwrap());
    let text = frame(&mut shell);
    assert!(text.contains("Leave this game?") && text.contains("Keep playing"), "{text}");
    assert!(text.contains("Go to the title screen without saving"), "{text}");
    assert!(!text.contains("Quit the game"), "{text}");
    shell.play(&fn_tui::keys::parse("<down> <enter>").unwrap());
    assert!(matches!(shell.mode, fn_tui::shell::Mode::Title(_)), "back at the title screen");
    assert!(!shell.quit);
}
