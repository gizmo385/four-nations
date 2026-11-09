//! The agent studio's feed (docs/design/agent-studio.md, "The agent feed"): the trail of calls
//! that led to each run, built from the same `LogLine`s `crate::watch::Watcher` replays, one
//! entry a line. [`Feed`] is pure state -- it never touches a terminal or the sim, and never
//! bakes a choice between the ASCII and Unicode glyph sets into what it keeps (the same reason
//! `crate::studio::CodePane::header_text` takes `ascii` as an argument rather than storing it) --
//! fed by `Watcher::start_next` alongside `crate::studio::CodePane`, and drawn into a `Buffer` by
//! [`Feed::render`].
//!
//! A `call` line (#497) is the one unambiguous place a new entry starts, but most of what an
//! entry actually *says* comes from the lines that follow it (a `save_script`/`run_script`
//! call's own summary knows nothing about the script's name or diff until its `Script` line
//! lands; `act`'s summary is just a step count until the `Keys` line it dispatched carries the
//! real label; `inspect` and `notes` carry nothing of their own at all). So this module leans on
//! the structural lines (`Script`, `Wake`, `Keys`, `Turn`, `Look`, `Inspect`, `Note`) for every
//! entry's own text, in exactly the shape the design's own table lays out, and only asks whether
//! a `call` line was replayed at all to decide how `Self::calls` counts: once real `call` lines
//! are in the log, replaying one always comes (and, in a replay, lands) right before the
//! structural lines it announces, with nothing between them to pace against -- so there is
//! nothing useful a `call` line's own text would add that the structural lines won't say better
//! a moment later. A log with none at all (`fn_agent play`, or one made before #497) gets the
//! exact same entries, just opened by the structural lines themselves (docs/design/
//! agent-studio.md: "Logs with no `call` lines... gets synthetic entries from the same lines"),
//! with `Self::calls` then counting those openings instead.
//!
//! A `say` line (#576, a playtest's remark) is the one entry that is not a call at all: it is
//! drawn as `says: ...`, wrapped to the box, with a `game problem:` line given a row of its own,
//! marked `!` and highlighted. "says", because it is what the model chose to write and not its
//! reasoning.

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};

use crate::studio::ScriptDiff;
use crate::widgets::Marks;

/// One finished entry, built once and never revisited -- only the still-open [`Pending`] entry
/// (if any) changes as more of the replay lands. The ASCII-sensitive separators (`·`/`-`,
/// `→`/`->`) stay unresolved until [`Line::format`], at render time.
#[derive(Clone, Debug, PartialEq)]
enum Line {
    /// Already fully formatted text with no ASCII-sensitive characters of its own: `act ...`,
    /// `end_turn N`, `look (x,y) wxh`, `inspect (x,y)`, `notes + "..."`.
    Plain(String),
    /// A dimmed sub-line, the same ASCII-insensitivity as [`Self::Plain`]: a running script's
    /// latest act label.
    Dim(String),
    /// `save_script name vN  <diff>` (docs/design/agent-studio.md's own table): self-contained,
    /// since both `version` and `diff` are everything this entry ever needs. `save_script`
    /// always assigns a version of 1 or more (saving is what makes a script "versioned" at
    /// all), so this never needs [`labelled`]'s own "no suffix" fallback.
    Save { name: String, version: u32, diff: ScriptDiff },
    /// A finished run: `run_script name vN · Bt → outcome` (`labelled`'s own rule for `name vN`,
    /// since an ad hoc run's `version` is 0). `outcome` is already fully formatted (the design's
    /// own `woke: why` / `budget, k acts` / `done` / `error: ...`), with no ASCII-sensitive
    /// characters of its own.
    Woken { name: String, version: u32, budget: u32, outcome: String },
    /// Something the model said in plain text (#576, a `say` line): wrapped to the feed's width
    /// when drawn, so it is kept whole here. Not the model's reasoning: only what it chose to
    /// write, which is why the label reads "says".
    Says(String),
}

/// How a row of the feed is drawn.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Kind {
    Normal,
    Dim,
    /// A remark's own text.
    Says,
    /// A `game problem:` line (#576): marked and highlighted so it stands out of the remarks.
    Problem,
}

/// One drawn row, before the marker column is added.
#[derive(Clone, Debug, PartialEq)]
struct Row {
    text: String,
    kind: Kind,
    /// Whether this row begins a thing (it gets the marker) rather than continues a wrapped one.
    first: bool,
}

impl Row {
    fn new(text: String, kind: Kind) -> Self {
        Self { text, kind, first: true }
    }
}

impl Line {
    /// `(text, dim)`, with every ASCII-sensitive separator resolved against `ascii`. A remark
    /// has no single row: [`Self::rows`] is what the feed draws.
    fn format(&self, ascii: bool) -> (String, bool) {
        match self {
            Self::Plain(s) => (s.clone(), false),
            Self::Dim(s) => (s.clone(), true),
            Self::Says(s) => (s.clone(), false),
            Self::Save { name, version, diff } => {
                (format!("save_script {}  {}", labelled(name, *version), diff.text(ascii)), false)
            }
            Self::Woken { name, version, budget, outcome } => {
                let dot = if ascii { "-" } else { "·" };
                let arrow = if ascii { "->" } else { "→" };
                let head = labelled(name, *version);
                (format!("run_script {head} {dot} {budget}t {arrow} {outcome}"), false)
            }
        }
    }

