//! The Nation screen (`N`, #308, docs/design/side-panel.md "The Nation screen"): every settler
//! grouped by activity (`fn_ui::view::nation_view`'s own order), a pointer that steps through
//! them (arrows/`W`/`S`, a page with `PgUp`/`PgDn`) and `Enter` selects the one under it; every
//! store and the nation's traits (today's panel's own Modifiers, moved here) sit under the
//! roster, with no pointer of their own.
//!
//! The roster shows a five-cell meter (with a `!` flag) for hunger, thirst, energy and health,
//! then the same five-cell meter for chi (never flagged: it can't hurt the unit, the selection
//! bar's own rule), a `Bends` column and a `Forms` column (`7/10`/`2/3`, or `12 *`/`4 *` for a
//! master: `fn_ui::text::mastery_cells`, docs/design/bending.md "Seeing it", #346, #361) -- when
//! the map region is at least
//! [`METER_MIN_WIDTH`] columns wide; narrower than that, a flag after the task names whichever
//! needs aren't ok instead (`fn_ui::view::need_word`), and chi and bends don't show at all (#346:
//! "Chi also satisfies #337's ... criterion", so the Nation screen is the one place both show).
//!
//! The meters sit at a fixed column just past a capped task column (#325), not stretched to the
//! screen's far edge on a wide terminal, under a header naming each one that stays put while the
//! roster scrolls (it sits above the scrolled window, not inside it). They wear their own glyphs
//! ([`need_glyphs`]), not the frame's own [`Marks::full`]/[`Marks::empty`]: those fill a cell's
//! whole height, and consecutive settlers with the same status would otherwise fuse into one
//! solid stripe instead of reading as one bar per person -- a problem the selection bar and the
//! side panel's research bar don't have, since each only ever draws one bar, never several
//! stacked rows of them.

use fn_sim::world::UnitId;
use fn_ui::Session;
use fn_ui::view::{NationView, NeedView, Status, UnitView, nation_view, need_word};
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;

use crate::state::UiState;
use crate::widgets::{Marks, Painter, Row, Tone, fit, meter, window_start};

/// At least this many columns in the map region (not the screen's own frame, which also carries
/// the panel beside it) before the meters show instead of a flag after the task.
pub const METER_MIN_WIDTH: u16 = 80;

/// A need meter is this many cells (the selection bar's own meters are ten; five, plus a flag
/// column, let four of them and the task text share the roster's width). Chi's meter shares it.
const NEED_CELLS: usize = 5;
const METER_COL: usize = NEED_CELLS + 1;
const METER_GAP: usize = 2;
/// The Bends and Forms columns' own width: wide enough for "100/100" to never truncate in
/// practice, and exactly [`METER_COL`] for the common one- and two-digit case the mockup shows
/// ("7/10", "12 *", "2/3", "4 *", `docs/design/bending.md` "Seeing it", #361).
const BENDS_COL: usize = METER_COL;
const FORMS_COL: usize = METER_COL;
/// Hunger, thirst, energy, health and chi (five meter columns), then Bends and Forms (text, no
/// flag).
const METERS_WIDTH: usize = METER_COL * 5 + BENDS_COL + FORMS_COL + METER_GAP * 6;

/// Hunger, thirst, energy and health, in the doc's own order (unlike the selection bar's
/// health-first `UnitView::needs`). Each is exactly [`METER_COL`] characters, so the header
/// built from them ([`meter_header`]) lines up over its meter with no extra padding.
const NEED_ORDER: [&str; 4] = ["Hunger", "Thirst", "Energy", "Health"];

fn ordered_needs(unit: &UnitView) -> impl Iterator<Item = &NeedView> {
    NEED_ORDER.into_iter().filter_map(move |label| unit.needs.iter().find(|n| n.label == label))
}

/// The task column's width before the meters start (`fit` truncates anything longer): capped so
/// the meters land just past the task text instead of stretching to the screen's far edge on a
/// wide terminal, but never wider than `width` leaves room for once the name, the gap and the
/// meters have theirs. The cap fits the longest everyday task whole ("Delivering 12 Ancient
/// Scrolls to a stockpile").
const TASK_CAP: usize = 46;

fn task_width(name_width: usize, width: usize) -> usize {
    width.saturating_sub(2 + name_width + 1 + METER_GAP + METERS_WIDTH).min(TASK_CAP)
}

/// The need meters' own filled/empty glyphs, not [`Marks::full`]/[`Marks::empty`] (#325): a
/// parallelogram that doesn't fill its cell's whole height, so one settler's meter doesn't visibly
/// touch the one above or below it even when both are entirely full. ASCII keeps `#` for full
/// (the same as the frame's own marks) but uses `-` rather than `.` for empty, since a run of dots
/// beside a meter reads as a decimal point.
fn need_glyphs(ascii: bool) -> (char, char) {
    if ascii { ('#', '-') } else { ('▰', '▱') }
}

