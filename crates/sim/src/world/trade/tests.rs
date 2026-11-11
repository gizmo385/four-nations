//! Caravans (#437): the schedule, turn order, deals, cancelling, leaving and requests.

use super::*;
use crate::defs::{BiomeId, Terrain};
use crate::geom::{Footprint, Grid};
use crate::mapgen::Tile;

fn at(x: i32, y: i32) -> Pos {
    Pos::new(x, y)
}

const TRADE: &str = include_str!("../../../../../data/trade.toml");

/// The bundled data with a quick schedule: a caravan every 10 minutes, 0.2 into each, staying
/// 8, so tests needn't run whole seasons. `opens` is dropped unless `keep_opens`, and `order`
/// replaced when given.
fn defs_with(keep_opens: bool, order: Option<&str>) -> Defs {
    let mut src = TRADE
        .replacen("interval_minutes = 15.0", "interval_minutes = 10.0", 1)
        .replacen("offset_minutes = 4.0", "offset_minutes = 0.2", 1)
        .replacen("stay_minutes = 9.0", "stay_minutes = 8.0", 1);
    if !keep_opens {
        src = src.replacen("opens = \"known_on_the_road\"", "", 1);
    }
    if let Some(order) = order {
        src = src.replacen("order = [\"earth\", \"fire\", \"water\", \"air\"]", order, 1);
    }
    Defs::bundled().with_trade(&src).expect("test trade data is valid")
}

fn minutes(m: f32) -> u64 {
    (m * 60.0 * TICKS_PER_SECOND as f32).round() as u64
}

/// A 40x40 field of land with `units` Earth settlers around (20, 20), and stores of food, wood
/// and stone beside them.
fn world(seed: u64, units: i32, defs: &Defs) -> World {
    let field = Grid::from_fn(40, 40, |_| Tile {
        elevation: 100,
        temperature: 128,
        moisture: 128,
        terrain: Terrain::Land,
        biome: BiomeId(0),
        resource: None,
        building: None,
    });
    let spawns: Vec<Pos> = (0..units).map(|n| at(19 + n, 20)).collect();
    let mut w = World::with_map(seed, field, Nation::Earth, &spawns, defs);
    for (x, item) in [(18, "fish"), (19, "fish"), (20, "wood"), (21, "wood"), (22, "stone")] {
        let pos = at(x, 22);
        w.designate_stockpile(pos).unwrap();
        w.stored.insert(pos, Stack { item: defs.item_id(item).unwrap(), count: 200 });
    }
    w
}

fn run(w: &mut World, defs: &Defs, ticks: u64) {
    for _ in 0..ticks {
        w.tick(defs);
    }
}

fn run_until(w: &mut World, defs: &Defs, limit: u64, done: impl Fn(&World) -> bool) {
    for _ in 0..limit {
        if done(w) {
            return;
        }
        w.tick(defs);
    }
    panic!("not done after {limit} ticks");
}

fn kinds(w: &World) -> Vec<CaravanEventKind> {
    w.caravan_events().map(|e| e.kind.clone()).collect()
}

#[test]
fn the_bundled_caravans_load() {
    let defs = Defs::bundled();
    let s = defs.trade.schedule.as_ref().expect("bundled");
    assert_eq!(s.opens.as_deref(), Some("known_on_the_road"));
    assert_eq!(defs.trade.caravans.len(), 4);
    for c in &defs.trade.caravans {
        assert!(c.deals.len() >= 3, "{:?}", c.nation);
        assert!(!c.requests.is_empty(), "{:?}", c.nation);
    }
    let visitors: Vec<Nation> = s.visitors(Nation::Earth).collect();
    assert_eq!(visitors, [Nation::Fire, Nation::Water, Nation::Air]);
}

