//! The top-level flow (#280): the title screen, then a game, and (on the menu's Quit…) back again or out.
//! `App` only ever knows the game; `Shell` is the one place that holds either it or the title
//! screen and switches between them, so `--frame`, the golden frames and the interactive loop
//! (`run`) all go through here instead of repeating the switch themselves.

use std::path::PathBuf;

use crate::keyev::{KeyCode, KeyEvent, KeyEventKind};
use fn_sim::embark::Survey;
use fn_sim::{Defs, Nation};
use fn_ui::Session;
use ratatui::buffer::Buffer;
use ratatui::layout::{Position, Rect};

use crate::app::App;
use crate::color::Palette;
use crate::embark::{self, EmbarkScreen};
use crate::keys::Step;
use crate::title::{self, Outcome, Page, TitleScreen};

/// What the shell is showing right now. All three are boxed: `App` and `EmbarkScreen` each
/// carry a whole generated map, and `TitleScreen` grew past `clippy::large_enum_variant`'s
/// threshold once the seed step gained a Difficulty row (#298).
#[derive(Clone)]
pub enum Mode {
    Title(Box<TitleScreen>),
    /// Between the seed step and the game (#290): choosing where to settle.
    Embark(Box<EmbarkScreen>),
    Game(Box<App>),
}

impl std::fmt::Debug for Mode {
    /// Just the variant's name: `App` doesn't derive `Debug` (it's large and mostly internal),
    /// so this is only for test failure messages, not full state dumps.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Mode::Title(_) => "Title",
            Mode::Embark(_) => "Embark",
            Mode::Game(_) => "Game",
        })
    }
}

#[derive(Clone)]
pub struct Shell {
    pub mode: Mode,
    /// `Q` in embark opens this instead of quitting at once: `T` goes back to the title screen,
    /// `Q`/Enter exits the process, anything else cancels (#280). A game's Quit… box asks the same
    /// two things itself (#703), so this is embark's alone.
    pub exit_prompt: bool,
    /// Set by anything that changes what the frame shows, cleared when the frame is drawn, as
    /// `App::dirty` is.
    pub dirty: bool,
    /// Set once the whole process should stop: the title screen's own Quit, the exit prompt's, or
    /// a game's `Ctrl+C` or Quit box.
    pub quit: bool,
    screen: (u16, u16),
    saves: Option<PathBuf>,
    defs: Defs,
    /// Offered as a fresh seed each time a title screen starts, and bumped whenever one does, so
    /// returning to the title screen more than once doesn't always suggest the same seed. Only
    /// `main` and `run` read the wall clock (docs/design/tui.md); this is seeded from there once.
    seed_hint: u64,
    /// `--fast` (#321): a process-wide host setting, so it survives the title screen, a new map
    /// or save, and a nation switch, the way the clock's own speed and pause deliberately don't.
    /// [`Self::with_fast`] sets it (and the current game's, if there is one already);
    /// [`Self::start`] and `App::restart` apply it to every game that appears after.
    fast: bool,
    /// The agent runs folder (#841): `main` resolves it (`fn_runfmt::runs_dir`, #846)
    /// and hands it in with [`Self::with_runs`]. With none, or with nothing in
    /// it, the title screen has no Replays item.
    runs: Option<PathBuf>,
    /// Set when the title screen's Replays was chosen, until the loop takes it
    /// ([`Self::take_replays`]) and opens the run browser on its own terminal.
    replays_wanted: bool,
}

impl Shell {
    /// Starts at the title screen.
    pub fn new_title(screen: (u16, u16), saves: Option<PathBuf>, seed_hint: u64) -> Self {
        let defs = Defs::bundled();
        let title = TitleScreen::new(saves.as_deref(), seed_hint);
        Self {
            mode: Mode::Title(Box::new(title)),
            exit_prompt: false,
            dirty: true,
            quit: false,
            screen,
            saves,
            defs,
            seed_hint,
            fast: false,
            runs: None,
            replays_wanted: false,
        }
    }

    /// Starts straight in a game, skipping the menu: a `SEED` (and `NATION`) on the command line.
    pub fn new_game(session: Session, screen: (u16, u16), saves: Option<PathBuf>) -> Self {
        let defs = session.defs.clone();
        let mut app = App::new(session, screen);
        app.saves = saves.clone();
        let fast = app.fast;
        Self {
            mode: Mode::Game(Box::new(app)),
            exit_prompt: false,
            dirty: true,
            quit: false,
            screen,
            saves,
            defs,
            seed_hint: 0,
            fast,
            runs: None,
            replays_wanted: false,
        }
    }