    /// The rows this line draws in `width` columns (the marker column not counted).
    fn rows(&self, ascii: bool, width: usize) -> Vec<Row> {
        match self {
            Self::Says(text) => say_rows(text, width),
            other => {
                let (text, dim) = other.format(ascii);
                vec![Row::new(text, if dim { Kind::Dim } else { Kind::Normal })]
            }
        }
    }
}

/// The label a remark's first line carries. "says", because that is all it is: what the model
/// chose to write, not what it was thinking.
const SAYS: &str = "says: ";

/// Whether `line` is a `game problem:` line (#576), as the playtest prompt asks the model to
/// write them: the words at the start (after a list bullet, if any), in any case.
pub fn is_game_problem(line: &str) -> bool {
    let line = line.trim_start().trim_start_matches(['-', '*']).trim_start();
    line.get(..13).is_some_and(|head| head.eq_ignore_ascii_case("game problem:"))
}

/// `text` as rows of at most `width` columns: each line of the remark wrapped at words, its first
/// row carrying `says: `, a `game problem:` line standing as its own highlighted row (or rows)
/// whatever its place in the remark.
fn say_rows(text: &str, width: usize) -> Vec<Row> {
    let mut rows = Vec::new();
    let mut said = false;
    for line in text.lines().map(str::trim).filter(|l| !l.is_empty()) {
        if is_game_problem(line) {
            let line = line.trim_start_matches(['-', '*']).trim_start();
            rows.extend(wrap(line, width, "", Kind::Problem));
        } else {
            let label = if said { "" } else { SAYS };
            said = true;
            rows.extend(wrap(line, width, label, Kind::Says));
        }
    }
    rows
}

/// `text` wrapped to `width` columns, `label` ahead of the first row and spaces of its width
/// ahead of the others. A word longer than a row is broken. Always at least one row.
fn wrap(text: &str, width: usize, label: &str, kind: Kind) -> Vec<Row> {
    let indent = label.chars().count();
    let room = width.saturating_sub(indent).max(1);
    let mut lines: Vec<String> = Vec::new();
    let mut cur = String::new();
    let mut cur_len = 0;
    for word in text.split_whitespace() {
        let mut word: &str = word;
        loop {
            let len = word.chars().count();
            let sep = usize::from(cur_len > 0);
            if cur_len + sep + len <= room {
                if sep == 1 {
                    cur.push(' ');
                }
                cur.push_str(word);
                cur_len += sep + len;
                break;
            }
            if cur_len > 0 {
                lines.push(std::mem::take(&mut cur));
                cur_len = 0;
                continue;
            }
            // A word wider than a whole row: break it.
            let cut = word.char_indices().nth(room).map_or(word.len(), |(i, _)| i);
            lines.push(word[..cut].to_owned());
            word = &word[cut..];
            if word.is_empty() {
                break;
            }
        }
    }
    if !cur.is_empty() || lines.is_empty() {
        lines.push(cur);
    }
    lines
        .into_iter()
        .enumerate()
        .map(|(i, l)| {
            let text =
                if i == 0 { format!("{label}{l}") } else { format!("{}{l}", " ".repeat(indent)) };
            Row { text, kind, first: i == 0 }
        })
        .collect()
}

/// `name vN`, or just `name` for an ad hoc run that was never saved (`version == 0`, the same
/// convention `crate::watch::ScriptStrip::label` uses for the plain strip).
fn labelled(name: &str, version: u32) -> String {
    if version > 0 { format!("{name} v{version}") } else { name.to_owned() }
}

/// A run in progress, between its `Script{turns > 0}` line and its `Wake` (docs/design/
/// agent-studio.md's own table, second row).
#[derive(Clone, Debug, PartialEq)]
struct Run {
    name: String,
    /// The `Script` line's own `version` (0 for an ad hoc, never-saved run -- `labelled`'s own
    /// "no suffix" case), not `crate::studio::CodePane`'s own revision ordinal (which numbers an
    /// ad hoc run's distinct codes from 1, for its own diffing -- a different thing this module
    /// never needs).
    version: u32,
    budget: u32,
    /// The watcher's own turn when this run's `Script` line landed: `Self::live_lines`'s own
    /// `current_turn` minus this is how many turns the run has had so far.
    start_turn: u32,
    acts: u32,
    latest_act: Option<String>,
}

