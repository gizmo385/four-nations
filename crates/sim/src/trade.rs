//! Caravans (#437, docs/design/pacing.md section 2): the data side, `data/trade.toml`.
//!
//! From the moment a chapter opens them (`[caravans].opens`, #439), traders from the other three
//! nations take turns to camp near the settlement once a season, each with a list of fixed
//! barters (`[[caravan.deal]]`: your food and wood for their stone, ore, tools, metal or bricks)
//! and a gift they ask for (`[[caravan.request]]`). There are no prices and no currency: a deal
//! is "these goods for those", up to `times` a visit. What the world does with them is
//! `world/trade.rs`.
//!
//! Loaded strictly, like `endeavor.rs`: unknown keys are rejected, items resolve at load
//! (`"food"` means any food item, as the food sign counts them), and every count is
//! range-checked. A reward whose system doesn't exist yet (`chart`) is refused at load, the way
//! the endeavor refuses a sign kind it can't count.

use serde::Deserialize;

use crate::defs::DefsError;
use crate::endeavor::ChapterDef;
use crate::jobs::{ItemDef, ItemId};
use crate::nation::Nation;

/// The most of one good a deal or request may ask for or give, in one side.
pub const MAX_COUNT: u32 = 500;

/// The most food one visit may sell in all (#437: "at most about 10 food a visit, paid in
/// stone", so farms stay necessary; #440 sets the exact lots).
pub const MAX_FOOD_PER_VISIT: u32 = 10;

/// `data/trade.toml`. Empty (no caravans) when the data has none, as in `Defs::from_files`: then
/// no caravan ever comes.
#[derive(Debug, Clone, Default)]
pub struct TradeDef {
    pub schedule: Option<Schedule>,
    /// One per visiting nation, in file order.
    pub caravans: Vec<CaravanDef>,
}

/// `[caravans]`: when caravans come and where they camp.
#[derive(Debug, Clone, PartialEq)]
pub struct Schedule {
    /// The chapter whose meeting opens the road (`World::chapter_met`); `None` means minute 0.
    pub opens: Option<String>,
    pub interval_minutes: f32,
    /// How far into each interval a caravan arrives.
    pub offset_minutes: f32,
    pub stay_minutes: f32,
    /// The camp's anchor is this many path tiles from the settlement anchor, inclusive.
    pub camp_distance: (u32, u32),
    pub deals_per_visit: u32,
    /// Whose turn it is, in order; the settlement's own nation is skipped.
    pub order: Vec<Nation>,
}

impl Schedule {
    /// The nations that visit `home`, in turn.
    pub fn visitors(&self, home: Nation) -> impl Iterator<Item = Nation> + '_ {
        self.order.iter().copied().filter(move |&n| n != home)
    }
}

/// One nation's caravan.
#[derive(Debug, Clone, PartialEq)]
pub struct CaravanDef {
    pub nation: Nation,
    /// "Fire Nation traders".
    pub name: String,
    pub deals: Vec<DealDef>,
    pub requests: Vec<RequestDef>,
}

/// One side of goods: `count` of any of `items` (one item, or every food for `"food"`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Goods {
    pub items: Vec<ItemId>,
    pub count: u32,
    /// `"food"`: any food item counts.
    pub any_food: bool,
}

impl Goods {
    pub fn accepts(&self, item: ItemId) -> bool {
        self.items.contains(&item)
    }
}

/// A fixed barter: the settlement hands over `give`, the caravan leaves `get` (or research
/// points), up to `times` a visit.
#[derive(Debug, Clone, PartialEq)]
pub struct DealDef {
    pub give: Vec<Goods>,
    /// Concrete items only: a stack of each appears at the camp.
    pub get: Vec<(ItemId, u32)>,
    /// Research points each time it completes (Air's knowledge), on top of `get`.
    pub knowledge: u32,
    pub times: u32,
}

