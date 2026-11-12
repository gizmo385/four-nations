//! The page frame (#881, docs/design/egui-frontend.md, `page` row; tui.md "Full screens"): one
//! shape for every full screen, as `fn_tui::page` draws it in cells.
//!
//! ```text
//! ┏───────────── ◆ BUILD ◆ ─────────────[ Close [Esc] ]┓   the title between two seals, and a
//! │ BUILDING   COST        SIZE   NEEDS                 │   close button that presses `Esc`
//! │ ...                                                 │   the body: the screen's own content
//! ├─────────────────────────────────────────────────────┤   a rule
//! │ Bed                                       5 Wood    │   the detail strip (optional)
//! ┗───────────────────────────── buildings 1-20 of 41 ──┛   the position, and nothing else
//! ```
//!
//! [`show`] covers the whole of the `Ui` it is given (the window above the hint bar; the side
//! panel is not shown at all), draws the frame, and hands the body and the strip each a `Ui` for
//! their rectangle. [`geometry`] says where those are without drawing, for a screen that sizes
//! its strip from the width. The close button is the key it stands for: [`show`] returns
//! whether it was clicked, and the window feeds the shell `Esc`.
//!
//! A selected row is the quiet `select` background with its text drawn bold ([`text_row`]):
//! egui bundles no bold monospace face, so bold is the same glyphs drawn twice, a fraction of a
//! point apart. Never reverse video, so a warning inside the row keeps its colour.
//!
//! (The web page's own glue, `F5` and the `data-fn-*` attributes, is [`crate::webpage`].)

use std::sync::Arc;

use eframe::egui::text::{LayoutJob, TextFormat};
use eframe::egui::{
    self, Align2, Button, Color32, CursorIcon, FontId, Galley, Pos2, Rect, Response, RichText,
    Sense, Stroke, StrokeKind, Ui, UiBuilder, Vec2, WidgetInfo, pos2, vec2,
};
use fn_tui::color::Palette;
use fn_tui::widgets::{Marks, Row, Tone, ToneStyle};

use crate::cells::Metrics;
use crate::color::to_egui;

/// The close button's label: the key it presses, in brackets as the side panel writes keys.
pub const CLOSE: &str = "Close [Esc]";

/// How far the faux bold's second copy sits from the first, in points.
const BOLD_OFFSET: f32 = 0.6;

/// What every widget of a page needs: the font and the theme's colours.
pub struct Look<'a> {
    pub palette: &'a Palette,
    pub font: FontId,
    /// One cell, in points.
    pub cell: Vec2,
    pub panel: Color32,
    /// The frame's lines.
    pub line: Color32,
    /// The selected row's background (`Theme::select`).
    pub select: Color32,
}

impl<'a> Look<'a> {
    pub fn new(palette: &'a Palette, metrics: &Metrics) -> Self {
        let fill = |rgb| to_egui(palette.token(rgb), Color32::BLACK);
        Self {
            palette,
            font: metrics.font.clone(),
            cell: metrics.cell,
            panel: fill(palette.theme.panel),
            line: fill(palette.theme.accent_soft),
            select: fn_tui::page::select_bg(palette)
                .map_or(Color32::TRANSPARENT, |c| to_egui(c, Color32::TRANSPARENT)),
        }
    }

    /// The text format of `tone`: the terminal's colour for it.
    pub fn format(&self, tone: Tone) -> TextFormat {
        let fg = tone.style(self.palette).fg;
        TextFormat {
            font_id: self.font.clone(),
            color: fg.map_or(Color32::WHITE, |c| to_egui(c, Color32::WHITE)),
            ..TextFormat::default()
        }
    }

    /// `spans` as one line of text.
    pub fn job(&self, spans: &[(String, Tone)]) -> LayoutJob {
        let mut job = LayoutJob::default();
        for (text, tone) in spans {
            job.append(text, 0.0, self.format(*tone));
        }
        job
    }
}

/// Where a page's parts are, in points.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Geometry {
    /// The frame's outline: the window, less half a cell.
    pub frame: Rect,
    /// Inside the border and two cells of padding each side: the body and the strip together.
    pub inner: Rect,
    pub body: Rect,
    /// The detail strip under the rule, as wide as the body, when it has rows.
    pub strip: Option<Rect>,
    /// The rule's height, when there is a strip.
    pub rule_y: f32,
}

