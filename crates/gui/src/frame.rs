//! The window's frame (#876): what is native and what is cells, and the layout of the native
//! part (docs/design/egui-frontend.md, "The window" and "Native and cell-painted surfaces").
//!
//! Each frame [`surface`] asks one question of the shell, with an exhaustive `match` over
//! [`Mode`], [`Screen`], [`Modal`] and [`Prompt`] (a new screen in the TUI fails to compile here
//! until someone says which it is), and [`Frame::show`] lays out the answer:
//!
//! - [`Surface::Native`]: the game as egui panels. A one-row hint bar along the bottom
//!   ([`crate::hints`]), the side panel on the right (300 points, resizable from 260 to 440) and
//!   the map in the middle ([`MapView`]) with the tool banner on its top edge. The side panel
//!   is native ([`crate::side`], inspector included).
//! - [`Surface::Page`]: a full screen as egui widgets in the page frame ([`crate::page`]): the
//!   whole window above the hint bar, the side panel and the map hidden. Build (#881), Research
//!   (#884), Abilities and Traders (#886), Saves (#885), the Nation roster and Endeavor (#887),
//!   the message log and the key help (#888): every full screen.
//! - [`Surface::Title`] and [`Surface::Embark`]: the title's steps and the embark screen, as
//!   egui widgets of their own ([`crate::title`], [`crate::embark`]) with the hint bar below.
//! - [`Surface::CellsFull`]: the whole terminal frame, at the window's size in cells.
//!
//! Over any of them goes at most one box ([`modal`], [`crate::boxes`]): the game menu, a confirm
//! box or embark's exit prompt, as an `egui::Modal` that dims the window and takes its clicks.
//!
//! The shell is always sized to the window in cells ([`Shell::resize`]), so a page's pagination
//! and a box's size match what the cell painter shows. Native surfaces draw no map cells at
//! all: the terminal's own map (10 000 glyphs in a game frame) is only drawn when the whole
//! frame is.

use web_time::Instant;

use eframe::egui::{
    self, Align2, Color32, FontFamily, FontId, Id, LayerId, Order, Rect, RichText, Ui, UiBuilder,
    vec2,
};
use fn_tui::color::Palette;
use fn_tui::layout::{MIN_HEIGHT, MIN_WIDTH};
use fn_tui::shell::{self, Mode, Shell};
use fn_tui::{banner, title};
use fn_ui::app::App;
use fn_ui::keyev::KeyEvent;
use fn_ui::state::{BANNER_HEIGHT, Modal, Prompt, Screen, UiState};
use ratatui::buffer::Buffer;

use crate::boxes;
use crate::cells::{self, CellLayout, Metrics};
use crate::color::to_egui;
use crate::embark::{self, EmbarkView};
use crate::hints;
use crate::mapview::MapView;
use crate::screens;
use crate::side;

/// The side panel's width on a fresh window, and the range the user can drag it through.
pub const PANEL_DEFAULT: f32 = 300.0;
pub const PANEL_RANGE: (f32, f32) = (260.0, 440.0);

/// Which box [`modal`] puts over the surface, as an [`egui::Modal`] ([`crate::boxes`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BoxKind {
    /// The game menu (`fn_tui::menu`).
    Menu,
    /// A confirm box (`fn_tui::ask`), for any [`Prompt`].
    Ask,
    /// Embark's exit prompt (`Shell::exit_prompt`).
    Exit,
}

/// Which native full screen a [`Surface::Page`] shows.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PageKind {
    /// The Build picker (`fn_gui::screens::build`).
    Build,
    /// The message log (`fn_gui::screens::log`).
    Log,
    /// The key help and its legend (`fn_gui::screens::help`).
    Help,
    /// The tech tree (`fn_gui::screens::research`), and its CHOOSE A SCHOOL box.
    Research,
    /// The abilities list (`fn_gui::screens::abilities`).
    Abilities,
    /// The caravan's camp (`fn_gui::screens::traders`).
    Traders,
    /// The save slots and the name field (`fn_gui::screens::saves`).
    Saves,
    /// The Nation roster (`fn_gui::screens::nation`).
    Nation,
    /// The Endeavor screen (`fn_gui::screens::endeavor`).
    Endeavor,
}

