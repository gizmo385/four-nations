//! The game menu (`Esc`, #306, #655): opens when there is nothing else for `Esc` to close (no
//! tool active, no selection bar, no screen, modal or prompt open already). Resume, Save…,
//! Load…, Key help, then below a gap the three that end the game as it is: New map…, Switch
//! nation… and Quit…. Those three live only here (#655): the map has no single key for them.
//! Draws in the centred box (`crate::modal`, #285), not the full-screen frame.
//!
//! **Save…** and **Load…** both open the saves screen (`App::menu_choose`): its `N` writes a new
//! slot, Shift+`S` saves over the highlighted one and `Enter` loads it. `F5`, `F6` and `F9` stay
//! as shortcuts. **New map…**, **Quit…** and every nation on **Switch nation…**'s list ask in a
//! centred box first (`crate::ask`).
//!
//! **Switch nation…** turns the row list into the list of nations in place
//! ([`MenuPage::Nations`], [`UiState::menu_page`](crate::state::UiState::menu_page)); `Esc` there
//! goes back to the rows.

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};

use crate::color::Palette;
use crate::modal;
use crate::widgets::{Marks, Painter, Row, Tone};
use fn_ui::state::UiState;

// The rows, pages and cursor stepping are `fn_ui`'s (#857); this module draws them.
pub use fn_ui::menu::*;

/// The modal's target size: the rows, the gap, room for the longest label and a little space
/// round them.
pub const SIZE: (u16, u16) = (40, 10);

/// The menu's box when drawn over `outer` (`modal::modal_area`).
pub fn area(outer: Rect) -> Rect {
    modal::modal_area(outer, SIZE)
}

/// Draws the menu: the row list or the list of nations (`ui.menu_page`), the highlighted row
/// (`ui.menu_cursor`) reversed, and the Quit… row as the host can quit or not (`ui.can_quit`,
/// #866). `nations` is each nation's name and whether it is the one being played. `clock` is the game's
/// one clock (#654, `fn_ui::time::clock_text`), the same words as the side panel's, on the bottom
/// edge.
pub fn draw(
    ui: &UiState,
    nations: &[(String, bool)],
    clock: &str,
    palette: &Palette,
    outer: Rect,
    buf: &mut Buffer,
) {
    let (page, at, can_quit) = (ui.menu_page, ui.menu_cursor, ui.can_quit);
    let marks = Marks::new(palette.ascii);
    let title = format!(" {0} {1} {0} ", marks.seal, page.title());
    let inner = modal::draw(buf, outer, SIZE, palette, &title, clock);
    let at = at.min(page.rows() - 1);
    let mut p = Painter::new(buf, inner, palette);
    let mut reversed = None;
    let mut rows: Vec<(bool, usize, String, Option<&str>)> = Vec::new();
    match page {
        MenuPage::Rows => {
            for (i, row) in MenuRow::ALL.iter().enumerate() {
                rows.push((row.gap_before(), i, row.label_for(can_quit).to_owned(), None));
            }
        }
        MenuPage::Nations => {
            for (i, (name, playing)) in nations.iter().enumerate() {
                rows.push((false, i, name.clone(), playing.then_some(PLAYING)));
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