#[test]
fn caravans_arrive_on_schedule_and_take_turns() {
    let defs = defs_with(false, None);
    let mut w = world(1, 1, &defs);
    let slot = |k: u64| k * minutes(10.0) + minutes(0.2);
    // Just past the third arrival: the first two have come and gone.
    run(&mut w, &defs, slot(2) + 1);
    let arrivals: Vec<(u64, Nation)> = w
        .caravan_events()
        .filter_map(|e| match e.kind {
            CaravanEventKind::Arrived { nation, .. } => Some((e.tick, nation)),
            _ => None,
        })
        .collect();
    assert_eq!(
        arrivals,
        [(slot(0), Nation::Fire), (slot(1), Nation::Water), (slot(2), Nation::Air)],
        "Earth's own traders never come"
    );
    let left: Vec<u64> = w
        .caravan_events()
        .filter(|e| matches!(e.kind, CaravanEventKind::Left { .. }))
        .map(|e| e.tick)
        .collect();
    assert_eq!(left, [slot(0) + minutes(8.0), slot(1) + minutes(8.0)]);
    let v = w.caravan().expect("the third is still here");
    assert_eq!(v.offers.len(), 4, "three deals and the request");
    assert!(matches!(v.offers[3].kind, OfferKind::Request(_)));
}

#[test]
fn the_camp_is_a_free_2x2_spot_in_the_distance_band() {
    let defs = defs_with(false, None);
    let mut w = world(1, 1, &defs);
    w.units[0].idle_until = u64::MAX;
    run(&mut w, &defs, minutes(0.2) + 1);
    let camp = w.caravan().expect("arrived").camp;
    let flood = Flood::new(&w.map, w.settlement_anchor(), &w.rules);
    let cost = flood.cost(camp).unwrap();
    assert!((80..=140).contains(&cost), "{camp:?} costs {cost}");
    // Row-major: nothing earlier on the map was in the band.
    let earlier = w.map.positions().take_while(|&p| p != camp);
    assert!(
        earlier.filter(|&p| flood.cost(p).is_some_and(|c| (80..=140).contains(&c))).count() == 0
    );
}

#[test]
fn with_nowhere_to_camp_the_visit_is_skipped_and_the_turn_passes() {
    let defs = defs_with(false, None);
    let mut w = world(1, 1, &defs);
    // Wall the settlers in: nothing beyond 3 tiles can be reached.
    let wall = defs.building_id("earth_wall").unwrap();
    for p in w.map.positions().collect::<Vec<_>>() {
        let d = (p.x - 20).abs().max((p.y - 20).abs());
        if d == 4 {
            w.put_building(Footprint::single(p), wall, &defs);
        }
    }
    run(&mut w, &defs, minutes(10.2) + 1);
    assert_eq!(
        kinds(&w),
        [
            CaravanEventKind::Skipped { nation: Nation::Fire },
            CaravanEventKind::Skipped { nation: Nation::Water }
        ]
    );
    assert!(w.caravan().is_none());
}

#[test]
fn no_caravan_comes_until_the_chapter_is_met_then_on_the_next_slot() {
    let defs = defs_with(true, None);
    let mut w = world(1, 1, &defs);
    run(&mut w, &defs, minutes(20.5));
    assert!(w.caravan_events().next().is_none(), "the road isn't known yet");

    // Meet "Known on the road" (ten people, four kinds of building, 60 food) only after slot 0
    // has passed: the fourth kind goes up a minute in.
    let mut w = world(1, 10, &defs);
    let raise = |w: &mut World, b: &str, x: i32| {
        let id = defs.building_id(b).unwrap();
        w.put_building(Footprint::single(at(x, 26)), id, &defs);
    };
    for (b, x) in [("bed", 10), ("campfire", 12), ("herb_garden", 14)] {
        raise(&mut w, b, x);
    }
    run(&mut w, &defs, minutes(1.0));
    assert!(w.chapter_met("known_on_the_road").is_none());
    raise(&mut w, "farm_plot", 16);
    run(&mut w, &defs, minutes(0.1));
    let met = w.chapter_met("known_on_the_road").expect("met at the next check");
    assert!(met > minutes(0.2), "after slot 0");
    assert!(w.caravan_events().next().is_none(), "slot 0 came before the chapter");
    run(&mut w, &defs, minutes(10.0));
    let first = w.caravan_events().next().expect("the next slot");
    assert_eq!(first.tick, minutes(10.0) + minutes(0.2));
}

