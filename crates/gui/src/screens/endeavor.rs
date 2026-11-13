//! The Endeavor screen (`G`, #887): `fn_ui::endeavor::screen_view` in the page frame, as
//! `fn_ui::screens::endeavor` words it for the terminal: the endeavor's name and how many signs
//! are met, its description, the chapter, the stage with what can be done with the gate, then
//! every sign under its heading, each with a mark once met, a bar and `value/target`, and the
//! hint (or the masters' "Closest" and the scrolls found) under it while it isn't.
//!
//! What a window adds: **every sign's progress is a bar** ([`crate::gauge`]) in the `ok` tone
//! once met and the accent while not, and the gate's own share of signs met is one across the
//! top. Long lines wrap to the page instead of being cut. The gate's action (`Enter` to choose
//! a site, `X` to withdraw it) is a button that presses that key.
//!
//! The screen is the shell's own (`UiState::scroll`), so every key works as it does in the
//! terminal. The page scrolls natively (wheel and bar); the arrow and page keys move it too,
//! to the row the shell's scroll says (the terminal's row, which a window's wrapped lines only
//! approximate). The close button is `Esc`.

use eframe::egui::{
    self, Button, CursorIcon, Label, Rect, RichText, ScrollArea, Sense, Ui, UiBuilder, WidgetInfo,
    pos2, vec2,
};
use fn_tui::color::Palette;
use fn_tui::widgets::{Marks, Row, Tone};
use fn_ui::app::App;
use fn_ui::endeavor::{ChapterView, GroupView, SignRow, screen_view};
use fn_ui::gate::{self, GateAction};
use fn_ui::keyev::{KeyCode, KeyEvent, KeyModifiers};
pub use fn_ui::screens::endeavor::*;

use crate::cells::Metrics;
use crate::gauge;
use crate::page::{self, Look, Spec};

/// A sign's bar is this many cells, as in the terminal.
const BAR_CELLS: usize = 10;
/// The `value/target` column after the bar, in cells.
const VALUE_CELLS: usize = 9;
/// The gate's own bar across the top, at most this many cells.
const GATE_CELLS: usize = 48;

fn key(code: KeyCode) -> KeyEvent {
    KeyEvent::new(code, KeyModifiers::NONE)
}

/// The key a gate action's button stands for: `Enter` to choose a site, `X` to withdraw it
/// (the tool key of `Tool::Remove`, as `App` reads the screen's keys).
pub fn action_key(action: GateAction) -> KeyEvent {
    match action {
        GateAction::Place => key(KeyCode::Enter),
        GateAction::Withdraw => key(KeyCode::Char('x')),
    }
}

/// A sign's words for a screen reader and the tests: its name, whether it is met and
/// `value/target`.
pub fn describe(sign: &SignRow) -> String {
    let state = if sign.met { "met" } else { "not met" };
    format!("{}, {state}, {}", sign.name, sign.value)
}

/// `text` wrapped to the page, indented two cells, in `tone`.
fn note(ui: &mut Ui, look: &Look<'_>, text: &str, tone: Tone) {
    let free = ui.available_rect_before_wrap();
    let at = Rect::from_min_max(pos2(free.left() + look.cell.x * 2.0, free.top()), free.max);
    ui.scope_builder(UiBuilder::new().max_rect(at), |ui| {
        let job = look.job(&[(text.to_owned(), tone)]);
        ui.add(Label::new(job).wrap().selectable(false));
    });
}

/// One sign: a mark, its name, a bar and the count in one row, then (while it isn't met) the
/// masters' "Closest", the scrolls found and the hint.
fn sign(ui: &mut Ui, look: &Look<'_>, sign: &SignRow, name_width: usize, marks: &Marks) {
    let cell = look.cell;
    let (rect, response) =
        ui.allocate_exact_size(vec2(ui.available_width(), cell.y), Sense::hover());
    let painter = ui.painter_at(rect);
    let at = |cells: usize| rect.left() + cells as f32 * cell.x;
    let (mark, mark_tone) = if sign.met { (marks.known, Tone::Ok) } else { (" ", Tone::Dim) };
    let name_tone = if sign.met { Tone::Dim } else { Tone::Text };
    let span = |x: usize, width: usize, row: Row| {
        let area =
            Rect::from_min_size(pos2(at(x), rect.top()), vec2(width as f32 * cell.x, cell.y));
        page::paint_row(&painter, look, &row, area, false);
    };
    span(0, 2, Row::text(mark, mark_tone));
    span(2, name_width, Row::text(sign.name.clone(), name_tone));
    let bar_at = 2 + name_width + 1;
    let fill = gauge::tone_color(look, if sign.met { Tone::Ok } else { Tone::Accent });
    let bar = gauge::rect(at(bar_at), rect.center().y, BAR_CELLS, cell);
    gauge::paint(&painter, bar, sign.frac, fill, gauge::track(look), false);
    let value = Row::pair("", Tone::Text, sign.value.clone(), Tone::Dim);
    span(bar_at + BAR_CELLS + 1, VALUE_CELLS, value);
    let label = describe(sign);
    response.widget_info(|| WidgetInfo::labeled(egui::Role::Label, true, label.clone()));

    if sign.met {
        return;
    }
    if let Some(closest) = &sign.closest {
        note(ui, look, closest, Tone::Text);
    }
    for found in &sign.found {
        let line = format!("{} - {} - {}", found.name, found.distance, found.status);
        note(ui, look, &line, Tone::Text);
    }
    if let Some(hint) = &sign.hint {
        note(ui, look, hint, Tone::Warn);
    }
}

