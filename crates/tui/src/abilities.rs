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

use fn_ui::bend::AbilityRow;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;

use crate::color::Palette;
use crate::page::{self, Page};
use crate::widgets::{Marks, Painter, Row, Tone, fit, window_start, wrap};

/// The gap between two columns of the list.
const GAP: &str = "  ";
/// The most rows the detail strip may take, whatever the page: the list keeps the rest.
const MAX_STRIP: usize = 6;

/// The pointer after a step of `dy` rows (its sign only), stopping at the list's edges.
pub fn step(rows: &[AbilityRow], at: usize, dy: i32) -> usize {
    if rows.is_empty() {
        return 0;
    }
    (at as i64 + i64::from(dy.signum())).clamp(0, rows.len() as i64 - 1) as usize
}

/// Where the pointer starts: on the ability the Bend tool channels now, else the first row.
pub fn initial_cursor(rows: &[AbilityRow]) -> usize {
    rows.iter().position(|r| r.current).unwrap_or(0)
}

/// The list's column widths, each as wide as its widest entry (the whole list, so the columns
/// stay put as it scrolls) and at least as wide as its header.
struct Columns {
    name: usize,
    casters: usize,
}

impl Columns {
    fn of(rows: &[AbilityRow]) -> Self {
        let widest = |head: &str, f: fn(&AbilityRow) -> &str| {
            rows.iter().map(|r| f(r).chars().count()).chain([head.chars().count()]).max().unwrap()
        };
        Self { name: widest("ABILITY", |r| &r.name), casters: widest("BENDERS", |r| &r.casters) }
    }

    /// One row as spans, each cell padded to its column (the pointer's mark and a space come
    /// before the name). The last column takes whatever width is left, cut with an ellipsis.
    fn spans(&self, pointer: &str, cells: [(&str, Tone); 3], width: usize, ellipsis: char) -> Row {
        let [(name, name_tone), (casters, casters_tone), (last, last_tone)] = cells;
        let used = 2 + self.name + GAP.len() + self.casters + GAP.len();
        Row::spans(vec![
            (format!("{pointer} {name:<w$}", w = self.name), name_tone),
            (format!("{GAP}{casters:<w$}", w = self.casters), casters_tone),
            (format!("{GAP}{}", fit(last, width.saturating_sub(used), ellipsis)), last_tone),
        ])
    }

    fn header(&self, width: usize, ellipsis: char) -> Row {
        self.spans(
            " ",
            [("ABILITY", Tone::Dim), ("BENDERS", Tone::Dim), ("WHAT IT DOES", Tone::Dim)],
            width,
            ellipsis,
        )
    }

    /// One ability's row. One that can't be used now is dim throughout and says in the last
    /// column what it needs; the highlighted one is a title and carries the pointer.
    fn row(&self, row: &AbilityRow, chosen: bool, width: usize, marks: &Marks) -> Row {
        let pointer = if chosen { marks.pointer } else { " " };
        let (name_tone, rest_tone) = match (row.usable(), chosen) {
            (false, _) => (Tone::Dim, Tone::Dim),
            (true, true) => (Tone::Title, Tone::Text),
            (true, false) => (Tone::Text, Tone::Text),
        };
        let last = row.unusable.as_deref().unwrap_or(&row.what);
        self.spans(
            pointer,
            [(&row.name, name_tone), (&row.casters, rest_tone), (last, rest_tone)],
            width,
            marks.ellipsis,
        )
    }
}

/// The detail strip's rows for `row`, wrapped to `width` and never cut: its name with who can
/// bend it on the right, why it can't be used yet (and that `Enter` then does nothing), what it
/// does, and whether the Bend tool channels it now.
fn detail_rows(row: &AbilityRow, width: usize, ellipsis: char) -> Vec<Row> {
    let wrapped = |text: &str, tone: Tone| -> Vec<Row> {
        wrap(text, width, usize::MAX, ellipsis).into_iter().map(|l| Row::text(l, tone)).collect()
    };
    let tone = if row.usable() { Tone::Text } else { Tone::Dim };
    let mut rows = Vec::new();
    // Who can bend it sits flush right of the name when both fit one line, else under it.
    if row.casters.is_empty() {
        rows.push(Row::text(row.name.clone(), Tone::Title));
    } else if row.name.chars().count() + GAP.len() + row.casters.chars().count() <= width {
        rows.push(Row::pair(row.name.clone(), Tone::Title, row.casters.clone(), tone));
    } else {
        rows.push(Row::text(row.name.clone(), Tone::Title));
        rows.extend(wrapped(&row.casters, tone));
    }
    if let Some(why) = &row.unusable {
        rows.extend(wrapped(&format!("{why}: Enter picks nothing until then."), Tone::Warn));
    }
    rows.extend(wrapped(&row.what, Tone::Text));
    if row.current {
        rows.extend(wrapped("The Bend tool channels it now.", Tone::Dim));
    }
    rows
}

/// How tall the detail strip is on a page `page_height` rows tall and `width` text columns
/// wide: as tall as the tallest ability's detail, so the rule does not move as the pointer
/// does, but never more than [`MAX_STRIP`] or what leaves the list half the page.
pub fn detail_height(rows: &[AbilityRow], width: usize, ascii: bool, page_height: u16) -> u16 {
    let ellipsis = Marks::new(ascii).ellipsis;
    let tallest = rows.iter().map(|r| detail_rows(r, width, ellipsis).len()).max().unwrap_or(0);
    let room = usize::from(page_height.saturating_sub(2) / 2).max(1);
    tallest.min(MAX_STRIP).min(room) as u16
}

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
