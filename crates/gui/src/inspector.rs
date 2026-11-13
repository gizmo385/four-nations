//! The native inspector (#882, docs/design/side-panel-v2.md "The inspector"): the last section
//! of the side panel, `fn_ui::inspector::Inspected` as egui widgets. The model is the terminal's
//! too (its rows are `fn_tui::inspector`'s), so a settler reads the same in both: task, what it
//! carries, the five meters, rank progress and the tile it stands on, or, with no settler
//! selected, the tile under the cursor.
//!
//! What a window can do that cells can't:
//!
//! - **A meter is a bar** in the `ok`, `warn` and `bad` tokens, with the word "low" after any
//!   that is not ok (so it reads without colour). Chi is in the accent and never warns.
//! - **Nothing is trimmed**: paragraphs wrap to the panel's width and the body scrolls in its
//!   own slot (`side::show`), where the terminal drops rows from the bottom.
//! - **The heading is a name**, in its own case: it is pinned above the scroll, so what the
//!   inspector describes is always in sight.

use eframe::egui::{Align, Button, CursorIcon, Label, Layout, ProgressBar, RichText, Ui, vec2};
use fn_tui::widgets::Tone;
use fn_ui::inspector::{Inspected, InspectorAction, Part};

use crate::side::{Clicks, Look, heading_cased, key};

/// A meter bar is this many cells wide, as in the terminal.
const BAR_CELLS: f32 = 10.0;
/// The label column of a meter ("Hunger"), in cells.
const LABEL_CELLS: usize = 6;
/// The percentage column, in cells ("100%").
const PERCENT_CELLS: f32 = 4.0;

/// The heading's pieces for a panel `cols` cells wide: the title (cut with an ellipsis when it
/// can't fit), the right-hand value, and what the heading gave to the body's first rows because
/// it had no room: the whole title, then the value.
fn split(cols: usize, model: &Inspected) -> (String, String, Vec<(String, Tone)>) {
    let (t, v) = (model.title.chars().count(), model.right.chars().count());
    // The rule's gap and padding take a few cells either side.
    let room = cols.saturating_sub(4);
    if t <= room && (model.right.is_empty() || t + v + 3 <= room) {
        return (model.title.clone(), model.right.clone(), Vec::new());
    }
    if t <= room {
        return (model.title.clone(), String::new(), vec![(model.right.clone(), Tone::Dim)]);
    }
    let mut cut: String = model.title.chars().take(room.saturating_sub(1)).collect();
    cut.push('…');
    let mut moved = vec![(model.title.clone(), Tone::Strong)];
    if !model.right.is_empty() {
        moved.push((model.right.clone(), Tone::Dim));
    }
    (cut, String::new(), moved)
}

/// The pinned heading: what the inspector describes, and the tile's position or the settler's
/// rank at its end.
pub(crate) fn title(ui: &mut Ui, look: &Look<'_>, model: &Inspected) {
    let (title, right, _) = split(look.cols, model);
    let mut none = Clicks::new();
    heading_cased(ui, look, (&title, false), &right, Tone::Heading, None, &mut none);
}

/// The groups of the inspector, a half-row gap between neighbours. Put it in a scroll area.
pub(crate) fn body(ui: &mut Ui, look: &Look<'_>, model: &Inspected, clicks: &mut Clicks) {
    let (_, _, moved) = split(look.cols, model);
    let mut first = true;
    let mut gap = |ui: &mut Ui| {
        if !std::mem::replace(&mut first, false) {
            ui.add_space(look.ch * 0.5);
        }
    };
    if !moved.is_empty() {
        gap(ui);
        for (text, tone) in &moved {
            ui.add(Label::new(look.text(text, *tone)).wrap().selectable(false));
        }
    }
    for group in model.groups.iter().filter(|g| !g.is_empty()) {
        gap(ui);
        for part in group {
            show_part(ui, look, part, clicks);
        }
    }
}

fn show_part(ui: &mut Ui, look: &Look<'_>, part: &Part, clicks: &mut Clicks) {
    match part {
        Part::Text(text, tone) | Part::Line(text, tone) => {
            ui.add(Label::new(look.text(text, *tone)).wrap().selectable(false));
        }
        Part::Standing { text, tail } => {
            let text = format!("{text} {tail}");
            ui.add(Label::new(look.text(&text, Tone::Dim)).wrap().selectable(false));
        }
        Part::Meter { label, frac, tone, low } => meter(ui, look, label, *frac, *tone, *low),
        Part::Action(action) => button(ui, look, *action, clicks),
    }
}

/// An action of the inspector as a button ("Keep it", #967). Its click comes back as the key the
/// terminal presses for it, as every click of the panel does.
fn button(ui: &mut Ui, look: &Look<'_>, action: InspectorAction, clicks: &mut Clicks) {
    let text = RichText::new(action.label()).font(look.font.clone());
    let response = ui.add(Button::new(text).small());
    if response.hovered() {
        ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
    }
    if response.clicked()
        && let Some(c) = action.key().chars().next()
    {
        clicks.push(key(c.to_ascii_lowercase()));
    }
}

/// One meter: `Hunger ▕██████░░░░▏ 72% low`. The bar wears the tone of the status; "low" is said
/// in words too, as in the terminal, so a bar reads without its colour.
fn meter(ui: &mut Ui, look: &Look<'_>, label: &str, frac: f32, tone: Tone, low: bool) {
    let fill = look.format(tone).color;
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = look.cw;
        let name = format!("{label:<LABEL_CELLS$}");
        ui.add(Label::new(look.text(name.trim_end(), Tone::Dim)).selectable(false));
        // The label column is fixed, so the bars line up.
        ui.add_space((LABEL_CELLS - label.chars().count().min(LABEL_CELLS)) as f32 * look.cw);
        ui.add(
            ProgressBar::new(frac.clamp(0.0, 1.0))
                .desired_width(look.cw * BAR_CELLS)
                .desired_height(look.ch * 0.5)
                .corner_radius(0)
                .fill(fill)
                .animate(false),
        );
        let percent = format!("{:.0}%", frac * 100.0);
        ui.allocate_ui_with_layout(
            vec2(look.cw * PERCENT_CELLS, look.ch),
            Layout::right_to_left(Align::Center),
            |ui| {
                ui.add(Label::new(look.text(&percent, Tone::Text)).selectable(false));
            },
        );
        if low {
            ui.add(Label::new(look.text("low", tone)).selectable(false));
        }
    });
}
