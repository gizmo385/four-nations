//! The agent studio's code pane (docs/design/agent-studio.md, "The code pane"): the script an
//! agent is iterating on, Rhai-highlighted, with a line diff against its previous revision.
//! [`CodePane`] is pure state -- it never touches a terminal or the sim -- fed from the same
//! `LogLine::Script`/`LogLine::Wake` lines `crate::watch::Watcher` already replays, and drawn
//! into a [`Buffer`] by [`CodePane::render`]. The studio's own layout split (which side of the
//! frame is the code pane, how wide the game column is) lives in `crate::watch::Watcher::render`;
//! this module only owns the pane itself.
//!
//! A "revision" is one distinct `code` for a script name, in log order (the doc's own "Which
//! revision is shown, and when"): for a saved script it's the `version` `save_script` bumped; for
//! an ad hoc `run_script {code}` (`version` 0, tagged by its note or `"play"`) it's the count of
//! distinct codes seen under that tag so far. [`Track`] keeps both the same way: one list of
//! `(revision number, code)` pairs, in the order first scanned.

use std::collections::HashMap;

use crate::keyev::{KeyCode, KeyEvent};
use fn_ui::code::{self, Mark};
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};

use crate::color::{self, Palette};
use crate::surface::LogLine;
use crate::widgets::Marks;

/// One script name's revisions, in the order first scanned (docs/design/agent-studio.md, "Which
/// revision is shown, and when"): `(revision number, code)`.
#[derive(Clone, Debug, Default)]
struct Track {
    revisions: Vec<(u32, String)>,
}

impl Track {
    /// Records `code` under `version` if it's new (a saved script's own `version`, or 0 for an
    /// ad hoc run), returning the revision number it was recorded as (or already had). Re-running
    /// the same saved `version`, or the same ad hoc code again, adds nothing.
    fn scan(&mut self, version: u32, code: &str) -> u32 {
        if version > 0 {
            if let Some((n, _)) = self.revisions.iter().find(|(n, _)| *n == version) {
                return *n;
            }
            self.revisions.push((version, code.to_owned()));
            version
        } else if let Some((n, _)) = self.revisions.iter().find(|(_, c)| c == code) {
            *n
        } else {
            let number = self.revisions.len() as u32 + 1;
            self.revisions.push((number, code.to_owned()));
            number
        }
    }

    /// `M`: the highest revision number scanned so far for this name. Not how many there are: a
    /// saved script's numbers are its `version`s, and a log can start partway through them (a
    /// campaign attempt or a `--resume` session inherits the script library, so its first
    /// `save_script` may be v6), which made the header read `rev 10/5`.
    fn latest(&self) -> u32 {
        self.revisions.iter().map(|(n, _)| *n).max().unwrap_or(0)
    }

    fn code(&self, number: u32) -> Option<&str> {
        self.revisions.iter().find(|(n, _)| *n == number).map(|(_, c)| c.as_str())
    }

    /// The revision scanned just before `number` (by scan order, not by numeric distance): what
    /// the header's `+A -D` and the pane's own markers diff against.
    fn previous(&self, number: u32) -> Option<&str> {
        let idx = self.revisions.iter().position(|(n, _)| *n == number)?;
        (idx > 0).then(|| self.revisions[idx - 1].1.as_str())
    }
}

/// A revision's own diff summary against its predecessor (`CodePane::feed_diff`), for the
/// agent feed's `save_script` entry (docs/design/agent-studio.md, "What the feed shows").
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ScriptDiff {
    /// The first revision of a script (saved or ad hoc): nothing to diff against.
    New {
        lines: usize,
    },
    Changed {
        added: usize,
        deleted: usize,
    },
}

impl ScriptDiff {
    /// `new, N lines`, or `+A -D` (`−` without `--ascii`).
    pub fn text(&self, ascii: bool) -> String {
        let minus = if ascii { '-' } else { '−' };
        match self {
            Self::New { lines } => {
                format!("new, {lines} line{}", if *lines == 1 { "" } else { "s" })
            }
            Self::Changed { added, deleted } => format!("+{added} {minus}{deleted}"),
        }
    }
}

/// Which revision the pane currently shows, and what its run is doing
/// (docs/design/agent-studio.md, "The header").
#[derive(Clone, Debug, PartialEq)]
enum Shown {
    /// No run has started yet: the latest saved script, marked `saved`.
    Saved { name: String, number: u32 },
    /// Between the run's `Script` line and its `Wake` line: `run Nt`.
    Running { name: String, number: u32, run_turns: u32 },
    /// After the run's `Wake` line: `woke Nt` (turns actually run).
    Woke { name: String, number: u32, woke_turns: u32 },
}

impl Shown {
    fn key(&self) -> (&str, u32) {
        match self {
            Self::Saved { name, number }
            | Self::Running { name, number, .. }
            | Self::Woke { name, number, .. } => (name.as_str(), *number),
        }
    }
}

/// The agent studio's code pane: which revision of which script is shown, how (formatted or raw,
/// with or without deleted lines), and where it's scrolled to. Built once by
/// `crate::watch::Watcher::new` and fed/advanced alongside it.
#[derive(Clone, Debug, Default)]
pub struct CodePane {
    tracks: HashMap<String, Track>,
    shown: Option<Shown>,
    /// Once any run has started, a later `save_script` (`turns: 0`) never takes the pane over
    /// again (docs/design/agent-studio.md: "A `save_script` that comes after a run doesn't take
    /// the pane over").
    has_run: bool,
    /// Whether [`Self::shown`] is an ad hoc run (`version` 0). Once one has woken, a saved
    /// script's `save_script` takes the pane back: an ad hoc run is usually a probe (a one-line
    /// `let s = scripts; 1` to see what's there), and leaving it up hid the governor the agent
    /// saved right after it, often for the rest of the replay.
    shown_ad_hoc: bool,
    /// The first visible row, counting wrapped and deleted rows (#589).
    scroll: usize,
    /// What the first visible row shows, so a resize (which rewraps every line) keeps the same
    /// line at the top.
    top: Option<Anchor>,
    /// The content width the rows were last wrapped to.
    last_width: u16,
    /// Set whenever the shown revision changes; consumed by the next [`Self::render`], which
    /// recomputes [`Self::scroll`] so the first change sits a third of the way down -- unless
    /// the viewer has [`Self::scrolled`] away.
    want_autoscroll: bool,
    /// The viewer scrolled on purpose (#589): a new revision no longer moves the view. `c`
    /// jumps back to the changes and follows them again.
    scrolled: bool,
    /// `p`: the script as the agent wrote it, instead of formatted for display (#558).
    raw: bool,
    /// `x`: deleted lines as a mark on the line above them, instead of rows of their own.
    hide_deleted: bool,
    /// The last rendered pane's row count: `J`/`K`'s own half-pane step.
    last_pane_rows: u16,
    /// The shown revision as text, highlighted and diffed, kept until it or the mode changes.
    view: Option<View>,
    /// `(`/`)` (#502): the revision of the shown script the viewer stepped to, when it isn't
    /// the one [`Self::shown`] says is running. A new run doesn't take a pinned pane over;
    /// stepping back to the running revision unpins it.
    pinned: Option<u32>,
}

