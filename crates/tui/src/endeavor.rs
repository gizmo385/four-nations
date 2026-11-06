//! The Endeavor screen (`G`, docs/design/endgame.md "Showing progress"): the same text as the
//! side panel's own section builds on, laid out as a full page -- the endeavor's name and how
//! many signs are met, its description, the stage tracker (what the gate's site is building, or
//! that the endeavor hasn't begun), then every sign grouped under its heading, each with a mark
//! once it is met, a hint while it isn't, its count and a 10-cell bar, like the selection bar's
//! meters. What it shows is `fn_ui::endeavor::screen_view`; this module only lays it out as rows
//! that scroll (`screens::draw_screen`).

use fn_ui::Session;
use fn_ui::endeavor::{GroupView, SignRow, screen_view};

use crate::widgets::{Marks, Row, Tone, meter, wrap};

/// A sign's bar is this many cells, the same as the selection bar's meters.
const BAR_CELLS: usize = 10;

/// One sign's row (a mark, its name, a bar and its count), the masters sign's own "Closest" row
/// while it isn't met, and, while it isn't met, a last row with its hint.
fn sign_rows(sign: &SignRow, name_width: usize, marks: &Marks) -> Vec<Row> {
    let (mark, mark_tone) = if sign.met { (marks.known, Tone::Ok) } else { (" ", Tone::Dim) };
    let name_tone = if sign.met { Tone::Dim } else { Tone::Text };
    let bar_tone = if sign.met { Tone::Ok } else { Tone::Accent };
    let (full, empty) = meter(sign.frac, BAR_CELLS, marks.full, marks.empty);
    let mut rows = vec![Row::spans(vec![
        (format!("{mark} "), mark_tone),
        (format!("{:<name_width$} ", sign.name), name_tone),
        (full, bar_tone),
        (empty, Tone::Dim),
        (format!(" {:>9}", sign.value), Tone::Dim),
    ])];
    if !sign.met {
        if let Some(closest) = &sign.closest {
            rows.push(Row::text(format!("  {closest}"), Tone::Text));
        }
        if let Some(hint) = &sign.hint {
            rows.push(Row::text(format!("  {hint}"), Tone::Warn));
        }
    }
    rows
}

/// One heading and its signs, in data order.
fn group_rows(group: &GroupView, name_width: usize, marks: &Marks) -> Vec<Row> {
    let mut rows = vec![Row::text(group.heading.to_uppercase(), Tone::Title)];
    for sign in &group.signs {
        rows.extend(sign_rows(sign, name_width, marks));
    }
    rows
}

/// The screen's text, `width` columns wide: the name and count, the description if the data has
/// one, a blank line, the stage tracker, then every group with a blank line before it.
pub fn lines(session: &Session, width: usize, marks: &Marks) -> Vec<Row> {
    let v = screen_view(session);
    let name_width =
        v.groups.iter().flat_map(|g| &g.signs).map(|s| s.name.chars().count()).max().unwrap_or(0);
    let mut rows = vec![Row::pair(v.name.clone(), Tone::Title, v.count.clone(), Tone::Accent)];
    if !v.description.is_empty() {
        rows.extend(
            wrap(&v.description, width, usize::MAX, marks.ellipsis)
                .into_iter()
                .map(|l| Row::text(l, Tone::Dim)),
        );
    }
    rows.push(Row::default());
    rows.push(Row::text(v.stage.clone(), Tone::Strong));
    for group in &v.groups {
        rows.push(Row::default());
        rows.extend(group_rows(group, name_width, marks));
    }
    rows
}

#[cfg(test)]
mod tests {
    use fn_sim::Nation;

    use super::*;
    use crate::new_session;

    #[test]
    fn the_screen_opens_with_the_name_the_count_and_the_stage() {
        let s = new_session(7, Nation::Earth);
        let marks = Marks::new(false);
        let rows = lines(&s, 80, &marks);
        let text: Vec<String> = rows.iter().map(Row::plain).collect();
        assert!(text[0].starts_with("The Harmony Gate"), "{text:?}");
        // Ten signs are designed (`docs/design/endgame.md`), but the data has only the ones
        // whose system exists yet (farm plots, #198, haven't landed): nine today.
        assert!(
            text[0].ends_with(&format!("0 of {} signs", s.defs.endeavor.signs.len())),
            "{text:?}"
        );
        assert!(text.contains(&"Stage: not begun".to_owned()), "{text:?}");
        assert!(text.iter().any(|t| t == "PEOPLE"), "{text:?}");
    }

    /// A met sign is marked, an unmet one carries its hint on the next line, and the hints only
    /// ever follow an unmet sign.
    #[test]
    fn a_met_sign_is_marked_and_an_unmet_one_carries_its_hint() {
        let s = new_session(7, Nation::Earth);
        let marks = Marks::new(false);
        let rows = lines(&s, 80, &marks);
        let pop = rows.iter().find(|r| r.plain().contains("Population")).unwrap();
        assert_eq!(pop.left[0].1, Tone::Dim, "not yet met");
        let hint = rows.iter().find(|r| r.plain().contains("Spare beds")).unwrap();
        assert_eq!(hint.left[0].1, Tone::Warn);
    }

    /// The masters sign carries its own "Closest" row (in plain text, not a warning) right
    /// before its hint row (`docs/design/endgame.md`, "Showing mastery", #347), whose hint names
    /// the Bend tool's own key and says how to start (#382).
    #[test]
    fn the_masters_sign_carries_a_closest_row_before_its_hint() {
        let s = new_session(7, Nation::Fire);
        let marks = Marks::new(false);
        let rows = lines(&s, 80, &marks);
        let i = rows.iter().position(|r| r.plain().contains("Bending masters")).unwrap();
        let closest = &rows[i + 1];
        assert!(closest.plain().contains("Closest:"), "{:?}", closest.plain());
        assert_eq!(closest.left[0].1, Tone::Text);
        let hint = &rows[i + 2];
        assert_eq!(
            hint.plain(),
            "  Designate a bend (B) on a tile a bender can work - credit to whoever channels it"
        );
        assert_eq!(hint.left[0].1, Tone::Warn);
    }

    /// Every sign's bar is 10 cells, as the selection bar's meters are.
    #[test]
    fn every_sign_s_bar_is_ten_cells() {
        let s = new_session(7, Nation::Earth);
        let marks = Marks::new(true);
        let rows = lines(&s, 80, &marks);
        for row in &rows {
            if let [(_, _), (_, _), (full, _), (empty, _), (_, _)] = row.left.as_slice() {
                assert_eq!(full.chars().count() + empty.chars().count(), BAR_CELLS, "{row:?}");
            }
        }
    }
}
