//! The window: the shell's whole frame painted through `egui_ratatui`, the map widget over its
//! map region while a game is on screen, and the egui side panel beside it.
//!
//! Like `fn_tui::run`, this is the only place that reads the clock: each frame hands the shell
//! the keys egui collected and the real seconds since the last frame.

use std::path::PathBuf;
use web_time::Instant;

use eframe::egui::{self, Color32, FontFamily, FontId, Rect, vec2};
use egui_ratatui::RataguiBackend;
use fn_sim::Nation;
use fn_tui::color::{ColorDepth, Palette};
use fn_tui::layout;
use fn_tui::shell::{self, Mode, Shell};
use fn_ui::state::BANNER_HEIGHT;
use ratatui::backend::Backend;
use ratatui::buffer::Buffer;
use soft_ratatui::embedded_graphics_unicodefonts::{
    mono_8x13_atlas, mono_8x13_bold_atlas, mono_8x13_italic_atlas,
};
use soft_ratatui::{EmbeddedGraphics, SoftBackend};

use crate::mapview::MapView;

pub struct GuiApp {
    shell: Shell,
    palette: Palette,
    nation: Nation,
    term: RataguiBackend<EmbeddedGraphics>,
    map: Option<MapView>,
    last_frame: Instant,
    /// The egui side panel beside the terminal one (the converted-section sample).
    show_panel: bool,
    /// The last frame's timings, in milliseconds: the shell's advance, the cell render and the
    /// whole frame.
    timings: (f32, f32, f32),
    first_cell: Option<f32>,
}

impl GuiApp {
    /// The shell `fn_tui`'s own arguments describe (the title screen with none, a game with
    /// `SEED NATION`, `--start`, `--map-size` and the rest, as `fn_tui --help` lists them).
    pub fn new(saves: Option<PathBuf>, seed_hint: u64) -> Self {
        // `--keys SCRIPT` is taken out first: `fn_tui`'s parser allows it only with `--frame`.
        let mut raw: Vec<String> = std::env::args().skip(1).collect();
        let mut keys = Vec::new();
        if let Some(i) = raw.iter().position(|a| a == "--keys") {
            let script = raw.get(i + 1).cloned().unwrap_or_default();
            raw.drain(i..(i + 2).min(raw.len()));
            keys = fn_ui::script::parse(&script).unwrap_or_else(|e| {
                eprintln!("fn_gui: --keys: {e}");
                std::process::exit(2)
            });
        }
        let args = fn_tui::cli::parse(raw).unwrap_or_else(|e| {
            eprintln!("fn_gui: {e}");
            std::process::exit(2)
        });
        let mut shell = Shell::for_args(&args, (120, 40), saves, seed_hint).unwrap_or_else(|e| {
            eprintln!("fn_gui: {e}");
            std::process::exit(2)
        });
        // `--keys` plays before the window opens, as `--frame --keys` does: handy for
        // screenshots of a particular state.
        shell.play(&keys);
        let mut app = Self::with_shell(shell);
        // `FN_GUI_CELL=N`: the map widget's first cell height, for screenshots of a zoom level.
        app.first_cell = std::env::var("FN_GUI_CELL").ok().and_then(|v| v.parse().ok());
        app
    }

    pub fn with_shell(shell: Shell) -> Self {
        let nation = shell.nation();
        let palette = Palette::new(shell.defs(), nation, ColorDepth::TrueColor, false);
        let soft = SoftBackend::<EmbeddedGraphics>::new(
            120,
            40,
            mono_8x13_atlas(),
            Some(mono_8x13_bold_atlas()),
            Some(mono_8x13_italic_atlas()),
        );
        Self {
            shell,
            palette,
            nation,
            term: RataguiBackend::new("fn_gui_cells", soft),
            map: None,
            last_frame: Instant::now(),
            show_panel: true,
            timings: (0.0, 0.0, 0.0),
            first_cell: None,
        }
    }

    pub fn shell(&self) -> &Shell {
        &self.shell
    }

