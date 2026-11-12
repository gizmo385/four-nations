//! The Build screen (`U`, #881): `fn_ui::build::picker_rows` as a list in the page frame, and
//! `building_detail` for the highlighted one in the strip under it, as `fn_tui::build` draws
//! them from the same rows (`fn_ui::screens::build`: the columns, the detail's lines).
//!
//! The screen is the shell's own (`UiState::build_cursor`, `Modal::Build`), so every key works as
//! it does in the terminal, and a click is the keys it stands for ([`click_keys`]): a click on a
//! row is as many `Up` or `Down` presses as it takes to put the highlight there, a double-click
//! adds `Enter`, which picks the building (a locked one only says what it needs, in the strip),
//! and the close button is `Esc`.
//!
//! The list scrolls (wheel, or the highlight moving past an edge); the position of the window
//! over it ("buildings 1-20 of 41") is on the frame's bottom edge once it does.

use eframe::egui::{ScrollArea, Sense, Ui, vec2};
use fn_tui::color::Palette;
use fn_tui::widgets::{Marks, Row, Tone};
use fn_ui::app::App;
use fn_ui::build::{BuildingDetail, building_detail, picker_rows};
use fn_ui::keyev::{KeyCode, KeyEvent, KeyModifiers};
pub use fn_ui::screens::build::*;

use crate::cells::Metrics;
use crate::page::{self, Look, Spec};

fn key(code: KeyCode) -> KeyEvent {
    KeyEvent::new(code, KeyModifiers::NONE)
}

/// The keys that put the highlight on row `to` from row `at`, and pick it if `pick`.
pub fn click_keys(at: usize, to: usize, pick: bool) -> Vec<KeyEvent> {
    let code = if to > at { KeyCode::Down } else { KeyCode::Up };
    let mut keys = vec![key(code); at.abs_diff(to)];
    if pick {
        keys.push(key(KeyCode::Enter));
    }
    keys
}

/// Draws the screen into `ui` (the window above the hint bar) and returns the keys this
/// frame's clicks stand for.
pub fn show(ui: &mut Ui, app: &App, palette: &Palette, metrics: &Metrics) -> Vec<KeyEvent> {
    let look = Look::new(palette, metrics);
    let marks = Marks::new(palette.ascii);
    let rows = picker_rows(&app.session);
    let mut keys = Vec::new();

    if rows.is_empty() {
        let spec = Spec { title: "Build", strip_rows: 0 };
        let nothing = Row::text("Nothing to build yet.", Tone::Dim);
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

    let at = app.ui.build_cursor.min(rows.len() - 1);
    // The strip is as tall as the tallest building's detail, so the rule holds still as the
    // highlight moves; the list keeps at least half the page.
    let geo = page::geometry(ui.available_rect_before_wrap(), look.cell, 0);
    let width = geo.cols(look.cell);
    let details: Vec<BuildingDetail> =
        rows.iter().map(|r| building_detail(&app.session, r.id)).collect();
    let tallest =
        details.iter().map(|d| detail_rows(d, width, marks.ellipsis).len()).max().unwrap_or(0);
    let room = (geo.rows(look.cell).saturating_sub(2) / 2).max(1);
    let strip_rows = tallest.min(MAX_STRIP).min(room);

    let columns = Columns::of(&rows);
    // Scrolls to the highlight when it moved (and when the screen opens), not every frame, so
    // the wheel can look elsewhere.
    let seen = ui.make_persistent_id("build screen: last highlight");
    let moved = ui.data(|d| d.get_temp::<usize>(seen)) != Some(at);
    ui.data_mut(|d| d.insert_temp(seen, at));

    let spec = Spec { title: "Build", strip_rows };
    let out = page::show(
        ui,
        &look,
        &spec,
        |ui| {
            page::text_row(ui, &look, &columns.header(), false, Sense::hover());
            let list = ScrollArea::vertical().id_salt("build list").auto_shrink([false, false]);
            let shown = list.show(ui, |ui| {
                ui.spacing_mut().item_spacing = vec2(0.0, 0.0);
                for (i, row) in rows.iter().enumerate() {
                    let chosen = i == at;
                    let row = columns.row(row, chosen, &marks);
                    let response = page::text_row(ui, &look, &row, chosen, Sense::click());
                    if chosen && moved {
                        response.scroll_to_me(None);
                    }
                    if response.double_clicked() {
                        keys.extend(click_keys(at, i, true));
                    } else if response.clicked() {
                        keys.extend(click_keys(at, i, false));
                    }
                }
            });
            let first = (shown.state.clamped_offset().y / look.cell.y).round().max(0.0) as usize;
            let visible = (shown.inner_rect.height() / look.cell.y).floor() as usize;
            fn_ui::text::window_label("buildings", first, visible, rows.len())
        },
        |ui| {
            for row in detail_rows(&details[at], width, marks.ellipsis) {
                page::text_row(ui, &look, &row, false, Sense::hover());
            }
        },
    );
    if out.close {
        keys.push(key(KeyCode::Esc));
    }
    keys
}
