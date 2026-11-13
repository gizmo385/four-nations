//! The title screen as egui widgets (#880): the steps of `fn_tui::title` (TITLE, NATION, SEED,
//! LOAD), painted natively from the same [`TitleScreen`]. The state, the keys and the words are
//! `fn_tui`'s and `fn_ui`'s; this is a second painter for them, and it adds no state.
//!
//! Every click is a key press the step already answers (docs/design/egui-frontend.md, "Mouse"):
//! [`show`] returns the keys, and the frame hands them to `Shell::key` after the pass, so a click
//! path and a keyboard path are the same commands:
//!
//! | Step | Click | Keys |
//! |---|---|---|
//! | TITLE | a row | `Up`/`Down` to the row, then `Enter` |
//! | NATION | a nation's row (double-click) | its digit `1`-`4` (then `Enter`) |
//! | NATION | `Choose` / `Back` | `Enter` / `Esc` |
//! | SEED | a row | `Up`/`Down` to the row |
//! | SEED | a row's `◂` / `▸` | `Up`/`Down` to the row, then `Left` / `Right` |
//! | SEED | `Embark` / `Back` | `Enter` / `Esc` |
//! | LOAD | a save (double-click) | `Up`/`Down` to it (then `Enter`) |
//! | LOAD | `Back` | `Esc` |
//!
//! The look: a centred column on the window's dark ground (the seal and the name, then the step in
//! an accent-framed box), in the bundled monospace font, wearing the colours of the nation being
//! picked (`fn_ui::theme`). Rows are painted by hand (square, flat) and hold no keyboard focus, so
//! `Enter` reaches the shell and not a focused button.

use eframe::egui::{
    self, Align2, Color32, FontFamily, FontId, Label, Margin, Rect, Response, RichText, ScrollArea,
    Sense, Stroke, StrokeKind, Ui, UiBuilder, pos2, vec2,
};
use fn_sim::{Defs, Nation};
use fn_tui::color::Palette;
use fn_tui::title::{MenuItem, Page, SeedRow, TitleScreen, seed_row_hint};
use fn_ui::keyev::{KeyCode, KeyEvent, KeyModifiers};
use fn_ui::rows::Marks;
use fn_ui::screens::title as words;
use fn_ui::theme::{Rgb, Theme};

/// The column's widest, in points, and the height the column is centred on (the header and the
/// tallest step), so the header stays where it is while the box under it changes between steps.
const COLUMN: f32 = 560.0;
const NOMINAL_HEIGHT: f32 = 460.0;
/// Space between the box's frame and its rows.
const MARGIN: f32 = 20.0;
const ROW: f32 = 40.0;
const BODY: f32 = 16.0;
/// The least room a row's text keeps clear of the row's edge, so a cut line ends inside it.
const ROW_PAD: f32 = 12.0;

// -----------------------------------------------------------------------------------------
// Click -> keys
// -----------------------------------------------------------------------------------------

fn press(code: KeyCode) -> KeyEvent {
    KeyEvent::new(code, KeyModifiers::NONE)
}

/// `Up` or `Down` presses that take a highlight from `from` to `to` (the shell's steps stop at
/// the edges, so the count is exact).
fn steps(from: usize, to: usize) -> Vec<KeyEvent> {
    let code = if to >= from { KeyCode::Down } else { KeyCode::Up };
    vec![press(code); from.abs_diff(to)]
}

/// The keys a click on menu row `row` presses: the arrows to it, then `Enter`.
pub fn menu_click(cursor: usize, row: usize) -> Vec<KeyEvent> {
    let mut keys = steps(cursor, row);
    keys.push(press(KeyCode::Enter));
    keys
}

/// The keys a click on nation `index` (0 to 3) presses: its digit, and `Enter` too for a
/// double-click.
pub fn nation_click(index: usize, confirm: bool) -> Vec<KeyEvent> {
    let digit = char::from(b'1' + u8::try_from(index.min(8)).unwrap_or(0));
    let mut keys = vec![press(KeyCode::Char(digit))];
    if confirm {
        keys.push(press(KeyCode::Enter));
    }
    keys
}

