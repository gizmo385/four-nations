//! The embark screen as egui widgets (#883, docs/design/egui-frontend.md, row 7): the survey on
//! square tiles in the middle, the site report as a native side panel on the right. The state,
//! the keys and the words are `fn_tui::embark::EmbarkScreen`'s and `fn_ui::embark`'s; this is a
//! second painter for them and adds no state of the screen's own, only the view of it (where the
//! map sits in its rectangle).
//!
//! Every click is a key press the screen already answers ([`Keys`], docs/design/egui-frontend.md,
//! "Mouse"), handed to the shell after the pass like the title's and the side panel's
//! (`Frame::take_keys`), so a click path and a keyboard path are the same commands:
//!
//! | Click | Keys |
//! |---|---|
//! | a block of the map | the arrows (`Shift` for ten) from the cursor's block to it |
//! | a block, twice | the same, then `Enter` (zoom in; at the closest zoom, settle) |
//! | `Zoom in` / `Zoom out` | `+` / `-` |
//! | `Settle` / `New seed` / `Back` / `Help` | `E` / `R` / `Esc` / `?` |
//! | the settle question's `Settle here` / `Stay` | `Y` / `N` |
//!
//! The map is the same look as the game's own ([`crate::mapview`]): a glyph centred in each
//! tile, one flat block per tile once tiles are too small to read, the cursor outlined. Its tiles
//! are blocks of the survey (`fn_tui::embark::MapFrame`): at the overview the whole map is fitted
//! to the window, at a closer zoom the view follows the cursor.

use eframe::egui::text::{LayoutJob, TextFormat};
use eframe::egui::{
    self, Align, Align2, Button, Color32, CursorIcon, FontFamily, FontId, Label, Layout, Margin,
    Mesh, Rect, RichText, Sense, Shape, Stroke, StrokeKind, Ui, Vec2, pos2, vec2,
};
use fn_sim::{Defs, Pos};
use fn_tui::color::Palette;
use fn_tui::embark::{EmbarkScreen, MapFrame};
use fn_tui::widgets::{Marks, Row, Tone, ToneStyle};
use fn_ui::embark as words;
use fn_ui::keyev::{KeyCode, KeyEvent, KeyModifiers};

use crate::camera::GLYPH_MIN;
use crate::cells::Metrics;
use crate::color::to_egui;

/// The keys a frame's clicks stand for, in order.
pub type Keys = Vec<KeyEvent>;

/// A tile's side at the zoomed levels, in points; the overview fits the whole map instead, from
/// [`MIN_TILE`] to [`MAX_TILE`].
pub const ZOOMED_TILE: f32 = 22.0;
pub const MIN_TILE: f32 = 3.0;
pub const MAX_TILE: f32 = 32.0;
/// How many tiles from the map's edge the cursor may come before the view follows it.
const FOLLOW_MARGIN: f32 = 2.0;
/// The panel's padding, in points.
const PAD: i8 = 8;

fn press(code: KeyCode) -> KeyEvent {
    KeyEvent::new(code, KeyModifiers::NONE)
}

fn key(c: char) -> KeyEvent {
    press(KeyCode::Char(c))
}

// -----------------------------------------------------------------------------------------
// Click -> keys
// -----------------------------------------------------------------------------------------

/// `Right`/`Left` (or `Down`/`Up`) presses that move `by` blocks, ten at a time with `Shift`.
fn moves(by: i32, plus: KeyCode, minus: KeyCode) -> Keys {
    let (code, n) = if by >= 0 { (plus, by) } else { (minus, -by) };
    let tens = (0..n / 10).map(|_| KeyEvent::new(code, KeyModifiers::SHIFT));
    let ones = (0..n % 10).map(|_| press(code));
    tens.chain(ones).collect()
}

/// The keys a click on block `to` presses, with the cursor in block `from`: the arrows between
/// them. The screen clamps at the map's edge exactly as it does for the same keys typed.
pub fn click_keys(from: Pos, to: Pos) -> Keys {
    let mut keys = moves(to.x - from.x, KeyCode::Right, KeyCode::Left);
    keys.extend(moves(to.y - from.y, KeyCode::Down, KeyCode::Up));
    keys
}

// -----------------------------------------------------------------------------------------
// Where the map sits
// -----------------------------------------------------------------------------------------

/// Where the blocks of the map are on screen this frame.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Geometry {
    /// The map's rectangle: what a click is taken in.
    pub rect: Rect,
    /// A tile's side, in points. Tiles are square.
    pub tile: f32,
    /// The top-left corner of block `(0, 0)`.
    pub origin: egui::Pos2,
    /// The survey's size in blocks.
    pub blocks: (i32, i32),
}

