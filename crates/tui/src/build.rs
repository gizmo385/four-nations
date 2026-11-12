//! The Build screen (`U`, #285): every building the nation can place or may yet unlock, on a full
//! screen in the shared frame (`crate::page`, #652). A list with a header and four aligned
//! columns (building, cost, size, needs) fills the body, the buildable rows first and the locked
//! ones after, dim, each naming in the needs column the tech that unlocks it (#658); under a rule
//! the frame's detail strip holds the highlighted building's detail and nowhere else. It takes
//! every key like a modal (`crate::state::Modal::Build`). `App::open_build_modal` opens it, on
//! the current choice if it's still placeable, else the first one that is. `Enter` there picks
//! the highlighted building, closes the screen and leaves the Build tool active with it
//! (`App::build_modal_key`); on a locked row it does nothing, and the strip says why. `Esc` or
//! `U` leaves the tool and the choice as they were. Arrows or `W`/`S` move the highlight by one
//! row, locked ones included, so the strip can show what they still need; any other letter jumps
//! to the next name that starts with it (`W`, `S` and `U` are taken, so a building beginning
//! with one can't be reached that way).

use crate::color::Palette;
use crate::page;
use crate::widgets::{Marks, Painter, Row, Tone, window_start};
use fn_ui::Session;
use fn_ui::build::{BuildingDetail, PickerRow, building_detail};
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;

// The rows, cursors and keys are `fn_ui`'s (#857); this module draws them.
pub use fn_ui::screens::build::*;

/// Draws the screen over `area` (the whole page, `crate::layout::Regions::page`): the shared
/// frame, the list under its header (sliding to keep `at` in view, as the saves screen's does,
/// its position on the bottom edge once it scrolls), and under a rule the highlighted
/// building's detail strip. The strip is as tall as the tallest building's detail, so the rule
/// does not move as the highlight does.
pub fn draw(
    session: &Session,
    rows: &[PickerRow],
    at: usize,
    palette: &Palette,
    area: Rect,
    buf: &mut Buffer,
) {
    let marks = Marks::new(palette.ascii);
    if rows.is_empty() {
        let page = page::draw(buf, area, palette, "BUILD", 0);
        Painter::new(buf, page.body, palette).row(&Row::text("Nothing to build yet.", Tone::Dim));
        return;
    }
    let at = at.min(rows.len() - 1);
    let width = usize::from(area.width.saturating_sub(4));
    let details: Vec<BuildingDetail> =
        rows.iter().map(|r| building_detail(session, r.id)).collect();
    let tallest =
        details.iter().map(|d| detail_rows(d, width, marks.ellipsis).len()).max().unwrap_or(0);
    // The list keeps at least half the page (its header and a few rows) whatever the strip wants.
    let room = usize::from(area.height.saturating_sub(2) / 2).max(1);
    let strip = tallest.min(MAX_STRIP).min(room);
    let page = page::draw(buf, area, palette, "BUILD", strip as u16);
    let columns = Columns::of(rows);

    let mut p = Painter::new(buf, page.body, palette);
    p.row(&columns.header());
    let shown = usize::from(page.body.height.saturating_sub(1));
    let first = window_start(at, shown);
    let mut chosen_at = None;
    for (i, row) in rows.iter().enumerate().skip(first).take(shown) {
        let chosen = i == at;
        if chosen {
            chosen_at = Some(page.body.y + p.row);
        }
        p.row(&columns.row(row, chosen, &marks));
    }
    if let Some(y) = chosen_at {
        page::highlight(buf, Rect::new(page.body.x, y, page.body.width, 1), palette);
    }
    let position = fn_ui::text::window_label("buildings", first, shown, rows.len());
    page.position(buf, palette, position.as_deref());

    if let Some(strip) = page.detail {
        let mut d = Painter::new(buf, strip, palette);
        for row in detail_rows(&details[at], width, marks.ellipsis) {
            d.row(&row);
        }
    }
}

#[cfg(test)]
mod tests {
    use fn_sim::Nation;
    use ratatui::buffer::Buffer;

    use super::*;
    use crate::color::ColorDepth;
    use crate::new_session;

    fn rows(nation: Nation) -> (Session, Vec<PickerRow>) {
        let s = new_session(7, nation);
        let rows = fn_ui::build::picker_rows(&s);
        (s, rows)
    }

    #[test]
    fn stepping_stops_at_the_edges_and_an_empty_list_stays_put() {
        let (_, rows) = rows(Nation::Earth);
        assert_eq!(step(&rows, 0, -1), 0);
        assert_eq!(step(&rows, 0, 1), 1);
        assert_eq!(step(&rows, rows.len() - 1, 1), rows.len() - 1);
        assert_eq!(step(&[], 0, 1), 0);
    }

