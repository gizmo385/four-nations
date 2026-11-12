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
//!   is native ([`crate::side`]), but for the inspector's rows, which are still cells.
//! - [`Surface::CellsFull`]: the whole terminal frame, at the window's size in cells.
//! - [`Surface::CellsBox`]: the native view, with only one box's rectangle painted from the
//!   terminal's own drawing over the map.
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
use fn_tui::{ask, banner, draw, menu};
use fn_ui::app::App;
use fn_ui::keyev::KeyEvent;
use fn_ui::state::{BANNER_HEIGHT, Modal, Prompt, Screen, UiState};
use ratatui::buffer::Buffer;

use crate::cells::{self, CellLayout, Metrics};
use crate::color::to_egui;
use crate::hints;
use crate::mapview::MapView;
use crate::side;

/// The side panel's width on a fresh window, and the range the user can drag it through.
pub const PANEL_DEFAULT: f32 = 300.0;
pub const PANEL_RANGE: (f32, f32) = (260.0, 440.0);

/// Which box a [`Surface::CellsBox`] paints.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BoxKind {
    /// The game menu (`fn_tui::menu`).
    Menu,
    /// A confirm box (`fn_tui::ask`).
    Ask,
}

/// What the shell shows now, as far as the window is concerned.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Surface {
    Native,
    CellsFull,
    CellsBox(BoxKind),
}

/// Whether what `shell` shows is native, cells over the whole window or cells in a box.
pub fn surface(shell: &Shell) -> Surface {
    // The exit prompt (embark's `Q`, the title's) is drawn over the whole frame.
    if shell.exit_prompt {
        return Surface::CellsFull;
    }
    match &shell.mode {
        Mode::Title(_) | Mode::Embark(_) => Surface::CellsFull,
        Mode::Game(app) => game_surface(&app.ui),
    }
}

fn game_surface(ui: &UiState) -> Surface {
    // The same order `fn_tui::draw::draw` takes: a screen covers a modal, a modal the map.
    let base = match (ui.screen, ui.modal) {
        (Some(screen), _) => screen_surface(screen),
        (None, Some(modal)) => modal_surface(modal),
        (None, None) => Surface::Native,
    };
    match (&ui.prompt, base) {
        // Asked from a full screen (the saves screen's load and overwrite): in the same
        // terminal frame.
        (Some(_), Surface::CellsFull) => Surface::CellsFull,
        // Asked over the map: the question alone, the menu behind it hidden, as the TUI does.
        (Some(prompt), _) => prompt_surface(prompt),
        (None, base) => base,
    }
}

/// Every screen is cells until its own native painter lands (rows 5 to 13 of the order).
fn screen_surface(screen: Screen) -> Surface {
    match screen {
        Screen::Log
        | Screen::Help
        | Screen::Endeavor
        | Screen::Research
        | Screen::Saves
        | Screen::Nation
        | Screen::Abilities
        | Screen::Traders => Surface::CellsFull,
    }
}

fn modal_surface(modal: Modal) -> Surface {
    match modal {
        // The Build picker is a full screen (`fn_tui::build`, #652).
        Modal::Build => Surface::CellsFull,
        Modal::Menu => Surface::CellsBox(BoxKind::Menu),
    }
}