impl Geometry {
    /// How many cells wide the body is.
    pub fn cols(&self, cell: Vec2) -> usize {
        (self.inner.width() / cell.x).floor().max(0.0) as usize
    }

    /// How many rows the page has between its top and bottom edges, for sizing the strip.
    pub fn rows(&self, cell: Vec2) -> usize {
        (self.frame.height() / cell.y).floor().max(0.0) as usize
    }
}

/// The parts of a page over `outer`, with `strip_rows` rows of detail (none: no strip). The
/// strip shrinks if the page is too short to leave the body a row, as the terminal's does.
pub fn geometry(outer: Rect, cell: Vec2, strip_rows: usize) -> Geometry {
    let frame = outer.shrink2(cell * 0.5);
    let inner = Rect::from_min_max(
        pos2(frame.left() + cell.x * 2.0, frame.top() + cell.y),
        pos2(frame.right() - cell.x * 2.0, frame.bottom() - cell.y * 0.75),
    );
    let room = ((inner.height() / cell.y).floor() - 2.0).max(0.0) as usize;
    let rows = strip_rows.min(room);
    if rows == 0 {
        return Geometry { frame, inner, body: inner, strip: None, rule_y: inner.bottom() };
    }
    let strip_top = inner.bottom() - rows as f32 * cell.y;
    let rule_y = strip_top - cell.y * 0.5;
    Geometry {
        frame,
        inner,
        body: Rect::from_min_max(inner.min, pos2(inner.right(), rule_y - cell.y * 0.25)),
        strip: Some(Rect::from_min_max(pos2(inner.left(), strip_top), inner.max)),
        rule_y,
    }
}

/// What [`show`] says about the frame this pass.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Outcome {
    /// The close button was clicked: the key it stands for is `Esc`.
    pub close: bool,
}

/// What to put in the page.
pub struct Spec<'a> {
    /// The screen's title, upper-cased between two seals.
    pub title: &'a str,
    /// The rows of the detail strip (0: none).
    pub strip_rows: usize,
}

/// Draws the page over all of `ui`'s free rectangle. `body` and `detail` fill their rectangles;
/// `body` returns the position text for the bottom edge (`None`: nothing, a screen whose rows
/// all fit).
pub fn show(
    ui: &mut Ui,
    look: &Look<'_>,
    spec: &Spec<'_>,
    body: impl FnOnce(&mut Ui) -> Option<String>,
    detail: impl FnOnce(&mut Ui),
) -> Outcome {
    let outer = ui.available_rect_before_wrap();
    let geo = geometry(outer, look.cell, spec.strip_rows);
    let painter = ui.painter_at(outer);
    painter.rect_filled(outer, 0.0, look.panel);
    let stroke = Stroke::new(2.0, look.line);
    painter.rect_stroke(geo.frame, 0.0, stroke, StrokeKind::Middle);
    if geo.strip.is_some() {
        painter.hline(geo.frame.x_range(), geo.rule_y, stroke);
    }
    crate::style::corner_brackets(
        &painter,
        geo.frame,
        to_egui(look.palette.token(look.palette.theme.accent), Color32::WHITE),
    );

    let marks = Marks::new(look.palette.ascii);
    let heading = format!(" {0} {1} {0} ", marks.seal, spec.title.to_uppercase());
    on_edge(ui, look, geo.frame.center_top(), Align2::CENTER_CENTER, &heading, Tone::Title);

    // The close button sits on the top edge, right.
    let mut out = Outcome::default();
    let width = (CLOSE.chars().count() + 2) as f32 * look.cell.x;
    let at = Rect::from_min_size(
        pos2(geo.frame.right() - width - look.cell.x * 2.0, geo.frame.top() - look.cell.y * 0.5),
        vec2(width, look.cell.y),
    );
    ui.scope_builder(UiBuilder::new().max_rect(at), |ui| {
        let text = RichText::new(CLOSE).font(look.font.clone());
        let button = Button::new(text).fill(look.panel).min_size(at.size());
        out.close = ui.add(button).on_hover_cursor(CursorIcon::PointingHand).clicked();
    });

    let position = ui
        .scope_builder(UiBuilder::new().max_rect(geo.body), |ui| {
            ui.set_clip_rect(geo.body.intersect(ui.clip_rect()));
            ui.spacing_mut().item_spacing = Vec2::ZERO;
            body(ui)
        })
        .inner;
    if let Some(text) = position.filter(|t| !t.is_empty()) {
        let at = pos2(geo.frame.right() - look.cell.x * 2.0, geo.frame.bottom());
        on_edge(ui, look, at, Align2::RIGHT_CENTER, &format!(" {text} "), Tone::Dim);
    }
    if let Some(strip) = geo.strip {
        ui.scope_builder(UiBuilder::new().max_rect(strip), |ui| {
            ui.set_clip_rect(strip.intersect(ui.clip_rect()));
            ui.spacing_mut().item_spacing = Vec2::ZERO;
            detail(ui);
        });
    }
    ui.advance_cursor_after_rect(outer);
    out
}

