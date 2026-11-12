//! The research screen (`T`, docs/design/research.md): the tech tree as columns by depth, one card
//! per tech. What it shows is `fn_ui::research::screen_view` (the visibility rule, which techs a
//! nation sees, is the sim's `Defs::techs_for`). A card has the name, the state, the cost and a
//! summary of what it unlocks. `Enter` issues `Command::SetResearch`; `fn_sim` queues the whole
//! prereq chain of a locked tech, and a queued prereq is drawn the same as the target.
//!
//! The cursor is a (column, row) pair in the view. The window onto the grid (which columns and
//! which cards show) follows from the cursor and the size alone, so a frame is a pure function of
//! the state like every other.

use crate::color::Palette;
use crate::page::{self, Page};
use crate::widgets::{Marks, Painter, Row, Tone, ToneStyle, window_start};
use fn_sim::research::TechId;
use fn_ui::Session;
use fn_ui::research::{ResearchPage, TechCard, has_newcomers, screen_page};
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;

// The rows, cursors and keys are `fn_ui`'s (#857); this module draws them.
pub use fn_ui::screens::research::*;

/// Draws the CHOOSE A SCHOOL box for `tech` centred over `area` (the page).
pub fn draw_confirm(
    session: &Session,
    tech: TechId,
    palette: &Palette,
    area: Rect,
    buf: &mut Buffer,
) {
    let width = CONFIRM_WIDTH as u16 + 4;
    let text = usize::from(width.saturating_sub(4).min(area.width.saturating_sub(8)).max(1));
    let marks = Marks::new(palette.ascii);
    let mut rows = confirm_body(session, tech, &marks, text);
    // The keys, centred.
    let keys = confirm_lines(session, tech).pop().unwrap_or_default();
    let pad = text.saturating_sub(keys.chars().count()) / 2;
    rows.push(Row::text(format!("{}{keys}", " ".repeat(pad)), Tone::Accent));
    let height = u16::try_from(rows.len() + 2).unwrap_or(u16::MAX);
    let inner = crate::modal::draw(buf, area, (width, height), palette, CONFIRM_TITLE, "");
    let mut p = Painter::new(buf, inner, palette);
    for row in &rows {
        p.row(row);
    }
}

/// Draws the screen's contents into `page` (the grid in its body, the chosen tech in its detail
/// strip) and returns where the visible tiers sit ("tiers 1-4 of 5") when some are out of sight,
/// for the frame's bottom edge (#651).
pub fn draw(
    session: &Session,
    shown_page: ResearchPage,
    at: Cursor,
    palette: &Palette,
    page: &Page,
    buf: &mut Buffer,
) -> Option<String> {
    let view = screen_page(session, shown_page);
    let marks = Marks::new(palette.ascii);
    // Once the gate stands the body opens with the page tabs and a blank row.
    let tabs = has_newcomers(session);
    let tab_rows = if tabs { TAB_ROWS.min(usize::from(page.body.height)) as u16 } else { 0 };
    if tabs {
        let strip = Rect::new(page.body.x, page.body.y, page.body.width, tab_rows.min(1));
        Painter::new(buf, strip, palette).row(&page_tabs(shown_page));
    }
    let inner =
        Rect { y: page.body.y + tab_rows, height: page.body.height - tab_rows, ..page.body };
    let grid = Grid::new(
        (
            usize::from(page.inner.width),
            usize::from(page.inner.height).saturating_sub(usize::from(tab_rows)),
        ),
        view.columns.len(),
    );
    let first_column = window_start(at.0, grid.columns);
    let grid_height = usize::from(inner.height.saturating_sub(1));

    // Every card of the columns in view, wrapped to the column: a grid row is as tall as its
    // tallest card, so the cards of a row line up across columns.
    let shown: Vec<(usize, &Vec<TechCard>)> =
        view.columns.iter().enumerate().skip(first_column).take(grid.columns).collect();
    let cards: Vec<Vec<Vec<Row>>> = shown
        .iter()
        .map(|&(c, column)| {
            column
                .iter()
                .enumerate()
                .map(|(r, card)| card_rows(card, c == at.0 && r == at.1, &marks, grid.column_width))
                .collect()
        })
        .collect();
    let height = |r: usize| cards.iter().filter_map(|col| col.get(r)).map(Vec::len).max();
    // The window onto the grid rows slides only once the cursor's row passes the bottom, and
    // shows whole rows, but always the cursor's.
    let mut first_card = at.1;
    let mut used = height(at.1).unwrap_or(0);
    while first_card > 0 {
        let above = height(first_card - 1).unwrap_or(0);
        if used + 1 + above > grid_height {
            break;
        }
        first_card -= 1;
        used += 1 + above;
    }
    let mut tops = Vec::new();
    let mut y = 0;
    let mut r = first_card;
    while let Some(h) = height(r) {
        if r > at.1 && y + h > grid_height {
            break;
        }
        tops.push((r, y));
        y += h + 1;
        r += 1;
    }
    let shown_rows = tops.len();

    for (slot, &(c, column)) in shown.iter().enumerate() {
        let x = inner.x + (slot * (grid.column_width + GAP)) as u16;
        let width = grid.column_width as u16;
        let chosen_column = c == at.0;

        // The column header: its depth, and arrows where cards are out of sight.
        let mut head = Painter::new(buf, Rect::new(x, inner.y, width, 1), palette);
        let flags = format!(
            "{}{}",
            if first_card > 0 { marks.up } else { "" },
            if column.len() > first_card + shown_rows { marks.down } else { "" },
        );
        let title_tone = if chosen_column { Tone::Title } else { Tone::Dim };
        head.row(&Row::pair(column_title(&view, c), title_tone, flags, Tone::Dim));

        let body = Rect::new(x, inner.y + 1, width, grid_height as u16);
        for &(r, top) in &tops {
            let Some(rows) = cards[slot].get(r) else { continue };
            let area =
                Rect::new(x, body.y + top as u16, width, rows.len() as u16).intersection(body);
            let mut p = Painter::new(buf, area, palette);
            for row in rows {
                p.row(row);
            }
            // The card under the cursor wears the selection's quiet background over its rows.
            if chosen_column && r == at.1 {
                page::highlight(buf, area, palette);
            }
        }
    }

    // Arrows at the frame's edge when columns are out of sight.
    let mut arrow = |x: u16, text: &str| {
        buf.set_string(x, inner.y, text, Tone::Accent.style(palette));
    };
    if first_column > 0 {
        arrow(inner.x - 1, marks.left);
    }
    if first_column + grid.columns < view.columns.len() {
        arrow(inner.right(), marks.right);
    }

    // The details of the tech under the cursor, in the frame's detail strip.
    if let (Some(card), Some(strip)) = (card_at(&view, at), page.detail) {
        let rows = detail_rows(session, card, &marks, usize::from(strip.width));
        let mut p = Painter::new(buf, strip, palette);
        for row in &rows {
            p.row(row);
        }
    }
    if shown_page == ResearchPage::Newcomers {
        // The other pages are nations; what is worth counting at the edge is the fusions.
        return fn_ui::text::window_label(
            "nations",
            first_column,
            grid.columns,
            view.columns.len(),
        )
        .or_else(|| {
            let n = view.columns.last().map_or(0, Vec::len);
            (n > 0).then(|| format!("{n} {}", if n == 1 { "fusion" } else { "fusions" }))
        });
    }
    fn_ui::text::window_label("tiers", first_column, grid.columns, view.columns.len())
}
