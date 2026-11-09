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
use ratatui::style::{Color, Style};

use crate::color::{self, Palette};
use crate::surface::LogLine;
use crate::widgets::Marks;

/// The blank columns between the two columns of a split code pane (`crate::modal::GAP`'s own
/// value: the rest of the TUI's "newspaper" and list/detail splits use the same gap).
const COLUMN_GAP: u16 = 2;

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

    /// `M`: how many distinct revisions have been scanned so far for this name.
    fn count(&self) -> u32 {
        self.revisions.len() as u32
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

/// The agent studio's code pane: which revision of which script is shown, its own scroll
/// position, and (in the next issue) which key it's pinned to. Built once by
/// `crate::watch::Watcher::new` and fed/advanced alongside it.
#[derive(Clone, Debug, Default)]
pub struct CodePane {
    tracks: HashMap<String, Track>,
    shown: Option<Shown>,
    /// Once any run has started, a later `save_script` (`turns: 0`) never takes the pane over
    /// again (docs/design/agent-studio.md: "A `save_script` that comes after a run doesn't take
    /// the pane over").
    has_run: bool,
    /// The first visible source line, in the shown revision's own line order (before it's split
    /// into one or two columns).
    scroll: usize,
    /// Set whenever the shown revision changes; consumed by the next [`Self::render`], which
    /// recomputes [`Self::scroll`] so the first marked line sits a third of the way down.
    want_autoscroll: bool,
    /// The last rendered pane's row count (one column's height): `J`/`K`'s own half-pane step.
    last_pane_rows: u16,
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
        if turns == 0 && self.has_run {
            return number;
        }
        let changed = self.shown.as_ref().map(Shown::key) != Some((name, number));
        self.shown = Some(if turns > 0 {
            self.has_run = true;
            Shown::Running { name: name.to_owned(), number, run_turns: turns }
        } else {
            Shown::Saved { name: name.to_owned(), number }
        });
        if changed {
            self.want_autoscroll = true;
        }
        number
    }

    /// The feed's own diff summary for `name`'s revision `number` (docs/design/agent-studio.md,
    /// "What the feed shows": the `save_script` row), reusing the same [`Track`] [`Self::feed`]
    /// and [`Self::on_script`] already scanned -- the feed never keeps its own copy of any
    /// script's source. `number <= 1` has no previous revision to diff against, so it's
    /// [`ScriptDiff::New`], with the revision's own line count instead.
    pub fn feed_diff(&self, name: &str, number: u32) -> ScriptDiff {
        let code = self.tracks.get(name).and_then(|t| t.code(number)).unwrap_or("");
        if number <= 1 {
            return ScriptDiff::New { lines: code.lines().count() };
        }
        let old = self.tracks.get(name).and_then(|t| t.previous(number)).unwrap_or("");
        let (_, added, deleted) = code::line_diff(old, code);
        ScriptDiff::Changed { added, deleted }
    }

    /// A `Wake` line reaches the replay: the running revision's header switches from `run Nt` to
    /// `woke Nt` (the turns the run actually ran, not its budget).
    pub fn on_wake(&mut self, turns_run: u32) {
        if let Some(Shown::Running { name, number, .. }) = self.shown.take() {
            self.shown = Some(Shown::Woke { name, number, woke_turns: turns_run });
        }
    }

    /// `j`/`k`/`J`/`K`: scrolls the pane a line, or half a pane (docs/design/agent-studio.md,
    /// "Gutter, scrolling, long lines, two columns"). `true` if `key` was one of these -- the
    /// caller (`crate::watch::Watcher::key`) tries nothing else once it is.
    pub fn key(&mut self, key: KeyEvent) -> bool {
        let half = i64::from(self.last_pane_rows.max(2)) / 2;
        match key.code {
            KeyCode::Char('j') => self.scroll_by(1),
            KeyCode::Char('k') => self.scroll_by(-1),
            KeyCode::Char('J') => self.scroll_by(half),
            KeyCode::Char('K') => self.scroll_by(-half),
            _ => return false,
        }
        true
    }

    fn scroll_by(&mut self, delta: i64) {
        let next = i64::try_from(self.scroll).unwrap_or(0) + delta;
        self.scroll = next.max(0) as usize;
    }

    fn shown_code(&self) -> Option<&str> {
        let (name, number) = self.shown.as_ref()?.key();
        self.tracks.get(name).and_then(|t| t.code(number))
    }

    fn previous_code(&self) -> Option<&str> {
        let (name, number) = self.shown.as_ref()?.key();
        self.tracks.get(name).and_then(|t| t.previous(number))
    }

    /// `name.rhai -- rev N/M -- run Nt/woke Nt/saved -- +A -D` (docs/design/agent-studio.md,
    /// "The header"); `None` with no script replayed yet.
    fn header_text(&self, ascii: bool) -> Option<String> {
        let shown = self.shown.as_ref()?;
        let (name, number) = shown.key();
        let max = self.tracks.get(name).map_or(number, Track::count);
        let run_part = match shown {
            Shown::Saved { .. } => "saved".to_owned(),
            Shown::Running { run_turns, .. } => format!("run {run_turns}t"),
            Shown::Woke { woke_turns, .. } => format!("woke {woke_turns}t"),
        };
        let dash = if ascii { "--" } else { "──" };
        let minus = if ascii { '-' } else { '−' };
        let diff_part = if number <= 1 {
            "new".to_owned()
        } else {
            let old = self.tracks.get(name).and_then(|t| t.previous(number)).unwrap_or("");
            let code = self.tracks.get(name).and_then(|t| t.code(number)).unwrap_or("");
            let (_, added, deleted) = code::line_diff(old, code);
            format!("+{added} {minus}{deleted}")
        };
        Some(format!("{name}.rhai {dash} rev {number}/{max} {dash} {run_part} {dash} {diff_part}"))
    }

    /// Draws the pane's border, header and (if a revision is shown) its highlighted, diffed
    /// source into `area`, scrolled and split into one or two columns
    /// (docs/design/agent-studio.md, "Gutter, scrolling, long lines, two columns"). A no-op on an
    /// area too small to draw a border in at all.
    pub fn render(&mut self, buf: &mut Buffer, area: Rect, palette: &Palette) {
        if area.width < 6 || area.height < 3 {
            return;
        }
        let ascii = palette.ascii;
        let marks = Marks::new(ascii);
        let title = self.header_text(ascii).unwrap_or_default();
        draw_box(buf, area, &marks, &title);
        let inner = Rect::new(area.x + 1, area.y + 1, area.width - 2, area.height - 2);

        let Some(code) = self.shown_code() else {
            let notice = "no scripts replayed yet";
            buf.set_stringn(inner.x, inner.y, notice, usize::from(inner.width), Style::new());
            self.last_pane_rows = inner.height;
            return;
        };
        let old = self.previous_code().unwrap_or("").to_owned();
        let code = code.to_owned();

        let lines: Vec<&str> = code.lines().collect();
        let total = lines.len();
        let highlighted = code::highlight(&code);
        let (marks_diff, _, _) = code::line_diff(&old, &code);

        let digits = total.max(1).to_string().len() as u16;
        let gutter_width = digits + 2; // the line number, the marker, one space

        let two_column_min = palette.glyphs.studio.two_column_min;
        let (cols, content_width) =
            pick_columns(total, inner.width, gutter_width, inner.height, two_column_min);
        self.last_pane_rows = inner.height;
        let capacity = usize::from(inner.height) * cols;
        let max_scroll = total.saturating_sub(capacity);

        if self.want_autoscroll {
            let first_marked = marks_diff.iter().position(|m| *m != Mark::Same).unwrap_or(0);
            self.scroll = first_marked.saturating_sub(capacity / 3);
            self.want_autoscroll = false;
        }
        self.scroll = self.scroll.min(max_scroll);

        let base_bg = color::to_u8(palette.theme.panel);
        let column_width = content_width + gutter_width;
        for col in 0..cols {
            let x0 = inner.x + (column_width + COLUMN_GAP) * col as u16;
            let start = self.scroll + col * usize::from(inner.height);
            for row in 0..inner.height {
                let idx = start + usize::from(row);
                if idx >= total {
                    break;
                }
                let row_bg = (marks_diff[idx] == Mark::Added)
                    .then(|| palette.code_added_bg(base_bg))
                    .flatten();
                let y = inner.y + row;
                draw_gutter(buf, (x0, y), digits, idx, marks_diff[idx], palette);
                let line = CodeLine { text: lines[idx], spans: &highlighted[idx] };
                draw_code_line(buf, (x0 + gutter_width, y), content_width, &line, row_bg, palette);
            }
        }
    }
}

