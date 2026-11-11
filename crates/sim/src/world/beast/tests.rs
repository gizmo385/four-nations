use super::*;
use crate::defs::{BiomeId, Terrain};
use crate::geom::Grid;
use crate::jobs::ItemId;
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

/// The stable's anchor in every test: its door is (5, 8).
const STABLE: Pos = Pos::new(4, 6);
const DOOR: Pos = Pos::new(4, 8);

/// The bundled data, with the Bison Stable burning `fish` or `wood` (6 a minute) when `fed`.
fn defs_with(fed: bool) -> Defs {
    if fed { slow_fuel_defs(6.0) } else { Defs::bundled() }
}

/// The bundled data, with the Bison Stable burning `fish` or `wood`, `per_minute` of them.
fn slow_fuel_defs(per_minute: f32) -> Defs {
    let mut files = crate::defs::BUNDLED_FILES;
    let old = "id = \"bison_stable\"\nname = \"Bison Stable\"\nverb = \"Build\"\nwork = 160.0\n\
               passable = false\nfootprint = [2, 2]\n";
    assert!(files[7].contains(old), "the Bison Stable block moved");
    let buildings = files[7].replacen(
        old,
        &format!(
            "{old}[building.fuel]\nitems = [\"fish\", \"wood\"]\nbunker = 6\n\
             per_minute = {per_minute:?}\n"
        ),
        1,
    );
    files[7] = &buildings;
    Defs::from_files(files)
        .and_then(|d| d.with_beasts(crate::defs::BEASTS_FILE))
        .expect("test data is valid")
}

/// A 70x12 field with `settlers`, a four-tile stockpile at (2..=5, 2), and, if `stable`, a
/// finished Bison Stable at `STABLE`. The world is Air's.
fn setup(settlers: &[Pos], stable: bool, defs: &Defs) -> World {
    let mut w = World::with_map(1, field(70, 12), Nation::Air, settlers, defs);
    for x in 2..=5 {
        w.designate_stockpile(at(x, 2)).unwrap();
    }
    if stable {
        w.raise(Footprint::new(STABLE, 2, 2), defs.building_id("bison_stable").unwrap(), defs);
    }
    w
}

fn beast(w: &World) -> &Unit {
    w.beasts().next().expect("a beast")
}

fn item(defs: &Defs, id: &str) -> ItemId {
    defs.item_id(id).unwrap()
}

#[test]
fn a_finished_home_puts_one_beast_at_its_door() {
    let defs = Defs::bundled();
    let mut w = setup(&[at(0, 0)], true, &defs);
    assert_eq!(w.beasts().count(), 0, "it arrives with the next tick");
    w.tick(&defs);
    let b = beast(&w);
    assert_eq!((b.name.as_str(), b.pos), ("Sky Bison", DOOR));
    assert_eq!(
        b.beast.map(|x| (x.id, x.home)),
        Some((defs.beast_id("sky_bison").unwrap(), STABLE))
    );
    assert!(w.beast_housed_at(STABLE).is_some());
    assert!(w.units.windows(2).all(|p| p[0].id < p[1].id), "units stay sorted by id");
    for _ in 0..50 {
        w.tick(&defs);
    }
    assert_eq!(w.beasts().count(), 1, "one per stable, however long it stands");
    // Other nations' settlements house nothing in it: the row is Air's.
    let mut other = World::with_map(1, field(70, 12), Nation::Fire, &[at(0, 0)], &defs);
    other.raise(Footprint::new(STABLE, 2, 2), defs.building_id("bison_stable").unwrap(), &defs);
    other.tick(&defs);
    assert_eq!(other.beasts().count(), 0);
}

#[test]
fn a_beast_is_not_a_person() {
    let defs = Defs::bundled();
    let mut w = setup(&[at(0, 0), at(1, 0)], true, &defs);
    w.tick(&defs);
    let b = beast(&w).clone();
    assert!(b.is_beast() && !b.is_settler() && !b.is_avatar());
    assert_eq!(w.units.len(), 3);
    assert_eq!(w.population(), 2, "the headcount leaves it out");
    let signs = w.endeavor_signs(&defs);
    let pop = signs.iter().find(|s| s.id == "population");
    if let Some(pop) = pop {
        assert_eq!(pop.value, 2, "no sign counts it");
    }
    // Ten minutes: no hunger, thirst, energy or health moves, it casts nothing, and it dies of
    // nothing, however long it goes.
    for _ in 0..6_000 {
        w.tick(&defs);
        w.assert_invariants(&defs);
    }
    let after = beast(&w);
    assert_eq!(after.needs, b.needs);
    assert_eq!(after.chi, 0.0);
    assert_eq!(w.deaths().count(), 0);
    let ability = defs.abilities.iter().position(|a| a.nation == Nation::Air).unwrap();
    assert!(!w.can_cast(after, crate::bending::AbilityId(ability as u16), &defs));
    // Growth counts people, not it: a bed per settler plus one leaves room for exactly one.
    assert_eq!(w.units.iter().filter(|u| u.is_settler()).count(), 2);
}

