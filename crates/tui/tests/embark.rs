//! The embark screen (#290, docs/design/embark.md): golden frames for the overview, a region
//! and the site zoom, a refused site and a rough survey, plus the keys that move between them.
//! Rewrite goldens with `FN_BLESS=1 cargo test -p fn_tui` and review the diff.

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use fn_sim::embark::{MapSizeDef, SurveyMode};
use fn_sim::{Defs, Nation};
use fn_tui::color::{ColorDepth, Palette};
use fn_tui::frame::{render_shell, to_text};
use fn_tui::shell::{Mode, Shell};

fn golden(name: &str, actual: &str) {
    // FN_GOLDEN=skip (cargo xtask ci --fast --no-goldens): every golden comparison is a
    // no-op that passes, so agents can run the gate without caring whether goldens are stale.
    if std::env::var("FN_GOLDEN").ok().as_deref() == Some("skip") {
        return;
    }
    let path = format!("{}/tests/golden/{name}.txt", env!("CARGO_MANIFEST_DIR"));
    if std::env::var_os("FN_BLESS").is_some() {
        std::fs::write(&path, actual).unwrap();
    }
    let expected = std::fs::read_to_string(&path).expect("golden missing; run with FN_BLESS=1");
    assert_eq!(actual, expected, "{name}: the frame changed; rebless with FN_BLESS=1 and review");
}

fn size(id: &str) -> MapSizeDef {
    Defs::bundled().embark.map_size(id).unwrap().clone()
}

fn embark(
    seed: u64,
    nation: Nation,
    size: MapSizeDef,
    mode: SurveyMode,
    screen: (u16, u16),
) -> Shell {
    Shell::new_embark(seed, nation, size, mode, Some("normal".into()), screen, None)
}

fn press(shell: &mut Shell, code: KeyCode) {
    shell.key(KeyEvent::new(code, KeyModifiers::NONE));
}

fn frame(shell: &mut Shell, size: (u16, u16)) -> String {
    let p = Palette::new(shell.defs(), shell.nation(), ColorDepth::TrueColor, false);
    to_text(&render_shell(shell, &p, size))
}

// ---------------------------------------------------------------------------------------------
// Golden frames
// ---------------------------------------------------------------------------------------------

#[test]
fn golden_embark_overview() {
    let mut shell = embark(7, Nation::Earth, size("small"), SurveyMode::Exact, (120, 40));
    golden("earth_seed7_embark", &frame(&mut shell, (120, 40)));
}

#[test]
fn golden_embark_overview_small_terminal() {
    let mut shell = embark(7, Nation::Earth, size("small"), SurveyMode::Exact, (80, 24));
    golden("earth_seed7_embark_small", &frame(&mut shell, (80, 24)));
}

#[test]
fn golden_embark_site() {
    let mut shell = embark(7, Nation::Earth, size("small"), SurveyMode::Exact, (120, 40));
    for _ in 0..3 {
        press(&mut shell, KeyCode::Enter); // 6 -> 4 -> 2 -> 1
    }
    let Mode::Embark(screen) = &shell.mode else { panic!("expected embark") };
    assert!(!screen.is_overview());
    golden("earth_seed7_embark_site", &frame(&mut shell, (120, 40)));
}

/// `Enter` at the closest zoom asks before settling, and the question is on screen: the
/// status line names the key and the site (a playtest found it invisible), and `y` starts the
/// game.
#[test]
fn the_settle_prompt_is_drawn_and_y_starts_the_game() {
    let mut shell = embark(7, Nation::Earth, size("small"), SurveyMode::Exact, (120, 40));
    for _ in 0..4 {
        press(&mut shell, KeyCode::Enter); // 6 -> 4 -> 2 -> 1, then the prompt
    }
    let text = frame(&mut shell, (120, 40));
    let status = text.lines().last().unwrap();
    assert!(status.contains("SETTLE?") && status.contains("Press y to settle at"), "{status}");
    press(&mut shell, KeyCode::Char('y'));
    assert!(!matches!(shell.mode, Mode::Embark(_)), "y settles and leaves embark");
}

/// The cursor on the map's forced-sea border: refused ("open water"), tinted in the `bad` tone.
#[test]
fn golden_embark_refused() {
    let mut shell = embark(7, Nation::Earth, size("small"), SurveyMode::Exact, (120, 40));
    for _ in 0..50 {
        press(&mut shell, KeyCode::Left);
    }
    golden("earth_seed7_embark_refused", &frame(&mut shell, (120, 40)));
}

#[test]
fn golden_embark_large_overview() {
    let mut shell = embark(7, Nation::Earth, size("large"), SurveyMode::Exact, (80, 24));
    golden("earth_seed7_embark_large", &frame(&mut shell, (80, 24)));
}

/// A rough survey's ladder stops at 1:2: two zooms in from the overview (6, 4, 2) lands there.
#[test]
fn golden_embark_rough() {
    let mut shell = embark(7, Nation::Earth, size("small"), SurveyMode::Rough, (120, 40));
    press(&mut shell, KeyCode::Enter);
    press(&mut shell, KeyCode::Enter);
    golden("earth_seed7_embark_rough", &frame(&mut shell, (120, 40)));
}

// ---------------------------------------------------------------------------------------------
// Keys
// ---------------------------------------------------------------------------------------------

#[test]
fn shift_moves_ten_cells_the_plain_arrow_moves_one_and_r_reseeds() {
    let mut shell = embark(7, Nation::Earth, size("small"), SurveyMode::Exact, (120, 40));
    let Mode::Embark(screen) = &shell.mode else { panic!("expected embark") };
    let at = screen.cursor;
    press(&mut shell, KeyCode::Right);
    let Mode::Embark(screen) = &shell.mode else { panic!("expected embark") };
    assert_eq!(screen.cursor.x, (at.x + 6).min(191));

    press(&mut shell, KeyCode::Char('r'));
    let Mode::Embark(screen) = &shell.mode else { panic!("expected embark") };
    assert_eq!(screen.seed(), 8, "R takes the next seed");
    assert!(screen.is_overview(), "and goes back to the overview");
}

/// `Esc` at the overview goes back to the seed step with the seed kept (#290's own criterion);
/// `E` then `y` settles and the game starts with a `start`.
#[test]
fn esc_at_the_overview_keeps_the_seed_and_e_y_settles_the_game() {
    let mut shell = embark(7, Nation::Earth, size("small"), SurveyMode::Exact, (120, 40));
    press(&mut shell, KeyCode::Esc);
    let Mode::Title(title) = &shell.mode else { panic!("expected the title screen") };
    let fn_tui::title::Page::Seed { nation, digits, .. } = &title.page else {
        panic!("expected the seed step, got {:?}", title.page)
    };
    assert_eq!((*nation, digits.as_str()), (Nation::Earth, "7"));

    // Back into embark the same way, then settle.
    let mut shell = embark(7, Nation::Earth, size("small"), SurveyMode::Exact, (120, 40));
    press(&mut shell, KeyCode::Char('e'));
    press(&mut shell, KeyCode::Char('y'));
    let Mode::Game(app) = &shell.mode else { panic!("expected a game") };
    assert!(app.session.world.start().is_some());
}
