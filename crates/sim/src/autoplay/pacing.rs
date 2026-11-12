//! The economy opening's use of draw and caravans (docs/design/pacing.md, sections 1 and 2;
//! #440): the three things a settlement can do to grow faster and the one thing it can do with
//! a surplus. Like the rest of `Economy`, every choice reads the world fresh, so a step repeated
//! is harmless and two identical worlds get the same commands. `Economy::with_beds_only` skips
//! all of it (the passive player: pacing.md's "beds-only" row).
//!
//! **Draw** (`World::draw_multiplier`) rises with warm beds, food variety and spare beds. Spare
//! beds the opening already keeps (one ahead of the headcount, `Economy::building_target`).
//!
//! - **Warm beds** (`raise_bed`, `raise_campfire`): every bed stands within a campfire's heat, so
//!   `World::is_heated` holds for all of them. Beds go on a free tile in reach of the campfire
//!   with the fewest beds, and a bed that still ends up outside every campfire's reach (their
//!   tiles are full, or it came before any campfire) gets another campfire on a tile within
//!   reach of it, one at a time, up to `MAX_CAMPFIRES`. A finished bed no campfire can be put
//!   within reach of is taken down, and the next goes where it is warm.
//! - **Food variety** (`forage_missing_kinds`): while fewer than `FOOD_KINDS_WANTED` food kinds
//!   have `food_kind_min` in stores, the standing harvest orders reach past `FORAGE_RADIUS`, to
//!   the nearest wild source of each kind that is short, `FAR_FORAGE_TILES` tiles at a time.
//!
//! **Caravans** (`take_deals`; `docs/design/pacing.md` section 2). Each step with a caravan
//! camped, in this order and for as long as the goods can be spared:
//!
//! 1. the deals that feed a sign that isn't met yet (the stone sign takes the stone and bricks a
//!    caravan sells; the ore sign is named too: a bought ore counts toward the harvested tally,
//!    #834, so a caravan's ore is a second route to the sign);
//! 2. its request, when the goods can be spared (a request's reward is draw or knowledge);
//! 3. then the deals that spend food or wood, whichever is the more over its threshold.
//!
//! **What can be spared.** A good is spared while the stores, less what taken deals still owe
//! the camp, stay at or above its *threshold* after the payment: for food `FOOD_KEPT_PER_HEAD` a
//! settler (at least a newcomer's `food_per_newcomer`), and once the settlement is as big as the
//! "population" sign asks, the "food" sign's own target too (the gate wants that much in stores);
//! for wood `WOOD_KEPT`. While the settlement is still short of that size, a request or a sign
//! deal may dip to `GROWING_KEPT` percent of the threshold. These are plain constants
//! here, not data: they say how much a *player* holds back, which is the opening's business, not
//! the sim's. The trade goods of phase 3 (`TRADE_GOODS`: wares and scroll copies, made for the
//! caravans and good for nothing else, #713) have no threshold: whatever stores hold, less what
//! taken deals still owe the camp, is spared, and a deal that pays in them goes ahead of the
//! rest. Nothing else is ever paid: stone and the rest are the gate's.

use crate::building::BuildingId;
use crate::defs::Defs;
use crate::endeavor::{SignKind, Tally};
use crate::geom::{Footprint, Pos};
use crate::jobs::ItemId;
use crate::world::{Command, OfferKind, World};

use super::{
    BUILD_SEARCH_RADIUS, DEPOSIT_RADIUS, FORAGE_RADIUS, building_count, have_enough, is_wild_food,
    nearby, nearest_build_site,
};

/// The most campfires the opening raises for warm beds. The settlement's beds fit well within
/// this: a campfire's 5x5 holds up to 24 beds, and the sites are tightly clustered.
const MAX_CAMPFIRES: u32 = 8;

/// How far east of the settlement's anchor the passive player (`Economy::beds_only`) lights its
/// one campfire, so the beds it puts near the anchor are not in its reach.
pub(super) const PASSIVE_FIRE_OFFSET: i32 = 8;