impl Run {
    /// While still running: `(text, dim)` for the head line (`turn i/N · k acts` appended to
    /// `run_script name vN · Bt`), plus a dimmed sub-line naming the latest act, if there has
    /// been one yet.
    fn live_lines(&self, current_turn: u32, ascii: bool) -> Vec<(String, bool)> {
        let dot = if ascii { "-" } else { "·" };
        let elapsed = current_turn.saturating_sub(self.start_turn);
        let head = labelled(&self.name, self.version);
        let mut out = vec![(
            format!(
                "run_script {head} {dot} {}t   turn {elapsed}/{} {dot} {} acts",
                self.budget, self.budget, self.acts
            ),
            false,
        )];
        if let Some(act) = &self.latest_act {
            out.push((format!("{dot} {act}"), true));
        }
        out
    }

    /// `woke: why`, or `budget, k acts` / `done` / `error: reason` with no `why`
    /// (docs/design/agent-studio.md's own table), with no ASCII-sensitive characters of its own.
    fn outcome(&self, reason: &str, why: Option<&str>) -> String {
        match why {
            Some(why) => format!("woke: {why}"),
            None if reason == "budget" => format!("budget, {} acts", self.acts),
            None if reason == "done" => "done".to_owned(),
            None => format!("error: {reason}"),
        }
    }
}

/// What's still open, waiting for the lines that will finish it (docs/design/agent-studio.md's
/// own table: "The lines after the call... fill that entry in"). At most one at a time -- a
/// `run_script` call's own acts and turns never overlap another call's (the design's own
/// assumption throughout).
#[derive(Clone, Debug, PartialEq)]
enum Pending {
    Run(Run),
    /// `act` (or, with no `call` lines at all, any `Keys` line outside a run): every label (and
    /// quoted note) it pressed, joined by `;` once this closes.
    Act {
        labels: Vec<String>,
    },
    /// With no `call` lines, consecutive `Turn` entries outside a run count toward one
    /// `end_turn N` (real `call` lines already carry `N` in their own summary, so this grouping
    /// is the synthetic fallback's only job).
    EndTurnGroup {
        count: u32,
    },
}

/// The agent feed: every entry replayed so far, and the one still being filled in, if any. Built
/// once by `crate::watch::Watcher::new` and fed alongside it, the same shape `crate::studio::
/// CodePane` is.
#[derive(Clone, Debug, Default)]
pub struct Feed {
    lines: Vec<Line>,
    pending: Option<Pending>,
    /// Set the first time a real `call` line is replayed: from then on, `Self::calls` counts
    /// those instead of the entries this module opens on its own (the module's own doc comment).
    saw_call: bool,
    calls: u32,
    runs: u32,
}

impl Feed {
    pub fn new() -> Self {
        Self::default()
    }

    /// `AGENT ── N calls · M runs` (`1 call`/`1 run`, singular, #501): what's been replayed so
    /// far, counting whichever of a real `call` line or a synthetic entry opened it
    /// (`Self::saw_call`'s own doc comment).
    pub fn title(&self, ascii: bool) -> String {
        let dash = if ascii { "--" } else { "──" };
        let dot = if ascii { "-" } else { "·" };
        let call = if self.calls == 1 { "call" } else { "calls" };
        let run = if self.runs == 1 { "run" } else { "runs" };
        format!("AGENT {dash} {} {call} {dot} {} {run}", self.calls, self.runs)
    }

    /// A `call` line reaches the replay: nothing about any entry's own text (the module's own
    /// doc comment), just that one more call has happened, and that `Self::calls` should count
    /// real `call` lines from here on.
    pub fn on_call(&mut self) {
        self.saw_call = true;
        self.calls += 1;
    }

    /// Counts one more synthetic "call" -- an entry this module opened on its own, because the
    /// log never had a real `call` line to count instead (`Self::saw_call`'s own doc comment).
    /// A no-op once a real one has been seen.
    fn count_synthetic_call(&mut self) {
        if !self.saw_call {
            self.calls += 1;
        }
    }

    /// Closes whatever's open in [`Self::pending`] into a finished [`Line`] (or two, for a run
    /// still mid-flight -- shouldn't happen in a well-formed log, since nothing closes a run but
    /// its own `Wake`, but a defensive fallback beats silently dropping it: it bakes its own
    /// ASCII-sensitive separators rather than threading `ascii` through every caller for a path
    /// that should never run), before a different kind of entry opens.
    fn finish_pending(&mut self, current_turn: u32) {
        match self.pending.take() {
            None => {}
            Some(Pending::Run(run)) => {
                for (text, dim) in run.live_lines(current_turn, true) {
                    self.lines.push(if dim { Line::Dim(text) } else { Line::Plain(text) });
                }
            }
            Some(Pending::Act { labels }) => self.lines.push(Line::Plain(act_text(&labels))),
            Some(Pending::EndTurnGroup { count }) => {
                self.lines.push(Line::Plain(format!("end_turn {count}")));
            }
        }
    }

