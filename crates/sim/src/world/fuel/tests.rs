use super::*;
use crate::defs::{BiomeId, Terrain};
use crate::geom::{Footprint, Grid};
use crate::mapgen::Tile;

fn field(width: i32, height: i32) -> TileMap {
    Grid::from_fn(width, height, |_| Tile {
        elevation: 100,
        temperature: 128,
        moisture: 128,
        terrain: Terrain::Land,
        biome: BiomeId(0),
        resource: None,
        building: None,
    })
}

fn at(x: i32, y: i32) -> Pos {
    Pos::new(x, y)
}

/// The bundled data with a test-only fuelled campfire: it burns wood, 1 a minute, from a bunker
/// of 4. Shipped campfires stay fuel-free (#672).
fn fuelled_defs() -> Defs {
    let mut files = crate::defs::BUNDLED_FILES;
    let old = "[building.heat]\nradius = 2\nsleep_bonus = 1.5\nseek_tiles = 20\n";
    assert!(files[7].contains(old), "the campfire's heat block moved");
    let buildings = files[7].replacen(
        old,
        &format!("{old}[building.fuel]\nitem = \"wood\"\nbunker = 4\nper_minute = 1.0\n"),
        1,
    );
    files[7] = &buildings;
    Defs::from_files(files).expect("test data is valid")
}

/// A 20x10 field, one unit at `unit`, and a finished fuelled campfire at (10, 5).
fn setup(unit: Pos) -> (World, Defs, Pos) {
    let defs = fuelled_defs();
    let mut w = World::with_map(1, field(20, 10), Nation::Earth, &[unit], &defs);
    let fire = at(10, 5);
    let id = defs.building_id("campfire").unwrap();
    w.raise(Footprint::single(fire), id, &defs);
    (w, defs, fire)
}

fn count(w: &World, pos: Pos) -> u32 {
    w.anchored(pos).and_then(|b| b.fuel).expect("a bunker").count
}

fn set_count(w: &mut World, pos: Pos, n: u32) {
    w.anchored_mut(pos).and_then(|b| b.fuel.as_mut()).unwrap().count = n;
}

fn refuel_jobs(w: &World) -> usize {
    w.jobs.iter().filter(|j| matches!(j.kind, JobKind::Refuel { .. })).count()
}

#[test]
fn fuel_is_validated() {
    let old = "[building.heat]\nradius = 2\nsleep_bonus = 1.5\nseek_tiles = 20\n";
    for (fuel, why) in [
        ("item = \"nope\"\nbunker = 4\nper_minute = 1.0", "unknown item"),
        ("item = \"wood\"\nbunker = 0\nper_minute = 1.0", "bunker"),
        ("item = \"wood\"\nbunker = 4\nper_minute = 0.0", "per_minute"),
        ("item = \"wood\"\nbunker = 4\nper_minute = 1.0\nextra = 1", "extra"),
        ("items = [\"wood\"]\nbunker = 4\nper_minute = 1.0", "at least two"),
        ("items = [\"wood\", \"nope\"]\nbunker = 4\nper_minute = 1.0", "unknown item"),
        ("items = [\"wood\", \"wood\"]\nbunker = 4\nper_minute = 1.0", "twice"),
        (
            "item = \"wood\"\nitems = [\"wood\", \"fish\"]\nbunker = 4\nper_minute = 1.0",
            "exactly one",
        ),
        ("bunker = 4\nper_minute = 1.0", "exactly one"),
    ] {
        let buildings = crate::defs::BUNDLED_FILES[7].replacen(
            old,
            &format!("{old}[building.fuel]\n{fuel}\n"),
            1,
        );
        let mut files = crate::defs::BUNDLED_FILES;
        files[7] = &buildings;
        let e = Defs::from_files(files).expect_err(fuel).0;
        assert!(e.contains(why), "{fuel}: {e}");
    }
    let defs = fuelled_defs();
    let fuel = defs.building(defs.building_id("campfire").unwrap()).fuel.clone().unwrap();
    assert_eq!((fuel.item, fuel.bunker), (defs.item_id("wood").unwrap(), 4));
    // 1 a minute at 10 ticks a second.
    assert_eq!(fuel.micros_per_tick, 1_667);
    // Among shipped buildings only the Oil Lamp (#678), the Hothouse (#680), the Balloon Dock
    // (#692) and the Stable (#696) burn anything;
    // campfires stay free (#672).
    let bundled = Defs::bundled();
    let burners: Vec<&str> =
        bundled.buildings.iter().filter(|b| b.fuel.is_some()).map(|b| b.id.as_str()).collect();
    assert_eq!(burners, ["oil_lamp", "hothouse", "balloon_dock", "stable"]);
}