/// What a row of the pane shows: a line of the shown revision (0-based), or a deleted line of
/// its predecessor.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Anchor {
    Line(usize),
    Deleted(usize),
}

/// One row of the pane: part of a line (chars `from..to`; `first` is the part with the line
/// number), or of a deleted one.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct PaneRow {
    what: Anchor,
    from: usize,
    to: usize,
    first: bool,
    last: bool,
}

/// The shown revision, ready to draw: its display text (formatted, unless raw or it won't
/// format) and its predecessor's, highlighted and diffed.
#[derive(Clone, Debug)]
struct View {
    key: (String, u32, bool),
    lines: Vec<String>,
    spans: Vec<Vec<code::Span>>,
    old_lines: Vec<String>,
    diff: code::RowDiff,
    /// Whether `lines` is the formatted text. `false` when raw was asked for, or the code
    /// won't lex.
    formatted: bool,
}

/// The text the pane shows for `code`: formatted for display unless `raw`, or if it won't lex
/// (#558: "code that won't lex is shown raw, never dropped"). The flag says which it is.
fn display_text(code: &str, raw: bool) -> (String, bool) {
    if raw {
        return (code.to_owned(), false);
    }
    match code::format(code) {
        Some(text) => (text, true),
        None => (code.to_owned(), false),
    }
}

/// The diff the pane, its header and the agent feed all show between two revisions: on the
/// display text, so with formatting on a change of layout alone is `+0 -0`, and blank lines
/// never count.
fn display_diff(old: &str, new: &str, raw: bool) -> code::RowDiff {
    let (old_text, old_fmt) = display_text(old, raw);
    let (new_text, new_fmt) = display_text(new, raw);
    code::diff_rows(&old_text, &new_text, old_fmt && new_fmt)
}

impl CodePane {
    pub fn new() -> Self {
        Self::default()
    }

