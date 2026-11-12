//! The native side panel (#877, docs/design/side-panel-v2.md): `fn_ui::panel::SidePanel`'s
//! words as egui widgets, in the panel's order, in the dark theme and the monospace font.
//!
//! - **Title and header**: the nation between two seals, the run state and speed with their
//!   buttons, and the clock.
//! - **SETTLEMENT**: the glance. Materials are wrapped as far as the width goes (no `+N` fold:
//!   the panel scrolls).
//! - **ALERTS**: each open alert whole, wrapped; a click is the message log's `Enter` on it.
//! - **The tool section**: the active tool's groups of lines, or the tool list (`/`).
//! - **The inspector** is still the terminal's (#882 converts it): its rows are painted as cells
//!   in a slot under the native sections, with its heading the native kind, and it never
//!   scrolls: it takes at most two fifths of the panel and the rest is the sections'.
//!
//! The panel never touches the game. A click returns the keys it stands for ([`Clicks`]), and
//! the window feeds them to the shell as if typed (docs/design/egui-frontend.md, "Mouse"): the
//! section keys `N`, `T`, `G` and `L`, `Space`, `+` and `-` from the header, the tool list's
//! letters, and for an alert `L`, one `Down` for each line above it and `Enter`
//! ([`alert_keys`]). Tones get their colours from `fn_tui::widgets::ToneStyle`, so a word means
//! what it means in the terminal.

use std::cell::Cell;

use eframe::egui::text::{LayoutJob, TextFormat};
use eframe::egui::{
    self, Align, Button, Color32, CursorIcon, FontId, Id, Label, Layout, Margin, RichText,
    ScrollArea, Sense, Stroke, Ui, UiBuilder, vec2,
};
use fn_tui::color::Palette;
use fn_tui::inspector::inspect;
use fn_tui::panel::tool_tint;
use fn_tui::widgets::{Marks, Painter, Row, Tone, ToneStyle};
use fn_ui::alerts::Toast;
use fn_ui::app::App;
use fn_ui::keyev::{KeyCode, KeyEvent, KeyModifiers};
use fn_ui::msglog;
use fn_ui::panel::{
    AlertsSection, ChoiceTones, SETTLEMENT_KEY, Settlement, SidePanel, ToolLine, ToolSection,
};
use fn_ui::state::UiState;
use ratatui::buffer::Buffer;

use crate::cells::{self, CellLayout, Metrics};
use crate::color::to_egui;

/// How much of the panel's height the terminal's inspector may take, as a fraction: two fifths.
const INSPECTOR_SHARE: (usize, usize) = (2, 5);
/// The panel's padding, in points.
const PAD: i8 = 6;

/// The keys the panel's clicks stand for this frame, in order.
pub type Clicks = Vec<KeyEvent>;

/// The terminal's key for the character `c`.
pub fn key(c: char) -> KeyEvent {
    KeyEvent::new(KeyCode::Char(c), KeyModifiers::NONE)
}

/// The panel's model for `app` (what the terminal's own panel is built from too).
pub fn model(app: &App, palette: &Palette) -> SidePanel {
    let marks = Marks::new(palette.ascii);
    SidePanel::build(&app.session, &app.ui, &marks, palette.glyphs.season.winter_food_per_head)
}

/// The keys a click on the open alert `toast` presses: `L` opens the message log at its newest
/// line, a `Down` for each line above the alert's own, and `Enter` goes to where the line is
/// about. A line with no place and no settler (a full stockpile) is where it stops, with the
/// log open on it: `Enter` would only say there is nowhere to go.
pub fn alert_keys(ui: &UiState, toast: &Toast) -> Clicks {
    let lines = msglog::lines(ui.alerts.history(), None, &[]);
    // The line the alert was raised with, else the newest of its kind.
    let at = lines
        .iter()
        .position(|l| l.kind == toast.kind && l.tick == toast.since)
        .or_else(|| lines.iter().position(|l| l.kind == toast.kind));
    let mut keys = vec![key('l')];
    let Some(at) = at else { return keys };
    keys.extend((0..at).map(|_| KeyEvent::new(KeyCode::Down, KeyModifiers::NONE)));
    if lines[at].place.is_some() || lines[at].unit.is_some() {
        keys.push(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
    }
    keys
}

/// What every widget of one frame needs to know.
struct Look<'a> {
    palette: &'a Palette,
    font: FontId,
    /// A row of text, in points.
    ch: f32,
    /// The panel's width in cells: when a pair of texts is too wide for it, they take a row each.
    cols: usize,
    panel: Color32,
    rule: Color32,
    /// Counts the clickable widgets, for their ids.
    next: Cell<u32>,
}

