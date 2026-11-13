//! The window: the shell, the keys and the clock. What goes on screen is [`crate::frame`]'s.
//!
//! Like `fn_tui::run`, this is the only place that reads the clock: each frame hands the shell
//! the keys egui collected and the real seconds since the last frame.

use web_time::Instant;

use eframe::egui;
use fn_sim::Nation;
use fn_tui::color::{ColorDepth, Palette};
use fn_tui::shell::{Mode, Shell};

use crate::frame::Frame;

/// Tells the game's alerts (if `shell` is in one) that the window draws corner notices.
fn pin_notices(shell: &mut Shell) {
    if let Mode::Game(app) = &mut shell.mode {
        app.ui.alerts.notices_mut().set_on(true);
    }
}

pub struct GuiApp {
    shell: Shell,
    palette: Palette,
    nation: Nation,
    /// The nation the egui style (`crate::style`) was last built for.
    styled: Option<Nation>,
    frame: Frame,
    last_frame: Instant,
    /// The state last written onto the page (`crate::webpage::publish`).
    #[cfg(target_arch = "wasm32")]
    published: String,
}

impl GuiApp {
    /// The shell `fn_tui`'s own arguments describe (the title screen with none, a game with
    /// `SEED NATION`, `--start`, `--map-size` and the rest, as `fn_tui --help` lists them).
    pub fn new(saves: Option<fn_ui::saves::Store>, seed_hint: u64) -> Self {
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
        pin_notices(&mut shell);
        shell.play(&keys);
        let mut app = Self::with_shell(shell);
        // `FN_GUI_CELL=N`: the map widget's first cell height, for screenshots of a zoom level.
        app.frame.first_cell = std::env::var("FN_GUI_CELL").ok().and_then(|v| v.parse().ok());
        app
    }

    pub fn with_shell(shell: Shell) -> Self {
        let nation = shell.nation();
        let palette = Palette::new(shell.defs(), nation, ColorDepth::TrueColor, false);
        Self {
            shell,
            palette,
            nation,
            styled: None,
            frame: Frame::default(),
            last_frame: Instant::now(),
            #[cfg(target_arch = "wasm32")]
            published: String::new(),
        }
    }

    pub fn shell(&self) -> &Shell {
        &self.shell
    }

    /// The window's frame state: what it showed last, and the hint bar's text.
    pub fn frame(&self) -> &Frame {
        &self.frame
    }

    fn step(&mut self, ctx: &egui::Context) {
        let now = Instant::now();
        let dt = now.duration_since(self.last_frame).as_secs_f64().min(0.25);
        self.last_frame = now;
        // A text field with the keyboard (the saves screen's name) turns typing into the keys
        // itself, so the raw typing isn't given to the shell as well.
        let field = ctx.egui_wants_keyboard_input();
        self.show_notices();
        for key in ctx.input(crate::keys::frame_keys) {
            if field && crate::keys::is_typing(&key) {
                continue;
            }
            // F12 is the window's own: it shows or hides the dev timing overlay.
            if key.code == fn_ui::keyev::KeyCode::F(12) {
                self.frame.overlay = !self.frame.overlay;
                continue;
            }
            self.shell.key(key);
        }
        #[cfg(target_arch = "wasm32")]
        crate::webpage::publish(&self.shell, &mut self.published);
        if self.shell.quit {
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }
        self.show_notices();
        let started = Instant::now();
        if self.shell.paused() {
            self.shell.age(dt);
        } else {
            self.shell.advance(dt);
        }
        self.frame.timings.sim = started.elapsed().as_secs_f32() * 1000.0;
        if self.shell.nation() != self.nation {
            self.nation = self.shell.nation();
            self.palette =
                Palette::new(self.shell.defs(), self.nation, ColorDepth::TrueColor, false);
        }
    }
}

impl GuiApp {
    /// Tells a game's alerts that this window draws corner notices (#945), so the events that
    /// are notices are no longer toasts as well. Every frame: a new game starts with it off.
    fn show_notices(&mut self) {
        pin_notices(&mut self.shell);
    }

    /// One frame of the window into `root`: the keys and the clock, then [`Frame::show`].
    pub fn show(&mut self, root: &mut egui::Ui) {
        let frame_start = Instant::now();
        let ctx = root.ctx().clone();
        self.step(&ctx);
        // The Seal style follows the nation: built on the first frame and when the nation
        // changes, and handed to the root `Ui` now, which took its copy before this frame's step.
        if self.styled != Some(self.nation) {
            crate::style::install(&ctx, &self.palette.theme);
            self.styled = Some(self.nation);
        }
        root.set_style(ctx.global_style());
        self.frame.show(root, &mut self.shell, &self.palette);
        // A click on the window's own widgets is the key it stands for, typed after the frame.
        for key in self.frame.take_keys() {
            self.shell.key(key);
        }
        self.frame.timings.frame = frame_start.elapsed().as_secs_f32() * 1000.0;
        ctx.request_repaint();
    }
}

impl eframe::App for GuiApp {
    fn ui(&mut self, root: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.show(root);
    }
}
