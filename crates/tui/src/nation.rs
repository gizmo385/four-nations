//! The Nation screen (`N`, #308, docs/design/side-panel.md "The Nation screen"): every settler
//! grouped by activity (`fn_ui::view::nation_view`'s own order), a pointer that steps through
//! them (arrows/`W`/`S`, a page with `PgUp`/`PgDn`) and `Enter` selects the one under it; every
//! store and the nation's traits (today's panel's own Modifiers, moved here) sit under the
//! roster, with no pointer of their own.
//!
//! The roster shows a five-cell meter (with a `!` flag) for hunger, thirst, energy and health
//! when the map region is at least [`METER_MIN_WIDTH`] columns wide; narrower than that, a flag
//! after the task names whichever aren't ok instead (`fn_ui::view::need_word`).

use fn_sim::world::UnitId;
use fn_ui::Session;
use fn_ui::view::{NationView, NeedView, Status, UnitView, nation_view, need_word};
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;

use crate::state::UiState;
use crate::widgets::{Marks, Painter, Row, Tone, fit, meter};

/// At least this many columns in the map region (not the screen's own frame, which also carries
/// the panel beside it) before the meters show instead of a flag after the task.
pub const METER_MIN_WIDTH: u16 = 80;

/// A need meter is this many cells (the selection bar's own meters are ten; five, plus a flag
/// column, let four of them and the task text share the roster's width).
const NEED_CELLS: usize = 5;
const METER_COL: usize = NEED_CELLS + 1;
const METER_GAP: usize = 2;
const METERS_WIDTH: usize = METER_COL * 4 + METER_GAP * 3;

/// Hunger, thirst, energy and health, in the doc's own order (unlike the selection bar's
/// health-first `UnitView::needs`).
const NEED_ORDER: [&str; 4] = ["Hunger", "Thirst", "Energy", "Health"];

fn ordered_needs(unit: &UnitView) -> impl Iterator<Item = &NeedView> {
    NEED_ORDER.into_iter().filter_map(move |label| unit.needs.iter().find(|n| n.label == label))
}

/// Every settler the pointer can land on, in the screen's own order: its groups
/// (`Activity::ALL`'s order), `World::units`' own order within one.
pub fn roster(view: &NationView) -> Vec<UnitId> {
    view.groups.iter().flat_map(|(_, units)| units.iter().map(|u| u.id)).collect()
}

/// The cursor after a step of `dy` settlers (arrows/`W`/`S` pass its sign; `PgUp`/`PgDn` a page),
/// stopping at the roster's edges.
pub fn step(view: &NationView, at: usize, dy: i32) -> usize {
    let n = roster(view).len();
    if n == 0 {
        return 0;
    }
    (at as i64 + i64::from(dy)).clamp(0, n as i64 - 1) as usize
}

/// The first row that shows so that `at` does: the roster starts at the top and slides only once
/// the pointer passes its far edge (the same rule the research grid and the saves list follow).
fn window_start(at: usize, shown: usize) -> usize {
    (at + 1).saturating_sub(shown)
}

/// One line of the roster area: a group's heading, or a settler's own row.
struct Line {
    row: Row,
    /// Whether this line is a settler (and so can be the pointer's target), in the roster's own
    /// order.
    unit: bool,
}

/// One need's five-cell meter and flag column: the filled cells and the empty ones wear the
/// status (`ok`, `warn`, `bad`), and the flag only shows once it isn't ok, the same shape the
/// selection bar's own meters use (`crate::bar::meter_row`), without the label.
fn need_cells(n: &NeedView, marks: &Marks) -> [(String, Tone); 3] {
    let tone = Tone::of(n.status);
    let (full, empty) = meter(n.frac, NEED_CELLS, marks);
    let flag = if n.status == Status::Ok { " " } else { "!" }.to_owned();
    [(full, tone), (empty, Tone::Dim), (flag, tone)]
}