impl Look<'_> {
    fn id(&self, ui: &Ui) -> Id {
        let n = self.next.get();
        self.next.set(n + 1);
        ui.make_persistent_id(("side-row", n))
    }

    fn format(&self, tone: Tone) -> TextFormat {
        let style = tone.style(self.palette);
        let colour = |c| to_egui(c, Color32::WHITE);
        TextFormat {
            font_id: self.font.clone(),
            color: style.fg.map_or(Color32::WHITE, colour),
            // Only the Avatar's tone sits on a colour of its own.
            background: if tone == Tone::Avatar {
                style.bg.map_or(Color32::TRANSPARENT, colour)
            } else {
                Color32::TRANSPARENT
            },
            ..TextFormat::default()
        }
    }

    fn job(&self, spans: &[(String, Tone)]) -> LayoutJob {
        let mut job = LayoutJob::default();
        for (text, tone) in spans {
            job.append(text, 0.0, self.format(*tone));
        }
        job
    }

    fn text(&self, text: &str, tone: Tone) -> LayoutJob {
        self.job(&[(text.to_owned(), tone)])
    }
}

fn chars(spans: &[(String, Tone)]) -> usize {
    spans.iter().map(|(t, _)| t.chars().count()).sum()
}

/// Draws the panel into `ui`, the side panel's slot, and returns the keys its clicks stand for.
/// `live` is false while a box or screen has the keys: the panel still shows but takes no click.
pub fn show(
    ui: &mut Ui,
    app: &App,
    palette: &Palette,
    metrics: &Metrics,
    live: bool,
    inspector_ms: &mut f32,
) -> Clicks {
    let model = model(app, palette);
    let fill = |rgb| to_egui(palette.token(rgb), Color32::BLACK);
    let mut clicks = Clicks::new();
    egui::Frame::NONE.inner_margin(Margin::symmetric(PAD, PAD / 2)).show(ui, |ui| {
        let width = ui.available_width();
        let look = Look {
            palette,
            font: metrics.font.clone(),
            ch: metrics.cell.y,
            cols: metrics.grid_in(vec2(width, 0.0)).0 as usize,
            panel: fill(palette.theme.panel),
            rule: fill(palette.theme.accent_soft),
            next: Cell::new(0),
        };
        ui.spacing_mut().item_spacing = vec2(0.0, 0.0);

        // The inspector's rows are the terminal's (#882): fitted into the slot under the
        // native sections, which scroll in what is left.
        let marks = Marks::new(palette.ascii);
        let inspector = inspect(&app.session, &app.ui, model.choice, look.cols, &marks);
        let total = ui.available_height();
        let rows = (total / look.ch).floor() as usize;
        let share = rows * INSPECTOR_SHARE.0 / INSPECTOR_SHARE.1;
        // Its heading, its first rows, and a line for the seed under them.
        let wanted = 1 + inspector.all_rows().len();
        let slot = wanted.min(share.max(4)).min(rows.saturating_sub(6));
        let top = (total - (slot + 1) as f32 * look.ch).max(look.ch * 4.0);

        ScrollArea::vertical()
            .id_salt("side-sections")
            .max_height(top)
            .auto_shrink([false, false])
            .show(ui, |ui| {
                title(ui, &look, &model.title);
                header(ui, &look, &model, app.session.clock.paused, &mut clicks);
                glance(ui, &look, &model.settlement, &mut clicks);
                alerts(ui, &look, &model.alerts, app, &mut clicks);
                if let Some(tool) = &model.tool {
                    tool_section(ui, &look, tool, &mut clicks);
                }
            });

        let started = web_time::Instant::now();
        let fitted = inspector.fitted(slot.saturating_sub(1));
        heading(ui, &look, &fitted.title, &fitted.right, Tone::Heading, None, &mut clicks);
        let rect = ui.allocate_space(vec2(width, fitted.rows.len().max(1) as f32 * look.ch)).1;
        let area =
            ratatui::layout::Rect::new(0, 0, look.cols as u16, fitted.rows.len().max(1) as u16);
        let mut buf = Buffer::empty(area);
        let mut p = Painter::new(&mut buf, area, palette);
        for row in &fitted.rows {
            p.row(row);
        }
        cells::paint(&ui.painter_at(rect), rect.min, metrics, &CellLayout::new(&buf));
        *inspector_ms = started.elapsed().as_secs_f32() * 1000.0;

        let seed = look.text(&model.footer, Tone::Dim);
        ui.with_layout(Layout::right_to_left(Align::Min), |ui| {
            ui.add(Label::new(seed).selectable(false));
        });
    });
    if !live {
        clicks.clear();
    }
    clicks
}

/// The nation between two seals, centred.
fn title(ui: &mut Ui, look: &Look<'_>, text: &str) {
    ui.vertical_centered(|ui| {
        ui.add(Label::new(look.text(text, Tone::Title)).selectable(false));
    });
}

