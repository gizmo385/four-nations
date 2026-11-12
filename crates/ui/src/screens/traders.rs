//! The Traders screen (`M`, #438, docs/design/pacing.md section 2, "Screen"): the caravan camped
//! now, on a full screen in the shared frame (`crate::page`, #652). Its name and how long it
//! stays head the page; DEALS lists the deals it offers, each with how many times it has been
//! taken and what the stores hold of what it asks; REQUEST is its one gift; UNDER WAY is every
//! taken deal still being hauled, with what has been delivered. With no caravan camped the page
//! says when the next is due, or that the road isn't open yet.
//!
//! A pointer (`▸` and the quiet background, `page::highlight`) moves over the deals and the
//! request. `Enter` (`App::take_deal`) takes the one under it once more, `X` cancels what is
//! still owed on it, and `Esc` or `M` closes. The view and both actions are `crate::trade`'s.

use crate::rows::{Marks, Row, Tone, fit};
use crate::trade::{CampView, OfferRow, TradersView};

/// The pointer after a step of `dy` rows (its sign only), stopping at the offers' edges.
pub fn step(view: &TradersView, at: usize, dy: i32) -> usize {
    let n = match view {
        TradersView::Camp(c) => c.offers.len(),
        TradersView::Away(_) => 0,
    };
    if n == 0 {
        return 0;
    }
    (at as i64 + i64::from(dy.signum())).clamp(0, n as i64 - 1) as usize
}

/// The offer under the pointer, if a caravan is camped.
pub fn offer_at(view: &TradersView, at: usize) -> Option<&OfferRow> {
    match view {
        TradersView::Camp(c) => c.offers.get(at.min(c.offers.len().saturating_sub(1))),
        TradersView::Away(_) => None,
    }
}

/// The screen's rows at `width`, and which row the pointer's offer is on.
pub fn rows(
    view: &TradersView,
    at: usize,
    width: usize,
    marks: &Marks,
) -> (Vec<Row>, Option<usize>) {
    let camp = match view {
        TradersView::Camp(c) => c,
        TradersView::Away(text) => return (vec![Row::text(text.clone(), Tone::Dim)], None),
    };
    let at = at.min(camp.offers.len().saturating_sub(1));
    let mut out = vec![Row::pair(camp.name.clone(), Tone::Title, camp.left.clone(), Tone::Dim)];
    let mut chosen = None;
    let mut offer = |out: &mut Vec<Row>, o: &OfferRow, text: String| {
        let pointer = if usize::from(o.slot) == at { marks.pointer } else { " " };
        if usize::from(o.slot) == at {
            chosen = Some(out.len());
        }
        let tone = if o.can_take || o.can_cancel { Tone::Text } else { Tone::Dim };
        let room = width.saturating_sub(o.taken.chars().count() + 2);
        let left = fit(&format!("{pointer} {text}"), room, marks.ellipsis);
        out.push(Row::pair(left, tone, o.taken.clone(), Tone::Dim));
        out.push(Row::text(fit(&format!("    {}", o.stores), width, marks.ellipsis), Tone::Dim));
    };

    out.push(Row::text("", Tone::Text));
    out.push(Row::text("DEALS", Tone::Accent));
    for o in camp.offers.iter().filter(|o| !o.request) {
        offer(&mut out, o, format!("{} for {}", o.give, o.get));
    }
    if let Some(tea) = &camp.tea {
        out.push(Row::text(fit(&format!("    {tea}"), width, marks.ellipsis), Tone::Dim));
    }
    if let Some(r) = camp.offers.iter().find(|o| o.request) {
        out.push(Row::text("", Tone::Text));
        out.push(Row::text("REQUEST", Tone::Accent));
        offer(&mut out, r, r.name.clone());
        let gives = format!("    {} for {}", r.give, r.get);
        out.push(Row::text(fit(&gives, width, marks.ellipsis), Tone::Dim));
    }
    let under_way = under_way(camp);
    if !under_way.is_empty() {
        out.push(Row::text("", Tone::Text));
        out.push(Row::text("UNDER WAY", Tone::Accent));
        for line in under_way {
            out.push(Row::text(fit(&format!("  {line}"), width, marks.ellipsis), Tone::Text));
        }
    }
    (out, chosen)
}

/// "30 food for 2 metal: 12 of 30 food delivered", for every offer still being hauled.
pub fn under_way(camp: &CampView) -> Vec<String> {
    camp.offers
        .iter()
        .filter_map(|o| {
            let what = if o.request { o.name.clone() } else { format!("{} for {}", o.give, o.get) };
            o.under_way.as_ref().map(|u| format!("{what}: {u}"))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn offer(slot: u8, request: bool) -> OfferRow {
        OfferRow {
            slot,
            request,
            name: if request { "Food for the families".to_owned() } else { String::new() },
            give: "30 food".to_owned(),
            get: "2 metal".to_owned(),
            taken: "0 of 2 taken".to_owned(),
            stores: "212 food in stores".to_owned(),
            under_way: None,
            can_take: true,
            can_cancel: false,
        }
    }

    /// The Tea House's line sits under the deals and above the request (#712).
    #[test]
    fn the_tea_line_closes_the_deals() {
        let camp = CampView {
            name: "Fire Nation traders".to_owned(),
            left: "11:00 left".to_owned(),
            offers: vec![offer(0, false), offer(1, false), offer(2, true)],
            tea: Some("Tea House: 1 more deal".to_owned()),
        };
        let marks = Marks::new(false);
        let (shown, _) = rows(&TradersView::Camp(camp.clone()), 0, 60, &marks);
        let text: Vec<String> = shown.iter().map(Row::plain).collect();
        let tea = text.iter().position(|l| l.trim() == "Tea House: 1 more deal");
        let request = text.iter().position(|l| l == "REQUEST");
        assert!(tea.is_some() && tea < request, "{text:?}");
        let none = CampView { tea: None, ..camp };
        let (shown, _) = rows(&TradersView::Camp(none), 0, 60, &marks);
        assert!(shown.iter().all(|r| !r.plain().contains("Tea House")));
    }
}