    /// A `Script` line reaches the replay: `turns == 0` is a `save_script` (self-contained --
    /// `version` and `diff` are everything the entry ever needs, so it closes at once); `turns >
    /// 0` is a `run_script` starting (`Pending::Run`, filled in turn by turn until its `Wake`).
    /// `version` is the line's own field, 0 for an ad hoc run (`Run`'s own doc comment) -- never
    /// `crate::studio::CodePane`'s revision ordinal, which a caller (`crate::watch::Watcher::
    /// start_next`) only ever needs to look up `diff` with, not to label this entry.
    pub fn on_script(&mut self, name: &str, version: u32, turns: u32, diff: ScriptDiff, turn: u32) {
        self.finish_pending(turn);
        self.count_synthetic_call();
        if turns == 0 {
            self.lines.push(Line::Save { name: name.to_owned(), version, diff });
        } else {
            self.runs += 1;
            self.pending = Some(Pending::Run(Run {
                name: name.to_owned(),
                version,
                budget: turns,
                start_turn: turn,
                acts: 0,
                latest_act: None,
            }));
        }
    }

    /// The matching `Wake` line: closes the open run into its own one-line `→ outcome` (the
    /// design's own mockup), a no-op if nothing was running (a malformed log; nothing here
    /// should ever see one).
    pub fn on_wake(&mut self, reason: &str, why: Option<&str>) {
        let Some(Pending::Run(run)) = self.pending.take() else { return };
        let outcome = run.outcome(reason, why);
        self.lines.push(Line::Woken {
            name: run.name,
            version: run.version,
            budget: run.budget,
            outcome,
        });
    }

    /// A `Keys` line reaches the replay. Inside a run (`Pending::Run`), it's one more of that
    /// run's own acts: the progress line's `k acts` counts it, and its label becomes the dimmed
    /// sub-line (docs/design/agent-studio.md's own mockup). Outside a run, it opens or continues
    /// an `act` entry, joining every label (and quoted note) with `;` (the design's own "keys
    /// (outside a run)" row) until a different kind of line closes it.
    pub fn on_keys(&mut self, label: Option<&str>, note: Option<&str>, keys: &str, turn: u32) {
        if let Some(Pending::Run(run)) = &mut self.pending {
            run.acts += 1;
            run.latest_act = Some(describe(label, note, keys));
            return;
        }
        if !matches!(self.pending, Some(Pending::Act { .. })) {
            self.finish_pending(turn);
            self.count_synthetic_call();
            self.pending = Some(Pending::Act { labels: Vec::new() });
        }
        if let Some(Pending::Act { labels }) = &mut self.pending {
            labels.push(describe(label, note, keys));
        }
    }

    /// A `Turn` line reaches the replay. Inside a run it's just one of its own turns passing
    /// (nothing of its own to show -- the run's progress line already counts elapsed turns, not
    /// ticks). Outside a run, with no `call` line to have already said `end_turn N`, consecutive
    /// `Turn`s group into one entry until something else closes it.
    pub fn on_turn(&mut self, turn: u32) {
        if matches!(self.pending, Some(Pending::Run(_))) {
            return;
        }
        if self.saw_call {
            // A real `call` line already wrote `end_turn N` in full (the module's own doc
            // comment): nothing left for the `Turn` lines that follow it to add.
            return;
        }
        if !matches!(self.pending, Some(Pending::EndTurnGroup { .. })) {
            self.finish_pending(turn);
            self.count_synthetic_call();
            self.pending = Some(Pending::EndTurnGroup { count: 0 });
        }
        if let Some(Pending::EndTurnGroup { count }) = &mut self.pending {
            *count += 1;
        }
    }

    /// A `Look`/`Inspect` line reaches the replay: a script's own read (inside a run) draws no
    /// feed entry either, the same reasoning it draws no attention ghost
    /// (`crate::watch::Watcher::start_next`'s own rule for `Script`-bracketed reads). Outside a
    /// run it's always its own one-line entry (docs/design/agent-studio.md's own "look /
    /// inspect" row), whether or not a `call` line preceded it -- `inspect`'s own `call` summary
    /// is empty either way (`fn_agent::tools::call_summary`), so there is nothing to lean on but
    /// the read itself.
    pub fn on_look(&mut self, x: i32, y: i32, w: u16, h: u16, turn: u32) {
        self.push_read(format!("look ({x},{y}) {w}x{h}"), turn);
    }

    pub fn on_inspect(&mut self, x: i32, y: i32, turn: u32) {
        self.push_read(format!("inspect ({x},{y})"), turn);
    }

    fn push_read(&mut self, text: String, turn: u32) {
        if matches!(self.pending, Some(Pending::Run(_))) {
            return;
        }
        self.finish_pending(turn);
        self.count_synthetic_call();
        self.lines.push(Line::Plain(text));
    }

    /// A `Note` line reaches the replay: `notes + "text"` (docs/design/agent-studio.md's own
    /// mockup), its own one-line entry.
    pub fn on_note(&mut self, text: &str, turn: u32) {
        self.push_read(format!("notes + {text:?}"), turn);
    }

    /// A `Say` line reaches the replay (#576): the model's remark, as its own entry, in the order
    /// the log has it. It is not a call and doesn't count as one. It closes whatever act group is
    /// open, so the remark sits between the acts it was said between; but a run in progress stays
    /// open (its own acts and turns go on counting), the remark drawn above the run's live lines.
    pub fn on_say(&mut self, text: &str, turn: u32) {
        if text.trim().is_empty() {
            return;
        }
        if !matches!(self.pending, Some(Pending::Run(_))) {
            self.finish_pending(turn);
        }
        self.lines.push(Line::Says(text.to_owned()));
    }