    #[test]
    fn a_letter_jumps_to_the_next_name_that_starts_with_it_wrapping_round() {
        let (_, rows) = rows(Nation::Earth);
        let bed = rows.iter().position(|r| r.name == "Bed").unwrap();
        let campfire = rows.iter().position(|r| r.name == "Campfire").unwrap();
        assert_eq!(bed, 0, "bed is the first building in the data");
        assert_eq!(jump_to_letter(&rows, bed, 'c'), campfire);
        // Past Campfire the next 'b' is the locked Badgermole Den, and past that it wraps round
        // to Bed.
        let den = rows.iter().position(|r| r.name == "Badgermole Den").unwrap();
        assert!(den > campfire, "locked rows come last");
        assert_eq!(jump_to_letter(&rows, campfire, 'b'), den);
        assert_eq!(jump_to_letter(&rows, den, 'b'), bed);
        // No building starts with Z: the cursor stays.
        assert_eq!(jump_to_letter(&rows, bed, 'z'), bed);
        assert_eq!(jump_to_letter(&[], 0, 'b'), 0);
    }

    /// Two buildings can share a letter (Air's Bed and Bison Stable): pressing it again from a
    /// match cycles past it to the next one, instead of staying put.
    #[test]
    fn pressing_the_same_letter_again_cycles_past_the_match_it_is_already_on() {
        let (_, rows) = rows(Nation::Air);
        let bed = rows.iter().position(|r| r.name == "Bed").unwrap();
        let bison = rows.iter().position(|r| r.name == "Bison Stable").unwrap();
        assert_eq!(bed, 0);
        assert_eq!(jump_to_letter(&rows, bed, 'b'), bison);
    }

    fn text_of(s: &Session, rows: &[PickerRow], at: usize, nation: Nation, area: Rect) -> String {
        let palette = Palette::new(&s.defs, nation, ColorDepth::TrueColor, false);
        let mut buf = Buffer::empty(area);
        draw(s, rows, at, &palette, area, &mut buf);
        crate::frame::to_text(&buf)
    }

    /// The list has a header over its four columns, and the highlighted building's detail is in
    /// the strip under the rule and nowhere else (#658).
    #[test]
    fn the_screen_draws_a_header_and_the_detail_strip_under_a_rule() {
        let (s, rows) = rows(Nation::Earth);
        let text = text_of(&s, &rows, 0, Nation::Earth, Rect::new(0, 0, 88, 39));
        let lines: Vec<&str> = text.lines().collect();
        assert!(lines[0].contains("BUILD"), "{text}");
        let header = lines[1];
        for head in ["BUILDING", "COST", "SIZE", "NEEDS"] {
            assert!(header.contains(head), "{header}");
        }
        let rule = lines.iter().position(|l| l.starts_with('├')).expect("a rule");
        let (list, strip) = (lines[..rule].join("\n"), lines[rule..].join("\n"));
        assert!(list.contains(&rows[0].name), "{text}");
        assert!(strip.contains("Size 1x1."), "the strip holds the detail:\n{text}");
        assert!(strip.contains("in stores"), "{text}");
        assert!(!list.contains("Size 1x1.") && !list.contains("in stores"), "{text}");
        assert!(!text.contains("Footprint:"), "{text}");
    }

    /// Every building of every nation sits in its column: the cost, size and needs of each row
    /// start where their header does, so no long name or cost breaks the alignment.
    #[test]
    fn every_building_of_every_nation_starts_each_column_where_its_header_does() {
        for nation in Nation::ALL {
            let (s, rows) = rows(nation);
            let text = text_of(&s, &rows, 0, nation, Rect::new(0, 0, 140, 90));
            let lines: Vec<Vec<char>> = text.lines().map(|l| l.chars().collect()).collect();
            let col = |line: &[char], word: &str| {
                let line: String = line.iter().collect();
                line.find(word).map(|b| line[..b].chars().count()).unwrap()
            };
            let header = &lines[1];
            let (name_at, cost_at, size_at, needs_at) = (
                col(header, "BUILDING"),
                col(header, "COST"),
                col(header, "SIZE"),
                col(header, "NEEDS"),
            );
            let cells = |line: &[char], at: usize, text: &str| {
                let n = text.chars().count();
                let got: String = line[at..at + n].iter().collect();
                (got == text, line[at - 2..at].iter().all(|c| *c == ' '))
            };
            assert_eq!(lines.len(), 90);
            for (i, row) in rows.iter().enumerate() {
                let line = &lines[2 + i];
                assert!(cells(line, name_at, &row.name).0, "{nation:?} {}", row.name);
                assert_eq!(
                    cells(line, cost_at, &row.price),
                    (true, true),
                    "{nation:?} {}",
                    row.name
                );
                assert_eq!(
                    cells(line, size_at, &row.size),
                    (true, true),
                    "{nation:?} {}",
                    row.name
                );
                if let Some(tech) = &row.tech {
                    assert_eq!(cells(line, needs_at, tech), (true, true), "{nation:?} {tech}");
                } else {
                    assert!(line[needs_at..].iter().all(|c| matches!(c, ' ' | '│')), "{nation:?}");
                }
            }
        }
    }

