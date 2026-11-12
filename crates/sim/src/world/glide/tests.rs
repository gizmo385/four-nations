use super::*;
use crate::defs::{BiomeId, Terrain};
use crate::geom::{Footprint, Grid};
use crate::jobs::Stack;
use crate::mapgen::Tile;

const WIDTH: i32 = 100;
const HEIGHT: i32 = 40;

/// Gate A west of the lake, gate B east of it: 30 apart, so a hop takes 5 s (50 ticks).
const A: Pos = Pos::new(30, 5);
const B: Pos = Pos::new(60, 5);
/// Where the walker starts, west of the lake, and the tree it is sent to, east of it.
const START: Pos = Pos::new(28, 5);
const TREE: Pos = Pos::new(66, 5);

fn at(x: i32, y: i32) -> Pos {
    Pos::new(x, y)
}

fn flat() -> TileMap {
    Grid::from_fn(WIDTH, HEIGHT, |_| Tile {
        elevation: 100,
        temperature: 128,
        moisture: 128,
        terrain: Terrain::Land,
        biome: BiomeId(0),
        resource: None,
        building: None,
    })
}

/// A lake from the top of the map to y = 34 at x = 45..=49. The only way round is the south shore,
/// 70 tiles out of the way.
fn lake(map: &mut TileMap) {
    for y in 0..35 {
        for x in 45..50 {
            map[at(x, y)].terrain = Terrain::Water;
        }
    }
}

/// A world with the lake, a walker at `START` who stays put until given work, and, if `gates`,
/// a linked pair at `A` and `B`.
fn setup(gates: bool) -> (World, Defs) {
    let defs = Defs::bundled();
    let mut map = flat();
    lake(&mut map);
    let mut w = World::with_map(1, map, Nation::Air, &[START], &defs);
    w.units[0].idle_until = u64::MAX;
    if gates {
        let gate = defs.building_id("wind_gate").unwrap();
        for p in [A, B] {
            w.put_building(Footprint::single(p), gate, &defs);
        }
        w.apply(Command::SetLink { pos: A, to: B }, &defs).unwrap();
    }
    (w, defs)
}

/// Sends the walker to the tree.
fn send(w: &mut World, defs: &Defs) {
    w.map[TREE].resource = defs.resource_id("oak_tree");
    w.map_changed();
    w.designate_harvest(TREE, defs).unwrap();
}

/// Ticks until the walker is beside the tree, and says how many it took.
fn walk_there(w: &mut World, defs: &Defs) -> u64 {
    let start = w.tick;
    for _ in 0..4000 {
        if w.units[0].pos.chebyshev(TREE) <= 1 {
            return w.tick - start;
        }
        w.tick(defs);
    }
    panic!("never got there, at {:?}", w.units[0].pos);
}

#[test]
fn the_wind_gate_is_airs_and_glides_people() {
    let defs = Defs::bundled();
    let id = defs.building_id("wind_gate").unwrap();
    let def = defs.building(id);
    assert!(def.passable, "a unit steps onto it to launch");
    assert_eq!(def.glide.unwrap().ticks(30), 50);
    assert_eq!(def.pair.map(|p| (p.reach, p.min_tiles)), Some((48, 12)));
    let tech = defs.tech(def.unlocked_by.unwrap());
    assert_eq!((tech.id.as_str(), tech.cost), ("wind_gates", 250));
    assert_eq!(tech.nation, Some(Nation::Air));
    assert!(!tech.core, "the gate doesn't grow");
}

#[test]
fn crossing_a_lake_beats_walking_round_it() {
    let (mut w, defs) = setup(false);
    send(&mut w, &defs);
    let round = walk_there(&mut w, &defs);

    let (mut w, defs) = setup(true);
    send(&mut w, &defs);
    let glided = walk_there(&mut w, &defs);
    assert!(glided * 2 < round, "the gates take {glided} ticks, the shore {round}");
    // Both ends counted the one crossing, and the unit is not still in the air.
    assert_eq!(w.anchored(A).unwrap().crossings, 1);
    assert_eq!(w.anchored(B).unwrap().crossings, 1);
    assert!(!w.is_gliding(w.units[0].id));
}