/// `text` on the frame's edge at `at`, on the panel's colour so the line breaks round it.
fn on_edge(ui: &Ui, look: &Look<'_>, at: Pos2, anchor: Align2, text: &str, tone: Tone) {
    let painter = ui.painter();
    let galley = painter.layout_job(look.job(&[(text.to_owned(), tone)]));
    let rect = anchor.anchor_size(at, galley.size());
    painter.rect_filled(rect, 0.0, look.panel);
    bold(painter, rect.min, galley);
    let text = text.trim().to_owned();
    ui.interact(rect, ui.scope_id().with(("page edge", text.clone())), Sense::hover())
        .widget_info(|| WidgetInfo::labeled(egui::Role::Label, true, text.clone()));
}

/// `galley` at `at`, drawn twice: faux bold.
fn bold(painter: &egui::Painter, at: Pos2, galley: Arc<Galley>) {
    painter.galley(at, galley.clone(), Color32::WHITE);
    painter.galley(at + vec2(BOLD_OFFSET, 0.0), galley, Color32::WHITE);
}

/// `galley` at `at`, bold or plain: for a screen that lays out its own (wrapped) text.
pub fn paint_galley(painter: &egui::Painter, at: Pos2, galley: Arc<Galley>, bold_text: bool) {
    if bold_text {
        bold(painter, at, galley);
    } else {
        painter.galley(at, galley, Color32::WHITE);
    }
}

/// `row`'s text painted into `rect` (one cell high): its left spans, and its right-hand text (if
/// any) flush right, bold when `bold`. Returns the row's words as the accessibility tree has
/// them. The background is the caller's.
pub fn paint_row(
    painter: &egui::Painter,
    look: &Look<'_>,
    row: &Row,
    rect: Rect,
    bold_text: bool,
) -> String {
    let draw = |galley: Arc<Galley>, at: Pos2| {
        if bold_text {
            bold(painter, at, galley);
        } else {
            painter.galley(at, galley, Color32::WHITE);
        }
    };
    let top = rect.center().y - look.cell.y * 0.5;
    draw(painter.layout_job(look.job(&row.left)), pos2(rect.left(), top));
    let mut text: String = row.left.iter().map(|(t, _)| t.as_str()).collect();
    if let Some((right, tone)) = &row.right {
        let galley = painter.layout_job(look.job(&[(right.clone(), *tone)]));
        let x = rect.right() - galley.size().x;
        draw(galley, pos2(x, top));
        text.push_str("  ");
        text.push_str(right);
    }
    text
}

/// One row of toned text across the free width of `ui`: its left spans, and its right-hand
/// text (if any) flush right. `selected` gives it the `select` background and bold text.
/// `sense` is `Sense::click()` for a row that can be picked. The row is exposed to the
/// accessibility tree as one node holding its text, so a screen reader and the kittest tests
/// can find and click it.
pub fn text_row(ui: &mut Ui, look: &Look<'_>, row: &Row, selected: bool, sense: Sense) -> Response {
    let (rect, response) = ui.allocate_exact_size(vec2(ui.available_width(), look.cell.y), sense);
    let painter = ui.painter_at(rect);
    let clickable = sense.senses_click();
    if selected {
        painter.rect_filled(rect, 0.0, look.select);
    } else if clickable && response.hovered() {
        painter.rect_filled(rect, 0.0, look.select.gamma_multiply(0.5));
    }
    if clickable && response.hovered() {
        ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
    }
    let text = paint_row(&painter, look, row, rect, selected);
    let role = if clickable { egui::Role::Button } else { egui::Role::Label };
    response.widget_info(|| WidgetInfo::labeled(role, true, text.clone()));
    response
}
