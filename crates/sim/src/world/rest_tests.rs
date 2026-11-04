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
    w.light_building_fire(pos, [1, 1], def);
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

// ---- beds bigger than 1x1 (#227) ----

/// Defs where the starter "bed" is a solid 2x2 building with `capacity` sleeping places inside
/// it (#227), reached through its door, rather than 1x1 with one sleeper lying on its tile.
fn big_bed_defs(capacity: u32) -> Defs {
    let mut defs = Defs::bundled();
    let bed = defs.building_id("bed").unwrap();
    let def = &mut defs.buildings[bed.0 as usize];
    def.footprint = (2, 2);
    def.passable = false;
    def.capacity = capacity;
    defs
}

/// Places a finished building of `size` anchored at `anchor`, the way `World::raise` would.
fn place_big(w: &mut World, id: BuildingId, anchor: Pos, size: (u8, u8)) -> crate::geom::Footprint {
    let foot = crate::geom::Footprint::new(anchor, size.0, size.1);
    for p in foot.tiles() {
        let part = [(p.x - anchor.x) as u8, (p.y - anchor.y) as u8];
        w.map[p].building =
            Some(Building { part, size: [size.0, size.1], ..Building::new(id, false, false) });
    }
    w.map_changed();
    foot
}

#[test]
fn a_tired_unit_walks_through_the_door_of_a_bigger_bed_and_sleeps_inside() {
    let defs = big_bed_defs(1);
    let base = defs.needs.general.sleep_energy_per_minute;
    let bonus = defs.building(defs.building_id("bed").unwrap()).bed.unwrap().sleep_bonus;
    let mut w = world(Nation::Earth, &[at(2, 4)], &defs);
    let bed = defs.building_id("bed").unwrap();
    let foot = place_big(&mut w, bed, at(10, 4), (2, 2));
    tire(&mut w, &defs, 0);
    run_until(&mut w, &defs, 400, |w| w.units[0].is_sleeping());

    assert_eq!(w.units[0].inside(), Some(foot.anchor), "stepped inside, through the door");
    assert_eq!(w.units[0].pos, foot.entrance(), "pos stays on the door while inside");
    assert!(w.units_at(w.units[0].pos).next().is_none(), "hidden from the map while inside");
    assert_eq!(w.units[0].bed, Some(foot.anchor));

    let before = w.units[0].needs.energy;
    for _ in 0..600 {
        w.tick(&defs);
    }
    let gained = w.units[0].needs.energy - before;
    assert!((gained - base * bonus).abs() < 0.01, "{gained}");
}

#[test]
fn a_bigger_beds_capacity_caps_how_many_sleep_inside_at_once() {
    let defs = big_bed_defs(2);
    let bed = defs.building_id("bed").unwrap();
    let mut w = world(Nation::Earth, &[at(2, 3), at(2, 4), at(2, 5)], &defs);
    let foot = place_big(&mut w, bed, at(10, 4), (2, 2));
    for i in 0..3 {
        tire(&mut w, &defs, i);
    }
    run_until(&mut w, &defs, 1000, |w| w.units.iter().filter(|u| u.is_sleeping()).count() == 3);

    let inside: Vec<_> = w.units.iter().filter(|u| u.inside() == Some(foot.anchor)).collect();
    assert_eq!(inside.len(), 2, "only two places inside");
    let outside: Vec<_> = w.units.iter().filter(|u| u.inside().is_none()).collect();
    assert_eq!(outside.len(), 1, "the third sleeps where it is, on the ground");
    assert_eq!(
        w.occupants(foot.anchor).collect::<Vec<_>>(),
        inside.iter().map(|u| u.id).collect::<Vec<_>>()
    );
}

#[test]
fn a_sleeper_in_a_bigger_bed_steps_out_when_hungry_and_back_in_once_fed() {
    let defs = big_bed_defs(1);
    let bed = defs.building_id("bed").unwrap();
    let mut w = world(Nation::Earth, &[at(2, 4)], &defs);
    let foot = place_big(&mut w, bed, at(10, 4), (2, 2));
    tire(&mut w, &defs, 0);
    run_until(&mut w, &defs, 400, |w| w.units[0].is_sleeping());
    assert_eq!(w.units[0].inside(), Some(foot.anchor));

    // Food within reach, and hungry enough to cut sleep short (`sleep_interrupt_below`,
    // data/jobs.toml): `low` on its own would let it sleep through a little hunger.
    let fish = defs.item_id("fish").unwrap();
    w.ground.insert(at(0, 0), Stack { item: fish, count: 5 });
    w.units[0].needs.hunger = defs.jobs.sleep_interrupt_below - 1.0;
    run_until(&mut w, &defs, 400, |w| matches!(w.units[0].task(), Some(Task::Eat(_))));
    assert!(w.units[0].inside().is_none(), "stepped out before eating (#226)");

    // Tired again once fed: it walks back in and re-claims a place inside.
    run_until(&mut w, &defs, 2000, |w| w.units[0].is_sleeping());
    assert_eq!(w.units[0].inside(), Some(foot.anchor), "back inside the same bed");
}

