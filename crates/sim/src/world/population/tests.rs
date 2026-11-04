//! Population growth (#232): a newcomer joins only when a bed and food are both there for it.

use super::*;
use crate::building::{Building, BuildingId};
use crate::defs::{BiomeId, Terrain};
use crate::geom::Grid;
use crate::mapgen::Tile;

fn at(x: i32, y: i32) -> Pos {
    Pos::new(x, y)
}

/// A flat 21x9 field of land, so every tile on the rectangular edge is passable.
fn field() -> TileMap {
    Grid::from_fn(21, 9, |_| Tile {
        elevation: 100,
        temperature: 128,
        moisture: 128,
        terrain: Terrain::Land,
        biome: BiomeId(0),
        resource: None,
        building: None,
    })
}

fn world(seed: u64, defs: &Defs) -> World {
    World::with_map(seed, field(), Nation::Earth, &[at(10, 4)], defs)
}

/// Stands a finished 1x1 building (as `raise` would), updating `building_index` (#292) too.
fn place(w: &mut World, defs: &Defs, pos: Pos, building: &str) -> BuildingId {
    let id = defs.building_id(building).unwrap();
    let def = defs.building(id);
    w.map[pos].building = Some(Building::new(id, def.passable, def.rail.is_some()));
    w.building_index.insert((pos.y, pos.x), pos);
    w.map_changed();
    id
}

fn stock_food(w: &mut World, defs: &Defs, count: u32) {
    let fish = defs.item_id("fish").unwrap();
    w.stored.insert(at(0, 4), Stack { item: fish, count });
}

fn interval_ticks(defs: &Defs) -> u64 {
    (defs.needs.growth.arrival_minutes * 60.0 * TICKS_PER_SECOND as f32).round() as u64
}

/// Ticks for exactly one growth check (growth checks only ever fire on an `interval_ticks`
/// boundary), which stays well inside the lone settler's dehydration clock: there is no water
/// on `field`, so a longer run would kill it off and confuse what these tests are checking.
fn tick_one_check(w: &mut World, defs: &Defs) {
    for _ in 0..interval_ticks(defs) {
        w.tick(defs);
    }
}

#[test]
fn a_newcomer_joins_with_a_free_bed_and_enough_food() {
    let defs = Defs::bundled();
    let mut w = world(1, &defs);
    place(&mut w, &defs, at(10, 2), "bed");
    stock_food(&mut w, &defs, defs.needs.growth.food_per_newcomer);
    let before = w.units.len();
    tick_one_check(&mut w, &defs);
    assert_eq!(w.units.len(), before + 1, "a newcomer should have joined");
    assert_eq!(w.arrivals().len(), 1);
    let arrival = &w.arrivals()[0];
    assert_eq!(arrival.unit, w.units.last().unwrap().id);
    assert!(!arrival.name.is_empty());
}

#[test]
fn a_newcomer_under_a_difficulty_preset_also_eats_newcomer_food() {
    // The difficulty preset's `newcomer_food` knob (#294): on top of #232's
    // `food_per_newcomer` threshold, the arrival itself takes `newcomer_food` more from stores,
    // the nearest stack first. Identity (`None`) takes none, so this only shows once a preset is
    // set. Calls `spawn_newcomer`/`take_newcomer_food` directly (as `tick_population` does) so
    // the lone settler's own needs can't incidentally eat into the stockpile and confuse it.
    let defs = Defs::bundled();
    let mut w = world(1, &defs);
    w.difficulty = Some("hard".into());
    let newcomer_food = w.knobs(&defs).newcomer_food;
    assert!(newcomer_food > 0, "test assumes hard's newcomer_food knob is above identity");
    let stocked = newcomer_food + 5;
    stock_food(&mut w, &defs, stocked);
    assert!(w.spawn_newcomer(&defs), "the field has room for a newcomer");
    w.take_newcomer_food(newcomer_food, &defs);
    let left: u32 = w.stored.values().map(|s| s.count).sum();
    assert_eq!(left, stocked - newcomer_food);
}

#[test]
fn no_newcomer_without_a_free_bed() {
    let defs = Defs::bundled();
    let mut w = world(1, &defs);
    stock_food(&mut w, &defs, defs.needs.growth.food_per_newcomer);
    let before = w.units.len();
    tick_one_check(&mut w, &defs);
    assert_eq!(w.units.len(), before, "no bed, no newcomer");
    assert!(w.arrivals().is_empty());
}

#[test]
fn no_newcomer_without_enough_food() {
    let defs = Defs::bundled();
    let mut w = world(1, &defs);
    place(&mut w, &defs, at(10, 2), "bed");
    stock_food(&mut w, &defs, defs.needs.growth.food_per_newcomer - 1);
    let before = w.units.len();
    tick_one_check(&mut w, &defs);
    assert_eq!(w.units.len(), before, "not enough food, no newcomer");
    assert!(w.arrivals().is_empty());
}

