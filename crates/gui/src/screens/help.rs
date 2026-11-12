//! The key help (`?`, #888): the keys of `fn_ui::input::help_rows` and the legend of the map's
//! glyphs, as the two tabs of one page, from the same words `fn_tui::screens` draws in cells
//! (`fn_ui::screens::{help_groups, HELP_TABS}`, `fn_ui::legend`, `fn_tui::legend`).
//!
//! The screen is the shell's own (`UiState::help_tab`), so every key works as it does in the
//! terminal, and a click is the key it stands for: the other tab's button is `Tab`, and the
//! close button is `Esc`.
//!
//! - **Keys**: a heading per group, a row per key with the keys in a column and the wording
//!   wrapped as egui wraps it. A page [`TWO_COLUMNS_FROM`] cells wide or more sets the groups
//!   in two columns, split where the taller column is shortest, as the terminal does.
//! - **Legend**: the glyphs of the map in view in the colours the map draws them in, under a
//!   heading per group, in as many columns as fit ([`fn_ui::legend::layout`]).
//!
//! Both scroll with the wheel.

use eframe::egui::{
    self, Align, Button, Color32, CursorIcon, Layout, Rect, RichText, ScrollArea, Sense, Ui,
    WidgetInfo, pos2, vec2,
};
use fn_tui::color::Palette;
use fn_tui::legend::{LegendEntry, entries};
use fn_tui::widgets::{Row, Tone};
use fn_ui::app::App;
use fn_ui::input::HelpRow;
use fn_ui::keyev::{KeyCode, KeyEvent, KeyModifiers};
use fn_ui::legend::{LegendGroup, LegendLine, cell_width, layout};
use fn_ui::screens::{HELP_TABS, TAB_NOTE, TWO_COLUMNS_FROM, help_groups, title};
use fn_ui::state::{HelpTab, Screen};
use ratatui::style::Modifier;

use crate::cells::Metrics;
use crate::color::to_egui;
use crate::page::{self, Look, Spec};

fn key(code: KeyCode) -> KeyEvent {
    KeyEvent::new(code, KeyModifiers::NONE)
}