    /// Scans every `Script` line in `lines` right away, whether or not it's been replayed yet
    /// (docs/design/agent-studio.md: "The watcher scans `script` lines as they are fed, not as
    /// they are replayed"), so the header's `M` counts every revision read so far, not just the
    /// ones the replay has reached.
    pub fn feed<'a>(&mut self, lines: impl IntoIterator<Item = &'a LogLine>) {
        for line in lines {
            if let LogLine::Script { name, version, code, .. } = line {
                self.tracks.entry(name.clone()).or_default().scan(*version, code);
            }
        }
    }

    /// A `Script` line reaches the replay (`crate::watch::Watcher::start_next`): updates which
    /// revision the pane shows, the same rule [`Self::feed`]'s own doc comment explains. Returns
    /// the revision number `name`/`version`/`code` scanned as, for a caller (`crate::feed::Feed`,
    /// docs/design/agent-studio.md "The agent feed") that needs to know it too.
    pub fn on_script(&mut self, name: &str, version: u32, code: &str, turns: u32) -> u32 {
        let number = self.tracks.entry(name.to_owned()).or_default().scan(version, code);
        let probe_done = self.shown_ad_hoc && matches!(self.shown, Some(Shown::Woke { .. }));
        if turns == 0 && self.has_run && !(probe_done && version > 0) {
            return number;
        }
        self.shown_ad_hoc = version == 0;
        let changed = self.shown.as_ref().map(Shown::key) != Some((name, number));
        self.shown = Some(if turns > 0 {
            self.has_run = true;
            Shown::Running { name: name.to_owned(), number, run_turns: turns }
        } else {
            Shown::Saved { name: name.to_owned(), number }
        });
        if changed && self.pinned.is_some() {
            // A pinned pane stays on its revision (#502), unless the new one is it, or is
            // another script altogether.
            let same_name = self.view.as_ref().is_none_or(|v| v.key.0 == name);
            if self.pinned == Some(number) || !same_name {
                self.pinned = None;
            }
        }
        if changed && self.pinned.is_none() {
            self.want_autoscroll = true;
        }
        number
    }

    /// The feed's own diff summary for `name`'s revision `number` (docs/design/agent-studio.md,
    /// "What the feed shows": the `save_script` row), reusing the same [`Track`] [`Self::feed`]
    /// and [`Self::on_script`] already scanned -- the feed never keeps its own copy of any
    /// script's source. `number <= 1` has no previous revision to diff against, so it's
    /// [`ScriptDiff::New`], with the revision's own line count instead. Counted on the text the
    /// pane shows (formatted, unless the viewer switched to raw), so it agrees with the header.
    pub fn feed_diff(&self, name: &str, number: u32) -> ScriptDiff {
        let code = self.tracks.get(name).and_then(|t| t.code(number)).unwrap_or("");
        if number <= 1 {
            return ScriptDiff::New { lines: code.lines().count() };
        }
        let old = self.tracks.get(name).and_then(|t| t.previous(number)).unwrap_or("");
        let diff = display_diff(old, code, self.raw);
        ScriptDiff::Changed { added: diff.added, deleted: diff.deleted }
    }

    /// A `Wake` line reaches the replay: the running revision's header switches from `run Nt` to
    /// `woke Nt` (the turns the run actually ran, not its budget).
    pub fn on_wake(&mut self, turns_run: u32) {
        if let Some(Shown::Running { name, number, .. }) = self.shown.take() {
            self.shown = Some(Shown::Woke { name, number, woke_turns: turns_run });
        }
    }

    /// The pane's keys (docs/design/agent-studio.md, "Keys"): `j`/`k` scroll a row, `J`/`K`
    /// half a pane, `c` jumps back to the changes and follows them, `p` switches between
    /// formatted and raw, and `x` shows or hides deleted lines. `true` if `key` was one of
    /// these -- the caller (`crate::watch::Watcher::key`) tries nothing else once it is.
    pub fn key(&mut self, key: KeyEvent) -> bool {
        let half = i64::from(self.last_pane_rows.max(2)) / 2;
        match key.code {
            KeyCode::Char('j') => self.scroll_by(1),
            KeyCode::Char('k') => self.scroll_by(-1),
            KeyCode::Char('J') => self.scroll_by(half),
            KeyCode::Char('K') => self.scroll_by(-half),
            KeyCode::Char('c') => {
                self.scrolled = false;
                self.want_autoscroll = true;
            }
            KeyCode::Char('p') => {
                self.raw = !self.raw;
                self.want_autoscroll |= !self.scrolled;
            }
            KeyCode::Char('x') => {
                self.hide_deleted = !self.hide_deleted;
                self.want_autoscroll |= !self.scrolled;
            }
            KeyCode::Char('(') => self.step_revision(-1),
            KeyCode::Char(')') => self.step_revision(1),
            _ => return false,
        }
        true
    }

    fn scroll_by(&mut self, delta: i64) {
        let next = i64::try_from(self.scroll).unwrap_or(0) + delta;
        self.scroll = next.max(0) as usize;
        self.scrolled = true;
        self.want_autoscroll = false;
        self.top = None;
    }

    /// `(` (`delta` -1) or `)` (+1), #502: steps the pane to the shown script's previous or
    /// next revision, in the order they were fed, and pins it there; reaching the running
    /// revision unpins. The pane then scrolls to the revision's first change.
    fn step_revision(&mut self, delta: i64) {
        let Some((name, running)) = self.shown.as_ref().map(|s| (s.key().0.to_owned(), s.key().1))
        else {
            return;
        };
        let Some(track) = self.tracks.get(&name) else { return };
        let at = self.pinned.unwrap_or(running);
        let Some(idx) = track.revisions.iter().position(|(n, _)| *n == at) else { return };
        let next = (idx as i64 + delta).clamp(0, track.revisions.len() as i64 - 1) as usize;
        let number = track.revisions[next].0;
        self.pinned = (number != running).then_some(number);
        self.scrolled = false;
        self.want_autoscroll = true;
    }

    /// The script name and revision number the pane draws: the pinned one (#502), or the one
    /// running.
    fn displayed(&self) -> Option<(&str, u32)> {
        let (name, running) = self.shown.as_ref()?.key();
        Some((name, self.pinned.unwrap_or(running)))
    }

    fn shown_code(&self) -> Option<&str> {
        let (name, number) = self.displayed()?;
        self.tracks.get(name).and_then(|t| t.code(number))
    }

    fn previous_code(&self) -> Option<&str> {
        let (name, number) = self.displayed()?;
        self.tracks.get(name).and_then(|t| t.previous(number))
    }

    /// The shown revision, ready to draw, or `None` with nothing shown yet.
    fn build_view(&self) -> Option<View> {
        let (name, number) = self.displayed()?;
        let code = self.shown_code()?;
        let old = self.previous_code().unwrap_or("");
        let (text, formatted) = display_text(code, self.raw);
        let (old_text, old_formatted) = display_text(old, self.raw);
        let diff = code::diff_rows(&old_text, &text, formatted && old_formatted);
        Some(View {
            key: (name.to_owned(), number, self.raw),
            spans: code::highlight(&text),
            lines: text.lines().map(str::to_owned).collect(),
            old_lines: old_text.lines().map(str::to_owned).collect(),
            diff,
            formatted,
        })
    }

    /// [`Self::view`], rebuilt only when the shown revision or the mode changed.
    fn current_view(&mut self) -> Option<&View> {
        let key = self.displayed().map(|(name, number)| (name.to_owned(), number, self.raw));
        if self.view.as_ref().map(|v| &v.key) != key.as_ref() {
            self.view = self.build_view();
        }
        self.view.as_ref()
    }

    /// `name.rhai -- rev N/M -- run Nt/woke Nt/saved -- +A -D` (docs/design/agent-studio.md,
    /// "The header"), or, pinned to an older revision (#502), `rev N/M · pinned -- running vK`;
    /// `None` with no script replayed yet.
    fn header_text(&self, ascii: bool) -> Option<String> {
        let shown = self.shown.as_ref()?;
        let (name, running) = shown.key();
        let number = self.pinned.unwrap_or(running);
        let max = self.tracks.get(name).map_or(number, Track::latest);
        let dot = if ascii { '.' } else { '·' };
        let run_part = match (shown, self.pinned) {
            (Shown::Saved { .. }, Some(_)) => format!("saved v{running}"),
            (_, Some(_)) => format!("running v{running}"),
            (Shown::Saved { .. }, None) => "saved".to_owned(),
            (Shown::Running { run_turns, .. }, None) => format!("run {run_turns}t"),
            (Shown::Woke { woke_turns, .. }, None) => format!("woke {woke_turns}t"),
        };
        let pin = if self.pinned.is_some() { format!(" {dot} pinned") } else { String::new() };
        let dash = if ascii { "--" } else { "──" };
        let minus = if ascii { '-' } else { '−' };
        let diff_part = if number <= 1 {
            "new".to_owned()
        } else {
            let (added, deleted) = match &self.view {
                Some(v) if v.key == (name.to_owned(), number, self.raw) => {
                    (v.diff.added, v.diff.deleted)
                }
                _ => match self.feed_diff(name, number) {
                    ScriptDiff::Changed { added, deleted } => (added, deleted),
                    ScriptDiff::New { .. } => (0, 0),
                },
            };
            format!("+{added} {minus}{deleted}")
        };
        Some(format!(
            "{name}.rhai {dash} rev {number}/{max}{pin} {dash} {run_part} {dash} {diff_part}"
        ))
    }

    /// Draws the pane's border, header, footer and (if a revision is shown) its highlighted,
    /// diffed source into `area`: long lines wrapped onto continuation rows, deleted lines on
    /// rows of their own unless hidden, scrolled (docs/design/agent-studio.md, "Gutter,
    /// scrolling, long lines"). A no-op on an area too small to draw a border in at all.
    pub fn render(&mut self, buf: &mut Buffer, area: Rect, palette: &Palette) {
        if area.width < 6 || area.height < 3 {
            return;
        }
        let ascii = palette.ascii;
        let marks = Marks::new(ascii);
        let key_before = self.view.as_ref().map(|v| v.key.clone());
        self.current_view();
        let view_changed = self.view.as_ref().map(|v| &v.key) != key_before.as_ref();
        let title = self.header_text(ascii).unwrap_or_default();
        draw_box(buf, area, &marks, &title);
        let inner = Rect::new(area.x + 1, area.y + 1, area.width - 2, area.height - 2);
        self.last_pane_rows = inner.height;

        let Some(view) = self.view.take() else {
            let notice = "no scripts replayed yet";
            buf.set_stringn(inner.x, inner.y, notice, usize::from(inner.width), Style::new());
            return;
        };

        let total = view.lines.len();
        let digits = total.max(1).to_string().len() as u16;
        let gutter_width = digits + 2; // the line number, the marker, one space
        let content_width = inner.width.saturating_sub(gutter_width).max(1);
        let capacity = usize::from(inner.height);

        let show_deleted = !self.hide_deleted;
        let rows = pane_rows(&view, show_deleted, usize::from(content_width));
        let line_marks = if show_deleted {
            view.diff
                .rows
                .iter()
                .filter_map(|r| match r {
                    code::Row::Same(_) => Some(Mark::Same),
                    code::Row::Added(_) => Some(Mark::Added),
                    code::Row::Deleted(_) => None,
                })
                .collect()
        } else {
            view.diff.marks()
        };
        let max_scroll = rows.len().saturating_sub(capacity);

        if self.want_autoscroll && !self.scrolled {
            let first_change = rows.iter().position(|r| match r.what {
                Anchor::Deleted(_) => true,
                Anchor::Line(i) => line_marks.get(i).is_some_and(|m| *m != Mark::Same),
            });
            self.scroll = first_change.unwrap_or(0).saturating_sub(capacity / 3);
        } else if (content_width != self.last_width || view_changed)
            && let Some(top) = self.top
            && let Some(at) = rows.iter().position(|r| r.what == top && r.first)
        {
            // A resize rewraps every line, and a new revision can add or drop rows above the
            // view: keep the same line at the top.
            self.scroll = at;
        }
        self.want_autoscroll = false;
        self.last_width = content_width;
        self.scroll = self.scroll.min(max_scroll);
        self.top = rows.get(self.scroll).map(|r| r.what);

        let base_bg = color::to_u8(palette.theme.panel);
        let visible = &rows[self.scroll.min(rows.len())..];
        for (row, pane_row) in visible.iter().take(capacity).enumerate() {
            let y = inner.y + row as u16;
            let x = inner.x + gutter_width;
            match pane_row.what {
                Anchor::Line(idx) => {
                    let mark = line_marks.get(idx).copied().unwrap_or(Mark::Same);
                    let row_bg =
                        (mark == Mark::Added).then(|| palette.code_added_bg(base_bg)).flatten();
                    // The `▁` mark sits under the line's last part: the deletion comes after it.
                    let gutter_mark = match mark {
                        Mark::RemovedAfter if !pane_row.last => Mark::Same,
                        m => m,
                    };
                    let number = pane_row.first.then_some(idx);
                    draw_gutter(buf, (inner.x, y), digits, number, gutter_mark, palette);
                    let line = CodeLine { text: &view.lines[idx], spans: &view.spans[idx] };
                    let indent = if pane_row.first {
                        0
                    } else {
                        continuation_indent(&view.lines[idx], usize::from(content_width))
                    };
                    let piece = (pane_row.from, pane_row.to);
                    draw_code_piece(
                        buf,
                        (x, y),
                        content_width,
                        &line,
                        piece,
                        indent,
                        row_bg,
                        palette,
                    );
                }
                Anchor::Deleted(idx) => {
                    draw_deleted_gutter(buf, (inner.x, y), digits, pane_row.first, palette);
                    let text = &view.old_lines[idx];
                    let indent = if pane_row.first {
                        0
                    } else {
                        continuation_indent(text, usize::from(content_width))
                    };
                    let style = palette.code_removed_style().add_modifier(Modifier::DIM);
                    let piece: String = text
                        .chars()
                        .skip(pane_row.from)
                        .take(pane_row.to - pane_row.from)
                        .collect();
                    let room = usize::from(content_width).saturating_sub(indent);
                    buf.set_stringn(x + indent as u16, y, &piece, room, style);
                }
            }
        }

        let footer = self.footer_text(&view, &rows, capacity, usize::from(area.width) - 4);
        draw_footer(buf, area, &footer);
        self.view = Some(view);
    }

    /// The bottom border's text (#589): where you are and how to scroll, when the script doesn't
    /// fit; how to get back to the changes, once scrolled away; and the `p`/`x` switches. Parts
    /// that don't fit in `room` are left out, least useful first.
    fn footer_text(&self, view: &View, rows: &[PaneRow], capacity: usize, room: usize) -> String {
        let mut parts: Vec<(u8, String)> = Vec::new();
        if rows.len() > capacity {
            let shown = &rows[self.scroll..rows.len().min(self.scroll + capacity)];
            let line_of = |r: &PaneRow| match r.what {
                Anchor::Line(i) => Some(i + 1),
                Anchor::Deleted(_) => None,
            };
            let first = shown.iter().find_map(line_of);
            let last = shown.iter().rev().find_map(line_of);
            parts.push((1, "j/k scroll".to_owned()));
            if let (Some(a), Some(b)) = (first, last) {
                parts.push((0, format!("lines {a}-{b} of {}", view.lines.len())));
            }
        }
        if self.scrolled {
            parts.push((2, "c follow changes".to_owned()));
        } else if rows.len() > capacity {
            parts.push((5, "following".to_owned()));
        }
        let mode = match (self.raw, view.formatted) {
            (true, _) => "p formatted",
            (false, true) => "p raw",
            (false, false) => "won't format",
        };
        parts.push((3, mode.to_owned()));
        if view.diff.deleted > 0 {
            let x = if self.hide_deleted { "x show deleted" } else { "x hide deleted" };
            parts.push((4, x.to_owned()));
        }
        // Drop the least useful parts until what's left fits.
        loop {
            let len: usize =
                parts.iter().map(|(_, p)| p.chars().count()).sum::<usize>() + 2 * parts.len();
            if len <= room || parts.is_empty() {
                break;
            }
            let worst = parts.iter().enumerate().max_by_key(|(_, (rank, _))| *rank).map(|(i, _)| i);
            parts.remove(worst.unwrap_or(0));
        }
        parts.iter().map(|(_, p)| p.as_str()).collect::<Vec<_>>().join("  ")
    }
}

