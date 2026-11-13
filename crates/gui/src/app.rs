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
    /// The turn-sideways screen's state (#975).
    sideways: crate::sideways::Sideways,
    /// Whether an upright window shows the turn-sideways screen: on the web, where it is a
    /// phone held the wrong way; never for a desktop window ([`GuiApp::with_upright_screen`]).
    upright_screen: bool,
    last_frame: Instant,
    /// The state last written onto the page (`crate::webpage::publish`).
    #[cfg(target_arch = "wasm32")]
    published: String,
    /// When the safe-area insets were last read from the page.
    #[cfg(target_arch = "wasm32")]
    safe_read: Option<Instant>,
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
            sideways: crate::sideways::Sideways::default(),
            upright_screen: cfg!(target_arch = "wasm32"),
            last_frame: Instant::now(),
            #[cfg(target_arch = "wasm32")]
            published: String::new(),
            #[cfg(target_arch = "wasm32")]
            safe_read: None,
        }
    }

    /// Turns the turn-sideways screen on or off (tests: it is the web's alone).
    pub fn with_upright_screen(mut self, on: bool) -> Self {
        self.upright_screen = on;
        self
    }

    pub fn shell(&self) -> &Shell {
        &self.shell
    }

    /// The window's frame state: what it showed last, and the hint bar's text.
    pub fn frame(&self) -> &Frame {
        &self.frame
    }

    /// The window's frame, to change its own state (the touch controls' switch, in tests).
    pub fn frame_mut(&mut self) -> &mut Frame {
        &mut self.frame
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
        // A save file exported or imported in the browser (#976): its line, and the list again.
        if let Some(outcome) = crate::savefiles::take_outcome()
            && let Mode::Game(app) = &mut self.shell.mode
        {
            if outcome.changed {
                app.saves_changed(outcome.status);
            } else {
                app.ui.status = outcome.status;
            }
        }
        #[cfg(target_arch = "wasm32")]
        {
            crate::webpage::publish(&self.shell, &mut self.published);
            // The insets change when the phone turns over (the notch moves sides): read them
            // twice a second, not every frame, since reading them makes the browser work out
            // the page's style.
            if self.safe_read.is_none_or(|t| now.duration_since(t).as_secs_f32() >= 0.5) {
                self.frame.safe = crate::webpage::safe_area();
                self.safe_read = Some(now);
            }
        }
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
        // An upright phone (#975): the turn-sideways screen, the game paused under it and deaf
        // to keys until the phone turns back.
        let window = root.max_rect();
        let upright = self.upright_screen && window.height() > window.width();
        if self.sideways.update(upright, &mut self.shell) {
            self.last_frame = Instant::now();
            ctx.input(crate::keys::frame_keys);
            let in_game = matches!(self.shell.mode, Mode::Game(_));
            crate::sideways::show(root, &self.palette, in_game);
            // No repaint asked for: the turn back is a resize, which repaints by itself.
            return;
        }
        self.step(&ctx);
        // The Seal style follows the nation: built on the first frame and when the nation
        // changes, and handed to the root `Ui` now, which took its copy before this frame's step.
        if self.styled != Some(self.nation) {
            crate::style::install(&ctx, &self.palette.theme);
            self.styled = Some(self.nation);
        }
        root.set_style(ctx.global_style());
        // Whatever the saves screen's file buttons were, it says again if it still shows them.
        crate::savefiles::set_targets(String::new());
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
