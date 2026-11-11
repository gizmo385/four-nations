//! The game menu (`Esc`, #306, #655): opens when there is nothing else for `Esc` to close (no
//! tool active, no selection bar, no screen, modal or prompt open already). Resume, Save…,
//! Load…, Key help, then below a gap the three that end the game as it is: New map…, Switch
//! nation… and Quit…. Those three live only here (#655): the map has no single key for them.
//! Reuses the Build modal's reusable frame (`crate::modal`, #285).
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
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};

use crate::color::Palette;
use crate::modal;
use crate::widgets::{Marks, Painter, Row, Tone};

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

    /// Whether a blank line sits above this row: the three that end the game as it is are set
    /// apart from the rest.
    fn gap_before(self) -> bool {
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

/// The modal's target size: the rows, the gap, room for the longest label and a little space
/// round them.
pub const SIZE: (u16, u16) = (40, 10);

/// The cursor after a step of `dy` rows (its sign only) on a list of `len`, stopping at the
/// list's edges, as `crate::build::step` and `crate::saves::step` do.
pub fn step(at: usize, dy: i32, len: usize) -> usize {
    let last = (len as i64 - 1).max(0);
    (at as i64 + i64::from(dy.signum())).clamp(0, last) as usize
}

/// Draws the menu: the row list or the list of nations (`page`), the highlighted row reversed.
/// `nations` is each nation's name and whether it is the one being played. `clock`, while
/// seasons are on, is the elapsed game clock (#416, owner decision 4 on #414): with the side
/// panel's header showing the season instead, the menu is one of the two places it moved to (the
/// Nation screen's header line, already there, is the other).
pub fn draw(
    page: MenuPage,
    at: usize,
    nations: &[(String, bool)],
    clock: Option<&str>,
    palette: &Palette,
    outer: Rect,
    buf: &mut Buffer,
) {
    let marks = Marks::new(palette.ascii);
    let name = match page {
        MenuPage::Rows => "MENU",
        MenuPage::Nations => "SWITCH NATION",
    };
    let title = format!(" {0} {name} {0} ", marks.seal);
    let inner = modal::draw(buf, outer, SIZE, palette, &title, clock.unwrap_or(""));
    let at = at.min(page.rows() - 1);
    let mut p = Painter::new(buf, inner, palette);
    let mut reversed = None;
    let mut rows: Vec<(bool, usize, String, Option<&str>)> = Vec::new();
    match page {
        MenuPage::Rows => {
            for (i, row) in MenuRow::ALL.iter().enumerate() {
                rows.push((row.gap_before(), i, row.label().to_owned(), None));
            }
        }
        MenuPage::Nations => {
            for (i, (name, playing)) in nations.iter().enumerate() {
                rows.push((false, i, name.clone(), playing.then_some("playing")));
            }
        }
    }
    for (gap, i, label, note) in rows {
        if gap {
            p.blank();
        }
        let chosen = i == at;
        if chosen {
            reversed = Some(inner.y + p.row);
        }
        let pointer = if chosen { marks.pointer } else { " " };
        let tone = if chosen { Tone::Title } else { Tone::Text };
        let left = format!("{pointer} {label}");
        match note {
            Some(note) => p.row(&Row::pair(left, tone, note, Tone::Dim)),
            None => p.row(&Row::text(left, tone)),
        }
    }
    if let Some(y) = reversed {
        for x in inner.x..inner.right() {
            buf[(x, y)].set_style(Style::new().add_modifier(Modifier::REVERSED));
        }
    }
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
    }

    #[test]
    fn the_three_that_end_the_game_are_set_apart() {
        let gaps: Vec<MenuRow> = MenuRow::ALL.into_iter().filter(|r| r.gap_before()).collect();
        assert_eq!(gaps, [MenuRow::NewMap]);
    }
}
