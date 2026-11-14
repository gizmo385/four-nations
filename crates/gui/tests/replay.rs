//! A run log replayed in the window (#997): the watcher's layout, the native game read only,
//! the code pane, feed and strip as cells, the watcher's own keys, and free look.

use eframe::egui::{self, Event};
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use fn_gui::frame::{PageKind, Surface};
use fn_gui::replay::ReplayApp;
use fn_sim::Nation;
use fn_tui::surface::Host;
use fn_tui::watch::{Pane, WindowLayout};

/// A log of `turns` turns: a Harvest rectangle and a look in the first, then turns ending.
fn log(turns: u32) -> String {
    let mut h = Host::new(7, Nation::Earth, None, None, None, false).unwrap();
    h.press_keys(
        "h <right*3> <down*2> <enter> <right*4> <down*3> <enter>",
        Some("harvest".into()),
        None,
    )
    .unwrap();
    h.look(5, 5, 3, 3);
    for _ in 0..turns {
        h.end_turn(1);
    }
    h.log().to_jsonl(&h.header())
}

fn harness(app: &mut ReplayApp, size: (f32, f32)) -> Harness<'_> {
    let mut h = Harness::builder()
        .with_size(egui::vec2(size.0, size.1))
        .with_context_setup(fn_gui::setup)
        .build_ui(|ui| app.show(ui));
    h.run_steps(2);
    h
}

fn typed(h: &mut Harness<'_>, keys: &str) {
    for c in keys.chars() {
        h.event(Event::Text(c.to_string()));
        h.run_steps(2);
    }
}

/// Big enough for the studio's 160 x 45 cells at the bundled font.
const STUDIO: (f32, f32) = (1800.0, 1000.0);

#[test]
fn a_big_window_shows_the_studio_with_the_native_game() {
    let mut app = ReplayApp::open(&log(3)).unwrap();
    let h = harness(&mut app, STUDIO);
    // The strip, the code pane and the feed are cells, and still readable text.
    assert!(h.query_by_label_contains("AGENT earth").is_some(), "the strip");
    assert!(h.query_by_label_contains("no scripts replayed yet").is_some(), "the code pane");
    drop(h);
    assert!(matches!(app.layout(), Some(WindowLayout::Studio { .. })), "{:?}", app.layout());
    assert_eq!(app.frame().surface, Some(Surface::Native));
}

#[test]
fn a_small_window_shows_one_pane_and_the_digits_switch_it() {
    let mut app = ReplayApp::open(&log(3)).unwrap();
    let mut h = harness(&mut app, (900.0, 600.0));
    drop(h);
    assert_eq!(app.layout(), Some(WindowLayout::Pane(Pane::Game)));
    assert_eq!(app.frame().surface, Some(Surface::Native));
    h = harness(&mut app, (900.0, 600.0));
    typed(&mut h, "3");
    drop(h);
    assert_eq!(app.layout(), Some(WindowLayout::Pane(Pane::Feed)));
}

#[test]
fn the_watchers_keys_step_the_replay_and_others_are_free_look() {
    let mut app = ReplayApp::open(&log(4)).unwrap();
    let mut h = harness(&mut app, STUDIO);
    // Pause first, so frame time plays nothing; `]` then ends one turn at a time.
    typed(&mut h, " ]]");
    drop(h);
    assert!(app.replay().paused());
    assert_eq!(app.replay().turn(), 2);
    // Any other key is free look's (#330), as in the terminal: the arrow moves the watcher's
    // own cursor, and `v` goes back to the agent's view.
    let mut h = harness(&mut app, STUDIO);
    h.key_press(egui::Key::ArrowLeft);
    h.run_steps(2);
    drop(h);
    assert!(!app.replay().watcher().following());
    let mut h = harness(&mut app, STUDIO);
    typed(&mut h, "v");
    drop(h);
    assert!(app.replay().watcher().following());
    // `[` goes back through the snapshots.
    let mut h = harness(&mut app, STUDIO);
    typed(&mut h, "[");
    for _ in 0..20 {
        h.run_steps(1);
    }
    drop(h);
    assert_eq!(app.replay().turn(), 1);
}

