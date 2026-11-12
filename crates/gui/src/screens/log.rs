//! The message log (`L`, #888): the lines of `fn_ui::msglog` in the page frame, newest first
//! under their season's heading, as `fn_tui::msglog` draws them from the same rows
//! (`fn_ui::screens::msglog`: the filters, the stamps, the tones), except that the text wraps as
//! egui wraps it, to the width of the window.
//!
//! The screen is the shell's own (`UiState::log_cursor`, `log_filter`, `log_open`), so every key
//! works as it does in the terminal, and a click is the keys it stands for:
//!
//! - a filter chip is its digit (`0` all, `1` to `5` a kind; the one that is on, pressed again,
//!   shows all);
//! - a click on a line puts the cursor there ([`click_keys`]: as many `Up` or `Down` presses as
//!   it takes), and a double-click adds `Enter`, which goes to where the line is about;
//! - a click on a fold's `+` or `-` puts the cursor on it and presses `Right` (unfold) or
//!   `Left` (fold);
//! - the close button is `Esc`.
//!
//! The list scrolls with the wheel, and to the cursor's line when it moves. The frame's bottom
//! edge says which lines are in view ("lines 1-14 of 80") once not all are.

use eframe::egui::{
    self, Button, CursorIcon, Rect, RichText, ScrollArea, Sense, Ui, WidgetInfo, pos2, vec2,
};
use fn_tui::color::Palette;
use fn_tui::widgets::{Marks, Row, Tone};
use fn_ui::app::App;
use fn_ui::keyev::{KeyCode, KeyEvent, KeyModifiers};
use fn_ui::msglog::{Line, LineKey, index_of};
use fn_ui::screens::msglog::{Chip, chips, line_tone, lines, stamps};
use fn_ui::screens::title;
use fn_ui::state::Screen;

use crate::cells::Metrics;
use crate::page::{self, Look, Spec};

/// What the log says when it has no line under the filter (the terminal's words).
pub use fn_ui::screens::msglog::empty_text;

fn key(code: KeyCode) -> KeyEvent {
    KeyEvent::new(code, KeyModifiers::NONE)
}

/// The keys that put the cursor on line `to` from line `at`, and go to it (`Enter`) if `pick`.
pub fn click_keys(at: usize, to: usize, pick: bool) -> Vec<KeyEvent> {
    super::build::click_keys(at, to, pick)
}

/// The key a filter chip is.
pub fn chip_key(chip: &Chip) -> KeyEvent {
    key(KeyCode::Char(chip.key))
}

/// Draws the screen into `ui` (the window above the hint bar) and returns the keys this frame's
/// clicks stand for.
pub fn show(ui: &mut Ui, app: &App, palette: &Palette, metrics: &Metrics) -> Vec<KeyEvent> {
    let look = Look::new(palette, metrics);
    let marks = Marks::new(palette.ascii);
    let all = lines(&app.ui);
    let stamped = stamps(&app.session, &all);
    let at = index_of(&all, app.ui.log_cursor);
    let mut keys = Vec::new();

    let spec = Spec { title: title(Screen::Log), strip_rows: 0 };
    let out = page::show(
        ui,
        &look,
        &spec,
        |ui| {
            keys.extend(filter_chips(ui, &look, app));
            ui.add_space(look.cell.y);
            if all.is_empty() {
                let text = empty_text(app.ui.log_filter, app.ui.spectating);
                let galley = ui.painter().layout(
                    text,
                    look.font.clone(),
                    look.format(Tone::Dim).color,
                    ui.available_width(),
                );
                let (rect, response) = ui.allocate_exact_size(galley.size(), Sense::hover());
                let label = galley.text().to_owned();
                response
                    .widget_info(|| WidgetInfo::labeled(egui::Role::Label, true, label.clone()));
                ui.painter().galley(rect.min, galley, look.format(Tone::Dim).color);
                return None;
            }

            let stamp_cols = stamped.iter().map(|s| s.left.chars().count()).max().unwrap_or(0);
            // Scrolls to the cursor when it moved (and when the screen opens), not every frame,
            // so the wheel can look elsewhere.
            let seen = ui.make_persistent_id("log screen: last cursor");
            let cursor = app.ui.log_cursor.map(|k: LineKey| (k.id, k.sub));
            let moved = ui.data(|d| d.get_temp::<Option<(u64, bool)>>(seen)) != Some(cursor);
            ui.data_mut(|d| d.insert_temp(seen, cursor));

            let (mut first, mut visible) = (None, 0);
            ScrollArea::vertical().id_salt("log lines").auto_shrink([false, false]).show(
                ui,
                |ui| {
                    ui.spacing_mut().item_spacing = vec2(0.0, 0.0);
                    for (i, (line, stamp)) in all.iter().zip(&stamped).enumerate() {
                        if let Some(heading) = &stamp.heading {
                            heading_row(ui, &look, heading, &marks);
                        }
                        let shown =
                            LineRow { line, stamp: &stamp.left, stamp_cols, chosen: i == at };
                        let row = shown.show(ui, &look, &marks);
                        if row.rect.intersects(ui.clip_rect()) {
                            first.get_or_insert(i);
                            visible += 1;
                        }
                        if shown.chosen && moved {
                            ui.scroll_to_rect(row.rect, None);
                        }
                        if row.sign {
                            // The fold's own button: put the cursor there, then open or fold it.
                            let code = if line.open { KeyCode::Left } else { KeyCode::Right };
                            keys.extend(click_keys(at, i, false));
                            keys.push(key(code));
                        } else if row.response.double_clicked() {
                            keys.extend(click_keys(at, i, true));
                        } else if row.response.clicked() {
                            keys.extend(click_keys(at, i, false));
                        }
                    }
                },
            );
            fn_ui::text::window_label("lines", first.unwrap_or(0), visible, all.len())
        },
        |_| {},
    );
    if out.close {
        keys.push(key(KeyCode::Esc));
    }
    keys
}

