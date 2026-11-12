//! The boxes that go over the game (#879): the game menu (`fn_tui::menu`), the confirm box
//! (`fn_tui::ask`) and embark's exit prompt, as [`egui::Modal`]s. The window behind is dimmed and
//! takes no clicks while one is open.
//!
//! Every word is `fn_ui`'s (`fn_ui::menu`, `fn_ui::ask`), the state is the shell's (`menu_cursor`,
//! `menu_page`, `ask_cursor`), and every button is the keys it stands for, handed to the shell
//! after the pass ([`crate::frame::Frame::take_keys`]), so a click and the keyboard take one path:
//!
//! | Box | Click | Keys |
//! |---|---|---|
//! | menu, confirm | a row | `Up`/`Down` to the row, then `Enter` |
//! | menu, confirm | outside the box | `Esc` (Resume, "Keep playing", back from the nations) |
//! | exit prompt | `Title screen`, `Exit`, `Stay` | `T`, `Q`, `Esc` |
//!
//! The rows hold no keyboard focus (`Sense::CLICK` alone), so `Enter` reaches the shell and never
//! a focused button as well. The look is the page frame's ([`crate::page`]): the panel colour, the
//! accent line, the title between two seals, the selected row on the `select` background.

use eframe::egui::{
    self, Color32, Context, Id, Margin, Modal, Sense, Stroke, Ui, WidgetInfo, pos2, vec2,
};
use fn_tui::color::Palette;
use fn_tui::shell::{Mode, Shell};
use fn_tui::widgets::{Marks, Row, Tone};
use fn_ui::app::App;
use fn_ui::ask::{self, ExitAnswer};
use fn_ui::keyev::{KeyCode, KeyEvent, KeyModifiers};
use fn_ui::menu::{self, MenuPage, MenuRow};

use crate::cells::Metrics;
use crate::color::to_egui;
use crate::frame::BoxKind;
use crate::page::{self, Look};

/// How wide each box is, in cells (the terminal's boxes' own widths: `fn_tui::menu::SIZE` and
/// `fn_tui::ask`'s). A window too narrow for it shrinks the box.
const MENU_COLS: u16 = 40;
const ASK_COLS: u16 = 56;
const EXIT_COLS: u16 = 50;

fn key(code: KeyCode) -> KeyEvent {
    KeyEvent::new(code, KeyModifiers::NONE)
}

/// The keys that put a box's highlight on row `to` from row `at` and take it: as many `Up` or
/// `Down` presses as it takes (the boxes' steps stop at the edges, so the count is exact), then
/// `Enter`.
pub fn click_keys(at: usize, to: usize) -> Vec<KeyEvent> {
    let code = if to >= at { KeyCode::Down } else { KeyCode::Up };
    let mut keys = vec![key(code); at.abs_diff(to)];
    keys.push(key(KeyCode::Enter));
    keys
}

/// Shows `kind`'s box over the window and returns the keys this frame's clicks stand for.
pub fn show(
    ctx: &Context,
    shell: &Shell,
    kind: BoxKind,
    palette: &Palette,
    metrics: &Metrics,
) -> Vec<KeyEvent> {
    let look = Look::new(palette, metrics);
    match (kind, &shell.mode) {
        (BoxKind::Menu, Mode::Game(app)) => show_menu(ctx, app, &look),
        (BoxKind::Ask, Mode::Game(app)) => show_ask(ctx, app, &look),
        (BoxKind::Exit, _) => show_exit(ctx, &look),
        _ => Vec::new(),
    }
}

/// The frame every box is in, `cols` cells wide at most, with its title and `body`. Returns the
/// keys of `body`, and `Esc` if the click landed outside the box.
fn modal(
    ctx: &Context,
    look: &Look<'_>,
    cols: u16,
    title: &str,
    body: impl FnOnce(&mut Ui) -> Vec<KeyEvent>,
) -> Vec<KeyEvent> {
    let width = (f32::from(cols) * look.cell.x)
        .min(ctx.content_rect().width() - look.cell.x * 8.0)
        .max(look.cell.x * 12.0);
    let frame = egui::Frame::NONE
        .fill(look.panel)
        .stroke(Stroke::new(2.0, look.line))
        .inner_margin(Margin::symmetric(
            (look.cell.x * 2.0).round() as i8,
            (look.cell.y * 0.75).round() as i8,
        ));
    let shown = Modal::new(Id::unique("fn_gui box"))
        .frame(frame)
        .backdrop_color(Color32::from_black_alpha(150))
        .show(ctx, |ui| {
            ui.set_width(width);
            ui.spacing_mut().item_spacing = vec2(0.0, 0.0);
            heading(ui, look, title);
            ui.add_space(look.cell.y);
            body(ui)
        });
    // The Seal theme's accent brackets at the box's corners (#874), as on the page frame.
    crate::style::corner_brackets(
        &ctx.layer_painter(shown.response.layer_id),
        shown.response.rect,
        to_egui(look.palette.token(look.palette.theme.accent), Color32::WHITE),
    );
    let mut keys = shown.inner;
    // A click outside the box is `Esc`: the safe answer, or back (never a decision).
    if shown.backdrop_response.clicked() {
        keys.push(key(KeyCode::Esc));
    }
    keys
}