#[test]
fn a_hop_takes_its_seconds_and_needs_do_not_interrupt_it() {
    let (mut w, defs) = setup(true);
    send(&mut w, &defs);
    let id = w.units[0].id;
    let mut guard = 0;
    while !w.is_gliding(id) {
        w.tick(&defs);
        guard += 1;
        assert!(guard < 1000, "never took off");
    }
    assert_eq!(w.units[0].pos, A, "the hop starts from the gate");
    // Starving, mid-hop: nothing is done about it until the unit lands.
    w.units[0].needs.hunger = defs.needs.hunger.low * 0.5;
    w.units[0].needs.thirst = defs.needs.thirst.low * 0.5;
    let mut ticks = 0;
    while w.is_gliding(id) {
        assert_eq!(w.units[0].pos, A, "stood on the gate for the hop");
        assert!(w.units[0].need_task.is_none(), "a need interrupted the hop");
        w.tick(&defs);
        ticks += 1;
    }
    assert_eq!(ticks, 50, "2 s plus 0.1 s a tile, over 30 tiles");
    assert_eq!(w.units[0].pos, B);
    // Once it has landed the need is its own again.
    for _ in 0..5 {
        w.tick(&defs);
    }
    assert!(w.units[0].need_task.is_some(), "it goes to eat or drink");
}

/// The plain walk from `from` to `to`, as a route.
fn direct(w: &World, from: Pos, to: Pos) -> Vec<Pos> {
    let flood = Flood::new(&w.map, from, &w.cart_rules);
    route_from(flood.path_to(to).expect("reachable"))
}

fn has_hop(w: &World) -> bool {
    let u = &w.units[0];
    let route = u.route.as_ref().unwrap();
    let mut from = u.pos;
    route.iter().rev().any(|&to| {
        let far = from.chebyshev(to) > 1;
        from = to;
        far
    })
}

#[test]
fn a_loaded_unit_walks() {
    let (mut w, defs) = setup(true);
    // The same trip, empty-handed and loaded.
    let route = direct(&w, START, at(70, 8));
    w.units[0].carrying = Some(Stack { item: defs.item_id("wood").unwrap(), count: 1 });
    w.units[0].route = Some(route.clone());
    w.walk(0, &defs);
    assert!(!has_hop(&w), "a load walks the long way");
    assert!(w.units[0].route.as_ref().unwrap().len() >= route.len() - 1);

    w.units[0].carrying = None;
    w.units[0].route = Some(route);
    w.units[0].pos = START;
    w.units[0].glider = Glider::default();
    w.walk(0, &defs);
    assert!(has_hop(&w), "with empty hands it rides the gates");
}

#[test]
fn a_short_trip_walks_and_a_tie_walks() {
    let (mut w, defs) = setup(true);
    let route = direct(&w, START, at(36, 8));
    w.units[0].route = Some(route.clone());
    w.walk(0, &defs);
    assert!(!has_hop(&w));
    assert!(w.units[0].route.as_ref().unwrap().len() >= route.len() - 1, "still the same walk");

    // A trip that needs a gate but that the gate's hop alone costs more than: the same lake
    // crossed by a unit with a very long hop is a walk.
    let mut slow = defs.clone();
    let gate = slow.building_id("wind_gate").unwrap();
    slow.buildings[gate.0 as usize].glide =
        Some(crate::building::GlideDef { seconds: 10_000.0, seconds_per_tile: 0.0, loads: false });
    let mut w2 = World::with_map(1, w.map.clone(), Nation::Air, &[START], &slow);
    for p in [A, B] {
        w2.put_building(Footprint::single(p), gate, &slow);
    }
    w2.apply(Command::SetLink { pos: A, to: B }, &slow).unwrap();
    w2.units[0].route = Some(direct(&w2, START, at(70, 8)));
    w2.walk(0, &slow);
    assert!(!has_hop(&w2), "a hop that costs more than the shore is not taken");
}

#[test]
fn a_cut_off_pair_is_not_ridden() {
    let (mut w, defs) = setup(true);
    // A wall of water all the way down the map splits the shore too: the pair is cut off.
    for y in 0..HEIGHT {
        w.map[at(47, y)].terrain = Terrain::Water;
    }
    w.map_changed();
    assert_eq!(w.pair_status(A, &defs), Some(PairStatus::CutOff));
    w.units[0].route = Some(direct(&w, START, at(36, 8)));
    // Nothing to compare: it just walks.
    w.walk(0, &defs);
    assert!(!has_hop(&w));
}

#[test]
fn a_field_is_kept_until_the_map_changes() {
    let (mut w, _defs) = setup(true);
    let far = at(80, 20);
    let before = w.link_field(B, 0).cost(far).expect("reachable");
    assert_eq!(w.link_fields.floods, 1);
    assert_eq!(w.link_field(B, 0).cost(far), Some(before));
    assert_eq!(w.link_fields.floods, 1, "the second ask is a lookup");
    w.link_field(A, 0);
    assert_eq!(w.link_fields.floods, 2, "one field per end");
    assert_eq!(w.link_field(B, 0).cost(at(46, 10)), None, "the lake is not walkable");

    // A wall of water between the gate and the far tile makes the walk longer; the old field,
    // which knew nothing of it, must not be read.
    for y in 0..30 {
        w.map[at(70, y)].terrain = Terrain::Water;
    }
    w.map_changed();
    let after = w.link_field(B, 0).cost(far).expect("round the wall");
    assert!(after > before, "{after} should be past {before}");
    assert_eq!(w.link_fields.floods, 3, "the change made a new field");
    assert_eq!(w.link_field(B, 0).cost(far), Some(after));
    assert_eq!(w.link_fields.floods, 3);
}

