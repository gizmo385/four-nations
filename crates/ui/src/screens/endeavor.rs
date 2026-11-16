//! The Endeavor screen (`G`, docs/design/endgame.md "Showing progress"): the same text as the
//! side panel's own section builds on, laid out as a full page -- the endeavor's name and how
//! many signs are met, its description, the stage tracker (what the gate's site is building, or
//! that the endeavor hasn't begun) with what can be done with the gate now (#643: `Enter`
//! chooses a site once Ready, `X` withdraws one no work has started on), then every sign grouped under its heading, each with a mark
//! once it is met, a hint while it isn't, its count and a 10-cell bar, like the selection bar's
//! meters. What it shows is `crate::endeavor::screen_view`; this module only lays it out as rows
//! that scroll (`screens::draw_screen`).

use crate::Session;
use crate::endeavor::{GroupView, LedgerRow, SignRow, UnitedView, goods_ledger, screen_view};
use crate::state::{CityTab, UiState};

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
    if u.standing.iter().any(|&n| n > 0) && !u.goods.is_empty() {
        rows.extend(standing_rows(u));
    }
    rows
}

/// The STANDING and GOODS sections (#1026, #1028): residents at each tier, Notables first, then
/// one row per good residents want with what the city makes and uses of it a minute.
fn standing_rows(u: &UnitedView) -> Vec<Row> {
    let [settlers, citizens, notables] = u.standing;
    let mut rows = Vec::new();
    // The spirits (#1030): the balance, its trend, and what moved it most.
    if let Some(b) = &u.balance {
        rows.push(Row::default());
        rows.push(Row::text("SPIRITS", Tone::Title));
        let tone = if b.value < 40 { Tone::Warn } else { Tone::Text };
        rows.push(Row::spans(vec![
            ("  Balance   ".to_owned(), Tone::Text),
            (format!("{:>3}", b.value), tone),
            (format!("   {:+.1} a minute", b.trend), Tone::Dim),
        ]));
        if !b.terms.is_empty() {
            rows.push(Row::text(format!("  {}", b.terms.join(", ")), Tone::Dim));
        }
        if b.portal_open {
            rows.push(Row::text("  The Spirit Portal is open.", Tone::Text));
        }
    }
    rows.extend([Row::default(), Row::text("STANDING", Tone::Title)]);
    for (name, count) in [("Notables", notables), ("Citizens", citizens), ("Settlers", settlers)] {
        rows.push(Row::spans(vec![
            (format!("  {name:<10}"), Tone::Text),
            (format!("{count:>3}"), Tone::Dim),
        ]));
    }
    rows.push(Row::default());
    rows.push(Row::pair(
        "GOODS".to_owned(),
        Tone::Title,
        format!("a minute, last {} minutes", fn_sim::world::FLOW_MINUTES),
        Tone::Dim,
    ));
    let name_width = u.goods.iter().map(|g| g.name.chars().count()).max().unwrap_or(0).max(4);
    rows.push(Row::text(
        format!("  {:<name_width$}   MADE   USED  STORES  WANTED BY", "GOOD"),
        Tone::Dim,
    ));
    for g in &u.goods {
        let mut spans = vec![
            (format!("  {:<name_width$} ", g.name), Tone::Text),
            (format!("{:>6.1} {:>6.1} {:>7}  ", g.made, g.used, g.stores), Tone::Dim),
            (g.wanted_by.clone(), Tone::Text),
        ];
        if g.short {
            spans.push(("  Short".to_owned(), Tone::Warn));
        }
        rows.push(Row::spans(spans));
    }
    rows
}

/// The tab strip under the United City's name (#1028): both tabs, the open one bracketed.
pub fn tab_strip(tab: CityTab) -> Row {
    let (city, goods) = match tab {
        CityTab::City => ("[City]", " Goods "),
        CityTab::Goods => (" City ", "[Goods]"),
    };
    Row::spans(vec![
        (city.to_owned(), if tab == CityTab::City { Tone::Accent } else { Tone::Dim }),
        (" ".to_owned(), Tone::Dim),
        (goods.to_owned(), if tab == CityTab::Goods { Tone::Accent } else { Tone::Dim }),
        ("   Tab switches".to_owned(), Tone::Dim),
    ])
}

/// The Goods tab's rows (#1028, docs/design/factory-endgame.md "The Goods screen"): a heading
/// with the window the rates cover, the column heads, then one row per good with what makes
/// it here and who wants it, and a closing note on the short ones.
///
/// `cursor` puts a pointer on that row, the good `Enter` opens in the planner.
pub fn goods_lines(
    rows: &[LedgerRow],
    width: usize,
    marks: &Marks,
    cursor: Option<usize>,
) -> Vec<Row> {
    let mut out = vec![Row::pair(
        "GOODS".to_owned(),
        Tone::Title,
        format!("a minute, last {} minutes", fn_sim::world::FLOW_MINUTES),
        Tone::Dim,
    )];
    if rows.is_empty() {
        out.push(Row::text("  No resident wants anything yet", Tone::Dim));
        return out;
    }
    let name_width = rows.iter().map(|g| g.name.chars().count()).max().unwrap_or(0).max(4);
    let at_width = rows.iter().map(|g| g.made_at.chars().count()).max().unwrap_or(0).max(7);
    out.push(Row::text(
        format!(
            "  {:<name_width$}   MADE   USED    NET  STORES  {:<at_width$}  WANTED BY",
            "GOOD", "MADE AT"
        ),
        Tone::Dim,
    ));
    for (i, g) in rows.iter().enumerate() {
        let net = g.made - g.used;
        let here = cursor == Some(i);
        let net_tone = if net < -0.05 { Tone::Warn } else { Tone::Dim };
        let at_tone = if g.made_at == "nothing" { Tone::Warn } else { Tone::Text };
        let mut spans = vec![
            (if here { "> " } else { "  " }.to_owned(), Tone::Accent),
            (format!("{:<name_width$} ", g.name), if here { Tone::Strong } else { Tone::Text }),
            (format!("{:>6.1} {:>6.1} ", g.made, g.used), Tone::Dim),
            (format!("{net:>+6.1} "), net_tone),
            (format!("{:>7}  ", g.stores), Tone::Dim),
            (format!("{:<at_width$}  ", g.made_at), at_tone),
            (g.wanted_by.clone(), Tone::Text),
        ];
        if g.short {
            spans.push(("  Short".to_owned(), Tone::Warn));
        }
        out.push(Row::spans(spans));
    }
    let short: Vec<&str> = rows.iter().filter(|g| g.short).map(|g| g.name.as_str()).collect();
    out.push(Row::default());
    out.push(Row::text(
        "Enter on a good plans the machines that make it. O shows every way to get more.",
        Tone::Dim,
    ));
    if !short.is_empty() {
        out.push(Row::default());
        let note = format!(
            "Short of {}. Set a building to make it (R on a selected building), or buy it from \
             a caravan.",
            short.join(", ")
        );
        for line in wrap(&note, width, usize::MAX, marks.ellipsis) {
            out.push(Row::text(line, Tone::Warn));
        }
    }
    out
}

