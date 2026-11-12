//! The Airship screen (`Enter` on an Airship Dock, #763, docs/design/horizons-phase-6.md,
//! "Mockups"): the nation to sail to, its deals and how many times each, the hold, and the SEND
//! THE AIRSHIP box over it. What it says and every key are `fn_ui::airship`'s; this draws them
//! in the shared page frame (`crate::page`, #652), as the Traders screen does.

use crate::color::Palette;
use crate::page::{self, Page};
use crate::widgets::{Marks, Painter, Row, Tone};
use fn_ui::Session;
use fn_ui::airship::{self, CONFIRM_TITLE, CONFIRM_WIDTH, Picks};
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;

// The rows are `fn_ui`'s; this module draws them.
pub use fn_ui::screens::airship::*;

/// Draws the screen's contents into `page` (the frame is drawn).
pub fn draw(session: &Session, picks: &Picks, palette: &Palette, page: &Page, buf: &mut Buffer) {
    let marks = Marks::new(palette.ascii);
    let body = page.body;
    let view = airship::view(session, picks);
    let (rows, chosen) = rows(&view, usize::from(body.width), &marks);
    let mut p = Painter::new(buf, body, palette);
    for row in &rows {
        p.row(row);
    }
    if let Some(i) = chosen.filter(|&i| i < usize::from(body.height)) {
        page::highlight(buf, Rect::new(body.x, body.y + i as u16, body.width, 1), palette);
    }
}

/// Draws the SEND THE AIRSHIP box centred over `area` (the page).
pub fn draw_confirm(
    session: &Session,
    picks: &Picks,
    palette: &Palette,
    area: Rect,
    buf: &mut Buffer,
) {
    let width = CONFIRM_WIDTH as u16 + 4;
    let text = usize::from(width.saturating_sub(4).min(area.width.saturating_sub(8)).max(1));
    let marks = Marks::new(palette.ascii);
    let mut rows = confirm_body(session, picks, &marks, text);
    // The keys, centred, after a blank row.
    let (go, back) = airship::confirm_buttons();
    let keys = format!("{go}     {back}");
    let pad = text.saturating_sub(keys.chars().count()) / 2;
    rows.push(Row::text(String::new(), Tone::Text));
    rows.push(Row::text(format!("{}{keys}", " ".repeat(pad)), Tone::Accent));
    let height = u16::try_from(rows.len() + 2).unwrap_or(u16::MAX);
    let inner = crate::modal::draw(buf, area, (width, height), palette, CONFIRM_TITLE, "");
    let mut p = Painter::new(buf, inner, palette);
    for row in &rows {
        p.row(row);
    }
}