/// `has_free_bed` and `tick_population` are internal, but testing them directly (rather than
/// through a long run of `World::tick`) is what lets this check the claim itself, not whatever
/// else a settler with nothing to sleep for gets up to over `arrival_minutes` of simulated
/// time (its `bed` is task-scoped state, cleared the moment any need task of its own finishes,
/// `work.rs`'s `finish_need`; faking one claimed without it actually sleeping doesn't survive a
/// real tick).
#[test]
fn a_bed_already_claimed_is_not_free() {
    let defs = Defs::bundled();
    let mut w = world(1, &defs);
    // A 1x1 bed always sleeps one, whatever `BuildingDef::capacity` says (#227 reserves that
    // field for beds bigger than 1x1; `has_free_bed` mirrors `bed_route`'s own rule here).
    place(&mut w, &defs, at(10, 2), "bed");
    assert!(w.has_free_bed(&defs), "nobody has claimed it yet");
    w.units[0].bed = Some(at(10, 2));
    assert!(!w.has_free_bed(&defs), "the only bed is already claimed");
}

#[test]
fn the_newcomer_lands_on_the_edge_tile_nearest_the_settlement() {
    let defs = Defs::bundled();
    let mut w = world(1, &defs);
    // Frozen so the settlement's one unit can't wander off (10, 4) and move the anchor.
    w.units[0].idle_until = u64::MAX;
    place(&mut w, &defs, at(10, 2), "bed");
    stock_food(&mut w, &defs, defs.needs.growth.food_per_newcomer);
    tick_one_check(&mut w, &defs);
    let newcomer = w.units.last().unwrap();
    // The settlement has no `start`, so the anchor is the mean position of its units (10, 4):
    // the nearest points on the field's rectangular edge (21x9, x in 0..=20, y in 0..=8) are
    // (10, 0) and (10, 8), tied at squared distance 16; `Pos`'s derived `Ord` compares `x` then
    // `y`, so (10, 0) wins the tie.
    assert_eq!(newcomer.pos, at(10, 0));
}

/// A real map's edge is sea (`MapGenConfig::water_border`), and a second island sits nearer the
/// edge than the settlement's own coast: the newcomer still arrives, on the settlement's own land
/// nearest the edge, never on the other island.
#[test]
fn on_a_sea_ringed_map_the_newcomer_arrives_on_its_own_coast_farthest_out() {
    let defs = Defs::bundled();
    // Land is x 4..=16, y 3..=7 on a 21x11 sea; a one-tile islet sits at (1, 1).
    let map = Grid::from_fn(21, 11, |p: Pos| {
        let land = ((4..=16).contains(&p.x) && (3..=7).contains(&p.y)) || p == at(1, 1);
        Tile {
            elevation: 100,
            temperature: 128,
            moisture: 128,
            terrain: if land { Terrain::Land } else { Terrain::Water },
            biome: BiomeId(0),
            resource: None,
            building: None,
        }
    });
    let mut w = World::with_map(1, map, Nation::Earth, &[at(10, 5)], &defs);
    w.units[0].idle_until = u64::MAX;
    place(&mut w, &defs, at(10, 4), "bed");
    stock_food(&mut w, &defs, defs.needs.growth.food_per_newcomer);
    // Stores sit at (0, 4) in `stock_food`, off this map's land; move them onto it.
    let stack = w.stored.remove(&at(0, 4)).unwrap();
    w.stored.insert(at(12, 6), stack);
    tick_one_check(&mut w, &defs);
    assert_eq!(w.units.len(), 2, "a newcomer arrived despite the sea border");
    // The land's rows 3 and 7 are 3 tiles from the edge; of those, (10, 3) and (10, 7) are
    // nearest the anchor (10, 5), and (10, 3) wins the `Pos` tie.
    assert_eq!(w.units[1].pos, at(10, 3));
}

#[test]
fn two_runs_from_one_seed_give_the_same_arrivals() {
    let defs = Defs::bundled();
    let setup = |seed| {
        let mut w = world(seed, &defs);
        place(&mut w, &defs, at(10, 2), "bed");
        stock_food(&mut w, &defs, defs.needs.growth.food_per_newcomer * 10);
        w
    };
    let mut a = setup(7);
    let mut b = setup(7);
    for _ in 0..interval_ticks(&defs) * 5 {
        a.tick(&defs);
        b.tick(&defs);
    }
    assert!(!a.arrivals().is_empty(), "growth should have fired at least once");
    assert_eq!(a.arrivals(), b.arrivals());
    assert_eq!(a.save(&defs).unwrap(), b.save(&defs).unwrap());
}

#[test]
fn old_saves_without_the_growth_fields_still_load() {
    let defs = Defs::bundled();
    let w = world(1, &defs);
    let text = w.save(&defs).unwrap();
    assert!(text.contains("next_unit:1") && text.contains("arrivals:[]"), "{text}");
    let old = text.replace("next_unit:1,", "").replace("arrivals:[],", "");
    let mut loaded = World::load(&old, &defs).unwrap();
    assert_eq!(loaded.arrivals(), &[]);
    // A missing `next_unit` deserializes as 0, which would collide with the one unit already on
    // file (also id 0); `World::load`'s fixup must have moved it past that before anything new
    // can spawn.
    place(&mut loaded, &defs, at(10, 2), "bed");
    stock_food(&mut loaded, &defs, defs.needs.growth.food_per_newcomer);
    tick_one_check(&mut loaded, &defs);
    let ids: Vec<_> = loaded.units.iter().map(|u| u.id).collect();
    let mut sorted = ids.clone();
    sorted.sort();
    sorted.dedup();
    assert_eq!(ids.len(), sorted.len(), "a newcomer's id collided with an existing unit: {ids:?}");
}
