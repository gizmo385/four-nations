//! The Traders screen (`M`, #438, docs/design/pacing.md section 2, "Screen"): the caravan camped
//! now, on a full screen in the shared frame (`crate::page`, #652). Its name and how long it
//! stays head the page; DEALS lists the deals it offers, each with how many times it has been
//! taken and what the stores hold of what it asks; REQUEST is its one gift; UNDER WAY is every
//! taken deal still being hauled, with what has been delivered. With no caravan camped the page
//! says when the next is due, or that the road isn't open yet.
//!
//! A pointer (`▸` and the quiet background, `page::highlight`) moves over the deals and the
//! request. `Enter` (`App::take_deal`) takes the one under it once more, `X` cancels what is
//! still owed on it, and `Esc` or `M` closes. The view and both actions are `fn_ui::trade`'s.

use crate::color::Palette;
use crate::page::{self, Page};
use crate::widgets::{Marks, Painter};
use fn_ui::trade::TradersView;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;

// The rows, cursors and keys are `fn_ui`'s (#857); this module draws them.
pub use fn_ui::screens::traders::*;

/// Draws the screen's contents into `page` (the frame is drawn).
pub fn draw(view: &TradersView, at: usize, palette: &Palette, page: &Page, buf: &mut Buffer) {
    let marks = Marks::new(palette.ascii);
    let body = page.body;
    let (rows, chosen) = rows(view, at, usize::from(body.width), &marks);
    let mut p = Painter::new(buf, body, palette);
    for row in &rows {
        p.row(row);
    }
    if let Some(i) = chosen.filter(|&i| i < usize::from(body.height)) {
        page::highlight(buf, Rect::new(body.x, body.y + i as u16, body.width, 1), palette);
    }
}
