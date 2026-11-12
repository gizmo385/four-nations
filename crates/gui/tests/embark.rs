//! The native embark screen (#883): its report and buttons by label through AccessKit, and clicks
//! on the survey that must land the cursor, and the settled site, exactly where the same
//! movement by keys does. No renderer: kittest's `build_ui_state` runs only egui's pass.

use eframe::egui::{self, Event, Key, Modifiers, PointerButton, Pos2};
use egui_kittest::Harness;
use egui_kittest::kittest::{NodeT, Queryable};
use fn_gui::app::GuiApp;
use fn_gui::embark::click_keys;
use fn_gui::frame::Surface;
use fn_sim::Pos;
use fn_sim::embark::SurveyMode;
use fn_sim::{Defs, Nation};
use fn_tui::embark::EmbarkScreen;
use fn_tui::shell::{Mode, Shell};
use fn_ui::keyev::{KeyCode, KeyEvent, KeyModifiers};

const SIZE: (u16, u16) = (120, 40);
const WINDOW: egui::Vec2 = egui::vec2(1280.0, 800.0);

fn shell() -> Shell {
    let size = Defs::bundled().embark.map_size("small").unwrap().clone();
    Shell::new_embark(7, Nation::Earth, size, SurveyMode::Exact, Some("normal".into()), SIZE, None)
}

fn harness() -> Harness<'static, GuiApp> {
    let mut h = Harness::builder()
        .with_size(WINDOW)
        .with_context_setup(fn_gui::setup)
        .build_ui_state(|ui, app: &mut GuiApp| app.show(ui), GuiApp::with_shell(shell()));
    h.run_steps(3);
    h
}

fn screen(app: &GuiApp) -> &EmbarkScreen {
    match &app.shell().mode {
        Mode::Embark(e) => e,
        other => panic!("expected embark, got {other:?}"),
    }
}

fn block_of(e: &EmbarkScreen) -> Pos {
    Pos::new(e.cursor.x.div_euclid(e.block_scale()), e.cursor.y.div_euclid(e.block_scale()))
}

fn press(shell: &mut Shell, code: KeyCode, n: usize) {
    for _ in 0..n {
        shell.key(KeyEvent::new(code, KeyModifiers::NONE));
    }
}

fn typed(shell: &mut Shell, c: char) {
    press(shell, KeyCode::Char(c), 1);
}

/// A primary click at `p`, as a pointer does it: move, press, release.
fn click_at(h: &mut Harness<'_, GuiApp>, p: Pos2) {
    h.hover_at(p);
    h.run_steps(1);
    for pressed in [true, false] {
        h.event(Event::PointerButton {
            pos: p,
            button: PointerButton::Primary,
            pressed,
            modifiers: Modifiers::NONE,
        });
        h.run_steps(1);
    }
    h.run_steps(2);
}

/// A click on the middle of block `at`, wherever the map put it this frame.
fn click_block(h: &mut Harness<'_, GuiApp>, at: Pos) {
    let g = h.state().frame().embark.geometry.expect("the map was drawn");
    click_at(h, g.block_rect(at.x, at.y).center());
}

/// A keyboard shell sized as the window's: the same ladder, so the same blocks.
fn twin(h: &Harness<'_, GuiApp>) -> Shell {
    let (cols, rows) = h.state().frame().size();
    let mut shell = shell();
    shell.resize(cols, rows);
    shell
}

fn cursor(shell: &Shell) -> Pos {
    match &shell.mode {
        Mode::Embark(e) => e.cursor,
        other => panic!("expected embark, got {other:?}"),
    }
}

