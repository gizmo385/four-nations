//! `fn_tui --watch LOG` (#319): follows a live agent run log or replays a finished one in the
//! normal TUI (docs/design/agent-play.md, "Watching"). The log is JSON Lines in the shape
//! `crate::surface::{RunLogHeader, LogLine}` defines -- the one schema both `fn_tui --watch` and
//! `fn_agent serve`/`keys` use (#324) -- a header line, then one `Keys`/`Look`/`Inspect`/`Turn`
//! event a line. [`Watcher::new`] refuses a header whose `data_digest` doesn't match the data
//! here, the same rule a save file follows.
//!
//! Replay goes through the same [`Shell`] a human's keyboard drives ([`Watcher::advance`]
//! presses the logged keys itself), animated at a readable rate instead of a human's hand, so
//! the tool banner, cursor walk, drag rectangle, designation, Build ghost, research selection and
//! selection bar all show as if a player made them. The bottom status row becomes an *agent
//! strip* instead of the key hints ([`Watcher::render`]), since a watcher's own keys are watcher
//! controls, not game keys: Space pauses, `+`/`-` change the pace, `.` steps one action, `]`
//! jumps to the end of the next turn and `q` quits (`fn_tui`'s interactive loop reads these;
//! nothing here does).

use std::collections::VecDeque;
use std::fs::File;
use std::io::{self, Read};
use std::path::Path;

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use fn_sim::world::TICKS_PER_SECOND;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};

use crate::app::App;
use crate::cli::{self, Args};
use crate::color::{ColorDepth, Palette};
use crate::frame::render_shell;
use crate::keys;
use crate::layout;
use crate::shell::{Mode, Shell};
use crate::state::UiState;
use crate::surface::{LogLine, RunLogHeader};

/// Keys a second at pace x1: about a readable rate (docs/design/agent-play.md, "Watching": "the
/// real keys, animated... about 20 keys a second").
pub const KEYS_PER_SECOND: f64 = 20.0;

/// A plain-arrow run this long or longer (two seconds at [`KEYS_PER_SECOND`]) gets compressed
/// into Shift-steps once it still has at least this many keys left to go
/// (docs/design/agent-play.md, "Watching": "a walk longer than 2 seconds is compressed into
/// Shift-steps"); the tail end still lands one plain step at a time, so a walk settles onto its
/// last tile visibly instead of snapping there.
const COMPRESS_FROM: usize = (KEYS_PER_SECOND * 2.0) as usize;

/// How many plain steps one compressed Shift-step stands for (Shift+arrow moves ten tiles, the
/// same as `keys::parse`'s `<s-right>`).
const SHIFT_STRIDE: usize = 10;

/// The watch pace ladder: a multiple of [`KEYS_PER_SECOND`] and of how fast turns fast-forward.
/// `+`/`-` step through it; the first is the default.
pub const PACES: [u32; 6] = [1, 2, 5, 10, 20, 40];

/// How long an attention ghost (a `look`'s region or an `inspect`'s tile outline) stays on
/// screen after the watcher reaches it, in real seconds.
const GHOST_SECONDS: f32 = 1.5;

// ------------------------------------------------------------------------------------------
// Reading the log
// ------------------------------------------------------------------------------------------

/// Parses a run log's full text so far: the first line must be a [`LogLine::Header`], and every
/// line after it one more [`LogLine`]. A trailing line with no newline after it (a live log
/// caught mid-write) is dropped rather than failing the whole parse, so a grower's latest,
/// not-yet-flushed line just isn't there yet.
pub fn parse(text: &str) -> Result<(RunLogHeader, Vec<LogLine>), String> {
    let complete = match text.rfind('\n') {
        Some(i) => &text[..i],
        None => return Err("empty run log".to_owned()),
    };
    let mut lines = complete.lines().filter(|l| !l.trim().is_empty());
    let first = lines.next().ok_or("empty run log")?;
    let header = match parse_line(first)? {
        LogLine::Header(h) => h,
        other => return Err(format!("the run log's first line must be a header, got {other:?}")),
    };
    let events = lines.map(parse_line).collect::<Result<Vec<_>, _>>()?;
    Ok((header, events))
}

fn parse_line(line: &str) -> Result<LogLine, String> {
    serde_json::from_str(line).map_err(|e| format!("{line:?}: {e}"))
}

/// Reads whatever of `path` is on disk right now, parsed the same way [`parse`] does: for
/// `--watch LOG --frame`, where the log is already finished (#319's acceptance criteria).
pub fn read_file(path: &Path) -> Result<(RunLogHeader, Vec<LogLine>), String> {
    let text = std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
    // `parse` wants its text to end past the last complete line; a finished file's last
    // character already is one, but appending a newline costs nothing if it somehow isn't.
    parse(&format!("{text}\n"))
}

/// `lines` as a `--keys` script (`fn_agent keys`, docs/design/agent-play.md, "Racing"): every
/// `Keys` entry's own script, in order, and `<tick N>` for every `Turn` (`header.ticks_per_turn`,
/// the same ticks `Host::end_turn` ran) -- what `fn_tui SEED NATION --frame --keys "..."`
/// replays to reach the same frame. `Look` and `Inspect` leave nothing to replay: `Shell::key`
/// has no equivalent for either.
pub fn to_keys_script(header: &RunLogHeader, lines: &[LogLine]) -> String {
    let mut tokens = Vec::new();
    for line in lines {
        match line {
            LogLine::Keys { keys, .. } => tokens.push(keys.clone()),
            LogLine::Turn { .. } => tokens.push(format!("<tick {}>", header.ticks_per_turn)),
            LogLine::Look { .. } | LogLine::Inspect { .. } | LogLine::Header(_) => {}
            // The end marker (#327) is a fact about the log, not a key anyone pressed: it
            // leaves nothing to replay.
            LogLine::End { .. } => {}
            // #479's own commentary (`LogLine`'s own doc comment): not a key either.
            LogLine::Script { .. } | LogLine::Wake { .. } | LogLine::Note { .. } => {}
        }
    }
    tokens.join(" ")
}

/// Tails a growing run log file: every call to [`Self::poll`] returns whatever whole lines have
/// landed since the last one, leaving a not-yet-newline-terminated tail buffered for next time
/// (docs/design/agent-play.md, "Watching": "If the log is still growing, it follows it live").
pub struct LogTail {
    file: File,
    buf: String,
}

impl LogTail {
    pub fn open(path: &Path) -> io::Result<Self> {
        Ok(Self { file: File::open(path)?, buf: String::new() })
    }

    /// The whole lines that have landed since the last call (empty if none have).
    pub fn poll(&mut self) -> io::Result<Vec<String>> {
        let mut chunk = String::new();
        self.file.read_to_string(&mut chunk)?;
        self.buf.push_str(&chunk);
        let mut lines = Vec::new();
        while let Some(i) = self.buf.find('\n') {
            lines.push(self.buf[..i].to_owned());
            self.buf.drain(..=i);
        }
        Ok(lines)
    }
}

// ------------------------------------------------------------------------------------------
// The watcher
// ------------------------------------------------------------------------------------------

/// What the agent strip shows while a `Keys` entry's script is still being pressed: its label
/// (or, with none, the raw keys) and note (docs/design/agent-play.md, "Watching": "An agent
/// strip... shows... the current action's label, the agent's note").
#[derive(Clone, Debug, PartialEq)]
struct CurrentAction {
    label: Option<String>,
    note: Option<String>,
    keys: Vec<KeyEvent>,
    pressed: usize,
}

