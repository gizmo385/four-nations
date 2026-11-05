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
    current: Option<CurrentAction>,
    ghost: Option<Ghost>,
    /// Fractional keys owed at the current pace, carried between [`Self::advance`] calls
    /// (`fn_ui::Clock::advance`'s own pattern).
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
            watch: None,
            pace: None,
            at: None,
        };
        let shell = Shell::for_args(&args, cli::DEFAULT_SIZE, None, 0)?;
        let digest = shell.defs().id_hash();
        if digest != header.data_digest {
            return Err(format!(
                "this run log was made against different data (digest {:#x}, the data here is \
                 {digest:#x}); it can't be replayed",
                header.data_digest
            ));
        }
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
    }

    // --------------------------------------------------------------------------------------
    // Replay
    // --------------------------------------------------------------------------------------

    /// Spends `real_secs` of real time animating the log at the current pace: `Keys` entries
    /// press at about `pace * 20` keys a second (compressing a long walk into Shift-steps,
    /// [`COMPRESS_FROM`]); `Look`, `Inspect` and `Turn` entries (which took no game time, or ran
    /// their ticks "whatever the clock says", `end_turn`'s own doc comment) apply the moment
    /// they're reached, uncapped, since nothing paces them but the keys around them. Does
    /// nothing while [`Self::toggle_pause`]d, except keep the attention ghost ageing.
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
        let owed = self.owed + real_secs.max(0.0) * f64::from(self.pace()) * KEYS_PER_SECOND;
        let mut budget = owed.floor();
        self.owed = owed - budget;
        while budget > 0.0 {
            if self.current.is_none() && !self.start_next() {
                break; // caught up mid-budget
            }
            if self.current.is_some() {
                self.press_one();
                budget -= 1.0;
            }
            // A `Look`/`Inspect`/`Turn` `start_next` just applied costs no budget; loop again
            // either to start the next thing or to spend the budget on a `Keys` entry's keys.
        }
    }

    /// Completes exactly one more act at once, ignoring the pace: the watcher's own `.` key
    /// (docs/design/agent-play.md, "Watching": "`.` steps one action"). Finishes
    /// [`Self::current`] if one is already mid-press, rather than starting yet another.
    pub fn step_one(&mut self) {
        if self.current.is_some() {
            while self.current.is_some() {
                self.press_one();
            }
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
                self.press_one();
                continue;
            }
            if !self.start_next() {
                break;
            }
        }
    }

    /// Pops the next pending entry and starts it: `Keys` becomes [`Self::current`] (nothing
    /// pressed yet), `Look`/`Inspect` sets the attention ghost and logs an Agent line at once
    /// (they take no game time), and `Turn` runs its ticks at once (`end_turn`'s own ticks,
    /// uncapped) and logs nothing beyond what the world itself logged. `false` with nothing
    /// pending.
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
                self.current = Some(CurrentAction { label, note, keys, pressed: 0 });
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
                self.app_mut().run_ticks(ticks);
                self.turn += 1;
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
        }
        true
    }

    /// Presses [`Self::current`]'s next key (or, deep in a long plain-arrow run, a whole
    /// Shift-step's worth at once, [`COMPRESS_FROM`]), finishing and logging the act once every
    /// key is pressed.
    fn press_one(&mut self) {
        let Some(action) = &self.current else { return };
        let remaining = &action.keys[action.pressed..];
        let (key, advance_by) = next_press(remaining);
        self.shell.key(key);
        let action = self.current.as_mut().expect("just matched Some above");
        action.pressed += advance_by;
        if action.pressed >= action.keys.len() {
            let CurrentAction { label, note, keys, .. } = self.current.take().unwrap();
            let script = keys.iter().map(|&k| crate::surface::key_token(k)).collect::<Vec<_>>();
            let tick = self.app().session.world.tick;
            self.app_mut().ui.alerts.push_agent(tick, describe_act(&label, &note, &script));
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
    /// (docs/design/agent-play.md, "Watching").
    pub fn render(&mut self, palette: &Palette, size: (u16, u16)) -> Buffer {
        let mut buf = render_shell(&mut self.shell, palette, size);
        if let Some(regions) = layout::regions(Rect::new(0, 0, size.0, size.1)) {
            self.draw_strip(&mut buf, regions.status);
            if let Some(ghost) = self.ghost {
                draw_ghost(&self.app().ui.view, ghost, regions.map, &mut buf);
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

    /// " AGENT earth  turn 6  02:00  designate harvest 6x4 at (86,60) "wood for two beds"  x40",
    /// "...  waiting for agent (12 s)  x40" once the watcher has caught up with a live log, or
    /// "...  run finished  x40" once the log's own `End` line has been replayed (#327): a
    /// finished run never says "waiting" for a wait that will never end.
    fn strip_text(&self) -> String {
        let clock = fn_ui::view::format_clock(self.app().session.world.tick);
        let action = match &self.current {
            Some(a) => describe_action_line(a),
            None if self.finished() => "run finished".to_owned(),
            None if self.caught_up() => {
                format!("waiting for agent ({} s)", self.waiting_secs.round() as u64)
            }
            None => String::new(),
        };
        format!(
            " AGENT {}  turn {}  {clock}  {action}  x{}",
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
                        KeyCode::Char(' ') => watcher.toggle_pause(),
                        KeyCode::Char('+' | '=') => watcher.faster(),
                        KeyCode::Char('-' | '_') => watcher.slower(),
                        KeyCode::Char('.') => watcher.step_one(),
                        KeyCode::Char(']') => watcher.fast_forward_to_turn(watcher.turn() + 1),
                        KeyCode::Char('q' | 'Q') => quit = true,
                        _ => {} // game keys are ignored: a watcher's keys are watcher controls
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
        assert!(w.current.is_some(), "not finished yet");
        let pressed_so_far = w.current.as_ref().unwrap().pressed;
        assert!((1..=3).contains(&pressed_so_far), "{pressed_so_far}");
        w.advance(1.0); // plenty to finish the rest
        assert!(w.caught_up());
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
}
