//! A run log replayed in the window (#997): the watcher's layout, the native game read only,
//! the code pane, feed and strip as cells, and only the watcher's own keys.

use eframe::egui::{self, Event};
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use fn_gui::frame::Surface;
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
fn the_watchers_keys_step_the_replay_and_others_do_nothing() {
    let mut app = ReplayApp::open(&log(4)).unwrap();
    let mut h = harness(&mut app, STUDIO);
    // Pause first, so frame time plays nothing; `]` then ends one turn at a time.
    typed(&mut h, " ]]");
    drop(h);
    assert!(app.replay().paused());
    assert_eq!(app.replay().turn(), 2);
    // A free-look key in the terminal: here the map is the free look, so it is ignored.
    let mut h = harness(&mut app, STUDIO);
    h.key_press(egui::Key::ArrowLeft);
    h.run_steps(2);
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
