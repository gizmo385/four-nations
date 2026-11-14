//! A run log replayed in the window (#994, #997): `fn_tui::replay::Replay` (the terminal
//! watcher plus snapshots for seeking) drawn the way the game is, not as a terminal frame.
//!
//! The layout is the watcher's own ([`Watcher::window_layout`](fn_tui::watch::Watcher)): at or
//! above the studio's size, the code pane on the left and the game column on the right with the
//! feed under it; below it (or with a pane zoomed by `1`/`2`/`3`), one pane over the whole window.
//! The agent strip takes the bottom row of the game column or of the pane, as in the terminal.
//!
//! - **The game** is [`Frame`] in watch-only mode (#996): the native map, with zoom, pan and
//!   tooltips, the native side panel and the native pages. The agent's attention ghost is
//!   outlined on the map, and, since the window's camera is its own, zooming or panning never
//!   moves the agent's cursor.
//! - **Free look** (#330) is the terminal watcher's: the game's keys, and the side panel's,
//!   pages' and notices' clicks, go to the spectator's own view of the game
//!   ([`Watcher::free_look_key`](fn_tui::watch::Watcher::free_look_key)), never to the game, and
//!   the window draws that view until `V` (or "Follow agent") goes back to the agent's. A
//!   notice's close button only hides it from this window, since the agent's notices are part of
//!   what is replayed.
//! - **The code pane, the feed and the strip** stay cells (decision #871), painted by
//!   [`crate::cells`].
//!
//! Keys go as the terminal watcher's do (`fn_tui::watch::control_key`): its controls first
//! (`Space`, `+`/`-`, `.`, `]`/`}`, `[`/`{`, `>`, `0`-`3`, and `j`/`k`/`J`/`K` for the code
//! pane), then free look. Nothing here reads the clock but the frame time.

use web_time::Instant;

use eframe::egui::{self, Color32, Rect, Ui, UiBuilder, pos2, vec2};
use fn_tui::color::{ColorDepth, Palette};
use fn_tui::keyev::{KeyCode, KeyEvent, KeyModifiers};
use fn_tui::replay::Replay;
use fn_tui::watch::{Pane, WindowLayout};
use ratatui::buffer::Buffer;

use crate::cells::{self, CellLayout, Metrics};
use crate::frame::Frame;
use crate::mapview::Outline;
use crate::timeline::{Action, Timeline};

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
    /// The controls and the timeline's markers (#998).
    timeline: Timeline,
    /// Why the last log dropped on the window could not be opened.
    status: Option<String>,
}

/// The height of the controls along the bottom (a row of buttons over the timeline), in points.
pub const CONTROLS_HEIGHT: f32 = 34.0 + crate::timeline::BAR_HEIGHT + 8.0;

/// A log's text from its bytes: a `run.jsonl` as it is, or gzipped (`.jsonl.gz`).
pub fn log_text(bytes: &[u8]) -> Result<String, String> {
    let bytes = if bytes.starts_with(&[0x1f, 0x8b]) { gunzip(bytes)? } else { bytes.to_vec() };
    String::from_utf8(bytes).map_err(|e| format!("the log isn't UTF-8: {e}"))
}

/// The most a gzipped log may inflate to: a guard against a damaged file, well above a real run.
const INFLATE_LIMIT: usize = 512 << 20;

/// A gzip member's contents (RFC 1952): the header, with its optional fields, then raw deflate.
fn gunzip(bytes: &[u8]) -> Result<Vec<u8>, String> {
    let bad = || "a damaged .gz file".to_owned();
    if bytes.len() < 18 || bytes[2] != 8 {
        return Err(bad());
    }
    let flags = bytes[3];
    let mut at = 10;
    if flags & 4 != 0 {
        let len = usize::from(u16::from_le_bytes([bytes[at], bytes[at + 1]]));
        at += 2 + len;
    }
    for flag in [8, 16] {
        if flags & flag != 0 {
            at += bytes.get(at..).and_then(|b| b.iter().position(|&c| c == 0)).ok_or_else(bad)? + 1;
        }
    }
    if flags & 2 != 0 {
        at += 2;
    }
    let body = bytes.get(at..).ok_or_else(bad)?;
    miniz_oxide::inflate::decompress_to_vec_with_limit(body, INFLATE_LIMIT).map_err(|_| bad())
}