#[test]
fn taking_the_home_down_sends_the_beast_back_to_the_wild() {
    let defs = Defs::bundled();
    let mut w = setup(&[at(0, 0)], true, &defs);
    w.tick(&defs);
    let wood = item(&defs, "wood");
    // It is carrying something when the stable comes down: that is dropped where it stands.
    let i = w.units.iter().position(Unit::is_beast).unwrap();
    w.units[i].carrying = Some(Stack { item: wood, count: 7 });
    let key = w.building_record(STABLE).unwrap();
    let key = w.map[key.anchor].building.unwrap().key();
    w.remove_building(key, &defs).expect("it stood");
    w.tick(&defs);
    assert_eq!(w.beasts().count(), 0);
    let events: Vec<String> = w.beast_events().map(BeastEvent::describe).collect();
    assert_eq!(events, ["The sky bison returns to the wild."]);
    let dropped: u32 = w.ground.values().filter(|s| s.item == wood).map(|s| s.count).sum();
    assert_eq!(dropped, 7);
    assert!(w.units.windows(2).all(|p| p[0].id < p[1].id));
    w.assert_invariants(&defs);
    // Rebuilt, it comes again.
    w.raise(Footprint::new(STABLE, 2, 2), defs.building_id("bison_stable").unwrap(), &defs);
    w.tick(&defs);
    assert_eq!(w.beasts().count(), 1);
}

#[test]
fn a_hauling_beast_takes_only_trips_of_at_least_min_trip() {
    let defs = Defs::bundled();
    // No settlers: whatever moves is the beast.
    let mut w = setup(&[], true, &defs);
    let wood = item(&defs, "wood");
    // 4 tiles from the stockpile (Chebyshev): a settler's trip.
    w.place_ground(at(9, 5), Stack { item: wood, count: 20 });
    for _ in 0..300 {
        w.tick(&defs);
        assert!(beast(&w).job_task.is_none(), "a short haul is not for the beast");
    }
    assert_eq!(w.stored_total(wood), 0);
    // 10 tiles from the nearest stockpile tile (5, 2): exactly the minimum.
    w.place_ground(at(15, 4), Stack { item: wood, count: 20 });
    let mut took = false;
    for _ in 0..1_500 {
        w.tick(&defs);
        took |= beast(&w).job_task.is_some();
        if w.stored_total(wood) == 20 {
            break;
        }
    }
    assert!(took, "a trip of 10 is the beast's");
    assert_eq!(w.stored_total(wood), 20, "it carried a full 20 in one go");
    assert!(w.ground.contains_key(&at(9, 5)), "the short stack was left alone");
    // It walks back to the stable's door and stands there.
    for _ in 0..600 {
        w.tick(&defs);
    }
    assert_eq!(beast(&w).pos, DOOR);
    assert!(beast(&w).task().is_none());
}

/// #205: a bison measurably speeds up hauling between two far points.
#[test]
fn a_bison_speeds_up_a_far_haul() {
    let defs = Defs::bundled();
    let wood = item(&defs, "wood");
    let ticks_to_store = |stable: bool| -> u64 {
        let mut w = setup(&[at(1, 4), at(2, 4)], stable, &defs);
        w.place_ground(at(62, 8), Stack { item: wood, count: 60 });
        for _ in 0..12_000 {
            w.tick(&defs);
            if w.stored_total(wood) == 60 {
                return w.tick;
            }
        }
        panic!("the wood never arrived (stable: {stable})");
    };
    let walking = ticks_to_store(false);
    let with_bison = ticks_to_store(true);
    // Settlers carry 5 a trip and the bison 20 at the same Air speed: well under 2/3 the time.
    assert!(
        with_bison * 3 < walking * 2,
        "with a bison {with_bison} ticks against {walking} walking"
    );
}