/// The longest settler name in the roster, the width every row's name column shares.
fn name_width(view: &NationView) -> usize {
    view.groups.iter().flat_map(|(_, us)| us).map(|u| u.name.chars().count()).max().unwrap_or(0)
}

/// The column header above the meters (docs/design/side-panel.md "The Nation screen"): one row
/// naming each need in [`NEED_ORDER`]'s own order, aligned over its own meter. It is drawn once,
/// outside the roster's scrolled window, so it stays visible no matter how far the roster scrolls.
fn meter_header(name_width: usize, width: usize) -> Row {
    let pad = " ".repeat(2 + name_width + 1 + task_width(name_width, width) + METER_GAP);
    let mut spans = vec![(pad, Tone::Dim)];
    for (i, label) in NEED_ORDER.into_iter().chain(["Chi"]).enumerate() {
        if i > 0 {
            spans.push((" ".repeat(METER_GAP), Tone::Dim));
        }
        spans.push((format!("{label:<METER_COL$}"), Tone::Dim));
    }
    spans.push((" ".repeat(METER_GAP), Tone::Dim));
    spans.push((format!("{:<BENDS_COL$}", "Bends"), Tone::Dim));
    spans.push((" ".repeat(METER_GAP), Tone::Dim));
    spans.push((format!("{:<FORMS_COL$}", "Forms"), Tone::Dim));
    Row::spans(spans)
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
fn need_cells(n: &NeedView, glyphs: (char, char)) -> [(String, Tone); 3] {
    let tone = Tone::of(n.status);
    let (full, empty) = meter(n.frac, NEED_CELLS, glyphs.0, glyphs.1);
    let flag = if n.status == Status::Ok { " " } else { "!" }.to_owned();
    [(full, tone), (empty, Tone::Dim), (flag, tone)]
}

/// Chi's own meter and flag column, the same shape as [`need_cells`] but always in the accent
/// and never flagged: chi never hurts the unit, so it has no "low" threshold or warning colour
/// (the selection bar's own rule, docs/design/bending.md).
fn chi_cells(chi_frac: f32, glyphs: (char, char)) -> [(String, Tone); 3] {
    let (full, empty) = meter(chi_frac, NEED_CELLS, glyphs.0, glyphs.1);
    [(full, Tone::Accent), (empty, Tone::Dim), (" ".to_owned(), Tone::Accent)]
}

/// A settler's row with its meters: the pointer, the name, the task (truncated to a fixed
/// width, #325, so the meters stay near the name instead of drifting to the far edge), the four
/// need meters and chi's own (`glyphs`, [`need_glyphs`], rather than the frame's own marks), then
/// the Bends and Forms columns (`unit.bends_cell`/`unit.forms_cell`, `fn_ui::text::mastery_cells`,
/// docs/design/bending.md "Seeing it", #346, #361).
fn wide_unit_row(
    unit: &UnitView,
    chosen: bool,
    name_width: usize,
    width: usize,
    marks: &Marks,
    glyphs: (char, char),
) -> Row {
    let pointer = if chosen { marks.pointer } else { " " };
    let task_width = task_width(name_width, width);
    let task = format!("{:<task_width$}", fit(&unit.task, task_width, marks.ellipsis));
    let name_tone = if chosen { Tone::Title } else { Tone::Text };
    let mut spans = vec![
        (format!("{pointer} "), Tone::Title),
        (format!("{:<name_width$} ", unit.name), name_tone),
        (task, Tone::Text),
        // A cut-short task's ellipsis would otherwise butt straight against the first meter.
        (" ".repeat(METER_GAP), Tone::Dim),
    ];
    for (i, n) in ordered_needs(unit).enumerate() {
        if i > 0 {
            spans.push((" ".repeat(METER_GAP), Tone::Dim));
        }
        spans.extend(need_cells(n, glyphs));
    }
    spans.push((" ".repeat(METER_GAP), Tone::Dim));
    spans.extend(chi_cells(unit.chi_frac, glyphs));
    spans.push((" ".repeat(METER_GAP), Tone::Dim));
    spans.push((format!("{:<BENDS_COL$}", unit.bends_cell), Tone::Text));
    spans.push((" ".repeat(METER_GAP), Tone::Dim));
    spans.push((format!("{:<FORMS_COL$}", unit.forms_cell), Tone::Text));
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
    glyphs: (char, char),
) -> Vec<Line> {
    let name_width = name_width(view);
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
                wide_unit_row(unit, chosen, name_width, width, marks, glyphs)
            } else {
                narrow_unit_row(unit, chosen, name_width, marks)
            };
            out.push(Line { row, unit: true });
        }
    }
    out
}

