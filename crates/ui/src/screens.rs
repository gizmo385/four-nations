//! The full screens (`crate::page`, #652), each covering the map and the side panel: the
//! key help (`?`) and the Endeavor screen (`G`, `endeavor.rs`) are pages of rows that scroll; the
//! message log (`L`, #661, `msglog.rs`) is a list of lines with a cursor under a filter row; the
//! research screen (`T`) is a grid of tech cards with a cursor and a detail
//! strip (`research.rs`); the Nation screen (`N`, #308, `nation.rs`) is a roster with a pointer;
//! the saves screen (`F6`, `saves.rs`) is a list with a pointer; the abilities screen (`I`, #719,
//! `abilities.rs`) is a list with a pointer and a detail strip. The Build picker (`U`,
//! `build.rs`) wears the same frame. Each closes on `Esc` or on the key that opened it.

use crate::Session;
use crate::alerts::Kind;
use crate::input::{Group, HelpRow, help_rows};
use crate::page;
use crate::rows::{Marks, Row, Tone, fit, wrap};
use crate::state::{HelpTab, Screen, UiState};

pub mod abilities;
pub mod airship;
pub mod build;
pub mod endeavor;
pub mod msglog;
pub mod nation;
pub mod research;
pub mod saves;
pub mod title;
pub mod traders;

/// How many rows the Legend tab of the key help has at a text width: what the frontend that draws
/// the legend says, because only it knows what the map in view looks like (the legend is built
/// from the same cells the map is drawn from, `fn_tui::legend`). `App::legend_rows` holds it.
pub type LegendRows = fn(&Session, &UiState, usize) -> usize;

/// The Legend tab for a frontend with no legend: no rows, so there is nothing to scroll.
pub fn no_legend(_: &Session, _: &UiState, _: usize) -> usize {
    0
}

/// How many rows the open screen scrolls through at `width`: [`line_count`], except that the key
/// help counts whichever tab shows, and the legend is `legend`'s to count.
pub fn row_count(
    session: &Session,
    ui: &UiState,
    screen: Screen,
    width: usize,
    legend: LegendRows,
) -> usize {
    if screen == Screen::Help && ui.help_tab == HelpTab::Legend {
        return legend(session, ui, width);
    }
    line_count(screen, session, width)
}

/// The text area of a screen on a `(width, height)` page (`UiState::page`): inside the frame and
/// one column of padding, as (columns, rows) (`crate::page::text_size`).
pub fn text_size(page: (i32, i32)) -> (usize, usize) {
    page::text_size(page)
}

/// The screen's text as rows, `width` columns wide. `ascii` picks the glyph set the Endeavor
/// screen's marks and bars wear (the message log and the key help don't need it).
pub fn lines(session: &Session, screen: Screen, width: usize, ascii: bool) -> Vec<Row> {
    let marks = Marks::new(ascii);
    match screen {
        Screen::Help => help_lines(width, marks.ellipsis),
        Screen::Endeavor => endeavor::lines(session, width, &marks),
        // A grid or a list with a cursor, not rows that scroll: `research::draw`, `saves::draw`,
        // `nation::draw` and `msglog::draw` lay them out.
        Screen::Log
        | Screen::Research
        | Screen::Saves
        | Screen::Nation
        | Screen::Abilities
        | Screen::Traders
        | Screen::Airship => Vec::new(),
    }
}

/// How many rows the screen's text has at `width`. `ascii` never changes a row count (only which
/// glyph a mark or a bar's cells wear), so this always measures the Unicode set.
pub fn line_count(screen: Screen, session: &Session, width: usize) -> usize {
    lines(session, screen, width, false).len()
}

/// How many rows of a screen's own text show at once on a `page`: the key help gives its first
/// two to the tab strip and a blank row, the message log its first two to the filter row.
pub fn body_rows(screen: Screen, page: (i32, i32)) -> usize {
    let rows = text_size(page).1;
    match screen {
        Screen::Help => rows.saturating_sub(HELP_HEAD_ROWS),
        Screen::Log => rows.saturating_sub(msglog::HEAD_ROWS),
        _ => rows,
    }
}

/// The tone a toast wears: discoveries read as good news, the winter warning and a full stockpile
/// as cautions. The message log's lines share this (`crate::msglog`), where an alert in any kind
/// wears the warning colour.
pub fn kind_tone(kind: Kind) -> Tone {
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
        // Traders coming and going (#438): news, not a warning.
        Kind::Trade => Tone::Ok,
        // No research target with points banked (#656): idle capacity, a caution.
        Kind::Research => Tone::Warn,
    }
}

/// How many rows the key help's tab strip takes: the tabs, and a blank row under them.
pub const HELP_HEAD_ROWS: usize = 2;

/// The text width from which the key help sets its groups in two columns: a page 100 cells wide
/// (`page::text_size` takes the frame and its padding off). Narrower, it is one column that
/// scrolls.
pub const TWO_COLUMNS_FROM: usize = 96;

/// The cells between the two columns.
pub const COLUMN_GAP: usize = 2;

/// The key help's tabs, left to right, and the words on each.
pub const HELP_TABS: [(HelpTab, &str); 2] = [(HelpTab::Keys, "Keys"), (HelpTab::Legend, "Legend")];

/// What the tab strip says `Tab` does, flush right.
pub const TAB_NOTE: &str = "Tab switches";

/// The tab strip: `[Keys]  Legend`, the open tab in brackets and the title tone, and what `Tab`
/// does flush right.
pub fn help_tabs(tab: HelpTab) -> Row {
    let name = |(this, text): (HelpTab, &str)| {
        if this == tab { (format!("[{text}]"), Tone::Title) } else { (text.to_owned(), Tone::Dim) }
    };
    let [keys, legend] = HELP_TABS.map(name);
    let mut row = Row::spans(vec![keys, ("  ".to_owned(), Tone::Text), legend]);
    row.right = Some((TAB_NOTE.to_owned(), Tone::Dim));
    row
}

/// The key help's groups, each with its rows, in the order the help lists them: what a window
/// that lays the keys out itself (wrapping the wording as it likes) shows.
pub fn help_groups() -> Vec<(Group, Vec<HelpRow>)> {
    let rows = help_rows();
    Group::ALL
        .into_iter()
        .map(|g| (g, rows.iter().filter(|r| r.group == g).cloned().collect::<Vec<_>>()))
        .filter(|(_, rows)| !rows.is_empty())
        .collect()
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
        Screen::Abilities => "ABILITIES",
        Screen::Traders => "TRADERS",
        Screen::Airship => "AIRSHIP",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