#[test]
fn an_empty_fire_is_out_and_a_fuelled_one_is_lit() {
    let (mut w, _defs, fire) = setup(at(0, 0));
    assert_eq!(count(&w, fire), 0, "it finishes empty");
    assert!(!w.is_heated(fire));
    assert!(!w.is_cooking(at(11, 5)));
    set_count(&mut w, fire, 2);
    assert!(w.is_heated(fire));
    assert!(w.is_cooking(at(11, 5)));
}

#[test]
fn it_burns_only_while_someone_is_in_reach() {
    // The unit stands far from the fire: nothing burns.
    let (mut w, defs, fire) = setup(at(0, 0));
    set_count(&mut w, fire, 4);
    w.units[0].pos = at(0, 0);
    for _ in 0..600 {
        w.tick_fuel(&defs);
    }
    assert_eq!(count(&w, fire), 4);
    // Beside it, one minute burns one wood.
    w.units[0].pos = at(11, 5);
    for _ in 0..600 {
        w.tick_fuel(&defs);
    }
    assert_eq!(count(&w, fire), 3);
}

#[test]
fn it_goes_out_when_empty_and_is_refuelled_from_stores() {
    let (mut w, defs, fire) = setup(at(9, 5));
    set_count(&mut w, fire, 1);
    // A minute in reach burns the last one, and the fire goes out.
    for _ in 0..601 {
        w.units[0].pos = at(9, 5);
        w.tick_fuel(&defs);
    }
    assert_eq!(count(&w, fire), 0);
    assert!(!w.is_heated(fire));
    assert_eq!(refuel_jobs(&w), 1, "at or below half full, a Refuel job opens");
    // Wood in a stockpile: a settler fetches the shortfall and tips it in.
    let wood = defs.item_id("wood").unwrap();
    w.designate_stockpile(at(2, 2)).unwrap();
    w.stored.insert(at(2, 2), Stack { item: wood, count: 10 });
    for _ in 0..2_000 {
        w.tick(&defs);
        if count(&w, fire) == 4 {
            break;
        }
    }
    assert_eq!(count(&w, fire), 4);
    assert!(w.is_heated(fire));
    assert_eq!(w.stored_total(wood), 6, "the bunker's wood left stores");
}

#[test]
fn a_full_bunker_opens_no_job_and_a_half_full_one_does() {
    let (mut w, defs, fire) = setup(at(0, 0));
    w.jobs.retain(|j| !matches!(j.kind, JobKind::Refuel { .. }));
    set_count(&mut w, fire, 3);
    w.tick_fuel(&defs);
    assert_eq!(refuel_jobs(&w), 0);
    set_count(&mut w, fire, 2);
    w.tick_fuel(&defs);
    w.tick_fuel(&defs);
    assert_eq!(refuel_jobs(&w), 1, "one job, however many ticks");
}