impl Geometry {
    /// The on-screen rectangle of block `(bx, by)`.
    pub fn block_rect(&self, bx: i32, by: i32) -> Rect {
        let at = self.origin + vec2(bx as f32, by as f32) * self.tile;
        Rect::from_min_size(at, Vec2::splat(self.tile))
    }

    /// The block under `p`, if it is on the map and inside the rectangle.
    pub fn block_at(&self, p: egui::Pos2) -> Option<Pos> {
        if !self.rect.contains(p) {
            return None;
        }
        let d = (p - self.origin) / self.tile;
        let (bx, by) = (d.x.floor() as i32, d.y.floor() as i32);
        let (bw, bh) = self.blocks;
        (bx >= 0 && by >= 0 && bx < bw && by < bh).then(|| Pos::new(bx, by))
    }

    /// The blocks at least partly in the rectangle, as `(x0, y0, x1, y1)` (end exclusive).
    fn visible(&self) -> (i32, i32, i32, i32) {
        let (bw, bh) = self.blocks;
        let lo = (self.rect.min - self.origin) / self.tile;
        let hi = (self.rect.max - self.origin) / self.tile;
        (
            (lo.x.floor() as i32).clamp(0, bw),
            (lo.y.floor() as i32).clamp(0, bh),
            (hi.x.ceil() as i32).clamp(0, bw),
            (hi.y.ceil() as i32).clamp(0, bh),
        )
    }
}

/// What the window keeps of the map between frames: which block the view is centred on.
#[derive(Default)]
pub struct EmbarkView {
    /// The block (fractional) at the middle of the rectangle, at the zoomed levels.
    centre: Vec2,
    /// The scale `centre` was set at: a new zoom level recentres on the cursor.
    scale: i32,
    /// Where the map was last drawn, for tests.
    pub geometry: Option<Geometry>,
}

impl EmbarkView {
    /// The geometry of the map in `rect` for a survey of `blocks` at `scale` with the cursor in
    /// block `cursor`; follows the cursor when it leaves the rectangle.
    fn place(
        &mut self,
        rect: Rect,
        blocks: (i32, i32),
        scale: i32,
        overview: bool,
        cursor: Pos,
    ) -> Geometry {
        let (bw, bh) = (blocks.0 as f32, blocks.1 as f32);
        if overview {
            // The whole survey, centred, on whole points.
            let tile =
                (rect.width() / bw).min(rect.height() / bh).floor().clamp(MIN_TILE, MAX_TILE);
            let size = vec2(bw, bh) * tile;
            let origin = rect.center() - size / 2.0;
            let origin = pos2(origin.x.round(), origin.y.round());
            self.scale = scale;
            return Geometry { rect, tile, origin, blocks };
        }
        let tile = ZOOMED_TILE;
        let cursor_at = vec2(cursor.x as f32 + 0.5, cursor.y as f32 + 0.5);
        if self.scale != scale {
            self.centre = cursor_at;
            self.scale = scale;
        }
        let geometry = |centre: &mut Vec2| {
            // A map smaller than the rectangle sits in its middle; a bigger one never shows
            // past its edge.
            let clamp = |c: f32, blocks: f32, extent: f32| {
                let half = extent / 2.0 / tile;
                if blocks <= extent / tile { blocks / 2.0 } else { c.clamp(half, blocks - half) }
            };
            centre.x = clamp(centre.x, bw, rect.width());
            centre.y = clamp(centre.y, bh, rect.height());
            let origin = rect.center() - *centre * tile;
            Geometry { rect, tile, origin: pos2(origin.x.round(), origin.y.round()), blocks }
        };
        let mut g = geometry(&mut self.centre);
        let inside = rect
            .shrink(FOLLOW_MARGIN * tile)
            .contains(g.origin + vec2(cursor.x as f32 + 0.5, cursor.y as f32 + 0.5) * tile);
        if !inside {
            self.centre = cursor_at;
            g = geometry(&mut self.centre);
        }
        g
    }
}

// -----------------------------------------------------------------------------------------
// The map
// -----------------------------------------------------------------------------------------

/// Draws the survey into `rect` and returns the keys this frame's clicks press. `live` is false
/// while the settle question or the key help is up: the map shows, and takes no click.
pub fn map(
    ui: &mut Ui,
    view: &mut EmbarkView,
    rect: Rect,
    screen: &EmbarkScreen,
    defs: &Defs,
    palette: &Palette,
    live: bool,
) -> Keys {
    let report = screen.report(defs);
    let frame = screen.map_frame(defs, palette, report.problem.is_some());
    let cursor_block = frame.block_of(screen.cursor);
    let geometry = view.place(rect, frame.blocks, frame.scale, frame.overview, cursor_block);
    view.geometry = Some(geometry);

    let sense = if live { Sense::click() } else { Sense::hover() };
    let response = ui.interact(rect, ui.scope_id().with("embark map"), sense);
    let mut keys = Keys::new();
    if live
        && let Some(p) = response.interact_pointer_pos()
        && let Some(block) = geometry.block_at(p)
    {
        if response.clicked() || response.double_clicked() {
            keys = click_keys(cursor_block, block);
        }
        if response.double_clicked() {
            keys.push(press(KeyCode::Enter));
        }
    }
    if live
        && response.hovered()
        && geometry.block_at(response.hover_pos().unwrap_or_default()).is_some()
    {
        ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
    }
    paint(ui, &geometry, &frame, palette);
    keys
}

