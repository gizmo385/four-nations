//! The Traders screen's view (#438, docs/design/pacing.md section 2, "Screen"): the caravan
//! camped now, what it offers and how far each taken deal has got, or when the next is due. And
//! the two actions behind it, `Command::TakeDeal` and `Command::CancelDeal`, with their outcome
//! worded for the player.
//!
//! Every frontend goes through here: the TUI's Traders screen (`M`; `Enter` takes, `X`
//! cancels), and through those same keys the agent surface. Which deals a visit offers, what the
//! camp still needs and whether a slot may be taken again are the sim's (`World::caravan`,
//! `World::trade_needs`); this only lays them out.

use fn_sim::Defs;
use fn_sim::jobs::ItemId;
use fn_sim::trade::{Goods, Reward};
use fn_sim::world::{Command, CommandError, OfferKind, TICKS_PER_SECOND, TradeError, World};

use crate::session::Session;
use crate::time::left_text;

/// The screen's contents.
#[derive(Clone, Debug, PartialEq)]
pub enum TradersView {
    /// A caravan is camped.
    Camp(CampView),
    /// None is: when the next is due ("Earth Kingdom traders are due in 6:10"), or that the road
    /// isn't open yet.
    Away(String),
}

/// The caravan camped now.
#[derive(Clone, Debug, PartialEq)]
pub struct CampView {
    /// "Fire Nation traders".
    pub name: String,
    /// "6:10 left".
    pub left: String,
    /// Its deals, then its request: one row per slot, in slot order.
    pub offers: Vec<OfferRow>,
}

/// One slot of the visit.
#[derive(Clone, Debug, PartialEq)]
pub struct OfferRow {
    /// The visit's slot number, what `Command::TakeDeal` takes.
    pub slot: u8,
    /// Whether it is the visit's request rather than a deal.
    pub request: bool,
    /// The request's own name; empty for a deal.
    pub name: String,
    /// What the settlement hands over, once: "30 food", "20 wood and 10 food".
    pub give: String,
    /// What it gets back, once: "20 stone", "40 knowledge", "half a newcomer's draw".
    pub get: String,
    /// "1 of 2 taken"; a request reads "taken" or "not taken".
    pub taken: String,
    /// What the stores hold of what it asks for: "204 food in stores".
    pub stores: String,
    /// While taken and not done: "12 of 30 food delivered". `None` otherwise.
    pub under_way: Option<String>,
    /// Whether `Enter` would take it once more.
    pub can_take: bool,
    /// Whether `X` would cancel it (something is still owed on it).
    pub can_cancel: bool,
}

/// The screen's contents right now.
pub fn view(session: &Session) -> TradersView {
    let (world, defs) = (&session.world, &session.defs);
    let (Some(visit), Some(caravan)) = (world.caravan(), world.caravan_def(defs)) else {
        return TradersView::Away(away_text(world, defs));
    };
    let secs = visit.leaves.saturating_sub(world.tick) as f32 / TICKS_PER_SECOND as f32;
    let offers = visit
        .offers
        .iter()
        .enumerate()
        .map(|(slot, offer)| {
            let slot = slot as u8;
            let (request, name, give, get, times) = match offer.kind {
                OfferKind::Deal(d) => {
                    let deal = &caravan.deals[usize::from(d)];
                    let mut get: Vec<String> =
                        deal.get.iter().map(|&(i, n)| count_of(defs, i, n)).collect();
                    if deal.knowledge > 0 {
                        get.push(format!("{} knowledge", deal.knowledge));
                    }
                    (false, String::new(), &deal.give, and_list(&get), deal.times)
                }
                OfferKind::Request(r) => {
                    let req = &caravan.requests[usize::from(r)];
                    (true, req.name.clone(), &req.give, reward_text(req.reward), 1)
                }
            };
            let owed = offer.taken - offer.done;
            let needs = world.trade_needs(slot, defs).unwrap_or_default();
            let under_way = (owed > 0).then(|| {
                let parts: Vec<String> = needs
                    .iter()
                    .map(|&(g, left)| {
                        let need = g.count * owed;
                        format!("{} of {need} {}", need - left.min(need), goods_name(defs, g))
                    })
                    .collect();
                format!("{} delivered", and_list(&parts))
            });
            let taken = if request {
                if offer.taken > 0 { "taken" } else { "not taken" }.to_owned()
            } else {
                format!("{} of {times} taken", offer.taken)
            };
            let stores: Vec<String> = give
                .iter()
                .map(|g| format!("{} {}", in_stores(world, g), goods_name(defs, g)))
                .collect();
            OfferRow {
                slot,
                request,
                name,
                give: and_list(&give.iter().map(|g| goods_text(defs, g)).collect::<Vec<_>>()),
                get,
                taken,
                stores: format!("{} in stores", and_list(&stores)),
                under_way,
                can_take: offer.taken < times,
                can_cancel: owed > 0,
            }
        })
        .collect();
    TradersView::Camp(CampView { name: caravan.name.clone(), left: left_text(secs), offers })
}