/// The keys that move the seed step's highlight from row `from` to row `to`.
pub fn seed_move(from: SeedRow, to: SeedRow) -> Vec<KeyEvent> {
    steps(seed_index(from), seed_index(to))
}

fn seed_index(row: SeedRow) -> usize {
    SeedRow::ALL.iter().position(|&r| r == row).unwrap_or(0)
}

/// A click on a seed row's `◂` (`dir < 0`) or `▸`: the arrows to the row, then `Left` or `Right`.
pub fn seed_change(from: SeedRow, to: SeedRow, dir: i32) -> Vec<KeyEvent> {
    let mut keys = seed_move(from, to);
    keys.push(press(if dir < 0 { KeyCode::Left } else { KeyCode::Right }));
    keys
}

/// The keys a click on save `row` presses: the arrows to it, and `Enter` for a double-click.
pub fn load_click(cursor: usize, row: usize, confirm: bool) -> Vec<KeyEvent> {
    let mut keys = steps(cursor, row);
    if confirm {
        keys.push(press(KeyCode::Enter));
    }
    keys
}

// -----------------------------------------------------------------------------------------
// Look
// -----------------------------------------------------------------------------------------

/// A theme token as egui draws it.
fn rgb(c: Rgb) -> Color32 {
    let b = |v: f32| (v.clamp(0.0, 1.0) * 255.0).round() as u8;
    Color32::from_rgb(b(c[0]), b(c[1]), b(c[2]))
}

/// The colours of one nation's theme, as egui's.
#[derive(Clone, Copy)]
struct Look {
    accent: Color32,
    accent_text: Color32,
    soft: Color32,
    select: Color32,
    inset: Color32,
    panel: Color32,
    text: Color32,
    dim: Color32,
    warn: Color32,
    on_accent: Color32,
}

impl Look {
    fn new(t: &Theme) -> Self {
        Self {
            accent: rgb(t.accent),
            accent_text: rgb(t.accent_text),
            soft: rgb(t.accent_soft),
            select: rgb(t.select),
            inset: rgb(t.inset),
            panel: rgb(t.panel),
            text: rgb(t.text),
            dim: rgb(t.dim),
            warn: rgb(t.warn),
            on_accent: rgb(t.on_accent),
        }
    }
}

fn mono(size: f32) -> FontId {
    FontId::new(size, FontFamily::Monospace)
}

/// A clickable row of `height`, as wide as the box. `label` is what a screen reader (and a test)
/// finds it by. `selected` is the step's highlight (the pointer's row); hovering tints it too.
/// `paint` draws the contents over the row's rectangle.
fn row(
    ui: &mut Ui,
    id: (&str, usize),
    label: &str,
    look: &Look,
    height: f32,
    selected: bool,
    paint: impl FnOnce(&egui::Painter, Rect),
) -> Response {
    let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), height), Sense::hover());
    let response = ui.interact(rect, ui.scope_id().with(("title row", id)), Sense::CLICK);
    response.widget_info(|| egui::WidgetInfo::selected(egui::Role::Button, true, selected, label));
    let hovered = response.hovered();
    let (fill, stroke) = match (selected, hovered) {
        (true, _) => (look.select, Stroke::new(1.0, look.accent)),
        (false, true) => (look.inset, Stroke::new(1.0, look.accent)),
        (false, false) => (look.inset, Stroke::new(1.0, look.soft)),
    };
    let painter = ui.painter();
    painter.rect(rect, 0.0, fill, stroke, StrokeKind::Inside);
    paint(painter, rect);
    if hovered {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    response
}

/// A button: `Enter`-sized text in a flat box, filled with the accent when `primary`.
fn button(ui: &mut Ui, label: &str, look: &Look, primary: bool, width: f32) -> Response {
    let (rect, _) = ui.allocate_exact_size(vec2(width, ROW), Sense::hover());
    let response = ui.interact(rect, ui.scope_id().with(("title button", label)), Sense::CLICK);
    response.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, true, label));
    let hovered = response.hovered();
    let (fill, stroke, ink) = if primary {
        let fill = if hovered { look.accent_text } else { look.accent };
        (fill, Stroke::new(1.0, look.accent_text), look.on_accent)
    } else if hovered {
        (look.select, Stroke::new(1.0, look.accent), look.accent_text)
    } else {
        (look.inset, Stroke::new(1.0, look.soft), look.text)
    };
    let painter = ui.painter();
    painter.rect(rect, 0.0, fill, stroke, StrokeKind::Inside);
    painter.text(rect.center(), Align2::CENTER_CENTER, label, mono(BODY), ink);
    if hovered {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    response
}