#[test]
fn the_agents_look_is_outlined_on_the_map() {
    use fn_tui::keyev::{KeyCode, KeyEvent, KeyModifiers};
    let press =
        |app: &mut ReplayApp, c| app.key(KeyEvent::new(KeyCode::Char(c), KeyModifiers::NONE));
    let mut app = ReplayApp::open(&log(1)).unwrap();
    // Paused, then step through the first turn's keys up to the look.
    press(&mut app, ' ');
    for _ in 0..60 {
        if app.replay().watcher().attention().is_some() {
            break;
        }
        press(&mut app, '.');
    }
    let attention = app.replay().watcher().attention().expect("the look's ghost");
    let h = harness(&mut app, STUDIO);
    drop(h);
    let outline = app.frame().outlines.first().expect("an outline on the map");
    assert_eq!((outline.at, outline.w, outline.h), (attention.at, 3, 3));
}

/// A log of `turns` turns of an Earth settlement with a research target: its first tech finishes
/// partway through, which the timeline marks.
fn research_log(turns: u32) -> String {
    let mut h = Host::new(7, Nation::Earth, None, None, None, false).unwrap();
    h.press_keys("t <enter> <esc>", Some("research".into()), None).unwrap();
    for _ in 0..turns {
        h.end_turn(1);
    }
    h.log().to_jsonl(&h.header())
}

#[test]
fn the_timeline_scans_in_slices_and_hides_a_kind() {
    use fn_gui::timeline::Timeline;
    use fn_tui::replay::scan::MarkerKind;
    let mut timeline = Timeline::new(&research_log(40));
    assert!(!timeline.scanned());
    // A zero budget is still one turn a call, so the scan always moves on.
    assert!(timeline.scan_for(0.0));
    while timeline.scan_for(0.01) {}
    assert!(timeline.scanned());
    let learned: Vec<u32> = timeline
        .ticks()
        .iter()
        .filter(|(_, k)| *k == MarkerKind::Learned)
        .map(|(t, _)| *t)
        .collect();
    assert!(!learned.is_empty(), "{:?}", timeline.markers());
    assert!(!timeline.events_at(learned[0]).is_empty());
    timeline.toggle(MarkerKind::Learned);
    assert!(timeline.ticks().iter().all(|(_, k)| *k != MarkerKind::Learned));
    assert!(timeline.events_at(learned[0]).iter().all(|m| m.kind != MarkerKind::Learned));
}

#[test]
fn the_buttons_are_the_watchers_keys_and_the_bar_seeks() {
    let mut app = ReplayApp::open(&log(10)).unwrap();
    let mut h = harness(&mut app, STUDIO);
    h.get_by_label("Pause replay").click();
    h.run_steps(2);
    h.get_by_label("Next turn").click();
    h.run_steps(2);
    h.get_by_label("Next turn").click();
    h.run_steps(2);
    drop(h);
    assert!(app.replay().paused());
    assert_eq!(app.replay().turn(), 2);
    // A click in the middle of the bar goes to the middle turn.
    let mut h = harness(&mut app, STUDIO);
    h.get_by_label_contains("timeline").click();
    for _ in 0..30 {
        h.run_steps(1);
    }
    drop(h);
    assert_eq!(app.replay().turn(), 5);
    assert!(!app.replay().seeking());
}