/// How many food kinds the opening keeps at `food_kind_min` or more: two is draw's first food
/// bonus (a kind beyond the first), which with warm beds is the 100% draw the pacing targets
/// assume (`docs/design/pacing.md`, section 1). A third kind would be 120%.
const FOOD_KINDS_WANTED: usize = 2;

/// How many far wild sources `forage_missing_kinds` stands up for a food kind in short supply.
const FAR_FORAGE_TILES: usize = 2;

/// Food a deal or request leaves in stores, per settler (module doc, "What can be spared").
/// Winter 2's food per head is 3 to 8 on the harness's targets, so this holds back the top of
/// that band.
const FOOD_KEPT_PER_HEAD: u32 = 8;

/// What a request or a sign deal may dip to, as a percent of the threshold, while the settlement
/// is still short of the "population" sign: a request's draw and a sign's material are worth more
/// to a growing settlement than a store kept in hand (module doc).
const GROWING_KEPT: u32 = 50;

/// Wood a deal or request leaves in stores (module doc): a bed is 5 and a building about 20 to
/// 60, so this is a few buildings in hand, well under `wood_at_120`'s 300.
const WOOD_KEPT: u32 = 150;

/// The items made for the caravans, by id (module doc): paid whenever stores hold them.
const TRADE_GOODS: &[&str] = &["wares", "scroll_copy"];

fn campfire(defs: &Defs) -> Option<BuildingId> {
    defs.building_id("campfire")
}

/// A campfire's heat reach for this settlement: `radius_for`'s rounding, as the sim applies it.
fn campfire_reach(world: &World, defs: &Defs) -> i32 {
    campfire(defs)
        .and_then(|c| defs.building(c).heat)
        .map_or(0, |h| h.radius_for(world.modifiers().heating) as i32)
}

/// Every campfire, standing or being built, as its footprint.
fn campfires(world: &World, defs: &Defs) -> Vec<Footprint> {
    let Some(fire) = campfire(defs) else { return Vec::new() };
    let (w, h) = defs.building(fire).footprint;
    let mut all: Vec<Footprint> = world
        .buildings()
        .filter(|b| b.id == fire)
        .map(|b| Footprint::new(b.anchor, w, h))
        .chain(world.build_sites().filter(|&(_, id, _)| id == fire).map(|(f, _, _)| f))
        .collect();
    all.sort_by_key(|f| f.anchor);
    all
}

/// Every bed, standing or designated, by its anchor tile, in position order.
fn beds(world: &World, defs: &Defs) -> Vec<Pos> {
    let mut all: Vec<Pos> = world
        .buildings()
        .filter(|b| defs.building(b.id).bed.is_some())
        .map(|b| b.anchor)
        .chain(
            world
                .build_sites()
                .filter(|&(_, id, _)| defs.building(id).bed.is_some())
                .map(|(f, _, _)| f.anchor),
        )
        .collect();
    all.sort();
    all
}

/// Every bed whose tile no campfire, standing or being built, reaches, in position order.
fn cold_beds(world: &World, defs: &Defs) -> Vec<Pos> {
    let reach = campfire_reach(world, defs);
    let fires = campfires(world, defs);
    beds(world, defs)
        .into_iter()
        .filter(|&bed| !world.is_heated(bed) && !fires.iter().any(|f| f.distance(bed) <= reach))
        .collect()
}

/// What `Economy::building_target` reports for the campfire: what stands or is under way, and at
/// least one (the first is wanted as soon as its tech is known). `raise_campfire` does the
/// placing, which a plain target can't say (it needs a site within a bed's reach).
pub(super) fn campfire_target(world: &World, defs: &Defs) -> u32 {
    campfire(defs).map_or(0, |fire| building_count(world, fire).max(1))
}