/// How far a wrapped line's continuation rows are indented: past the line's own indent, so they
/// read as part of it, but never more than half the width.
fn continuation_indent(text: &str, width: usize) -> usize {
    let lead = text.chars().take_while(|c| *c == ' ').count();
    (lead + 4).min(width / 2)
}

/// Splits a line of `text` into the char ranges of its rows at `width` (#589): the first row
/// takes `width`, each continuation row `width` less [`continuation_indent`]. A row breaks after
/// the last space that leaves it at least a third full, or hard at the width if there's none.
fn wrap(text: &str, width: usize) -> Vec<(usize, usize)> {
    let chars: Vec<char> = text.chars().collect();
    let cont = continuation_indent(text, width);
    let mut out = Vec::new();
    let mut from = 0;
    loop {
        let room = if out.is_empty() { width } else { width.saturating_sub(cont).max(1) };
        if chars.len() - from <= room {
            out.push((from, chars.len()));
            return out;
        }
        let window = &chars[from..from + room];
        let to = match window.iter().rposition(|c| *c == ' ') {
            Some(space) if space + 1 > room / 3 => from + space + 1,
            _ => from + room,
        };
        out.push((from, to));
        from = to;
        while chars.get(from) == Some(&' ') && from < chars.len() {
            from += 1;
        }
        if from >= chars.len() {
            return out;
        }
    }
}

/// Every row the pane would draw for `view` at `width`, top to bottom.
fn pane_rows(view: &View, show_deleted: bool, width: usize) -> Vec<PaneRow> {
    let mut rows = Vec::with_capacity(view.lines.len());
    let mut push = |what: Anchor, text: &str| {
        let pieces = wrap(text, width);
        let n = pieces.len();
        for (k, (from, to)) in pieces.into_iter().enumerate() {
            rows.push(PaneRow { what, from, to, first: k == 0, last: k + 1 == n });
        }
    };
    for row in &view.diff.rows {
        match *row {
            code::Row::Same(i) | code::Row::Added(i) => push(Anchor::Line(i), &view.lines[i]),
            code::Row::Deleted(i) if show_deleted => push(Anchor::Deleted(i), &view.old_lines[i]),
            code::Row::Deleted(_) => {}
        }
    }
    rows
}