/// What [`Watcher`] is mid-way through right now, spent gradually by [`Watcher::advance`]'s
/// budget instead of all at once (#328): a `Keys` entry's script, pressed key by key, or a
/// turn's ticks, run a few at a time like the normal game clock instead of in one uncapped burst.
#[derive(Clone, Debug, PartialEq)]
enum Current {
    Keys(CurrentAction),
    /// A turn's `remaining` ticks, out of `turn_seconds * TICKS_PER_SECOND`, counting down to
    /// zero as [`Watcher::advance`] spends its budget on them.
    Ticks {
        remaining: u32,
    },
}

/// A `look`'s region or an `inspect`'s tile, briefly outlined on the map
/// (docs/design/agent-play.md, "Watching": "attention ghosts").
#[derive(Clone, Copy, Debug, PartialEq)]
struct Ghost {
    x: i32,
    y: i32,
    w: u16,
    h: u16,
    left: f32,
}

/// Replays a run log into a real [`Shell`], animated (docs/design/agent-play.md, "Watching").
/// Does its own thing with time: [`Self::advance`] takes real seconds, the same shape
/// `Shell::advance` does for a normal game, but spends them on *replay* pace instead of the
/// sim's own clock (which a watched session never reads: see `surface::Host::refuses`-refused
/// keys, none of which this ever presses either).
pub struct Watcher {
    shell: Shell,
    turn_seconds: u32,
    pending: VecDeque<LogLine>,
    current: Option<Current>,
    ghost: Option<Ghost>,
    /// Real seconds owed at the current pace, carried between [`Self::advance`] calls
    /// (`fn_ui::Clock::advance`'s own pattern): spent on keys (at [`KEYS_PER_SECOND`]) or on a
    /// turn's ticks (at `TICKS_PER_SECOND`, #328), whichever [`Self::current`] is.
    owed: f64,
    pace_index: usize,
    paused: bool,
    turn: u32,
    /// How many real seconds [`Self::advance`] has had nothing to do (pending empty, nothing
    /// mid-press): a live log's "waiting for agent (N s)".
    waiting_secs: f64,
    nation_label: String,
    /// The log's `End` line's own reason, once replayed (#327): a finished run's strip reads
    /// "run finished" instead of counting up a wait that will never end.
    finished: Option<String>,
    /// The latest act's strip text (its label and note), kept on the strip while that act's turn
    /// plays out, so the agent's note stays readable for longer than its keys take to press.
    last_action: Option<String>,
    /// Free-look spectating (#330): a second, separate `UiState` over the same `Session` as
    /// [`Self::shell`]'s own game, so a spectator moving around, opening screens or selecting a
    /// unit never touches what [`Self::advance`] replays. `crate::spectate::key` is the only way
    /// into it; nothing here ever reaches it through anything that takes `&mut Session`.
    spectator: UiState,
    /// Whether the frame draws the agent's own view (`true`, cursor, tool banner and ghosts as
    /// `Shell::draw` always has) or [`Self::spectator`]'s (`false`): the strip says which
    /// (`FOLLOW`/`FREE`, #330's acceptance criteria). Starts in Follow; [`Self::spectator_key`]
    /// switches to Free the moment any spectator action does anything at all, since otherwise
    /// its effect would never show.
    follow: bool,
}

impl Watcher {
    /// Builds the session `header` describes (the way `surface::Host::new` does), with nothing
    /// yet to replay: feed it events with [`Self::feed`]. Refuses a log made against different
    /// data (`header.data_digest` against the bundled data's own `Defs::id_hash` -- #324's
    /// acceptance criteria), the same rule `fn_sim::save` applies to a save file: a replay only
    /// holds for the same data a header's `Host` was built against.
    pub fn new(header: &RunLogHeader) -> Result<Self, String> {
        let args = Args {
            seed: Some(header.seed),
            nation: Some(header.nation),
            frame: false,
            size: cli::DEFAULT_SIZE,
            ticks: 0,
            keys: Vec::new(),
            ascii: false,
            ansi: false,
            color: None,
            embark: false,
            start: header.site,
            map_size: header.map_size.clone(),
            rough: header.rough,
            difficulty: header.difficulty.clone(),
            fast: false,
            scenario: None,
            watch: None,
            pace: None,
            at: None,
        };
        let mut shell = Shell::for_args(&args, cli::DEFAULT_SIZE, None, 0)?;
        let digest = shell.defs().id_hash();
        if digest != header.data_digest {
            return Err(format!(
                "this run log was made against different data (digest {:#x}, the data here is \
                 {digest:#x}); it can't be replayed",
                header.data_digest
            ));
        }
        // `header.prelude` (#481): run it again, exactly as the live `Host` that wrote this log
        // did, before `Self::feed` replays anything -- the logged lines only ever cover turn 0
        // on.
        if let (Some(prelude), Mode::Game(app)) = (header.prelude, &mut shell.mode) {
            crate::surface::prelude::run(app, &prelude);
        }
        let spectator = match &shell.mode {
            Mode::Game(app) => UiState::new(&app.session, cli::DEFAULT_SIZE),
            _ => unreachable!("Shell::for_args with a seed and a nation always builds a game"),
        };
        Ok(Self {
            shell,
            turn_seconds: header.turn_seconds.max(1),
            pending: VecDeque::new(),
            current: None,
            ghost: None,
            owed: 0.0,
            pace_index: 0,
            paused: false,
            turn: 0,
            waiting_secs: 0.0,
            nation_label: header.nation.as_str().to_owned(),
            finished: None,
            last_action: None,
            spectator,
            follow: true,
        })
    }

    /// Queues more of the log to replay, oldest first.
    pub fn feed(&mut self, events: impl IntoIterator<Item = LogLine>) {
        self.pending.extend(events);
    }

    fn app(&self) -> &App {
        match &self.shell.mode {
            Mode::Game(app) => app,
            _ => unreachable!("Watcher::new always builds a game"),
        }
    }

    fn app_mut(&mut self) -> &mut App {
        match &mut self.shell.mode {
            Mode::Game(app) => app,
            _ => unreachable!("Watcher::new always builds a game"),
        }
    }

    pub fn turn(&self) -> u32 {
        self.turn
    }

    /// Nothing left to replay and nothing mid-press: a finished log's watcher has reached the
    /// end (though a live one might still grow).
    pub fn caught_up(&self) -> bool {
        self.pending.is_empty() && self.current.is_none()
    }

    /// Whether the log's `End` line (#327) has been replayed: the run is over for good, not
    /// merely caught up with a live agent that might still write more.
    pub fn finished(&self) -> bool {
        self.finished.is_some()
    }

    pub fn toggle_pause(&mut self) {
        self.paused = !self.paused;
    }

    pub fn faster(&mut self) {
        self.pace_index = (self.pace_index + 1).min(PACES.len() - 1);
    }

    pub fn slower(&mut self) {
        self.pace_index = self.pace_index.saturating_sub(1);
    }

    pub fn pace(&self) -> u32 {
        PACES[self.pace_index]
    }

    pub fn defs(&self) -> &fn_sim::Defs {
        self.shell.defs()
    }

    pub fn nation(&self) -> fn_sim::Nation {
        self.shell.nation()
    }

    pub fn resize(&mut self, width: u16, height: u16) {
        self.shell.resize(width, height);
        self.spectator.resize(width, height);
    }

    // --------------------------------------------------------------------------------------
    // Free-look spectating (#330)
    // --------------------------------------------------------------------------------------

