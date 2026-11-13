//! Touch controls (#978, docs/design/touch-pwa.md §4): the tool rail down the map's left edge,
//! the action bar in place of the hint bar, and the More sheet, so every keyboard-only action
//! has something to tap. Like the side panel, none of it touches the game: each button is the
//! keys it stands for, which the window feeds the shell as if typed.
//!
//! - **The rail** shows on a touch screen, and on desktop when the player turns it on from the
//!   More sheet. At a window of [`WIDE`] or more its buttons carry the tool's name and key;
//!   below that, icons only. Every button is at least [`TARGET`] points square.
//! - **The action bar** replaces the hint bar while the rail shows: the mode's badge, its
//!   shortest hint, and the mode's buttons ([`actions`]).
//! - **More** (at the action bar's right end, or a corner button on the map when the rail is off) opens
//!   the sheet: speed, the saves, the game menu, and the rail's switch.
//!
//! The key arithmetic ([`rail_keys`], [`actions`], [`speed_keys`], [`menu_keys`]) is pure, so a
//! test checks it without a window.

use eframe::egui::{
    self, Align, Align2, Color32, FontFamily, FontId, Layout, Pos2, Rect, Response, RichText,
    Sense, Stroke, StrokeKind, Ui, WidgetInfo, pos2, vec2,
};
use fn_tui::color::Palette;
use fn_ui::app::App;
use fn_ui::clock::{FAST_SPEEDS, SPEEDS};
use fn_ui::keyev::{KeyCode, KeyEvent, KeyModifiers};
use fn_ui::tools::Tool;

use crate::color::to_egui;

/// The smallest tap target, in points (Apple's and Google's guidance).
pub const TARGET: f32 = 44.0;
/// A window at least this size (points) is a tablet's or a desktop's: names on the rail.
pub const WIDE: (f32, f32) = (1000.0, 600.0);
/// The rail's width: names (wide) or icons only.
pub const RAIL_WIDE: f32 = 76.0;
pub const RAIL_NARROW: f32 = 56.0;
/// The action bar's height.
pub const BAR_HEIGHT: f32 = 40.0;

/// The window's own state for the controls between frames.
#[derive(Clone, Copy, Debug, Default)]
pub struct Controls {
    /// The rail switched on from the More sheet (it always shows on a touch screen).
    pub rail_on: bool,
    /// The More sheet is open.
    pub more_open: bool,
    /// In a compact window (#979), the side panel's drawer is open over the map.
    pub drawer_open: bool,
    /// Lay out for a finger even without a touch screen seen (tests): see [`touch_rows`].
    pub force_touch: bool,
}

/// Where [`set_touch_rows`] keeps this frame's answer.
fn touch_rows_id() -> egui::Id {
    egui::Id::unique("fn_gui touch rows")
}

/// Sets, for this frame, whether list rows are sized for a finger ([`touch_rows`]).
pub fn set_touch_rows(ctx: &egui::Context, on: bool) {
    ctx.data_mut(|d| d.insert_temp(touch_rows_id(), on));
}

/// Whether list rows are sized for a finger this frame (#979): a touch screen, or
/// [`Controls::force_touch`].
pub fn touch_rows(ctx: &egui::Context) -> bool {
    ctx.data(|d| d.get_temp(touch_rows_id())).unwrap_or(false)
}

/// The height of one list row: a cell, or [`TARGET`] when rows are sized for a finger.
pub fn row_height(ctx: &egui::Context, cell: f32) -> f32 {
    if touch_rows(ctx) { cell.max(TARGET) } else { cell }
}

fn key(code: KeyCode) -> KeyEvent {
    KeyEvent::new(code, KeyModifiers::NONE)
}

fn ch(c: char) -> KeyEvent {
    key(KeyCode::Char(c))
}

/// Whether this device has a touch screen: egui has seen a touch, or (in the browser) the
/// device says it has touch points, so the rail is there before the first tap.
pub fn touch_device(ctx: &egui::Context) -> bool {
    if ctx.input(|i| i.has_touch_screen()) {
        return true;
    }
    #[cfg(target_arch = "wasm32")]
    {
        if let Some(w) = web_sys::window() {
            return w.navigator().max_touch_points() > 0;
        }
    }
    false
}

/// Whether the rail shows.
pub fn rail_shown(ctx: &egui::Context, controls: &Controls) -> bool {
    controls.rail_on || touch_device(ctx)
}

