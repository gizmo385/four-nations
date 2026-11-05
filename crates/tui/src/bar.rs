//! The selection bar (docs/design/tui.md, "Screen layout"): seven rows along the bottom of the map
//! area, present only while a unit is selected. Its top edge names the unit and its nation; inside
//! are what it is doing, what it carries and where it stands, and its meters as 10-cell bars in
//! the `ok`, `warn` and `bad` tokens, with a `!` on any meter that is not ok so they read without
//! colour. Chi is in the accent and never warns: it can't hurt the unit.

use fn_ui::Session;
use fn_ui::view::{Status, UnitView, unit_view};
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;

use crate::color::Palette;
use crate::state::{BAR_HEIGHT, UiState};
use crate::widgets::{Marks, Painter, Row, TitleAt, Tone, draw_frame, meter, wrap};

/// A meter bar is this many cells.
const BAR_CELLS: usize = 10;
/// The label column of a meter ("Hunger"), and the whole meter: label, space, bar, space, a
/// percentage like "100%", and the mark.
const LABEL: usize = 6;
const METER_WIDTH: u16 = (LABEL + 1 + BAR_CELLS + 1 + 4 + 1) as u16;
/// Content rows inside the frame.
const ROWS: u16 = BAR_HEIGHT as u16 - 2;

/// Where the bar goes: the bottom rows of the map region.
pub fn bar_area(map: Rect) -> Rect {
    let height = (BAR_HEIGHT as u16).min(map.height);
    Rect::new(map.x, map.bottom() - height, map.width, height)
}

/// One meter as a row: `Hunger ███████░░░  72%!`. The `!` marks anything that is not ok.
pub fn meter_row(label: &str, frac: f32, tone: Tone, flag: bool, marks: &Marks) -> Row {
    let (full, empty) = meter(frac, BAR_CELLS, marks.full, marks.empty);
    Row::spans(vec![
        (format!("{label:<LABEL$} "), Tone::Dim),
        (full, tone),
        (empty, Tone::Dim),
        (format!(" {:>4}", format!("{:.0}%", frac * 100.0)), Tone::Text),
        (if flag { "!" } else { " " }.to_owned(), tone),
    ])
}

/// The meters, top to bottom: health, hunger, thirst, energy, then chi.
pub fn meter_rows(view: &UnitView, marks: &Marks) -> Vec<Row> {
    let mut rows: Vec<Row> = view
        .needs
        .iter()
        .map(|n| meter_row(n.label, n.frac, Tone::of(n.status), n.status != Status::Ok, marks))
        .collect();
    rows.push(meter_row("Chi", view.chi_frac, Tone::Accent, false, marks));
    rows
}

/// The key hints, as many as fit in `width`, the most useful first.
fn key_hints(following: bool, width: usize) -> Row {
    let follow = if following { "unfollow" } else { "follow" };
    let hints = [("Esc", "close"), ("Tab", "next"), ("F", follow)];
    let len = |n: usize| -> usize {
        hints[..n].iter().map(|(k, w)| k.len() + w.len() + 3).sum::<usize>() + 2 * (n - 1)
    };
    let n = (1..=hints.len()).rev().find(|&n| len(n) <= width).unwrap_or(1);
    let mut spans = Vec::new();
    for (i, (key, what)) in hints[..n].iter().enumerate() {
        if i > 0 {
            spans.push(("  ".to_owned(), Tone::Dim));
        }
        spans.push((format!("[{key}]"), Tone::Accent));
        spans.push((format!(" {what}"), Tone::Dim));
    }
    Row::spans(spans)
}

