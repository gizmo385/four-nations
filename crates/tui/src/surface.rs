//! The agent surface (#313, #315): `Host` wraps a [`Shell`] built the way `--start` builds one,
//! and gives an agent exactly what a human player has -- reads built from `fn_ui`'s view models,
//! the map's per-cell look (#314) and the renderer, and keys fed to the same `Shell::key` a
//! keyboard feeds. It does no I/O and reads no clock (docs/design/agent-play.md, "Architecture").
//!
//! **Time is lockstep** ([`Host::end_turn`]): the sim stands still between calls, and a turn is
//! a fixed slice of game time (`Host::turn_seconds`, 20 by default), so every call gets the same
//! amount of the world moving. A turn ends early the moment a new alert lands or a need flag
//! first shows, which stands in for a human's continuous attention without telling the agent
//! anything the alert log and the side panel don't already say.
//!
//! **Keys, not commands** ([`Host::press_keys`], [`Host::act`]): every action goes through
//! `Shell::key`, the same path a human's keyboard feeds, so an agent can't do anything a key
//! can't. A short, fixed list of keys is refused outright -- the ones that would run ticks on
//! their own, save, load, quit or leave the game for a new map or nation -- because the host
//! owns time and the session, not the agent (docs/design/agent-play.md, "Action"). Named actions
//! (`designate`, `build`, `select`, `research`, `cursor`, `keys`, #316) spare an agent counting
//! cursor steps and tracking modal state: [`Host::act`] plans each [`ActionStep`] from `UiState`
//! (the cursor, the open screen or modal, and the active tool) and presses exactly the keys a
//! player would, closing any open screen or modal first where the step needs the plain map, and
//! leaving the active tool's own key to switch tools (it already does that unconditionally). A
//! step whose plan can't be made (an unknown tech, a building the Build modal doesn't offer, a
//! tile off the map, ...) presses nothing and the steps after it don't run either.
//!
//! **A run log** ([`RunLog`]) remembers every key actually pressed and every turn run, in order,
//! so it can be replayed with `Shell::play` (a seed plus keys is the whole game, as a key script
//! always is) and exported as a `--keys` script (`RunLog::to_keys_script`).

use std::collections::BTreeMap;

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use fn_sim::world::TICKS_PER_SECOND;
use fn_sim::{Nation, Pos};
use fn_ui::inspect::Line;
use fn_ui::view::{activity_counts, nation_view, need_flags, side_view};
use fn_ui::{Session, Tool};
use serde::{Deserialize, Serialize};

use crate::app::App;
use crate::cli::{self, Args};
use crate::color::{ColorDepth, Palette};
use crate::frame::{render_shell, to_text};
use crate::input::{self, Action};
use crate::keys::{self, Step};
use crate::map::{Ground, Mark, Overlays, Thing, cell_look};
use crate::menu::MenuRow;
use crate::panel::hint_text;
use crate::shell::{Mode, Shell};
use crate::state::Modal;

/// At most this wide or tall a [`Host::look`] may ask for (#315's acceptance criteria): big
/// enough to read a whole screen's worth of map, small enough that a call stays cheap.
pub const MAX_LOOK: (u16, u16) = (64, 40);

/// How many game seconds one [`Host::end_turn`] turn covers, until races (#318) make it data
/// per preset.
pub const DEFAULT_TURN_SECONDS: u32 = 20;

// ------------------------------------------------------------------------------------------
// Host
// ------------------------------------------------------------------------------------------

/// Wraps a [`Shell`] for an agent (#313, #315): built the way `--start` builds one (seed,
/// nation, map size, difficulty and an optional site), with no save directory. Does no I/O and
/// reads no clock; every tick comes from [`Host::end_turn`], never the wall clock.
pub struct Host {
    shell: Shell,
    /// Built once for `--ascii` (so [`Host::frame`] is plain text) and truecolor (so a blocked
    /// Build ghost still draws as the ghost, the richer of the two a player might see):
    /// `cell_look`'s one colour-depth-dependent choice (`map`'s doc comment).
    palette: Palette,
    turn_seconds: u32,
    /// How many `end_turn` turns have completed.
    turn: u32,
    /// How many lines of `ui.alerts.history()` already belong to a turn `observe` has reported:
    /// an `observe` between turns, or right after one, reports exactly what's new since then.
    alerts_since_turn: usize,
    log: RunLog,
}

impl Host {
    /// A new agent session: `seed`, `nation`, `map_size` and `rough` as `--size`/`--rough` would
    /// (only read with `site`), `difficulty` as `--difficulty`, and `site` as `--start X,Y`
    /// (`None` for the plain `SEED NATION` path, centred on the map). Never opens embark or the
    /// title screen: a bad `site` or `difficulty` is an error instead.
    pub fn new(
        seed: u64,
        nation: Nation,
        map_size: Option<String>,
        difficulty: Option<String>,
        site: Option<(i32, i32)>,
        rough: bool,
    ) -> Result<Self, String> {
        let args = Args {
            seed: Some(seed),
            nation: Some(nation),
            frame: false,
            size: cli::DEFAULT_SIZE,
            ticks: 0,
            keys: Vec::new(),
            ascii: false,
            ansi: false,
            color: None,
            embark: false,
            start: site,
            map_size,
            rough,
            difficulty,
        };
        let shell = Shell::for_args(&args, cli::DEFAULT_SIZE, None, 0)?;
        let palette = Palette::new(shell.defs(), nation, ColorDepth::TrueColor, true);
        Ok(Self {
            shell,
            palette,
            turn_seconds: DEFAULT_TURN_SECONDS,
            turn: 0,
            alerts_since_turn: 0,
            log: RunLog::new(seed, nation),
        })
    }

    /// Sets how many game seconds one `end_turn` turn covers (20 by default); at least one.
    pub fn with_turn_seconds(mut self, turn_seconds: u32) -> Self {
        self.turn_seconds = turn_seconds.max(1);
        self
    }

    /// The run log so far: every key pressed and every turn run, in order.
    pub fn log(&self) -> &RunLog {
        &self.log
    }

    /// `Host::new` never leaves embark or the title screen open (a bad site or difficulty is an
    /// error instead), so this always holds.
    fn app(&self) -> &App {
        match &self.shell.mode {
            Mode::Game(app) => app,
            _ => unreachable!("Host always wraps a game; embark is out of scope (#315)"),
        }
    }

    fn app_mut(&mut self) -> &mut App {
        match &mut self.shell.mode {
            Mode::Game(app) => app,
            _ => unreachable!("Host always wraps a game; embark is out of scope (#315)"),
        }
    }

    // --------------------------------------------------------------------------------------
    // Keys
    // --------------------------------------------------------------------------------------

    /// Plays `script` (`--keys` syntax, without `<tick N>`) through `Shell::key`, skipping any
    /// key [`Self::refuses`] refuses; `label` and `note` are kept with the pressed keys in the
    /// run log. Errors if `script` doesn't parse or asks for a tick.
    pub fn press_keys(
        &mut self,
        script: &str,
        label: Option<String>,
        note: Option<String>,
    ) -> Result<PressResult, String> {
        let steps = keys::parse(script)?;
        if steps.iter().any(|s| matches!(s, Step::Tick(_))) {
            return Err(
                "<tick N> isn't allowed in press_keys; call end_turn instead (the host owns \
                 time)"
                    .into(),
            );
        }
        let mut pressed = Vec::new();
        let mut refused = 0usize;
        for step in steps {
            let Step::Key(key) = step else { unreachable!("ticks are refused above") };
            if self.refuses(key) {
                refused += 1;
                continue;
            }
            self.shell.key(key);
            pressed.push(key);
        }
        let result = PressResult { pressed: pressed.len(), refused };
        self.log.push(LogEntry::Keys { keys: pressed, label, note });
        Ok(result)
    }

    /// Whether `key` is refused outright: `Space`, `+`, `-`, `.` (the host owns the clock), `R`
    /// (a new map), `1`-`4` (switch nation), `q` and Ctrl+C (quit), `F5`, `F9` (save and load at
    /// once) and `F6` (the saves screen) -- the same bindings `input::bindings` already names,
    /// so a future rebinding can't quietly change what parity allows -- plus `Enter` on the Esc
    /// menu's Save as…, Quicksave, Load…, Quickload or Quit row, which do the same things
    /// without a function key (#306).
    pub fn refuses(&self, key: KeyEvent) -> bool {
        if let Some(action) = input::map_key(key)
            && matches!(
                action,
                Action::TogglePause
                    | Action::Faster
                    | Action::Slower
                    | Action::Step
                    | Action::AskNewMap
                    | Action::AskNation(_)
                    | Action::QuickSave
                    | Action::AskQuickLoad
                    | Action::ShowSaves
                    | Action::AskQuit
                    | Action::QuitNow
            )
        {
            return true;
        }
        if key.code == KeyCode::Enter {
            let ui = &self.app().ui;
            if ui.modal == Some(Modal::Menu) && ui.menu_name.is_none() {
                let row = MenuRow::ALL[ui.menu_cursor];
                if matches!(
                    row,
                    MenuRow::SaveAs
                        | MenuRow::Quicksave
                        | MenuRow::Load
                        | MenuRow::Quickload
                        | MenuRow::Quit
                ) {
                    return true;
                }
            }
        }
        false
    }