/// Whether `window` is wide enough for the named rail.
pub fn wide(window: Rect) -> bool {
    window.width() >= WIDE.0 && window.height() >= WIDE.1
}

/// The `Esc`s that take the map back to plain Select: one drops a corner, one leaves the tool,
/// one deselects. In plain Select, none (one more `Esc` would open the game menu).
fn back_to_select(app: &App) -> Vec<KeyEvent> {
    let n = usize::from(app.ui.anchor.is_some()) + usize::from(app.ui.tool != Tool::Select);
    vec![key(KeyCode::Esc); n]
}

/// The keys a rail button for `tool` stands for: its letter, or for Select the `Esc`s back to
/// it.
pub fn rail_keys(app: &App, tool: Tool) -> Vec<KeyEvent> {
    match tool.letter() {
        Some(letter) => vec![ch(letter.to_ascii_lowercase())],
        None => back_to_select(app),
    }
}

/// One button of the action bar.
#[derive(Clone, Debug, PartialEq)]
pub struct Action {
    /// What it says.
    pub label: &'static str,
    /// What a screen reader says, when the label is a symbol.
    pub name: &'static str,
    pub keys: Vec<KeyEvent>,
    /// Drawn filled: the way out of a tool.
    pub primary: bool,
}

fn action(label: &'static str, name: &'static str, keys: Vec<KeyEvent>) -> Action {
    Action { label, name, keys, primary: false }
}

/// The action bar's buttons for `app`'s mode (docs/design/touch-pwa.md §4): in Select the
/// settlers and the panel's sections, with a tool its Repeat (Harvest) and Done.
pub fn actions(app: &App) -> Vec<Action> {
    if app.ui.tool == Tool::Select && app.ui.anchor.is_none() {
        return vec![
            action("‹", "Previous settler", vec![key(KeyCode::BackTab)]),
            action("›", "Next settler", vec![key(KeyCode::Tab)]),
            action("Research", "Research", vec![ch('t')]),
            action("Nation", "Nation", vec![ch('n')]),
            action("Log", "Log", vec![ch('l')]),
        ];
    }
    let mut out = Vec::new();
    if app.ui.tool == Tool::Harvest {
        out.push(action("Repeat", "Repeat", vec![ch('o')]));
    }
    out.push(Action { primary: true, ..action("Done", "Done", back_to_select(app)) });
    out
}

/// The speeds the clock steps through, as the More sheet offers them.
pub fn speeds(app: &App) -> &'static [u32] {
    if app.session.clock.is_fast() { &FAST_SPEEDS } else { &SPEEDS }
}

/// The keys that set the clock to `target` (a speed on [`speeds`]) and running, or with `None`
/// that pause it: `+` or `-` for each step of the ladder, and `Space` to start or stop.
pub fn speed_keys(app: &App, target: Option<u32>) -> Vec<KeyEvent> {
    let clock = &app.session.clock;
    let Some(target) = target else {
        return if clock.paused { Vec::new() } else { vec![ch(' ')] };
    };
    let ladder = speeds(app);
    let now = ladder.iter().position(|&s| s == clock.speed()).unwrap_or(0);
    let to = ladder.iter().position(|&s| s == target).unwrap_or(now);
    let step = if to > now { '+' } else { '-' };
    let mut keys = vec![ch(step); now.abs_diff(to)];
    if clock.paused {
        keys.push(ch(' '));
    }
    keys
}

/// The keys that open the game menu from wherever the map is: back to plain Select, deselect,
/// then the `Esc` that opens it.
pub fn menu_keys(app: &App) -> Vec<KeyEvent> {
    let mut keys = back_to_select(app);
    if app.ui.selected.is_some() {
        keys.push(key(KeyCode::Esc));
    }
    keys.push(key(KeyCode::Esc));
    keys
}

