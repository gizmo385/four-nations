//! The native title screen (#880): its steps by label through AccessKit, the same way a screen
//! reader finds them, and a walk from the title to embark by clicks and by keys that must end in
//! the same place. No renderer: kittest's `build_ui` runs only egui's pass.

use eframe::egui::{self, Event, Key};
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use fn_gui::app::GuiApp;
use fn_gui::frame::Surface;
use fn_gui::title::{load_click, menu_click, nation_click, seed_change, seed_move};
use fn_sim::Nation;
use fn_sim::embark::SurveyMode;
use fn_tui::shell::{Mode, Shell};
use fn_tui::title::{Page, SeedRow};
use fn_ui::keyev::{KeyCode, KeyEvent, KeyModifiers};

const SIZE: (u16, u16) = (120, 40);
const WINDOW: egui::Vec2 = egui::vec2(1280.0, 800.0);

fn harness(app: &mut GuiApp) -> Harness<'_> {
    Harness::builder()
        .with_size(WINDOW)
        .with_context_setup(fn_gui::setup)
        .build_ui(move |ui| app.show(ui))
}

/// What a walk ends on: the embark screen's seed, nation, map size and survey.
fn embarked(app: &GuiApp) -> (u64, Nation, String, SurveyMode) {
    match &app.shell().mode {
        Mode::Embark(e) => (e.seed(), e.nation, e.size().id.clone(), e.mode()),
        other => panic!("expected embark, got {other:?}"),
    }
}

fn page(app: &GuiApp) -> Page {
    match &app.shell().mode {
        Mode::Title(t) => t.page.clone(),
        other => panic!("expected the title, got {other:?}"),
    }
}

/// The title step's words, and that it is a native surface with its own hint bar.
#[test]
fn the_menu_shows_new_game_load_and_quit_with_the_titles_keys() {
    let mut app = GuiApp::with_shell(Shell::new_title(SIZE, None, 42));
    let mut h = harness(&mut app);
    h.run_steps(2);
    for label in ["New game", "Load", "Quit"] {
        assert!(h.query_by_label(label).is_some(), "{label}");
    }
    assert!(h.query_by_label("Continue").is_none(), "nothing saved: nothing to continue");
    drop(h);
    assert_eq!(app.frame().surface, Some(Surface::Title));
    assert_eq!(app.frame().hint_text, "TITLE Up/Down choose · Enter select");
}

/// Centred and monospace: the rows sit on the window's middle line, and the Seal's name is there.
#[test]
fn the_column_is_centred_in_the_window() {
    let mut app = GuiApp::with_shell(Shell::new_title(SIZE, None, 42));
    let mut h = harness(&mut app);
    h.run_steps(2);
    for label in ["New game", "Load", "Quit"] {
        let rect = h.get_by_label(label).rect();
        let off = (rect.center().x - WINDOW.x / 2.0).abs();
        assert!(off < 1.0, "{label} is {off} points off the middle");
    }
    // The rows stack in menu order, none overlapping.
    let tops: Vec<f32> =
        ["New game", "Load", "Quit"].iter().map(|l| h.get_by_label(l).rect().top()).collect();
    assert!(tops.windows(2).all(|w| w[0] < w[1]), "{tops:?}");
}

/// Quit is a row only where the host can quit (#866): the browser's title has none.
#[test]
fn quit_is_not_a_row_where_the_host_cannot_quit() {
    let mut app = GuiApp::with_shell(Shell::new_title(SIZE, None, 42).with_quit(false));
    let mut h = harness(&mut app);
    h.run_steps(2);
    assert!(h.query_by_label("New game").is_some());
    assert!(h.query_by_label("Load").is_some());
    assert!(h.query_by_label("Quit").is_none());
}

/// The walk the browser check does, by keys: New game, the third nation, a typed seed, embark.
fn walk_by_keys(seed_hint: u64) -> GuiApp {
    let mut app = GuiApp::with_shell(Shell::new_title(SIZE, None, seed_hint));
    let mut h = harness(&mut app);
    h.run_steps(2);
    h.key_press(Key::Enter); // New game
    h.run_steps(2);
    h.event(Event::Text("3".into())); // Earth Kingdom
    h.run_steps(2);
    h.key_press(Key::Enter);
    h.run_steps(2);
    h.event(Event::Text("7".into())); // the seed's digits
    h.run_steps(2);
    h.key_press(Key::Enter); // Embark
    h.run_steps(2);
    drop(h);
    app
}

