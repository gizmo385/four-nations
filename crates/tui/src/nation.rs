//! The Nation screen (`N`, #308, docs/design/side-panel.md "The Nation screen"): every settler
//! grouped by activity (`fn_ui::view::nation_view`'s own order, each group under its full word,
//! "HAULING 3", "EATING AND DRINKING 7"), a pointer that steps through them (arrows/`W`/`S`, a
//! page with `PgUp`/`PgDn`) and `Enter` selects the one under it; every store (with `full` after
//! one no stockpile can take more of) and the nation's traits (today's panel's own Modifiers,
//! moved here) sit under the roster, with no pointer of their own.
//!
//! The roster is a table under a header that stays put while the roster scrolls (it sits above
//! the scrolled window, not inside it, #660):
//!
//! ```text
//!   NAME  DOING                                  HUNGER  THIRST  ENERGY  HEALTH  BENDING
//! HAULING 3
//! ▸ Hua   Delivering 2 Fruit to the stockpile    ██░░░   ███░░   ██░░░   █████   Student, 4 bends
//! ```
//!
//! DOING takes every column the others leave, so a task is whole unless the screen is narrow;
//! one that still does not fit is cut at a word, and the detail strip under the rule shows it in
//! full, with the settler's progress to the next rank. The four needs are the game's one meter
//! (#654, `crate::widgets::meter`) at five cells: a low need wears the warning colour with the
//! `!` directly against it, and the selected row keeps those colours (`page::highlight` only
//! changes the background). Chi, bends and forms are one BENDING column in words
//! (`fn_ui::text::bending_words`), which is the first thing dropped when DOING would get too
//! narrow: at 80 columns every meter stays and BENDING goes.
//!
//! Narrower than [`METER_MIN_WIDTH`] columns (never at a size the game draws at, since the
//! screen covers the side panel too, #652) a flag after the task names whichever needs aren't
//! ok instead (`fn_ui::view::need_word`).

use fn_sim::world::UnitId;
use fn_ui::Session;
use fn_ui::view::{NationView, NeedView, Status, UnitView, nation_view, need_word};
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;

use crate::page::{self, Page};
use crate::state::UiState;
use crate::widgets::{Marks, Painter, Row, Tone, fit, meter, window_start, wrap};

/// At least this many columns in the page (the whole terminal's width since #652) before the
/// meters show instead of a flag after the task.
pub const METER_MIN_WIDTH: u16 = 80;

/// A need meter is this many cells (five, plus a flag column, let four of them, the task and the
/// bending words share the roster's width).
const NEED_CELLS: usize = 5;
const METER_COL: usize = NEED_CELLS + 1;
const GAP: usize = 2;
/// Hunger, thirst, energy and health.
const METERS_WIDTH: usize = METER_COL * 4 + GAP * 3;
/// The BENDING column goes when DOING would get narrower than this with it.
const DOING_MIN: usize = 24;
/// The widest the BENDING column gets ("Earthbending Master, 100 bends").
const BENDING_CAP: usize = 30;

/// Hunger, thirst, energy and health, in the doc's own order (unlike the selection bar's
/// health-first `UnitView::needs`).
const NEED_ORDER: [&str; 4] = ["Hunger", "Thirst", "Energy", "Health"];

fn ordered_needs(unit: &UnitView) -> impl Iterator<Item = &NeedView> {
    NEED_ORDER.into_iter().filter_map(move |label| unit.needs.iter().find(|n| n.label == label))
}

/// The roster's column widths, shared by its header and every row so they line up.
struct Columns {
    name: usize,
    /// Whatever the other columns leave.
    doing: usize,
    /// `None` when the page is too narrow for it: it is dropped before any meter.
    bending: Option<usize>,
}

