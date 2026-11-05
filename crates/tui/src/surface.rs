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
use fn_ui::view::{activity_counts, nation_view, need_flags, season_view, side_view};
use fn_ui::{Session, Tool};
use serde::{Deserialize, Serialize};

use crate::app::App;
use crate::cli::{self, Args};
use crate::color::{ColorDepth, Palette};
use crate::frame::{render_shell, to_text};
use crate::input::{self, Action};
use crate::keys::{self, Step};
use crate::map::{CellLook, Ground, Mark, Overlays, Thing, cell_look, style_cell};
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
    /// [`Self::new`]'s own arguments, kept only so [`Self::header`] can describe how to rebuild
    /// this session (a watcher, #319, or `fn_agent verify`, #317/#318, start from a header, not
    /// from this live `Host`).
    header: RunLogHeader,
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
            fast: false,
            watch: None,
            pace: None,
            at: None,
        };
        let shell = Shell::for_args(&args, cli::DEFAULT_SIZE, None, 0)?;
        let palette = Palette::new(shell.defs(), nation, ColorDepth::TrueColor, true);
        let data_digest = shell.defs().id_hash();
        let header = RunLogHeader {
            seed,
            nation,
            turn_seconds: DEFAULT_TURN_SECONDS,
            ticks_per_turn: DEFAULT_TURN_SECONDS * TICKS_PER_SECOND,
            map_size: args.map_size,
            difficulty: args.difficulty,
            site,
            rough,
            data_digest,
            crate_version: env!("CARGO_PKG_VERSION").to_owned(),
        };
        Ok(Self {
            shell,
            palette,
            turn_seconds: DEFAULT_TURN_SECONDS,
            turn: 0,
            alerts_since_turn: 0,
            log: RunLog::new(seed, nation),
            header,
        })
    }

    /// Sets how many game seconds one `end_turn` turn covers (20 by default); at least one.
    pub fn with_turn_seconds(mut self, turn_seconds: u32) -> Self {
        self.turn_seconds = turn_seconds.max(1);
        self.header.turn_seconds = self.turn_seconds;
        self.header.ticks_per_turn = self.turn_seconds * TICKS_PER_SECOND;
        self
    }

    /// The run log so far: every key pressed and every turn run, in order.
    pub fn log(&self) -> &RunLog {
        &self.log
    }

    /// How many game seconds one `end_turn` turn covers (`Self::with_turn_seconds`).
    pub fn turn_seconds(&self) -> u32 {
        self.turn_seconds
    }

    /// How many ticks one `end_turn` turn runs (`Self::turn_seconds` times
    /// `TICKS_PER_SECOND`): what a run log needs to turn a turn count back into the `<tick N>`
    /// a key script would use (`fn_agent keys`, docs/design/agent-play.md, "Racing").
    pub fn ticks_per_turn(&self) -> u32 {
        self.turn_seconds * TICKS_PER_SECOND
    }

    /// The world's own tick: for a run log's diagnostics (`fn_agent`'s own `Turn` event), which
    /// names the tick a turn ended on. Not part of [`Observation`] (#315: an observation only
    /// says what a screen does, and no screen prints the raw tick), but harmless to read here
    /// since it changes nothing and isn't returned to an agent.
    pub fn tick(&self) -> u64 {
        self.app().session.ticks_elapsed()
    }

    /// `Defs::id_hash`: a stable hash of the bundled data's id tables (docs/design/agent-play.md,
    /// "Racing"), for a run log's header -- a replay only holds for the same code and data, so
    /// the header records what the run was made against.
    pub fn data_digest(&self) -> u64 {
        self.shell.defs().id_hash()
    }

    /// Enough of how this session started to rebuild it (`seed`, `nation`, `turn_seconds`,
    /// `map_size`, `difficulty`, `site` and `rough`, [`Self::new`]'s own arguments), plus
    /// `ticks_per_turn`, `data_digest` and `crate_version` (what the run was made against): the
    /// first line a run log writes (docs/design/agent-play.md, "Racing": "Run log"), and what
    /// `fn_tui --watch` (#324) and `fn_agent keys`/`verify` (#317/#318) read to start one of
    /// their own.
    pub fn header(&self) -> RunLogHeader {
        self.header.clone()
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
    /// without a function key (#306), plus `Esc` itself when nothing is open to close and it
    /// would only open the game menu (#327: every row but Resume and Key help is already
    /// refused above, so for an agent the menu is only a trap, not a tool).
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
        if key.code == KeyCode::Esc && self.opens_menu() {
            return true;
        }
        false
    }

    /// Whether an `Esc` right now would do nothing but `App::cancel`'s own last resort
    /// (`App::open_menu`): no prompt, modal or screen open, the Select tool, and nothing
    /// selected -- the exact condition `App::cancel` checks before it opens the menu.
    fn opens_menu(&self) -> bool {
        let ui = &self.app().ui;
        ui.prompt.is_none()
            && ui.modal.is_none()
            && ui.screen.is_none()
            && ui.tool == Tool::Select
            && ui.selected.is_none()
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
        // `season_view`'s own heading is "Spring · Y1"; the bare name before " · " is the same
        // word the panel's divider prints (uppercased there, a styling difference the
        // observation-containment test treats as such -- docs/design/agent-play.md, "The map").
        let season = season_view(session, 0)
            .map(|s| s.heading.split(" · ").next().unwrap_or(&s.heading).to_owned());
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
            endeavor: side.endeavor.map(|e| {
                // The same row `panel::endeavor_rows` draws under the Endeavor heading: `e.line`
                // on its own ("Signs met - G: open", the Ready text, or the stage) only once
                // nothing is missing; otherwise the first missing sign, the same way, so this
                // never claims signs are met while `next` still lists one (#320, "Observation
                // containment").
                let line = match e.next.first() {
                    Some(next) => format!("Next: {next}"),
                    None => e.line,
                };
                // `panel::endeavor_rows` only ever names the first missing sign; the rest of
                // `e.next` (`fn_ui::endeavor::NEXT_SHOWN`'s headroom for a future comma-joined
                // row) isn't shown anywhere yet, so `next` stops at one too (#320, "Observation
                // containment"; a richer row is a TUI change first, docs/design/agent-play.md,
                // "The parity rule").
                let next = e.next.into_iter().take(1).collect();
                EndeavorView { name: e.name, count: e.count, line, next }
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
    /// `cell_look` and `style_cell` the painter uses (#314), so this can't show anything the
    /// screen hides. Each cell's `things` character is the very glyph `--ascii` draws there
    /// (marks never change a cell's glyph, only its colour and modifier, so `style_cell` alone
    /// decides it); `ground` is the same glyph `style_cell` would choose with nothing on top,
    /// which only matters apart from `things` where something is (#320, "Map-cell equality").
    /// The legend is local to this response: one character per distinct thing or ground
    /// actually present, so there are no collisions (docs/design/agent-play.md, "The map") --
    /// the real glyph when that's still free in this response, a fallback character on the rare
    /// ascii collision (docs/design/agent-play.md, "Frames and keys": "F, *, $, %, = and S each
    /// mean two or more things").
    pub fn look(&mut self, x: i32, y: i32, w: u16, h: u16) -> MapView {
        let (w, h) = (w.clamp(1, MAX_LOOK.0), h.clamp(1, MAX_LOOK.1));
        let app = self.app();
        let (session, ui) = (&app.session, &app.ui);
        let defs = &session.defs;
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
                        let thing_glyph = style_cell(&cell, defs, &self.palette).glyph;
                        let ground_only = CellLook {
                            thing: Thing::Ground,
                            ground: cell.ground,
                            marks: Vec::new(),
                        };
                        let ground_glyph = style_cell(&ground_only, defs, &self.palette).glyph;
                        thing_line.push(
                            pool.char_for(&describe_thing(session, &cell.thing, pos), thing_glyph),
                        );
                        ground_line.push(
                            pool.char_for(&describe_ground(session, cell.ground), ground_glyph),
                        );
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

    /// `nation`, `research`, `endeavor`, `log`, `help` or `build`, as data: `nation_view`,
    /// `research::screen_view`, `endeavor::screen_view`, `input::help_rows` or
    /// `fn_ui::build::picker_rows`, reachable in the TUI the same way (`N`, `T`, `G`, `L`, `?`,
    /// `U`).
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
            "build" => Ok(ScreenView::Build(self.build_screen())),
            other => Err(format!(
                "no such screen {other:?} (nation, research, endeavor, log, help or build)"
            )),
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
                            // `crate::research::state_word`, not `TechState::label`: every
                            // card's own row carries the short word ("Locked"); the longer
                            // sentence (`label`, "Needs prereqs") only shows for whichever card
                            // the cursor is on, under the grid (#320, "Observation
                            // containment").
                            state: crate::research::state_word(c.state).to_owned(),
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

    /// `fn_ui::build::picker_rows`, the Build modal's own list (#327): every row's name, size
    /// (`defs.building(id).footprint`, "1x1" or "2x2", the same text `BuildingDetail::footprint`
    /// already formats) and the modal's own list text for the rest of the row (the real cost,
    /// "5 Wood", or "Locked" when the nation hasn't unlocked it yet, `crate::build::list_row`'s
    /// own choice) -- so an agent can read the list the way the Build modal's own rows show it,
    /// without a `build` step to probe it blind.
    fn build_screen(&self) -> BuildScreenView {
        let session = &self.app().session;
        let rows = fn_ui::build::picker_rows(session)
            .into_iter()
            .map(|r| {
                let (w, h) = session.defs.building(r.id).footprint;
                let locked = r.locked();
                let cost = if locked { "Locked".to_owned() } else { r.cost };
                BuildRowView { name: r.name, size: format!("{w}x{h}"), cost, locked }
            })
            .collect();
        BuildScreenView { rows }
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
    ///
    /// `ActResult::notices` carries each `designate` or `build` step's own notice (#327: the "N
    /// done, M skipped: reason" text a player sees in the Tool section), so a mostly-skipped
    /// rectangle is as visible to an agent as it is to a player, even in a call with several
    /// steps -- a tool switch clears `UiState::notice` first (`UiState::set_tool`), so the step
    /// right after a designate or build always reads that step's own, not a stale one from
    /// before it.
    pub fn act(&mut self, steps: &[ActionStep], note: Option<String>) -> ActResult {
        let mut pressed = Vec::new();
        let mut labels = Vec::new();
        let mut notices = Vec::new();
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
                    let notice = self.app().ui.notice.clone();
                    if !notice.is_empty() {
                        notices.push(StepNotice { step: index, notice });
                    }
                }
                Err(reason) => {
                    error = Some(ActError { step: index, reason });
                    break;
                }
            }
        }
        let label = (!labels.is_empty()).then(|| labels.join("; "));
        let note = note.map(|n| n.chars().take(80).collect::<String>());
        self.log.push(LogEntry::Keys { keys: pressed.clone(), label: label.clone(), note });
        ActResult {
            keys: pressed.into_iter().map(key_token).collect(),
            label,
            notices,
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
        let (w, h) = self.app().session.map_size();
        (0..w).contains(&pos.x) && (0..h).contains(&pos.y)
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
    /// Every step that ran, in order, joined as the mockup's own "designate harvest 6x4 at
    /// (86,60)" style (`None` if every step failed to plan, so nothing ran): the same label
    /// kept in [`Host::log`].
    pub label: Option<String>,
    /// Each `designate` or `build` step's own notice (#327), in the order its step ran: the "N
    /// done, M skipped: reason" text a player sees in the Tool section, so a mostly-skipped
    /// rectangle is obvious even several steps into one `act` call. Empty for a call with no
    /// `designate` or `build` step, or where one left no notice to show.
    pub notices: Vec<StepNotice>,
    pub error: Option<ActError>,
    pub observe: Observation,
}

/// One step's own notice, from [`Host::act`]'s `notices` (#327): `step` indexes the same
/// `steps` array `ActError::step` would, so an agent can tell which designate or build it
/// belongs to even when several ran in the one call.
#[derive(Clone, Debug, Serialize)]
pub struct StepNotice {
    pub step: usize,
    pub notice: String,
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
    let defs = &session.defs;
    let nation = session.nation();
    match thing {
        Thing::Avatar { .. } => "the Avatar".to_owned(),
        Thing::Unit { .. } => session
            .unit_name_at(pos)
            .map(|name| format!("settler ({name})"))
            .unwrap_or_else(|| "settler".to_owned()),
        Thing::Ghost { building, free } => {
            let name = defs.building(*building).display_name(nation);
            format!("{name} ghost ({})", if *free { "fits" } else { "blocked" })
        }
        Thing::Occupied { building, n } => {
            format!("{} ({n} inside)", defs.building(*building).display_name(nation))
        }
        Thing::Plot(stage) => format!("farm plot ({stage:?})"),
        Thing::Building { building, .. } => {
            defs.building(*building).display_name(nation).to_owned()
        }
        Thing::GateStage(stage_id) => format!("Harmony Gate ({stage_id})"),
        Thing::Site { building, .. } => {
            format!("{} (site)", defs.building(*building).display_name(nation))
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

/// Assigns one character per distinct description, preferring each description's own `--ascii`
/// glyph (so [`Host::look`]'s `things` and `ground` read as the frame does, #320's "Map-cell
/// equality"), and only falling back to a pool character on the rare ascii collision -- two
/// different descriptions sharing one glyph in this response, which colour alone would tell
/// apart (docs/design/agent-play.md, "Frames and keys"). Either way there's one character per
/// distinct description actually present, so there are no collisions in the legend itself
/// (docs/design/agent-play.md, "The map").
#[derive(Default)]
struct CharPool {
    by_description: BTreeMap<String, char>,
    legend: BTreeMap<String, String>,
}

/// Printable ASCII, cheapest and least ambiguous characters first; `?` once the pool runs out
/// (an agent's region would need more than this many distinct things or grounds at once, which
/// no bundled map comes close to).
const POOL: &str =
    "0123456789abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ!@#$%^&*()[]{};:'\"\\|/~`+=_<>";

impl CharPool {
    /// `preferred` is the glyph `style_cell` would actually draw for `description`; used as is
    /// unless this response has already given it to a different description, in which case the
    /// next free pool character stands in instead.
    fn char_for(&mut self, description: &str, preferred: char) -> char {
        if let Some(&ch) = self.by_description.get(description) {
            return ch;
        }
        let taken = |ch: char| self.legend.contains_key(&ch.to_string());
        let ch = if !taken(preferred) {
            preferred
        } else {
            POOL.chars().find(|&c| !taken(c)).unwrap_or('?')
        };
        self.by_description.insert(description.to_owned(), ch);
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
    Build(BuildScreenView),
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

/// `screen {"build"}` (#327): the Build modal's own list (`fn_ui::build::picker_rows`), in the
/// same order it draws them.
#[derive(Clone, Debug, Serialize)]
pub struct BuildScreenView {
    pub rows: Vec<BuildRowView>,
}

#[derive(Clone, Debug, Serialize)]
pub struct BuildRowView {
    pub name: String,
    /// "1x1" or "2x2" (`BuildingDef::footprint`): what `build {building, from, to}`'s rectangle
    /// has to cover.
    pub size: String,
    /// The modal's own list text for this row: the real cost ("5 Wood") when it can be built,
    /// or "Locked" when the nation hasn't unlocked it yet (`crate::build::list_row`).
    pub cost: String,
    pub locked: bool,
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

    /// Every entry as a [`LogLine`], in the order [`Host`] made them (not including the header,
    /// which is [`Host::header`]): the shape a run log's lines after the first take
    /// (docs/design/agent-play.md, "Racing": "Run log"), that `fn_tui --watch` (#319) reads back
    /// and `fn_agent serve` (#317, #324) writes as it goes (`fn_agent::tools::Tools` diffs
    /// against this on every tool call, so it never has its own copy of what to log).
    pub fn lines(&self) -> Vec<LogLine> {
        self.entries.iter().map(LogLine::from).collect()
    }

    /// The whole run log as JSON Lines: `header`'s own line, then [`Self::lines`], one JSON
    /// object a line. What a watcher reads (`fn_tui --watch`, #319), and the shape `fn_agent
    /// serve` (#317, #324) writes incrementally rather than all at once.
    pub fn to_jsonl(&self, header: &RunLogHeader) -> String {
        let mut out = String::new();
        out.push_str(&serde_json::to_string(&LogLine::Header(header.clone())).unwrap());
        out.push('\n');
        for line in self.lines() {
            out.push_str(&serde_json::to_string(&line).unwrap());
            out.push('\n');
        }
        out
    }
}

/// Enough of how a [`Host`] session started to rebuild it: [`Host::new`]'s own arguments, plus
/// how many game seconds (and ticks) one `end_turn` turn covers, and what the run was made
/// against. The first line of a run log (docs/design/agent-play.md, "Racing": "Run log"), read
/// by `fn_tui --watch` (#319) and `fn_agent keys`/`verify` (#317/#318) to build the same session
/// a live `Host` started from. `data_digest` and `crate_version` are not needed to rebuild the
/// session (a fresh one always uses whatever data and code are at hand), only to tell a reader
/// whether it still can: `--watch` refuses a log whose `data_digest` doesn't match
/// (`Watcher::new`), the same rule `fn_sim::save` applies to a save file.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct RunLogHeader {
    pub seed: u64,
    pub nation: Nation,
    pub turn_seconds: u32,
    /// How many ticks one `end_turn` turn runs (`turn_seconds * TICKS_PER_SECOND`): what turns a
    /// `Turn` line's `index` back into a `<tick N>` a key script would use (`fn_agent keys`,
    /// docs/design/agent-play.md, "Racing"), without a reader having to know `TICKS_PER_SECOND`
    /// itself.
    pub ticks_per_turn: u32,
    pub map_size: Option<String>,
    pub difficulty: Option<String>,
    pub site: Option<(i32, i32)>,
    pub rough: bool,
    /// `Defs::id_hash`: a stable hash of the bundled data's id tables this run was made against
    /// (`Host::data_digest`'s own doc comment). A replay only holds for the same data.
    pub data_digest: u64,
    /// `env!("CARGO_PKG_VERSION")` at the time this run was made: the other half of "replays only
    /// hold for the same code and data".
    pub crate_version: String,
}

/// One line of a run log file, after the first ([`RunLogHeader`]'s own line): the JSON shape
/// `fn_agent serve` (#317, #324) writes, one object a line, flushed as it goes, and `fn_tui
/// --watch` (#319) reads back -- the one schema both sides use, so they can't drift apart.
/// `keys` is the pressed keys as a `--keys` script (`key_token`'s syntax, parsed back by
/// `crate::keys::parse`), not the raw [`KeyEvent`]s, since those have no `serde` of their own
/// and a script is what a human watching -- or re-running `fn_agent keys` -- wants to read
/// anyway. `End` (#327) is the last line a finished run's log ever gets: `fn_agent serve` writes
/// one when its session ends (stdin closed, or an error), with the world's own tick at that
/// moment and the reason, so `fn_tui --watch` can tell a run that's finished from one that's
/// merely caught up with a live agent still thinking.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum LogLine {
    Header(RunLogHeader),
    Keys { keys: String, label: Option<String>, note: Option<String> },
    Look { x: i32, y: i32, w: u16, h: u16 },
    Inspect { x: i32, y: i32 },
    Turn { index: u32, tick: u64 },
    End { tick: u64, reason: String },
}

impl From<&LogEntry> for LogLine {
    fn from(entry: &LogEntry) -> Self {
        match entry {
            LogEntry::Keys { keys, label, note } => LogLine::Keys {
                keys: keys.iter().map(|&k| key_token(k)).collect::<Vec<_>>().join(" "),
                label: label.clone(),
                note: note.clone(),
            },
            LogEntry::Look { x, y, w, h } => LogLine::Look { x: *x, y: *y, w: *w, h: *h },
            LogEntry::Inspect { x, y } => LogLine::Inspect { x: *x, y: *y },
            LogEntry::Turn { index, tick } => LogLine::Turn { index: *index, tick: *tick },
        }
    }
}

/// The `--keys` token for `key` (the inverse of `keys::named_key`/`char_key`): round-trips every
/// key [`Host::press_keys`] can actually send, since those all came from `keys::parse` in the
/// first place.
pub(crate) fn key_token(key: KeyEvent) -> String {
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
    use crate::state::{BANNER_HEIGHT, BAR_HEIGHT};

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
            fast: false,
            watch: None,
            pace: None,
            at: None,
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
    /// (`Enter` on any of them), while Resume and Key help still work. An agent can never open
    /// the menu itself (#327), so this drives `App::key` directly, as if the menu were already
    /// open (a loaded save can start mid-menu), to check the rows alone.
    #[test]
    fn the_esc_menus_save_load_and_quit_rows_are_refused() {
        let mut h = host(Nation::Earth);
        h.app_mut().key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
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

    /// A bare `Esc` with nothing else open would, for a human, open the game menu and pause
    /// the clock; for an agent it's refused outright instead (#327), a no-op that changes
    /// nothing -- the menu is only a trap for an agent (every row but Resume and Key help is
    /// already refused above).
    #[test]
    fn a_bare_esc_with_nothing_open_is_refused_and_does_not_open_the_menu() {
        let mut h = host(Nation::Earth);
        let paused_before = h.app().session.clock.paused;
        let before = h.frame(40, 20);
        let result = h.press_keys("<esc>", None, None).unwrap();
        assert_eq!(result, PressResult { pressed: 0, refused: 1 });
        assert_eq!(h.app().ui.modal, None);
        assert_eq!(h.app().session.clock.paused, paused_before);
        assert_eq!(h.frame(40, 20), before);
    }

    /// `act`'s `keys` step refuses the same bare `Esc` `press_keys` does (#327): planned through
    /// `plan_keys` but then skipped by the same `refuses` check `act` already runs every
    /// pressed key through.
    #[test]
    fn act_keys_also_refuses_a_bare_esc_that_would_open_the_menu() {
        let mut h = host(Nation::Earth);
        let steps = vec![ActionStep::Keys { script: "<esc>".into() }];
        let result = h.act(&steps, None);
        assert!(result.error.is_none(), "{result:?}");
        assert_eq!(result.keys, Vec::<String>::new());
        assert_eq!(h.app().ui.modal, None);
    }

    /// Once a tool has a corner set, or the Select bar has something selected, `Esc` still does
    /// its ordinary job instead of being refused: only the no-op case (nothing left to close)
    /// is special.
    #[test]
    fn esc_still_works_when_there_is_something_to_close() {
        let mut h = host(Nation::Earth);
        h.press_keys("h", None, None).unwrap(); // Harvest tool, no corner yet
        let result = h.press_keys("<esc>", None, None).unwrap();
        assert_eq!(result, PressResult { pressed: 1, refused: 0 });
        assert_eq!(h.observe().tool, "Select", "Esc with no corner leaves the tool");
        assert_eq!(h.app().ui.modal, None);
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
        for name in ["nation", "research", "endeavor", "log", "help", "build"] {
            h.screen(name).unwrap();
        }
        assert!(h.screen("bogus").is_err());
    }

    /// `screen {"build"}` is the Build modal's own list (#327): every row's name, footprint
    /// size, and the modal's own list text ("5 Wood" when it can be built, "Locked" otherwise).
    #[test]
    fn screen_build_matches_the_build_modals_own_rows() {
        let h = host(Nation::Earth);
        let ScreenView::Build(view) = h.screen("build").unwrap() else { panic!("not Build") };
        let rows = fn_ui::build::picker_rows(&h.app().session);
        assert_eq!(view.rows.len(), rows.len());
        for (got, row) in view.rows.iter().zip(&rows) {
            assert_eq!(got.name, row.name);
            assert_eq!(got.locked, row.locked());
            let (w, hh) = h.app().session.defs.building(row.id).footprint;
            assert_eq!(got.size, format!("{w}x{hh}"));
            if row.locked() {
                assert_eq!(got.cost, "Locked");
            } else {
                assert_eq!(got.cost, row.cost);
            }
        }
        // A new game always has at least one buildable thing (a bed).
        assert!(view.rows.iter().any(|r| r.name == "Bed" && !r.locked));
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
            fast: false,
            watch: None,
            pace: None,
            at: None,
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

    /// #327: the real bug a first agent run hit -- a stockpile placed on water stored nothing,
    /// and the agent didn't notice for turns because a second designate in the same `act` call
    /// overwrote `observe().notice` with its own. `act`'s own `notices` keeps every step's own
    /// notice, so the water stockpile's "0 done" is still there even once the harvest step after
    /// it has replaced the UI's one live notice.
    #[test]
    fn act_keeps_each_designates_own_notice_even_once_a_later_step_overwrites_the_uis_one() {
        let mut h = host(Nation::Earth);
        let water = water_tile(&h);
        let cursor = h.app().ui.cursor;
        let land_from = Pos::new(cursor.x + 2, cursor.y + 1);
        let land_to = Pos::new(land_from.x + 1, land_from.y + 1);
        let steps = vec![
            ActionStep::Designate {
                tool: DesignateTool::Stockpile,
                from: Coord { x: water.x, y: water.y },
                to: Coord { x: water.x, y: water.y },
                standing: false,
            },
            ActionStep::Designate {
                tool: DesignateTool::Harvest,
                from: Coord { x: land_from.x, y: land_from.y },
                to: Coord { x: land_to.x, y: land_to.y },
                standing: false,
            },
        ];
        let result = h.act(&steps, None);
        assert!(result.error.is_none(), "{result:?}");
        assert_eq!(result.notices.len(), 2, "{:?}", result.notices);
        assert_eq!(result.notices[0].step, 0);
        assert!(
            result.notices[0].notice.starts_with("designate stockpile: 0 done, 1 skipped"),
            "{}",
            result.notices[0].notice
        );
        assert_eq!(result.notices[1].step, 1);
        assert!(
            result.notices[1].notice.starts_with("designate harvest"),
            "{}",
            result.notices[1].notice
        );
        // The live UI notice (what a bare `observe` would have shown) is only the second step's
        // own -- the first step's failure would otherwise have gone unseen, #327's own bug.
        assert_eq!(h.observe().notice, result.notices[1].notice);
        assert_ne!(h.observe().notice, result.notices[0].notice);
    }

    /// A water tile, for a designate that the sim refuses (#327's own stockpile-on-water bug).
    fn water_tile(h: &Host) -> Pos {
        h.app()
            .session
            .world
            .map
            .iter()
            .find(|(_, t)| t.terrain == fn_sim::Terrain::Water)
            .map(|(p, _)| p)
            .expect("a new map always has water")
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

    /// The run log's JSON Lines shape (#319's acceptance criteria rely on this): a header line,
    /// then one line an entry, each round-tripping through `serde_json`, with `keys` back as a
    /// `--keys` script `crate::keys::parse` accepts.
    #[test]
    fn to_jsonl_round_trips_through_serde_json() {
        let mut h = host(Nation::Earth).with_turn_seconds(12);
        h.press_keys("h <right*3> <enter>", Some("harvest".into()), Some("wood".into())).unwrap();
        h.end_turn(1);
        h.look(1, 1, 4, 4);
        h.inspect(2, 2);

        let header = h.header();
        assert_eq!(
            header,
            RunLogHeader {
                seed: 7,
                nation: Nation::Earth,
                turn_seconds: 12,
                ticks_per_turn: 12 * TICKS_PER_SECOND,
                map_size: None,
                difficulty: None,
                site: None,
                rough: false,
                data_digest: h.data_digest(),
                crate_version: env!("CARGO_PKG_VERSION").to_owned(),
            }
        );

        let text = h.log().to_jsonl(&header);
        let mut lines = text.lines();
        let first: LogLine = serde_json::from_str(lines.next().unwrap()).unwrap();
        assert_eq!(first, LogLine::Header(header));

        let rest: Vec<LogLine> =
            lines.map(|l| serde_json::from_str(l).unwrap()).collect::<Vec<_>>();
        assert_eq!(rest, h.log().lines());
        assert_eq!(rest.len(), 4, "keys, turn, look, inspect");
        let LogLine::Keys { keys, label, note } = &rest[0] else { panic!("{:?}", rest[0]) };
        assert_eq!(keys::parse(keys).unwrap().len(), 5, "h <right*3> <enter>");
        assert_eq!(label.as_deref(), Some("harvest"));
        assert_eq!(note.as_deref(), Some("wood"));
        assert!(matches!(rest[1], LogLine::Turn { index: 1, .. }));
        assert_eq!(rest[2], LogLine::Look { x: 1, y: 1, w: 4, h: 4 });
        assert_eq!(rest[3], LogLine::Inspect { x: 2, y: 2 });
    }

    // ----------------------------------------------------------------------------------------
    // Parity: map-cell equality (#320's acceptance criteria)
    // ----------------------------------------------------------------------------------------

    /// A seed-7 game past its opening moments, a settler selected and the Build tool active
    /// with a building already chosen (its ghost on the map, not just its banner) -- one golden
    /// scenario per nation, the richest ordinary state `observe`, `screen` and `look` all have
    /// something to say about (docs/design/agent-play.md, "Enforcing and testing parity").
    /// Bounded to a few dozen turns, cheap enough for `ci --fast` (#291).
    fn mid_game(nation: Nation) -> Host {
        let mut h = host(nation);
        for _ in 0..20 {
            h.end_turn(50);
        }
        let name = h.app().session.world.units[0].name.clone();
        let select = h.act(&[ActionStep::Select { x: None, y: None, name: Some(name) }], None);
        assert!(select.error.is_none(), "{select:?}");
        let build = h.press_keys("u <enter>", None, None).unwrap();
        assert_eq!(build.refused, 0, "u and <enter> are never refused");
        assert_eq!(h.app().ui.tool, Tool::Build, "the Build tool is active");
        h
    }

    /// `Host::look`'s `things` glyph at every cell is the very glyph `--ascii` draws there: a
    /// mark never changes a cell's glyph, only its colour and modifier (`map`'s own doc
    /// comment), so this holds everywhere in the visible view, not just the plain tiles.
    #[test]
    fn looks_things_glyph_equals_the_ascii_frames_glyph_for_every_nation() {
        // 80x24: within `MAX_LOOK`, so the whole map region in one `look` lines up with the
        // whole map region of one `frame`.
        const SIZE: (u16, u16) = (80, 24);
        for nation in Nation::ALL {
            let mut h = mid_game(nation);
            let frame = h.frame(SIZE.0, SIZE.1);
            let frame_lines: Vec<&str> = frame.lines().collect();
            let ui = &h.app().ui;
            let (view, (vw, vh)) = (ui.view, ui.viewport());
            // The tool banner (top) and the selection bar (bottom) cover their own rows of the
            // map area with their own text, not a tile glyph (`UiState::visible`'s own doc
            // comment); `mid_game` has both.
            let top = if ui.banner() { BANNER_HEIGHT } else { 0 };
            let bottom = if ui.selected.is_some() { BAR_HEIGHT } else { 0 };
            let rows = (vh - top - bottom).max(1);
            let look = h.look(view.x, view.y + top, vw as u16, rows as u16);
            for (row, thing_line) in look.things.iter().enumerate() {
                let frame_row = top as usize + row;
                let frame_line = frame_lines.get(frame_row).copied().unwrap_or("");
                for (col, ch) in thing_line.chars().enumerate() {
                    let frame_ch = frame_line.chars().nth(col).unwrap_or(' ');
                    if ch == frame_ch {
                        continue;
                    }
                    // A real ascii collision (docs/design/agent-play.md, "Frames and keys":
                    // "ASCII glyphs collide"): this cell's own thing drew in a character this
                    // response's legend had already given to a different thing, so `CharPool`
                    // fell back to another one. Not a parity problem -- the legend still names
                    // this cell correctly -- as long as the frame's own character really is
                    // somebody else's in this legend, not an unrelated mismatch.
                    assert!(
                        look.legend.contains_key(&frame_ch.to_string()),
                        "{nation:?}: world ({},{}) (frame row {frame_row}, col {col}): look \
                         says {ch:?} but the frame says {frame_ch:?}, which isn't any \
                         description in this response's own legend {:?}",
                        view.x + col as i32,
                        view.y + top + row as i32,
                        look.legend,
                    );
                }
            }
        }
    }

    /// Every mark `Host::look` reports changes the look a mono palette (no colour, #320's
    /// "mono-palette modifier") would give its cell: the agent is never told about a mark a
    /// colourless human wouldn't also see some sign of, through bold, underline, reverse video
    /// or dim (`map::marks_effect`).
    #[test]
    fn looks_marks_change_the_mono_palette_look_of_their_cell_for_every_nation() {
        for nation in Nation::ALL {
            let mut h = mid_game(nation);
            // Settles the view at a known size first (`Host::frame`'s own `shell.resize`), so
            // the cursor -- always kept in view -- is inside this `look` region too.
            h.frame(80, 24);
            let (origin, (vw, vh)) = (h.app().ui.view, h.app().ui.viewport());
            let view = h.look(origin.x, origin.y, vw as u16, vh as u16);
            assert!(!view.marks.is_empty(), "{nation:?}: the cursor's own mark always shows");

            let app = h.app();
            let (session, ui) = (&app.session, &app.ui);
            let over = Overlays::gather(session, ui);
            let defs = &session.defs;
            let mono = Palette::new(defs, nation, ColorDepth::Never, true);
            for mark in &view.marks {
                let pos = Pos::new(mark.x, mark.y);
                let cell = cell_look(session, ui, &h.palette, &over, pos)
                    .expect("a mark is only ever reported on a tile that's on the map");
                // The Avatar, and a selected unit, already draw reversed in mono on their own
                // (`thing_look`'s own `mono || *selected` and `*selected` checks), with no mark
                // involved at all. A cursor or ghost mark landing on either can't add anything
                // a mono human would see apart from that -- not a parity problem, just this
                // thing's own look already covering what the mark would say.
                let self_reverses = matches!(cell.thing, Thing::Avatar { .. })
                    || matches!(cell.thing, Thing::Unit { selected: true, .. });
                if self_reverses {
                    continue;
                }
                let plain =
                    CellLook { thing: cell.thing.clone(), ground: cell.ground, marks: Vec::new() };
                let with_marks = style_cell(&cell, defs, &mono);
                let without = style_cell(&plain, defs, &mono);
                assert_ne!(
                    with_marks, without,
                    "{nation:?}: {:?} at ({},{}) should change the mono-palette look",
                    mark.kinds, pos.x, pos.y,
                );
            }
        }
    }
}