    /// Whether the frame draws the agent's own view (`true`) or the spectator's free-look one
    /// (`false`).
    pub fn following(&self) -> bool {
        self.follow
    }

    /// `V`: explicitly switches between Follow and Free look, whatever either is doing.
    pub fn toggle_view(&mut self) {
        self.follow = !self.follow;
    }

    /// Every other key, while a spectator is driving: applies it to [`Self::spectator`]'s own
    /// `UiState` through `crate::spectate::key`, which only ever reads the `Session` -- the one
    /// path into this that the command-log equality test (`tests::spectating_never_changes_the_
    /// replay`) exists to prove never reaches it. Switches to Free look the moment anything at
    /// all happens, so an action's effect is never invisible because Follow is still showing the
    /// agent's own view (#330's acceptance criteria: "Moving the camera or cursor switches to
    /// free look automatically").
    pub fn spectator_key(&mut self, key: KeyEvent) {
        let session = match &self.shell.mode {
            Mode::Game(app) => &app.session,
            _ => unreachable!("Watcher::new always builds a game"),
        };
        if crate::spectate::key(&mut self.spectator, session, key) {
            self.follow = false;
        }
    }

    /// `V` (the Follow/Free toggle) or [`Self::spectator_key`]: the one entry point for every
    /// key a spectator might press, whether typed live (`run`'s own loop, once the watcher's
    /// own controls don't match it first) or played by `--keys` before a `--watch --frame`, so
    /// free look can be golden-tested without a terminal (#330's acceptance criteria).
    pub fn free_look_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Char('v' | 'V') => self.toggle_view(),
            _ => self.spectator_key(key),
        }
    }

    // --------------------------------------------------------------------------------------
    // Replay
    // --------------------------------------------------------------------------------------

    /// Spends `real_secs` of real time animating the log at the current pace: `Keys` entries
    /// press at about `pace * 20` keys a second (compressing a long walk into Shift-steps,
    /// [`COMPRESS_FROM`]), and a turn's ticks run at `pace` game seconds a real second -- x1 is
    /// real time, x5 is five game seconds a second, the same ladder the normal game clock uses
    /// (`fn_ui::Clock::advance`), instead of a whole turn landing the instant it's reached (#328:
    /// the owner's "nothing happens while the agent is thinking and then everything happens very
    /// fast"). `Look` and `Inspect` (which took no game time) apply the moment they're reached,
    /// uncapped, since nothing paces them but the keys and ticks around them. Does nothing while
    /// [`Self::toggle_pause`]d, except keep the attention ghost ageing.
    pub fn advance(&mut self, real_secs: f64) {
        self.age_ghost(real_secs);
        if self.paused || self.finished() {
            return;
        }
        if self.caught_up() {
            self.waiting_secs += real_secs;
            return;
        }
        self.waiting_secs = 0.0;
        let mut owed = self.owed + real_secs.max(0.0) * f64::from(self.pace());
        loop {
            if self.current.is_none() && !self.start_next() {
                break; // caught up mid-budget
            }
            if let Some(Current::Keys(_)) = &self.current {
                let cost = 1.0 / KEYS_PER_SECOND;
                if owed < cost {
                    break;
                }
                owed -= cost;
                self.press_one();
                continue;
            }
            if let Some(Current::Ticks { remaining }) = &self.current {
                let remaining = *remaining;
                let cost = 1.0 / f64::from(TICKS_PER_SECOND);
                if owed < cost {
                    break;
                }
                let affordable = (owed / cost).floor() as u32;
                let run = affordable.max(1).min(remaining);
                owed -= f64::from(run) * cost;
                self.spend_ticks(run);
                continue;
            }
            // `start_next` just applied a `Look`/`Inspect`/`End` directly (they take no game
            // time, so nothing paces them): loop again, either to start the next thing or to
            // spend the budget on it.
        }
        self.owed = owed;
    }

    /// Completes exactly one more act at once, ignoring the pace: the watcher's own `.` key
    /// (docs/design/agent-play.md, "Watching": "`.` steps one action"). Finishes
    /// [`Self::current`] if one is already mid-press (every remaining key of a `Keys` act, or
    /// every remaining tick of a turn), rather than starting yet another.
    pub fn step_one(&mut self) {
        if self.current.is_some() {
            self.finish_current();
            return;
        }
        self.start_next();
    }

    /// Replays everything up to and including the end of turn `turn`, all at once -- for
    /// `--watch LOG --frame --at TURN` (#319's acceptance criteria), where nothing is animated
    /// because there is no terminal to animate it in. Stops early if the log doesn't reach that
    /// turn yet.
    pub fn fast_forward_to_turn(&mut self, turn: u32) {
        while self.turn < turn {
            if self.current.is_some() {
                self.finish_current();
                continue;
            }
            if !self.start_next() {
                break;
            }
        }
    }

    /// Finishes whatever's mid-progress right now, in one shot and ignoring pace: every
    /// remaining key of a `Keys` act ([`Self::press_one`], repeated), or every remaining tick of
    /// a turn ([`Self::spend_ticks`], all at once) -- [`Self::step_one`] and
    /// [`Self::fast_forward_to_turn`]'s shared way of draining [`Self::current`].
    fn finish_current(&mut self) {
        match &self.current {
            Some(Current::Keys(_)) => {
                while matches!(self.current, Some(Current::Keys(_))) {
                    self.press_one();
                }
            }
            Some(Current::Ticks { remaining }) => {
                let remaining = *remaining;
                self.spend_ticks(remaining);
            }
            None => {}
        }
    }

    /// How many turns the log already holds (as queued `LogLine::Turn` entries) that
    /// [`Self::advance`] hasn't reached yet: a live log's backlog once the agent writes turns
    /// faster than the chosen pace plays them back (#328's acceptance criteria: "the strip says
    /// how far"). Zero whenever the watcher is keeping up.
    pub fn turns_behind(&self) -> u32 {
        self.pending.iter().filter(|l| matches!(l, LogLine::Turn { .. })).count() as u32
    }

    /// Pops the next pending entry and starts it: `Keys` and `Turn` become [`Self::current`]
    /// (nothing pressed or ticked yet -- [`Self::advance`] spends its budget on them next),
    /// while `Look`/`Inspect` set the attention ghost and log an Agent line at once, since they
    /// take no game time. `false` with nothing pending.
    fn start_next(&mut self) -> bool {
        let Some(entry) = self.pending.pop_front() else { return false };
        match entry {
            LogLine::Keys { keys, label, note } => {
                let Ok(steps) = keys::parse(&keys) else { return self.start_next() };
                let keys = steps
                    .into_iter()
                    .filter_map(|s| match s {
                        keys::Step::Key(k) => Some(k),
                        keys::Step::Tick(_) => None,
                    })
                    .collect::<Vec<_>>();
                if keys.is_empty() {
                    return self.start_next();
                }
                let action = CurrentAction { label, note, keys, pressed: 0 };
                self.last_action = Some(describe_action_line(&action));
                self.current = Some(Current::Keys(action));
            }
            LogLine::Look { x, y, w, h } => {
                self.ghost = Some(Ghost { x, y, w, h, left: GHOST_SECONDS });
                let tick = self.app().session.world.tick;
                self.app_mut()
                    .ui
                    .alerts
                    .push_agent(tick, format!("Agent looked at ({x}, {y}) {w}x{h}"));
            }
            LogLine::Inspect { x, y } => {
                self.ghost = Some(Ghost { x, y, w: 1, h: 1, left: GHOST_SECONDS });
                let tick = self.app().session.world.tick;
                self.app_mut().ui.alerts.push_agent(tick, format!("Agent inspected ({x}, {y})"));
            }
            LogLine::Turn { .. } => {
                let ticks = self.turn_seconds * TICKS_PER_SECOND;
                self.current = Some(Current::Ticks { remaining: ticks });
            }
            // The run is over (#327): nothing left to replay, ever, however much more of the
            // log there might be. Logged once, like `Look`/`Inspect`, since it takes no game
            // time either.
            LogLine::End { reason, .. } => {
                let tick = self.app().session.world.tick;
                self.app_mut().ui.alerts.push_agent(tick, format!("Run finished: {reason}"));
                self.finished = Some(reason);
            }
            LogLine::Header(_) => return self.start_next(), // shouldn't recur; harmless if it did
            // #479's own commentary (`LogLine`'s own doc comment): takes no game time and has
            // no display of its own yet (#482), so skip straight to whatever's next.
            LogLine::Script { .. } | LogLine::Wake { .. } | LogLine::Note { .. } => {
                return self.start_next();
            }
        }
        true
    }

    /// Presses [`Self::current`]'s next key (or, deep in a long plain-arrow run, a whole
    /// Shift-step's worth at once, [`COMPRESS_FROM`]), finishing and logging the act once every
    /// key is pressed. A no-op if [`Self::current`] isn't a `Keys` act.
    fn press_one(&mut self) {
        let Some(Current::Keys(action)) = &self.current else { return };
        let remaining = &action.keys[action.pressed..];
        let (key, advance_by) = next_press(remaining);
        self.shell.key(key);
        let Some(Current::Keys(action)) = self.current.as_mut() else {
            unreachable!("just matched Some(Current::Keys) above")
        };
        action.pressed += advance_by;
        if action.pressed >= action.keys.len() {
            let Some(Current::Keys(CurrentAction { label, note, keys, .. })) = self.current.take()
            else {
                unreachable!("just matched Some(Current::Keys) above")
            };
            let script = keys.iter().map(|&k| crate::surface::key_token(k)).collect::<Vec<_>>();
            let tick = self.app().session.world.tick;
            self.app_mut().ui.alerts.push_agent(tick, describe_act(&label, &note, &script));
        }
    }

    /// Runs `run` of the current turn's remaining ticks ([`Current::Ticks`]), advancing
    /// [`Self::turn`] once none are left. A no-op if [`Self::current`] isn't a turn's ticks, or
    /// if `run` is 0.
    fn spend_ticks(&mut self, run: u32) {
        if run == 0 || !matches!(self.current, Some(Current::Ticks { .. })) {
            return;
        }
        self.app_mut().run_ticks(run);
        let Some(Current::Ticks { remaining }) = &mut self.current else {
            unreachable!("just matched Some(Current::Ticks) above")
        };
        *remaining = remaining.saturating_sub(run);
        if *remaining == 0 {
            self.current = None;
            self.turn += 1;
        }
    }

    fn age_ghost(&mut self, real_secs: f64) {
        if let Some(ghost) = &mut self.ghost {
            ghost.left -= real_secs as f32;
            if ghost.left <= 0.0 {
                self.ghost = None;
            }
        }
    }

    // --------------------------------------------------------------------------------------
    // Drawing
    // --------------------------------------------------------------------------------------

    /// The frame for this watcher's session, with the agent strip over the bottom status row
    /// instead of the key hints, and an attention ghost outlined over the map, if one is live
    /// (docs/design/agent-play.md, "Watching"). In Follow this is the agent's own view, as
    /// always; in Free look (#330) it's the spectator's own `UiState` instead, with the agent's
    /// own cursor outlined on the map too, the same look an attention ghost wears, so a
    /// spectator looking elsewhere can still see where the agent is pointing.
    pub fn render(&mut self, palette: &Palette, size: (u16, u16)) -> Buffer {
        self.resize(size.0, size.1);
        let mut buf = if self.follow {
            render_shell(&mut self.shell, palette, size)
        } else {
            crate::frame::render(&self.app().session, &self.spectator, palette, size)
        };
        if let Some(regions) = layout::regions(Rect::new(0, 0, size.0, size.1)) {
            self.draw_strip(&mut buf, regions.status);
            let view = if self.follow { self.app().ui.view } else { self.spectator.view };
            if let Some(ghost) = self.ghost {
                draw_ghost(&view, ghost, regions.map, &mut buf);
            }
            if !self.follow {
                let agent = self.app().ui.cursor;
                let marker = Ghost { x: agent.x, y: agent.y, w: 1, h: 1, left: 0.0 };
                draw_ghost(&view, marker, regions.map, &mut buf);
            }
        }
        buf
    }

    fn draw_strip(&self, buf: &mut Buffer, area: Rect) {
        let style = Style::new().add_modifier(Modifier::REVERSED);
        for x in area.left()..area.right() {
            buf[(x, area.y)].set_char(' ').set_style(style);
        }
        let text = self.strip_text();
        buf.set_stringn(area.x, area.y, &text, usize::from(area.width), style);
    }

    /// " AGENT earth  FOLLOW  turn 6  02:00  designate harvest 6x4 at (86,60) "wood for two
    /// beds"  x40": the latest act stays on the strip while its turn plays out. A live log's
    /// agent that has written turns faster than the chosen pace plays them back adds "  3 turns
    /// behind" ([`Self::turns_behind`], #328), "waiting for agent (12 s)" shows once the watcher
    /// has caught up, and "run finished" once the log's own `End` line has been replayed (#327):
    /// a finished run never says "waiting" for a wait that will never end. `FOLLOW`/`FREE`
    /// (#330) says which view is on, right after the nation.
    fn strip_text(&self) -> String {
        let clock = fn_ui::view::format_clock(self.app().session.world.tick);
        let behind = self.turns_behind();
        let latest = self.last_action.clone().unwrap_or_default();
        let action = if self.finished() {
            "run finished".to_owned()
        } else if behind > 0 {
            format!("{latest}  {behind} turn{} behind", if behind == 1 { "" } else { "s" })
        } else if self.current.is_none() && self.caught_up() {
            format!("waiting for agent ({} s)", self.waiting_secs.round() as u64)
        } else {
            latest
        };
        let view = if self.follow { "FOLLOW" } else { "FREE" };
        format!(
            " AGENT {}  {view}  turn {}  {clock}  {action}  x{}",
            self.nation_label,
            self.turn,
            self.pace()
        )
    }
}

