//! The agent surface (#313, #315): `Host` wraps a [`Shell`] built the way `--start` builds one,
//! and gives an agent exactly what a human player has -- reads built from `fn_ui`'s view models,
//! the map's per-cell look (#314) and the renderer, and keys fed to the same `Shell::key` a
//! keyboard feeds. It does no I/O and reads no clock (docs/design/agent-play.md, "Architecture").
//!
//! **Time is lockstep** ([`Host::end_turn`]): the sim stands still between calls, and a turn is
//! a fixed slice of game time (`Host::turn_seconds`, 20 by default), so every call gets the same
//! amount of the world moving. A turn ends early the moment an alert worth an agent's attention
//! lands (`fn_ui::alerts::Entry::worth_stopping`: a death, an arrival, an endeavor event -- which
//! covers #347's "is now a bending master" too -- a finished tech, a season event, or a bend
//! that *failed*; a bend that merely finished is routine and never stops it on its own, #377) or
//! a need flag shows that wasn't already up at the call's start and hasn't shown yet in this
//! same call (a flag that flickers on and off doesn't count twice), which stands in for a
//! human's continuous attention without telling the agent anything the alert log and the side
//! panel don't already say. [`Observation::stopped`] names which.
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
//! leaving the active tool's own key to switch tools (it already does that unconditionally,
//! except `designate {tool: "bend", ability}`, #333: that presses the bend key however many
//! times moves it from whatever ability is active now to the one named, including zero). A
//! step whose plan can't be made (an unknown tech, a building the Build modal doesn't offer, a
//! tile off the map, an unknown or locked bend ability, ...) presses nothing and the steps
//! after it don't run either.
//!
//! **A run log** ([`RunLog`]) remembers every key actually pressed and every turn run, in order,
//! so it can be replayed with `Shell::play` (a seed plus keys is the whole game, as a key script
//! always is) and exported as a `--keys` script (`RunLog::to_keys_script`).

