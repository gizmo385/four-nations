//! The message log screen (`L`, #661): the log as a player reads it. The newest line is at the
//! top, a run of the same event is one line (`Right` opens it, `Left` folds it), a row of
//! filters picks one kind of line (`1` to `5`, `0` for all; `Tab` in the `--watch` spectator,
//! where the studio has the digits, #774), the cursor moves a line at a time,
//! and `Enter` goes to where the line is about.
//!
//! ```text
//! ┏──────────────────────────── ◆ MESSAGE LOG ◆ ────────────────────────────┓
//! │ Show  [All]  1 Alerts  2 People  3 Knowledge  4 Bending  5 Work          │
//! │                                                                          │
//! │ ── WINTER, YEAR 1 ────────────────────────────────────────────────────── │
//! │   0:51 left   Learned Pottery                                            │
//! │ ▸ 8:11 left + Raise Earth used 7 times by 6 benders, near (51, 65)       │
//! │  10:00 left   Jin has joined the settlement                              │
//! ```
//!
//! What the lines are, how they fold and what the cursor is on is `fn_ui::msglog`; this module
//! draws them and turns the keys into changes of [`UiState`]. The app and the watch spectator
//! share [`act`] and [`press`], so the two never disagree. It only reads the history: folding
//! and filtering are never written back, and the run log an agent reads is one line per event.

use crate::color::Palette;
use crate::page::{self, Page};
use crate::state::UiState;
use crate::widgets::{Marks, Painter};
use fn_ui::Session;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;

// The rows, cursors and keys are `fn_ui`'s (#857); this module draws them.
pub use fn_ui::screens::msglog::*;

/// Draws the screen into `page`: the filter row, a blank row, then the window of lines.
pub fn draw(
    session: &Session,
    ui: &UiState,
    palette: &Palette,
    page: &Page,
    buf: &mut Buffer,
) -> Option<String> {
    let body_area = page.body;
    let marks = Marks::new(palette.ascii);
    let head = (HEAD_ROWS as u16).min(body_area.height);
    let mut strip =
        Painter::new(buf, Rect::new(body_area.x, body_area.y, body_area.width, head), palette);
    strip.row(&filter_row(ui.log_filter, ui.spectating));
    let below =
        Rect::new(body_area.x, body_area.y + head, body_area.width, body_area.height - head);
    let all = lines(ui);
    let laid = body(
        session,
        &all,
        ui.log_filter,
        ui.spectating,
        ui.log_cursor,
        usize::from(below.width),
        &marks,
    );
    let shown = usize::from(below.height);
    let start = window(ui.scroll, laid.cursor.as_ref(), shown, laid.rows.len());
    let mut p = Painter::new(buf, below, palette);
    for row in laid.rows.iter().skip(start) {
        p.row(row);
    }
    if let Some(c) = &laid.cursor {
        // The chosen line wears the selection, every row it wraps to that shows.
        let (top, bottom) = (c.start.max(start), c.end.min(start + shown));
        if top < bottom {
            let area = Rect::new(
                below.x,
                below.y + (top - start) as u16,
                below.width,
                (bottom - top) as u16,
            );
            page::highlight(buf, area, palette);
        }
    }
    fn_ui::text::window_label("", start, shown, laid.rows.len())
}