/// The key to press next, and how many of `remaining`'s keys it accounts for: a whole
/// [`SHIFT_STRIDE`] at once (a Shift-step) while a plain-arrow run still has [`COMPRESS_FROM`] or
/// more of itself left to go, one key otherwise.
fn next_press(remaining: &[KeyEvent]) -> (KeyEvent, usize) {
    let first = remaining[0];
    if is_plain_arrow(first) {
        let run = remaining.iter().take_while(|&&k| k == first).count();
        if run >= COMPRESS_FROM {
            let code = first.code;
            return (KeyEvent::new(code, KeyModifiers::SHIFT), SHIFT_STRIDE.min(run));
        }
    }
    (first, 1)
}

fn is_plain_arrow(key: KeyEvent) -> bool {
    key.modifiers == KeyModifiers::NONE
        && matches!(key.code, KeyCode::Up | KeyCode::Down | KeyCode::Left | KeyCode::Right)
}

/// The agent strip's text for the action still being pressed: its label, or (raw `keys`, #317
/// until named actions, #316, land) the script itself, then its note in quotes.
fn describe_action_line(action: &CurrentAction) -> String {
    let script = action.keys.iter().map(|&k| crate::surface::key_token(k)).collect::<Vec<_>>();
    let what = action.label.clone().unwrap_or_else(|| script.join(" "));
    match &action.note {
        Some(note) if !note.is_empty() => format!("{what} \"{note}\""),
        _ => what,
    }
}

