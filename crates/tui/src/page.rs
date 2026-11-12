//! The full-screen frame (#652, docs/design/tui.md "Full screens"): one shape for every screen
//! that takes over the game, the Nation, Abilities, Research, Endeavor, Message log, Key help,
//! Saves and Build screens. It covers the map and the side panel together, every column and every row
//! above the hint bar (`crate::layout::Regions::page`), so a screen gets the whole terminal's
//! width:
//!
//! ```text
//! ┏───────────────── ◆ RESEARCH ◆ ─────────────────┓   the title, centred on the top edge
//! │ TIER 1            TIER 2           TIER 3      │
//! │ ...                                            │   the body: the screen's own content
//! ├────────────────────────────────────────────────┤   a rule joined to the frame
//! │ Carpentry                   Available · 60 pts │   the detail strip (optional): the
//! │ Saws, joints and the workbench: build in wood. │   highlighted row, in more words
//! ┗────────────────────────────── tiers 1-3 of 5 ─┛   the position, and nothing else
//! ```
//!
//! [`draw`] draws the frame and hands back a [`Page`]: where the body goes, where the detail
//! strip goes (when the screen asked for one), and [`Page::position`] for the bottom edge.
//! The bottom edge carries how far down a long screen is ("12-48 of 61",
//! `fn_ui::text::window_label`) and never key hints: those are the hint bar's alone (#651).
//!
//! A selected row or card is marked by [`Marks::pointer`] and [`highlight`]: a quiet background
//! (`Theme::select`) with brighter (bold) text, never reverse video, so a meter or a warning
//! inside it keeps its own colour.
//!
//! The game menu and the confirm boxes are not full screens: they stay centred boxes over the
//! map (`crate::menu`, `crate::ask`).

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier};

use crate::color::Palette;
use crate::widgets::{Marks, TitleAt, Tone, ToneStyle, draw_frame, draw_position};

/// A full screen's parts, once its frame is drawn.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Page {
    /// The frame itself, border included: the page `draw` was given.
    pub area: Rect,
    /// Inside the border and a column of padding each side: the body, the rule and the detail
    /// strip together.
    pub inner: Rect,
    /// Where the screen's own content goes.
    pub body: Rect,
    /// The detail strip under the rule, as wide as the body, when the screen asked for one.
    pub detail: Option<Rect>,
}

impl Page {
    /// Writes `label` on the bottom edge, flush right (`┗──── 12-48 of 61 ─┛`); nothing for
    /// `None`, a screen whose rows all fit.
    pub fn position(&self, buf: &mut Buffer, palette: &Palette, label: Option<&str>) {
        draw_position(buf, self.area, palette, label.unwrap_or(""));
    }
}

// The size of a page's text is `fn_ui`'s, because the game's scrolling needs it (#857).
pub use fn_ui::page::text_size;

/// Draws the frame over `area` (the whole page, `crate::layout::Regions::page`): the panel
/// colour over every cell, the Seal border, `title` upper case between two seals on the top edge,
/// and, when `detail_rows` is more than 0, a rule joined to the side lines with that many rows
/// under it for the detail strip. The strip shrinks if the page is too short to leave the body a
/// row. The bottom edge is left for [`Page::position`].
pub fn draw(
    buf: &mut Buffer,
    area: Rect,
    palette: &Palette,
    title: &str,
    detail_rows: u16,
) -> Page {
    let marks = Marks::new(palette.ascii);
    let heading = format!(" {0} {1} {0} ", marks.seal, title.to_uppercase());
    draw_frame(buf, area, palette, &heading, TitleAt::Centre, "");
    let inner = Rect::new(
        area.x + 2,
        area.y + 1,
        area.width.saturating_sub(4),
        area.height.saturating_sub(2),
    );
    // The body keeps at least one row; the rule takes one more.
    let detail_rows = detail_rows.min(inner.height.saturating_sub(2));
    if detail_rows == 0 {
        return Page { area, inner, body: inner, detail: None };
    }
    let rule_y = inner.bottom() - detail_rows - 1;
    let rule = Tone::Rule.style(palette);
    buf[(area.x, rule_y)].set_char(marks.ls).set_style(rule);
    for x in area.x + 1..area.right() - 1 {
        buf[(x, rule_y)].set_char(marks.h).set_style(rule);
    }
    buf[(area.right() - 1, rule_y)].set_char(marks.rs).set_style(rule);
    let body = Rect::new(inner.x, inner.y, inner.width, rule_y - inner.y);
    let detail = Rect::new(inner.x, rule_y + 1, inner.width, detail_rows);
    Page { area, inner, body, detail: Some(detail) }
}

