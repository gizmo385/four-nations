//! The app: the game session plus what the player sees of it, and the one place keys change them.

use std::path::PathBuf;

use crossterm::event::{KeyCode, KeyEvent, KeyEventKind};
use fn_sim::{Nation, World};
use fn_ui::research::screen_view;
use fn_ui::saves;
use fn_ui::tools::{apply_rect, cycle_ability, switch_tool};
use fn_ui::{Session, Tool};

use crate::input::{Action, map_key};
use crate::keys::Step;
use crate::research;
use crate::saves as saves_screen;
use crate::screens;
use crate::state::{Modal, PINNED, Prompt, Screen, UiState};

/// What `F5`, `F9` and `F6` say when there is no save directory.
const NO_SAVE_DIR: &str = "Save failed: this platform has no data directory (set FN_SAVE_DIR)";

/// The game and what the player sees of it. All input goes through [`App::key`].
pub struct App {
    pub session: Session,
    pub ui: UiState,
    /// Set by anything that changes what the frame shows, cleared when the frame is drawn.
    pub dirty: bool,
    /// The terminal's size, for a new map's view.
    screen: (u16, u16),
    /// Where `F5` and `F9` keep the quicksave (`fn_ui::saves`); `None` when there is nowhere,
    /// and then both keys say so. The binary sets it from the environment; tests set their own.
    pub saves: Option<PathBuf>,
}