#[test]
fn the_bunker_survives_a_save_round_trip() {
    let (mut w, defs, fire) = setup(at(9, 5));
    set_count(&mut w, fire, 3);
    for _ in 0..300 {
        w.tick_fuel(&defs);
    }
    let before = w.anchored(fire).unwrap().fuel.unwrap();
    assert!(before.burned > 0);
    let saved = w.save(&defs).expect("saves");
    let w2 = World::load(&saved, &defs).expect("loads");
    assert_eq!(w2.anchored(fire).unwrap().fuel.unwrap(), before);
}

#[test]
fn taking_it_down_returns_the_bunker_in_full() {
    let (mut w, defs, fire) = setup(at(9, 5));
    set_count(&mut w, fire, 3);
    let wood = defs.item_id("wood").unwrap();
    w.designate_deconstruct(fire, &defs).unwrap();
    for _ in 0..2_000 {
        w.tick(&defs);
        if w.anchored(fire).is_none() {
            break;
        }
    }
    assert!(w.anchored(fire).is_none());
    // The bunker's 3, plus the refund of its 3-wood bill.
    let refund = defs.deconstruct.refund_of(3);
    let on_ground: u32 = w.ground.values().filter(|s| s.item == wood).map(|s| s.count).sum();
    let carried: u32 =
        w.units.iter().filter_map(|u| u.carrying).filter(|s| s.item == wood).map(|s| s.count).sum();
    assert_eq!(on_ground + carried + w.stored_total(wood), 3 + refund);
}

/// A 20x10 Water field, one unit at `unit`, and a finished Oil Lamp (#678) at (10, 5).
fn lamp(unit: Pos) -> (World, Defs, Pos) {
    let defs = Defs::bundled();
    let mut w = World::with_map(1, field(20, 10), Nation::Water, &[unit], &defs);
    let pos = at(10, 5);
    w.raise(Footprint::single(pos), defs.building_id("oil_lamp").unwrap(), &defs);
    (w, defs, pos)
}

#[test]
fn the_oil_lamp_is_waters_and_burns_fish() {
    let defs = Defs::bundled();
    let id = defs.building_id("oil_lamp").unwrap();
    let def = defs.building(id);
    assert_eq!(def.unlocked_by, defs.tech_id("oil_lamps"));
    let tech = defs.tech(defs.tech_id("oil_lamps").unwrap());
    assert_eq!(tech.nation, Some(Nation::Water));
    assert!(!tech.core, "an optional horizons tech");
    assert_eq!(tech.prereqs, vec![defs.tech_id("fishing").unwrap()]);
    let fuel = def.fuel.as_ref().expect("the lamp has a bunker");
    assert_eq!(defs.item(fuel.item).id, "fish");
    assert!(def.cost.iter().all(|c| defs.item(c.item).id != "wood"), "no wood in its bill");
}

#[test]
fn waters_lamp_warms_a_seven_by_seven_square_only_while_fuelled() {
    let (mut w, _defs, pos) = lamp(at(0, 0));
    // Finished empty: it neither warms nor cooks.
    assert!(!w.is_heated(pos) && !w.is_cooking(at(11, 5)));
    set_count(&mut w, pos, 6);
    assert_eq!(w.heat_at(pos).unwrap().radius, 3, "radius 4 x Water's heating 0.8");
    for (p, warm) in [(at(13, 8), true), (at(7, 2), true), (at(14, 5), false), (at(10, 9), false)] {
        assert_eq!(w.is_heated(p), warm, "{p:?}");
    }
    assert!(w.is_cooking(at(12, 5)));
}

#[test]
fn the_lamp_burns_a_fish_every_four_minutes_only_while_someone_is_in_reach() {
    let (mut w, defs, pos) = lamp(at(0, 0));
    set_count(&mut w, pos, 6);
    for _ in 0..4 * 600 {
        w.tick_fuel(&defs);
    }
    assert_eq!(count(&w, pos), 6, "nobody in reach");
    w.units[0].pos = at(12, 6);
    for _ in 0..4 * 600 {
        w.tick_fuel(&defs);
    }
    assert_eq!(count(&w, pos), 5, "four minutes in reach burn one fish");
}
