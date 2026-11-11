//! The full screens (`crate::page`, #652), each covering the map and the side panel: the message
//! log (`L`), the key help (`?`) and the Endeavor screen (`G`, `endeavor.rs`) are pages of rows
//! that scroll; the research screen (`T`) is a grid of tech cards with a cursor and a detail
//! strip (`research.rs`); the Nation screen (`N`, #308, `nation.rs`) is a roster with a pointer;
//! the saves screen (`F6`, `saves.rs`) is a list with a pointer. The Build picker (`U`,
//! `build.rs`) wears the same frame. Each closes on `Esc` or on the key that opened it.

use fn_ui::Session;
use fn_ui::alerts::{Alerts, Kind};
use fn_ui::time::{Period, game_time};
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;

use crate::color::{ColorDepth, Palette};
use crate::input::{Group, HelpRow, help_rows};
use crate::legend;
use crate::page;
use crate::research;
use crate::saves;
use crate::state::{HelpTab, Screen, UiState};
use crate::widgets::{Marks, Painter, Row, Tone, fit, wrap};

/// The text area of a screen on a `(width, height)` page (`UiState::page`): inside the frame and
/// one column of padding, as (columns, rows) (`crate::page::text_size`).
pub fn text_size(page: (i32, i32)) -> (usize, usize) {
    page::text_size(page)
}

/// The screen's text as rows, `width` columns wide. `ascii` picks the glyph set the Endeavor
/// screen's marks and bars wear (the message log and the key help don't need it).
pub fn lines(
    session: &Session,
    screen: Screen,
    alerts: &Alerts,
    width: usize,
    ascii: bool,
) -> Vec<Row> {
    let marks = Marks::new(ascii);
    match screen {
        Screen::Log => {
            let history: Vec<_> =
                alerts.history().iter().map(|e| (e.tick, e.kind, e.text.as_str())).collect();
            log_lines(session, &history, width, &marks)
        }
        Screen::Help => help_lines(width, marks.ellipsis),
        Screen::Endeavor => crate::endeavor::lines(session, width, &marks),
        // A grid or a list with a cursor, not rows that scroll: `research::draw`, `saves::draw`
        // and `nation::draw` lay them out.
        Screen::Research | Screen::Saves | Screen::Nation => Vec::new(),
    }
}

/// How many rows the screen's text has at `width`. `ascii` never changes a row count (only which
/// glyph a mark or a bar's cells wear), so this always measures the Unicode set.
pub fn line_count(screen: Screen, session: &Session, alerts: &Alerts, width: usize) -> usize {
    lines(session, screen, alerts, width, false).len()
}

/// How many rows of a screen's own text show at once on a `page`: the key help gives its first
/// two to the tab strip and a blank row.
pub fn body_rows(screen: Screen, page: (i32, i32)) -> usize {
    let rows = text_size(page).1;
    if screen == Screen::Help { rows.saturating_sub(HELP_HEAD_ROWS) } else { rows }
}

/// How many rows the open screen scrolls through at `width`: [`line_count`], except that the key
/// help counts whichever tab shows (the legend is built from the map in view).
pub fn row_count(session: &Session, ui: &UiState, screen: Screen, width: usize) -> usize {
    if screen == Screen::Help && ui.help_tab == HelpTab::Legend {
        // Which entries there are never depends on the palette, only how they draw.
        let palette = Palette::new(&session.defs, session.nation(), ColorDepth::TrueColor, false);
        return legend::line_count(session, ui, &palette, width);
    }
    line_count(screen, session, &ui.alerts, width)
}

/// The tone a log line wears: deaths stand out, discoveries read as good news. The side panel's
/// Alerts toasts share this (`crate::panel`), so the two never disagree.
pub(crate) fn kind_tone(kind: Kind) -> Tone {
    match kind {
        Kind::Bend => Tone::Text,
        Kind::Tech => Tone::Ok,
        Kind::Death => Tone::Bad,
        Kind::Endeavor => Tone::Ok,
        Kind::Arrival => Tone::Ok,
        // The warning, winter itself, frost and a plant picked to death are all cautionary;
        // spring's good news is the one exception, not worth a tone of its own.
        Kind::Season => Tone::Warn,
        // A full stockpile (#337) is exactly the same kind of warning: something worth fixing,
        // not a disaster that already happened.
        Kind::Stockpile => Tone::Warn,
        // A finished chart is a discovery, good news like a finished tech.
        Kind::Chart => Tone::Ok,
        // A watched run's own entries (#319): plain text, like a human's own actions would read.
        Kind::Agent => Tone::Text,
        // A gentle onboarding nudge (#382), not something gone wrong: the same tone as a
        // routine bend, not a warning.
        Kind::BendTip => Tone::Text,
    }
}

