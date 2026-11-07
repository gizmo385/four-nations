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
use crate::feed::Feed;
use crate::frame::render_shell;
use crate::glyphs::StudioDef;
use crate::keys;
use crate::layout;
use crate::shell::{Mode, Shell};
use crate::state::UiState;
use crate::studio::CodePane;
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

/// How long the tab strip's "studio needs WxH" notice (#501, docs/design/agent-studio.md, "The
/// tab fallback": "`0` does nothing [below the threshold], and the strip briefly says `studio
/// needs 160x45`") stays up after `0` is pressed there, in real seconds.
const SMALL_NOTICE_SECONDS: f32 = 2.0;

/// Which of the studio's three panes is shown full screen: as a tab below `[studio] min_width` x
/// `min_height`, or zoomed above it (#501, docs/design/agent-studio.md "The tab fallback": "there
/// is one model, not two"). `Game` is the default -- "the same as today's view" -- for both.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Pane {
    Code,
    Game,
    Feed,
}

impl Pane {
    /// The key that selects this pane (`1`/`2`/`3`), used both to read a key and to build the
    /// tab strip's own `1[2]3` indicator.
    fn digit(self) -> char {
        match self {
            Self::Code => '1',
            Self::Game => '2',
            Self::Feed => '3',
        }
    }
}

/// `1[2]3`: every pane's own digit, the shown one (`current`) bracketed (#501's acceptance
/// criteria: "a `1[2]3` indicator on the right").
fn tab_indicator(current: Pane) -> String {
    [Pane::Code, Pane::Game, Pane::Feed]
        .into_iter()
        .map(|p| if p == current { format!("[{}]", p.digit()) } else { p.digit().to_string() })
        .collect()
}

/// Which strip [`Watcher::draw_strip`] draws: the split's own game column
/// ([`Self::Minimal`], #499/#500 -- no `SCRIPT`/action text, a wider sparkline, no tab
/// indicator) or a full-screen pane's ([`Self::Tab`], #501 -- today's full text, the tab
/// sparkline and the `1[2]3` indicator), whether that pane is a tab below the studio's own
/// threshold or zoomed above it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum StripMode {
    Minimal,
    Tab(Pane),
}

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
            // #479/#497's own commentary (`LogLine`'s own doc comment): not a key either.
            LogLine::Script { .. }
            | LogLine::Wake { .. }
            | LogLine::Note { .. }
            | LogLine::Call { .. }
            | LogLine::Plan { .. } => {}
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
    /// What a script run (#482) shows on the strip: `Running` between its `Script` line and
    /// its `Wake` line, `Woke` from then until the next act (docs/design/agent-scripting.md,
    /// "Determinism, replay and watching": "the strip says why until the next act"). `None`
    /// outside any script run.
    script_strip: Option<ScriptStrip>,
    /// [`Self::turn`] when the current (or most recent) script run's `Script` line landed, so
    /// its `Wake` line can report how many turns it actually ran.
    script_turn_start: u32,
    /// The agent studio's code pane (#499, docs/design/agent-studio.md): which revision of which
    /// script is shown, fed and advanced alongside the replay. Drawn only at `[studio] min_width`
    /// x `min_height` or above (`Self::render`); kept, and fed, regardless of size, so resizing
    /// into the studio mid-replay shows the right revision right away.
    code_pane: CodePane,
    /// The agent studio's feed (#500, docs/design/agent-studio.md "The agent feed"): fed
    /// alongside `Self::code_pane`, drawn only in the studio for the same reason.
    feed: Feed,
    /// Acts (replayed `Keys` lines) per turn, indexed by turn number, for the studio strip's
    /// sparkline (#500, docs/design/agent-studio.md "The sparkline and the strip"). Grown lazily
    /// as turns pass; `Self::turn`'s own entry always exists once anything has been replayed
    /// this turn.
    acts: Vec<u16>,
    /// `data/tui.toml`'s `[studio]` thresholds and proportions, read once at construction
    /// (`crate::glyphs::Glyphs::bundled`'s own copy, the same one every `Palette` carries) so
    /// [`Self::key`] can tell the studio from the tab fallback without needing a `Palette` of
    /// its own (#501).
    studio: StudioDef,
    /// Which pane is shown full screen: as a tab below the studio's own threshold, or zoomed
    /// above it (#501, docs/design/agent-studio.md "The tab fallback"). Persists across resizes,
    /// so crossing the threshold either way keeps the chosen pane.
    tab: Pane,
    /// Above the threshold only: whether [`Self::tab`] is shown full screen (`true`) instead of
    /// the studio's own split (`false`, the default). Meaningless below the threshold, where
    /// there is no split to zoom away from.
    zoomed: bool,
    /// The terminal size `--frame`/`Self::render` will draw at, set by [`Self::set_frame_size`]
    /// ahead of [`Self::key`] so a `1`/`2`/`3`/`0` press can tell the studio from the tab
    /// fallback before the next [`Self::render`] call confirms it (`Self::resize`'s own size
    /// isn't it: `Self::render_studio` resizes the shell to the game column's own, smaller size).
    frame_size: (u16, u16),
    /// Real seconds left to show the tab strip's "studio needs WxH" notice
    /// ([`SMALL_NOTICE_SECONDS`]), aged down by [`Self::advance`] the same way
    /// [`Self::ghost`] is. Zero once it's gone.
    small_notice_left: f32,
}

/// What the agent strip shows for a script run in progress or just woken, between the clock and
/// the action (#482, docs/design/agent-scripting.md's own mockup: `SCRIPT keep-fed v3`).
#[derive(Clone, Debug, PartialEq)]
enum ScriptStrip {
    /// Between the run's `Script` line and its `Wake` line: reads made in here draw no
    /// attention ghosts (the acceptance criteria's own "MCP reads still do").
    Running { name: String, version: u32 },
    /// From the run's `Wake` line until the next act: the strip's action area shows `why`
    /// instead of the latest act, until a `Keys` line clears this back to `None`.
    Woke { name: String, version: u32, why: String },
}

