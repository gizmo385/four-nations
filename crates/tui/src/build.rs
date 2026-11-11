//! The Build modal (`U`, #285): every building the nation can place or may yet unlock, a list on
//! the left and the highlighted one's detail on the right, in the reusable `crate::modal` frame.
//! `App::open_build_modal` opens it, on the current choice if it's still placeable, else the
//! first one that is. `Enter` there picks the highlighted building, closes the modal and leaves
//! the Build tool active with it (`App::build_modal_key`); `Esc` leaves the tool and the choice
//! as they were. Arrows or `W`/`S` move the highlight by one row, locked ones included, so the
//! detail pane can show what they still need; any other letter jumps to the next name that
//! starts with it (`W` and `S` are taken by movement, so a building beginning with one can't be
//! reached that way).

use fn_ui::Session;
use fn_ui::build::{BuildingDetail, PickerRow, building_detail};
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};

use crate::color::Palette;
use crate::modal;
use crate::widgets::{Marks, Painter, Row, Tone, window_start, wrap};

/// The modal's height: enough rows for a dozen buildings and the detail's wrapped effects. Its
/// width follows the list ([`list_width`]) plus [`DETAIL_WIDTH`].
const HEIGHT: u16 = 18;
/// The list column's narrowest width, kept even when every row is short so the box doesn't jump
/// about between nations.
const MIN_LIST_WIDTH: u16 = 24;
/// The detail pane's width: room for its longest fixed lines ("Room for 1 inside, through the
/// door"); effects wrap within it.
const DETAIL_WIDTH: u16 = 38;
/// What the detail pane keeps when a small terminal shrinks the box: the list gives way first
/// (its names take an ellipsis), so the detail never collapses to a sliver.
const MIN_DETAIL_WIDTH: u16 = 20;

/// The cursor after a step of `dy` rows (its sign only), stopping at the list's edges, as every
/// other windowed list in this crate does (`crate::saves::step`).
pub fn step(rows: &[PickerRow], at: usize, dy: i32) -> usize {
    if rows.is_empty() {
        return 0;
    }
    let last = rows.len() - 1;
    (at as i64 + i64::from(dy.signum())).clamp(0, last as i64) as usize
}

/// The next row after `at` (wrapping, `at` itself last) whose name starts with `c`, ignoring
/// case; `at` unchanged if none does, `at` included.
pub fn jump_to_letter(rows: &[PickerRow], at: usize, c: char) -> usize {
    if rows.is_empty() {
        return at;
    }
    let want = c.to_ascii_lowercase();
    let starts =
        |r: &PickerRow| r.name.chars().next().is_some_and(|f| f.to_ascii_lowercase() == want);
    // Starts the search at `at + 1`, not `at` itself, so pressing the same letter again cycles
    // past a match the cursor is already on to the next one (two buildings can share a letter:
    // Bed and Bison Stable).
    (1..=rows.len()).map(|step| (at + step) % rows.len()).find(|&i| starts(&rows[i])).unwrap_or(at)
}

/// The list column's width: its widest row, pointer, name, gap and cost (or "Locked") together,
/// so no name has to give way to an ellipsis for a long cost ("10 Stone, 6 Wood, 2 Tool, 2x2").
fn list_width(rows: &[PickerRow]) -> u16 {
    let widest = rows
        .iter()
        .map(|r| {
            let right = if r.locked() { "Locked".len() } else { r.cost.chars().count() };
            2 + r.name.chars().count() + 1 + right
        })
        .max()
        .unwrap_or(0);
    u16::try_from(widest).unwrap_or(u16::MAX).max(MIN_LIST_WIDTH)
}

/// One row of the list: the highlighted row's pointer, its name and its cost, or "Locked"
/// (dimmed) in place of the cost; the detail pane names the tech a locked one still needs in
/// full, so the list stays narrow even for the longest tech name.
fn list_row(row: &PickerRow, chosen: bool, marks: &Marks) -> Row {
    let pointer = if chosen { marks.pointer } else { " " };
    let (name_tone, right, right_tone) = if row.locked() {
        (Tone::Dim, "Locked".to_owned(), Tone::Warn)
    } else if chosen {
        (Tone::Title, row.cost.clone(), Tone::Dim)
    } else {
        (Tone::Text, row.cost.clone(), Tone::Dim)
    };
    Row::pair(format!("{pointer} {}", row.name), name_tone, right, right_tone)
}