/// The message log, oldest first (#654): a heading row per season ("── WINTER, YEAR 1 ──", or per
/// day with seasons off), and under it each line stamped with the time left in that season when
/// it happened ("14:50 left"), the same words as the panel's clock, so a line can be placed
/// against it. The text wraps under the stamp.
pub fn log_lines(
    session: &Session,
    history: &[(u64, Kind, &str)],
    width: usize,
    marks: &Marks,
) -> Vec<Row> {
    if history.is_empty() {
        let none = "Nothing has happened yet: no bends, no discoveries, no deaths.";
        return wrap(none, width, usize::MAX, marks.ellipsis)
            .into_iter()
            .map(|line| Row::text(line, Tone::Dim))
            .collect();
    }
    let seasons = session.world.knobs(&session.defs).seasons;
    let times: Vec<_> =
        history.iter().map(|&(tick, ..)| game_time(&session.defs, tick, seasons)).collect();
    let stamps: Vec<String> = times.iter().map(|t| t.left()).collect();
    let stamp_width = stamps.iter().map(|s| s.chars().count()).max().unwrap_or(0);
    let indent = stamp_width + 1;
    let room = width.saturating_sub(indent).max(1);
    let rule = marks.h.to_string().repeat(2);
    let mut rows = Vec::new();
    let mut period: Option<&Period> = None;
    for ((&(_, kind, text), stamp), time) in history.iter().zip(&stamps).zip(&times) {
        if period != Some(&time.period) {
            period = Some(&time.period);
            let heading = format!("{rule} {} {rule}", time.heading());
            rows.push(Row::text(fit(&heading, width, marks.ellipsis), Tone::Title));
        }
        let tone = kind_tone(kind);
        let text = wrap(text, room, usize::MAX, marks.ellipsis);
        for (i, part) in text.into_iter().enumerate() {
            let lead = if i == 0 { format!("{stamp:>stamp_width$} ") } else { " ".repeat(indent) };
            rows.push(Row::spans(vec![(lead, Tone::Dim), (part, tone)]));
        }
    }
    rows
}

/// How many rows the key help's tab strip takes: the tabs, and a blank row under them.
pub const HELP_HEAD_ROWS: usize = 2;

/// The text width from which the key help sets its groups in two columns: a page 100 cells wide
/// (`page::text_size` takes the frame and its padding off). Narrower, it is one column that
/// scrolls.
pub const TWO_COLUMNS_FROM: usize = 96;

/// The cells between the two columns.
const COLUMN_GAP: usize = 2;

/// The tab strip: `[Keys]  Legend`, the open tab in brackets and the title tone, and what `Tab`
/// does flush right.
pub fn help_tabs(tab: HelpTab) -> Row {
    let name = |this: HelpTab, text: &str| {
        if this == tab { (format!("[{text}]"), Tone::Title) } else { (text.to_owned(), Tone::Dim) }
    };
    let (keys, legend) = (name(HelpTab::Keys, "Keys"), name(HelpTab::Legend, "Legend"));
    let mut row = Row::spans(vec![keys, ("  ".to_owned(), Tone::Text), legend]);
    row.right = Some(("Tab switches".to_owned(), Tone::Dim));
    row
}

/// One group of the key help as rows: its heading, then a row per key, the keys in a column as
/// wide as the group's longest label.
fn group_block(rows: &[&HelpRow], group: Group) -> Vec<Row> {
    let keys_width = rows.iter().map(|r| r.keys.chars().count()).max().unwrap_or(0);
    let mut out = vec![Row::text(group.title().to_uppercase(), Tone::Title)];
    out.extend(rows.iter().map(|r| {
        Row::spans(vec![
            (format!("{:<keys_width$}  ", r.keys), Tone::Accent),
            (r.what.clone(), Tone::Text),
        ])
    }));
    out
}

