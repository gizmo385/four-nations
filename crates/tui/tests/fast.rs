//! `--fast` (#321): the host setting adds x10, x20 and x50 to the `+`/`-` ladder, applies to
//! every game in the process (including after a new map or a nation switch), and is off by
//! default. Through the public API, as a real `fn_tui --fast` would reach it.

use fn_sim::Nation;
use fn_tui::cli::{self, Args};
use fn_tui::keyev::{KeyCode, KeyEvent, KeyModifiers};
use fn_tui::shell::{Mode, Shell};

const SIZE: (u16, u16) = (120, 40);

fn press(shell: &mut Shell, code: KeyCode) {
    shell.key(KeyEvent::new(code, KeyModifiers::NONE));
}

fn args(extra: &str) -> Args {
    cli::parse(format!("7 earth {extra}").split_whitespace().map(str::to_owned)).unwrap()
}

fn speed(shell: &Shell) -> u32 {
    let Mode::Game(app) = &shell.mode else { panic!("expected a game") };
    app.session.clock.speed()
}

/// Without `--fast`, `+` tops out at x5, same as ever (#321's acceptance criteria: "Without it,
/// the ladder stays `[1, 2, 3, 5]`").
#[test]
fn without_fast_the_ladder_is_unchanged() {
    let mut shell = Shell::for_args(&args(""), SIZE, None, 0).unwrap();
    for _ in 0..10 {
        press(&mut shell, KeyCode::Char('+'));
    }
    assert_eq!(speed(&shell), 5);
}

/// `--fast` (alone, or with `SEED NATION`) adds x10, x20 and x50.
#[test]
fn fast_adds_the_three_higher_speeds() {
    let mut shell = Shell::for_args(&args("--fast"), SIZE, None, 0).unwrap();
    let mut seen = vec![speed(&shell)];
    for _ in 0..6 {
        press(&mut shell, KeyCode::Char('+'));
        seen.push(speed(&shell));
    }
    assert_eq!(seen, vec![1, 2, 3, 5, 10, 20, 50]);
}

/// `App::restart` (a new map in place, the fallback `R` takes when no map-size preset matches;
/// the common `R` path reopens embark instead, whose `Settle` goes through `Shell::start`, also
/// tested here) keeps `--fast`'s ladder: it's a host setting, not the player's own clock state,
/// which a restart otherwise starts over (`App::restart`'s doc comment).
#[test]
fn fast_survives_app_restart() {
    let mut shell = Shell::for_args(&args("--fast"), SIZE, None, 0).unwrap();
    for _ in 0..6 {
        press(&mut shell, KeyCode::Char('+'));
    }
    assert_eq!(speed(&shell), 50);
    let Mode::Game(app) = &mut shell.mode else { panic!("expected a game") };
    app.restart(8, Nation::Fire);
    assert!(app.session.clock.is_fast(), "the restarted game should still be fast");
}

/// A nation switch (the menu's Switch nation…, confirmed) keeps `--fast`'s ladder too.
#[test]
fn fast_survives_a_nation_switch() {
    let mut shell = Shell::for_args(&args("--fast"), SIZE, None, 0).unwrap();
    shell.play(
        &fn_tui::keys::parse("<esc> <down*5> <enter> <down> <enter> <down> <enter>").unwrap(),
    );
    let Mode::Game(app) = &shell.mode else { panic!("expected a game") };
    assert_eq!(app.session.world.nation, Nation::Water);
    assert!(app.session.clock.is_fast(), "the switched nation should still be fast");
}

/// `--help` lists `--fast` (#321's acceptance criteria).
#[test]
fn help_lists_fast() {
    assert!(cli::usage().contains("--fast"));
}
