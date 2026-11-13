//! The economy opening's play in the United City (docs/design/horizons-phase-6.md, "Measuring
//! it", #764): what it does once the gate stands, to learn from the newcomers and use what it
//! learns. Everything here waits for `World::united`, so nothing before the gate changes. Like
//! the rest of `Economy`, every choice reads the world fresh, so a step repeated is harmless.
//!
//! - **Research** (`set_targets`). Another nation's teachable techs first, each as soon as it
//!   can be queued, cheapest first (ties in the list's order); then each fusion, in file order. A
//!   fusion whose bill names an item missing from stores altogether waits for the next one, as
//!   `hold_missing_bills` does for the home tree. It takes the research queue over from the home
//!   tree (the points in the pool carry over), but never interrupts an exchange tech that can be
//!   finished, nor a bill that has begun to fill. A fusion passed over for its bill is queued
//!   after all when the home tree has nothing left. It only asks for what `set_target` would
//!   take (`can_queue`): a refused command a minute would raise the `refused` flag.
//! - **Beds and wood**: the "population" sign's ceiling lifts (`Economy::bed_ceiling`): the sign
//!   has been met, and a newcomer needs a free bed. A bed costs wood, and the trees near the
//!   stockpile are gone by the gate, so `gather_far_wood` cuts the far ones while stores hold
//!   under `WOOD_WANTED`.
//! - **Aqueducts** (`raise_aqueduct`): one aqueduct, the shortest run of free land from the water
//!   to within the channel's reach of a farm plot, at most `AQUEDUCT_TILES` long and only when
//!   the stone for all of it is in stores. The same breadth-first search as Carve Canal's.
//! - **Steelworks**: the one-of-everything sweep raises it, like any recipe building.
//! - **Sky Lifts** (`raise_lift`): one pair, when a range splits the work from the stores: the
//!   deposit the opening quarries that walking reaches the slow way round (`LIFT_DETOUR` times the
//!   straight line, at least the pair's `min_tiles`), a pair end on rock at each side that saves
//!   the most walking, `LIFT_SAVING` or more (ten tiles).
//! - **Steam Power** (`raise_bank_engine`): one Steam Engine on the bank, the nearest tile of
//!   open ground beside the water. The inland engine stays: its fuel half is #433's, and taking it
//!   down could unpower the yard.
//! - **Airships** (`plan_voyage`): the Dock the sweep raises sends one voyage at a time, taking one
//!   caravan deal that fits the hold and that stores can spare, preferring the one whose goods
//!   (stone, ore) the gate wanted and the city holds least of.
//! - **Cloudbending** (`call_rain`): each spring, Call Rain on the farm plot with the most plots
//!   within the patch of it, while no rain is falling.

use std::collections::{BTreeMap, BTreeSet};

use crate::bending::AbilityId;
use crate::defs::Defs;
use crate::geom::{Footprint, Pos};
use crate::jobs::ItemId;
use crate::nation::Nation;
use crate::research::TechId;
use crate::world::{Command, ExchangeKind, ExchangeStanding, World};

use super::horizons::far_deposits;
use super::{DEPOSIT_RADIUS, building_count, deposit_items, have_enough, pacing};

/// The longest aqueduct the opening lays: 3 stone a tile, so 72 stone at most.
pub(super) const AQUEDUCT_TILES: usize = 24;

/// A deposit counts as split from the stores by a range when walking to it costs at least this
/// many times the straight line (`path::STRAIGHT` units a tile).
const LIFT_DETOUR: f64 = 1.6;

/// The walking a lift pair must save, in `path.rs` cost units (10 a straight tile): ten tiles.
const LIFT_SAVING: i64 = 100;

/// How far from the stockpile edge, and from the deposit's stand, a lift end is looked for.
const LIFT_SEARCH: i32 = 10;

/// How many minutes pass between the searches that cost a flood (`raise_lift`,
/// `raise_bank_engine`).
const SEARCH_EVERY: u64 = 5;

/// How far from a river or lake the bank engine's site is looked for, in tiles.
const BANK_SEARCH: i32 = 3;

/// The wet tiles `raise_bank_engine` tries, nearest the anchor first.
const BANK_TRIES: usize = 24;

/// The patch Call Rain covers, as the data names it (`radius`): the opening centres it on the
/// plot with the most plots in reach.
const RAIN_DEFAULT_RADIUS: u32 = 6;

/// Whether `exchange` lays the building itself, so the one-of-everything sweep leaves it alone.
pub(super) fn raises_itself(id: &str) -> bool {
    matches!(id, "aqueduct" | "sky_lift")
}

