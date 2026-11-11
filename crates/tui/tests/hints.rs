//! The hint bar (#651, docs/design/tui.md "Hints"): the status line is always the mode's name,
//! that mode's own keys and a fixed tail, built from the one key table in `fn_ui::keys`.

use fn_sim::Nation;
use fn_tui::app::App;
use fn_tui::color::{ColorDepth, Palette};
use fn_tui::draw::hint_mode;
use fn_tui::frame::{render_app, to_text};
use fn_tui::keys;
use fn_tui::new_session;
use fn_ui::keys::{Mode, hints};

/// A script that reaches each mode, and the mode it must reach.
const MODES: &[(&str, Mode)] = &[
    ("", Mode::Select),
    ("<enter>", Mode::Selected),
    ("h", Mode::Tool),
    ("h <enter>", Mode::ToolCorner),
    ("t", Mode::Research),
    ("g", Mode::Endeavor),
    ("n", Mode::Nation),
    ("l", Mode::Log),
    ("?", Mode::Help),
    ("u", Mode::Build),
    ("<esc>", Mode::Menu),
];

fn status_line(script: &str, width: u16) -> (String, Mode) {
    let size = (width, 30);
    let mut app = App::new(new_session(7, Nation::Earth), size);
    app.play(&keys::parse(script).unwrap());
    let mode = hint_mode(&app.session, &app.ui);
    let palette =
        Palette::new(&app.session.defs, app.session.world.nation, ColorDepth::TrueColor, false);
    let text = to_text(&render_app(&mut app, &palette, size));
    (text.lines().last().unwrap().to_owned(), mode)
}

#[test]
fn every_mode_is_reached_and_says_its_name_its_keys_and_the_tail() {
    for &(script, want) in MODES {
        let (line, mode) = status_line(script, 160);
        assert_eq!(mode, want, "{script:?}");
        let name = fn_ui::keys::mode_name(mode, fn_ui::Tool::Harvest);
        assert!(line.starts_with(&format!(" {name} ")), "{script:?}: {line}");
        let all: Vec<String> = hints(mode).iter().map(|h| h.text()).collect();
        assert!(line.contains(&all.join(" · ")), "{script:?}: every item fits at 160:\n{line}");
        assert!(line.trim_end().ends_with("? keys"), "{script:?}: {line}");
    }
}

/// Whatever the width, the items shown are the first few of the mode's, each whole, and the tail
/// stays.
#[test]
fn narrow_bars_drop_whole_items_from_the_end_and_keep_the_tail() {
    for &(script, _) in MODES {
        for width in (80..=160).step_by(7) {
            let (line, mode) = status_line(script, width);
            assert!(line.trim_end().ends_with("? keys"), "{script:?} at {width}: {line}");
            let items: Vec<String> = hints(mode).iter().map(|h| h.text()).collect();
            let shown = (0..=items.len())
                .rev()
                .find(|&n| n == 0 || line.contains(&items[..n].join(" · ")))
                .unwrap();
            let joined = items[..shown].join(" · ");
            let rest = line.find(&joined).map_or("", |at| &line[at + joined.len()..]);
            // No separator dangles, and the next item is not there in part.
            assert!(!rest.contains(" · "), "{script:?} at {width}: {line}");
            if let Some(next) = items.get(shown) {
                let first_word = next.split(' ').next().unwrap();
                assert!(!rest.contains(first_word), "{script:?} at {width}: {line}");
            }
        }
    }
}

/// In Select mode the bar names every tool key or none of them, at every width.
#[test]
fn select_mode_never_lists_a_subset_of_the_tool_keys() {
    let all = fn_ui::keys::tool_keys();
    for script in ["", "<enter>"] {
        for width in 80..=170 {
            let (line, _) = status_line(script, width);
            if line.contains(" tools") {
                assert!(line.contains(&format!("{all} tools")), "{script:?} at {width}:\n{line}");
            } else {
                // Not one of the tool letters follows another as a list would.
                assert!(!line.contains("H X"), "{script:?} at {width}:\n{line}");
            }
        }
    }
}

#[test]
fn paused_comes_before_the_tail() {
    let (line, _) = status_line("<space>", 120);
    assert!(line.trim_end().ends_with("PAUSED ? keys"), "{line}");
}

/// `--help`'s key paragraph is built from the same table as the key help screen: every row of
/// the help is in it, and the drift the issue named is gone.
#[test]
fn the_help_text_lists_every_key_help_row() {
    let usage = fn_tui::cli::usage();
    for row in fn_ui::keys::help_rows() {
        assert!(usage.contains(&row.keys), "{:?} in --help", row.keys);
        assert!(usage.contains(row.what.split(' ').next().unwrap()), "{:?} in --help", row.what);
    }
    assert!(usage.contains("U: pick one") && !usage.contains("U again cycles"));
}