#[test]
fn embark_is_native_and_shows_the_report_and_its_keys() {
    let h = harness();
    assert_eq!(h.state().frame().surface, Some(Surface::Embark));
    assert!(h.state().frame().hint_text.starts_with("EMBARK Arrows move"), "{}", {
        h.state().frame().hint_text.clone()
    });
    for words in ["CHOOSE A SITE", "Seed 7", "Earth Kingdom", "Zoom 1:6 overview", "Room to roam"] {
        assert!(h.query_by_label_contains(words).is_some(), "{words}");
    }
    for button in ["Settle", "Zoom in", "Zoom out", "New seed", "Back", "Keys"] {
        assert!(h.query_by_label(button).is_some(), "{button}");
    }
    // The overview has nothing to zoom out of, and a closer look has the water line and the
    // groups of the WITHIN section.
    assert!(h.query_by_label("Zoom out").unwrap().accesskit_node().is_disabled());
    // Tiles are square, and the whole survey is in the map's rectangle at the overview.
    let g = h.state().frame().embark.geometry.expect("the map was drawn");
    let (first, last) = (g.block_rect(0, 0), g.block_rect(g.blocks.0 - 1, g.blocks.1 - 1));
    assert_eq!(first.width(), first.height());
    assert!(g.rect.contains_rect(first) && g.rect.contains_rect(last), "{g:?}");
    assert!(h.query_by_label_contains("Water").is_none());
}

/// The pure part: the arrows between two blocks, ten at a time with Shift.
#[test]
fn a_click_is_the_arrows_between_the_blocks() {
    let key = |code, shift| {
        KeyEvent::new(code, if shift { KeyModifiers::SHIFT } else { KeyModifiers::NONE })
    };
    assert!(click_keys(Pos::new(4, 4), Pos::new(4, 4)).is_empty());
    assert_eq!(
        click_keys(Pos::new(4, 4), Pos::new(6, 3)),
        [key(KeyCode::Right, false), key(KeyCode::Right, false), key(KeyCode::Up, false)]
    );
    let far = click_keys(Pos::new(30, 2), Pos::new(7, 15));
    assert_eq!(
        far.iter()
            .filter(|k| k.code == KeyCode::Left && k.modifiers.contains(KeyModifiers::SHIFT))
            .count(),
        2
    );
    assert_eq!(
        far.iter()
            .filter(|k| k.code == KeyCode::Left && !k.modifiers.contains(KeyModifiers::SHIFT))
            .count(),
        3
    );
    assert_eq!(far.iter().filter(|k| k.code == KeyCode::Down).count(), 4);
}

/// At the overview a block is `N` tiles: a click lands in the block clicked, and the cursor is
/// where the arrow keys would have taken it.
#[test]
fn a_click_on_the_overview_moves_the_cursor_as_the_arrows_do() {
    let mut h = harness();
    let mut keys = twin(&h);
    let from = block_of(screen(h.state()));
    // Far enough to take a Shift move and a few single ones, left and up.
    let target = Pos::new((from.x - 11).max(1), (from.y - 3).max(1));
    click_block(&mut h, target);
    assert_eq!(block_of(screen(h.state())), target);

    press(&mut keys, KeyCode::Left, usize::try_from(from.x - target.x).unwrap());
    press(&mut keys, KeyCode::Up, usize::try_from(from.y - target.y).unwrap());
    assert_eq!(screen(h.state()).cursor, cursor(&keys));

    // A click on the block the cursor is in moves nothing.
    let at = screen(h.state()).cursor;
    click_block(&mut h, target);
    assert_eq!(screen(h.state()).cursor, at);
}

