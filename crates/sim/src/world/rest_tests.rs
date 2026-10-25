//! Beds, campfires and cooking (issue #58): where tired units sleep, and when a meal is cooked.

use super::*;
use crate::building::{Building, BuildingId};
use crate::defs::{BiomeId, Terrain};
use crate::geom::Grid;
use crate::mapgen::Tile;

fn at(x: i32, y: i32) -> Pos {
    Pos::new(x, y)
}

/// A 21x9 plains field.
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

fn world(nation: Nation, spawns: &[Pos], defs: &Defs) -> World {
    World::with_map(3, field(), nation, spawns, defs)
}

fn run_until(w: &mut World, defs: &Defs, limit: u64, mut done: impl FnMut(&World) -> bool) -> u64 {
    for n in 0..limit {
        if done(w) {
            return n;
        }
        w.tick(defs);
    }
    panic!("not done after {limit} ticks");
}

/// Stands a finished building on `pos` the way `run_build` leaves it.
fn place(w: &mut World, defs: &Defs, pos: Pos, building: &str) -> BuildingId {
    let id = defs.building_id(building).unwrap();
    let def = defs.building(id);
    w.map[pos].building = Some(Building::new(id, def.passable, def.rail.is_some()));
    w.light_building_fire(pos, def);
    w.map_changed();
    id
}

fn tire(w: &mut World, defs: &Defs, unit: usize) {
    w.units[unit].needs.energy = defs.needs.energy.low - 1.0;
}

/// Energy regained over a minute of sleep for a unit put to sleep at `pos`.
fn minute_of_sleep(w: &mut World, defs: &Defs, pos: Pos) -> f32 {
    w.units[0].pos = pos;
    w.units[0].need_task = Some(Task::Sleep);
    w.units[0].route = None;
    w.units[0].needs = Needs { energy: 10.0, ..Needs::full(&defs.needs) };
    for _ in 0..600 {
        w.tick(defs);
    }
    assert!(w.units[0].is_sleeping());
    w.units[0].needs.energy - 10.0
}

fn cabbage(defs: &Defs) -> crate::jobs::ItemId {
    defs.item_id("cabbage").unwrap()
}

#[test]
fn a_tired_unit_walks_to_the_nearest_bed_and_sleeps_faster_there() {
    let defs = Defs::bundled();
    let base = defs.needs.general.sleep_energy_per_minute;
    let mut w = world(Nation::Earth, &[at(2, 4)], &defs);
    place(&mut w, &defs, at(14, 4), "bed");
    place(&mut w, &defs, at(6, 4), "bed");
    tire(&mut w, &defs, 0);
    run_until(&mut w, &defs, 400, |w| w.units[0].is_sleeping());
    assert_eq!(w.units[0].pos, at(6, 4), "the nearer bed");
    assert_eq!(w.units[0].task(), Some(Task::Sleep));

    let bonus = defs.building(defs.building_id("bed").unwrap()).bed.unwrap().sleep_bonus;
    assert!(bonus > 1.0);
    let in_bed = minute_of_sleep(&mut w, &defs, at(6, 4));
    assert!((in_bed - base * bonus).abs() < 0.01, "{in_bed}");
    // A tile beside it is the ground again.
    let on_ground = minute_of_sleep(&mut w, &defs, at(7, 4));
    assert!((on_ground - base).abs() < 0.01, "{on_ground}");
}

#[test]
fn nobody_shares_a_bed_and_the_odd_one_out_sleeps_on_the_ground() {
    let defs = Defs::bundled();
    let mut w = world(Nation::Earth, &[at(2, 4), at(2, 5)], &defs);
    place(&mut w, &defs, at(6, 4), "bed");
    tire(&mut w, &defs, 0);
    tire(&mut w, &defs, 1);
    run_until(&mut w, &defs, 400, |w| w.units.iter().all(|u| u.is_sleeping()));
    let spots: Vec<Pos> = w.units.iter().map(|u| u.pos).collect();
    assert_eq!(spots.iter().filter(|&&p| p == at(6, 4)).count(), 1, "{spots:?}");
    // The one without a bed slept where it stood when it got tired.
    assert!(spots.contains(&at(2, 4)) || spots.contains(&at(2, 5)), "{spots:?}");
}