    // --------------------------------------------------------------------------------------
    // Time
    // --------------------------------------------------------------------------------------

    /// Runs `turns` turns of `turn_seconds` game seconds each, through the same path a
    /// `<tick N>` of a key script takes (`App::run_ticks`). Stops early at the end of the first
    /// turn in which a new alert lands or a need flag shows for the first time, so a long
    /// `turns` never runs past the next thing worth an agent's attention. Returns the
    /// observation after whichever turn it stopped on.
    pub fn end_turn(&mut self, turns: u32) -> Observation {
        for _ in 0..turns {
            let before_alerts = self.app().ui.alerts.history().len();
            let before_needs: Vec<String> =
                need_flags(&self.app().session).into_iter().map(|(w, _)| w).collect();
            let ticks = self.turn_seconds * TICKS_PER_SECOND;
            self.app_mut().run_ticks(ticks);
            self.turn += 1;
            let tick = self.app().session.world.tick;
            self.log.push(LogEntry::Turn { index: self.turn, tick });
            let after_alerts = self.app().ui.alerts.history().len();
            let new_need_flag = need_flags(&self.app().session)
                .into_iter()
                .any(|(w, _)| !before_needs.contains(&w));
            self.alerts_since_turn = after_alerts;
            if after_alerts > before_alerts || new_need_flag {
                break;
            }
        }
        self.observe()
    }

    // --------------------------------------------------------------------------------------
    // Reads
    // --------------------------------------------------------------------------------------

    /// Turn, clock, season, tool, notice, people (activity counts and need flags), research,
    /// stores, endeavor, the cursor's own tile and alerts since the last turn: everything
    /// `side_view` and the alert log already say, free, changing nothing and taking no game
    /// time.
    pub fn observe(&self) -> Observation {
        let app = self.app();
        let (session, ui) = (&app.session, &app.ui);
        let side = side_view(session, ui.tool, &ui.notice, ui.choice, &ui.alerts);
        let activity =
            activity_counts(session).into_iter().map(|(a, n)| (a.label().to_owned(), n)).collect();
        let needs = need_flags(session).into_iter().collect();
        let season = session.world.knobs(&session.defs).seasons.then(|| {
            let defs = &session.defs;
            session.world.season(defs).name.clone()
        });
        let history = ui.alerts.history();
        let start = self.alerts_since_turn.min(history.len());
        let alerts = history[start..].iter().map(|e| e.text.clone()).collect();
        Observation {
            turn: self.turn,
            clock: side.clock,
            season,
            tool: ui.tool.name().to_owned(),
            notice: side.notice,
            people: PeopleView { total: side.people, activity, needs },
            research: ResearchView {
                label: side.research.label,
                pool: side.research.pool,
                rate: side.research.rate,
            },
            stores: side.stores.into_iter().map(StoreView::from).collect(),
            endeavor: side.endeavor.map(|e| EndeavorView {
                name: e.name,
                count: e.count,
                line: e.line,
                next: e.next,
            }),
            cursor: self.cursor_view(ui.cursor),
            alerts,
        }
    }

    /// That tile's Cursor-section lines (`fn_ui::inspect::tooltip_lines`), as if the cursor sat
    /// there, without moving it: logged, for a watcher's attention ghosts (#319).
    pub fn inspect(&mut self, x: i32, y: i32) -> CursorView {
        let view = self.cursor_view(Pos::new(x, y));
        self.log.push(LogEntry::Inspect { x, y });
        view
    }

    fn cursor_view(&self, pos: Pos) -> CursorView {
        let app = self.app();
        let lines = fn_ui::inspect::tooltip_lines(&app.session, app.ui.tool, pos)
            .unwrap_or_default()
            .into_iter()
            .filter_map(line_text)
            .collect();
        CursorView { x: pos.x, y: pos.y, lines }
    }

    /// A map region, at most [`MAX_LOOK`]: `things`, `ground` and `marks`, through the same
    /// `cell_look` the painter uses (#314), so this can't show anything the screen hides. The
    /// legend is local to this response: one character per distinct thing or ground actually
    /// present, so there are no collisions (docs/design/agent-play.md, "The map"). Logged, for a
    /// watcher's attention ghosts (#319).
    pub fn look(&mut self, x: i32, y: i32, w: u16, h: u16) -> MapView {
        let (w, h) = (w.clamp(1, MAX_LOOK.0), h.clamp(1, MAX_LOOK.1));
        let app = self.app();
        let (session, ui) = (&app.session, &app.ui);
        let over = Overlays::gather(session, ui);
        let mut pool = CharPool::default();
        let mut things = Vec::with_capacity(h as usize);
        let mut ground = Vec::with_capacity(h as usize);
        let mut marks = Vec::new();
        for row in 0..h {
            let mut thing_line = String::with_capacity(w as usize);
            let mut ground_line = String::with_capacity(w as usize);
            for col in 0..w {
                let pos = Pos::new(x + i32::from(col), y + i32::from(row));
                match cell_look(session, ui, &self.palette, &over, pos) {
                    Some(cell) => {
                        thing_line.push(pool.char_for(&describe_thing(session, &cell.thing, pos)));
                        ground_line.push(pool.char_for(&describe_ground(session, cell.ground)));
                        if !cell.marks.is_empty() {
                            let kinds = cell.marks.iter().map(describe_mark).collect();
                            marks.push(MarkView { x: pos.x, y: pos.y, kinds });
                        }
                    }
                    None => {
                        thing_line.push(' ');
                        ground_line.push(' ');
                    }
                }
            }
            things.push(thing_line);
            ground.push(ground_line);
        }
        self.log.push(LogEntry::Look { x, y, w, h });
        MapView { x, y, w, h, things, ground, legend: pool.legend, marks }
    }

    /// `nation`, `research`, `endeavor`, `log` or `help`, as data: `nation_view`,
    /// `research::screen_view`, `endeavor::screen_view` or `input::help_rows`, reachable in the
    /// TUI the same way (`N`, `T`, `G`, `L`, `?`).
    pub fn screen(&self, name: &str) -> Result<ScreenView, String> {
        match name {
            "nation" => Ok(ScreenView::Nation(self.nation_screen())),
            "research" => Ok(ScreenView::Research(self.research_screen())),
            "endeavor" => Ok(ScreenView::Endeavor(self.endeavor_screen())),
            "log" => Ok(ScreenView::Log(LogScreenView {
                lines: self.app().ui.alerts.history().iter().map(|e| e.text.clone()).collect(),
            })),
            "help" => Ok(ScreenView::Help(HelpScreenView {
                rows: input::help_rows()
                    .into_iter()
                    .map(|r| HelpRowView {
                        group: r.group.title().to_owned(),
                        keys: r.keys,
                        what: r.what,
                    })
                    .collect(),
            })),
            other => {
                Err(format!("no such screen {other:?} (nation, research, endeavor, log or help)"))
            }
        }
    }

    fn nation_screen(&self) -> NationScreenView {
        let v = nation_view(&self.app().session);
        NationScreenView {
            nation: v.nation,
            population: v.population,
            clock: v.clock,
            groups: v
                .groups
                .into_iter()
                .map(|(a, units)| {
                    (a.label().to_owned(), units.into_iter().map(|u| u.name).collect())
                })
                .collect(),
            stores: v.stores.into_iter().map(StoreView::from).collect(),
            traits: v.traits,
        }
    }

    fn research_screen(&self) -> ResearchScreenView {
        let v = fn_ui::research::screen_view(&self.app().session);
        ResearchScreenView {
            columns: v
                .columns
                .into_iter()
                .map(|col| {
                    col.into_iter()
                        .map(|c| TechCardView {
                            name: c.name,
                            cost: c.cost,
                            state: c.state.label().to_owned(),
                            unlock: c.unlock,
                        })
                        .collect()
                })
                .collect(),
        }
    }

    fn endeavor_screen(&self) -> EndeavorScreenView {
        let v = fn_ui::endeavor::screen_view(&self.app().session);
        EndeavorScreenView {
            name: v.name,
            count: v.count,
            stage: v.stage,
            groups: v
                .groups
                .into_iter()
                .map(|g| EndeavorGroupView {
                    heading: g.heading,
                    signs: g
                        .signs
                        .into_iter()
                        .map(|s| SignRowView {
                            name: s.name,
                            met: s.met,
                            hint: s.hint,
                            value: s.value,
                        })
                        .collect(),
                })
                .collect(),
        }
    }

    /// The rendered frame as ASCII text (`frame::render_shell`, `--ascii`): the parity escape
    /// hatch (docs/design/agent-play.md, "Frames and keys"). Resizes the view, as `--frame`
    /// does; changes nothing else and takes no game time.
    pub fn frame(&mut self, w: u16, h: u16) -> String {
        let size = (w.max(1), h.max(1));
        let buf = render_shell(&mut self.shell, &self.palette, size);
        to_text(&buf)
    }

    // --------------------------------------------------------------------------------------
    // Action (#316)
    // --------------------------------------------------------------------------------------