    /// The feed's own border, title and the last entries that fit `area` -- newest at the
    /// bottom, tailing automatically (docs/design/agent-studio.md: "The feed only shows what's
    /// been replayed... Newest is at the bottom and it tails automatically"). A no-op on an area
    /// too small to draw a border in at all.
    pub fn render(&self, buf: &mut Buffer, area: Rect, ascii: bool, current_turn: u32) {
        if area.width < 4 || area.height < 3 {
            return;
        }
        let marks = Marks::new(ascii);
        draw_box(buf, area, &marks, &self.title(ascii));
        let inner =
            Rect::new(area.x + 1, area.y + 1, area.width.saturating_sub(2), area.height - 2);

        // The marker column is two wide (a pointer and a space), whatever the row.
        let width = usize::from(inner.width).saturating_sub(2);
        let mut all: Vec<Row> = self.lines.iter().flat_map(|l| l.rows(ascii, width)).collect();
        if let Some(Pending::Run(run)) = &self.pending {
            all.extend(
                run.live_lines(current_turn, ascii)
                    .into_iter()
                    .map(|(text, dim)| Row::new(text, if dim { Kind::Dim } else { Kind::Normal })),
            );
        }
        let start = all.len().saturating_sub(usize::from(inner.height));
        for (row, r) in all[start..].iter().enumerate() {
            let (marker, style) = match r.kind {
                Kind::Dim => (" ", Style::new().add_modifier(Modifier::DIM)),
                Kind::Problem => ("!", Style::new().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
                Kind::Normal | Kind::Says => (marks.pointer, Style::new()),
            };
            // A wrapped row's continuation has no marker of its own.
            let marker = if r.first || r.kind == Kind::Dim { marker } else { " " };
            let shown = format!("{marker} {}", r.text);
            let y = inner.y + row as u16;
            buf.set_stringn(inner.x, y, &shown, usize::from(inner.width), style);
        }
    }
}

/// `label` (or the raw `--keys` tokens, if there was none), with `note` in quotes if it had one
/// (the same shape `crate::watch::describe_action_line` reads for the agent strip).
fn describe(label: Option<&str>, note: Option<&str>, keys: &str) -> String {
    let what = label.unwrap_or(keys);
    match note {
        Some(note) if !note.is_empty() => format!("{what} {note:?}"),
        _ => what.to_owned(),
    }
}

/// `act <label1>; <label2>; ...` (docs/design/agent-studio.md's own "keys (outside a run)" row).
fn act_text(labels: &[String]) -> String {
    format!("act {}", labels.join("; "))
}

/// The feed box's own border: light corners, matching `crate::studio::draw_box`'s own reasoning
/// ("drawn over the map, not another frame").
fn draw_box(buf: &mut Buffer, area: Rect, marks: &Marks, title: &str) {
    for y in area.top()..area.bottom() {
        for x in area.left()..area.right() {
            let cell = &mut buf[(x, y)];
            cell.reset();
            cell.set_char(' ');
        }
    }
    let (l, r, t, b) = (area.left(), area.right() - 1, area.top(), area.bottom() - 1);
    for x in l + 1..r {
        buf[(x, t)].set_char(marks.h);
        buf[(x, b)].set_char(marks.h);
    }
    for y in t + 1..b {
        buf[(l, y)].set_char(marks.v);
        buf[(r, y)].set_char(marks.v);
    }
    buf[(l, t)].set_char(marks.box_tl);
    buf[(r, t)].set_char(marks.box_tr);
    buf[(l, b)].set_char(marks.box_bl);
    buf[(r, b)].set_char(marks.box_br);

    let room = usize::from(r.saturating_sub(l + 1));
    let text = format!(" {title} ");
    for (i, c) in text.chars().take(room).enumerate() {
        buf[(l + 1 + i as u16, t)].set_char(c);
    }
}

#[cfg(test)]
mod tests {
    use ratatui::buffer::Buffer;
    use ratatui::layout::Rect;

    use super::*;

    fn text(buf: &Buffer) -> String {
        crate::frame::to_text(buf)
    }

    #[test]
    fn a_fresh_feed_has_no_calls_or_runs() {
        let feed = Feed::new();
        assert!(feed.title(false).contains("0 calls"), "{}", feed.title(false));
        assert!(feed.title(false).contains("0 runs"), "{}", feed.title(false));
    }

