//! The game menu (`Esc`, #306): opens when there is nothing else for `Esc` to close (no tool
//! active, no selection bar, no screen, modal or prompt open already) — Resume, Save as…,
//! Quicksave, Load…, Quickload, Key help and Quit, so every save/load action is reachable
//! without a function key, which some terminals (Windows Terminal on WSL among them, #312) never
//! pass through (`App::open_menu`/`App::menu_key`, `crate::app`). Reuses the Build modal's
//! reusable frame (`crate::modal`, #285).
//!
//! **Save as…** (#312) turns the row list into a one-line name prompt in place
//! (`App::menu_name_key`, [`UiState::menu_name`](crate::state::UiState::menu_name)): typed text
//! replaces the row list until `Enter` writes the slot (asking first if the name is already
//! taken, as Shift+`S` does in the saves screen) or `Esc` returns to the row list. An empty name
//! falls back to the saves screen's own `N` naming (`fn_ui::saves::new_name`), so Enter alone
//! always saves something.

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
    /// Types a name in place (see the module docs) and writes a new slot under it, or over an
    /// existing one after a y/n prompt like Shift+`S`. The other way to a named slot is `Load`'s
    /// saves screen below, whose own `N` and Shift+`S` do the same two things.
    SaveAs,
    /// Writes the quicksave slot at once, the same as `F5` (`App::quicksave`).
    Quicksave,
    /// Opens the saves screen (#276): `Enter` loads the highlighted slot, `N` writes a new one
    /// and Shift+`S` saves over it, so it is also where a named-slot save lives.
    Load,
    /// Asks the same y/n `F9` does (`Prompt::QuickLoad`), then the quicksave becomes the game.
    /// Reads as disabled with no quicksave yet, as there is then nothing to load.
    Quickload,
    /// The key help screen (`?`).
    Help,
    /// Asks to quit, the same question `Q` does (`Prompt::Quit`); the menu stays open behind
    /// the prompt until it's answered, so declining leaves the menu where it was.
    Quit,
}

impl MenuRow {
    pub const ALL: [MenuRow; 7] = [
        Self::Resume,
        Self::SaveAs,
        Self::Quicksave,
        Self::Load,
        Self::Quickload,
        Self::Help,
        Self::Quit,
    ];

    /// The row's text. `has_quicksave` only changes [`Self::Quickload`]'s, which says so when
    /// there's nothing to load yet.
    pub fn label(self, has_quicksave: bool) -> &'static str {
        match self {
            Self::Resume => "Resume",
            Self::SaveAs => "Save as… (named slot)",
            Self::Quicksave => "Quicksave",
            Self::Load => "Load… (and named saves)",
            Self::Quickload => {
                if has_quicksave {
                    "Quickload"
                } else {
                    "Quickload (none yet)"
                }
            }
            Self::Help => "Key help",
            Self::Quit => "Quit",
        }
    }

    /// The function key this row doubles for, where it has one: the whole point of the menu is
    /// reaching these when such a key doesn't arrive, so the row says which it is.
    pub fn hint(self) -> Option<&'static str> {
        match self {
            Self::Quicksave => Some("F5"),
            Self::Load => Some("F6"),
            Self::Quickload => Some("F9"),
            Self::Resume | Self::SaveAs | Self::Help | Self::Quit => None,
        }
    }
}

/// The modal's target size: seven short rows, room for the longest label and its hint, and a
/// little space round them.
pub const SIZE: (u16, u16) = (40, 11);

/// The cursor after a step of `dy` rows (its sign only), stopping at the list's edges, as
/// `crate::build::step` and `crate::saves::step` do.
pub fn step(at: usize, dy: i32) -> usize {
    let last = MenuRow::ALL.len() as i64 - 1;
    (at as i64 + i64::from(dy.signum())).clamp(0, last) as usize
}

/// The naming prompt's footer.
const NAME_FOOTER: &str = "Type a name  Enter save  Esc back";

