use super::*;
use crate::building::BuildingId;
use crate::defs::{BiomeId, Terrain};
use crate::geom::{Footprint, Grid};
use crate::mapgen::Tile;
use crate::world::{Blocker, BuildError, FoodSource, Task};

fn field() -> TileMap {
    Grid::from_fn(50, 12, |_| Tile {
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

/// Where the Waystation stands: its door is (30, 5).
const ANCHOR: Pos = Pos::new(30, 3);
const DOOR: Pos = Pos::new(30, 5);
/// A stockpile far to the west, 28 tiles from the Waystation.
const STORES: Pos = Pos::new(2, 2);

fn waystation(defs: &Defs) -> BuildingId {
    defs.building_id("waystation").unwrap()
}

/// A 50x12 field, units at `units`, a stockpile at `STORES` holding `stored`, and a finished
/// Waystation at `ANCHOR`.
fn setup(units: &[Pos], stored: Option<(&str, u32)>) -> (World, Defs) {
    let defs = Defs::bundled();
    let mut w = World::with_map(1, field(), Nation::Earth, units, &defs);
    w.designate_stockpile(STORES).unwrap();
    if let Some((item, count)) = stored {
        w.stored.insert(STORES, Stack { item: defs.item_id(item).unwrap(), count });
    }
    w.raise(Footprint::new(ANCHOR, 2, 2), waystation(&defs), &defs);
    (w, defs)
}

fn stock_pantry(w: &mut World, defs: &Defs, item: &str, count: u32) {
    let item = defs.item_id(item).unwrap();
    let pantry = w.anchored_mut(ANCHOR).and_then(|b| b.pantry.as_mut()).expect("a pantry");
    pantry.stacks.push(Stack { item, count });
}

fn pantry_total(w: &World) -> u32 {
    w.anchored(ANCHOR).and_then(|b| b.pantry()).map_or(0, Pantry::total)
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

#[test]
fn the_waystation_is_a_shared_tech_with_a_pantry_two_beds_and_a_barrel() {
    let defs = Defs::bundled();
    let id = waystation(&defs);
    let def = defs.building(id);
    let tech = defs.tech(defs.tech_id("waystations").unwrap());
    assert_eq!((tech.nation, tech.cost, tech.core), (None, 250, false));
    assert_eq!(tech.prereqs, [defs.tech_id("cartography").unwrap()]);
    assert_eq!(def.unlocked_by, defs.tech_id("waystations"));
    assert_eq!((def.footprint, def.capacity, def.passable), ((2, 2), 2, false));
    assert_eq!(def.bed.map(|b| (b.sleep_bonus, b.seek_tiles)), Some((1.2, 30)));
    let outpost = def.outpost.as_ref().expect("an outpost");
    let food: Vec<_> = ["cabbage", "fruit", "fish"].map(|i| defs.item_id(i).unwrap()).to_vec();
    assert_eq!(
        *outpost,
        OutpostDef { far_from_stores: 20, pantry: 12, pantry_items: food, barrel: true }
    );
    // The decision on #688: no general store, only a pantry.
    assert_eq!(def.fuel, None);
    let others = defs.buildings.iter().filter(|b| b.outpost.is_some()).count();
    assert_eq!(others, 1, "only the Waystation uses it for now");
}

#[test]
fn outposts_are_validated() {
    let old = "pantry_items = [\"cabbage\", \"fruit\", \"fish\"]";
    for (new, why) in [
        ("pantry_items = [\"nope\"]", "unknown item"),
        ("pantry_items = [\"wood\"]", "not a food"),
        ("pantry_items = [\"fish\", \"fish\"]", "twice"),
        ("pantry_items = []", "both a pantry size"),
        ("pantry_items = [\"fish\"]\nextra = 1", "extra"),
    ] {
        let buildings = crate::defs::BUNDLED_FILES[7].replacen(old, new, 1);
        let mut files = crate::defs::BUNDLED_FILES;
        files[7] = &buildings;
        let e = Defs::from_files(files).expect_err(new).0;
        assert!(e.contains(why), "{new}: {e}");
    }
    // A pantry or a barrel is served from a door, so it needs a capacity.
    let buildings = crate::defs::BUNDLED_FILES[7].replacen("capacity = 2           #", "#", 1);
    let mut files = crate::defs::BUNDLED_FILES;
    files[7] = &buildings;
    let e = Defs::from_files(files).expect_err("no capacity").0;
    assert!(e.contains("capacity"), "{e}");
}

#[test]
fn it_is_refused_within_twenty_tiles_of_a_stockpile() {
    let (mut w, defs) = setup(&[at(0, 0)], None);
    let id = waystation(&defs);
    w.research.grant(defs.tech_id("waystations").unwrap());
    // The stockpile is at x = 2: a footprint whose nearest tile is 19 away is too close.
    let near = at(21, 3);
    assert_eq!(
        w.check_build(near, id, &defs),
        Err(BuildError::CannotBuild { at: near, why: Blocker::NearStores(20) })
    );
    assert_eq!(Blocker::NearStores(20).to_string(), "must be 20 tiles from any stockpile");
    // 20 away is fine, and so is any distance on the other axis.
    assert_eq!(w.check_build(at(22, 3), id, &defs), Ok(()));
    w.designate_stockpile(at(49, 11)).unwrap();
    assert_eq!(w.check_build(at(22, 3), id, &defs), Ok(()), "a stockpile 26 east, 7 south");
    w.designate_stockpile(at(30, 11)).unwrap();
    let e = w.check_build(at(22, 3), id, &defs).expect_err("7 tiles east and south");
    assert!(matches!(e, BuildError::CannotBuild { why: Blocker::NearStores(20), .. }), "{e:?}");
}

#[test]
fn the_rule_is_checked_only_when_it_is_placed() {
    let (mut w, defs) = setup(&[at(0, 0)], None);
    w.designate_stockpile(at(33, 5)).unwrap();
    let sheet = w.save(&defs).expect("saves");
    let w2 = World::load(&sheet, &defs).expect("loads");
    assert!(w2.anchored(ANCHOR).is_some(), "a stockpile drawn beside it later is allowed");
}

#[test]
fn a_hungry_unit_nearby_eats_from_the_pantry() {
    let (mut w, defs) = setup(&[at(33, 6)], Some(("cabbage", 20)));
    stock_pantry(&mut w, &defs, "fish", 3);
    w.units[0].needs.hunger = defs.needs.hunger.low - 1.0;
    w.tick(&defs);
    assert_eq!(w.units[0].task(), Some(Task::Eat(FoodSource::Pantry(ANCHOR))));
    assert_eq!(w.units[0].route.as_ref().and_then(|r| r.first()), Some(&DOOR));
    run_until(&mut w, &defs, 500, |w| w.units[0].task().is_none());
    assert_eq!(pantry_total(&w), 2);
    assert!(w.units[0].needs.hunger > defs.needs.hunger.low + 10.0, "{:?}", w.units[0].needs);
    assert_eq!(w.stored_total(defs.item_id("cabbage").unwrap()), 20, "stores untouched");
}

#[test]
fn pantry_food_is_not_counted_in_stores() {
    let (mut w, defs) = setup(&[at(33, 6)], None);
    stock_pantry(&mut w, &defs, "fish", 6);
    let fish = defs.item_id("fish").unwrap();
    assert_eq!(w.stored_total(fish), 0);
    assert_eq!(w.pantry_at(ANCHOR, &defs).map(|(p, size)| (p.total(), size)), Some((6, 12)));
}

#[test]
fn an_empty_pantry_falls_back_to_stores() {
    let (mut w, defs) = setup(&[at(33, 6)], Some(("cabbage", 20)));
    w.units[0].needs.hunger = defs.needs.hunger.low - 1.0;
    w.tick(&defs);
    assert_eq!(w.units[0].task(), Some(Task::Eat(FoodSource::Stockpile(STORES))));
}

#[test]
fn a_thirsty_unit_nearby_drinks_at_the_barrel_and_is_never_made_sick() {
    // No water anywhere on the map: the barrel is the only place to drink.
    let (mut w, defs) = setup(&[at(33, 6)], None);
    w.difficulty = Some("normal".into());
    w.units[0].needs.thirst = defs.needs.thirst.low - 1.0;
    w.tick(&defs);
    assert_eq!(w.units[0].task(), Some(Task::Drink));
    assert_eq!(w.units[0].route.as_ref().and_then(|r| r.first()), Some(&DOOR));
    run_until(&mut w, &defs, 500, |w| w.units[0].task().is_none());
    assert!(w.units[0].needs.thirst > defs.needs.thirst.low + 10.0, "{:?}", w.units[0].needs);
    assert!(w.is_barrel_door(DOOR, &defs));
    assert!(!w.is_barrel_door(at(31, 5), &defs));
    assert_eq!(w.sickness_chance(DOOR, &defs), None);
}

#[test]
fn two_tired_units_sleep_inside_and_a_third_does_not_fit() {
    let (mut w, defs) = setup(&[at(33, 6), at(34, 6), at(35, 6)], None);
    for u in &mut w.units {
        u.needs.energy = defs.needs.energy.low - 1.0;
    }
    run_until(&mut w, &defs, 600, |w| {
        w.units.iter().filter(|u| u.inside == Some(ANCHOR)).count() >= 2
    });
    let inside: Vec<_> = w.units.iter().filter(|u| u.bed == Some(ANCHOR)).collect();
    assert_eq!(inside.len(), 2, "two sleeping places");
    assert!(inside.iter().all(|u| u.inside == Some(ANCHOR)), "inside, not on the ground");
}

#[test]
fn a_half_empty_pantry_is_refilled_by_a_refuel_haul() {
    let (mut w, defs) = setup(&[at(5, 3)], Some(("cabbage", 30)));
    stock_pantry(&mut w, &defs, "fish", 6);
    let refuels =
        |w: &World| w.jobs.iter().filter(|j| matches!(j.kind, JobKind::Refuel { .. })).count();
    w.tick(&defs);
    assert_eq!(refuels(&w), 1, "at half, a Refuel job opens");
    run_until(&mut w, &defs, 4_000, |w| pantry_total(w) > 6);
    let cabbage = defs.item_id("cabbage").unwrap();
    let pantry = w.anchored(ANCHOR).unwrap().pantry().unwrap();
    assert!(pantry.count_of(cabbage) > 0, "{pantry:?}");
    assert!(pantry.total() <= 12);
    assert_eq!(w.stored_total(cabbage), 30 - pantry.count_of(cabbage), "it left stores");
}

#[test]
fn a_pantry_above_half_opens_no_job() {
    let (mut w, defs) = setup(&[at(0, 0)], Some(("cabbage", 30)));
    stock_pantry(&mut w, &defs, "fish", 7);
    w.jobs.retain(|j| !matches!(j.kind, JobKind::Refuel { .. }));
    w.tick(&defs);
    assert!(!w.jobs.iter().any(|j| matches!(j.kind, JobKind::Refuel { .. })));
}

#[test]
fn taking_it_down_returns_the_pantry() {
    let (mut w, defs) = setup(&[at(33, 6)], None);
    stock_pantry(&mut w, &defs, "fish", 5);
    let fish = defs.item_id("fish").unwrap();
    w.designate_deconstruct(ANCHOR, &defs).unwrap();
    run_until(&mut w, &defs, 4_000, |w| w.anchored(ANCHOR).is_none());
    let on_ground: u32 = w.ground.values().filter(|s| s.item == fish).map(|s| s.count).sum();
    let carried: u32 =
        w.units.iter().filter_map(|u| u.carrying).filter(|s| s.item == fish).map(|s| s.count).sum();
    assert_eq!(on_ground + carried + w.stored_total(fish), 5);
}

#[test]
fn the_pantry_survives_a_save_round_trip() {
    let (mut w, defs) = setup(&[at(33, 6)], None);
    stock_pantry(&mut w, &defs, "fish", 4);
    stock_pantry(&mut w, &defs, "fruit", 2);
    let saved = w.save(&defs).expect("saves");
    let w2 = World::load(&saved, &defs).expect("loads");
    assert_eq!(w2.anchored(ANCHOR).unwrap().pantry(), w.anchored(ANCHOR).unwrap().pantry());
}

#[test]
fn a_crew_at_a_waystation_is_deterministic() {
    let run = || {
        let (mut w, defs) = setup(&[at(33, 6), at(34, 6)], Some(("cabbage", 30)));
        stock_pantry(&mut w, &defs, "fish", 6);
        w.units[0].needs.hunger = defs.needs.hunger.low - 1.0;
        w.units[1].needs.thirst = defs.needs.thirst.low - 1.0;
        for _ in 0..1_500 {
            w.tick(&defs);
        }
        w.save(&defs).expect("saves")
    };
    assert_eq!(run(), run());
}