/// The key help's keys as rows `width` cells wide: the groups of [`help_rows`], each key beside
/// what it does, one row per key. From [`TWO_COLUMNS_FROM`] the groups are set in two columns,
/// split between groups where the taller column is shortest; below it, in one column that wraps
/// a long row under its keys.
pub fn help_lines(width: usize, ellipsis: char) -> Vec<Row> {
    let rows = help_rows();
    let groups: Vec<Group> =
        Group::ALL.into_iter().filter(|g| rows.iter().any(|r| r.group == *g)).collect();
    if width >= TWO_COLUMNS_FROM {
        let blocks: Vec<Vec<Row>> = groups
            .iter()
            .map(|g| group_block(&rows.iter().filter(|r| r.group == *g).collect::<Vec<_>>(), *g))
            .collect();
        return two_columns(&blocks, width, ellipsis);
    }
    one_column(&rows, &groups, width, ellipsis)
}

/// The height of `blocks` stacked with a blank row between.
fn stacked(blocks: &[Vec<Row>]) -> usize {
    blocks.iter().map(Vec::len).sum::<usize>() + blocks.len().saturating_sub(1)
}

/// `blocks` in two columns across `width`: the first `k` blocks on the left, the rest on the
/// right, for the `k` that makes the taller column shortest (the first such, on a tie).
fn two_columns(blocks: &[Vec<Row>], width: usize, ellipsis: char) -> Vec<Row> {
    let split = (1..blocks.len())
        .min_by_key(|&k| stacked(&blocks[..k]).max(stacked(&blocks[k..])))
        .unwrap_or(blocks.len());
    let (left, right) = blocks.split_at(split);
    let column = (width - COLUMN_GAP) / 2;
    let stack = |blocks: &[Vec<Row>]| -> Vec<Row> {
        let mut out: Vec<Row> = Vec::new();
        for block in blocks {
            if !out.is_empty() {
                out.push(Row::default());
            }
            out.extend(block.iter().cloned());
        }
        out
    };
    let (left, right) = (stack(left), stack(right));
    (0..left.len().max(right.len()))
        .map(|i| {
            let mut spans = clipped(left.get(i), column, ellipsis);
            let used: usize = spans.iter().map(|(t, _)| t.chars().count()).sum();
            spans.push((" ".repeat(column - used + COLUMN_GAP), Tone::Text));
            spans.extend(clipped(right.get(i), column, ellipsis));
            Row::spans(spans)
        })
        .collect()
}

/// `row`'s spans, cut with an ellipsis if they are wider than `width`.
fn clipped(row: Option<&Row>, width: usize, ellipsis: char) -> Vec<(String, Tone)> {
    let mut left = width;
    let mut out = Vec::new();
    for (text, tone) in row.map_or(&[][..], |r| r.left.as_slice()) {
        let shown = fit(text, left, ellipsis);
        left -= shown.chars().count();
        out.push((shown, *tone));
    }
    out
}

/// The key help in one column, for a page too narrow for two: each row's wording wraps under its
/// keys.
fn one_column(rows: &[HelpRow], groups: &[Group], width: usize, ellipsis: char) -> Vec<Row> {
    let longest = rows.iter().map(|r| r.keys.chars().count()).max().unwrap_or(0);
    let keys_width = longest.min(width * 2 / 5).max(1);
    let what_width = width.saturating_sub(keys_width + 2).max(1);
    let mut out = Vec::new();
    for &group in groups {
        if !out.is_empty() {
            out.push(Row::default());
        }
        out.push(Row::text(group.title().to_uppercase(), Tone::Title));
        for r in rows.iter().filter(|r| r.group == group) {
            let keys = wrap(&r.keys, keys_width, usize::MAX, ellipsis);
            let what = wrap(&r.what, what_width, usize::MAX, ellipsis);
            for i in 0..keys.len().max(what.len()) {
                out.push(Row::spans(vec![
                    (
                        format!("{:<keys_width$}  ", keys.get(i).map_or("", String::as_str)),
                        Tone::Accent,
                    ),
                    (what.get(i).cloned().unwrap_or_default(), Tone::Text),
                ]));
            }
        }
    }
    out
}

/// The title a screen's frame carries (`crate::page::draw` puts it between two seals).
pub fn title(screen: Screen) -> &'static str {
    match screen {
        Screen::Log => "MESSAGE LOG",
        Screen::Help => "KEY HELP",
        Screen::Endeavor => "ENDEAVOR",
        Screen::Research => "RESEARCH",
        Screen::Saves => "SAVES",
        Screen::Nation => "NATION",
    }
}