    /// Runs `steps` in order, each planned from `UiState` and pressed through `Shell::key`
    /// (docs/design/agent-play.md, "Action"). A step whose plan can't be made (an unknown tech,
    /// a building the Build modal doesn't offer, a tile off the map, ...) presses nothing; its
    /// reason is returned and the steps after it don't run, but the steps before it already
    /// happened. `note` is the agent's one-line reason, kept with the keys in the run log
    /// (truncated to 80 characters), and never affects the game.
    pub fn act(&mut self, steps: &[ActionStep], note: Option<String>) -> ActResult {
        let mut pressed = Vec::new();
        let mut labels = Vec::new();
        let mut error = None;
        for (index, step) in steps.iter().enumerate() {
            match self.plan_step(step) {
                Ok(keys) => {
                    for key in keys {
                        if self.refuses(key) {
                            continue;
                        }
                        self.shell.key(key);
                        pressed.push(key);
                    }
                    labels.push(step_label(step));
                }
                Err(reason) => {
                    error = Some(ActError { step: index, reason });
                    break;
                }
            }
        }
        let label = (!labels.is_empty()).then(|| labels.join("; "));
        let note = note.map(|n| n.chars().take(80).collect::<String>());
        self.log.push(LogEntry::Keys { keys: pressed.clone(), label, note });
        ActResult {
            keys: pressed.into_iter().map(key_token).collect(),
            error,
            observe: self.observe(),
        }
    }

    /// The keys one step presses, or why it can't be planned at all: read-only (nothing is
    /// pressed here, so a failed step truly presses nothing). Validates first (an off-map tile,
    /// an unknown tech, a building the Build modal doesn't offer, an unknown settler, a `keys`
    /// step that doesn't parse or asks for a tick), then plans the keys from the current
    /// `UiState`.
    fn plan_step(&self, step: &ActionStep) -> Result<Vec<KeyEvent>, String> {
        match step {
            ActionStep::Designate { tool, from, to, standing } => {
                self.plan_designate(*tool, from.pos(), to.pos(), *standing)
            }
            ActionStep::Build { building, from, to } => {
                self.plan_build(building, from.pos(), to.pos())
            }
            ActionStep::Select { x: Some(x), y: Some(y), name: None } => {
                self.plan_select_at(Pos::new(*x, *y))
            }
            ActionStep::Select { x: None, y: None, name: Some(name) } => {
                self.plan_select_named(name)
            }
            ActionStep::Select { .. } => {
                Err("select needs either x and y, or name, but not both or neither".into())
            }
            ActionStep::Research { tech } => self.plan_research(tech),
            ActionStep::Cursor { x, y } => self.plan_cursor(Pos::new(*x, *y)),
            ActionStep::Keys { script } => self.plan_keys(script),
        }
    }

    /// `designate`: Esc to close any open screen or modal (the tool's letter and `o` wouldn't
    /// reach the game otherwise), the tool's letter, `o` for a standing order, a cursor walk to
    /// `from` (Shift for runs of 10), Enter, a walk to `to`, Enter.
    fn plan_designate(
        &self,
        tool: DesignateTool,
        from: Pos,
        to: Pos,
        standing: bool,
    ) -> Result<Vec<KeyEvent>, String> {
        if !self.in_bounds(from) || !self.in_bounds(to) {
            return Err(format!(
                "designate: ({},{}) to ({},{}) isn't on the map",
                from.x, from.y, to.x, to.y
            ));
        }
        if standing && tool != DesignateTool::Harvest {
            return Err("standing orders are only for the harvest tool".into());
        }
        let letter =
            tool.tool().letter().expect("every designate tool has a letter").to_ascii_lowercase();
        let mut keys = self.close_overlay_keys();
        keys.push(char_key(letter));
        if standing {
            keys.push(char_key('o'));
        }
        keys.extend(walk_keys(self.app().ui.cursor, from));
        keys.push(enter_key());
        keys.extend(walk_keys(from, to));
        keys.push(enter_key());
        Ok(keys)
    }

    /// `build`: as `plan_designate`, but `u` and the Build modal's own navigation (arrows/`W`/
    /// `S`, as the list is flat) to `building`, then Enter, replace the tool's letter.
    fn plan_build(&self, building: &str, from: Pos, to: Pos) -> Result<Vec<KeyEvent>, String> {
        if !self.in_bounds(from) || !self.in_bounds(to) {
            return Err(format!(
                "build: ({},{}) to ({},{}) isn't on the map",
                from.x, from.y, to.x, to.y
            ));
        }
        let session = &self.app().session;
        let rows = fn_ui::build::picker_rows(session);
        let target = rows
            .iter()
            .position(|r| r.name == building)
            .ok_or_else(|| format!("no such building {building:?} in the Build modal"))?;
        if rows[target].locked() {
            return Err(format!("{building} isn't unlocked yet"));
        }
        let choice = fn_ui::build::valid_choice(&rows, self.app().ui.choice);
        let start = choice.and_then(|id| rows.iter().position(|r| r.id == id)).unwrap_or(0);
        let mut keys = self.close_overlay_keys();
        keys.push(char_key('u'));
        keys.extend(step_rows(start, target));
        keys.push(enter_key());
        keys.extend(walk_keys(self.app().ui.cursor, from));
        keys.push(enter_key());
        keys.extend(walk_keys(from, to));
        keys.push(enter_key());
        Ok(keys)
    }

    /// `select {x,y}`: Esc to Select mode and close any open screen or modal if needed, a cursor
    /// walk, Enter.
    fn plan_select_at(&self, pos: Pos) -> Result<Vec<KeyEvent>, String> {
        if !self.in_bounds(pos) {
            return Err(format!("select: ({},{}) isn't on the map", pos.x, pos.y));
        }
        let mut keys = self.close_overlay_keys();
        keys.extend(self.close_tool_keys());
        keys.extend(walk_keys(self.app().ui.cursor, pos));
        keys.push(enter_key());
        Ok(keys)
    }

    /// `select {name}`: `N`, row navigation (one settler per key, as the roster is a flat list
    /// with no page shortcut that lands exactly), Enter.
    fn plan_select_named(&self, name: &str) -> Result<Vec<KeyEvent>, String> {
        let session = &self.app().session;
        let view = nation_view(session);
        let id = view
            .groups
            .iter()
            .flat_map(|(_, units)| units.iter())
            .find(|u| u.name == name)
            .map(|u| u.id)
            .ok_or_else(|| format!("no such settler {name:?}"))?;
        let target = crate::nation::roster(&view)
            .iter()
            .position(|&i| i == id)
            .expect("the roster holds every settler nation_view does");
        let mut keys = self.close_overlay_keys();
        keys.push(char_key('n'));
        keys.extend(step_rows(0, target));
        keys.push(enter_key());
        Ok(keys)
    }

    /// `research`: `t`, arrow navigation to the card (reusing `crate::research::step`, the same
    /// function the screen's own arrows call, so the path it finds is one the screen would take
    /// too), Enter, Esc (closing the screen again, so a later step starts from the plain map).
    fn plan_research(&self, tech: &str) -> Result<Vec<KeyEvent>, String> {
        let session = &self.app().session;
        let view = fn_ui::research::screen_view(session);
        let target = view
            .columns
            .iter()
            .enumerate()
            .find_map(|(c, col)| col.iter().position(|card| card.name == tech).map(|r| (c, r)))
            .ok_or_else(|| format!("no such tech {tech:?}"))?;
        let start = crate::research::initial_cursor(session, &view);
        let mut keys = self.close_overlay_keys();
        keys.push(char_key('t'));
        keys.extend(research_nav_keys(&view, start, target));
        keys.push(enter_key());
        keys.push(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
        Ok(keys)
    }

    /// `cursor {x,y}`: Esc to close any open screen or modal if needed (so the walk moves the
    /// map's own cursor, not a screen's), then the walk alone -- a plain tool's cursor moves the
    /// same way whatever tool is active, so there is nothing else to plan.
    fn plan_cursor(&self, pos: Pos) -> Result<Vec<KeyEvent>, String> {
        if !self.in_bounds(pos) {
            return Err(format!("cursor: ({},{}) isn't on the map", pos.x, pos.y));
        }
        let mut keys = self.close_overlay_keys();
        keys.extend(walk_keys(self.app().ui.cursor, pos));
        Ok(keys)
    }

    /// `keys {script}`: the script as written, in `--keys` syntax, without `<tick>` (`end_turn`
    /// is how an agent spends game time; a tick here is refused the same way `press_keys`
    /// refuses one).
    fn plan_keys(&self, script: &str) -> Result<Vec<KeyEvent>, String> {
        let steps = keys::parse(script)?;
        if steps.iter().any(|s| matches!(s, Step::Tick(_))) {
            return Err(
                "<tick N> isn't allowed in a keys step; call end_turn instead (the host owns \
                 time)"
                    .into(),
            );
        }
        Ok(steps
            .into_iter()
            .map(|s| match s {
                Step::Key(key) => key,
                Step::Tick(_) => unreachable!("ticks are refused above"),
            })
            .collect())
    }

    /// Esc for each of an open modal and an open screen, in that order (the order `App::key`
    /// itself checks them in): closing both leaves the plain map and the active tool, which is
    /// where every step's own keys assume they start.
    fn close_overlay_keys(&self) -> Vec<KeyEvent> {
        let ui = &self.app().ui;
        let n = usize::from(ui.modal.is_some()) + usize::from(ui.screen.is_some());
        std::iter::repeat_n(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE), n).collect()
    }