/// What the shell shows now, as far as the window is concerned. A box over it is [`modal`]'s.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Surface {
    /// The map, the side panel and the hint bar. The menu is this under its box.
    Native,
    /// The title screen's steps (`fn_gui::title`), natively.
    Title,
    Page(PageKind),
    /// The embark screen (`fn_gui::embark`): the survey and its report, natively.
    Embark,
    CellsFull,
}

/// Whether what `shell` shows is native, a native page, or cells over the whole window, under
/// whatever box [`modal`] puts over it.
pub fn surface(shell: &Shell) -> Surface {
    match &shell.mode {
        Mode::Title(_) => Surface::Title,
        Mode::Embark(_) => Surface::Embark,
        Mode::Game(app) => game_surface(&app.ui),
    }
}

fn game_surface(ui: &UiState) -> Surface {
    // The same order `fn_tui::draw::draw` takes: a screen covers a modal, a modal the map.
    match (ui.screen, ui.modal) {
        (Some(screen), _) => screen_surface(screen),
        (None, Some(modal)) => modal_surface(modal),
        (None, None) => Surface::Native,
    }
}

/// Every screen is cells until its own native painter lands (rows 5 to 13 of the order).
fn screen_surface(screen: Screen) -> Surface {
    match screen {
        Screen::Research => Surface::Page(PageKind::Research),
        Screen::Abilities => Surface::Page(PageKind::Abilities),
        Screen::Traders => Surface::Page(PageKind::Traders),
        Screen::Saves => Surface::Page(PageKind::Saves),
        Screen::Nation => Surface::Page(PageKind::Nation),
        Screen::Endeavor => Surface::Page(PageKind::Endeavor),
        Screen::Log => Surface::Page(PageKind::Log),
        Screen::Help => Surface::Page(PageKind::Help),
        // The Airship screen (#763) is drawn in cells, as the terminal does, until it has a
        // native painter.
        Screen::Airship => Surface::CellsFull,
    }
}

fn modal_surface(modal: Modal) -> Surface {
    match modal {
        // The Build picker is a full screen (`fn_tui::build`, #652), native since #881.
        Modal::Build => Surface::Page(PageKind::Build),
        // The menu is a box over the map, which stays as it is under it.
        Modal::Menu => Surface::Native,
        // The LOST TEACHING box (#743): the terminal's frame until it has a native painter.
        Modal::Teaching => Surface::CellsFull,
    }
}

/// The box (#879) that is open over whatever [`surface`] says, if any: embark's exit prompt, a
/// confirm box for the open [`Prompt`] (over the page or the screen it was asked from, or over
/// the map, the menu behind it hidden as the TUI hides it), or the game menu.
pub fn modal(shell: &Shell) -> Option<BoxKind> {
    if shell.exit_prompt {
        return Some(BoxKind::Exit);
    }
    let Mode::Game(app) = &shell.mode else { return None };
    if let Some(prompt) = &app.ui.prompt {
        return Some(prompt_box(prompt));
    }
    match app.ui.modal {
        // A screen covers the menu, as it does in the TUI.
        Some(Modal::Menu) if app.ui.screen.is_none() => Some(BoxKind::Menu),
        Some(Modal::Menu | Modal::Build | Modal::Teaching) | None => None,
    }
}

fn prompt_box(prompt: &Prompt) -> BoxKind {
    match prompt {
        Prompt::Quit
        | Prompt::NewMap
        | Prompt::Nation(_)
        | Prompt::QuickLoad
        | Prompt::LoadSave(_)
        | Prompt::SaveOver(_) => BoxKind::Ask,
    }
}

/// The last frame's costs, in milliseconds, for the F12 overlay.
#[derive(Clone, Copy, Debug, Default)]
pub struct Timings {
    /// The shell's advance (the sim).
    pub sim: f32,
    /// Building and painting the cells: the whole frame, the panel's slot or the box.
    pub cells: f32,
    /// The map widget's paint.
    pub map: f32,
    /// The whole frame, from the keys to the last widget.
    pub frame: f32,
}