#[test]
fn clicks_and_keys_reach_the_same_embark() {
    let by_keys = walk_by_keys(42);
    assert_eq!(embarked(&by_keys), (7, Nation::Earth, "small".into(), SurveyMode::Exact));

    let mut app = GuiApp::with_shell(Shell::new_title(SIZE, None, 42));
    let mut h = harness(&mut app);
    h.run_steps(2);
    h.get_by_label("New game").click();
    h.run_steps(2);
    // The nation step: the pointer is on Air; a click on a nation is its digit key.
    assert!(h.query_by_label("Fire Nation").is_some());
    h.get_by_label("Earth Kingdom").click();
    h.run_steps(2);
    h.get_by_label("Choose Earth Kingdom").click();
    h.run_steps(2);
    // The seed step: type the digits (a click can't), then the button.
    assert!(h.query_by_label("Seed").is_some());
    h.event(Event::Text("7".into()));
    h.run_steps(2);
    h.get_by_label("Embark").click();
    h.run_steps(2);
    drop(h);
    assert_eq!(embarked(&app), embarked(&by_keys));
    assert_eq!(app.frame().surface, Some(Surface::Embark));
}

/// Back is `Esc` on every step, so a click steps back as the key does.
#[test]
fn back_buttons_press_escape() {
    let mut app = GuiApp::with_shell(Shell::new_title(SIZE, None, 42));
    let mut h = harness(&mut app);
    h.run_steps(2);
    h.get_by_label("New game").click();
    h.run_steps(2);
    h.get_by_label("Choose Air Nomads").click();
    h.run_steps(2);
    h.get_by_label("Back").click();
    h.run_steps(2);
    h.get_by_label("Back").click();
    h.run_steps(2);
    assert!(h.query_by_label("New game").is_some());
    drop(h);
    assert_eq!(page(&app), Page::Menu);
}

/// The seed step's arrows change the row they sit on, moving the highlight there first, and the
/// result carries into embark as the keys' would.
#[test]
fn the_seed_step_arrows_change_size_and_survey() {
    let mut app = GuiApp::with_shell(Shell::new_title(SIZE, None, 9));
    let mut h = harness(&mut app);
    h.run_steps(2);
    h.get_by_label("New game").click();
    h.run_steps(2);
    h.get_by_label("Choose Air Nomads").click();
    h.run_steps(2);
    h.get_by_label("Next Survey").click();
    h.run_steps(2);
    h.get_by_label("Next Size").click();
    h.run_steps(2);
    h.get_by_label("Embark").click();
    h.run_steps(2);
    drop(h);
    let (seed, nation, size, mode) = embarked(&app);
    assert_eq!((seed, nation, mode), (9, Nation::Air, SurveyMode::Rough));
    assert_ne!(size, "small", "the Size row's right arrow moved off the first size");
}

/// Quit, when it is a row, ends the program as the key does.
#[test]
fn clicking_quit_quits() {
    let mut app = GuiApp::with_shell(Shell::new_title(SIZE, None, 42));
    let mut h = harness(&mut app);
    h.run_steps(2);
    h.get_by_label("Quit").click();
    h.run_steps(2);
    drop(h);
    assert!(app.shell().quit);
}

/// The Load step lists the saves by name, and a click then Load starts that game.
#[test]
fn the_load_step_lists_saves_and_loads_one() {
    let dir = std::env::temp_dir().join(format!("fn_gui_title_{}", std::process::id()));
    let store = fn_ui::saves::DiskStore::shared(&dir);
    let session = fn_tui::new_session(7, Nation::Earth);
    fn_ui::saves::save(store.as_ref(), "camp", &session).unwrap();

    let mut app = GuiApp::with_shell(Shell::new_title(SIZE, Some(store), 1));
    let mut h = harness(&mut app);
    h.run_steps(2);
    assert!(h.query_by_label("Continue").is_some(), "a loadable save: Continue shows");
    h.get_by_label("Load").click();
    h.run_steps(2);
    assert!(h.query_by_label("camp").is_some());
    h.get_by_label("camp").click();
    h.run_steps(2);
    h.get_by_label("Load").click();
    h.run_steps(2);
    drop(h);
    let _ = std::fs::remove_dir_all(&dir);
    match &app.shell().mode {
        Mode::Game(game) => {
            assert_eq!(game.session.world.seed, 7);
            assert_eq!(game.session.world.nation, Nation::Earth);
        }
        other => panic!("expected the game, got {other:?}"),
    }
}

