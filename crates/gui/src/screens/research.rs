//! The Research screen (`T`, #884): the tech tree as tiers of cards in the page frame, and the
//! CHOOSE A SCHOOL box over it, from `fn_ui::research::screen_view` and the words of
//! `fn_ui::screens::research` (the same rows `fn_tui::research` draws in cells).
//!
//! The screen is the shell's own (`UiState::tech`, `UiState::school_confirm`), so every key works
//! as it does in the terminal, and a click is the keys it stands for ([`click_keys`]): a click
//! on a card is the arrow keys that put the cursor there, a double-click adds `Enter`, which
//! sets the target (a school option asks first, in the box), and the close button is `Esc`.
//!
//! The tiers show as many columns as fit, windowed on the cursor's column as in the terminal
//! (the arrows on the edges step the cursor a tier, and the frame's bottom edge says which
//! tiers are in view). The cards scroll vertically, with the wheel or with the cursor.
//!
//! The box is an egui modal inside this module (the backdrop and `Esc` or its "Back" button
//! cancel, `Enter` or its "Choose" button sets the target): it is not a `Prompt`, so the
//! terminal-box surfaces have nothing to say about it.

use eframe::egui::{
    self, Button, CursorIcon, Frame as Panel, Modal, Rect, RichText, ScrollArea, Sense, Stroke, Ui,
    UiBuilder, WidgetInfo, pos2, vec2,
};
use fn_tui::color::Palette;
use fn_tui::widgets::{Marks, Row, Tone, window_start};
use fn_ui::app::App;
use fn_ui::keyev::{KeyCode, KeyEvent, KeyModifiers};
use fn_ui::research::{ScreenView, screen_view};
pub use fn_ui::screens::research::*;

use crate::cells::Metrics;
use crate::page::{self, Look, Spec};

fn key(code: KeyCode) -> KeyEvent {
    KeyEvent::new(code, KeyModifiers::NONE)
}

/// `n` presses of the arrow key `code`.
fn presses(code: KeyCode, n: usize) -> Vec<KeyEvent> {
    vec![key(code); n]
}

/// The arrow presses that take the cursor from `at` to `to`, as `step` moves it: a step sideways
/// keeps the row only where the next column has that many cards, so the order matters. The
/// shortest of "across then down", "down then across" and (which always works) "to the top,
/// across, down" is the one that lands there.
fn moves(view: &ScreenView, at: Cursor, to: Cursor) -> Vec<KeyEvent> {
    let across = |from: usize, to: usize| {
        if to > from {
            presses(KeyCode::Right, to - from)
        } else {
            presses(KeyCode::Left, from - to)
        }
    };
    let down = |from: usize, to: usize| {
        if to > from { presses(KeyCode::Down, to - from) } else { presses(KeyCode::Up, from - to) }
    };
    let candidates = [
        [across(at.0, to.0), down(at.1, to.1)].concat(),
        [down(at.1, to.1), across(at.0, to.0)].concat(),
        [down(at.1, 0), across(at.0, to.0), down(0, to.1)].concat(),
    ];
    let lands = |keys: &Vec<KeyEvent>| {
        let end = keys.iter().fold(at, |cursor, k| {
            let (dx, dy) = match k.code {
                KeyCode::Left => (-1, 0),
                KeyCode::Right => (1, 0),
                KeyCode::Up => (0, -1),
                _ => (0, 1),
            };
            step(view, cursor, dx, dy)
        });
        end == to
    };
    candidates.into_iter().filter(lands).min_by_key(Vec::len).unwrap_or_default()
}

/// The keys that put the cursor on card `to` from `at`, and `Enter` (set the target) if `pick`.
pub fn click_keys(view: &ScreenView, at: Cursor, to: Cursor, pick: bool) -> Vec<KeyEvent> {
    let mut keys = moves(view, at, to);
    if pick {
        keys.push(key(KeyCode::Enter));
    }
    keys
}

/// The detail strip's height: the tallest card's detail, so the rule holds still as the cursor
/// moves, and a third of the page at most (the rows past it are cut).
fn strip_height(
    app: &App,
    view: &ScreenView,
    marks: &Marks,
    cols: usize,
    page_rows: usize,
) -> usize {
    let tallest = view
        .columns
        .iter()
        .flatten()
        .map(|card| detail_rows(&app.session, card, marks, cols).len())
        .max()
        .unwrap_or(0);
    tallest.max(usize::from(DETAIL_ROWS)).min((page_rows / 3).max(1))
}