/// The line icons (original, on a 24-point grid), one polyline per stroke.
fn icon(tool: Tool) -> &'static [&'static [(f32, f32)]] {
    match tool {
        Tool::Select => &[&[(6.0, 3.0), (19.0, 11.0), (13.0, 12.5), (10.0, 19.0), (6.0, 3.0)]],
        Tool::Harvest => &[
            &[(12.5, 7.5), (4.0, 20.0)],
            &[(13.0, 3.0), (20.0, 10.0), (17.0, 11.5), (11.5, 6.0), (13.0, 3.0)],
        ],
        Tool::Remove => &[&[(6.0, 6.0), (18.0, 18.0)], &[(18.0, 6.0), (6.0, 18.0)]],
        Tool::Clear => &[
            &[(14.0, 4.0), (20.0, 10.0)],
            &[(17.0, 7.0), (9.0, 15.0)],
            &[(5.0, 15.0), (9.0, 19.0), (5.0, 20.0), (4.0, 19.0), (5.0, 15.0)],
        ],
        Tool::Stockpile => &[
            &[(4.0, 8.0), (20.0, 8.0), (20.0, 19.0), (4.0, 19.0), (4.0, 8.0)],
            &[(4.0, 12.0), (20.0, 12.0)],
            &[(10.0, 8.0), (10.0, 12.0), (14.0, 12.0), (14.0, 8.0)],
        ],
        Tool::Bend => &[&[
            (12.0, 21.0),
            (7.0, 19.0),
            (5.0, 14.0),
            (7.0, 10.0),
            (9.0, 8.0),
            (10.0, 10.0),
            (12.0, 11.0),
            (13.0, 7.0),
            (16.0, 3.0),
            (17.0, 7.0),
            (19.0, 10.0),
            (19.0, 14.0),
            (17.0, 19.0),
            (12.0, 21.0),
        ]],
        Tool::Build => &[
            &[(4.0, 20.0), (12.5, 11.5)],
            &[
                (10.0, 4.0),
                (16.0, 4.0),
                (19.0, 8.0),
                (16.0, 9.5),
                (14.0, 7.5),
                (11.0, 10.5),
                (8.5, 8.0),
                (10.0, 4.0),
            ],
        ],
        Tool::Sow => &[
            &[(12.0, 21.0), (12.0, 12.0)],
            &[(12.0, 12.0), (9.0, 7.0), (5.0, 6.0), (6.0, 10.0), (12.0, 12.0)],
            &[(12.0, 14.0), (14.0, 10.0), (18.0, 9.0), (17.0, 12.0), (12.0, 14.0)],
        ],
        Tool::Gate => {
            &[&[(6.0, 20.0), (6.0, 6.0), (18.0, 6.0), (18.0, 20.0)], &[(4.0, 9.0), (20.0, 9.0)]]
        }
    }
}

const MORE_ICON: &[&[(f32, f32)]] =
    &[&[(5.0, 7.0), (19.0, 7.0)], &[(5.0, 12.0), (19.0, 12.0)], &[(5.0, 17.0), (19.0, 17.0)]];

fn paint_icon(ui: &Ui, lines: &[&[(f32, f32)]], centre: Pos2, size: f32, colour: Color32) {
    let scale = size / 24.0;
    let origin = centre - vec2(size, size) * 0.5;
    for line in lines {
        let points: Vec<Pos2> =
            line.iter().map(|&(x, y)| origin + vec2(x * scale, y * scale)).collect();
        ui.painter().add(egui::Shape::line(points, Stroke::new(1.8, colour)));
    }
}

/// The colours the controls take from the palette.
struct Look {
    panel: Color32,
    inset: Color32,
    line: Color32,
    text: Color32,
    dim: Color32,
    accent: Color32,
    on_accent: Color32,
}

impl Look {
    fn new(palette: &Palette) -> Self {
        let c = |rgb| to_egui(palette.token(rgb), Color32::GRAY);
        let t = &palette.theme;
        Self {
            panel: c(t.panel),
            inset: c(t.inset),
            line: c(t.accent_soft),
            text: c(t.text),
            dim: c(t.dim),
            accent: c(t.accent),
            on_accent: c(t.on_accent),
        }
    }
}

/// One square button with an icon (and, wide, its name and key under it).
fn rail_button(
    ui: &mut Ui,
    look: &Look,
    size: egui::Vec2,
    lines: &[&[(f32, f32)]],
    label: (&str, &str),
    active: bool,
) -> Response {
    let (rect, response) = ui.allocate_exact_size(size, Sense::click());
    let (name, letter) = label;
    response.widget_info(|| WidgetInfo::labeled(egui::Role::Button, true, name));
    let (bg, fg) = if active { (look.accent, look.on_accent) } else { (look.inset, look.text) };
    let bg = if response.hovered() && !active { look.line } else { bg };
    ui.painter().rect_filled(rect, 6.0, bg);
    ui.painter().rect_stroke(rect, 6.0, Stroke::new(1.0, look.line), StrokeKind::Inside);
    if size.y >= 56.0 {
        paint_icon(ui, lines, rect.center() - vec2(0.0, 8.0), 24.0, fg);
        let text = if letter.is_empty() { name.to_owned() } else { format!("{name} {letter}") };
        ui.painter().text(
            rect.center_bottom() - vec2(0.0, 5.0),
            Align2::CENTER_BOTTOM,
            text,
            FontId::new(10.0, FontFamily::Monospace),
            fg,
        );
    } else {
        paint_icon(ui, lines, rect.center(), 22.0, fg);
        if !letter.is_empty() {
            let colour = if active { look.on_accent } else { look.dim };
            ui.painter().text(
                rect.right_bottom() - vec2(3.0, 2.0),
                Align2::RIGHT_BOTTOM,
                letter,
                FontId::new(9.0, FontFamily::Monospace),
                colour,
            );
        }
    }
    response
}

