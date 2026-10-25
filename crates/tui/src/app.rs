//! The app: the game session plus what the player sees of it, and the one place keys change them.

use crossterm::event::{KeyCode, KeyEvent, KeyEventKind};
use fn_sim::Nation;
use fn_ui::research::screen_view;
use fn_ui::tools::{apply_rect, cycle_ability, switch_tool};
use fn_ui::{Session, Tool};

use crate::input::{Action, map_key};
use crate::keys::Step;
use crate::research;
use crate::screens;
use crate::state::{PINNED, Prompt, Screen, UiState};

/// The game and what the player sees of it. All input goes through [`App::key`].
pub struct App {
    pub session: Session,
    pub ui: UiState,
    /// Set by anything that changes what the frame shows, cleared when the frame is drawn.
    pub dirty: bool,
    /// The terminal's size, for a new map's view.
    screen: (u16, u16),
}

impl App {
    pub fn new(session: Session, screen: (u16, u16)) -> Self {
        let mut ui = UiState::new(&session, screen);
        ui.alerts.update(&session, 0.0);
        Self { session, ui, dirty: true, screen }
    }

    pub fn resize(&mut self, width: u16, height: u16) {
        self.screen = (width, height);
        self.ui.resize(width, height);
        self.dirty = true;
    }

    /// Handles one key. Keys act between ticks: the loop runs ticks only after the pending keys
    /// are handled, so a command a key issues never lands in the middle of one.
    pub fn key(&mut self, key: KeyEvent) {
        if key.kind == KeyEventKind::Release {
            return;
        }
        self.dirty = true;
        if let Some(prompt) = self.ui.prompt.take() {
            // Only a plain y confirms; anything else (Esc, n, a stray key) stays.
            if matches!(key.code, KeyCode::Char('y' | 'Y')) {
                self.confirm_prompt(prompt);
            }
            return;
        }
        if let Some(action) = map_key(key) {
            self.act(action);
        }
    }

    pub fn act(&mut self, action: Action) {
        if self.ui.screen.is_some() {
            return self.act_in_screen(action);
        }
        match action {
            Action::Move { dx, dy, tiles } => self.ui.move_cursor(dx * tiles, dy * tiles),
            Action::Jump { dx, dy } => self.ui.jump(dx, dy),
            Action::PickTool(tool) => self.pick_tool(tool),
            Action::Confirm => self.confirm(),
            Action::Cancel => self.cancel(),
            Action::NextUnit if self.ui.tool == Tool::Select => self.ui.cycle(&self.session, 1),
            Action::PrevUnit if self.ui.tool == Tool::Select => self.ui.cycle(&self.session, -1),
            Action::Follow if self.ui.tool == Tool::Select => {
                self.ui.toggle_follow(&self.session);
            }
            // Selecting and following are for Select mode: in a tool these keys do nothing.
            Action::NextUnit | Action::PrevUnit | Action::Follow => {}
            Action::ShowResearch => self.open(Screen::Research),
            Action::ShowLog => self.open(Screen::Log),
            Action::ShowHelp => self.open(Screen::Help),
            Action::AskNewMap => self.ui.prompt = Some(Prompt::NewMap),
            Action::AskNation(nation) => self.ui.prompt = Some(Prompt::Nation(nation)),
            Action::TogglePause => self.session.clock.toggle_pause(),
            Action::Faster => self.session.clock.faster(),
            Action::Slower => self.session.clock.slower(),
            Action::Step => self.step(),
            Action::AskQuit => self.ui.prompt = Some(Prompt::Quit),
            Action::QuitNow => self.ui.quit = true,
        }
    }

