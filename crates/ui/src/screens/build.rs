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
//! with one can't be reached that way). `1` to `9` put the highlight on that row (#952).

use crate::Tool;
use crate::build::{BuildingDetail, PickerRow};
use crate::input::{Action, map_key};
use crate::keyev::{KeyCode, KeyEvent};
use crate::rows::{Marks, Row, Tone, wrap};

/// Whether `key` closes the screen: `Esc`, or the key that opened it (`U`, either case, #652).
pub fn closes(key: KeyEvent) -> bool {
    key.code == KeyCode::Esc || matches!(map_key(key), Some(Action::PickTool(Tool::Build)))
}

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

/// The gap between two columns of the list.
pub const GAP: &str = "  ";

/// The most rows the detail strip may take, whatever the page: the list keeps the rest.
pub const MAX_STRIP: usize = 9;

/// The list's column widths, each as wide as its widest entry on show (the whole list, not just
/// the rows in view, so the columns stay put as it scrolls) and at least as wide as its header.
pub struct Columns {
    name: usize,
    price: usize,
    size: usize,
}

impl Columns {
    pub fn of(rows: &[PickerRow]) -> Self {
        let widest = |head: &str, f: fn(&PickerRow) -> &str| {
            rows.iter().map(|r| f(r).chars().count()).chain([head.chars().count()]).max().unwrap()
        };
        Self {
            name: widest("BUILDING", |r| &r.name),
            price: widest("COST", |r| &r.price),
            size: widest("SIZE", |r| &r.size),
        }
    }

    /// One row of the list as spans, each cell padded to its column (the pointer's mark and a
    /// space come before the name). The needs column is last and takes whatever width is left.
    pub fn spans(&self, pointer: &str, cells: [(&str, Tone); 4]) -> Row {
        let [(name, name_tone), (price, price_tone), (size, size_tone), (needs, needs_tone)] =
            cells;
        Row::spans(vec![
            (format!("{pointer} {name:<w$}", w = self.name), name_tone),
            (format!("{GAP}{price:<w$}", w = self.price), price_tone),
            (format!("{GAP}{size:<w$}", w = self.size), size_tone),
            (format!("{GAP}{needs}"), needs_tone),
        ])
    }

    /// The header row: the four column names, dim.
    pub fn header(&self) -> Row {
        self.spans(
            " ",
            [
                ("BUILDING", Tone::Dim),
                ("COST", Tone::Dim),
                ("SIZE", Tone::Dim),
                ("NEEDS", Tone::Dim),
            ],
        )
    }

    /// One building's row. A locked one is dim throughout and names the tech that unlocks it in
    /// the needs column; a buildable one writes its cost in the warning colour when stores
    /// cannot pay it; the highlighted one is a title and carries the pointer.
    pub fn row(&self, row: &PickerRow, chosen: bool, marks: &Marks) -> Row {
        let pointer = if chosen { marks.pointer } else { " " };
        let locked = row.locked();
        let name_tone = match (locked, chosen) {
            (true, _) => Tone::Dim,
            (false, true) => Tone::Title,
            (false, false) => Tone::Text,
        };
        let price_tone = match (locked, row.affordable) {
            (true, _) => Tone::Dim,
            (false, true) => Tone::Text,
            (false, false) => Tone::Warn,
        };
        let size_tone = if locked { Tone::Dim } else { Tone::Text };
        let needs = row.tech.as_deref().unwrap_or("");
        self.spans(
            pointer,
            [
                (&row.name, name_tone),
                (&row.price, price_tone),
                (&row.size, size_tone),
                (needs, Tone::Dim),
            ],
        )
    }
}

/// The detail strip's rows for `detail`, wrapped to `width` and never cut: its name with its
/// cost against stores on the right, what is short of it, what unlocks a locked one and why
/// `Enter` does nothing, what it does, and its size and where it can go.
pub fn detail_rows(detail: &BuildingDetail, width: usize, ellipsis: char) -> Vec<Row> {
    let wrapped = |text: &str, tone: Tone| -> Vec<Row> {
        wrap(text, width, usize::MAX, ellipsis).into_iter().map(|l| Row::text(l, tone)).collect()
    };
    let locked = detail.tech.is_some();
    let short = detail.short_of();
    let cost_tone = if locked {
        Tone::Dim
    } else if short.is_some() {
        Tone::Warn
    } else {
        Tone::Text
    };
    let cost = detail.cost_line();
    let mut rows = Vec::new();
    // The cost sits flush right of the name when both fit one line, else under it.
    if detail.name.chars().count() + GAP.len() + cost.chars().count() <= width {
        rows.push(Row::pair(detail.name.clone(), Tone::Title, cost, cost_tone));
    } else {
        rows.push(Row::text(detail.name.clone(), Tone::Title));
        rows.extend(wrapped(&cost, cost_tone));
    }
    if let Some(tech) = &detail.tech {
        rows.extend(wrapped(
            &format!("Needs {tech}. Research it first: Enter builds nothing until then."),
            Tone::Warn,
        ));
    } else if let Some(short) = short {
        rows.extend(wrapped(&format!("Short of {short}."), Tone::Warn));
    }
    for line in &detail.effects {
        rows.extend(wrapped(line, Tone::Text));
    }
    rows.extend(wrapped(&format!("Size {}. {}", detail.footprint, detail.placement), Tone::Dim));
    rows
}
