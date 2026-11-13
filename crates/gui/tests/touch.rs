//! Touch on the map (#977, `fn_gui::touch`): scripted finger sequences, no window.

use eframe::egui::pos2;
use fn_gui::touch::{Frame, Gestures};

const P: eframe::egui::Pos2 = pos2(100.0, 100.0);

/// A finger held still past the long-press time shows the tooltip there, and its release is
/// swallowed (no click, no one-tile apply).
#[test]
fn a_still_finger_becomes_a_long_press_and_its_release_is_no_click() {
    let mut g = Gestures::default();
    assert_eq!(g.update(true, Some(P), 1, 0.0).long_press, None);
    assert_eq!(g.update(true, Some(pos2(104.0, 103.0)), 1, 0.3).long_press, None, "too soon");
    let held = g.update(true, Some(pos2(104.0, 103.0)), 1, 0.6);
    assert_eq!(held.long_press, Some(pos2(104.0, 103.0)), "within the slop, past the time");
    assert!(g.long_pressing());
    let up = g.update(false, None, 0, 0.8);
    assert!(up.swallow_release && up.long_press.is_none());
    assert!(g.update(false, None, 0, 0.9).swallow_release, "and the frame after, for a late click");
    assert!(!g.update(false, None, 0, 1.0).swallow_release, "then nothing");
}

/// A quick tap is a click: nothing to swallow.
#[test]
fn a_tap_is_left_alone() {
    let mut g = Gestures::default();
    g.update(true, Some(P), 1, 0.0);
    assert_eq!(g.update(false, None, 0, 0.1), Frame::default());
}

/// A finger that moves past the slop is a drag, never a long press, however long it lasts.
#[test]
fn a_drag_is_never_a_long_press() {
    let mut g = Gestures::default();
    g.update(true, Some(P), 1, 0.0);
    g.update(true, Some(pos2(130.0, 100.0)), 1, 0.2);
    let late = g.update(true, Some(pos2(130.0, 100.0)), 1, 2.0);
    assert_eq!(late.long_press, None);
    assert!(!g.update(false, None, 0, 2.1).swallow_release, "a drag's release is the drag's");
}

/// The second finger cancels once, the press stays two-finger until every finger lifts (the one
/// left behind doesn't draw), and the release is swallowed.
#[test]
fn a_second_finger_cancels_once_and_stays_until_all_lift() {
    let mut g = Gestures::default();
    let one = g.update(true, Some(P), 1, 0.0);
    assert!(!one.multi && !one.cancel);
    let two = g.update(true, Some(P), 2, 0.1);
    assert!(two.multi && two.cancel, "the second finger lands");
    let still = g.update(true, Some(P), 2, 0.2);
    assert!(still.multi && !still.cancel, "cancelled once");
    let left = g.update(true, Some(pos2(160.0, 100.0)), 1, 0.3);
    assert!(left.multi, "one finger left: still a two-finger press");
    let late = g.update(true, Some(pos2(160.0, 100.0)), 1, 2.0);
    assert_eq!(late.long_press, None, "never a long press either");
    assert!(g.update(false, None, 0, 2.1).swallow_release);
    assert!(!g.update(true, Some(P), 1, 3.0).multi, "the next press starts fresh");
}

/// A long press that gains a second finger turns into a pinch: the tooltip goes.
#[test]
fn a_pinch_ends_a_long_press() {
    let mut g = Gestures::default();
    g.update(true, Some(P), 1, 0.0);
    assert!(g.update(true, Some(P), 1, 0.6).long_press.is_some());
    let pinch = g.update(true, Some(P), 2, 0.7);
    assert!(pinch.multi && pinch.long_press.is_none());
}