/// Whether the opening is in a United City.
pub(super) fn active(world: &World) -> bool {
    world.united()
}

fn known(world: &World, defs: &Defs, tech: &str) -> bool {
    defs.tech_id(tech).is_some_and(|t| world.research().is_known(t))
}

/// Whether the minute is one of the every-`SEARCH_EVERY` ones that search.
fn search_minute(world: &World) -> bool {
    let minute = world.tick / (60 * u64::from(crate::world::TICKS_PER_SECOND));
    minute.is_multiple_of(SEARCH_EVERY)
}

/// Whether `tech` is one the exchange page lists for this settlement.
fn is_exchange(defs: &Defs, nation: Nation, tech: TechId) -> bool {
    let t = defs.tech(tech);
    t.is_fusion() || t.borrowed_from(nation).is_some()
}

/// Sets the next exchange tech as the research target (module doc, "Research"). With
/// `hold_short`, a tech whose bill names an item missing from stores altogether is passed over;
/// the opening asks again without it when the home tree has nothing left to queue.
pub(super) fn set_targets(world: &mut World, defs: &Defs, hold_short: bool) {
    let nation = world.nation;
    let facts = world.facts();
    let queue = world.research().queue();
    // An exchange tech that can be finished is left to finish; one waiting on a condition that
    // has fallen is not worth holding the queue for.
    if queue.iter().any(|&t| {
        is_exchange(defs, nation, t) && defs.tech(t).unmet(nation, &facts, defs).is_empty()
    }) {
        return;
    }
    // A bill that has begun to fill would be refunded by a change of target.
    if queue.iter().any(|&t| world.research().bill(t, defs).iter().any(|&(_, got, _)| got > 0)) {
        return;
    }
    let exchange = world.exchange(defs);
    let mut borrowed: Vec<_> = exchange
        .entries
        .iter()
        .filter(|e| matches!(e.kind, ExchangeKind::Borrowed { .. }))
        .collect();
    borrowed.sort_by_key(|e| e.cost);
    let fusions = exchange.entries.iter().filter(|e| matches!(e.kind, ExchangeKind::Fusion { .. }));
    let wanted: Vec<TechId> = borrowed
        .into_iter()
        .chain(fusions)
        .filter(|e| !matches!(e.standing, ExchangeStanding::Known | ExchangeStanding::Queued))
        .map(|e| e.tech)
        .collect();
    for tech in wanted {
        // Only what `set_target` would take: asking for the rest would be refused every minute.
        if !can_queue(world, defs, tech) {
            continue;
        }
        // A bill with an item missing from stores altogether might never fill (tools, most often).
        let short =
            defs.tech(tech).cost_items.iter().any(|s| {
                !have_enough(world, &[crate::building::CostItem { item: s.item, count: 1 }])
            });
        if short && hold_short {
            continue;
        }
        if world.apply(Command::SetResearch { tech }, defs).is_ok() {
            return;
        }
    }
}

/// Whether `Research::set_target` would take `tech` now: every tech of its closure that isn't
/// known (the borrowed ones by their waived prereqs) has its conditions met.
fn can_queue(world: &World, defs: &Defs, tech: TechId) -> bool {
    let nation = world.nation;
    let facts = world.facts();
    let mut seen: BTreeSet<TechId> = BTreeSet::new();
    let mut stack = vec![tech];
    while let Some(t) = stack.pop() {
        if world.research().is_known(t) || !seen.insert(t) {
            continue;
        }
        let def = defs.tech(t);
        if !def.learnable_by(nation) || !def.unmet(nation, &facts, defs).is_empty() {
            return false;
        }
        if def.borrowed_from(nation).is_some() {
            stack.extend(defs.borrow_prereqs(t));
        } else {
            stack.extend(def.prereqs.iter().copied());
        }
    }
    true
}

fn plot_footprints(world: &World, defs: &Defs) -> Vec<Footprint> {
    let Some(plot) = defs.building_id("farm_plot") else { return Vec::new() };
    world.buildings().filter(|b| b.id == plot).map(|b| b.footprint()).collect()
}

const DIRS: [(i32, i32); 4] = [(0, -1), (1, 0), (0, 1), (-1, 0)];

