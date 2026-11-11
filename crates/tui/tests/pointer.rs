//! A pointer on the map (`fn_tui::pointer`, #842/#843): a click moves the cursor, and a drag with
//! a tool active issues exactly the command the keyboard rectangle does.

use fn_sim::{Nation, Pos};
use fn_tui::app::App;
use fn_tui::keys;
use fn_tui::new_session;
use fn_tui::pointer::{self, Pointer};
use fn_ui::tools::Tool;

const SIZE: (u16, u16) = (120, 40);

fn game() -> App {
    App::new(new_session(7, Nation::Earth), SIZE)
}

fn play(app: &mut App, script: &str) {
    app.play(&keys::parse(script).expect("a valid script"));
}

#[test]
fn a_click_moves_the_cursor_and_nothing_else() {
    let mut app = game();
    let start = app.ui.cursor;
    let to = Pos::new(start.x + 9, start.y - 4);
    pointer::click(&mut app, to);
    assert_eq!(app.ui.cursor, to);
    assert!(app.session.world.command_log().is_empty());

    // With a tool active a click still only moves the cursor: no corner is set.
    play(&mut app, "h");
    pointer::click(&mut app, start);
    assert_eq!((app.ui.cursor, app.ui.anchor), (start, None));
    assert!(app.session.world.command_log().is_empty());
}

#[test]
fn a_click_off_the_map_stops_at_the_edge() {
    let mut app = game();
    pointer::click(&mut app, Pos::new(-5, 100_000));
    let h = app.session.world.map.height();
    assert_eq!(app.ui.cursor, Pos::new(0, h - 1));
}

#[test]
fn a_drag_with_a_tool_issues_the_same_commands_as_the_keyboard_rectangle() {
    let mut keyboard = game();
    let start = keyboard.ui.cursor;
    play(&mut keyboard, "h <left*6> <up*3> <enter> <right*5> <down*4> <enter>");
    assert!(!keyboard.session.world.command_log().is_empty(), "the rectangle marked something");

    let mut mouse = game();
    play(&mut mouse, "h");
    let mut p = Pointer::new();
    let (a, b) = (Pos::new(start.x - 6, start.y - 3), Pos::new(start.x - 1, start.y + 1));
    p.down(&mut mouse, a);
    p.moved(&mut mouse, Pos::new(a.x + 2, a.y + 1));
    assert!(p.dragging() && mouse.ui.anchor == Some(a), "the live rectangle shows mid-drag");
    p.moved(&mut mouse, b);
    p.up(&mut mouse, b);

    assert_eq!(mouse.session.world.command_log(), keyboard.session.world.command_log());
    assert_eq!((mouse.ui.tool, mouse.ui.anchor, mouse.ui.cursor), (Tool::Harvest, None, b));
    assert_eq!(mouse.ui.notice, keyboard.ui.notice);
}

#[test]
fn a_drag_in_select_mode_only_moves_the_cursor() {
    let mut app = game();
    let start = app.ui.cursor;
    pointer::drag(&mut app, start, Pos::new(start.x + 3, start.y + 3));
    assert_eq!(app.ui.cursor, Pos::new(start.x + 3, start.y + 3));
    assert!(app.session.world.command_log().is_empty());
}

#[test]
fn the_pointer_does_nothing_with_a_screen_open() {
    let mut app = game();
    play(&mut app, "<esc>"); // the game menu
    let before = app.ui.cursor;
    pointer::click(&mut app, Pos::new(before.x + 5, before.y));
    assert_eq!(app.ui.cursor, before);
}
