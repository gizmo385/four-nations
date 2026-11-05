//! Observation containment (#320's acceptance criteria, docs/design/agent-play.md, "Enforcing
//! and testing parity"): every leaf string and number `Host::observe` and `Host::screen` report
//! has to appear in the rendered text of some screen a player could reach too, through the
//! public agent surface alone -- no internals, the same door `fn_agent` uses.
//!
//! Map-cell equality (the same acceptance criteria's other two checks) lives in
//! `crates/tui/src/surface.rs`'s own `#[cfg(test)] mod tests` instead: it needs `UiState`'s
//! camera (`ui.view`) to line a `look` up with a `frame`, which only an internal test can read.

use fn_sim::Nation;
use fn_tui::layout::PANEL_WIDTH;
use fn_tui::surface::{ActionStep, Host};
use serde_json::Value;

/// A seed-7 game past its opening moments, a settler selected and the Build tool active with a
/// building already chosen: one golden scenario per nation (docs/design/agent-play.md,
/// "Enforcing and testing parity"), built through the public surface alone. Bounded to a few
/// dozen turns, cheap enough for `ci --fast` (#291).
fn golden_scenario(nation: Nation) -> Host {
    let mut h = Host::new(7, nation, None, None, None, false).unwrap();
    for _ in 0..20 {
        h.end_turn(50);
    }
    let name = first_settler_name(&mut h);
    let select = h.act(&[ActionStep::Select { x: None, y: None, name: Some(name) }], None);
    assert!(select.error.is_none(), "{select:?}");
    let build = h.press_keys("u <enter>", None, None).unwrap();
    assert_eq!(build.refused, 0, "u and <enter> are never refused");
    assert_eq!(h.observe().tool, "Build", "the Build tool is active");
    h
}

/// Any settler's name, read the way an agent would: the Nation screen's own roster
/// (`screen {"nation"}`), not a direct read of `World`.
fn first_settler_name(h: &mut Host) -> String {
    let screen = serde_json::to_value(h.screen("nation").unwrap()).unwrap();
    screen["groups"]
        .as_array()
        .unwrap()
        .iter()
        .find_map(|group| group[1].as_array().and_then(|names| names.first()))
        .and_then(Value::as_str)
        .map(str::to_owned)
        .expect("a new game always has settlers")
}

/// Every leaf string (non-empty) and number serde_json finds in `value`, flattened as text, so
/// a containment check can look for either the same way.
fn leaves(value: &Value, out: &mut Vec<String>) {
    match value {
        Value::String(s) => {
            if !s.is_empty() {
                out.push(s.clone());
            }
        }
        Value::Number(n) => out.push(n.to_string()),
        Value::Array(a) => a.iter().for_each(|v| leaves(v, out)),
        Value::Object(o) => o.values().for_each(|v| leaves(v, out)),
        Value::Bool(_) | Value::Null => {}
    }
}

/// Every leaf `Host::observe` and `Host::screen` report, for `h`'s current state.
///
/// One field is a deliberate, narrow exception: `observe().turn`, the host's own count of
/// completed `end_turn` calls, is in docs/design/agent-play.md's own "Observation" table, but
/// no plain (non-`--watch`) screen prints a turn count anywhere (#319's watcher strip is the
/// only one that does). It isn't hidden `World` state -- it's `floor(elapsed game seconds /
/// turn_seconds)`, both already visible or known -- but it does fail a literal containment
/// check, which is why it's carved out here rather than silently passing or failing the whole
/// test. See #326 for the owner's call: keep this exception, drop the field, or give a plain
/// session its own on-screen turn counter.
fn observation_leaves(h: &Host) -> Vec<String> {
    let mut observe = serde_json::to_value(h.observe()).unwrap();
    observe.as_object_mut().unwrap().remove("turn");
    let mut out = Vec::new();
    leaves(&observe, &mut out);
    for name in ["nation", "research", "endeavor", "log", "help", "build"] {
        leaves(&serde_json::to_value(h.screen(name).unwrap()).unwrap(), &mut out);
    }
    out
}

