//! The message log screen (`L`, #661): the log as a player reads it. The newest line is at the
//! top, a run of the same event is one line (`Right` opens it, `Left` folds it), a row of
//! filters picks one kind of line (`1` to `5`, `0` for all; `Tab` in the `--watch` spectator,
//! where the studio has the digits, #774), the cursor moves a line at a time,
//! and `Enter` goes to where the line is about.
//!
//! ```text
//! ┏──────────────────────────── ◆ MESSAGE LOG ◆ ────────────────────────────┓
//! │ Show  [All]  1 Alerts  2 People  3 Knowledge  4 Bending  5 Work          │
//! │                                                                          │
//! │ ── WINTER, YEAR 1 ────────────────────────────────────────────────────── │
//! │   0:51 left   Learned Pottery                                            │
//! │ ▸ 8:11 left + Raise Earth used 7 times by 6 benders, near (51, 65)       │
//! │  10:00 left   Jin has joined the settlement                              │
//! ```
//!
//! What the lines are, how they fold and what the cursor is on is `crate::msglog`; this module
//! draws them and turns the keys into changes of [`UiState`]. The app and the watch spectator
//! share [`act`] and [`press`], so the two never disagree. It only reads the history: folding
//! and filtering are never written back, and the run log an agent reads is one line per event.

use crate::Session;
use crate::alerts::{Category, Kind};
use crate::input::Action;
use crate::msglog::{self as model, Line, LineKey};
use crate::rows::{Marks, Row, Tone, fit, wrap};
use crate::screens::kind_tone;
use crate::state::{Screen, UiState};
use crate::time::{Period, game_time};
use std::ops::Range;

/// The rows above the log's lines: the filter row and a blank one.
pub const HEAD_ROWS: usize = 2;

/// The lines the log shows now, newest first.
pub fn lines(ui: &UiState) -> Vec<Line> {
    model::lines(ui.alerts.history(), ui.log_filter, &ui.log_open)
}

/// The tone a line wears: an alert in the warning colour, the rest as their kind reads
/// (`screens::kind_tone`).
pub fn line_tone(kind: Kind, category: Category) -> Tone {
    if category == Category::Alert { Tone::Warn } else { kind_tone(kind) }
}

/// The filter row: `Show  [All]  1 Alerts  2 People ...`, the one that is on in brackets and the
/// title tone, the rest dim. For the `--watch` spectator (`spectating`) the chips carry no
/// digits: they are not its keys, `Tab` steps through them.
pub fn filter_row(filter: Option<Category>, spectating: bool) -> Row {
    let mut spans = vec![("Show".to_owned(), Tone::Dim)];
    let mut chip = |on: bool, text: String| {
        spans.push(("  ".to_owned(), Tone::Text));
        if on {
            spans.push((format!("[{text}]"), Tone::Title));
        } else {
            spans.push((text, Tone::Dim));
        }
    };
    chip(filter.is_none(), "All".to_owned());
    for c in Category::ALL {
        let text =
            if spectating { c.label().to_owned() } else { format!("{} {}", c.key(), c.label()) };
        chip(filter == Some(c), text);
    }
    Row::spans(spans)
}

/// The log's rows below the filter row, `width` cells wide, and where the cursor's line is in
/// them (all the rows it wraps to).
pub struct Body {
    pub rows: Vec<Row>,
    pub cursor: Option<Range<usize>>,
}

