//! Caravans (#437, docs/design/pacing.md section 2): the live side. The data is
//! `crate::trade` (`data/trade.toml`).
//!
//! - **Arrival.** From the moment `[caravans].opens` is met (`World::chapter_met`; no chapter
//!   means minute 0), a caravan arrives on every tick `k x interval + offset` and stays `stay`.
//!   The other nations take turns in `order`: each slot is the next one's turn, so there is no
//!   RNG in who comes. It camps on the first free 2x2 spot whose anchor is `camp_distance` path
//!   tiles from the settlement anchor, searched in row-major order. With none, the visit is
//!   skipped (`CaravanEventKind::Skipped`) and the turn still passes.
//! - **Offers.** A visit offers `deals_per_visit` of the caravan's deals, drawn from
//!   `rng::stream(seed, "trade.deals")`, then one of its requests. They are the visit's slots,
//!   deals first.
//! - **Indirect control.** `Command::TakeDeal` takes a slot once more (a deal up to its `times`,
//!   a request once); `tick_trade` then keeps one `JobKind::Trade` open per slot that still
//!   needs goods, and a settler fetches up to a load from stores and tips it into the camp,
//!   which takes only what open slots still need (`run_trade`, the Refuel haul's pattern). Each
//!   time a deal's goods are all in, its `get` appears as stacks at the camp (`place_ground`, so
//!   ordinary hauling brings them home) and its knowledge is added; a request applies its
//!   reward. `Command::CancelDeal` drops what is still owed, and the goods already handed over
//!   for an unfinished deal go back on the ground at the camp. So does everything left over
//!   when the caravan leaves: nothing is lost.
//! - **No map footprint.** The camp is a reserved rectangle the world remembers, not a building:
//!   nothing about it needs pathing, and its glyphs and screen are a follow-up.

use serde::{Deserialize, Serialize};

use crate::flood::Flood;
use crate::jobs::{ItemId, JobKind, Stack};
use crate::nation::Nation;
use crate::path::STRAIGHT;
use crate::trade::{CaravanDef, Goods, Reward};

use super::work::route_from;
use super::*;

pub(super) fn default_rng() -> SimRng {
    rng::stream(0, "trade.deals")
}

pub(super) fn new_rng(seed: u64) -> SimRng {
    rng::stream(seed, "trade.deals")
}

/// Where the caravans stand: whose turn is next and the visit in progress.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TradeState {
    /// How many arrival slots have passed since the road opened: whose turn comes next.
    #[serde(default)]
    turns: u32,
    #[serde(default)]
    visit: Option<Visit>,
    /// Draws each visit's deals (`trade.deals`). A save from before caravans loads this seeded
    /// by 0, as `orchard_rng` does for #142; still deterministic.
    #[serde(default = "default_rng")]
    rng: SimRng,
}

impl TradeState {
    pub(super) fn new(seed: u64) -> Self {
        Self { turns: 0, visit: None, rng: new_rng(seed) }
    }
}

impl Default for TradeState {
    fn default() -> Self {
        Self { turns: 0, visit: None, rng: default_rng() }
    }
}

/// A caravan camped by the settlement.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Visit {
    pub nation: Nation,
    pub arrived: u64,
    /// The tick it packs up.
    pub leaves: u64,
    /// The camp's top-left tile; it covers 2x2 from there.
    pub camp: Pos,
    /// What it offers, deals first, then the request.
    pub offers: Vec<Offer>,
}

/// What one slot of a visit is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum OfferKind {
    /// The caravan's deal at this index (`CaravanDef::deals`).
    Deal(u8),
    /// The caravan's request at this index (`CaravanDef::requests`).
    Request(u8),
}

/// One slot of a visit and how far it has got.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Offer {
    pub kind: OfferKind,
    /// How many times the player has taken it, those done included.
    pub taken: u32,
    /// How many times it has completed.
    pub done: u32,
    /// Goods handed over toward the times not done yet.
    pub held: Vec<Stack>,
}

impl Offer {
    fn held_of(&self, goods: &Goods) -> u32 {
        self.held.iter().filter(|s| goods.accepts(s.item)).map(|s| s.count).sum()
    }
}

/// Something a caravan did, for the log and (later) alerts.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CaravanEvent {
    pub tick: u64,
    pub kind: CaravanEventKind,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum CaravanEventKind {
    /// Made camp at `camp`.
    Arrived { nation: Nation, camp: Pos },
    /// Its turn came, but there was no free spot to camp.
    Skipped { nation: Nation },
    /// One of its deals completed once (`deal` indexes `CaravanDef::deals`).
    DealDone { nation: Nation, deal: u8 },
    /// Its request was met (`request` indexes `CaravanDef::requests`).
    RequestDone { nation: Nation, request: u8 },
    /// Packed up and left.
    Left { nation: Nation },
}