/// Whether a click's `key` is one of the replay's pace keys (`Space`, `+`/`-`, `.`): the side
/// panel's clock buttons press these, and they drive the replay. Any other key a click presses is
/// free look's, even one that is a watcher control when typed.
pub fn is_pace_key(key: &KeyEvent) -> bool {
    matches!(key.code, KeyCode::Char(' ' | '+' | '=' | '-' | '_' | '.'))
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
            timeline: Timeline::new(text),
            status: None,
        })
    }

    /// Replaces the replay with one of `text`, keeping this one if `text` is refused.
    pub fn replace(&mut self, text: &str) -> Result<(), String> {
        *self = Self::open(text)?;
        Ok(())
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

    pub fn timeline(&self) -> &Timeline {
        &self.timeline
    }

    pub fn timeline_mut(&mut self) -> &mut Timeline {
        &mut self.timeline
    }

    /// Carries out one of the controls' actions.
    pub fn act(&mut self, action: Action) {
        match action {
            Action::Key(c) => self.key(KeyEvent::new(KeyCode::Char(c), KeyModifiers::NONE)),
            Action::Seek(turn) => self.replay.seek(turn),
        }
    }

    /// A file dropped on the window (or picked on the web page): its log replaces the replay,
    /// or the controls say why not.
    fn take_dropped(&mut self, ctx: &egui::Context) {
        if let Some(file) = ctx.input(|i| i.raw.dropped_files.first().cloned()) {
            let name = file.path().display().to_string();
            #[cfg(not(target_arch = "wasm32"))]
            self.open_bytes(&name, file.bytes());
            #[cfg(target_arch = "wasm32")]
            wasm_bindgen_futures::spawn_local(async move {
                let bytes = file.bytes_async().await;
                arrived(name, bytes);
            });
        }
        if let Some((name, bytes)) = take_arrived() {
            self.open_bytes(&name, bytes);
        }
    }

    /// Opens the log `name` holds, as read (`bytes`), or says why it could not.
    pub fn open_bytes(&mut self, name: &str, bytes: Result<Vec<u8>, String>) {
        let result = bytes.and_then(|b| log_text(&b)).and_then(|t| self.replace(&t));
        if let Err(e) = result {
            self.status = Some(format!("{name}: {e}"));
        }
    }

    /// One typed key, as the terminal watcher takes it: a watcher control, else free look.
    pub fn key(&mut self, key: KeyEvent) {
        self.replay.key(key);
    }

    /// One key a click in the game pressed: the clock's go to the replay, the rest to free look.
    fn click_key(&mut self, key: KeyEvent) {
        if is_pace_key(&key) {
            self.replay.key(key);
        } else {
            self.replay.watcher_mut().free_look_key(key);
        }
    }

    /// The keys and the clock: a seek takes up to [`SEEK_BUDGET`] of each frame, and otherwise
    /// the replay plays the real seconds since the last frame at its pace.
    fn step(&mut self, ctx: &egui::Context) {
        let now = Instant::now();
        let dt = now.duration_since(self.last_frame).as_secs_f64().min(0.25);
        self.last_frame = now;
        // A key typed while a button has the focus is the button's (Space presses it).
        if !ctx.egui_wants_keyboard_input() {
            for key in ctx.input(crate::keys::frame_keys) {
                self.key(key);
            }
        }
        self.take_dropped(ctx);
        self.timeline.scan_for(crate::timeline::SCAN_BUDGET);
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
        let whole = root.available_rect_before_wrap();
        root.painter().rect_filled(whole, 0.0, cells::DEFAULT_BG);
        let split = (whole.bottom() - CONTROLS_HEIGHT).max(whole.top());
        let window = Rect::from_min_max(whole.min, pos2(whole.right(), split));
        let controls = Rect::from_min_max(pos2(whole.left(), split), whole.max);
        self.show_controls(root, controls);
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

    /// The controls and the timeline in `rect`, then what they asked for.
    fn show_controls(&mut self, root: &mut Ui, rect: Rect) {
        let fill = cells::DEFAULT_BG.lerp_to_gamma(Color32::WHITE, 0.06);
        root.painter().rect_filled(rect, 0.0, fill);
        let mut actions = Vec::new();
        root.scope_builder(UiBuilder::new().max_rect(rect.shrink2(vec2(8.0, 4.0))), |ui| {
            if let Some(status) = &self.status {
                ui.colored_label(Color32::from_rgb(0xe0, 0x6c, 0x6c), status);
            }
            actions = self.timeline.show(ui, &self.replay);
        });
        for action in actions {
            self.act(action);
        }
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
        // In free look the agent's cursor is marked, so the watcher can find it again.
        if !watcher.following() {
            let at = watcher.agent_cursor();
            self.frame.outlines.push(Outline { at, w: 1, h: 1, color: ATTENTION });
        }
        let mut ui = root.new_child(UiBuilder::new().max_rect(rect));
        ui.set_clip_rect(rect.intersect(root.clip_rect()));
        let (palette, frame) = (&self.palette, &mut self.frame);
        let watcher = self.replay.watcher_mut();
        watcher.with_viewed_shell(|shell| frame.show(&mut ui, shell, palette));
        for key in self.frame.take_keys() {
            self.click_key(key);
        }
        for action in self.frame.take_view_actions() {
            self.replay.watcher_mut().spectator_notice_action(action);
        }
    }
}

/// The replay page (#999): a [`ReplayApp`] once a log is open, and before that a note saying how
/// to open one. A log arrives by a drop on the window, or from the page around the canvas (its
/// file picker and its run list hand the bytes to [`arrived`]); a later one replaces the first.
#[derive(Default)]
pub struct ReplayPage {
    app: Option<ReplayApp>,
    /// Why the last log could not be opened, while none is.
    status: Option<String>,
    /// The state last written onto the page (`crate::webpage::publish_replay`).
    #[cfg(target_arch = "wasm32")]
    published: String,
}

impl ReplayPage {
    /// The replay, once a log is open.
    pub fn app(&self) -> Option<&ReplayApp> {
        self.app.as_ref()
    }

    /// Why the last log could not be opened, while none is.
    pub fn status(&self) -> Option<&str> {
        self.status.as_deref()
    }

    pub fn show(&mut self, root: &mut Ui) {
        let ctx = root.ctx().clone();
        if self.app.is_none() {
            self.take_first(&ctx);
        }
        #[cfg(target_arch = "wasm32")]
        {
            let state = match &self.app {
                Some(app) => vec![
                    ("mode", "replay".to_owned()),
                    ("turn", app.replay().turn().to_string()),
                    ("total", app.replay().total_turns().to_string()),
                    ("status", app.status.clone().unwrap_or_default()),
                ],
                None => vec![
                    ("mode", "replay-empty".to_owned()),
                    ("status", self.status.clone().unwrap_or_default()),
                ],
            };
            crate::webpage::publish_replay(&state, &mut self.published);
        }
        match &mut self.app {
            Some(app) => app.show(root),
            None => {
                let window = root.available_rect_before_wrap();
                root.painter().rect_filled(window, 0.0, cells::DEFAULT_BG);
                let mut note =
                    "Open a run's run.jsonl (or .jsonl.gz) with Open log…, or drop it here.\n\
                    It's replayed here and never uploaded anywhere."
                        .to_owned();
                if let Some(status) = &self.status {
                    note = format!("{status}\n\n{note}");
                }
                root.painter().text(
                    window.center(),
                    egui::Align2::CENTER_CENTER,
                    note,
                    egui::FontId::proportional(16.0),
                    Color32::from_gray(150),
                );
                ctx.request_repaint_after(std::time::Duration::from_millis(100));
            }
        }
    }

    /// The first log: dropped on the window, or handed over by the page.
    fn take_first(&mut self, ctx: &egui::Context) {
        if let Some(file) = ctx.input(|i| i.raw.dropped_files.first().cloned()) {
            let name = file.path().display().to_string();
            #[cfg(not(target_arch = "wasm32"))]
            arrived(name, file.bytes());
            #[cfg(target_arch = "wasm32")]
            wasm_bindgen_futures::spawn_local(async move {
                let bytes = file.bytes_async().await;
                arrived(name, bytes);
            });
        }
        if let Some((name, bytes)) = take_arrived() {
            match bytes.and_then(|b| log_text(&b)).and_then(|t| ReplayApp::open(&t)) {
                Ok(app) => {
                    self.app = Some(app);
                    self.status = None;
                }
                Err(e) => self.status = Some(format!("Can't replay {name}: {e}")),
            }
        }
    }
}

impl eframe::App for ReplayPage {
    fn ui(&mut self, root: &mut Ui, _frame: &mut eframe::Frame) {
        self.show(root);
    }
}

/// A file's name and its bytes as read (or why they could not be).
type ReadFile = (String, Result<Vec<u8>, String>);

thread_local! {
    /// A log read in the background (the browser reads a dropped or picked file asynchronously),
    /// waiting for the next frame to open it.
    static ARRIVED: std::cell::RefCell<Option<ReadFile>> =
        const { std::cell::RefCell::new(None) };
}

/// Hands a log read in the background to the next frame ([`ReplayApp`] opens it).
pub fn arrived(name: String, bytes: Result<Vec<u8>, String>) {
    ARRIVED.with(|a| *a.borrow_mut() = Some((name, bytes)));
}

fn take_arrived() -> Option<ReadFile> {
    ARRIVED.with(|a| a.borrow_mut().take())
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