/// Lays `lines` out as rows: a heading row where the season (or the day) changes, then each line
/// stamped with the time left in it, a pointer in front of the one under the cursor, a `+` or a
/// `-` before a fold, and an open fold's own lines indented under it. A line's text wraps under
/// its stamp.
pub fn body(
    session: &Session,
    lines: &[Line],
    filter: Option<Category>,
    spectating: bool,
    at: Option<LineKey>,
    width: usize,
    marks: &Marks,
) -> Body {
    if lines.is_empty() {
        let none = match filter {
            None => "Nothing has happened yet: no bends, no discoveries, no deaths.".to_owned(),
            Some(c) if spectating => {
                format!("Nothing under {} yet. Press Tab to go on to the next kind.", c.label())
            }
            Some(c) => format!(
                "Nothing under {} yet. Press {} again to see everything.",
                c.label(),
                c.key()
            ),
        };
        let rows = wrap(&none, width, usize::MAX, marks.ellipsis)
            .into_iter()
            .map(|line| Row::text(line, Tone::Dim))
            .collect();
        return Body { rows, cursor: None };
    }
    let seasons = session.world.knobs(&session.defs).seasons;
    let times: Vec<_> = lines.iter().map(|l| game_time(&session.defs, l.tick, seasons)).collect();
    let stamps: Vec<String> = times.iter().map(|t| t.left()).collect();
    let stamp_width = stamps.iter().map(|s| s.chars().count()).max().unwrap_or(0);
    // The pointer, the stamp, the fold's sign: "▸ 8:11 left + text".
    let lead = 2 + stamp_width + 1 + 2;
    let room = width.saturating_sub(lead).max(1);
    let rule = marks.h.to_string().repeat(2);
    let cursor = model::index_of(lines, at);
    let mut rows = Vec::new();
    let mut span = None;
    let mut period: Option<&Period> = None;
    for (i, ((line, stamp), time)) in lines.iter().zip(&stamps).zip(&times).enumerate() {
        // An open fold's own lines sit under it, in its heading.
        if !line.key.sub && period != Some(&time.period) {
            period = Some(&time.period);
            let heading = format!("{rule} {} {rule}", time.heading());
            rows.push(Row::text(fit(&heading, width, marks.ellipsis), Tone::Title));
        }
        let first = rows.len();
        let chosen = i == cursor;
        let pointer = if chosen { format!("{} ", marks.pointer) } else { "  ".to_owned() };
        let sign = match (line.is_fold(), line.open) {
            (true, true) => "- ",
            (true, false) => "+ ",
            _ => "  ",
        };
        let indent = if line.key.sub { 2 } else { 0 };
        let tone = line_tone(line.kind, line.category);
        let text = wrap(&line.text, room.saturating_sub(indent).max(1), usize::MAX, marks.ellipsis);
        for (n, part) in text.into_iter().enumerate() {
            let head = if n == 0 {
                format!("{stamp:>stamp_width$} ")
            } else {
                " ".repeat(stamp_width + 1)
            };
            let sign = if n == 0 { sign } else { "  " };
            let pointer = if n == 0 { pointer.clone() } else { "  ".to_owned() };
            rows.push(Row::spans(vec![
                (pointer, Tone::Title),
                (head, Tone::Dim),
                (sign.to_owned(), Tone::Accent),
                (format!("{}{part}", " ".repeat(indent)), tone),
            ]));
        }
        if chosen {
            span = Some(first..rows.len());
        }
    }
    Body { rows, cursor: span }
}

/// The first row of the log's window so the cursor's lines show: `scroll` if they already do,
/// else the nearest that shows them. The first line shows its heading too, and the last row of
/// the log never leaves the window blank.
pub fn window(scroll: usize, cursor: Option<&Range<usize>>, shown: usize, total: usize) -> usize {
    let max = total.saturating_sub(shown);
    let mut start = scroll.min(max);
    if let Some(c) = cursor {
        if c.start < start {
            start = c.start;
        } else if c.end > start + shown {
            start = c.end.saturating_sub(shown);
        }
        // The cursor on the first line scrolls all the way up: its heading is above it.
        if c.start <= 1 {
            start = 0;
        }
    }
    start.min(max)
}

/// How many rows of lines show at once on the open page.
pub fn shown(page: (i32, i32)) -> usize {
    crate::screens::body_rows(Screen::Log, page)
}

/// Moves the scroll so the cursor's lines show, after the cursor or the lines changed.
pub fn keep_in_view(ui: &mut UiState, session: &Session) {
    let all = lines(ui);
    let width = crate::screens::text_size(ui.page()).0;
    let laid =
        body(session, &all, ui.log_filter, ui.spectating, ui.log_cursor, width, &Marks::new(false));
    let shown = shown(ui.page());
    ui.scroll = window(ui.scroll, laid.cursor.as_ref(), shown, laid.rows.len());
}

/// Starts the log over: the newest line at the top with the cursor on it, everything shown,
/// every fold closed.
pub fn reset(ui: &mut UiState) {
    ui.log_filter = None;
    ui.log_cursor = None;
    ui.log_open.clear();
    ui.scroll = 0;
}