/// What the screen says with no caravan camped.
fn away_text(world: &World, defs: &Defs) -> String {
    match world.next_caravan(defs) {
        Some((at, nation)) => {
            let name = defs
                .trade
                .caravans
                .iter()
                .find(|c| c.nation == nation)
                .map_or("Traders", |c| c.name.as_str());
            let secs = at.saturating_sub(world.tick) as f32 / TICKS_PER_SECOND as f32;
            let secs = secs.max(0.0).round() as u64;
            format!("{name} are due in {}:{:02}.", secs / 60, secs % 60)
        }
        None if defs.trade.schedule.is_none() => "No traders come to this land.".to_owned(),
        None => "The road isn't open yet.".to_owned(),
    }
}

/// `Enter`: takes slot `slot` once more. The outcome as a status line either way.
pub fn take(session: &mut Session, slot: u8) -> Result<String, String> {
    let label = offer_label(session, slot);
    match session.apply(Command::TakeDeal { slot }) {
        Ok(()) => Ok(format!("Taken: {label}")),
        Err(e) => Err(refusal("Can't take that deal", e)),
    }
}

/// `X`: drops what is still owed on slot `slot`.
pub fn cancel(session: &mut Session, slot: u8) -> Result<String, String> {
    let label = offer_label(session, slot);
    match session.apply(Command::CancelDeal { slot }) {
        Ok(()) => Ok(format!("Cancelled: {label}")),
        Err(e) => Err(refusal("Nothing to cancel", e)),
    }
}

/// "30 food for 20 stone", or a request's own name.
fn offer_label(session: &Session, slot: u8) -> String {
    match view(session) {
        TradersView::Camp(c) => c.offers.get(usize::from(slot)).map_or_else(String::new, |o| {
            if o.request { o.name.clone() } else { format!("{} for {}", o.give, o.get) }
        }),
        TradersView::Away(_) => String::new(),
    }
}

fn refusal(head: &str, e: CommandError) -> String {
    match e {
        CommandError::Trade(TradeError::NotTaken) => format!("{head}: nothing is owed on it"),
        CommandError::Trade(t) => format!("{head}: {t}"),
        other => format!("{head}: {other}"),
    }
}

/// How many of what `goods` asks for the stores hold: every food item for "food".
fn in_stores(world: &World, goods: &Goods) -> u32 {
    goods.items.iter().map(|&i| world.stored_total(i)).sum()
}

/// "food", or the one item's name in lower case.
fn goods_name(defs: &Defs, goods: &Goods) -> String {
    if goods.any_food {
        "food".to_owned()
    } else {
        goods.items.first().map_or_else(String::new, |&i| defs.item(i).name.to_lowercase())
    }
}

/// "30 food".
fn goods_text(defs: &Defs, goods: &Goods) -> String {
    format!("{} {}", goods.count, goods_name(defs, goods))
}

/// "20 stone".
fn count_of(defs: &Defs, item: ItemId, n: u32) -> String {
    format!("{n} {}", defs.item(item).name.to_lowercase())
}

/// What a request grants, in words.
fn reward_text(reward: Reward) -> String {
    match reward {
        Reward::Knowledge(n) => format!("{n} knowledge"),
        Reward::Draw(milli) if milli == fn_sim::world::DRAW_ONE => "a newcomer's draw".to_owned(),
        Reward::Draw(milli) if milli * 2 == fn_sim::world::DRAW_ONE => {
            "half a newcomer's draw".to_owned()
        }
        Reward::Draw(milli) => {
            format!("{:.1} of a newcomer's draw", milli as f32 / fn_sim::world::DRAW_ONE as f32)
        }
    }
}

/// "a", "a and b", "a, b and c".
fn and_list(parts: &[String]) -> String {
    match parts {
        [] => String::new(),
        [one] => one.clone(),
        [rest @ .., last] => format!("{} and {last}", rest.join(", ")),
    }
}

#[cfg(test)]
mod tests;