/// `text` laid out on one line in `font`, cut short with a `…` when it would be wider than
/// `max` points (#943): a row's text never spills past the row, whatever a save is called.
pub fn fitted(
    painter: &egui::Painter,
    text: &str,
    font: FontId,
    colour: Color32,
    max: f32,
) -> std::sync::Arc<egui::Galley> {
    let whole = painter.layout_no_wrap(text.to_owned(), font.clone(), colour);
    if whole.size().x <= max {
        return whole;
    }
    let chars: Vec<char> = text.chars().collect();
    let cut = |n: usize| format!("{}…", chars[..n].iter().collect::<String>().trim_end());
    // The most characters that still fit with the ellipsis; width only grows with length.
    let (mut lo, mut hi) = (0, chars.len());
    while lo < hi {
        let mid = (lo + hi).div_ceil(2);
        if painter.layout_no_wrap(cut(mid), font.clone(), colour).size().x <= max {
            lo = mid;
        } else {
            hi = mid - 1;
        }
    }
    painter.layout_no_wrap(cut(lo), font, colour)
}

/// Text centred in `rect`, `dy` points off the middle, with the pointer `▸` to its left when
/// `pointer` is set. It is cut short with an ellipsis to fit the row, less a margin each side.
fn centred(
    painter: &egui::Painter,
    rect: Rect,
    text: &str,
    font: FontId,
    colour: Color32,
    dy: f32,
    pointer: Option<(&str, Color32)>,
) {
    let galley = fitted(painter, text, font.clone(), colour, rect.width() - 2.0 * ROW_PAD);
    let at = Align2::CENTER_CENTER.anchor_size(rect.center() + vec2(0.0, dy), galley.size());
    painter.galley(at.min, galley, colour);
    if let Some((mark, ink)) = pointer {
        painter.text(pos2(at.min.x - 10.0, at.center().y), Align2::RIGHT_CENTER, mark, font, ink);
    }
}

/// Text starting at `x`, `dy` points off the row's middle, cut short to end before the row's
/// right padding.
fn left_text(
    painter: &egui::Painter,
    rect: Rect,
    x: f32,
    text: &str,
    font: FontId,
    colour: Color32,
    dy: f32,
) {
    let galley = fitted(painter, text, font, colour, rect.right() - ROW_PAD - x);
    let at = Align2::LEFT_CENTER.anchor_size(pos2(x, rect.center().y + dy), galley.size());
    painter.galley(at.min, galley, colour);
}

// -----------------------------------------------------------------------------------------
// The screen
// -----------------------------------------------------------------------------------------

/// Draws the title screen into `ui` (the window's whole central panel) and returns the keys the
/// frame's clicks pressed, in order.
pub fn show(ui: &mut Ui, screen: &TitleScreen, defs: &Defs, palette: &Palette) -> Vec<KeyEvent> {
    let look = Look::new(&palette.theme);
    let mut keys = Vec::new();
    let avail = ui.available_rect_before_wrap();
    ui.painter().rect_filled(avail, 0.0, look.inset);

    let width = (avail.width() - 32.0).clamp(240.0, COLUMN);
    let top = ((avail.height() - NOMINAL_HEIGHT) / 2.0).max(16.0);
    let column = Rect::from_min_max(
        pos2(avail.center().x - width / 2.0, avail.top() + top),
        pos2(avail.center().x + width / 2.0, avail.bottom()),
    );
    ui.scope_builder(UiBuilder::new().max_rect(column), |ui| {
        ScrollArea::vertical().id_salt("title scroll").auto_shrink([false, true]).show(ui, |ui| {
            header(ui, &look, palette);
            ui.add_space(20.0);
            let frame = egui::Frame::NONE
                .fill(look.panel)
                .stroke(Stroke::new(1.0, look.soft))
                .inner_margin(Margin::same(MARGIN as i8));
            frame.show(ui, |ui| {
                ui.set_width(width - 2.0 * MARGIN - 2.0);
                ui.spacing_mut().item_spacing = vec2(8.0, 8.0);
                match &screen.page {
                    Page::Menu => menu(ui, screen, defs, &look, &mut keys),
                    Page::Nation { cursor } => nation(ui, defs, *cursor, &look, &mut keys),
                    Page::Seed { .. } => seed(ui, screen, defs, &look, &mut keys),
                    Page::Load => load(ui, screen, defs, &look, &mut keys),
                }
            });
        });
    });
    keys
}