    /// Esc enough times to reach the Select tool (`App::cancel`: one Esc drops a corner without
    /// changing tool, so a tool with one set needs a second to actually leave it); none if
    /// already there. Assumes no modal or screen is open ([`Self::close_overlay_keys`] first).
    fn close_tool_keys(&self) -> Vec<KeyEvent> {
        let ui = &self.app().ui;
        let n = if ui.tool == Tool::Select {
            0
        } else if ui.anchor.is_some() {
            2
        } else {
            1
        };
        std::iter::repeat_n(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE), n).collect()
    }

    /// Whether `pos` is on the map at all (`Host::look`'s own regions clamp instead; a step's
    /// tile has to be an exact one, so off the map is a planning failure, not a silent clamp).
    fn in_bounds(&self, pos: Pos) -> bool {
        let map = &self.app().session.world.map;
        (0..map.width()).contains(&pos.x) && (0..map.height()).contains(&pos.y)
    }
}

/// One line of a tooltip as plain text (`fn_ui::inspect::Line`): a hint with no words
/// (`Hint::ApplyTool`, already implied by the tool banner) leaves nothing out of the lines
/// `panel::hint_text` would skip too.
fn line_text(line: Line) -> Option<String> {
    match line {
        Line::Title(s) | Line::Detail(s) => Some(s),
        Line::Hint(h) => hint_text(h),
    }
}

// ------------------------------------------------------------------------------------------
// Action (#316): named steps, lowered to keys by `Host::act`
// ------------------------------------------------------------------------------------------

/// One step of [`Host::act`] (docs/design/agent-play.md, "Action"): a tag-and-fields shape
/// (`{"step": "designate", ...}`), so it reads straight off an MCP tool call's JSON (#317)
/// without a translation layer of its own.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(tag = "step", rename_all = "snake_case")]
pub enum ActionStep {
    /// `{tool, from, to, standing?}`: a rectangle designation. `standing` (default `false`) is
    /// only for `tool: "harvest"`; any other tool with it set fails to plan.
    Designate {
        tool: DesignateTool,
        from: Coord,
        to: Coord,
        #[serde(default)]
        standing: bool,
    },
    /// `{building, from, to}`: the Build modal's own name for the building (as `look`'s legend
    /// and `screen {"nation"}` would show it), and the rectangle to place it over.
    Build { building: String, from: Coord, to: Coord },
    /// `{x, y}` or `{name}`, not both and not neither: the unit at a tile, or the settler with
    /// that name (`screen {"nation"}`'s own roster).
    Select {
        #[serde(default)]
        x: Option<i32>,
        #[serde(default)]
        y: Option<i32>,
        #[serde(default)]
        name: Option<String>,
    },
    /// `{tech}`: the tech's own name (`screen {"research"}`'s cards).
    Research { tech: String },
    /// `{x, y}`: moves the cursor there, with no `Enter` (so `observe`'s `cursor` field reads
    /// it, without designating or selecting anything).
    Cursor { x: i32, y: i32 },
    /// `{script}`: `--keys` syntax, without `<tick N>` (`Host::end_turn` is how an agent spends
    /// game time).
    Keys { script: String },
}

/// A tile, for [`ActionStep::Designate`] and [`ActionStep::Build`]'s `from` and `to`.
#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
pub struct Coord {
    pub x: i32,
    pub y: i32,
}

impl Coord {
    fn pos(self) -> Pos {
        Pos::new(self.x, self.y)
    }
}

/// The tools [`ActionStep::Designate`] may name (docs/design/agent-play.md, "Action"): every
/// designation tool except `Build`, which has its own step since it needs the modal first.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum DesignateTool {
    Harvest,
    Cancel,
    Stockpile,
    Bend,
}

impl DesignateTool {
    fn tool(self) -> Tool {
        match self {
            Self::Harvest => Tool::Harvest,
            Self::Cancel => Tool::CancelHarvest,
            Self::Stockpile => Tool::Stockpile,
            Self::Bend => Tool::Bend,
        }
    }

    fn name(self) -> &'static str {
        match self {
            Self::Harvest => "harvest",
            Self::Cancel => "cancel",
            Self::Stockpile => "stockpile",
            Self::Bend => "bend",
        }
    }
}

/// What [`Host::act`] did: every key actually pressed (as [`RunLog::to_keys_script`]'s own
/// tokens, so a watcher or a test can read them without decoding `KeyEvent`), the step that
/// failed if one did (the steps before it already happened; the ones after it did not run), and
/// the resulting [`Host::observe`].
#[derive(Clone, Debug, Serialize)]
pub struct ActResult {
    pub keys: Vec<String>,
    pub error: Option<ActError>,
    pub observe: Observation,
}

/// Which step of an [`Host::act`] call failed, and why (`Host::plan_step`'s own reason: an
/// unknown tech, a building the Build modal doesn't offer, a tile off the map, a settler by a
/// name nobody has, or a `keys` script that doesn't parse or asks for a tick).
#[derive(Clone, Debug, Serialize)]
pub struct ActError {
    pub step: usize,
    pub reason: String,
}

/// A plain character key, unmodified (`keys::char_key`'s own rule for a lower-case letter).
fn char_key(c: char) -> KeyEvent {
    KeyEvent::new(KeyCode::Char(c), KeyModifiers::NONE)
}

fn enter_key() -> KeyEvent {
    KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE)
}

/// A cursor walk from `from` to `to`, x then y: Shift-steps (`input::SHIFT_STEP`, 10 tiles) for
/// as much of the distance as they cover, then single steps for the rest, so no walk needs more
/// keys than it has to and none ever needs an F-key (docs/design/agent-play.md, "Action").
fn walk_keys(from: Pos, to: Pos) -> Vec<KeyEvent> {
    let mut keys = Vec::new();
    push_axis(&mut keys, to.x - from.x, KeyCode::Right, KeyCode::Left);
    push_axis(&mut keys, to.y - from.y, KeyCode::Down, KeyCode::Up);
    keys
}

fn push_axis(keys: &mut Vec<KeyEvent>, delta: i32, positive: KeyCode, negative: KeyCode) {
    let code = if delta >= 0 { positive } else { negative };
    let mut remaining = delta.unsigned_abs();
    let shift_step = input::SHIFT_STEP.unsigned_abs();
    for _ in 0..remaining / shift_step {
        keys.push(KeyEvent::new(code, KeyModifiers::SHIFT));
    }
    remaining %= shift_step;
    keys.extend(std::iter::repeat_n(KeyEvent::new(code, KeyModifiers::NONE), remaining as usize));
}

/// `Down` or `Up`, one per row, from `start` to `target` (the Build modal's list and the Nation
/// screen's roster are both flat, so this always lands exactly: `crate::build::step` and
/// `crate::nation::step` only ever clamp at either end, never partway, and `target` is always a
/// valid index of the same list).
fn step_rows(start: usize, target: usize) -> Vec<KeyEvent> {
    let (code, n) = if target >= start {
        (KeyCode::Down, target - start)
    } else {
        (KeyCode::Up, start - target)
    };
    std::iter::repeat_n(KeyEvent::new(code, KeyModifiers::NONE), n).collect()
}

/// `Left`/`Right` to `target`'s column, then `Up`/`Down` to its row within it, one key per card
/// (`crate::research::step`'s own rule: a column move is sign-only and a row move can clamp to
/// a shorter column, which only matters while still crossing columns). Replaying
/// `crate::research::step` here, the same function the screen's own arrows call, means the path
/// this finds is one the screen would take too, clamps included.
fn research_nav_keys(
    view: &fn_ui::research::ScreenView,
    start: crate::research::Cursor,
    target: crate::research::Cursor,
) -> Vec<KeyEvent> {
    let mut keys = Vec::new();
    let mut at = start;
    while at.0 != target.0 {
        let dx = if target.0 > at.0 { 1 } else { -1 };
        at = crate::research::step(view, at, dx, 0);
        keys.push(KeyEvent::new(
            if dx > 0 { KeyCode::Right } else { KeyCode::Left },
            KeyModifiers::NONE,
        ));
    }
    while at.1 != target.1 {
        let dy = if target.1 > at.1 { 1 } else { -1 };
        at = crate::research::step(view, at, 0, dy);
        keys.push(KeyEvent::new(
            if dy > 0 { KeyCode::Down } else { KeyCode::Up },
            KeyModifiers::NONE,
        ));
    }
    keys
}

/// What [`Host::act`] records in the run log for one step (the mockup's own "designate harvest
/// 6x4 at (86,60)" style, docs/design/agent-play.md, "Watching").
fn step_label(step: &ActionStep) -> String {
    match step {
        ActionStep::Designate { tool, from, to, .. } => {
            let (w, h) = ((to.x - from.x).abs() + 1, (to.y - from.y).abs() + 1);
            format!("designate {} {w}x{h} at ({},{})", tool.name(), from.x, from.y)
        }
        ActionStep::Build { building, from, to } => {
            let (w, h) = ((to.x - from.x).abs() + 1, (to.y - from.y).abs() + 1);
            format!("build {building} {w}x{h} at ({},{})", from.x, from.y)
        }
        ActionStep::Select { name: Some(name), .. } => format!("select {name}"),
        ActionStep::Select { x: Some(x), y: Some(y), .. } => format!("select ({x},{y})"),
        ActionStep::Select { .. } => "select".to_owned(),
        ActionStep::Research { tech } => format!("research {tech}"),
        ActionStep::Cursor { x, y } => format!("cursor ({x},{y})"),
        ActionStep::Keys { script } => format!("keys {script}"),
    }
}