/// The rail, in `ui` (a left panel): a button per tool. Returns the keys its taps stand for.
pub fn rail(ui: &mut Ui, app: &App, palette: &Palette, wide: bool, live: bool) -> Vec<KeyEvent> {
    let look = Look::new(palette);
    let size = if wide { vec2(RAIL_WIDE - 12.0, 60.0) } else { vec2(TARGET, TARGET) };
    let mut keys = Vec::new();
    ui.spacing_mut().item_spacing = vec2(0.0, 3.0);
    crate::style::scroll_vertical().id_salt("rail").auto_shrink([false, false]).show(ui, |ui| {
        ui.add_space(5.0);
        ui.vertical_centered(|ui| {
            for tool in Tool::ALL {
                let letter = if tool == Tool::Select { "" } else { tool.key() };
                let r = rail_button(
                    ui,
                    &look,
                    size,
                    icon(tool),
                    (tool.name(), letter),
                    app.ui.tool == tool,
                );
                if r.clicked() && live {
                    keys.extend(rail_keys(app, tool));
                }
            }
        });
    });
    keys
}

/// The More button over the map's top-left corner, for when the rail is off.
pub fn corner_more(ctx: &egui::Context, map: Rect, palette: &Palette, controls: &mut Controls) {
    let look = Look::new(palette);
    egui::Area::new(egui::Id::unique("fn_gui more corner"))
        .fixed_pos(map.left_top() + vec2(6.0, 6.0))
        .order(egui::Order::Foreground)
        .show(ctx, |ui| {
            let r = rail_button(ui, &look, vec2(32.0, 32.0), MORE_ICON, ("More", ""), false);
            if r.clicked() {
                controls.more_open = true;
            }
        });
}

/// How tall the floating time controls are ([`float_time`]), so what else sits in the map's
/// top-right corner can start below them.
pub const FLOAT_HEIGHT: f32 = 36.0;

/// In a compact window with the drawer closed (#979, board 1): pause, slower and faster at the
/// map's top-right corner, and More when the rail (whose bar has its own) is off. Returns the
/// keys they stand for.
pub fn float_time(
    ctx: &egui::Context,
    map: Rect,
    app: &App,
    palette: &Palette,
    live: bool,
    rail: bool,
    controls: &mut Controls,
) -> Vec<KeyEvent> {
    let look = Look::new(palette);
    let mut keys = Vec::new();
    let paused = app.session.clock.paused;
    let h = FLOAT_HEIGHT - 4.0;
    egui::Area::new(egui::Id::unique("fn_gui float time"))
        .pivot(egui::Align2::RIGHT_TOP)
        .fixed_pos(map.right_top() + vec2(-6.0, 6.0))
        .order(egui::Order::Foreground)
        .show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 4.0;
                let (label, name) = if paused { ("▶", "Play") } else { ("Pause", "Pause") };
                if text_button(ui, &look, label, name, h, paused).clicked() && live {
                    keys.push(ch(' '));
                }
                if text_button(ui, &look, "-", "Slower", h, false).clicked() && live {
                    keys.push(ch('-'));
                }
                if text_button(ui, &look, "+", "Faster", h, false).clicked() && live {
                    keys.push(ch('+'));
                }
                if !rail {
                    let r = rail_button(ui, &look, vec2(h, h), MORE_ICON, ("More", ""), false);
                    if r.clicked() && live {
                        controls.more_open = true;
                    }
                }
            });
        });
    keys
}