#[test]
fn a_gate_taken_down_mid_walk_drops_the_route() {
    let (mut w, defs) = setup(true);
    send(&mut w, &defs);
    // Walk until the unit stands on gate A, about to hop.
    let mut guard = 0;
    while w.units[0].pos != A {
        w.tick(&defs);
        guard += 1;
        assert!(guard < 1000);
    }
    // The partner goes: the pair no longer works.
    let key_gone = w.pair_partner(A, &defs);
    assert_eq!(key_gone, Some(B));
    let key = w.map[B].building.unwrap().key();
    w.remove_building(key, &defs).unwrap();
    assert_eq!(w.pair_partner(A, &defs), None);
    for _ in 0..3 {
        w.tick(&defs);
    }
    assert!(!w.is_gliding(w.units[0].id), "no hop to a gate that is gone");
    assert_eq!(w.units[0].pos, A);
}

#[test]
fn a_save_round_trip_mid_hop_lands_the_same() {
    let (mut w, defs) = setup(true);
    send(&mut w, &defs);
    let id = w.units[0].id;
    while !w.is_gliding(id) {
        w.tick(&defs);
    }
    for _ in 0..10 {
        w.tick(&defs);
    }
    let mut loaded = World::load(&w.save(&defs).expect("saves"), &defs).expect("loads");
    assert!(loaded.is_gliding(id));
    for _ in 0..80 {
        w.tick(&defs);
        loaded.tick(&defs);
    }
    assert_eq!(w.units[0].pos, loaded.units[0].pos);
    assert_eq!(w.anchored(B).unwrap().crossings, loaded.anchored(B).unwrap().crossings);
    assert_eq!(w.save(&defs).unwrap(), loaded.save(&defs).unwrap());
}

#[test]
fn gliding_is_deterministic() {
    let run = || {
        let (mut w, defs) = setup(true);
        send(&mut w, &defs);
        for _ in 0..400 {
            w.tick(&defs);
        }
        w.save(&defs).expect("saves")
    };
    assert_eq!(run(), run());
}

#[test]
fn a_world_with_no_gate_keeps_no_glide_state() {
    let (mut w, defs) = setup(false);
    send(&mut w, &defs);
    for _ in 0..100 {
        w.tick(&defs);
    }
    assert!(w.units[0].glider.is_idle());
}

// --- The Sky Lift (#756): a pair anyone rides, loads and all ---

/// `setup`, with a pair of `id` at `A` and `B` instead of Wind Gates.
fn setup_pair(id: &str) -> (World, Defs) {
    let (mut w, defs) = setup(false);
    let id = defs.building_id(id).unwrap();
    for p in [A, B] {
        w.put_building(Footprint::single(p), id, &defs);
    }
    w.apply(Command::SetLink { pos: A, to: B }, &defs).unwrap();
    (w, defs)
}

fn load(w: &mut World, defs: &Defs) {
    w.units[0].carrying = Some(Stack { item: defs.item_id("wood").unwrap(), count: 2 });
}

#[test]
fn the_sky_lift_is_earth_and_airs_and_carries_loads() {
    let defs = Defs::bundled();
    let def = defs.building(defs.building_id("sky_lift").unwrap());
    assert!(def.passable, "a unit steps on to board");
    let lift = def.glide.unwrap();
    assert!(lift.loads);
    assert_eq!(lift.ticks(32), 120, "4 s plus 0.25 s a tile: 12 s over 32 tiles");
    assert_eq!(def.pair.map(|p| (p.reach, p.min_tiles)), Some((32, 8)));
    assert_eq!(def.beside.as_deref(), Some(&[Terrain::Mountain][..]));
    let tech = defs.tech(def.unlocked_by.unwrap());
    assert_eq!((tech.id.as_str(), tech.cost), ("sky_lifts", 1000));
    assert_eq!(tech.fusion, Some([Nation::Earth, Nation::Air]));
    // The Wind Gate carries nobody's load.
    let gate = defs.building(defs.building_id("wind_gate").unwrap());
    assert!(!gate.glide.unwrap().loads);
}

