//! Free-look spectator input (#330, `crate::watch::Watcher`): the same keys and [`Action`]s the
//! game answers to, applied to a spectator's own [`UiState`] instead of the agent's. [`key`] is
//! the one entry point, called in place of `App::key` while free look is on; it never takes
//! `&mut Session`, only `&Session`, so there is no path from here back into the agent's replay --
//! the hard constraint #330 asks a test to prove (`crate::watch::tests`, the command-log
//! equality test).
//!
//! The logic below mirrors a handful of `App`'s own private methods (`move_tech`,
//! `move_nation_cursor`, `confirm_nation`, `scroll_by`, `open`, `open_build_modal`,
//! `build_modal_key`), kept deliberately separate rather than shared: `App`'s versions mutate
//! `Session` in other branches (`research::choose`, placing a building), and a spectator must
//! never be one signature change away from doing that by accident. Keeping them apart means
//! `App`'s own tests stay the only ones that can break if its behaviour changes, while this
//! module's own tests are the only ones that can break if the spectator's does.

use crate::keyev::{KeyCode, KeyEvent};
use fn_ui::Session;

use crate::input::{self, Action};
use crate::state::{Modal, Screen, UiState};
use crate::{build, msglog, nation, research, screens};

/// Handles one key for the spectator's own `ui`, reading `session` but never changing it.
/// Returns whether it did anything at all, which `Watcher` uses to switch from Follow to Free
/// look automatically (#330's acceptance criteria: "Moving the camera or cursor switches to
/// free look automatically" -- generalised here to every spectator action, movement included,
/// since nothing else would ever make free look's own effect visible).
pub fn key(ui: &mut UiState, session: &Session, key: KeyEvent) -> bool {
    if ui.modal == Some(Modal::Build) {
        return build_modal_key(ui, session, key);
    }
    let Some(action) = input::map_key(key) else { return false };
    if !input::spectator_allowed(action) {
        return false;
    }
    if ui.screen.is_some() {
        return screen_action(ui, session, action);
    }
    match action {
        Action::Move { dx, dy, tiles } => {
            ui.move_cursor(dx * tiles, dy * tiles);
            true
        }
        Action::Jump { dx, dy } => {
            ui.jump(dx, dy);
            true
        }
        Action::NextUnit => {
            ui.cycle(session, 1);
            true
        }
        Action::PrevUnit => {
            ui.cycle(session, -1);
            true
        }
        Action::Follow => {
            ui.toggle_follow(session);
            true
        }
        Action::Confirm => {
            ui.select_here(session);
            true
        }
        // Allowed only for `Tool::Build` (`input::spectator_allowed`): opens the browse-only
        // picker, the same way `App::open_build_modal` does for the game, but `Enter` inside it
        // never places anything (`build_modal_key` below).
        Action::PickTool(_) => {
            open_build_modal(ui, session);
            true
        }
        Action::ShowResearch => open_screen(ui, session, Screen::Research),
        Action::ShowLog => open_screen(ui, session, Screen::Log),
        Action::ShowHelp => open_screen(ui, session, Screen::Help),
        Action::ShowEndeavor => open_screen(ui, session, Screen::Endeavor),
        Action::ShowNation => open_screen(ui, session, Screen::Nation),
        Action::ShowAbilities => open_screen(ui, session, Screen::Abilities),
        Action::ShowTraders => open_screen(ui, session, Screen::Traders),
        // Every other action `input::spectator_allowed` lets through is handled above; nothing
        // else reaches this match with `true`.
        _ => false,
    }
}