/// `◆ FOUR NATIONS ◆`, the terminal's own title, large and in the accent.
fn header(ui: &mut Ui, look: &Look, palette: &Palette) {
    let seal = Marks::new(palette.ascii).seal;
    ui.vertical_centered(|ui| {
        ui.add(
            Label::new(
                RichText::new(format!("{seal} FOUR NATIONS {seal}"))
                    .font(mono(32.0))
                    .color(look.accent_text)
                    .strong(),
            )
            .selectable(false),
        );
    });
}

fn dim(look: &Look, text: impl Into<String>) -> Label {
    Label::new(RichText::new(text.into()).font(mono(BODY - 2.0)).color(look.dim)).selectable(false)
}

fn heading(ui: &mut Ui, text: impl Into<String>, colour: Color32) {
    ui.add(Label::new(RichText::new(text.into()).font(mono(18.0)).color(colour)).selectable(false));
    ui.add_space(4.0);
}

/// The menu: the welcome line, one row per item, then a load failure's note.
fn menu(ui: &mut Ui, screen: &TitleScreen, defs: &Defs, look: &Look, keys: &mut Vec<KeyEvent>) {
    ui.add(dim(look, words::WELCOME));
    ui.add_space(8.0);
    for (i, item) in screen.items().into_iter().enumerate() {
        let selected = i == screen.cursor;
        // Continue says which save it would load.
        let detail = match (item, &screen.continue_slot) {
            (MenuItem::Continue, Some(slot)) => Some(slot.summary(defs, "·")),
            _ => None,
        };
        let height = if detail.is_some() { 58.0 } else { 46.0 };
        let r = row(ui, ("menu", i), item.label(), look, height, selected, |p, rect| {
            let ink = if selected { look.accent_text } else { look.text };
            let pointer = selected.then_some(("▸", look.accent));
            match &detail {
                Some(d) => {
                    centred(p, rect, item.label(), mono(20.0), ink, -9.0, pointer);
                    centred(p, rect, d, mono(12.0), look.dim, 14.0, None);
                }
                None => centred(p, rect, item.label(), mono(20.0), ink, 0.0, pointer),
            }
        });
        if r.clicked() {
            keys.extend(menu_click(screen.cursor, i));
        }
    }
    if !screen.status.is_empty() {
        ui.add_space(4.0);
        ui.add(
            Label::new(RichText::new(&screen.status).font(mono(BODY - 2.0)).color(look.warn))
                .selectable(false),
        );
    }
}

