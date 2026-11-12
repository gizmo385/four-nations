//! The hint bar (#876): the one row under the map and the side panel, as egui labels. It says
//! what `fn_tui::draw::draw_status` says, from the same words: the mode's name as a badge on
//! the left; that mode's own keys (`fn_ui::keys::hints_for`), as many whole items as fit; and
//! on the right `PAUSED` while the clock is stopped, then `? keys`. A result notice takes the
//! keys' place while it lasts (and the tail's `? keys` with it).
//!
//! [`bar`] builds the [`Bar`] (no egui context, so a test can read it), [`show`] draws it.

use eframe::egui::text::{LayoutJob, TextFormat};
use eframe::egui::{self, Color32, FontId, Label, RichText, Ui};
use fn_tui::banner;
use fn_tui::color::Palette;
use fn_tui::draw::{hint_mode, mode_label};
use fn_ui::app::App;
use fn_ui::keys::{self, Hint, Mode};

use crate::color::to_egui;

/// What the hint bar says.
#[derive(Clone, Debug)]
pub struct Bar {
    /// The mode's name (or a notice's badge), padded with a space either side.
    pub badge: String,
    pub badge_fg: Color32,
    pub badge_bg: Color32,
    /// A result notice (a save's, a refused key's), in place of the keys.
    pub message: Option<String>,
    /// The mode's keys that fit, in the table's order.
    pub items: Vec<Hint>,
    pub paused: bool,
    /// `? keys`, absent while a notice has the bar.
    pub tail: Option<Hint>,
    pub dim: Color32,
    pub accent: Color32,
    pub fill: Color32,
    pub warn: Color32,
    pub on_accent: Color32,
}

/// The separator between two items.
const SEP: &str = " · ";

impl Bar {
    /// The whole bar as one line of text, left to right: badge, keys or notice, `PAUSED`, tail.
    pub fn text(&self) -> String {
        let mut out = self.badge.trim().to_owned();
        out.push(' ');
        match &self.message {
            Some(m) => out.push_str(m),
            None => out.push_str(&self.items_text()),
        }
        if self.paused {
            out.push_str(" PAUSED");
        }
        if let Some(t) = &self.tail {
            out.push(' ');
            out.push_str(&t.text());
        }
        out
    }

    /// The keys, as the bar shows them: `Enter set a corner · Esc leave`.
    pub fn items_text(&self) -> String {
        self.items.iter().map(Hint::text).collect::<Vec<_>>().join(SEP)
    }
}

/// The bar for `app`'s state, `cols` cells wide (the window's, which is what decides how many
/// items fit).
pub fn bar(app: &App, palette: &Palette, cols: u16) -> Bar {
    let (session, ui) = (&app.session, &app.ui);
    let theme = &palette.theme;
    let colour = |rgb| to_egui(palette.token(rgb), Color32::GRAY);
    let message = (!ui.status.is_empty()).then(|| ui.status.clone());
    let mode = hint_mode(session, ui);

    let (badge, fg, bg) = if message.is_some() {
        let text = ui.status_badge.unwrap_or(" SAVE ");
        let refused = ui.status.contains("failed") || ui.status.starts_with("Can't");
        (text.to_owned(), theme.on_accent, if refused { theme.warn } else { theme.accent })
    } else {
        (format!(" {} ", mode_label(ui, mode)), theme.on_accent, theme.accent)
    };
    let (mut badge_fg, mut badge_bg) = (colour(fg), colour(bg));
    // A tool's badge wears the banner's own tint, so the two match (#307).
    if message.is_none()
        && matches!(mode, Mode::Tool | Mode::ToolCorner)
        && let Some(tint) = banner::tint(ui)
    {
        let style = palette.banner_style(tint);
        badge_fg = style.fg.map_or(badge_fg, |c| to_egui(c, badge_fg));
        badge_bg = style.bg.map_or(badge_bg, |c| to_egui(c, badge_bg));
    }

    let tail = message.is_none().then(keys::tail);
    let paused = session.clock.paused;
    let width = |s: &str| s.chars().count();
    let right =
        tail.as_ref().map_or(0, |t| t.width() + 1) + if paused { width(" PAUSED ") + 1 } else { 0 };
    let start = width(&badge) + 1;
    let room = usize::from(cols).saturating_sub(start + right + 1);
    let items = if message.is_some() {
        Vec::new()
    } else {
        let all = keys::hints_for(mode, ui.tool);
        let n = keys::fit(&all, room, width(SEP));
        all.into_iter().take(n).collect()
    };
    Bar {
        badge,
        badge_fg,
        badge_bg,
        message,
        items,
        paused,
        tail,
        dim: colour(theme.dim),
        accent: colour(theme.accent),
        fill: colour(theme.panel),
        warn: colour(theme.warn),
        on_accent: colour(theme.on_accent),
    }
}

/// Draws `bar` into `ui`, which is the bottom panel's one row.
pub fn show(ui: &mut Ui, bar: &Bar, font: &FontId) {
    ui.spacing_mut().item_spacing.x = 0.0;
    let gap = font.size * 0.6;
    ui.horizontal_centered(|ui| {
        ui.add(
            Label::new(
                RichText::new(&bar.badge)
                    .font(font.clone())
                    .color(bar.badge_fg)
                    .background_color(bar.badge_bg)
                    .strong(),
            )
            .selectable(false),
        );
        ui.add_space(gap);
        let dim = TextFormat { font_id: font.clone(), color: bar.dim, ..Default::default() };
        let mut job = LayoutJob::default();
        match &bar.message {
            Some(m) => job.append(m, 0.0, dim.clone()),
            None => {
                let key = TextFormat { color: bar.accent, ..dim.clone() };
                for (i, item) in bar.items.iter().enumerate() {
                    if i > 0 {
                        job.append(SEP, 0.0, dim.clone());
                    }
                    job.append(&item.keys, 0.0, key.clone());
                    job.append(&format!(" {}", item.verb), 0.0, dim.clone());
                }
            }
        }
        ui.add(Label::new(job).selectable(false));

        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if let Some(tail) = &bar.tail {
                let mut job = LayoutJob::default();
                job.append(&tail.keys, 0.0, TextFormat { color: bar.accent, ..dim.clone() });
                job.append(&format!(" {}", tail.verb), 0.0, dim.clone());
                ui.add(Label::new(job).selectable(false));
                ui.add_space(gap);
            }
            if bar.paused {
                ui.add(
                    Label::new(
                        RichText::new(" PAUSED ")
                            .font(font.clone())
                            .color(bar.on_accent)
                            .background_color(bar.warn)
                            .strong(),
                    )
                    .selectable(false),
                );
                ui.add_space(gap);
            }
        });
    });
}