/// The screen's rows for the tab `ui` has open: [`lines`] on the City tab (and always before
/// the gate), the name, the tab strip and [`goods_lines`] on the Goods tab.
pub fn tab_lines(session: &Session, ui: &UiState, width: usize, marks: &Marks) -> Vec<Row> {
    if !session.world.united() || ui.city_tab == CityTab::City {
        let mut rows = lines(session, width, marks);
        if session.world.united() {
            rows.insert(1, tab_strip(CityTab::City));
        }
        return rows;
    }
    let v = screen_view(session);
    let mut rows = vec![
        Row::pair(v.name.clone(), Tone::Title, v.count.clone(), Tone::Accent),
        tab_strip(CityTab::Goods),
        Row::default(),
    ];
    let ledger = goods_ledger(&session.world, &session.defs);
    let cursor = ui.goods_cursor.min(ledger.len().saturating_sub(1));
    rows.extend(goods_lines(&ledger, width, marks, Some(cursor)));
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

    /// Tab opens the Goods tab after the gate (#1028): each wanted good, then the made goods
    /// on the way to them (fittings for furniture, steel for fittings), and a note naming the
    /// short ones. Before the gate Tab does nothing and there is no strip.
    #[test]
    fn tab_switches_the_united_city_to_its_goods() {
        let mut s = new_session(7, Nation::Fire);
        let marks = Marks::new(false);
        let mut ui = UiState::new(&s, (100, 30));
        ui.switch_city_tab(&s);
        assert_eq!(ui.city_tab, CityTab::City, "no Goods tab before the gate");
        let before: Vec<String> = tab_lines(&s, &ui, 100, &marks).iter().map(Row::plain).collect();
        assert!(!before.iter().any(|t| t.contains("Tab switches")), "{before:?}");

        let defs = s.defs.clone();
        s.world.start_republic(&defs);
        let city: Vec<String> = tab_lines(&s, &ui, 100, &marks).iter().map(Row::plain).collect();
        assert!(city[1].starts_with("[City]"), "{city:?}");
        ui.switch_city_tab(&s);
        assert_eq!(ui.city_tab, CityTab::Goods);
        let goods: Vec<String> = tab_lines(&s, &ui, 100, &marks).iter().map(Row::plain).collect();
        assert!(goods[1].contains("[Goods]"), "{goods:?}");
        let row = |name: &str| {
            goods.iter().position(|t| t.trim_start_matches("> ").trim_start().starts_with(name))
        };
        assert!(goods.iter().any(|t| t.starts_with("> Lantern")), "the pointer starts on the top");
        let (lantern, fittings, steel) = (row("Lantern "), row("Fittings "), row("Steel "));
        assert!(lantern.is_some() && fittings.is_some() && steel.is_some(), "{goods:?}");
        assert!(lantern < fittings, "wanted goods come first: {goods:?}");
        assert!(goods[lantern.unwrap()].contains("nothing"), "no Forge makes lanterns yet");
        assert!(
            !goods.iter().any(|t| t.trim_start().starts_with("Wood ")),
            "raw goods are left out"
        );
    }

    /// A Republic start (#1025) shows its residents by standing and the goods they want (#1028),
    /// with the stocked lanterns in stores and nothing made yet.
    #[test]
    fn the_united_city_shows_standing_and_the_wanted_goods() {
        let mut s = new_session(7, Nation::Fire);
        let defs = s.defs.clone();
        s.world.start_republic(&defs);
        let text: Vec<String> = lines(&s, 80, &Marks::new(false)).iter().map(Row::plain).collect();
        assert!(text.iter().any(|t| t == "STANDING"), "{text:?}");
        assert!(text.iter().any(|t| t.starts_with("GOODS")), "{text:?}");
        let lantern =
            text.iter().find(|t| t.trim_start().starts_with("Lantern")).expect("lanterns");
        assert!(lantern.contains("0.0    0.0      10"), "{lantern}");
        assert!(!lantern.contains("Short"), "{lantern}");
        // Nobody here is Water, and the stocked smoked fish is listed anyway, since it's in stores.
        assert!(text.iter().any(|t| t.trim_start().starts_with("Smoked Fish")), "{text:?}");
        assert!(!text.iter().any(|t| t.trim_start().starts_with("Parka")), "{text:?}");
    }

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