    /// Starts straight in embark, skipping the menu: `--embark` (#290), with `--difficulty`'s
    /// preset id, if any (#298).
    #[allow(clippy::too_many_arguments)]
    pub fn new_embark(
        seed: u64,
        nation: Nation,
        size: fn_sim::embark::MapSizeDef,
        mode: fn_sim::embark::SurveyMode,
        difficulty: Option<String>,
        screen: (u16, u16),
        saves: Option<PathBuf>,
    ) -> Self {
        let defs = Defs::bundled();
        let embark_screen = EmbarkScreen::new(seed, nation, size, mode, difficulty, screen, &defs);
        Self {
            mode: Mode::Embark(Box::new(embark_screen)),
            exit_prompt: false,
            dirty: true,
            quit: false,
            screen,
            saves,
            defs,
            seed_hint: 0,
            fast: false,
            runs: None,
            replays_wanted: false,
        }
    }

    /// Builds the shell `args` describes (`--embark`, `--start X,Y`, plain `SEED NATION`, or
    /// neither for the title screen): the one place `--frame` and the interactive `run` share
    /// this resolution, since each knows `screen` (the terminal's size) differently.
    pub fn for_args(
        args: &crate::cli::Args,
        screen: (u16, u16),
        saves: Option<PathBuf>,
        seed_hint: u64,
    ) -> Result<Self, String> {
        Self::for_args_inner(args, screen, saves, seed_hint).map(|s| s.with_fast(args.fast))
    }

    /// [`Self::for_args`], before `--fast` (#321) is applied: every return in here reaches a
    /// title screen, embark or a game, so applying it once at the end covers them all.
    fn for_args_inner(
        args: &crate::cli::Args,
        screen: (u16, u16),
        saves: Option<PathBuf>,
        seed_hint: u64,
    ) -> Result<Self, String> {
        if let Some(id) = &args.scenario {
            return Self::for_scenario(id, screen, saves);
        }
        let (Some(seed), Some(nation)) = (args.seed, args.nation) else {
            return Ok(Self::new_title(screen, saves, seed_hint));
        };
        let defs = Defs::bundled();
        // `--difficulty ID` (#298): checked here, against the bundled data, for every path
        // that reaches this point, so an unknown id is a clear usage error rather than a panic
        // or a silent fallback deeper in.
        let difficulty = match &args.difficulty {
            Some(id) if defs.difficulty.preset(id).is_none() => {
                return Err(format!("unknown difficulty preset {id:?}"));
            }
            Some(id) => Some(id.clone()),
            None => None,
        };
        if args.start.is_none() && !args.embark {
            // Plain `SEED NATION`: unchanged, today's centre-spiral path, except `--difficulty`
            // now swaps `None` for the chosen preset.
            let mut session = crate::new_session(seed, nation);
            if let Some(id) = difficulty {
                session.world.set_difficulty(Some(id));
            }
            return Ok(Self::new_game(session, screen, saves));
        }
        let size_id = args.map_size.as_deref().unwrap_or("small");
        let size = defs
            .embark
            .map_size(size_id)
            .cloned()
            .ok_or_else(|| format!("unknown map size {size_id:?}"))?;
        let mode = if args.rough {
            fn_sim::embark::SurveyMode::Rough
        } else {
            fn_sim::embark::SurveyMode::Exact
        };
        if let Some((x, y)) = args.start {
            let cfg = Survey::config(seed, &size);
            let survey = Survey::new(&cfg, &defs);
            let pos = fn_sim::Pos::new(x, y);
            let terrain = survey.map().get(pos).map_or(fn_sim::Terrain::Water, |t| t.terrain);
            let session = Session::found(survey, nation, pos, difficulty).map_err(|p| {
                format!("can't start at ({x}, {y}): {}", embark::problem_text(p, terrain))
            })?;
            Ok(Self::new_game(session, screen, saves))
        } else {
            Ok(Self::new_embark(seed, nation, size, mode, difficulty, screen, saves))
        }
    }

    /// `--scenario ID` (#481, docs/design/agent-scripting.md, "Scenario starts"): the preset's
    /// own seed, nation, map size, difficulty and site, run through [`Self::for_args_inner`]
    /// exactly as if they had been typed on the command line (a synthetic `Args`, the same way
    /// `crate::surface::Host::new` already builds one), then its prelude if it names one
    /// (`crate::surface::prelude::run`) -- so `fn_tui --scenario ID` and `fn_agent serve
    /// --scenario ID` (`Host::from_scenario`) start exactly the same world.
    fn for_scenario(id: &str, screen: (u16, u16), saves: Option<PathBuf>) -> Result<Self, String> {
        let scenario = crate::scenario::find(id)?;
        let synthetic = crate::cli::Args {
            seed: Some(scenario.seed),
            nation: Some(scenario.nation),
            frame: false,
            size: crate::cli::DEFAULT_SIZE,
            ticks: 0,
            keys: Vec::new(),
            ascii: false,
            ansi: false,
            color: None,
            embark: false,
            start: scenario.start()?,
            map_size: scenario.map_size.clone(),
            rough: false,
            difficulty: scenario.difficulty.clone(),
            fast: false,
            scenario: None,
            watch: None,
            pace: None,
            at: None,
            record: None,
            fps: None,
        };
        let mut shell = Self::for_args_inner(&synthetic, screen, saves, 0)?;
        if let (Some(prelude), Mode::Game(app)) = (&scenario.prelude, &mut shell.mode) {
            crate::surface::prelude::run(app, prelude);
        }
        Ok(shell)
    }