/// What a key does in the open log, as an [`Action`] the app reads: the arrows choose a line
/// (`Right` opens a fold, `Left` folds it), the page keys a page of them, `Enter` goes to the
/// line's place. `true` when the action was the log's to take.
pub fn act(ui: &mut UiState, session: &Session, action: Action) -> bool {
    match action {
        Action::Move { dx, dy, tiles } => {
            if dy != 0 {
                step(ui, session, i64::from(dy * tiles));
            } else if dx > 0 {
                open(ui, session);
            } else if dx < 0 {
                close(ui, session);
            }
            true
        }
        Action::Jump { dy, .. } => {
            let page = shown(ui.page()).max(1) as i64;
            step(ui, session, i64::from(dy) * page);
            true
        }
        Action::Confirm => {
            go(ui, session);
            true
        }
        _ => false,
    }
}

/// A digit in the open log: `1` to `5` show one kind of line, `0` or the same digit again shows
/// all. `true` when the key was a filter's.
pub fn press(ui: &mut UiState, session: &Session, key: char) -> bool {
    let Some(filter) = model::filter_after(ui.log_filter, key) else { return false };
    set_filter(ui, session, filter);
    true
}

/// `Tab` (`back`: `Shift+Tab`) in the `--watch` spectator's log: the next (previous) kind of
/// line, going round through all (#774). Reads and writes only the spectator's own [`UiState`].
pub fn cycle(ui: &mut UiState, session: &Session, back: bool) {
    let filter = model::filter_cycled(ui.log_filter, back);
    set_filter(ui, session, filter);
}

/// Shows `filter`'s lines, the cursor on the top one.
pub fn set_filter(ui: &mut UiState, session: &Session, filter: Option<Category>) {
    ui.log_filter = filter;
    // The line the cursor was on may be gone from the new set: read as the top.
    let all = lines(ui);
    ui.log_cursor = all.first().map(|l| l.key);
    ui.scroll = 0;
    keep_in_view(ui, session);
}

/// Moves the cursor `dy` lines, stopping at the ends.
pub fn step(ui: &mut UiState, session: &Session, dy: i64) {
    let all = lines(ui);
    ui.log_cursor = model::step(&all, ui.log_cursor, dy);
    keep_in_view(ui, session);
}

/// `Right`: opens the fold under the cursor into its lines.
pub fn open(ui: &mut UiState, session: &Session) {
    let all = lines(ui);
    let at = model::index_of(&all, ui.log_cursor);
    if let Some(line) = all.get(at).filter(|l| l.is_fold() && !l.open) {
        ui.log_open.push(line.key.id);
        ui.log_cursor = Some(line.key);
    }
    keep_in_view(ui, session);
}

/// `Left`: folds the fold under the cursor, or the one a line of it belongs to, and puts the
/// cursor on it.
pub fn close(ui: &mut UiState, session: &Session) {
    let all = lines(ui);
    let at = model::index_of(&all, ui.log_cursor);
    if let Some(line) = all.get(at) {
        // One of a fold's own lines: the open fold above it.
        let parent = if line.key.sub {
            all[..at].iter().rev().find(|l| l.open).map(|l| l.key)
        } else if line.open {
            Some(line.key)
        } else {
            None
        };
        if let Some(parent) = parent {
            ui.log_open.retain(|&o| o != parent.id);
            ui.log_cursor = Some(parent);
        }
    }
    keep_in_view(ui, session);
}

/// `Enter`: goes to what the line under the cursor is about.
pub fn go(ui: &mut UiState, session: &Session) {
    let all = lines(ui);
    if let Some(line) = all.get(model::index_of(&all, ui.log_cursor)) {
        jump(ui, session, line);
    }
}

/// Closes the log with the map cursor on the line's place; for a line with only a settler, with
/// that settler selected. A line about neither says so and the log stays open.
pub fn jump(ui: &mut UiState, session: &Session, line: &Line) {
    if let Some(place) = line.place {
        ui.go_to(place);
        ui.screen = None;
    } else if let Some(unit) = line.unit {
        if session.world.units.iter().any(|u| u.id == unit) {
            ui.select_unit(session, unit);
            ui.screen = None;
        } else {
            ui.status = "That settler is no longer here.".to_owned();
        }
    } else {
        ui.status = "That line has no place to go to.".to_owned();
    }
}