/// A button for the header: a key as a click.
fn button(ui: &mut Ui, look: &Look<'_>, text: &str, hint: &str) -> bool {
    let text = RichText::new(text).font(look.font.clone());
    let response =
        ui.add(Button::new(text).min_size(vec2(look.ch * 1.6, look.ch))).on_hover_text(hint);
    response.clicked()
}

/// The run state and speed with their buttons, then the clock.
fn header(ui: &mut Ui, look: &Look<'_>, model: &SidePanel, paused: bool, clicks: &mut Clicks) {
    let state = &model.header[0];
    ui.horizontal(|ui| {
        spans(ui, look, state, false);
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            // Right to left: the last added sits leftmost.
            if button(ui, look, "+", "+ faster") {
                clicks.push(key('+'));
            }
            if button(ui, look, "-", "- slower") {
                clicks.push(key('-'));
            }
            let (word, hint) =
                if paused { ("Play", "Space: run") } else { ("Pause", "Space: pause") };
            if button(ui, look, word, hint) {
                clicks.push(key(' '));
            }
        });
    });
    for row in &model.header[1..] {
        row_widget(ui, look, row, None, clicks);
    }
}

/// One label of a row's left spans, and the right-hand text (if `with_right`) flush right.
fn spans(ui: &mut Ui, look: &Look<'_>, row: &Row, with_right: bool) {
    ui.add(Label::new(look.job(&row.left)).selectable(false));
    if let (true, Some((text, tone))) = (with_right, &row.right) {
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            ui.add(Label::new(look.text(text, *tone)).selectable(false));
        });
    }
}

/// One row of toned text: its left spans, and its right-hand text flush right (under it, when
/// the two don't fit together). `click` makes the whole row a key.
fn row_widget(ui: &mut Ui, look: &Look<'_>, row: &Row, click: Option<char>, clicks: &mut Clicks) {
    let right = row.right.as_ref().map_or(0, |(t, _)| t.chars().count() + 1);
    let stacked = right > 0 && chars(&row.left) + right > look.cols;
    let rect = if stacked {
        let first = ui.add(Label::new(look.job(&row.left)).selectable(false));
        let mut rect = first.rect;
        if let Some((text, tone)) = &row.right {
            let second = ui.with_layout(Layout::right_to_left(Align::Min), |ui| {
                ui.add(Label::new(look.text(text, *tone)).selectable(false))
            });
            rect = rect.union(second.response.rect);
        }
        rect.with_max_x(ui.max_rect().right())
    } else {
        ui.horizontal(|ui| spans(ui, look, row, true)).response.rect
    };
    make_clickable(ui, look, rect, click, clicks);
}

/// Makes `rect` press `key` when clicked.
fn make_clickable(
    ui: &mut Ui,
    look: &Look<'_>,
    rect: egui::Rect,
    key: Option<char>,
    out: &mut Clicks,
) {
    let Some(c) = key else { return };
    let response = ui.interact(rect, look.id(ui), Sense::click());
    if response.hovered() {
        ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
    }
    if response.clicked() {
        out.push(self::key(c));
    }
}

/// A section's heading: its title in `tone` on the left, the rule across, and the right-hand
/// text (a key, `[N]`) at the end, which presses `click` when that is a key.
fn heading(
    ui: &mut Ui,
    look: &Look<'_>,
    title: &str,
    right: &str,
    tone: Tone,
    click: Option<char>,
    clicks: &mut Clicks,
) {
    let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), look.ch), Sense::hover());
    ui.painter().hline(rect.x_range(), rect.center().y, Stroke::new(1.0, look.rule));
    let gap = |ui: &mut Ui, add: &mut dyn FnMut(&mut Ui)| {
        egui::Frame::NONE.fill(look.panel).inner_margin(Margin::symmetric(4, 0)).show(ui, add);
    };
    ui.scope_builder(
        UiBuilder::new().max_rect(rect).layout(Layout::left_to_right(Align::Center)),
        |ui| {
            ui.spacing_mut().item_spacing.x = 0.0;
            gap(ui, &mut |ui| {
                ui.add(Label::new(look.text(&title.to_uppercase(), tone)).selectable(false));
            });
            if right.is_empty() {
                return;
            }
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                gap(ui, &mut |ui| {
                    let label = Label::new(look.text(right, Tone::Text)).selectable(false);
                    let label = if click.is_some() { label.sense(Sense::click()) } else { label };
                    let response = ui.add(label);
                    if click.is_some() && response.hovered() {
                        ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
                    }
                    if let (Some(c), true) = (click, response.clicked()) {
                        clicks.push(key(c));
                    }
                });
            });
        },
    );
}

