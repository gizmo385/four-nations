//! The meter as a bar (docs/design/egui-frontend.md, "The look"): where the terminal draws `█`
//! and `░` cells, a page draws a filled rectangle on an inset track, `cells` cells wide and half
//! a cell high, in the tone of the status (`ok`, `warn`, `bad`; the accent for progress). The
//! Nation roster's needs and the Endeavor screen's signs share it, so a bar means the same on
//! both. The side panel's inspector has its own (`egui::ProgressBar` in a row of widgets).

use eframe::egui::{Color32, Painter, Rect, Vec2, pos2, vec2};
use fn_tui::widgets::Tone;

use crate::color::to_egui;
use crate::page::Look;

/// The bar's height as a share of a cell.
const HEIGHT: f32 = 0.5;

/// The empty part of a bar: the theme's `inset`.
pub fn track(look: &Look<'_>) -> Color32 {
    to_egui(look.palette.token(look.palette.theme.inset), Color32::BLACK)
}

/// The colour a tone wears.
pub fn tone_color(look: &Look<'_>, tone: Tone) -> Color32 {
    look.format(tone).color
}

/// The rectangle of a bar `cells` wide whose left end is at `(x, centre_y)`.
pub fn rect(x: f32, centre_y: f32, cells: usize, cell: Vec2) -> Rect {
    Rect::from_center_size(
        pos2(x + cells as f32 * cell.x * 0.5, centre_y),
        vec2(cells as f32 * cell.x, cell.y * HEIGHT),
    )
}

/// Paints `frac` (clamped to `[0, 1]`) of `bar` in `fill` over `track`. A bar that is `low`
/// has its whole track in the warning colour too, as the terminal's empty cells are.
pub fn paint(painter: &Painter, bar: Rect, frac: f32, fill: Color32, track: Color32, low: bool) {
    let track = if low { fill.gamma_multiply(0.35) } else { track };
    painter.rect_filled(bar, 0.0, track);
    let width = bar.width() * frac.clamp(0.0, 1.0);
    if width > 0.0 {
        painter.rect_filled(Rect::from_min_size(bar.min, vec2(width, bar.height())), 0.0, fill);
    }
}