/// Raises a campfire when one is wanted, one site at a time and only once its cost is in hand
/// (`ensure_building`'s own rules): the first beside the bed nearest the anchor (the anchor if
/// there is no bed yet), and then one beside the first bed no campfire reaches, on a tile within
/// its reach, up to `MAX_CAMPFIRES` (module doc, "Warm beds"). A cold bed with no such tile is
/// left cold.
pub(super) fn raise_campfire(world: &mut World, defs: &Defs, anchor: Pos) {
    let Some(fire) = campfire(defs) else { return };
    let def = defs.building(fire);
    if def.unlocked_by.is_some_and(|t| !world.research().is_known(t))
        || world.build_sites().any(|(_, id, _)| id == fire)
        || !have_enough(world, &def.cost)
    {
        return;
    }
    let have = building_count(world, fire);
    let site = if have == 0 {
        let near = beds(world, defs)
            .into_iter()
            .min_by_key(|&b| (b.chebyshev(anchor), b))
            .unwrap_or(anchor);
        nearest_build_site(world, defs, near, fire, BUILD_SEARCH_RADIUS)
    } else if have < MAX_CAMPFIRES {
        let reach = campfire_reach(world, defs);
        let cold = cold_beds(world, defs);
        let site = cold.iter().find_map(|&bed| nearest_build_site(world, defs, bed, fire, reach));
        if site.is_none() {
            // No tile in reach of any cold bed (it is boxed in): take a finished one down.
            if let Some(&bed) = cold.iter().find(|&&b| world.building_at(b).is_some()) {
                let _ = world.apply(Command::Deconstruct { pos: bed }, defs);
            }
        }
        site
    } else {
        None
    };
    if let Some(pos) = site {
        let _ = world.apply(Command::DesignateBuild { pos, building: fire }, defs);
    }
}

/// Builds a bed when `target` wants one more, by `ensure_building`'s rules (one site at a time,
/// only once its cost is in hand), on a tile within reach of a campfire when there is one: the
/// campfire with the fewest beds in its reach first. With no free tile in any campfire's reach,
/// anywhere near the anchor, and `raise_campfire` then warms it.
pub(super) fn raise_bed(world: &mut World, defs: &Defs, anchor: Pos, target: u32) {
    let Some(bed) = defs.building_id("bed") else { return };
    let def = defs.building(bed);
    if def.unlocked_by.is_some_and(|t| !world.research().is_known(t))
        || building_count(world, bed) >= target
        || world.build_sites().any(|(_, id, _)| id == bed)
        || !have_enough(world, &def.cost)
    {
        return;
    }
    let reach = campfire_reach(world, defs);
    let beds = beds(world, defs);
    let mut fires = campfires(world, defs);
    fires.sort_by_key(|f| (beds.iter().filter(|&&b| f.distance(b) <= reach).count(), f.anchor));
    let warm = fires.iter().find_map(|f| nearest_build_site(world, defs, f.anchor, bed, reach));
    let site = warm.or_else(|| nearest_build_site(world, defs, anchor, bed, BUILD_SEARCH_RADIUS));
    if let Some(pos) = site {
        let _ = world.apply(Command::DesignateBuild { pos, building: bed }, defs);
    }
}

/// Food items of every kind in `defs`, in item order.
fn food_kinds(defs: &Defs) -> impl Iterator<Item = ItemId> + '_ {
    (0..defs.items.len() as u16).map(ItemId).filter(|&i| defs.food_of(i).is_some())
}

/// Stands harvest orders on far wild sources of the food kinds short of `food_kind_min` (module
/// doc, "Food variety"): only while fewer kinds are stocked than draw can count.
pub(super) fn forage_missing_kinds(world: &mut World, defs: &Defs, anchor: Pos) {
    let draw = defs.needs.growth.draw;
    let want = FOOD_KINDS_WANTED;
    let stocked = |world: &World, kind: ItemId| world.stored_total(kind) >= draw.food_kind_min;
    let have = food_kinds(defs).filter(|&k| stocked(world, k)).count();
    if have >= want {
        return;
    }
    let mut short: Vec<ItemId> = food_kinds(defs).filter(|&k| !stocked(world, k)).collect();
    short.sort_by_key(|&k| (world.stored_total(k), k));
    for kind in short {
        let wild = |t: &crate::mapgen::Tile, defs: &Defs| {
            is_wild_food(t, defs)
                && t.resource.and_then(|r| defs.harvest(r)).is_some_and(|h| h.item == kind)
        };
        let mut sources: Vec<Pos> = nearby(anchor, DEPOSIT_RADIUS, world, defs, wild)
            .into_iter()
            .filter(|p| p.chebyshev(anchor) > FORAGE_RADIUS)
            .collect();
        sources.sort_by_key(|&p| (p.chebyshev(anchor), p));
        // Orders already standing out there count against the few asked for.
        let mut standing = sources.iter().filter(|&&p| world.is_repeating_harvest(p)).count();
        let fresh: Vec<Pos> =
            sources.into_iter().filter(|&p| !world.is_repeating_harvest(p)).collect();
        for pos in fresh {
            if standing >= FAR_FORAGE_TILES {
                break;
            }
            if world.apply(Command::DesignateRepeatingHarvest { pos }, defs).is_ok() {
                standing += 1;
            }
        }
    }
}

