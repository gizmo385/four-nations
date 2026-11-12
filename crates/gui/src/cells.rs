//! The cell painter (#872): a `ratatui::buffer::Buffer` painted with egui shapes, in two steps.
//!
//! 1. [`CellLayout::new`] walks the buffer once, with no egui context: one background run per
//!    stretch of equal background in a row, and one glyph per cell that shows something. It is
//!    pure, so a test can compare its glyph grid with the buffer (and with `fn_tui --frame`'s
//!    text) and a bench can time it alone.
//! 2. [`paint`] turns the layout into shapes: one mesh holding a rectangle per background run,
//!    then one text shape per glyph, in the bundled monospace font ([`crate::fonts`]), with
//!    colours through [`crate::color`].
//!
//! This replaces `egui_ratatui` + `soft_ratatui`, which rasterised every cell on the CPU and
//! uploaded a texture each frame (5 to 7 ms in release at the spike's size). Here the GPU
//! draws the glyphs from egui's own font atlas, and the CPU only builds shapes.
//!
//! Each glyph is centred in its cell, whatever font it fell back to and whatever its advance, so
//! nothing drifts off the grid; a wide glyph (an emoji, say) is centred over the two cells it
//! covers. Bold has no counterpart (egui bundles no bold monospace), so it is not drawn;
//! `REVERSED`, `DIM`, `UNDERLINED` and `CROSSED_OUT` are.

use eframe::egui::epaint::text::FontsView;
use eframe::egui::{
    self, Color32, FontFamily, FontId, Mesh, Painter, Pos2, Rect, Shape, Stroke, Vec2, pos2, vec2,
};
use ratatui::buffer::{Buffer, Cell};
use ratatui::style::Modifier;
use unicode_width::UnicodeWidthStr;

use crate::color::to_egui;

/// The terminal's own default colours, for a cell whose colour is `Color::Reset`.
pub const DEFAULT_BG: Color32 = Color32::BLACK;
pub const DEFAULT_FG: Color32 = Color32::from_rgb(204, 204, 204);

/// The point size of the font the cells are drawn in: with Hack, an 8 by 17 point cell.
pub const FONT_SIZE: f32 = 14.0;

/// How big a cell is, and the font its glyph is drawn in.
#[derive(Clone, Debug)]
pub struct Metrics {
    /// One cell, in points, rounded to whole physical pixels so neighbouring rectangles meet
    /// exactly.
    pub cell: Vec2,
    pub font: FontId,
}

impl Metrics {
    /// The metrics for `ctx`'s monospace font at [`FONT_SIZE`] and its current scale.
    pub fn new(ctx: &egui::Context) -> Self {
        Self::at(ctx, FONT_SIZE)
    }

    /// As [`Metrics::new`], at `size` points.
    pub fn at(ctx: &egui::Context, size: f32) -> Self {
        let font = FontId::new(size, FontFamily::Monospace);
        let ppp = ctx.pixels_per_point();
        let (advance, height) = ctx.fonts_mut(|f| (f.glyph_width(&font, 'M'), f.row_height(&font)));
        let snap = |v: f32| ((v * ppp).round().max(1.0)) / ppp;
        Self { cell: vec2(snap(advance), snap(height)), font }
    }

    /// How many whole cells fit in `size`.
    pub fn grid_in(&self, size: Vec2) -> (u16, u16) {
        let cols = (size.x / self.cell.x).floor().clamp(0.0, f32::from(u16::MAX));
        let rows = (size.y / self.cell.y).floor().clamp(0.0, f32::from(u16::MAX));
        (cols as u16, rows as u16)
    }
}

/// A run of cells in one row with the same background.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BgRun {
    pub row: u16,
    pub col: u16,
    pub len: u16,
    pub color: Color32,
}

/// One cell's glyph.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Glyph<'a> {
    pub row: u16,
    pub col: u16,
    /// How many cells it covers: 2 for a wide glyph.
    pub width: u16,
    pub symbol: &'a str,
    pub fg: Color32,
    pub underline: bool,
    pub strike: bool,
}

/// What a buffer looks like, as runs and glyphs.
#[derive(Debug, Default)]
pub struct CellLayout<'a> {
    pub cols: u16,
    pub rows: u16,
    /// Backgrounds that differ from [`DEFAULT_BG`] (the base rectangle covers the rest).
    pub backgrounds: Vec<BgRun>,
    pub glyphs: Vec<Glyph<'a>>,
}

/// The colours a cell shows: foreground and background, with `REVERSED` and `DIM` applied.
fn colours(cell: &Cell) -> (Color32, Color32) {
    let mut fg = to_egui(cell.fg, DEFAULT_FG);
    let mut bg = to_egui(cell.bg, DEFAULT_BG);
    if cell.modifier.contains(Modifier::REVERSED) {
        std::mem::swap(&mut fg, &mut bg);
    }
    if cell.modifier.contains(Modifier::DIM) {
        fg = fg.lerp_to_gamma(bg, 0.4);
    }
    (fg, bg)
}

