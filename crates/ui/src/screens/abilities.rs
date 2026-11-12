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

use crate::bend::AbilityRow;
use crate::rows::{Marks, Row, Tone, fit, wrap};

/// The gap between two columns of the list.
pub const GAP: &str = "  ";

/// The most rows the detail strip may take, whatever the page: the list keeps the rest.
pub const MAX_STRIP: usize = 6;

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
pub struct Columns {
    name: usize,
    casters: usize,
}

impl Columns {
    pub fn of(rows: &[AbilityRow]) -> Self {
        let widest = |head: &str, f: fn(&AbilityRow) -> &str| {
            rows.iter().map(|r| f(r).chars().count()).chain([head.chars().count()]).max().unwrap()
        };
        Self { name: widest("ABILITY", |r| &r.name), casters: widest("BENDERS", |r| &r.casters) }
    }

    /// One row as spans, each cell padded to its column (the pointer's mark and a space come
    /// before the name). The last column takes whatever width is left, cut with an ellipsis.
    pub fn spans(
        &self,
        pointer: &str,
        cells: [(&str, Tone); 3],
        width: usize,
        ellipsis: char,
    ) -> Row {
        let [(name, name_tone), (casters, casters_tone), (last, last_tone)] = cells;
        let used = 2 + self.name + GAP.len() + self.casters + GAP.len();
        Row::spans(vec![
            (format!("{pointer} {name:<w$}", w = self.name), name_tone),
            (format!("{GAP}{casters:<w$}", w = self.casters), casters_tone),
            (format!("{GAP}{}", fit(last, width.saturating_sub(used), ellipsis)), last_tone),
        ])
    }

    pub fn header(&self, width: usize, ellipsis: char) -> Row {
        self.spans(
            " ",
            [("ABILITY", Tone::Dim), ("BENDERS", Tone::Dim), ("WHAT IT DOES", Tone::Dim)],
            width,
            ellipsis,
        )
    }

    /// One ability's row. One that can't be used now is dim throughout and says in the last
    /// column what it needs; the highlighted one is a title and carries the pointer.
    pub fn row(&self, row: &AbilityRow, chosen: bool, width: usize, marks: &Marks) -> Row {
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
pub fn detail_rows(row: &AbilityRow, width: usize, ellipsis: char) -> Vec<Row> {
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
    for effect in &row.effects {
        rows.extend(wrapped(effect, Tone::Dim));
    }
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