/// How much of each good the camp is still owed by deals already taken, so a second take doesn't
/// count goods the first will haul.
#[derive(Default)]
struct Owed {
    food: u32,
    wood: u32,
    /// What the camp is owed of each trade good.
    goods: Vec<(ItemId, u32)>,
}

/// What one side of a deal is, for the spare test.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Pay {
    Food,
    Item(ItemId),
}

fn pay_of(g: &crate::trade::Goods) -> Option<Pay> {
    if g.any_food {
        Some(Pay::Food)
    } else if let [item] = g.items[..] {
        Some(Pay::Item(item))
    } else {
        None
    }
}

/// How the opening stands on food and wood: stores, less what the camp is owed, and the level
/// it keeps (module doc, "What can be spared").
struct Stock {
    food: u32,
    food_keep: u32,
    wood: u32,
    wood_keep: u32,
    wood_item: Option<ItemId>,
    /// Each trade good in stores, less what the camp is owed (`TRADE_GOODS`): spared in full.
    goods: Vec<(ItemId, u32)>,
    /// Still short of the "population" sign: a request or a sign deal may then dip to
    /// `GROWING_KEPT` percent of the threshold (module doc).
    growing: bool,
}

impl Stock {
    fn read(world: &World, defs: &Defs) -> Self {
        let wood_item = defs.item_id("wood");
        let trade_goods: Vec<ItemId> = TRADE_GOODS.iter().filter_map(|g| defs.item_id(g)).collect();
        let mut owed = Owed::default();
        let slots = world.caravan().map_or(0, |v| v.offers.len() as u8);
        for slot in 0..slots {
            for (g, left) in world.trade_needs(slot, defs).unwrap_or_default() {
                match pay_of(g) {
                    Some(Pay::Food) => owed.food += left,
                    Some(Pay::Item(i)) if Some(i) == wood_item => owed.wood += left,
                    Some(Pay::Item(i)) if trade_goods.contains(&i) => {
                        match owed.goods.iter_mut().find(|(g, _)| *g == i) {
                            Some((_, n)) => *n += left,
                            None => owed.goods.push((i, left)),
                        }
                    }
                    // Only food, wood and the trade goods are ever paid, so only they can be owed.
                    Some(Pay::Item(_)) | None => {}
                }
            }
        }
        let people = world.population() as u32;
        let target = |id: &str| {
            defs.endeavor
                .signs
                .iter()
                .find(|s| s.id == id)
                .and_then(|s| s.target_for(world.nation))
                .unwrap_or(0)
        };
        let mut food_keep = (FOOD_KEPT_PER_HEAD * people).max(defs.needs.growth.food_per_newcomer);
        let growing = people < target("population");
        // Once the settlement is as big as the gate asks, what it holds is the food sign's.
        if !growing {
            food_keep = food_keep.max(target("food"));
        }
        Self {
            food: world.stored_food(defs).saturating_sub(owed.food),
            food_keep,
            wood: wood_item.map_or(0, |w| world.stored_total(w)).saturating_sub(owed.wood),
            wood_keep: WOOD_KEPT,
            wood_item,
            goods: trade_goods
                .iter()
                .map(|&g| {
                    let owed = owed.goods.iter().find(|(i, _)| *i == g).map_or(0, |&(_, n)| n);
                    (g, world.stored_total(g).saturating_sub(owed))
                })
                .collect(),
            growing,
        }
    }