/// The nation step: four rows each in its nation's own colours, the chosen one's description,
/// then Back and Choose.
fn nation(ui: &mut Ui, defs: &Defs, cursor: usize, look: &Look, keys: &mut Vec<KeyEvent>) {
    ui.add(dim(look, words::NATION_PROMPT));
    ui.add_space(8.0);
    let cursor = cursor % Nation::ALL.len();
    for (i, n) in Nation::ALL.into_iter().enumerate() {
        let def = defs.nation(n);
        let theirs = Look::new(&Theme::for_nation(def.color));
        let selected = i == cursor;
        let text = format!("{}. {}", i + 1, def.name);
        let r = row(ui, ("nation", i), &def.name, &theirs, ROW + 4.0, selected, |p, rect| {
            let swatch =
                Rect::from_center_size(pos2(rect.left() + 22.0, rect.center().y), vec2(14.0, 14.0));
            p.rect_filled(swatch, 0.0, theirs.accent);
            let ink = if selected { theirs.accent_text } else { look.text };
            if selected {
                p.text(
                    pos2(rect.left() + 38.0, rect.center().y),
                    Align2::LEFT_CENTER,
                    "▸",
                    mono(18.0),
                    theirs.accent,
                );
            }
            p.text(
                pos2(rect.left() + 58.0, rect.center().y),
                Align2::LEFT_CENTER,
                &text,
                mono(18.0),
                ink,
            );
        });
        if r.clicked() {
            keys.extend(nation_click(i, r.double_clicked()));
        }
    }
    ui.add_space(8.0);
    let chosen = defs.nation(Nation::ALL[cursor]);
    ui.add(
        Label::new(RichText::new(&chosen.description).font(mono(BODY - 2.0)).color(look.text))
            .selectable(false)
            .wrap(),
    );
    ui.add_space(12.0);
    buttons(
        ui,
        look,
        ("Back", KeyCode::Esc),
        (&format!("Choose {}", chosen.name), KeyCode::Enter),
        keys,
    );
}

/// Back on the left and the step's own way forward on the right, each pressing its key.
fn buttons(
    ui: &mut Ui,
    look: &Look,
    back: (&str, KeyCode),
    next: (&str, KeyCode),
    keys: &mut Vec<KeyEvent>,
) {
    let gap = ui.spacing().item_spacing.x;
    let side = (ui.available_width() - gap) / 2.0;
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = gap;
        if button(ui, back.0, look, false, side).clicked() {
            keys.push(press(back.1));
        }
        if button(ui, next.0, look, true, side).clicked() {
            keys.push(press(next.1));
        }
    });
}

/// The seed step: its four rows, the highlighted row's hint, then Back and Embark.
fn seed(ui: &mut Ui, screen: &TitleScreen, defs: &Defs, look: &Look, keys: &mut Vec<KeyEvent>) {
    let Page::Seed { nation, digits, row: at, size_idx, survey, difficulty_idx, .. } = &screen.page
    else {
        return;
    };
    let (at, size_idx, difficulty_idx) = (*at, *size_idx, *difficulty_idx);
    heading(ui, words::seed_heading(&defs.nation(*nation).name), look.accent_text);

    let size = defs.embark.map_sizes.get(size_idx).map_or("Small", |s| s.name.as_str());
    let difficulty =
        defs.difficulty.presets.get(difficulty_idx).map_or("Normal", |p| p.name.as_str());
    let rows = [
        (SeedRow::Digits, words::SEED_LABEL, format!("{digits}_")),
        (SeedRow::Size, words::SIZE_LABEL, size.to_owned()),
        (SeedRow::Survey, words::SURVEY_LABEL, words::survey_name(*survey).to_owned()),
        (SeedRow::Difficulty, words::DIFFICULTY_LABEL, difficulty.to_owned()),
    ];
    for (which, label, value) in rows {
        let selected = which == at;
        let id = ("seed", seed_index(which));
        let mut change = 0;
        let r = row(ui, id, label, look, ROW, selected, |p, rect| {
            let ink = if selected { look.accent_text } else { look.text };
            let y = rect.center().y;
            if selected {
                p.text(
                    pos2(rect.left() + 10.0, y),
                    Align2::LEFT_CENTER,
                    "▸",
                    mono(BODY),
                    look.accent,
                );
            }
            p.text(pos2(rect.left() + 28.0, y), Align2::LEFT_CENTER, label, mono(BODY), ink);
        });
        let rect = r.rect;
        let painter = ui.painter().clone();
        if which == SeedRow::Digits {
            painter.text(
                pos2(rect.right() - 14.0, rect.center().y),
                Align2::RIGHT_CENTER,
                &value,
                mono(BODY),
                look.text,
            );
        } else {
            // `◂ value ▸`: the arrows are buttons of their own, drawn over the row.
            for (dir, name, x) in
                [(-1, "Previous", rect.right() - 168.0), (1, "Next", rect.right() - 34.0)]
            {
                let hit =
                    Rect::from_min_size(pos2(x, rect.top() + 4.0), vec2(26.0, rect.height() - 8.0));
                let arrow =
                    ui.interact(hit, ui.scope_id().with(("title arrow", id, dir)), Sense::CLICK);
                arrow.widget_info(|| {
                    egui::WidgetInfo::labeled(egui::Role::Button, true, format!("{name} {label}"))
                });
                let ink = if arrow.hovered() { look.accent_text } else { look.dim };
                if arrow.hovered() {
                    painter.rect_filled(hit, 0.0, look.select);
                }
                painter.text(
                    hit.center(),
                    Align2::CENTER_CENTER,
                    if dir < 0 { "◂" } else { "▸" },
                    mono(BODY),
                    ink,
                );
                if arrow.clicked() {
                    change = dir;
                }
            }
            painter.text(
                pos2(rect.right() - 101.0, rect.center().y),
                Align2::CENTER_CENTER,
                &value,
                mono(BODY),
                look.text,
            );
        }
        if change != 0 {
            keys.extend(seed_change(at, which, change));
        } else if r.clicked() {
            keys.extend(seed_move(at, which));
        }
    }
    ui.add_space(4.0);
    // A fixed three lines, so the buttons stay put as the hint's length changes.
    let hint = seed_row_hint(at, defs, size_idx, *survey, difficulty_idx);
    ui.allocate_ui(vec2(ui.available_width(), 3.0 * 20.0), |ui| {
        ui.add(dim(look, hint).wrap());
    });
    ui.add_space(8.0);
    buttons(ui, look, ("Back", KeyCode::Esc), ("Embark", KeyCode::Enter), keys);
}