impl ScriptStrip {
    /// `"keep-fed v3"` for a saved script's own run, or just `"keep-fed"` for an ad hoc one
    /// (`version` 0, never saved -- `run_script`'s own doc comment on `ScriptLimits`).
    fn label(&self) -> String {
        let (name, version) = match self {
            Self::Running { name, version } | Self::Woke { name, version, .. } => (name, *version),
        };
        if version > 0 { format!("{name} v{version}") } else { name.clone() }
    }
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
            script_strip: None,
            script_turn_start: 0,
            code_pane: CodePane::new(),
            feed: Feed::new(),
            acts: vec![0],
            studio: crate::glyphs::Glyphs::bundled().studio,
            tab: Pane::Game,
            zoomed: false,
            frame_size: cli::DEFAULT_SIZE,
            small_notice_left: 0.0,
        })
    }

    /// Queues more of the log to replay, oldest first. The code pane scans every `Script` line
    /// right away, whether or not it's been replayed yet (`CodePane::feed`'s own doc comment).
    pub fn feed(&mut self, events: impl IntoIterator<Item = LogLine>) {
        let events: Vec<LogLine> = events.into_iter().collect();
        self.code_pane.feed(&events);
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

    /// Logs one more Agent line (docs/design/agent-play.md, "The message log (`L`) gains an
    /// Agent entry for every act") to both [`Self::app`]'s own `Alerts` (what Follow draws, and
    /// what a script's own `keys("l")` would open into) and [`Self::spectator`]'s separate one
    /// (#330: its own `UiState`, so a human can only ever actually open the message log through
    /// free-look -- Follow never forwards a real key to the game). Pushing to both never
    /// touches `Session` or its command log, so it can't violate the hard constraint
    /// `tests::spectating_never_changes_the_replay` proves.
    fn push_agent(&mut self, tick: u64, text: String) {
        self.app_mut().ui.alerts.push_agent(tick, text.clone());
        self.spectator.alerts.push_agent(tick, text);
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

    /// Records the terminal size the next frame will draw at, ahead of time (#501): called by
    /// `--watch --frame` before it plays `--keys` (which reaches [`Self::key`] before
    /// [`Self::render`] ever runs) and by `run`'s own loop whenever the terminal resizes,
    /// alongside [`Self::resize`]. Deliberately not [`Self::resize`] itself: that one also
    /// resizes [`Self::shell`], which `Self::render_studio` calls at the game column's own,
    /// smaller size -- a value [`Self::studio_fits`] must never see.
    pub fn set_frame_size(&mut self, width: u16, height: u16) {
        self.frame_size = (width, height);
    }

    /// Whether [`Self::frame_size`] is at or above `[studio] min_width` x `min_height`: the
    /// studio's own split and zoom, rather than the tab fallback (#501, docs/design/
    /// agent-studio.md "The tab fallback").
    fn studio_fits(&self) -> bool {
        self.studio.fits(self.frame_size.0, self.frame_size.1)
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

    /// The agent studio's own keys (docs/design/agent-studio.md "Keys"): `1`/`2`/`3` zoom or tab
    /// to a pane (#501), `0` returns to the split (or, below the threshold, flashes the "studio
    /// needs WxH" notice), and `j`/`k`/`J`/`K` scroll the code pane (#499). `true` if `key` was
    /// one of these, so both `watch::run`'s own loop and the `--watch --frame --keys` path try
    /// it before the watcher's own controls and free look -- "watcher keys win over free look",
    /// and digits are already refused for a free-look spectator (`input::spectator_allowed`,
    /// #330), so claiming them here first changes nothing for one -- and every one of them can
    /// be golden-tested without a terminal either way.
    pub fn key(&mut self, key: KeyEvent) -> bool {
        match key.code {
            KeyCode::Char('1') => return self.press_pane(Pane::Code),
            KeyCode::Char('2') => return self.press_pane(Pane::Game),
            KeyCode::Char('3') => return self.press_pane(Pane::Feed),
            KeyCode::Char('0') => return self.press_zero(),
            _ => {}
        }
        self.code_pane.key(key)
    }

    /// `1`/`2`/`3` (#501, docs/design/agent-studio.md "The tab fallback"): at or above the
    /// studio's own threshold, zooms `pane` full screen, or -- pressed again on the pane already
    /// zoomed -- returns to the split; below it, just switches which tab is shown (there is no
    /// split to zoom away from there).
    fn press_pane(&mut self, pane: Pane) -> bool {
        if self.studio_fits() {
            if self.zoomed && self.tab == pane {
                self.zoomed = false;
            } else {
                self.tab = pane;
                self.zoomed = true;
            }
        } else {
            self.tab = pane;
        }
        true
    }

    /// `0`: above the threshold, returns to the split, the same as pressing the zoomed pane's
    /// own digit again; below it, there is no split to return to, so it does nothing but flash
    /// the tab strip's "studio needs WxH" notice ([`SMALL_NOTICE_SECONDS`]).
    fn press_zero(&mut self) -> bool {
        if self.studio_fits() {
            self.zoomed = false;
        } else {
            self.small_notice_left = SMALL_NOTICE_SECONDS;
        }
        true
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
    /// [`Self::toggle_pause`]d, except keep the attention ghost (and the tab strip's own
    /// "studio needs WxH" notice, #501) ageing.
    pub fn advance(&mut self, real_secs: f64) {
        self.age_ghost(real_secs);
        self.small_notice_left = (self.small_notice_left - real_secs as f32).max(0.0);
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
            LogLine::Keys { keys: raw_keys, label, note } => {
                let Ok(steps) = keys::parse(&raw_keys) else { return self.start_next() };
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
                // #500: one `keys` line counts as one act, toward both the studio feed's own
                // running progress (`k acts`) and the strip's own sparkline -- once it's
                // replayed, not once every one of its keys has actually been pressed.
                self.feed.on_keys(label.as_deref(), note.as_deref(), &raw_keys, self.turn);
                self.acts[self.turn as usize] += 1;
                // The `Woke` text stays on the strip only until the next act (#482,
                // docs/design/agent-scripting.md's own "the strip says why until the next
                // act"); a script run still mid-flight (`Running`) isn't touched, since its
                // own acts don't end it.
                if matches!(self.script_strip, Some(ScriptStrip::Woke { .. })) {
                    self.script_strip = None;
                }
                let action = CurrentAction { label, note, keys, pressed: 0 };
                self.last_action = Some(describe_action_line(&action));
                self.current = Some(Current::Keys(action));
            }
            LogLine::Look { x, y, w, h } => {
                // A read a script made itself draws no attention ghost -- the strip already
                // says `SCRIPT name vN` is why things are happening, so a ghost on every one of
                // a script's own reads would just be noise (#482's acceptance criteria: "Reads
                // made by a script draw no attention ghosts, while MCP reads still do").
                if !matches!(self.script_strip, Some(ScriptStrip::Running { .. })) {
                    self.ghost = Some(Ghost { x, y, w, h, left: GHOST_SECONDS });
                }
                self.feed.on_look(x, y, w, h, self.turn); // #500
                let tick = self.app().session.world.tick;
                self.push_agent(tick, format!("Agent looked at ({x}, {y}) {w}x{h}"));
            }
            LogLine::Inspect { x, y } => {
                if !matches!(self.script_strip, Some(ScriptStrip::Running { .. })) {
                    self.ghost = Some(Ghost { x, y, w: 1, h: 1, left: GHOST_SECONDS });
                }
                self.feed.on_inspect(x, y, self.turn); // #500
                let tick = self.app().session.world.tick;
                self.push_agent(tick, format!("Agent inspected ({x}, {y})"));
            }
            LogLine::Turn { .. } => {
                self.feed.on_turn(self.turn); // #500
                let ticks = self.turn_seconds * TICKS_PER_SECOND;
                self.current = Some(Current::Ticks { remaining: ticks });
            }
            // The run is over (#327): nothing left to replay, ever, however much more of the
            // log there might be. Logged once, like `Look`/`Inspect`, since it takes no game
            // time either.
            LogLine::End { reason, .. } => {
                let tick = self.app().session.world.tick;
                self.push_agent(tick, format!("Run finished: {reason}"));
                self.finished = Some(reason);
            }
            LogLine::Header(_) => return self.start_next(), // shouldn't recur; harmless if it did
            // #482: a run's own start -- `turns == 0` is `save_script`'s own "never run" marker
            // (`fn_agent::tools::Tools`'s own doc comment on its `"save_script"` dispatch arm),
            // which no `Wake` line will ever follow, so it leaves no strip or message either.
            LogLine::Script { name, version, code, turns, .. } => {
                // #499: the code pane follows every `Script` line, saved-only ones (`turns: 0`)
                // included, whether or not it ends up shown (`CodePane::on_script`'s own rule).
                let number = self.code_pane.on_script(&name, version, &code, turns);
                // #500: the feed's own entry is labelled with the `Script` line's own `version`
                // (0 for an ad hoc run -- `crate::feed::Run`'s own doc comment), not `number`
                // (`CodePane`'s revision ordinal), which `diff` alone needs to look up.
                let diff = self.code_pane.feed_diff(&name, number);
                self.feed.on_script(&name, version, turns, diff, self.turn);
                if turns > 0 {
                    self.script_turn_start = self.turn;
                    let strip = ScriptStrip::Running { name, version };
                    let tick = self.app().session.world.tick;
                    self.push_agent(tick, format!("Agent: ran {}, {turns} turns", strip.label()));
                    self.script_strip = Some(strip);
                }
            }
            // #482: that run's own end -- `why` (an event's own kind and text, or a script's
            // own `wake(msg)`) when there is one, `reason` ("budget", "done", ...) otherwise.
            LogLine::Wake { reason, why, .. } => {
                let turns_run = self.turn.saturating_sub(self.script_turn_start);
                self.code_pane.on_wake(turns_run); // #499
                self.feed.on_wake(&reason, why.as_deref()); // #500
                let detail = why.unwrap_or(reason);
                let tick = self.app().session.world.tick;
                self.push_agent(
                    tick,
                    format!(
                        "Agent: woke after {turns_run} turn{}: {detail}",
                        if turns_run == 1 { "" } else { "s" }
                    ),
                );
                if let Some(ScriptStrip::Running { name, version }) = self.script_strip.take() {
                    self.script_strip = Some(ScriptStrip::Woke { name, version, why: detail });
                }
            }
            // `notes {append}`'s own commentary: no display of its own in the message log (out
            // of this issue's scope -- "Showing script source in the watcher" is explicitly
            // out, and a note is read the same way), but the feed gets its own entry (#500).
            LogLine::Note { text } => self.feed.on_note(&text, self.turn),
            // #497: logged ahead of the call it announces, so the feed (#500,
            // docs/design/agent-studio.md's own "The agent feed") sees it before that same
            // call's own structural lines.
            LogLine::Call { .. } => self.feed.on_call(),
            // #524: the agent's plan is `fn_agent --resume`'s to restore; nothing to show or
            // replay here.
            LogLine::Plan { .. } => {}
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
            self.push_agent(tick, describe_act(&label, &note, &script));
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
            self.acts.push(0); // #500: `Self::acts`' own invariant, `acts.len() == turn + 1`.
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

    /// The frame for this watcher's session (#501, docs/design/agent-studio.md "The tab
    /// fallback": "there is one model, not two"): at or above `[studio] min_width` x
    /// `min_height`, the split (#499, "Layout" -- the code pane, the game column and the feed
    /// under it) unless [`Self::zoomed`] has zoomed [`Self::tab`] full screen; below it, always
    /// [`Self::tab`] full screen, as a tab. [`Self::set_frame_size`] records `size` first, so a
    /// `1`/`2`/`3`/`0` played by `--keys` right before this call (which never resizes
    /// [`Self::shell`] itself ahead of time) already knows which of the two it's choosing
    /// between.
    pub fn render(&mut self, palette: &Palette, size: (u16, u16)) -> Buffer {
        self.set_frame_size(size.0, size.1);
        if self.studio_fits() {
            if self.zoomed {
                self.render_pane(self.tab, palette, size)
            } else {
                self.render_studio(palette, size)
            }
        } else {
            self.render_pane(self.tab, palette, size)
        }
    }

    /// The game column's own bottom rows the feed takes (#500, docs/design/agent-studio.md
    /// "Layout": "the bottom `max(feed_min_rows, H / 4)` rows"), out of a studio `height` rows
    /// tall; the game pane (its own strip included) gets the rest.
    fn feed_height(palette: &Palette, height: u16) -> u16 {
        palette.glyphs.studio.feed_min_rows.max(height / 4).min(height)
    }

    fn render_studio(&mut self, palette: &Palette, size: (u16, u16)) -> Buffer {
        let game_width = palette.glyphs.studio.game_width(size.0);
        let code_width = size.0 - game_width;
        let feed_height = Self::feed_height(palette, size.1);
        let game_height = size.1 - feed_height;
        let game = self.render_game(palette, (game_width, game_height), StripMode::Minimal);
        let mut buf = Buffer::empty(Rect::new(0, 0, size.0, size.1));
        for y in 0..game_height {
            for x in 0..game_width {
                buf[(code_width + x, y)] = game[(x, y)].clone();
            }
        }
        if feed_height > 0 {
            let feed_area = Rect::new(code_width, game_height, game_width, feed_height);
            self.feed.render(&mut buf, feed_area, palette.ascii, self.turn);
        }
        self.code_pane.render(&mut buf, Rect::new(0, 0, code_width, size.1), palette);
        buf
    }

    /// `pane` full screen over `size`, above the agent strip (#501, docs/design/agent-studio.md
    /// "The tab fallback"): the game pane is [`Self::render_game`] at the whole frame's own
    /// size, the way it always drew below the studio; the code and feed panes get the rest of
    /// the frame above a strip row of their own, drawn the same shape
    /// (`crate::studio::CodePane::render`/`crate::feed::Feed::render` already draw their own
    /// border and header, the same ones the split shows).
    fn render_pane(&mut self, pane: Pane, palette: &Palette, size: (u16, u16)) -> Buffer {
        if pane == Pane::Game {
            return self.render_game(palette, size, StripMode::Tab(pane));
        }
        self.resize(size.0, size.1);
        let mut buf = Buffer::empty(Rect::new(0, 0, size.0, size.1));
        let pane_height = size.1.saturating_sub(1);
        let pane_area = Rect::new(0, 0, size.0, pane_height);
        match pane {
            Pane::Code => self.code_pane.render(&mut buf, pane_area, palette),
            Pane::Feed => self.feed.render(&mut buf, pane_area, palette.ascii, self.turn),
            Pane::Game => unreachable!("handled above"),
        }
        if size.1 > 0 {
            let strip_area = Rect::new(0, pane_height, size.0, 1);
            self.draw_strip(&mut buf, strip_area, StripMode::Tab(pane), palette);
        }
        buf
    }

    /// The watch view at `size`: the agent strip over the bottom status row instead of the key
    /// hints, and an attention ghost outlined over the map, if one is live
    /// (docs/design/agent-play.md, "Watching"). In Follow this is the agent's own view, as
    /// always; in Free look (#330) it's the spectator's own `UiState` instead, with the agent's
    /// own cursor outlined on the map too, the same look an attention ghost wears, so a
    /// spectator looking elsewhere can still see where the agent is pointing. `mode` picks the
    /// strip [`Self::draw_strip`] draws: [`StripMode::Minimal`] drops `SCRIPT name vN` and the
    /// action text (#499's acceptance criteria: the studio's code header and feed carry them
    /// instead), [`StripMode::Tab`] keeps them and adds the tab indicator (#501).
    fn render_game(&mut self, palette: &Palette, size: (u16, u16), mode: StripMode) -> Buffer {
        self.resize(size.0, size.1);
        let mut buf = if self.follow {
            render_shell(&mut self.shell, palette, size)
        } else {
            crate::frame::render(&self.app().session, &self.spectator, palette, size)
        };
        if let Some(regions) = layout::regions(Rect::new(0, 0, size.0, size.1)) {
            self.draw_strip(&mut buf, regions.status, mode, palette);
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

    fn draw_strip(&self, buf: &mut Buffer, area: Rect, mode: StripMode, palette: &Palette) {
        let style = Style::new().add_modifier(Modifier::REVERSED);
        for x in area.left()..area.right() {
            buf[(x, area.y)].set_char(' ').set_style(style);
        }
        match mode {
            StripMode::Minimal => {
                let text = self.strip_text(true);
                buf.set_stringn(area.x, area.y, &text, usize::from(area.width), style);
                self.draw_sparkline(buf, area, palette, style); // #500
            }
            StripMode::Tab(pane) => self.draw_tab_strip(buf, area, pane, palette, style),
        }
    }

    /// The tab strip (#501, docs/design/agent-studio.md "The tab fallback"): today's agent strip
    /// text, then `acts <bars> <peak>` over `[studio] spark_turns_tabs` turns (or, briefly after
    /// `0` is pressed below the threshold, "studio needs WxH" in its place), then the `1[2]3`
    /// tab indicator. The action text (`Self::strip_text`'s own, between `SCRIPT name vN` and
    /// the pace) is cut first when the row is too narrow for all of it, since it's the part a
    /// zoomed code or feed pane's own box already repeats nothing of; if even that doesn't fit,
    /// the sparkline and indicator are dropped too rather than drawing something unreadable.
    fn draw_tab_strip(
        &self,
        buf: &mut Buffer,
        area: Rect,
        pane: Pane,
        palette: &Palette,
        base: Style,
    ) {
        let width = usize::from(area.width);
        let indicator = tab_indicator(pane);
        let body = if self.small_notice_left > 0.0 {
            format!("studio needs {}x{}", self.studio.min_width, self.studio.min_height)
        } else {
            self.sparkline_text(
                palette.ascii,
                self.studio.spark_turns_tabs,
                self.studio.spark_floor,
            )
        };
        let suffix = format!(" {body} {indicator}");

        let full = self.strip_text(false);
        let text = if (full.chars().count() + suffix.chars().count()) <= width {
            format!("{full}{suffix}")
        } else {
            let bare = self.strip_text_no_action();
            let with_suffix = format!("{bare}{suffix}");
            if with_suffix.chars().count() <= width { with_suffix } else { bare }
        };
        buf.set_stringn(area.x, area.y, &text, width, base);
    }

    /// `acts <bars> <peak>` over the last `window_turns` turns (docs/design/agent-studio.md,
    /// "The sparkline and the strip"), as plain text -- the tab strip's own
    /// [`Self::draw_tab_strip`] draws it with no per-character styling, unlike
    /// [`Self::draw_sparkline`]'s dimmed zero bars, since it's one more piece of a row that's
    /// already being trimmed to fit rather than the split's own, roomier strip.
    fn sparkline_text(&self, ascii: bool, window_turns: u16, floor: u16) -> String {
        let window_len = usize::from(window_turns).min(self.acts.len());
        let window = &self.acts[self.acts.len() - window_len..];
        let levels = fn_ui::sparkline::levels(window, floor);
        let ramp: [char; 8] = if ascii {
            ['_', '.', ':', '-', '=', '+', '*', '#']
        } else {
            ['▁', '▂', '▃', '▄', '▅', '▆', '▇', '█']
        };
        let peak = fn_ui::sparkline::peak(window);
        let bars: String = levels.iter().map(|&l| ramp[usize::from(l)]).collect();
        format!("acts {bars} {peak}")
    }

    /// `acts <bars> <peak>` over the last `[studio] spark_turns` turns, the rightmost bar the
    /// turn in progress, right-aligned onto the strip's own row (docs/design/agent-studio.md,
    /// "The sparkline and the strip"). A no-op if the strip isn't wide enough to fit it rather
    /// than drawing something truncated and unreadable.
    fn draw_sparkline(&self, buf: &mut Buffer, area: Rect, palette: &Palette, base: Style) {
        let studio = &palette.glyphs.studio;
        let window_len = usize::from(studio.spark_turns).min(self.acts.len());
        let window = &self.acts[self.acts.len() - window_len..];
        let levels = fn_ui::sparkline::levels(window, studio.spark_floor);
        let ramp: [char; 8] = if palette.ascii {
            ['_', '.', ':', '-', '=', '+', '*', '#']
        } else {
            ['▁', '▂', '▃', '▄', '▅', '▆', '▇', '█']
        };
        let peak = fn_ui::sparkline::peak(window);
        let prefix = "acts ";
        let suffix = format!(" {peak}");
        let total = 1 + prefix.chars().count() + levels.len() + suffix.chars().count();
        if usize::from(area.width) < total {
            return;
        }
        let mut x = area.right() - total as u16;
        buf[(x, area.y)].set_char(' ').set_style(base);
        x += 1;
        for c in prefix.chars() {
            buf[(x, area.y)].set_char(c).set_style(base);
            x += 1;
        }
        for &level in &levels {
            let style = if level == 0 { base.add_modifier(Modifier::DIM) } else { base };
            buf[(x, area.y)].set_char(ramp[usize::from(level)]).set_style(style);
            x += 1;
        }
        for c in suffix.chars() {
            buf[(x, area.y)].set_char(c).set_style(base);
            x += 1;
        }
    }

    /// " AGENT earth  FOLLOW  turn 6  02:00  designate harvest 6x4 at (86,60) "wood for two
    /// beds"  x40": the latest act stays on the strip while its turn plays out. A live log's
    /// agent that has written turns faster than the chosen pace plays them back adds "  3 turns
    /// behind" ([`Self::turns_behind`], #328), "waiting for agent (12 s)" shows once the watcher
    /// has caught up, and "run finished" once the log's own `End` line has been replayed (#327):
    /// a finished run never says "waiting" for a wait that will never end. `FOLLOW`/`FREE`
    /// (#330) says which view is on, right after the nation. While a script is running, or has
    /// just woken, "SCRIPT keep-fed v3" lands right after the clock, and the action area reads
    /// "woke: death: Ping died of thirst" instead of the latest act until the next one (#482,
    /// docs/design/agent-scripting.md's own mockup). In the studio (#499), `SCRIPT name vN` and
    /// the action text are dropped: the code pane's own header and the feed (a later issue)
    /// carry them instead.
    fn strip_text(&self, studio: bool) -> String {
        self.strip_text_impl(studio, studio)
    }

    /// `Self::strip_text(false)` with the action text dropped (#501, docs/design/agent-studio.md
    /// "The tab fallback": "the action text is cut first when space runs short"), but
    /// `SCRIPT name vN` kept -- unlike [`Self::strip_text`]'s own `studio` case, which drops
    /// both.
    fn strip_text_no_action(&self) -> String {
        self.strip_text_impl(false, true)
    }

    fn strip_text_impl(&self, drop_script: bool, drop_action: bool) -> String {
        let clock = fn_ui::view::format_clock(self.app().session.world.tick);
        let behind = self.turns_behind();
        let script = match &self.script_strip {
            Some(s) if !drop_script => format!("SCRIPT {}  ", s.label()),
            _ => String::new(),
        };
        let latest = match &self.script_strip {
            Some(ScriptStrip::Woke { why, .. }) => format!("woke: {why}"),
            _ => self.last_action.clone().unwrap_or_default(),
        };
        let woke = matches!(self.script_strip, Some(ScriptStrip::Woke { .. }));
        let action = if self.finished() {
            "run finished".to_owned()
        } else if behind > 0 {
            format!("{latest}  {behind} turn{} behind", if behind == 1 { "" } else { "s" })
        } else if !woke && self.current.is_none() && self.caught_up() {
            // A freshly woken script still has something to say (`latest`'s own "woke: ...")
            // even once the watcher has caught up with the log: it never says "waiting for
            // agent" over that, since the agent just finished talking (#482).
            format!("waiting for agent ({} s)", self.waiting_secs.round() as u64)
        } else {
            latest
        };
        let action = if drop_action { String::new() } else { action };
        let view = if self.follow { "FOLLOW" } else { "FREE" };
        format!(
            " AGENT {}  {view}  turn {}  {clock}  {script}{action}  x{}",
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
                        // Studio keys (#499, `j`/`k`/`J`/`K` scrolling the code pane) win over
                        // free look too; everything else is free look's: `V` (not an F-key,
                        // since the owner's WSL terminal never sees those) toggles the view, and
                        // everything else is the spectator's own, read-only
                        // (`Watcher::free_look_key`).
                        _ if watcher.key(key) => {}
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
        assert!(w.strip_text(false).contains("3 turns behind"), "{}", w.strip_text(false));

        w.fast_forward_to_turn(2);
        assert_eq!(w.turns_behind(), 1);
        assert!(w.strip_text(false).contains("1 turn behind"), "{}", w.strip_text(false));

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
        assert!(w.strip_text(false).contains("\"scout\""), "{}", w.strip_text(false));

        w.feed([LogLine::Turn { index: 2, tick: 400 }]);
        let strip = w.strip_text(false);
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
        assert!(w.strip_text(false).contains("waiting for agent (5 s)"), "{}", w.strip_text(false));
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
    // Script and wake lines (#482)
    // ----------------------------------------------------------------------------------------

    fn wake(reason: &str, why: Option<&str>) -> LogLine {
        LogLine::Wake {
            reason: reason.to_owned(),
            why: why.map(str::to_owned),
            turn: 0,
            memory: serde_json::json!({}),
        }
    }

    fn script(name: &str, version: u32, turns: u32) -> LogLine {
        LogLine::Script {
            name: name.to_owned(),
            version,
            code: String::new(),
            turns,
            wake_on: Vec::new(),
        }
    }

    /// A running script's `Script` line puts `SCRIPT name vN` on the strip (between the clock
    /// and the action, docs/design/agent-scripting.md's own mockup) and logs an `Agent: ran
    /// ..., N turns` line right away.
    #[test]
    fn a_script_line_shows_on_the_strip_and_logs_its_own_start() {
        let mut w = Watcher::new(&header(Nation::Earth)).unwrap();
        w.feed([script("keep-fed", 3, 45)]);
        w.advance(1.0);
        assert!(w.strip_text(false).contains("SCRIPT keep-fed v3"), "{}", w.strip_text(false));
        let lines: Vec<&str> =
            w.app().ui.alerts.history().iter().map(|e| e.text.as_str()).collect();
        assert!(lines.iter().any(|l| l == &"Agent: ran keep-fed v3, 45 turns"), "{lines:?}");
    }

    /// An ad hoc `run_script {code}` call (`version` 0, never saved) shows its own tag with no
    /// `vN` suffix, since there is no saved version to name.
    #[test]
    fn an_ad_hoc_script_has_no_version_suffix() {
        let mut w = Watcher::new(&header(Nation::Earth)).unwrap();
        w.feed([script("scout", 0, 10)]);
        w.advance(1.0);
        assert!(w.strip_text(false).contains("SCRIPT scout"), "{}", w.strip_text(false));
        assert!(!w.strip_text(false).contains(" v0"), "{}", w.strip_text(false));
    }

    /// `save_script`'s own `Script` line (#480) carries `turns: 0`, since it was never run: no
    /// strip and no "ran" message, as nothing here is actually running.
    #[test]
    fn a_save_only_script_line_shows_nothing() {
        let mut w = Watcher::new(&header(Nation::Earth)).unwrap();
        w.feed([script("keep-fed", 1, 0)]);
        w.advance(1.0);
        assert!(!w.strip_text(false).contains("SCRIPT"), "{}", w.strip_text(false));
        let lines: Vec<&str> =
            w.app().ui.alerts.history().iter().map(|e| e.text.as_str()).collect();
        assert!(lines.is_empty(), "{lines:?}");
    }

    /// A `Wake` line counts the turns the script actually ran (not its requested budget), logs
    /// `Agent: woke after N turns: <why>`, and keeps the script's own label on the strip while
    /// swapping the action area for "woke: <why>" (docs/design/agent-scripting.md's own
    /// mockup), until the next act clears both.
    #[test]
    fn a_wake_line_reports_turns_run_and_holds_the_strip_until_the_next_act() {
        let mut w = Watcher::new(&header(Nation::Earth)).unwrap();
        w.feed([
            script("keep-fed", 3, 45),
            LogLine::Turn { index: 0, tick: 100 },
            LogLine::Turn { index: 1, tick: 200 },
            wake("event", Some("death: Ping died of thirst")),
        ]);
        w.fast_forward_to_turn(u32::MAX);
        assert!(w.caught_up());

        let text = w.strip_text(false);
        assert!(text.contains("SCRIPT keep-fed v3"), "{text}");
        assert!(text.contains("woke: death: Ping died of thirst"), "{text}");
        let lines: Vec<&str> =
            w.app().ui.alerts.history().iter().map(|e| e.text.as_str()).collect();
        assert!(
            lines.iter().any(|l| l == &"Agent: woke after 2 turns: death: Ping died of thirst"),
            "{lines:?}"
        );

        // The next act clears the woke text (and the script label) back off the strip -- not
        // `strip_text` itself, since this act is alone in the log and so is immediately caught
        // up again once it's pressed ("waiting for agent", the same as any other finished act
        // with nothing queued after it).
        w.feed([LogLine::Keys { keys: "h".to_owned(), label: Some("harvest".into()), note: None }]);
        w.advance(1.0);
        assert_eq!(w.script_strip, None, "the next act should have cleared the woke state");
        assert!(w.last_action.as_deref().unwrap_or("").contains("harvest"));
    }

    /// With no `why` (a plain `"budget"`/`"done"` wake), the strip and the message fall back to
    /// `reason` itself.
    #[test]
    fn a_wake_with_no_why_falls_back_to_the_reason() {
        let mut w = Watcher::new(&header(Nation::Earth)).unwrap();
        w.feed([script("keep-fed", 1, 5), wake("budget", None)]);
        w.fast_forward_to_turn(u32::MAX);
        assert!(w.strip_text(false).contains("woke: budget"), "{}", w.strip_text(false));
    }

    /// Reads a script makes itself (between its own `Script` and `Wake` lines) draw no
    /// attention ghost, while an ordinary `Look`/`Inspect` (an MCP read, outside any script)
    /// still does (#482's acceptance criteria).
    #[test]
    fn reads_inside_a_running_script_draw_no_ghost_but_mcp_reads_still_do() {
        let mut w = Watcher::new(&header(Nation::Earth)).unwrap();
        w.feed([script("keep-fed", 1, 5), LogLine::Look { x: 5, y: 5, w: 3, h: 3 }]);
        w.advance(1.0);
        assert_eq!(w.ghost, None, "a script's own read shouldn't draw a ghost");

        let mut w = Watcher::new(&header(Nation::Earth)).unwrap();
        w.feed([LogLine::Inspect { x: 9, y: 9 }]);
        w.advance(1.0);
        assert_eq!(
            w.ghost,
            Some(Ghost { x: 9, y: 9, w: 1, h: 1, left: GHOST_SECONDS }),
            "an MCP read still draws one"
        );
    }

    // ----------------------------------------------------------------------------------------
    // The agent feed and the acts sparkline (#500)
    // ----------------------------------------------------------------------------------------

    /// Each replayed `Keys` line counts one act toward its own turn, and a `Turn` line starts a
    /// fresh entry at zero -- `Self::acts`' own invariant (docs/design/agent-studio.md, "The
    /// sparkline and the strip": "Each replayed `keys` line counts one toward the current
    /// turn").
    #[test]
    fn acts_count_once_per_replayed_keys_line_and_a_new_turn_starts_at_zero() {
        let mut w = Watcher::new(&header(Nation::Earth)).unwrap();
        w.feed([
            LogLine::Keys { keys: "h".to_owned(), label: None, note: None },
            LogLine::Keys { keys: "<right*3>".to_owned(), label: None, note: None },
            LogLine::Turn { index: 0, tick: u64::from(10 * TICKS_PER_SECOND) },
            LogLine::Keys { keys: "x".to_owned(), label: None, note: None },
        ]);
        w.fast_forward_to_turn(1);
        w.step_one(); // starts the final `Keys` line, queued but not reached by the turn alone
        assert_eq!(w.acts, vec![2, 1], "two acts in turn 0, one so far in turn 1");
    }

    /// `Look`/`Inspect` don't count as acts: only `Keys` lines do.
    #[test]
    fn reads_do_not_count_as_acts() {
        let mut w = Watcher::new(&header(Nation::Earth)).unwrap();
        w.feed([LogLine::Look { x: 1, y: 1, w: 1, h: 1 }, LogLine::Inspect { x: 2, y: 2 }]);
        w.advance(1.0);
        assert_eq!(w.acts, vec![0]);
    }

    /// The studio split's own strip ends with `acts <bars> <peak>` (#500) and nothing else; the
    /// tab fallback's strip (#501, below the threshold here) carries the same sparkline plus a
    /// `1[2]3` tab indicator the split never shows (there's nothing to tab between once every
    /// pane is already on screen at once).
    #[test]
    fn the_split_and_tab_strips_both_show_a_sparkline_but_only_tabs_get_an_indicator() {
        let mut w = Watcher::new(&header(Nation::Earth)).unwrap();
        w.feed([LogLine::Keys { keys: "h".to_owned(), label: None, note: None }]);
        w.advance(1.0);
        let palette =
            Palette::new(&fn_sim::Defs::bundled(), Nation::Earth, ColorDepth::TrueColor, false);
        let studio_text = crate::frame::to_text(&w.render(&palette, (160, 45)));
        assert!(studio_text.contains("acts "), "{studio_text}");
        assert!(!studio_text.contains("1[2]3"), "{studio_text}: the split has no tab indicator");

        let tab_text = crate::frame::to_text(&w.render(&palette, (120, 40)));
        assert!(tab_text.contains("acts "), "{tab_text}");
        assert!(tab_text.contains("1[2]3"), "{tab_text}: the game tab is the default");
    }

    /// The studio's feed box shows under the game pane, which gets what's left of the column
    /// (docs/design/agent-studio.md, "Layout": "the bottom `max(feed_min_rows, H / 4)` rows").
    #[test]
    fn the_studio_splits_the_game_column_with_the_feed_below_it() {
        let mut w = Watcher::new(&header(Nation::Earth)).unwrap();
        w.feed([script("keep-fed", 1, 5)]);
        w.advance(1.0);
        let palette =
            Palette::new(&fn_sim::Defs::bundled(), Nation::Earth, ColorDepth::TrueColor, false);
        let text = crate::frame::to_text(&w.render(&palette, (160, 45)));
        assert!(text.contains("AGENT ── "), "{text}: the feed's own box title should show");
        assert!(text.contains("run_script keep-fed v1"), "{text}");
    }

    // ----------------------------------------------------------------------------------------
    // The tab fallback and zoom (#501, docs/design/agent-studio.md "The tab fallback")
    // ----------------------------------------------------------------------------------------

    fn digit(c: char) -> KeyEvent {
        KeyEvent::new(KeyCode::Char(c), KeyModifiers::NONE)
    }

    /// Every studio digit (`0`/`1`/`2`/`3`) is claimed by `Watcher::key`, so `watch::run`'s own
    /// loop and `--watch --frame --keys` never offer them to free look (which already refuses
    /// them anyway, `input::spectator_allowed` -- this is the "win over free look" contract
    /// itself, not just its effect).
    #[test]
    fn every_studio_digit_is_claimed_by_key() {
        let mut w = Watcher::new(&header(Nation::Earth)).unwrap();
        for c in ['0', '1', '2', '3'] {
            assert!(w.key(digit(c)), "{c}");
        }
    }

    /// Below the studio's own threshold, a digit just switches which tab is shown -- there's no
    /// split to zoom away from, so nothing toggles even if the same digit is pressed twice.
    #[test]
    fn digits_switch_tabs_below_the_threshold_without_zooming() {
        let mut w = Watcher::new(&header(Nation::Earth)).unwrap();
        w.set_frame_size(110, 34);
        assert!(w.key(digit('1')));
        assert_eq!(w.tab, Pane::Code);
        assert!(!w.zoomed, "there is no split to zoom away from below the threshold");

        assert!(w.key(digit('1')));
        assert_eq!(w.tab, Pane::Code, "the same digit again just stays on the same tab");
    }

    /// At or above the threshold, a digit zooms its pane full screen, and the same digit again
    /// returns to the split (#501's acceptance criteria).
    #[test]
    fn digits_zoom_a_pane_above_the_threshold_and_the_same_digit_returns_to_the_split() {
        let mut w = Watcher::new(&header(Nation::Earth)).unwrap();
        w.set_frame_size(160, 45);
        assert!(w.key(digit('1')));
        assert_eq!(w.tab, Pane::Code);
        assert!(w.zoomed);

        assert!(w.key(digit('1')));
        assert!(!w.zoomed, "the same digit again returns to the split");
    }

    /// `0` returns to the split above the threshold, the same as the zoomed pane's own digit;
    /// below it, there is nothing for `0` to do but flash the "studio needs WxH" notice.
    #[test]
    fn zero_returns_to_the_split_above_the_threshold_and_flashes_a_notice_below_it() {
        let mut w = Watcher::new(&header(Nation::Earth)).unwrap();
        w.set_frame_size(160, 45);
        w.key(digit('2'));
        assert!(w.zoomed);
        assert!(w.key(digit('0')));
        assert!(!w.zoomed);
        assert_eq!(w.small_notice_left, 0.0, "the notice is only for below the threshold");

        w.set_frame_size(110, 34);
        assert!(w.key(digit('0')));
        assert!(w.small_notice_left > 0.0);
    }

    /// The tab strip shows "studio needs 160x45" while the notice is live, and it ages back out
    /// (`Self::advance` ages it the same way an attention ghost ages).
    #[test]
    fn the_tab_strip_shows_the_notice_text_while_it_is_live() {
        let mut w = Watcher::new(&header(Nation::Earth)).unwrap();
        w.set_frame_size(110, 34);
        w.key(digit('0'));
        let palette =
            Palette::new(&fn_sim::Defs::bundled(), Nation::Earth, ColorDepth::TrueColor, false);
        let text = crate::frame::to_text(&w.render(&palette, (110, 34)));
        assert!(text.contains("studio needs 160x45"), "{text}");

        w.advance(10.0); // well past SMALL_NOTICE_SECONDS
        let text = crate::frame::to_text(&w.render(&palette, (110, 34)));
        assert!(!text.contains("studio needs"), "{text}");
    }

    /// Resizing across the threshold switches the studio for the tab fallback (or back), but
    /// keeps the pane the owner chose (#501's acceptance criteria: "keeps the chosen zoom").
    #[test]
    fn resizing_across_the_threshold_keeps_the_chosen_pane() {
        let mut w = Watcher::new(&header(Nation::Earth)).unwrap();
        w.set_frame_size(160, 45);
        w.key(digit('1')); // zoom the code pane
        assert!(w.zoomed);
        assert_eq!(w.tab, Pane::Code);

        let palette =
            Palette::new(&fn_sim::Defs::bundled(), Nation::Earth, ColorDepth::TrueColor, false);
        let text = crate::frame::to_text(&w.render(&palette, (110, 34)));
        assert_eq!(w.tab, Pane::Code, "the chosen pane survives the resize");
        assert!(text.contains("no scripts replayed yet"), "{text}: the code tab should show");
    }

    /// The tab strip's action text is cut before its sparkline or tab indicator are, once the
    /// row is too narrow for all three (#501's acceptance criteria).
    #[test]
    fn the_tab_strips_action_text_is_cut_first_when_the_row_is_narrow() {
        let mut w = Watcher::new(&header(Nation::Earth)).unwrap();
        w.feed([
            LogLine::Keys {
                keys: "h".to_owned(),
                label: Some("harvest".into()),
                note: Some("a very long note that will not fit on a narrow strip at all".into()),
            },
            LogLine::Turn { index: 0, tick: u64::from(10 * TICKS_PER_SECOND) },
        ]);
        w.advance(0.5);
        assert!(matches!(w.current, Some(Current::Ticks { .. })), "should be mid-turn");

        let palette =
            Palette::new(&fn_sim::Defs::bundled(), Nation::Earth, ColorDepth::TrueColor, false);
        // Both below the studio's own threshold (height 40 < 45): tabs either way.
        let wide = crate::frame::to_text(&w.render(&palette, (150, 40)));
        assert!(wide.contains("a very long note"), "{wide}");
        assert!(wide.contains("acts "), "{wide}");

        let narrow = crate::frame::to_text(&w.render(&palette, (90, 40)));
        assert!(!narrow.contains("a very long note"), "{narrow}: the action should be cut first");
        assert!(narrow.contains("acts "), "{narrow}: the sparkline should still fit");
        assert!(narrow.contains("1[2]3"), "{narrow}");
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
        assert!(w.strip_text(false).contains("FOLLOW"), "{}", w.strip_text(false));
        w.toggle_view();
        assert!(w.strip_text(false).contains("FREE"), "{}", w.strip_text(false));
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

        // Wraps the whole run as if a script made it (#482): the spectator must still never
        // touch the replay with `Script`/`Wake` lines (and the strip they drive) in the mix,
        // not only with the plain `Keys`/`Turn`/`Look`/`Inspect` lines above.
        let scripted_lines = || {
            let mut lines = vec![LogLine::Script {
                name: "keep-fed".into(),
                version: 3,
                code: String::new(),
                turns: 45,
                wake_on: Vec::new(),
            }];
            lines.extend(h.log().lines());
            lines.push(LogLine::Wake {
                reason: "done".to_owned(),
                why: None,
                turn: 0,
                memory: serde_json::json!({}),
            });
            lines
        };

        let mut plain = Watcher::new(&h.header()).unwrap();
        plain.feed(scripted_lines());
        plain.fast_forward_to_turn(u32::MAX);
        assert!(plain.caught_up());

        let mut spied = Watcher::new(&h.header()).unwrap();
        spied.feed(scripted_lines());
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