/// A settler's row with its meters: the pointer, the name, the task (truncated to leave room),
/// then the four need meters.
fn wide_unit_row(
    unit: &UnitView,
    chosen: bool,
    name_width: usize,
    width: usize,
    marks: &Marks,
) -> Row {
    let pointer = if chosen { marks.pointer } else { " " };
    let task_width = width.saturating_sub(2 + name_width + 1 + METERS_WIDTH);
    let task = format!("{:<task_width$}", fit(&unit.task, task_width, marks.ellipsis));
    let name_tone = if chosen { Tone::Title } else { Tone::Text };
    let mut spans = vec![
        (format!("{pointer} "), Tone::Title),
        (format!("{:<name_width$} ", unit.name), name_tone),
        (task, Tone::Text),
    ];
    for (i, n) in ordered_needs(unit).enumerate() {
        if i > 0 {
            spans.push((" ".repeat(METER_GAP), Tone::Dim));
        }
        spans.extend(need_cells(n, marks));
    }
    Row::spans(spans)
}

/// A settler's row with no room for meters: the pointer, the name, the task, then a flag for
/// each need that isn't ok (`Idle  Hungry!`); none while everyone is fine.
fn narrow_unit_row(unit: &UnitView, chosen: bool, name_width: usize, marks: &Marks) -> Row {
    let pointer = if chosen { marks.pointer } else { " " };
    let name_tone = if chosen { Tone::Title } else { Tone::Text };
    let flags: String = ordered_needs(unit)
        .filter_map(|n| need_word(n.label, n.status))
        .map(|w| format!("  {w}!"))
        .collect();
    Row::spans(vec![
        (format!("{pointer} "), Tone::Title),
        (format!("{:<name_width$} ", unit.name), name_tone),
        (format!("{}{flags}", unit.task), Tone::Text),
    ])
}

/// `items`, packed as "name value" entries into as few lines as fit `width` (the Stores and
/// Traits sections' own shape, the same packing the panel's `store_rows` uses).
fn packed_rows(items: impl Iterator<Item = String>, width: usize) -> Vec<Row> {
    let mut rows = Vec::new();
    let mut line = String::new();
    for entry in items {
        if !line.is_empty() && line.chars().count() + 2 + entry.chars().count() > width {
            rows.push(Row::text(std::mem::take(&mut line), Tone::Text));
        }
        if !line.is_empty() {
            line.push_str("  ");
        }
        line.push_str(&entry);
    }
    if !line.is_empty() {
        rows.push(Row::text(line, Tone::Text));
    }
    rows
}

/// The roster's lines, in order: a group's heading, then its settlers (wide or narrow rows,
/// `wide`'s own choice). `at` is the roster index of the settler the pointer marks.
fn roster_lines(
    view: &NationView,
    at: usize,
    wide: bool,
    width: usize,
    marks: &Marks,
) -> Vec<Line> {
    let name_width = view
        .groups
        .iter()
        .flat_map(|(_, us)| us)
        .map(|u| u.name.chars().count())
        .max()
        .unwrap_or(0);
    let units = roster(view);
    let mut out = Vec::new();
    for (activity, group) in &view.groups {
        out.push(Line {
            row: Row::text(
                format!("{} {}", activity.label().to_uppercase(), group.len()),
                Tone::Title,
            ),
            unit: false,
        });
        for unit in group {
            let chosen = units.get(at) == Some(&unit.id);
            let row = if wide {
                wide_unit_row(unit, chosen, name_width, width, marks)
            } else {
                narrow_unit_row(unit, chosen, name_width, marks)
            };
            out.push(Line { row, unit: true });
        }
    }
    out
}

