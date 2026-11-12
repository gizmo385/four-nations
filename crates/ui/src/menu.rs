//! The game menu (`Esc`, #306, #655): opens when there is nothing else for `Esc` to close (no
//! tool active, no selection bar, no screen, modal or prompt open already). Resume, Save…,
//! Load…, Key help, then below a gap the three that end the game as it is: New map…, Switch
//! nation… and Quit…. Those three live only here (#655): the map has no single key for them.
//! Draws in the centred box (`fn_tui::modal`, #285), not the full-screen frame.
//!
//! **Save…** and **Load…** both open the saves screen (`App::menu_choose`): its `N` writes a new
//! slot, Shift+`S` saves over the highlighted one and `Enter` loads it. `F5`, `F6` and `F9` stay
//! as shortcuts. **New map…**, **Quit…** and every nation on **Switch nation…**'s list ask in a
//! centred box first (`crate::ask`).
//!
//! **Switch nation…** turns the row list into the list of nations in place
//! ([`MenuPage::Nations`], [`UiState::menu_page`](crate::state::UiState::menu_page)); `Esc` there
//! goes back to the rows.

use fn_sim::Nation;

use crate::Session;

/// One row of the menu, in the order it lists them.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MenuRow {
    /// Closes the menu, as `Esc` does too.
    Resume,
    /// Opens the saves screen (#276), where `N` writes a new slot and Shift+`S` saves over one.
    Save,
    /// Opens the saves screen too: `Enter` loads the highlighted slot, after asking.
    Load,
    /// The key help screen (`?`).
    Help,
    /// Asks, then opens embark on the next seed (`Prompt::NewMap`).
    NewMap,
    /// Lists the nations ([`MenuPage::Nations`]); choosing one asks (`Prompt::Nation`).
    SwitchNation,
    /// Asks, then quits (`Prompt::Quit`).
    Quit,
}

impl MenuRow {
    pub const ALL: [MenuRow; 7] = [
        Self::Resume,
        Self::Save,
        Self::Load,
        Self::Help,
        Self::NewMap,
        Self::SwitchNation,
        Self::Quit,
    ];

    /// The row's text. A trailing `…` says choosing it goes on to another step.
    pub fn label(self) -> &'static str {
        match self {
            Self::Resume => "Resume",
            Self::Save => "Save…",
            Self::Load => "Load…",
            Self::Help => "Key help",
            Self::NewMap => "New map…",
            Self::SwitchNation => "Switch nation…",
            Self::Quit => "Quit…",
        }
    }

    /// [`Self::label`] for a host that may not be able to quit (`UiState::can_quit`, #866): with
    /// nowhere to quit to, the Quit… row is the way back to the title screen and says so.
    pub fn label_for(self, can_quit: bool) -> &'static str {
        match self {
            Self::Quit if !can_quit => "Title screen…",
            row => row.label(),
        }
    }

    /// Whether a blank line sits above this row: the three that end the game as it is are set
    /// apart from the rest.
    pub fn gap_before(self) -> bool {
        self == Self::NewMap
    }
}

/// Which list the menu shows.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum MenuPage {
    /// [`MenuRow::ALL`].
    #[default]
    Rows,
    /// [`Nation::ALL`], under Switch nation….
    Nations,
}

impl MenuPage {
    /// How many rows the page lists (never none).
    pub fn rows(self) -> usize {
        match self {
            Self::Rows => MenuRow::ALL.len(),
            Self::Nations => Nation::ALL.len(),
        }
    }
}

impl MenuPage {
    /// The box's title: the menu's, or the nation list's.
    pub fn title(self) -> &'static str {
        match self {
            Self::Rows => "MENU",
            Self::Nations => "SWITCH NATION",
        }
    }
}

/// The note on the nation list's row for the nation being played.
pub const PLAYING: &str = "playing";

/// [`MenuPage::Nations`]' rows, in [`Nation::ALL`]'s order: each nation's name, and whether it is
/// the one being played (which the row says, [`PLAYING`]).
pub fn nation_rows(session: &Session) -> Vec<(String, bool)> {
    Nation::ALL
        .iter()
        .map(|&n| (session.defs.nation(n).name.clone(), n == session.world.nation))
        .collect()
}

/// The cursor after a step of `dy` rows (its sign only) on a list of `len`, stopping at the
/// list's edges, as `crate::build::step` and `crate::saves::step` do.
pub fn step(at: usize, dy: i32, len: usize) -> usize {
    let last = (len as i64 - 1).max(0);
    (at as i64 + i64::from(dy.signum())).clamp(0, last) as usize
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stepping_stops_at_the_edges() {
        let len = MenuPage::Rows.rows();
        assert_eq!(step(0, -1, len), 0);
        assert_eq!(step(0, 1, len), 1);
        assert_eq!(step(len - 1, 1, len), len - 1);
        assert_eq!(step(3, 1, MenuPage::Nations.rows()), 3);
    }

    #[test]
    fn every_row_has_a_label_and_the_ones_that_go_on_say_so() {
        for row in MenuRow::ALL {
            assert!(!row.label().is_empty());
        }
        for row in [MenuRow::Save, MenuRow::Load, MenuRow::NewMap, MenuRow::SwitchNation] {
            assert!(row.label().ends_with('…'), "{row:?}");
        }
        assert!(MenuRow::Quit.label().ends_with('…'));
        assert_eq!(MenuRow::Quit.label_for(true), "Quit…");
        assert_eq!(MenuRow::Quit.label_for(false), "Title screen…");
        assert_eq!(MenuRow::Save.label_for(false), "Save…");
    }

    #[test]
    fn the_pages_have_titles_and_the_nation_list_marks_the_one_playing() {
        assert_eq!(MenuPage::Rows.title(), "MENU");
        assert_eq!(MenuPage::Nations.title(), "SWITCH NATION");
        let session = Session::new(7, Nation::Fire, 48, 32);
        let rows = nation_rows(&session);
        assert_eq!(rows.len(), Nation::ALL.len());
        let playing: Vec<&str> = rows.iter().filter(|(_, p)| *p).map(|(n, _)| n.as_str()).collect();
        assert_eq!(playing, [session.defs.nation(Nation::Fire).name.as_str()]);
    }

    #[test]
    fn the_three_that_end_the_game_are_set_apart() {
        let gaps: Vec<MenuRow> = MenuRow::ALL.into_iter().filter(|r| r.gap_before()).collect();
        assert_eq!(gaps, [MenuRow::NewMap]);
    }
}