    /// The stock of `pay` and the level to leave, or `None` for what is never paid.
    fn level(&self, pay: Pay) -> Option<(u32, u32)> {
        match pay {
            Pay::Food => Some((self.food, self.food_keep)),
            Pay::Item(i) if Some(i) == self.wood_item => Some((self.wood, self.wood_keep)),
            Pay::Item(i) => self.goods.iter().find(|&&(g, _)| g == i).map(|&(_, have)| (have, 0)),
        }
    }

    /// Whether paying all of `give` still leaves each threshold in stores. A request or a sign
    /// deal (`tier` 0 or 1) may dip to `GROWING_KEPT` percent of it while the settlement grows.
    fn can_pay(&self, give: &[crate::trade::Goods], tier: u8) -> bool {
        let percent = if tier < 2 && self.growing { GROWING_KEPT } else { 100 };
        give.iter().all(|g| {
            let Some((have, keep)) = pay_of(g).and_then(|p| self.level(p)) else { return false };
            have >= g.count + keep * percent / 100
        })
    }

    /// How far over its threshold the good a deal pays in is, in percent: the most over of its
    /// goods (module doc, step 3). Higher goes first.
    fn over(&self, give: &[crate::trade::Goods]) -> u32 {
        give.iter()
            .filter_map(|g| pay_of(g).and_then(|p| self.level(p)))
            .map(|(have, keep)| have * 100 / keep.max(1))
            .max()
            .unwrap_or(0)
    }
}

/// Whether `item` feeds a sign that isn't met yet: the sign counts it in stores or as harvested
/// (module doc, step 1). The chapter's signs aren't in `defs.endeavor.signs`, so they can't be
/// named here.
fn feeds_missing_sign(world: &World, defs: &Defs, item: ItemId) -> bool {
    let progress = world.endeavor_signs(defs);
    defs.endeavor.signs.iter().zip(progress).any(|(sign, p)| {
        !p.met
            && match &sign.kind {
                SignKind::Stored { items } | SignKind::Tally(Tally::Harvested(items)) => {
                    items.contains(&item)
                }
                _ => false,
            }
    })
}

/// Takes the camped caravan's deals as the module doc lays out, one `Command::TakeDeal` at a
/// time, re-reading the world after each so the goods already promised are never promised twice.
pub(super) fn take_deals(world: &mut World, defs: &Defs) {
    loop {
        let Some(slot) = next_take(world, defs) else { return };
        if world.apply(Command::TakeDeal { slot }, defs).is_err() {
            return;
        }
    }
}

/// The slot the opening takes next, if any: signs first, then the request, then the most
/// over-threshold payment.
fn next_take(world: &World, defs: &Defs) -> Option<u8> {
    let visit = world.caravan()?;
    let caravan = world.caravan_def(defs)?;
    let stock = Stock::read(world, defs);
    let mut best: Option<((u8, u32), u8)> = None;
    for (slot, offer) in visit.offers.iter().enumerate() {
        let (give, times, tier) = match offer.kind {
            OfferKind::Deal(d) => {
                let deal = &caravan.deals[usize::from(d)];
                let feeds = deal.get.iter().any(|&(i, _)| feeds_missing_sign(world, defs, i));
                (&deal.give, deal.times, if feeds { 0 } else { 2 })
            }
            OfferKind::Request(r) => (&caravan.requests[usize::from(r)].give, 1, 1),
        };
        if offer.taken >= times || !stock.can_pay(give, tier) {
            continue;
        }
        // Lower tier first, then the more over-threshold payment, then the earlier slot.
        let key = (tier, u32::MAX - stock.over(give));
        if best.is_none_or(|(k, _)| key < k) {
            best = Some((key, slot as u8));
        }
    }
    best.map(|(_, slot)| slot)
}

#[cfg(test)]
mod tests;