#[test]
fn a_bed_is_free_again_once_its_sleeper_wakes() {
    let defs = Defs::bundled();
    let mut w = world(Nation::Earth, &[at(2, 4), at(2, 5)], &defs);
    place(&mut w, &defs, at(6, 4), "bed");
    tire(&mut w, &defs, 0);
    run_until(&mut w, &defs, 400, |w| w.units[0].is_sleeping());
    assert_eq!(w.units[0].bed, Some(at(6, 4)));
    run_until(&mut w, &defs, 10 * 60 * 10, |w| !w.units[0].is_sleeping());
    assert_eq!(w.units[0].bed, None);
    tire(&mut w, &defs, 1);
    run_until(&mut w, &defs, 400, |w| w.units[1].is_sleeping());
    assert_eq!(w.units[1].pos, at(6, 4));
}

#[test]
fn a_bed_out_of_seek_range_is_ignored() {
    let defs = Defs::bundled();
    let seek = defs.building(defs.building_id("bed").unwrap()).bed.unwrap().seek_tiles as i32;
    let strip = Grid::from_fn(seek + 8, 3, |_| Tile {
        elevation: 100,
        temperature: 128,
        moisture: 128,
        terrain: Terrain::Land,
        biome: BiomeId(0),
        resource: None,
        building: None,
    });
    let mut w = World::with_map(3, strip, Nation::Earth, &[at(0, 1)], &defs);
    // One tile past what the bed calls from.
    place(&mut w, &defs, at(seek + 3, 1), "bed");
    tire(&mut w, &defs, 0);
    run_until(&mut w, &defs, 100, |w| w.units[0].is_sleeping());
    assert_eq!(w.units[0].pos, at(0, 1));
}

#[test]
fn nobody_walks_to_a_bed_across_water() {
    let defs = Defs::bundled();
    let mut w = world(Nation::Earth, &[at(2, 4)], &defs);
    for y in 0..9 {
        w.map[at(5, y)].terrain = Terrain::Water;
    }
    place(&mut w, &defs, at(8, 4), "bed");
    tire(&mut w, &defs, 0);
    run_until(&mut w, &defs, 100, |w| w.units[0].is_sleeping());
    assert_eq!(w.units[0].pos, at(2, 4));
}

#[test]
fn a_bed_inside_a_campfires_warmth_is_better_than_either_alone() {
    let defs = Defs::bundled();
    let base = defs.needs.general.sleep_energy_per_minute;
    let bed = defs.building(defs.building_id("bed").unwrap()).bed.unwrap().sleep_bonus;
    let fire = defs.building(defs.building_id("campfire").unwrap()).heat.unwrap().sleep_bonus;
    let mut w = world(Nation::Earth, &[at(2, 4)], &defs);
    place(&mut w, &defs, at(8, 4), "campfire");
    place(&mut w, &defs, at(9, 4), "bed");
    let both = minute_of_sleep(&mut w, &defs, at(9, 4));
    assert!((both - base * bed * fire).abs() < 0.01, "{both}");
}

#[test]
fn a_finished_campfire_keeps_a_permanent_fire_scaled_by_heating() {
    let defs = Defs::bundled();
    let heat = defs.building(defs.building_id("campfire").unwrap()).heat.unwrap();
    for (nation, heating) in [(Nation::Earth, 1.0), (Nation::Water, 0.8), (Nation::Fire, 2.5)] {
        let mut w = world(nation, &[at(1, 1)], &defs);
        place(&mut w, &defs, at(10, 4), "campfire");
        let source = w.heat[&at(10, 4)];
        assert_eq!(w.modifiers().heating, heating, "{nation:?}");
        assert_eq!(source.radius, heat.radius_for(heating), "{nation:?}");
        assert!(source.is_permanent());
        // It doesn't go out, and it isn't a Hearthflame.
        for _ in 0..50 {
            w.tick(&defs);
        }
        assert!(w.is_heated(at(10, 4)));
        assert!(w.flames().is_empty() && !w.is_lit(at(10, 4)));
    }
    let earth = world(Nation::Earth, &[at(1, 1)], &defs);
    let fire = world(Nation::Fire, &[at(1, 1)], &defs);
    assert!(heat.radius_for(fire.modifiers().heating) > heat.radius_for(earth.modifiers().heating));
}

