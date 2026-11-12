//! The full screens (`crate::page`, #652), each covering the map and the side panel: the
//! key help (`?`) and the Endeavor screen (`G`, `endeavor.rs`) are pages of rows that scroll; the
//! message log (`L`, #661, `msglog.rs`) is a list of lines with a cursor under a filter row; the
//! research screen (`T`) is a grid of tech cards with a cursor and a detail
//! strip (`research.rs`); the Nation screen (`N`, #308, `nation.rs`) is a roster with a pointer;
//! the saves screen (`F6`, `saves.rs`) is a list with a pointer; the abilities screen (`I`, #719,
//! `abilities.rs`) is a list with a pointer and a detail strip. The Build picker (`U`,
//! `build.rs`) wears the same frame. Each closes on `Esc` or on the key that opened it.

use fn_ui::Session;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;

use crate::abilities;
use crate::color::{ColorDepth, Palette};
use crate::legend;
use crate::page;
use crate::research;
use crate::saves;
use crate::state::{HelpTab, Screen, UiState};
use crate::widgets::{Marks, Painter};

// What the screens say and how far they scroll is `fn_ui`'s (#857); this module draws them.
pub use fn_ui::screens::{
    COLUMN_GAP, HELP_HEAD_ROWS, LegendRows, TWO_COLUMNS_FROM, body_rows, help_lines, help_tabs,
    kind_tone, line_count, lines, no_legend, text_size, title,
};

/// How many rows the key help's Legend tab has at `width`: the legend of the map in view.
/// `App::legend_rows` is set to this by the terminal frontend (`crate::shell::Shell`).
pub fn legend_rows(session: &Session, ui: &UiState, width: usize) -> usize {
    // Which entries there are never depends on the palette, only how they draw.
    let palette = Palette::new(&session.defs, session.nation(), ColorDepth::TrueColor, false);
    legend::line_count(session, ui, &palette, width)
}

/// How many rows the open screen scrolls through at `width`: [`line_count`], except that the key
/// help counts whichever tab shows (the legend is built from the map in view).
pub fn row_count(session: &Session, ui: &UiState, screen: Screen, width: usize) -> usize {
    fn_ui::screens::row_count(session, ui, screen, width, legend_rows)
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
    let detail_rows = match screen {
        Screen::Research => {
            let width = page::text_size((i32::from(area.width), i32::from(area.height))).0;
            research::detail_height(session, ui.research_page, ui.tech, width)
        }
        Screen::Nation => crate::nation::detail_rows(area.height),
        Screen::Abilities => {
            let width = page::text_size((i32::from(area.width), i32::from(area.height))).0;
            let rows = fn_ui::bend::abilities(session);
            abilities::detail_height(&rows, width, palette.ascii, area.height)
        }
        _ => 0,
    };
    let page = page::draw(buf, area, palette, title(screen), detail_rows);
    let position = match screen {
        Screen::Research => {
            let position = research::draw(session, ui.research_page, ui.tech, palette, &page, buf);
            if let Some(tech) = ui.school_confirm {
                research::draw_confirm(session, tech, palette, area, buf);
            }
            position
        }
        Screen::Saves => {
            // A name being typed (#704) takes the body's last row; the list gets the rest.
            let body = page.body;
            let (list, name_row) = match &ui.save_name {
                Some(typed) if body.height > 1 => {
                    let list = Rect { height: body.height - 1, ..body };
                    let row = Rect { y: body.bottom() - 1, height: 1, ..body };
                    (list, Some((typed, row)))
                }
                _ => (body, None),
            };
            let position =
                saves::draw(&ui.save_slots, ui.save_cursor, &session.defs, palette, list, buf);
            if let Some((typed, row)) = name_row {
                let suggestion = fn_ui::saves::new_name(&ui.save_slots);
                saves::draw_name(typed, &suggestion, palette, row, buf);
            }
            position
        }
        Screen::Nation => {
            // Whether the page is wide enough for the meters (docs/design/side-panel.md "The
            // Nation screen").
            let wide = area.width >= crate::nation::METER_MIN_WIDTH;
            crate::nation::draw(session, ui, wide, palette, &page, buf)
        }
        Screen::Abilities => {
            let rows = fn_ui::bend::abilities(session);
            abilities::draw(&rows, ui.ability_cursor, palette, &page, buf)
        }
        Screen::Traders => {
            let view = fn_ui::trade::view(session);
            crate::traders::draw(&view, ui.trade_cursor, palette, &page, buf);
            None
        }
        Screen::Help => draw_help(session, ui, palette, page.body, buf),
        Screen::Log => crate::msglog::draw(session, ui, palette, &page, buf),
        Screen::Endeavor => {
            let body = page.body;
            let rows = lines(session, screen, usize::from(body.width), palette.ascii);
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