/// `thing` in a few words, for [`Host::look`]'s legend: the actual unit's name for
/// [`Thing::Unit`] (`pos`'s own occupant, since the cell's `Thing` only keeps its nation), the
/// display name for a building or item, "-" for [`Thing::Ground`] (nothing on top; `ground`
/// already names what's under it).
fn describe_thing(session: &Session, thing: &Thing, pos: Pos) -> String {
    let (world, defs) = (&session.world, &session.defs);
    match thing {
        Thing::Avatar { .. } => "the Avatar".to_owned(),
        Thing::Unit { .. } => world
            .units_at(pos)
            .next()
            .map(|u| format!("settler ({})", u.name))
            .unwrap_or_else(|| "settler".to_owned()),
        Thing::Ghost { building, free } => {
            let name = defs.building(*building).display_name(world.nation);
            format!("{name} ghost ({})", if *free { "fits" } else { "blocked" })
        }
        Thing::Occupied { building, n } => {
            format!("{} ({n} inside)", defs.building(*building).display_name(world.nation))
        }
        Thing::Plot(stage) => format!("farm plot ({stage:?})"),
        Thing::Building { building, .. } => {
            defs.building(*building).display_name(world.nation).to_owned()
        }
        Thing::GateStage(stage_id) => format!("Harmony Gate ({stage_id})"),
        Thing::Site { building, .. } => {
            format!("{} (site)", defs.building(*building).display_name(world.nation))
        }
        Thing::Item(item) => defs.item(*item).name.clone(),
        Thing::Flame => "flame".to_owned(),
        Thing::Sapling => "sapling".to_owned(),
        Thing::RestingResource(r) => format!("{} (resting)", defs.resource(*r).name),
        Thing::Resource(r) => defs.resource(*r).name.clone(),
        Thing::Ground => "-".to_owned(),
    }
}

/// `ground` in a few words: the biome's name, or the terrain's own.
fn describe_ground(session: &Session, ground: Ground) -> String {
    match ground {
        Ground::Terrain(t) => format!("{t:?}").to_lowercase(),
        Ground::Biome(id) => session.defs.biome(id).name.clone(),
    }
}

/// One mark's kind, as [`Host::look`]'s `marks` names it (this crate's `map` module doc comment
/// lists every kind).
fn describe_mark(mark: &Mark) -> String {
    match mark {
        Mark::Cursor => "cursor",
        Mark::Harvest => "harvest",
        Mark::Standing => "standing",
        Mark::Sowing => "sowing",
        Mark::Bend => "bend",
        Mark::Build => "build",
        Mark::Stockpile => "stockpile",
        Mark::Dragged(_) => "dragged",
        Mark::Ghost { .. } => "ghost",
        Mark::Door { .. } => "door",
        Mark::Wet => "wet",
        Mark::Warm => "warmth",
        Mark::AvatarAccent => "avatar",
    }
    .to_owned()
}

/// Assigns one character per distinct description, in the order each is first seen, from a pool
/// generous enough for any region [`MAX_LOOK`] allows in practice (docs/design/agent-play.md,
/// "The map": "one character per distinct thing actually present, so there are no collisions").
#[derive(Default)]
struct CharPool {
    order: Vec<String>,
    legend: BTreeMap<String, String>,
}

/// Printable ASCII, cheapest and least ambiguous characters first; `?` once the pool runs out
/// (an agent's region would need more than this many distinct things or grounds at once, which
/// no bundled map comes close to).
const POOL: &str =
    "0123456789abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ!@#$%^&*()[]{};:'\"\\|/~`+=_<>";

impl CharPool {
    fn char_for(&mut self, description: &str) -> char {
        let index = match self.order.iter().position(|d| d == description) {
            Some(i) => i,
            None => {
                self.order.push(description.to_owned());
                self.order.len() - 1
            }
        };
        let ch = POOL.chars().nth(index).unwrap_or('?');
        self.legend.insert(ch.to_string(), description.to_owned());
        ch
    }
}

// ------------------------------------------------------------------------------------------
// Reads, as data (serde-serializable, built only from fn_ui's view models, the map's cell
// look and the renderer, #315's acceptance criteria)
// ------------------------------------------------------------------------------------------

#[derive(Clone, Debug, Serialize)]
pub struct Observation {
    pub turn: u32,
    pub clock: String,
    pub season: Option<String>,
    pub tool: String,
    pub notice: String,
    pub people: PeopleView,
    pub research: ResearchView,
    pub stores: Vec<StoreView>,
    pub endeavor: Option<EndeavorView>,
    pub cursor: CursorView,
    pub alerts: Vec<String>,
}

#[derive(Clone, Debug, Serialize)]
pub struct PeopleView {
    pub total: usize,
    pub activity: BTreeMap<String, usize>,
    pub needs: BTreeMap<String, usize>,
}

#[derive(Clone, Debug, Serialize)]
pub struct ResearchView {
    pub label: String,
    pub pool: String,
    pub rate: String,
}

#[derive(Clone, Debug, Serialize)]
pub struct StoreView {
    pub name: String,
    pub count: u32,
}