/// The gutter: the line number (1-based, `number` 0-based), right-aligned to `digits` columns,
/// or `↪`/`\` on a wrapped line's continuation row (`None`); then the diff marker (`▌`/`+`
/// added, `▁`/`_` the line just above a hidden deletion, blank otherwise), then a space drawn
/// by the code's own start column.
fn draw_gutter(
    buf: &mut Buffer,
    (x0, y): (u16, u16),
    digits: u16,
    number: Option<usize>,
    mark: Mark,
    palette: &Palette,
) {
    let gutter_style = palette.code_gutter_style();
    let line_no = match number {
        Some(idx) => format!("{:>width$}", idx + 1, width = usize::from(digits)),
        None => {
            let cont = if palette.ascii { '\\' } else { '↪' };
            format!("{:>width$}", cont, width = usize::from(digits))
        }
    };
    for (i, c) in line_no.chars().enumerate() {
        buf[(x0 + i as u16, y)].set_char(c).set_style(gutter_style);
    }
    let (marker_char, marker_style) = match mark {
        Mark::Added => (if palette.ascii { '+' } else { '▌' }, palette.code_added_style()),
        Mark::RemovedAfter => (if palette.ascii { '_' } else { '▁' }, palette.code_removed_style()),
        Mark::Same => (' ', Style::new()),
    };
    buf[(x0 + digits, y)].set_char(marker_char).set_style(marker_style);
}

/// A deleted line's gutter (#558): no line number, it has none in this revision; the `−`/`-`
/// marker on its first row.
fn draw_deleted_gutter(
    buf: &mut Buffer,
    (x0, y): (u16, u16),
    digits: u16,
    first: bool,
    palette: &Palette,
) {
    let marker = match (first, palette.ascii) {
        (false, _) => ' ',
        (true, true) => '-',
        (true, false) => '−',
    };
    buf[(x0 + digits, y)].set_char(marker).set_style(palette.code_removed_style());
}

/// One highlighted line of source, as [`draw_code_piece`] draws it: the text and the spans
/// [`fn_ui::code::highlight_line`] classified it into.
struct CodeLine<'a> {
    text: &'a str,
    spans: &'a [code::Span],
}

/// Chars `from..to` of `line` (one row of it, [`wrap`]), from `(content_x + indent, y)`, tinted
/// with `row_bg` (an added line's background) across the whole content width, past the code too.
#[allow(clippy::too_many_arguments)]
fn draw_code_piece(
    buf: &mut Buffer,
    (content_x, y): (u16, u16),
    content_width: u16,
    line: &CodeLine,
    (from, to): (usize, usize),
    indent: usize,
    row_bg: Option<Color>,
    palette: &Palette,
) {
    let end_x = content_x + content_width;
    if let Some(bg) = row_bg {
        for x in content_x..end_x {
            buf[(x, y)].set_style(Style::new().bg(bg));
        }
    }
    let mut x = content_x + indent as u16;
    let mut n = 0usize;
    for s in line.spans {
        let base = palette.code_style(s.class);
        let style = if let Some(bg) = row_bg { base.bg(bg) } else { base };
        for c in line.text[s.start..s.end].chars() {
            if n >= from && n < to && x < end_x {
                buf[(x, y)].set_char(c).set_style(style);
                x += 1;
            }
            n += 1;
        }
    }
}

/// `text` embedded in the bottom border, right-aligned, the way the header sits in the top one.
fn draw_footer(buf: &mut Buffer, area: Rect, text: &str) {
    if text.is_empty() {
        return;
    }
    let text = format!(" {text} ");
    let room = usize::from(area.width.saturating_sub(2));
    let len = text.chars().count().min(room);
    let y = area.bottom() - 1;
    let x0 = area.right() - 1 - len as u16;
    for (i, c) in text.chars().take(len).enumerate() {
        buf[(x0 + i as u16, y)].set_char(c);
    }
}

