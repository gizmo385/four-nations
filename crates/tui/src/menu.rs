//! The game menu (`Esc`, #306): opens when there is nothing else for `Esc` to close (no tool
//! active, no selection bar, no screen, modal or prompt open already) — Resume, Save, Load, Key
//! help and Quit, so saving and loading don't live on function keys alone
//! (`App::open_menu`/`App::menu_key`, `crate::app`). Reuses the Build modal's reusable frame
//! (`crate::modal`, #285).

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
    /// Writes the quicksave slot at once, the same as `F5` (`App::quicksave`). A named slot is
    /// `Load`'s saves screen below, which writes one too (`N`, Shift+`S`) — between the two,
    /// every way to save is reachable without a function key.
    Save,
    /// Opens the saves screen (#276): `Enter` loads the highlighted slot, `N` writes a new one
    /// and Shift+`S` saves over it, so it is also where a named-slot save lives.
    Load,
    /// The key help screen (`?`).
    Help,
    /// Asks to quit, the same question `Q` does (`Prompt::Quit`); the menu stays open behind
    /// the prompt until it's answered, so declining leaves the menu where it was.
    Quit,
}

impl MenuRow {
    pub const ALL: [MenuRow; 5] = [Self::Resume, Self::Save, Self::Load, Self::Help, Self::Quit];

    pub fn label(self) -> &'static str {
        match self {
            Self::Resume => "Resume",
            Self::Save => "Save (quicksave)",
            Self::Load => "Load (and named saves)",
            Self::Help => "Key help",
            Self::Quit => "Quit",
        }
    }
}

/// The modal's target size: five short rows and a little room round them.
pub const SIZE: (u16, u16) = (32, 9);

/// The cursor after a step of `dy` rows (its sign only), stopping at the list's edges, as
/// `crate::build::step` and `crate::saves::step` do.
pub fn step(at: usize, dy: i32) -> usize {
    let last = MenuRow::ALL.len() as i64 - 1;
    (at as i64 + i64::from(dy.signum())).clamp(0, last) as usize
}

/// Draws the menu: the frame and the five rows, the highlighted one reversed.
pub fn draw(at: usize, palette: &Palette, outer: Rect, buf: &mut Buffer) {
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
        let pointer = if chosen { marks.pointer } else { " " };
        let tone = if chosen { Tone::Title } else { Tone::Text };
        p.row(&Row::text(format!("{pointer} {}", row.label()), tone));
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
        assert_eq!(step(0, -1), 0);
        assert_eq!(step(0, 1), 1);
        assert_eq!(step(MenuRow::ALL.len() - 1, 1), MenuRow::ALL.len() - 1);
    }

    #[test]
    fn every_row_has_a_label() {
        for row in MenuRow::ALL {
            assert!(!row.label().is_empty());
        }
    }
}
