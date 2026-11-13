//! The corner notices (#962): `fn_ui::notice::Notices` as boxes over the top right of the map,
//! newest on top, that stay until dismissed (docs/design/tui.md). The window draws the same
//! notices as widgets (`fn_gui`); the model, the actions and the keys are `fn_ui`'s.
//!
//! ```text
//! ┌──────────────────────────────────┐
//! │ Fire Nation traders              │
//! │ made camp                        │
//! │ Enter jump · M trade · Del ✕     │
//! └──────────────────────────────────┘
//! ┌──────────────────────────────────┐
//! │ Research done: Ice Construction  │
//! │ Enter · T research · Del ✕       │
//! └──────────────────────────────────┘
//!  +1 more · Del dismiss
//! ```
//!
//! A box is [`WIDTH`] columns, less on a small terminal; its edge wears the notice's tone (the
//! warning colour for traders, the bad one for a death, the accent for the rest). At most
//! [`MAX_BOXES`] are drawn, then a "+N more" line; a box that doesn't fit under the ones above
//! it is dropped, and the first shrinks its body to fit. Keys only: the terminal takes no
//! mouse. `Enter` runs the top notice's first action and `Del` (or `Esc`, with nothing
//! selected) dismisses it (`fn_ui::app::App`).

use fn_ui::alerts::Kind;
use fn_ui::notice::Notice;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Style;

use crate::color::Palette;
use crate::widgets::{Marks, Tone, ToneStyle, fit, wrap};

/// A box's width, in columns, on a terminal with room.
pub const WIDTH: u16 = 36;
/// The most boxes drawn; the rest wait behind them as a "+N more" line.
pub const MAX_BOXES: usize = 2;
/// The most lines a box's body takes.
const BODY_LINES: usize = 2;
/// The blank columns between the boxes and the map's right edge.
const MARGIN: u16 = 1;
/// Narrower than this, a box can't hold its hints: none are drawn.
const MIN_WIDTH: u16 = 20;

/// What a box says, cut to `room` columns.
struct Face {
    title: String,
    body: Vec<String>,
    /// The key hints and the dismiss key: the pieces of the last line, key first.
    hints: Vec<(String, String)>,
    kind: Kind,
}

impl Face {
    fn of(notice: &Notice, room: usize, marks: &Marks) -> Self {
        let mut body = Vec::new();
        for line in &notice.body {
            let left = BODY_LINES - body.len();
            if left == 0 {
                break;
            }
            body.extend(wrap(line, room, left, marks.ellipsis));
        }
        body.truncate(BODY_LINES);
        let ascii = |s: &str| if marks.ascii { s.replace('·', "-") } else { s.to_owned() };
        let mut hints: Vec<(String, String)> =
            notice.key_hints().into_iter().map(|h| (ascii(&h.key), h.label.to_owned())).collect();
        hints.push(("Del".to_owned(), if marks.ascii { "x" } else { "✕" }.to_owned()));
        Self { title: fit(&notice.title, room, marks.ellipsis), body, hints, kind: notice.kind }
    }

    /// The box's height with `body` body lines: the edges, the title and the hints.
    fn height(&self, body: usize) -> u16 {
        (2 + 1 + body.min(self.body.len()) + 1) as u16
    }
}

/// How many boxes of these `heights` fit in `rows` rows, keeping a row for the "+N more" line
/// whenever notices beyond the ones drawn are up (`total` counts them all).
fn fitting(heights: &[u16], total: usize, rows: u16) -> usize {
    let mut used = 0u16;
    let mut drawn = 0;
    for &h in heights {
        let more_row = u16::from(total > drawn + 1);
        if used + h + more_row > rows {
            break;
        }
        used += h;
        drawn += 1;
    }
    drawn
}