fn group(ui: &mut Ui, look: &Look<'_>, group: &GroupView, name_width: usize, marks: &Marks) {
    let heading = Row::text(group.heading.to_uppercase(), Tone::Title);
    page::text_row(ui, look, &heading, false, Sense::hover());
    for s in &group.signs {
        sign(ui, look, s, name_width, marks);
    }
}

fn chapter(ui: &mut Ui, look: &Look<'_>, c: &ChapterView, name_width: usize, marks: &Marks) {
    let blank = Row::default();
    page::text_row(ui, look, &blank, false, Sense::hover());
    page::text_row(ui, look, &Row::text("CHAPTER", Tone::Title), false, Sense::hover());
    if c.met {
        let row =
            Row::spans(vec![(format!("{} ", marks.known), Tone::Ok), (c.name.clone(), Tone::Dim)]);
        page::text_row(ui, look, &row, false, Sense::hover());
        return;
    }
    let head = Row::pair(format!("  {}", c.name), Tone::Strong, c.count.clone(), Tone::Accent);
    page::text_row(ui, look, &head, false, Sense::hover());
    for s in &c.signs {
        sign(ui, look, s, name_width, marks);
    }
    if let Some(hint) = &c.hint {
        note(ui, look, hint, Tone::Dim);
    }
}

/// Draws the screen into `ui` (the window above the hint bar) and returns the keys this
/// frame's clicks stand for.
pub fn show(ui: &mut Ui, app: &App, palette: &Palette, metrics: &Metrics) -> Vec<KeyEvent> {
    let look = Look::new(palette, metrics);
    let marks = Marks::new(palette.ascii);
    let view = screen_view(&app.session);
    let action = gate::action(&app.session);
    let mut keys = Vec::new();

    let chapter_signs = view.chapter.iter().flat_map(|c| &c.signs);
    let name_width = view
        .groups
        .iter()
        .flat_map(|g| &g.signs)
        .chain(chapter_signs)
        .map(|s| s.name.chars().count())
        .max()
        .unwrap_or(0);

    // The page follows the shell's scroll (the arrow and page keys) when that moved, or when
    // the screen has just opened; otherwise the wheel and the bar are in charge.
    let seen = ui.make_persistent_id("endeavor screen: key scroll");
    let frame = ui.ctx().cumulative_frame_nr();
    let last = ui.data(|d| d.get_temp::<(usize, u64)>(seen));
    let follow = last.is_none_or(|(scroll, at)| scroll != app.ui.scroll || at + 2 < frame);
    ui.data_mut(|d| d.insert_temp(seen, (app.ui.scroll, frame)));

    let spec = Spec { title: "Endeavor", strip_rows: 0 };
    let out = page::show(
        ui,
        &look,
        &spec,
        |ui| {
            let mut area =
                ScrollArea::vertical().id_salt("endeavor page").auto_shrink([false, false]);
            if follow {
                area = area.vertical_scroll_offset(app.ui.scroll as f32 * look.cell.y);
            }
            let shown = area.show(ui, |ui| {
                ui.spacing_mut().item_spacing = vec2(0.0, 0.0);
                let head =
                    Row::pair(view.name.clone(), Tone::Title, view.count.clone(), Tone::Accent);
                page::text_row(ui, &look, &head, false, Sense::hover());
                let width = ui.available_width().min(GATE_CELLS as f32 * look.cell.x);
                let (rect, response) =
                    ui.allocate_exact_size(vec2(width, look.cell.y), Sense::hover());
                let bar = Rect::from_center_size(rect.center(), vec2(width, look.cell.y * 0.5));
                let fill = gauge::tone_color(&look, Tone::Accent);
                gauge::paint(
                    &ui.painter_at(rect),
                    bar,
                    view.frac,
                    fill,
                    gauge::track(&look),
                    false,
                );
                let label = format!("{} met", view.count);
                response
                    .widget_info(|| WidgetInfo::labeled(egui::Role::Label, true, label.clone()));
                if !view.description.is_empty() {
                    let job = look.job(&[(view.description.clone(), Tone::Dim)]);
                    ui.add(Label::new(job).wrap().selectable(false));
                }
                if let Some(c) = &view.chapter {
                    chapter(ui, &look, c, name_width, &marks);
                }
                page::text_row(ui, &look, &Row::default(), false, Sense::hover());
                page::text_row(
                    ui,
                    &look,
                    &Row::text(view.stage.clone(), Tone::Strong),
                    false,
                    Sense::hover(),
                );
                if let Some(u) = &view.united {
                    for row in united_rows(u, &marks) {
                        page::text_row(ui, &look, &row, false, Sense::hover());
                    }
                }
                if let Some(action) = action {
                    let text = RichText::new(action.hint()).font(look.font.clone());
                    let button = Button::new(text).fill(look.panel);
                    if ui.add(button).on_hover_cursor(CursorIcon::PointingHand).clicked() {
                        keys.push(action_key(action));
                    }
                }
                for g in &view.groups {
                    page::text_row(ui, &look, &Row::default(), false, Sense::hover());
                    group(ui, &look, g, name_width, &marks);
                }
            });
            let first = (shown.state.clamped_offset().y / look.cell.y).round().max(0.0) as usize;
            let visible = (shown.inner_rect.height() / look.cell.y).floor() as usize;
            let total = (shown.content_size.y / look.cell.y).ceil() as usize;
            fn_ui::text::window_label("", first, visible, total)
        },
        |_| {},
    );
    if out.close {
        keys.push(key(KeyCode::Esc));
    }
    keys
}