/// How many columns the code pane splits into, and how wide each one's content is (not counting
/// its own gutter): two, newspaper order, once the source doesn't fit in one column's height and
/// each column would still get at least `two_column_min` code characters; one otherwise.
fn pick_columns(
    total: usize,
    inner_width: u16,
    gutter_width: u16,
    inner_height: u16,
    two_column_min: u16,
) -> (usize, u16) {
    let one_col_content = inner_width.saturating_sub(gutter_width);
    if inner_height == 0 || total as u16 <= inner_height {
        return (1, one_col_content);
    }
    if inner_width > COLUMN_GAP + 2 * gutter_width {
        let each = (inner_width - COLUMN_GAP) / 2;
        let content = each.saturating_sub(gutter_width);
        if content >= two_column_min {
            return (2, content);
        }
    }
    (1, one_col_content)
}

/// The gutter: `idx`'s line number (1-based), right-aligned to `digits` columns, then the diff
/// marker (`▌`/`+` added, `▁`/`_` the line just above a deletion, blank otherwise), then a space
/// drawn by [`draw_code_line`]'s own start column.
fn draw_gutter(
    buf: &mut Buffer,
    (x0, y): (u16, u16),
    digits: u16,
    idx: usize,
    mark: Mark,
    palette: &Palette,
) {
    let line_no = format!("{:>width$}", idx + 1, width = usize::from(digits));
    let gutter_style = palette.code_gutter_style();
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

/// One highlighted line of source, as [`draw_code_line`] draws it: the text and the spans
/// [`fn_ui::code::highlight_line`] classified it into.
struct CodeLine<'a> {
    text: &'a str,
    spans: &'a [code::Span],
}