/// What a key does while a screen is open, mirroring `App::act_in_screen`'s own split: Research
/// moves a tech cursor and never chooses one (that would set the agent's own research target),
/// Nation moves a roster pointer and `Enter` there only selects a settler to see their bar, and
/// every other screen just scrolls. Re-pressing the key that opened it closes it, the same
/// toggle `open_screen` uses to open one.
fn screen_action(ui: &mut UiState, session: &Session, action: Action) -> bool {
    let screen = ui.screen.expect("called only once a screen is confirmed open");
    // The log's cursor, folds and `Enter` (#661), shared with `App`. Read-only: `Enter` only moves
    // the spectator's own cursor or selects a settler to see their bar.
    if screen == Screen::Log && msglog::act(ui, session, action) {
        return true;
    }
    // The log's filters (#774): the digits are the studio's panes, so `Tab` and `Shift+Tab` step
    // through the kinds. Only the spectator's own filter changes.
    if screen == Screen::Log
        && let Action::NextUnit | Action::PrevUnit = action
    {
        msglog::cycle(ui, session, action == Action::PrevUnit);
        return true;
    }
    match screen {
        Screen::Research => match action {
            Action::Move { dx, dy, .. } => return move_tech(ui, session, dx, dy),
            Action::Jump { dy, .. } => {
                return move_tech(ui, session, 0, dy * tech_page(ui, session));
            }
            // `Enter` here would call `research::choose`, changing the agent's own research
            // target (#330's acceptance criteria: "chooses research" is refused).
            Action::Confirm => return false,
            _ => {}
        },
        Screen::Nation => match action {
            Action::Move { dy, .. } => return move_nation_cursor(ui, session, dy),
            Action::Jump { dy, .. } => {
                return move_nation_cursor(ui, session, dy * nation_page(ui));
            }
            Action::Confirm => return confirm_nation(ui, session),
            _ => {}
        },
        // The abilities screen only browses: `Enter` would pick the agent's own Bend ability
        // (`App::pick_ability`), so it is refused.
        Screen::Abilities => match action {
            Action::Move { dy, .. } => return move_ability_cursor(ui, session, dy),
            Action::Jump { dy, .. } => {
                let page = screens::text_size(ui.page()).1.saturating_sub(1).max(1) as i32;
                return move_ability_cursor(ui, session, dy * page);
            }
            Action::Confirm => return false,
            _ => {}
        },
        // The Traders screen only browses: `Enter` would take a deal in the agent's own game.
        Screen::Traders => match action {
            Action::Move { dy, .. } => {
                let view = fn_ui::trade::view(session);
                ui.trade_cursor = crate::traders::step(&view, ui.trade_cursor, dy);
                return true;
            }
            Action::Confirm => return false,
            _ => {}
        },
        // `input::spectator_allowed` refuses `Action::ShowSaves`, so the spectator never opens
        // the saves screen in the first place.
        Screen::Saves => unreachable!("the saves screen is refused, never opened here"),
        Screen::Log | Screen::Help | Screen::Endeavor => {}
    }
    match action {
        // `Tab` flips the key help between its keys and its legend (#662).
        Action::NextUnit | Action::PrevUnit if screen == Screen::Help => ui.switch_help_tab(),
        Action::Move { dy, tiles, .. } => scroll_by(ui, session, screen, dy * tiles),
        Action::Jump { dy, .. } => {
            let page = screen_rows(ui);
            scroll_by(ui, session, screen, dy * page);
        }
        Action::ShowResearch => return open_screen(ui, session, Screen::Research),
        Action::ShowLog => return open_screen(ui, session, Screen::Log),
        Action::ShowHelp => return open_screen(ui, session, Screen::Help),
        Action::ShowEndeavor => return open_screen(ui, session, Screen::Endeavor),
        Action::ShowNation => return open_screen(ui, session, Screen::Nation),
        Action::ShowAbilities => return open_screen(ui, session, Screen::Abilities),
        Action::ShowTraders => return open_screen(ui, session, Screen::Traders),
        _ => return false,
    }
    true
}

/// Moves the research cursor, as `App::move_tech` does.
fn move_tech(ui: &mut UiState, session: &Session, dx: i32, dy: i32) -> bool {
    let view = fn_ui::research::screen_view(session);
    ui.tech = research::step(&view, ui.tech, dx.signum(), dy);
    true
}

/// A page of tech cards, as `App::tech_page` does.
fn tech_page(ui: &UiState, session: &Session) -> i32 {
    let view = fn_ui::research::screen_view(session);
    let (width, rows) = screens::text_size(ui.page());
    research::Grid::new((width, rows), view.columns.len()).cards as i32
}

/// Moves the abilities screen's pointer, as `App::move_ability_cursor` does.
fn move_ability_cursor(ui: &mut UiState, session: &Session, dy: i32) -> bool {
    let rows = fn_ui::bend::abilities(session);
    let at = ui.ability_cursor.min(rows.len().saturating_sub(1)) as i64;
    ui.ability_cursor = (at + i64::from(dy)).clamp(0, rows.len() as i64 - 1).max(0) as usize;
    true
}