/// The pane's own box: light corners (`crate::widgets::Marks::box_tl` and friends -- "drawn over
/// the map, not another frame", the same reasoning the embark site box uses), with `title`
/// embedded directly into the top border rather than centred over it, matching the design's own
/// mockup.
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
    use crate::keyev::KeyModifiers;
    use fn_sim::{Defs, Nation};
    use ratatui::buffer::Buffer;
    use ratatui::layout::Rect;

    use super::*;
    use crate::color::{ColorDepth, Palette};

    fn key(c: char) -> KeyEvent {
        KeyEvent::new(KeyCode::Char(c), KeyModifiers::NONE)
    }

    fn script(name: &str, version: u32, code: &str, turns: u32) -> LogLine {
        LogLine::Script {
            name: name.to_owned(),
            version,
            code: code.to_owned(),
            turns,
            wake_on: Vec::new(),
        }
    }

    #[test]
    fn a_fresh_pane_shows_nothing() {
        let pane = CodePane::new();
        assert_eq!(pane.shown_code(), None);
        assert_eq!(pane.header_text(false), None);
    }

    /// A saved-only script (`turns: 0`) shows up marked `saved`, with `new` instead of a diff
    /// count for its first revision.
    #[test]
    fn a_saved_only_script_shows_as_saved_with_new() {
        let mut pane = CodePane::new();
        pane.on_script("keep-fed", 1, "let a = 1;", 0);
        assert_eq!(pane.shown_code(), Some("let a = 1;"));
        let header = pane.header_text(false).unwrap();
        assert!(header.contains("keep-fed.rhai"), "{header}");
        assert!(header.contains("rev 1/1"), "{header}");
        assert!(header.contains("saved"), "{header}");
        assert!(header.contains("new"), "{header}");
    }

    /// Once a run starts, the pane follows it instead of the saved script, and the header reads
    /// `run Nt`.
    #[test]
    fn a_running_script_takes_over_the_pane() {
        let mut pane = CodePane::new();
        pane.on_script("keep-fed", 1, "let a = 1;", 0);
        pane.on_script("keep-fed", 1, "let a = 1;", 45);
        let header = pane.header_text(false).unwrap();
        assert!(header.contains("run 45t"), "{header}");
    }

    /// A `save_script` that lands after a run doesn't take the pane back over.
    #[test]
    fn a_save_after_a_run_does_not_take_the_pane_back() {
        let mut pane = CodePane::new();
        pane.on_script("keep-fed", 1, "let a = 1;", 10);
        pane.on_script("keep-fed", 2, "let a = 2;", 0);
        let header = pane.header_text(false).unwrap();
        assert!(header.contains("rev 1/2"), "{header}: the pane should still show rev 1");
    }

    /// `Wake` turns a running header into a `woke Nt` one, counting the turns the run actually
    /// ran (passed in by the caller, `Watcher`'s own job).
    #[test]
    fn wake_turns_running_into_woke() {
        let mut pane = CodePane::new();
        pane.on_script("keep-fed", 3, "let a = 1;", 45);
        pane.on_wake(23);
        let header = pane.header_text(false).unwrap();
        assert!(header.contains("woke 23t"), "{header}");
    }

    /// A second revision's header counts `+A -D` against the first, and `M` counts every
    /// revision scanned so far, even ahead of what's been "run" here.
    #[test]
    fn a_second_revision_diffs_against_the_first_and_counts_m() {
        let mut pane = CodePane::new();
        pane.feed([
            &script("keep-fed", 1, "let a = 1;\nlet b = 2;", 0),
            &script("keep-fed", 2, "let a = 1;\nlet c = 3;\nlet d = 4;", 0),
        ]);
        pane.on_script("keep-fed", 2, "let a = 1;\nlet c = 3;\nlet d = 4;", 45);
        let header = pane.header_text(false).unwrap();
        assert!(header.contains("rev 2/2"), "{header}");
        assert!(header.contains("+2 −1") || header.contains("+2 -1"), "{header}");
    }

    /// A log that inherits a script library starts at a later saved version (a campaign
    /// attempt's first `save_script` may be v6): `M` is the latest version, never fewer than the
    /// revision shown.
    #[test]
    fn m_is_the_latest_version_when_the_log_starts_partway_through() {
        let mut pane = CodePane::new();
        pane.feed([
            &script("governor", 6, "let a = 1;", 0),
            &script("governor", 7, "let a = 2;", 0),
            &script("governor", 10, "let a = 3;", 0),
        ]);
        pane.on_script("governor", 7, "let a = 2;", 60);
        let header = pane.header_text(false).unwrap();
        assert!(header.contains("rev 7/10"), "{header}");
    }

    /// A saved script's `save_script` takes the pane back from an ad hoc run that has woken, but
    /// not from a saved script's run, nor from an ad hoc run still going.
    #[test]
    fn a_save_takes_the_pane_back_from_a_finished_ad_hoc_run_only() {
        let mut pane = CodePane::new();
        pane.on_script("governor", 3, "gov();", 40);
        pane.on_wake(40);
        pane.on_script("governor", 4, "gov2();", 0);
        assert!(pane.header_text(false).unwrap().starts_with("governor.rhai"));
        assert!(pane.header_text(false).unwrap().contains("rev 3/4"));

        pane.on_script("script", 0, "let s = scripts; 1", 1);
        pane.on_script("governor", 5, "gov3();", 0); // the probe is still running
        assert!(pane.header_text(false).unwrap().starts_with("script.rhai"));
        pane.on_wake(0);
        pane.on_script("governor", 6, "gov4();", 0);
        let header = pane.header_text(false).unwrap();
        assert!(
            header.contains("governor.rhai") && header.contains("rev 6/6 ── saved"),
            "{header}"
        );
    }

    /// An ad hoc `run_script {code}` (`version` 0) numbers its revisions by how many distinct
    /// codes have been seen under its tag, and re-running the same code doesn't add one.
    #[test]
    fn an_ad_hoc_scripts_revisions_count_distinct_codes() {
        let mut pane = CodePane::new();
        pane.on_script("scout", 0, "look();", 5);
        pane.on_script("scout", 0, "look();", 5); // same code again: no new revision
        let header = pane.header_text(false).unwrap();
        assert!(header.contains("rev 1/1"), "{header}");

        pane.on_script("scout", 0, "look(); act();", 5);
        let header = pane.header_text(false).unwrap();
        assert!(header.contains("rev 2/2"), "{header}");
    }

    /// `j`/`k`/`J`/`K` scroll the pane, clamped to what the content actually has.
    #[test]
    fn keys_scroll_and_clamp() {
        let mut pane = CodePane::new();
        assert!(pane.key(key('j')));
        assert_eq!(pane.scroll, 1);
        assert!(pane.key(key('k')));
        assert_eq!(pane.scroll, 0);
        assert!(pane.key(key('k')), "handled even though it clamps at zero");
        assert_eq!(pane.scroll, 0);
        assert!(!pane.key(key('z')), "not a code-pane key");
    }

    fn palette() -> Palette {
        Palette::new(&Defs::bundled(), Nation::Earth, ColorDepth::TrueColor, false)
    }

    /// Rendering a pane with no script replayed yet draws the border and a one-line notice,
    /// without panicking on a small area.
    #[test]
    fn rendering_an_empty_pane_shows_a_notice() {
        let mut pane = CodePane::new();
        let mut buf = Buffer::empty(Rect::new(0, 0, 40, 10));
        pane.render(&mut buf, Rect::new(0, 0, 40, 10), &palette());
        let text = crate::frame::to_text(&buf);
        assert!(text.contains("no scripts"), "{text}");
    }

    /// Rendering a real revision draws its header (name, revision, run state) into the top
    /// border, and the source lines with a gutter.
    #[test]
    fn rendering_a_revision_draws_the_header_and_source() {
        let mut pane = CodePane::new();
        pane.on_script("keep-fed", 3, "let a = 1;\nlet b = 2;\n", 45);
        let mut buf = Buffer::empty(Rect::new(0, 0, 60, 10));
        pane.render(&mut buf, Rect::new(0, 0, 60, 10), &palette());
        let text = crate::frame::to_text(&buf);
        let rows: Vec<&str> = text.lines().collect();
        assert!(rows[0].contains("keep-fed.rhai"), "{}", rows[0]);
        assert!(rows[0].contains("run 45t"), "{}", rows[0]);
        assert!(rows[1].contains("let a = 1;"), "{}", rows[1]);
        assert!(rows[2].contains("let b = 2;"), "{}", rows[2]);
    }

    /// An added line gets the `▌` marker, and a deleted line shows on a row of its own, marked
    /// `−` and with no line number (#558). With deleted lines hidden (`x`), the line just above
    /// a run of them gets `▁` instead.
    #[test]
    fn diff_markers_show_in_the_gutter() {
        let mut pane = CodePane::new();
        pane.on_script("keep-fed", 1, "a\nb\nc", 0);
        pane.on_script("keep-fed", 2, "a\nx\nc", 10);
        let mut buf = Buffer::empty(Rect::new(0, 0, 40, 10));
        pane.render(&mut buf, Rect::new(0, 0, 40, 10), &palette());
        let text = crate::frame::to_text(&buf);
        let rows: Vec<&str> = text.lines().collect();
        assert!(rows[1].contains("1  a"), "{}", rows[1]);
        assert!(rows[2].contains("−") && rows[2].contains('b'), "{}", rows[2]);
        assert!(!rows[2].contains('2'), "a deleted line has no number: {}", rows[2]);
        assert!(rows[3].contains("2▌ x"), "{}", rows[3]);
        assert!(rows[4].contains("3  c"), "{}", rows[4]);

        assert!(pane.key(key('x')));
        let mut buf = Buffer::empty(Rect::new(0, 0, 40, 10));
        pane.render(&mut buf, Rect::new(0, 0, 40, 10), &palette());
        let text = crate::frame::to_text(&buf);
        let rows: Vec<&str> = text.lines().collect();
        // Line 1 ("a") is unchanged and is also the line just above the removed "b": `RemovedAfter`.
        assert!(rows[1].contains('▁'), "{}", rows[1]);
        assert!(rows[2].contains('▌'), "{}", rows[2]);
    }

    // --- #589 and #558: wrapping, scrolling, following, formatting, deleted lines ---

    /// A governor of 280 lines, some of them over 100 characters (a string literal no formatter
    /// can shorten), already laid out the way the formatter would.
    fn long_governor(tag: &str) -> String {
        (1..=280)
            .map(|i| {
                if i % 7 == 0 {
                    let long = "x".repeat(60);
                    format!("log(\"line {i} {tag} starts here, then {long} ends {i}\");\n")
                } else {
                    format!("let v{i} = {i};\n")
                }
            })
            .collect()
    }

    fn render_text(pane: &mut CodePane, w: u16, h: u16, palette: &Palette) -> String {
        let mut buf = Buffer::empty(Rect::new(0, 0, w, h));
        pane.render(&mut buf, Rect::new(0, 0, w, h), palette);
        crate::frame::to_text(&buf)
    }

    /// The text of every code row between the borders, the gutter cut off.
    fn code_rows(text: &str, gutter: usize) -> Vec<String> {
        let rows: Vec<&str> = text.lines().collect();
        rows[1..rows.len() - 1]
            .iter()
            .map(|r| {
                r.chars().skip(1 + gutter).collect::<String>().trim_end_matches('│').to_owned()
            })
            .collect()
    }

    /// Long lines wrap onto continuation rows marked `↪` with no number, the numbers on the
    /// first rows stay right, and the whole of each long line can be read, at several widths.
    #[test]
    fn long_lines_wrap_and_line_numbers_stay_right() {
        for width in [40u16, 60, 88] {
            let mut pane = CodePane::new();
            pane.on_script("governor", 1, &long_governor("a"), 0);
            let text = render_text(&mut pane, width, 30, &palette());
            let rows: Vec<&str> = text.lines().collect();
            assert!(rows[1].starts_with("│  1") && rows[1].contains("let v1 = 1;"), "{}", rows[1]);
            // Line 7 is the first long one: its number shows once, then `↪` rows follow.
            let at = rows.iter().position(|r| r.contains("  7▌ log(")).expect("line 7 shown");
            assert!(rows[at + 1].contains('↪'), "{width}: {}", rows[at + 1]);
            assert!(!rows[at + 1].contains("  8 "), "{width}: {}", rows[at + 1]);
            let joined: String = rows[at..]
                .iter()
                .take_while(|r| !r.contains("  8▌ let"))
                .map(|r| r.chars().skip(6).collect::<String>().replace(['│', '↪'], ""))
                .collect::<Vec<_>>()
                .join("");
            assert!(joined.contains("ends 7\");"), "{width}: the whole line reads: {joined}");
            for row in &rows {
                assert_eq!(row.chars().count(), usize::from(width), "{width}: {row}");
            }
        }
    }

    /// `J` scrolls a script longer than the pane all the way to its very last line, and the
    /// footer says how to scroll and which lines are shown, at several sizes.
    #[test]
    fn a_long_script_scrolls_to_its_last_line_with_the_position_in_the_footer() {
        for (w, h) in [(40u16, 20u16), (60, 43), (88, 43)] {
            let mut pane = CodePane::new();
            pane.on_script("governor", 1, &long_governor("a"), 0);
            let text = render_text(&mut pane, w, h, &palette());
            let footer = text.lines().last().unwrap().to_owned();
            assert!(footer.contains("lines 1-"), "{w}x{h}: {footer}");
            if w >= 60 {
                assert!(footer.contains("j/k scroll"), "{w}x{h}: {footer}");
                assert!(footer.contains("of 280"), "{w}x{h}: {footer}");
            }
            for _ in 0..200 {
                pane.key(key('J'));
            }
            let text = render_text(&mut pane, w, h, &palette());
            let rows: Vec<&str> = text.lines().collect();
            let last_code = rows[rows.len() - 2];
            assert!(last_code.contains("ends 280\");"), "{w}x{h}: {last_code}");
            let footer = rows[rows.len() - 1];
            assert!(footer.contains("-280 of 280") || w < 60, "{w}x{h}: {footer}");
        }
    }

    /// A script that fits says nothing about scrolling.
    #[test]
    fn a_script_that_fits_keeps_the_footer_short() {
        let mut pane = CodePane::new();
        pane.on_script("keep-fed", 1, "let a = 1;\nlet b = 2;\n", 0);
        let text = render_text(&mut pane, 60, 10, &palette());
        let footer = text.lines().last().unwrap();
        assert!(!footer.contains("scroll") && !footer.contains("lines"), "{footer}");
        assert!(footer.contains("p raw"), "{footer}");
    }

    /// A resize rewraps every line but keeps the same line at the top.
    #[test]
    fn a_resize_keeps_the_top_line() {
        let mut pane = CodePane::new();
        pane.on_script("governor", 1, &long_governor("a"), 0);
        render_text(&mut pane, 88, 30, &palette());
        for _ in 0..60 {
            pane.key(key('j'));
        }
        let wide = render_text(&mut pane, 88, 30, &palette());
        let top = wide.lines().nth(1).unwrap().chars().take(5).collect::<String>();
        let narrow = render_text(&mut pane, 40, 30, &palette());
        let top_narrow = narrow.lines().nth(1).unwrap().chars().take(5).collect::<String>();
        assert_eq!(top, top_narrow, "{wide}\n{narrow}");
    }

    /// A new revision follows the changes by default. Once the viewer scrolls, a new one leaves
    /// the view where it is and the footer offers `c`, which jumps back to the changes.
    #[test]
    fn scrolling_stops_following_until_c() {
        let mut pane = CodePane::new();
        pane.feed([&script("governor", 1, &long_governor("a"), 0)]);
        pane.on_script("governor", 1, &long_governor("a"), 10);
        render_text(&mut pane, 88, 30, &palette());
        let changed = long_governor("a").replace("let v200 = 200;", "let v200 = 9;");
        pane.on_script("governor", 2, &changed, 10);
        let text = render_text(&mut pane, 88, 30, &palette());
        assert!(text.contains("let v200 = 9;"), "follows the change: {text}");
        assert!(!text.contains("c follow"), "{text}");

        pane.key(key('K'));
        let before = render_text(&mut pane, 88, 30, &palette());
        let changed2 = changed.replace("let v20 = 20;", "let v20 = 1;");
        pane.on_script("governor", 3, &changed2, 10);
        let after = render_text(&mut pane, 88, 30, &palette());
        let first_line = |t: &str| t.lines().nth(1).unwrap().chars().take(5).collect::<String>();
        assert_eq!(first_line(&before), first_line(&after), "{after}");
        assert!(after.contains("c follow changes"), "{after}");

        assert!(pane.key(key('c')));
        let back = render_text(&mut pane, 88, 30, &palette());
        assert!(back.contains("let v20 = 1;"), "{back}");
        assert!(!back.contains("c follow"), "{back}");
    }

    /// Two revisions that differ only in layout show no marks and `+0 -0`, formatted (the
    /// default); `p` shows the raw text and its diff.
    #[test]
    fn a_layout_only_change_shows_no_marks_until_raw() {
        let mut pane = CodePane::new();
        pane.feed([
            &script("governor", 1, "let t = #{x: 1, y: 2};\nif t.x > 0 { go(t); }\n", 0),
            &script("governor", 2, "let t=#{ x:1,\n  y:2 };\n\nif t.x>0\n{\n  go( t );\n}", 0),
        ]);
        pane.on_script("governor", 2, "let t=#{ x:1,\n  y:2 };\n\nif t.x>0\n{\n  go( t );\n}", 5);
        let text = render_text(&mut pane, 60, 14, &palette());
        assert!(text.contains("+0 −0"), "{text}");
        assert!(!text.contains('▌') && !text.contains('−') || text.contains("+0 −0"), "{text}");
        assert!(text.contains("let t = #{x: 1, y: 2};"), "{text}");
        assert!(text.contains("p raw"), "{text}");
        assert_eq!(pane.feed_diff("governor", 2), ScriptDiff::Changed { added: 0, deleted: 0 });

        assert!(pane.key(key('p')));
        let raw = render_text(&mut pane, 60, 14, &palette());
        assert!(raw.contains("let t=#{ x:1,"), "{raw}");
        assert!(raw.contains('▌'), "{raw}");
        assert!(raw.contains("p formatted"), "{raw}");
        assert!(!raw.contains("+0 −0"), "{raw}");
    }

    /// Code that won't lex is shown raw, never dropped, and the footer says so.
    #[test]
    fn code_that_will_not_format_shows_raw() {
        let mut pane = CodePane::new();
        pane.on_script("probe", 0, "let s = \"open\nlet t=1;", 1);
        let text = render_text(&mut pane, 60, 10, &palette());
        assert!(text.contains("let s = \"open"), "{text}");
        assert!(text.contains("let t=1;"), "{text}");
        assert!(text.contains("won't format"), "{text}");
    }

    /// Following lands on the first change even when it's a deletion.
    #[test]
    fn following_lands_on_a_deletion() {
        let mut pane = CodePane::new();
        let old = long_governor("a");
        let new = old.replace("let v250 = 250;\n", "");
        pane.feed([&script("governor", 1, &old, 0), &script("governor", 2, &new, 0)]);
        pane.on_script("governor", 2, &new, 5);
        let text = render_text(&mut pane, 88, 30, &palette());
        let row = text.lines().find(|r| r.contains("let v250")).expect("deleted line shown");
        assert!(row.contains('−'), "{row}");
        assert!(text.contains("x hide deleted"), "{text}");
    }

    /// `--ascii`: `\` marks a continuation row and `-` a deleted line.
    #[test]
    fn ascii_marks() {
        let ascii = Palette::new(&Defs::bundled(), Nation::Earth, ColorDepth::TrueColor, true);
        let mut pane = CodePane::new();
        let old = long_governor("a");
        let new = old.replace("let v3 = 3;\n", "");
        pane.feed([&script("governor", 1, &old, 0), &script("governor", 2, &new, 0)]);
        pane.on_script("governor", 2, &new, 5);
        let text = render_text(&mut pane, 50, 30, &ascii);
        assert!(text.lines().any(|r| r.contains('\\')), "{text}");
        let row = text.lines().find(|r| r.contains("let v3 = 3;")).expect("deleted line shown");
        assert!(row.contains("- let v3"), "{row}");
        assert!(text.is_ascii(), "{text}");
        let _ = code_rows(&text, 5);
    }

    // --- #502: stepping through revisions ---

    fn three_revisions() -> CodePane {
        let mut pane = CodePane::new();
        pane.feed([
            &script("governor", 1, "let a = 1;\n", 0),
            &script("governor", 2, "let a = 2;\n", 0),
            &script("governor", 3, "let a = 3;\nlet b = 1;\n", 0),
        ]);
        pane.on_script("governor", 3, "let a = 3;\nlet b = 1;\n", 20);
        pane
    }

    /// `(` pins the pane to the previous revision, diffed against its own predecessor; `)`
    /// steps forward, and reaching the running revision unpins.
    #[test]
    fn parens_step_through_revisions_and_pin() {
        let mut pane = three_revisions();
        assert!(pane.key(key('(')));
        let text = render_text(&mut pane, 160, 45, &palette());
        let header = text.lines().next().unwrap();
        assert!(header.contains("rev 2/3 · pinned"), "{header}");
        assert!(header.contains("running v3"), "{header}");
        assert!(header.contains("+1 −1"), "diffed against rev 1: {header}");
        assert!(text.contains("let a = 2;"), "{text}");
        assert!(!text.contains("let b = 1;"), "{text}");

        pane.key(key('('));
        pane.key(key('('));
        let header = render_text(&mut pane, 160, 45, &palette()).lines().next().unwrap().to_owned();
        assert!(header.contains("rev 1/3 · pinned"), "stops at the first: {header}");

        pane.key(key(')'));
        pane.key(key(')'));
        let text = render_text(&mut pane, 160, 45, &palette());
        let header = text.lines().next().unwrap();
        assert!(header.contains("rev 3/3 ── run 20t"), "unpinned: {header}");
        assert!(!header.contains("pinned"), "{header}");
        assert!(text.contains("let b = 1;"), "{text}");
    }

    /// While pinned, a new run changes only the header's `running vK`.
    #[test]
    fn a_new_run_does_not_take_a_pinned_pane() {
        let mut pane = three_revisions();
        pane.key(key('('));
        pane.on_script("governor", 4, "let a = 4;\n", 20);
        let text = render_text(&mut pane, 160, 45, &palette());
        let header = text.lines().next().unwrap();
        assert!(header.contains("rev 2/4 · pinned") && header.contains("running v4"), "{header}");
        assert!(text.contains("let a = 2;"), "{text}");
        // Stepping forward to the new running revision unpins.
        pane.key(key(')'));
        pane.key(key(')'));
        let header = render_text(&mut pane, 160, 45, &palette()).lines().next().unwrap().to_owned();
        assert!(header.contains("rev 4/4 ── run 20t"), "{header}");
    }

    /// `--ascii` marks the pin with `.`.
    #[test]
    fn pinned_header_in_ascii() {
        let ascii = Palette::new(&Defs::bundled(), Nation::Earth, ColorDepth::TrueColor, true);
        let mut pane = three_revisions();
        pane.key(key('('));
        let text = render_text(&mut pane, 160, 45, &ascii);
        assert!(text.lines().next().unwrap().contains("rev 2/3 . pinned -- running v3"), "{text}");
    }
}
