//! The app: the game session plus what the player sees of it, and the one place keys change them.

use std::path::PathBuf;

use crate::keyev::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use crate::research::screen_view;
use crate::saves;
use crate::toolpanel;
use crate::tools::{Order, apply_order, cycle_ability, switch_tool};
use crate::{Session, Tool};
use fn_sim::{Nation, Pos, World};

use crate::ask::Answer;
use crate::input::{Action, map_key};
use crate::menu::{MenuPage, MenuRow};
use crate::screens::saves as saves_screen;
use crate::screens::{self, abilities, build as build_screen, msglog, nation, research, traders};
use crate::script::Step;
use crate::state::{HelpTab, Modal, Prompt, Screen, UiState};

/// What `F5`, `F9` and `F6` say when there is no save directory.
const NO_SAVE_DIR: &str = "Save failed: this platform has no data directory (set FN_SAVE_DIR)";

/// The game and what the player sees of it. All input goes through [`App::key`].
#[derive(Clone)]
pub struct App {
    pub session: Session,
    pub ui: UiState,
    /// Set by anything that changes what the frame shows, cleared when the frame is drawn.
    pub dirty: bool,
    /// The terminal's size, for a new map's view.
    screen: (u16, u16),
    /// Where `F5` and `F9` keep the quicksave (`crate::saves`); `None` when there is nowhere,
    /// and then both keys say so. The binary sets it from the environment; tests set their own.
    pub saves: Option<PathBuf>,
    /// New map…, confirmed: `Shell` reopens embark on the next seed instead of restarting in place
    /// (#290, docs/design/embark.md, "In the game"), since only it can leave the game
    /// behind for a pre-game screen.
    pub reopen_embark: bool,
    /// The clock's pause from just before the game menu opened (#306), so `close_menu` can put
    /// it back rather than the menu itself toggling pause.
    menu_prev_paused: Option<bool>,
    /// `--fast` (#321): carried across New map… and Switch nation… (`Self::restart`), which otherwise build a
    /// brand new [`Session`] (and so a plain [`crate::Clock::default`]) from scratch. A host or
    /// session setting, not the player's own clock state, so it survives a restart the way the
    /// clock's speed and pause deliberately don't (`Self::restart`'s doc comment).
    pub fast: bool,
    /// How many rows the key help's Legend tab has (see [`screens::LegendRows`]): the frontend
    /// that draws the legend sets it, as `fn_tui::shell::Shell` does; until then the tab has
    /// nothing to scroll.
    pub legend_rows: screens::LegendRows,
}