/// The window's own state between frames: the map widget's camera, the dev overlay.
#[derive(Default)]
pub struct Frame {
    map: Option<MapView>,
    /// The embark screen's view of the survey.
    pub embark: EmbarkView,
    /// The shell's size in cells, as last given to [`Shell::resize`].
    size: (u16, u16),
    /// The map widget's first cell height (`FN_GUI_CELL`), for screenshots of a zoom level.
    pub first_cell: Option<f32>,
    /// F12: the dev timing overlay.
    pub overlay: bool,
    pub timings: Timings,
    /// What was shown last frame.
    pub surface: Option<Surface>,
    /// The box over it last frame.
    pub modal: Option<BoxKind>,
    /// The hint bar's text last frame (for tests and the web page's `data-` attributes).
    pub hint_text: String,
    /// The keys the window's own widgets (the side panel's clicks) stand for, not yet given to
    /// the shell ([`Frame::take_keys`]).
    keys: Vec<KeyEvent>,
}

impl Frame {
    /// The shell's size in cells as of the last frame: the window's.
    pub fn size(&self) -> (u16, u16) {
        self.size
    }

    /// The keys this frame's clicks stand for, oldest first, for the shell to take as typed.
    pub fn take_keys(&mut self) -> Vec<KeyEvent> {
        std::mem::take(&mut self.keys)
    }

    /// Lays out one frame of `shell` into `root`, the window's root [`Ui`].
    pub fn show(&mut self, root: &mut Ui, shell: &mut Shell, palette: &Palette) {
        let metrics = Metrics::new(root.ctx());
        let window = root.available_rect_before_wrap();
        let (cols, rows) = metrics.grid_in(window.size());
        let (cols, rows) = (cols.max(MIN_WIDTH), rows.max(MIN_HEIGHT));
        if self.size != (cols, rows) {
            shell.resize(cols, rows);
            self.size = (cols, rows);
        }
        let surface = surface(shell);
        let modal = modal(shell);
        self.surface = Some(surface);
        self.modal = modal;
        match (&mut shell.mode, surface) {
            (Mode::Game(app), Surface::Native) => {
                self.show_native(root, app, modal.is_none(), palette, &metrics, (cols, rows));
            }
            (Mode::Title(_), Surface::Title) => {
                self.map = None;
                self.show_title(root, shell, palette, &metrics, cols);
            }
            (Mode::Game(app), Surface::Page(kind)) => {
                self.show_page(root, app, kind, palette, &metrics, cols);
            }
            (Mode::Embark(_), Surface::Embark) => {
                self.map = None;
                self.show_embark(root, shell, palette, &metrics, cols);
            }
            _ => {
                self.map = None;
                self.show_cells(root, shell, palette, &metrics, (cols, rows));
            }
        }
        if let Some(kind) = modal {
            let clicks = boxes::show(root.ctx(), shell, kind, palette, &metrics);
            self.keys.extend(clicks);
        }
        if self.overlay {
            self.show_overlay(root, window, surface, &metrics);
        }
    }

    /// The whole terminal frame, as the terminal would show it.
    fn show_cells(
        &mut self,
        root: &mut Ui,
        shell: &mut Shell,
        palette: &Palette,
        metrics: &Metrics,
        (cols, rows): (u16, u16),
    ) {
        self.hint_text.clear();
        egui::CentralPanel::default().frame(egui::Frame::NONE.fill(Color32::BLACK)).show(
            root,
            |ui| {
                let avail = ui.available_rect_before_wrap();
                let started = Instant::now();
                let mut buf = Buffer::empty(ratatui::layout::Rect::new(0, 0, cols, rows));
                shell::draw_under_boxes(shell, palette, &mut buf);
                let layout = CellLayout::new(&buf);
                cells::paint(&ui.painter_at(avail), avail.min, metrics, &layout);
                cells::expose(ui, avail.min, metrics, &layout, buf.area);
                self.timings.cells = started.elapsed().as_secs_f32() * 1000.0;
            },
        );
    }

    /// Shows `bar`, the one row along the window's bottom, under whatever else is shown.
    fn show_bar(&mut self, root: &mut Ui, bar: &hints::Bar, metrics: &Metrics) {
        self.hint_text = bar.text();
        egui::Panel::bottom("hints")
            .exact_size(metrics.cell.y)
            .resizable(false)
            .show_separator_line(false)
            .frame(egui::Frame::NONE.fill(bar.fill))
            .show(root, |ui| hints::show(ui, bar, &metrics.font));
    }