#[cfg(test)]
mod tests {
    use fn_sim::Pos;
    use fn_sim::world::UnitId;

    use super::*;

    #[test]
    fn the_filter_row_brackets_the_one_that_is_on() {
        assert_eq!(
            filter_row(None, false).plain(),
            "Show  [All]  1 Alerts  2 People  3 Knowledge  4 Bending  5 Work"
        );
        assert_eq!(
            filter_row(Some(Category::Knowledge), false).plain(),
            "Show  All  1 Alerts  2 People  [3 Knowledge]  4 Bending  5 Work"
        );
        // The watch spectator's chips have no digits (#774).
        assert_eq!(
            filter_row(Some(Category::Knowledge), true).plain(),
            "Show  All  Alerts  People  [Knowledge]  Bending  Work"
        );
    }

    #[test]
    fn the_window_keeps_the_cursor_in_sight() {
        // 30 rows, 10 shown.
        assert_eq!(window(0, Some(&(12..13)), 10, 30), 3);
        assert_eq!(window(20, Some(&(5..6)), 10, 30), 5);
        assert_eq!(window(4, Some(&(6..8)), 10, 30), 4);
        // Never past the end.
        assert_eq!(window(99, None, 10, 30), 20);
        // The first line shows its heading.
        assert_eq!(window(5, Some(&(1..2)), 10, 30), 0);
    }

    fn line(place: Option<Pos>, unit: Option<UnitId>) -> Line {
        Line {
            key: LineKey { id: 1, sub: false },
            tick: 0,
            kind: Kind::Bend,
            category: Category::Bending,
            text: "x".to_owned(),
            count: 1,
            open: false,
            place,
            unit,
        }
    }

    fn open_ui(session: &Session) -> UiState {
        let mut ui = UiState::new(session, (120, 40));
        ui.screen = Some(Screen::Log);
        ui
    }

    /// `Enter` on a line with a place closes the log with the map cursor there, the view centred.
    #[test]
    fn enter_on_a_place_puts_the_map_cursor_there_and_closes_the_log() {
        let s = crate::new_session(7, fn_sim::Nation::Earth);
        let mut ui = open_ui(&s);
        ui.cursor = Pos::new(1, 1);
        let place = Pos::new(s.world.map.width() - 5, s.world.map.height() - 5);
        jump(&mut ui, &s, &line(Some(place), None));
        assert_eq!(ui.screen, None);
        assert_eq!(ui.cursor, place);
        assert!(ui.cursor_cell().is_some(), "the cursor is in view");
    }

    /// With only a settler, that settler is selected (and the cursor goes to them).
    #[test]
    fn enter_on_a_settler_selects_them_and_closes_the_log() {
        let s = crate::new_session(7, fn_sim::Nation::Earth);
        let mut ui = open_ui(&s);
        let unit = &s.world.units[2];
        jump(&mut ui, &s, &line(None, Some(unit.id)));
        assert_eq!(ui.screen, None);
        assert_eq!(ui.selected, Some(unit.id));
        assert_eq!(ui.cursor, unit.pos);
    }

    /// Nothing to go to, or a settler who is gone: the log stays open and says why. A place wins
    /// over a settler.
    #[test]
    fn enter_on_a_line_with_nowhere_to_go_keeps_the_log_open() {
        let s = crate::new_session(7, fn_sim::Nation::Earth);
        let mut ui = open_ui(&s);
        jump(&mut ui, &s, &line(None, None));
        assert_eq!(ui.screen, Some(Screen::Log));
        assert!(ui.status.contains("no place"), "{}", ui.status);
        jump(&mut ui, &s, &line(None, Some(UnitId(9999))));
        assert_eq!(ui.screen, Some(Screen::Log));
        assert!(ui.status.contains("no longer here"), "{}", ui.status);
        let place = Pos::new(3, 3);
        jump(&mut ui, &s, &line(Some(place), Some(s.world.units[0].id)));
        assert_eq!(ui.cursor, place);
        assert_eq!(ui.selected, None, "the place wins");
    }
}