/// Moves the Nation screen's pointer, as `App::move_nation_cursor` does.
fn move_nation_cursor(ui: &mut UiState, session: &Session, dy: i32) -> bool {
    let view = fn_ui::view::nation_view(session);
    ui.nation_cursor = nation::step(&view, ui.nation_cursor, dy);
    true
}

/// A page of the Nation roster, as `App::nation_page` does.
fn nation_page(ui: &UiState) -> i32 {
    screens::text_size(ui.page()).1 as i32
}

/// `Enter` in the Nation screen: selects the settler under the pointer and closes the screen, as
/// `App::confirm_nation` does -- read-only, since selecting only changes the spectator's own
/// `UiState`.
fn confirm_nation(ui: &mut UiState, session: &Session) -> bool {
    let view = fn_ui::view::nation_view(session);
    let Some(&id) = nation::roster(&view).get(ui.nation_cursor) else { return false };
    ui.select_unit(session, id);
    ui.screen = None;
    true
}

/// How many rows the open screen shows at once, as `App::screen_rows` does.
fn screen_rows(ui: &UiState) -> i32 {
    ui.screen.map_or(0, |s| screens::body_rows(s, ui.page())) as i32
}

/// Scrolls the open screen, as `App::scroll_by` does.
fn scroll_by(ui: &mut UiState, session: &Session, screen: Screen, lines: i32) {
    if matches!(screen, Screen::Research | Screen::Saves | Screen::Abilities | Screen::Traders) {
        return;
    }
    let width = screens::text_size(ui.page()).0;
    let rows = screens::body_rows(screen, ui.page());
    let total = screens::row_count(session, ui, screen, width);
    let max = total.saturating_sub(rows);
    let now = ui.scroll.min(max) as i64;
    let to = (now + i64::from(lines)).clamp(0, max as i64) as usize;
    ui.scroll = to;
}

/// Opens `screen`, or closes it if it is the one already open, as `App::open` does (minus the
/// saves screen's own disk read, which the spectator never reaches).
fn open_screen(ui: &mut UiState, session: &Session, screen: Screen) -> bool {
    if ui.screen == Some(screen) {
        ui.screen = None;
    } else {
        ui.screen = Some(screen);
        ui.scroll = 0;
        if screen == Screen::Log {
            msglog::reset(ui);
        }
        ui.help_tab = crate::state::HelpTab::default();
        if screen == Screen::Research {
            let view = fn_ui::research::screen_view(session);
            ui.tech = research::initial_cursor(session, &view);
        }
        if screen == Screen::Nation {
            ui.nation_cursor = 0;
        }
        if screen == Screen::Abilities {
            let rows = fn_ui::bend::abilities(session);
            ui.ability_cursor = crate::abilities::initial_cursor(&rows);
        }
        if screen == Screen::Traders {
            ui.trade_cursor = 0;
        }
    }
    true
}

/// Opens the Build modal to browse, as `App::open_build_modal` does.
fn open_build_modal(ui: &mut UiState, session: &Session) {
    let rows = fn_ui::build::picker_rows(session);
    let choice = fn_ui::build::valid_choice(&rows, ui.choice);
    ui.build_cursor = choice.and_then(|id| rows.iter().position(|r| r.id == id)).unwrap_or(0);
    ui.modal = Some(Modal::Build);
}

/// The Build modal's own keys, browse-only: arrows or `W`/`S` move the highlight, any other
/// letter jumps to the next name starting with it, and `Esc` leaves. Unlike `App::build_modal_key`,
/// `Enter` does nothing here -- it would place the building, changing the agent's own game
/// (#330's acceptance criteria: "Build list (U, browse only)").
fn build_modal_key(ui: &mut UiState, session: &Session, key: KeyEvent) -> bool {
    if build::closes(key) {
        ui.modal = None;
        return true;
    }
    let rows = fn_ui::build::picker_rows(session);
    if rows.is_empty() {
        return false;
    }
    let at = ui.build_cursor.min(rows.len() - 1);
    match key.code {
        KeyCode::Up | KeyCode::Char('w' | 'W') => ui.build_cursor = build::step(&rows, at, -1),
        KeyCode::Down | KeyCode::Char('s' | 'S') => ui.build_cursor = build::step(&rows, at, 1),
        KeyCode::Char(c) if c.is_alphabetic() => {
            ui.build_cursor = build::jump_to_letter(&rows, at, c);
        }
        _ => return false,
    }
    true
}

