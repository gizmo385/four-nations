//! The saves screen (`F6`, #276): every slot in the save directory, newest first, with its
//! `Slot::describe` text; an unloadable slot's reason shows dimmed. `Enter` asks to load the
//! highlighted slot through `App::load`; `S` (Shift+`s`: plain `s` already moves the highlight)
//! asks to save over it; `N` types a name for a new one (#704): `Enter` saves it, with the next
//! free `save-N` (`fn_ui::saves::new_name`) when nothing was typed, and `Esc` goes back. A name
//! `fn_ui::saves::check_name` refuses is refused with the typing kept, and one already taken asks
//! first, as Shift+`S` does.
//!
//! Listing touches disk, so unlike the other screens the slots are read once, when the screen
//! opens or a save changes them (`App::refresh_saves`), and cached on `UiState`; [`draw`] here
//! is then a pure function of that cache like every other screen.

use crate::color::Palette;
use crate::page;
use crate::widgets::{Marks, Painter, Row, Tone, window_start};
use fn_sim::Defs;
use fn_ui::saves::Slot;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;

// The rows, cursors and keys are `fn_ui`'s (#857); this module draws them.
pub use fn_ui::screens::saves::*;

/// Draws the screen's contents (the list, or a note that there is none yet) into `inner` (a full
/// screen's body, or the title screen's Load page). The slot under `at` wears the selection
/// (`page::highlight`), as the research screen's chosen card does.
pub fn draw(
    slots: &[Slot],
    at: usize,
    defs: &Defs,
    palette: &Palette,
    inner: Rect,
    buf: &mut Buffer,
) -> Option<String> {
    let marks = Marks::new(palette.ascii);
    let mut p = Painter::new(buf, inner, palette);
    if slots.is_empty() {
        p.row(&Row::text("No saves yet: press N to write one.", Tone::Dim));
        return None;
    }
    let at = at.min(slots.len() - 1);
    let shown = usize::from(inner.height);
    let first = window_start(at, shown);
    let mut chosen_at = None;
    for (i, slot) in slots.iter().enumerate().skip(first).take(shown) {
        let chosen = i == at;
        if chosen {
            chosen_at = Some(inner.y + p.row);
        }
        p.row(&slot_row(slot, defs, chosen, &marks));
    }
    if let Some(y) = chosen_at {
        page::highlight(buf, Rect::new(inner.x, y, inner.width, 1), palette);
    }
    fn_ui::text::window_label("slots", first, shown, slots.len())
}

/// The row a new save's name is typed on (#704): "Name: " and what is typed so far with a cursor
/// after it. Nothing typed shows the suggestion dimmed after the cursor, since `Enter` on nothing
/// saves under it.
pub fn draw_name(typed: &str, suggestion: &str, palette: &Palette, row: Rect, buf: &mut Buffer) {
    let mut p = Painter::new(buf, row, palette);
    let mut spans = vec![("Name: ".to_owned(), Tone::Title), (format!("{typed}_"), Tone::Strong)];
    if typed.is_empty() {
        spans.push((format!(" {suggestion}"), Tone::Dim));
    }
    p.row(&Row::spans(spans));
}