/// The selection's background: `Theme::select`, or, where the palette rounds it to the panel's
/// own colour (16 colours), the terminal's dark grey, so it still shows; none without colour.
pub fn select_bg(palette: &Palette) -> Option<Color> {
    if palette.mono() {
        return None;
    }
    let select = palette.token(palette.theme.select);
    Some(if select == palette.token(palette.theme.panel) { Color::DarkGray } else { select })
}

/// Marks `area` (a row, or a card's rows) as the selection: the quiet `select` background under
/// every cell and its text bold, so it reads brighter. Each cell keeps its own foreground, so a
/// meter's or a warning's colour inside the selection stays as it is; never reverse video. With
/// no colour at all only the bold shows, beside the row's own pointer.
pub fn highlight(buf: &mut Buffer, area: Rect, palette: &Palette) {
    let area = area.intersection(buf.area);
    let bg = select_bg(palette);
    for y in area.top()..area.bottom() {
        for x in area.left()..area.right() {
            let cell = &mut buf[(x, y)];
            if let Some(bg) = bg {
                cell.set_bg(bg);
            }
            cell.modifier.insert(Modifier::BOLD);
        }
    }
}

#[cfg(test)]
mod tests {
    use fn_sim::Nation;

    use super::*;
    use crate::color::ColorDepth;

    fn palette() -> Palette {
        let s = crate::new_session(7, Nation::Earth);
        Palette::new(&s.defs, Nation::Earth, ColorDepth::TrueColor, false)
    }

    #[test]
    fn the_frame_titles_the_page_and_gives_the_body_its_inside() {
        let area = Rect::new(0, 0, 80, 23);
        let mut buf = Buffer::empty(area);
        let page = draw(&mut buf, area, &palette(), "research", 0);
        assert_eq!(page.inner, Rect::new(2, 1, 76, 21));
        assert_eq!(page.body, page.inner);
        assert_eq!(page.detail, None);
        let text = crate::frame::to_text(&buf);
        let top = text.lines().next().unwrap();
        assert!(top.starts_with('┏') && top.ends_with('┓'), "{top}");
        assert!(top.contains(" ◆ RESEARCH ◆ "), "{top}");
        assert_eq!(text_size((80, 23)), (76, 21));
    }

    /// The detail strip sits under a rule joined to the side lines, just above the bottom edge.
    #[test]
    fn a_detail_strip_sits_under_a_rule_joined_to_the_frame() {
        let area = Rect::new(0, 0, 80, 23);
        let mut buf = Buffer::empty(area);
        let page = draw(&mut buf, area, &palette(), "Build", 2);
        let detail = page.detail.expect("asked for two rows");
        assert_eq!(detail, Rect::new(2, 20, 76, 2));
        assert_eq!(page.body, Rect::new(2, 1, 76, 18));
        let text = crate::frame::to_text(&buf);
        let rule = text.lines().nth(19).unwrap();
        assert!(rule.starts_with("├─") && rule.ends_with("─┤"), "{rule}");
        assert_eq!(rule.chars().count(), 80);
    }

    #[test]
    fn a_strip_too_tall_for_the_page_leaves_the_body_a_row() {
        let area = Rect::new(0, 0, 40, 6);
        let mut buf = Buffer::empty(area);
        let page = draw(&mut buf, area, &palette(), "Log", 10);
        assert_eq!(page.body.height, 1);
        assert_eq!(page.detail.map(|d| d.height), Some(2));
    }

    #[test]
    fn the_position_goes_on_the_bottom_edge_flush_right() {
        let area = Rect::new(0, 0, 80, 23);
        let mut buf = Buffer::empty(area);
        let p = palette();
        let page = draw(&mut buf, area, &p, "Log", 0);
        page.position(&mut buf, &p, Some("12-48 of 61"));
        let text = crate::frame::to_text(&buf);
        let bottom = text.lines().nth(22).unwrap();
        assert!(bottom.ends_with(" 12-48 of 61 ─┛"), "{bottom}");
    }

    /// The selection is a background and bold, never reverse video, and each cell keeps its own
    /// foreground.
    #[test]
    fn a_highlight_keeps_each_cells_own_colour() {
        let p = palette();
        let area = Rect::new(0, 0, 10, 1);
        let mut buf = Buffer::empty(area);
        buf.set_string(0, 0, "ab", Tone::Warn.style(&p));
        highlight(&mut buf, area, &p);
        let cell = &buf[(0, 0)];
        assert_eq!(cell.fg, p.token(p.theme.warn));
        assert_eq!(Some(cell.bg), select_bg(&p));
        assert!(cell.modifier.contains(Modifier::BOLD));
        assert!(!cell.modifier.contains(Modifier::REVERSED));
    }
}
