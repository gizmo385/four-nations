//! The Traders screen (`M`, #886): the caravan camped now, in the page frame, from
//! `fn_ui::trade::view` and the rows of `fn_ui::screens::traders` (the same rows
//! `fn_tui::traders` draws in cells): its name and how long it stays, DEALS, the REQUEST and
//! what is UNDER WAY, or when the next is due.
//!
//! The screen is the shell's own (`UiState::trade_cursor`), so every key works as it does in the
//! terminal, and a click is the keys it stands for ([`click_keys`]): a click on an offer is as
//! many `Up` or `Down` presses as it takes to put the pointer there, a double-click adds
//! `Enter`, which takes the deal once more (the screen stays open and the outcome is on the
//! status line), and the close button is `Esc`. Cancelling what is still owed is `X`, as ever.
//!
//! An offer is one widget over all its rows (the deal and what the stores hold of what it asks;
//! the request and what it gives too), highlighted whole.

use eframe::egui::{self, CursorIcon, Rect, Response, Sense, Ui, WidgetInfo, pos2, vec2};
use fn_tui::color::Palette;
use fn_tui::widgets::{Marks, Row};
use fn_ui::app::App;
use fn_ui::keyev::{KeyCode, KeyEvent, KeyModifiers};
pub use fn_ui::screens::traders::*;
use fn_ui::trade::{TradersView, view};

use crate::cells::Metrics;
use crate::page::{self, Look, Spec};

fn key(code: KeyCode) -> KeyEvent {
    KeyEvent::new(code, KeyModifiers::NONE)
}

/// The keys that put the pointer on offer `to` from offer `at`, and take it if `pick`.
pub fn click_keys(at: usize, to: usize, pick: bool) -> Vec<KeyEvent> {
    let code = if to > at { KeyCode::Down } else { KeyCode::Up };
    let mut keys = vec![key(code); at.abs_diff(to)];
    if pick {
        keys.push(key(KeyCode::Enter));
    }
    keys
}

/// Where the offers are among a screen's rows: `(first row, how many rows, slot)` for each. A
/// deal is its line and what the stores hold; the request has what it gives under those. The
/// first row is wherever the screen's own rows put the pointer for that slot.
pub fn blocks(view: &TradersView, width: usize, marks: &Marks) -> Vec<(usize, usize, usize)> {
    let TradersView::Camp(camp) = view else { return Vec::new() };
    camp.offers
        .iter()
        .filter_map(|o| {
            let slot = usize::from(o.slot);
            let (_, first) = rows(view, slot, width, marks);
            first.map(|first| (first, if o.request { 3 } else { 2 }, slot))
        })
        .collect()
}

/// Draws the screen into `ui` (the window above the hint bar) and returns the keys this
/// frame's clicks stand for.
pub fn show(ui: &mut Ui, app: &App, palette: &Palette, metrics: &Metrics) -> Vec<KeyEvent> {
    let look = Look::new(palette, metrics);
    let marks = Marks::new(palette.ascii);
    let trade = view(&app.session);
    let mut keys = Vec::new();

    let geo = page::geometry(ui.available_rect_before_wrap(), look.cell, 0);
    let width = geo.cols(look.cell);
    let at = app.ui.trade_cursor;
    let (shown, _) = rows(&trade, at, width, &marks);
    let blocks = blocks(&trade, width, &marks);
    let chosen = match &trade {
        TradersView::Camp(c) => at.min(c.offers.len().saturating_sub(1)),
        TradersView::Away(_) => 0,
    };
    // Scrolls to the pointer when it moved (and when the screen opens), not every frame, so the
    // wheel can look elsewhere.
    let seen = ui.make_persistent_id("traders screen: last pointer");
    let moved = ui.data(|d| d.get_temp::<usize>(seen)) != Some(chosen);
    ui.data_mut(|d| d.insert_temp(seen, chosen));

    let spec = Spec { title: "Traders", strip_rows: 0 };
    let out = page::show(
        ui,
        &look,
        &spec,
        |ui| {
            let list =
                crate::style::scroll_vertical().id_salt("traders list").auto_shrink([false, false]);
            list.show(ui, |ui| {
                ui.spacing_mut().item_spacing = vec2(0.0, 0.0);
                let mut i = 0;
                while i < shown.len() {
                    let Some(&(_, len, slot)) = blocks.iter().find(|b| b.0 == i) else {
                        page::text_row(ui, &look, &shown[i], false, Sense::hover());
                        i += 1;
                        continue;
                    };
                    let len = len.min(shown.len() - i);
                    let size = vec2(ui.available_width(), len as f32 * look.cell.y);
                    let (rect, response) = ui.allocate_exact_size(size, Sense::click());
                    offer(ui, &look, rect, &response, &shown[i..i + len], slot == chosen);
                    if slot == chosen && moved {
                        response.scroll_to_me(None);
                    }
                    if response.double_clicked() {
                        keys.extend(click_keys(chosen, slot, true));
                    } else if response.clicked() {
                        keys.extend(click_keys(chosen, slot, false));
                    }
                    i += len;
                }
            });
            None
        },
        |_| {},
    );
    if out.close {
        keys.push(key(KeyCode::Esc));
    }
    keys
}

/// One offer's rows in `rect`: the pointer's on the `select` background with bold text, a
/// hovered one on half of it. The response is one accessibility node holding the offer's words.
fn offer(ui: &Ui, look: &Look<'_>, rect: Rect, response: &Response, rows: &[Row], chosen: bool) {
    let painter = ui.painter_at(rect);
    if chosen {
        painter.rect_filled(rect, 0.0, look.select);
    } else if response.hovered() {
        painter.rect_filled(rect, 0.0, look.select.gamma_multiply(0.5));
    }
    if response.hovered() {
        ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
    }
    let words: Vec<String> = rows
        .iter()
        .enumerate()
        .map(|(i, row)| {
            let line = Rect::from_min_size(
                pos2(rect.left(), rect.top() + i as f32 * look.cell.y),
                vec2(rect.width(), look.cell.y),
            );
            page::paint_row(&painter, look, row, line, chosen)
        })
        .collect();
    let words = words.join(" ");
    response.widget_info(|| WidgetInfo::labeled(egui::Role::Button, true, words.clone()));
}
