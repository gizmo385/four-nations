//! Where things go on the screen (docs/design/tui.md, "Screen layout"): the map on the left, the
//! side panel on the right (32 columns, 36 on a wide terminal, #423) and a status line under both.
//! A full screen (`crate::page`, #652) covers the map and the side panel together: every column,
//! every row above the status line.

use ratatui::layout::Rect;

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

/// The three regions of a screen of at least the minimum size, and the page a full screen
/// covers.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Regions {
    /// Where map tiles draw, one cell per tile.
    pub map: Rect,
    pub panel: Rect,
    pub status: Rect,
    /// The map and the panel together (`crate::page`, #652): where a full screen draws, so
    /// neither shows while one is open.
    pub page: Rect,
}

/// The regions of `area`, or `None` when it's below 80x24.
pub fn regions(area: Rect) -> Option<Regions> {
    if area.width < MIN_WIDTH || area.height < MIN_HEIGHT {
        return None;
    }
    let body_height = area.height - 1;
    let panel_width = panel_width(area.width);
    let map_width = area.width - panel_width;
    Some(Regions {
        map: Rect::new(area.x, area.y, map_width, body_height),
        panel: Rect::new(area.x + map_width, area.y, panel_width, body_height),
        status: Rect::new(area.x, area.y + body_height, area.width, 1),
        page: Rect::new(area.x, area.y, area.width, body_height),
    })
}

/// How big a full screen's page is on a `width` x `height` terminal, as (columns, rows): 0 x 0
/// when it's too small to draw anything.
pub fn page_size(width: u16, height: u16) -> (i32, i32) {
    regions(Rect::new(0, 0, width, height))
        .map_or((0, 0), |r| (i32::from(r.page.width), i32::from(r.page.height)))
}

/// How many tiles fit across and down the map region of a `width` x `height` terminal (0 x 0 when
/// it's too small to draw anything).
pub fn viewport(width: u16, height: u16) -> (i32, i32) {
    regions(Rect::new(0, 0, width, height))
        .map_or((0, 0), |r| (i32::from(r.map.width), i32::from(r.map.height)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_regions_tile_the_screen() {
        let r = regions(Rect::new(0, 0, 120, 40)).unwrap();
        assert_eq!(r.map, Rect::new(0, 0, 88, 39));
        assert_eq!(r.panel, Rect::new(88, 0, 32, 39));
        assert_eq!(r.status, Rect::new(0, 39, 120, 1));
        assert_eq!(r.page, Rect::new(0, 0, 120, 39));
        assert_eq!(viewport(120, 40), (88, 39));
        assert_eq!(page_size(120, 40), (120, 39));
        assert_eq!(page_size(80, 24), (80, 23));
    }

    /// The panel stays 32 columns at 139 columns wide, and widens to 36 at 140 (#423,
    /// docs/design/side-panel-v2.md "Width"): the map takes whatever the panel gives up.
    #[test]
    fn the_panel_widens_at_a_hundred_and_forty_columns() {
        let narrow = regions(Rect::new(0, 0, 139, 40)).unwrap();
        assert_eq!(narrow.panel.width, 32);
        assert_eq!(narrow.map.width, 139 - 32);
        assert_eq!(panel_width(139), 32);

        let wide = regions(Rect::new(0, 0, 140, 40)).unwrap();
        assert_eq!(wide.panel.width, 36);
        assert_eq!(wide.panel, Rect::new(140 - 36, 0, 36, 39));
        assert_eq!(wide.map.width, 140 - 36);
        assert_eq!(panel_width(140), 36);
    }

    #[test]
    fn eighty_by_twenty_four_is_the_smallest() {
        assert!(regions(Rect::new(0, 0, 80, 24)).is_some());
        assert!(regions(Rect::new(0, 0, 79, 24)).is_none());
        assert!(regions(Rect::new(0, 0, 80, 23)).is_none());
        assert_eq!(viewport(79, 40), (0, 0));
    }
}
