//! The screens that open over the map area (the side panel stays): the message log (`L`), the
//! key help (`?`) and the Endeavor screen (`G`, `endeavor.rs`) are framed pages of rows that
//! scroll; the research screen (`T`) is a framed grid of tech cards with a cursor
//! (`research.rs`); the Nation screen (`N`, #308, `nation.rs`) is a framed roster with a pointer.

use fn_ui::Session;
use fn_ui::alerts::{Alerts, Kind};
use fn_ui::view::format_clock;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;

use crate::color::Palette;
use crate::input::{Group, help_rows};
use crate::research;
use crate::saves;
use crate::state::{Screen, UiState};
use crate::widgets::{Marks, Painter, Row, TitleAt, Tone, draw_frame, draw_position, wrap};

/// The text area of a screen drawn over a `(width, height)` map region: inside the frame and one
/// column of padding, as (columns, rows).
pub fn text_size(region: (i32, i32)) -> (usize, usize) {
    let (w, h) = region;
    ((w - 4).max(0) as usize, (h - 2).max(0) as usize)
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

/// Draws the open screen into `area` (the map region): the frame, then the rows from the scroll.
pub fn draw_screen(
    session: &Session,
    ui: &UiState,
    screen: Screen,
    palette: &Palette,
    area: Rect,
    buf: &mut Buffer,
) {
    let marks = crate::widgets::Marks::new(palette.ascii);
    let title = match screen {
        Screen::Log => "MESSAGE LOG",
        Screen::Help => "KEY HELP",
        Screen::Endeavor => "ENDEAVOR",
        Screen::Research => "RESEARCH",
        Screen::Saves => "SAVES",
        Screen::Nation => "NATION",
    };
    let title = format!(" {0} {title} {0} ", marks.seal);
    // The bottom edge carries no keys, only how far down a long screen is (#651).
    draw_frame(buf, area, palette, &title, TitleAt::Centre, "");
    let inner = Rect::new(area.x + 2, area.y + 1, area.width - 4, area.height - 2);
    if screen == Screen::Research {
        let position = research::draw(session, ui.tech, palette, inner, buf);
        draw_position(buf, area, palette, position.as_deref().unwrap_or(""));
        return;
    }
    if screen == Screen::Saves {
        let position =
            saves::draw(&ui.save_slots, ui.save_cursor, &session.defs, palette, inner, buf);
        draw_position(buf, area, palette, position.as_deref().unwrap_or(""));
        return;
    }
    if screen == Screen::Nation {
        // Whether the map region (`area`, not `inner`) is wide enough for the meters
        // (docs/design/side-panel.md "The Nation screen"); the panel sits beside it, so this is
        // not the screen's own frame width.
        let wide = area.width >= crate::nation::METER_MIN_WIDTH;
        let position = crate::nation::draw(session, ui, wide, palette, inner, buf);
        draw_position(buf, area, palette, position.as_deref().unwrap_or(""));
        return;
    }
    let rows = lines(session, screen, &ui.alerts, usize::from(inner.width), palette.ascii);
    let max = rows.len().saturating_sub(usize::from(inner.height));
    let start = ui.scroll.min(max);
    let mut p = Painter::new(buf, inner, palette);
    for row in rows.iter().skip(start) {
        p.row(row);
    }
    let shown = usize::from(inner.height);
    let position = fn_ui::text::window_label("", start, shown, rows.len());
    draw_position(buf, area, palette, position.as_deref().unwrap_or(""));
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
