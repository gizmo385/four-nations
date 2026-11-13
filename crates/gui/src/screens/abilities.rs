//! The abilities screen (`I`, #886): `fn_ui::bend::abilities` as a list in the page frame, and the
//! highlighted ability in full words in the strip under it, as `fn_tui::abilities` draws them
//! from the same rows (`fn_ui::screens::abilities`: the columns, the detail's lines).
//!
//! The screen is the shell's own (`UiState::ability_cursor`), so every key works as it does in
//! the terminal, and a click is the keys it stands for ([`click_keys`]): a click on a row is as
//! many `Up` or `Down` presses as it takes to put the highlight there, a double-click adds
//! `Enter`, which picks the ability for the Bend tool and closes the screen (one that can't be
//! used yet does nothing, and the strip says why), and the close button is `Esc`.
//!
//! The list scrolls (wheel, or the highlight moving past an edge); the position of the window
//! over it ("abilities 1-9 of 12") is on the frame's bottom edge once it does.

use eframe::egui::{Sense, Ui, vec2};
use fn_tui::color::Palette;
use fn_tui::widgets::{Marks, Row, Tone};
use fn_ui::app::App;
use fn_ui::bend::abilities;
use fn_ui::keyev::{KeyCode, KeyEvent, KeyModifiers};
pub use fn_ui::screens::abilities::*;

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
    let rows = abilities(&app.session);
    let mut keys = Vec::new();

    if rows.is_empty() {
        let spec = Spec { title: "Abilities", strip_rows: 0 };
        let nothing = Row::text("No abilities yet.", Tone::Dim);
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

    let at = app.ui.ability_cursor.min(rows.len() - 1);
    // The strip is as tall as the tallest ability's detail (as the terminal's), so the rule
    // holds still as the highlight moves.
    let geo = page::geometry(ui.available_rect_before_wrap(), look.cell, 0);
    let width = geo.cols(look.cell);
    let strip_rows = detail_height(&rows, width, palette.ascii, geo.rows(look.cell) as u16);
    let columns = Columns::of(&rows);
    // Scrolls to the highlight when it moved (and when the screen opens), not every frame, so
    // the wheel can look elsewhere.
    let seen = ui.make_persistent_id("abilities screen: last highlight");
    let moved = ui.data(|d| d.get_temp::<usize>(seen)) != Some(at);
    ui.data_mut(|d| d.insert_temp(seen, at));

    let spec = Spec { title: "Abilities", strip_rows: usize::from(strip_rows) };
    let out = page::show(
        ui,
        &look,
        &spec,
        |ui| {
            let header = columns.header(width, marks.ellipsis);
            page::text_row(ui, &look, &header, false, Sense::hover());
            let list = crate::style::scroll_vertical()
                .id_salt("abilities list")
                .auto_shrink([false, false]);
            let shown = list.show(ui, |ui| {
                ui.spacing_mut().item_spacing = vec2(0.0, 0.0);
                for (i, row) in rows.iter().enumerate() {
                    let chosen = i == at;
                    let row = columns.row(row, chosen, width, &marks);
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
            fn_ui::text::window_label("abilities", first, visible, rows.len())
        },
        |ui| {
            for row in detail_rows(&rows[at], width, marks.ellipsis) {
                page::text_row(ui, &look, &row, false, Sense::hover());
            }
        },
    );
    if out.close {
        keys.push(key(KeyCode::Esc));
    }
    keys
}