/// Lays one aqueduct from the water to the farm plots (module doc).
pub(super) fn raise_aqueduct(world: &mut World, defs: &Defs) {
    let Some(id) = defs.building_id("aqueduct") else { return };
    let def = defs.building(id);
    let Some(aq) = def.aqueduct.as_ref() else { return };
    if !known(world, defs, "aqueducts") || building_count(world, id) > 0 {
        return;
    }
    let plots = plot_footprints(world, defs);
    if plots.is_empty() {
        return;
    }
    let zones: Vec<Pos> = world.stockpile_zones().collect();
    let ground: Vec<Pos> = world.ground_items().map(|(p, _)| p).collect();
    let free = |p: Pos| {
        world.check_build(p, id, defs).is_ok()
            && world.to_clear(p, id, defs).is_empty()
            && !zones.contains(&p)
            && !ground.contains(&p)
    };
    let wet = |p: Pos| world.map.get(p).is_some_and(|t| aq.source.contains(&t.terrain));
    let steps = |p: Pos| DIRS.map(|(dx, dy)| Pos::new(p.x + dx, p.y + dy));
    let radius = aq.irrigation_radius as i32;
    let waters = |p: Pos| plots.iter().any(|f| f.distance(p) <= radius);
    let mut parent: BTreeMap<Pos, Option<Pos>> = BTreeMap::new();
    let mut layer: Vec<Pos> = world
        .map
        .iter()
        .filter(|&(p, _)| steps(p).into_iter().any(wet))
        .map(|(p, _)| p)
        .filter(|&p| free(p))
        .collect();
    layer.sort();
    for &p in &layer {
        parent.insert(p, None);
    }
    let mut goal = None;
    for _ in 0..AQUEDUCT_TILES {
        goal = layer.iter().copied().find(|&p| waters(p));
        if goal.is_some() {
            break;
        }
        let mut next: Vec<Pos> = Vec::new();
        for &p in &layer {
            for q in steps(p) {
                if !parent.contains_key(&q) && free(q) {
                    parent.insert(q, Some(p));
                    next.push(q);
                }
            }
        }
        next.sort();
        next.dedup();
        if next.is_empty() {
            break;
        }
        layer = next;
    }
    let Some(mut at) = goal else { return };
    let mut line = vec![at];
    while let Some(Some(from)) = parent.get(&at) {
        line.push(*from);
        at = *from;
    }
    line.reverse();
    // The stone for every tile, so the line is never left half paid for.
    let cost: Vec<crate::building::CostItem> = def
        .cost
        .iter()
        .map(|c| crate::building::CostItem { item: c.item, count: c.count * line.len() as u32 })
        .collect();
    if !have_enough(world, &cost) {
        return;
    }
    for pos in line {
        let _ = world.apply(Command::DesignateBuild { pos, building: id }, defs);
    }
}

/// Lays one Sky Lift pair across a range (module doc).
pub(super) fn raise_lift(world: &mut World, defs: &Defs, anchor: Pos) {
    let Some(id) = defs.building_id("sky_lift") else { return };
    let def = defs.building(id);
    let Some(pair) = def.pair else { return };
    if !known(world, defs, "sky_lifts") || building_count(world, id) > 0 || !search_minute(world) {
        return;
    }
    let both: Vec<_> = def
        .cost
        .iter()
        .map(|c| crate::building::CostItem { item: c.item, count: c.count * 2 })
        .collect();
    if !have_enough(world, &both) {
        return;
    }
    let zones: Vec<Pos> = world.stockpile_zones().collect();
    let Some(&first) = zones.iter().min() else { return };
    let from_stores = world.flood_from_anchor(first, defs);
    // The deposit the opening quarries that walking reaches the slowest way round.
    let split = far_deposits(world, defs, anchor).into_iter().find(|f| {
        let line = zones.iter().map(|z| z.chebyshev(f.stand)).min().unwrap_or(0);
        let walk = from_stores.cost(f.stand).map_or(f64::INFINITY, f64::from);
        line >= pair.min_tiles as i32 && walk >= LIFT_DETOUR * f64::from(line.max(1) * 10)
    });
    let Some(work) = split else { return };
    let edge = zones.iter().copied().min_by_key(|z| (z.chebyshev(work.stand), *z)).unwrap_or(first);
    let (w, h) = def.footprint;
    let ends = |near: Pos| -> Vec<Pos> {
        (-LIFT_SEARCH..=LIFT_SEARCH)
            .flat_map(|dy| {
                (-LIFT_SEARCH..=LIFT_SEARCH).map(move |dx| Pos::new(near.x + dx, near.y + dy))
            })
            .filter(|&p| {
                world.check_build(p, id, defs).is_ok() && world.to_clear(p, id, defs).is_empty()
            })
            .filter(|&p| from_stores.cost(Footprint::new(p, w, h).entrance()).is_some())
            .collect()
    };
    let (near_ends, far_ends) = (ends(edge), ends(work.stand));
    let cost_of =
        |p: Pos| i64::from(from_stores.cost(Footprint::new(p, w, h).entrance()).unwrap_or(0));
    let mut best: Option<(i64, Pos, Pos)> = None;
    for &a in &near_ends {
        for &b in &far_ends {
            let apart = a.chebyshev(b);
            if apart < pair.min_tiles as i32 || apart > pair.reach as i32 {
                continue;
            }
            let saving = cost_of(b) - cost_of(a) - 10 * i64::from(apart);
            if saving >= LIFT_SAVING && best.is_none_or(|(s, ..)| saving > s) {
                best = Some((saving, a, b));
            }
        }
    }
    let Some((_, a, b)) = best else { return };
    for pos in [a, b] {
        let _ = world.apply(Command::DesignateBuild { pos, building: id }, defs);
    }
    let _ = world.apply(Command::SetLink { pos: a, to: b }, defs);
}

