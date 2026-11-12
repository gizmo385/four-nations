//! A pointer on the map (`fn_tui::pointer`, #842/#843): a click moves the cursor, and a drag with
//! a tool active issues exactly the command the keyboard rectangle does; a click is the keys
//! `Enter` or `Esc` (#878): each path's command log is the keyboard path's.

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

/// The first settler's tile.
fn settler(app: &App) -> Pos {
    app.session.world.units.iter().find(|u| u.inside().is_none()).expect("a settler").pos
}

#[test]
fn a_click_in_select_mode_selects_the_unit_there_as_enter_does() {
    let mut keyboard = game();
    let mut mouse = game();
    let at = settler(&mouse);
    // The cursor starts on the settler: move both off it, so the click has to point.
    let off = Pos::new(at.x + 6, at.y);
    keyboard.ui.point_at(off);
    mouse.ui.point_at(off);
    keyboard.ui.point_at(at);
    play(&mut keyboard, "<enter>");
    pointer::click(&mut mouse, at);
    assert!(mouse.ui.selected.is_some(), "the settler is selected");
    assert_eq!(mouse.ui.selected, keyboard.ui.selected);
    assert_eq!(mouse.ui.cursor, at);
    assert_eq!(mouse.session.world.command_log(), keyboard.session.world.command_log());
    assert!(mouse.session.world.command_log().is_empty(), "selecting is no command");
}

#[test]
fn a_click_on_bare_ground_deselects_as_esc_does_and_never_opens_the_menu() {
    let mut app = game();
    let at = settler(&app);
    pointer::click(&mut app, at);
    assert!(app.ui.selected.is_some());
    let bare = Pos::new(at.x + 12, at.y + 9);
    assert!(app.session.world.units_at(bare).next().is_none());
    pointer::click(&mut app, bare);
    assert_eq!((app.ui.selected, app.ui.cursor), (None, bare));
    // With nothing selected a second click on the ground does nothing: no menu.
    pointer::click(&mut app, Pos::new(bare.x + 1, bare.y));
    assert!(app.ui.modal.is_none(), "Esc here would have opened the game menu");
}

#[test]
fn a_click_with_a_tool_is_a_one_tile_apply_and_the_keyboard_s_two_enters() {
    let mut keyboard = game();
    let target = {
        let w = &keyboard.session.world;
        let (w0, h0) = (w.map.width(), w.map.height());
        let c = keyboard.ui.cursor;
        (0..h0)
            .flat_map(|y| (0..w0).map(move |x| Pos::new(x, y)))
            .filter(|&p| w.map.get(p).is_some_and(|t| t.resource.is_some()))
            .min_by_key(|p| (p.x - c.x).abs() + (p.y - c.y).abs())
            .expect("a resource")
    };
    play(&mut keyboard, "h");
    keyboard.ui.point_at(target);
    play(&mut keyboard, "<enter> <enter>");
    assert!(!keyboard.session.world.command_log().is_empty(), "the tile was marked");

    let mut mouse = game();
    play(&mut mouse, "h");
    pointer::click(&mut mouse, target);
    assert_eq!(mouse.session.world.command_log(), keyboard.session.world.command_log());
    assert_eq!((mouse.ui.tool, mouse.ui.anchor, mouse.ui.cursor), (Tool::Harvest, None, target));
    assert_eq!(mouse.ui.notice, keyboard.ui.notice);
}

#[test]
fn a_click_with_a_corner_set_is_the_second_corner() {
    let mut keyboard = game();
    let start = keyboard.ui.cursor;
    play(&mut keyboard, "h <enter> <right*5> <down*3> <enter>");
    let mut mouse = game();
    play(&mut mouse, "h <enter>");
    pointer::click(&mut mouse, Pos::new(start.x + 5, start.y + 3));
    assert_eq!(mouse.session.world.command_log(), keyboard.session.world.command_log());
    assert_eq!(mouse.ui.anchor, None);
}

#[test]
fn a_right_click_is_esc_with_a_tool_and_nothing_in_select() {
    let mut keyboard = game();
    let mut mouse = game();
    // A tool with a corner set: drop the corner, then leave the tool.
    play(&mut keyboard, "h <enter> <esc>");
    play(&mut mouse, "h <enter>");
    pointer::cancel(&mut mouse);
    assert_eq!((mouse.ui.tool, mouse.ui.anchor), (keyboard.ui.tool, keyboard.ui.anchor));
    assert_eq!(mouse.ui.tool, Tool::Harvest, "the first Esc dropped only the corner");
    play(&mut keyboard, "<esc>");
    pointer::cancel(&mut mouse);
    assert_eq!(mouse.ui.tool, Tool::Select);
    assert_eq!(mouse.ui.tool, keyboard.ui.tool);
    // In Select it does nothing (Esc would open the menu).
    pointer::cancel(&mut mouse);
    assert!(mouse.ui.modal.is_none());
    assert!(mouse.session.world.command_log().is_empty());
}

#[test]
fn a_press_with_the_tool_list_open_only_closes_it() {
    let mut app = game();
    play(&mut app, "/");
    assert!(app.ui.tool_list.is_some());
    let at = Pos::new(app.ui.cursor.x + 3, app.ui.cursor.y);
    pointer::click(&mut app, at);
    assert!(app.ui.tool_list.is_none());
    assert_eq!((app.ui.tool, app.ui.anchor), (Tool::Select, None));
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
