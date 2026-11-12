//! The Nation screen (`N`, #887): the roster as a grid in the page frame, from
//! `fn_ui::view::nation_view` and the words of `fn_ui::screens::nation` (the same columns and
//! rows `fn_tui::nation` draws in cells, and the same detail strip).
//!
//! ```text
//! Earth Kingdom · 9 people · seed 7                          Summer, year 1 · 3:20
//!
//!   NAME   DOING                                   HUNGER  THIRST  ENERGY  HEALTH  BENDING
//! HAULING 3
//! ▸ Hua    Delivering 2 Fruit to the stockpile     ▆▆▆▆▆   ▆▆▆▆▆   ▆▆▆▆▆   ▆▆▆▆▆   Student, 4 bends
//! ```
//!
//! What a window adds: **the needs are bars** ([`crate::gauge`]) in the status's tone, with the
//! terminal's `!` against a low one, and the grid keeps the terminal's columns (the name column
//! as wide as the longest name, DOING taking what the others leave, BENDING dropped first when
//! DOING would get narrow). The header and the stores, almanac and traits stay put; the roster
//! between them scrolls, to the pointer when it moves (and the wheel can look elsewhere).
//!
//! The screen is the shell's own (`UiState::nation_cursor`), so every key works as it does in
//! the terminal, and a click is the keys it stands for ([`click_keys`]): a click on a settler's
//! row puts the pointer there and presses `Enter`, which selects them on the map and closes the
//! screen, and the close button is `Esc`.

use eframe::egui::{
    self, CursorIcon, Rect, Response, ScrollArea, Sense, Ui, UiBuilder, WidgetInfo, pos2, vec2,
};
use fn_tui::color::Palette;
use fn_tui::widgets::{Marks, Row, Tone};
use fn_ui::app::App;
use fn_ui::keyev::{KeyCode, KeyEvent, KeyModifiers};
pub use fn_ui::screens::nation::*;
use fn_ui::view::{NationView, Status, UnitView, nation_view};

use crate::cells::Metrics;
use crate::gauge;
use crate::page::{self, Look, Spec};

fn key(code: KeyCode) -> KeyEvent {
    KeyEvent::new(code, KeyModifiers::NONE)
}

/// The keys that put the pointer on settler `to` from settler `at` and press `Enter`: a click
/// on a row selects that settler as `Enter` does.
pub fn click_keys(at: usize, to: usize) -> Vec<KeyEvent> {
    let code = if to > at { KeyCode::Down } else { KeyCode::Up };
    let mut keys = vec![key(code); at.abs_diff(to)];
    keys.push(key(KeyCode::Enter));
    keys
}

/// A settler's words for a screen reader (and the tests): the name, the task, each need in
/// percent and the bending, where the terminal has glyph meters.
pub fn describe(unit: &UnitView) -> String {
    let needs: Vec<String> =
        ordered_needs(unit).map(|n| format!("{} {}", n.label, n.percent())).collect();
    format!("{}, {}, {}, {}", unit.name, unit.task, needs.join(", "), unit.bending)
}

/// The rows under the roster: stores, then the almanac and the traits, as the terminal sets
/// them (a blank row, a heading and the packed entries).
fn footer_rows(view: &NationView, width: usize) -> Vec<Row> {
    let stores = packed_rows(
        view.stores
            .iter()
            .map(|r| format!("{} {}{}", r.name, r.count, if r.full { " full" } else { "" })),
        width,
    );
    let traits = packed_rows(view.traits.iter().map(|(l, v)| format!("{l} {v}")), width);
    let mut rows = vec![Row::default(), Row::text("STORES", Tone::Title)];
    if stores.is_empty() {
        rows.push(Row::text("Nothing yet", Tone::Dim));
    }
    rows.extend(stores);
    if !view.almanac.is_empty() {
        rows.push(Row::default());
        rows.push(Row::text("ALMANAC", Tone::Title));
        rows.extend(view.almanac.iter().map(|l| Row::text(l.clone(), Tone::Text)));
    }
    if !traits.is_empty() {
        rows.push(Row::default());
        rows.push(Row::text("TRAITS", Tone::Title));
        rows.extend(traits);
    }
    rows
}