/// `line`, from `(content_x, y)` across `content_width` columns: cut with `…`/`>` if it's longer,
/// and tinted with `row_bg` (an added line's background) across the whole width, past the code
/// too.
fn draw_code_line(
    buf: &mut Buffer,
    (content_x, y): (u16, u16),
    content_width: u16,
    line: &CodeLine,
    row_bg: Option<Color>,
    palette: &Palette,
) {
    let width = usize::from(content_width);
    let total_chars = line.text.chars().count();
    let cut = total_chars > width;
    let take = if cut { width.saturating_sub(1) } else { width };
    let mut x = content_x;
    let mut drawn = 0usize;
    'spans: for s in line.spans {
        let base = palette.code_style(s.class);
        let style = if let Some(bg) = row_bg { base.bg(bg) } else { base };
        for c in line.text[s.start..s.end].chars() {
            if drawn >= take {
                break 'spans;
            }
            buf[(x, y)].set_char(c).set_style(style);
            x += 1;
            drawn += 1;
        }
    }
    if cut {
        let ell = if palette.ascii { '>' } else { '…' };
        let style = if let Some(bg) = row_bg { Style::new().bg(bg) } else { Style::new() };
        buf[(x, y)].set_char(ell).set_style(style);
        x += 1;
    }
    if let Some(bg) = row_bg {
        while x < content_x + content_width {
            buf[(x, y)].set_style(Style::new().bg(bg));
            x += 1;
        }
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
        assert!(!pane.key(key('x')), "not a code-pane key");
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

    /// An added line gets the `▌` marker, and the line just above a run of deleted lines gets
    /// `▁`.
    #[test]
    fn diff_markers_show_in_the_gutter() {
        let mut pane = CodePane::new();
        pane.on_script("keep-fed", 1, "a\nb\nc", 0);
        pane.on_script("keep-fed", 2, "a\nx\nc", 10);
        let mut buf = Buffer::empty(Rect::new(0, 0, 40, 10));
        pane.render(&mut buf, Rect::new(0, 0, 40, 10), &palette());
        let text = crate::frame::to_text(&buf);
        let rows: Vec<&str> = text.lines().collect();
        // Line 1 ("a") is unchanged and is also the line just above the removed "b": `RemovedAfter`.
        assert!(rows[1].contains('▁'), "{}", rows[1]);
        assert!(rows[2].contains('▌'), "{}", rows[2]);
    }

    #[test]
    fn pick_columns_splits_when_the_source_does_not_fit_and_there_is_room() {
        assert_eq!(pick_columns(100, 160, 4, 20, 56), (2, 75));
        // Not enough width for two columns of at least `two_column_min`: stays one column.
        assert_eq!(pick_columns(100, 80, 4, 20, 56), (1, 76));
        // Fits in one column's height: stays one column even with plenty of width.
        assert_eq!(pick_columns(10, 250, 4, 20, 56), (1, 246));
    }
}