impl From<fn_ui::view::StoreRow> for StoreView {
    fn from(row: fn_ui::view::StoreRow) -> Self {
        Self { name: row.name, count: row.count }
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct EndeavorView {
    pub name: String,
    pub count: String,
    pub line: String,
    pub next: Vec<String>,
}

#[derive(Clone, Debug, Serialize)]
pub struct CursorView {
    pub x: i32,
    pub y: i32,
    pub lines: Vec<String>,
}

#[derive(Clone, Debug, Serialize)]
pub struct MapView {
    pub x: i32,
    pub y: i32,
    pub w: u16,
    pub h: u16,
    pub things: Vec<String>,
    pub ground: Vec<String>,
    pub legend: BTreeMap<String, String>,
    pub marks: Vec<MarkView>,
}

#[derive(Clone, Debug, Serialize)]
pub struct MarkView {
    pub x: i32,
    pub y: i32,
    pub kinds: Vec<String>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(tag = "screen", rename_all = "lowercase")]
pub enum ScreenView {
    Nation(NationScreenView),
    Research(ResearchScreenView),
    Endeavor(EndeavorScreenView),
    Log(LogScreenView),
    Help(HelpScreenView),
}

#[derive(Clone, Debug, Serialize)]
pub struct NationScreenView {
    pub nation: String,
    pub population: usize,
    pub clock: String,
    pub groups: Vec<(String, Vec<String>)>,
    pub stores: Vec<StoreView>,
    pub traits: Vec<(String, String)>,
}

#[derive(Clone, Debug, Serialize)]
pub struct ResearchScreenView {
    pub columns: Vec<Vec<TechCardView>>,
}

#[derive(Clone, Debug, Serialize)]
pub struct TechCardView {
    pub name: String,
    pub cost: u32,
    pub state: String,
    pub unlock: String,
}

#[derive(Clone, Debug, Serialize)]
pub struct EndeavorScreenView {
    pub name: String,
    pub count: String,
    pub stage: String,
    pub groups: Vec<EndeavorGroupView>,
}

#[derive(Clone, Debug, Serialize)]
pub struct EndeavorGroupView {
    pub heading: String,
    pub signs: Vec<SignRowView>,
}

#[derive(Clone, Debug, Serialize)]
pub struct SignRowView {
    pub name: String,
    pub met: bool,
    pub hint: Option<String>,
    pub value: String,
}

#[derive(Clone, Debug, Serialize)]
pub struct LogScreenView {
    pub lines: Vec<String>,
}

#[derive(Clone, Debug, Serialize)]
pub struct HelpScreenView {
    pub rows: Vec<HelpRowView>,
}

#[derive(Clone, Debug, Serialize)]
pub struct HelpRowView {
    pub group: String,
    pub keys: String,
    pub what: String,
}

/// What [`Host::press_keys`] did: how many of the script's keys were pressed and how many were
/// refused ([`Host::refuses`]), in the order they appeared.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize)]
pub struct PressResult {
    pub pressed: usize,
    pub refused: usize,
}

// ------------------------------------------------------------------------------------------
// The run log
// ------------------------------------------------------------------------------------------

/// One thing [`Host`] did, in order (#315's acceptance criteria): a key script (with the
/// `label` and `note` the caller gave it), a `look` or an `inspect` (kept for a watcher's
/// attention ghosts, #319, though they play back as nothing), or a completed turn.
#[derive(Clone, Debug, PartialEq)]
enum LogEntry {
    Keys { keys: Vec<KeyEvent>, label: Option<String>, note: Option<String> },
    Look { x: i32, y: i32, w: u16, h: u16 },
    Inspect { x: i32, y: i32 },
    Turn { index: u32, tick: u64 },
}

/// Every [`LogEntry`] a [`Host`] has made, plus enough of how it started to replay it
/// (`RunLog::to_keys_script`): a seed and a nation are a [`Host::new`] call away from a fresh
/// session, and the keys and ticks since are the rest of the game (docs/design/agent-play.md,
/// "Determinism: a run is a seed plus keys and `<tick N>`s").
#[derive(Clone, Debug, PartialEq)]
pub struct RunLog {
    pub seed: u64,
    pub nation: Nation,
    entries: Vec<LogEntry>,
}

impl RunLog {
    fn new(seed: u64, nation: Nation) -> Self {
        Self { seed, nation, entries: Vec::new() }
    }

    fn push(&mut self, entry: LogEntry) {
        self.entries.push(entry);
    }

    /// How many entries of each kind the log holds, for tests: (keys scripts, looks, inspects,
    /// turns).
    #[cfg(test)]
    fn counts(&self) -> (usize, usize, usize, usize) {
        let mut c = (0, 0, 0, 0);
        for e in &self.entries {
            match e {
                LogEntry::Keys { .. } => c.0 += 1,
                LogEntry::Look { .. } => c.1 += 1,
                LogEntry::Inspect { .. } => c.2 += 1,
                LogEntry::Turn { .. } => c.3 += 1,
            }
        }
        c
    }

    /// The run as a `--keys` script (docs/design/agent-play.md, "Racing", `fn_agent keys`):
    /// every key actually pressed, in order, and `<tick N>` for each turn run
    /// (`turn_seconds × TICKS_PER_SECOND`, the same ticks `end_turn` ran). `look` and `inspect`
    /// leave nothing to replay: `Shell::key` has no equivalent for either.
    pub fn to_keys_script(&self, turn_seconds: u32) -> String {
        let ticks = turn_seconds * TICKS_PER_SECOND;
        let mut tokens = Vec::new();
        for entry in &self.entries {
            match entry {
                LogEntry::Keys { keys, .. } => tokens.extend(keys.iter().map(|&k| key_token(k))),
                LogEntry::Turn { .. } => tokens.push(format!("<tick {ticks}>")),
                LogEntry::Look { .. } | LogEntry::Inspect { .. } => {}
            }
        }
        tokens.join(" ")
    }
}

/// The `--keys` token for `key` (the inverse of `keys::named_key`/`char_key`): round-trips every
/// key [`Host::press_keys`] can actually send, since those all came from `keys::parse` in the
/// first place.
fn key_token(key: KeyEvent) -> String {
    let (ctrl, shift) = (
        key.modifiers.contains(KeyModifiers::CONTROL),
        key.modifiers.contains(KeyModifiers::SHIFT),
    );
    match key.code {
        KeyCode::Char(c) if ctrl => format!("<c-{}>", c.to_ascii_lowercase()),
        // A plain character already carries Shift through its case (`keys::char_key`).
        KeyCode::Char(c) => c.to_string(),
        KeyCode::Enter => "<enter>".to_owned(),
        KeyCode::Esc => "<esc>".to_owned(),
        KeyCode::BackTab => "<s-tab>".to_owned(),
        KeyCode::Tab if shift => "<s-tab>".to_owned(),
        KeyCode::Tab => "<tab>".to_owned(),
        KeyCode::Up => named_token("up", shift),
        KeyCode::Down => named_token("down", shift),
        KeyCode::Left => named_token("left", shift),
        KeyCode::Right => named_token("right", shift),
        KeyCode::PageUp => "<pgup>".to_owned(),
        KeyCode::PageDown => "<pgdn>".to_owned(),
        KeyCode::Home => "<home>".to_owned(),
        KeyCode::End => "<end>".to_owned(),
        KeyCode::Backspace => "<bs>".to_owned(),
        KeyCode::Delete => "<del>".to_owned(),
        KeyCode::F(n) => format!("<f{n}>"),
        other => format!("<{other:?}>"),
    }
}

fn named_token(name: &str, shift: bool) -> String {
    if shift { format!("<s-{name}>") } else { format!("<{name}>") }
}

#[cfg(test)]
mod tests {
    use fn_sim::Nation;

    use super::*;

    fn host(nation: Nation) -> Host {
        Host::new(7, nation, None, None, None, false).unwrap()
    }

    /// A plain `Shell::for_args` game, the same shape `Host::new` builds, for round-trip tests
    /// to replay a key script against.
    fn fresh_shell(nation: Nation) -> Shell {
        let args = Args {
            seed: Some(7),
            nation: Some(nation),
            frame: false,
            size: cli::DEFAULT_SIZE,
            ticks: 0,
            keys: Vec::new(),
            ascii: false,
            ansi: false,
            color: None,
            embark: false,
            start: None,
            map_size: None,
            rough: false,
            difficulty: None,
        };
        Shell::for_args(&args, cli::DEFAULT_SIZE, None, 0).unwrap()
    }

    /// Asserts `replay` (a [`fresh_shell`] that has played `script`, #316's acceptance
    /// criteria) ends up with the same frame and the same command log as `h`: the proof that
    /// `h`'s named action really is nothing but the keys in `script`.
    fn assert_same_as_key_script(h: &mut Host, nation: Nation, script: &str) {
        let mut replay = fresh_shell(nation);
        replay.play(&keys::parse(script).unwrap());
        let size = (100, 40);
        let want = to_text(&render_shell(&mut replay, &h.palette, size));
        let got = h.frame(size.0, size.1);
        assert_eq!(got, want, "{script}");
        let (Mode::Game(replay_app), Mode::Game(host_app)) = (&replay.mode, &h.shell.mode) else {
            panic!("both are games");
        };
        assert_eq!(
            replay_app.session.world.command_log(),
            host_app.session.world.command_log(),
            "{script}"
        );
    }

    #[test]
    fn a_new_host_starts_a_game_not_embark_or_the_title_screen() {
        let h = host(Nation::Earth);
        assert!(matches!(h.shell.mode, Mode::Game(_)));
        assert_eq!(h.observe().turn, 0);
    }

    #[test]
    fn observe_reports_the_people_and_the_tool() {
        let h = host(Nation::Earth);
        let obs = h.observe();
        assert_eq!(obs.people.total, 6, "a new game's starting settlers");
        assert_eq!(obs.tool, "Select");
        assert!(obs.alerts.is_empty());
    }

    #[test]
    fn pressing_a_tool_key_changes_the_tool_and_is_logged() {
        let mut h = host(Nation::Earth);
        let result =
            h.press_keys("h", Some("harvest".into()), Some("switch to Harvest".into())).unwrap();
        assert_eq!(result, PressResult { pressed: 1, refused: 0 });
        assert_eq!(h.observe().tool, "Harvest");
        assert_eq!(h.log.counts(), (1, 0, 0, 0));
    }

    #[test]
    fn press_keys_refuses_a_tick_token_outright() {
        let mut h = host(Nation::Earth);
        let err = h.press_keys("h <tick 5>", None, None).unwrap_err();
        assert!(err.contains("end_turn"), "{err}");
        // Nothing ran: not even the `h` before the refused token.
        assert_eq!(h.observe().tool, "Select");
    }

    /// Space, +, -, ., R, 1-4, q and F5/F6/F9 are refused, and refusing changes nothing
    /// (#315's acceptance criteria).
    #[test]
    fn the_listed_keys_are_refused_and_change_nothing() {
        let mut h = host(Nation::Earth);
        let before = h.frame(40, 20);
        let script = "<space> + - . R 1 2 3 4 q <c-c> <f5> <f6> <f9>";
        let result = h.press_keys(script, None, None).unwrap();
        assert_eq!(result.pressed, 0, "{result:?}");
        assert_eq!(result.refused, 14, "{result:?}");
        assert_eq!(h.frame(40, 20), before);
        assert!(!h.shell.quit);
    }

    /// The Esc menu's Save as…, Quicksave, Load…, Quickload and Quit rows are refused too
    /// (`Enter` on any of them), while Resume and Key help still work.
    #[test]
    fn the_esc_menus_save_load_and_quit_rows_are_refused() {
        let mut h = host(Nation::Earth);
        h.press_keys("<esc>", None, None).unwrap(); // opens the menu (nothing else to close)
        assert_eq!(h.app().ui.modal, Some(Modal::Menu));
        for _ in 0..crate::menu::MenuRow::ALL.len() {
            let row = crate::menu::MenuRow::ALL[h.app().ui.menu_cursor];
            let result = h.press_keys("<enter>", None, None).unwrap();
            let refused = matches!(
                row,
                MenuRow::SaveAs
                    | MenuRow::Quicksave
                    | MenuRow::Load
                    | MenuRow::Quickload
                    | MenuRow::Quit
            );
            assert_eq!(result.refused, usize::from(refused), "{row:?}");
            if refused {
                assert_eq!(h.app().ui.modal, Some(Modal::Menu), "{row:?}: still open");
            }
            h.press_keys("<down>", None, None).unwrap();
        }
    }

    #[test]
    fn end_turn_runs_turn_seconds_of_ticks_and_counts_the_turn() {
        let mut h = host(Nation::Earth).with_turn_seconds(5);
        let obs = h.end_turn(1);
        assert_eq!(obs.turn, 1);
        assert_eq!(h.app().session.world.tick, 5 * u64::from(TICKS_PER_SECOND));
        assert_eq!(h.log.counts(), (0, 0, 0, 1));
    }

    #[test]
    fn end_turn_stops_early_once_a_need_flag_first_shows() {
        let mut h = host(Nation::Earth).with_turn_seconds(60);
        // Long enough for hunger to show, short enough it hasn't on the first turn either.
        let obs = h.end_turn(200);
        assert!(obs.turn >= 1 && obs.turn < 200, "{}", obs.turn);
        assert!(!obs.people.needs.is_empty(), "a need flag should have stopped it");
    }

    #[test]
    fn look_stays_within_max_look_and_the_legend_has_no_collisions() {
        let mut h = host(Nation::Earth);
        let view = h.look(0, 0, 200, 200);
        assert_eq!((view.w, view.h), MAX_LOOK);
        assert_eq!(view.things.len(), MAX_LOOK.1 as usize);
        assert!(view.things.iter().all(|row| row.chars().count() == MAX_LOOK.0 as usize));
        let chars: Vec<char> = view.legend.keys().map(|k| k.chars().next().unwrap()).collect();
        assert_eq!(chars.len(), chars.iter().collect::<std::collections::BTreeSet<_>>().len());
        assert_eq!(h.log.counts(), (0, 1, 0, 0));
    }

    #[test]
    fn looks_things_glyph_matches_what_cell_look_decided() {
        let mut h = host(Nation::Earth);
        let from = h.app().session.world.units[0].pos;
        let view = h.look(from.x - 1, from.y - 1, 3, 3);
        // The settler is somewhere in this 3x3: its description names "settler" and isn't "-".
        let has_settler = view.legend.values().any(|d| d.contains("settler"));
        assert!(has_settler, "{:?}", view.legend);
    }

    #[test]
    fn inspect_reads_an_arbitrary_tile_without_moving_the_cursor() {
        let mut h = host(Nation::Earth);
        let cursor_before = h.app().ui.cursor;
        let elsewhere = Pos::new(cursor_before.x + 5, cursor_before.y + 5);
        let view = h.inspect(elsewhere.x, elsewhere.y);
        assert_eq!((view.x, view.y), (elsewhere.x, elsewhere.y));
        assert_eq!(h.app().ui.cursor, cursor_before, "inspect doesn't move the real cursor");
        assert_eq!(h.log.counts(), (0, 0, 1, 0));
    }

    #[test]
    fn every_screen_name_resolves_and_an_unknown_one_errors() {
        let h = host(Nation::Earth);
        for name in ["nation", "research", "endeavor", "log", "help"] {
            h.screen(name).unwrap();
        }
        assert!(h.screen("bogus").is_err());
    }

    /// Replaying a run log's `to_keys_script` through a fresh `Shell::play` gives the same frame
    /// and command log (#315's acceptance criteria).
    #[test]
    fn replaying_the_run_log_gives_the_same_frame_and_command_log() {
        let mut h = host(Nation::Fire).with_turn_seconds(10);
        h.press_keys("h <right*3> <enter> <down*2> <enter>", None, None).unwrap();
        h.end_turn(2);
        h.look(0, 0, 10, 10);
        h.inspect(5, 5);
        h.press_keys("x", None, None).unwrap();

        let script = h.log().to_keys_script(h.turn_seconds);
        let steps = keys::parse(&script).unwrap();
        let args = Args {
            seed: Some(7),
            nation: Some(Nation::Fire),
            frame: false,
            size: cli::DEFAULT_SIZE,
            ticks: 0,
            keys: Vec::new(),
            ascii: false,
            ansi: false,
            color: None,
            embark: false,
            start: None,
            map_size: None,
            rough: false,
            difficulty: None,
        };
        let mut replay = Shell::for_args(&args, cli::DEFAULT_SIZE, None, 0).unwrap();
        replay.play(&steps);

        let size = (100, 40);
        let want = to_text(&render_shell(&mut replay, &h.palette, size));
        let got = h.frame(size.0, size.1);
        assert_eq!(got, want);

        let (Mode::Game(replay_app), Mode::Game(host_app)) = (&replay.mode, &h.shell.mode) else {
            panic!("both are games");
        };
        assert_eq!(replay_app.session.world.command_log(), host_app.session.world.command_log());
    }

    /// Two hosts given the same calls produce identical logs and frames (#315's acceptance
    /// criteria): determinism, since nothing here reads a clock.
    #[test]
    fn two_hosts_given_the_same_calls_agree() {
        let mut a = host(Nation::Water).with_turn_seconds(15);
        let mut b = host(Nation::Water).with_turn_seconds(15);
        for h in [&mut a, &mut b] {
            h.press_keys("p <right*4> <enter> <down*2> <enter>", Some("stockpile".into()), None)
                .unwrap();
            h.end_turn(3);
            h.look(2, 2, 8, 8);
            h.inspect(3, 3);
        }
        assert_eq!(a.log, b.log);
        assert_eq!(a.frame(80, 30), b.frame(80, 30));
    }

    // ----------------------------------------------------------------------------------------
    // Action (#316)
    // ----------------------------------------------------------------------------------------

    #[test]
    fn act_designate_round_trips_with_its_key_script_and_designates_the_rectangle() {
        let mut h = host(Nation::Earth);
        let cursor = h.app().ui.cursor;
        let from = Pos::new(cursor.x + 2, cursor.y + 1);
        let to = Pos::new(from.x + 1, from.y + 1);
        let steps = vec![ActionStep::Designate {
            tool: DesignateTool::Harvest,
            from: Coord { x: from.x, y: from.y },
            to: Coord { x: to.x, y: to.y },
            standing: false,
        }];
        let result = h.act(&steps, Some("wood for a bed".into()));
        assert!(result.error.is_none(), "{result:?}");
        assert_eq!(h.observe().tool, "Harvest");
        assert_eq!(h.log.counts(), (1, 0, 0, 0));

        let script = result.keys.join(" ");
        assert_eq!(script, "h <right> <right> <down> <enter> <right> <down> <enter>");
        assert_same_as_key_script(&mut h, Nation::Earth, &script);
    }

    #[test]
    fn act_designate_standing_order_presses_o_after_the_tool_letter() {
        let mut h = host(Nation::Fire);
        let cursor = h.app().ui.cursor;
        let from = Pos::new(cursor.x + 1, cursor.y);
        let to = from;
        let steps = vec![ActionStep::Designate {
            tool: DesignateTool::Harvest,
            from: Coord { x: from.x, y: from.y },
            to: Coord { x: to.x, y: to.y },
            standing: true,
        }];
        let result = h.act(&steps, None);
        assert!(result.error.is_none(), "{result:?}");
        assert_eq!(result.keys.join(" "), "h o <right> <enter> <enter>");
        assert!(h.app().ui.repeating);
        assert_same_as_key_script(&mut h, Nation::Fire, &result.keys.join(" "));
    }

    #[test]
    fn act_designate_refuses_standing_orders_outside_the_harvest_tool_and_presses_nothing() {
        let mut h = host(Nation::Earth);
        let cursor = h.app().ui.cursor;
        let steps = vec![ActionStep::Designate {
            tool: DesignateTool::Stockpile,
            from: Coord { x: cursor.x, y: cursor.y },
            to: Coord { x: cursor.x, y: cursor.y },
            standing: true,
        }];
        let result = h.act(&steps, None);
        let err = result.error.unwrap();
        assert_eq!(err.step, 0);
        assert!(err.reason.contains("harvest"), "{}", err.reason);
        assert!(result.keys.is_empty());
        assert_eq!(h.observe().tool, "Select", "nothing was pressed");
    }

    #[test]
    fn act_designate_an_off_map_tile_fails_and_presses_nothing() {
        let mut h = host(Nation::Earth);
        let steps = vec![ActionStep::Designate {
            tool: DesignateTool::Harvest,
            from: Coord { x: -1, y: 0 },
            to: Coord { x: 0, y: 0 },
            standing: false,
        }];
        let result = h.act(&steps, None);
        let err = result.error.unwrap();
        assert!(err.reason.contains("isn't on the map"), "{}", err.reason);
        assert!(result.keys.is_empty());
        assert_eq!(h.observe().tool, "Select");
    }

    #[test]
    fn act_build_round_trips_with_its_key_script_and_picks_the_building() {
        let mut h = host(Nation::Earth);
        let rows = fn_ui::build::picker_rows(&h.app().session);
        let unlocked: Vec<_> = rows.iter().filter(|r| !r.locked()).collect();
        assert!(unlocked.len() >= 2, "need two unlocked buildings to exercise the navigation");
        let building = unlocked[1].name.clone();
        let cursor = h.app().ui.cursor;
        let from = Pos::new(cursor.x + 3, cursor.y + 2);
        let to = from;
        let steps = vec![ActionStep::Build {
            building: building.clone(),
            from: Coord { x: from.x, y: from.y },
            to: Coord { x: to.x, y: to.y },
        }];
        let result = h.act(&steps, None);
        assert!(result.error.is_none(), "{result:?}");
        assert_eq!(h.observe().tool, "Build");
        let want_id = unlocked[1].id;
        assert_eq!(h.app().ui.choice, Some(want_id));
        assert_same_as_key_script(&mut h, Nation::Earth, &result.keys.join(" "));
    }

    #[test]
    fn act_build_an_unknown_building_fails_and_presses_nothing() {
        let mut h = host(Nation::Earth);
        let cursor = h.app().ui.cursor;
        let steps = vec![ActionStep::Build {
            building: "Not A Real Building".into(),
            from: Coord { x: cursor.x, y: cursor.y },
            to: Coord { x: cursor.x, y: cursor.y },
        }];
        let result = h.act(&steps, None);
        let err = result.error.unwrap();
        assert!(err.reason.contains("Not A Real Building"), "{}", err.reason);
        assert!(result.keys.is_empty());
        assert_eq!(h.observe().tool, "Select");
    }

    #[test]
    fn act_select_at_round_trips_and_selects_the_unit_there() {
        let mut h = host(Nation::Water);
        let target = h.app().session.world.units[0].pos;
        let want_id = h.app().session.world.units[0].id;
        let steps = vec![ActionStep::Select { x: Some(target.x), y: Some(target.y), name: None }];
        let result = h.act(&steps, None);
        assert!(result.error.is_none(), "{result:?}");
        assert_eq!(h.app().ui.selected, Some(want_id));
        assert_same_as_key_script(&mut h, Nation::Water, &result.keys.join(" "));
    }

    /// Leaving a tool with an anchor already set takes two Esc (`App::cancel`'s own rule: one
    /// drops the corner, a second actually leaves the tool), and `select` plans both before the
    /// walk and the final Enter.
    #[test]
    fn act_select_at_leaves_a_tool_with_an_anchor_set_first() {
        let mut h = host(Nation::Water);
        let prep = "h <right> <enter>";
        h.press_keys(prep, None, None).unwrap();
        assert!(h.app().ui.anchor.is_some());
        let target = h.app().session.world.units[1].pos;
        let want_id = h.app().session.world.units[1].id;
        let steps = vec![ActionStep::Select { x: Some(target.x), y: Some(target.y), name: None }];
        let result = h.act(&steps, None);
        assert!(result.error.is_none(), "{result:?}");
        assert_eq!(h.observe().tool, "Select");
        assert_eq!(h.app().ui.selected, Some(want_id));
        assert!(result.keys.starts_with(&["<esc>".to_owned(), "<esc>".to_owned()]), "{result:?}");
        let script = format!("{prep} {}", result.keys.join(" "));
        assert_same_as_key_script(&mut h, Nation::Water, &script);
    }

    #[test]
    fn act_select_named_round_trips_and_selects_that_settler() {
        let mut h = host(Nation::Air);
        let want_id = h.app().session.world.units[2].id;
        let name = h.app().session.world.units[2].name.clone();
        let steps = vec![ActionStep::Select { x: None, y: None, name: Some(name) }];
        let result = h.act(&steps, None);
        assert!(result.error.is_none(), "{result:?}");
        assert_eq!(h.app().ui.selected, Some(want_id));
        assert_eq!(h.observe().tool, "Select", "the Nation screen closes once more (#308)");
        assert_same_as_key_script(&mut h, Nation::Air, &result.keys.join(" "));
    }

    #[test]
    fn act_select_an_unknown_name_fails_and_presses_nothing() {
        let mut h = host(Nation::Air);
        let steps = vec![ActionStep::Select { x: None, y: None, name: Some("Nobody".into()) }];
        let result = h.act(&steps, None);
        let err = result.error.unwrap();
        assert!(err.reason.contains("Nobody"), "{}", err.reason);
        assert!(result.keys.is_empty());
    }

    #[test]
    fn act_select_wants_either_coordinates_or_a_name_not_both_or_neither() {
        let mut h = host(Nation::Air);
        for steps in [
            vec![ActionStep::Select { x: None, y: None, name: None }],
            vec![ActionStep::Select { x: Some(0), y: Some(0), name: Some("Anyone".into()) }],
            vec![ActionStep::Select { x: Some(0), y: None, name: None }],
        ] {
            let result = h.act(&steps, None);
            assert!(result.error.is_some(), "{steps:?}");
            assert!(result.keys.is_empty());
        }
    }

    #[test]
    fn act_research_round_trips_and_sets_the_target() {
        let mut h = host(Nation::Fire);
        let view = fn_ui::research::screen_view(&h.app().session);
        let tech = view
            .columns
            .iter()
            .flatten()
            .find(|c| c.state == fn_ui::research::TechState::Available)
            .expect("a new game always has something researchable")
            .name
            .clone();
        let steps = vec![ActionStep::Research { tech: tech.clone() }];
        let result = h.act(&steps, None);
        assert!(result.error.is_none(), "{result:?}");
        assert_eq!(h.observe().research.label, tech);
        assert_eq!(h.observe().tool, "Select", "the Esc at the end closes the screen again");
        assert!(result.keys.last().is_some_and(|k| k == "<esc>"));
        assert_same_as_key_script(&mut h, Nation::Fire, &result.keys.join(" "));
    }

    #[test]
    fn act_research_an_unknown_tech_fails_and_presses_nothing() {
        let mut h = host(Nation::Fire);
        let steps = vec![ActionStep::Research { tech: "Not A Real Tech".into() }];
        let result = h.act(&steps, None);
        let err = result.error.unwrap();
        assert!(err.reason.contains("Not A Real Tech"), "{}", err.reason);
        assert!(result.keys.is_empty());
    }

    #[test]
    fn act_cursor_round_trips_and_moves_the_cursor_with_no_enter() {
        let mut h = host(Nation::Earth);
        let cursor = h.app().ui.cursor;
        let to = Pos::new(cursor.x + 23, cursor.y - 4);
        let steps = vec![ActionStep::Cursor { x: to.x, y: to.y }];
        let result = h.act(&steps, None);
        assert!(result.error.is_none(), "{result:?}");
        assert_eq!(h.app().ui.cursor, to);
        assert_eq!(h.observe().cursor.x, to.x);
        assert_eq!(h.observe().cursor.y, to.y);
        // A run of 23 is two Shift-steps (20) and three single ones, on x alone; y is within 10.
        assert_eq!(
            result.keys.join(" "),
            "<s-right> <s-right> <right> <right> <right> <up> <up> <up> <up>"
        );
        assert_same_as_key_script(&mut h, Nation::Earth, &result.keys.join(" "));
    }

    #[test]
    fn act_cursor_an_off_map_tile_fails_and_presses_nothing() {
        let mut h = host(Nation::Earth);
        let steps = vec![ActionStep::Cursor { x: -5, y: 0 }];
        let result = h.act(&steps, None);
        let err = result.error.unwrap();
        assert!(err.reason.contains("isn't on the map"), "{}", err.reason);
        assert!(result.keys.is_empty());
    }

    #[test]
    fn act_keys_round_trips_and_refuses_the_same_keys_press_keys_does() {
        let mut h = host(Nation::Earth);
        let steps = vec![ActionStep::Keys { script: "h <right*2> <enter> q".into() }];
        let result = h.act(&steps, None);
        assert!(result.error.is_none(), "{result:?}");
        assert_eq!(result.keys, vec!["h", "<right>", "<right>", "<enter>"]);
        assert!(!h.shell.quit, "q is refused");
        assert_same_as_key_script(&mut h, Nation::Earth, "h <right*2> <enter>");
    }

    #[test]
    fn act_keys_refuses_a_tick_token_and_presses_nothing() {
        let mut h = host(Nation::Earth);
        let steps = vec![ActionStep::Keys { script: "h <tick 5>".into() }];
        let result = h.act(&steps, None);
        let err = result.error.unwrap();
        assert!(err.reason.contains("end_turn"), "{}", err.reason);
        assert!(result.keys.is_empty());
        assert_eq!(h.observe().tool, "Select");
    }

    /// A step that can't be planned fails with its own reason, and the steps after it never
    /// run, but the ones before it already happened (#316's acceptance criteria).
    #[test]
    fn a_failing_step_stops_the_rest_but_not_what_already_ran() {
        let mut h = host(Nation::Earth);
        let cursor = h.app().ui.cursor;
        let to = Pos::new(cursor.x + 1, cursor.y);
        let steps = vec![
            ActionStep::Cursor { x: to.x, y: to.y },
            ActionStep::Research { tech: "Not A Real Tech".into() },
            ActionStep::Cursor { x: cursor.x, y: cursor.y },
        ];
        let result = h.act(&steps, None);
        let err = result.error.unwrap();
        assert_eq!(err.step, 1);
        // Only the first step's one key: the third step never ran.
        assert_eq!(result.keys, vec!["<right>"]);
        assert_eq!(h.app().ui.cursor, to, "the first step's own effect stands");
    }

    /// The log keeps the action's label and the agent's note, the note capped at 80 characters
    /// (#316's acceptance criteria).
    #[test]
    fn act_logs_a_label_and_truncates_a_long_note() {
        let mut h = host(Nation::Earth);
        let cursor = h.app().ui.cursor;
        let note: String = std::iter::repeat_n('x', 200).collect();
        let to = Pos::new(cursor.x + 1, cursor.y);
        let steps = vec![ActionStep::Cursor { x: to.x, y: to.y }];
        h.act(&steps, Some(note));
        let LogEntry::Keys { label, note, .. } = h.log.entries.last().unwrap() else {
            panic!("act logs a Keys entry");
        };
        let want_label = format!("cursor ({},{})", to.x, to.y);
        assert_eq!(label.as_deref(), Some(want_label.as_str()));
        assert_eq!(note.as_ref().unwrap().chars().count(), 80);
    }
}