    /// A long name never loses its column: the name column grows to the longest name on show.
    #[test]
    fn a_long_name_widens_the_name_column_for_every_row() {
        let (s, mut rows) = rows(Nation::Earth);
        rows[0].name = "A Building With A Very Long Name".to_owned();
        let text = text_of(&s, &rows, 1, Nation::Earth, Rect::new(0, 0, 120, 40));
        let col = |l: &str, w: &str| l.find(w).unwrap();
        let lines: Vec<&str> = text.lines().collect();
        let cost = col(lines[1], "COST");
        assert!(lines[2].contains("A Building With A Very Long Name  "), "{text}");
        for line in &lines[2..4] {
            assert_eq!(line.find(" Wood").map(|i| i > cost), Some(true), "{text}");
        }
    }

    /// Locked rows are dim throughout; a cost stores cannot pay is in the warning colour, an
    /// affordable one is not.
    #[test]
    fn locked_rows_are_dim_and_an_unaffordable_cost_wears_the_warning_colour() {
        let (_, rows) = rows(Nation::Earth);
        let columns = Columns::of(&rows);
        let marks = Marks::new(false);
        let tones = |row: &Row| row.left.iter().map(|(_, t)| *t).collect::<Vec<_>>();
        let mut open = rows.iter().find(|r| !r.locked()).unwrap().clone();
        open.affordable = true;
        assert_eq!(
            tones(&columns.row(&open, false, &marks)),
            [Tone::Text, Tone::Text, Tone::Text, Tone::Dim]
        );
        open.affordable = false;
        assert_eq!(columns.row(&open, false, &marks).left[1].1, Tone::Warn);
        let locked = rows.iter().find(|r| r.locked()).unwrap();
        assert_eq!(tones(&columns.row(locked, false, &marks)), [Tone::Dim; 4]);
        let text = columns.row(locked, false, &marks).plain();
        assert!(text.contains(locked.tech.as_deref().unwrap()), "{text}");
    }

    /// The strip says what is short, and why a locked building can't be built.
    #[test]
    fn the_strip_names_what_is_short_and_what_a_locked_building_needs() {
        let (s, rows) = rows(Nation::Earth);
        let strip = |row: &PickerRow| -> Vec<String> {
            detail_rows(&building_detail(&s, row.id), 100, '…').iter().map(Row::plain).collect()
        };
        let locked = rows.iter().find(|r| r.locked()).unwrap();
        let tech = locked.tech.as_deref().unwrap();
        let lines = strip(locked);
        assert!(
            lines.iter().any(|l| l.starts_with(&format!("Needs {tech}.")) && l.contains("Enter")),
            "{lines:?}"
        );
        let poor = rows.iter().find(|r| !r.locked() && !r.affordable);
        if let Some(poor) = poor {
            assert!(strip(poor).iter().any(|l| l.starts_with("Short of ")), "{:?}", strip(poor));
        }
    }

    /// A long line wraps in the strip instead of being cut, and the strip is as tall as the
    /// tallest building's detail so the rule holds still as the highlight moves.
    #[test]
    fn the_strip_wraps_and_holds_its_height_as_the_highlight_moves() {
        let (s, rows) = rows(Nation::Fire);
        let area = Rect::new(0, 0, 60, 40);
        let rule_of = |at| {
            let text = text_of(&s, &rows, at, Nation::Fire, area);
            let rule = text.lines().position(|l| l.starts_with('├')).unwrap();
            (rule, text)
        };
        let (rule, text) = rule_of(0);
        for at in 0..rows.len() {
            assert_eq!(rule_of(at).0, rule, "row {at}");
        }
        for r in &rows {
            let width = 56;
            let lines = detail_rows(&building_detail(&s, r.id), width, '…');
            assert!(lines.iter().all(|l| l.plain().chars().count() <= width), "{}", r.name);
            assert!(!lines.iter().any(|l| l.plain().contains('…')), "{}", r.name);
        }
        assert!(text.contains("├"), "{text}");
    }

    #[test]
    fn an_empty_list_says_so_instead_of_drawing_a_detail() {
        let s = new_session(7, Nation::Earth);
        let palette = Palette::new(&s.defs, Nation::Earth, ColorDepth::TrueColor, false);
        let mut buf = Buffer::empty(Rect::new(0, 0, 88, 39));
        draw(&s, &[], 0, &palette, Rect::new(0, 0, 88, 39), &mut buf);
        assert!(crate::frame::to_text(&buf).contains("Nothing to build yet."));
    }
}