/// The filter row: `Show` and a button per chip, the one that is on selected. A click is the
/// digit the chip is labelled with.
fn filter_chips(ui: &mut Ui, look: &Look<'_>, app: &App) -> Vec<KeyEvent> {
    let mut keys = Vec::new();
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing = vec2(look.cell.x, 0.0);
        let dim = look.format(Tone::Dim).color;
        ui.label(RichText::new("Show").font(look.font.clone()).color(dim));
        for chip in chips(app.ui.log_filter) {
            let text = if chip.filter.is_none() {
                chip.label.to_owned()
            } else {
                format!("{} {}", chip.key, chip.label)
            };
            let tone = if chip.on { Tone::Title } else { Tone::Dim };
            let text = RichText::new(text).font(look.font.clone()).color(look.format(tone).color);
            let button = Button::new(text).selected(chip.on).min_size(vec2(0.0, look.cell.y));
            if ui.add(button).on_hover_cursor(CursorIcon::PointingHand).clicked() {
                keys.push(chip_key(&chip));
            }
        }
    });
    keys
}

/// A season's heading: `── WINTER, YEAR 1 ───────`, the rule running on to the right edge.
fn heading_row(ui: &mut Ui, look: &Look<'_>, heading: &str, marks: &Marks) {
    let (rect, response) =
        ui.allocate_exact_size(vec2(ui.available_width(), look.cell.y), Sense::hover());
    let rule = marks.h.to_string().repeat(2);
    let text = format!("{rule} {heading} ");
    let painter = ui.painter_at(rect);
    let row = Row::text(text.clone(), Tone::Title);
    page::paint_row(&painter, look, &row, rect, false);
    let after = rect.left() + text.chars().count() as f32 * look.cell.x;
    painter.hline(after..=rect.right(), rect.center().y, egui::Stroke::new(1.0, look.line));
    let label = heading.to_owned();
    response.widget_info(|| WidgetInfo::labeled(egui::Role::Label, true, label.clone()));
}

/// One line of the log: the pointer, the stamp, the fold's sign and the text, wrapped.
struct LineRow<'a> {
    line: &'a Line,
    stamp: &'a str,
    /// How many cells wide the stamp column is.
    stamp_cols: usize,
    chosen: bool,
}

/// What a [`LineRow`] drew, and what was clicked.
struct Drawn {
    rect: Rect,
    response: egui::Response,
    /// The fold's `+` or `-` was clicked.
    sign: bool,
}

impl LineRow<'_> {
    fn show(&self, ui: &mut Ui, look: &Look<'_>, marks: &Marks) -> Drawn {
        let cell = look.cell;
        let line = self.line;
        let width = ui.available_width();
        // The pointer, the stamp, the fold's sign: "▸ 8:11 left + text", an open fold's own
        // lines indented under it.
        let sign_at = (2 + self.stamp_cols + 1) as f32 * cell.x;
        let indent = if line.key.sub { 2.0 * cell.x } else { 0.0 };
        let text_at = sign_at + 2.0 * cell.x + indent;
        let tone = line_tone(line.kind, line.category);
        let color = look.format(tone).color;
        let galley = ui.painter().layout(
            line.text.clone(),
            look.font.clone(),
            color,
            (width - text_at).max(cell.x),
        );
        let height = galley.size().y.max(cell.y);
        let (rect, response) = ui.allocate_exact_size(vec2(width, height), Sense::click());
        let painter = ui.painter_at(rect);
        if self.chosen {
            painter.rect_filled(rect, 0.0, look.select);
        } else if response.hovered() {
            painter.rect_filled(rect, 0.0, look.select.gamma_multiply(0.5));
        }
        if response.hovered() {
            ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
        }

        let top = rect.top();
        let put = |x: f32, text: String, tone: Tone| {
            let galley = painter.layout_job(look.job(&[(text, tone)]));
            page::paint_galley(&painter, pos2(rect.left() + x, top), galley, self.chosen);
        };
        if self.chosen {
            put(0.0, marks.pointer.to_owned(), Tone::Title);
        }
        let stamp_w = self.stamp.chars().count() as f32 * cell.x;
        put(
            2.0 * cell.x + (self.stamp_cols as f32 * cell.x - stamp_w),
            self.stamp.to_owned(),
            Tone::Dim,
        );
        let sign = match (line.is_fold(), line.open) {
            (true, true) => "-",
            (true, false) => "+",
            _ => "",
        };
        if !sign.is_empty() {
            put(sign_at, sign.to_owned(), Tone::Accent);
        }
        page::paint_galley(&painter, pos2(rect.left() + text_at, top), galley.clone(), self.chosen);

        let text = format!("{} {}", self.stamp, line.text);
        response.widget_info(|| WidgetInfo::labeled(egui::Role::Button, true, text.clone()));

        // The fold's sign is a button of its own over the row's.
        let mut clicked = false;
        if line.is_fold() {
            let area =
                Rect::from_min_size(pos2(rect.left() + sign_at, top), vec2(2.0 * cell.x, cell.y));
            let id = ui.scope_id().with(("log fold", line.key.id));
            let button = ui.interact(area, id, Sense::click());
            let what =
                format!("{} {} lines", if line.open { "Fold" } else { "Unfold" }, line.count);
            button.widget_info(|| WidgetInfo::labeled(egui::Role::Button, true, what.clone()));
            clicked = button.on_hover_cursor(CursorIcon::PointingHand).clicked();
        }
        Drawn { rect, response, sign: clicked }
    }
}
