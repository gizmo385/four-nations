//! The Nation screen (`N`, #308, docs/design/side-panel.md "The Nation screen"): every settler
//! grouped by activity (`fn_ui::view::nation_view`'s own order, each group under its full word,
//! "HAULING 3", "EATING AND DRINKING 7"), a pointer that steps through them (arrows/`W`/`S`, a
//! page with `PgUp`/`PgDn`) and `Enter` selects the one under it; every store (with `full` after
//! one no stockpile can take more of) and the nation's traits (today's panel's own Modifiers,
//! moved here) sit under the roster, with no pointer of their own.
//!
//! The roster is a table under a header that stays put while the roster scrolls (it sits above
//! the scrolled window, not inside it, #660):
//!
//! ```text
//!   NAME  DOING                                  HUNGER  THIRST  ENERGY  HEALTH  BENDING
//! HAULING 3
//! ▸ Hua   Delivering 2 Fruit to the stockpile    ██░░░   ███░░   ██░░░   █████   Student, 4 bends
//! ```
//!
//! DOING takes every column the others leave, so a task is whole unless the screen is narrow;
//! one that still does not fit is cut at a word, and the detail strip under the rule shows it in
//! full, with the settler's progress to the next rank. The four needs are the game's one meter
//! (#654, `crate::widgets::meter`) at five cells: a low need wears the warning colour with the
//! `!` directly against it, and the selected row keeps those colours (`page::highlight` only
//! changes the background). Chi, bends and forms are one BENDING column in words
//! (`fn_ui::text::bending_words`), which is the first thing dropped when DOING would get too
//! narrow: at 80 columns every meter stays and BENDING goes.
//!
//! Full meters are solid `█`, so settlers' meters stacked in rows would read as one block (#779):
//! when the page can show every settler with a blank row between neighbours of a group, it does
//! (`spaced`); when it can't (80x24 with a handful of settlers) the rows stack as before, so no
//! settler is pushed off the page for it.
//!
//! Narrower than [`METER_MIN_WIDTH`] columns (never at a size the game draws at, since the
//! screen covers the side panel too, #652) a flag after the task names whichever needs aren't
//! ok instead (`fn_ui::view::need_word`).

use crate::page::{self, Page};
use crate::state::UiState;
use crate::widgets::{Marks, Painter, Row, Tone, window_start};
use fn_ui::Session;
use fn_ui::view::nation_view;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;

// The rows, cursors and keys are `fn_ui`'s (#857); this module draws them.
pub use fn_ui::screens::nation::*;

/// Draws the screen's contents into `page.body` (the frame's inside, minus its padding and the
/// detail strip): the header line, the column header (wide only, outside the roster's own scroll
/// so it never scrolls away), the roster (windowed round the pointer), every store, then the
/// nation's traits; and the settler under the pointer's detail in `page.detail`. `wide` says
/// whether the page is at least [`METER_MIN_WIDTH`] columns. The settler under the pointer wears
/// the selection across the page (`page::highlight`): a quiet background, never reverse video,
/// so its meters keep their own colours.
pub fn draw(
    session: &Session,
    ui: &UiState,
    wide: bool,
    palette: &crate::color::Palette,
    page: &Page,
    buf: &mut Buffer,
) -> Option<String> {
    let inner = page.body;
    let marks = Marks::new(palette.ascii);
    let view = nation_view(session);
    let width = usize::from(inner.width);
    let height = usize::from(inner.height);

    let store_lines = packed_rows(
        view.stores
            .iter()
            .map(|r| format!("{} {}{}", r.name, r.count, if r.full { " full" } else { "" })),
        width,
    );
    let trait_lines = packed_rows(view.traits.iter().map(|(l, v)| format!("{l} {v}")), width);
    let almanac_block = if view.almanac.is_empty() { 0 } else { 1 + 1 + view.almanac.len() };

    let store_block = 1 + store_lines.len().max(1);
    let trait_block = if trait_lines.is_empty() { 0 } else { 1 + 1 + trait_lines.len() };
    let footer_rows = 1 + store_block + almanac_block + trait_block;
    let header_rows = if wide { 3 } else { 2 };
    let roster_h = height.saturating_sub(header_rows + footer_rows).max(1);

    // A blank row between settlers keeps full meters apart, if every settler still fits (#779).
    // Only the wide rows have meters to fuse.
    let packed = roster_lines(&view, ui.nation_cursor, wide, false, width, &marks);
    let lines = if wide {
        let spaced = roster_lines(&view, ui.nation_cursor, wide, true, width, &marks);
        if spaced.len() <= roster_h { spaced } else { packed }
    } else {
        packed
    };
    let chosen_row =
        lines.iter().enumerate().filter(|(_, l)| l.unit).nth(ui.nation_cursor).map(|(i, _)| i);
    let first = window_start(chosen_row.unwrap_or(0), roster_h);

    let mut p = Painter::new(buf, inner, palette);
    p.row(&Row::pair(
        format!(
            "{} {} {} people {} seed {}",
            view.nation, marks.dot, view.population, marks.dot, view.seed
        ),
        Tone::Title,
        crate::widgets::clock(&view.clock, &marks),
        Tone::Dim,
    ));
    p.blank();
    if wide {
        p.row(&header(&Columns::of(&view, width)));
    }
    let mut chosen_at = None;
    for (i, line) in lines.iter().enumerate().skip(first).take(roster_h) {
        if Some(i) == chosen_row {
            chosen_at = Some(inner.y + p.row);
        }
        p.row(&line.row);
    }

    p.row = (header_rows + roster_h) as u16;
    p.blank();
    p.row(&Row::text("STORES", Tone::Title));
    if store_lines.is_empty() {
        p.row(&Row::text("Nothing yet", Tone::Dim));
    } else {
        for row in &store_lines {
            p.row(row);
        }
    }
    if !view.almanac.is_empty() {
        p.blank();
        p.row(&Row::text("ALMANAC", Tone::Title));
        for line in &view.almanac {
            p.row(&Row::text(line.clone(), Tone::Text));
        }
    }
    if !trait_lines.is_empty() {
        p.blank();
        p.row(&Row::text("TRAITS", Tone::Title));
        for row in &trait_lines {
            p.row(row);
        }
    }

    if let Some(y) = chosen_at.filter(|&y| y < inner.bottom()) {
        page::highlight(p.buf, Rect::new(inner.x, y, inner.width, 1), palette);
    }

    if let (Some(strip), Some(&id)) = (page.detail, roster(&view).get(ui.nation_cursor)) {
        let rows = detail(session, id, usize::from(strip.height), usize::from(strip.width), &marks);
        let mut d = Painter::new(buf, strip, palette);
        for row in &rows {
            d.row(row);
        }
    }

    // Where the roster's window sits, in settlers (the headings between groups don't count).
    let settlers = |lines: &[Line]| lines.iter().filter(|l| l.unit).count();
    let end = (first + roster_h).min(lines.len());
    fn_ui::text::window_label(
        "settlers",
        settlers(&lines[..first.min(lines.len())]),
        settlers(&lines[first.min(end)..end]),
        settlers(&lines),
    )
}