/// One settler's row as a grid: the pointer, the name, the task, a bar per need and the bending
/// words, in the columns of [`Columns`]. Returns the click's response.
fn unit_row(
    ui: &mut Ui,
    look: &Look<'_>,
    unit: &UnitView,
    chosen: bool,
    cols: &Columns,
    marks: &Marks,
) -> Response {
    let cell = look.cell;
    let size = vec2(ui.available_width(), cell.y);
    let (rect, response) = ui.allocate_exact_size(size, Sense::click());
    let painter = ui.painter_at(rect);
    if chosen {
        painter.rect_filled(rect, 0.0, look.select);
    } else if response.hovered() {
        painter.rect_filled(rect, 0.0, look.select.gamma_multiply(0.5));
    }
    if response.hovered() {
        ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
    }
    let at = |cells: usize| rect.left() + cells as f32 * cell.x;
    let text = |x: usize, width: usize, span: String, tone: Tone| {
        let area =
            Rect::from_min_size(pos2(at(x), rect.top()), vec2(width as f32 * cell.x, cell.y));
        page::paint_row(&painter, look, &Row::text(span, tone), area, chosen);
    };
    let name_at = 2;
    let doing_at = name_at + cols.name + GAP;
    let meters_at = doing_at + cols.doing + GAP;
    let pointer = if chosen { marks.pointer.to_owned() } else { String::new() };
    text(0, 2, pointer, Tone::Title);
    let name_tone = if chosen { Tone::Title } else { Tone::Text };
    text(name_at, cols.name + GAP, unit.name.clone(), name_tone);
    text(doing_at, cols.doing, fit_words(&unit.task, cols.doing, marks.ellipsis), Tone::Text);
    let track = gauge::track(look);
    for (i, need) in ordered_needs(unit).enumerate() {
        let col = meters_at + i * (METER_COL + GAP);
        let low = need.status != Status::Ok;
        let tone = Tone::of(need.status);
        let fill = gauge::tone_color(look, tone);
        let bar = gauge::rect(at(col), rect.center().y, NEED_CELLS, cell);
        gauge::paint(&painter, bar, need.frac, fill, track, low);
        if low {
            text(col + NEED_CELLS, 1, "!".to_owned(), tone);
        }
    }
    if let Some(width) = cols.bending {
        let col = meters_at + 4 * (METER_COL + GAP);
        text(col, width, fit_words(&unit.bending, width, marks.ellipsis), Tone::Text);
    }
    let label = describe(unit);
    response.widget_info(|| WidgetInfo::labeled(egui::Role::Button, true, label.clone()));
    response
}

