//! The web replayer (#595, docs/design/agent-play.md, "Watching on the web"): `fn_tui`'s run-log
//! [`Watcher`] compiled to wasm32, so a browser replays a `run.jsonl` the same way
//! `cargo xtask agent-run watch` does in a terminal -- by re-simulating it from its seed and
//! pressing the logged keys, not by playing back a recording.
//!
//! [`Replay`] is the whole replayer: a watcher, the palette it draws with, and snapshots for
//! seeking backwards. It never reads a clock or a file: the page hands it the log's text and
//! the real seconds between frames. [`abi`] is the thin C ABI the page calls (no wasm-bindgen,
//! so the build needs nothing beyond the `wasm32-unknown-unknown` target), and [`encode`] turns
//! a frame into the flat cell array the page draws onto a canvas. [`scan`] replays the whole log
//! once more, apart from the one being watched, to find the events the timeline marks.

pub mod abi;
pub mod encode;
pub mod scan;

use std::collections::BTreeMap;

use fn_tui::color::{ColorDepth, Palette};
use fn_tui::keyev::{KeyCode, KeyEvent, KeyModifiers};
use fn_tui::surface::LogLine;
use fn_tui::watch::{self, Watcher};
use ratatui::buffer::Buffer;

/// A snapshot is kept at every turn that is a multiple of this, as the replay first reaches it.
/// A backwards seek restores the nearest one at or before its target and fast-forwards from
/// there, so it never replays more than this many turns. At ~50 ms a turn in a browser, that's
/// about a second, while a 3 h run (540 turns) holds 27 snapshots.
pub const SNAPSHOT_EVERY: u32 = 20;

/// The pace a replay starts at: `cargo xtask agent-run watch`'s default.
pub const START_PACE: u32 = 20;

/// One log being replayed.
pub struct Replay {
    watcher: Watcher,
    palette: Palette,
    /// Watchers as they were at the end of the turn they're keyed by (turn 0: before anything
    /// was replayed). Only turns the replay has already reached are here.
    snapshots: BTreeMap<u32, Watcher>,
    /// How many `Turn` lines the log holds: the scrubber's length.
    total_turns: u32,
    /// The turn a seek is heading for, until [`Self::seek_step`] gets there.
    seek_target: Option<u32>,
    /// The log header's `build` (the commit it was made on), for the page to show.
    build: Option<String>,
}

impl Replay {
    /// A replay of the whole log `text`, at its first frame. Refuses a log whose `data_digest`
    /// doesn't match this build's data (`Watcher::new`'s rule), naming the build it was made on.
    pub fn new(text: &str) -> Result<Self, String> {
        let (header, lines) = watch::parse(&format!("{text}\n"))?;
        let build = header.build.clone();
        let mut watcher = Watcher::new(&header).map_err(|e| match &build {
            Some(b) => format!("{e} (the log was made on build {b})"),
            None => e,
        })?;
        let total_turns = lines.iter().filter(|l| matches!(l, LogLine::Turn { .. })).count();
        watcher.feed(lines);
        // `cargo xtask agent-run watch`'s own starting pace.
        while watcher.pace() < START_PACE {
            watcher.faster();
        }
        let palette = Palette::new(watcher.defs(), watcher.nation(), ColorDepth::TrueColor, false);
        let snapshots = BTreeMap::from([(0, watcher.clone())]);
        Ok(Self {
            watcher,
            palette,
            snapshots,
            total_turns: total_turns as u32,
            seek_target: None,
            build,
        })
    }

    /// Plays `real_secs` more of the replay at the current pace (nothing while paused or
    /// seeking).
    pub fn advance(&mut self, real_secs: f64) {
        if self.seek_target.is_some() {
            return;
        }
        let before = self.watcher.turn();
        self.watcher.advance(real_secs);
        if self.watcher.turn() != before {
            self.snapshot_if_due();
        }
    }

    /// One key, as the terminal watcher reads it (`watch::control_key`): Space pauses, `+`/`-`
    /// change the pace, `.` steps, `]`/`}` jump one or ten turns ahead, `>` skips to the agent's
    /// next move, `0`-`3` pick the studio panes, and the rest is free look. `[`/`{` go back one or
    /// ten turns through [`Self::seek`], as the terminal's own rewind does. `q` does nothing
    /// here; there's nothing to quit to.
    pub fn key(&mut self, key: KeyEvent) {
        if matches!(key.code, KeyCode::Char('q' | 'Q')) || self.seek_target.is_some() {
            return;
        }
        let back = match key.code {
            KeyCode::Char('[') => Some(1),
            KeyCode::Char('{') => Some(watch::BIG_STEP_TURNS),
            _ => None,
        };
        if let Some(turns) = back {
            self.seek_now(self.watcher.turn().saturating_sub(turns));
            return;
        }
        let before = self.watcher.turn();
        watch::control_key(&mut self.watcher, key);
        if self.watcher.turn() != before {
            self.snapshot_if_due();
        }
    }