/// What the left column says: the task (up to two lines), what the unit carries, and its tile --
/// or, while it's inside a building (#226, #229), which one ("In the Academy"). `avatar_rite`
/// marks the moment the Avatar holds the Harmony Rite (#242): the task line wears its own look,
/// bold on the nation's accent, so the Avatar State reads apart from anything else he could be
/// doing.
pub fn detail_rows(view: &UnitView, width: usize, ellipsis: char, avatar_rite: bool) -> Vec<Row> {
    let task_tone = if avatar_rite { Tone::Avatar } else { Tone::Text };
    let mut rows: Vec<Row> =
        wrap(&view.task, width, 2, ellipsis).into_iter().map(|l| Row::text(l, task_tone)).collect();
    rows.push(Row::text(format!("Carrying: {}", view.carrying), Tone::Text));
    let where_ = match &view.inside {
        Some(building) => format!("In the {building}"),
        None => format!("At ({}, {})", view.tile.x, view.tile.y),
    };
    rows.push(Row::text(where_, Tone::Dim));
    rows
}

/// Draws the bar for the selected unit into `area` (see [`bar_area`]). Draws nothing without one.
pub fn draw_bar(session: &Session, ui: &UiState, palette: &Palette, area: Rect, buf: &mut Buffer) {
    let Some(unit) = ui.selected_unit(session) else { return };
    if area.height < BAR_HEIGHT as u16 || area.width < METER_WIDTH + 12 {
        return;
    }
    let mut view = unit_view(session, unit);
    // While the unit channels a bend, the task line names the ability and the time left
    // (#278), in place of the plain task `unit_view` gives every other frontend.
    if let Some(text) = crate::panel::channel_text(session, unit) {
        view.task = text;
    }
    // The Avatar State (#238, #242): the moment he holds the Harmony Rite, marked apart on the
    // task line.
    let avatar_rite = unit.is_avatar() && unit.task() == Some(fn_sim::world::Task::Rite);
    let marks = Marks::new(palette.ascii);
    let title = format!(" {} {} {} ", view.name, marks.dot, view.nation);
    draw_frame(buf, area, palette, &title, TitleAt::Left, "");

    let inner = Rect::new(area.x + 2, area.y + 1, area.width - 4, ROWS);
    let left_width = inner.width - METER_WIDTH - 2;
    let left = Rect::new(inner.x, inner.y, left_width, ROWS);
    let right = Rect::new(inner.right() - METER_WIDTH, inner.y, METER_WIDTH, ROWS);

    let mut p = Painter::new(buf, left, palette);
    for row in detail_rows(&view, usize::from(left_width), marks.ellipsis, avatar_rite)
        .iter()
        .take(ROWS as usize - 1)
    {
        p.row(row);
    }
    // The key hints sit on the last row.
    p.row = ROWS - 1;
    p.row(&key_hints(ui.following, usize::from(left_width)));

    let mut p = Painter::new(buf, right, palette);
    for row in meter_rows(&view, &marks) {
        p.row(&row);
    }
}

#[cfg(test)]
mod tests {
    use fn_sim::Nation;

    use super::*;

    #[test]
    fn a_meter_row_is_label_bar_percent_and_a_mark_only_when_not_ok() {
        let m = Marks::new(true);
        let ok = meter_row("Hunger", 0.72, Tone::Ok, false, &m);
        assert_eq!(ok.plain(), "Hunger #######...  72% ");
        let low = meter_row("Energy", 0.23, Tone::Bad, true, &m);
        assert_eq!(low.plain(), "Energy ##........  23%!");
        let full = meter_row("Health", 1.0, Tone::Ok, false, &m);
        assert_eq!(full.plain(), "Health ########## 100% ");
        assert_eq!(ok.plain().chars().count(), usize::from(METER_WIDTH));
        // The bar wears the status, the empty cells are dim.
        assert_eq!(low.left[1].1, Tone::Bad);
        assert_eq!(low.left[2].1, Tone::Dim);
    }