/// Why a trade command was refused.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TradeError {
    /// No caravan is camped.
    NoCaravan,
    /// The visit has no slot with that number.
    NoSuchOffer,
    /// Already taken as many times as it allows.
    AllTaken,
    /// Nothing is owed on it to cancel.
    NotTaken,
}

impl std::fmt::Display for TradeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::NoCaravan => "no traders are camped",
            Self::NoSuchOffer => "no such deal",
            Self::AllTaken => "already taken in full",
            Self::NotTaken => "nothing taken to cancel",
        })
    }
}

impl World {
    /// The caravan camped right now, if any.
    pub fn caravan(&self) -> Option<&Visit> {
        self.trade.visit.as_ref()
    }

    /// The data of the caravan camped right now.
    pub fn caravan_def<'d>(&self, defs: &'d Defs) -> Option<&'d CaravanDef> {
        let v = self.trade.visit.as_ref()?;
        defs.trade.caravans.iter().find(|c| c.nation == v.nation)
    }

    /// The goods of slot `slot` and how many of each it still needs, for the visit camped now.
    pub fn trade_needs<'d>(&self, slot: u8, defs: &'d Defs) -> Option<Vec<(&'d Goods, u32)>> {
        let caravan = self.caravan_def(defs)?;
        let offer = self.trade.visit.as_ref()?.offers.get(usize::from(slot))?;
        let give = match offer.kind {
            OfferKind::Deal(d) => &caravan.deals.get(usize::from(d))?.give,
            OfferKind::Request(r) => &caravan.requests.get(usize::from(r))?.give,
        };
        let owed = offer.taken - offer.done;
        Some(give.iter().map(|g| (g, (g.count * owed).saturating_sub(offer.held_of(g)))).collect())
    }

    /// Whether a `Trade` job for `slot` at `camp` is still wanted: that visit is here and the
    /// slot still needs goods.
    pub(super) fn trade_job_wanted(&self, camp: Pos, slot: u8, defs: &Defs) -> bool {
        self.trade.visit.as_ref().is_some_and(|v| v.camp == camp)
            && self.trade_needs(slot, defs).is_some_and(|n| n.iter().any(|&(_, c)| c > 0))
    }

    /// The first item `slot` still needs that some store holds, with how many: what a `Trade`
    /// job fetches next. Goods in data order, then items in id order.
    pub(super) fn trade_source(&self, slot: u8, defs: &Defs) -> Option<(ItemId, u32)> {
        let needs = self.trade_needs(slot, defs)?;
        needs.into_iter().filter(|&(_, n)| n > 0).find_map(|(g, n)| {
            g.items.iter().copied().find(|&i| self.any_source_for_fuel(i)).map(|i| (i, n))
        })
    }

    /// `Command::TakeDeal`: takes slot `slot` once more.
    pub(super) fn take_deal(&mut self, slot: u8, defs: &Defs) -> Result<(), TradeError> {
        let caravan = self.caravan_def(defs).ok_or(TradeError::NoCaravan)?;
        let visit = self.trade.visit.as_mut().ok_or(TradeError::NoCaravan)?;
        let offer = visit.offers.get_mut(usize::from(slot)).ok_or(TradeError::NoSuchOffer)?;
        let most = match offer.kind {
            OfferKind::Deal(d) => caravan.deals[usize::from(d)].times,
            OfferKind::Request(_) => 1,
        };
        if offer.taken >= most {
            return Err(TradeError::AllTaken);
        }
        offer.taken += 1;
        self.open_trade_jobs(defs);
        Ok(())
    }

    /// `Command::CancelDeal`: drops what is still owed on slot `slot`; goods handed over for it
    /// go back on the ground at the camp, and its haul job goes.
    pub(super) fn cancel_deal(&mut self, slot: u8) -> Result<(), TradeError> {
        let visit = self.trade.visit.as_mut().ok_or(TradeError::NoCaravan)?;
        let camp = visit.camp;
        let offer = visit.offers.get_mut(usize::from(slot)).ok_or(TradeError::NoSuchOffer)?;
        if offer.taken == offer.done {
            return Err(TradeError::NotTaken);
        }
        offer.taken = offer.done;
        let held = std::mem::take(&mut offer.held);
        for stack in held {
            self.place_ground(camp, stack);
        }
        self.jobs.retain(|j| j.kind != JobKind::Trade { pos: camp, slot });
        Ok(())
    }

    /// Arrivals and departures on their ticks, and once a second a `Trade` job for every slot
    /// that still needs goods.
    pub(super) fn tick_trade(&mut self, defs: &Defs) {
        let Some(s) = defs.trade.schedule.as_ref() else { return };
        let minutes = |m: f32| (m * 60.0 * TICKS_PER_SECOND as f32).round() as u64;
        if self.trade.visit.as_ref().is_some_and(|v| self.tick >= v.leaves) {
            self.caravan_leaves();
        }
        let interval = minutes(s.interval_minutes).max(1);
        let opened = match &s.opens {
            None => true,
            Some(chapter) => self.chapter_met(chapter).is_some_and(|t| t <= self.tick),
        };
        if opened && self.tick % interval == minutes(s.offset_minutes) % interval {
            self.caravan_arrives(minutes(s.stay_minutes), defs);
        }
        if self.tick.is_multiple_of(u64::from(TICKS_PER_SECOND)) {
            self.open_trade_jobs(defs);
        }
    }

    /// One `Trade` job for every slot that still needs goods and has none.
    fn open_trade_jobs(&mut self, defs: &Defs) {
        let Some(v) = self.trade.visit.as_ref() else { return };
        let camp = v.camp;
        for slot in 0..v.offers.len() as u8 {
            let kind = JobKind::Trade { pos: camp, slot };
            if self.trade_job_wanted(camp, slot, defs) && !self.jobs.iter().any(|j| j.kind == kind)
            {
                self.add_job(kind);
            }
        }
    }

    /// The next nation's turn: it camps and offers its deals, or is skipped.
    fn caravan_arrives(&mut self, stay: u64, defs: &Defs) {
        let s = defs.trade.schedule.as_ref().expect("checked by tick_trade");
        let visitors: Vec<Nation> = s.visitors(self.nation).collect();
        if visitors.is_empty() || self.trade.visit.is_some() {
            return;
        }
        let nation = visitors[self.trade.turns as usize % visitors.len()];
        self.trade.turns += 1;
        let Some(camp) = self.find_camp(s.camp_distance) else {
            self.log_event(CaravanEvent {
                tick: self.tick,
                kind: CaravanEventKind::Skipped { nation },
            });
            return;
        };
        let caravan = defs.trade.caravans.iter().find(|c| c.nation == nation).expect("validated");
        // A partial shuffle: the first `deals_per_visit` of the deals, drawn without repeats,
        // then listed in data order.
        let mut deals: Vec<u8> = (0..caravan.deals.len() as u8).collect();
        let count = (s.deals_per_visit as usize).min(deals.len());
        for k in 0..count {
            let j = self.trade.rng.random_range(k..deals.len());
            deals.swap(k, j);
        }
        deals.truncate(count);
        deals.sort_unstable();
        let request = self.trade.rng.random_range(0..caravan.requests.len()) as u8;
        let offer = |kind| Offer { kind, taken: 0, done: 0, held: Vec::new() };
        let mut offers: Vec<Offer> = deals.into_iter().map(|d| offer(OfferKind::Deal(d))).collect();
        offers.push(offer(OfferKind::Request(request)));
        self.trade.visit =
            Some(Visit { nation, arrived: self.tick, leaves: self.tick + stay, camp, offers });
        self.log_event(CaravanEvent {
            tick: self.tick,
            kind: CaravanEventKind::Arrived { nation, camp },
        });
    }

    /// The caravan packs up: whatever was handed over for unfinished slots goes back on the
    /// ground at the camp, and its haul jobs go.
    fn caravan_leaves(&mut self) {
        let Some(v) = self.trade.visit.take() else { return };
        for stack in v.offers.into_iter().flat_map(|o| o.held) {
            self.place_ground(v.camp, stack);
        }
        self.jobs.retain(|j| !matches!(j.kind, JobKind::Trade { .. }));
        let nation = v.nation;
        self.log_event(CaravanEvent { tick: self.tick, kind: CaravanEventKind::Left { nation } });
    }

    /// The first free 2x2 spot, in row-major order, whose top-left tile is `near` to `far` path
    /// tiles from the settlement anchor: every tile walkable land with no building, site,
    /// resource, stockpile or ground stack.
    fn find_camp(&self, (near, far): (u32, u32)) -> Option<Pos> {
        let flood = Flood::new(&self.map, self.settlement_anchor(), &self.rules);
        let free = |p: Pos| {
            self.map.get(p).is_some_and(|t| {
                self.rules.passable(t) && t.building.is_none() && t.resource.is_none()
            }) && !self.zones.contains(&p)
                && !self.ground.contains_key(&p)
                && !self.sites.contains_key(&p)
        };
        self.map.positions().find(|&p| {
            flood.cost(p).is_some_and(|c| (near * STRAIGHT..=far * STRAIGHT).contains(&c))
                && (0..2).all(|dy| (0..2).all(|dx| free(Pos::new(p.x + dx, p.y + dy))))
        })
    }

    /// Every time all of slot `slot`'s goods are in and it is still owed, it completes once:
    /// a deal leaves its goods and knowledge, a request applies its reward.
    fn settle_offer(&mut self, slot: u8, defs: &Defs) {
        let Some(caravan) = self.caravan_def(defs) else { return };
        loop {
            let v = self.trade.visit.as_mut().expect("caravan_def found it");
            let (nation, camp) = (v.nation, v.camp);
            let offer = &mut v.offers[usize::from(slot)];
            let give = match offer.kind {
                OfferKind::Deal(d) => &caravan.deals[usize::from(d)].give,
                OfferKind::Request(r) => &caravan.requests[usize::from(r)].give,
            };
            if offer.done >= offer.taken || give.iter().any(|g| offer.held_of(g) < g.count) {
                return;
            }
            // The goods are spent: take each side's count out of what was handed over.
            for g in give {
                let mut left = g.count;
                for s in offer.held.iter_mut().filter(|s| g.accepts(s.item)) {
                    let take = left.min(s.count);
                    s.count -= take;
                    left -= take;
                }
            }
            offer.held.retain(|s| s.count > 0);
            offer.done += 1;
            let kind = offer.kind;
            let tick = self.tick;
            match kind {
                OfferKind::Deal(d) => {
                    let deal = &caravan.deals[usize::from(d)];
                    for &(item, count) in &deal.get {
                        self.place_ground(camp, Stack { item, count });
                    }
                    if deal.knowledge > 0 {
                        self.research.add_knowledge(u64::from(deal.knowledge) * 1000);
                    }
                    self.log_event(CaravanEvent {
                        tick,
                        kind: CaravanEventKind::DealDone { nation, deal: d },
                    });
                }
                OfferKind::Request(r) => {
                    match caravan.requests[usize::from(r)].reward {
                        Reward::Knowledge(k) => self.research.add_knowledge(u64::from(k) * 1000),
                        Reward::Draw(milli) => self.add_draw(milli, defs),
                    }
                    self.log_event(CaravanEvent {
                        tick,
                        kind: CaravanEventKind::RequestDone { nation, request: r },
                    });
                }
            }
        }
    }

    /// One tick of a unit carrying goods to the camp for slot `slot`: fetch up to a load of the
    /// first good still needed from the nearest store, carry it to the camp and hand it over.
    /// One trip finishes the job; `tick_trade` opens another while the slot still needs more.
    pub(super) fn run_trade(&mut self, i: usize, id: JobId, defs: &Defs) {
        let Some(idx) = self.job_index(id) else { return self.finish_job(i) };
        let JobKind::Trade { pos, slot } = self.jobs[idx].kind else { return self.finish_job(i) };
        if !self.trade_job_wanted(pos, slot, defs) {
            // Met some other way, cancelled, or the caravan left: anything in hand goes back to
            // stores (`run_idle`).
            self.jobs.remove(idx);
            return self.finish_job(i);
        }
        let needs = self.trade_needs(slot, defs).expect("wanted, so here");
        let carried = self.units[i].carrying;
        if let Some(stack) = carried
            && let Some(&(_, need)) = needs.iter().find(|(g, n)| *n > 0 && g.accepts(stack.item))
        {
            if !self.go_or_abandon(i, id, StandRule::Tile(pos), defs) {
                return;
            }
            let put = stack.count.min(need);
            let v = self.trade.visit.as_mut().expect("wanted, so here");
            let held = &mut v.offers[usize::from(slot)].held;
            match held.iter_mut().find(|s| s.item == stack.item) {
                Some(s) => s.count += put,
                None => held.push(Stack { item: stack.item, count: put }),
            }
            let left = stack.count - put;
            self.units[i].carrying = (left > 0).then_some(Stack { item: stack.item, count: left });
            self.units[i].route = None;
            let idx = self.job_index(id).expect("still queued");
            self.jobs.remove(idx);
            self.settle_offer(slot, defs);
            return self.finish_job(i);
        }
        if carried.is_some() {
            // Hands full of something the camp doesn't take: done for now.
            self.jobs.remove(idx);
            return self.finish_job(i);
        }
        let Some((item, need)) = self.trade_source(slot, defs) else {
            return self.abandon_job(i, id, defs);
        };
        if self.keep_walking(i, defs) {
            return;
        }
        let flood = self.flood_from(i);
        let Some((source, stand)) = self.nearest_source(&flood, item) else {
            return self.abandon_job(i, id, defs);
        };
        if self.units[i].route.is_none() {
            match flood.path_to(stand) {
                Some(path) => self.units[i].route = Some(route_from(path)),
                None => return self.abandon_job(i, id, defs),
            }
        }
        if !self.walk(i, defs) {
            return;
        }
        let take = self.take_for_build(i, source, need, defs);
        if take == 0 {
            return self.abandon_job(i, id, defs);
        }
        self.units[i].carrying = Some(Stack { item, count: take });
        self.units[i].route = None;
    }
}

#[cfg(test)]
mod tests;
