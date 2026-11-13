//! The abilities screen (`B` or `I`, #719, #952): every bending ability on a full screen in the shared frame,
//! what each does and whether it can be used now, and `Enter` to pick one for the Bend tool.
//! The list's drawing is unit-tested in `crates/tui/src/abilities.rs`; this drives it through
//! the keys.

use fn_sim::Nation;
use fn_sim::world::Command;
use fn_tui::app::App;
use fn_tui::color::{ColorDepth, Palette};
use fn_tui::frame::{render_app, to_text};
use fn_tui::state::Screen;
use fn_tui::{keys, new_session};
use fn_ui::Tool;
use fn_ui::tools::bend_ability;

const SIZE: (u16, u16) = (120, 40);

fn run(app: &mut App, script: &str) {
    app.play(&keys::parse(script).expect("a valid script"));
}

fn text(app: &mut App, size: (u16, u16)) -> String {
    let p = Palette::new(&app.session.defs, app.session.world.nation, ColorDepth::TrueColor, false);
    to_text(&render_app(app, &p, size))
}

/// A Fire settlement that has researched Dancing Dragon: Hearthflame, Inner Fire and Weld.
fn fire_with_forms() -> App {
    let mut a = App::new(new_session(3, Nation::Fire), SIZE);
    let dragon = a.session.defs.tech_id("dancing_dragon").unwrap();
    a.session.apply(Command::SetResearch { tech: dragon }).unwrap();
    for _ in 0..200_000 {
        if a.session.world.research().is_known(dragon) {
            break;
        }
        a.session.tick();
    }
    assert!(a.session.world.research().is_known(dragon), "researched");
    a
}

#[test]
fn i_opens_the_screen_and_i_b_or_esc_close_it_over_the_side_panel() {
    let mut a = App::new(new_session(7, Nation::Earth), SIZE);
    run(&mut a, "i");
    assert_eq!(a.ui.screen, Some(Screen::Abilities));
    let frame = text(&mut a, SIZE);
    assert!(frame.contains("ABILITIES"), "{frame}");
    assert!(frame.contains("Raise Earth"), "{frame}");
    assert!(frame.contains("Raise the riverbed into stepping stones."), "{frame}");
    // A locked ability is in the list, naming the tech it needs.
    assert!(frame.contains("Needs "), "{frame}");
    // A full screen covers the side panel (#652); the status line says what it is.
    assert!(!frame.contains("SETTLEMENT"), "{frame}");
    let status = frame.lines().last().unwrap();
    assert!(status.contains("ABILITIES"), "{status}");
    assert!(status.contains("Esc, B or I close"), "{status}");
    assert!(status.contains("1-9 row"), "{status}");

    run(&mut a, "i");
    assert_eq!(a.ui.screen, None);
    run(&mut a, "i <esc>");
    assert_eq!(a.ui.screen, None);
    run(&mut a, "i b");
    assert_eq!(a.ui.screen, None, "B closes it too");
    assert_eq!(a.ui.tool, Tool::Select, "and leaves the tool as it was");
    run(&mut a, "I");
    assert_eq!(a.ui.screen, Some(Screen::Abilities));
}

/// The key is a hint-bar item only: the map's status line names it, the key help does not.
#[test]
fn the_key_is_in_the_hint_bar_and_not_in_the_key_help() {
    let mut a = App::new(new_session(7, Nation::Earth), (160, 40));
    let frame = text(&mut a, (160, 40));
    assert!(frame.lines().last().unwrap().contains("I abilities"), "{frame}");
    // With the Bend tool up, and not with another.
    run(&mut a, "b <enter>");
    assert_eq!(a.ui.tool, Tool::Bend);
    assert!(text(&mut a, (160, 40)).lines().last().unwrap().contains("I abilities"));
    run(&mut a, "h");
    assert!(!text(&mut a, (160, 40)).lines().last().unwrap().contains("I abilities"));
    assert!(fn_ui::keys::help_rows().iter().all(|r| r.keys != "I"), "no key help row");
}

#[test]
fn the_map_keys_wait_while_it_is_open() {
    let mut a = App::new(new_session(7, Nation::Earth), SIZE);
    let (cursor, view) = (a.ui.cursor, a.ui.view);
    run(&mut a, "i h x p u <pgdn> <tab> f");
    assert_eq!(a.ui.cursor, cursor);
    assert_eq!(a.ui.view, view);
    assert_eq!(a.ui.tool, Tool::Select, "no tool was picked");
    assert_eq!(a.ui.screen, Some(Screen::Abilities));
    run(&mut a, "<esc> <right>");
    assert_ne!(a.ui.cursor, cursor);
}