/// Draws the screen into `ui` (the window above the hint bar) and returns the keys this
/// frame's clicks stand for.
pub fn show(ui: &mut Ui, app: &App, palette: &Palette, metrics: &Metrics) -> Vec<KeyEvent> {
    let look = Look::new(palette, metrics);
    let marks = Marks::new(palette.ascii);
    let view = nation_view(&app.session);
    let ids = roster(&view);
    let at = app.ui.nation_cursor.min(ids.len().saturating_sub(1));
    let mut keys = Vec::new();

    let outer = ui.available_rect_before_wrap();
    let geo = page::geometry(outer, look.cell, 0);
    let page_rows = geo.rows(look.cell).min(usize::from(u16::MAX)) as u16;
    let strip_rows = if ids.is_empty() { 0 } else { usize::from(detail_rows(page_rows)) };
    let width = geo.cols(look.cell);
    let wide = (outer.width() / look.cell.x).floor() as u16 >= METER_MIN_WIDTH;
    let cols = Columns::of(&view, width);
    let footer = footer_rows(&view, width);

    // Scrolls to the pointer when it moved (and when the screen opens), not every frame.
    let seen = ui.make_persistent_id("nation screen: last pointer");
    let moved = ui.data(|d| d.get_temp::<usize>(seen)) != Some(at);
    ui.data_mut(|d| d.insert_temp(seen, at));

    let spec = Spec { title: "Nation", strip_rows };
    let out = page::show(
        ui,
        &look,
        &spec,
        |ui| {
            let body = ui.max_rect();
            let head = Row::pair(
                format!(
                    "{} {} {} people {} seed {}",
                    view.nation, marks.dot, view.population, marks.dot, view.seed
                ),
                Tone::Title,
                fn_ui::rows::clock(&view.clock, &marks),
                Tone::Dim,
            );
            page::text_row(ui, &look, &head, false, Sense::hover());
            page::text_row(ui, &look, &Row::default(), false, Sense::hover());
            if wide {
                page::text_row(ui, &look, &header(&cols), false, Sense::hover());
            }
            let footer_h = footer.len() as f32 * look.cell.y;
            let top = ui.cursor().top();
            let roster_h = (body.bottom() - top - footer_h).max(look.cell.y);
            let list = Rect::from_min_size(pos2(body.left(), top), vec2(body.width(), roster_h));
            // The settlers drawn, as (top, bottom, place in the roster), for the position on
            // the frame's edge.
            let mut placed: Vec<(f32, f32, usize)> = Vec::new();
            let shown = ui
                .scope_builder(UiBuilder::new().max_rect(list), |ui| {
                    ScrollArea::vertical()
                        .id_salt("nation roster")
                        .auto_shrink([false, false])
                        .show(ui, |ui| {
                            ui.spacing_mut().item_spacing = vec2(0.0, 0.0);
                            let mut n = 0;
                            for (activity, group) in &view.groups {
                                let name = activity.heading().to_uppercase();
                                let heading =
                                    Row::text(format!("{name} {}", group.len()), Tone::Title);
                                page::text_row(ui, &look, &heading, false, Sense::hover());
                                for unit in group {
                                    let chosen = n == at;
                                    let response = if wide {
                                        unit_row(ui, &look, unit, chosen, &cols, &marks)
                                    } else {
                                        let row = narrow_unit_row(unit, chosen, cols.name, &marks);
                                        page::text_row(ui, &look, &row, chosen, Sense::click())
                                    };
                                    placed.push((response.rect.top(), response.rect.bottom(), n));
                                    if chosen && moved {
                                        response.scroll_to_me(None);
                                    }
                                    if response.clicked() {
                                        keys.extend(click_keys(at, n));
                                    }
                                    n += 1;
                                }
                            }
                        })
                })
                .inner;
            let (low, high) = (shown.inner_rect.top() - 0.5, shown.inner_rect.bottom() + 0.5);
            let on_screen: Vec<usize> = placed
                .iter()
                .filter(|(top, bottom, _)| *top >= low && *bottom <= high)
                .map(|(_, _, n)| *n)
                .collect();
            let position = on_screen.first().and_then(|&first| {
                fn_ui::text::window_label("settlers", first, on_screen.len(), ids.len())
            });

            // The footer, pinned under the roster.
            let below = Rect::from_min_max(pos2(body.left(), list.bottom()), body.max);
            ui.scope_builder(UiBuilder::new().max_rect(below), |ui| {
                for row in &footer {
                    page::text_row(ui, &look, row, false, Sense::hover());
                }
            });
            position
        },
        |ui| {
            let Some(&id) = ids.get(at) else { return };
            for row in detail(&app.session, id, strip_rows, width, &marks) {
                page::text_row(ui, &look, &row, false, Sense::hover());
            }
        },
    );
    if out.close {
        keys.push(key(KeyCode::Esc));
    }
    keys
}
