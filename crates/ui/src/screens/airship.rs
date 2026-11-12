//! The Airship screen's rows (docs/design/horizons-phase-6.md, "Mockups", #763), in the shared
//! page frame (`crate::page`, #652): the dock and what it is doing on the top row, SAIL TO and
//! DEALS side by side with the times each deal is taken and what the stores hold, then the HOLD:
//! what goes out, how full it is and what comes back. The SEND THE AIRSHIP box is its own rows
//! (`confirm_body`), drawn over the page.
//!
//! The pointer (`▸` and the quiet background, `page::highlight`) sits in one column at a time:
//! Left and Right move between SAIL TO and DEALS, Up and Down within. The view and every action
//! are `crate::airship`'s.

use crate::airship::{self, AirshipView, Column, Picks};
use crate::rows::{Marks, Row, Tone, fit, wrap};
use crate::session::Session;

/// Text cells between one column and the next.
const GAP: usize = 2;

/// `text` cut to `width` cells and padded with spaces to it.
fn cell(text: &str, width: usize, marks: &Marks) -> String {
    let mut out = fit(text, width, marks.ellipsis);
    let used = out.chars().count();
    out.extend(std::iter::repeat_n(' ', width.saturating_sub(used)));
    out
}

/// The screen's rows at `width`, and which row the pointer's own item is on.
pub fn rows(view: &AirshipView, width: usize, marks: &Marks) -> (Vec<Row>, Option<usize>) {
    let mut out = Vec::new();
    let right = [view.status.as_str(), view.fuel.as_str(), view.crew.as_str()]
        .into_iter()
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join("   ");
    let room = width.saturating_sub(right.chars().count() + GAP);
    out.push(Row::pair(fit(&view.head, room, marks.ellipsis), Tone::Title, right, Tone::Dim));
    out.push(Row::text("", Tone::Text));

    // The columns: the nations, the deals, the times and what the stores hold.
    let nations = view.nations.iter().map(|n| n.chars().count() + 2).max().unwrap_or(0);
    let c1 = nations.max("SAIL TO".len()) + GAP;
    let c3 =
        view.deals.iter().map(|d| d.times.chars().count()).max().unwrap_or(0).max("Times".len());
    let c4 = view
        .deals
        .iter()
        .map(|d| d.stores.chars().count())
        .max()
        .unwrap_or(0)
        .max("In stores".len());
    let c2 = width.saturating_sub(c1 + c3 + c4 + 2 * GAP).max(8);
    let focus =
        |col: Column| if view.column == col && view.editable { Tone::Accent } else { Tone::Dim };
    out.push(Row::spans(vec![
        (cell("SAIL TO", c1, marks), focus(Column::Nation)),
        (cell("DEALS", c2, marks), focus(Column::Deals)),
        (" ".repeat(GAP), Tone::Dim),
        (cell("Times", c3, marks), Tone::Dim),
        (" ".repeat(GAP), Tone::Dim),
        ("In stores".to_owned(), Tone::Dim),
    ]));
    let first = out.len();
    let lines = view.nations.len().max(view.deals.len());
    let mut chosen = None;
    for i in 0..lines {
        let mut spans = Vec::new();
        match view.nations.get(i) {
            Some(name) => {
                let on = i == view.nation_at;
                let pointer = if on { marks.pointer } else { " " };
                let tone = match (on, view.column == Column::Nation && view.editable) {
                    (true, true) => Tone::Title,
                    (true, false) => Tone::Text,
                    (false, _) => Tone::Dim,
                };
                spans.push((cell(&format!("{pointer} {name}"), c1, marks), tone));
            }
            None => spans.push((" ".repeat(c1), Tone::Text)),
        }
        if let Some(deal) = view.deals.get(i) {
            let on = i == view.deal_at;
            let pointer = if on && view.editable { marks.pointer } else { " " };
            let tone = match (deal.picked, on && view.column == Column::Deals) {
                (_, true) => Tone::Title,
                (true, false) => Tone::Text,
                (false, false) => Tone::Dim,
            };
            spans.push((cell(&format!("{pointer} {}", deal.text), c2, marks), tone));
            spans.push((" ".repeat(GAP), Tone::Dim));
            spans.push((
                cell(&deal.times, c3, marks),
                if deal.picked { Tone::Text } else { Tone::Dim },
            ));
            spans.push((" ".repeat(GAP), Tone::Dim));
            spans.push((deal.stores.clone(), Tone::Dim));
        }
        let at_pointer = match view.column {
            Column::Nation => i == view.nation_at,
            Column::Deals => view.editable && i == view.deal_at && i < view.deals.len(),
        };
        if at_pointer {
            chosen = Some(first + i);
        }
        out.push(Row::spans(spans));
    }
    out.push(Row::text("", Tone::Text));
    out.push(Row::text("HOLD", Tone::Accent));
    let load_room = width.saturating_sub(view.load.chars().count() + GAP);
    out.push(Row::pair(
        fit(&format!("  {}", view.out), load_room, marks.ellipsis),
        Tone::Text,
        view.load.clone(),
        Tone::Dim,
    ));
    out.push(Row::text(fit(&format!("  {}", view.back), width, marks.ellipsis), Tone::Dim));
    (out, chosen)
}

/// The box's body as rows wrapped to `text` cells: the question in the title's tone, the rest as
/// text, a blank row between them.
pub fn confirm_body(session: &Session, picks: &Picks, marks: &Marks, text: usize) -> Vec<Row> {
    let mut rows = Vec::new();
    for (i, line) in airship::confirm_lines(session, picks).into_iter().enumerate() {
        if line.is_empty() {
            rows.push(Row::text(String::new(), Tone::Text));
            continue;
        }
        let tone = if i == 0 { Tone::Title } else { Tone::Text };
        for piece in wrap(&line, text.max(1), usize::MAX, marks.ellipsis) {
            rows.push(Row::text(piece, tone));
        }
    }
    rows
}
