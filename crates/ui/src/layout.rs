//! How big the parts of a character-cell screen are (docs/design/tui.md, "Screen layout"): the map
//! on the left, the side panel on the right (32 columns, 36 on a wide terminal, #423) and a status
//! line under both. A full screen (`crate::page`, #652) covers the map and the side panel
//! together: every column, every row above the status line.
//!
//! Only the sizes are here, in cells, because the game's state needs them (how many tiles the
//! map shows, how big a page is): `fn_tui::layout` turns them into rectangles. A frontend that
//! isn't a grid of cells (the window of `fn_gui`) still hands the game the size of its map and
//! page in tiles and rows.

/// The terminal width (inclusive) from which the side panel widens to [`PANEL_WIDTH_WIDE`]
/// columns (#423, docs/design/side-panel-v2.md "Width").
pub const WIDE_TERMINAL: u16 = 140;

/// The side panel's width in columns below [`WIDE_TERMINAL`].
pub const PANEL_WIDTH: u16 = 32;

/// The side panel's width in columns at [`WIDE_TERMINAL`] or wider (#423).
pub const PANEL_WIDTH_WIDE: u16 = 36;

/// The side panel's width for a terminal `width` columns wide: [`PANEL_WIDTH_WIDE`] at
/// [`WIDE_TERMINAL`] or wider, else [`PANEL_WIDTH`].
pub fn panel_width(width: u16) -> u16 {
    if width >= WIDE_TERMINAL { PANEL_WIDTH_WIDE } else { PANEL_WIDTH }
}

/// The smallest terminal the layout works in. Below it the frame is only a notice.
pub const MIN_WIDTH: u16 = 80;
pub const MIN_HEIGHT: u16 = 24;

/// Whether a `width` x `height` terminal is big enough to lay out.
pub fn fits(width: u16, height: u16) -> bool {
    width >= MIN_WIDTH && height >= MIN_HEIGHT
}

/// How big a full screen's page is on a `width` x `height` terminal, as (columns, rows): 0 x 0
/// when it's too small to draw anything. Every column, and every row above the status line.
pub fn page_size(width: u16, height: u16) -> (i32, i32) {
    if !fits(width, height) {
        return (0, 0);
    }
    (i32::from(width), i32::from(height - 1))
}

/// How many tiles fit across and down the map region of a `width` x `height` terminal (0 x 0 when
/// it's too small to draw anything).
pub fn viewport(width: u16, height: u16) -> (i32, i32) {
    if !fits(width, height) {
        return (0, 0);
    }
    (i32::from(width - panel_width(width)), i32::from(height - 1))
}
