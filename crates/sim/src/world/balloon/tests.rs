use super::*;
use crate::defs::{BiomeId, Terrain};
use crate::geom::{Footprint, Grid};
use crate::jobs::JobKind;
use crate::mapgen::Tile;
use crate::world::fuel::Bunker;

/// Two plains 20 tiles apart with a sea wall between them that ends only far to the south: the
/// docks are in one region, but the walk round is long.
fn walled() -> TileMap {
    Grid::from_fn(80, 60, |p| Tile {
        elevation: 100,
        temperature: 128,
        moisture: 128,
        terrain: if p.x == 30 && p.y < 56 { Terrain::Water } else { Terrain::Land },
        biome: BiomeId(0),
        resource: None,
        building: None,
    })
}

fn at(x: i32, y: i32) -> Pos {
    Pos::new(x, y)
}

const A: Pos = Pos::new(20, 5);
const B: Pos = Pos::new(40, 5);

/// A pair of docks across the wall, linked, each with `fuel` wood, and a hauler at (17, 6).
fn setup(fuel: u32) -> (World, Defs) {
    setup_with(fuel, at(17, 6))
}

fn setup_with(fuel: u32, unit: Pos) -> (World, Defs) {
    let mut defs = Defs::bundled();
    defs.needs.hunger.decay_per_minute = 0.0;
    defs.needs.thirst.decay_per_minute = 0.0;
    defs.needs.energy.decay_per_minute = 0.0;
    let dock = defs.building_id("balloon_dock").unwrap();
    let mut w = World::with_map(1, walled(), Nation::Fire, &[unit], &defs);
    for p in [A, B] {
        w.put_building(Footprint::new(p, 2, 2), dock, &defs);
        w.anchored_mut(p).unwrap().fuel = Some(Bunker { count: fuel, burned: 0, item: None });
    }
    w.apply(Command::SetLink { pos: A, to: B }, &defs).unwrap();
    (w, defs)
}

fn run_until(w: &mut World, defs: &Defs, limit: u64, mut done: impl FnMut(&World) -> bool) {
    for _ in 0..limit {
        if done(w) {
            return;
        }
        w.tick(defs);
    }
    panic!("not done after {limit} ticks");
}

fn fuel(w: &World, dock: Pos) -> u32 {
    w.anchored(dock).and_then(|b| b.fuel).expect("a bunker").count
}

fn hold_at(w: &mut World, dock: Pos, stack: Stack, at: u64) {
    w.anchored_mut(dock).unwrap().dock.get_or_insert_default().hold.push(Held { stack, at });
}

fn held(w: &World, dock: Pos, defs: &Defs) -> u32 {
    w.dock_hold(dock, defs).unwrap().0
}

fn ground_total(w: &World, item: ItemId) -> u32 {
    w.ground.values().filter(|s| s.item == item).map(|s| s.count).sum()
}

fn has_balloon(w: &World, dock: Pos) -> bool {
    w.anchored(dock).unwrap().dock.as_ref().unwrap().balloon.is_some()
}

#[test]
fn the_balloon_dock_is_fires_and_parses() {
    let defs = Defs::bundled();
    let def = defs.building(defs.building_id("balloon_dock").unwrap());
    let tech = defs.tech(defs.tech_id("cargo_balloons").unwrap());
    assert_eq!(def.unlocked_by, defs.tech_id("cargo_balloons"));
    assert_eq!(tech.nation, Some(Nation::Fire));
    assert!(!tech.core, "an optional horizons tech");
    assert_eq!(tech.cost, 500);
    let balloon = def.balloon.expect("a balloon");
    assert_eq!((balloon.load, balloon.hold, balloon.fuel_per_flight), (20, 40, 2));
    assert_eq!(balloon.wait_ticks, 600);
    let fuel = def.fuel.as_ref().expect("a bunker");
    assert_eq!((fuel.bunker, fuel.per_minute, fuel.micros_per_tick), (10, None, 0));
    assert_eq!(def.pair, Some(crate::building::PairDef { reach: 64, min_tiles: 16 }));
}