    fn step(&mut self, ctx: &egui::Context) {
        let now = Instant::now();
        let dt = now.duration_since(self.last_frame).as_secs_f64().min(0.25);
        self.last_frame = now;
        for key in ctx.input(crate::keys::frame_keys) {
            // F12 is the window's own: it shows or hides the egui panel.
            if key.code == fn_ui::keyev::KeyCode::F(12) {
                self.show_panel = !self.show_panel;
                continue;
            }
            self.shell.key(key);
        }
        if self.shell.quit {
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }
        let started = Instant::now();
        if self.shell.paused() {
            self.shell.age(dt);
        } else {
            self.shell.advance(dt);
        }
        self.timings.0 = started.elapsed().as_secs_f32() * 1000.0;
        if self.shell.nation() != self.nation {
            self.nation = self.shell.nation();
            self.palette =
                Palette::new(self.shell.defs(), self.nation, ColorDepth::TrueColor, false);
        }
    }
}

impl eframe::App for GuiApp {
    fn ui(&mut self, root: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = &root.ctx().clone();
        let frame_start = Instant::now();
        self.step(ctx);

        let in_game = matches!(self.shell.mode, Mode::Game(_));
        if in_game && self.show_panel {
            egui::Panel::right("people").default_size(260.0).show_inside(root, |ui| {
                if let Mode::Game(app) = &mut self.shell.mode {
                    crate::panel::show(ui, app);
                }
            });
        }

        egui::CentralPanel::default().frame(egui::Frame::NONE.fill(Color32::BLACK)).show_inside(
            root,
            |ui| {
                let avail = ui.available_rect_before_wrap();
                let (cw, ch) = (
                    self.term.soft_backend.char_width as f32,
                    self.term.soft_backend.char_height as f32,
                );
                let cols = ((avail.width() / cw) as u16).max(layout::MIN_WIDTH);
                let rows = ((avail.height() / ch) as u16).max(layout::MIN_HEIGHT);
                if self.shell_size() != (cols, rows) {
                    self.shell.resize(cols, rows);
                    self.term.soft_backend.resize(cols, rows);
                }

                // The whole frame, as the terminal would show it.
                let started = Instant::now();
                let mut buf = Buffer::empty(ratatui::layout::Rect::new(0, 0, cols, rows));
                shell::draw(&mut self.shell, &self.palette, &mut buf);
                let cells = buf
                    .content
                    .iter()
                    .enumerate()
                    .map(|(i, c)| ((i % cols as usize) as u16, (i / cols as usize) as u16, c));
                let _ = self.term.draw(cells);
                self.timings.1 = started.elapsed().as_secs_f32() * 1000.0;
                let image = ui.add(&mut self.term);
                let origin = image.rect.min;

                // The map widget over the map region, while the map is what's showing.
                let Mode::Game(app) = &mut self.shell.mode else {
                    self.map = None;
                    return;
                };
                if !fn_ui::pointer::map_has_pointer(app) {
                    return;
                }
                let Some(regions) = layout::regions(ratatui::layout::Rect::new(0, 0, cols, rows))
                else {
                    return;
                };
                let m = regions.map;
                let top = if app.ui.banner() { BANNER_HEIGHT as u16 } else { 0 };
                let rect = Rect::from_min_size(
                    origin + vec2(f32::from(m.x) * cw, f32::from(m.y + top) * ch),
                    vec2(f32::from(m.width) * cw, f32::from(m.height - top) * ch),
                );
                let first_cell = self.first_cell.take();
                let view = self.map.get_or_insert_with(|| MapView::new(app));
                if let Some(cell) = first_cell {
                    view.camera.cell = cell;
                }
                view.show(ui, rect, app, &self.palette);
                let note = format!(
                    "cell {:.0}px  map {:.1} ms  cells {:.1} ms  sim {:.1} ms  frame {:.1} ms",
                    view.camera.cell,
                    view.last_paint_ms,
                    self.timings.1,
                    self.timings.0,
                    self.timings.2
                );
                ui.painter().text(
                    rect.left_bottom() + vec2(6.0, -4.0),
                    egui::Align2::LEFT_BOTTOM,
                    note,
                    FontId::new(12.0, FontFamily::Monospace),
                    Color32::from_rgba_unmultiplied(255, 255, 255, 180),
                );
            },
        );
        self.timings.2 = frame_start.elapsed().as_secs_f32() * 1000.0;
        ctx.request_repaint();
    }
}

impl GuiApp {
    fn shell_size(&self) -> (u16, u16) {
        let s = self.term.soft_backend.size().unwrap_or_default();
        (s.width, s.height)
    }
}
