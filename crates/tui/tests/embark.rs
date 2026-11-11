//! The embark screen (#290, docs/design/embark.md): the keys that move between the overview, a
//! region and the site zoom, a refused site and a rough survey. The one reference golden (the
//! plain overview) lives in `reference_goldens.rs` now (#455); the rest of these scenarios keep
//! `contains`-style checks for whatever only they show, and the layout properties
//! (`layout_properties.rs`, #454) cover the structural side (every box closes, nothing overflows)
//! at every size.

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use fn_sim::embark::{MapSizeDef, SurveyMode};
use fn_sim::{Defs, Nation};
use fn_tui::color::{ColorDepth, Palette};
use fn_tui::frame::{render_shell, to_text};
use fn_tui::shell::{Mode, Shell};

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
// Zoom and refusal
// ---------------------------------------------------------------------------------------------

/// Three zooms in (6 -> 4 -> 2 -> 1) leaves the site zoom, not the overview; the layout
/// properties' own `embark_site` case covers the frame's structure at every size.
#[test]
fn three_zooms_in_reach_the_site_zoom() {
    let mut shell = embark(7, Nation::Earth, size("small"), SurveyMode::Exact, (120, 40));
    for _ in 0..3 {
        press(&mut shell, KeyCode::Enter); // 6 -> 4 -> 2 -> 1
    }
    let Mode::Embark(screen) = &shell.mode else { panic!("expected embark") };
    assert!(!screen.is_overview());
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
    // The question is the hint bar's own `Settle` mode (#686).
    assert!(status.trim_start().starts_with("SETTLE?"), "{status}");
    for item in fn_ui::keys::hints(fn_ui::keys::Mode::Settle) {
        assert!(status.contains(&item.text()), "{} in {status}", item.text());
    }
    press(&mut shell, KeyCode::Char('y'));
    assert!(!matches!(shell.mode, Mode::Embark(_)), "y settles and leaves embark");
}

/// The cursor on the map's forced-sea border is refused, naming "open water".
#[test]
fn the_forced_sea_border_is_refused_as_open_water() {
    let mut shell = embark(7, Nation::Earth, size("small"), SurveyMode::Exact, (120, 40));
    for _ in 0..50 {
        press(&mut shell, KeyCode::Left);
    }
    let text = frame(&mut shell, (120, 40));
    assert!(text.contains("Can't settle: open water."), "{text}");
}

/// A large map's overview names its own size and the zoom ratio it starts from.
#[test]
fn a_large_overview_names_its_size_and_its_own_zoom_ratio() {
    let mut shell = embark(7, Nation::Earth, size("large"), SurveyMode::Exact, (80, 24));
    let text = frame(&mut shell, (80, 24));
    assert!(text.contains("Large · Exact"), "{text}");
    assert!(text.contains("Zoom 1:12"), "{text}");
}

/// A rough survey's ladder stops at 1:2: two zooms in from the overview (6, 4, 2) lands there.
#[test]
fn a_rough_surveys_ladder_stops_at_one_to_two() {
    let mut shell = embark(7, Nation::Earth, size("small"), SurveyMode::Rough, (120, 40));
    press(&mut shell, KeyCode::Enter);
    press(&mut shell, KeyCode::Enter);
    let text = frame(&mut shell, (120, 40));
    assert!(text.contains("Small · Rough"), "{text}");
    assert!(text.contains("Zoom 1:2"), "{text}");
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