#[test]
fn goods_cross_a_range_by_balloon_and_arrive_as_hauls() {
    let (mut w, defs) = setup(10);
    let stone = defs.item_id("stone").unwrap();
    w.designate_stockpile(at(45, 6)).unwrap();
    w.ground.insert(at(17, 7), Stack { item: stone, count: 4 });
    w.ensure_haul_job(at(17, 7));
    // The walk round the wall is far longer than the dock, the flight and the short walk after.
    run_until(&mut w, &defs, 600, |w| w.units[0].job_task == Some(Task::Balloon(A)));
    run_until(&mut w, &defs, 600, |w| held(w, A, &defs) == 4);
    assert!(w.units[0].carrying.is_none());
    // It waits for the first item's wait, then flies (2 wood from A), then lands beside B's door.
    run_until(&mut w, &defs, 1500, |w| held(w, A, &defs) == 0);
    assert_eq!(fuel(&w, A), 8);
    assert_eq!(fuel(&w, B), 10);
    assert!(matches!(w.balloon_status(B, &defs), Some(BalloonStatus::Landing { .. })));
    assert_eq!(w.balloon_status(A, &defs), Some(BalloonStatus::Away));
    run_until(&mut w, &defs, 600, |w| ground_total(w, stone) == 4);
    let (&spilled, _) = w.ground.iter().find(|(_, s)| s.item == stone).expect("landed");
    assert!(spilled.chebyshev(B) <= 3, "beside the far dock: {spilled:?}");
    assert!(w.jobs.iter().any(|j| j.kind == JobKind::Haul { pos: spilled }), "an ordinary haul");
    assert_eq!(w.balloon_status(B, &defs), Some(BalloonStatus::Parked));
    run_until(&mut w, &defs, 3000, |w| w.stored_total(stone) == 4);
    assert_eq!(fuel(&w, A) + fuel(&w, B), 18, "one flight, one launch burn");
}

#[test]
fn a_hauler_walks_when_stores_are_nearer_than_the_dock_route() {
    let (mut w, defs) = setup(10);
    let stone = defs.item_id("stone").unwrap();
    w.designate_stockpile(at(15, 6)).unwrap();
    w.ground.insert(at(17, 7), Stack { item: stone, count: 4 });
    w.ensure_haul_job(at(17, 7));
    for _ in 0..600 {
        assert!(!matches!(w.units[0].job_task, Some(Task::Balloon(_))));
        w.tick(&defs);
    }
    assert_eq!(w.stored_total(stone), 4);
}

#[test]
fn a_refuel_never_uses_a_dock() {
    let (mut w, defs) = setup(10);
    let wood = defs.item_id("wood").unwrap();
    w.designate_stockpile(at(45, 6)).unwrap();
    w.stored.insert(at(45, 6), Stack { item: wood, count: 10 });
    // A's bunker is low: the refuel haul fetches from the far side of the wall, and carries it
    // back on foot rather than through the dock.
    w.anchored_mut(A).unwrap().fuel = Some(Bunker::default());
    run_until(&mut w, &defs, 8000, |w| {
        assert!(!matches!(w.units[0].job_task, Some(Task::Balloon(_))), "a refuel walks");
        fuel(w, A) > 0
    });
}

#[test]
fn a_part_load_waits_the_wait_and_a_full_load_goes_at_once() {
    let (mut w, defs) = setup(10);
    let stone = defs.item_id("stone").unwrap();
    w.units.clear();
    hold_at(&mut w, A, Stack { item: stone, count: 3 }, 0);
    for _ in 0..590 {
        w.tick(&defs);
    }
    assert_eq!(held(&w, A, &defs), 3, "not yet: 3 items, waited 59 s");
    run_until(&mut w, &defs, 40, |w| held(w, A, &defs) == 0);
    assert_eq!(fuel(&w, A), 8);
    // 25 items in the hold: 20 fly now and 5 wait.
    let (mut w, defs) = setup(10);
    w.units.clear();
    hold_at(&mut w, A, Stack { item: stone, count: 15 }, 0);
    hold_at(&mut w, A, Stack { item: stone, count: 10 }, 0);
    w.tick(&defs);
    assert_eq!(held(&w, A, &defs), 5);
    assert_eq!(fuel(&w, A), 8);
}