/// The message log's (`L`) line for a finished act: its label (or the keys it pressed), and its
/// note in quotes, if it had one.
fn describe_act(label: &Option<String>, note: &Option<String>, script: &[String]) -> String {
    let what = label.clone().unwrap_or_else(|| script.join(" "));
    match note {
        Some(note) if !note.is_empty() => format!("Agent: {what} \"{note}\""),
        _ => format!("Agent: {what}"),
    }
}

/// Outlines `ghost`'s region (or, 1x1, just its tile) in reverse video, over whichever of its
/// cells are in `view`'s camera and inside `area` (the map region): the border only, so it
/// reads as an outline rather than hiding what's under it.
fn draw_ghost(view: &fn_sim::Pos, ghost: Ghost, area: Rect, buf: &mut Buffer) {
    let style = Style::new().add_modifier(Modifier::REVERSED);
    for row in 0..ghost.h {
        for col in 0..ghost.w {
            let on_border = row == 0 || row + 1 == ghost.h || col == 0 || col + 1 == ghost.w;
            if !on_border {
                continue;
            }
            let (wx, wy) = (ghost.x + i32::from(col), ghost.y + i32::from(row));
            let (sx, sy) = (wx - view.x, wy - view.y);
            if sx < 0 || sy < 0 || sx >= i32::from(area.width) || sy >= i32::from(area.height) {
                continue;
            }
            let (x, y) = (area.x + sx as u16, area.y + sy as u16);
            let patched = buf[(x, y)].style().patch(style);
            buf[(x, y)].set_style(patched);
        }
    }
}

// ------------------------------------------------------------------------------------------
// The interactive loop
// ------------------------------------------------------------------------------------------