/// Draws the notices up over `map` (the map region), the top `skip` rows of it left clear (the
/// tool banner's). Nothing with none up or no room.
pub fn draw(
    notices: &fn_ui::notice::Notices,
    palette: &Palette,
    map: Rect,
    skip: u16,
    buf: &mut Buffer,
) {
    if !notices.any() || map.width < MIN_WIDTH + MARGIN || map.height <= skip {
        return;
    }
    let marks = Marks::new(palette.ascii);
    let width = WIDTH.min(map.width - MARGIN);
    let x = map.right() - MARGIN - width;
    let rows = map.height - skip;
    let room = usize::from(width).saturating_sub(4);
    let total = notices.shown().count() + notices.hidden();
    let faces: Vec<Face> =
        notices.shown().take(MAX_BOXES).map(|n| Face::of(n, room, &marks)).collect();
    let heights: Vec<u16> = faces.iter().map(|f| f.height(BODY_LINES)).collect();

    let mut drawn = fitting(&heights, total, rows);
    // The first box shrinks (its body, a line at a time) rather than give up its place.
    let mut first_body = BODY_LINES;
    if drawn == 0 {
        let Some(face) = faces.first() else { return };
        let more_row = u16::from(total > 1);
        let mut body = face.body.len();
        while body > 0 && face.height(body) + more_row > rows {
            body -= 1;
        }
        if face.height(body) + more_row > rows {
            return;
        }
        first_body = body;
        drawn = 1;
    }

    let mut y = map.y + skip;
    for face in &faces[..drawn] {
        let body = if y == map.y + skip { first_body } else { BODY_LINES };
        let height = face.height(body);
        draw_box(face, body, Rect::new(x, y, width, height), palette, &marks, buf);
        y += height;
    }
    let more = total - drawn;
    if more > 0 && y < map.bottom() {
        let text = format!("+{more} more {} Del dismiss", if palette.ascii { "-" } else { "·" });
        let area = Rect::new(x, y, width, 1);
        fill(buf, area, palette);
        let text = fit(&format!(" {text}"), usize::from(width), marks.ellipsis);
        buf.set_stringn(x, y, &text, usize::from(width), Tone::Dim.style(palette));
    }
}

/// Blanks `area` to the panel colour, whatever the map left in the cells.
fn fill(buf: &mut Buffer, area: Rect, palette: &Palette) {
    let bg = Style::new().bg(palette.token(palette.theme.panel));
    for y in area.top()..area.bottom() {
        for x in area.left()..area.right() {
            let cell = &mut buf[(x, y)];
            cell.reset();
            cell.set_char(' ').set_style(bg);
        }
    }
}

/// The colour of a notice's edge: the warning for traders, the bad one for a death and the
/// accent for the rest (what the window does).
fn edge(kind: Kind, palette: &Palette) -> Style {
    let t = &palette.theme;
    let rgb = match kind {
        Kind::Death => t.bad,
        Kind::Trade => t.warn,
        _ => t.accent,
    };
    Style::new().fg(palette.token(rgb)).bg(palette.token(t.panel))
}

fn draw_box(
    face: &Face,
    body: usize,
    area: Rect,
    palette: &Palette,
    marks: &Marks,
    buf: &mut Buffer,
) {
    fill(buf, area, palette);
    let line = edge(face.kind, palette);
    let (l, r, t, b) = (area.left(), area.right() - 1, area.top(), area.bottom() - 1);
    for x in l + 1..r {
        buf[(x, t)].set_char(marks.h).set_style(line);
        buf[(x, b)].set_char(marks.h).set_style(line);
    }
    for y in t + 1..b {
        buf[(l, y)].set_char(marks.v).set_style(line);
        buf[(r, y)].set_char(marks.v).set_style(line);
    }
    for (x, y, c) in
        [(l, t, marks.box_tl), (r, t, marks.box_tr), (l, b, marks.box_bl), (r, b, marks.box_br)]
    {
        buf[(x, y)].set_char(c).set_style(line);
    }
    let room = usize::from(area.width).saturating_sub(4);
    let x = l + 2;
    let mut y = t + 1;
    buf.set_stringn(x, y, &face.title, room, Tone::Strong.style(palette));
    for text in face.body.iter().take(body) {
        y += 1;
        buf.set_stringn(x, y, text, room, Tone::Text.style(palette));
    }
    // The hints, key first in the accent and the word after it, split by dots; with no room for
    // them all, whole ones from the left (the dismiss key last, so it is the first to go).
    y += 1;
    let (key, word, dim) = (Tone::Title.style(palette), Tone::Text.style(palette), Tone::Dim);
    let sep = if marks.ascii { " - " } else { " · " };
    let mut at = 0usize;
    for (i, (k, w)) in face.hints.iter().enumerate() {
        let lead = if i == 0 { 0 } else { sep.chars().count() };
        let need = lead + k.chars().count() + 1 + w.chars().count();
        if at + need > room {
            break;
        }
        if i > 0 {
            buf.set_string(x + at as u16, y, sep, dim.style(palette));
        }
        at += lead;
        buf.set_string(x + at as u16, y, k, key);
        at += k.chars().count() + 1;
        buf.set_string(x + at as u16, y, w, word);
        at += w.chars().count();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn as_many_boxes_fit_as_the_rows_allow_keeping_a_row_for_the_more_line() {
        // Two boxes of 5 rows, nothing more behind them.
        assert_eq!(fitting(&[5, 5], 2, 10), 2);
        assert_eq!(fitting(&[5, 5], 2, 9), 1);
        // With more behind, the second must leave a row for "+N more".
        assert_eq!(fitting(&[5, 5], 3, 10), 1);
        assert_eq!(fitting(&[5, 5], 3, 11), 2);
        assert_eq!(fitting(&[5, 5], 3, 4), 0);
    }
}