/// Raises one Steam Engine on the bank (module doc).
pub(super) fn raise_bank_engine(world: &mut World, defs: &Defs, anchor: Pos) {
    let Some(id) = defs.building_id("steam_engine") else { return };
    let def = defs.building(id);
    let Some((tech, bank)) = def.bank_upgrade.as_ref() else { return };
    if !world.research().is_known(*tech) || !search_minute(world) || !have_enough(world, &def.cost)
    {
        return;
    }
    let (w, h) = def.footprint;
    let on_bank = |f: Footprint| {
        f.ring().filter_map(|p| world.map.get(p)).any(|t| bank.beside.contains(&t.terrain))
    };
    let standing = world
        .buildings()
        .filter(|b| b.id == id)
        .map(|b| b.footprint())
        .chain(world.build_sites().filter(|&(_, b, _)| b == id).map(|(f, _, _)| f))
        .any(on_bank);
    if standing {
        return;
    }
    let mut wet: Vec<Pos> = world
        .map
        .iter()
        .filter(|(_, t)| bank.beside.contains(&t.terrain))
        .map(|(p, _)| p)
        .filter(|p| p.chebyshev(anchor) <= DEPOSIT_RADIUS)
        .collect();
    wet.sort_by_key(|&p| (p.chebyshev(anchor), p));
    let before = world.flood_from_anchor(anchor, defs);
    for water in wet.into_iter().take(BANK_TRIES) {
        let site = (0..=BANK_SEARCH).find_map(|r| {
            let mut ring: Vec<Pos> = (-r..=r)
                .flat_map(|dy| (-r..=r).map(move |dx| Pos::new(water.x + dx, water.y + dy)))
                .filter(|&p| p.chebyshev(water) == r)
                .filter(|&p| {
                    world.check_build(p, id, defs).is_ok()
                        && world.to_clear(p, id, defs).is_empty()
                        && on_bank(Footprint::new(p, w, h))
                        && world.build_site_viable(&before, Footprint::new(p, w, h), defs)
                })
                .collect();
            ring.sort();
            ring.into_iter().next()
        });
        if let Some(pos) = site {
            let _ = world.apply(Command::DesignateBuild { pos, building: id }, defs);
            return;
        }
    }
}

/// A voyage `plan_voyage` weighs: its rank (see there), the nation, the deal and the times.
type Voyage = ((bool, u32, usize, usize), Nation, u8, u8);

/// Plans a voyage from every docked, idle Airship Dock (module doc).
pub(super) fn plan_voyage(world: &mut World, defs: &Defs) {
    let Some(dock) = defs.building_id("airship_dock") else { return };
    let Some(air) = defs.building(dock).airship else { return };
    let docks: Vec<Pos> = world
        .buildings()
        .filter(|b| b.id == dock)
        .map(|b| b.anchor)
        .filter(|&p| world.voyage_plan(p).is_none() && world.voyage_away(p).is_none())
        .collect();
    if docks.is_empty() {
        return;
    }
    let united = world.united();
    let wanted_items: BTreeSet<ItemId> = deposit_items(defs).into_iter().collect();
    // (does not hold what the gate wanted, how much it holds of the first thing it gets,
    // nation, deal, times)
    let mut best: Option<Voyage> = None;
    for (n, &nation) in Nation::ALL.iter().enumerate() {
        let Some(caravan) = defs.trade.caravan(nation, united) else { continue };
        for (d, deal) in caravan.deals.iter().enumerate() {
            let per: u32 = deal.give.iter().map(|g| g.count).sum();
            if per == 0 || deal.get.is_empty() && deal.knowledge == 0 {
                continue;
            }
            let most = (air.hold / per).min(deal.times);
            let Some(times) =
                (1..=most).rev().find(|&t| pacing::can_spare(world, defs, &deal.give, t))
            else {
                continue;
            };
            let first = deal.get.first().map(|&(i, _)| i);
            let gets_wanted = first.is_some_and(|i| wanted_items.contains(&i));
            let held = first.map_or(u32::MAX, |i| world.stored_total(i));
            let key = (!gets_wanted, held, n, d);
            if best.is_none_or(|(k, ..)| key < k) {
                best = Some((key, nation, d as u8, times as u8));
            }
        }
    }
    let Some((_, nation, deal, times)) = best else { return };
    let mut deals = [(0_u8, 0_u8); crate::building::MAX_VOYAGE_DEALS];
    deals[0] = (deal, times);
    for pos in docks {
        let _ = world.apply(Command::PlanVoyage { dock: pos, nation, deals }, defs);
    }
}

