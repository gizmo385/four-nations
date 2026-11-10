//! Population growth (#232): a newcomer joins only when a bed and food are both there for it.

use super::*;
use crate::building::BuildingId;
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

/// Stands a finished 1x1 building (`put_building`).
fn place(w: &mut World, defs: &Defs, pos: Pos, building: &str) -> BuildingId {
    let id = defs.building_id(building).unwrap();
    w.put_building(crate::geom::Footprint::single(pos), id, defs);
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
    // One settler, so two beds: capacity above the headcount (#552).
    place(&mut w, &defs, at(10, 2), "bed");
    place(&mut w, &defs, at(12, 2), "bed");
    stock_food(&mut w, &defs, defs.needs.growth.food_per_newcomer);
    let before = w.units.len();
    tick_one_check(&mut w, &defs);
    assert_eq!(w.units.len(), before + 1, "a newcomer should have joined");
    assert_eq!(w.arrivals().count(), 1);
    let arrival = &w.arrivals().next().unwrap();
    assert_eq!(arrival.unit, w.units.last().unwrap().id);
    assert!(!arrival.name.is_empty());
}

/// #303: as long as the nation's pool still has an unused name, every newcomer up to its size
/// gets a distinct one; nobody shares a name with the starting settler either. Calls
/// `spawn_newcomer` directly (as the difficulty preset test below does), since it doesn't check
/// beds or stores itself.
#[test]
fn newcomers_up_to_the_pool_size_have_distinct_names() {
    let defs = Defs::bundled();
    let mut w = world(1, &defs);
    let pool_size = defs.nation(Nation::Earth).names.len();
    for _ in 0..pool_size {
        assert!(w.spawn_newcomer(&defs), "the field always has room for a newcomer");
    }
    let names: BTreeSet<&str> = w.units.iter().map(|u| u.name.as_str()).collect();
    assert_eq!(
        names.len(),
        w.units.len(),
        "every unit, the starting settler included, should have a distinct name"
    );
}

/// #303: once every name in the pool already belongs to a living unit, a newcomer past the pool
/// size still gets a name nobody else has, by a numeral suffix (mirrors `spawn_units`' own
/// overflow rule for a starting roster bigger than its pool).
#[test]
fn once_the_pool_is_exhausted_a_newcomer_gets_a_numeral_suffix() {
    let defs = Defs::bundled();
    let mut w = world(1, &defs);
    let pool_size = defs.nation(Nation::Earth).names.len();
    for _ in 0..pool_size {
        assert!(w.spawn_newcomer(&defs));
    }
    assert!(w.spawn_newcomer(&defs), "the field still has room past the pool size");
    let names: Vec<&str> = w.units.iter().map(|u| u.name.as_str()).collect();
    let unique: BTreeSet<&str> = names.iter().copied().collect();
    assert_eq!(unique.len(), names.len(), "the overflow newcomer should still be unique");
    let last = names.last().expect("at least one unit was spawned");
    assert!(
        last.chars().last().is_some_and(|c| c.is_ascii_digit()),
        "an overflow newcomer's name should carry a numeral suffix, got {last:?}"
    );
}

/// #303: the draw is still deterministic once the pool is exhausted, the same as the simple
/// draw `two_runs_from_one_seed_give_the_same_arrivals` already checks.
#[test]
fn the_overflow_suffix_is_deterministic_for_a_seed() {
    let defs = Defs::bundled();
    let names_for = |seed| {
        let mut w = world(seed, &defs);
        let pool_size = defs.nation(Nation::Earth).names.len();
        for _ in 0..pool_size + 3 {
            assert!(w.spawn_newcomer(&defs));
        }
        w.units.iter().map(|u| u.name.clone()).collect::<Vec<_>>()
    };
    assert_eq!(names_for(11), names_for(11));
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
    assert!(w.arrivals().next().is_none());
}

#[test]
fn no_newcomer_without_enough_food() {
    let defs = Defs::bundled();
    let mut w = world(1, &defs);
    place(&mut w, &defs, at(10, 2), "bed");
    place(&mut w, &defs, at(12, 2), "bed");
    stock_food(&mut w, &defs, defs.needs.growth.food_per_newcomer - 1);
    let before = w.units.len();
    tick_one_check(&mut w, &defs);
    assert_eq!(w.units.len(), before, "not enough food, no newcomer");
    assert!(w.arrivals().next().is_none());
}