impl<'a> CellLayout<'a> {
    pub fn new(buf: &'a Buffer) -> Self {
        let area = buf.area;
        let mut layout = CellLayout {
            cols: area.width,
            rows: area.height,
            backgrounds: Vec::new(),
            glyphs: Vec::with_capacity(buf.content.len() / 3),
        };
        let width = usize::from(area.width);
        if width == 0 {
            return layout;
        }
        for (row, cells) in buf.content.chunks(width).enumerate() {
            let row = row as u16;
            let mut run: Option<BgRun> = None;
            let mut skip_to = 0;
            for (col, cell) in cells.iter().enumerate() {
                let col = col as u16;
                let (fg, bg) = colours(cell);
                match &mut run {
                    Some(r) if r.color == bg => r.len += 1,
                    _ => {
                        layout.backgrounds.extend(run.take().filter(|r| r.color != DEFAULT_BG));
                        run = Some(BgRun { row, col, len: 1, color: bg });
                    }
                }
                let symbol = cell.symbol();
                // The cells a wide glyph covers hold nothing of their own; a blank shows only
                // its background.
                if col < skip_to || symbol.trim().is_empty() {
                    continue;
                }
                let span = if symbol.is_ascii() { 1 } else { symbol.width().clamp(1, 2) as u16 };
                skip_to = col + span;
                layout.glyphs.push(Glyph {
                    row,
                    col,
                    width: span,
                    symbol,
                    fg,
                    underline: cell.modifier.contains(Modifier::UNDERLINED),
                    strike: cell.modifier.contains(Modifier::CROSSED_OUT),
                });
            }
            layout.backgrounds.extend(run.take().filter(|r| r.color != DEFAULT_BG));
        }
        layout
    }

    /// The glyph grid as text, the way `fn_tui --frame` prints a buffer: one line per row, a
    /// cell with no glyph a space, trailing spaces trimmed. A test compares this with the
    /// buffer's own text, so the painter provably draws what the terminal shows.
    pub fn text(&self) -> String {
        let mut rows: Vec<Vec<&str>> =
            vec![vec![" "; usize::from(self.cols)]; usize::from(self.rows)];
        for g in &self.glyphs {
            rows[usize::from(g.row)][usize::from(g.col)] = g.symbol;
        }
        let mut out = String::new();
        for row in rows {
            out.push_str(row.concat().trim_end());
            out.push('\n');
        }
        out
    }
}

impl BgRun {
    fn rect(&self, origin: Pos2, cell: Vec2) -> Rect {
        let min = origin + vec2(f32::from(self.col) * cell.x, f32::from(self.row) * cell.y);
        Rect::from_min_size(min, vec2(f32::from(self.len) * cell.x, cell.y))
    }
}

/// The shapes for `layout` with its top-left corner at `origin`, in paint order: the base
/// rectangle (`DEFAULT_BG` over the whole grid), a mesh of the background runs, then the glyphs.
pub fn shapes(
    fonts: &mut FontsView<'_>,
    origin: Pos2,
    metrics: &Metrics,
    layout: &CellLayout<'_>,
) -> Vec<Shape> {
    let cell = metrics.cell;
    let grid = Rect::from_min_size(
        origin,
        vec2(f32::from(layout.cols) * cell.x, f32::from(layout.rows) * cell.y),
    );
    let mut out = Vec::with_capacity(layout.glyphs.len() + 2);
    out.push(Shape::rect_filled(grid, 0.0, DEFAULT_BG));

    let mut mesh = Mesh::default();
    mesh.reserve_triangles(layout.backgrounds.len() * 2);
    mesh.reserve_vertices(layout.backgrounds.len() * 4);
    for run in &layout.backgrounds {
        mesh.add_colored_rect(run.rect(origin, cell), run.color);
    }
    out.push(Shape::mesh(mesh));

    for g in &layout.glyphs {
        let galley = fonts.layout_no_wrap(g.symbol.to_owned(), metrics.font.clone(), g.fg);
        let span = vec2(f32::from(g.width) * cell.x, cell.y);
        let at = origin + vec2(f32::from(g.col) * cell.x, f32::from(g.row) * cell.y);
        out.push(Shape::galley(at + (span - galley.size()) / 2.0, galley, g.fg));
        if g.underline || g.strike {
            let y = at.y + if g.underline { cell.y - 1.5 } else { cell.y / 2.0 };
            out.push(Shape::line_segment(
                [pos2(at.x, y), pos2(at.x + span.x, y)],
                Stroke::new(1.0, g.fg),
            ));
        }
    }
    out
}

/// Paints `layout` through `painter` (see [`shapes`]).
pub fn paint(painter: &Painter, origin: Pos2, metrics: &Metrics, layout: &CellLayout<'_>) {
    let shapes = painter.fonts_mut(|fonts| shapes(fonts, origin, metrics, layout));
    painter.extend(shapes);
}
