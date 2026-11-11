//! Observation containment (#320's acceptance criteria, docs/design/agent-play.md, "Enforcing
//! and testing parity"): every leaf string and number `Host::observe` and `Host::screen` report
//! has to appear in the rendered text of some screen a player could reach too, through the
//! public agent surface alone -- no internals, the same door `fn_agent` uses.
//!
//! Map-cell equality (the same acceptance criteria's other two checks) lives in
//! `crates/tui/src/surface.rs`'s own `#[cfg(test)] mod tests` instead: it needs `UiState`'s
//! camera (`ui.view`) to line a `look` up with a `frame`, which only an internal test can read.

use fn_sim::Nation;
use fn_tui::layout::panel_width;
use fn_tui::surface::{ActionStep, Host, Prelude, PreludeOpening, ScreenView};
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
    for name in ["nation", "research", "endeavor", "log", "help", "build", "bend"] {
        leaves(&serde_json::to_value(h.screen(name).unwrap()).unwrap(), &mut out);
    }
    out
}

/// `frame`'s own left side (the map, or a centred box over it) and right side (the side panel,
/// [`panel_width`] columns wide, `layout::regions`'s own split), each joined on their own so
/// a sentence [`fn_tui::widgets::wrap`] breaks across more than one row reads as one
/// contiguous span again, rather than one broken up by the panel's own row in between every
/// physical line of it.
fn columns(frame: &str) -> (String, String) {
    let width = frame.lines().map(|l| l.chars().count()).max().unwrap_or(0);
    let split_at = width.saturating_sub(usize::from(panel_width(width as u16)));
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
/// highlighted row, ever shows it in full. `T`'s own tree gets the same treatment (#338): a
/// card's own row is squeezed the same way, and only the detail pane, for whichever card is
/// under the cursor, ever shows a long "Unlocks:" or "Needs:" line in full.
fn reachable_text(h: &mut Host) -> String {
    const SIZE: (u16, u16) = (220, 50);
    let mut text = String::new();
    let push = |frame: &str, text: &mut String| {
        let (left, right) = columns(frame);
        text.push_str(&left);
        text.push_str(&right);
    };
    // A full screen covers the side panel too (#652): its rows are whole, not split at the
    // panel's column.
    let push_page = |frame: &str, text: &mut String| {
        text.push_str(frame);
        text.push('\n');
    };
    push(&h.frame(SIZE.0, SIZE.1), &mut text);
    for key in ["n", "g", "l", "?"] {
        h.press_keys(key, None, None).unwrap();
        push_page(&h.frame(SIZE.0, SIZE.1), &mut text);
        h.press_keys("<esc>", None, None).unwrap();
    }
    h.press_keys("t", None, None).unwrap();
    let column_lens: Vec<usize> = match h.screen("research").unwrap() {
        fn_tui::surface::ScreenView::Research(v) => v.columns.iter().map(Vec::len).collect(),
        other => unreachable!("{other:?}"),
    };
    // A big enough step to reach either edge from anywhere in the tree, so the raster below
    // starts at a known corner (top-left) whatever card `initial_cursor` happened to pick.
    const FAR: usize = 64;
    h.press_keys(&format!("<up*{FAR}> <left*{FAR}>"), None, None).unwrap();
    for (c, &rows) in column_lens.iter().enumerate() {
        for r in 0..rows {
            push_page(&h.frame(SIZE.0, SIZE.1), &mut text);
            if r + 1 < rows {
                h.press_keys("<down>", None, None).unwrap();
            }
        }
        if c + 1 < column_lens.len() {
            h.press_keys(&format!("<right> <up*{FAR}>"), None, None).unwrap();
        }
    }
    h.press_keys("<esc>", None, None).unwrap();
    let rows = match h.screen("build").unwrap() {
        fn_tui::surface::ScreenView::Build(v) => v.rows.len(),
        other => unreachable!("{other:?}"),
    };
    h.press_keys("u", None, None).unwrap();
    push_page(&h.frame(SIZE.0, SIZE.1), &mut text);
    for _ in 1..rows {
        h.press_keys("<down>", None, None).unwrap();
        push_page(&h.frame(SIZE.0, SIZE.1), &mut text);
    }
    h.press_keys("<esc>", None, None).unwrap();
    // The Bend tool's banner and Tool row (#586: `screen {"bend"}`'s own leaves), once per
    // ability `B` cycles through.
    let abilities = bend_rows(h).len();
    h.press_keys("b", None, None).unwrap();
    push(&h.frame(SIZE.0, SIZE.1), &mut text);
    for _ in 1..abilities {
        h.press_keys("b", None, None).unwrap();
        push(&h.frame(SIZE.0, SIZE.1), &mut text);
    }
    h.press_keys("<esc>", None, None).unwrap();
    text
}

/// `screen {"bend"}`'s rows, as `(name, casters, current)`.
fn bend_rows(h: &Host) -> Vec<(String, String, bool)> {
    match h.screen("bend").unwrap() {
        ScreenView::Bend(v) => {
            v.abilities.into_iter().map(|a| (a.name, a.casters, a.current)).collect()
        }
        other => unreachable!("{other:?}"),
    }
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

/// A settlement whose research has unlocked more than the signature ability: an hour of the
/// Economy opening researching branch-first, so the nation's own branch comes before the shared
/// tier. Every nation has a second ability by then on seed 7 (Air, Earth and Fire by minute 20,
/// Water by 45). The default research order took three hours to get even Earth there.
fn midgame(nation: Nation) -> Host {
    Host::new(7, nation, None, None, None, false)
        .unwrap()
        .with_prelude(Prelude { opening: PreludeOpening::EconomyBranchFirst, minutes: 60.0 })
}

/// #586: `screen {"bend"}` is the Bend tool banner. For every nation, in a new game and at
/// midgame, with any tool active: each press of `B` puts the next listed ability on the banner
/// badge and its count on the Tool row, in the order `screen` lists them; `B: next ability` is
/// on the banner exactly when there is more than one; and nothing the banner never names (any
/// other nation's ability) is listed.
#[test]
fn the_bend_screen_matches_the_banner_for_every_nation() {
    let mut multi = 0;
    for nation in Nation::ALL {
        let hosts = [("new game", golden_scenario(nation)), ("midgame", midgame(nation))];
        for (when, mut h) in hosts {
            let listed = bend_rows(&h);
            assert!(!listed.is_empty(), "{nation:?} {when}");
            // Readable with the Build tool open, or none: the same list.
            h.press_keys("<esc>", None, None).unwrap();
            assert_eq!(bend_rows(&h), listed, "{nation:?} {when}: tool-independent");
            assert_eq!(listed.iter().filter(|r| r.2).count(), 1, "{nation:?} {when}: one current");
            if listed.len() > 1 {
                multi += 1;
            }

            // `b` picks the tool without moving the choice; every `b` after that moves on.
            let start = listed.iter().position(|r| r.2).unwrap();
            for press in 0..listed.len() {
                h.press_keys("b", None, None).unwrap();
                let at = (start + press) % listed.len();
                let (name, casters, _) = &listed[at];
                let frame = fold(&h.frame(220, 50));
                assert!(
                    frame.contains(&format!("bend {}", name.to_lowercase())),
                    "{nation:?} {when}: banner should name {name:?} on press {press}:\n{frame}"
                );
                assert!(frame.contains(&fold(casters)), "{nation:?} {when}: {casters:?}");
                assert_eq!(frame.contains("b: next ability"), listed.len() > 1, "{nation:?}");
                let now = bend_rows(&h);
                assert_eq!(now.iter().position(|r| r.2), Some(at), "{nation:?} {when}");
                let names = |rows: &[(String, String, bool)]| {
                    rows.iter().map(|r| r.0.clone()).collect::<Vec<_>>()
                };
                assert_eq!(names(&now), names(&listed), "{nation:?} {when}: B keeps the list");
            }

            // Nothing the banner can't show: every listed name is this nation's own ability.
            let session = fn_ui::Session::new(7, nation, 48, 32);
            for (name, ..) in &listed {
                let def = session.defs.abilities.iter().find(|a| &a.name == name);
                assert_eq!(def.map(|a| a.nation), Some(nation), "{nation:?} {when}: {name:?}");
            }
        }
    }
    assert_eq!(
        multi,
        Nation::ALL.len(),
        "every nation's midgame settlement should have unlocked a second ability"
    );
}
