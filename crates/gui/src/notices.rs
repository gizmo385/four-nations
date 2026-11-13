//! The corner notices (#945): `fn_ui::notice::Notices` as boxes stacked in the map's top right
//! corner, newest on top, that stay until dismissed. Each is a bordered box (the warning colour
//! for traders, the accent for research, the bad one for a death) with a bold heading and a
//! close button, a line or two under it, and a button for every contextual action it carries
//! (`fn_ui::notice::NoticeAction`). Past the cap a "+N more" row with "Dismiss all" closes the
//! stack. In Select mode `Enter` runs the top notice's first action and `Del` (or `Esc`, with
//! nothing selected) dismisses the top notice (`App`).
//!
//! Like the side panel, this never touches the game: a click comes back as a [`Click`] for the
//! frame to carry out, so a button and its keyboard counterpart leave the same game.

use eframe::egui::{
    self, Align, Align2, Area, Button, Color32, CursorIcon, Frame, Id, Label, Layout, Margin,
    Order, Rect, RichText, Stroke, Ui, vec2,
};
use fn_tui::color::Palette;
use fn_ui::alerts::Kind;
use fn_ui::app::App;
use fn_ui::notice::{Notice, NoticeAction};

use crate::cells::Metrics;
use crate::color::to_egui;

/// A notice box's width and the gap between boxes and from the map's edge, in points.
const WIDTH: f32 = 340.0;
const GAP: f32 = 6.0;

/// What a click on a notice asks for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Click {
    /// The close button of the notice with this id.
    Dismiss(u64),
    /// One of its action buttons.
    Act(NoticeAction),
    /// "Dismiss all", on the "+N more" row.
    DismissAll,
}

/// Carries out `clicks` on `app`.
pub fn apply(app: &mut App, clicks: &[Click]) {
    for click in clicks {
        match *click {
            Click::Dismiss(id) => {
                app.ui.alerts.notices_mut().dismiss(id);
            }
            Click::Act(action) => app.do_notice_action(action),
            Click::DismissAll => app.ui.alerts.notices_mut().dismiss_all(),
        }
    }
}

/// Draws the notices over the map in `area` and returns what was clicked. With `live` false (a
/// box is open over the window) they show but take no click.
pub fn show(
    ctx: &egui::Context,
    area: Rect,
    app: &App,
    palette: &Palette,
    metrics: &Metrics,
    live: bool,
) -> Vec<Click> {
    let mut clicks = Vec::new();
    let notices = app.ui.alerts.notices();
    if !notices.any() {
        return clicks;
    }
    let fill = |rgb| to_egui(palette.token(rgb), Color32::BLACK);
    let width = WIDTH.min(area.width() - 2.0 * GAP).max(0.0);
    let dim = to_egui(palette.token(palette.theme.dim), Color32::GRAY);
    Area::new(Id::unique("corner-notices"))
        .order(Order::Foreground)
        .pivot(Align2::RIGHT_TOP)
        .fixed_pos(area.right_top() + vec2(-GAP, GAP))
        .interactable(live)
        .show(ctx, |ui| {
            ui.spacing_mut().item_spacing = vec2(0.0, GAP);
            ui.set_width(width);
            for notice in notices.shown() {
                let edge = match notice.kind {
                    Kind::Death => palette.theme.bad,
                    Kind::Trade => palette.theme.warn,
                    _ => palette.theme.accent,
                };
                Frame::NONE
                    .fill(fill(palette.theme.inset))
                    .stroke(Stroke::new(1.0, fill(edge)))
                    .inner_margin(Margin::same(6))
                    .show(ui, |ui| {
                        ui.set_width(width - 12.0);
                        one(ui, notice, palette, metrics, &mut clicks);
                    });
            }
            let hidden = notices.hidden();
            if hidden > 0 {
                Frame::NONE.fill(fill(palette.theme.inset)).inner_margin(Margin::same(4)).show(
                    ui,
                    |ui| {
                        ui.horizontal(|ui| {
                            ui.spacing_mut().item_spacing = vec2(8.0, 0.0);
                            let font = metrics.font.clone();
                            let more = RichText::new(format!("+{hidden} more")).font(font.clone());
                            ui.label(more.color(dim));
                            let all = Button::new(RichText::new("Dismiss all").font(font));
                            if ui.add(all.small()).clicked() {
                                clicks.push(Click::DismissAll);
                            }
                        });
                    },
                );
            }
        });
    clicks
}

/// One notice: its words with the close button at the right, its action buttons below.
fn one(
    ui: &mut Ui,
    notice: &Notice,
    palette: &Palette,
    metrics: &Metrics,
    clicks: &mut Vec<Click>,
) {
    let text_colour = to_egui(palette.token(palette.theme.text), Color32::WHITE);
    let font = metrics.font.clone();
    ui.with_layout(Layout::right_to_left(Align::Min), |ui| {
        let close = Button::new(RichText::new("✕").font(font.clone())).frame(false);
        let response = ui.add(close).on_hover_text("Dismiss (Del)");
        if response.hovered() {
            ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
        }
        if response.clicked() {
            clicks.push(Click::Dismiss(notice.id));
        }
        ui.with_layout(Layout::left_to_right(Align::Min), |ui| {
            let title = RichText::new(format!("◆ {}", notice.title)).font(font.clone());
            ui.add(Label::new(title.strong().color(text_colour)).wrap().selectable(false));
        });
    });
    for line in &notice.body {
        let text = RichText::new(line).font(font.clone()).color(text_colour);
        ui.add(Label::new(text).wrap().selectable(false));
    }
    if notice.actions.is_empty() {
        return;
    }
    ui.add_space(metrics.cell.y * 0.25);
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing = vec2(6.0, 4.0);
        for &action in &notice.actions {
            if ui.button(RichText::new(action.label()).font(font.clone())).clicked() {
                clicks.push(Click::Act(action));
            }
        }
    });
}