/// #552, #513: growth needs bed capacity above the living settlers, whether or not anyone is
/// lying in a bed right now (`Unit::bed` is only set while walking to or sleeping in one, so the
/// old "some bed is unclaimed" check was true for any settlement with a bed). Calls
/// `has_room_for_newcomer` directly, so the settlers' own needs can't muddy it.
#[test]
fn growth_needs_bed_capacity_above_the_headcount() {
    let defs = Defs::bundled();
    let mut w = world(1, &defs);
    assert_eq!(w.bed_capacity(&defs), 0);
    assert!(!w.has_room_for_newcomer(&defs), "no beds");
    // Spare capacity: two 1x1 beds (one sleeper each), one settler.
    place(&mut w, &defs, at(10, 2), "bed");
    place(&mut w, &defs, at(12, 2), "bed");
    assert_eq!(w.bed_capacity(&defs), 2);
    assert!(w.has_room_for_newcomer(&defs), "two beds, one settler");
    // Exactly full: two settlers fill both beds, though nobody is lying in one right now.
    assert!(w.spawn_newcomer(&defs));
    assert!(w.units.iter().all(|u| u.bed.is_none()), "nobody is in a bed");
    assert!(!w.has_room_for_newcomer(&defs), "two beds, two settlers: full");
    // A third bed reopens it; without it, it is closed again, and nobody is evicted.
    place(&mut w, &defs, at(14, 2), "bed");
    assert!(w.has_room_for_newcomer(&defs));
    let third = w.map[at(14, 2)].building.unwrap().key();
    w.remove_building(third, &defs).unwrap();
    assert_eq!(w.bed_capacity(&defs), 2, "capacity lost");
    assert!(!w.has_room_for_newcomer(&defs));
    assert_eq!(w.units.len(), 2, "no one is evicted");
}

/// The Avatar (#237) isn't one of the settlers growth makes room for.
#[test]
fn the_avatar_does_not_use_up_a_bed_for_growth() {
    let defs = Defs::bundled();
    let mut w = world(1, &defs);
    place(&mut w, &defs, at(10, 2), "bed");
    place(&mut w, &defs, at(12, 2), "bed");
    assert!(w.spawn_newcomer(&defs));
    assert!(!w.has_room_for_newcomer(&defs));
    w.units[1].guest = Some(Guest::Avatar);
    assert!(w.has_room_for_newcomer(&defs), "two beds, one settler and the Avatar");
}

#[test]
fn growth_stalls_when_the_beds_are_full_even_with_food() {
    let defs = Defs::bundled();
    let mut w = world(1, &defs);
    place(&mut w, &defs, at(10, 2), "bed");
    stock_food(&mut w, &defs, defs.needs.growth.food_per_newcomer * 4);
    let before = w.units.len();
    tick_one_check(&mut w, &defs);
    assert_eq!(w.units.len(), before, "one bed, one settler: no newcomer");
}

#[test]
fn the_newcomer_lands_on_the_edge_tile_nearest_the_settlement() {
    let defs = Defs::bundled();
    let mut w = world(1, &defs);
    // Frozen so the settlement's one unit can't wander off (10, 4) and move the anchor.
    w.units[0].idle_until = u64::MAX;
    place(&mut w, &defs, at(10, 2), "bed");
    place(&mut w, &defs, at(12, 2), "bed");
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
    place(&mut w, &defs, at(12, 4), "bed");
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
        for x in [10, 12, 14, 16] {
            place(&mut w, &defs, at(x, 2), "bed");
        }
        stock_food(&mut w, &defs, defs.needs.growth.food_per_newcomer * 10);
        w
    };
    let mut a = setup(7);
    let mut b = setup(7);
    for _ in 0..interval_ticks(&defs) * 5 {
        a.tick(&defs);
        b.tick(&defs);
    }
    assert!(a.arrivals().next().is_some(), "growth should have fired at least once");
    assert!(a.arrivals().eq(b.arrivals()));
    assert_eq!(a.save(&defs).unwrap(), b.save(&defs).unwrap());
}

/// Before the Harmony Gate is complete, `newcomer_nation` never consults `newcomer_rng`: it is
/// always the settlement's own nation, whatever the weights say (#240).
#[test]
fn newcomer_nation_is_always_the_settlements_own_before_completion() {
    let defs = Defs::bundled();
    let mut w = world(1, &defs);
    for _ in 0..20 {
        assert_eq!(w.newcomer_nation(&defs), Nation::Earth);
    }
}

/// Once the gate is `Complete`, `newcomer_nation` draws from the weighted mix
/// (`docs/design/endgame.md`, "The transformation: the United City"): the same seed gives the
/// same sequence of nations, and over enough draws it includes both the settlement's own nation
/// and at least one other.
#[test]
fn the_newcomers_nation_mix_is_deterministic_for_a_seed() {
    let defs = Defs::bundled();
    let draws = |seed| {
        let mut w = world(seed, &defs);
        w.endeavor = EndeavorState::Complete { tick: 0 };
        (0..50).map(|_| w.newcomer_nation(&defs)).collect::<Vec<_>>()
    };
    let a = draws(7);
    let b = draws(7);
    assert_eq!(a, b, "the same seed must draw the same sequence of nations");
    assert!(a.contains(&Nation::Earth), "the settlement's own nation should still be drawn");
    assert!(a.iter().any(|&n| n != Nation::Earth), "another nation should be drawn too");
}