/// Draws the screen's contents into `inner` (the frame's inside, minus its padding): the header
/// line, the meter column header (wide only, #325, outside the roster's own scroll so it never
/// scrolls away), the roster (windowed round the pointer), every store, then the nation's traits.
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
    let glyphs = need_glyphs(palette.ascii);
    let view = nation_view(session);
    let width = usize::from(inner.width);
    let height = usize::from(inner.height);

    let store_lines =
        packed_rows(view.stores.iter().map(|r| format!("{} {}", r.name, r.count)), width);
    let trait_lines = packed_rows(view.traits.iter().map(|(l, v)| format!("{l} {v}")), width);

    let store_block = 1 + store_lines.len().max(1);
    let trait_block = if trait_lines.is_empty() { 0 } else { 1 + 1 + trait_lines.len() };
    let footer_rows = 1 + store_block + trait_block;
    let header_rows = if wide { 3 } else { 2 };
    let roster_h = height.saturating_sub(header_rows + footer_rows).max(1);

    let lines = roster_lines(&view, ui.nation_cursor, wide, width, &marks, glyphs);
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
    if wide {
        p.row(&meter_header(name_width(&view), width));
    }
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
        let lines = roster_lines(&v, 0, true, 120, &marks, need_glyphs(false));
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
        let row = wide_unit_row(&v.groups[0].1[0], false, 4, 120, &marks, need_glyphs(false));
        assert!(!row.plain().contains("Starving"), "{}", row.plain());
        assert!(row.plain().contains('!'), "{}", row.plain());
    }

    /// The wide row carries chi's own meter (always unflagged, even with a need in trouble) and
    /// the Bends and Forms columns, in `fn_ui::text::mastery_cells`'s own text
    /// (`docs/design/bending.md` "Seeing it", #346, #361), right after the four need meters.
    #[test]
    fn a_wide_row_carries_chi_bends_and_forms() {
        let mut s = new_session(7, Nation::Earth);
        s.world.units[0].needs.hunger = 0.0;
        let v = nation_view(&s);
        let unit = &v.groups[0].1[0];
        assert_eq!(unit.bends_cell, "0/8", "Earth's own threshold, the bundled bends_per_nation");
        assert_eq!(unit.forms_cell, "0/3", "the bundled master_forms");
        let marks = Marks::new(true);
        let row = wide_unit_row(unit, false, 4, 120, &marks, need_glyphs(true));
        let plain = row.plain();
        // Hunger (zeroed, flagged) - Thirst, Energy, Health (full, unflagged) - Chi (full,
        // never flagged even though a need just before it is) - Bends ("0/8") - Forms ("0/3").
        let tail = "-----!  #####   #####   #####   #####   0/8     0/3   ";
        assert!(plain.ends_with(tail), "{plain}\nwant suffix {tail:?}");
    }

    /// Once a settler's bends and forms meet the threshold, the Bends and Forms columns read
    /// `fn_ui::text::mastery_cells`'s master form ("12 *"/"4 *") instead of "n/target".
    #[test]
    fn a_wide_row_marks_a_master_in_the_bends_and_forms_columns() {
        let s = new_session(7, Nation::Earth);
        let v = nation_view(&s);
        let mut unit = v.groups[0].1[0].clone();
        unit.bends_cell = "12 *".to_owned();
        unit.forms_cell = "4 *".to_owned();
        let marks = Marks::new(true);
        let row = wide_unit_row(&unit, false, 4, 120, &marks, need_glyphs(true));
        assert!(row.plain().ends_with("12 *    4 *   "), "{}", row.plain());
    }

    /// The header names each need right above its own meter: the pad before it matches
    /// `wide_unit_row`'s own fixed columns, so "Hunger" lands over the hunger meter's first cell.
    #[test]
    fn the_meter_header_lines_up_over_the_first_meter() {
        let header = meter_header(4, 120).plain();
        let marks = Marks::new(false);
        let row = wide_unit_row(
            &view(7, Nation::Earth).groups[0].1[0],
            false,
            4,
            120,
            &marks,
            need_glyphs(false),
        );
        let meter_start = row.plain().chars().position(|c| c == '▰' || c == '▱').unwrap();
        assert_eq!(header.find("Hunger"), Some(meter_start), "{header}\n{}", row.plain());
        // Chi, Bends and Forms (#346, #361) follow the four needs, in that order.
        assert!(header.find("Chi").unwrap() > header.find("Health").unwrap(), "{header}");
        assert!(header.find("Bends").unwrap() > header.find("Chi").unwrap(), "{header}");
        assert!(header.find("Forms").unwrap() > header.find("Bends").unwrap(), "{header}");
    }

    /// A terminal wide enough for the meters still caps the task column instead of stretching it
    /// to the far edge (#325): the meters sit a fixed distance past the name, not past whatever
    /// `width` happens to be.
    #[test]
    fn the_task_column_is_capped_on_a_wide_terminal() {
        assert_eq!(task_width(4, 400), TASK_CAP);
        assert!(task_width(4, 60) < TASK_CAP, "a narrow terminal still shrinks it");
    }
}
