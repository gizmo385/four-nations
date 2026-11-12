//! The LOST TEACHING box (#743, docs/design/horizons-phase-5.md, "Mockups"): a #655-style
//! centred box over the map, in the Seal frame the confirm box uses. What it says and what its
//! rows do is `fn_ui::teaching`'s; this module draws it.
//!
//! "Decide later" is the first row and where the cursor starts. Each `Keep …` row has its cost
//! flush right, then its unlock line and the sim's reason line, indented under it. The chosen
//! row's first line is reversed, as the confirm box's is.

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};

use crate::color::Palette;
use crate::modal;
use crate::widgets::{Marks, Painter, Row, Tone, wrap};

pub use fn_ui::teaching::*;

/// How wide the box would like to be; it shrinks to fit a small terminal (`modal::modal_area`).
const WIDTH: u16 = 76;

/// The box's rows, wrapped to `width`, and which row each answer's first line is on.
fn rows(draft: &Draft, at: usize, marks: &Marks, width: usize) -> (Vec<Row>, Vec<usize>) {
    let width = width.max(1);
    let mut out = Vec::new();
    for line in &draft.lines {
        for piece in wrap(line, width, usize::MAX, marks.ellipsis) {
            out.push(Row::text(piece, Tone::Text));
        }
    }
    out.push(Row::text(String::new(), Tone::Text));
    let pointer = |i: usize| if i == at { marks.pointer } else { " " };
    let tone = |i: usize| if i == at { Tone::Title } else { Tone::Text };
    let mut starts = vec![out.len()];
    out.push(Row::text(format!("{} Decide later", pointer(0)), tone(0)));
    for (k, keep) in draft.keeps.iter().enumerate() {
        let i = k + 1;
        starts.push(out.len());
        out.push(Row::pair(
            format!("{} {}", pointer(i), keep.label),
            tone(i),
            &keep.cost,
            Tone::Dim,
        ));
        let indent = "    ";
        let room = width.saturating_sub(indent.len()).max(1);
        for piece in wrap(&keep.unlock, room, usize::MAX, marks.ellipsis) {
            out.push(Row::text(format!("{indent}{piece}"), Tone::Text));
        }
        if let Some(reason) = &keep.reason {
            for piece in wrap(reason, room, usize::MAX, marks.ellipsis) {
                out.push(Row::text(format!("{indent}{piece}"), Tone::Warn));
            }
        }
    }
    (out, starts)
}

/// The box's own rectangle when drawn over `outer` (`modal::modal_area`).
pub fn area(draft: &Draft, at: usize, ascii: bool, outer: Rect) -> Rect {
    let width = modal::modal_area(outer, (WIDTH, 1)).width;
    let (rows, _) = rows(draft, at, &Marks::new(ascii), usize::from(width.saturating_sub(4)));
    modal::modal_area(outer, (WIDTH, rows.len() as u16 + 2))
}

/// Draws the box over the map area `outer`, the row at `at` chosen.
pub fn draw(draft: &Draft, at: usize, palette: &Palette, outer: Rect, buf: &mut Buffer) {
    let marks = Marks::new(palette.ascii);
    let width = modal::modal_area(outer, (WIDTH, 1)).width;
    let (rows, starts) = rows(draft, at, &marks, usize::from(width.saturating_sub(4)));
    let title = format!(" {0} {1} {0} ", marks.seal, draft.title);
    let height = u16::try_from(rows.len() + 2).unwrap_or(u16::MAX);
    let inner = modal::draw(buf, outer, (WIDTH, height), palette, &title, "");
    let mut p = Painter::new(buf, inner, palette);
    for row in &rows {
        p.row(row);
    }
    if let Some(&start) = starts.get(at) {
        let y = inner.y + start as u16;
        if y < inner.bottom() {
            for x in inner.x..inner.right() {
                buf[(x, y)].set_style(Style::new().add_modifier(Modifier::REVERSED));
            }
        }
    }
}
