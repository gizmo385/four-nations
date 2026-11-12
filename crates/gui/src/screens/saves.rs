//! The Saves screen (`F6`, #885): every slot, newest first, as rows in the page frame, and a text
//! field for naming a new one, from `fn_ui::saves` and the rows of `fn_ui::screens::saves` (the
//! same `Slot::describe` text `fn_tui::saves` draws in cells, and the store behind them: the
//! save directory natively, `localStorage` in the browser, #860).
//!
//! The screen is the shell's own (`UiState::save_slots`, `save_cursor`, `save_name`), so every
//! key works as it does in the terminal, and a click is the keys it stands for
//! ([`click_keys`]): a click on a slot is the `Up` and `Down` presses that put the highlight
//! there, a double-click adds `Enter` (ask to load it), and the buttons are `N` (a new name),
//! `Shift+S` (save over the highlighted slot), `Enter` and `Esc`. Loading and saving over ask
//! first, in the confirm box over the page ([`crate::boxes`]).
//!
//! The name is a real text field (`egui::TextEdit`), but the shell's `UiState::save_name` stays
//! the one copy: each frame the field shows it, and whatever the field changed ([`edit_keys`])
//! goes back as the `Char` and `Backspace` presses that make that change, and `Enter` then saves
//! under it, as typing at the terminal would. While a text field has focus the window hands the
//! shell none of the raw typing (`crate::app`), so a letter isn't typed twice; with the field
//! unfocused the keyboard types the name as it always did.

use eframe::egui::{
    self, Button, CursorIcon, Id, Label, RichText, ScrollArea, Sense, TextEdit, Ui, vec2,
};
use fn_tui::color::Palette;
use fn_tui::widgets::{Marks, Row, Tone};
use fn_ui::app::App;
use fn_ui::keyev::{KeyCode, KeyEvent, KeyModifiers};
use fn_ui::saves::{MAX_NAME, check_name, new_name};
pub use fn_ui::screens::saves::*;

use crate::cells::Metrics;
use crate::page::{self, Look, Spec};

/// The text field's name in the accessibility tree.
pub const NAME_FIELD: &str = "Save name";

fn key(code: KeyCode) -> KeyEvent {
    KeyEvent::new(code, KeyModifiers::NONE)
}

/// The keys that put the highlight on slot `to` from slot `at`, and ask to load it if `pick`.
pub fn click_keys(at: usize, to: usize, pick: bool) -> Vec<KeyEvent> {
    let code = if to > at { KeyCode::Down } else { KeyCode::Up };
    let mut keys = vec![key(code); at.abs_diff(to)];
    if pick {
        keys.push(key(KeyCode::Enter));
    }
    keys
}

/// The presses that turn the name `from` into `to`: `Backspace` for each character past what
/// they share at the start, then the rest of `to` as characters. Whatever the field did (a
/// letter, a paste, a deletion in the middle) comes out as the edits the shell's name takes.
pub fn edit_keys(from: &str, to: &str) -> Vec<KeyEvent> {
    let shared = from.chars().zip(to.chars()).take_while(|(a, b)| a == b).count();
    let mut keys = vec![key(KeyCode::Backspace); from.chars().count() - shared];
    for c in to.chars().skip(shared) {
        let mods = if c.is_uppercase() { KeyModifiers::SHIFT } else { KeyModifiers::NONE };
        keys.push(KeyEvent::new(KeyCode::Char(c), mods));
    }
    keys
}

/// Why `typed` can't be a name yet, as a line under the field (`None`: it can, or nothing is
/// typed, which takes the suggestion).
pub fn name_problem(typed: &str) -> Option<String> {
    (!typed.is_empty() && check_name(typed).is_err())
        .then(|| format!("Use 1 to {MAX_NAME} letters, digits, - and _"))
}