impl App {
    pub fn new(session: Session, screen: (u16, u16)) -> Self {
        let fast = session.clock.is_fast();
        let mut ui = UiState::new(&session, screen);
        ui.alerts.update(&session, 0.0);
        Self {
            session,
            ui,
            dirty: true,
            screen,
            saves: None,
            reopen_embark: false,
            menu_prev_paused: None,
            fast,
            legend_rows: screens::no_legend,
        }
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
        self.ui.status_badge = None;
        // Ctrl+C quits at once, whatever is open, a question included.
        if map_key(key) == Some(Action::QuitNow) {
            self.ui.quit = true;
            return;
        }
        if let Some(prompt) = self.ui.prompt.clone() {
            self.prompt_key(prompt, key);
            return;
        }
        if let Some(modal) = self.ui.modal {
            self.modal_key(modal, key);
            return;
        }
        // Typing a name for a new save takes every key (#704).
        if self.ui.screen == Some(Screen::Saves) && self.ui.save_name.is_some() {
            return self.save_name_key(key);
        }
        // The saves screen's own two keys: `n` (new) is free everywhere, but `s` already moves
        // the highlight down, so only its shifted, upper-case spelling saves over it (a
        // terminal sends Shift+s as `S`, with or without the Shift flag: `input::map_key`'s own
        // comment on upper case).
        if self.ui.screen == Some(Screen::Saves) {
            match key.code {
                KeyCode::Char('n' | 'N') => return self.start_save_name(),
                KeyCode::Char('S') => return self.ask_save_over(),
                _ => {}
            }
        }
        // The message log's filters (#661): `1` to `5` and `0` on the number row. A keypad digit
        // still moves, as everywhere else.
        if self.ui.screen == Some(Screen::Log)
            && let KeyCode::Char(c @ '0'..='9') = key.code
            && !key.state.contains(crate::keyev::KeyEventState::KEYPAD)
        {
            msglog::press(&mut self.ui, &self.session, c);
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
        if self.ui.tool_list.is_some() && self.tool_list_act(action) {
            return;
        }
        match action {
            Action::Move { dx, dy, tiles } => {
                self.ui.move_cursor(dx * tiles, dy * tiles);
                self.foot_hint();
            }
            Action::Jump { dx, dy } => {
                self.ui.jump(dx, dy);
                self.foot_hint();
            }
            // `U` opens the Build modal instead of switching tools (#285); every other tool key
            // still goes straight to `pick_tool`.
            Action::PickTool(Tool::Build) => self.open_build_modal(),
            Action::PickTool(tool) => self.pick_tool(tool),
            // Standing orders (#277) are Harvest's own toggle, teardown (#335) is Cancel's;
            // `O` does nothing in any other tool, including Select.
            Action::ToggleRepeat if self.ui.tool == Tool::Harvest => {
                self.ui.repeating = !self.ui.repeating;
            }
            Action::ToggleRepeat if self.ui.tool == Tool::CancelHarvest => {
                self.ui.teardown = !self.ui.teardown;
            }
            // `O` on a chute (or chute site) in Select mode re-picks its foot (#683), and on a
            // pair end (any tile of it) its partner (#698).
            Action::ToggleRepeat
                if self.ui.tool == Tool::Select
                    && crate::link::head_at(&self.session, self.ui.cursor).is_some() =>
            {
                self.ui.linking = crate::link::head_at(&self.session, self.ui.cursor);
                self.foot_hint();
            }
            // `O` on a recipe building in an automating yard (#725, #730) turns automation on or
            // off.
            Action::ToggleRepeat if self.ui.tool == Tool::Select => {
                if let Some(outcome) =
                    crate::recipe::toggle_automation(&mut self.session, self.ui.cursor)
                {
                    self.ui.notice = outcome.unwrap_or_else(|why| why);
                }
            }
            Action::ToggleRepeat => {}
            // `R` switches the Workshop under the cursor in Select mode (#712); in a tool it does
            // nothing, like `F`.
            Action::NextRecipe if self.ui.tool == Tool::Select => self.next_recipe(),
            Action::NextRecipe => {}
            Action::Confirm => self.confirm(),
            Action::Cancel => self.cancel(),
            Action::NextUnit if self.ui.tool == Tool::Select => self.ui.cycle(&self.session, 1),
            Action::PrevUnit if self.ui.tool == Tool::Select => self.ui.cycle(&self.session, -1),
            Action::Follow if self.ui.tool == Tool::Select => {
                self.ui.toggle_follow(&self.session);
            }
            // Selecting and following are for Select mode: in a tool these keys do nothing.
            Action::NextUnit | Action::PrevUnit | Action::Follow => {}
            Action::ShowTools => self.open_tool_list(),
            Action::ShowResearch => self.open(Screen::Research),
            Action::ShowLog => self.open(Screen::Log),
            Action::ShowHelp => self.open(Screen::Help),
            Action::ShowEndeavor => self.open(Screen::Endeavor),
            Action::ShowNation => self.open(Screen::Nation),
            Action::ShowAbilities => self.open(Screen::Abilities),
            Action::ShowTraders => self.open(Screen::Traders),
            Action::ShowSaves => self.show_saves(),
            Action::TogglePause => self.session.clock.toggle_pause(),
            Action::Faster => self.session.clock.faster(),
            Action::Slower => self.session.clock.slower(),
            Action::Step => self.step(),
            Action::QuickSave => self.quicksave(),
            Action::AskQuickLoad => self.ask(Prompt::QuickLoad),
            Action::QuitNow => self.ui.quit = true,
        }
    }

    /// `/` (#664): opens the tool list in the panel, the cursor on the active tool's row (the
    /// first with none).
    fn open_tool_list(&mut self) {
        let active = match self.ui.tool {
            Tool::Deconstruct => Tool::Clear,
            other => other,
        };
        let rows = toolpanel::choices(&self.session);
        self.ui.tool_list = Some(rows.iter().position(|c| c.tool == active).unwrap_or(0));
    }

    /// What a key does while the tool list is open (#664); whether it was one the list took. Up
    /// and Down choose (a sideways step or a jump does nothing, rather than moving the map
    /// cursor under a list that is not looking at it), `Enter` or a tool's own letter picks,
    /// `Esc` or `/` closes. The clock keeps working with the list open. Anything else closes
    /// the list and goes on to do what it does.
    fn tool_list_act(&mut self, action: Action) -> bool {
        let rows = toolpanel::choices(&self.session);
        let last = rows.len().saturating_sub(1);
        let at = self.ui.tool_list.unwrap_or(0).min(last);
        match action {
            Action::Move { dy, .. } => {
                self.ui.tool_list = Some((at as i32 + dy.signum()).clamp(0, last as i32) as usize);
                true
            }
            Action::Jump { .. } => true,
            Action::Confirm => {
                if let Some(row) = rows.get(at) {
                    self.pick_from_list(row.tool);
                }
                true
            }
            Action::PickTool(tool) => {
                self.pick_from_list(tool);
                true
            }
            Action::Cancel | Action::ShowTools => {
                self.ui.tool_list = None;
                true
            }
            Action::TogglePause | Action::Faster | Action::Slower | Action::Step => false,
            _ => {
                self.ui.tool_list = None;
                false
            }
        }
    }

    /// Picks `tool` from the list and closes it. The tool already active is left as it is (its
    /// letter pressed again would cycle Bend's ability or flip Clear to buildings, which is not
    /// what picking a row asks for); Build opens its picker, as `U` does.
    fn pick_from_list(&mut self, tool: Tool) {
        self.ui.tool_list = None;
        let active = match self.ui.tool {
            Tool::Deconstruct => Tool::Clear,
            other => other,
        };
        if tool == Tool::Build {
            self.open_build_modal();
        } else if tool != active {
            self.pick_tool(tool);
        }
    }

    /// What the keys do while a screen is open: they scroll it and close it, and the clock, the
    /// prompts and quitting still work. Everything else waits until it is closed.
    fn act_in_screen(&mut self, action: Action) {
        if self.ui.screen == Some(Screen::Research)
            && let Some(tech) = self.ui.school_confirm
        {
            // The CHOOSE A SCHOOL box (#729) takes the keys until it closes; the clock and
            // quitting still work.
            match action {
                Action::Confirm => {
                    self.ui.school_confirm = None;
                    return research::confirm_school(&mut self.session, tech);
                }
                Action::Cancel | Action::ShowResearch => {
                    self.ui.school_confirm = None;
                    return;
                }
                Action::Move { .. } | Action::Jump { .. } | Action::PickTool(_) => return,
                _ => {}
            }
        }
        if self.ui.screen == Some(Screen::Research) {
            // The tree has a cursor on its cards, not a scroll: the arrows move it (a Shift
            // step is still one card) and Enter chooses.
            match action {
                Action::Move { dx, dy, .. } => return self.move_tech(dx, dy),
                Action::Jump { dy, .. } => return self.move_tech(0, dy * self.tech_page()),
                Action::Confirm => {
                    // A school option asks first (#729): choosing it closes its mate.
                    if let Some(tech) = research::needs_confirm(&self.session, self.ui.tech) {
                        self.ui.school_confirm = Some(tech);
                        return;
                    }
                    return research::choose(&mut self.session, self.ui.tech);
                }
                Action::PickTool(Tool::CancelHarvest) => return self.clear_research(),
                _ => {}
            }
        }
        // The log (#661) has a cursor on its lines: the arrows choose, Right opens a fold, Left
        // folds it, and Enter goes to the line's place.
        if self.ui.screen == Some(Screen::Log) && msglog::act(&mut self.ui, &self.session, action) {
            return;
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
        if self.ui.screen == Some(Screen::Endeavor) {
            // The gate (#643): Enter chooses a site once the endeavor is Ready, `X` withdraws a
            // site no work has started on. Both say why when there's nothing to do.
            match action {
                Action::Confirm => return self.start_gate(),
                Action::PickTool(Tool::CancelHarvest) => return self.withdraw_gate(),
                _ => {}
            }
        }
        if self.ui.screen == Some(Screen::Nation) {
            // The roster has a pointer, not a scroll (#308): the arrows move it by one settler
            // (a Shift step is still one), PgUp/PgDn by a page, and Enter selects.
            match action {
                Action::Move { dy, .. } => return self.move_nation_cursor(dy),
                Action::Jump { dy, .. } => return self.move_nation_cursor(dy * self.nation_page()),
                Action::Confirm => return self.confirm_nation(),
                _ => {}
            }
        }
        if self.ui.screen == Some(Screen::Abilities) {
            // The list has a pointer, not a scroll (#719): the arrows move it by one ability (a
            // Shift step is still one), PgUp/PgDn by a page, and Enter picks for the Bend tool.
            match action {
                Action::Move { dy, .. } => return self.move_ability_cursor(dy),
                Action::Jump { dy, .. } => {
                    return self.move_ability_cursor(dy * self.ability_page());
                }
                Action::Confirm => return self.pick_ability(),
                _ => {}
            }
        }
        if self.ui.screen == Some(Screen::Traders) {
            // The offers have a pointer, not a scroll (#438): the arrows move it by one, Enter
            // takes the one under it once more and `X` cancels what is still owed on it.
            match action {
                Action::Move { dy, .. } => {
                    let view = crate::trade::view(&self.session);
                    self.ui.trade_cursor = traders::step(&view, self.ui.trade_cursor, dy);
                    return;
                }
                Action::Confirm => return self.trade(false),
                Action::PickTool(Tool::CancelHarvest) => return self.trade(true),
                _ => {}
            }
        }
        match action {
            // `Tab` flips the key help between its keys and its legend (#662).
            Action::NextUnit | Action::PrevUnit if self.ui.screen == Some(Screen::Help) => {
                self.ui.switch_help_tab();
            }
            Action::Move { dy, tiles, .. } => self.scroll_by(dy * tiles),
            Action::Jump { dy, .. } => {
                let page = self.screen_rows();
                self.scroll_by(dy * page);
            }
            Action::Cancel => self.ui.screen = None,
            Action::ShowResearch => self.open(Screen::Research),
            Action::ShowLog => self.open(Screen::Log),
            Action::ShowHelp => self.open(Screen::Help),
            Action::ShowEndeavor => self.open(Screen::Endeavor),
            Action::ShowNation => self.open(Screen::Nation),
            Action::ShowAbilities => self.open(Screen::Abilities),
            Action::ShowTraders => self.open(Screen::Traders),
            Action::ShowSaves => self.show_saves(),
            Action::TogglePause => self.session.clock.toggle_pause(),
            Action::Faster => self.session.clock.faster(),
            Action::Slower => self.session.clock.slower(),
            Action::Step => self.step(),
            Action::QuickSave => self.quicksave(),
            Action::AskQuickLoad => self.ask(Prompt::QuickLoad),
            Action::QuitNow => self.ui.quit = true,
            Action::PickTool(_)
            | Action::ShowTools
            | Action::ToggleRepeat
            | Action::NextRecipe
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
            self.ui.tool_list = None;
            // The log opens at its newest line (its top), the help at its top, the tree on its
            // target.
            self.ui.scroll = 0;
            if screen == Screen::Log {
                msglog::reset(&mut self.ui);
            }
            self.ui.help_tab = HelpTab::default();
            if screen == Screen::Research {
                let view = screen_view(&self.session);
                self.ui.tech = research::initial_cursor(&self.session, &view);
            }
            if screen == Screen::Saves {
                self.refresh_saves();
                self.ui.save_cursor = 0;
            }
            if screen == Screen::Nation {
                self.ui.nation_cursor = 0;
            }
            if screen == Screen::Abilities {
                let rows = crate::bend::abilities(&self.session);
                self.ui.ability_cursor = abilities::initial_cursor(&rows);
            }
            if screen == Screen::Traders {
                self.ui.trade_cursor = 0;
            }
        }
    }

    /// `Enter` (or, with `cancel`, `X`) in the Traders screen (#438): takes the offer under the
    /// pointer once more, or drops what is still owed on it. The screen stays open; the outcome
    /// goes to the status line under its own badge.
    fn trade(&mut self, cancel: bool) {
        let view = crate::trade::view(&self.session);
        let Some(slot) = traders::offer_at(&view, self.ui.trade_cursor).map(|o| o.slot) else {
            return;
        };
        let outcome = if cancel {
            crate::trade::cancel(&mut self.session, slot)
        } else {
            crate::trade::take(&mut self.session, slot)
        };
        self.ui.status = outcome.unwrap_or_else(|why| why);
        self.ui.status_badge = Some(" TRADE ");
    }

    /// Moves the research cursor `(dx, dy)` cards.
    fn move_tech(&mut self, dx: i32, dy: i32) {
        let view = screen_view(&self.session);
        self.ui.tech = research::step(&view, self.ui.tech, dx.signum(), dy);
    }

    /// How many cards PgUp and PgDn move in the research screen: a page of them.
    fn tech_page(&self) -> i32 {
        let view = screen_view(&self.session);
        let (width, rows) = screens::text_size(self.ui.page());
        research::Grid::new((width, rows), view.columns.len()).cards as i32
    }

    /// Moves the Nation screen's pointer (#308) `dy` settlers, stopping at the roster's edges.
    fn move_nation_cursor(&mut self, dy: i32) {
        let view = crate::view::nation_view(&self.session);
        self.ui.nation_cursor = nation::step(&view, self.ui.nation_cursor, dy);
    }

    /// How many settlers PgUp and PgDn move in the Nation screen: a page of rows (headings
    /// included, which undercounts slightly, close enough for a page jump).
    fn nation_page(&self) -> i32 {
        screens::text_size(self.ui.page()).1 as i32
    }

    /// `Enter` in the Nation screen: selects the settler under the pointer, puts the cursor on
    /// them and closes the screen (#308, docs/design/side-panel.md "The Nation screen").
    fn confirm_nation(&mut self) {
        let view = crate::view::nation_view(&self.session);
        if let Some(&id) = nation::roster(&view).get(self.ui.nation_cursor) {
            self.ui.select_unit(&self.session, id);
            self.ui.screen = None;
        }
    }

    /// Moves the abilities screen's pointer (#719) `dy` abilities, stopping at the list's edges.
    fn move_ability_cursor(&mut self, dy: i32) {
        let rows = crate::bend::abilities(&self.session);
        let at = self.ui.ability_cursor.min(rows.len().saturating_sub(1)) as i64;
        let to = (at + i64::from(dy)).clamp(0, rows.len() as i64 - 1).max(0);
        self.ui.ability_cursor = to as usize;
    }

    /// How many abilities PgUp and PgDn move: a page of the list (its header takes a row).
    fn ability_page(&self) -> i32 {
        screens::text_size(self.ui.page()).1.saturating_sub(1).max(1) as i32
    }

    /// `Enter` in the abilities screen (#719): the ability under the pointer becomes the Bend
    /// tool's choice, the tool becomes active and the screen closes. On one that can't be used
    /// yet it does nothing (the detail strip says why).
    fn pick_ability(&mut self) {
        let rows = crate::bend::abilities(&self.session);
        let Some(row) = rows.get(self.ui.ability_cursor) else { return };
        if !row.usable() {
            return;
        }
        self.session.bend_choice = Some(row.id);
        let (tool, choice) = switch_tool(&self.session, self.ui.tool, self.ui.choice, Tool::Bend);
        self.ui.choice = choice;
        self.ui.set_tool(tool);
        self.ui.screen = None;
    }

    /// How many rows of text the open screen shows at once.
    fn screen_rows(&self) -> i32 {
        self.ui.screen.map_or(0, |s| screens::body_rows(s, self.ui.page())) as i32
    }

    /// Scrolls the open screen `lines` down (negative: up), within its text.
    fn scroll_by(&mut self, lines: i32) {
        let Some(screen) = self.ui.screen.filter(|s| {
            !matches!(s, Screen::Research | Screen::Saves | Screen::Abilities | Screen::Traders)
        }) else {
            return;
        };
        let width = screens::text_size(self.ui.page()).0;
        let rows = screens::body_rows(screen, self.ui.page());
        let total = screens::row_count(&self.session, &self.ui, screen, width, self.legend_rows);
        let max = total.saturating_sub(rows);
        let now = self.ui.scroll.min(max) as i64;
        let to = (now + i64::from(lines)).clamp(0, max as i64) as usize;
        self.ui.scroll = to;
    }

    /// Opens the confirm box for `prompt` (`crate::ask`): the cursor on its first row, the safe
    /// answer.
    fn ask(&mut self, prompt: Prompt) {
        self.ui.ask_cursor = 0;
        self.ui.ask_can_save = self.saves.is_some();
        self.ui.prompt = Some(prompt);
    }

    /// What a key does while the confirm box is up (#655): arrows or `W`/`S` move the cursor,
    /// `Enter` takes the answer it is on, and `Esc` takes the safe one. Anything else, a bare `y`
    /// among it, does nothing.
    fn prompt_key(&mut self, prompt: Prompt, key: KeyEvent) {
        let Some(ask) = crate::ask::current(&self.session, &self.ui) else { return };
        let last = ask.answers.len() - 1;
        let at = self.ui.ask_cursor.min(last);
        match key.code {
            KeyCode::Esc => self.answer(prompt, Answer::Keep),
            KeyCode::Up | KeyCode::Char('w' | 'W') => {
                self.ui.ask_cursor = at.saturating_sub(1);
            }
            KeyCode::Down | KeyCode::Char('s' | 'S') => self.ui.ask_cursor = (at + 1).min(last),
            KeyCode::Enter => self.answer(prompt, ask.answers[at].0),
            _ => {}
        }
    }

    /// Takes `answer` to `prompt`. "Save, then" writes a new slot first and, if that fails,
    /// leaves the box open with the reason in the status line instead of going on.
    fn answer(&mut self, prompt: Prompt, answer: Answer) {
        match answer {
            Answer::Keep => {
                self.ui.prompt = None;
                self.decline(&prompt);
            }
            Answer::SaveThen => {
                if self.save_new_slot() {
                    self.ui.prompt = None;
                    self.confirm_prompt(prompt);
                }
            }
            Answer::Go => {
                self.ui.prompt = None;
                self.confirm_prompt(prompt);
            }
            Answer::Exit => {
                self.ui.prompt = None;
                self.close_menu();
                self.ui.quit = true;
            }
        }
    }

    /// The safe answer was taken. A question asked from the menu that ends the game as it is
    /// ("Keep playing") closes the menu too, so the game runs again; a load or a save over a slot
    /// was asked in the saves screen, which stays.
    fn decline(&mut self, prompt: &Prompt) {
        match prompt {
            Prompt::Quit | Prompt::NewMap | Prompt::Nation(_) | Prompt::QuickLoad => {
                self.close_menu();
            }
            Prompt::LoadSave(_) | Prompt::SaveOver(_) => {}
        }
    }

    /// "Save, then …": writes the game to a new slot ([`saves::new_name`]), so no earlier save is
    /// replaced. Says in the status line why not if it can't, and returns whether it did.
    fn save_new_slot(&mut self) -> bool {
        let Some(dir) = self.saves.clone() else {
            self.ui.status = NO_SAVE_DIR.to_owned();
            return false;
        };
        self.refresh_saves();
        let name = saves::new_name(&self.ui.save_slots);
        match saves::save(&dir, &name, &self.session) {
            Ok(()) => {
                self.ui.saved_at = Some(self.session.world.tick);
                true
            }
            Err(why) => {
                self.ui.status = format!("Save failed: {why}");
                false
            }
        }
    }

    /// The answer to a prompt was to go on.
    fn confirm_prompt(&mut self, prompt: Prompt) {
        match prompt {
            // The menu closes first, so the game is not left in a paused menu. `Shell` takes it
            // from here: it leaves the game behind for the title screen (#703).
            Prompt::Quit => {
                self.close_menu();
                self.ui.to_title = true;
            }
            // `Shell` reads this on the way out: it leaves the game behind for embark, rather
            // than restarting in place (#290).
            Prompt::NewMap => {
                self.close_menu();
                self.reopen_embark = true;
            }
            Prompt::Nation(nation) => {
                let seed = self.session.world.seed;
                self.restart(seed, nation);
            }
            // `close_menu` after both: harmless when the prompt came from `F9` or the saves
            // screen directly (there is no menu behind it then), and needed when it came from
            // the game menu (#312), since a successful quickload replaces `self.ui` outright
            // (`Self::load`) and a successful save as… has nothing left open behind it either.
            Prompt::QuickLoad => {
                self.quickload();
                self.close_menu();
            }
            Prompt::LoadSave(name) => self.load_save(&name),
            Prompt::SaveOver(name) => {
                self.save_over(&name);
                self.close_menu();
            }
        }
    }

    /// `F5` (or the game menu's `Save`, #306): writes the game to the quicksave slot and says
    /// in the status line whether it worked, with the path it went to — so a failed or missing
    /// save (the wrong platform data directory, say) can't pass silently.
    fn quicksave(&mut self) {
        self.ui.status = match self.saves.as_deref() {
            None => NO_SAVE_DIR.to_owned(),
            Some(dir) => match saves::quicksave(dir, &self.session) {
                Ok(()) => {
                    self.ui.saved_at = Some(self.session.world.tick);
                    let path = saves::slot_path(dir, saves::QUICKSAVE);
                    format!("Quicksaved to {} (tick {})", path.display(), self.session.world.tick)
                }
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
            Some(dir) => match saves::list(dir) {
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
            let prompt = Prompt::LoadSave(slot.name.clone());
            self.ask(prompt);
        }
    }

    /// `Shift+S` in the saves screen: asks to save over the highlighted slot.
    fn ask_save_over(&mut self) {
        if let Some(slot) = self.ui.save_slots.get(self.ui.save_cursor) {
            let prompt = Prompt::SaveOver(slot.name.clone());
            self.ask(prompt);
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

    /// `Shift+S` in the saves screen, once confirmed: writes the game over the named slot, with
    /// the path it went to in the status line (#306), as `quicksave` says.
    fn save_over(&mut self, name: &str) {
        self.ui.status = match self.saves.as_deref() {
            None => NO_SAVE_DIR.to_owned(),
            Some(dir) => match saves::save(dir, name, &self.session) {
                Ok(()) => {
                    self.ui.saved_at = Some(self.session.world.tick);
                    let path = saves::slot_path(dir, name);
                    format!("Saved to {} (tick {})", path.display(), self.session.world.tick)
                }
                Err(why) => format!("Save failed: {why}"),
            },
        };
        self.refresh_saves();
    }

    /// `N` in the saves screen: starts typing a name for a new slot (#704). Without a save
    /// directory it says so instead, as `F5` does.
    fn start_save_name(&mut self) {
        if self.saves.is_none() {
            self.ui.status = NO_SAVE_DIR.to_owned();
            return;
        }
        self.refresh_saves();
        self.ui.save_name = Some(String::new());
    }

    /// A key while a save name is being typed (#704): any printable character goes in (up to
    /// [`saves::MAX_NAME`]), `Backspace` edits, `Enter` saves and `Esc` goes back to the list
    /// with nothing written. Whether the name is allowed is `Enter`'s to say, so a typo shows up
    /// as a message instead of a key that silently did nothing.
    fn save_name_key(&mut self, key: KeyEvent) {
        let Some(name) = &mut self.ui.save_name else { return };
        match key.code {
            KeyCode::Esc => self.ui.save_name = None,
            KeyCode::Backspace => {
                name.pop();
            }
            KeyCode::Char(c)
                if !c.is_control()
                    && !key.modifiers.contains(KeyModifiers::CONTROL)
                    && name.chars().count() < saves::MAX_NAME =>
            {
                name.push(c);
            }
            KeyCode::Enter => self.submit_save_name(),
            _ => {}
        }
    }

    /// `Enter` on a typed name (#704). Nothing typed takes the screen's own suggestion
    /// ([`saves::new_name`]), so `N` then `Enter` always saves something. A name that can't be a
    /// slot ([`saves::check_name`]) is refused with the typing left as it was. A name already in
    /// the list asks first, as Shift+`S` does (`Prompt::SaveOver`). Any other name is written at
    /// once, with no prompt since nothing is overwritten.
    fn submit_save_name(&mut self) {
        let Some(typed) = self.ui.save_name.clone() else { return };
        let name = if typed.is_empty() { saves::new_name(&self.ui.save_slots) } else { typed };
        if saves::check_name(&name).is_err() {
            self.ui.status = format!(
                "Can't name a save {name:?}: use 1 to {} letters, digits, - and _",
                saves::MAX_NAME
            );
            return;
        }
        self.ui.save_name = None;
        // File systems that ignore case would make `Save-1` replace `save-1` unasked.
        if let Some(taken) = self.ui.save_slots.iter().find(|s| s.name.eq_ignore_ascii_case(&name))
        {
            let prompt = Prompt::SaveOver(taken.name.clone());
            self.ask(prompt);
        } else {
            self.save_over(&name);
        }
    }

    /// Makes `world` the game. Only the world is in a save: the cursor, the selection, the tool,
    /// the screens and the log start over, and the clock (speed and pause) is the player's and
    /// stays.
    pub fn load(&mut self, world: World) {
        self.session.world = world;
        self.session.bend_choice = None;
        self.ui = UiState::new(&self.session, self.screen);
        // What was loaded is what was saved: nothing has happened since.
        self.ui.saved_at = Some(self.session.world.tick);
        // The loaded game's past events are not news: they age out at once instead of showing
        // as toasts.
        self.ui.alerts.update(&self.session, f32::MAX);
        self.dirty = true;
    }

    /// A new game on a map of the same size: the clock, the selection, the log and the screens
    /// all start over. If this world was founded at a chosen site (`World::start`, #289), the
    /// new one is founded there too, instead of snapping back to the map centre (#290, the `1`-
    /// `4` nation switch); `Session::new`'s plain centre spiral otherwise. Either way, the
    /// current world's difficulty preset carries over (#298): this is a new game, not a change
    /// of preset mid-game.
    pub fn restart(&mut self, seed: u64, nation: Nation) {
        let (width, height) = (self.session.world.map.width(), self.session.world.map.height());
        let difficulty = self.session.world.difficulty().map(str::to_owned);
        self.session = match self.session.world.start() {
            Some(start) => {
                // The preset matching this map's size, so a bigger map's own
                // `continent_frequency` override survives the switch (docs/design/embark.md,
                // "Map size is a choice"); the plain defaults otherwise.
                let cfg = match self
                    .session
                    .defs
                    .embark
                    .map_sizes
                    .iter()
                    .find(|s| s.width == width && s.height == height)
                {
                    Some(size) => fn_sim::embark::Survey::config(seed, size),
                    None => fn_sim::MapGenConfig { seed, width, height, ..Default::default() },
                };
                let survey = fn_sim::embark::Survey::new(&cfg, &self.session.defs);
                Session::found(survey, nation, start, difficulty.clone()).unwrap_or_else(|_| {
                    let mut session = Session::new(seed, nation, width, height);
                    session.world.set_difficulty(difficulty.clone());
                    session
                })
            }
            None => {
                let mut session = Session::new(seed, nation, width, height);
                session.world.set_difficulty(difficulty);
                session
            }
        };
        self.session.clock = self.session.clock.clone().with_fast(self.fast);
        self.menu_prev_paused = None;
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
    /// choice if it can still be placed, else the first one that can (`crate::build::valid_choice`,
    /// as `switch_tool` used to resolve it for the plain tool key).
    fn open_build_modal(&mut self) {
        let rows = crate::build::picker_rows(&self.session);
        let choice = crate::build::valid_choice(&rows, self.ui.choice);
        self.ui.build_cursor =
            choice.and_then(|id| rows.iter().position(|r| r.id == id)).unwrap_or(0);
        self.ui.modal = Some(Modal::Build);
    }

    /// What a key does while a modal is open: it captures every key until the modal resolves
    /// itself, so no tool key, screen key or clock action reaches [`Self::act`].
    fn modal_key(&mut self, modal: Modal, key: KeyEvent) {
        match modal {
            Modal::Build => self.build_modal_key(key),
            Modal::Menu => self.menu_key(key),
        }
    }

    /// The Build screen's own keys (#285, `crate::build`): arrows or `W`/`S` move the highlight
    /// by one row (locked included); `Enter` on an unlocked building picks it, closes the screen
    /// and leaves the Build tool active with it; `Esc` or `U` leaves the tool and the choice as
    /// they were; any other letter jumps the highlight to the next name that starts with it.
    fn build_modal_key(&mut self, key: KeyEvent) {
        if build_screen::closes(key) {
            self.ui.modal = None;
            return;
        }
        let rows = crate::build::picker_rows(&self.session);
        if rows.is_empty() {
            return;
        }
        let at = self.ui.build_cursor.min(rows.len() - 1);
        match key.code {
            KeyCode::Up | KeyCode::Char('w' | 'W') => {
                self.ui.build_cursor = build_screen::step(&rows, at, -1);
            }
            KeyCode::Down | KeyCode::Char('s' | 'S') => {
                self.ui.build_cursor = build_screen::step(&rows, at, 1);
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
                self.ui.build_cursor = build_screen::jump_to_letter(&rows, at, c);
            }
            _ => {}
        }
    }

    /// `Enter` in the Endeavor screen (#643): once the endeavor is Ready, closes the screen and
    /// arms the Gate tool, whose ghost shows where the site would go; otherwise says why not in
    /// the status line ("Can't place The Harmony Gate: 5 of 9 signs met").
    fn start_gate(&mut self) {
        match crate::gate::place_refusal(&self.session) {
            None => {
                self.ui.screen = None;
                self.ui.set_tool(Tool::Gate);
            }
            Some(why) => self.gate_status(why),
        }
    }

    /// `R` in Select mode (#712): sets the building under the cursor to its next recipe
    /// (`crate::recipe::switch`), the outcome in the Tool section's notice.
    fn next_recipe(&mut self) {
        self.ui.notice =
            crate::recipe::switch(&mut self.session, self.ui.cursor).unwrap_or_else(|why| why);
    }

    /// `X` in the Research screen (#712): clears the research target so the Archive's scholars
    /// scribe (`crate::research::clear_target`). The screen stays open; the outcome goes to the
    /// status line under its own badge.
    fn clear_research(&mut self) {
        self.ui.status = crate::research::clear_target(&mut self.session).unwrap_or_else(|why| why);
        self.ui.status_badge = Some(" RESEARCH ");
    }

    /// Shows a gate outcome in the status line, under its own badge (#643).
    fn gate_status(&mut self, text: String) {
        self.ui.status = text;
        self.ui.status_badge = Some(" GATE ");
    }

    /// `X` in the Endeavor screen (#643): withdraws the gate's site while no work on it has
    /// started (`crate::gate::withdraw`). The screen stays open; the outcome is both the status
    /// line and the Tool section's notice.
    fn withdraw_gate(&mut self) {
        let text = crate::gate::withdraw(&mut self.session).unwrap_or_else(|why| why);
        self.gate_status(text.clone());
        self.ui.notice = text;
    }

    /// `Enter` with the Gate tool (#643): places the site centred on the cursor. Placed, the
    /// tool goes back to Select (there is only one gate); refused, it stays for another try.
    /// Either way the notice says what happened.
    fn place_gate(&mut self) {
        match crate::gate::place(&mut self.session, self.ui.cursor) {
            Ok(text) => {
                self.ui.set_tool(Tool::Select);
                self.gate_status(text.clone());
                self.ui.notice = text;
            }
            Err(why) => self.ui.notice = why,
        }
    }

    /// `Enter`. In Select mode it selects the unit under the cursor (again: the next one on that
    /// tile). In a tool the first sets a corner, the second applies the tool to every tile of the
    /// rectangle through `fn_ui`'s shared logic, and the tool stays active for the next one. The
    /// Gate tool has no rectangle: one `Enter` places it.
    fn confirm(&mut self) {
        if let Some(head) = self.ui.linking {
            return self.set_foot(head);
        }
        if self.ui.tool == Tool::Select {
            self.ui.select_here(&self.session);
            return;
        }
        if self.ui.tool == Tool::Gate {
            return self.place_gate();
        }
        match self.ui.anchor.take() {
            None => self.ui.anchor = Some(self.ui.cursor),
            Some(corner) => {
                // The shared logic also takes the Shift state (a repeating harvest): `O`
                // (`Action::ToggleRepeat`) stands in for it here, since a terminal can't
                // reliably report Shift+Enter (#277). The same key, read as the Cancel tool's
                // own teardown toggle instead (#335).
                let report = apply_order(
                    &mut self.session,
                    Order {
                        tool: self.ui.tool,
                        from: corner,
                        to: self.ui.cursor,
                        repeating: self.ui.repeating,
                        teardown: self.ui.teardown,
                        choice: self.ui.choice,
                    },
                );
                self.ui.notice = report.notice.clone();
                self.ui.marked = Some((self.ui.cursor, report.summary()));
                // A chute site just placed asks for its foot next (#683), a pair's end for its
                // partner (#698).
                if self.ui.tool == Tool::Build
                    && let Some(head) = crate::link::unlinked_in(
                        &self.session,
                        corner,
                        self.ui.cursor,
                        self.ui.choice,
                    )
                {
                    self.ui.linking = Some(head);
                    let ask = crate::link::asking(&self.session, head);
                    self.ui.notice = format!("{}. {ask}", report.notice);
                }
            }
        }
    }

    /// While a chute's foot or a pair's partner is being picked (#683, #698), the notice follows
    /// the cursor: why it can't go here, or the prompt again where it can.
    fn foot_hint(&mut self) {
        if let Some(head) = self.ui.linking {
            self.ui.notice = crate::link::refusal(&self.session, head, self.ui.cursor)
                .unwrap_or_else(|| crate::link::asking(&self.session, head).to_owned());
        }
    }

    /// `Enter` while a foot or partner is being picked (#683, #698): sets it at the cursor (a
    /// partner places a new site there unless an end is under the cursor) and stops picking, or
    /// says why not and keeps picking.
    fn set_foot(&mut self, head: Pos) {
        match crate::link::set(&mut self.session, head, self.ui.cursor) {
            Ok(text) => {
                self.ui.linking = None;
                self.ui.notice = text;
            }
            Err(why) => self.ui.notice = why,
        }
    }

    /// `Esc` in a tool: drop the corner, or with none leave the tool. In Select mode it closes
    /// the inspector's settler; with nothing left to close (#306), it opens the game menu instead.
    fn cancel(&mut self) {
        if let Some(head) = self.ui.linking.take() {
            self.ui.notice = crate::link::left_unset(&self.session, head).to_owned();
            return;
        }
        if self.ui.tool == Tool::Select {
            if self.ui.selected.is_some() {
                self.ui.select(None);
            } else {
                self.open_menu();
            }
        } else if self.ui.anchor.take().is_none() {
            self.ui.set_tool(Tool::Select);
        }
    }

    /// `Esc` in Select mode with nothing selected (#306): no tool, no bar, no screen and no
    /// modal were left to close, so `Esc` opens the game menu (Resume, Save…, Load…, Key help,
    /// New map…, Switch nation…, Quit…, #655) instead. Pauses the clock while it's open;
    /// [`Self::close_menu`] puts it back.
    fn open_menu(&mut self) {
        self.menu_prev_paused = Some(self.session.clock.paused);
        self.session.clock.paused = true;
        self.ui.modal = Some(Modal::Menu);
        self.ui.menu_cursor = 0;
        self.ui.menu_page = MenuPage::Rows;
    }

    /// Closes the game menu (`Resume`, or `Esc` inside it), restoring the clock's pause from
    /// just before it opened. Harmless to call when the menu isn't open (nothing to restore):
    /// a quickload through the confirm box replaces `self.ui` outright (`Self::load`), so every
    /// answer to a question asked from the menu comes through here to give the pause back.
    fn close_menu(&mut self) {
        if let Some(paused) = self.menu_prev_paused.take() {
            self.session.clock.paused = paused;
        }
        self.ui.modal = None;
    }

    /// What a key does while the game menu is open (#306): arrows or `W`/`S` move the row
    /// highlight, `Enter` acts on it, and `Esc` resumes, as choosing `Resume` itself does; on
    /// Switch nation…'s list it goes back to the rows instead.
    fn menu_key(&mut self, key: KeyEvent) {
        let page = self.ui.menu_page;
        match key.code {
            KeyCode::Esc if page == MenuPage::Nations => self.menu_back(),
            KeyCode::Esc => self.close_menu(),
            KeyCode::Up | KeyCode::Char('w' | 'W') => {
                self.ui.menu_cursor = crate::menu::step(self.ui.menu_cursor, -1, page.rows());
            }
            KeyCode::Down | KeyCode::Char('s' | 'S') => {
                self.ui.menu_cursor = crate::menu::step(self.ui.menu_cursor, 1, page.rows());
            }
            KeyCode::Enter if page == MenuPage::Nations => {
                let nation = Nation::ALL[self.ui.menu_cursor.min(Nation::ALL.len() - 1)];
                self.ask(Prompt::Nation(nation));
            }
            KeyCode::Enter => self.menu_choose(),
            _ => {}
        }
    }

    /// Back from Switch nation…'s list to the rows, the highlight on that row again.
    fn menu_back(&mut self) {
        self.ui.menu_page = MenuPage::Rows;
        self.ui.menu_cursor =
            MenuRow::ALL.iter().position(|r| *r == MenuRow::SwitchNation).unwrap_or(0);
    }

    /// `Enter` in the game menu: what the highlighted row does.
    fn menu_choose(&mut self) {
        match MenuRow::ALL[self.ui.menu_cursor] {
            MenuRow::Resume => self.close_menu(),
            // Both open the saves screen: `N` there writes a new slot, Shift+`S` saves over one
            // and `Enter` loads one (each asking first where it can lose something).
            MenuRow::Save | MenuRow::Load => {
                self.close_menu();
                self.show_saves();
            }
            MenuRow::Help => {
                self.close_menu();
                self.open(Screen::Help);
            }
            // The menu stays open behind the box (`draw` hides it while one is up), so the
            // "Keep playing" answer closes it (`Self::decline`) and `Esc` there is the same.
            MenuRow::NewMap => self.ask(Prompt::NewMap),
            MenuRow::SwitchNation => {
                self.ui.menu_page = MenuPage::Nations;
                self.ui.menu_cursor = 0;
            }
            MenuRow::Quit => self.ask(Prompt::Quit),
        }
    }

    /// After ticks ran: turns what the world logged into alerts (the toasts age by `dt` real
    /// seconds), deselects a unit that died and moves the cursor with a followed one.
    ///
    /// `pub` so `fn_tui::surface`'s scenario prelude (#481) can catch `ui.alerts` up to
    /// the end of a prelude it ran directly against `self.session`, without `run_ticks` -- the
    /// one call every other path here already makes through it.
    pub fn settle(&mut self, dt: f32) {
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
            if self.ui.quit || self.ui.to_title {
                return;
            }
        }
    }

    /// Runs `n` ticks whatever the clock says, as a key script's `<tick N>` does, and settles
    /// what they logged. Public so `fn_tui::shell::Shell` can run the same ticks from a script
    /// that starts at the title screen (#280).
    pub fn run_ticks(&mut self, n: u32) {
        self.session.tick_n(n);
        self.settle(0.0);
    }
}