    /// What the keys do while a screen is open: they scroll it and close it, and the clock, the
    /// prompts and quitting still work. Everything else waits until it is closed.
    fn act_in_screen(&mut self, action: Action) {
        if self.ui.screen == Some(Screen::Research) {
            // The tree has a cursor on its cards, not a scroll: the arrows move it (a Shift
            // step is still one card) and Enter chooses.
            match action {
                Action::Move { dx, dy, .. } => return self.move_tech(dx, dy),
                Action::Jump { dy, .. } => return self.move_tech(0, dy * self.tech_page()),
                Action::Confirm => return research::choose(&mut self.session, self.ui.tech),
                _ => {}
            }
        }
        match action {
            Action::Move { dy, tiles, .. } => self.scroll_by(dy * tiles),
            Action::Jump { dy, .. } => {
                let page = self.screen_rows();
                self.scroll_by(dy * page);
            }
            Action::Cancel => self.ui.screen = None,
            Action::ShowResearch => self.open(Screen::Research),
            Action::ShowLog => self.open(Screen::Log),
            Action::ShowHelp => self.open(Screen::Help),
            Action::TogglePause => self.session.clock.toggle_pause(),
            Action::Faster => self.session.clock.faster(),
            Action::Slower => self.session.clock.slower(),
            Action::Step => self.step(),
            Action::AskNewMap => self.ui.prompt = Some(Prompt::NewMap),
            Action::AskNation(nation) => self.ui.prompt = Some(Prompt::Nation(nation)),
            Action::AskQuit => self.ui.prompt = Some(Prompt::Quit),
            Action::QuitNow => self.ui.quit = true,
            Action::PickTool(_)
            | Action::Confirm
            | Action::NextUnit
            | Action::PrevUnit
            | Action::Follow => {}
        }
    }

    /// `.`: one tick while paused.
    fn step(&mut self) {
        if self.session.step() > 0 {
            self.settle(0.0);
        }
    }

    /// Opens `screen`, or closes it if it is the one that is open (`L` and `?` toggle).
    fn open(&mut self, screen: Screen) {
        if self.ui.screen == Some(screen) {
            self.ui.screen = None;
        } else {
            self.ui.screen = Some(screen);
            // The log opens at its newest line, the help at its top, the tree on its target.
            self.ui.scroll = if screen == Screen::Log { PINNED } else { 0 };
            if screen == Screen::Research {
                let view = screen_view(&self.session);
                self.ui.tech = research::initial_cursor(&self.session, &view);
            }
        }
    }

    /// Moves the research cursor `(dx, dy)` cards.
    fn move_tech(&mut self, dx: i32, dy: i32) {
        let view = screen_view(&self.session);
        self.ui.tech = research::step(&view, self.ui.tech, dx.signum(), dy);
    }

    /// How many cards PgUp and PgDn move in the research screen: a page of them.
    fn tech_page(&self) -> i32 {
        let view = screen_view(&self.session);
        let (width, rows) = screens::text_size(self.ui.viewport());
        research::Grid::new((width, rows), view.columns.len()).cards as i32
    }

    /// How many rows of text the open screen shows at once.
    fn screen_rows(&self) -> i32 {
        screens::text_size(self.ui.viewport()).1 as i32
    }

    /// Scrolls the open screen `lines` down (negative: up), within its text.
    fn scroll_by(&mut self, lines: i32) {
        let Some(screen) = self.ui.screen.filter(|s| *s != Screen::Research) else { return };
        let (width, rows) = screens::text_size(self.ui.viewport());
        let total = screens::line_count(screen, &self.session, &self.ui.alerts, width);
        let max = total.saturating_sub(rows);
        let now = self.ui.scroll.min(max) as i64;
        let to = (now + i64::from(lines)).clamp(0, max as i64) as usize;
        // The log sticks to its newest line once it has been scrolled back there.
        self.ui.scroll = if screen == Screen::Log && to == max { PINNED } else { to };
    }

    /// The answer to a prompt was yes.
    fn confirm_prompt(&mut self, prompt: Prompt) {
        let world = &self.session.world;
        match prompt {
            Prompt::Quit => self.ui.quit = true,
            // The client's own rule: R takes the next seed, a digit keeps the seed.
            Prompt::NewMap => self.restart(world.seed.wrapping_add(1), world.nation),
            Prompt::Nation(nation) => self.restart(world.seed, nation),
        }
    }