fn paint(ui: &Ui, g: &Geometry, frame: &MapFrame<'_>, palette: &Palette) {
    let painter = ui.painter_at(g.rect);
    painter.rect_filled(g.rect, 0.0, Color32::from_rgb(8, 8, 12));
    let colour = |rgb| to_egui(palette.color(rgb), Color32::BLACK);
    let glyphs = g.tile >= GLYPH_MIN;
    let font = FontId::new(g.tile * 0.8, FontFamily::Monospace);
    let (x0, y0, x1, y1) = g.visible();
    let mut mesh = Mesh::default();
    let mut shapes = Vec::new();
    let mut cursor = None;
    for by in y0..y1 {
        for bx in x0..x1 {
            let Some(cell) = frame.cell(bx, by) else { continue };
            let rect = g.block_rect(bx, by);
            let bg = if cell.in_box && !palette.mono() { frame.boxed(cell.bg) } else { cell.bg };
            let (bg, fg) = (colour(bg), colour(cell.fg));
            // Too small to read: one block a tile, in the glyph's own colour where it stands out
            // (the site box's corners).
            mesh.add_colored_rect(rect, if !glyphs && cell.corner { fg } else { bg });
            if glyphs && cell.glyph != ' ' {
                let galley =
                    ui.fonts_mut(|f| f.layout_no_wrap(cell.glyph.to_string(), font.clone(), fg));
                shapes.push(Shape::galley(rect.center() - galley.size() / 2.0, galley, fg));
            }
            if cell.cursor {
                cursor = Some(rect);
            }
        }
    }
    painter.add(Shape::mesh(mesh));
    painter.extend(shapes);
    if let Some(rect) = cursor {
        painter.rect_stroke(
            rect.expand(1.0),
            0.0,
            Stroke::new(2.0_f32, Color32::WHITE),
            StrokeKind::Outside,
        );
    }
}

// -----------------------------------------------------------------------------------------
// The report panel
// -----------------------------------------------------------------------------------------

/// What the row widgets share.
struct Look<'a> {
    palette: &'a Palette,
    font: FontId,
    /// The panel's width in cells: a row whose two halves don't fit takes two lines.
    cols: usize,
    /// A line of text, in points.
    ch: f32,
}

impl Look<'_> {
    fn job(&self, spans: &[(String, Tone)]) -> LayoutJob {
        let mut job = LayoutJob::default();
        for (text, tone) in spans {
            let style = tone.style(self.palette);
            let fg = style.fg.map_or(Color32::WHITE, |c| to_egui(c, Color32::WHITE));
            job.append(
                text,
                0.0,
                TextFormat { font_id: self.font.clone(), color: fg, ..TextFormat::default() },
            );
        }
        job
    }

    fn label(&self, spans: &[(String, Tone)]) -> Label {
        Label::new(self.job(spans)).selectable(false)
    }

    /// One row of toned text: its left spans, and its right-hand text flush right (on a line of
    /// its own when the two don't fit together). An empty row is a blank line.
    fn row(&self, ui: &mut Ui, row: &Row) {
        if row.left.is_empty() && row.right.is_none() {
            ui.add_space(self.ch);
            return;
        }
        let chars = |s: &[(String, Tone)]| s.iter().map(|(t, _)| t.chars().count()).sum::<usize>();
        let right = row.right.as_ref().map_or(0, |(t, _)| t.chars().count() + 1);
        if right > 0 && chars(&row.left) + right > self.cols {
            ui.add(self.label(&row.left));
            if let Some(r) = &row.right {
                ui.with_layout(Layout::right_to_left(Align::Min), |ui| {
                    ui.add(self.label(std::slice::from_ref(r)));
                });
            }
            return;
        }
        ui.horizontal(|ui| {
            ui.add(self.label(&row.left));
            if let Some(r) = &row.right {
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    ui.add(self.label(std::slice::from_ref(r)));
                });
            }
        });
    }
}