/// Draws the screen into `ui` (the window above the hint bar) and returns the keys this
/// frame's clicks and the name field's edits stand for.
pub fn show(ui: &mut Ui, app: &App, palette: &Palette, metrics: &Metrics) -> Vec<KeyEvent> {
    let look = Look::new(palette, metrics);
    let marks = Marks::new(palette.ascii);
    let slots = &app.ui.save_slots;
    let typing = app.ui.save_name.is_some();
    // Nothing is clicked while a question is up over the page.
    let asked = app.ui.prompt.is_some();
    let live = !typing && !asked;
    let at = app.ui.save_cursor.min(slots.len().saturating_sub(1));
    let mut keys = Vec::new();
    // Whether the name field has been shown since the screen last had no name being typed, so
    // it takes the keyboard once, as it opens.
    let opened = ui.make_persistent_id("saves screen: name field opened");
    if !typing {
        ui.data_mut(|d| d.remove::<bool>(opened));
    }

    // Scrolls to the highlight when it moved (and when the screen opens), not every frame, so
    // the wheel can look elsewhere.
    let seen = ui.make_persistent_id("saves screen: last highlight");
    let moved = ui.data(|d| d.get_temp::<usize>(seen)) != Some(at);
    ui.data_mut(|d| d.insert_temp(seen, at));

    let spec = Spec { title: "Saves", strip_rows: 0 };
    let out = page::show(
        ui,
        &look,
        &spec,
        |ui| {
            buttons(ui, &look, typing, !slots.is_empty(), !asked, &mut keys);
            ui.add_space(look.cell.y);
            if let Some(typed) = &app.ui.save_name {
                name_field(ui, &look, typed, &new_name(slots), opened, &mut keys);
            }
            if slots.is_empty() {
                let note = Row::text("No saves yet: press N to write one.", Tone::Dim);
                page::text_row(ui, &look, &note, false, Sense::hover());
                return None;
            }
            let list = ScrollArea::vertical().id_salt("saves list").auto_shrink([false, false]);
            let shown = list.show(ui, |ui| {
                for (i, slot) in slots.iter().enumerate() {
                    let chosen = i == at;
                    let row = slot_row(slot, &app.session.defs, chosen, &marks);
                    let sense = if live { Sense::click() } else { Sense::hover() };
                    let response = page::text_row(ui, &look, &row, chosen, sense);
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
            fn_ui::text::window_label("slots", first, visible, slots.len())
        },
        |_| {},
    );
    if out.close {
        keys.push(key(KeyCode::Esc));
    }
    keys
}

/// The row of buttons under the title, each the key it stands for: `N`, `Shift+S` and `Enter`
/// on the list; `Enter` and `Esc` while a name is being typed. `live` is whether they take
/// clicks (not with a question up), `any` whether there is a slot to save over or load.
fn buttons(
    ui: &mut Ui,
    look: &Look<'_>,
    typing: bool,
    any: bool,
    live: bool,
    keys: &mut Vec<KeyEvent>,
) {
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing = vec2(look.cell.x * 2.0, 0.0);
        ui.spacing_mut().button_padding = vec2(look.cell.x, 0.0);
        let mut button = |ui: &mut Ui, label: &str, enabled: bool, press: KeyEvent| {
            let text = RichText::new(label).font(look.font.clone());
            let b = Button::new(text).fill(look.panel).min_size(vec2(0.0, look.cell.y));
            let response = ui.add_enabled(enabled && live, b);
            if response.on_hover_cursor(CursorIcon::PointingHand).clicked() {
                keys.push(press);
            }
        };
        if typing {
            button(ui, "Save [Enter]", true, key(KeyCode::Enter));
            button(ui, "Cancel [Esc]", true, key(KeyCode::Esc));
        } else {
            let new = KeyEvent::new(KeyCode::Char('n'), KeyModifiers::NONE);
            button(ui, "New save [N]", true, new);
            let over = KeyEvent::new(KeyCode::Char('S'), KeyModifiers::SHIFT);
            button(ui, "Save over [Shift+S]", any, over);
            button(ui, "Load [Enter]", any, key(KeyCode::Enter));
        }
    });
}

/// The row a new save is named on: "Name:" and the text field holding `typed` (the shell's
/// copy), with `suggestion` as its hint, since `Enter` on nothing saves under it. The field
/// takes the keyboard when it opens (`opened` remembers that it has). A problem with the name is the line under it.
fn name_field(
    ui: &mut Ui,
    look: &Look<'_>,
    typed: &str,
    suggestion: &str,
    opened: Id,
    keys: &mut Vec<KeyEvent>,
) {
    let mut text = typed.to_owned();
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing = vec2(look.cell.x, 0.0);
        ui.add(Label::new(look.job(&[("Name:".to_owned(), Tone::Title)])).selectable(false));
        let hint = RichText::new(suggestion).font(look.font.clone()).weak();
        let field = TextEdit::singleline(&mut text)
            .font(look.font.clone())
            .char_limit(MAX_NAME)
            .hint_text(hint)
            .desired_width((MAX_NAME + 1) as f32 * look.cell.x);
        let response = ui.add(field);
        response.widget_info(|| egui::WidgetInfo::labeled(egui::Role::TextInput, true, NAME_FIELD));
        // Focus on the frame the field first shows, not on every frame: a click elsewhere hands
        // the keyboard back to the shell, which types the name all the same.
        if ui.data(|d| d.get_temp::<bool>(opened)).is_none() {
            response.request_focus();
            ui.data_mut(|d| d.insert_temp(opened, true));
        }
    });
    if text != typed {
        keys.extend(edit_keys(typed, &text));
    }
    if let Some(problem) = name_problem(typed) {
        page::text_row(ui, look, &Row::text(problem, Tone::Warn), false, Sense::hover());
    }
}
