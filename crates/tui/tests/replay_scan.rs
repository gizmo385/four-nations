//! The timeline's scan (crates/tui/src/replay/scan.rs) marks each event at the turn whose end first
//! shows it, the turn a seek to the marker lands on.

use fn_sim::Nation;
use fn_tui::replay::scan::{MarkerKind, Scan};
use fn_tui::surface::Host;
use fn_tui::watch::Watcher;

/// A log of `turns` turns of an Earth settlement with a research target and nothing else: its
/// first tech finishes partway through.
fn log(turns: u32) -> String {
    let mut h = Host::new(7, Nation::Earth, None, None, None, false).unwrap();
    h.press_keys("t <enter> <esc>", Some("research".into()), None).unwrap();
    for _ in 0..turns {
        h.end_turn(1);
    }
    h.log().to_jsonl(&h.header())
}

/// How many techs a watcher of `text` has learned by the end of `turn`.
fn techs_at(text: &str, turn: u32) -> usize {
    let mut w = Watcher::from_text(text).unwrap();
    w.fast_forward_to_turn(turn);
    w.session().world.tech_completions().count()
}

#[test]
fn a_learned_tech_is_marked_at_the_first_turn_that_shows_it() {
    let text = log(30);
    let mut scan = Scan::new(&text).unwrap();
    let mut steps = 0;
    while scan.step() {
        steps += 1;
    }
    assert!(scan.done());
    assert_eq!((scan.turn(), steps), (30, 29));

    let learned: Vec<_> = scan.markers().iter().filter(|m| m.kind == MarkerKind::Learned).collect();
    let [tech] = learned.as_slice() else { panic!("{:?}", scan.markers()) };
    assert!(tech.text.starts_with("Learned "), "{}", tech.text);
    assert_eq!(techs_at(&text, tech.turn - 1), 0);
    assert_eq!(techs_at(&text, tech.turn), 1);
}

#[test]
fn a_log_the_build_cannot_replay_is_refused() {
    let text = log(1).replacen("\"data_digest\":", "\"data_digest\":1", 1);
    assert!(Scan::new(&text).is_err());
}
