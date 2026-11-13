//! A run log replayed in the window (#994, #997): `fn_tui::replay::Replay` (the terminal
//! watcher plus snapshots for seeking) drawn the way the game is, not as a terminal frame.
//!
//! The layout is the watcher's own ([`Watcher::window_layout`](fn_tui::watch::Watcher)): at or
//! above the studio's size, the code pane on the left and the game column on the right with the
//! feed under it; below it (or with a pane zoomed by `1`/`2`/`3`), one pane over the whole window.
//! The agent strip takes the bottom row of the game column or of the pane, as in the terminal.
//!
//! - **The game** is [`Frame`] in watch-only mode (#996): the native map, with zoom, pan and
//!   tooltips, the native side panel and the native pages, none of them taking a click. The
//!   agent's attention ghost is outlined on the map, and, since the window's camera is its own,
//!   zooming or panning never moves the agent's cursor; that camera is the free look.
//! - **The code pane, the feed and the strip** stay cells (decision #871), painted by
//!   [`crate::cells`].
//!
//! Keys are the watcher's controls only (`Space`, `+`/`-`, `.`, `]`/`}`, `[`/`{`, `>`, `0`-`3`,
//! and `j`/`k`/`J`/`K` for the code pane); the terminal's free-look keys have the map instead.
//! Nothing here reads the clock but the frame time.

use web_time::Instant;

use eframe::egui::{self, Color32, Rect, Ui, UiBuilder, pos2, vec2};
use fn_tui::color::{ColorDepth, Palette};
use fn_tui::keyev::{KeyCode, KeyEvent};
use fn_tui::replay::Replay;
use fn_tui::watch::{Pane, WindowLayout};
use ratatui::buffer::Buffer;

use crate::cells::{self, CellLayout, Metrics};
use crate::frame::Frame;
use crate::mapview::Outline;

/// How long a frame may spend on a seek before drawing, in seconds (the canvas page's budget).
pub const SEEK_BUDGET: f64 = 0.012;

/// The attention ghost's colour on the map, at full strength.
const ATTENTION: Color32 = Color32::from_rgb(255, 196, 64);

pub struct ReplayApp {
    replay: Replay,
    palette: Palette,
    frame: Frame,
    last_frame: Instant,
    /// Whether the Seal style has been installed for the replay's nation.
    styled: bool,
    /// The layout of the last frame, for tests.
    layout: Option<WindowLayout>,
}

/// Whether `key` is one of the watcher's controls ([`ReplayApp`]'s module note).
pub fn is_watch_key(key: &KeyEvent) -> bool {
    matches!(
        key.code,
        KeyCode::Char(
            ' ' | '+'
                | '='
                | '-'
                | '_'
                | '.'
                | ']'
                | '}'
                | '['
                | '{'
                | '>'
                | '0'
                | '1'
                | '2'
                | '3'
                | 'j'
                | 'k'
                | 'J'
                | 'K'
        )
    )
}

impl ReplayApp {
    /// A replay of the log `text`, at its first frame; refuses what [`Replay::new`] refuses.
    pub fn open(text: &str) -> Result<Self, String> {
        let replay = Replay::new(text)?;
        let watcher = replay.watcher();
        let palette = Palette::new(watcher.defs(), watcher.nation(), ColorDepth::TrueColor, false);
        Ok(Self {
            replay,
            palette,
            frame: Frame::watching(),
            last_frame: Instant::now(),
            styled: false,
            layout: None,
        })
    }

    pub fn replay(&self) -> &Replay {
        &self.replay
    }

    pub fn replay_mut(&mut self) -> &mut Replay {
        &mut self.replay
    }

    /// The game's frame state: what it showed last.
    pub fn frame(&self) -> &Frame {
        &self.frame
    }

    /// What the last frame showed: the studio's split or one pane.
    pub fn layout(&self) -> Option<WindowLayout> {
        self.layout
    }

    /// One key: a watcher control goes to the replay; anything else does nothing.
    pub fn key(&mut self, key: KeyEvent) {
        if is_watch_key(&key) {
            self.replay.key(key);
        }
    }

    /// The keys and the clock: a seek takes up to [`SEEK_BUDGET`] of each frame, and otherwise
    /// the replay plays the real seconds since the last frame at its pace.
    fn step(&mut self, ctx: &egui::Context) {
        let now = Instant::now();
        let dt = now.duration_since(self.last_frame).as_secs_f64().min(0.25);
        self.last_frame = now;
        for key in ctx.input(crate::keys::frame_keys) {
            self.key(key);
        }
        if self.replay.seeking() {
            let started = Instant::now();
            while self.replay.seek_step() && started.elapsed().as_secs_f64() < SEEK_BUDGET {}
        } else {
            self.replay.advance(dt);
        }
    }