/// The title between two seals, centred, in bold (the page frame's faux bold).
fn heading(ui: &mut Ui, look: &Look<'_>, title: &str) {
    let marks = Marks::new(look.palette.ascii);
    let text = format!("{0} {1} {0}", marks.seal, title);
    let (rect, response) =
        ui.allocate_exact_size(vec2(ui.available_width(), look.cell.y), Sense::hover());
    let painter = ui.painter_at(rect);
    let galley = painter.layout_job(look.job(&[(text.clone(), Tone::Title)]));
    let at = pos2(rect.center().x - galley.size().x * 0.5, rect.top());
    painter.galley(at, galley.clone(), Color32::WHITE);
    painter.galley(at + vec2(0.6, 0.0), galley, Color32::WHITE);
    response.widget_info(|| WidgetInfo::labeled(egui::Role::Label, true, text.clone()));
}

/// A row of the box that is only read.
fn line(ui: &mut Ui, look: &Look<'_>, text: &str, tone: Tone) {
    page::text_row(ui, look, &Row::text(text, tone), false, Sense::hover());
}

/// The answers or rows of a list box: the one at `at` highlighted, a click on row `i` giving the
/// keys that take it. `gap` says which rows have a blank line above them.
fn list(ui: &mut Ui, look: &Look<'_>, rows: &[(Row, bool)], at: usize, keys: &mut Vec<KeyEvent>) {
    for (i, (row, gap)) in rows.iter().enumerate() {
        if *gap {
            ui.add_space(look.cell.y);
        }
        if page::text_row(ui, look, row, i == at, Sense::CLICK).clicked() {
            keys.extend(click_keys(at, i));
        }
    }
}

/// The game menu: its rows, or the nation list under Switch nation…, and the game's clock on the
/// bottom edge.
fn show_menu(ctx: &Context, app: &App, look: &Look<'_>) -> Vec<KeyEvent> {
    let page = app.ui.menu_page;
    let rows: Vec<(Row, bool)> = match page {
        MenuPage::Rows => MenuRow::ALL
            .iter()
            .map(|row| {
                let text = row.label_for(app.ui.can_quit);
                (Row::text(text, Tone::Text), row.gap_before())
            })
            .collect(),
        MenuPage::Nations => menu::nation_rows(&app.session)
            .into_iter()
            .map(|(name, playing)| {
                let row = if playing {
                    Row::pair(name, Tone::Text, menu::PLAYING, Tone::Dim)
                } else {
                    Row::text(name, Tone::Text)
                };
                (row, false)
            })
            .collect(),
    };
    let at = app.ui.menu_cursor.min(page.rows() - 1);
    let marks = Marks::new(look.palette.ascii);
    let clock = fn_tui::widgets::clock(&fn_ui::time::clock_text(&app.session), &marks);
    modal(ctx, look, MENU_COLS, page.title(), |ui| {
        let mut keys = Vec::new();
        list(ui, look, &rows, at, &mut keys);
        ui.add_space(look.cell.y);
        let clock = Row::pair("", Tone::Dim, clock, Tone::Dim);
        page::text_row(ui, look, &clock, false, Sense::hover());
        keys
    })
}

/// The confirm box for the open prompt: the question, what follows from it, and the answers.
fn show_ask(ctx: &Context, app: &App, look: &Look<'_>) -> Vec<KeyEvent> {
    let Some(question) = ask::current(&app.session, &app.ui) else { return Vec::new() };
    let at = app.ui.ask_cursor.min(question.answers.len() - 1);
    let answers: Vec<(Row, bool)> =
        question.answers.iter().map(|(_, label)| (Row::text(label, Tone::Text), false)).collect();
    modal(ctx, look, ASK_COLS, question.title, |ui| {
        let mut keys = Vec::new();
        line(ui, look, &question.question, Tone::Strong);
        ui.add_space(look.cell.y);
        for text in &question.lines {
            line(ui, look, text, Tone::Text);
        }
        ui.add_space(look.cell.y);
        list(ui, look, &answers, at, &mut keys);
        keys
    })
}

/// Embark's exit prompt (`Q`): go back to the title screen, exit, or stay. It has no cursor, so
/// each button presses the key it is labelled with.
fn show_exit(ctx: &Context, look: &Look<'_>) -> Vec<KeyEvent> {
    modal(ctx, look, EXIT_COLS, ask::EXIT_TITLE, |ui| {
        let mut keys = Vec::new();
        line(ui, look, ask::EXIT_QUESTION, Tone::Strong);
        ui.add_space(look.cell.y);
        for answer in ExitAnswer::ALL {
            let hint = format!("[{}]", answer.key_name());
            let row = Row::pair(answer.label(), Tone::Text, hint, Tone::Accent);
            if page::text_row(ui, look, &row, false, Sense::CLICK).clicked() {
                keys.push(key(answer.key()));
            }
        }
        keys
    })
}
