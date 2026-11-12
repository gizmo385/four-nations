//! The confirm box (#655): every question that can lose progress (a new map, a switch of nation,
//! quit, a load, a quickload, a save over a slot) is asked in a centred box, in the Seal frame the
//! menu uses, and not in the hint bar (`crate::modal`).
//!
//! The box says what happens, says how long ago the game was last saved (in game time, from
//! [`UiState::saved_at`](crate::state::UiState::saved_at): `fn_ui` never reads the wall clock) or
//! that it never was, and lists its answers as rows. The cursor starts on the first, which is
//! always the safe one, and `Esc` picks it too; a bare `y` does nothing (`App::prompt_key`).
//! Where saving first makes sense (a new map, a switch, quit) the second row is "Save, then …",
//! which writes a new slot (`App::save_new_slot`) and only then goes on.

use crate::color::Palette;
use crate::modal;
use crate::widgets::{Marks, Painter, Row, Tone};
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};

// What the box says is `fn_ui`'s (#857); this module draws it.
pub use fn_ui::ask::*;

/// How wide the box would like to be; it shrinks to fit a small terminal (`modal::modal_area`).
const WIDTH: u16 = 56;

/// The box's target size: the question, a blank, what follows, a blank, the answers.
fn size(ask: &Ask) -> (u16, u16) {
    let rows = 1 + 1 + ask.lines.len() + 1 + ask.answers.len();
    (WIDTH, rows as u16 + 2)
}

/// Draws the box over the map area `outer`, the answer at `at` reversed.
pub fn draw(ask: &Ask, at: usize, palette: &Palette, outer: Rect, buf: &mut Buffer) {
    let marks = Marks::new(palette.ascii);
    let title = format!(" {0} {1} {0} ", marks.seal, ask.title);
    let inner = modal::draw(buf, outer, size(ask), palette, &title, "");
    let mut p = Painter::new(buf, inner, palette);
    p.row(&Row::text(ask.question.clone(), Tone::Strong));
    p.blank();
    for line in &ask.lines {
        p.row(&Row::text(line.clone(), Tone::Text));
    }
    p.blank();
    let mut reversed = None;
    for (i, (_, label)) in ask.answers.iter().enumerate() {
        let chosen = i == at;
        if chosen {
            reversed = Some(inner.y + p.row);
        }
        let pointer = if chosen { marks.pointer } else { " " };
        let tone = if chosen { Tone::Title } else { Tone::Text };
        p.row(&Row::text(format!("{pointer} {label}"), tone));
    }
    if let Some(y) = reversed {
        for x in inner.x..inner.right() {
            buf[(x, y)].set_style(Style::new().add_modifier(Modifier::REVERSED));
        }
    }
}
