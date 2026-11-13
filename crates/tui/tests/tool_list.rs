//! The tool list in the panel (#664): `/` opens it, Up/Down choose, `Enter` or a tool's own
//! letter picks, `Esc` closes; the letters keep working straight from the map.

use fn_sim::Nation;
use fn_tui::app::App;
use fn_tui::color::{ColorDepth, Palette};
use fn_tui::frame::{render_app, to_text};
use fn_tui::keys;
use fn_tui::new_session;
use fn_ui::Tool;

fn app(script: &str) -> App {
    let mut app = App::new(new_session(7, Nation::Earth), (120, 40));
    app.play(&keys::parse(script).unwrap());
    app
}

fn frame(app: &mut App) -> String {
    let palette =
        Palette::new(&app.session.defs, app.session.world.nation, ColorDepth::TrueColor, false);
    to_text(&render_app(app, &palette, (120, 40)))
}

#[test]
fn the_slash_key_lists_the_tools_in_the_panel_and_marks_the_active_one() {
    let mut a = app("/");
    assert_eq!(a.ui.tool_list, Some(0));
    let text = frame(&mut a);
    assert!(text.contains("DESIGNATE"), "{text}");
    for row in ["H Harvest", "X Remove", "E Clear", "P Stockpile", "B Bend", "U Build", "C Sow"] {
        assert!(text.contains(row), "{row} missing:\n{text}");
    }
    // The Bend row names the nation's ability.
    assert!(text.contains("B Bend       Raise Earth"), "{text}");
    // With a tool active the list opens on its row.
    assert_eq!(app("p /").ui.tool_list, Some(3));
}

#[test]
fn a_tool_with_nothing_to_do_says_why() {
    let mut a = app("/");
    assert!(frame(&mut a).contains("C Sow        no farm plot"));
}

#[test]
fn up_down_choose_and_enter_picks() {
    let a = app("/ <down> <enter>");
    assert_eq!(a.ui.tool, Tool::Remove);
    assert_eq!(a.ui.tool_list, None);
    // Up stops at the top, Down at the bottom, and the map cursor stays where it was.
    let start = app("").ui.cursor;
    let top = app("/ <up> <up>");
    assert_eq!(top.ui.tool_list, Some(0));
    let bottom = app("/ <down*20>");
    assert_eq!(bottom.ui.tool_list, Some(6));
    assert_eq!(top.ui.cursor, start);
    assert_eq!(bottom.ui.cursor, start);
    // `W` and `S` choose too, as in every other list.
    assert_eq!(app("/ s s w").ui.tool_list, Some(1));
}

#[test]
fn a_tools_own_letter_picks_it_from_the_list() {
    for (letter, tool) in [
        ("h", Tool::Harvest),
        ("x", Tool::Remove),
        ("e", Tool::Clear),
        ("p", Tool::Stockpile),
        ("c", Tool::Sow),
    ] {
        let a = app(&format!("/ {letter}"));
        assert_eq!(a.ui.tool, tool, "{letter}");
        assert_eq!(a.ui.tool_list, None, "{letter}");
    }
    // Build opens its picker, as `U` does.
    let a = app("/ u");
    assert_eq!(a.ui.tool_list, None);
    assert!(a.ui.modal.is_some());
    // Bend opens the abilities list, as `B` does (#952): the tool waits for `Enter` there.
    let a = app("/ b");
    assert_eq!(a.ui.tool_list, None);
    assert_eq!(a.ui.screen, Some(fn_tui::state::Screen::Abilities));
    assert_eq!(a.ui.tool, Tool::Select);
}

#[test]
fn the_seven_letters_still_work_straight_from_the_map() {
    for (letter, tool) in [
        ("h", Tool::Harvest),
        ("x", Tool::Remove),
        ("e", Tool::Clear),
        ("p", Tool::Stockpile),
        ("c", Tool::Sow),
    ] {
        let a = app(letter);
        assert_eq!((a.ui.tool, a.ui.tool_list), (tool, None), "{letter}");
    }
    assert!(app("u").ui.modal.is_some());
    assert_eq!(app("b").ui.screen, Some(fn_tui::state::Screen::Abilities));
}

#[test]
fn esc_and_slash_close_the_list_and_change_nothing() {
    for script in ["/ <esc>", "/ /"] {
        let a = app(script);
        assert_eq!(a.ui.tool_list, None, "{script}");
        assert_eq!(a.ui.tool, Tool::Select, "{script}");
        assert!(a.ui.modal.is_none(), "{script}: Esc closes the list, not the game menu");
    }
    // In a tool, a corner already set survives the list being closed.
    let a = app("h <enter> / <esc>");
    assert!(a.ui.anchor.is_some());
    assert_eq!(a.ui.tool, Tool::Harvest);
}

#[test]
fn picking_the_active_tool_leaves_it_as_it_is() {
    // Its letter pressed again would flip Clear to buildings; picking its row doesn't.
    let a = app("e / <enter>");
    assert_eq!(a.ui.tool, Tool::Clear);
    let a = app("e / e");
    assert_eq!(a.ui.tool, Tool::Clear);
    // Another tool's row drops the corner, as switching tools does.
    let a = app("h <enter> / <down> <enter>");
    assert_eq!(a.ui.tool, Tool::Remove);
    assert_eq!(a.ui.anchor, None);
}

#[test]
fn opening_a_screen_closes_the_list_and_the_clock_keeps_working() {
    assert_eq!(app("/ t").ui.tool_list, None);
    assert_eq!(app("/ ?").ui.tool_list, None);
    let a = app("/ <space>");
    assert_eq!(a.ui.tool_list, Some(0));
    assert_ne!(a.session.clock.paused, app("").session.clock.paused);
}

#[test]
fn the_list_has_its_own_hints_and_key_help_row() {
    let a = app("/");
    assert_eq!(fn_tui::draw::hint_mode(&a.session, &a.ui), fn_ui::keys::Mode::Tools);
    assert!(
        fn_ui::keys::help_rows().iter().any(|r| r.keys == "/" && r.what.contains("tools")),
        "the key help names it"
    );
}