/// Draws the screen into `ui` (the window above the hint bar) and returns the keys this frame's
/// clicks stand for.
pub fn show(ui: &mut Ui, app: &App, palette: &Palette, metrics: &Metrics) -> Vec<KeyEvent> {
    let look = Look::new(palette, metrics);
    let marks = Marks::new(palette.ascii);
    let view = screen_view(&app.session);
    let mut keys = Vec::new();

    if view.columns.is_empty() {
        let spec = Spec { title: "Research", strip_rows: 0 };
        let nothing = Row::text("Nothing to research yet.", Tone::Dim);
        let out = page::show(
            ui,
            &look,
            &spec,
            |ui| {
                page::text_row(ui, &look, &nothing, false, Sense::hover());
                None
            },
            |_| {},
        );
        if out.close {
            keys.push(key(KeyCode::Esc));
        }
        return keys;
    }

    let at = step(&view, app.ui.tech, 0, 0);
    let geo = page::geometry(ui.available_rect_before_wrap(), look.cell, 0);
    let (cols, page_rows) = (geo.cols(look.cell), geo.rows(look.cell));
    let strip_rows = strip_height(app, &view, &marks, cols, page_rows);
    // A click goes nowhere while the school box is up over the cards.
    let live = app.ui.school_confirm.is_none();

    let grid = Grid::new((cols, page_rows), view.columns.len());
    let first = window_start(at.0, grid.columns);
    let shown = first..(first + grid.columns).min(view.columns.len());
    let cell_w = grid.column_width as f32 * look.cell.x;
    let slot_x = |slot: usize| slot as f32 * (grid.column_width + GAP) as f32 * look.cell.x;

    // Every card of the columns in view as rows, wrapped to the column, and the height of each
    // grid row (its tallest card, so the cards of a row line up across columns).
    let cards: Vec<Vec<Vec<Row>>> = shown
        .clone()
        .map(|c| {
            view.columns[c]
                .iter()
                .enumerate()
                .map(|(r, card)| card_rows(card, (c, r) == at, &marks, grid.column_width))
                .collect()
        })
        .collect();
    let depth = cards.iter().map(Vec::len).max().unwrap_or(0);
    let heights: Vec<usize> = (0..depth)
        .map(|r| cards.iter().filter_map(|c| c.get(r)).map(Vec::len).max().unwrap_or(0))
        .collect();

    // Scrolls to the cursor when it moved (and when the screen opens), not every frame, so the
    // wheel can look elsewhere.
    let seen = ui.make_persistent_id("research screen: last cursor");
    let moved = ui.data(|d| d.get_temp::<Cursor>(seen)) != Some(at);
    ui.data_mut(|d| d.insert_temp(seen, at));

    let spec = Spec { title: "Research", strip_rows };
    let out = page::show(
        ui,
        &look,
        &spec,
        |ui| {
            let body = ui.max_rect();
            // The column headers: the depth, the cursor's tier in the title's tone.
            let head = Rect::from_min_size(body.min, vec2(body.width(), look.cell.y));
            for (slot, c) in shown.clone().enumerate() {
                let rect = Rect::from_min_size(
                    pos2(body.left() + slot_x(slot), head.top()),
                    vec2(cell_w, look.cell.y),
                );
                let tone = if c == at.0 { Tone::Title } else { Tone::Dim };
                let row = Row::text(column_title(&view, c), tone);
                ui.scope_builder(UiBuilder::new().max_rect(rect), |ui| {
                    page::text_row(ui, &look, &row, false, Sense::hover());
                });
            }
            // Arrows in the padding when tiers are out of sight: a click steps the cursor a tier.
            let arrow = |ui: &mut Ui, x: f32, text: &str, label: &str| {
                let rect = Rect::from_min_size(pos2(x, head.top()), look.cell);
                let response = ui.interact(rect, ui.scope_id().with(label), Sense::click());
                let tone = if response.hovered() { Tone::Title } else { Tone::Accent };
                page::paint_row(&ui.painter_at(rect), &look, &Row::text(text, tone), rect, false);
                response.widget_info(|| WidgetInfo::labeled(egui::Role::Button, true, label));
                response.on_hover_cursor(CursorIcon::PointingHand).clicked()
            };
            if live
                && first > 0
                && arrow(ui, head.left() - look.cell.x, marks.left, "Earlier tiers")
            {
                keys.push(key(KeyCode::Left));
            }
            if live
                && shown.end < view.columns.len()
                && arrow(ui, head.left() + cols as f32 * look.cell.x, marks.right, "Later tiers")
            {
                keys.push(key(KeyCode::Right));
            }

            let below = Rect::from_min_max(pos2(body.left(), head.bottom()), body.max);
            ui.scope_builder(UiBuilder::new().max_rect(below), |ui| {
                let scroll =
                    ScrollArea::vertical().id_salt("research cards").auto_shrink([false, false]);
                scroll.show(ui, |ui| {
                    let total: usize = heights.iter().map(|h| h + 1).sum();
                    let (area, _) = ui.allocate_exact_size(
                        vec2(ui.available_width(), total as f32 * look.cell.y),
                        Sense::hover(),
                    );
                    let mut top = area.top();
                    for (r, &height) in heights.iter().enumerate() {
                        for (slot, c) in shown.clone().enumerate() {
                            let Some(rows) = cards[slot].get(r) else { continue };
                            let rect = Rect::from_min_size(
                                pos2(area.left() + slot_x(slot), top),
                                vec2(cell_w, rows.len() as f32 * look.cell.y),
                            );
                            let sense = if live { Sense::click() } else { Sense::hover() };
                            let chosen = (c, r) == at;
                            let response = card(ui, &look, rect, rows, chosen, sense);
                            if chosen && moved {
                                response.scroll_to_me(None);
                            }
                            if response.double_clicked() {
                                keys.extend(click_keys(&view, at, (c, r), true));
                            } else if response.clicked() {
                                keys.extend(click_keys(&view, at, (c, r), false));
                            }
                        }
                        top += (height + 1) as f32 * look.cell.y;
                    }
                });
            });
            fn_ui::text::window_label("tiers", first, grid.columns, view.columns.len())
        },
        |ui| {
            if let Some(card) = card_at(&view, at) {
                let width = (ui.max_rect().width() / look.cell.x).floor() as usize;
                for row in detail_rows(&app.session, card, &marks, width) {
                    page::text_row(ui, &look, &row, false, Sense::hover());
                }
            }
        },
    );
    if out.close {
        keys.push(key(KeyCode::Esc));
    }
    if let Some(tech) = app.ui.school_confirm {
        keys.extend(school_box(ui, app, tech, &look, &marks, cols));
    }
    keys
}