    /// A `save_script` entry (`Script{turns: 0}`) is self-contained: the first revision reads
    /// `new, N lines`, a later one `+A -D`.
    #[test]
    fn save_script_entries_show_new_or_a_diff() {
        let mut feed = Feed::new();
        feed.on_script("keep-fed", 1, 0, ScriptDiff::New { lines: 22 }, 0);
        feed.on_script("keep-fed", 2, 0, ScriptDiff::Changed { added: 4, deleted: 1 }, 0);
        let mut buf = Buffer::empty(Rect::new(0, 0, 60, 10));
        feed.render(&mut buf, Rect::new(0, 0, 60, 10), false, 0);
        let rendered = text(&buf);
        assert!(rendered.contains("save_script keep-fed v1"), "{rendered}");
        assert!(rendered.contains("new, 22 lines"), "{rendered}");
        assert!(rendered.contains("save_script keep-fed v2"), "{rendered}");
        assert!(rendered.contains("+4") && rendered.contains('1'), "{rendered}");
    }

    /// A running script shows its progress (`turn i/N · k acts`), its latest act as a dimmed
    /// sub-line, and finishes on `Wake` with a single `→ outcome` line.
    #[test]
    fn a_run_shows_progress_then_its_wake() {
        let mut feed = Feed::new();
        feed.on_script("keep-fed", 3, 45, ScriptDiff::New { lines: 1 }, 10);
        feed.on_keys(Some("harvest"), Some("wood"), "h", 12);
        let mut buf = Buffer::empty(Rect::new(0, 0, 60, 10));
        feed.render(&mut buf, Rect::new(0, 0, 60, 10), false, 12);
        let running = text(&buf);
        assert!(running.contains("run_script keep-fed v3 · 45t"), "{running}");
        assert!(running.contains("turn 2/45 · 1 acts"), "{running}");
        assert!(running.contains("harvest"), "{running}");
        assert!(running.contains("\"wood\""), "{running}");

        feed.on_wake("event", Some("death: Ping died of thirst"));
        let mut buf = Buffer::empty(Rect::new(0, 0, 90, 10));
        feed.render(&mut buf, Rect::new(0, 0, 90, 10), false, 12);
        let woke = text(&buf);
        assert!(woke.contains("run_script keep-fed v3 · 45t"), "{woke}");
        assert!(woke.contains("→ woke: death: Ping died of thirst"), "{woke}");
        assert!(!woke.contains("turn 2/45"), "{woke}: the progress line should be gone");
        assert!(feed.title(false).contains("1 run"), "{}", feed.title(false));
    }

    /// An ad hoc run (`version == 0`) is labelled with its tag alone -- never `v0` or
    /// `CodePane`'s own revision ordinal.
    #[test]
    fn an_ad_hoc_run_has_no_version_suffix() {
        let mut feed = Feed::new();
        feed.on_script("scout", 0, 1, ScriptDiff::New { lines: 1 }, 0);
        let mut buf = Buffer::empty(Rect::new(0, 0, 60, 10));
        feed.render(&mut buf, Rect::new(0, 0, 60, 10), false, 0);
        let rendered = text(&buf);
        assert!(rendered.contains("run_script scout · 1t"), "{rendered}");
        assert!(!rendered.contains('v'), "{rendered}");
    }

    /// With no `why`, a `budget` wake counts the run's own acts, and a `done` wake just says so.
    #[test]
    fn a_wake_with_no_why_falls_back_to_the_reason() {
        let mut feed = Feed::new();
        feed.on_script("keep-fed", 1, 5, ScriptDiff::New { lines: 1 }, 0);
        feed.on_keys(None, None, "h", 1);
        feed.on_keys(None, None, "j", 2);
        feed.on_wake("budget", None);
        let mut buf = Buffer::empty(Rect::new(0, 0, 60, 10));
        feed.render(&mut buf, Rect::new(0, 0, 60, 10), false, 5);
        assert!(text(&buf).contains("budget, 2 acts"), "{}", text(&buf));
    }

    /// `act` (`Keys` lines outside a run) joins every label it pressed with `;`.
    #[test]
    fn act_entries_join_every_label_with_a_semicolon() {
        let mut feed = Feed::new();
        feed.on_keys(Some("designate harvest 6x4"), Some("wood"), "h <enter>", 0);
        feed.on_keys(Some("build Bed"), None, "b <enter>", 0);
        feed.on_look(1, 1, 3, 3, 0); // a different kind of line closes the group
        let mut buf = Buffer::empty(Rect::new(0, 0, 70, 10));
        feed.render(&mut buf, Rect::new(0, 0, 70, 10), false, 0);
        let rendered = text(&buf);
        assert!(rendered.contains("act designate harvest 6x4 \"wood\"; build Bed"), "{rendered}");
    }

    /// With no `call` lines, consecutive `Turn` entries outside a run group into one
    /// `end_turn N`.
    #[test]
    fn synthetic_end_turn_groups_consecutive_turns() {
        let mut feed = Feed::new();
        feed.on_turn(0);
        feed.on_turn(1);
        feed.on_turn(2);
        feed.on_look(5, 5, 1, 1, 3); // closes the group
        let mut buf = Buffer::empty(Rect::new(0, 0, 60, 10));
        feed.render(&mut buf, Rect::new(0, 0, 60, 10), false, 3);
        assert!(text(&buf).contains("end_turn 3"), "{}", text(&buf));
    }