/// The detail pane's rows for `detail`: its name, its footprint, its cost against stores, what
/// it does, and the tech it still needs.
fn detail_rows(detail: &BuildingDetail, width: usize, ellipsis: char) -> Vec<Row> {
    let mut rows = vec![
        Row::text(detail.name.clone(), Tone::Title),
        Row::text(format!("Footprint: {}", detail.footprint), Tone::Dim),
        Row::default(),
    ];
    rows.extend(detail.cost.iter().map(|line| Row::text(line.clone(), Tone::Text)));
    rows.push(Row::default());
    for line in &detail.effects {
        rows.extend(
            wrap(line, width, usize::MAX, ellipsis).into_iter().map(|l| Row::text(l, Tone::Text)),
        );
    }
    if let Some(requires) = &detail.requires {
        rows.push(Row::default());
        rows.push(Row::text(requires.clone(), Tone::Warn));
    }
    rows
}

/// Draws the modal: the frame, the list (sliding to keep `at` in view, as the saves screen's
/// does), and the highlighted building's detail.
pub fn draw(
    session: &Session,
    rows: &[PickerRow],
    at: usize,
    palette: &Palette,
    outer: Rect,
    buf: &mut Buffer,
) {
    let marks = Marks::new(palette.ascii);
    let title = format!(" {0} BUILD {0} ", marks.seal);
    let list_width = list_width(rows);
    // The frame's own padding: a border and a blank column on each side (`modal::draw`).
    let size = (list_width + modal::GAP + DETAIL_WIDTH + 4, HEIGHT);
    // The bottom edge says where the list's window sits, once it scrolls (#651).
    let shown = usize::from(modal::modal_area(outer, size).height.saturating_sub(2));
    let position = fn_ui::text::window_label(
        "buildings",
        window_start(at.min(rows.len().saturating_sub(1)), shown),
        shown,
        rows.len(),
    );
    let inner = modal::draw(buf, outer, size, palette, &title, position.as_deref().unwrap_or(""));
    let room = inner.width.saturating_sub(modal::GAP + MIN_DETAIL_WIDTH);
    let split = modal::split(inner, list_width.min(room.max(MIN_LIST_WIDTH)));

    let mut p = Painter::new(buf, split.list, palette);
    if rows.is_empty() {
        p.row(&Row::text("Nothing to build yet.", Tone::Dim));
        return;
    }
    let at = at.min(rows.len() - 1);
    let shown = usize::from(split.list.height);
    let first = window_start(at, shown);
    let mut reversed = None;
    for (i, row) in rows.iter().enumerate().skip(first).take(shown) {
        let chosen = i == at;
        if chosen {
            reversed = Some(split.list.y + p.row);
        }
        p.row(&list_row(row, chosen, &marks));
    }
    if let Some(y) = reversed {
        for x in split.list.x..split.list.right() {
            buf[(x, y)].set_style(Style::new().add_modifier(Modifier::REVERSED));
        }
    }

    let detail = building_detail(session, rows[at].id);
    let mut d = Painter::new(buf, split.detail, palette);
    for row in detail_rows(&detail, usize::from(split.detail.width), marks.ellipsis) {
        d.row(&row);
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
        // Wrapping: past Campfire, 'b' comes back round to Bed.
        assert_eq!(jump_to_letter(&rows, campfire, 'b'), bed);
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

    #[test]
    fn the_modal_draws_the_list_and_the_highlighted_buildings_detail() {
        let (s, rows) = rows(Nation::Earth);
        let palette = Palette::new(&s.defs, Nation::Earth, ColorDepth::TrueColor, false);
        let mut buf = Buffer::empty(Rect::new(0, 0, 88, 39));
        draw(&s, &rows, 0, &palette, Rect::new(0, 0, 88, 39), &mut buf);
        let text = crate::frame::to_text(&buf);
        assert!(text.contains("BUILD"), "{text}");
        assert!(text.contains(&rows[0].name), "{text}");
        assert!(text.contains("Footprint:"), "{text}");
    }

    /// Every nation's longest name and cost fit the list whole: no name or cost is cut short.
    #[test]
    fn every_row_shows_its_whole_name_and_cost_at_a_roomy_size() {
        for nation in [Nation::Earth, Nation::Fire, Nation::Water, Nation::Air] {
            let (s, rows) = rows(nation);
            let palette = Palette::new(&s.defs, nation, ColorDepth::TrueColor, false);
            let area = Rect::new(0, 0, 120, 40);
            let mut buf = Buffer::empty(area);
            draw(&s, &rows, 0, &palette, area, &mut buf);
            let text = crate::frame::to_text(&buf);
            for row in rows.iter().take(usize::from(HEIGHT) - 2) {
                assert!(text.contains(&row.name), "{nation:?} {}:\n{text}", row.name);
                if !row.locked() {
                    assert!(text.contains(&row.cost), "{nation:?} {}:\n{text}", row.cost);
                }
            }
        }
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