/// A newcomer joining through `spawn_newcomer` is the settlement's own nation while the gate is
/// Dormant, and may be another nation once it is `Complete` (#240).
#[test]
fn a_newcomer_is_of_another_nation_only_once_the_gate_is_complete() {
    let defs = Defs::bundled();
    // Unlike `field()`, a strip of water along one edge, so a long run doesn't dehydrate the
    // whole settlement (`tick_one_check`'s doc): this test runs many checks to give the weighted
    // draw a real chance, not just one.
    let map = Grid::from_fn(21, 9, |p: Pos| Tile {
        elevation: 100,
        temperature: 128,
        moisture: 128,
        terrain: if p.y == 8 { Terrain::River } else { Terrain::Land },
        biome: BiomeId(0),
        resource: None,
        building: None,
    });
    let mut w = World::with_map(1, map, Nation::Earth, &[at(10, 4)], &defs);
    // Several beds, well spread out, so a sleeper claiming one never blocks the next check.
    for x in (2..20).step_by(3) {
        place(&mut w, &defs, at(x, 2), "bed");
    }
    stock_food(&mut w, &defs, defs.needs.growth.food_per_newcomer);
    tick_one_check(&mut w, &defs);
    assert_eq!(w.units.len(), 2, "a newcomer should have joined while dormant");
    assert!(w.units.iter().all(|u| w.nation_of(u) == Nation::Earth));

    w.endeavor = EndeavorState::Complete { tick: w.tick };
    let mut saw_another_nation = false;
    for _ in 0..60 {
        let before = w.units.len();
        // Keep the larder well above the threshold, since eating between checks draws it down;
        // `has_free_bed`/`stored_food` are all the check looks at.
        stock_food(&mut w, &defs, defs.needs.growth.food_per_newcomer * 10);
        tick_one_check(&mut w, &defs);
        if w.units.len() > before && w.nation_of(w.units.last().unwrap()) != Nation::Earth {
            saw_another_nation = true;
            break;
        }
    }
    assert!(saw_another_nation, "a newcomer of another nation should have arrived by now");
}

/// `World::modifiers_for` and `unit_can_bend` already read `Unit::nation` per unit (#239); this
/// checks the United City actually uses that: an Earth newcomer living in a Water settlement can
/// still raise a ford, which no Water bender could do.
#[test]
fn a_water_settlement_with_an_earth_newcomer_can_ford_a_river() {
    let defs = Defs::bundled();
    let plains = BiomeId(defs.biomes.iter().position(|b| b.id == "plains").unwrap() as u8);
    let map = Grid::from_fn(13, 7, |p: Pos| Tile {
        elevation: 100,
        temperature: 128,
        moisture: 128,
        terrain: if (5..=7).contains(&p.x) { Terrain::River } else { Terrain::Land },
        biome: plains,
        resource: None,
        building: None,
    });
    let mut w = World::with_map(1, map, Nation::Water, &[at(3, 3)], &defs);
    // An Earth newcomer who joined after the Harmony Gate: `Unit::nation` is all #240 needs to
    // set, since per-unit modifiers and bend eligibility (#239) already read it.
    w.units[0].nation = Some(Nation::Earth);
    w.units[0].chi = defs.chi.max;
    let ability = defs.ability_id("raise_earth").unwrap();
    w.apply(Command::Bend { ability, pos: at(5, 3) }, &defs).unwrap();
    for _ in 0..500 {
        if w.map[at(5, 3)].terrain == Terrain::Ford {
            break;
        }
        w.tick(&defs);
    }
    assert_eq!(w.map[at(5, 3)].terrain, Terrain::Ford, "the Earth newcomer should raise a ford");
}

#[test]
fn old_saves_without_the_growth_fields_still_load() {
    let defs = Defs::bundled();
    let w = world(1, &defs);
    let text = w.save(&defs).unwrap();
    // Arrivals are in the event log since #605; a v4 save has no `arrivals` list to leave out.
    assert!(text.contains("next_unit:1") && !text.contains("arrivals"), "{text}");
    let old = text.replace("next_unit:1,", "");
    let mut loaded = World::load(&old, &defs).unwrap();
    assert_eq!(loaded.arrivals().count(), 0);
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