    /// A new game on a map of the same size: the clock, the selection, the log and the screens
    /// all start over.
    pub fn restart(&mut self, seed: u64, nation: Nation) {
        let map = &self.session.world.map;
        self.session = Session::new(seed, nation, map.width(), map.height());
        self.ui = UiState::new(&self.session, self.screen);
        self.ui.alerts.update(&self.session, 0.0);
        self.dirty = true;
    }

    /// A tool key: the tool becomes active, and the corner and the last result are dropped
    /// (pressing the active tool's own key again does just that).
    fn pick_tool(&mut self, requested: Tool) {
        // `B` again moves on to the next ability the settlement can use (#123), and `U` again to
        // the next building.
        cycle_ability(&mut self.session, self.ui.tool, requested);
        let (tool, choice) = switch_tool(&self.session, self.ui.tool, self.ui.building, requested);
        self.ui.set_tool(tool);
        self.ui.building = choice;
    }

    /// `Enter`. In Select mode it selects the unit under the cursor (again: the next one on that
    /// tile). In a tool the first sets a corner, the second applies the tool to every tile of the
    /// rectangle through `fn_ui`'s shared logic, and the tool stays active for the next one.
    fn confirm(&mut self) {
        if self.ui.tool == Tool::Select {
            self.ui.select_here(&self.session);
            return;
        }
        match self.ui.anchor.take() {
            None => self.ui.anchor = Some(self.ui.cursor),
            Some(corner) => {
                // The shared logic also takes the Shift state (a repeating harvest), which has
                // no key here yet.
                self.ui.notice = apply_rect(
                    &mut self.session,
                    self.ui.tool,
                    corner,
                    self.ui.cursor,
                    false,
                    self.ui.building,
                );
            }
        }
    }

    /// `Esc` in a tool: drop the corner, or with none leave the tool. In Select mode it closes
    /// the selection bar.
    fn cancel(&mut self) {
        if self.ui.tool == Tool::Select {
            self.ui.select(None);
        } else if self.ui.anchor.take().is_none() {
            self.ui.set_tool(Tool::Select);
        }
    }

    /// After ticks ran: turns what the world logged into alerts (the toasts age by `dt` real
    /// seconds), closes the bar on a unit that died and moves the cursor with a followed one.
    fn settle(&mut self, dt: f32) {
        self.ui.alerts.update(&self.session, dt);
        self.ui.track(&self.session);
        self.dirty = true;
    }

    /// Whether a toast is showing, so the loop keeps waking to age it.
    pub fn has_toasts(&self) -> bool {
        !self.ui.alerts.toasts().is_empty()
    }

    /// Runs the ticks `real_secs` of wall time is worth.
    pub fn advance(&mut self, real_secs: f64) {
        if self.session.advance(real_secs) > 0 {
            self.settle(real_secs as f32);
        } else {
            self.age(real_secs);
        }
    }

    /// Time passed with no tick (the game is paused): only the toasts age.
    pub fn age(&mut self, real_secs: f64) {
        if self.has_toasts() {
            self.settle(real_secs as f32);
        }
    }

    /// Plays a key script (`--keys`): each key as if pressed between ticks, and `<tick N>` as N
    /// ticks of the world whatever the clock says. No time passes except through those, so a
    /// seed and a script always give the same game.
    ///
    /// A toast has no real time to age by here, so it stays until later alerts push it out: a
    /// frame after `<tick N>` shows what those ticks just logged.
    pub fn play(&mut self, steps: &[Step]) {
        for step in steps {
            match *step {
                Step::Key(key) => self.key(key),
                Step::Tick(n) => {
                    self.session.tick_n(n);
                    self.settle(0.0);
                }
            }
            if self.ui.quit {
                return;
            }
        }
    }
}