fn prompt_surface(prompt: &Prompt) -> Surface {
    match prompt {
        Prompt::Quit
        | Prompt::NewMap
        | Prompt::Nation(_)
        | Prompt::QuickLoad
        | Prompt::LoadSave(_)
        | Prompt::SaveOver(_) => Surface::CellsBox(BoxKind::Ask),
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
    /// The shell's size in cells, as last given to [`Shell::resize`].
    size: (u16, u16),
    /// The map widget's first cell height (`FN_GUI_CELL`), for screenshots of a zoom level.
    pub first_cell: Option<f32>,
    /// F12: the dev timing overlay.
    pub overlay: bool,
    pub timings: Timings,
    /// What was shown last frame.
    pub surface: Option<Surface>,
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
        self.surface = Some(surface);
        match (&mut shell.mode, surface) {
            (Mode::Game(app), Surface::Native | Surface::CellsBox(_)) => {
                self.show_native(root, app, surface, palette, &metrics, (cols, rows));
            }
            _ => {
                self.map = None;
                self.show_cells(root, shell, palette, &metrics, (cols, rows));
            }
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
                shell::draw(shell, palette, &mut buf);
                let layout = CellLayout::new(&buf);
                cells::paint(&ui.painter_at(avail), avail.min, metrics, &layout);
                cells::expose(ui, avail.min, metrics, &layout, buf.area);
                self.timings.cells = started.elapsed().as_secs_f32() * 1000.0;
            },
        );
    }

    /// The native game: hint bar, side panel, map, banner, and a box over the map if one is
    /// open.
    fn show_native(
        &mut self,
        root: &mut Ui,
        app: &mut App,
        surface: Surface,
        palette: &Palette,
        metrics: &Metrics,
        (cols, _rows): (u16, u16),
    ) {
        let fill = |rgb| to_egui(palette.token(rgb), Color32::BLACK);
        let panel_fill = fill(palette.theme.panel);
        let ch = metrics.cell.y;

        // The bar first: the panel beside the map then stops above it, as the TUI's status line
        // runs under both.
        let bar = hints::bar(app, palette, cols);
        self.hint_text = bar.text();
        egui::Panel::bottom("hints")
            .exact_size(ch)
            .resizable(false)
            .show_separator_line(false)
            .frame(egui::Frame::NONE.fill(bar.fill))
            .show(root, |ui| hints::show(ui, &bar, &metrics.font));

        // The native side panel (#877). Its clicks come back as keys, which the window feeds
        // the shell; with a box open over the map it shows but takes none.
        let live = surface == Surface::Native;
        let (mut clicks, mut inspector_ms) = (Vec::new(), 0.0);
        egui::Panel::right("side")
            .default_size(PANEL_DEFAULT)
            .size_range(PANEL_RANGE.0..=PANEL_RANGE.1)
            .resizable(true)
            .frame(egui::Frame::NONE.fill(panel_fill))
            .show(root, |ui| {
                clicks = side::show(ui, app, palette, metrics, live, &mut inspector_ms);
            });
        self.keys.extend(clicks);
        self.timings.cells = inspector_ms;

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
            if let Surface::CellsBox(kind) = surface {
                let started = Instant::now();
                paint_box(ui, kind, avail, app, palette, metrics);
                self.timings.cells = started.elapsed().as_secs_f32() * 1000.0;
            }
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

/// Draws `kind`'s box as the terminal would, centred in `over` (the map's rectangle), and paints
/// only the box's own rectangle.
fn paint_box(
    ui: &mut Ui,
    kind: BoxKind,
    over: Rect,
    app: &App,
    palette: &Palette,
    metrics: &Metrics,
) {
    let (cols, rows) = metrics.grid_in(over.size());
    let outer = ratatui::layout::Rect::new(0, 0, cols, rows);
    let mut buf = Buffer::empty(outer);
    let (session, state) = (&app.session, &app.ui);
    let area = match kind {
        BoxKind::Menu => {
            draw::draw_modal(session, state, Modal::Menu, palette, outer, &mut buf);
            menu::area(outer)
        }
        BoxKind::Ask => {
            let Some(question) = ask::current(session, state) else { return };
            ask::draw(&question, state.ask_cursor, palette, outer, &mut buf);
            ask::area(&question, outer)
        }
    };
    // Only the box: copy its cells out, so the map's own look shows round it.
    let mut only = Buffer::empty(ratatui::layout::Rect::new(0, 0, area.width, area.height));
    for y in 0..area.height {
        for x in 0..area.width {
            only[(x, y)] = buf[(area.x + x, area.y + y)].clone();
        }
    }
    let origin =
        over.min + vec2(f32::from(area.x) * metrics.cell.x, f32::from(area.y) * metrics.cell.y);
    let layout = CellLayout::new(&only);
    cells::paint(&ui.painter_at(over), origin, metrics, &layout);
    cells::expose(ui, origin, metrics, &layout, only.area);
}