/// The ability Call Rain is, once cloudbending is known (so the generic bend rotation leaves it
/// to `call_rain`).
pub(super) fn rain_ability(world: &World, defs: &Defs) -> Option<AbilityId> {
    if !known(world, defs, "cloudbending") {
        return None;
    }
    defs.ability_id("call_rain")
}

/// Calls rain on the biggest patch of plots, once each spring (module doc).
pub(super) fn call_rain(world: &mut World, defs: &Defs) {
    let Some(ability) = rain_ability(world, defs) else { return };
    if world.season_index(defs) != 0
        || world.rains().next().is_some()
        || world.bend_designations().any(|(a, _)| a == ability)
        || world.check_ability(ability, defs).is_err()
    {
        return;
    }
    let radius = match &defs.ability(ability).effect {
        crate::bending::Effect::Rain { radius, .. } => *radius,
        _ => RAIN_DEFAULT_RADIUS,
    };
    let Some(plot) = defs.building_id("farm_plot") else { return };
    let plots: Vec<Pos> = world.buildings().filter(|b| b.id == plot).map(|b| b.anchor).collect();
    let center = plots
        .iter()
        .map(|&c| (plots.iter().filter(|p| p.chebyshev(c) <= radius as i32).count(), c))
        .min_by_key(|&(n, c)| (std::cmp::Reverse(n), c));
    if let Some((_, pos)) = center {
        let _ = world.apply(Command::Bend { ability, pos }, defs);
    }
}

/// Stores of wood below which the opening cuts the far trees too (`gather_far_wood`).
const WOOD_WANTED: u32 = 100;

/// How many far trees one step designates.
const FAR_TREES: usize = 12;

/// Designates the nearest trees past `FORAGE_RADIUS` (within `DEPOSIT_RADIUS`) while wood is
/// short: the trees near the stockpile are gone by the time the gate stands, and a bed costs wood,
/// so a city that can't cut more stops growing. The one-shot orders repeat harmlessly until cut.
fn gather_far_wood(world: &mut World, defs: &Defs, anchor: Pos) {
    let Some(wood) = defs.item_id("wood") else { return };
    let have = world.stored_total(wood)
        + world.ground_items().filter(|(_, s)| s.item == wood).map(|(_, s)| s.count).sum::<u32>();
    if have >= WOOD_WANTED {
        return;
    }
    let Some(start) = world.stockpile_zones().min_by_key(|&p| (p.chebyshev(anchor), p)) else {
        return;
    };
    let mut far: Vec<Pos> = super::nearby(anchor, DEPOSIT_RADIUS, world, defs, |t, defs| {
        super::yields_any(t, defs, &[wood])
    })
    .into_iter()
    .filter(|p| p.chebyshev(anchor) > super::FORAGE_RADIUS)
    .collect();
    let flood = world.flood_from_anchor(start, defs);
    far.retain(|p| {
        (-1..=1).any(|dy| (-1..=1).any(|dx| flood.cost(Pos::new(p.x + dx, p.y + dy)).is_some()))
    });
    far.sort_by_key(|&p| (p.chebyshev(anchor), p));
    for pos in far.into_iter().take(FAR_TREES) {
        let _ = world.apply(Command::DesignateHarvest { pos }, defs);
    }
}

/// Everything the United City adds to a step, after the sweep (module doc).
pub(super) fn step(world: &mut World, defs: &Defs, anchor: Pos) {
    if !active(world) {
        return;
    }
    gather_far_wood(world, defs, anchor);
    raise_aqueduct(world, defs);
    raise_lift(world, defs, anchor);
    raise_bank_engine(world, defs, anchor);
    plan_voyage(world, defs);
    call_rain(world, defs);
}

#[cfg(test)]
mod tests;