#[test]
fn a_gzipped_log_opens_like_the_plain_one() {
    use std::io::Write;
    let text = log(2);
    // A stored-block gzip member by hand: miniz_oxide's deflate plus RFC 1952's header.
    let body = miniz_oxide::deflate::compress_to_vec(text.as_bytes(), 6);
    let mut gz = vec![0x1f, 0x8b, 8, 8, 0, 0, 0, 0, 0, 255];
    gz.write_all(b"run.jsonl\0").unwrap();
    gz.extend_from_slice(&body);
    gz.extend_from_slice(&[0; 8]);
    assert_eq!(fn_gui::replay::log_text(&gz).unwrap(), text);
    assert_eq!(fn_gui::replay::log_text(text.as_bytes()).unwrap(), text);
    assert!(fn_gui::replay::log_text(&[0x1f, 0x8b, 8, 0]).is_err());
}

#[test]
fn the_page_opens_the_log_it_is_handed_and_says_why_it_cannot() {
    use fn_gui::replay::{ReplayPage, arrived};
    let mut page = ReplayPage::default();
    let mut h = Harness::builder()
        .with_size(egui::vec2(STUDIO.0, STUDIO.1))
        .with_context_setup(fn_gui::setup)
        .build_ui(|ui| page.show(ui));
    h.run_steps(2);
    arrived("bad.jsonl".into(), Ok(b"not a log".to_vec()));
    h.run_steps(2);
    drop(h);
    assert!(page.app().is_none());
    assert!(page.status().is_some_and(|s| s.contains("bad.jsonl")), "{:?}", page.status());

    let mut h = Harness::builder()
        .with_size(egui::vec2(STUDIO.0, STUDIO.1))
        .with_context_setup(fn_gui::setup)
        .build_ui(|ui| page.show(ui));
    arrived("run.jsonl".into(), Ok(log(2).into_bytes()));
    h.run_steps(3);
    drop(h);
    assert_eq!(page.app().map(|a| a.replay().total_turns()), Some(2));
    assert_eq!(page.status(), None);
}

/// The agent's own screen, whatever the window shows.
fn agents_screen(app: &ReplayApp) -> Option<fn_ui::state::Screen> {
    match &app.replay().watcher().shell().mode {
        fn_tui::shell::Mode::Game(game) => game.ui.screen,
        _ => None,
    }
}

#[test]
fn a_screen_opens_in_free_look_and_never_in_the_agents_view() {
    let mut app = ReplayApp::open(&log(3)).unwrap();
    let mut h = harness(&mut app, STUDIO);
    // `g` is the endeavor screen's key: free look's, so the watcher's own view shows it.
    typed(&mut h, " g");
    drop(h);
    assert!(!app.replay().watcher().following());
    assert_eq!(app.frame().surface, Some(Surface::Page(PageKind::Endeavor)));
    assert_eq!(agents_screen(&app), None);
    // `Esc` (the page's close button) closes it; "Follow agent" goes back to the agent's view.
    let mut h = harness(&mut app, STUDIO);
    h.key_press(egui::Key::Escape);
    h.run_steps(2);
    h.get_by_label("Follow agent").click();
    h.run_steps(2);
    drop(h);
    assert_eq!(app.frame().surface, Some(Surface::Native));
    assert!(app.replay().watcher().following());
}

#[test]
fn closing_a_notice_hides_it_here_and_leaves_the_agents() {
    let mut app = ReplayApp::open(&research_log(40)).unwrap();
    let notices = |app: &ReplayApp| match &app.replay().watcher().shell().mode {
        fn_tui::shell::Mode::Game(game) => game.ui.alerts.notices().shown().count(),
        _ => 0,
    };
    app.replay_mut().seek(40);
    while app.replay_mut().seek_step() {}
    assert!(notices(&app) > 0, "the research notice is up");
    let before = notices(&app);
    let mut h = harness(&mut app, STUDIO);
    // Paused, so the replay stands still while the notice is closed.
    typed(&mut h, " ");
    let shown = h.query_all_by_label("✕").count();
    assert!(shown > 0, "a close button");
    h.get_all_by_label("✕").next().unwrap().click();
    h.run_steps(2);
    assert_eq!(h.query_all_by_label("✕").count(), shown - 1);
    drop(h);
    assert_eq!(notices(&app), before, "the agent's notice stays up");
}