/// The Load step: every save, newest first, each with its name and the short line the Continue row
/// shows (#943), both cut to the row.
fn load(ui: &mut Ui, screen: &TitleScreen, defs: &Defs, look: &Look, keys: &mut Vec<KeyEvent>) {
    if screen.save_slots.is_empty() {
        ui.add(dim(look, words::NO_SAVES));
    } else {
        let cursor = screen.load_cursor.min(screen.save_slots.len() - 1);
        // The list scrolls to the highlight only when a key moved it, not on every frame.
        let id = ui.scope_id().with("title load cursor");
        let moved = ui.data_mut(|d| {
            let before = d.get_temp::<usize>(id);
            d.insert_temp(id, cursor);
            before != Some(cursor)
        });
        ScrollArea::vertical().id_salt("title saves").max_height(360.0).show(ui, |ui| {
            for (i, slot) in screen.save_slots.iter().enumerate() {
                let selected = i == cursor;
                let (name_ink, ink) = match (slot.loadable(), selected) {
                    (false, _) => (look.dim, look.dim),
                    (true, true) => (look.accent_text, look.text),
                    (true, false) => (look.text, look.dim),
                };
                let r = row(ui, ("save", i), &slot.name, look, 52.0, selected, |p, rect| {
                    let x = rect.left() + 28.0;
                    if selected {
                        p.text(
                            pos2(rect.left() + 10.0, rect.center().y),
                            Align2::LEFT_CENTER,
                            "▸",
                            mono(BODY),
                            look.accent,
                        );
                    }
                    left_text(p, rect, x, &slot.name, mono(BODY), name_ink, -9.0);
                    left_text(p, rect, x, &slot.summary(defs, "·"), mono(12.0), ink, 11.0);
                });
                if selected && moved {
                    r.scroll_to_me(None);
                }
                if r.clicked() {
                    keys.extend(load_click(cursor, i, r.double_clicked()));
                }
            }
        });
        if !screen.status.is_empty() {
            ui.add(
                Label::new(RichText::new(&screen.status).font(mono(BODY - 2.0)).color(look.warn))
                    .selectable(false),
            );
        }
    }
    ui.add_space(8.0);
    if screen.save_slots.is_empty() {
        let width = ui.available_width();
        if button(ui, "Back", look, false, width).clicked() {
            keys.push(press(KeyCode::Esc));
        }
    } else {
        buttons(ui, look, ("Back", KeyCode::Esc), ("Load", KeyCode::Enter), keys);
    }
}
