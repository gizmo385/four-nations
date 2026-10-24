//! Where things go on the screen (docs/design/tui.md, "Screen layout"): the map on the left, the
//! 32-column side panel on the right and a status line under both.

use ratatui::layout::Rect;

/// The side panel's width in columns.
pub const PANEL_WIDTH: u16 = 32;

/// The smallest terminal the layout works in. Below it the frame is only a notice.
pub const MIN_WIDTH: u16 = 80;
pub const MIN_HEIGHT: u16 = 24;

/// The three regions of a screen of at least the minimum size.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Regions {
    /// Where map tiles draw, one cell per tile.
    pub map: Rect,
    pub panel: Rect,
    pub status: Rect,
}

/// The regions of `area`, or `None` when it's below 80x24.
pub fn regions(area: Rect) -> Option<Regions> {
    if area.width < MIN_WIDTH || area.height < MIN_HEIGHT {
        return None;
    }
    let body_height = area.height - 1;
    let map_width = area.width - PANEL_WIDTH;
    Some(Regions {
        map: Rect::new(area.x, area.y, map_width, body_height),
        panel: Rect::new(area.x + map_width, area.y, PANEL_WIDTH, body_height),
        status: Rect::new(area.x, area.y + body_height, area.width, 1),
    })
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
        assert_eq!(viewport(120, 40), (88, 39));
    }

    #[test]
    fn eighty_by_twenty_four_is_the_smallest() {
        assert!(regions(Rect::new(0, 0, 80, 24)).is_some());
        assert!(regions(Rect::new(0, 0, 79, 24)).is_none());
        assert!(regions(Rect::new(0, 0, 80, 23)).is_none());
        assert_eq!(viewport(79, 40), (0, 0));
    }
}