    /// The native title screen (#880): its step, and the hint bar with the step's keys. The
    /// clicks come back as keys, queued for the shell like the side panel's ([`Frame::take_keys`]).
    fn show_title(
        &mut self,
        root: &mut Ui,
        shell: &mut Shell,
        palette: &Palette,
        metrics: &Metrics,
        cols: u16,
    ) {
        let Mode::Title(screen) = &shell.mode else { return };
        let bar = hints::title_bar(title::hint_mode(&screen.page), palette, cols);
        self.show_bar(root, &bar, metrics);
        let mut keys = Vec::new();
        egui::CentralPanel::default().frame(egui::Frame::NONE).show(root, |ui| {
            keys = crate::title::show(ui, screen, shell.defs(), palette);
        });
        self.keys.extend(keys);
    }

    /// A native full screen: the page frame over the whole window above the hint bar. No side
    /// panel and no map; the map's camera is kept for when the screen closes.
    fn show_page(
        &mut self,
        root: &mut Ui,
        app: &App,
        kind: PageKind,
        palette: &Palette,
        metrics: &Metrics,
        cols: u16,
    ) {
        self.show_bar(root, &hints::bar(app, palette, cols), metrics);
        let started = Instant::now();
        egui::CentralPanel::default().frame(egui::Frame::NONE).show(root, |ui| {
            let clicks = match kind {
                PageKind::Build => screens::build::show(ui, app, palette, metrics),
                PageKind::Research => screens::research::show(ui, app, palette, metrics),
                PageKind::Abilities => screens::abilities::show(ui, app, palette, metrics),
                PageKind::Traders => screens::traders::show(ui, app, palette, metrics),
                PageKind::Saves => screens::saves::show(ui, app, palette, metrics),
                PageKind::Nation => screens::nation::show(ui, app, palette, metrics),
                PageKind::Endeavor => screens::endeavor::show(ui, app, palette, metrics),
                PageKind::Log => screens::log::show(ui, app, palette, metrics),
                PageKind::Help => screens::help::show(ui, app, palette, metrics),
            };
            self.keys.extend(clicks);
        });
        self.timings.cells = started.elapsed().as_secs_f32() * 1000.0;
    }

    /// The native embark screen (#883): the hint bar, the site report as the side panel and the
    /// survey in the middle, with the settle question or the key help over it. The clicks come
    /// back as keys, queued for the shell like the title's ([`Frame::take_keys`]).
    fn show_embark(
        &mut self,
        root: &mut Ui,
        shell: &mut Shell,
        palette: &Palette,
        metrics: &Metrics,
        cols: u16,
    ) {
        let Mode::Embark(screen) = &shell.mode else { return };
        let defs = shell.defs();
        let bar =
            hints::embark_bar(fn_tui::embark::hint_mode(screen), &screen.status, palette, cols);
        self.show_bar(root, &bar, metrics);

        // The question and the help take every key: the map and the panel show but press none.
        let live = !screen.asking() && !screen.helping() && !shell.exit_prompt;
        let panel_fill = to_egui(palette.token(palette.theme.panel), Color32::BLACK);
        let mut keys = Vec::new();
        egui::Panel::right("side")
            .default_size(PANEL_DEFAULT)
            .size_range(PANEL_RANGE.0..=PANEL_RANGE.1)
            .resizable(true)
            .frame(egui::Frame::NONE.fill(panel_fill))
            .show(root, |ui| {
                keys.extend(embark::side(ui, screen, defs, palette, metrics, live));
            });
        let view = &mut self.embark;
        let mut map_ms = 0.0;
        egui::CentralPanel::default().frame(egui::Frame::NONE).show(root, |ui| {
            let rect = ui.available_rect_before_wrap();
            let started = Instant::now();
            keys.extend(embark::map(ui, view, rect, screen, defs, palette, live));
            map_ms = started.elapsed().as_secs_f32() * 1000.0;
        });
        self.timings.map = map_ms;
        keys.extend(embark::overlay(root.ctx(), screen, palette, metrics));
        self.keys.extend(keys);
    }