#[test]
fn fuel_is_counted_per_flight_empty_flights_too() {
    let (mut w, defs) = setup(10);
    let stone = defs.item_id("stone").unwrap();
    w.units.clear();
    hold_at(&mut w, A, Stack { item: stone, count: 20 }, 0);
    w.tick(&defs);
    assert_eq!((fuel(&w, A), fuel(&w, B)), (8, 10), "loaded flight, burned at A");
    // Landed at B (20 tiles at 1.5 a second is 14 s). Now the goods are at A: B's balloon flies
    // there empty, burning B's wood, then A launches them loaded.
    run_until(&mut w, &defs, 300, |w| w.balloon_status(B, &defs) == Some(BalloonStatus::Parked));
    hold_at(&mut w, A, Stack { item: stone, count: 20 }, 0);
    w.tick(&defs);
    assert_eq!((fuel(&w, A), fuel(&w, B)), (8, 8), "the empty flight burned B's");
    assert!(matches!(w.balloon_status(A, &defs), Some(BalloonStatus::Landing { .. })));
    run_until(&mut w, &defs, 300, |w| held(w, A, &defs) == 0);
    assert_eq!((fuel(&w, A), fuel(&w, B)), (6, 8), "then the loaded flight, burned at A");
}

#[test]
fn no_fuel_no_launch_and_the_status_says_so() {
    let (mut w, defs) = setup(1);
    let stone = defs.item_id("stone").unwrap();
    let wood = defs.item_id("wood").unwrap();
    w.units.clear();
    hold_at(&mut w, A, Stack { item: stone, count: 20 }, 0);
    for _ in 0..50 {
        w.tick(&defs);
    }
    assert_eq!(held(&w, A, &defs), 20, "1 wood is short of a 2 wood flight");
    assert_eq!(fuel(&w, A), 1);
    assert_eq!(w.balloon_status(A, &defs), Some(BalloonStatus::OutOfFuel(wood)));
    w.anchored_mut(A).unwrap().fuel = Some(Bunker { count: 2, burned: 0, item: None });
    w.tick(&defs);
    assert_eq!(held(&w, A, &defs), 0);
    assert_eq!(fuel(&w, A), 0);
    // The bunker has no per-minute burn: nothing burns while the balloon idles.
    for _ in 0..600 {
        w.tick(&defs);
    }
    assert_eq!(fuel(&w, B), 1);
}

#[test]
fn a_full_hold_sends_haulers_walking() {
    let (mut w, defs) = setup(0);
    let stone = defs.item_id("stone").unwrap();
    // No fuel, so the hold stays put: 40 of 40.
    hold_at(&mut w, A, Stack { item: stone, count: 40 }, 0);
    assert_eq!(w.dock_hold(A, &defs), Some((40, 40)));
    w.designate_stockpile(at(45, 6)).unwrap();
    w.ground.insert(at(17, 7), Stack { item: stone, count: 4 });
    w.ensure_haul_job(at(17, 7));
    for _ in 0..600 {
        assert!(!matches!(w.units[0].job_task, Some(Task::Balloon(_))), "it never heads to it");
        w.tick(&defs);
    }
    assert_eq!(held(&w, A, &defs), 40);
    // It walks all the way round the wall.
    run_until(&mut w, &defs, 6000, |w| w.stored_total(stone) == 4);
}

#[test]
fn a_unit_that_finds_the_dock_full_on_arrival_walks_on() {
    let (mut w, defs) = setup(0);
    let stone = defs.item_id("stone").unwrap();
    w.designate_stockpile(at(45, 6)).unwrap();
    w.ground.insert(at(17, 7), Stack { item: stone, count: 4 });
    w.ensure_haul_job(at(17, 7));
    run_until(&mut w, &defs, 600, |w| w.units[0].job_task == Some(Task::Balloon(A)));
    // The hold fills while the unit is on its way.
    let now = w.tick;
    hold_at(&mut w, A, Stack { item: stone, count: 40 }, now);
    run_until(&mut w, &defs, 600, |w| w.units[0].job_task == Some(Task::Deliver));
    assert_eq!(held(&w, A, &defs), 40);
    assert_eq!(w.units[0].carrying, Some(Stack { item: stone, count: 4 }));
}