/// A gift the caravan asks for, once a visit, and what it grants.
#[derive(Debug, Clone, PartialEq)]
pub struct RequestDef {
    /// "Food for the families at Fire Fountain City".
    pub name: String,
    pub give: Vec<Goods>,
    pub reward: Reward,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Reward {
    /// Research points.
    Knowledge(u32),
    /// Draw toward the next newcomer, in milli-units (`World::add_draw`, #435).
    Draw(u32),
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct TradeFile {
    caravans: RawSchedule,
    #[serde(default)]
    caravan: Vec<RawCaravan>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawSchedule {
    #[serde(default)]
    opens: Option<String>,
    interval_minutes: f32,
    offset_minutes: f32,
    stay_minutes: f32,
    camp_distance: [u32; 2],
    deals_per_visit: u32,
    order: Vec<Nation>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawCaravan {
    nation: Nation,
    name: String,
    #[serde(default)]
    deal: Vec<RawDeal>,
    #[serde(default)]
    request: Vec<RawRequest>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawGoods {
    item: String,
    count: u32,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawDeal {
    give: Vec<RawGoods>,
    #[serde(default)]
    get: Vec<RawGoods>,
    #[serde(default)]
    knowledge: u32,
    times: u32,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawRequest {
    name: String,
    give: Vec<RawGoods>,
    reward: RawReward,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawReward {
    #[serde(default)]
    knowledge: Option<u32>,
    #[serde(default)]
    draw: Option<f32>,
    #[serde(default)]
    chart: Option<u32>,
}

impl TradeFile {
    /// Validates the file and resolves item and chapter ids. `draw_bank` is
    /// `[growth.draw].bank`, the most a draw reward may grant. `copy_item` is the Archive's scroll
    /// copy (#709), if the buildings have one: no deal or request may take it and pay knowledge,
    /// or copies would turn back into research points (#711).
    pub(crate) fn resolve(
        self,
        items: &[ItemDef],
        chapters: &[ChapterDef],
        draw_bank: f32,
        copy_item: Option<ItemId>,
    ) -> Result<TradeDef, DefsError> {
        let err = |msg: String| DefsError(format!("trade.toml: {msg}"));
        let s = self.caravans;
        if let Some(c) = &s.opens
            && !chapters.iter().any(|d| d.id == *c)
        {
            return Err(err(format!("caravans.opens: unknown chapter {c:?}")));
        }
        for (what, v) in
            [("interval_minutes", s.interval_minutes), ("stay_minutes", s.stay_minutes)]
        {
            if !(v.is_finite() && v > 0.0 && v <= 600.0) {
                return Err(err(format!("caravans.{what} must be in (0, 600]")));
            }
        }
        if !(s.offset_minutes.is_finite() && s.offset_minutes >= 0.0) {
            return Err(err("caravans.offset_minutes must be 0 or more".into()));
        }
        if s.offset_minutes + s.stay_minutes > s.interval_minutes {
            return Err(err(
                "caravans: offset_minutes + stay_minutes must fit in interval_minutes".into(),
            ));
        }
        let [near, far] = s.camp_distance;
        if near > far || far > 64 {
            return Err(err(
                "caravans.camp_distance must be [near, far] with near <= far <= 64".into()
            ));
        }
        if !(1..=8).contains(&s.deals_per_visit) {
            return Err(err("caravans.deals_per_visit must be in 1..=8".into()));
        }
        if s.order.is_empty() {
            return Err(err("caravans.order must name at least one nation".into()));
        }
        for (i, n) in s.order.iter().enumerate() {
            if s.order[..i].contains(n) {
                return Err(err(format!("caravans.order lists {n:?} twice")));
            }
        }
        let mut caravans: Vec<CaravanDef> = Vec::with_capacity(self.caravan.len());
        for raw in self.caravan {
            let nation = raw.nation;
            if caravans.iter().any(|c| c.nation == nation) {
                return Err(err(format!("two caravans for {nation:?}")));
            }
            if !s.order.contains(&nation) {
                return Err(err(format!("caravan {nation:?} isn't in caravans.order")));
            }
            caravans.push(
                raw.resolve(items, draw_bank, copy_item)
                    .map_err(|m| err(format!("caravan {nation:?}: {m}")))?,
            );
        }
        if let Some(n) = s.order.iter().find(|n| !caravans.iter().any(|c| c.nation == **n)) {
            return Err(err(format!("caravans.order names {n:?}, which has no [[caravan]]")));
        }
        let schedule = Schedule {
            opens: s.opens,
            interval_minutes: s.interval_minutes,
            offset_minutes: s.offset_minutes,
            stay_minutes: s.stay_minutes,
            camp_distance: (near, far),
            deals_per_visit: s.deals_per_visit,
            order: s.order,
        };
        Ok(TradeDef { schedule: Some(schedule), caravans })
    }
}

fn item_id(items: &[ItemDef], id: &str) -> Result<ItemId, String> {
    items
        .iter()
        .position(|d| d.id == id)
        .map(|n| ItemId(n as u16))
        .ok_or_else(|| format!("unknown item {id:?}"))
}

/// One side's goods: each entry known and counted in range, `"food"` for any food, and no two
/// entries able to take the same item (so a delivery always counts toward exactly one).
fn resolve_goods(raw: Vec<RawGoods>, items: &[ItemDef]) -> Result<Vec<Goods>, String> {
    if raw.is_empty() {
        return Err("give must list at least one good".into());
    }
    let mut out: Vec<Goods> = Vec::with_capacity(raw.len());
    for g in raw {
        if !(1..=MAX_COUNT).contains(&g.count) {
            return Err(format!("{}: count must be in 1..={MAX_COUNT}", g.item));
        }
        let goods = if g.item == "food" {
            let food: Vec<ItemId> = items
                .iter()
                .enumerate()
                .filter(|(_, d)| d.food.is_some())
                .map(|(n, _)| ItemId(n as u16))
                .collect();
            Goods { items: food, count: g.count, any_food: true }
        } else {
            Goods { items: vec![item_id(items, &g.item)?], count: g.count, any_food: false }
        };
        if out.iter().any(|o| o.items.iter().any(|i| goods.items.contains(i))) {
            return Err(format!("{} overlaps another good on the same side", g.item));
        }
        out.push(goods);
    }
    Ok(out)
}

impl RawCaravan {
    fn resolve(
        self,
        items: &[ItemDef],
        draw_bank: f32,
        copy_item: Option<ItemId>,
    ) -> Result<CaravanDef, String> {
        let takes_copies =
            |give: &[Goods]| copy_item.is_some_and(|c| give.iter().any(|g| g.accepts(c)));
        if self.name.trim().is_empty() {
            return Err("name must not be empty".into());
        }
        if self.deal.is_empty() {
            return Err("needs at least one [[caravan.deal]]".into());
        }
        if self.deal.len() > usize::from(u8::MAX) {
            return Err("too many deals".into());
        }
        let mut deals = Vec::with_capacity(self.deal.len());
        for (n, d) in self.deal.into_iter().enumerate() {
            let at = |m: String| format!("deal {}: {m}", n + 1);
            if !(1..=10).contains(&d.times) {
                return Err(at("times must be in 1..=10".into()));
            }
            if d.knowledge > 10_000 {
                return Err(at("knowledge must be at most 10000".into()));
            }
            let give = resolve_goods(d.give, items).map_err(at)?;
            if d.knowledge > 0 && takes_copies(&give) {
                return Err(at("takes scroll copies and pays knowledge, a loop".into()));
            }
            let mut get = Vec::with_capacity(d.get.len());
            for g in d.get {
                if g.item == "food" {
                    return Err(at("get names concrete items, not \"food\"".into()));
                }
                if !(1..=MAX_COUNT).contains(&g.count) {
                    return Err(at(format!("{}: count must be in 1..={MAX_COUNT}", g.item)));
                }
                let id = item_id(items, &g.item).map_err(at)?;
                if get.iter().any(|&(i, _)| i == id) {
                    return Err(at(format!("{} listed twice in get", g.item)));
                }
                get.push((id, g.count));
            }
            if get.is_empty() && d.knowledge == 0 {
                return Err(at("needs get or knowledge".into()));
            }
            let food: u32 = get
                .iter()
                .filter(|&&(i, _)| items[i.0 as usize].food.is_some())
                .map(|&(_, c)| c * d.times)
                .sum();
            if food > MAX_FOOD_PER_VISIT {
                return Err(at(format!(
                    "sells {food} food a visit; caravans sell at most {MAX_FOOD_PER_VISIT}"
                )));
            }
            deals.push(DealDef { give, get, knowledge: d.knowledge, times: d.times });
        }
        let mut requests = Vec::with_capacity(self.request.len());
        for r in self.request {
            let at = |m: String| format!("request {:?}: {m}", r.name);
            if r.name.trim().is_empty() {
                return Err("request name must not be empty".into());
            }
            let reward = match (r.reward.knowledge, r.reward.draw, r.reward.chart) {
                (Some(k), None, None) if (1..=10_000).contains(&k) => Reward::Knowledge(k),
                (None, Some(d), None) if d.is_finite() && d > 0.0 && d <= draw_bank => {
                    Reward::Draw((d * 1000.0).round() as u32)
                }
                (None, None, Some(_)) => {
                    // #402's reveal is a Map Room's, measured from the room; a caravan's chart
                    // needs its own origin first.
                    return Err(at("chart rewards aren't supported yet".into()));
                }
                (None, None, None) => return Err(at("needs a reward".into())),
                (Some(_), None, None) => return Err(at("knowledge must be in 1..=10000".into())),
                (None, Some(_), None) => {
                    return Err(at(format!("draw must be in (0, {draw_bank}] (growth.draw.bank)")));
                }
                _ => return Err(at("one reward only".into())),
            };
            let give = resolve_goods(r.give, items).map_err(at)?;
            if matches!(reward, Reward::Knowledge(_)) && takes_copies(&give) {
                return Err(at("takes scroll copies and pays knowledge, a loop".into()));
            }
            requests.push(RequestDef { name: r.name.clone(), give, reward });
        }
        if requests.is_empty() {
            return Err("needs at least one [[caravan.request]]".into());
        }
        Ok(CaravanDef { nation: self.nation, name: self.name, deals, requests })
    }
}

#[cfg(test)]
mod tests;