/// A flat button in the panel's font; `false` greys it out (a zoom there is no further to go).
fn button(ui: &mut Ui, look: &Look, text: &str, enabled: bool) -> bool {
    let text = RichText::new(text).font(look.font.clone());
    let wide = ui.available_width();
    ui.add_enabled(enabled, Button::new(text).min_size(vec2(wide, look.ch * 1.4))).clicked()
}

/// Draws the report panel into `ui`, the side panel's slot, and returns the keys its buttons
/// press. `live` is false while the settle question or the key help is up.
pub fn side(
    ui: &mut Ui,
    screen: &EmbarkScreen,
    defs: &Defs,
    palette: &Palette,
    metrics: &Metrics,
    live: bool,
) -> Keys {
    let mut keys = Keys::new();
    // The panel scrolls when the window is too short for the report and the buttons (#973).
    let scroll = crate::style::scroll_vertical().id_salt("embark side").auto_shrink([false, false]);
    scroll.show(ui, |ui| {
        egui::Frame::NONE.inner_margin(Margin::symmetric(PAD, PAD / 2)).show(ui, |ui| {
            let cols = metrics.grid_in(vec2(ui.available_width(), 0.0)).0 as usize;
            let look = Look { palette, font: metrics.font.clone(), cols, ch: metrics.cell.y };
            ui.spacing_mut().item_spacing = vec2(0.0, 0.0);
            let marks = Marks::new(palette.ascii);
            ui.vertical_centered(|ui| {
                ui.add(look.label(&[(words::title(&marks), Tone::Title)]));
            });
            ui.add_space(look.ch * 0.5);
            let report = screen.report(defs);
            for row in words::report_rows(&screen.site(), &report, defs, &marks, cols) {
                look.row(ui, &row);
            }
            ui.add_space(look.ch);
            ui.spacing_mut().item_spacing = vec2(0.0, 6.0);
            let overview = screen.is_overview();
            let mut pressed = |text: &str, enabled: bool, k: KeyEvent| {
                if button(ui, &look, text, enabled && live) {
                    keys.push(k);
                }
            };
            pressed("Settle", true, key('e'));
            pressed("Zoom in", screen.can_zoom_in(), key('+'));
            pressed("Zoom out", !overview, key('-'));
            pressed("New seed", true, key('r'));
            pressed("Back", overview, press(KeyCode::Esc));
            pressed("Keys", true, key('?'));
        })
    });
    keys
}

// -----------------------------------------------------------------------------------------
// The settle question and the key help
// -----------------------------------------------------------------------------------------

/// The box over the map while the settle question or the key help is up, and the keys its
/// buttons press: `Y` or `N` for the question, `Esc` closes the help.
pub fn overlay(
    ctx: &egui::Context,
    screen: &EmbarkScreen,
    palette: &Palette,
    metrics: &Metrics,
) -> Keys {
    let (asking, helping) = (screen.asking(), screen.helping());
    let mut keys = Keys::new();
    if !asking && !helping {
        return keys;
    }
    let theme = &palette.theme;
    let colour = |t| to_egui(palette.token(t), Color32::GRAY);
    let look = Look { palette, font: metrics.font.clone(), cols: 80, ch: metrics.cell.y };
    egui::Area::new(egui::Id::unique("embark overlay"))
        .order(egui::Order::Foreground)
        .anchor(Align2::CENTER_CENTER, vec2(0.0, 0.0))
        .show(ctx, |ui| {
            egui::Frame::NONE
                .fill(colour(theme.panel))
                .stroke(Stroke::new(1.0, colour(theme.accent)))
                .inner_margin(Margin::same(14))
                .show(ui, |ui| {
                    ui.spacing_mut().item_spacing = vec2(8.0, 6.0);
                    if asking {
                        let at = screen.cursor;
                        let line = format!("Found the settlement at ({}, {})?", at.x, at.y);
                        ui.add(look.label(&[("SETTLE?".to_owned(), Tone::Title)]));
                        ui.add(look.label(&[(line, Tone::Text)]));
                        ui.horizontal(|ui| {
                            if ui
                                .button(RichText::new("Settle here").font(look.font.clone()))
                                .clicked()
                            {
                                keys.push(key('y'));
                            }
                            if ui.button(RichText::new("Stay").font(look.font.clone())).clicked() {
                                keys.push(key('n'));
                            }
                        });
                    } else {
                        ui.add(look.label(&[(words::HELP_TITLE.to_owned(), Tone::Title)]));
                        for (k, what) in words::HELP {
                            ui.add(look.label(&[
                                (format!("{k:<13}"), Tone::Title),
                                (what.to_owned(), Tone::Text),
                            ]));
                        }
                        if ui.button(RichText::new("Close").font(look.font.clone())).clicked() {
                            keys.push(press(KeyCode::Esc));
                        }
                    }
                });
        });
    keys
}