/// The Settlement glance.
fn glance(ui: &mut Ui, look: &Look<'_>, s: &Settlement, clicks: &mut Clicks) {
    heading(ui, look, &s.title, &s.right, Tone::Heading, key_of(SETTLEMENT_KEY), clicks);
    row_widget(ui, look, &s.people, None, clicks);
    for row in s.beasts.iter().chain(&s.need_flags) {
        row_widget(ui, look, row, None, clicks);
    }
    row_widget(ui, look, &s.food, None, clicks);
    if let Some(row) = &s.winter {
        row_widget(ui, look, row, None, clicks);
    }
    // Every kind of material, as many to a row as the width holds.
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing.x = look.font.size * 1.2;
        for item in &s.materials {
            ui.add(Label::new(look.text(item, Tone::Text)).selectable(false));
        }
    });
    row_widget(ui, look, &s.research, Some('t'), clicks);
    if let Some(row) = &s.research_bill {
        row_widget(ui, look, row, Some('t'), clicks);
    }
    for row in &s.gate {
        row_widget(ui, look, row, Some('g'), clicks);
    }
}

/// The key a heading's `[X]` names, as the terminal's.
fn key_of(bracketed: &str) -> Option<char> {
    let c = bracketed.strip_prefix('[')?.strip_suffix(']')?;
    let mut chars = c.chars();
    let c = chars.next()?;
    chars.next().is_none().then(|| c.to_ascii_lowercase())
}

/// The key a heading's right-hand text ends with: `[L]` in "2 · [L]".
fn trailing_key(right: &str) -> Option<char> {
    key_of(right.rsplit(' ').next()?)
}

/// The open alerts, each whole and wrapped, a gap between them.
fn alerts(ui: &mut Ui, look: &Look<'_>, a: &AlertsSection, app: &App, clicks: &mut Clicks) {
    heading(ui, look, &a.title, &a.right, Tone::Heading, trailing_key(&a.right), clicks);
    // The model's entries are `Alerts::open`'s, one for one.
    let open = app.ui.alerts.open();
    for (i, entry) in a.entries.iter().enumerate() {
        if i > 0 {
            ui.add_space(look.ch * 0.5);
        }
        let label = Label::new(look.text(&entry.text, entry.tone)).wrap().selectable(false);
        let response = ui.add(label.sense(Sense::click()));
        if response.hovered() {
            ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
        }
        if response.clicked()
            && let Some(toast) = open.get(i)
        {
            clicks.extend(alert_keys(&app.ui, toast));
        }
    }
}

/// The Tool section: its heading in the tool's own tint, then its groups of lines.
fn tool_section(ui: &mut Ui, look: &Look<'_>, t: &ToolSection, clicks: &mut Clicks) {
    let tone = if t.list {
        Tone::Heading
    } else {
        tool_tint(t).map_or(Tone::Heading, |tint| Tone::Custom(look.palette.tint_rgb(tint)))
    };
    // Only the list's `/` is a click: the tool's own letter, pressed in its own tool, does
    // nothing worth a button.
    let click = if t.list { Some('/') } else { None };
    heading(ui, look, &t.title, &t.right, tone, click, clicks);
    for (i, block) in t.blocks.iter().enumerate() {
        if i > 0 && !block.is_empty() {
            ui.add_space(look.ch * 0.5);
        }
        for line in block {
            tool_line(ui, look, line, clicks);
        }
    }
}

fn tool_line(ui: &mut Ui, look: &Look<'_>, line: &ToolLine, clicks: &mut Clicks) {
    match line {
        ToolLine::Text(text, tone) => {
            ui.add(Label::new(look.text(text, *tone)).wrap().selectable(false));
        }
        ToolLine::Setting { label, value, value_tone } => {
            let job = look.job(&[(format!("{label}: "), Tone::Text), (value.clone(), *value_tone)]);
            ui.add(Label::new(job).wrap().selectable(false));
        }
        ToolLine::Choice { selected, key: k, name, what, tones } => {
            choice(ui, look, (*selected, *k, name, what), *tones, clicks);
        }
        ToolLine::Building(row) | ToolLine::Ability(row) => {
            row_widget(ui, look, row, None, clicks);
        }
    }
}

/// A row of the tool list: the pointer, the tool's letter, its name and what it is for. A click
/// presses the tool's letter, which picks it.
fn choice(
    ui: &mut Ui,
    look: &Look<'_>,
    (selected, k, name, what): (bool, char, &str, &str),
    tones: ChoiceTones,
    clicks: &mut Clicks,
) {
    let pointer = Marks::new(look.palette.ascii).pointer;
    let head = if selected { format!("{pointer} ") } else { "  ".to_owned() };
    let job = look.job(&[
        (head, tones.head),
        (format!("{k} "), tones.key),
        (format!("{name:<9}  "), tones.name),
        (what.to_owned(), tones.what),
    ]);
    let response = ui.add(Label::new(job).truncate().selectable(false).sense(Sense::click()));
    if response.hovered() {
        ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
    }
    if response.clicked() {
        clicks.push(key(k.to_ascii_lowercase()));
    }
}