#[cfg(test)]
mod tests {
    use crate::keyev::{KeyCode, KeyEvent, KeyModifiers};
    use fn_sim::Nation;
    use fn_sim::Pos;
    use fn_ui::Session;

    use super::*;
    use crate::state::UiState;

    fn session() -> Session {
        Session::new(7, Nation::Earth, 100, 80)
    }

    fn ui(session: &Session) -> UiState {
        UiState::new(session, (120, 40))
    }

    fn k(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    /// Arrow keys move the spectator's own cursor, never the session.
    #[test]
    fn arrows_move_the_spectators_cursor() {
        let s = session();
        let mut u = ui(&s);
        let before = u.cursor;
        assert!(key(&mut u, &s, k(KeyCode::Right)));
        assert_eq!(u.cursor, Pos::new(before.x + 1, before.y));
    }

    /// Designation tool keys do nothing: `H` would switch to Harvest in the game, but is refused
    /// for the spectator.
    #[test]
    fn a_designation_tool_key_does_nothing() {
        let s = session();
        let mut u = ui(&s);
        assert!(!key(&mut u, &s, k(KeyCode::Char('h'))));
        assert_eq!(u.tool, fn_ui::Tool::Select);
    }

    /// `U` opens the Build modal to browse, but `Enter` inside it never places anything, and
    /// arrows still move the highlight.
    #[test]
    fn the_build_modal_opens_to_browse_but_never_places() {
        let s = session();
        let mut u = ui(&s);
        assert!(key(&mut u, &s, k(KeyCode::Char('u'))));
        assert_eq!(u.modal, Some(Modal::Build));
        let before = u.build_cursor;
        assert!(key(&mut u, &s, k(KeyCode::Down)));
        assert_ne!(u.build_cursor, before, "the highlight should move");
        assert!(!key(&mut u, &s, k(KeyCode::Enter)), "Enter does nothing, placing included");
        assert_eq!(u.modal, Some(Modal::Build), "Enter never places or closes it");
        assert!(key(&mut u, &s, k(KeyCode::Esc)));
        assert_eq!(u.modal, None);
    }

    /// `T` opens the research screen; inside it, `Enter` does nothing (it would set the agent's
    /// own research target), but arrows still move the tech cursor.
    #[test]
    fn research_opens_but_enter_never_chooses_a_target() {
        let s = session();
        let mut u = ui(&s);
        assert!(key(&mut u, &s, k(KeyCode::Char('t'))));
        assert_eq!(u.screen, Some(Screen::Research));
        let tech_before = u.tech;
        assert!(key(&mut u, &s, k(KeyCode::Char('d'))));
        assert_ne!(u.tech, tech_before, "the tech cursor should still move");
        assert!(!key(&mut u, &s, k(KeyCode::Enter)), "Enter is refused in Research");
        // Re-pressing `T` closes the screen, the same toggle the game's own `T` has.
        assert!(key(&mut u, &s, k(KeyCode::Char('t'))));
        assert_eq!(u.screen, None);
    }

    /// `N` opens the Nation screen; `Enter` there selects the settler under the pointer and
    /// closes the screen, read-only (it never touches the session).
    #[test]
    fn nation_enter_selects_a_settler_and_closes_the_screen() {
        let s = session();
        let mut u = ui(&s);
        assert!(key(&mut u, &s, k(KeyCode::Char('n'))));
        assert_eq!(u.screen, Some(Screen::Nation));
        assert!(key(&mut u, &s, k(KeyCode::Enter)));
        assert_eq!(u.screen, None);
        assert!(u.selected.is_some());
    }

    /// Saving and loading, a new map, switching nation, pausing and the game's own speed, and
    /// quitting from the game menu are all refused outright.
    #[test]
    fn mutating_and_game_level_keys_do_nothing() {
        let s = session();
        let mut u = ui(&s);
        for code in [
            KeyCode::F(5),
            KeyCode::F(6),
            KeyCode::F(9),
            KeyCode::Char('r'),
            KeyCode::Char('1'),
            KeyCode::Char(' '),
            KeyCode::Char('+'),
            KeyCode::Char('-'),
            KeyCode::Char('.'),
            KeyCode::Char('q'),
            KeyCode::Esc,
        ] {
            assert!(!key(&mut u, &s, k(code)), "{code:?} should be refused");
        }
        assert_eq!(u, ui(&s), "nothing should have changed the spectator's own state either");
    }
}