    /// One frame of the window into `root`.
    pub fn show(&mut self, root: &mut Ui) {
        let ctx = root.ctx().clone();
        self.step(&ctx);
        if !self.styled {
            crate::style::install(&ctx, &self.palette.theme);
            self.styled = true;
        }
        root.set_style(ctx.global_style());
        let metrics = Metrics::new(&ctx);
        let window = root.available_rect_before_wrap();
        root.painter().rect_filled(window, 0.0, cells::DEFAULT_BG);
        let (cols, rows) = metrics.grid_in(window.size());
        let (cols, rows) = (cols.max(1), rows.max(2));
        let layout = self.replay.watcher_mut().window_layout(&self.palette, (cols, rows));
        self.layout = Some(layout);
        let cell = metrics.cell;
        let row_y = |row: u16| window.min.y + f32::from(row) * cell.y;
        match layout {
            WindowLayout::Studio { code_cols, game_cols, feed_rows } => {
                let code = self.replay.watcher_mut().render_code(&self.palette, (code_cols, rows));
                paint_cells(root, "code", window.min, &metrics, &code);
                let left = window.min.x + f32::from(code_cols) * cell.x;
                let game_rows = rows.saturating_sub(feed_rows).max(1);
                let strip_row = game_rows - 1;
                let game = Rect::from_min_max(
                    pos2(left, window.min.y),
                    pos2(window.max.x, row_y(strip_row)),
                );
                self.show_game(root, game);
                let strip = self.replay.watcher().render_strip(&self.palette, game_cols, layout);
                paint_cells(root, "strip", pos2(left, row_y(strip_row)), &metrics, &strip);
                if feed_rows > 0 {
                    let feed =
                        self.replay.watcher().render_feed(&self.palette, (game_cols, feed_rows));
                    paint_cells(root, "feed", pos2(left, row_y(game_rows)), &metrics, &feed);
                }
            }
            WindowLayout::Pane(pane) => {
                let strip_row = rows - 1;
                let above = (cols, strip_row);
                match pane {
                    Pane::Game => {
                        let game =
                            Rect::from_min_max(window.min, pos2(window.max.x, row_y(strip_row)));
                        self.show_game(root, game);
                    }
                    Pane::Code => {
                        let code = self.replay.watcher_mut().render_code(&self.palette, above);
                        paint_cells(root, "code", window.min, &metrics, &code);
                    }
                    Pane::Feed => {
                        let feed = self.replay.watcher().render_feed(&self.palette, above);
                        paint_cells(root, "feed", window.min, &metrics, &feed);
                    }
                }
                let strip = self.replay.watcher().render_strip(&self.palette, cols, layout);
                paint_cells(root, "strip", pos2(window.min.x, row_y(strip_row)), &metrics, &strip);
            }
        }
        ctx.request_repaint();
    }

    /// The replayed game in `rect`: [`Frame`], watch only, with the agent's attention outlined.
    fn show_game(&mut self, root: &mut Ui, rect: Rect) {
        let watcher = self.replay.watcher();
        self.frame.outlines.clear();
        if let Some(a) = watcher.attention() {
            let alpha = (a.left * 255.0).round().clamp(64.0, 255.0) as u8;
            let color =
                Color32::from_rgba_unmultiplied(ATTENTION.r(), ATTENTION.g(), ATTENTION.b(), alpha);
            self.frame.outlines.push(Outline { at: a.at, w: a.w, h: a.h, color });
        }
        let mut ui = root.new_child(UiBuilder::new().max_rect(rect));
        ui.set_clip_rect(rect.intersect(root.clip_rect()));
        let palette = &self.palette;
        self.frame.show(&mut ui, self.replay.watcher_mut().shell_mut(), palette);
    }
}

/// Paints `buf` with its top-left cell at `origin`, and tells the accessibility tree its rows
/// (under `name`, so two buffers' rows never share an id).
fn paint_cells(ui: &mut Ui, name: &str, origin: egui::Pos2, metrics: &Metrics, buf: &Buffer) {
    let layout = CellLayout::new(buf);
    let size = vec2(
        f32::from(buf.area.width) * metrics.cell.x,
        f32::from(buf.area.height) * metrics.cell.y,
    );
    let painter = ui.painter_at(Rect::from_min_size(origin, size));
    cells::paint(&painter, origin, metrics, &layout);
    ui.push_id(name, |ui| cells::expose(ui, origin, metrics, &layout, buf.area));
}

impl eframe::App for ReplayApp {
    fn ui(&mut self, root: &mut Ui, _frame: &mut eframe::Frame) {
        self.show(root);
    }
}