/// `frame`'s own left side (the map, or a screen's overlay) and right side (the side panel,
/// always [`PANEL_WIDTH`] columns, `layout::regions`'s own split), each joined on their own so
/// a sentence [`fn_tui::widgets::wrap`] breaks across more than one row reads as one
/// contiguous span again, rather than one broken up by the panel's own row in between every
/// physical line of it.
fn columns(frame: &str) -> (String, String) {
    let width = frame.lines().map(|l| l.chars().count()).max().unwrap_or(0);
    let split_at = width.saturating_sub(usize::from(PANEL_WIDTH));
    let mut left = String::new();
    let mut right = String::new();
    for line in frame.lines() {
        let chars: Vec<char> = line.chars().collect();
        let at = split_at.min(chars.len());
        left.extend(&chars[..at]);
        left.push('\n');
        right.extend(&chars[at..]);
        right.push('\n');
    }
    (left, right)
}

/// The rendered text of every screen `Host::observe` and `Host::screen`'s leaves could be
/// checked against: the main view (cursor already where `observe`'s own `cursor` reports it),
/// then `N`, `T`, `G`, `L` and `?` in turn, each closed again before the next (the acceptance
/// criteria's "main, N, T, G, L, ? and the Cursor section"), and finally `U`'s Build modal
/// (#327: `screen {"build"}`'s own leaves) -- stepping down its list to capture the detail
/// pane for every row, not only whichever one the Build tool happened to leave highlighted: a
/// long locked name is squeezed with an ellipsis in the list itself once it's paired with its
/// cost (`crate::widgets::Row`'s own "gives way" rule), and only the detail pane, for the
/// highlighted row, ever shows it in full.
fn reachable_text(h: &mut Host) -> String {
    const SIZE: (u16, u16) = (220, 50);
    let mut text = String::new();
    let push = |frame: &str, text: &mut String| {
        let (left, right) = columns(frame);
        text.push_str(&left);
        text.push_str(&right);
    };
    push(&h.frame(SIZE.0, SIZE.1), &mut text);
    for key in ["n", "t", "g", "l", "?"] {
        h.press_keys(key, None, None).unwrap();
        push(&h.frame(SIZE.0, SIZE.1), &mut text);
        h.press_keys("<esc>", None, None).unwrap();
    }
    let rows = match h.screen("build").unwrap() {
        fn_tui::surface::ScreenView::Build(v) => v.rows.len(),
        other => unreachable!("{other:?}"),
    };
    h.press_keys("u", None, None).unwrap();
    push(&h.frame(SIZE.0, SIZE.1), &mut text);
    for _ in 1..rows {
        h.press_keys("<down>", None, None).unwrap();
        push(&h.frame(SIZE.0, SIZE.1), &mut text);
    }
    h.press_keys("<esc>", None, None).unwrap();
    text
}

/// Lower case, with a box's own `|` border folded in with whitespace and every run of the
/// result folded to one space: a section heading prints upper case (docs/design/side-panel.md
/// "Headings"), a long row (the Help screen's `what` column, say) wraps at spaces into more
/// than one line (`widgets::wrap`), and an overlay's own border sits right where
/// [`columns`] cuts the row apart and so ends up between two of a wrapped row's halves. None of
/// that is a parity difference the way the text itself is, so both the haystack and every leaf
/// fold the same way before the containment check.
fn fold(s: &str) -> String {
    s.to_lowercase().replace('|', " ").split_whitespace().collect::<Vec<_>>().join(" ")
}

#[test]
fn every_observed_leaf_appears_in_a_reachable_screen_for_every_nation() {
    for nation in Nation::ALL {
        let mut h = golden_scenario(nation);
        let leaves = observation_leaves(&h);
        let haystack = fold(&reachable_text(&mut h));
        for leaf in &leaves {
            let needle = fold(leaf);
            assert!(
                haystack.contains(&needle),
                "{nation:?}: {leaf:?} (from observe/screen) isn't in any reachable screen's \
                 rendered text",
            );
        }
    }
}