/// The closed drawer's tab on the map's right edge (#979, board 1), with the open-alert count:
/// a tap opens the drawer.
pub fn drawer_tab(
    ctx: &egui::Context,
    map: Rect,
    palette: &Palette,
    alerts: usize,
    controls: &mut Controls,
) {
    let look = Look::new(palette);
    let size = vec2(28.0, 64.0);
    egui::Area::new(egui::Id::unique("fn_gui drawer tab"))
        .fixed_pos(pos2(map.right() - size.x, map.center().y - size.y / 2.0))
        .order(egui::Order::Foreground)
        .show(ctx, |ui| {
            let (rect, r) = ui.allocate_exact_size(size, Sense::click());
            let painter = ui.painter_at(rect);
            let rounding = egui::CornerRadius { nw: 8, sw: 8, ne: 0, se: 0 };
            painter.rect(
                rect,
                rounding,
                look.panel,
                Stroke::new(1.0, look.line),
                StrokeKind::Inside,
            );
            let font = egui::FontId::monospace(16.0);
            painter.text(
                rect.center() - vec2(0.0, 12.0),
                Align2::CENTER_CENTER,
                "‹",
                font.clone(),
                look.text,
            );
            if alerts > 0 {
                let dot = rect.center() + vec2(0.0, 12.0);
                painter.circle_filled(dot, 9.0, look.accent);
                let n = if alerts > 9 { "9+".to_owned() } else { alerts.to_string() };
                painter.text(
                    dot,
                    Align2::CENTER_CENTER,
                    n,
                    egui::FontId::monospace(11.0),
                    look.on_accent,
                );
            }
            let name = match alerts {
                0 => "Open panel".to_owned(),
                1 => "Open panel: 1 alert".to_owned(),
                n => format!("Open panel: {n} alerts"),
            };
            r.widget_info(|| WidgetInfo::labeled(egui::Role::Button, true, name.clone()));
            if r.clicked() {
                controls.drawer_open = true;
            }
        });
}

/// The open drawer's close chevron, in `ui` at `rect`'s top-left. Returns whether it was tapped.
pub fn drawer_close(ui: &mut Ui, palette: &Palette, at: Pos2) -> bool {
    let look = Look::new(palette);
    let rect = Rect::from_min_size(at, vec2(TARGET, 28.0));
    let r = ui.interact(rect, ui.scope_id().with("fn_gui drawer close"), Sense::click());
    let painter = ui.painter_at(rect);
    let colour = if r.hovered() { look.text } else { look.dim };
    painter.text(
        rect.left_center() + vec2(8.0, 0.0),
        Align2::LEFT_CENTER,
        "›",
        egui::FontId::monospace(18.0),
        colour,
    );
    r.widget_info(|| WidgetInfo::labeled(egui::Role::Button, true, "Close panel"));
    r.clicked()
}

/// A text button at least [`TARGET`] wide and `height` tall.
fn text_button(
    ui: &mut Ui,
    look: &Look,
    text: &str,
    name: &str,
    height: f32,
    filled: bool,
) -> Response {
    let (bg, fg) = if filled { (look.accent, look.on_accent) } else { (look.inset, look.text) };
    let button = egui::Button::new(RichText::new(text).color(fg).size(14.0))
        .fill(bg)
        .stroke(Stroke::new(1.0, look.line))
        .corner_radius(6.0)
        .min_size(vec2(TARGET, height));
    let r = ui.add(button);
    r.widget_info(|| WidgetInfo::labeled(egui::Role::Button, true, name));
    r
}

/// The action bar, in `ui` (a bottom panel): the badge, the shortest hint, [`actions`], and
/// More at the right end, which sets `controls.more_open`.
pub fn action_bar(
    ui: &mut Ui,
    app: &App,
    bar: &crate::hints::Bar,
    palette: &Palette,
    live: bool,
    controls: &mut Controls,
) -> Vec<KeyEvent> {
    let look = Look::new(palette);
    let mut keys = Vec::new();
    ui.horizontal_centered(|ui| {
        ui.add_space(8.0);
        ui.label(
            RichText::new(bar.badge.trim())
                .monospace()
                .strong()
                .color(bar.badge_fg)
                .background_color(bar.badge_bg),
        );
        ui.add_space(6.0);
        let hint = bar.message.clone().or_else(|| bar.items.first().map(|h| h.text()));
        let buttons = actions(app);
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            ui.add_space(6.0);
            let size = vec2(BAR_HEIGHT - 8.0, BAR_HEIGHT - 8.0);
            let r = rail_button(
                ui,
                &look,
                size.max(vec2(TARGET, 0.0)),
                MORE_ICON,
                ("More", ""),
                controls.more_open,
            );
            if r.clicked() && live {
                controls.more_open = true;
            }
            // On a narrow bar the last buttons (Log, then Nation...) give way, so the badge
            // stays whole: the keyboard and the More sheet still reach what they did.
            let spacing = ui.spacing().item_spacing.x;
            let pad = ui.spacing().button_padding.x * 2.0;
            let width = |a: &Action| {
                let text = ui.fonts_mut(|f| {
                    f.layout_no_wrap(
                        a.label.to_owned(),
                        egui::FontId::proportional(14.0),
                        look.text,
                    )
                });
                (text.size().x + pad).max(TARGET) + spacing
            };
            let mut shown = buttons.len();
            let room = ui.available_width();
            while shown > 0 && buttons[..shown].iter().map(width).sum::<f32>() > room {
                shown -= 1;
            }
            for a in buttons[..shown].iter().rev() {
                if text_button(ui, &look, a.label, a.name, BAR_HEIGHT - 8.0, a.primary).clicked()
                    && live
                {
                    keys.extend(a.keys.clone());
                }
            }
            if let Some(hint) = hint {
                ui.with_layout(Layout::left_to_right(Align::Center), |ui| {
                    ui.add(
                        egui::Label::new(RichText::new(hint).monospace().color(look.dim))
                            .truncate(),
                    );
                });
            }
        });
    });
    keys
}

