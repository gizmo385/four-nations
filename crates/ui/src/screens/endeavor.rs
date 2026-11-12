//! The Endeavor screen (`G`, docs/design/endgame.md "Showing progress"): the same text as the
//! side panel's own section builds on, laid out as a full page -- the endeavor's name and how
//! many signs are met, its description, the stage tracker (what the gate's site is building, or
//! that the endeavor hasn't begun) with what can be done with the gate now (#643: `Enter`
//! chooses a site once Ready, `X` withdraws one no work has started on), then every sign grouped under its heading, each with a mark
//! once it is met, a hint while it isn't, its count and a 10-cell bar, like the selection bar's
//! meters. What it shows is `crate::endeavor::screen_view`; this module only lays it out as rows
//! that scroll (`screens::draw_screen`).

use crate::Session;
use crate::endeavor::{GroupView, SignRow, UnitedView, screen_view};

use crate::rows::{Marks, Row, Tone, meter, wrap};

/// A sign's bar is this many cells, the same as the selection bar's meters.
const BAR_CELLS: usize = 10;

/// One sign's row (a mark, its name, a bar and its count), the masters sign's own "Closest" row
/// while it isn't met, and, while it isn't met, a last row with its hint.
fn sign_rows(sign: &SignRow, name_width: usize, width: usize, marks: &Marks) -> Vec<Row> {
    let (mark, mark_tone) = if sign.met { (marks.known, Tone::Ok) } else { (" ", Tone::Dim) };
    let name_tone = if sign.met { Tone::Dim } else { Tone::Text };
    let bar_tone = if sign.met { Tone::Ok } else { Tone::Accent };
    let [full, empty, flag] = meter(sign.frac, BAR_CELLS, bar_tone, false, marks);
    let mut rows = vec![Row::spans(vec![
        (format!("{mark} "), mark_tone),
        (format!("{:<name_width$} ", sign.name), name_tone),
        full,
        empty,
        flag,
        (format!("{:>9}", sign.value), Tone::Dim),
    ])];
    if !sign.met {
        if let Some(closest) = &sign.closest {
            // Wrapped, since the names in words run long (#663).
            for line in wrap(closest, width.saturating_sub(2), usize::MAX, marks.ellipsis) {
                rows.push(Row::text(format!("  {line}"), Tone::Text));
            }
        }
        // The scrolls sign's own list (#380, #403): one row per marked scroll or charted ruin,
        // nearest first, with its state. Empty until there is one, leaving only the hint.
        for found in &sign.found {
            rows.push(Row::text(
                format!("  {} - {} - {}", found.name, found.distance, found.status),
                Tone::Text,
            ));
        }
        if let Some(hint) = &sign.hint {
            rows.push(Row::text(format!("  {hint}"), Tone::Warn));
        }
    }
    rows
}

/// One heading and its signs, in data order.
fn group_rows(group: &GroupView, name_width: usize, width: usize, marks: &Marks) -> Vec<Row> {
    let mut rows = vec![Row::text(group.heading.to_uppercase(), Tone::Title)];
    for sign in &group.signs {
        rows.extend(sign_rows(sign, name_width, width, marks));
    }
    rows
}

/// The United City's rows (#442), under the stage line: the people by nation, then the next
/// population milestone with a bar toward it, or that there are no more.
pub fn united_rows(u: &UnitedView, marks: &Marks) -> Vec<Row> {
    let mut rows = vec![Row::default(), Row::text("PEOPLE", Tone::Title)];
    let name_width = u.by_nation.iter().map(|(n, _)| n.chars().count()).max().unwrap_or(0);
    for (name, count) in &u.by_nation {
        rows.push(Row::spans(vec![
            (format!("  {name:<name_width$} "), Tone::Text),
            (format!("{count:>3}"), Tone::Dim),
        ]));
    }
    rows.push(Row::default());
    rows.push(Row::text("NEXT MILESTONE", Tone::Title));
    match u.next {
        Some(next) => {
            let [full, empty, flag] = meter(u.frac, BAR_CELLS, Tone::Accent, false, marks);
            rows.push(Row::spans(vec![
                ("  ".to_owned(), Tone::Dim),
                full,
                empty,
                flag,
                (format!(" {}/{next} people", u.population), Tone::Text),
            ]));
        }
        None => rows.push(Row::text("  Every milestone is reached", Tone::Dim)),
    }
    rows
}

/// The screen's text, `width` columns wide: the name and count, the description if the data has
/// one, a blank line, the stage tracker, then every group with a blank line before it.
pub fn lines(session: &Session, width: usize, marks: &Marks) -> Vec<Row> {
    let v = screen_view(session);
    let chapter_signs = v.chapter.iter().flat_map(|c| &c.signs);
    let name_width = v
        .groups
        .iter()
        .flat_map(|g| &g.signs)
        .chain(chapter_signs)
        .map(|s| s.name.chars().count())
        .max()
        .unwrap_or(0);
    let mut rows = vec![Row::pair(v.name.clone(), Tone::Title, v.count.clone(), Tone::Accent)];
    if !v.description.is_empty() {
        rows.extend(
            wrap(&v.description, width, usize::MAX, marks.ellipsis)
                .into_iter()
                .map(|l| Row::text(l, Tone::Dim)),
        );
    }
    // The United City (#442): the gate stands, so its stage and then who lives there and the next
    // milestone take the place of the chapter and the signs.
    if let Some(u) = &v.united {
        rows.push(Row::default());
        rows.push(Row::text(v.stage.clone(), Tone::Strong));
        rows.extend(united_rows(u, marks));
        return rows;
    }
    // The chapter (#439), at the top: its signs and what it brings until it's met, then one line
    // with its name.
    if let Some(c) = &v.chapter {
        rows.push(Row::default());
        rows.push(Row::text("CHAPTER", Tone::Title));
        if c.met {
            rows.push(Row::spans(vec![
                (format!("{} ", marks.known), Tone::Ok),
                (c.name.clone(), Tone::Dim),
            ]));
        } else {
            rows.push(Row::pair(
                format!("  {}", c.name),
                Tone::Strong,
                c.count.clone(),
                Tone::Accent,
            ));
            for sign in &c.signs {
                rows.extend(sign_rows(sign, name_width, width, marks));
            }
            if let Some(hint) = &c.hint {
                rows.push(Row::text(format!("  {hint}"), Tone::Dim));
            }
        }
    }
    rows.push(Row::default());
    rows.push(Row::text(v.stage.clone(), Tone::Strong));
    // What can be done with the gate now (#643): place it once Ready, withdraw it until work
    // starts. The whole line is the hint.
    if let Some(action) = &v.action {
        rows.push(Row::text(action.clone(), Tone::Accent));
    }
    for group in &v.groups {
        rows.push(Row::default());
        rows.extend(group_rows(group, name_width, width, marks));
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
        // Three names in words don't fit one 80-column row, so the line wraps (#663).
        let wrapped = rows[i + 1..].iter().take_while(|r| r.left[0].1 == Tone::Text).count();
        assert!(wrapped >= 1 && rows[i + 1..=i + wrapped].iter().all(|r| r.plain().len() <= 80));
        let hint = &rows[i + 1 + wrapped];
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
            if let [(_, _), (_, _), (full, _), (empty, _), (_, _), (_, _)] = row.left.as_slice() {
                assert_eq!(full.chars().count() + empty.chars().count(), BAR_CELLS, "{row:?}");
            }
        }
    }
}