/// Draws the screen into `ui` (the window above the hint bar) and returns the keys this frame's
/// clicks stand for.
pub fn show(ui: &mut Ui, app: &App, palette: &Palette, metrics: &Metrics) -> Vec<KeyEvent> {
    let look = Look::new(palette, metrics);
    let mut keys = Vec::new();
    let spec = Spec { title: title(Screen::Help), strip_rows: 0 };
    let out = page::show(
        ui,
        &look,
        &spec,
        |ui| {
            keys.extend(tabs(ui, &look, app.ui.help_tab));
            ui.add_space(look.cell.y);
            ScrollArea::vertical()
                .id_salt(("help", app.ui.help_tab as u8))
                .auto_shrink([false, false])
                .show(ui, |ui| match app.ui.help_tab {
                    HelpTab::Keys => keys_tab(ui, &look),
                    HelpTab::Legend => legend_tab(ui, &look, app, palette),
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

/// The tab strip: a button per tab with the open one selected, and what `Tab` does at the right.
/// The tab that is not open is `Tab`.
fn tabs(ui: &mut Ui, look: &Look<'_>, open: HelpTab) -> Vec<KeyEvent> {
    let mut keys = Vec::new();
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing = vec2(look.cell.x, 0.0);
        for (tab, name) in HELP_TABS {
            let tone = if tab == open { Tone::Title } else { Tone::Dim };
            let text = RichText::new(name).font(look.font.clone()).color(look.format(tone).color);
            let button = Button::new(text).selected(tab == open).min_size(vec2(0.0, look.cell.y));
            if ui.add(button).on_hover_cursor(CursorIcon::PointingHand).clicked() && tab != open {
                keys.push(key(KeyCode::Tab));
            }
        }
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            let dim = look.format(Tone::Dim).color;
            ui.label(RichText::new(TAB_NOTE).font(look.font.clone()).color(dim));
        });
    });
    keys
}

/// A group's title, as every heading of the help is: upper case in the title tone.
fn heading(ui: &mut Ui, look: &Look<'_>, text: &str) {
    let row = Row::text(text.to_uppercase(), Tone::Title);
    page::text_row(ui, look, &row, false, Sense::hover());
}

/// Where to split the groups, by their rows, between two columns so the taller is shortest.
pub fn split(heights: &[usize]) -> usize {
    (0..=heights.len())
        .min_by_key(|&at| {
            let left: usize = heights[..at].iter().sum();
            let right: usize = heights[at..].iter().sum();
            left.max(right)
        })
        .unwrap_or(0)
}

fn keys_tab(ui: &mut Ui, look: &Look<'_>) {
    let groups = help_groups();
    let cols = (ui.available_width() / look.cell.x).floor() as usize;
    if cols < TWO_COLUMNS_FROM {
        for (group, rows) in &groups {
            group_block(ui, look, group.title(), rows);
        }
        return;
    }
    // Each group is its heading, its rows and a blank one.
    let heights: Vec<usize> = groups.iter().map(|(_, rows)| rows.len() + 2).collect();
    let at = split(&heights);
    ui.columns(2, |columns| {
        for (i, (group, rows)) in groups.iter().enumerate() {
            let column = &mut columns[usize::from(i >= at)];
            group_block(column, look, group.title(), rows);
        }
    });
}

/// A group: its heading, a row per key (the keys in a column as wide as the group's longest
/// label, the wording wrapped beside them) and a blank row.
fn group_block(ui: &mut Ui, look: &Look<'_>, title: &str, rows: &[HelpRow]) {
    heading(ui, look, title);
    let keys_cols = rows.iter().map(|r| r.keys.chars().count()).max().unwrap_or(0);
    let keys_w = (keys_cols + 2) as f32 * look.cell.x;
    for row in rows {
        let width = ui.available_width();
        let what = ui.painter().layout(
            row.what.clone(),
            look.font.clone(),
            look.format(Tone::Text).color,
            (width - keys_w).max(look.cell.x),
        );
        let height = what.size().y.max(look.cell.y);
        let (rect, response) = ui.allocate_exact_size(vec2(width, height), Sense::hover());
        let painter = ui.painter_at(rect);
        let accent = painter.layout_job(look.job(&[(row.keys.clone(), Tone::Accent)]));
        page::paint_galley(&painter, rect.min, accent, false);
        page::paint_galley(&painter, rect.min + vec2(keys_w, 0.0), what, false);
        let text = format!("{}  {}", row.keys, row.what);
        response.widget_info(|| WidgetInfo::labeled(egui::Role::Label, true, text.clone()));
    }
    ui.add_space(look.cell.y);
}

fn legend_tab(ui: &mut Ui, look: &Look<'_>, app: &App, palette: &Palette) {
    let found = entries(&app.session, &app.ui, palette);
    let groups: Vec<LegendGroup> = found.iter().map(|e| e.group).collect();
    let longest = found.iter().map(|e| e.name.chars().count()).max().unwrap_or(0);
    let cell = cell_width(longest);
    let cols = (ui.available_width() / look.cell.x).floor() as usize;
    for line in layout(&groups, cell, cols) {
        match line {
            LegendLine::Blank => {
                ui.add_space(look.cell.y);
            }
            LegendLine::Heading(group) => heading(ui, look, group.title()),
            LegendLine::Cells(row) => {
                let (rect, _) =
                    ui.allocate_exact_size(vec2(ui.available_width(), look.cell.y), Sense::hover());
                for (i, &e) in row.iter().enumerate() {
                    let x = (i * (cell + fn_ui::legend::GAP)) as f32 * look.cell.x;
                    entry(ui, look, &found[e], pos2(rect.left() + x, rect.top()), cell);
                }
            }
        }
    }
}

/// One glyph as the map draws it (its colours, bold if it is), and its name beside it.
fn entry(ui: &mut Ui, look: &Look<'_>, e: &LegendEntry, at: egui::Pos2, cell: usize) {
    let area = Rect::from_min_size(at, vec2(cell as f32 * look.cell.x, look.cell.y));
    let painter = ui.painter_at(area);
    let tint =
        |c: Option<ratatui::style::Color>, fallback| c.map_or(fallback, |c| to_egui(c, fallback));
    let glyph = Rect::from_min_size(at, look.cell);
    if let Some(bg) = e.style.bg {
        painter.rect_filled(glyph, 0.0, to_egui(bg, Color32::TRANSPARENT));
    }
    let bold = e.style.add_modifier.contains(Modifier::BOLD);
    let mut format = look.format(Tone::Text);
    format.color = tint(e.style.fg, Color32::WHITE);
    let mut job = egui::text::LayoutJob::default();
    job.append(&e.glyph.to_string(), 0.0, format);
    page::paint_galley(&painter, at, painter.layout_job(job), bold);
    let name = painter.layout_job(look.job(&[(e.name.clone(), Tone::Text)]));
    page::paint_galley(&painter, at + vec2(2.0 * look.cell.x, 0.0), name, false);
    let label = format!("{} {}", e.glyph, e.name);
    ui.interact(
        area,
        ui.scope_id().with(("legend", e.group as u8, e.name.clone())),
        Sense::hover(),
    )
    .widget_info(|| WidgetInfo::labeled(egui::Role::Label, true, label.clone()));
}