/// Draws the open screen over `area` (the whole page, `crate::layout::Regions::page`, #652): the
/// shared frame, then the screen's own content and its position on the bottom edge.
pub fn draw_screen(
    session: &Session,
    ui: &UiState,
    screen: Screen,
    palette: &Palette,
    area: Rect,
    buf: &mut Buffer,
) {
    let detail_rows = if screen == Screen::Research { research::DETAIL_ROWS } else { 0 };
    let page = page::draw(buf, area, palette, title(screen), detail_rows);
    let position = match screen {
        Screen::Research => research::draw(session, ui.tech, palette, &page, buf),
        Screen::Saves => {
            saves::draw(&ui.save_slots, ui.save_cursor, &session.defs, palette, page.body, buf)
        }
        Screen::Nation => {
            // Whether the page is wide enough for the meters (docs/design/side-panel.md "The
            // Nation screen").
            let wide = area.width >= crate::nation::METER_MIN_WIDTH;
            crate::nation::draw(session, ui, wide, palette, &page, buf)
        }
        Screen::Help => draw_help(session, ui, palette, page.body, buf),
        Screen::Log | Screen::Endeavor => {
            let body = page.body;
            let rows = lines(session, screen, &ui.alerts, usize::from(body.width), palette.ascii);
            let max = rows.len().saturating_sub(usize::from(body.height));
            let start = ui.scroll.min(max);
            let mut p = Painter::new(buf, body, palette);
            for row in rows.iter().skip(start) {
                p.row(row);
            }
            fn_ui::text::window_label("", start, usize::from(body.height), rows.len())
        }
    };
    page.position(buf, palette, position.as_deref());
}

/// The key help: its tab strip, a blank row, then the open tab's rows from the scroll. Returns the
/// position for the bottom edge (`None` when everything fits).
fn draw_help(
    session: &Session,
    ui: &UiState,
    palette: &Palette,
    body: Rect,
    buf: &mut Buffer,
) -> Option<String> {
    let head = (HELP_HEAD_ROWS as u16).min(body.height);
    let mut tabs = Painter::new(buf, Rect::new(body.x, body.y, body.width, head), palette);
    tabs.row(&help_tabs(ui.help_tab));
    let below = Rect::new(body.x, body.y + head, body.width, body.height - head);
    let shown = usize::from(below.height);
    let width = usize::from(below.width);
    let (start, total) = match ui.help_tab {
        HelpTab::Keys => {
            let rows = help_lines(width, Marks::new(palette.ascii).ellipsis);
            let start = ui.scroll.min(rows.len().saturating_sub(shown));
            let mut p = Painter::new(buf, below, palette);
            for row in rows.iter().skip(start) {
                p.row(row);
            }
            (start, rows.len())
        }
        HelpTab::Legend => {
            let total = legend::line_count(session, ui, palette, width);
            let start = ui.scroll.min(total.saturating_sub(shown));
            legend::draw(session, ui, palette, start, below, buf);
            (start, total)
        }
    };
    fn_ui::text::window_label("", start, shown, total)
}

#[cfg(test)]
mod tests {
    use fn_sim::Nation;

    use super::*;
    use crate::new_session;

    /// A line is stamped with the time left in its season, under a heading row per season, and
    /// wraps under the stamp.
    #[test]
    fn the_log_puts_the_time_left_first_under_a_heading_per_season() {
        let mut s = new_session(7, Nation::Earth);
        s.world.set_difficulty(Some("normal".to_owned()));
        let per = s.defs.seasons.season_ticks();
        let history = [
            (0, Kind::Bend, "Haru used Raise Earth at (5, 3)"),
            (650, Kind::Death, "Tyro died of dehydration, far from any water"),
            (3 * per + 10, Kind::Season, "Winter has come"),
        ];
        let rows = log_lines(&s, &history, 30, &Marks::new(false));
        let text: Vec<String> = rows.iter().map(Row::plain).collect();
        assert_eq!(text[0], "── SPRING, YEAR 1 ──");
        assert_eq!(text[1], "15:00 left Haru used Raise");
        assert_eq!(text[2], "           Earth at (5, 3)");
        assert!(text[3].starts_with("13:55 left Tyro died of"), "{text:?}");
        assert!(text.contains(&"── WINTER, YEAR 1 ──".to_owned()), "{text:?}");
        assert!(text.iter().any(|t| t.starts_with("14:59 left Winter has")), "{text:?}");
        assert!(text.iter().all(|t| t.chars().count() <= 30), "{text:?}");
        // The kinds wear their tones; a heading is a title.
        assert_eq!(rows[0].left[0].1, Tone::Title);
        assert_eq!(rows[1].left[1].1, Tone::Text);
        assert_eq!(rows[3].left[1].1, Tone::Bad);
    }