impl App {
    pub fn new(session: Session, screen: (u16, u16)) -> Self {
        let mut ui = UiState::new(&session, screen);
        ui.alerts.update(&session, 0.0);
        Self { session, ui, dirty: true, screen, saves: None }
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
        self.ui.status.clear();
        if let Some(prompt) = self.ui.prompt.take() {
            // Only a plain y confirms; anything else (Esc, n, a stray key) stays.
            if matches!(key.code, KeyCode::Char('y' | 'Y')) {
                self.confirm_prompt(prompt);
            }
            return;
        }
        if let Some(modal) = self.ui.modal {
            self.modal_key(modal, key);
            return;
        }
        // The saves screen's own two keys: `n` (new) is free everywhere, but `s` already moves
        // the highlight down, so only its shifted, upper-case spelling saves over it (a
        // terminal sends Shift+s as `S`, with or without the Shift flag: `input::map_key`'s own
        // comment on upper case).
        if self.ui.screen == Some(Screen::Saves) {
            match key.code {
                KeyCode::Char('n' | 'N') => return self.new_save(),
                KeyCode::Char('S') => return self.ask_save_over(),
                _ => {}
            }
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
            // `U` opens the Build modal instead of switching tools (#285); every other tool key
            // still goes straight to `pick_tool`.
            Action::PickTool(Tool::Build) => self.open_build_modal(),
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
            Action::ShowSaves => self.show_saves(),
            Action::AskNewMap => self.ui.prompt = Some(Prompt::NewMap),
            Action::AskNation(nation) => self.ui.prompt = Some(Prompt::Nation(nation)),
            Action::TogglePause => self.session.clock.toggle_pause(),
            Action::Faster => self.session.clock.faster(),
            Action::Slower => self.session.clock.slower(),
            Action::Step => self.step(),
            Action::QuickSave => self.quicksave(),
            Action::AskQuickLoad => self.ui.prompt = Some(Prompt::QuickLoad),
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
        if self.ui.screen == Some(Screen::Saves) {
            // The list has a highlight, not a scroll: the arrows move it by one slot whatever
            // their Shift state (there is no page to jump), and Enter asks to load.
            match action {
                Action::Move { dy, .. } => return self.move_save_cursor(dy),
                Action::Confirm => return self.ask_load_save(),
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
            Action::ShowSaves => self.show_saves(),
            Action::TogglePause => self.session.clock.toggle_pause(),
            Action::Faster => self.session.clock.faster(),
            Action::Slower => self.session.clock.slower(),
            Action::Step => self.step(),
            Action::AskNewMap => self.ui.prompt = Some(Prompt::NewMap),
            Action::AskNation(nation) => self.ui.prompt = Some(Prompt::Nation(nation)),
            Action::QuickSave => self.quicksave(),
            Action::AskQuickLoad => self.ui.prompt = Some(Prompt::QuickLoad),
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
            if screen == Screen::Saves {
                self.refresh_saves();
                self.ui.save_cursor = 0;
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
        let Some(screen) =
            self.ui.screen.filter(|s| !matches!(s, Screen::Research | Screen::Saves))
        else {
            return;
        };
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
            Prompt::QuickLoad => self.quickload(),
            Prompt::LoadSave(name) => self.load_save(&name),
            Prompt::SaveOver(name) => self.save_over(&name),
        }
    }

    /// `F5`: writes the game to the quicksave slot and says how that went in the status line.
    fn quicksave(&mut self) {
        self.ui.status = match self.saves.as_deref() {
            None => NO_SAVE_DIR.to_owned(),
            Some(dir) => match saves::quicksave(dir, &self.session) {
                Ok(()) => format!("Quicksaved (tick {})", self.session.world.tick),
                Err(why) => format!("Quicksave failed: {why}"),
            },
        };
    }

    /// `F9`, once confirmed: the quicksave becomes the game. A load that fails changes nothing
    /// but the status line.
    fn quickload(&mut self) {
        let loaded = match self.saves.as_deref() {
            None => Err(NO_SAVE_DIR.to_owned()),
            Some(dir) => saves::quickload(dir, &self.session.defs)
                .map_err(|why| format!("Quickload failed: {why}")),
        };
        match loaded {
            Ok(world) => {
                self.load(world);
                self.ui.status = format!("Quickloaded (tick {})", self.session.world.tick);
            }
            Err(why) => self.ui.status = why,
        }
    }

    /// `F6`: opens the saves screen (or closes it, as `L` and `?` do their own), re-reading the
    /// directory. Without one, it says so in the status line, as `F5` and `F9` do, and does not
    /// open an empty screen.
    fn show_saves(&mut self) {
        if self.saves.is_none() {
            self.ui.status = NO_SAVE_DIR.to_owned();
            return;
        }
        self.open(Screen::Saves);
    }

    /// Re-reads the save directory into the saves screen's list, clamping the highlight to it.
    /// Called when the screen opens and whenever a save in it changes the list.
    fn refresh_saves(&mut self) {
        self.ui.save_slots = match self.saves.as_deref() {
            None => Vec::new(),
            Some(dir) => match saves::list(dir, &self.session.defs) {
                Ok(slots) => slots,
                Err(why) => {
                    self.ui.status = why;
                    Vec::new()
                }
            },
        };
        let last = self.ui.save_slots.len().saturating_sub(1);
        self.ui.save_cursor = self.ui.save_cursor.min(last);
    }

    /// The saves screen's arrows (and `W`/`S`): one slot up or down, whatever their Shift state.
    fn move_save_cursor(&mut self, dy: i32) {
        self.ui.save_cursor = saves_screen::step(&self.ui.save_slots, self.ui.save_cursor, dy);
    }

    /// `Enter` in the saves screen: asks to load the highlighted slot.
    fn ask_load_save(&mut self) {
        if let Some(slot) = self.ui.save_slots.get(self.ui.save_cursor) {
            self.ui.prompt = Some(Prompt::LoadSave(slot.name.clone()));
        }
    }

    /// `Shift+S` in the saves screen: asks to save over the highlighted slot.
    fn ask_save_over(&mut self) {
        if let Some(slot) = self.ui.save_slots.get(self.ui.save_cursor) {
            self.ui.prompt = Some(Prompt::SaveOver(slot.name.clone()));
        }
    }

    /// `Enter` in the saves screen, once confirmed: loads the named slot through [`Self::load`],
    /// as `F9` does the quicksave. A load that fails changes nothing but the status line.
    fn load_save(&mut self, name: &str) {
        let loaded = match self.saves.as_deref() {
            None => Err(NO_SAVE_DIR.to_owned()),
            Some(dir) => saves::load(dir, name, &self.session.defs)
                .map_err(|why| format!("Load failed: {why}")),
        };
        match loaded {
            Ok(world) => {
                self.load(world);
                self.ui.status = format!("Loaded {name} (tick {})", self.session.world.tick);
            }
            Err(why) => self.ui.status = why,
        }
    }

    /// `Shift+S` in the saves screen, once confirmed: writes the game over the named slot.
    fn save_over(&mut self, name: &str) {
        self.ui.status = match self.saves.as_deref() {
            None => NO_SAVE_DIR.to_owned(),
            Some(dir) => match saves::save(dir, name, &self.session) {
                Ok(()) => format!("Saved {name} (tick {})", self.session.world.tick),
                Err(why) => format!("Save failed: {why}"),
            },
        };
        self.refresh_saves();
    }

    /// `N` in the saves screen: writes a new slot ([`fn_ui::saves::new_name`]), with no prompt
    /// since nothing is overwritten.
    fn new_save(&mut self) {
        let Some(dir) = self.saves.clone() else {
            self.ui.status = NO_SAVE_DIR.to_owned();
            return;
        };
        let name = saves::new_name(&self.ui.save_slots);
        self.ui.status = match saves::save(&dir, &name, &self.session) {
            Ok(()) => format!("Saved {name} (tick {})", self.session.world.tick),
            Err(why) => format!("Save failed: {why}"),
        };
        self.refresh_saves();
    }

    /// Makes `world` the game. As in the Bevy client, only the world is in a save: the cursor,
    /// the selection, the tool, the screens and the log start over, and the clock (speed and
    /// pause) is the player's and stays.
    pub fn load(&mut self, world: World) {
        self.session.world = world;
        self.session.bend_choice = None;
        self.ui = UiState::new(&self.session, self.screen);
        // The loaded game's past events are not news: they age out at once instead of showing
        // as toasts.
        self.ui.alerts.update(&self.session, f32::MAX);
        self.dirty = true;
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

    /// A tool key other than `U`, which opens the Build modal instead ([`Self::open_build_modal`]):
    /// the tool becomes active, and the corner and the last result are dropped (pressing the
    /// active tool's own key again does just that).
    fn pick_tool(&mut self, requested: Tool) {
        // `B` again moves on to the next ability the settlement can use (#123).
        cycle_ability(&mut self.session, self.ui.tool, requested);
        let (tool, choice) = switch_tool(&self.session, self.ui.tool, self.ui.choice, requested);
        self.ui.choice = choice;
        self.ui.set_tool(tool);
    }

    /// `U`: opens the Build modal (#285) instead of switching tools; the tool and the choice
    /// stay as they were until `Enter` there picks one. The highlight starts on the current
    /// choice if it can still be placed, else the first one that can (`fn_ui::build::valid_choice`,
    /// as `switch_tool` used to resolve it for the plain tool key).
    fn open_build_modal(&mut self) {
        let rows = fn_ui::build::picker_rows(&self.session);
        let choice = fn_ui::build::valid_choice(&rows, self.ui.choice);
        self.ui.build_cursor =
            choice.and_then(|id| rows.iter().position(|r| r.id == id)).unwrap_or(0);
        self.ui.modal = Some(Modal::Build);
    }

    /// What a key does while a modal is open: it captures every key until the modal resolves
    /// itself, so no tool key, screen key or clock action reaches [`Self::act`].
    fn modal_key(&mut self, modal: Modal, key: KeyEvent) {
        match modal {
            Modal::Build => self.build_modal_key(key),
        }
    }

    /// The Build modal's own keys (#285, `crate::build`): arrows or `W`/`S` move the highlight
    /// by one row (locked included); `Enter` on an unlocked building picks it, closes the modal
    /// and leaves the Build tool active with it; `Esc` leaves the tool and the choice as they
    /// were; any other letter jumps the highlight to the next name that starts with it.
    fn build_modal_key(&mut self, key: KeyEvent) {
        let rows = fn_ui::build::picker_rows(&self.session);
        if rows.is_empty() {
            if key.code == KeyCode::Esc {
                self.ui.modal = None;
            }
            return;
        }
        let at = self.ui.build_cursor.min(rows.len() - 1);
        match key.code {
            KeyCode::Esc => self.ui.modal = None,
            KeyCode::Up | KeyCode::Char('w' | 'W') => {
                self.ui.build_cursor = crate::build::step(&rows, at, -1);
            }
            KeyCode::Down | KeyCode::Char('s' | 'S') => {
                self.ui.build_cursor = crate::build::step(&rows, at, 1);
            }
            KeyCode::Enter => {
                let row = &rows[at];
                if !row.locked() {
                    self.ui.choice = Some(row.id);
                    self.ui.set_tool(Tool::Build);
                    self.ui.modal = None;
                }
            }
            KeyCode::Char(c) if c.is_alphabetic() => {
                self.ui.build_cursor = crate::build::jump_to_letter(&rows, at, c);
            }
            _ => {}
        }
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
                    self.ui.choice,
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
                Step::Tick(n) => self.run_ticks(n),
            }
            if self.ui.quit {
                return;
            }
        }
    }

    /// Runs `n` ticks whatever the clock says, as a key script's `<tick N>` does, and settles
    /// what they logged. Public so [`crate::shell::Shell`] can run the same ticks from a script
    /// that starts at the title screen (#280).
    pub fn run_ticks(&mut self, n: u32) {
        self.session.tick_n(n);
        self.settle(0.0);
    }
}