    #[test]
    fn a_unit_s_meters_flag_exactly_the_ones_that_are_not_ok() {
        let mut s = Session::new(7, Nation::Earth, 48, 32);
        let id = s.world.units[0].id;
        s.world.units[0].needs.energy = 0.0;
        let unit = s.world.units.iter().find(|u| u.id == id).unwrap();
        let view = unit_view(&s, unit);
        let rows = meter_rows(&view, &Marks::new(false));
        assert_eq!(rows.len(), 5, "health, hunger, thirst, energy and chi");
        let flagged: Vec<&str> = view
            .needs
            .iter()
            .zip(&rows)
            .filter(|(_, r)| r.plain().ends_with('!'))
            .map(|(n, _)| n.label)
            .collect();
        assert_eq!(flagged, ["Energy"]);
        assert!(!rows[4].plain().ends_with('!'), "chi never warns");
        assert!(rows[4].plain().starts_with("Chi"));
    }

    #[test]
    fn the_details_say_the_task_what_is_carried_and_the_tile() {
        let s = Session::new(7, Nation::Earth, 48, 32);
        let view = unit_view(&s, &s.world.units[0]);
        let rows = detail_rows(&view, 40, '…', false);
        let text: Vec<String> = rows.iter().map(Row::plain).collect();
        assert_eq!(text[0], view.task);
        assert_eq!(text[1], "Carrying: nothing");
        assert_eq!(text[2], format!("At ({}, {})", view.tile.x, view.tile.y));
    }

    /// The Harmony Rite's Avatar State (#238, #242) marks the task line apart: bold, on the
    /// nation's accent, not the panel; anything else he (or anyone else) is doing keeps the
    /// plain look.
    #[test]
    fn the_avatar_state_marks_the_task_line_apart() {
        let s = Session::new(7, Nation::Fire, 48, 32);
        let view = unit_view(&s, &s.world.units[0]);
        let plain = detail_rows(&view, 40, '…', false);
        assert_eq!(plain[0].left[0].1, Tone::Text);
        let rite = detail_rows(&view, 40, '…', true);
        assert_eq!(rite[0].left[0].1, Tone::Avatar);
        // Only the task line wears it, not what the unit carries or where it stands.
        assert_eq!(rite[1].left[0].1, Tone::Text);
    }

    /// A unit inside a building (#226, #229) reads "In the Academy" in place of its tile.
    #[test]
    fn the_details_name_the_building_a_unit_is_inside() {
        use fn_sim::building::Building;
        use fn_sim::geom::Footprint;
        let mut s = Session::new(7, Nation::Fire, 64, 64);
        let hall = s.defs.building_id("study_hall").unwrap();
        let passable = s.defs.building(hall).passable;
        let at = fn_sim::Pos::new(30, 30);
        for dy in 0..3 {
            for dx in 0..3 {
                let t = s.world.map.get_mut(fn_sim::Pos::new(at.x + dx, at.y + dy)).unwrap();
                t.terrain = fn_sim::Terrain::Land;
                t.resource = None;
            }
        }
        let foot = Footprint::new(at, 2, 2);
        for p in foot.tiles() {
            let part = [(p.x - at.x) as u8, (p.y - at.y) as u8];
            let b = Building { part, size: [2, 2], ..Building::new(hall, passable, false) };
            s.world.map.get_mut(p).unwrap().building = Some(b);
        }
        s.world.units[0].pos = foot.entrance();
        assert!(s.world.enter_building(0, at, &s.defs));
        let view = unit_view(&s, &s.world.units[0]);
        let rows = detail_rows(&view, 40, '…', false);
        assert_eq!(rows[2].plain(), "In the Academy");
    }

    #[test]
    fn the_key_hints_shrink_to_fit() {
        let plain = |w| key_hints(false, w).plain();
        assert_eq!(plain(60), "[Esc] close  [Tab] next  [F] follow");
        assert_eq!(plain(30), "[Esc] close  [Tab] next");
        assert_eq!(plain(15), "[Esc] close");
        assert_eq!(key_hints(true, 60).plain(), "[Esc] close  [Tab] next  [F] unfollow");
    }

    #[test]
    fn the_bar_sits_on_the_bottom_rows_of_the_map() {
        let a = bar_area(Rect::new(0, 0, 88, 39));
        assert_eq!(a, Rect::new(0, 32, 88, 7));
    }
}