/// `Enter` on the ability the pointer opens on picks the Bend tool with it and closes the screen.
#[test]
fn enter_picks_the_highlighted_ability_for_the_bend_tool() {
    let mut a = fire_with_forms();
    let ids: Vec<_> = fn_ui::bend::abilities(&a.session).iter().map(|r| r.id).collect();
    let (hearth, inner) = (ids[0], ids[1]);
    assert_eq!(bend_ability(&a.session), Some(hearth));

    run(&mut a, "i");
    assert_eq!(a.ui.ability_cursor, 0, "opens on what the tool channels now");
    run(&mut a, "<down>");
    assert_eq!(a.ui.ability_cursor, 1);
    run(&mut a, "<enter>");
    assert_eq!(a.ui.screen, None, "picking closes it");
    assert_eq!(a.ui.tool, Tool::Bend);
    assert_eq!(bend_ability(&a.session), Some(inner));
    assert!(text(&mut a, SIZE).contains("BEND Inner Fire"));

    // Reopened, it opens on the one now in use; Esc picks and changes nothing.
    run(&mut a, "i");
    assert_eq!(a.ui.ability_cursor, 1);
    run(&mut a, "<up> <esc>");
    assert_eq!(bend_ability(&a.session), Some(inner));
    assert_eq!(a.ui.tool, Tool::Bend);

    // B opens the same screen from the map (#952) on the ability last channelled, so B Enter
    // re-arms it; pressing B again never moves on to the next one.
    run(&mut a, "h b");
    assert_eq!(a.ui.screen, Some(Screen::Abilities));
    assert_eq!(a.ui.tool, Tool::Harvest, "opening it leaves the tool as it was");
    assert_eq!(a.ui.ability_cursor, 1);
    run(&mut a, "<enter>");
    assert_eq!((a.ui.screen, a.ui.tool), (None, Tool::Bend));
    assert_eq!(bend_ability(&a.session), Some(inner));
    run(&mut a, "b <esc> b <enter>");
    assert_eq!(bend_ability(&a.session), Some(inner));
    assert_eq!(a.ui.tool, Tool::Bend);
}

/// `1` to `9` put the pointer on that row, locked ones included; a row past the list's end does
/// nothing, and `Enter` still decides.
#[test]
fn number_keys_put_the_pointer_on_a_row() {
    let mut a = fire_with_forms();
    let rows = fn_ui::bend::abilities(&a.session);
    assert!(rows.len() >= 3 && rows.len() < 9, "{}", rows.len());
    run(&mut a, "b 3");
    assert_eq!(a.ui.ability_cursor, 2);
    run(&mut a, "1");
    assert_eq!(a.ui.ability_cursor, 0);
    run(&mut a, "9");
    assert_eq!(a.ui.ability_cursor, 0, "past the end of the list");
    run(&mut a, "2 <enter>");
    assert_eq!((a.ui.screen, a.ui.tool), (None, Tool::Bend));
    assert_eq!(bend_ability(&a.session), Some(rows[1].id));
}

/// `B` then `Enter` on a locked row does nothing: the screen stays and the strip says why.
#[test]
fn b_then_enter_on_a_locked_row_picks_nothing() {
    let mut a = App::new(new_session(7, Nation::Fire), SIZE);
    let rows = fn_ui::bend::abilities(&a.session);
    let first_locked = rows.iter().position(|r| !r.usable()).unwrap();
    run(&mut a, &format!("b {} <enter>", first_locked + 1));
    assert_eq!(a.ui.screen, Some(Screen::Abilities));
    assert_eq!(a.ui.tool, Tool::Select);
    let why = rows[first_locked].unusable.clone().unwrap();
    assert!(text(&mut a, SIZE).contains(&format!("{why}: Enter picks nothing until then.")));
}

/// `Enter` on an ability that can't be used yet does nothing: the screen stays and the strip
/// says what it needs.
#[test]
fn enter_on_a_locked_ability_does_nothing_and_the_strip_says_why() {
    let mut a = App::new(new_session(7, Nation::Fire), SIZE);
    let rows = fn_ui::bend::abilities(&a.session);
    let first_locked = rows.iter().position(|r| !r.usable()).unwrap();
    run(&mut a, &format!("i <down*{first_locked}> <enter>"));
    assert_eq!(a.ui.screen, Some(Screen::Abilities));
    assert_eq!(a.ui.tool, Tool::Select);
    let why = rows[first_locked].unusable.clone().unwrap();
    let frame = text(&mut a, SIZE);
    assert!(frame.contains(&format!("{why}: Enter picks nothing until then.")), "{frame}");
    // Down stops at the list's last row.
    run(&mut a, "<down*40>");
    assert_eq!(a.ui.ability_cursor, rows.len() - 1);
}

/// At the smallest terminal every nation's whole list fits with room to spare, so the bottom
/// edge says nothing; the scrolling list and its position are unit-tested in
/// `crates/tui/src/abilities.rs` on a page shorter than the terminal allows.
#[test]
fn every_nations_list_fits_the_smallest_terminal_whole() {
    let size = (80, 24);
    for nation in Nation::ALL {
        let mut a = App::new(new_session(7, nation), size);
        run(&mut a, "i");
        let frame = text(&mut a, size);
        for row in fn_ui::bend::abilities(&a.session) {
            assert!(frame.contains(&row.name), "{nation:?}: {} in\n{frame}", row.name);
        }
        let bottom = frame.lines().nth(size.1 as usize - 2).unwrap();
        assert!(!bottom.contains(" of "), "{nation:?}: {bottom}");
    }
}