/// The first slot of the visit whose deal gets `item`.
fn deal_getting(w: &World, defs: &Defs, item: &str) -> Option<u8> {
    let item = defs.item_id(item).unwrap();
    let caravan = w.caravan_def(defs)?;
    w.caravan()?
        .offers
        .iter()
        .position(|o| match o.kind {
            OfferKind::Deal(d) => caravan.deals[usize::from(d)].get.iter().any(|&(i, _)| i == item),
            OfferKind::Request(_) => false,
        })
        .map(|n| n as u8)
}

/// A Fire visit (Earth's first), whose deals all trade food, wood or stone, which the stores
/// hold plenty of.
fn fire_visit(seed: u64, units: i32) -> (World, Defs) {
    let defs = defs_with(false, None);
    let mut w = world(seed, units, &defs);
    run(&mut w, &defs, minutes(0.2) + 1);
    assert_eq!(w.caravan().unwrap().nation, Nation::Fire);
    (w, defs)
}

#[test]
fn a_taken_deal_is_hauled_and_its_goods_appear_at_the_camp() {
    let (mut w, defs) = fire_visit(1, 3);
    let slot = 0u8;
    let caravan = w.caravan_def(&defs).unwrap().clone();
    let OfferKind::Deal(d) = w.caravan().unwrap().offers[0].kind else { panic!("a deal") };
    let deal = caravan.deals[usize::from(d)].clone();
    assert_eq!(w.apply(Command::TakeDeal { slot }, &defs), Ok(()));
    assert!(w.jobs.iter().any(|j| matches!(j.kind, JobKind::Trade { slot: 0, .. })));
    run_until(&mut w, &defs, minutes(7.0), |w| {
        w.caravan_events().any(|e| matches!(e.kind, CaravanEventKind::DealDone { .. }))
    });
    let camp = w.caravan().unwrap().camp;
    let offer = &w.caravan().unwrap().offers[0];
    assert_eq!((offer.taken, offer.done), (1, 1));
    assert!(offer.held.is_empty(), "the goods were spent");
    for &(item, count) in &deal.get {
        let near: u32 = w
            .ground
            .iter()
            .filter(|(p, s)| s.item == item && p.chebyshev(camp) <= 3)
            .map(|(_, s)| s.count)
            .sum();
        assert!(near >= count, "{count} {item:?} by the camp, found {near}");
    }
    assert!(!w.jobs.iter().any(|j| matches!(j.kind, JobKind::Trade { .. })), "nothing owed");
    // Taking a slot past its times is refused.
    for _ in 1..deal.times {
        w.apply(Command::TakeDeal { slot }, &defs).unwrap();
    }
    assert_eq!(
        w.apply(Command::TakeDeal { slot }, &defs),
        Err(CommandError::Trade(TradeError::AllTaken))
    );
}

#[test]
fn a_cancelled_deal_leaves_what_was_handed_over_at_the_camp() {
    let (mut w, defs) = fire_visit(1, 1);
    // The wood-for-bricks deal needs 40 wood: more than one trip.
    let slot =
        deal_getting(&w, &defs, "brick").or_else(|| deal_getting(&w, &defs, "metal")).unwrap();
    w.apply(Command::TakeDeal { slot }, &defs).unwrap();
    run_until(&mut w, &defs, minutes(7.0), |w| {
        !w.caravan().unwrap().offers[usize::from(slot)].held.is_empty()
    });
    let held = w.caravan().unwrap().offers[usize::from(slot)].held.clone();
    let camp = w.caravan().unwrap().camp;
    assert_eq!(w.apply(Command::CancelDeal { slot }, &defs), Ok(()));
    let offer = &w.caravan().unwrap().offers[usize::from(slot)];
    assert_eq!((offer.taken, offer.done), (0, 0));
    assert!(offer.held.is_empty());
    for s in held {
        let near: u32 = w
            .ground
            .iter()
            .filter(|(p, g)| g.item == s.item && p.chebyshev(camp) <= 3)
            .map(|(_, g)| g.count)
            .sum();
        assert!(near >= s.count, "{s:?} back on the ground");
    }
    assert!(!w.jobs.iter().any(|j| matches!(j.kind, JobKind::Trade { .. })));
    assert_eq!(
        w.apply(Command::CancelDeal { slot }, &defs),
        Err(CommandError::Trade(TradeError::NotTaken))
    );
}

