//! The abilities screen (`I`, #719): every bending ability the settlement has or may yet unlock,
//! on a full screen in the shared frame (`crate::page`, #652). A list with a header and three
//! columns (the ability, who can bend it, what it does) fills the body, the ones that can be
//! used now first and the rest after, dim, each naming in the last column what it still needs;
//! under a rule the frame's detail strip holds the highlighted ability in full words. The
//! highlighted row is marked by `▸` and the quiet background (`page::highlight`), and its place
//! in the list is on the bottom edge once the list scrolls.
//!
//! The pointer opens on the ability the Bend tool channels now. `Enter` (`App::pick_ability`)
//! picks the highlighted one: the Bend tool becomes active with it and the screen closes; on one
//! that can't be used yet it does nothing, and the strip says why. `Esc` or `I` closes the
//! screen and leaves the tool and its ability as they were. `B` still picks the Bend tool
//! directly, and pressed again moves on to the next usable ability, as before.

use crate::color::Palette;
use crate::page::{self, Page};
use crate::widgets::{Marks, Painter, Row, Tone, window_start};
use fn_ui::bend::AbilityRow;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;

// The rows, cursors and keys are `fn_ui`'s (#857); this module draws them.
pub use fn_ui::screens::abilities::*;

/// Draws the screen's contents into `page` (the frame is drawn): the list under its header,
/// sliding to keep `at` in view, and the highlighted ability's detail in the strip. Returns the
/// position for the bottom edge (`None` when everything fits).
pub fn draw(
    rows: &[AbilityRow],
    at: usize,
    palette: &Palette,
    page: &Page,
    buf: &mut Buffer,
) -> Option<String> {
    let marks = Marks::new(palette.ascii);
    let body = page.body;
    if rows.is_empty() {
        Painter::new(buf, body, palette).row(&Row::text("No abilities yet.", Tone::Dim));
        return None;
    }
    let at = at.min(rows.len() - 1);
    let width = usize::from(body.width);
    let columns = Columns::of(rows);

    let mut p = Painter::new(buf, body, palette);
    p.row(&columns.header(width, marks.ellipsis));
    let shown = usize::from(body.height.saturating_sub(1));
    let first = window_start(at, shown);
    let mut chosen_at = None;
    for (i, row) in rows.iter().enumerate().skip(first).take(shown) {
        let chosen = i == at;
        if chosen {
            chosen_at = Some(body.y + p.row);
        }
        p.row(&columns.row(row, chosen, width, &marks));
    }
    if let Some(y) = chosen_at {
        page::highlight(buf, Rect::new(body.x, y, body.width, 1), palette);
    }
    if let Some(strip) = page.detail {
        let mut d = Painter::new(buf, strip, palette);
        for row in detail_rows(&rows[at], width, marks.ellipsis) {
            d.row(&row);
        }
    }
    fn_ui::text::window_label("abilities", first, shown, rows.len())
}

#[cfg(test)]
mod tests {
    use fn_sim::Nation;

    use super::*;
    use crate::color::ColorDepth;
    use crate::new_session;

    fn text_of(nation: Nation, at: usize, area: Rect) -> (Vec<AbilityRow>, String) {
        let s = new_session(7, nation);
        let rows = fn_ui::bend::abilities(&s);
        let palette = Palette::new(&s.defs, nation, ColorDepth::TrueColor, false);
        let mut buf = Buffer::empty(area);
        let width = page::text_size((i32::from(area.width), i32::from(area.height))).0;
        let strip = detail_height(&rows, width, false, area.height);
        let frame = page::draw(&mut buf, area, &palette, "ABILITIES", strip);
        let position = draw(&rows, at, &palette, &frame, &mut buf);
        frame.position(&mut buf, &palette, position.as_deref());
        (rows, crate::frame::to_text(&buf))
    }

    #[test]
    fn stepping_stops_at_the_edges_and_an_empty_list_stays_put() {
        let s = new_session(7, Nation::Earth);
        let rows = fn_ui::bend::abilities(&s);
        assert_eq!(step(&rows, 0, -1), 0);
        assert_eq!(step(&rows, 0, 1), 1);
        assert_eq!(step(&rows, rows.len() - 1, 1), rows.len() - 1);
        assert_eq!(step(&rows, 0, 5), 1, "the sign only");
        assert_eq!(step(&[], 0, 1), 0);
    }

    #[test]
    fn the_pointer_opens_on_the_ability_the_bend_tool_channels() {
        let s = new_session(7, Nation::Fire);
        let rows = fn_ui::bend::abilities(&s);
        assert_eq!(initial_cursor(&rows), 0);
        assert!(rows[0].current);
    }

    /// The list has a header over its three columns, every ability of the nation with its
    /// requirement or description, and the highlighted one's full detail under a rule.
    #[test]
    fn the_screen_draws_a_header_every_ability_and_the_detail_strip_under_a_rule() {
        for nation in Nation::ALL {
            let (rows, text) = text_of(nation, 0, Rect::new(0, 0, 100, 30));
            let lines: Vec<&str> = text.lines().collect();
            assert!(lines[0].contains("ABILITIES"), "{text}");
            for head in ["ABILITY", "BENDERS", "WHAT IT DOES"] {
                assert!(lines[1].contains(head), "{}", lines[1]);
            }
            let rule = lines.iter().position(|l| l.starts_with('├')).expect("a rule");
            let (list, strip) = (lines[..rule].join("\n"), lines[rule..].join("\n"));
            for r in &rows {
                assert!(list.contains(&r.name), "{nation:?}: {} in\n{text}", r.name);
            }
            assert!(lines[2].contains('▸'), "{}", lines[2]);
            assert!(
                strip.contains(&rows[0].what),
                "{nation:?}: the strip holds the detail\n{text}"
            );
            assert!(strip.contains("The Bend tool channels it now."), "{text}");
            let locked = rows.iter().find(|r| !r.usable()).unwrap();
            assert!(list.contains(locked.unusable.as_deref().unwrap()), "{nation:?}: {text}");
        }
    }

    /// Moving the pointer to a locked ability puts why it can't be used into the strip.
    #[test]
    fn a_locked_ability_says_in_the_strip_what_it_needs() {
        let s = new_session(7, Nation::Fire);
        let rows = fn_ui::bend::abilities(&s);
        let at = rows.iter().position(|r| !r.usable()).unwrap();
        let (_, text) = text_of(Nation::Fire, at, Rect::new(0, 0, 100, 30));
        let strip = text.lines().skip_while(|l| !l.starts_with('├')).collect::<Vec<_>>().join("\n");
        let why = rows[at].unusable.clone().unwrap();
        assert!(strip.contains(&format!("{why}: Enter picks nothing until then.")), "{text}");
        assert!(!strip.contains("channels it now"), "{text}");
    }

    /// A page too short for the whole list scrolls it, keeps the pointer in view and says where
    /// on the bottom edge.
    #[test]
    fn a_short_page_scrolls_and_says_where_on_the_bottom_edge() {
        let s = new_session(7, Nation::Earth);
        let total = fn_ui::bend::abilities(&s).len();
        let (_, text) = text_of(Nation::Earth, total - 1, Rect::new(0, 0, 80, 10));
        let last = text.lines().last().unwrap();
        assert!(last.contains("abilities "), "{text}");
        assert!(last.contains(&format!("-{total} of {total}")), "{text}");
        assert!(text.contains('▸'), "{text}");
    }
}
