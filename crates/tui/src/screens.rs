//! The full screens (`crate::page`, #652), each covering the map and the side panel: the message
//! log (`L`), the key help (`?`) and the Endeavor screen (`G`, `endeavor.rs`) are pages of rows
//! that scroll; the research screen (`T`) is a grid of tech cards with a cursor and a detail
//! strip (`research.rs`); the Nation screen (`N`, #308, `nation.rs`) is a roster with a pointer;
//! the saves screen (`F6`, `saves.rs`) is a list with a pointer. The Build picker (`U`,
//! `build.rs`) wears the same frame. Each closes on `Esc` or on the key that opened it.

use fn_ui::Session;
use fn_ui::alerts::{Alerts, Kind};
use fn_ui::view::format_clock;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;

use crate::color::Palette;
use crate::input::{Group, help_rows};
use crate::page;
use crate::research;
use crate::saves;
use crate::state::{Screen, UiState};
use crate::widgets::{Marks, Painter, Row, Tone, wrap};

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
            log_lines(&history, width, marks.ellipsis)
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

/// The message log, oldest first: the time it happened and what, wrapped under the time.
pub fn log_lines(history: &[(u64, Kind, &str)], width: usize, ellipsis: char) -> Vec<Row> {
    if history.is_empty() {
        let none = "Nothing has happened yet: no bends, no discoveries, no deaths.";
        return wrap(none, width, usize::MAX, ellipsis)
            .into_iter()
            .map(|line| Row::text(line, Tone::Dim))
            .collect();
    }
    // "MM:SS " is 6 cells; a game past 99 minutes grows it, so size the column to the widest.
    let stamps: Vec<String> = history.iter().map(|&(tick, ..)| format_clock(tick)).collect();
    let stamp_width = stamps.iter().map(|s| s.chars().count()).max().unwrap_or(5);
    let indent = stamp_width + 1;
    let room = width.saturating_sub(indent).max(1);
    let mut rows = Vec::new();
    for (&(_, kind, text), stamp) in history.iter().zip(&stamps) {
        let tone = kind_tone(kind);
        let text = wrap(text, room, usize::MAX, ellipsis);
        for (i, part) in text.into_iter().enumerate() {
            let lead = if i == 0 { format!("{stamp:>stamp_width$} ") } else { " ".repeat(indent) };
            rows.push(Row::spans(vec![(lead, Tone::Dim), (part, tone)]));
        }
    }
    rows
}

/// The key help: the groups of [`help_rows`], each key list beside what it does.
pub fn help_lines(width: usize, ellipsis: char) -> Vec<Row> {
    let rows = help_rows();
    let longest = rows.iter().map(|r| r.keys.chars().count()).max().unwrap_or(0);
    let keys_width = longest.min(width * 2 / 5).max(1);
    let what_width = width.saturating_sub(keys_width + 2).max(1);
    let mut out = Vec::new();
    for group in Group::ALL {
        if !rows.iter().any(|r| r.group == group) {
            continue;
        }
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
        Screen::Log | Screen::Help | Screen::Endeavor => {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_log_puts_the_time_first_and_wraps_under_it() {
        let history = [
            (0, Kind::Bend, "Haru used Raise Earth at (5, 3)"),
            (650, Kind::Death, "Tyro died of dehydration, far from any water"),
        ];
        let rows = log_lines(&history, 24, '…');
        let text: Vec<String> = rows.iter().map(Row::plain).collect();
        assert_eq!(text[0], "00:00 Haru used Raise");
        assert_eq!(text[1], "      Earth at (5, 3)");
        assert!(text[2].starts_with("01:05 Tyro died of"), "{text:?}");
        assert!(text.iter().all(|t| t.chars().count() <= 24), "{text:?}");
        // The kinds wear their tones.
        assert_eq!(rows[0].left[1].1, Tone::Text);
        assert_eq!(rows[2].left[1].1, Tone::Bad);
    }

    #[test]
    fn an_empty_log_says_so() {
        let rows = log_lines(&[], 40, '…');
        assert!(rows[0].plain().starts_with("Nothing has happened"));
    }

    #[test]
    fn the_help_has_a_heading_per_group_and_keeps_to_its_width() {
        for width in [40, 44, 70] {
            let rows = help_lines(width, '…');
            let text: Vec<String> = rows.iter().map(Row::plain).collect();
            for g in Group::ALL {
                assert!(text.contains(&g.title().to_uppercase()), "{width}: {g:?}");
            }
            assert!(text.iter().all(|t| t.chars().count() <= width), "{width}: {text:?}");
            assert!(text.iter().any(|t| t.contains("Harvest tool")), "{width}");
        }
    }
}
