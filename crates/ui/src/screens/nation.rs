//! The Nation screen (`N`, #308, docs/design/side-panel.md "The Nation screen"): every settler
//! grouped by activity (`crate::view::nation_view`'s own order, each group under its full word,
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
//! (`crate::text::bending_words`), which is the first thing dropped when DOING would get too
//! narrow: at 80 columns every meter stays and BENDING goes.
//!
//! Full meters are solid `█`, so settlers' meters stacked in rows would read as one block (#779):
//! when the page can show every settler with a blank row between neighbours of a group, it does
//! (`spaced`); when it can't (80x24 with a handful of settlers) the rows stack as before, so no
//! settler is pushed off the page for it.
//!
//! Narrower than [`METER_MIN_WIDTH`] columns (never at a size the game draws at, since the
//! screen covers the side panel too, #652) a flag after the task names whichever needs aren't
//! ok instead (`crate::view::need_word`).

use crate::Session;
use crate::rows::{Marks, Row, Tone, fit, meter, wrap};
use crate::view::{NationView, NeedView, Status, UnitView, need_word};
use fn_sim::world::UnitId;

/// At least this many columns in the page (the whole terminal's width since #652) before the
/// meters show instead of a flag after the task.
pub const METER_MIN_WIDTH: u16 = 80;

/// A need meter is this many cells (five, plus a flag column, let four of them, the task and the
/// bending words share the roster's width).
pub const NEED_CELLS: usize = 5;

pub const METER_COL: usize = NEED_CELLS + 1;

pub const GAP: usize = 2;

/// Hunger, thirst, energy and health.
pub const METERS_WIDTH: usize = METER_COL * 4 + GAP * 3;

/// The BENDING column goes when DOING would get narrower than this with it.
pub const DOING_MIN: usize = 24;

/// The widest the BENDING column gets ("Earthbending Master, 100 bends").
pub const BENDING_CAP: usize = 30;

/// Hunger, thirst, energy and health, in the doc's own order (unlike the selection bar's
/// health-first `UnitView::needs`).
pub const NEED_ORDER: [&str; 4] = ["Hunger", "Thirst", "Energy", "Health"];

pub fn ordered_needs(unit: &UnitView) -> impl Iterator<Item = &NeedView> {
    NEED_ORDER.into_iter().filter_map(move |label| unit.needs.iter().find(|n| n.label == label))
}

/// The roster's column widths, shared by its header and every row so they line up.
pub struct Columns {
    name: usize,
    /// Whatever the other columns leave.
    doing: usize,
    /// `None` when the page is too narrow for it: it is dropped before any meter.
    bending: Option<usize>,
}

impl Columns {
    pub fn of(view: &NationView, width: usize) -> Self {
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
pub fn fit_words(text: &str, width: usize, ellipsis: char) -> String {
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
pub fn header(cols: &Columns) -> Row {
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
pub struct Line {
    pub row: Row,
    /// Whether this line is a settler (and so can be the pointer's target), in the roster's own
    /// order.
    pub unit: bool,
}

/// One need's five-cell meter and its flag column (`crate::widgets::meter`): the cells wear the
/// status (`ok`, `warn`, `bad`), the whole meter wears the warning once it isn't ok, and the
/// `!` sits directly against it, the same shape the inspector's own meters use
/// (`crate::inspector::meter_row`), without the label.
pub fn need_cells(n: &NeedView, marks: &Marks) -> [(String, Tone); 3] {
    meter(n.frac, NEED_CELLS, Tone::of(n.status), n.status != Status::Ok, marks)
}

/// A settler's row: the pointer, the name, the task (the rest of the width, cut at a word only
/// when it can't fit), the four need meters, then the bending words when there is room.
pub fn wide_unit_row(unit: &UnitView, chosen: bool, cols: &Columns, marks: &Marks) -> Row {
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
pub fn narrow_unit_row(unit: &UnitView, chosen: bool, name_width: usize, marks: &Marks) -> Row {
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
pub fn packed_rows(items: impl Iterator<Item = String>, width: usize) -> Vec<Row> {
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
/// `wide`'s own choice), a blank line between two settlers of a group when `spaced` (#779: full
/// meters would fuse into a block). `at` is the roster index of the settler the pointer marks.
pub fn roster_lines(
    view: &NationView,
    at: usize,
    wide: bool,
    spaced: bool,
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
        for (i, unit) in group.iter().enumerate() {
            if spaced && i > 0 {
                out.push(Line { row: Row::text("", Tone::Text), unit: false });
            }
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
/// settler the inspector describes (`crate::view::settler_view`, so a bend being channelled
/// reads the same), their task in full with what they carry, and their progress to the next rank.
pub fn detail(session: &Session, id: UnitId, rows: usize, width: usize, marks: &Marks) -> Vec<Row> {
    let Some(unit) = session.world.units.iter().find(|u| u.id == id) else { return Vec::new() };
    let view = crate::view::settler_view(session, unit);
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

#[cfg(test)]
mod tests {
    use fn_sim::Nation;

    use super::*;
    use crate::new_session;
    use crate::rows::window_start;
    use crate::view::nation_view;

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
        let lines = roster_lines(&v, 0, true, false, 116, &marks);
        assert_eq!(lines[0].row.plain(), format!("IDLE {}", v.groups[0].1.len()));
        assert!(!lines[0].unit);
        assert!(lines[1].unit);
        assert!(lines[1].row.plain().starts_with('▸'), "{}", lines[1].row.plain());
        assert!(!lines[2].row.plain().starts_with('▸'));
    }

    /// A blank line goes between two settlers of a group when `spaced`, never before the first,
    /// after the last or between a heading and its settler (#779).
    #[test]
    fn spaced_lines_put_a_blank_between_settlers_of_a_group_only() {
        let v = view(7, Nation::Earth);
        let marks = Marks::new(false);
        let n = roster(&v).len();
        let packed = roster_lines(&v, 0, true, false, 116, &marks);
        let spaced = roster_lines(&v, 0, true, true, 116, &marks);
        assert_eq!(packed.len(), n + 1, "a heading and a row each");
        assert_eq!(spaced.len(), packed.len() + n - 1, "one group: n - 1 blanks");
        assert!(spaced[1].unit && !spaced[2].unit && spaced[2].row.plain().trim().is_empty());
        assert!(spaced[3].unit && spaced.last().unwrap().unit);
        assert_eq!(spaced.iter().filter(|l| l.unit).count(), n, "blanks are not settlers");
    }

    /// Group headings are full words, not the side panel's short verbs.
    #[test]
    fn group_headings_are_full_words() {
        let s = new_session(7, Nation::Earth);
        let marks = Marks::new(false);
        let mut v = nation_view(&s);
        let unit = v.groups[0].1.pop().unwrap();
        v.groups.push((crate::view::Activity::Eat, vec![unit.clone()]));
        v.groups.push((crate::view::Activity::Haul, vec![unit]));
        let lines = roster_lines(&v, 0, true, false, 116, &marks);
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