impl Columns {
    fn of(view: &NationView, width: usize) -> Self {
        let units = || view.groups.iter().flat_map(|(_, us)| us);
        let name = units().map(|u| u.name.chars().count()).max().unwrap_or(0).max("NAME".len());
        let bending = units()
            .map(|u| u.bending.chars().count())
            .max()
            .unwrap_or(0)
            .clamp("BENDING".len(), BENDING_CAP);
        let room = width.saturating_sub(2 + name + GAP + GAP + METERS_WIDTH);
        let with = room.saturating_sub(GAP + bending);
        if with >= DOING_MIN {
            Self { name, doing: with, bending: Some(bending) }
        } else {
            Self { name, doing: room, bending: None }
        }
    }
}

/// `text` cut to `width` cells at a word, with an ellipsis, if it doesn't fit (a single word
/// longer than the column is cut where it must be).
fn fit_words(text: &str, width: usize, ellipsis: char) -> String {
    if text.chars().count() <= width {
        return text.to_owned();
    }
    let head: String = text.chars().take(width.saturating_sub(1)).collect();
    // The cut is clean when the next character is a space; else the last word is a part of one.
    let clean = text.chars().nth(head.chars().count()) == Some(' ');
    let words = if clean { head.as_str() } else { head.rsplit_once(' ').map_or("", |(w, _)| w) };
    if words.trim_end().is_empty() {
        return fit(text, width, ellipsis);
    }
    format!("{}{ellipsis}", words.trim_end())
}