/// Draws the menu: the row list ([`draw_rows`]), or in its place the Save as… name prompt
/// (`naming`, `Some` while [`crate::state::UiState::menu_name`] is).
pub fn draw(
    at: usize,
    naming: Option<&str>,
    has_quicksave: bool,
    palette: &Palette,
    outer: Rect,
    buf: &mut Buffer,
) {
    match naming {
        Some(name) => draw_naming(name, palette, outer, buf),
        None => draw_rows(at, has_quicksave, palette, outer, buf),
    }
}

/// The row list, the highlighted one reversed.
fn draw_rows(at: usize, has_quicksave: bool, palette: &Palette, outer: Rect, buf: &mut Buffer) {
    let marks = Marks::new(palette.ascii);
    let title = format!(" {0} MENU {0} ", marks.seal);
    let footer = "Arrows/W S choose  Enter select  Esc resume";
    let inner = modal::draw(buf, outer, SIZE, palette, &title, footer);
    let at = at.min(MenuRow::ALL.len() - 1);
    let mut p = Painter::new(buf, inner, palette);
    let mut reversed = None;
    for (i, row) in MenuRow::ALL.iter().enumerate() {
        let chosen = i == at;
        if chosen {
            reversed = Some(inner.y + p.row);
        }
        let disabled = *row == MenuRow::Quickload && !has_quicksave;
        let pointer = if chosen { marks.pointer } else { " " };
        let tone = if disabled {
            Tone::Dim
        } else if chosen {
            Tone::Title
        } else {
            Tone::Text
        };
        let left = format!("{pointer} {}", row.label(has_quicksave));
        match row.hint() {
            Some(hint) => p.row(&Row::pair(left, tone, hint, Tone::Dim)),
            None => p.row(&Row::text(left, tone)),
        }
    }
    if let Some(y) = reversed {
        for x in inner.x..inner.right() {
            buf[(x, y)].set_style(Style::new().add_modifier(Modifier::REVERSED));
        }
    }
}

/// The Save as… name prompt, in the same frame: one line of typed text, a cursor block at its
/// end.
fn draw_naming(name: &str, palette: &Palette, outer: Rect, buf: &mut Buffer) {
    let marks = Marks::new(palette.ascii);
    let title = format!(" {0} SAVE AS {0} ", marks.seal);
    let inner = modal::draw(buf, outer, SIZE, palette, &title, NAME_FOOTER);
    let mut p = Painter::new(buf, inner, palette);
    p.row(&Row::text(format!("Name: {name}"), Tone::Text));
    p.blank();
    p.row(&Row::text("Letters, digits, - and _ only.", Tone::Dim));
    let y = inner.y;
    let x = inner.x + "Name: ".chars().count() as u16 + name.chars().count() as u16;
    if x < inner.right() {
        buf[(x, y)].set_style(Style::new().add_modifier(Modifier::REVERSED));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stepping_stops_at_the_edges() {
        assert_eq!(step(0, -1), 0);
        assert_eq!(step(0, 1), 1);
        assert_eq!(step(MenuRow::ALL.len() - 1, 1), MenuRow::ALL.len() - 1);
    }

    #[test]
    fn every_row_has_a_label() {
        for row in MenuRow::ALL {
            assert!(!row.label(true).is_empty());
            assert!(!row.label(false).is_empty());
        }
    }

    #[test]
    fn only_the_f_key_rows_hint_their_key() {
        for row in MenuRow::ALL {
            let expect = matches!(row, MenuRow::Quicksave | MenuRow::Load | MenuRow::Quickload);
            assert_eq!(row.hint().is_some(), expect, "{row:?}");
        }
    }

    #[test]
    fn quickload_says_so_when_there_is_nothing_to_load() {
        assert_eq!(MenuRow::Quickload.label(true), "Quickload");
        assert!(MenuRow::Quickload.label(false).contains("none yet"));
    }
}