#[test]
fn a_beast_uses_its_row_s_speed_or_the_nation_s() {
    let defs = Defs::bundled();
    let mut w = setup(&[at(0, 0)], true, &defs);
    w.tick(&defs);
    let b = beast(&w).clone();
    assert_eq!(w.unit_speed(&b, &defs), w.modifiers().move_speed, "no speed: the nation's own");
    let slow = crate::defs::BEASTS_FILE
        .replace("home = \"bison_stable\"", "home = \"bison_stable\"\nspeed = 0.5");
    let slow_defs = Defs::bundled().with_beasts(&slow).unwrap();
    assert_eq!(w.unit_speed(&b, &slow_defs), 0.5);
}

#[test]
fn a_home_with_fuel_burns_while_it_houses_a_beast() {
    let defs = defs_with(true);
    let fish = item(&defs, "fish");
    let mut w = setup(&[at(0, 0)], true, &defs);
    w.tick(&defs);
    let set = |w: &mut World, n: u32| {
        let b = w.anchored_mut(STABLE).and_then(|b| b.fuel.as_mut()).unwrap();
        b.count = n;
        b.item = Some(fish);
    };
    let count = |w: &World| w.anchored(STABLE).unwrap().fuel.unwrap().count;
    set(&mut w, 6);
    // 6 a minute, nobody near it: it still burns, for the beast living there.
    for _ in 0..600 {
        w.tick(&defs);
    }
    assert_eq!(count(&w), 0);
    assert!(w.beast_hungry(beast(&w), &defs));
    // Taking the beast away (the unit alone) stops the burn.
    set(&mut w, 6);
    let i = w.units.iter().position(Unit::is_beast).unwrap();
    w.units.remove(i);
    for _ in 0..300 {
        w.tick_fuel(&defs);
    }
    assert_eq!(count(&w), 6);
}

#[test]
fn a_hungry_beast_rests_at_home_and_works_nothing_until_fed() {
    let defs = defs_with(true);
    let wood = item(&defs, "wood");
    let mut w = setup(&[], true, &defs);
    w.place_ground(at(40, 8), Stack { item: wood, count: 60 });
    w.tick(&defs);
    assert!(w.beast_hungry(beast(&w), &defs), "the bunker finishes empty");
    for _ in 0..400 {
        w.tick(&defs);
        let b = beast(&w);
        assert!(b.job_task.is_none() && b.carrying.is_none(), "it works nothing while hungry");
        assert_eq!(b.pos, DOOR);
    }
    assert!(w.ground.contains_key(&at(40, 8)), "the wood is where it was");
    // Never dies or leaves: still there, still all its own.
    assert_eq!(w.beasts().count(), 1);
    assert_eq!(w.deaths().count(), 0);
    // Fed, it goes to work.
    let bunker = w.anchored_mut(STABLE).and_then(|b| b.fuel.as_mut()).unwrap();
    bunker.count = 6;
    bunker.item = Some(item(&defs, "fish"));
    let mut worked = false;
    for _ in 0..200 {
        w.tick(&defs);
        worked |= beast(&w).job_task.is_some();
    }
    assert!(worked, "fed, it hauls again");
}

#[test]
fn a_refuel_brings_whichever_listed_item_stores_hold_the_most_of() {
    let defs = defs_with(true);
    let (fish, wood) = (item(&defs, "fish"), item(&defs, "wood"));
    let mut w = setup(&[], true, &defs);
    w.tick(&defs);
    let shortfall = |w: &World| w.fuel_shortfall(STABLE, &defs).unwrap();
    // Nothing on hand: the first listed.
    assert_eq!(shortfall(&w), (fish, 6));
    w.stored.insert(at(2, 2), Stack { item: wood, count: 9 });
    assert_eq!(shortfall(&w), (wood, 6), "only wood is on hand");
    w.stored.insert(at(3, 2), Stack { item: fish, count: 9 });
    assert_eq!(shortfall(&w), (fish, 6), "a tie goes to the list's order");
    w.stored.get_mut(&at(2, 2)).unwrap().count = 10;
    assert_eq!(shortfall(&w), (wood, 6), "the most plentiful wins");
    w.ground.insert(at(30, 5), Stack { item: fish, count: 5 });
    assert_eq!(shortfall(&w), (fish, 6), "ground stacks count as stores");
}

