//! The top-level flow (#280): the title screen, then a game, and (on `Q`) back again or out.
//! `App` only ever knows the game; `Shell` is the one place that holds either it or the title
//! screen and switches between them, so `--frame`, the golden frames and the interactive loop
//! (`run`) all go through here instead of repeating the switch themselves.

use std::path::PathBuf;

use crossterm::event::{KeyCode, KeyEvent, KeyEventKind};
use fn_sim::{Defs, Nation};
use fn_ui::Session;
use ratatui::buffer::Buffer;
use ratatui::layout::{Position, Rect};

use crate::app::App;
use crate::color::Palette;
use crate::keys::Step;
use crate::title::{self, Outcome, TitleScreen};

/// What the shell is showing right now. `App` is boxed: it carries a whole `World`, far bigger
/// than a `TitleScreen`, and `clippy::large_enum_variant` would otherwise flag the difference.
pub enum Mode {
    Title(TitleScreen),
    Game(Box<App>),
}

pub struct Shell {
    pub mode: Mode,
    /// `Q` then `y` in a game opens this instead of quitting at once: `T` goes back to the title
    /// screen, `Q`/Enter exits the process, anything else cancels and the game resumes (#280).
    pub exit_prompt: bool,
    /// Set by anything that changes what the frame shows, cleared when the frame is drawn, as
    /// `App::dirty` is.
    pub dirty: bool,
    /// Set once the whole process should stop: the title screen's own Quit, or the exit
    /// prompt's.
    pub quit: bool,
    screen: (u16, u16),
    saves: Option<PathBuf>,
    defs: Defs,
    /// Offered as a fresh seed each time a title screen starts, and bumped whenever one does, so
    /// returning to the title screen more than once doesn't always suggest the same seed. Only
    /// `main` and `run` read the wall clock (docs/design/tui.md); this is seeded from there once.
    seed_hint: u64,
}

impl Shell {
    /// Starts at the title screen.
    pub fn new_title(screen: (u16, u16), saves: Option<PathBuf>, seed_hint: u64) -> Self {
        let defs = Defs::bundled();
        let title = TitleScreen::new(&defs, saves.as_deref(), seed_hint);
        Self {
            mode: Mode::Title(title),
            exit_prompt: false,
            dirty: true,
            quit: false,
            screen,
            saves,
            defs,
            seed_hint,
        }
    }

    /// Starts straight in a game, skipping the menu: a `SEED` (and `NATION`) on the command line.
    pub fn new_game(session: Session, screen: (u16, u16), saves: Option<PathBuf>) -> Self {
        let defs = session.defs.clone();
        let mut app = App::new(session, screen);
        app.saves = saves.clone();
        Self {
            mode: Mode::Game(Box::new(app)),
            exit_prompt: false,
            dirty: true,
            quit: false,
            screen,
            saves,
            defs,
            seed_hint: 0,
        }
    }

    /// The bundled game data: a game loads its own copy, but the title screen (which has no
    /// game yet) reads this one to list nations and saves.
    pub fn defs(&self) -> &Defs {
        &self.defs
    }

    pub fn resize(&mut self, width: u16, height: u16) {
        self.screen = (width, height);
        if let Mode::Game(app) = &mut self.mode {
            app.resize(width, height);
        }
        self.dirty = true;
    }

    /// The nation whose colours the frame should wear right now: the game's, or at the title
    /// screen, whichever nation is being picked or seeded.
    pub fn nation(&self) -> Nation {
        match &self.mode {
            Mode::Game(app) => app.session.world.nation,
            Mode::Title(title) => title.display_nation(),
        }
    }

    /// Whether the sim should stay still: always at the title screen (there is nothing to run),
    /// or the game's own pause.
    pub fn paused(&self) -> bool {
        match &self.mode {
            Mode::Game(app) => app.session.clock.paused,
            Mode::Title(_) => true,
        }
    }

    /// Whether a toast is still ageing (only possible in a game).
    pub fn has_toasts(&self) -> bool {
        matches!(&self.mode, Mode::Game(app) if app.has_toasts())
    }