#[test]
fn a_loaded_unit_rides_a_sky_lift_and_an_empty_one_does_too() {
    let (mut w, defs) = setup_pair("sky_lift");
    let route = direct(&w, START, at(70, 8));
    load(&mut w, &defs);
    w.units[0].route = Some(route.clone());
    w.walk(0, &defs);
    assert!(has_hop(&w), "a load rides the lift past the lake");

    w.units[0].carrying = None;
    w.units[0].route = Some(route);
    w.units[0].pos = START;
    w.units[0].glider = Glider::default();
    w.walk(0, &defs);
    assert!(has_hop(&w), "and so does a unit with empty hands");
}

#[test]
fn a_load_crosses_by_lift_and_arrives_with_the_unit() {
    let (mut w, defs) = setup_pair("sky_lift");
    let id = w.units[0].id;
    send(&mut w, &defs);
    // Picks up a load once on its way (a hauler's trip is the same walk with a load).
    let mut guard = 0;
    while w.units[0].route.as_ref().is_none_or(|r| r.is_empty()) {
        w.tick(&defs);
        guard += 1;
        assert!(guard < 1000, "never set out");
    }
    load(&mut w, &defs);
    let stack = w.units[0].carrying;
    guard = 0;
    while !w.is_gliding(id) {
        w.tick(&defs);
        guard += 1;
        assert!(guard < 1000, "never boarded");
    }
    assert_eq!(w.units[0].pos, A);
    assert_eq!(w.units[0].carrying, stack, "the load is aboard");
    let mut ticks = 0;
    while w.is_gliding(id) {
        w.tick(&defs);
        ticks += 1;
    }
    assert_eq!(ticks, 4 * 10 + 30 * 10 / 4, "4 s plus 0.25 s over 30 tiles");
    assert_eq!(w.units[0].pos, B);
    assert_eq!(w.units[0].carrying, stack, "and lands with it");
    assert_eq!(w.crossings_at(A), 1);
    assert_eq!(w.crossings_at(B), 1);
}

#[test]
fn a_wind_gate_still_leaves_the_load_behind() {
    let (mut w, defs) = setup_pair("wind_gate");
    load(&mut w, &defs);
    w.units[0].route = Some(direct(&w, START, at(70, 8)));
    w.walk(0, &defs);
    assert!(!has_hop(&w));
}

#[test]
fn a_beast_never_rides_a_lift() {
    let (mut w, defs) = setup_pair("sky_lift");
    w.units[0].beast = Some(Beast { id: crate::beast::BeastId(0), home: START, bored: 0 });
    w.units[0].route = Some(direct(&w, START, at(70, 8)));
    w.walk(0, &defs);
    assert!(!has_hop(&w));
}

#[test]
fn a_cut_off_lift_is_not_ridden_loaded_either() {
    let (mut w, defs) = setup_pair("sky_lift");
    for y in 0..HEIGHT {
        w.map[at(47, y)].terrain = Terrain::Water;
    }
    w.map_changed();
    assert_eq!(w.pair_status(A, &defs), Some(PairStatus::CutOff));
    load(&mut w, &defs);
    w.units[0].route = Some(direct(&w, START, at(36, 8)));
    w.walk(0, &defs);
    assert!(!has_hop(&w));
}

#[test]
fn both_ends_of_a_lift_must_stand_against_rock() {
    let mut defs = Defs::bundled();
    let lift = defs.building_id("sky_lift").unwrap();
    // Research is not what is tested here.
    defs.buildings[lift.0 as usize].unlocked_by = None;
    let mut w = World::with_map(1, flat(), Nation::Air, &[START], &defs);
    let site = at(20, 20);
    let refused = w.check_build(site, lift, &defs);
    assert!(
        matches!(refused, Err(BuildError::CannotBuild { why: Blocker::Beside("rock"), .. })),
        "{refused:?}"
    );
    w.map[at(21, 20)].terrain = Terrain::Mountain;
    w.map_changed();
    assert_eq!(w.check_build(site, lift, &defs), Ok(()));
}

#[test]
fn a_lift_ride_saves_mid_ride_and_lands_the_same() {
    let (mut w, defs) = setup_pair("sky_lift");
    let id = w.units[0].id;
    load(&mut w, &defs);
    send(&mut w, &defs);
    for _ in 0..1000 {
        if w.is_gliding(id) {
            break;
        }
        w.tick(&defs);
    }
    assert!(w.is_gliding(id), "never boarded");
    for _ in 0..10 {
        w.tick(&defs);
    }
    let mut loaded = World::load(&w.save(&defs).expect("saves"), &defs).expect("loads");
    assert!(loaded.is_gliding(id));
    for _ in 0..200 {
        w.tick(&defs);
        loaded.tick(&defs);
    }
    assert_eq!(w.units[0].pos, loaded.units[0].pos);
    assert_eq!(w.save(&defs).unwrap(), loaded.save(&defs).unwrap());
}