#[test]
fn a_hauling_beast_stocks_its_own_bunker_from_far_stores() {
    let defs = defs_with(true);
    let wood = item(&defs, "wood");
    let mut w = setup(&[], true, &defs);
    // Stores 26 tiles from the stable: past the 10-tile minimum.
    w.designate_stockpile(at(30, 8)).unwrap();
    w.stored.insert(at(30, 8), Stack { item: wood, count: 20 });
    w.tick(&defs);
    // Hungry, it would rest and leave the trip to a settler; with some left it goes itself.
    w.anchored_mut(STABLE).and_then(|b| b.fuel.as_mut()).unwrap().count = 3;
    let mut fed = false;
    for _ in 0..3_000 {
        w.tick(&defs);
        let bunker = w.anchored(STABLE).unwrap().fuel.unwrap();
        if bunker.count >= 5 {
            assert_eq!(bunker.item, Some(wood));
            fed = true;
            break;
        }
    }
    assert!(fed, "the beast fetched the wood itself");
    assert!(!w.beast_hungry(beast(&w), &defs));
}

#[test]
fn a_mixed_bunker_gives_back_the_item_last_tipped_in() {
    let defs = slow_fuel_defs(0.1);
    let wood = item(&defs, "wood");
    let mut w = setup(&[at(5, 9)], true, &defs);
    let b = w.anchored_mut(STABLE).and_then(|b| b.fuel.as_mut()).unwrap();
    b.count = 3;
    b.item = Some(wood);
    w.designate_deconstruct(STABLE, &defs).unwrap();
    for _ in 0..3_000 {
        w.tick(&defs);
        if w.anchored(STABLE).is_none() {
            break;
        }
    }
    assert!(w.anchored(STABLE).is_none(), "it came down");
    // Dropped by the anchor, then hauled to stores by the settler.
    for _ in 0..600 {
        w.tick(&defs);
    }
    // The refund of the stable's own 20 wood comes with it.
    let refund = defs.deconstruct.refund_of(20);
    let loose: u32 = w.ground.values().filter(|s| s.item == wood).map(|s| s.count).sum();
    let carried: u32 =
        w.units.iter().filter_map(|u| u.carrying).filter(|s| s.item == wood).map(|s| s.count).sum();
    assert_eq!(
        w.stored_total(wood) + loose + carried,
        refund + 3,
        "the bunker came back as the wood tipped in"
    );
}

#[test]
fn a_beast_survives_a_save_round_trip() {
    let defs = defs_with(true);
    let wood = item(&defs, "wood");
    let mut w = setup(&[at(0, 0)], true, &defs);
    w.place_ground(at(40, 8), Stack { item: wood, count: 40 });
    w.tick(&defs);
    let b = w.anchored_mut(STABLE).and_then(|b| b.fuel.as_mut()).unwrap();
    b.count = 4;
    b.item = Some(wood);
    for _ in 0..250 {
        w.tick(&defs);
    }
    assert!(beast(&w).job_task.is_some(), "saved mid-job");
    let text = w.save(&defs).expect("saves");
    let loaded = World::load(&text, &defs).expect("loads");
    assert_eq!(loaded.units, w.units);
    assert_eq!(loaded.anchored(STABLE).unwrap().fuel, w.anchored(STABLE).unwrap().fuel);
    // It carries on the same way.
    let (mut a, mut b) = (w, loaded);
    for _ in 0..600 {
        a.tick(&defs);
        b.tick(&defs);
    }
    assert_eq!(a.units, b.units);
    assert_eq!(a.stored_total(wood), b.stored_total(wood));
}

#[test]
fn a_save_made_before_beasts_gets_its_stable_s_beast() {
    let defs = Defs::bundled();
    let mut w = setup(&[at(0, 0)], true, &defs);
    // A stable that stood before beasts existed: no beast, as in an old save.
    assert_eq!(w.beasts().count(), 0);
    let text = w.save(&defs).expect("saves");
    let mut loaded = World::load(&text, &defs).expect("loads");
    loaded.tick(&defs);
    assert_eq!(loaded.beasts().count(), 1);
    w.tick(&defs);
    assert_eq!(w.units, loaded.units);
}

#[test]
fn a_save_that_names_a_missing_beast_is_refused() {
    let defs = Defs::bundled();
    let mut w = setup(&[at(0, 0)], true, &defs);
    w.tick(&defs);
    let text = w.save(&defs).expect("saves");
    let without = Defs::from_files(crate::defs::BUNDLED_FILES).unwrap();
    assert!(World::load(&text, &without).is_err(), "the beast's row is gone");
}