#[test]
fn building_a_campfire_lights_it_and_not_before() {
    let defs = Defs::bundled();
    let campfire = defs.building_id("campfire").unwrap();
    let def = defs.building(campfire);
    let wood = defs.item_id("wood").unwrap();
    // Fire starts with `fire_making`, which unlocks the campfire.
    let mut w = world(Nation::Fire, &[at(1, 1)], &defs);
    w.designate_stockpile(at(0, 0)).unwrap();
    w.stored.insert(at(0, 0), Stack { item: wood, count: def.cost[0].count });
    w.designate_build(at(6, 4), campfire, &defs).unwrap();
    for _ in 0..20 {
        w.tick(&defs);
    }
    assert!(!w.is_heated(at(6, 4)), "a site gives no warmth");
    run_until(&mut w, &defs, 6000, |w| w.building_at(at(6, 4)).is_some());
    assert!(w.is_heated(at(6, 4)));
    assert!(w.is_heated(at(6 + w.heat[&at(6, 4)].radius as i32, 4)));
    assert!(!w.is_heated(at(6 + w.heat[&at(6, 4)].radius as i32 + 1, 4)));
}

#[test]
fn a_meal_near_a_campfire_is_cooked_and_raw_food_works_without_one() {
    let defs = Defs::bundled();
    let food = defs.foods.iter().find(|f| f.id == "cabbage").unwrap();
    let (raw, cooked) = (food.hunger, food.cooked_hunger.unwrap());
    let before = defs.needs.hunger.low - 5.0;
    let eat = |w: &mut World| {
        w.units[0].needs.hunger = before;
        w.units[0].carrying = Some(Stack { item: cabbage(&defs), count: 1 });
        run_until(w, &defs, 200, |w| w.units[0].carrying.is_none());
        w.units[0].needs.hunger - before
    };
    // With no hearth at all the cabbage is raw, and it still feeds.
    let mut w = world(Nation::Earth, &[at(10, 4)], &defs);
    assert!((eat(&mut w) - raw).abs() < 0.5);

    // Beside a campfire it is cooked.
    let mut w = world(Nation::Earth, &[at(11, 4)], &defs);
    place(&mut w, &defs, at(10, 4), "campfire");
    assert!((eat(&mut w) - cooked).abs() < 0.5);

    // A campfire out of reach does nothing for it.
    let mut w = world(Nation::Earth, &[at(2, 4)], &defs);
    place(&mut w, &defs, at(10, 4), "campfire");
    assert!((eat(&mut w) - raw).abs() < 0.5);
}

#[test]
fn a_hearthflame_cannot_be_kindled_on_a_campfire() {
    let defs = Defs::bundled();
    let mut w = world(Nation::Fire, &[at(10, 4)], &defs);
    place(&mut w, &defs, at(8, 4), "campfire");
    let ability = defs.ability_id("hearthflame").unwrap();
    assert_eq!(
        w.apply(Command::Bend { ability, pos: at(8, 4) }, &defs),
        Err(CommandError::Bend(BendError::InvalidTarget("something is in the way")))
    );
}

#[test]
fn a_sleeper_in_a_bed_by_a_campfire_survives_a_save_and_load() {
    let defs = Defs::bundled();
    let mut w = world(Nation::Fire, &[at(2, 4)], &defs);
    place(&mut w, &defs, at(8, 4), "campfire");
    place(&mut w, &defs, at(9, 4), "bed");
    tire(&mut w, &defs, 0);
    run_until(&mut w, &defs, 400, |w| w.units[0].is_sleeping());
    assert_eq!(w.units[0].bed, Some(at(9, 4)));

    let text = w.save(&defs).unwrap();
    let mut loaded = World::load(&text, &defs).unwrap();
    assert_eq!(loaded.save(&defs).unwrap(), text);
    assert_eq!(loaded.units, w.units);
    assert!(loaded.heat[&at(8, 4)].is_permanent());
    // It carries on the same: still in its bed, still warm, waking at the same moment.
    for _ in 0..3000 {
        w.tick(&defs);
        loaded.tick(&defs);
    }
    assert_eq!(loaded.save(&defs).unwrap(), w.save(&defs).unwrap());
    assert!(loaded.is_heated(at(9, 4)));
}