    /// Once a real `call` line has been replayed, `Turn` lines add nothing of their own (the
    /// call already said `end_turn N` in full) -- but `Self::calls` only counts the real calls
    /// from then on, not any entry this module would otherwise have opened.
    #[test]
    fn real_call_lines_switch_the_calls_counter_and_stop_turn_grouping() {
        let mut feed = Feed::new();
        feed.on_call();
        feed.on_turn(0);
        feed.on_turn(1);
        let mut buf = Buffer::empty(Rect::new(0, 0, 60, 10));
        feed.render(&mut buf, Rect::new(0, 0, 60, 10), false, 1);
        assert!(!text(&buf).contains("end_turn"), "{}", text(&buf));
        assert!(feed.title(false).contains("1 call"), "{}", feed.title(false));
    }

    /// `look`/`inspect`/`notes` entries read exactly as the design's own table shows, and a
    /// script's own reads (inside a run) draw no entry at all.
    #[test]
    fn look_inspect_and_notes_entries_and_reads_inside_a_run_are_silent() {
        let mut feed = Feed::new();
        feed.on_look(80, 60, 6, 4, 0);
        feed.on_inspect(5, 5, 0);
        feed.on_note("thirst wakes me too late", 0);
        feed.on_script("keep-fed", 1, 5, ScriptDiff::New { lines: 1 }, 0);
        feed.on_look(1, 1, 1, 1, 0); // inside the run: silent
        let mut buf = Buffer::empty(Rect::new(0, 0, 70, 10));
        feed.render(&mut buf, Rect::new(0, 0, 70, 10), false, 0);
        let rendered = text(&buf);
        assert!(rendered.contains("look (80,60) 6x4"), "{rendered}");
        assert!(rendered.contains("inspect (5,5)"), "{rendered}");
        assert!(rendered.contains("notes + \"thirst wakes me too late\""), "{rendered}");
        // Only the three reads above and the run's own live progress line should show -- not a
        // fourth "look" entry for the one made inside the run.
        assert_eq!(rendered.matches("look").count(), 1, "{rendered}");
    }

    /// Rendering shows only the most recent entries that fit, newest at the bottom.
    #[test]
    fn rendering_tails_to_the_newest_entries() {
        let mut feed = Feed::new();
        for i in 0..10 {
            feed.on_look(i, i, 1, 1, 0);
        }
        let mut buf = Buffer::empty(Rect::new(0, 0, 40, 5));
        feed.render(&mut buf, Rect::new(0, 0, 40, 5), false, 0);
        let rendered = text(&buf);
        assert!(rendered.contains("look (9,9)"), "{rendered}");
        assert!(!rendered.contains("look (0,0)"), "{rendered}");
    }

    /// With `--ascii`, every separator this module bakes in falls back too.
    #[test]
    fn ascii_mode_uses_ascii_separators() {
        let mut feed = Feed::new();
        feed.on_script("keep-fed", 1, 5, ScriptDiff::New { lines: 1 }, 0);
        feed.on_wake("done", None);
        let mut buf = Buffer::empty(Rect::new(0, 0, 60, 10));
        feed.render(&mut buf, Rect::new(0, 0, 60, 10), true, 0);
        let rendered = text(&buf);
        assert!(rendered.contains("keep-fed v1 - 5t -> done"), "{rendered}");
        assert!(!rendered.contains('·') && !rendered.contains('→'), "{rendered}");
    }
}

#[cfg(test)]
mod say_tests {
    use ratatui::buffer::Buffer;
    use ratatui::layout::Rect;
    use ratatui::style::{Color, Modifier};

    use super::*;

    fn render(feed: &Feed, w: u16, h: u16, ascii: bool) -> Buffer {
        let mut buf = Buffer::empty(Rect::new(0, 0, w, h));
        feed.render(&mut buf, Rect::new(0, 0, w, h), ascii, 0);
        buf
    }

    fn rows_of(buf: &Buffer) -> Vec<String> {
        crate::frame::to_text(buf).lines().map(str::to_owned).collect()
    }

    #[test]
    fn a_remark_is_labelled_says_and_never_thinking() {
        let mut feed = Feed::new();
        feed.on_say("I expect water within ten tiles", 0);
        let rendered = crate::frame::to_text(&render(&feed, 60, 6, false));
        assert!(rendered.contains("says: I expect water within ten tiles"), "{rendered}");
        let lower = rendered.to_lowercase();
        assert!(!lower.contains("think") && !lower.contains("reason"), "{rendered}");
    }

    #[test]
    fn a_remark_is_not_a_call_and_an_empty_one_is_nothing() {
        let mut feed = Feed::new();
        feed.on_say("hello", 0);
        feed.on_say("   \n ", 0);
        assert!(feed.title(false).contains("0 calls"), "{}", feed.title(false));
        let rendered = crate::frame::to_text(&render(&feed, 40, 6, false));
        assert_eq!(rendered.matches("says:").count(), 1, "{rendered}");
    }