/// Draws the screen's contents into `inner` (the frame's inside, minus its padding): the header
/// line, the roster (windowed round the pointer), every store, then the nation's traits.
/// `wide` says whether the map region is at least [`METER_MIN_WIDTH`] columns -- the frame around
/// `inner`, not `inner` itself, since the panel beside it already eats into `inner`'s own width.
pub fn draw(
    session: &Session,
    ui: &UiState,
    wide: bool,
    palette: &crate::color::Palette,
    inner: Rect,
    buf: &mut Buffer,
) {
    let marks = Marks::new(palette.ascii);
    let view = nation_view(session);
    let width = usize::from(inner.width);
    let height = usize::from(inner.height);

    let store_lines =
        packed_rows(view.stores.iter().map(|r| format!("{} {}", r.name, r.count)), width);
    let trait_lines = packed_rows(view.traits.iter().map(|(l, v)| format!("{l} {v}")), width);

    let store_block = 1 + store_lines.len().max(1);
    let trait_block = if trait_lines.is_empty() { 0 } else { 1 + 1 + trait_lines.len() };
    let footer_rows = 1 + store_block + trait_block;
    let header_rows = 2;
    let roster_h = height.saturating_sub(header_rows + footer_rows).max(1);

    let lines = roster_lines(&view, ui.nation_cursor, wide, width, &marks);
    let chosen_row =
        lines.iter().enumerate().filter(|(_, l)| l.unit).nth(ui.nation_cursor).map(|(i, _)| i);
    let first = window_start(chosen_row.unwrap_or(0), roster_h);

    let mut p = Painter::new(buf, inner, palette);
    p.row(&Row::pair(
        format!(
            "{} {} seed {} {} {} people",
            view.nation, marks.dot, view.seed, marks.dot, view.population
        ),
        Tone::Title,
        view.clock.clone(),
        Tone::Dim,
    ));
    p.blank();
    for line in lines.iter().skip(first).take(roster_h) {
        p.row(&line.row);
    }

    p.row = (header_rows + roster_h) as u16;
    p.blank();
    p.row(&Row::text("STORES", Tone::Title));
    if store_lines.is_empty() {
        p.row(&Row::text("Nothing yet", Tone::Dim));
    } else {
        for row in &store_lines {
            p.row(row);
        }
    }
    if !trait_lines.is_empty() {
        p.blank();
        p.row(&Row::text("TRAITS", Tone::Title));
        for row in &trait_lines {
            p.row(row);
        }
    }
}

#[cfg(test)]
mod tests {
    use fn_sim::Nation;

    use super::*;
    use crate::new_session;

    fn view(seed: u64, nation: Nation) -> NationView {
        nation_view(&new_session(seed, nation))
    }

    #[test]
    fn stepping_stops_at_the_roster_s_edges() {
        let v = view(7, Nation::Earth);
        let n = roster(&v).len();
        assert_eq!(step(&v, 0, -1), 0);
        assert_eq!(step(&v, 0, 1), 1);
        assert_eq!(step(&v, n - 1, 1), n - 1);
        assert_eq!(step(&v, 0, n as i32 + 5), n - 1, "a page past the end stops at the last");
    }

    #[test]
    fn the_window_slides_only_once_the_pointer_passes_its_edge() {
        assert_eq!((0..3).map(|at| window_start(at, 3)).collect::<Vec<_>>(), [0, 0, 0]);
        assert_eq!((3..5).map(|at| window_start(at, 3)).collect::<Vec<_>>(), [1, 2]);
    }

    /// A fresh settlement is every settler Idle: one heading, then one row per settler, the
    /// first one marked (the pointer starts there).
    #[test]
    fn the_roster_lines_up_a_heading_then_a_row_per_settler() {
        let v = view(7, Nation::Earth);
        let marks = Marks::new(false);
        let lines = roster_lines(&v, 0, true, 120, &marks);
        assert_eq!(lines[0].row.plain(), format!("IDLE {}", v.groups[0].1.len()));
        assert!(!lines[0].unit);
        assert!(lines[1].unit);
        assert!(lines[1].row.plain().starts_with('▸'), "{}", lines[1].row.plain());
        assert!(!lines[2].row.plain().starts_with('▸'));
    }

    /// Narrower than the meter width, a unit with something wrong carries a flag after its task
    /// instead of a meter.
    #[test]
    fn a_narrow_row_flags_trouble_after_the_task() {
        let mut s = new_session(7, Nation::Earth);
        s.world.units[0].needs.hunger = 0.0;
        let v = nation_view(&s);
        let marks = Marks::new(false);
        let row = narrow_unit_row(&v.groups[0].1[0], false, 4, &marks);
        assert!(row.plain().contains("Starving!"), "{}", row.plain());
    }

    /// Wide enough, the same unit's row carries a flagged meter instead, and the text has no
    /// flag word (the meter's `!` is the only mark).
    #[test]
    fn a_wide_row_carries_a_flagged_meter_instead_of_a_word() {
        let mut s = new_session(7, Nation::Earth);
        s.world.units[0].needs.hunger = 0.0;
        let v = nation_view(&s);
        let marks = Marks::new(false);
        let row = wide_unit_row(&v.groups[0].1[0], false, 4, 120, &marks);
        assert!(!row.plain().contains("Starving"), "{}", row.plain());
        assert!(row.plain().contains('!'), "{}", row.plain());
    }
}