/// One card's rows in `rect`: the cursor's card on the `select` background with bold text, a
/// hovered one on half of it. The response is one accessibility node holding the card's words.
fn card(
    ui: &mut Ui,
    look: &Look<'_>,
    rect: Rect,
    rows: &[Row],
    chosen: bool,
    sense: Sense,
) -> egui::Response {
    let id = ui.scope_id().with(("research card", rect.min.x as i32, rect.min.y as i32));
    let response = ui.interact(rect, id, sense);
    let painter = ui.painter_at(rect);
    if chosen {
        painter.rect_filled(rect, 0.0, look.select);
    } else if sense.senses_click() && response.hovered() {
        painter.rect_filled(rect, 0.0, look.select.gamma_multiply(0.5));
    }
    if sense.senses_click() && response.hovered() {
        ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
    }
    let mut words = Vec::new();
    for (i, row) in rows.iter().enumerate() {
        let line = Rect::from_min_size(
            pos2(rect.left(), rect.top() + i as f32 * look.cell.y),
            vec2(rect.width(), look.cell.y),
        );
        words.push(page::paint_row(&painter, look, row, line, chosen));
    }
    let words = words.join(" ");
    let role = if sense.senses_click() { egui::Role::Button } else { egui::Role::Label };
    response.widget_info(|| WidgetInfo::labeled(role, true, words.clone()));
    response
}

/// The CHOOSE A SCHOOL box over the screen: the fork's question, what choosing gives up, the
/// bill, and the two answers as buttons, which are `Enter` and `Esc`. Its backdrop swallows the
/// clicks the cards would take; a click on it is `Esc`, back out.
fn school_box(
    ui: &Ui,
    app: &App,
    tech: fn_sim::research::TechId,
    look: &Look<'_>,
    marks: &Marks,
    cols: usize,
) -> Vec<KeyEvent> {
    let text = CONFIRM_WIDTH.min(cols.saturating_sub(8)).max(1);
    let frame = Panel::new()
        .fill(look.panel)
        .stroke(Stroke::new(2.0, look.line))
        .inner_margin(look.cell.x * 2.0)
        .outer_margin(0.0);
    let (choose, back) = confirm_buttons(&app.session, tech);
    let mut keys = Vec::new();
    let modal = Modal::new(ui.make_persistent_id("research school box")).frame(frame).show(
        ui.ctx(),
        |ui| {
            ui.set_width(text as f32 * look.cell.x);
            ui.spacing_mut().item_spacing = vec2(look.cell.x * 2.0, 0.0);
            let title = Row::text(CONFIRM_TITLE, Tone::Title);
            page::text_row(ui, look, &title, false, Sense::hover());
            page::text_row(ui, look, &Row::text(String::new(), Tone::Text), false, Sense::hover());
            for row in confirm_body(&app.session, tech, marks, text) {
                page::text_row(ui, look, &row, false, Sense::hover());
            }
            ui.horizontal(|ui| {
                let button = |label: &str| {
                    Button::new(RichText::new(label).font(look.font.clone())).fill(look.panel)
                };
                if ui.add(button(&choose)).on_hover_cursor(CursorIcon::PointingHand).clicked() {
                    keys.push(key(KeyCode::Enter));
                }
                if ui.add(button(&back)).on_hover_cursor(CursorIcon::PointingHand).clicked() {
                    keys.push(key(KeyCode::Esc));
                }
            });
        },
    );
    if modal.backdrop_response.clicked() {
        keys.push(key(KeyCode::Esc));
    }
    keys
}