    /// Remarks keep the order of the log, between the entries they were said between.
    #[test]
    fn remarks_sit_between_the_entries_around_them() {
        let mut feed = Feed::new();
        feed.on_look(1, 1, 2, 2, 0);
        feed.on_say("first remark", 0);
        feed.on_inspect(3, 3, 0);
        feed.on_say("second remark", 0);
        let rows = rows_of(&render(&feed, 50, 10, false)).join("\n");
        let at = |needle: &str| rows.find(needle).unwrap_or_else(|| panic!("{needle} in {rows}"));
        assert!(at("look (1,1)") < at("first remark"));
        assert!(at("first remark") < at("inspect (3,3)"));
        assert!(at("inspect (3,3)") < at("second remark"));
    }

    /// A remark closes the act group it interrupts, so the acts before and after are two entries.
    #[test]
    fn a_remark_splits_an_act_group() {
        let mut feed = Feed::new();
        feed.on_keys(Some("build Bed"), None, "b", 0);
        feed.on_say("that looked cheap", 0);
        feed.on_keys(Some("build Well"), None, "w", 0);
        let rows = rows_of(&render(&feed, 50, 10, false)).join("\n");
        assert!(rows.contains("act build Bed"), "{rows}");
        assert!(!rows.contains("build Bed; build Well"), "{rows}");
    }

    /// A remark said while a run is open leaves the run open: its wake still closes it.
    #[test]
    fn a_remark_does_not_close_a_run() {
        let mut feed = Feed::new();
        feed.on_script("governor", 1, 5, ScriptDiff::New { lines: 1 }, 0);
        feed.on_say("running it now", 0);
        feed.on_wake("done", None);
        let rows = rows_of(&render(&feed, 60, 10, false)).join("\n");
        assert!(rows.contains("run_script governor v1 · 5t → done"), "{rows}");
        assert!(rows.contains("running it now"), "{rows}");
    }

    #[test]
    fn game_problem_lines_are_recognised() {
        for yes in [
            "game problem: the well gives no water",
            "Game Problem: x",
            "  - game problem: bullet",
            "* GAME PROBLEM: starred",
        ] {
            assert!(is_game_problem(yes), "{yes}");
        }
        for no in ["a game problem: later", "SDK gap: x", "game problems", "", "game"] {
            assert!(!is_game_problem(no), "{no}");
        }
    }

    /// A `game problem:` line is its own row, marked `!` and drawn bold in colour, whether it
    /// is the whole remark or a line inside a longer one.
    #[test]
    fn game_problem_lines_are_highlighted() {
        let mut feed = Feed::new();
        feed.on_say("Trying the stockpile.\ngame problem: the briefing says 4 beds, I see 3", 0);
        let buf = render(&feed, 70, 8, false);
        let rows = rows_of(&buf);
        let y = rows.iter().position(|r| r.contains("game problem:")).expect("a problem row");
        let x = rows[y].find("game problem:").unwrap();
        let col = rows[y][..x].chars().count() as u16;
        assert!(rows[y][..x].contains('!'), "{}", rows[y]);
        let cell = &buf[(col, y as u16)];
        assert_eq!(cell.fg, Color::Yellow);
        assert!(cell.modifier.contains(Modifier::BOLD));
        // The ordinary line above it is plain.
        let says = rows.iter().position(|r| r.contains("says: Trying")).expect("a says row");
        assert!(!rows[says].contains('!'), "{}", rows[says]);
        let plain = &buf[(4, says as u16)];
        assert_eq!(plain.fg, Color::Reset);
    }

    /// Wrapping at every width: no row is wider than the box, and every word survives (in order)
    /// when the box is tall enough, in both glyph sets.
    #[test]
    fn remarks_wrap_at_any_width_without_losing_words() {
        let text = "Water keeps dropping although the wells are staffed, which I did not expect; \
                    supercalifragilisticexpialidocious_is_one_very_long_unbroken_token afterwards";
        let want: String = text.chars().filter(|c| c.is_alphanumeric()).collect();
        for ascii in [false, true] {
            for w in 4..=90u16 {
                let mut feed = Feed::new();
                feed.on_say(text, 0);
                let rows = rows_of(&render(&feed, w, 80, ascii));
                assert!(rows.iter().all(|r| r.chars().count() <= usize::from(w)), "w={w}");
                if w >= 12 {
                    // Every row below the title, minus the box's sides and the marker column.
                    let body: String = rows[1..rows.len() - 1]
                        .iter()
                        .flat_map(|r| r.chars().skip(1))
                        .filter(|c| c.is_alphanumeric())
                        .collect();
                    assert!(body.contains(&want), "w={w} ascii={ascii}\n{}", rows.join("\n"));
                }
            }
        }
    }

    /// A short box shows the newest rows, so the latest remark is what's there.
    #[test]
    fn a_short_feed_tails_to_the_newest_remark() {
        let mut feed = Feed::new();
        for i in 0..10 {
            feed.on_say(&format!("remark number {i}"), 0);
        }
        let rendered = crate::frame::to_text(&render(&feed, 40, 5, false));
        assert!(rendered.contains("remark number 9"), "{rendered}");
        assert!(!rendered.contains("remark number 0"), "{rendered}");
    }
}