/// The column header above the roster: NAME, DOING, each need and BENDING over their own
/// columns. It is drawn once, outside the roster's scrolled window.
fn header(cols: &Columns) -> Row {
    let mut spans = vec![
        ("  ".to_owned(), Tone::Dim),
        (format!("{:<w$}", "NAME", w = cols.name + GAP), Tone::Dim),
        (format!("{:<w$}", "DOING", w = cols.doing + GAP), Tone::Dim),
    ];
    for (i, label) in NEED_ORDER.into_iter().enumerate() {
        if i > 0 {
            spans.push((" ".repeat(GAP), Tone::Dim));
        }
        spans.push((format!("{:<METER_COL$}", label.to_uppercase()), Tone::Dim));
    }
    if cols.bending.is_some() {
        spans.push((" ".repeat(GAP), Tone::Dim));
        spans.push(("BENDING".to_owned(), Tone::Dim));
    }
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

/// One need's five-cell meter and its flag column (`crate::widgets::meter`): the cells wear the
/// status (`ok`, `warn`, `bad`), the whole meter wears the warning once it isn't ok, and the
/// `!` sits directly against it, the same shape the inspector's own meters use
/// (`crate::inspector::meter_row`), without the label.
fn need_cells(n: &NeedView, marks: &Marks) -> [(String, Tone); 3] {
    meter(n.frac, NEED_CELLS, Tone::of(n.status), n.status != Status::Ok, marks)
}

/// A settler's row: the pointer, the name, the task (the rest of the width, cut at a word only
/// when it can't fit), the four need meters, then the bending words when there is room.
fn wide_unit_row(unit: &UnitView, chosen: bool, cols: &Columns, marks: &Marks) -> Row {
    let pointer = if chosen { marks.pointer } else { " " };
    let task = fit_words(&unit.task, cols.doing, marks.ellipsis);
    let name_tone = if chosen { Tone::Title } else { Tone::Text };
    let mut spans = vec![
        (format!("{pointer} "), Tone::Title),
        (format!("{:<w$}", unit.name, w = cols.name + GAP), name_tone),
        (format!("{task:<w$}", w = cols.doing + GAP), Tone::Text),
    ];
    for (i, n) in ordered_needs(unit).enumerate() {
        if i > 0 {
            spans.push((" ".repeat(GAP), Tone::Dim));
        }
        spans.extend(need_cells(n, marks));
    }
    if let Some(w) = cols.bending {
        spans.push((" ".repeat(GAP), Tone::Dim));
        spans.push((fit_words(&unit.bending, w, marks.ellipsis), Tone::Text));
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
    let cols = Columns::of(view, width);
    let units = roster(view);
    let mut out = Vec::new();
    for (activity, group) in &view.groups {
        out.push(Line {
            row: Row::text(
                format!("{} {}", activity.heading().to_uppercase(), group.len()),
                Tone::Title,
            ),
            unit: false,
        });
        for unit in group {
            let chosen = units.get(at) == Some(&unit.id);
            let row = if wide {
                wide_unit_row(unit, chosen, &cols, marks)
            } else {
                narrow_unit_row(unit, chosen, cols.name, marks)
            };
            out.push(Line { row, unit: true });
        }
    }
    out
}

/// The rows the detail strip takes on a page `page_height` rows tall: the settler's name and
/// rank, two lines for the task and what they carry, and the progress to the next rank; one task
/// line on a short page, so the roster keeps its rows.
pub fn detail_rows(page_height: u16) -> u16 {
    if page_height >= 30 { 4 } else { 3 }
}

/// The detail strip's rows for the settler `id`, `rows` of them, wrapped to `width`: the same
/// settler the inspector describes (`crate::inspector::settler_view`, so a bend being channelled
/// reads the same), their task in full with what they carry, and their progress to the next rank.
fn detail(session: &Session, id: UnitId, rows: usize, width: usize, marks: &Marks) -> Vec<Row> {
    let Some(unit) = session.world.units.iter().find(|u| u.id == id) else { return Vec::new() };
    let view = crate::inspector::settler_view(session, unit);
    let who = format!("{} {} {}", view.name, marks.dot, view.rank_title);
    let hint = "Enter: go to them on the map";
    let mut out = vec![if who.chars().count() + 2 + hint.chars().count() <= width {
        Row::pair(who, Tone::Title, hint, Tone::Dim)
    } else {
        Row::text(who, Tone::Title)
    }];
    let doing = format!("{}. Carrying {}.", view.task.trim_end_matches('.'), view.carrying);
    let task_lines = rows.saturating_sub(2).max(1);
    let paragraph = |text: &str, max: usize| -> Vec<Row> {
        wrap(text, width, max, marks.ellipsis)
            .into_iter()
            .map(|l| Row::text(l, Tone::Text))
            .collect()
    };
    out.extend(paragraph(&doing, task_lines));
    out.extend(paragraph(&format!("{}.", view.mastery_line.trim_end_matches('.')), 1));
    out
}

/// Draws the screen's contents into `page.body` (the frame's inside, minus its padding and the
/// detail strip): the header line, the column header (wide only, outside the roster's own scroll
/// so it never scrolls away), the roster (windowed round the pointer), every store, then the
/// nation's traits; and the settler under the pointer's detail in `page.detail`. `wide` says
/// whether the page is at least [`METER_MIN_WIDTH`] columns. The settler under the pointer wears
/// the selection across the page (`page::highlight`): a quiet background, never reverse video,
/// so its meters keep their own colours.
pub fn draw(
    session: &Session,
    ui: &UiState,
    wide: bool,
    palette: &crate::color::Palette,
    page: &Page,
    buf: &mut Buffer,
) -> Option<String> {
    let inner = page.body;
    let marks = Marks::new(palette.ascii);
    let view = nation_view(session);
    let width = usize::from(inner.width);
    let height = usize::from(inner.height);

    let store_lines = packed_rows(
        view.stores
            .iter()
            .map(|r| format!("{} {}{}", r.name, r.count, if r.full { " full" } else { "" })),
        width,
    );
    let trait_lines = packed_rows(view.traits.iter().map(|(l, v)| format!("{l} {v}")), width);

    let store_block = 1 + store_lines.len().max(1);
    let trait_block = if trait_lines.is_empty() { 0 } else { 1 + 1 + trait_lines.len() };
    let footer_rows = 1 + store_block + trait_block;
    let header_rows = if wide { 3 } else { 2 };
    let roster_h = height.saturating_sub(header_rows + footer_rows).max(1);

    let lines = roster_lines(&view, ui.nation_cursor, wide, width, &marks);
    let chosen_row =
        lines.iter().enumerate().filter(|(_, l)| l.unit).nth(ui.nation_cursor).map(|(i, _)| i);
    let first = window_start(chosen_row.unwrap_or(0), roster_h);

    let mut p = Painter::new(buf, inner, palette);
    p.row(&Row::pair(
        format!(
            "{} {} {} people {} seed {}",
            view.nation, marks.dot, view.population, marks.dot, view.seed
        ),
        Tone::Title,
        crate::widgets::clock(&view.clock, &marks),
        Tone::Dim,
    ));
    p.blank();
    if wide {
        p.row(&header(&Columns::of(&view, width)));
    }
    let mut chosen_at = None;
    for (i, line) in lines.iter().enumerate().skip(first).take(roster_h) {
        if Some(i) == chosen_row {
            chosen_at = Some(inner.y + p.row);
        }
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

    if let Some(y) = chosen_at.filter(|&y| y < inner.bottom()) {
        page::highlight(p.buf, Rect::new(inner.x, y, inner.width, 1), palette);
    }

    if let (Some(strip), Some(&id)) = (page.detail, roster(&view).get(ui.nation_cursor)) {
        let rows = detail(session, id, usize::from(strip.height), usize::from(strip.width), &marks);
        let mut d = Painter::new(buf, strip, palette);
        for row in &rows {
            d.row(row);
        }
    }

    // Where the roster's window sits, in settlers (the headings between groups don't count).
    let settlers = |lines: &[Line]| lines.iter().filter(|l| l.unit).count();
    let end = (first + roster_h).min(lines.len());
    fn_ui::text::window_label(
        "settlers",
        settlers(&lines[..first.min(lines.len())]),
        settlers(&lines[first.min(end)..end]),
        settlers(&lines),
    )
}

#[cfg(test)]
mod tests {
    use fn_sim::Nation;

    use super::*;
    use crate::new_session;

    fn view(seed: u64, nation: Nation) -> NationView {
        nation_view(&new_session(seed, nation))
    }

    fn wide_row(view: &NationView, unit: &UnitView, width: usize, marks: &Marks) -> Row {
        wide_unit_row(unit, false, &Columns::of(view, width), marks)
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
        let lines = roster_lines(&v, 0, true, 116, &marks);
        assert_eq!(lines[0].row.plain(), format!("IDLE {}", v.groups[0].1.len()));
        assert!(!lines[0].unit);
        assert!(lines[1].unit);
        assert!(lines[1].row.plain().starts_with('▸'), "{}", lines[1].row.plain());
        assert!(!lines[2].row.plain().starts_with('▸'));
    }

    /// Group headings are full words, not the side panel's short verbs.
    #[test]
    fn group_headings_are_full_words() {
        let s = new_session(7, Nation::Earth);
        let marks = Marks::new(false);
        let mut v = nation_view(&s);
        let unit = v.groups[0].1.pop().unwrap();
        v.groups.push((fn_ui::view::Activity::Eat, vec![unit.clone()]));
        v.groups.push((fn_ui::view::Activity::Haul, vec![unit]));
        let lines = roster_lines(&v, 0, true, 116, &marks);
        let headings: Vec<String> =
            lines.iter().filter(|l| !l.unit).map(|l| l.row.plain()).collect();
        assert!(headings.contains(&"EATING AND DRINKING 1".to_owned()), "{headings:?}");
        assert!(headings.contains(&"HAULING 1".to_owned()), "{headings:?}");
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
        let row = wide_row(&v, &v.groups[0].1[0], 116, &marks);
        assert!(!row.plain().contains("Starving"), "{}", row.plain());
        assert!(row.plain().contains('!'), "{}", row.plain());
    }

    /// The wide row ends in four meters (the first zeroed and flagged) and then the bending
    /// words: chi, bends and forms are one column, in words.
    #[test]
    fn a_wide_row_carries_four_meters_then_the_bending_in_words() {
        let mut s = new_session(7, Nation::Earth);
        s.world.units[0].needs.hunger = 0.0;
        let v = nation_view(&s);
        let unit = &v.groups[0].1[0];
        assert_eq!(unit.bending, "Student, 0 bends");
        let marks = Marks::new(true);
        let plain = wide_row(&v, unit, 116, &marks).plain();
        assert_eq!(plain.matches('#').count(), 15, "three full meters, hunger empty: {plain}");
        assert!(plain.ends_with(".....!  #####   #####   #####   Student, 0 bends"), "{plain}");
        assert!(!plain.contains("Chi") && !plain.contains("0/8"), "{plain}");
    }

    /// The header names each need right above its own meter and BENDING above the words.
    #[test]
    fn the_header_lines_up_over_the_meters_and_the_bending_words() {
        let v = view(7, Nation::Earth);
        let cols = Columns::of(&v, 116);
        let header = header(&cols).plain();
        let marks = Marks::new(false);
        let row = wide_row(&v, &v.groups[0].1[0], 116, &marks).plain();
        let meter_start = row.chars().position(|c| c == '█' || c == '░').unwrap();
        assert_eq!(header.find("HUNGER"), Some(meter_start), "{header}\n{row}");
        let words = row[..row.find("Student").unwrap()].chars().count();
        assert_eq!(header.find("BENDING"), Some(words), "{header}\n{row}");
        assert!(header.starts_with("  NAME  DOING"), "{header}");
        for (a, b) in [("HUNGER", "THIRST"), ("THIRST", "ENERGY"), ("ENERGY", "HEALTH")] {
            assert!(header.find(a).unwrap() < header.find(b).unwrap(), "{header}");
        }
        assert!(!header.contains("CHI") && !header.contains("FORMS"), "{header}");
    }

    /// DOING takes every column the others leave: the row fills the page's width exactly.
    #[test]
    fn the_task_column_takes_every_column_the_others_leave() {
        let v = view(7, Nation::Earth);
        let marks = Marks::new(false);
        for width in [100, 116, 200] {
            let cols = Columns::of(&v, width);
            let row = wide_row(&v, &v.groups[0].1[0], width, &marks).plain();
            assert_eq!(row.chars().count(), width, "width {width}: {row}");
            assert!(cols.doing > 24, "width {width}");
        }
        assert!(Columns::of(&v, 200).doing > Columns::of(&v, 116).doing);
    }

    /// At 80 columns (a 76-column body) BENDING is gone and all four meters remain.
    #[test]
    fn at_80_columns_bending_goes_before_any_meter() {
        let v = view(7, Nation::Earth);
        let cols = Columns::of(&v, 76);
        assert_eq!(cols.bending, None);
        let marks = Marks::new(false);
        let row = wide_row(&v, &v.groups[0].1[0], 76, &marks).plain();
        let header = header(&cols).plain();
        for text in [&row, &header] {
            assert!(!text.contains("Student") && !text.contains("BENDING"), "{text}");
        }
        assert_eq!(row.matches('█').count(), 20, "all four meters: {row}");
        for label in NEED_ORDER {
            assert!(header.contains(&label.to_uppercase()), "{header}");
        }
        assert!(row.chars().count() <= 76, "{row}");
    }

    /// A task too long for the column is cut at a word, never mid-word.
    #[test]
    fn a_task_too_long_is_cut_at_a_word() {
        assert_eq!(
            fit_words("Delivering 2 Fruit to the stockpile", 40, '…'),
            "Delivering 2 Fruit to the stockpile"
        );
        assert_eq!(
            fit_words("Delivering 2 Fruit to the stockpile", 20, '…'),
            "Delivering 2 Fruit…"
        );
        assert_eq!(
            fit_words("Delivering 2 Fruit to the stockpile", 19, '…'),
            "Delivering 2 Fruit…"
        );
        assert_eq!(fit_words("Delivering 2 Fruit to the stockpile", 18, '…'), "Delivering 2…");
        assert_eq!(fit_words("Supercalifragilistic and more", 8, '…'), "Superca…");
        assert_eq!(fit_words("ab cd", 4, '…'), "ab…");
    }
}