/// `fn_tui --watch LOG` (docs/design/agent-play.md, "Watching"): opens `path`, waits for its
/// header (retrying while the file doesn't exist yet, or is empty, so starting the watcher
/// before the agent has written anything just waits), then replays it live in a real terminal.
/// Real wall time becomes the watcher's own replay pace (never the sim's clock, which a watched
/// session never reads); the render is throttled to 30 fps; the watcher's own keys steer the
/// replay (Space pauses, `+`/`-` change the pace, `.` steps one action, `]` jumps to the end of
/// the next turn, `q` quits) and every other key is ignored, since a watcher's keys are never
/// the game's. The other place besides `run::run` that reads the wall clock.
pub fn run(
    path: &Path,
    depth: ColorDepth,
    ascii: bool,
    initial_pace: Option<u32>,
) -> io::Result<()> {
    use std::time::{Duration, Instant};

    use crossterm::event::{Event, KeyEventKind};

    const FRAME: Duration = Duration::from_millis(33);
    const RETRY: Duration = Duration::from_millis(100);

    let _restore = crate::run::Restore;
    let mut terminal = crate::run::setup()?;

    let mut tail = loop {
        match LogTail::open(path) {
            Ok(tail) => break tail,
            Err(e) if e.kind() == io::ErrorKind::NotFound => std::thread::sleep(RETRY),
            Err(e) => return Err(e),
        }
    };
    let mut watcher = loop {
        let lines = tail.poll()?;
        let mut lines = lines.into_iter();
        let Some(first) = lines.next() else {
            std::thread::sleep(RETRY);
            continue;
        };
        let header = match parse_line(&first) {
            Ok(LogLine::Header(h)) => h,
            Ok(other) => {
                let message = format!("the run log's first line must be a header, got {other:?}");
                return Err(io::Error::new(io::ErrorKind::InvalidData, message));
            }
            Err(e) => return Err(io::Error::new(io::ErrorKind::InvalidData, e)),
        };
        let mut watcher =
            Watcher::new(&header).map_err(|e| io::Error::new(io::ErrorKind::InvalidInput, e))?;
        if let Some(pace) = initial_pace {
            while watcher.pace() < pace {
                watcher.faster();
            }
        }
        for line in lines {
            let event =
                parse_line(&line).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
            watcher.feed([event]);
        }
        break watcher;
    };

    let (w, h) = crossterm::terminal::size()?;
    watcher.resize(w, h);
    let palette = Palette::new(watcher.defs(), watcher.nation(), depth, ascii);

    let mut last_tick = Instant::now();
    let mut quit = false;
    while !quit {
        for line in tail.poll()? {
            let event =
                parse_line(&line).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
            watcher.feed([event]);
        }

        terminal.draw(|f| {
            let area = f.area();
            let buf = watcher.render(&palette, (area.width, area.height));
            f.buffer_mut().clone_from(&buf);
        })?;

        let wait = FRAME;
        let mut pending = crossterm::event::poll(wait)?;
        while pending {
            match crossterm::event::read()? {
                Event::Key(key) if key.kind != KeyEventKind::Release => {
                    use crossterm::event::KeyCode;
                    match key.code {
                        // The watcher's own controls (docs/design/agent-play.md, "Watching")
                        // win over anything the game or the spectator would otherwise do with
                        // the same key (#330's acceptance criteria).
                        KeyCode::Char(' ') => watcher.toggle_pause(),
                        KeyCode::Char('+' | '=') => watcher.faster(),
                        KeyCode::Char('-' | '_') => watcher.slower(),
                        KeyCode::Char('.') => watcher.step_one(),
                        KeyCode::Char(']') => watcher.fast_forward_to_turn(watcher.turn() + 1),
                        KeyCode::Char('q' | 'Q') => quit = true,
                        // Every other key is free look's: `V` (not an F-key, since the owner's
                        // WSL terminal never sees those) toggles the view, and everything else
                        // is the spectator's own, read-only (`Watcher::free_look_key`).
                        _ => watcher.free_look_key(key),
                    }
                }
                // A watched session's nation never changes (there's no key for it, #319's
                // refused list), so the palette built at the start stays right.
                Event::Resize(w, h) => watcher.resize(w, h),
                _ => {}
            }
            pending = crossterm::event::poll(Duration::ZERO)?;
        }

        let now = Instant::now();
        watcher.advance(now.duration_since(last_tick).as_secs_f64());
        last_tick = now;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use fn_sim::Nation;

    use super::*;
    use crate::surface::Host;

    fn header(nation: Nation) -> RunLogHeader {
        RunLogHeader {
            seed: 7,
            nation,
            turn_seconds: 10,
            ticks_per_turn: 10 * TICKS_PER_SECOND,
            map_size: None,
            difficulty: None,
            site: None,
            rough: false,
            data_digest: fn_sim::Defs::bundled().id_hash(),
            crate_version: env!("CARGO_PKG_VERSION").to_owned(),
            prelude: None,
        }
    }

    /// `Watcher::new` refuses a log whose `data_digest` doesn't match the data here, with a
    /// clear message (#324's acceptance criteria).
    #[test]
    fn a_mismatched_data_digest_is_refused() {
        let mut h = header(Nation::Earth);
        h.data_digest = h.data_digest.wrapping_add(1);
        let err = match Watcher::new(&h) {
            Ok(_) => panic!("a mismatched digest should be refused"),
            Err(e) => e,
        };
        assert!(err.contains("different data"), "{err}");
    }

    #[test]
    fn parse_reads_the_header_and_every_whole_line() {
        let mut h = Host::new(7, Nation::Earth, None, None, None, false).unwrap();
        h.press_keys("h <right*3> <enter>", Some("harvest".into()), None).unwrap();
        h.end_turn(1);
        let text = h.log().to_jsonl(&h.header());
        let (got_header, events) = parse(&text).unwrap();
        assert_eq!(got_header, h.header());
        assert_eq!(events, h.log().lines());
    }

    /// A trailing, not-yet-newline-terminated line (a live log caught mid-write) is left out
    /// rather than failing the parse (#319's acceptance criteria: "If the log is still growing,
    /// it follows it live").
    #[test]
    fn a_dangling_partial_line_is_dropped_not_an_error() {
        let h = Host::new(7, Nation::Earth, None, None, None, false).unwrap();
        let mut text = h.log().to_jsonl(&h.header());
        text.push_str("{\"type\":\"keys\",\"keys\":\"h\""); // no closing brace, no newline
        let (got_header, events) = parse(&text).unwrap();
        assert_eq!(got_header, h.header());
        assert!(events.is_empty());
    }

    #[test]
    fn an_empty_log_is_an_error() {
        assert!(parse("").is_err());
    }

    /// `LogTail::poll` only returns whole lines, buffering a partial one for next time.
    #[test]
    fn log_tail_buffers_a_partial_line_until_it_is_complete() {
        let path =
            std::env::temp_dir().join(format!("fn_tui_watch_test_{}.jsonl", std::process::id()));
        std::fs::write(&path, "line one\nline two\npart").unwrap();
        let mut tail = LogTail::open(&path).unwrap();
        assert_eq!(tail.poll().unwrap(), vec!["line one", "line two"]);
        assert_eq!(tail.poll().unwrap(), Vec::<String>::new(), "the partial line isn't whole yet");
        let mut f = std::fs::OpenOptions::new().append(true).open(&path).unwrap();
        use std::io::Write;
        writeln!(f, "ial line\nline three").unwrap();
        assert_eq!(tail.poll().unwrap(), vec!["partial line", "line three"]);
        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn a_new_watcher_starts_a_game_at_turn_zero() {
        let w = Watcher::new(&header(Nation::Water)).unwrap();
        assert_eq!(w.turn(), 0);
        assert!(w.caught_up());
        assert_eq!(w.app().session.world.nation, Nation::Water);
    }

    /// Replaying the same calls `surface::tests` already covers gives the same frame as the
    /// `Host` that made the log: watching changes nothing (#319's acceptance criteria).
    #[test]
    fn fast_forwarding_the_whole_log_gives_the_same_frame_as_the_host_that_made_it() {
        let mut h =
            Host::new(7, Nation::Fire, None, None, None, false).unwrap().with_turn_seconds(10);
        h.press_keys(
            "h <right*3> <enter> <down*2> <enter>",
            Some("harvest".into()),
            Some("wood".into()),
        )
        .unwrap();
        h.end_turn(2);
        h.look(0, 0, 10, 10);
        h.inspect(5, 5);
        h.press_keys("x", None, None).unwrap();

        let mut w = Watcher::new(&h.header()).unwrap();
        w.feed(h.log().lines());
        w.fast_forward_to_turn(u32::MAX);
        assert!(w.caught_up());

        // `Host::new` builds its own frame's palette the same way (ASCII truecolor); nothing
        // here changes the bundled data, so the same construction matches it.
        let palette = crate::color::Palette::new(
            &fn_sim::Defs::bundled(),
            Nation::Fire,
            crate::color::ColorDepth::TrueColor,
            true,
        );
        let want = h.frame(100, 40);
        let got = crate::frame::to_text(&w.render(&palette, (100, 40)));
        // The last row (the agent strip) differs by design; compare everything above it.
        let trim_strip = |s: &str| s.lines().take(39).collect::<Vec<_>>().join("\n");
        assert_eq!(trim_strip(&got), trim_strip(&want));
    }

    #[test]
    fn next_press_compresses_a_long_plain_arrow_run_into_shift_steps() {
        let right = KeyEvent::new(KeyCode::Right, KeyModifiers::NONE);
        let run = vec![right; COMPRESS_FROM + 5];
        let (key, by) = next_press(&run);
        assert_eq!(key, KeyEvent::new(KeyCode::Right, KeyModifiers::SHIFT));
        assert_eq!(by, SHIFT_STRIDE);
    }

    #[test]
    fn next_press_falls_back_to_one_key_once_the_run_is_short() {
        let right = KeyEvent::new(KeyCode::Right, KeyModifiers::NONE);
        let run = vec![right; COMPRESS_FROM - 1];
        let (key, by) = next_press(&run);
        assert_eq!(key, right);
        assert_eq!(by, 1);
    }

    #[test]
    fn next_press_never_compresses_a_non_arrow_key() {
        let enter = KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE);
        let run = vec![enter; COMPRESS_FROM + 5];
        let (key, by) = next_press(&run);
        assert_eq!((key, by), (enter, 1));
    }

    /// `advance` presses keys gradually, not all at once, so the animation is actually readable
    /// (#319's acceptance criteria).
    #[test]
    fn advance_presses_keys_gradually_at_the_chosen_pace() {
        let mut w = Watcher::new(&header(Nation::Earth)).unwrap();
        w.feed([LogLine::Keys {
            keys: "<right*10>".to_owned(),
            label: Some("walk".into()),
            note: None,
        }]);
        w.advance(0.1); // at pace x1 (20 keys/s): about 2 keys
        let pressed_so_far = match w.current.as_ref() {
            Some(Current::Keys(a)) => a.pressed,
            other => panic!("not finished yet: {other:?}"),
        };
        assert!((1..=3).contains(&pressed_so_far), "{pressed_so_far}");
        w.advance(1.0); // plenty to finish the rest
        assert!(w.caught_up());
    }

    /// `advance` spends a turn's ticks gradually too (#328): at pace x1 (real time), a
    /// `turn_seconds`-long turn's worth of ticks should take about that many real seconds, not
    /// land the instant the `Turn` entry is reached.
    #[test]
    fn advance_spreads_a_turns_ticks_across_real_time_at_the_chosen_pace() {
        let h = header(Nation::Earth); // turn_seconds: 10
        let mut w = Watcher::new(&h).unwrap();
        w.feed([LogLine::Turn { index: 0, tick: u64::from(h.ticks_per_turn) }]);

        w.advance(1.0); // one real second at pace x1: about one game second of ticks
        assert_eq!(w.turn(), 0, "a ten-second turn shouldn't be done after one second");
        let ticks_so_far = w.app().session.world.tick;
        assert!(
            (TICKS_PER_SECOND / 2..=TICKS_PER_SECOND * 2).contains(&(ticks_so_far as u32)),
            "{ticks_so_far}"
        );

        w.advance(20.0); // plenty to finish the rest
        assert!(w.caught_up());
        assert_eq!(w.turn(), 1);
    }

    /// At a faster pace, the same turn's ticks land in proportionally less real time: x5 plays
    /// five game seconds a real second, the same ladder the normal game clock uses.
    #[test]
    fn a_faster_pace_spends_a_turns_ticks_proportionally_faster() {
        let h = header(Nation::Earth); // turn_seconds: 10
        let mut w = Watcher::new(&h).unwrap();
        w.feed([LogLine::Turn { index: 0, tick: u64::from(h.ticks_per_turn) }]);
        for _ in 0..2 {
            w.faster(); // pace x5 (PACES[2])
        }
        assert_eq!(w.pace(), 5);

        w.advance(1.0); // one real second at pace x5: about five game seconds of ticks
        assert_eq!(w.turn(), 0);
        let ticks_so_far = w.app().session.world.tick;
        let want = u64::from(TICKS_PER_SECOND) * 5;
        assert!((want / 2..=want * 2).contains(&ticks_so_far), "{ticks_so_far} vs about {want}");
    }

    /// `turns_behind` counts the turns a live log has already written but the watcher hasn't
    /// reached yet (#328's acceptance criteria: "the strip says how far"), and the strip shows
    /// it instead of "waiting" once it's nonzero.
    #[test]
    fn turns_behind_counts_queued_turns_not_yet_reached() {
        let mut w = Watcher::new(&header(Nation::Earth)).unwrap();
        assert_eq!(w.turns_behind(), 0);

        w.feed([
            LogLine::Turn { index: 0, tick: 100 },
            LogLine::Turn { index: 1, tick: 200 },
            LogLine::Turn { index: 2, tick: 300 },
        ]);
        assert_eq!(w.turns_behind(), 3);
        assert!(w.strip_text().contains("3 turns behind"), "{}", w.strip_text());

        w.fast_forward_to_turn(2);
        assert_eq!(w.turns_behind(), 1);
        assert!(w.strip_text().contains("1 turn behind"), "{}", w.strip_text());

        w.fast_forward_to_turn(100);
        assert_eq!(w.turns_behind(), 0);
    }

    /// The latest act's note stays on the strip while its turn's ticks play out, and a backlog is
    /// added after it rather than replacing it: the note is what a watcher is there to read.
    #[test]
    fn the_latest_note_stays_on_the_strip_through_its_turn_and_a_backlog() {
        let mut w = Watcher::new(&header(Nation::Earth)).unwrap();
        w.feed([
            LogLine::Keys { keys: "<right>".to_owned(), label: None, note: Some("scout".into()) },
            LogLine::Turn { index: 1, tick: 200 },
        ]);
        w.advance(1.0);
        assert!(matches!(w.current, Some(Current::Ticks { .. })), "mid-turn");
        assert!(w.strip_text().contains("\"scout\""), "{}", w.strip_text());

        w.feed([LogLine::Turn { index: 2, tick: 400 }]);
        let strip = w.strip_text();
        assert!(strip.contains("\"scout\"") && strip.contains("1 turn behind"), "{strip}");
    }

    /// Pausing freezes the replay but still ages the attention ghost down (it's cosmetic, not
    /// part of the replay).
    #[test]
    fn pausing_freezes_the_replay() {
        let mut w = Watcher::new(&header(Nation::Earth)).unwrap();
        w.feed([LogLine::Keys { keys: "<right*10>".to_owned(), label: None, note: None }]);
        w.toggle_pause();
        w.advance(10.0);
        assert!(!w.caught_up(), "paused: nothing should have been pressed");
    }

    /// `+`/`-` step through [`PACES`] and stop at both ends.
    #[test]
    fn faster_and_slower_stop_at_both_ends() {
        let mut w = Watcher::new(&header(Nation::Earth)).unwrap();
        assert_eq!(w.pace(), PACES[0]);
        w.slower();
        assert_eq!(w.pace(), PACES[0], "already the slowest");
        let mut seen = vec![w.pace()];
        for _ in 1..PACES.len() {
            w.faster();
            seen.push(w.pace());
        }
        assert_eq!(seen, PACES.to_vec());
        w.faster();
        assert_eq!(w.pace(), *PACES.last().unwrap(), "stays at the fastest");
    }

    /// A caught-up watcher's strip says it's waiting, and the wait grows with real time.
    #[test]
    fn a_caught_up_watcher_reports_waiting() {
        let mut w = Watcher::new(&header(Nation::Earth)).unwrap();
        w.advance(5.0);
        assert!(w.strip_text().contains("waiting for agent (5 s)"), "{}", w.strip_text());
    }

    /// `fast_forward_to_turn` stops at the requested turn, even with more log left to replay,
    /// and running off the end of a short log just stops instead of erroring (#319's acceptance
    /// criteria: "so a test checks the frame at a given turn").
    #[test]
    fn fast_forward_to_turn_stops_exactly_there() {
        let mut h = Host::new(7, Nation::Earth, None, None, None, false).unwrap();
        h.end_turn(1);
        h.end_turn(1);
        h.end_turn(1);
        let mut w = Watcher::new(&h.header()).unwrap();
        w.feed(h.log().lines());

        w.fast_forward_to_turn(2);
        assert_eq!(w.turn(), 2);
        assert!(!w.caught_up(), "turn 3 is still queued");

        w.fast_forward_to_turn(100);
        assert_eq!(w.turn(), 3, "the log only has three turns");
        assert!(w.caught_up());
    }

    /// `look` and `inspect` set an attention ghost at once (they take no game time) and log an
    /// Agent line.
    #[test]
    fn look_and_inspect_set_a_ghost_and_log_an_agent_line() {
        let mut h = Host::new(7, Nation::Earth, None, None, None, false).unwrap();
        h.look(5, 5, 3, 3);
        h.inspect(9, 9);
        let mut w = Watcher::new(&h.header()).unwrap();
        w.feed(h.log().lines());

        w.fast_forward_to_turn(1); // nothing to stop at turn 1, so this drains the queue
        assert!(w.caught_up());
        assert_eq!(w.ghost, Some(Ghost { x: 9, y: 9, w: 1, h: 1, left: GHOST_SECONDS }));
        let lines: Vec<&str> =
            w.app().ui.alerts.history().iter().map(|e| e.text.as_str()).collect();
        assert!(lines.iter().any(|l| l.contains("looked at (5, 5)")), "{lines:?}");
        assert!(lines.iter().any(|l| l.contains("inspected (9, 9)")), "{lines:?}");
    }

    /// Every finished `Keys` act logs an `Agent:` line naming its label and note.
    #[test]
    fn a_finished_act_logs_its_label_and_note() {
        let mut w = Watcher::new(&header(Nation::Earth)).unwrap();
        w.feed([LogLine::Keys {
            keys: "h".to_owned(),
            label: Some("harvest".into()),
            note: Some("for beds".into()),
        }]);
        w.advance(1.0);
        assert!(w.caught_up());
        let lines: Vec<&str> =
            w.app().ui.alerts.history().iter().map(|e| e.text.as_str()).collect();
        assert!(lines.iter().any(|l| l == &"Agent: harvest \"for beds\""), "{lines:?}");
    }

    // ----------------------------------------------------------------------------------------
    // Free-look spectating (#330)
    // ----------------------------------------------------------------------------------------

    /// A fresh watcher starts in Follow, and `V` switches it to Free look and back, whatever
    /// else is going on.
    #[test]
    fn toggle_view_switches_between_follow_and_free() {
        let mut w = Watcher::new(&header(Nation::Earth)).unwrap();
        assert!(w.following());
        w.toggle_view();
        assert!(!w.following());
        w.toggle_view();
        assert!(w.following());
    }

    /// Moving the spectator's cursor switches to Free look on its own, without needing `V`
    /// first (#330's acceptance criteria).
    #[test]
    fn moving_the_spectator_switches_to_free_look_automatically() {
        let mut w = Watcher::new(&header(Nation::Earth)).unwrap();
        assert!(w.following());
        w.spectator_key(KeyEvent::new(KeyCode::Right, KeyModifiers::NONE));
        assert!(!w.following());
    }

    /// A key the spectator refuses outright (a designation tool, which would need to touch the
    /// session) does nothing, including not switching the view -- there is nothing new to see.
    #[test]
    fn a_refused_key_does_not_switch_to_free_look() {
        let mut w = Watcher::new(&header(Nation::Earth)).unwrap();
        w.spectator_key(KeyEvent::new(KeyCode::Char('h'), KeyModifiers::NONE));
        assert!(w.following(), "a refused action shouldn't switch the view");
    }

    /// In free look, the agent's own cursor still shows on the map -- the same outline an
    /// attention ghost wears -- even once the spectator's own camera has moved well away from
    /// it (#330's acceptance criteria).
    #[test]
    fn the_agents_cursor_still_shows_on_the_map_in_free_look() {
        let mut w = Watcher::new(&header(Nation::Earth)).unwrap();
        w.toggle_view();
        for _ in 0..5 {
            w.spectator_key(KeyEvent::new(KeyCode::Right, KeyModifiers::NONE));
        }
        let palette =
            Palette::new(&fn_sim::Defs::bundled(), Nation::Earth, ColorDepth::TrueColor, true);
        let buf = w.render(&palette, (100, 40));
        let agent = w.app().ui.cursor;
        let view = w.spectator.view;
        let regions = layout::regions(Rect::new(0, 0, 100, 40)).unwrap();
        let (sx, sy) = (agent.x - view.x, agent.y - view.y);
        assert!(
            (0..i32::from(regions.map.width)).contains(&sx),
            "the agent should still be on screen"
        );
        assert!((0..i32::from(regions.map.height)).contains(&sy));
        let (x, y) = (regions.map.x + sx as u16, regions.map.y + sy as u16);
        assert!(
            buf[(x, y)].style().add_modifier.contains(Modifier::REVERSED),
            "the agent's cursor should be outlined"
        );
    }

    /// The strip names whichever view is on.
    #[test]
    fn the_strip_says_which_view_is_on() {
        let mut w = Watcher::new(&header(Nation::Earth)).unwrap();
        assert!(w.strip_text().contains("FOLLOW"), "{}", w.strip_text());
        w.toggle_view();
        assert!(w.strip_text().contains("FREE"), "{}", w.strip_text());
    }

    /// The watcher's own controls -- Space, `+`/`-`, `.`, `]`, `q` -- are read directly by
    /// `watch::run`'s own loop before anything reaches `spectator_key` (the match in `run`
    /// above), so pressing `q` here (never routed through `spectator_key` in the real loop)
    /// doesn't quit the replay or touch the spectator; this just documents that `spectator_key`
    /// itself, if a game key ever did collide, treats an unmapped or refused key as a no-op
    /// rather than guessing.
    #[test]
    fn spectator_key_ignores_keys_with_no_allowed_action() {
        let mut w = Watcher::new(&header(Nation::Earth)).unwrap();
        w.spectator_key(KeyEvent::new(KeyCode::Char('q'), KeyModifiers::NONE));
        assert!(w.following(), "q maps to AskQuit, which is refused for the spectator");
    }

    /// The hard constraint (#330): free-look spectator input never changes what the replay
    /// does. Replaying the same log twice -- once untouched, once with a spectator moving the
    /// cursor, opening and closing every read-only screen and the Build modal, trying (and
    /// failing) to choose research and place a building, switching views and stepping one
    /// action at a time between every one of those -- lands on the exact same command log
    /// (`fn_sim::world::command_log`, the ledger `World::apply` keeps for replay): the proof
    /// that nothing here ever reaches a `&mut Session`.
    #[test]
    fn spectating_never_changes_the_replay() {
        let mut h =
            Host::new(7, Nation::Earth, None, None, None, false).unwrap().with_turn_seconds(5);
        h.press_keys(
            "h <right*3> <enter> <down*2> <enter>",
            Some("harvest".into()),
            Some("wood".into()),
        )
        .unwrap();
        h.end_turn(1);
        h.press_keys("p <right*2> <enter> <down*2> <enter>", None, None).unwrap();
        h.end_turn(1);
        h.look(10, 10, 5, 5);
        h.inspect(20, 20);
        h.press_keys("x", None, None).unwrap();
        h.end_turn(1);

        // Every key a spectator might try, mutating and refused ones included: free movement,
        // every read-only screen (opened, then closed by the same key), the Build modal
        // (opened, Enter tried inside it, then closed), a tool key, Tab/F, and the Research
        // screen's own `Enter` (which would set the agent's research target if it reached the
        // session).
        let spectator_keys: Vec<KeyEvent> = [
            (KeyCode::Right, KeyModifiers::NONE),
            (KeyCode::Down, KeyModifiers::SHIFT),
            (KeyCode::Char('n'), KeyModifiers::NONE),
            (KeyCode::Enter, KeyModifiers::NONE),
            (KeyCode::Char('n'), KeyModifiers::NONE),
            (KeyCode::Char('t'), KeyModifiers::NONE),
            (KeyCode::Right, KeyModifiers::NONE),
            (KeyCode::Enter, KeyModifiers::NONE),
            (KeyCode::Char('t'), KeyModifiers::NONE),
            (KeyCode::Char('u'), KeyModifiers::NONE),
            (KeyCode::Down, KeyModifiers::NONE),
            (KeyCode::Enter, KeyModifiers::NONE),
            (KeyCode::Esc, KeyModifiers::NONE),
            (KeyCode::Char('h'), KeyModifiers::NONE),
            (KeyCode::Char('g'), KeyModifiers::NONE),
            (KeyCode::Char('g'), KeyModifiers::NONE),
            (KeyCode::Char('l'), KeyModifiers::NONE),
            (KeyCode::Char('l'), KeyModifiers::NONE),
            (KeyCode::Char('?'), KeyModifiers::NONE),
            (KeyCode::Char('?'), KeyModifiers::NONE),
            (KeyCode::Tab, KeyModifiers::NONE),
            (KeyCode::Char('f'), KeyModifiers::NONE),
            (KeyCode::Char('1'), KeyModifiers::NONE),
            (KeyCode::F(5), KeyModifiers::NONE),
            (KeyCode::F(9), KeyModifiers::NONE),
        ]
        .into_iter()
        .map(|(code, mods)| KeyEvent::new(code, mods))
        .collect();

        let mut plain = Watcher::new(&h.header()).unwrap();
        plain.feed(h.log().lines());
        plain.fast_forward_to_turn(u32::MAX);
        assert!(plain.caught_up());

        let mut spied = Watcher::new(&h.header()).unwrap();
        spied.feed(h.log().lines());
        let mut guard = 0;
        while !spied.caught_up() {
            for &key in &spectator_keys {
                spied.spectator_key(key);
            }
            spied.toggle_view();
            spied.step_one();
            guard += 1;
            assert!(guard < 100_000, "should have caught up long before this");
        }

        assert_eq!(
            spied.app().session.world.command_log(),
            plain.app().session.world.command_log(),
            "spectator input must never change the replay's own commands"
        );
        assert_eq!(spied.app().session.world.tick, plain.app().session.world.tick);
        assert_eq!(spied.turn(), plain.turn());
    }
}