    /// The native game: hint bar, side panel, map and banner. `live` is whether the window takes
    /// the side panel's clicks: not with a box over it.
    fn show_native(
        &mut self,
        root: &mut Ui,
        app: &mut App,
        live: bool,
        palette: &Palette,
        metrics: &Metrics,
        (cols, _rows): (u16, u16),
    ) {
        let fill = |rgb| to_egui(palette.token(rgb), Color32::BLACK);
        let panel_fill = fill(palette.theme.panel);
        let ch = metrics.cell.y;

        // The bar first: the panel beside the map then stops above it, as the TUI's status line
        // runs under both.
        self.show_bar(root, &hints::bar(app, palette, cols), metrics);

        // The native side panel (#877). Its clicks come back as keys, which the window feeds
        // the shell; with a box open it shows but takes none.
        let mut clicks = Vec::new();
        egui::Panel::right("side")
            .default_size(PANEL_DEFAULT)
            .size_range(PANEL_RANGE.0..=PANEL_RANGE.1)
            .resizable(true)
            .frame(egui::Frame::NONE.fill(panel_fill))
            .show(root, |ui| {
                clicks = side::show(ui, app, palette, metrics, live);
            });
        self.keys.extend(clicks);

        egui::CentralPanel::default().frame(egui::Frame::NONE).show(root, |ui| {
            let avail = ui.available_rect_before_wrap();
            // The banner's strip is reserved whenever a tool is active, so the map does not
            // jump when a box opens over it; the menu hides the banner itself.
            let top = if app.ui.banner() { BANNER_HEIGHT as f32 * ch } else { 0.0 };
            let map_rect = Rect::from_min_max(avail.min + vec2(0.0, top), avail.max);
            if top > 0.0 {
                let strip = Rect::from_min_max(avail.min, avail.min + vec2(avail.width(), top));
                if app.ui.modal.is_none() {
                    show_banner(ui, strip, app, palette, &metrics.font);
                } else {
                    ui.painter().rect_filled(strip, 0.0, Color32::from_rgb(8, 8, 12));
                }
            }
            let first_cell = self.first_cell.take();
            let view = self.map.get_or_insert_with(|| MapView::new(app));
            if let Some(cell) = first_cell {
                view.camera.cell = cell;
            }
            view.show(ui, map_rect, app, palette);
            self.timings.map = view.last_paint_ms;
        });
    }

    fn show_overlay(&self, root: &Ui, window: Rect, surface: Surface, metrics: &Metrics) {
        let t = self.timings;
        let cell = self.map.as_ref().map_or(0.0, |m| m.camera.cell);
        let note = format!(
            "{surface:?}  cell {cell:.0}px  map {:.1} ms  cells {:.1} ms  sim {:.1} ms  \
             frame {:.1} ms  ({}x{} cells)",
            t.map, t.cells, t.sim, t.frame, self.size.0, self.size.1
        );
        let painter = root.ctx().layer_painter(LayerId::new(Order::Tooltip, Id::unique("timing")));
        let font = FontId::new(12.0, FontFamily::Monospace);
        let at = window.left_bottom() + vec2(6.0, -metrics.cell.y - 4.0);
        let galley = painter.layout_no_wrap(note.clone(), font.clone(), Color32::from_gray(255));
        painter.rect_filled(
            Rect::from_min_size(at - vec2(0.0, galley.size().y), galley.size()).expand(3.0),
            2.0,
            Color32::from_black_alpha(190),
        );
        painter.text(at, Align2::LEFT_BOTTOM, note, font, Color32::from_gray(255));
    }
}

/// The active tool's banner across the top of the map: `━ HARVEST ━ Enter: set a corner · Esc:
/// leave`, in the same tint as the hint bar's badge.
fn show_banner(ui: &mut Ui, strip: Rect, app: &App, palette: &Palette, font: &FontId) {
    let Some(tint) = banner::tint(&app.ui) else { return };
    let style = palette.banner_style(tint);
    let fg = style.fg.map_or(Color32::WHITE, |c| to_egui(c, Color32::WHITE));
    let bg = style.bg.map_or(Color32::BLACK, |c| to_egui(c, Color32::BLACK));
    ui.painter().rect_filled(strip, 0.0, bg);
    let text = format!(
        "━ {} ━ {}",
        banner::badge(&app.session, &app.ui),
        banner::segments(&app.session, &app.ui).join("  ·  ")
    );
    ui.scope_builder(UiBuilder::new().max_rect(strip.shrink2(vec2(font.size * 0.5, 0.0))), |ui| {
        ui.add(
            egui::Label::new(RichText::new(text).font(font.clone()).color(fg).strong())
                .selectable(false)
                .truncate(),
        );
    });
}