use std::collections::{BTreeMap, BTreeSet};

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use fn_sim::world::TICKS_PER_SECOND;
use fn_sim::{Nation, Pos};
use fn_ui::alerts::{Entry as AlertEntry, Kind as AlertKind};
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
use crate::layout;
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
    /// The persistent event log [`Self::end_turn`] grows every turn it runs: the same alert
    /// kinds that are worth stopping for, mapped to [`EventKind`], plus a need flag the first
    /// time it's ever shown this whole session ([`Self::needs_ever_seen`]). Never rotated or
    /// cleared, so [`Self::events_since`] can always answer "since mark M" for the life of this
    /// `Host` (#478, docs/design/agent-scripting.md, "The trampoline": `events()`).
    events: Vec<Event>,
    /// Every need-flag word [`Self::end_turn`] has ever seen shown, across every call this
    /// session has made (not reset per call, unlike `end_turn`'s own local early-stop `seen`):
    /// the set [`Self::end_turn`] checks before adding a [`EventKind::Need`] event, so a flag
    /// already up when this `Host` was built is never "first shown" again later.
    needs_ever_seen: BTreeSet<String>,
    log: RunLog,
    /// [`Self::new`]'s own arguments, kept only so [`Self::header`] can describe how to rebuild
    /// this session (a watcher, #319, or `fn_agent verify`, #317/#318, start from a header, not
    /// from this live `Host`).
    header: RunLogHeader,
    /// [`Host::look`]'s character assignments, kept for the whole session rather than rebuilt
    /// each call (#332): a description keeps the character it was first given, whatever else is
    /// in a later region, so the legend reads the same call after call instead of depending on
    /// what else that particular region happened to contain.
    char_pool: CharPool,
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
            scenario: None,
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
            prelude: None,
        };
        let mut host = Self {
            shell,
            palette,
            turn_seconds: DEFAULT_TURN_SECONDS,
            turn: 0,
            alerts_since_turn: 0,
            events: Vec::new(),
            needs_ever_seen: BTreeSet::new(),
            log: RunLog::new(seed, nation),
            header,
            char_pool: CharPool::default(),
        };
        // Seeds `needs_ever_seen` from whatever's already flagged the moment this session
        // exists, the same way `end_turn`'s own local `seen` is seeded at the start of a call:
        // a flag already up before any turn has run is never "first shown" later.
        host.needs_ever_seen =
            need_flags(&host.app().session).into_iter().map(|(w, _)| w).collect();
        Ok(host)
    }

    /// Sets how many game seconds one `end_turn` turn covers (20 by default); at least one.
    pub fn with_turn_seconds(mut self, turn_seconds: u32) -> Self {
        self.turn_seconds = turn_seconds.max(1);
        self.header.turn_seconds = self.turn_seconds;
        self.header.ticks_per_turn = self.turn_seconds * TICKS_PER_SECOND;
        self
    }

    /// Runs `prelude` before anyone plays (#481, docs/design/agent-scripting.md, "Scenario
    /// starts"): the deterministic autoplay opening it names, fast-forwarded `prelude.minutes`
    /// game minutes, directly against this session's own world (`prelude::run`). Turn 0 starts
    /// right after, and nothing the prelude logged shows in the first `observe`
    /// (`Self::alerts_since_turn` is reset to skip it) -- a player who starts the same scenario
    /// sees exactly the same screen a fresh `Host::new` never built up to. `Self::header` keeps
    /// `prelude` from here on, so `--watch`, `--resume` and `fn_agent keys` can run it again.
    pub fn with_prelude(mut self, prelude: Prelude) -> Self {
        prelude::run(self.app_mut(), &prelude);
        self.alerts_since_turn = self.app().ui.alerts.history().len();
        self.header.prelude = Some(prelude);
        self
    }

    /// A new agent session from a `data/scenarios.toml` preset (`crate::scenario::find`, #481,
    /// docs/design/agent-scripting.md, "Scenario starts"): the same as [`Self::new`] with the
    /// preset's own seed, nation, map size, difficulty and site, plus [`Self::with_prelude`] if
    /// it names one. `fn_agent serve --scenario ID` and `fn_tui --scenario ID` both go through
    /// this (or `crate::shell::Shell`'s own copy of it), so they start the same world.
    pub fn from_scenario(id: &str) -> Result<Self, String> {
        let scenario = crate::scenario::find(id)?;
        let site = scenario.start()?;
        let host = Self::new(
            scenario.seed,
            scenario.nation,
            scenario.map_size.clone(),
            scenario.difficulty.clone(),
            site,
            false,
        )?;
        Ok(match scenario.prelude {
            Some(prelude) => host.with_prelude(prelude),
            None => host,
        })
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

    /// Rebuilds the session `header` describes (`Self::new`, with `header`'s own
    /// `turn_seconds`), then replays `lines` into it in order -- `Keys` through
    /// [`Self::press_keys`], `Look` through [`Self::look`], `Inspect` through [`Self::inspect`]
    /// and `Turn` as one [`Self::end_turn`] call each (every `Turn` line is already one whole
    /// turn's worth of ticks, never a partial one: [`Self::end_turn`]'s own early-stop only ever
    /// decides whether a *later* turn in the same call runs, not whether this one does) -- so an
    /// agent run can be played as a chain of short sessions, each with a run log the size of one
    /// chunk's own context instead of the whole game's (`fn_agent serve --resume`, #341). `Header`
    /// and `End` lines are skipped: a header only ever makes sense first, and an `End` is a fact
    /// about where a previous session stopped, not something to replay.
    ///
    /// Refuses a log made against different data, the same rule and message
    /// `watch::Watcher::new` applies (#324): a replay only holds for the same data a header's
    /// `Host` was built against. Every other error here is `Self::press_keys`'s own (a line that
    /// somehow doesn't parse as a key script) -- it shouldn't happen to a log this crate wrote
    /// itself, but a corrupted or hand-edited log should fail loudly rather than desync.
    ///
    /// If `header` names a `prelude` (#481), it runs again right after `Self::new`, exactly as
    /// the live `Host` that wrote it did, before any of `lines` replays: `lines` only ever holds
    /// what happened from turn 0 on.
    pub fn resume(header: &RunLogHeader, lines: &[LogLine]) -> Result<Self, String> {
        let mut host = Self::new(
            header.seed,
            header.nation,
            header.map_size.clone(),
            header.difficulty.clone(),
            header.site,
            header.rough,
        )?
        .with_turn_seconds(header.turn_seconds);
        let digest = host.data_digest();
        if digest != header.data_digest {
            return Err(format!(
                "this run log was made against different data (digest {:#x}, the data here is \
                 {digest:#x}); it can't be resumed",
                header.data_digest
            ));
        }
        if let Some(prelude) = header.prelude {
            host = host.with_prelude(prelude);
        }
        for line in lines {
            match line {
                // `Script`, `Wake`, `Note` and `Call` are #479/#497's own commentary (`LogLine`'s
                // own doc comment): nothing a `Host` ever made, so nothing to replay back into
                // one.
                LogLine::Header(_)
                | LogLine::End { .. }
                | LogLine::Script { .. }
                | LogLine::Wake { .. }
                | LogLine::Note { .. }
                | LogLine::Call { .. }
                | LogLine::Plan { .. } => {}
                LogLine::Keys { keys, label, note } => {
                    host.press_keys(keys, label.clone(), note.clone())?;
                }
                LogLine::Look { x, y, w, h } => {
                    host.look(*x, *y, *w, *h);
                }
                LogLine::Inspect { x, y } => {
                    host.inspect(*x, *y);
                }
                LogLine::Turn { .. } => {
                    host.end_turn(1);
                }
            }
        }
        Ok(host)
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
    /// turn in which an alert worth an agent's attention lands -- a death, an arrival, an
    /// endeavor event (including #347's "is now a bending master"), a finished tech, a season
    /// event, or a bend that *failed*; a bend that merely finished is routine and never stops it
    /// on its own, since a settlement that bends a lot would otherwise stop every turn or two
    /// (#377, `fn_ui::alerts::Entry::worth_stopping`) -- or a need flag shows that wasn't
    /// already up when this call started and hasn't shown yet in an earlier turn of this same
    /// call -- so a flag that clears and comes back within the call's own window (thirst easing
    /// off and returning a few turns later, say) doesn't count as new every time it recurs, only
    /// the first time. Returns the observation after whichever turn it stopped on, with
    /// [`Observation::stopped`] naming why (`None` only when every requested turn ran with
    /// nothing to report). Every alert the turns covered -- worth stopping for or not -- is
    /// still in the observation's own `alerts` list either way (`Self::observe`).
    pub fn end_turn(&mut self, turns: u32) -> Observation {
        // Everything already flagged when the call started never counts as new; `first_new_flag`
        // folds in every turn's own flags too, so only something genuinely unseen in this
        // call's whole window can stop it.
        let mut seen: BTreeSet<String> =
            need_flags(&self.app().session).into_iter().map(|(w, _)| w).collect();
        let mut stopped = None;
        for _ in 0..turns {
            let before_alerts = self.app().ui.alerts.history().len();
            let ticks = self.turn_seconds * TICKS_PER_SECOND;
            self.app_mut().run_ticks(ticks);
            self.turn += 1;
            let tick = self.app().session.world.tick;
            self.log.push(LogEntry::Turn { index: self.turn, tick });
            let after_alerts = self.app().ui.alerts.history().len();
            self.alerts_since_turn = after_alerts;
            if after_alerts > before_alerts {
                let history = self.app().ui.alerts.history();
                let fresh = &history[before_alerts..after_alerts];
                let worth: Vec<&str> =
                    fresh.iter().filter(|e| e.worth_stopping()).map(|e| e.text.as_str()).collect();
                let new_events: Vec<Event> = fresh
                    .iter()
                    .filter_map(|entry| {
                        event_kind_for_alert(entry).map(|kind| Event {
                            turn: self.turn,
                            kind,
                            text: entry.text.clone(),
                        })
                    })
                    .collect();
                let stop_now = !worth.is_empty();
                if stop_now {
                    stopped = Some(format!("new alert: {}", worth.join("; ")));
                }
                self.events.extend(new_events);
                if stop_now {
                    break;
                }
            }
            let now_needs = need_flags(&self.app().session);
            for (word, _) in &now_needs {
                if self.needs_ever_seen.insert(word.clone()) {
                    self.events.push(Event {
                        turn: self.turn,
                        kind: EventKind::Need,
                        text: format!("need flag: {word} first shown"),
                    });
                }
            }
            if let Some(flag) = first_new_flag(&mut seen, now_needs) {
                stopped = Some(format!("need flag: {flag} first shown"));
                break;
            }
        }
        let mut obs = self.observe();
        obs.stopped = stopped;
        obs
    }

    /// The persistent event log's own current length (`Self::events`): a script run's starting
    /// mark (docs/design/agent-scripting.md, "`run_script`": `events()`), captured once when
    /// the run begins so [`Self::events_since`] can later answer "since the script started".
    pub fn event_mark(&self) -> usize {
        self.events.len()
    }

    /// Every event [`Self::end_turn`] has logged since `mark` (`Self::event_mark`), oldest
    /// first: the same categories `Self::end_turn`'s own doc comment stops for, turn-tagged.
    pub fn events_since(&self, mark: usize) -> Vec<Event> {
        self.events[mark.min(self.events.len())..].to_vec()
    }

    // --------------------------------------------------------------------------------------
    // Reads
    // --------------------------------------------------------------------------------------

    /// Turn, clock, season, tool, notice, people (activity counts and need flags), research,
    /// stores, stockpile capacity, endeavor, the cursor's own tile and alerts since the last
    /// turn: everything `side_view`, `stockpile_capacity` and the alert log already say, free,
    /// changing nothing and taking no game time.
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
                // The panel's compact Research row (#416, `panel::research_row`) reads
                // "Research: none set" with no target, not the plain internal label; with a
                // target, the tech's own name still shows (the bar row's own Strong text).
                label: if side.research.label == "No target set" {
                    "Research: none set".to_owned()
                } else {
                    side.research.label
                },
                // The row's short shape (target, bar, percent) drops the points pool and the
                // rate from the panel entirely, so there's no on-screen text left to echo here
                // (#320, "Observation containment": an agent must never see what a player
                // watching the screen couldn't).
                pool: String::new(),
                rate: String::new(),
            },
            stores: side.stores.into_iter().map(StoreView::from).collect(),
            stockpile: fn_ui::view::stockpile_capacity(session).into(),
            endeavor: side.endeavor.map(|e| {
                // The same text the Gate row carries (`panel::gate_row`, #416): `e.line` on its
                // own ("Signs met - G: open", the Ready text, or the stage) only once nothing is
                // missing; otherwise the first missing sign's short count ("10 people"), so this
                // never claims signs are met while `next` still lists one (#320, "Observation
                // containment").
                let line = match e.next.first() {
                    Some(next) => next.clone(),
                    None => e.line,
                };
                // The Gate row only ever names the first missing sign; the rest of `e.next`
                // (`fn_ui::endeavor::NEXT_SHOWN`'s headroom for a future comma-joined row) isn't
                // shown anywhere yet, so `next` stops at one too (#320, "Observation
                // containment"; a richer row is a TUI change first, docs/design/agent-play.md,
                // "The parity rule").
                let next = e.next.into_iter().take(1).collect();
                EndeavorView { name: e.name, count: e.count, line, next }
            }),
            cursor: self.cursor_view(ui.cursor),
            alerts,
            stopped: None,
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
    /// `legend` is this response's own subset of [`Self::char_pool`]'s session-long assignment
    /// (#332): one character per distinct thing or ground actually present, so there are no
    /// collisions within it (docs/design/agent-play.md, "The map") -- the real glyph when
    /// another description hasn't already claimed it this session, a fallback character on the
    /// rare ascii collision (docs/design/agent-play.md, "Frames and keys": "F, *, $, %, = and S
    /// each mean two or more things") -- and, once given, a description's character never
    /// changes for the rest of the session, whatever else later shares a region with it.
    pub fn look(&mut self, x: i32, y: i32, w: u16, h: u16) -> MapView {
        let (w, h) = (w.clamp(1, MAX_LOOK.0), h.clamp(1, MAX_LOOK.1));
        // Taken out of `self` so the loop below can hold `self.app()`'s borrow (`session`,
        // `ui`, `defs`) and still assign characters through it; put back before returning.
        let mut pool = std::mem::take(&mut self.char_pool);
        let app = self.app();
        let (session, ui) = (&app.session, &app.ui);
        let defs = &session.defs;
        let over = Overlays::gather(session, ui);
        let mut legend = BTreeMap::new();
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
                        let thing_desc = describe_thing(session, &cell.thing, pos);
                        let thing_ch = pool.char_for(&thing_desc, thing_glyph);
                        legend.insert(thing_ch.to_string(), thing_desc);
                        thing_line.push(thing_ch);
                        let ground_desc = describe_ground(session, cell.ground);
                        let ground_ch = pool.char_for(&ground_desc, ground_glyph);
                        legend.insert(ground_ch.to_string(), ground_desc);
                        ground_line.push(ground_ch);
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
        self.char_pool = pool;
        self.log.push(LogEntry::Look { x, y, w, h });
        MapView { x, y, w, h, things, ground, legend, marks }
    }

    /// A map region, at most [`MAX_LOOK`], as one `#{x, y, thing, ground, marks}` per on-map
    /// cell (#480, docs/design/agent-scripting.md, "Plumbing: 1:1 with `Host`"): `thing` and
    /// `ground` are the same descriptions [`Host::look`]'s own legend would decode its glyph
    /// to, and `marks` the same kind names, so a script can read a region without parsing a
    /// legend first -- the porcelain layer's own `find` is built on this. Unlike [`Host::look`],
    /// this takes no `&mut self` and logs nothing: it draws no attention ghost (docs/design/
    /// agent-scripting.md, "Plumbing": "script reads don't draw attention ghosts"), since the
    /// porcelain calls it once per loop iteration and a per-call ghost would outline the whole
    /// search area every turn. A cell off the map is simply missing from the result, not a
    /// blank entry.
    pub fn cells(&self, x: i32, y: i32, w: u16, h: u16) -> Vec<CellView> {
        let (w, h) = (w.clamp(1, MAX_LOOK.0), h.clamp(1, MAX_LOOK.1));
        let app = self.app();
        let (session, ui) = (&app.session, &app.ui);
        let over = Overlays::gather(session, ui);
        let mut out = Vec::with_capacity((w as usize) * (h as usize));
        for row in 0..h {
            for col in 0..w {
                let pos = Pos::new(x + i32::from(col), y + i32::from(row));
                let Some(cell) = cell_look(session, ui, &self.palette, &over, pos) else {
                    continue;
                };
                let thing = describe_thing(session, &cell.thing, pos);
                let ground = describe_ground(session, cell.ground);
                let marks = cell.marks.iter().map(describe_mark).collect();
                out.push(CellView { x: pos.x, y: pos.y, thing, ground, marks });
            }
        }
        out
    }

    /// Whether `building` could be designated with its anchor (top-left tile) at `(x, y)`
    /// (#509): `#{ok, reason}`, read-only -- it presses no keys, writes no `keys` line in the
    /// run log, and changes nothing in the world. Reuses the Build tool's own confirm check
    /// (`fn_ui::build::placement_at`, `World::check_build`) rather than a second copy of it, so
    /// this can never say a spot works when the real `build` step would refuse it, or the other
    /// way round. `building` not in the Build modal's own list, or there but locked, gives
    /// `ok: false` with a reason naming which, the same way [`Self::plan_build`] does for an
    /// actual `build` step; a tile off the map does too.
    pub fn can_build(&self, building: &str, x: i32, y: i32) -> CanBuildView {
        let session = &self.app().session;
        let rows = fn_ui::build::picker_rows(session);
        let Some(row) = rows.iter().find(|r| r.name == building) else {
            return CanBuildView {
                ok: false,
                reason: Some(format!("no such building {building:?} in the Build modal")),
            };
        };
        if let Some(reason) = &row.requires {
            return CanBuildView { ok: false, reason: Some(reason.clone()) };
        }
        let anchor = Pos::new(x, y);
        if !self.in_bounds(anchor) {
            return CanBuildView { ok: false, reason: Some(format!("({x},{y}) isn't on the map")) };
        }
        let placement = fn_ui::build::placement_at(session, anchor, row.id);
        match placement.refused {
            None => CanBuildView { ok: true, reason: None },
            Some(err) => {
                let reason = fn_ui::build::describe_build_error(session, err);
                CanBuildView { ok: false, reason: Some(reason) }
            }
        }
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
                            needs: c.needs,
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
                            closest: s.closest,
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
    /// does; changes nothing else and takes no game time. `w`/`h` are clamped up to
    /// [`layout::MIN_WIDTH`]/[`layout::MIN_HEIGHT`] first (#334), so a size the renderer can't
    /// lay out ever renders the game at the smallest size it does support instead of just
    /// `draw::too_small_text`'s own one-line notice -- [`Self::frame_size`] tells a caller when
    /// that happened.
    pub fn frame(&mut self, w: u16, h: u16) -> String {
        let buf = render_shell(&mut self.shell, &self.palette, Self::clamp_frame_size(w, h));
        to_text(&buf)
    }

    /// The size [`Self::frame`] actually renders at for a `w`/`h` request: both clamped up to
    /// the renderer's own minimum ([`layout::MIN_WIDTH`]/[`layout::MIN_HEIGHT`]).
    pub fn clamp_frame_size(w: u16, h: u16) -> (u16, u16) {
        (w.max(layout::MIN_WIDTH), h.max(layout::MIN_HEIGHT))
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
    /// steps. Only a `designate` or `build` step is ever recorded: a `confirm` always
    /// overwrites `UiState::notice` with that step's own fresh result (never a stale one,
    /// whatever ran before it), but every other step kind (`select`, `research`, `cursor`,
    /// `keys`) never touches `UiState::notice` at all, so reading it after one of those would
    /// only ever surface whatever an earlier designate or build call (in this `act` or a
    /// previous one) left there (#334) -- not this step's own notice, since it set none.
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
                    if matches!(step, ActionStep::Designate { .. } | ActionStep::Build { .. }) {
                        let notice = self.app().ui.notice.clone();
                        if !notice.is_empty() {
                            notices.push(StepNotice { step: index, notice });
                        }
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
            ActionStep::Designate { tool, from, to, standing, teardown, ability } => self
                .plan_designate(
                    *tool,
                    from.pos(),
                    to.pos(),
                    *standing,
                    *teardown,
                    ability.as_deref(),
                ),
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
    /// reach the game otherwise), the tool's letter (or, for `tool: "bend"` with an `ability`,
    /// however many `B` presses that ability needs, [`Self::bend_ability_keys`]), `o` for a
    /// standing order or (#335) a teardown cancel, a cursor walk to `from` (Shift for runs of
    /// 10), Enter, a walk to `to`, Enter.
    fn plan_designate(
        &self,
        tool: DesignateTool,
        from: Pos,
        to: Pos,
        standing: bool,
        teardown: bool,
        ability: Option<&str>,
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
        if teardown && tool != DesignateTool::Cancel {
            return Err("teardown is only for the cancel tool".into());
        }
        if ability.is_some() && tool != DesignateTool::Bend {
            return Err("ability is only for the bend tool".into());
        }
        let mut keys = self.close_overlay_keys();
        match ability {
            Some(name) => keys.extend(self.bend_ability_keys(name)?),
            None => {
                let letter = tool
                    .tool()
                    .letter()
                    .expect("every designate tool has a letter")
                    .to_ascii_lowercase();
                keys.push(char_key(letter));
            }
        }
        if standing || teardown {
            keys.push(char_key('o'));
        }
        keys.extend(walk_keys(self.app().ui.cursor, from));
        keys.push(enter_key());
        keys.extend(walk_keys(from, to));
        keys.push(enter_key());
        Ok(keys)
    }

    /// How many `B` presses move the Bend tool from whatever ability is active now to `name`
    /// (the ability's own display name, as the Bend tool's banner shows it, e.g. "Bore Tunnel"),
    /// for `plan_designate`'s own `tool: "bend"` with an `ability` (#333): one press switches
    /// into the Bend tool without changing which ability is chosen
    /// (`fn_ui::tools::cycle_ability`'s own rule), so that press is only included if the Bend
    /// tool isn't already active; every press after that moves on to the next ability
    /// `fn_ui::tools::usable_abilities` lists, wrapping, so the count is exactly the distance (in
    /// that order) from the one active now. An unknown name, or one the settlement can't use yet
    /// (not researched, or the wrong rank), is a planning error: nothing is pressed for it, same
    /// as any other step that can't be planned.
    fn bend_ability_keys(&self, name: &str) -> Result<Vec<KeyEvent>, String> {
        let session = &self.app().session;
        let id = session
            .defs
            .abilities
            .iter()
            .position(|a| a.name == name)
            .map(|i| fn_sim::bending::AbilityId(i as u16))
            .ok_or_else(|| format!("no such ability {name:?}"))?;
        let usable = fn_ui::tools::usable_abilities(session);
        let target = usable
            .iter()
            .position(|&a| a == id)
            .ok_or_else(|| format!("{name} isn't usable yet"))?;
        let start = fn_ui::tools::bend_ability(session)
            .and_then(|a| usable.iter().position(|&u| u == a))
            .unwrap_or(0);
        let distance = (target + usable.len() - start) % usable.len();
        let switch = usize::from(self.app().ui.tool != Tool::Bend);
        Ok(std::iter::repeat_n(char_key('b'), switch + distance).collect())
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

// ------------------------------------------------------------------------------------------
// Scenario starts (#481, docs/design/agent-scripting.md, "Scenario starts")
// ------------------------------------------------------------------------------------------

/// A scenario preset's prelude (`crate::scenario::Scenario`): a deterministic autoplay opening,
/// run for `minutes` game minutes before anyone plays. Carried on [`RunLogHeader`] so `--watch`,
/// `--resume` and `fn_agent keys` can rebuild the same start a live [`Host::with_prelude`] ran.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Prelude {
    pub opening: PreludeOpening,
    /// Game minutes the opening runs for, before turn 0.
    pub minutes: f32,
}

/// Which `fn_sim::autoplay` opening a [`Prelude`] runs.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PreludeOpening {
    /// `fn_sim::autoplay::Economy`: plays toward the Harmony Gate (research, stone and ore,
    /// buildings, farms, bending) -- the only kind a bundled `data/scenarios.toml` preset uses
    /// today.
    Economy,
    /// The plainer `fn_sim::autoplay::Opening`: beds, a campfire and a short research queue.
    Opening,
}

/// Runs a scenario's prelude directly on a session's own [`fn_sim::World`] (module doc,
/// "Scenario starts"): the one span outside [`Host::end_turn`] this file lets touch `World`
/// directly, allow-listed by the parity lint (#320) the same way `end_turn` and a future
/// `referee` module are, since it's the other place that applies `Command`s and runs ticks --
/// before an agent, or a player, ever gets to see the game. [`Host::with_prelude`] calls this,
/// and so does `crate::shell::Shell`'s own `--scenario` path, so a human and an agent reach
/// exactly the same start.
pub(crate) mod prelude {
    use fn_sim::autoplay::{Economy, Opening};
    use fn_sim::world::TICKS_PER_SECOND;
    use fn_ui::{STARTING_UNITS, Session};

    use super::{App, Prelude, PreludeOpening};

    /// More than every `fn_ui::alerts::Kind::toast_seconds` (10, today): the `dt` `Self::run`
    /// ages every alert by right after the prelude, so whatever it logged is already expired as
    /// a toast the instant it becomes history -- a scenario's prelude is the settlement's past,
    /// not something anyone just watched happen.
    const CATCH_UP_DT: f32 = 15.0;

    /// Issues `prelude`'s opening every `Opening::due` tick, for `prelude.minutes` game minutes,
    /// directly against `app.session`'s own world -- the same loop `fn_sim::balance` and
    /// `tests/balance.rs` drive by hand -- with the same fixed starting headcount every new
    /// game gets (`fn_ui::STARTING_UNITS`). Settles once at the end (`App::settle`, with
    /// `CATCH_UP_DT`): without it, the first real `end_turn` or frame would be the first time
    /// `ui.alerts` ever saw the world, and would read the whole prelude's backlog as alerts
    /// that just happened -- including ones `end_turn`'s own early-stop treats as worth
    /// stopping for.
    pub(crate) fn run(app: &mut App, prelude: &Prelude) {
        let total_ticks = (prelude.minutes * 60.0 * TICKS_PER_SECOND as f32).round() as u64;
        for tick in 0..total_ticks {
            if Opening::due(tick) {
                step(&mut app.session, prelude.opening);
            }
            app.session.world.tick(&app.session.defs);
        }
        app.settle(CATCH_UP_DT);
    }

    fn step(session: &mut Session, opening: PreludeOpening) {
        match opening {
            PreludeOpening::Economy => {
                Economy::new(STARTING_UNITS).step(&mut session.world, &session.defs);
            }
            PreludeOpening::Opening => {
                Opening::new(STARTING_UNITS).step(&mut session.world, &session.defs);
            }
        }
    }
}

/// The first of `now`'s need words that isn't already in `seen` ([`Host::end_turn`]'s own doc
/// comment, #331): the thing that stops an `end_turn` call, if anything does. Every word in
/// `now` is folded into `seen` regardless, so a flag that clears and later shows again within
/// the same call can't count as new a second time -- only the call's very first sight of it
/// can. A pure, standalone function (no `Session` or ticks) so a test can drive the exact rule
/// with a flag's own flicker, one turn's worth of `need_flags` output at a time, without
/// needing the AI to actually clear and re-trip a real need on a schedule.
fn first_new_flag(seen: &mut BTreeSet<String>, now: Vec<(String, usize)>) -> Option<String> {
    let flag = now.iter().find(|(w, _)| !seen.contains(w)).map(|(w, _)| w.clone());
    seen.extend(now.into_iter().map(|(w, _)| w));
    flag
}

/// `entry`'s own [`EventKind`], or `None` for a kind [`Host`]'s persistent event log never
/// carries: reusing [`AlertEntry::worth_stopping`] as the single source of truth for "worth an
/// agent's attention" means a finished `Bend`, `Agent` (`--watch` replay only) and `BendTip`
/// (a gentle, one-time onboarding nudge, #382) are all `None` here exactly because they're
/// `false` there -- the only `Bend` this ever sees `true` for is a failed one (`worth_stopping`'s
/// own doc comment), so it alone becomes [`EventKind::BendFailed`].
fn event_kind_for_alert(entry: &AlertEntry) -> Option<EventKind> {
    if !entry.worth_stopping() {
        return None;
    }
    match entry.kind {
        AlertKind::Death => Some(EventKind::Death),
        AlertKind::Arrival => Some(EventKind::Arrival),
        AlertKind::Tech => Some(EventKind::Tech),
        AlertKind::Endeavor => Some(EventKind::Endeavor),
        AlertKind::Season => Some(EventKind::Season),
        AlertKind::Stockpile => Some(EventKind::Stockpile),
        AlertKind::Bend => Some(EventKind::BendFailed),
        AlertKind::Agent | AlertKind::BendTip => None,
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
    /// `{tool, from, to, standing?, teardown?, ability?}`: a rectangle designation. `standing`
    /// (default `false`) is only for `tool: "harvest"`; `teardown` (#335, default `false`) is
    /// only for `tool: "cancel"` -- without it, a cancel withdraws only job designations (a
    /// harvest, a bend, a Sow or a Clear mark) and leaves a build site or a stockpile tile
    /// alone; with it, those go too. `ability` (#333) is only for `tool: "bend"`: the ability's
    /// own name (as the Bend tool's own banner shows it, e.g. "Bore Tunnel"), lowered to however
    /// many `B` presses move the Bend tool from whatever ability is active now to that one
    /// (wrapping through every ability the settlement can currently use, `B`'s own order); with
    /// none given, the Bend tool is picked (or kept) without changing which ability is active.
    /// Any of these three flags set on the wrong tool, or an `ability` that's unknown or not yet
    /// usable, fails to plan.
    Designate {
        tool: DesignateTool,
        from: Coord,
        to: Coord,
        #[serde(default)]
        standing: bool,
        #[serde(default)]
        teardown: bool,
        #[serde(default)]
        ability: Option<String>,
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
    /// Clear the ground (docs/design/clearing.md, #411/#426): `e` plus the rectangle.
    Clear,
    Stockpile,
    Bend,
    /// Sow an empty farm plot (issue #198, #333): `c` plus the rectangle.
    Sow,
}

impl DesignateTool {
    fn tool(self) -> Tool {
        match self {
            Self::Harvest => Tool::Harvest,
            Self::Cancel => Tool::CancelHarvest,
            Self::Clear => Tool::Clear,
            Self::Stockpile => Tool::Stockpile,
            Self::Bend => Tool::Bend,
            Self::Sow => Tool::Sow,
        }
    }

    fn name(self) -> &'static str {
        match self {
            Self::Harvest => "harvest",
            Self::Cancel => "cancel",
            Self::Clear => "clear",
            Self::Stockpile => "stockpile",
            Self::Bend => "bend",
            Self::Sow => "sow",
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
        Thing::EarthTent => "earth tent".to_owned(),
        Thing::AirShield => "air shield".to_owned(),
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
        Mark::Clear => "clear",
        Mark::Bend => "bend",
        Mark::Build => "build",
        Mark::Stockpile => "stockpile",
        Mark::Dragged(_) => "dragged",
        Mark::Ghost { .. } => "ghost",
        Mark::Door { .. } => "door",
        Mark::Wet => "wet",
        Mark::Warm => "warmth",
        Mark::Powered => "powered",
        Mark::AvatarAccent => "avatar",
    }
    .to_owned()
}

/// Assigns one character per distinct description, for the whole session (kept on [`Host`] as
/// [`Host::char_pool`], not rebuilt per call, #332): preferring each description's own `--ascii`
/// glyph (so [`Host::look`]'s `things` and `ground` read as the frame does, #320's "Map-cell
/// equality"), and only falling back to a pool character on the rare ascii collision -- two
/// different descriptions sharing one glyph, which colour alone would tell apart
/// (docs/design/agent-play.md, "Frames and keys"). Once a description has a character it keeps
/// it for the rest of the session, whatever else later shares a region with it -- so the legend
/// reads the same call after call -- and no two descriptions are ever given the same character,
/// so a single response's legend (built from the subset of this session's assignments its own
/// region touches) never collides either (docs/design/agent-play.md, "The map").
#[derive(Default)]
struct CharPool {
    by_description: BTreeMap<String, char>,
    used: BTreeSet<char>,
}

/// Printable ASCII, cheapest and least ambiguous characters first; `?` once the pool runs out
/// (an agent's region would need more than this many distinct things or grounds at once, which
/// no bundled map comes close to).
const POOL: &str =
    "0123456789abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ!@#$%^&*()[]{};:'\"\\|/~`+=_<>";

impl CharPool {
    /// `preferred` is the glyph `style_cell` would actually draw for `description`; used as is
    /// unless an earlier, different description already claimed it this session, in which case
    /// the next free pool character stands in instead. Either way, once `description` has a
    /// character this returns the same one for the rest of the session.
    fn char_for(&mut self, description: &str, preferred: char) -> char {
        if let Some(&ch) = self.by_description.get(description) {
            return ch;
        }
        let ch = if !self.used.contains(&preferred) {
            preferred
        } else {
            POOL.chars().find(|c| !self.used.contains(c)).unwrap_or('?')
        };
        self.by_description.insert(description.to_owned(), ch);
        self.used.insert(ch);
        ch
    }
}

// ------------------------------------------------------------------------------------------
// Reads, as data (serde-serializable, built only from fn_ui's view models, the map's cell
// look and the renderer, #315's acceptance criteria)
// ------------------------------------------------------------------------------------------

/// What one [`Event`] is about: the same categories [`Host::end_turn`]'s own doc comment names
/// as worth stopping for -- every [`fn_ui::alerts::Kind`] `Entry::worth_stopping` ever answers
/// `true` for, mapped 1:1 except `Bend`, which only ever reaches here failed (a finished one
/// is routine and never worth stopping, #377) and so becomes [`Self::BendFailed`] -- plus
/// [`Self::Need`], a need flag shown for the first time this session, which isn't an alert at
/// all. docs/design/agent-scripting.md, "`run_script`": "Event kinds are the alert kinds plus
/// needs".
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EventKind {
    Death,
    Arrival,
    Tech,
    Endeavor,
    Season,
    Stockpile,
    BendFailed,
    Need,
}

impl EventKind {
    /// The same word [`EventKind`]'s own `snake_case` serde form uses (`"death"`, `"need"`,
    /// ...): for building the wake report's own `since_start.events` counts without a JSON
    /// round trip.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Death => "death",
            Self::Arrival => "arrival",
            Self::Tech => "tech",
            Self::Endeavor => "endeavor",
            Self::Season => "season",
            Self::Stockpile => "stockpile",
            Self::BendFailed => "bend_failed",
            Self::Need => "need",
        }
    }
}

/// One line of [`Host`]'s own persistent event log (`Host::events_since`): what `end_turn`
/// stopped for, or would have, turn-tagged -- not reset or rotated for the life of a `Host`, so
/// a script's own `events()` can always say "since the script started" (#478).
#[derive(Clone, Debug, Serialize)]
pub struct Event {
    pub turn: u32,
    pub kind: EventKind,
    pub text: String,
}

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
    /// How full the stockpiles are (#509): `fn_ui::view::stockpile_capacity`, unrelated to any
    /// one item -- it's the other half of "we're low" vs "we're full and the haulers have
    /// stopped" that `stores`' own counts alone can't answer.
    pub stockpile: StockpileView,
    pub endeavor: Option<EndeavorView>,
    pub cursor: CursorView,
    pub alerts: Vec<String>,
    /// Why the turn-run that produced this reading stopped early: `"new alert: ..."` or
    /// `"need flag: ... first shown"` ([`Host::end_turn`]'s own doc comment). `None` from a bare
    /// `observe` (nothing ran), and `None` from `end_turn` too when every requested turn ran
    /// with nothing to report -- never left unset while `alerts` is empty, so an early stop
    /// can't look silent.
    pub stopped: Option<String>,
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

/// `fn_ui::view::StockpileCapacity`'s own JSON shape (#509): `tiles` designated as a stockpile
/// zone, `used` of them currently holding something, and `cap`, the per-tile limit every one
/// shares (`defs.jobs.stockpile_stack`, the same number a tile's own tooltip divides by).
#[derive(Clone, Copy, Debug, Serialize)]
pub struct StockpileView {
    pub tiles: u32,
    pub used: u32,
    pub cap: u32,
}

impl From<fn_ui::view::StockpileCapacity> for StockpileView {
    fn from(c: fn_ui::view::StockpileCapacity) -> Self {
        Self { tiles: c.tiles, used: c.used, cap: c.cap }
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct EndeavorView {
    pub name: String,
    pub count: String,
    pub line: String,
    pub next: Vec<String>,
}

/// [`Host::can_build`]'s own result: whether a building would be designated there, and why not
/// when it wouldn't.
#[derive(Clone, Debug, Serialize)]
pub struct CanBuildView {
    pub ok: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
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

/// One on-map cell, from [`Host::cells`]: the same `thing` and `ground` text [`Host::look`]'s
/// own legend would decode its glyph to, and the same mark kind names.
#[derive(Clone, Debug, Serialize)]
pub struct CellView {
    pub x: i32,
    pub y: i32,
    pub thing: String,
    pub ground: String,
    pub marks: Vec<String>,
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
    /// Names of its prereqs not yet known, comma-separated; empty once `state` isn't "Locked"
    /// (`fn_ui::research::TechCard::needs`, #338). Keeps `screen {"research"}` at parity with
    /// what a locked card's own row now says in the frame, instead of a dash that only
    /// confirming -- by researching each candidate and rechecking -- would explain.
    pub needs: String,
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
    /// The masters sign's own line, naming who has mastered and who is closest (#347). `None`
    /// for every other sign.
    pub closest: Option<String>,
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
    /// The scenario prelude this run started with, if any (`Host::with_prelude`, #481,
    /// docs/design/agent-scripting.md, "Scenario starts"): `--watch`, `--resume` and `fn_agent
    /// keys` run it again, before replaying anything else, so turn 0 starts from the same state
    /// a live `Host::from_scenario` reached. `#[serde(default)]` so a run log written before
    /// #481 still parses.
    #[serde(default)]
    pub prelude: Option<Prelude>,
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
///
/// `Script`, `Wake`, `Note` and `Call` (#479, #497, docs/design/agent-scripting.md, "Memory
/// across sessions"; docs/design/agent-studio.md, "A new log line: `call`") are `fn_agent`'s own
/// commentary on an MCP `tools/call`: nothing a `Host` ever makes itself (`fn_agent::tools::Tools`
/// writes them directly, around the `Keys`/`Turn` lines a script's own `act`/`keys`/`wait` calls
/// produce), and nothing [`Host::resume`] or `crate::watch::to_keys_script` replay: a chunked
/// `--resume` session reads `Script`, `Wake` and `Note` back to rebuild its script library,
/// `memory` and lab notes (`fn_agent::tools::Tools::resume`), but the game itself is exactly the
/// same seed, keys and turns with or without any of these four.
/// - `Script { name, version, code, turns, wake_on }`: a `run_script` call's own start -- `name`
///   and `code` are the tag and source a `run_script {code}` ad hoc call used (`version == 0`,
///   never saved) or a saved script's own name and code (`version >= 1`, `run_script {name}`).
/// - `Wake { reason, why, turn, memory }`: that same call's own end -- `reason`/`why` as
///   `fn_agent::script::Outcome`'s own fields, `turn` the world's turn count when it stopped,
///   and `memory` the resulting `memory` map (always present, even unchanged). Always the next
///   `Wake` after a `Script` line, since `run_script` calls never overlap.
/// - `Note { text }`: one line appended by `notes {append}`.
/// - `Call { tool, summary }`: written by `fn_agent::tools::Tools::call` just before it dispatches
///   `tool`'s own request, so it always comes ahead of that same call's own `Keys`/`Look`/
///   `Inspect`/`Script`/`Wake`/`Note` lines -- a live watcher sees "started" the moment a call
///   lands, not only once its result (and a `run_script`'s own lines) are in. `summary` is built
///   from the arguments alone (never a result, and never `run_script`'s own code), capped at 60
///   characters; empty for a tool whose arguments say nothing worth summarizing (`observe`,
///   `briefing`, `inspect`, `save_script`, `scripts`, `sdk_reference`).
/// - `Plan { plan }`: written by `fn_agent`'s `plan {set?, merge?}` tool after each change (#524,
///   docs/design/agent-hillclimb.md, "Within a run: a governor and a plan"): `plan` is the whole
///   plan map after the change. A `--resume` session restores the latest one; replay, `verify` and
///   `--watch` ignore it, like the other commentary lines.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum LogLine {
    Header(RunLogHeader),
    Keys { keys: String, label: Option<String>, note: Option<String> },
    Look { x: i32, y: i32, w: u16, h: u16 },
    Inspect { x: i32, y: i32 },
    Turn { index: u32, tick: u64 },
    End { tick: u64, reason: String },
    Script { name: String, version: u32, code: String, turns: u32, wake_on: Vec<EventKind> },
    Wake { reason: String, why: Option<String>, turn: u32, memory: serde_json::Value },
    Note { text: String },
    Call { tool: String, summary: String },
    Plan { plan: serde_json::Value },
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
    use fn_sim::world::Command;
    use fn_sim::{Nation, Terrain};
    use fn_ui::tools::ability_for_nation;

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
            scenario: None,
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

    /// `frame` clamps a size below the renderer's own minimum up to it (#334) instead of
    /// rendering `draw::too_small_text`'s one-line notice: an `h: 2` request still draws the
    /// whole game, at 80x24 (`layout::MIN_WIDTH`/`MIN_HEIGHT`).
    #[test]
    fn frame_clamps_a_too_small_size_up_to_the_renderers_minimum_instead_of_rendering_a_notice() {
        let mut h = host(Nation::Earth);
        assert_eq!(Host::clamp_frame_size(80, 2), (layout::MIN_WIDTH, layout::MIN_HEIGHT));
        assert_eq!(Host::clamp_frame_size(1, 1), (layout::MIN_WIDTH, layout::MIN_HEIGHT));
        assert_eq!(Host::clamp_frame_size(120, 40), (120, 40), "big enough: untouched");
        let text = h.frame(80, 2);
        assert!(!text.contains("Terminal too small"), "{text}");
        assert_eq!(text, h.frame(layout::MIN_WIDTH, layout::MIN_HEIGHT));
    }

    #[test]
    fn observe_reports_the_people_and_the_tool() {
        let h = host(Nation::Earth);
        let obs = h.observe();
        assert_eq!(obs.people.total, 6, "a new game's starting settlers");
        assert_eq!(obs.tool, "Select");
        assert!(obs.alerts.is_empty());
    }

    /// `observe().stockpile` (#509) is `fn_ui::view::stockpile_capacity`'s own numbers, straight
    /// through: zero tiles designated on a fresh game, and `cap` matches the data regardless.
    #[test]
    fn observe_reports_stockpile_capacity_with_no_zone_designated_yet() {
        let h = host(Nation::Earth);
        let obs = h.observe();
        assert_eq!((obs.stockpile.tiles, obs.stockpile.used), (0, 0));
        assert_eq!(obs.stockpile.cap, h.app().session.defs.jobs.stockpile_stack);
    }

    /// Designating a stockpile tile through `act` (the same path a player's `p` key takes) is
    /// reflected in the very next `observe().stockpile.tiles` -- an agent doesn't have to count
    /// `cells()` marks itself to know how much room it just made.
    #[test]
    fn observe_reports_stockpile_capacity_after_designating_a_zone() {
        let mut h = host(Nation::Water);
        let at = Pos::new(30, 30);
        let tile = h.app_mut().session.world.map.get_mut(at).unwrap();
        tile.terrain = fn_sim::Terrain::Land;
        tile.resource = None;
        let steps = vec![ActionStep::Designate {
            tool: DesignateTool::Stockpile,
            from: Coord { x: at.x, y: at.y },
            to: Coord { x: at.x, y: at.y },
            standing: false,
            teardown: false,
            ability: None,
        }];
        let result = h.act(&steps, None);
        assert!(result.error.is_none(), "{result:?}");
        assert!(
            result.notices[0].notice.starts_with("designate stockpile: 1 done"),
            "{:?}",
            result.notices
        );
        let obs = h.observe();
        assert_eq!(obs.stockpile.tiles, 1);
        assert_eq!(obs.stockpile.used, 0, "nothing stored on it yet");
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
        let stopped = obs.stopped.expect("an early stop always says why");
        assert!(stopped.contains("need flag:"), "{stopped}");
    }

    /// A need flag that was already up when the call started, and that flickers off and back
    /// on inside the call's own window, never stops `end_turn` -- only something genuinely
    /// unseen in that window does (#331). Drives `run_one_turn` directly (needs set by hand
    /// between turns) rather than `end_turn` itself, so the flicker is exact and doesn't depend
    /// on the AI's own timing for when a need actually clears.
    #[test]
    fn first_new_flag_ignores_a_flag_that_already_showed_and_flickers() {
        // A colony where Thirsty is already up when the call starts (so it's in `seen` before
        // turn 1 runs at all, exactly as `end_turn` seeds it).
        let mut seen: BTreeSet<String> = ["Thirsty".to_owned()].into_iter().collect();

        // Turn 1: the flag clears (the colonist found water).
        assert_eq!(first_new_flag(&mut seen, vec![]), None);

        // Turn 2: it flickers back -- still not new, since it showed at the call's start.
        assert_eq!(
            first_new_flag(&mut seen, vec![("Thirsty".to_owned(), 1)]),
            None,
            "a flag that flickers back isn't new"
        );

        // Turn 3: a different need crossing its own threshold for the first time still stops.
        assert_eq!(
            first_new_flag(&mut seen, vec![("Hungry".to_owned(), 1)]),
            Some("Hungry".to_owned())
        );

        // Turn 4: and now that Hungry has shown once, it flickering too isn't new either.
        assert_eq!(first_new_flag(&mut seen, vec![]), None);
        assert_eq!(first_new_flag(&mut seen, vec![("Hungry".to_owned(), 1)]), None);
    }

    /// A need flag already up at the call's start doesn't stop `end_turn` even once, through
    /// the public API end to end (#331): the companion to the test above, which drives the
    /// flicker turn by turn.
    #[test]
    fn end_turn_runs_the_full_count_when_the_only_flag_already_showed() {
        let mut h = host(Nation::Earth).with_turn_seconds(1);
        let hungry = h.app().session.defs.needs.hunger.low - 1.0;
        h.app_mut().session.world.units[0].needs.hunger = hungry;
        let obs = h.end_turn(3);
        assert_eq!(obs.turn, 3, "a flag already showing at the start never stops it");
        assert_eq!(obs.stopped, None);
    }

    /// A truly new alert -- not a need flag -- still stops `end_turn` early, and
    /// `Observation::stopped` names it instead of leaving `alerts: []` to look silent (#331).
    #[test]
    fn end_turn_stops_on_a_new_alert_and_names_it() {
        let mut h = host(Nation::Earth).with_turn_seconds(1);
        // Already flagged (Starving, Dying) before the call starts, so those don't stop it;
        // the death that follows almost at once is a new alert instead.
        h.app_mut().session.world.units[1].needs.hunger = 0.0;
        h.app_mut().session.world.units[1].needs.health = 0.01;
        let obs = h.end_turn(5);
        assert!(obs.turn >= 1 && obs.turn < 5, "{}", obs.turn);
        let stopped = obs.stopped.expect("a death is a new alert, never silent");
        assert!(stopped.starts_with("new alert:") && stopped.contains("died of"), "{stopped}");
    }

    /// A settlement that bends a lot (Fire's Hearthflame, say) finishes several bends over a
    /// run and none of them stop it (#377): routine `Bend` alerts never counted toward
    /// `Observation::stopped`, only a death, an arrival, an endeavor event, a finished tech, a
    /// season event or a *failed* bend do.
    #[test]
    fn end_turn_runs_every_turn_despite_several_finished_bends() {
        let mut h = host(Nation::Fire).with_turn_seconds(5);
        let ability = ability_for_nation(&h.app().session.defs, Nation::Fire).unwrap();
        // Every unit kindles its own flame: several finished bends well inside the window below.
        let positions: Vec<_> = h.app().session.world.units.iter().map(|u| u.pos).collect();
        for pos in positions {
            h.app_mut().session.apply(Command::Bend { ability, pos }).unwrap();
        }
        let obs = h.end_turn(30);
        assert_eq!(obs.turn, 30, "finished bends alone should never cut a call short");
        assert_eq!(obs.stopped, None);
        let events = h.app().session.world.bend_events();
        assert!(events.len() > 1, "expected several finished bends, got {}", events.len());
        assert!(events.iter().all(|e| e.outcome == fn_sim::world::BendOutcome::Done));
    }

    /// Unlike a finished bend, a *failed* one still stops `end_turn` and names it (#377): the
    /// ground it targeted turns to rock before the channel completes.
    #[test]
    fn end_turn_stops_on_a_failed_bend_and_names_it() {
        let mut h = host(Nation::Fire).with_turn_seconds(1);
        let ability = ability_for_nation(&h.app().session.defs, Nation::Fire).unwrap();
        let pos = h.app().session.world.units[0].pos;
        h.app_mut().session.apply(Command::Bend { ability, pos }).unwrap();
        h.app_mut().session.world.map[pos].terrain = Terrain::Mountain;
        h.app_mut().session.world.map_changed();
        let obs = h.end_turn(20);
        assert!(obs.turn >= 1 && obs.turn < 20, "{}", obs.turn);
        let stopped = obs.stopped.expect("a failed bend is worth an agent's attention");
        assert!(stopped.starts_with("new alert:") && stopped.contains("failed"), "{stopped}");
    }

    /// The persistent event log (#478, `Host::events_since`) carries a failed bend as
    /// [`EventKind::BendFailed`], but never a merely finished one: the same "worth stopping"
    /// line `end_turn`'s own doc comment draws, reused by [`event_kind_for_alert`].
    #[test]
    fn events_since_carries_a_failed_bend_but_not_a_finished_one() {
        let mut h = host(Nation::Fire).with_turn_seconds(5);
        let ability = ability_for_nation(&h.app().session.defs, Nation::Fire).unwrap();
        let positions: Vec<_> = h.app().session.world.units.iter().map(|u| u.pos).collect();
        for pos in positions {
            h.app_mut().session.apply(Command::Bend { ability, pos }).unwrap();
        }
        let mark = h.event_mark();
        h.end_turn(30);
        let finished_events = h.events_since(mark);
        assert!(finished_events.is_empty(), "a finished bend is routine: {finished_events:?}");

        let pos = h.app().session.world.units[0].pos;
        h.app_mut().session.apply(Command::Bend { ability, pos }).unwrap();
        h.app_mut().session.world.map[pos].terrain = Terrain::Mountain;
        h.app_mut().session.world.map_changed();
        let mark = h.event_mark();
        h.end_turn(20);
        let events = h.events_since(mark);
        assert_eq!(events.len(), 1, "{events:?}");
        assert_eq!(events[0].kind, EventKind::BendFailed);
        assert!(events[0].text.contains("failed"), "{}", events[0].text);
    }

    /// A need flag shown for the first time this session becomes an [`EventKind::Need`] event,
    /// once -- not every turn it stays up, and not again later once it's cleared and flickers
    /// back (#478's own persistent tracking, `Host::needs_ever_seen`).
    #[test]
    fn events_since_carries_a_need_flag_first_shown_but_not_its_repeats() {
        let mut h = host(Nation::Earth).with_turn_seconds(1);
        let hungry = h.app().session.defs.needs.hunger.low - 1.0;
        h.app_mut().session.world.units[0].needs.hunger = hungry;
        let mark = h.event_mark();
        h.end_turn(5);
        let events = h.events_since(mark);
        let needs: Vec<_> = events.iter().filter(|e| e.kind == EventKind::Need).collect();
        assert_eq!(needs.len(), 1, "{events:?}");
        assert!(needs[0].text.contains("first shown"), "{}", needs[0].text);

        // Feeding the unit clears the flag, then it flickers back: still only counted once,
        // ever, for the life of this `Host`.
        h.app_mut().session.world.units[0].needs.hunger = 1.0;
        h.app_mut().session.world.units[0].needs.hunger = hungry;
        let mark = h.event_mark();
        h.end_turn(5);
        let repeats: Vec<_> =
            h.events_since(mark).into_iter().filter(|e| e.kind == EventKind::Need).collect();
        assert!(repeats.is_empty(), "a flag shown before never counts as new again: {repeats:?}");
    }

    /// `event_mark` lets a later caller read only what's new since it was taken, the same way a
    /// script's own `events()` only ever sees what happened since it started (#478).
    #[test]
    fn event_mark_only_sees_events_from_after_it_was_taken() {
        let mut h = host(Nation::Earth).with_turn_seconds(1);
        let hungry = h.app().session.defs.needs.hunger.low - 1.0;
        let before_mark = h.event_mark();
        h.app_mut().session.world.units[0].needs.hunger = hungry;
        h.end_turn(5);
        assert!(!h.events_since(before_mark).is_empty(), "the need flag shows from this mark");

        let after_mark = h.event_mark();
        assert!(h.events_since(after_mark).is_empty(), "nothing new yet from this later mark");
    }

    /// `Host::look`'s "blank tile" entry ("-", [`describe_thing`]'s `Thing::Ground` case) names
    /// whatever bare ground its call happens to see first, so the *named* ground that preferred
    /// glyph actually belongs to collides with it and falls back to a pool character (#332) --
    /// this is the real-world shape of the bug: at seed 7 Earth, a `look` from the map's corner
    /// sees water first, so "-" claims water's own glyph (`~`) and "water" itself is bumped to a
    /// fallback. Two overlapping regions with different contents -- one whose own corner is
    /// water, one whose corner is Plains instead, but which still has water somewhere in view --
    /// must still agree on "water"'s character: `CharPool` persisted on `Host` (not rebuilt each
    /// call) means the fallback "water" was given the first time is the one it keeps, rather
    /// than reverting to `~` just because this particular region's blank tile claimed something
    /// else.
    #[test]
    fn look_gives_a_shared_description_the_same_character_across_overlapping_regions() {
        let mut h = host(Nation::Earth);
        // The map's corner is water with nothing on it (confirmed for this seed): "-" claims
        // water's own glyph here, so "water" itself falls back to a pool character.
        let first = h.look(0, 0, 64, 40);
        assert_eq!(first.legend.get("~"), Some(&"-".to_owned()));
        let (water_char, _) = first
            .legend
            .iter()
            .find(|(_, d)| d.as_str() == "water")
            .expect("water is on screen and bumped off its own glyph");
        assert_ne!(water_char, "~", "water lost its own glyph to the blank-tile entry");

        // A different, overlapping region whose own corner is Plains, not water, so a fresh
        // `CharPool` would let "-" claim Plains's glyph here instead and hand "water" its real
        // glyph back. With `CharPool` kept on `Host`, "water" already has a character and keeps
        // it.
        let second = h.look(80, 18, 64, 40);
        let (water_char_2, _) = second
            .legend
            .iter()
            .find(|(_, d)| d.as_str() == "water")
            .expect("water is still somewhere in this region too");
        assert_eq!(
            water_char, water_char_2,
            "\"water\" got a different character in an overlapping region: {:?} vs {:?}",
            first.legend, second.legend
        );
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

    /// `can_build` (#509) says a free land tile is fine, reuses the same `World::check_build`
    /// a `build` step's own confirm does to refuse a blocked one with the player's own words,
    /// names an unknown building and a locked one, and -- the acceptance criteria's own test --
    /// changes nothing at all: no key pressed, no `keys` line, the same `observe` before and
    /// after.
    #[test]
    fn can_build_judges_a_spot_the_same_way_a_build_step_would_and_touches_nothing() {
        let mut h = host(Nation::Earth);
        let at = Pos::new(30, 30);
        let tile = h.app_mut().session.world.map.get_mut(at).unwrap();
        tile.terrain = fn_sim::Terrain::Land;
        tile.resource = None;

        let before_obs = serde_json::to_value(h.observe()).unwrap();
        let before_lines = h.log().lines().len();

        let free = h.can_build("Bed", at.x, at.y);
        assert!(free.ok, "{free:?}");
        assert!(free.reason.is_none(), "{free:?}");

        let unknown = h.can_build("Not A Real Building", at.x, at.y);
        assert!(!unknown.ok);
        assert!(unknown.reason.unwrap().contains("Not A Real Building"));

        let rows = fn_ui::build::picker_rows(&h.app().session);
        let locked = rows.iter().find(|r| r.locked()).expect("a new game has a locked building");
        let locked_view = h.can_build(&locked.name, at.x, at.y);
        assert!(!locked_view.ok);
        assert_eq!(locked_view.reason, locked.requires.clone());

        // On a stockpile zone, the same way `World::check_build` refuses it for a real `build`
        // step (`fn_ui::build::describe_blocker`'s own words).
        let water = water_tile(&h);
        let on_water = h.can_build("Bed", water.x, water.y);
        assert!(!on_water.ok);
        assert_eq!(on_water.reason.as_deref(), Some("on water"));

        assert_eq!(serde_json::to_value(h.observe()).unwrap(), before_obs, "nothing changed");
        assert_eq!(h.log().lines().len(), before_lines, "no line was written");
    }

    #[test]
    fn can_build_off_the_map_is_refused_with_a_reason() {
        let h = host(Nation::Earth);
        let (w, hh) = h.app().session.map_size();
        let view = h.can_build("Bed", w, hh);
        assert!(!view.ok);
        assert!(view.reason.unwrap().contains("isn't on the map"));
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
            scenario: None,
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
    // Resuming a run log (#341)
    // ----------------------------------------------------------------------------------------

    /// Play a few turns, stop, then `Host::resume` from the header and lines so far: the
    /// resumed session's `observe` and frame match the one it left off at (#341's acceptance
    /// criteria), and it keeps playing from the same point rather than restarting.
    #[test]
    fn resuming_a_log_continues_from_the_same_observation_and_frame() {
        let mut h = host(Nation::Water).with_turn_seconds(10);
        h.press_keys("h <right*3> <enter> <down*2> <enter>", Some("harvest".into()), None).unwrap();
        h.end_turn(2);
        h.look(0, 0, 10, 10);
        h.inspect(5, 5);

        let header = h.header();
        let lines = h.log().lines();
        let mut resumed = Host::resume(&header, &lines).unwrap();

        let size = (100, 40);
        assert_eq!(
            serde_json::to_value(resumed.observe()).unwrap(),
            serde_json::to_value(h.observe()).unwrap(),
        );
        assert_eq!(resumed.frame(size.0, size.1), h.frame(size.0, size.1));
        assert_eq!(resumed.turn, h.turn);

        // Resuming isn't a snapshot: the resumed `Host` keeps playing from here, exactly as the
        // original would have.
        resumed.press_keys("x", None, None).unwrap();
        h.press_keys("x", None, None).unwrap();
        assert_eq!(resumed.frame(size.0, size.1), h.frame(size.0, size.1));
    }

    /// `Host::resume` refuses a log made against different data, the same message
    /// `watch::Watcher::new` gives (#324's rule applied here too).
    #[test]
    fn resume_refuses_a_mismatched_data_digest() {
        let h = host(Nation::Earth);
        let mut header = h.header();
        header.data_digest = header.data_digest.wrapping_add(1);
        let err = match Host::resume(&header, &h.log().lines()) {
            Ok(_) => panic!("a mismatched digest should be refused"),
            Err(e) => e,
        };
        assert!(err.contains("different data"), "{err}");
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
            teardown: false,
            ability: None,
        }];
        let result = h.act(&steps, Some("wood for a bed".into()));
        assert!(result.error.is_none(), "{result:?}");
        assert_eq!(h.observe().tool, "Harvest");
        assert_eq!(h.log.counts(), (1, 0, 0, 0));

        let script = result.keys.join(" ");
        assert_eq!(script, "h <right> <right> <down> <enter> <right> <down> <enter>");
        assert_same_as_key_script(&mut h, Nation::Earth, &script);
    }

    /// `designate {tool: clear}` (docs/design/clearing.md, #411/#426) lowers to `e` plus the
    /// rectangle, exactly as `designate {tool: harvest}` lowers to `h`.
    #[test]
    fn act_designate_clear_round_trips_with_its_key_script_and_clears_the_rectangle() {
        let mut h = host(Nation::Earth);
        let cursor = h.app().ui.cursor;
        let from = Pos::new(cursor.x + 2, cursor.y + 1);
        let to = Pos::new(from.x + 1, from.y + 1);
        let steps = vec![ActionStep::Designate {
            tool: DesignateTool::Clear,
            from: Coord { x: from.x, y: from.y },
            to: Coord { x: to.x, y: to.y },
            standing: false,
            teardown: false,
            ability: None,
        }];
        let result = h.act(&steps, Some("make room for a bed".into()));
        assert!(result.error.is_none(), "{result:?}");
        assert_eq!(h.observe().tool, "Clear");
        assert_eq!(h.log.counts(), (1, 0, 0, 0));

        let script = result.keys.join(" ");
        assert_eq!(script, "e <right> <right> <down> <enter> <right> <down> <enter>");
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
                teardown: false,
                ability: None,
            },
            ActionStep::Designate {
                tool: DesignateTool::Harvest,
                from: Coord { x: land_from.x, y: land_from.y },
                to: Coord { x: land_to.x, y: land_to.y },
                standing: false,
                teardown: false,
                ability: None,
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

    /// A step that sets no notice of its own (`cursor`, here) right after a `designate` doesn't
    /// inherit that designate's own text (#334's own bug: a step that isn't `designate` or
    /// `build` never touches `UiState::notice`, so reading it blindly just replays whatever an
    /// earlier step -- in this `act` call or an entirely separate one -- left there).
    #[test]
    fn act_never_reports_a_later_steps_notice_as_a_stale_carryover_from_an_earlier_one() {
        let mut h = host(Nation::Earth);
        let water = water_tile(&h);
        let steps = vec![
            ActionStep::Designate {
                tool: DesignateTool::Stockpile,
                from: Coord { x: water.x, y: water.y },
                to: Coord { x: water.x, y: water.y },
                standing: false,
                teardown: false,
                ability: None,
            },
            ActionStep::Cursor { x: water.x, y: water.y },
        ];
        let result = h.act(&steps, None);
        assert!(result.error.is_none(), "{result:?}");
        // Only the designate step (index 0) left a notice; the cursor step (index 1) set none
        // and must not show up at all, not even with the designate's own stale text.
        assert_eq!(result.notices.len(), 1, "{:?}", result.notices);
        assert_eq!(result.notices[0].step, 0);
        assert!(
            result.notices[0].notice.starts_with("designate stockpile: 0 done, 1 skipped"),
            "{}",
            result.notices[0].notice
        );
    }

    /// The same, across two separate `act` calls: a `research` step in a call of its own must
    /// not surface an unrelated designate's notice left over from an earlier call (the exact
    /// shape #334 was filed from: a `research Fishing` step reporting a stockpile designate's
    /// "N done, M skipped" text).
    #[test]
    fn act_does_not_carry_a_stale_notice_across_separate_act_calls() {
        let mut h = host(Nation::Fire);
        let water = water_tile(&h);
        let designate = vec![ActionStep::Designate {
            tool: DesignateTool::Stockpile,
            from: Coord { x: water.x, y: water.y },
            to: Coord { x: water.x, y: water.y },
            standing: false,
            teardown: false,
            ability: None,
        }];
        let first = h.act(&designate, None);
        assert!(first.error.is_none(), "{first:?}");
        assert_eq!(first.notices.len(), 1, "{:?}", first.notices);

        let view = fn_ui::research::screen_view(&h.app().session);
        let tech = view
            .columns
            .iter()
            .flatten()
            .find(|c| c.state == fn_ui::research::TechState::Available)
            .expect("a new game always has something researchable")
            .name
            .clone();
        let research = h.act(&[ActionStep::Research { tech }], None);
        assert!(research.error.is_none(), "{research:?}");
        assert!(research.notices.is_empty(), "{:?}", research.notices);
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
            teardown: false,
            ability: None,
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
            teardown: false,
            ability: None,
        }];
        let result = h.act(&steps, None);
        let err = result.error.unwrap();
        assert_eq!(err.step, 0);
        assert!(err.reason.contains("harvest"), "{}", err.reason);
        assert!(result.keys.is_empty());
        assert_eq!(h.observe().tool, "Select", "nothing was pressed");
    }

    /// #335: `designate {tool: cancel, teardown: true}` presses `o` after `x`, the same key the
    /// standing-order toggle uses, which also withdraws a build site or a stockpile tile.
    #[test]
    fn act_designate_teardown_presses_o_after_the_cancel_tool_letter() {
        let mut h = host(Nation::Fire);
        let cursor = h.app().ui.cursor;
        let steps = vec![ActionStep::Designate {
            tool: DesignateTool::Cancel,
            from: Coord { x: cursor.x, y: cursor.y },
            to: Coord { x: cursor.x, y: cursor.y },
            standing: false,
            teardown: true,
            ability: None,
        }];
        let result = h.act(&steps, None);
        assert!(result.error.is_none(), "{result:?}");
        assert_eq!(result.keys.join(" "), "x o <enter> <enter>");
        assert!(h.app().ui.teardown);
        assert_same_as_key_script(&mut h, Nation::Fire, &result.keys.join(" "));
    }

    #[test]
    fn act_designate_refuses_teardown_outside_the_cancel_tool_and_presses_nothing() {
        let mut h = host(Nation::Earth);
        let cursor = h.app().ui.cursor;
        let steps = vec![ActionStep::Designate {
            tool: DesignateTool::Harvest,
            from: Coord { x: cursor.x, y: cursor.y },
            to: Coord { x: cursor.x, y: cursor.y },
            standing: false,
            teardown: true,
            ability: None,
        }];
        let result = h.act(&steps, None);
        let err = result.error.unwrap();
        assert_eq!(err.step, 0);
        assert!(err.reason.contains("cancel"), "{}", err.reason);
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
            teardown: false,
            ability: None,
        }];
        let result = h.act(&steps, None);
        let err = result.error.unwrap();
        assert!(err.reason.contains("isn't on the map"), "{}", err.reason);
        assert!(result.keys.is_empty());
        assert_eq!(h.observe().tool, "Select");
    }

    /// Plants a bare, empty farm plot (#333) right on `h`'s cursor tile, bypassing the build
    /// queue (`World::adopt_farm_plots` registers it as a plot the way a loaded save's would):
    /// a farm plot's tests in `world/farm/tests.rs` do the same, for the same reason -- `act`'s
    /// own `designate {tool: sow}` doesn't need a finished build site, just a plot to sow.
    fn farm_plot_tile(h: &mut Host) -> Pos {
        let pos = h.app().ui.cursor;
        let farm = h.app().session.defs.building_id("farm_plot").expect("bundled data has one");
        let tile = h.app_mut().session.world.map.get_mut(pos).expect("the cursor is on the map");
        tile.resource = None;
        tile.building = Some(fn_sim::building::Building::new(farm, true, false));
        let defs = h.app().session.defs.clone();
        h.app_mut().session.world.adopt_farm_plots(&defs);
        pos
    }

    /// `designate {tool: sow}` (#333, issue #198) lowers to `c` plus the rectangle, exactly as
    /// every other designate tool, and the sim opens a Sow job for the plot. No
    /// `assert_same_as_key_script` here: the plot is planted straight onto `h`'s own world
    /// (`farm_plot_tile`), not through any key a fresh game's own replay could reach, so the two
    /// can't be compared frame for frame the way every other designate test here is.
    #[test]
    fn act_designate_sow_round_trips_with_its_key_script_and_sows_the_plot() {
        let mut h = host(Nation::Earth);
        let pos = farm_plot_tile(&mut h);
        let steps = vec![ActionStep::Designate {
            tool: DesignateTool::Sow,
            from: Coord { x: pos.x, y: pos.y },
            to: Coord { x: pos.x, y: pos.y },
            standing: false,
            teardown: false,
            ability: None,
        }];
        let result = h.act(&steps, None);
        assert!(result.error.is_none(), "{result:?}");
        assert_eq!(h.observe().tool, "Sow");
        assert_eq!(result.keys.join(" "), "c <enter> <enter>");
        assert!(h.app().session.world.sow_designations().any(|p| p == pos));
    }

    /// #333's own bug report: re-designating a plot that's already marked for sowing, or
    /// already sown and growing, used to be indistinguishable from one that's "not a farm plot"
    /// at all. The notice now names which it actually is.
    #[test]
    fn act_designate_sow_names_an_already_marked_or_sown_plot_instead_of_not_a_farm_plot() {
        let mut h = host(Nation::Earth);
        let pos = farm_plot_tile(&mut h);
        let step = |pos: Pos| {
            vec![ActionStep::Designate {
                tool: DesignateTool::Sow,
                from: Coord { x: pos.x, y: pos.y },
                to: Coord { x: pos.x, y: pos.y },
                standing: false,
                teardown: false,
                ability: None,
            }]
        };
        let first = h.act(&step(pos), None);
        assert!(first.error.is_none(), "{first:?}");
        assert_eq!(first.notices[0].notice, "sow an empty farm plot: 1 done, 0 skipped");

        // Already marked: a Sow job is pending, but the plot itself is still empty.
        let marked = h.act(&step(pos), None);
        assert_eq!(
            marked.notices[0].notice,
            "sow an empty farm plot: 0 done, 1 skipped (already designated)"
        );
        assert!(!marked.notices[0].notice.contains("not a farm plot"), "{marked:?}");

        // Run until the plot is actually sown and growing.
        for _ in 0..5_000 {
            if h.app().session.world.plot_stage(pos) == Some(fn_sim::world::PlotStage::Growing) {
                break;
            }
            h.end_turn(1);
        }
        assert_eq!(
            h.app().session.world.plot_stage(pos),
            Some(fn_sim::world::PlotStage::Growing),
            "the plot should have been sown by now"
        );

        // Already sown: growing, not empty.
        let sown = h.act(&step(pos), None);
        assert_eq!(
            sown.notices[0].notice,
            "sow an empty farm plot: 0 done, 1 skipped (already sown)"
        );
        assert!(!sown.notices[0].notice.contains("not a farm plot"), "{sown:?}");
    }

    /// `designate {tool: bend, ability}` (#333) lowers to however many `B` presses move the
    /// Bend tool from whatever ability is active now to the one named -- including a different
    /// one than the last `designate {tool: bend}` call picked, and including none at all when
    /// it's already the active one. A Fire settlement with an Earth newcomer (`Unit::nation`,
    /// #248's own trick) can use two abilities with no research at all, so this doesn't need to
    /// wait on one.
    #[test]
    fn act_designate_bend_ability_presses_the_right_number_of_bend_keys_from_whatever_was_active_last()
     {
        let mut h = host(Nation::Fire);
        h.app_mut().session.world.units[1].nation = Some(Nation::Earth);
        let hearth = ability_for_nation(&h.app().session.defs, Nation::Fire).unwrap();
        let raise_earth = ability_for_nation(&h.app().session.defs, Nation::Earth).unwrap();
        assert_eq!(
            fn_ui::tools::usable_abilities(&h.app().session),
            [raise_earth, hearth],
            "data order, not which nation founded it"
        );
        assert_eq!(
            fn_ui::tools::bend_ability(&h.app().session),
            Some(raise_earth),
            "the first usable, with no choice yet"
        );
        let pos = h.app().ui.cursor;
        let step = |ability: &str| {
            vec![ActionStep::Designate {
                tool: DesignateTool::Bend,
                from: Coord { x: pos.x, y: pos.y },
                to: Coord { x: pos.x, y: pos.y },
                standing: false,
                teardown: false,
                ability: Some(ability.to_owned()),
            }]
        };

        // Nothing is active yet (the tool itself isn't Bend): one press switches the tool, one
        // more cycles past Raise Earth onto Hearthflame.
        let to_hearth = h.act(&step(&h.app().session.defs.ability(hearth).name.clone()), None);
        assert!(to_hearth.error.is_none(), "{to_hearth:?}");
        assert_eq!(to_hearth.keys, ["b", "b", "<enter>", "<enter>"]);
        assert_eq!(fn_ui::tools::bend_ability(&h.app().session), Some(hearth));

        // The Bend tool is already active and on Hearthflame: one press cycles back round to
        // Raise Earth, a different ability than the one just used.
        let to_raise = h.act(&step(&h.app().session.defs.ability(raise_earth).name.clone()), None);
        assert!(to_raise.error.is_none(), "{to_raise:?}");
        assert_eq!(to_raise.keys, ["b", "<enter>", "<enter>"]);
        assert_eq!(fn_ui::tools::bend_ability(&h.app().session), Some(raise_earth));

        // Asking for the ability that's already active presses no bend key at all.
        let repeat = h.act(&step(&h.app().session.defs.ability(raise_earth).name.clone()), None);
        assert!(repeat.error.is_none(), "{repeat:?}");
        assert_eq!(repeat.keys, ["<enter>", "<enter>"]);
        assert_eq!(fn_ui::tools::bend_ability(&h.app().session), Some(raise_earth));
    }

    #[test]
    fn act_designate_bend_refuses_an_unknown_or_locked_ability_and_presses_nothing() {
        let mut h = host(Nation::Fire);
        let cursor = h.app().ui.cursor;
        let steps = vec![ActionStep::Designate {
            tool: DesignateTool::Bend,
            from: Coord { x: cursor.x, y: cursor.y },
            to: Coord { x: cursor.x, y: cursor.y },
            standing: false,
            teardown: false,
            ability: Some("Not A Real Ability".into()),
        }];
        let result = h.act(&steps, None);
        let err = result.error.unwrap();
        assert!(err.reason.contains("no such ability"), "{}", err.reason);
        assert!(result.keys.is_empty());
        assert_eq!(h.observe().tool, "Select");

        // Inner Fire is a real ability, but Fire alone hasn't researched it yet.
        let steps = vec![ActionStep::Designate {
            tool: DesignateTool::Bend,
            from: Coord { x: cursor.x, y: cursor.y },
            to: Coord { x: cursor.x, y: cursor.y },
            standing: false,
            teardown: false,
            ability: Some("Inner Fire".into()),
        }];
        let result = h.act(&steps, None);
        let err = result.error.unwrap();
        assert!(err.reason.contains("isn't usable yet"), "{}", err.reason);
        assert!(result.keys.is_empty());
        assert_eq!(h.observe().tool, "Select");
    }

    #[test]
    fn act_designate_refuses_an_ability_outside_the_bend_tool_and_presses_nothing() {
        let mut h = host(Nation::Earth);
        let cursor = h.app().ui.cursor;
        let steps = vec![ActionStep::Designate {
            tool: DesignateTool::Harvest,
            from: Coord { x: cursor.x, y: cursor.y },
            to: Coord { x: cursor.x, y: cursor.y },
            standing: false,
            teardown: false,
            ability: Some("Hearthflame".into()),
        }];
        let result = h.act(&steps, None);
        let err = result.error.unwrap();
        assert_eq!(err.step, 0);
        assert!(err.reason.contains("ability"), "{}", err.reason);
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
                prelude: None,
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

    /// `Host::cells` (#480) reports, for every on-map cell in a region, exactly the `thing` and
    /// `ground` text `Host::look`'s own legend would decode its glyph to, and the same marks:
    /// one more "says the same thing" check alongside this section's existing two, so a script
    /// reading `cells()` never learns anything `look()` wouldn't also say.
    #[test]
    fn cells_reports_exactly_what_looks_legend_would_decode_to() {
        for nation in Nation::ALL {
            let mut h = mid_game(nation);
            h.frame(80, 24);
            let (origin, (vw, vh)) = (h.app().ui.view, h.app().ui.viewport());
            let look = h.look(origin.x, origin.y, vw as u16, vh as u16);
            let cells = h.cells(origin.x, origin.y, vw as u16, vh as u16);
            assert_eq!(cells.len(), (vw as usize) * (vh as usize), "{nation:?}");
            for cell in &cells {
                let row = (cell.y - origin.y) as usize;
                let col = (cell.x - origin.x) as usize;
                let thing_ch = look.things[row].chars().nth(col).unwrap();
                let ground_ch = look.ground[row].chars().nth(col).unwrap();
                assert_eq!(
                    look.legend[&thing_ch.to_string()],
                    cell.thing,
                    "{nation:?} thing at ({},{})",
                    cell.x,
                    cell.y
                );
                assert_eq!(
                    look.legend[&ground_ch.to_string()],
                    cell.ground,
                    "{nation:?} ground at ({},{})",
                    cell.x,
                    cell.y
                );
                let want_marks: Vec<String> = look
                    .marks
                    .iter()
                    .find(|m| m.x == cell.x && m.y == cell.y)
                    .map(|m| m.kinds.clone())
                    .unwrap_or_default();
                assert_eq!(cell.marks, want_marks, "{nation:?} marks at ({},{})", cell.x, cell.y);
            }
        }
    }
}