fn key(code: KeyCode) -> KeyEvent {
    KeyEvent::new(code, KeyModifiers::NONE)
}

/// The click-to-key table, as data: what each click presses.
#[test]
fn clicks_map_to_the_keys_the_step_answers() {
    use KeyCode::{Char, Down, Enter, Left, Right, Up};
    // The menu: the arrows to the row, then Enter.
    assert_eq!(menu_click(0, 2), [key(Down), key(Down), key(Enter)]);
    assert_eq!(menu_click(2, 0), [key(Up), key(Up), key(Enter)]);
    assert_eq!(menu_click(1, 1), [key(Enter)]);
    // Nations: the digit, and Enter for a double-click.
    assert_eq!(nation_click(2, false), [key(Char('3'))]);
    assert_eq!(nation_click(0, true), [key(Char('1')), key(Enter)]);
    // Seed rows: the arrows to the row; the row's own arrows then Left or Right.
    assert_eq!(seed_move(SeedRow::Digits, SeedRow::Survey), [key(Down), key(Down)]);
    assert_eq!(seed_move(SeedRow::Difficulty, SeedRow::Size), [key(Up), key(Up)]);
    assert_eq!(seed_change(SeedRow::Digits, SeedRow::Size, 1), [key(Down), key(Right)]);
    assert_eq!(seed_change(SeedRow::Size, SeedRow::Size, -1), [key(Left)]);
    // Saves: the arrows, and Enter for a double-click.
    assert_eq!(load_click(0, 1, false), [key(Down)]);
    assert_eq!(load_click(1, 0, true), [key(Up), key(Enter)]);
}

/// The Continue and Load rows' lines stay inside their row (#943): a save with a longest-allowed
/// name and a clock-derived 19-digit seed says "Water Tribes · Day 1" (no seed, no tick, no
/// nation repeated as a slot id), and `fitted` cuts whatever is still too wide with an ellipsis.
#[test]
fn a_long_save_name_and_seed_stay_inside_the_continue_row() {
    let dir = std::env::temp_dir().join(format!("fn_gui_title_long_{}", std::process::id()));
    let store = fn_ui::saves::DiskStore::shared(&dir);
    let session = fn_tui::new_session(1_794_509_133_825_000_000, Nation::Water);
    let name = "water-tribe_".repeat(4)[..fn_ui::saves::MAX_NAME].to_owned();
    fn_ui::saves::save(store.as_ref(), &name, &session).unwrap();

    // The line itself is short and says no seed.
    let defs = fn_sim::Defs::bundled();
    let slots = fn_ui::saves::list(store.as_ref()).unwrap();
    let said = slots[0].summary(&defs, "·");
    assert_eq!(said, "Water Tribes · Day 1");

    for width in [1280.0_f32, 420.0, 300.0] {
        let mut app = GuiApp::with_shell(Shell::new_title(SIZE, Some(store.clone()), 1));
        let mut h = Harness::builder()
            .with_size(egui::vec2(width, 800.0))
            .with_context_setup(fn_gui::setup)
            .build_ui(|ui| {
                let paint = |text: &str, size: f32, max: f32| {
                    let g = fn_gui::title::fitted(
                        ui.painter(),
                        text,
                        egui::FontId::new(size, egui::FontFamily::Monospace),
                        egui::Color32::WHITE,
                        max,
                    );
                    assert!(g.size().x <= max, "{text:?} is {} wide in {max}", g.size().x);
                    g.text().to_owned()
                };
                let raw = format!("{name}: Water Tribes, seed 1794509133825000000, Spring Y4 - 4:54 left (tick 114060)");
                let row = (width - 32.0).clamp(240.0, 560.0) - 2.0 * 22.0;
                let cut = paint(&raw, 12.0, row - 24.0);
                assert!(cut.ends_with('…') && cut.len() < raw.len(), "{cut:?}");
                assert_eq!(paint("Water Tribes · Spring Y4", 12.0, 1000.0), "Water Tribes · Spring Y4");
                app.show(ui);
            });
        h.run_steps(2);
        let rect = h.get_by_label("Continue").rect();
        assert!(rect.left() >= 0.0 && rect.right() <= width, "{width}: {rect:?}");
    }
    let _ = std::fs::remove_dir_all(&dir);
}