#[test]
fn a_sleeper_in_a_building_that_keeps_its_own_fire_is_warmed_through_the_door() {
    let mut defs = big_bed_defs(1);
    let bed = defs.building_id("bed").unwrap();
    {
        let def = &mut defs.buildings[bed.0 as usize];
        def.heat = Some(crate::building::HeatDef { radius: 2, sleep_bonus: 1.5, seek_tiles: 20 });
    }
    let base = defs.needs.general.sleep_energy_per_minute;
    let bed_bonus = defs.building(bed).bed.unwrap().sleep_bonus;
    let heat_bonus = defs.building(bed).heat.unwrap().sleep_bonus;
    let mut w = world(Nation::Earth, &[at(2, 4)], &defs);
    let foot = place_big(&mut w, bed, at(10, 4), (2, 2));
    w.light_building_fire(foot.anchor, [2, 2], defs.building(bed));
    tire(&mut w, &defs, 0);
    run_until(&mut w, &defs, 400, |w| w.units[0].is_sleeping());
    assert_eq!(w.units[0].inside(), Some(foot.anchor));

    // The door, where `pos` stays while inside, is on the fire's own footprint ring: it is
    // warmed the same way it already would be from outside (#227).
    assert!(w.is_heated(w.units[0].pos));
    assert!((w.sleep_factor_for(&w.units[0], &defs) - bed_bonus * heat_bonus).abs() < 0.001);

    let before = w.units[0].needs.energy;
    for _ in 0..600 {
        w.tick(&defs);
    }
    let gained = w.units[0].needs.energy - before;
    assert!((gained - base * bed_bonus * heat_bonus).abs() < 0.01, "{gained}");
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

#[test]
fn a_bigger_heat_building_warms_its_radius_out_from_every_side() {
    let defs = Defs::bundled();
    let mut w = world(Nation::Fire, &[at(1, 1)], &defs);
    let id = defs.building_id("campfire").unwrap();
    let def = defs.building(id);
    let anchor = at(8, 3);
    let size = [2, 2];
    for (dx, dy) in [(0, 0), (1, 0), (0, 1), (1, 1)] {
        let part = [dx as u8, dy as u8];
        w.map[at(anchor.x + dx, anchor.y + dy)].building =
            Some(Building { part, size, ..Building::new(id, def.passable, false) });
    }
    w.light_building_fire(anchor, size, def);
    let r = def.heat.unwrap().radius_for(w.modifiers.heating) as i32;
    // Right of the footprint's right column, and below its bottom row: warm out to r.
    for p in [at(anchor.x + 1 + r, anchor.y + 1), at(anchor.x + 1, anchor.y + 1 + r)] {
        assert!(w.is_heated(p), "{p:?}");
        assert!(w.sleep_factor_at(p) > 1.0, "{p:?}");
    }
    for p in [at(anchor.x + 2 + r, anchor.y + 1), at(anchor.x - 1 - r, anchor.y)] {
        assert!(!w.is_heated(p), "{p:?}");
        assert_eq!(w.sleep_factor_at(p), 1.0, "{p:?}");
    }

    // A tired unit walks only as far as the edge of the warmth, measured from the footprint.
    tire(&mut w, &defs, 0);
    let route = w.warm_sleep_route(0).expect("the fire is in reach");
    let end = route.first().copied().unwrap_or(w.units[0].pos);
    let foot = crate::geom::Footprint::new(anchor, 2, 2);
    assert_eq!(foot.distance(end), r, "stopped at {end:?}");
}

#[test]
fn a_heat_sources_size_is_saved_only_when_bigger_than_one_tile() {
    let defs = Defs::bundled();
    let mut w = world(Nation::Fire, &[at(1, 1)], &defs);
    let def = defs.building(defs.building_id("campfire").unwrap());
    w.light_building_fire(at(4, 4), [1, 1], def);
    assert!(!w.save(&defs).unwrap().contains("size"));
    w.light_building_fire(at(10, 4), [2, 2], def);
    let loaded = World::load(&w.save(&defs).unwrap(), &defs).unwrap();
    assert_eq!(loaded.heat[&at(10, 4)].size, [2, 2]);
    assert_eq!(loaded.heat[&at(4, 4)].size, [1, 1]);
}