    /// The bundled game data: a game loads its own copy, but the title screen (which has no
    /// game yet) reads this one to list nations and saves.
    pub fn defs(&self) -> &Defs {
        &self.defs
    }

    pub fn resize(&mut self, width: u16, height: u16) {
        self.screen = (width, height);
        match &mut self.mode {
            Mode::Game(app) => app.resize(width, height),
            Mode::Embark(screen) => screen.resize(width, height),
            Mode::Title(_) => {}
        }
        self.dirty = true;
    }

    /// The nation whose colours the frame should wear right now: the game's, or at the title
    /// screen (or embark), whichever nation is being picked or seeded.
    pub fn nation(&self) -> Nation {
        match &self.mode {
            Mode::Game(app) => app.session.world.nation,
            Mode::Embark(screen) => screen.nation,
            Mode::Title(title) => title.display_nation(),
        }
    }

    /// Whether the sim should stay still: always at the title screen or embark (there is nothing
    /// to run), or the game's own pause.
    pub fn paused(&self) -> bool {
        match &self.mode {
            Mode::Game(app) => app.session.clock.paused,
            Mode::Title(_) | Mode::Embark(_) => true,
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
        // New map…, confirmed in a game: reopen embark on the next seed instead of restarting in
        // place (docs/design/embark.md, "In the game"). Collected here, outside the match,
        // so it can call `Self::open_embark` once the match's borrow of `self.mode` is done.
        let mut reopen: Option<(u64, Nation, fn_sim::embark::MapSizeDef, Option<String>)> = None;
        match &mut self.mode {
            Mode::Title(title) => {
                let outcome = title.key(key, &self.defs, self.saves.as_deref());
                self.apply(outcome);
            }
            Mode::Embark(screen) => {
                let outcome = screen.key(key, &self.defs);
                if screen.quit {
                    // `Q` in embark asks the same back-to-title question a game's Quit… does
                    // (docs/design/embark.md, "Flow and keys").
                    screen.quit = false;
                    self.exit_prompt = true;
                }
                self.apply_embark(outcome);
            }
            Mode::Game(app) => {
                app.key(key);
                if app.ui.quit {
                    // `Ctrl+C`, or the Quit box's last answer: the process stops, with nothing
                    // more to ask (#703).
                    self.quit = true;
                } else if app.ui.to_title {
                    // The Quit box's way back: it already asked, and saved if asked to.
                    self.go_to_title();
                } else if app.reopen_embark {
                    app.reopen_embark = false;
                    let world = &app.session.world;
                    let seed = world.seed.wrapping_add(1);
                    let nation = world.nation;
                    let difficulty = world.difficulty().map(str::to_owned);
                    let size = self
                        .defs
                        .embark
                        .map_sizes
                        .iter()
                        .find(|s| s.width == world.map.width() && s.height == world.map.height())
                        .cloned();
                    match size {
                        Some(size) => reopen = Some((seed, nation, size, difficulty)),
                        // No preset matches this map's size (shouldn't happen in practice):
                        // fall back to the old in-place restart rather than losing the key.
                        None => app.restart(seed, nation),
                    }
                }
            }
        }
        if let Some((seed, nation, size, difficulty)) = reopen {
            self.open_embark(seed, nation, size, fn_sim::embark::SurveyMode::Exact, difficulty);
        }
    }

    /// Leaves whatever is open for a fresh title screen.
    fn go_to_title(&mut self) {
        self.seed_hint = self.seed_hint.wrapping_add(1);
        self.mode = Mode::Title(Box::new(self.title_screen()));
    }

    /// A fresh title screen, with Replays shown if the runs folder has anything in it.
    fn title_screen(&self) -> TitleScreen {
        TitleScreen::new(self.saves.as_deref(), self.seed_hint)
            .with_replays(self.runs.as_deref().is_some_and(crate::browser::has_runs))
    }

    /// Sets the agent runs folder (#841), the Replays item's target, and shows or hides the
    /// item on a title screen already open.
    pub fn with_runs(mut self, runs: Option<PathBuf>) -> Self {
        self.runs = runs;
        let replays = self.runs.as_deref().is_some_and(crate::browser::has_runs);
        if let Mode::Title(title) = &mut self.mode {
            title.set_replays(replays);
        }
        self
    }

    /// The runs folder to open in the run browser, once, after the title screen's Replays was
    /// chosen (#841). The loop that owns the terminal opens it there and carries on with this
    /// shell when the browser returns; nothing in the shell changes meanwhile.
    pub fn take_replays(&mut self) -> Option<PathBuf> {
        if !std::mem::take(&mut self.replays_wanted) {
            return None;
        }
        self.runs.clone()
    }

    fn key_exit_prompt(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Char('t' | 'T') => {
                self.go_to_title();
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
            Outcome::Replays => self.replays_wanted = self.runs.is_some(),
            Outcome::Embark { seed, nation, size, mode, difficulty } => {
                self.open_embark(seed, nation, size, mode, difficulty)
            }
            Outcome::Loaded(world) => {
                let session = Session {
                    world: *world,
                    defs: self.defs.clone(),
                    clock: fn_ui::clock::Clock::default(),
                    bend_choice: None,
                };
                self.start(session);
                // A loaded game is the saved one: the confirm box counts from here.
                if let Mode::Game(app) = &mut self.mode {
                    app.ui.saved_at = Some(app.session.world.tick);
                }
            }
        }
    }

    /// Opens embark for `seed`, `nation`, `size` and `difficulty` (#290, #298): the title
    /// screen's seed step, a game's New map…, or the direct `--embark` path all come through
    /// here.
    fn open_embark(
        &mut self,
        seed: u64,
        nation: Nation,
        size: fn_sim::embark::MapSizeDef,
        mode: fn_sim::embark::SurveyMode,
        difficulty: Option<String>,
    ) {
        let screen =
            EmbarkScreen::new(seed, nation, size, mode, difficulty, self.screen, &self.defs);
        self.mode = Mode::Embark(Box::new(screen));
    }

    /// What an embark key did: back to the seed step (the seed, size, survey and difficulty
    /// kept), or a settled site becoming the game.
    fn apply_embark(&mut self, outcome: embark::Outcome) {
        match outcome {
            embark::Outcome::None => {}
            embark::Outcome::Back => {
                let Mode::Embark(screen) = &self.mode else { return };
                let mut title = self.title_screen();
                let size_idx = self
                    .defs
                    .embark
                    .map_sizes
                    .iter()
                    .position(|s| s.id == screen.size().id)
                    .unwrap_or(0);
                let difficulty_idx = screen
                    .difficulty()
                    .and_then(|id| self.defs.difficulty.presets.iter().position(|p| p.id == id))
                    .unwrap_or_else(|| crate::title::default_difficulty_idx(&self.defs));
                title.page = Page::Seed {
                    nation: screen.nation,
                    digits: screen.seed().to_string(),
                    edited: true,
                    row: crate::title::SeedRow::Digits,
                    size_idx,
                    survey: screen.mode(),
                    difficulty_idx,
                };
                self.mode = Mode::Title(Box::new(title));
            }
            embark::Outcome::Settle(pos) => {
                let Mode::Embark(screen) = &self.mode else { return };
                let survey: Survey = screen.survey().clone();
                let nation = screen.nation;
                let difficulty = screen.difficulty().map(str::to_owned);
                // The prompt only shows once the report has already said `pos` isn't refused
                // (`EmbarkScreen::ask_or_refuse`), so this can't fail.
                if let Ok(session) = Session::found(survey, nation, pos, difficulty) {
                    self.start(session);
                }
            }
        }
    }

    fn start(&mut self, session: Session) {
        self.defs = session.defs.clone();
        let mut session = session;
        session.clock = session.clock.with_fast(self.fast);
        let mut app = App::new(session, self.screen);
        app.saves = self.saves.clone();
        self.mode = Mode::Game(Box::new(app));
    }

    /// Sets `--fast` (#321): a process-wide host setting applied to the game in front of us now
    /// (if there is one) and every game that appears after, through `Self::start` and
    /// `App::restart`. Agents never call this; it isn't on the agent surface (`crate::surface`).
    pub fn with_fast(mut self, fast: bool) -> Self {
        self.fast = fast;
        if let Mode::Game(app) = &mut self.mode {
            app.fast = fast;
            app.session.clock = app.session.clock.clone().with_fast(fast);
        }
        self
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

/// Draws the shell's frame: the title screen, embark or the game, with embark's exit prompt over it.
pub fn draw(shell: &mut Shell, palette: &Palette, buf: &mut Buffer) {
    let area = buf.area;
    match &shell.mode {
        Mode::Title(title) => title::draw_title(title, &shell.defs, palette, area, buf),
        Mode::Embark(screen) => embark::draw_embark(screen, &shell.defs, palette, area, buf),
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
