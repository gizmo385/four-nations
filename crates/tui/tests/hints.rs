//! The hint bar (#651, docs/design/tui.md "Hints"): the status line is always the mode's name,
//! that mode's own keys and a fixed tail, built from the one key table in `fn_ui::keys`.

use fn_sim::Nation;
use fn_tui::app::App;
use fn_tui::color::{ColorDepth, Palette};
use fn_tui::draw::hint_mode;
use fn_tui::frame::{render_app, to_text};
use fn_tui::keys;
use fn_tui::new_session;
use fn_ui::Tool;
use fn_ui::keys::{Mode, hints_for};

/// A script that reaches each mode, and the mode it must reach.
const MODES: &[(&str, Mode)] = &[
    ("", Mode::Select),
    ("<enter>", Mode::Selected),
    ("h", Mode::Tool),
    ("h <enter>", Mode::ToolCorner),
    ("t", Mode::Research),
    ("g", Mode::Endeavor),
    ("n", Mode::Nation),
    ("i", Mode::Abilities),
    ("l", Mode::Log),
    ("?", Mode::Help),
    ("u", Mode::Build),
    ("/", Mode::Tools),
    ("<esc>", Mode::Menu),
    ("<esc> <down*5> <enter>", Mode::MenuNations),
    ("<esc> <down*4> <enter>", Mode::Ask),
    ("<f9>", Mode::Ask),
];

fn status_line(script: &str, width: u16) -> (String, Mode) {
    status_lines(script, [width]).remove(0)
}

/// The status line at each of `widths` after `script`, from one game: making the game is nearly
/// all of a call's cost, and the bar at a width depends only on the mode and the width.
fn status_lines(script: &str, widths: impl IntoIterator<Item = u16>) -> Vec<(String, Mode)> {
    let mut app = App::new(new_session(7, Nation::Earth), (160, 30));
    app.play(&keys::parse(script).unwrap());
    let palette =
        Palette::new(&app.session.defs, app.session.world.nation, ColorDepth::TrueColor, false);
    widths
        .into_iter()
        .map(|width| {
            let text = to_text(&render_app(&mut app, &palette, (width, 30)));
            (text.lines().last().unwrap().to_owned(), hint_mode(&app.session, &app.ui))
        })
        .collect()
}

#[test]
fn every_mode_is_reached_and_says_its_name_its_keys_and_the_tail() {
    for &(script, want) in MODES {
        let (line, mode) = status_line(script, 160);
        assert_eq!(mode, want, "{script:?}");
        let name = fn_ui::keys::mode_name(mode, fn_ui::Tool::Harvest);
        assert!(line.starts_with(&format!(" {name} ")), "{script:?}: {line}");
        let all: Vec<String> = hints_for(mode, Tool::Harvest).iter().map(|h| h.text()).collect();
        assert!(line.contains(&all.join(" · ")), "{script:?}: every item fits at 160:\n{line}");
        assert!(line.trim_end().ends_with("? keys"), "{script:?}: {line}");
    }
}

/// Whatever the width, the items shown are the first few of the mode's, each whole, and the tail
/// stays.
#[test]
fn narrow_bars_drop_whole_items_from_the_end_and_keep_the_tail() {
    for &(script, _) in MODES {
        let widths = (80..=160).step_by(7);
        for (width, (line, mode)) in widths.clone().zip(status_lines(script, widths)) {
            assert!(line.trim_end().ends_with("? keys"), "{script:?} at {width}: {line}");
            let items: Vec<String> =
                hints_for(mode, Tool::Harvest).iter().map(|h| h.text()).collect();
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

/// In Select mode the bar has the one tools key, `/`, and never a run of tool letters (#664).
#[test]
fn select_mode_names_the_one_tools_key_not_the_letters() {
    for script in ["", "<enter>"] {
        for (width, (line, _)) in (80..=170).zip(status_lines(script, 80..=170)) {
            // Not one of the tool letters follows another as a list would.
            assert!(!line.contains("H X"), "{script:?} at {width}:\n{line}");
        }
        let (line, _) = status_line(script, 120);
        assert!(line.contains("/ tools"), "{script:?}:\n{line}");
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
    assert!(usage.contains("Harvest: chop, mine, gather"));
}

/// A tool's toggle is in the bar while that tool is up and in no other tool's (#781).
#[test]
fn a_tool_names_its_own_toggle_and_no_other_tools() {
    for (script, toggle, other) in [
        ("h", "O repeat", "teardown"),
        ("h <enter>", "O repeat", "teardown"),
        ("x", "O teardown", "repeat"),
        ("x <enter>", "O teardown", "repeat"),
    ] {
        let (line, _) = status_line(script, 120);
        assert!(line.contains(toggle), "{script:?}:\n{line}");
        assert!(!line.contains(other), "{script:?}:\n{line}");
    }
    // A tool with no toggle names none, and neither does Select.
    for script in ["", "e", "p", "b <enter>", "c"] {
        let (line, _) = status_line(script, 120);
        assert!(!line.contains("O repeat") && !line.contains("O teardown"), "{script:?}:\n{line}");
    }
}

/// The key help lists each toggle, so it is found without picking the tool first.
#[test]
fn the_key_help_lists_each_tool_toggle() {
    let rows = fn_ui::keys::help_rows();
    for what in ["Standing order (Harvest)", "Also sites, stockpiles (Cancel)"] {
        assert!(rows.iter().any(|r| r.keys == "O" && r.what == what), "{what}");
    }
}

/// The map banner names the tool but not what it does: that is the panel's Tool section (#781).
#[test]
fn the_map_banner_has_the_tool_name_and_not_its_description() {
    let size = (120, 30);
    for script in ["h", "x"] {
        let mut app = App::new(new_session(7, Nation::Earth), size);
        app.play(&keys::parse(script).unwrap());
        let palette =
            Palette::new(&app.session.defs, app.session.world.nation, ColorDepth::TrueColor, false);
        let text = to_text(&render_app(&mut app, &palette, size));
        let banner = text.lines().find(|l| l.starts_with('━')).expect("a banner row");
        assert!(banner.contains("Enter: set a corner"), "{banner}");
        assert!(!banner.contains("chop") && !banner.contains("mark"), "{banner}");
    }
}