    pub fn key(&mut self, key: KeyEvent) {
        if key.kind == KeyEventKind::Release {
            return;
        }
        self.dirty = true;
        if self.exit_prompt {
            self.key_exit_prompt(key);
            return;
        }
        match &mut self.mode {
            Mode::Title(title) => {
                let outcome = title.key(key, &self.defs, self.saves.as_deref());
                self.apply(outcome);
            }
            Mode::Game(app) => {
                app.key(key);
                if app.ui.quit {
                    // The game asked only `y` to confirm `Q`; that becomes this screen's own
                    // question instead of stopping the process outright (#280).
                    app.ui.quit = false;
                    self.exit_prompt = true;
                }
            }
        }
    }

    fn key_exit_prompt(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Char('t' | 'T') => {
                self.seed_hint = self.seed_hint.wrapping_add(1);
                self.mode = Mode::Title(TitleScreen::new(
                    &self.defs,
                    self.saves.as_deref(),
                    self.seed_hint,
                ));
                self.exit_prompt = false;
            }
            KeyCode::Char('q' | 'Q') | KeyCode::Enter => self.quit = true,
            _ => self.exit_prompt = false,
        }
    }

    fn apply(&mut self, outcome: Outcome) {
        match outcome {
            Outcome::None => {}
            Outcome::Quit => self.quit = true,
            Outcome::Start { seed, nation } => {
                let session = crate::new_session(seed, nation);
                self.start(session);
            }
            Outcome::Loaded(world) => {
                let session = Session {
                    world: *world,
                    defs: self.defs.clone(),
                    clock: fn_ui::clock::Clock::default(),
                    bend_choice: None,
                };
                self.start(session);
            }
        }
    }

    fn start(&mut self, session: Session) {
        self.defs = session.defs.clone();
        let mut app = App::new(session, self.screen);
        app.saves = self.saves.clone();
        self.mode = Mode::Game(Box::new(app));
    }

    /// Runs the ticks `real_secs` of wall time is worth; a no-op at the title screen.
    pub fn advance(&mut self, real_secs: f64) {
        if let Mode::Game(app) = &mut self.mode {
            app.advance(real_secs);
            self.dirty = self.dirty || app.dirty;
        }
    }

    /// Time passed with no tick (paused, or the title screen): only a game's toasts age.
    pub fn age(&mut self, real_secs: f64) {
        if let Mode::Game(app) = &mut self.mode {
            app.age(real_secs);
            self.dirty = self.dirty || app.dirty;
        }
    }

    /// Plays a key script (`--keys`): title navigation and in-game keys both go through
    /// [`Self::key`], so a script can start a game from the menu and carry on playing it.
    pub fn play(&mut self, steps: &[Step]) {
        for step in steps {
            match *step {
                Step::Key(key) => self.key(key),
                Step::Tick(n) => {
                    if let Mode::Game(app) = &mut self.mode {
                        app.run_ticks(n);
                        self.dirty = true;
                    }
                }
            }
            if self.quit {
                return;
            }
        }
    }
}

/// Draws the shell's frame: the title screen or the game, with the exit prompt over either.
pub fn draw(shell: &mut Shell, palette: &Palette, buf: &mut Buffer) {
    let area = buf.area;
    match &shell.mode {
        Mode::Title(title) => title::draw_title(title, &shell.defs, palette, area, buf),
        Mode::Game(app) => crate::draw::draw(&app.session, &app.ui, palette, buf),
    }
    if shell.exit_prompt {
        title::draw_exit_prompt(palette, area, buf);
    }
}

/// Where the terminal's own cursor goes: only in a game, and only as `crate::draw::terminal_cursor`
/// already decides (nowhere with a screen open, the exit prompt included).
pub fn terminal_cursor(shell: &Shell, area: Rect) -> Option<Position> {
    match &shell.mode {
        Mode::Game(app) if !shell.exit_prompt => crate::draw::terminal_cursor(&app.ui, area),
        _ => None,
    }
}
