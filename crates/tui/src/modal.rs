//! A reusable modal (#285): a centred, bordered box in the Seal frame style that takes the keys
//! until it closes. It draws over the map area the way a `Screen` does, but as a box, centred,
//! rather than filling it, and (when it has one) splits its inside into a list on the left and a
//! detail pane on the right ([`split`]). The Build picker (`crate::build`) is its first user; the
//! save list and future pickers can reuse this frame (`docs/design/tui.md`).

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;

use crate::color::Palette;
use crate::widgets::{TitleAt, draw_frame};

/// How far in from the edges of `area` (the map region) the modal's box sits at least, so it
/// always reads as a dialog floating over the map rather than another full page.
const MARGIN: u16 = 2;

/// The blank columns [`split`] leaves between the list and the detail pane, as the selection
/// bar's own two columns do (`crate::bar`).
const GAP: u16 = 2;

/// The modal's box: centred in `area`, `target` sized if `area` has room for it with [`MARGIN`]
/// to spare all round, shrunk to fit otherwise. Every size the frame draws at (80x24 and up,
/// `docs/design/tui.md`) leaves a usable box, however small `target` has to shrink.
pub fn modal_area(area: Rect, target: (u16, u16)) -> Rect {
    let width = target.0.min(area.width.saturating_sub(MARGIN * 2)).max(1);
    let height = target.1.min(area.height.saturating_sub(MARGIN * 2)).max(1);
    let x = area.x + (area.width - width) / 2;
    let y = area.y + (area.height - height) / 2;
    Rect::new(x, y, width, height)
}

/// Draws the modal's frame (the Seal box, its title and footer) into `outer`'s centred
/// [`modal_area`] and returns the inside, minus a column and a row of padding, for the caller to
/// fill.
pub fn draw(
    buf: &mut Buffer,
    outer: Rect,
    target: (u16, u16),
    palette: &Palette,
    title: &str,
    footer: &str,
) -> Rect {
    let area = modal_area(outer, target);
    draw_frame(buf, area, palette, title, TitleAt::Centre, footer);
    Rect::new(area.x + 2, area.y + 1, area.width.saturating_sub(4), area.height.saturating_sub(2))
}

/// A modal's inside, split into a list (left) and a detail pane (right).
pub struct Split {
    pub list: Rect,
    pub detail: Rect,
}

/// Splits `inner` into a list column at most `list_width` wide (left) and a detail column
/// (right), with [`GAP`] blank columns of padding between them.
pub fn split(inner: Rect, list_width: u16) -> Split {
    let list_width = list_width.min(inner.width.saturating_sub(GAP + 2)).max(1);
    let list = Rect::new(inner.x, inner.y, list_width, inner.height);
    let detail_x = inner.x + list_width + GAP;
    let detail_width = inner.width.saturating_sub(list_width + GAP);
    let detail = Rect::new(detail_x, inner.y, detail_width, inner.height);
    Split { list, detail }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_box_is_centred_and_shrinks_to_fit_with_the_margin_kept() {
        let big = modal_area(Rect::new(0, 0, 88, 39), (60, 18));
        assert_eq!(big, Rect::new(14, 10, 60, 18));
        // 80x24's map region (48x23): too narrow and short for 60x18, so it shrinks.
        let small = modal_area(Rect::new(0, 0, 48, 23), (60, 18));
        assert_eq!(small, Rect::new(2, 2, 44, 18));
        // Smaller still, it never grows past what's left once the margin is kept.
        let tiny = modal_area(Rect::new(0, 0, 10, 10), (60, 18));
        assert_eq!(tiny, Rect::new(2, 2, 6, 6));
    }

    #[test]
    fn the_split_keeps_a_gap_between_list_and_detail() {
        let s = split(Rect::new(10, 10, 50, 10), 20);
        assert_eq!(s.list, Rect::new(10, 10, 20, 10));
        assert_eq!(s.detail, Rect::new(32, 10, 28, 10));
    }

    #[test]
    fn a_list_wider_than_the_inside_is_clamped() {
        let s = split(Rect::new(0, 0, 20, 5), 100);
        assert_eq!(s.list.width, 16);
        assert_eq!(s.list.width + GAP + s.detail.width, 20);
    }
}