    /// Starts a seek to the end of `turn` (clamped to the log's length). Going backwards, or
    /// forwards past a snapshot, restores the nearest snapshot before it first; the rest happens
    /// a turn at a time in [`Self::seek_step`], so the page stays responsive.
    pub fn seek(&mut self, turn: u32) {
        let turn = turn.min(self.total_turns);
        // The nearest snapshot strictly before the target (turn 0's for a seek to 0): a
        // snapshot's watcher may already be partway into the turn after its own, so it is only
        // ever fast-forwarded from, never shown as the end of its own turn.
        let from = *self.snapshots.range(..turn.max(1)).next_back().expect("turn 0's is kept").0;
        // The watcher can't go back, and may be partway past the end of `turn` even when its
        // turn count equals it, so anything not strictly ahead restores.
        if turn <= self.watcher.turn() || from > self.watcher.turn() {
            self.restore(from);
        }
        self.seek_target = Some(turn);
    }

    /// One turn more of a seek. `true` while it still has further to go.
    pub fn seek_step(&mut self) -> bool {
        let Some(target) = self.seek_target else { return false };
        if self.watcher.turn() < target {
            self.watcher.fast_forward_to_turn(self.watcher.turn() + 1);
            self.snapshot_if_due();
        }
        let more = self.watcher.turn() < target && !self.watcher.caught_up();
        if !more {
            self.seek_target = None;
        }
        more
    }

    /// Runs a seek to the end, all at once: for tests and a native caller.
    pub fn seek_now(&mut self, turn: u32) {
        self.seek(turn);
        while self.seek_step() {}
    }

    /// The frame at `width` x `height` cells.
    pub fn render(&mut self, width: u16, height: u16) -> Buffer {
        self.watcher.resize(width, height);
        self.watcher.set_frame_size(width, height);
        self.watcher.render(&self.palette, (width, height))
    }

    pub fn turn(&self) -> u32 {
        self.watcher.turn()
    }

    pub fn total_turns(&self) -> u32 {
        self.total_turns
    }

    pub fn seeking(&self) -> bool {
        self.seek_target.is_some()
    }

    pub fn paused(&self) -> bool {
        self.watcher.paused()
    }

    pub fn pace(&self) -> u32 {
        self.watcher.pace()
    }

    /// Nothing left to replay.
    pub fn at_end(&self) -> bool {
        self.watcher.caught_up()
    }

    pub fn build(&self) -> Option<&str> {
        self.build.as_deref()
    }

    /// The watcher itself, for tests.
    pub fn watcher(&self) -> &Watcher {
        &self.watcher
    }

    fn snapshot_if_due(&mut self) {
        let turn = self.watcher.turn();
        if turn.is_multiple_of(SNAPSHOT_EVERY) && !self.snapshots.contains_key(&turn) {
            self.snapshots.insert(turn, self.watcher.clone());
        }
    }

    /// Goes back to the snapshot at `turn`, keeping the viewer's own choices (pace, pause).
    fn restore(&mut self, turn: u32) {
        let (pace, paused) = (self.watcher.pace(), self.watcher.paused());
        self.watcher = self.snapshots[&turn].clone();
        while self.watcher.pace() < pace {
            self.watcher.faster();
        }
        while self.watcher.pace() > pace {
            self.watcher.slower();
        }
        if self.watcher.paused() != paused {
            self.watcher.toggle_pause();
        }
    }
}

/// The key a page sends as `code` (a Unicode scalar, or one of these above the Unicode range)
/// and `mods` (bit 0 Shift, bit 1 Ctrl, bit 2 Alt). `None` for a code that's neither.
pub fn key_from_codes(code: u32, mods: u32) -> Option<KeyEvent> {
    const SPECIAL: u32 = 0x11_0000;
    let code = match code {
        c if c < SPECIAL => KeyCode::Char(char::from_u32(c)?),
        c => match c - SPECIAL {
            0 => KeyCode::Enter,
            1 => KeyCode::Esc,
            2 => KeyCode::Left,
            3 => KeyCode::Right,
            4 => KeyCode::Up,
            5 => KeyCode::Down,
            6 => KeyCode::Tab,
            7 => KeyCode::BackTab,
            8 => KeyCode::Backspace,
            9 => KeyCode::Home,
            10 => KeyCode::End,
            11 => KeyCode::PageUp,
            12 => KeyCode::PageDown,
            13 => KeyCode::Delete,
            _ => return None,
        },
    };
    let mut m = KeyModifiers::NONE;
    if mods & 1 != 0 {
        m |= KeyModifiers::SHIFT;
    }
    if mods & 2 != 0 {
        m |= KeyModifiers::CONTROL;
    }
    if mods & 4 != 0 {
        m |= KeyModifiers::ALT;
    }
    Some(KeyEvent::new(code, m))
}