#[test]
fn goods_for_an_unfinished_deal_are_left_when_the_caravan_goes() {
    let (mut w, defs) = fire_visit(1, 1);
    let slot =
        deal_getting(&w, &defs, "brick").or_else(|| deal_getting(&w, &defs, "metal")).unwrap();
    // Taken three times over (or as many as it allows): more than one settler can bring in time.
    for _ in 0..3 {
        let _ = w.apply(Command::TakeDeal { slot }, &defs);
    }
    run_until(&mut w, &defs, minutes(7.0), |w| {
        !w.caravan().unwrap().offers[usize::from(slot)].held.is_empty()
    });
    let camp = w.caravan().unwrap().camp;
    run_until(&mut w, &defs, minutes(9.0), |w| w.caravan().is_none());
    assert!(matches!(kinds(&w).last(), Some(CaravanEventKind::Left { nation: Nation::Fire })));
    let left_at_camp: u32 =
        w.ground.iter().filter(|(p, _)| p.chebyshev(camp) <= 3).map(|(_, s)| s.count).sum();
    assert!(left_at_camp > 0, "what was handed over is on the ground");
    assert!(!w.jobs.iter().any(|j| matches!(j.kind, JobKind::Trade { .. })));
}

#[test]
fn a_request_grants_its_knowledge() {
    let defs = defs_with(false, Some("order = [\"air\", \"earth\", \"fire\", \"water\"]"));
    let mut w = world(1, 3, &defs);
    run(&mut w, &defs, minutes(0.2) + 1);
    let v = w.caravan().unwrap();
    assert_eq!(v.nation, Nation::Air);
    let slot = (v.offers.len() - 1) as u8;
    let caravan = w.caravan_def(&defs).unwrap();
    let OfferKind::Request(r) = v.offers[usize::from(slot)].kind else { panic!("the request") };
    assert_eq!(caravan.requests[usize::from(r)].reward, Reward::Knowledge(60));
    w.apply(Command::TakeDeal { slot }, &defs).unwrap();
    assert_eq!(
        w.apply(Command::TakeDeal { slot }, &defs),
        Err(CommandError::Trade(TradeError::AllTaken)),
        "a request is taken once"
    );
    let mut before = w.research().pool_millis();
    for _ in 0..minutes(7.0) {
        before = w.research().pool_millis();
        w.tick(&defs);
        if w.caravan_events().any(|e| matches!(e.kind, CaravanEventKind::RequestDone { .. })) {
            break;
        }
    }
    assert!(w.caravan_events().any(|e| matches!(e.kind, CaravanEventKind::RequestDone { .. })));
    assert!(w.research().pool_millis() >= before + 60_000, "60 research points");
}

#[test]
fn two_runs_from_one_seed_log_the_same_caravans() {
    let play = || {
        let (mut w, defs) = fire_visit(7, 2);
        w.apply(Command::TakeDeal { slot: 0 }, &defs).unwrap();
        w.apply(Command::TakeDeal { slot: 1 }, &defs).unwrap();
        run(&mut w, &defs, minutes(25.0));
        w
    };
    let (a, b) = (play(), play());
    assert!(a.caravan_events().count() >= 4);
    assert!(a.caravan_events().eq(b.caravan_events()));
    assert_eq!(a.caravan(), b.caravan());
}

#[test]
fn a_caravan_survives_a_save() {
    let (mut w, defs) = fire_visit(1, 1);
    w.apply(Command::TakeDeal { slot: 0 }, &defs).unwrap();
    run(&mut w, &defs, 200);
    let loaded = World::load(&w.save(&defs).unwrap(), &defs).unwrap();
    assert_eq!(loaded.caravan(), w.caravan());
    assert!(loaded.caravan_events().eq(w.caravan_events()));
}