    /// The stamp is the panel's own time left: a line from this very tick reads like the clock.
    #[test]
    fn a_log_stamp_matches_the_panel_clock_at_that_tick() {
        let mut s = new_session(7, Nation::Earth);
        s.world.set_difficulty(Some("normal".to_owned()));
        s.world.tick = 4321;
        let clock = fn_ui::time::clock_text(&s);
        let rows = log_lines(&s, &[(4321, Kind::Bend, "x")], 60, &Marks::new(true));
        let left = clock.split(" · ").nth(1).unwrap();
        assert!(rows[1].plain().starts_with(left), "{clock} vs {}", rows[1].plain());
        assert_eq!(rows[0].plain(), format!("-- {} --", fn_ui::time::session_time(&s).heading()));
    }

    /// With seasons off the log runs on days, in the same shape.
    #[test]
    fn with_seasons_off_the_log_has_a_heading_per_day() {
        let s = new_session(7, Nation::Earth);
        let history = [(0, Kind::Bend, "a"), (3100, Kind::Bend, "b")];
        let text: Vec<String> =
            log_lines(&s, &history, 40, &Marks::new(false)).iter().map(Row::plain).collect();
        assert_eq!(text[0], "── DAY 1 ──");
        assert_eq!(text[1], "5:00 left a");
        assert_eq!(text[2], "── DAY 2 ──");
        assert!(text[3].starts_with("4:50 left b"), "{text:?}");
    }

    #[test]
    fn an_empty_log_says_so() {
        let s = new_session(7, Nation::Earth);
        let rows = log_lines(&s, &[], 40, &Marks::new(false));
        assert!(rows[0].plain().starts_with("Nothing has happened"));
    }

    fn plain(rows: &[Row]) -> Vec<String> {
        rows.iter().map(Row::plain).collect()
    }

    #[test]
    fn the_help_has_a_heading_per_group_and_keeps_to_its_width() {
        for width in [40, 44, 70, 95, 96, 116, 200] {
            let text = plain(&help_lines(width, '…'));
            for g in Group::ALL {
                let title = g.title().to_uppercase();
                assert!(text.iter().any(|t| t.contains(&title)), "{width}: {g:?}");
            }
            assert!(text.iter().all(|t| t.chars().count() <= width), "{width}: {text:?}");
            assert!(text.iter().any(|t| t.contains("Harvest: chop")), "{width}");
        }
    }

    /// From 100 columns (96 of text) the help is two columns of one row per key: no row is cut or
    /// wrapped, every row is whole inside its column, and no issue number shows.
    #[test]
    fn the_help_is_two_columns_of_whole_one_line_rows_from_a_hundred_columns() {
        let rows = help_rows();
        for width in [TWO_COLUMNS_FROM, 100, 116] {
            let text = plain(&help_lines(width, '…'));
            let column = (width - COLUMN_GAP) / 2;
            for r in &rows {
                let line = text
                    .iter()
                    .find(|t| t.contains(&r.what) && t.contains(&r.keys))
                    .unwrap_or_else(|| {
                        panic!("{width}: {r:?} is not whole on one row:\n{text:#?}")
                    });
                // Where the row's wording ends, counted from the start of its own column.
                let at = line.find(&r.what).unwrap();
                let end = line[..at].chars().count() + r.what.chars().count();
                let own = if end <= column { end } else { end - (column + COLUMN_GAP) };
                assert!(own <= column, "{width}: {r:?} is wider than its column");
            }
            assert!(text.iter().all(|t| !t.contains('…') && !t.contains('#')), "{width}");
            // The groups are split between two columns, not stacked in one.
            let stacked = Group::ALL.len() * 2 - 1 + rows.len();
            assert!(text.len() < stacked, "{width}: {} rows of {stacked}", text.len());
        }
    }

    /// Below that it is one column, and the groups stack.
    #[test]
    fn the_help_is_one_column_below_a_hundred_columns() {
        let text = plain(&help_lines(TWO_COLUMNS_FROM - 1, '…'));
        for g in Group::ALL {
            let title = g.title().to_uppercase();
            assert!(text.contains(&title), "{title} has a row of its own:\n{text:#?}");
        }
    }

    #[test]
    fn the_tab_strip_brackets_the_open_tab() {
        assert_eq!(help_tabs(HelpTab::Keys).plain(), "[Keys]  Legend Tab switches");
        assert_eq!(help_tabs(HelpTab::Legend).plain(), "Keys  [Legend] Tab switches");
    }
}