/// The same at the closest zoom, where a block is a tile, and then the settled site is the same
/// site: both shells end in a game founded at the same cursor.
#[test]
fn a_clicked_site_settles_where_the_keyboard_path_does() {
    let mut h = harness();
    let mut keys = twin(&h);
    // Zoom in through the buttons; the keyboard twin uses Enter.
    for _ in 0..3 {
        h.get_by_label("Zoom in").click();
        h.run_steps(3);
        press(&mut keys, KeyCode::Enter, 1);
    }
    assert_eq!(screen(h.state()).block_scale(), 1);
    assert_eq!(screen(h.state()).cursor, cursor(&keys));
    assert!(h.query_by_label_contains("Water").is_some(), "the site zoom reports water");

    // A few tiles off, on a site that can be settled (the default one sits by the water).
    let from = screen(h.state()).cursor;
    let e = screen(h.state());
    let defs = Defs::bundled();
    let (dx, dy) = (-3..=3)
        .flat_map(|dx| (-3..=3).map(move |dy| (dx, dy)))
        .filter(|&(dx, dy)| dx * dx + dy * dy >= 4)
        .find(|&(dx, dy)| {
            let at = Pos::new(from.x + dx, from.y + dy);
            e.survey().report(at, e.mode(), e.difficulty(), &defs).problem.is_none()
        })
        .expect("a settleable site near the default one");
    let target = Pos::new(from.x + dx, from.y + dy);
    click_block(&mut h, target);
    press(
        &mut keys,
        if dx < 0 { KeyCode::Left } else { KeyCode::Right },
        dx.unsigned_abs() as usize,
    );
    press(&mut keys, if dy < 0 { KeyCode::Up } else { KeyCode::Down }, dy.unsigned_abs() as usize);
    assert_eq!(screen(h.state()).cursor, target);
    assert_eq!(screen(h.state()).cursor, cursor(&keys));

    // Settle: the button's `E` and then the question's own button, against `E` and `Y`.
    h.get_by_label("Settle").click();
    h.run_steps(3);
    assert!(h.state().frame().hint_text.starts_with("SETTLE?"), "{}", {
        h.state().frame().hint_text.clone()
    });
    h.get_by_label("Settle here").click();
    h.run_steps(3);
    typed(&mut keys, 'e');
    typed(&mut keys, 'y');

    let app = h.into_state();
    let (Mode::Game(by_click), Mode::Game(by_keys)) = (&app.shell().mode, &keys.mode) else {
        panic!("both should have founded a game");
    };
    assert_eq!(by_click.ui.cursor, by_keys.ui.cursor);
    let spots = |app: &fn_ui::app::App| -> Vec<Pos> {
        app.session.world.units.iter().map(|u| u.pos).collect()
    };
    assert!(!spots(by_click).is_empty());
    assert_eq!(spots(by_click), spots(by_keys));
}

/// A refused site says why in the hint bar, as the keys do, and a double-click on the closest
/// zoom is `Enter`: it asks.
#[test]
fn a_refused_site_is_a_notice_and_the_question_takes_the_map_clicks() {
    let mut h = harness();
    // The map's border is always sea.
    click_block(&mut h, Pos::new(0, 0));
    h.get_by_label("Settle").click();
    h.run_steps(3);
    assert!(h.state().frame().hint_text.contains("Can't settle: open water."), "{}", {
        h.state().frame().hint_text.clone()
    });

    // Back to a site worth asking about: a new seed's default.
    h.get_by_label("New seed").click();
    h.run_steps(3);
    assert_eq!(screen(h.state()).seed(), 8);
    for _ in 0..3 {
        h.key_press(Key::Enter);
        h.run_steps(2);
    }
    h.key_press(Key::Enter);
    h.run_steps(2);
    assert!(screen(h.state()).asking());
    // With the question up the map takes no click.
    let at = screen(h.state()).cursor;
    click_block(&mut h, Pos::new(at.x + 3, at.y));
    assert_eq!(screen(h.state()).cursor, at);
    assert!(screen(h.state()).asking());
    h.get_by_label("Stay").click();
    h.run_steps(3);
    assert!(!screen(h.state()).asking());
}

/// `?` shows the key help over the map until a key closes it.
#[test]
fn the_key_help_is_a_box_over_the_map() {
    let mut h = harness();
    h.get_by_label("Keys").click();
    h.run_steps(3);
    assert!(screen(h.state()).helping());
    assert!(h.query_by_label_contains("EMBARK KEYS").is_some());
    h.get_by_label("Close").click();
    h.run_steps(3);
    assert!(!screen(h.state()).helping());
}