#[test]
fn taking_a_dock_down_returns_its_hold_cargo_and_bunker() {
    let (mut w, defs) = setup(6);
    let stone = defs.item_id("stone").unwrap();
    let wood = defs.item_id("wood").unwrap();
    hold_at(&mut w, B, Stack { item: stone, count: 7 }, 0);
    w.anchored_mut(B).unwrap().dock.as_mut().unwrap().balloon = Some(Balloon {
        flight: Some(Flight { left: 100_000, cargo: vec![Stack { item: stone, count: 3 }] }),
    });
    w.units.clear();
    let key = w.map[B].building.unwrap().key();
    let record = w.remove_building(key, &defs).unwrap();
    assert_eq!(
        record.dock_goods(),
        [Stack { item: stone, count: 7 }, Stack { item: stone, count: 3 }]
    );
    assert_eq!(w.link_at(A), None, "the partner has no partner");
    // Through the real task: the deconstruct drops the hold and bunker by the anchor.
    let (mut w, defs) = setup_with(6, at(41, 8));
    hold_at(&mut w, B, Stack { item: stone, count: 7 }, 0);
    w.apply(Command::Deconstruct { pos: B }, &defs).unwrap();
    run_until(&mut w, &defs, 4000, |w| w.anchored(B).is_none());
    assert_eq!(ground_total(&w, stone), 7);
    assert_eq!(ground_total(&w, wood), 6 + defs.deconstruct.refund_of(10));
}

#[test]
fn a_balloon_in_the_air_lands_its_cargo_even_if_the_pair_is_broken() {
    let (mut w, defs) = setup(10);
    let stone = defs.item_id("stone").unwrap();
    w.units.clear();
    hold_at(&mut w, A, Stack { item: stone, count: 20 }, 0);
    w.tick(&defs);
    // The launching dock goes down mid-flight.
    let key = w.map[A].building.unwrap().key();
    w.remove_building(key, &defs).unwrap();
    assert_eq!(w.link_at(B), None);
    run_until(&mut w, &defs, 400, |w| ground_total(w, stone) == 20);
}

#[test]
fn one_balloon_per_pair_and_it_starts_at_the_first_dock_finished() {
    let (mut w, defs) = setup(10);
    w.tick(&defs);
    assert!(has_balloon(&w, A) && !has_balloon(&w, B), "A was raised first");
    for _ in 0..100 {
        w.tick(&defs);
    }
    assert_eq!([A, B].iter().filter(|&&p| has_balloon(&w, p)).count(), 1);
}

#[test]
fn hold_and_balloon_survive_a_save_round_trip() {
    let (mut w, defs) = setup(10);
    let stone = defs.item_id("stone").unwrap();
    w.units.clear();
    hold_at(&mut w, A, Stack { item: stone, count: 20 }, 0);
    hold_at(&mut w, A, Stack { item: stone, count: 5 }, 3);
    w.tick(&defs);
    let w2 = World::load(&w.save(&defs).expect("saves"), &defs).expect("loads");
    assert_eq!(w2.dock_hold(A, &defs), Some((5, 40)));
    assert_eq!(w2.balloon_status(B, &defs), w.balloon_status(B, &defs));
    assert_eq!(w2.save(&defs).unwrap(), w.save(&defs).unwrap());
}

#[test]
fn flying_goods_is_deterministic() {
    let run = || {
        let (mut w, defs) = setup(10);
        let stone = defs.item_id("stone").unwrap();
        w.designate_stockpile(at(45, 6)).unwrap();
        for y in 7..10 {
            w.ground.insert(at(17, y), Stack { item: stone, count: 6 });
            w.ensure_haul_job(at(17, y));
        }
        for _ in 0..2500 {
            w.tick(&defs);
        }
        w.save(&defs).expect("saves")
    };
    assert_eq!(run(), run());
}