/// The More sheet (board 6), while `controls.more_open`. Returns the keys its buttons stand
/// for; Close, a tap outside or any button closes it.
pub fn more_sheet(
    ctx: &egui::Context,
    app: &App,
    palette: &Palette,
    controls: &mut Controls,
    touch: bool,
) -> Vec<KeyEvent> {
    if !controls.more_open {
        return Vec::new();
    }
    let look = Look::new(palette);
    let mut keys = Vec::new();
    let mut close = false;
    let frame = egui::Frame::NONE
        .fill(look.panel)
        .stroke(Stroke::new(1.0, look.line))
        .corner_radius(10.0)
        .inner_margin(14);
    let shown =
        egui::Modal::new(egui::Id::unique("fn_gui more sheet")).frame(frame).show(ctx, |ui| {
            ui.set_width(380.0_f32.min(ctx.content_rect().width() - 40.0));
            crate::style::scroll_vertical()
                .max_height(ctx.content_rect().height() - 60.0)
                .auto_shrink([false, true])
                .show(ui, |ui| {
                    ui.spacing_mut().item_spacing = vec2(6.0, 8.0);
                    ui.horizontal(|ui| {
                        ui.label(RichText::new("More").strong().size(15.0).color(look.text));
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            if text_button(ui, &look, "Close", "Close", TARGET, false).clicked() {
                                close = true;
                            }
                        });
                    });
                    ui.label(RichText::new("SPEED").monospace().size(11.0).color(look.dim));
                    ui.horizontal_wrapped(|ui| {
                        let clock = &app.session.clock;
                        if text_button(ui, &look, "Pause", "Speed: pause", TARGET, clock.paused)
                            .clicked()
                        {
                            keys.extend(speed_keys(app, None));
                        }
                        for &s in speeds(app) {
                            let on = !clock.paused && clock.speed() == s;
                            let label = format!("x{s}");
                            if text_button(
                                ui,
                                &look,
                                &label,
                                &format!("Speed: {label}"),
                                TARGET,
                                on,
                            )
                            .clicked()
                            {
                                keys.extend(speed_keys(app, Some(s)));
                            }
                        }
                    });
                    ui.label(RichText::new("SAVES").monospace().size(11.0).color(look.dim));
                    ui.horizontal_wrapped(|ui| {
                        for (label, k) in [
                            ("Quicksave", key(KeyCode::F(5))),
                            ("Quickload", key(KeyCode::F(9))),
                            ("All saves", key(KeyCode::F(6))),
                        ] {
                            if text_button(ui, &look, label, label, TARGET, false).clicked() {
                                keys.push(k);
                                close = true;
                            }
                        }
                        if text_button(ui, &look, "Game menu", "Game menu", TARGET, false).clicked()
                        {
                            keys.extend(menu_keys(app));
                            close = true;
                        }
                    });
                    if !touch {
                        let label =
                            if controls.rail_on { "Tool rail: on" } else { "Tool rail: off" };
                        if text_button(ui, &look, label, "Tool rail", TARGET, controls.rail_on)
                            .clicked()
                        {
                            controls.rail_on = !controls.rail_on;
                        }
                    }
                });
        });
    if close || shown.should_close() {
        controls.more_open = false;
    }
    keys
}
