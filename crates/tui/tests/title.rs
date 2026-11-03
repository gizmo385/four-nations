//! The title screen through the public API (#280): golden frames for the menu and the nation
//! picker (rewrite with `FN_BLESS=1 cargo test -p fn_tui`, and say so in the PR), and the shell's
//! own switch between the title screen, a game, and back.

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use fn_sim::Nation;
use fn_tui::color::{ColorDepth, Palette};
use fn_tui::frame::{render_shell, to_text};
use fn_tui::shell::Shell;

const SIZE: (u16, u16) = (120, 40);

fn golden(name: &str, actual: &str) {
    let path = format!("{}/tests/golden/{name}.txt", env!("CARGO_MANIFEST_DIR"));
    if std::env::var_os("FN_BLESS").is_some() {
        std::fs::write(&path, actual).unwrap();
    }
    let expected = std::fs::read_to_string(&path).expect("golden missing; run with FN_BLESS=1");
    assert_eq!(actual, expected, "{name}: the frame changed; see the module docs to rebless");
}

fn frame(shell: &mut Shell) -> String {
    let p = Palette::new(shell.defs(), shell.nation(), ColorDepth::TrueColor, false);
    to_text(&render_shell(shell, &p, SIZE))
}

fn press(shell: &mut Shell, code: KeyCode) {
    shell.key(KeyEvent::new(code, KeyModifiers::NONE));
}

// ---------------------------------------------------------------------------------------------
// Golden frames
// ---------------------------------------------------------------------------------------------

/// With no saves to continue, the fresh title screen: New game, Load, Quit.
#[test]
fn golden_title_screen() {
    let mut shell = Shell::new_title(SIZE, None, 1);
    golden("title_menu", &frame(&mut shell));
}

/// The nation picker, on the first nation (Air Nomads), its blurb in its own colours.
#[test]
fn golden_nation_picker() {
    let mut shell = Shell::new_title(SIZE, None, 1);
    press(&mut shell, KeyCode::Enter);
    golden("title_nation_picker", &frame(&mut shell));
}

// ---------------------------------------------------------------------------------------------
// The shell's own switching
// ---------------------------------------------------------------------------------------------

#[test]
fn a_full_script_starts_a_game_from_the_title_screen_and_plays_it() {
    let mut shell = Shell::new_title(SIZE, None, 7);
    let script = fn_tui::keys::parse("<enter> <right> <right> <enter> <enter> <tick 600>").unwrap();
    shell.play(&script);
    let text = frame(&mut shell);
    assert_eq!(shell.nation(), Nation::Earth);
    assert!(text.contains("EARTH KINGDOM"), "{text}");
    assert!(text.contains("01:00"), "<tick 600> is a minute:\n{text}");
}

#[test]
fn quitting_a_game_asks_whether_to_go_to_the_title_screen_or_exit() {
    let mut shell = Shell::new_title(SIZE, None, 1);
    let start = fn_tui::keys::parse("<enter> <enter> <enter>").unwrap();
    shell.play(&start);
    assert!(matches!(shell.mode, fn_tui::shell::Mode::Game(_)));

    press(&mut shell, KeyCode::Char('q'));
    assert!(!shell.exit_prompt, "q only asks to quit the game first");
    press(&mut shell, KeyCode::Char('y'));
    assert!(shell.exit_prompt, "confirming asks where to go next");
    assert!(!shell.quit);

    // Anything but T, Q or Enter cancels and the game resumes.
    press(&mut shell, KeyCode::Char('x'));
    assert!(!shell.exit_prompt && !shell.quit);
    assert!(matches!(shell.mode, fn_tui::shell::Mode::Game(_)));

    press(&mut shell, KeyCode::Char('q'));
    press(&mut shell, KeyCode::Char('y'));
    press(&mut shell, KeyCode::Char('t'));
    assert!(!shell.exit_prompt && !shell.quit);
    assert!(matches!(shell.mode, fn_tui::shell::Mode::Title(_)), "back at the title screen");

    press(&mut shell, KeyCode::Down);
    press(&mut shell, KeyCode::Down);
    // The menu is New game, Load, Quit with no saves: the third row is Quit.
    press(&mut shell, KeyCode::Enter);
    assert!(shell.quit, "the title screen's own Quit stops the process without asking again");
}

#[test]
fn q_then_enter_exits_the_process_instead_of_the_title_screen() {
    let mut shell = Shell::new_title(SIZE, None, 1);
    let start = fn_tui::keys::parse("<enter> <enter> <enter>").unwrap();
    shell.play(&start);
    press(&mut shell, KeyCode::Char('q'));
    press(&mut shell, KeyCode::Char('y'));
    press(&mut shell, KeyCode::Enter);
    assert!(shell.quit);
}

#[test]
fn seed_and_nation_together_skip_the_menu_straight_into_a_game() {
    let mut shell = Shell::new_game(fn_tui::new_session(7, Nation::Fire), SIZE, None);
    let text = frame(&mut shell);
    assert!(text.contains("FIRE NATION"), "{text}");
    assert!(!text.contains("FOUR NATIONS"), "no title screen shows at all:\n{text}");
}
