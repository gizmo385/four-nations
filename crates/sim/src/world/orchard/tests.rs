use super::*;
use crate::building::Building;
use crate::defs::{BiomeId, Terrain};
use crate::geom::{Footprint, Grid};
use crate::jobs::JobKind;
use crate::mapgen::Tile;

fn arena(size: i32) -> TileMap {
    Grid::from_fn(size, size, |_| Tile {
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

const SIZE: i32 = 15;
const MID: Pos = Pos::new(7, 7);

fn setup(nation: Nation, seed: u64, units: &[Pos]) -> (World, Defs) {
    let defs = Defs::bundled();
    (World::with_map(seed, arena(SIZE), nation, units, &defs), defs)
}

fn orchard_def(defs: &Defs) -> OrchardDef {
    defs.building(defs.building_id("orchard").unwrap()).orchard.expect("the orchard plants")
}

/// Puts a finished orchard on `pos`, as if built.
fn raise_orchard(w: &mut World, defs: &Defs, pos: Pos) {
    let id = defs.building_id("orchard").unwrap();
    let def = defs.building(id);
    w.map[pos].building = Some(Building::new(id, def.passable, false));
    w.open_orchard(pos, def.orchard.as_ref().unwrap());
}

fn secs(s: f32) -> u64 {
    (s * TICKS_PER_SECOND as f32).ceil() as u64
}

fn run(w: &mut World, defs: &Defs, ticks: u64) {
    for _ in 0..ticks {
        w.tick(defs);
    }
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

fn fruit_trees(w: &World, defs: &Defs) -> Vec<Pos> {
    let tree = defs.resource_id("fruit_tree").unwrap();
    w.map.iter().filter(|(_, t)| t.resource == Some(tree)).map(|(p, _)| p).collect()
}

#[test]
fn a_finished_orchard_plants_a_sapling_every_interval_up_to_its_maximum() {
    let (mut w, defs) = setup(Nation::Air, 1, &[at(0, 0)]);
    let def = orchard_def(&defs);
    raise_orchard(&mut w, &defs, MID);
    assert!(fruit_trees(&w, &defs).is_empty());

    // Nothing is planted before the first interval is up...
    run(&mut w, &defs, secs(def.plant_seconds) - 2);
    assert!(fruit_trees(&w, &defs).is_empty());
    // ...then one sapling per interval, every one of them resting and marked as a sapling.
    run(&mut w, &defs, 4);
    let first = fruit_trees(&w, &defs);
    assert_eq!(first.len(), 1);
    assert!(w.is_sapling(first[0]) && w.is_resting(first[0]));
    run(&mut w, &defs, secs(def.plant_seconds));
    assert_eq!(fruit_trees(&w, &defs).len(), 2);

    run(&mut w, &defs, secs(def.plant_seconds) * (def.max_trees as u64 + 2));
    let trees = fruit_trees(&w, &defs);
    assert_eq!(trees.len() as u32, def.max_trees, "it tends no more than its maximum");
    assert_eq!(w.orchard_trees(MID), Some(def.max_trees as usize));
}

#[test]
fn planted_trees_stand_in_reach_apart_and_off_the_things_in_the_way() {
    let (mut w, defs) = setup(Nation::Air, 7, &[at(0, 0)]);
    let def = orchard_def(&defs);
    raise_orchard(&mut w, &defs, MID);
    // A stockpile tile, a ground stack and a build site all within reach must be left alone.
    w.designate_stockpile(at(6, 7)).unwrap();
    w.place_ground(at(8, 7), Stack { item: defs.item_id("wood").unwrap(), count: 1 });
    let bed = defs.building_id("bed").unwrap();
    w.designate_build(at(7, 6), bed, &defs).unwrap();
    run(&mut w, &defs, secs(def.plant_seconds) * (def.max_trees as u64 + 1));

    let trees = fruit_trees(&w, &defs);
    assert_eq!(trees.len() as u32, def.max_trees);
    for (i, &t) in trees.iter().enumerate() {
        assert!(t.chebyshev(MID) <= def.radius as i32, "{t:?} is out of reach");
        assert!(t.chebyshev(MID) >= def.spacing as i32, "{t:?} crowds the orchard");
        assert!(![at(6, 7), at(8, 7), at(7, 6)].contains(&t), "{t:?} is in use");
        for &u in &trees[i + 1..] {
            assert!(t.chebyshev(u) >= def.spacing as i32, "{t:?} and {u:?} are too close");
        }
    }
}

/// #226: a unit standing on a tile only because it is hidden inside a building there (its door)
/// doesn't hold the tile against planting the way a unit actually standing on the map does.
#[test]
fn a_unit_inside_a_building_does_not_block_its_door_from_planting() {
    let (mut w, mut defs) = setup(Nation::Air, 7, &[at(0, 0)]);
    let hall = defs.building_id("bed").unwrap();
    {
        let d = &mut defs.buildings[hall.0 as usize];
        d.footprint = (2, 2);
        d.bed = None;
        d.passable = false;
        d.capacity = 1;
    }
    let anchor = at(4, 4);
    let foot = Footprint::new(anchor, 2, 2);
    w.raise(foot, hall, defs.building(hall));
    let door = foot.entrance();
    w.units[0].pos = door;
    assert!(w.enter_building(0, anchor, &defs));

    // Its own `spacing` (1) only rules out the tile itself, so the door, merely beside the
    // hall, is otherwise free to plant.
    let def = OrchardDef {
        plants: defs.resource_id("fruit_tree").unwrap(),
        radius: 10,
        max_trees: 1,
        plant_seconds: 1.0,
        grow_seconds: 1.0,
        tend_boost: 1.0,
        spacing: 1,
        auto_pick: false,
    };
    assert!(w.can_plant(door, &def, &defs), "the hidden unit doesn't hold its door");

    // The same tile, visibly occupied instead, blocks it, as it always has.
    w.leave_building(0);
    assert!(!w.can_plant(door, &def, &defs));
}

/// #425: `can_plant` treats a pending build site like a finished building, footprint and door
/// both: not on it, not on its door, and not within `spacing` of either, so a sapling never ends
/// up blocking the door the moment the site is cleared.
#[test]
fn an_orchard_never_plants_on_or_beside_a_pending_site_or_its_door() {
    let (mut w, mut defs) = setup(Nation::Air, 7, &[at(0, 0)]);
    let hall = defs.building_id("bed").unwrap();
    {
        let d = &mut defs.buildings[hall.0 as usize];
        d.footprint = (2, 2);
        d.bed = None;
        d.passable = false;
        d.capacity = 2;
    }
    let anchor = at(7, 5);
    w.designate_build(anchor, hall, &defs).unwrap();
    let foot = Footprint::new(anchor, 2, 2);
    let door = foot.entrance();

    let def = OrchardDef {
        plants: defs.resource_id("fruit_tree").unwrap(),
        radius: 10,
        max_trees: 1,
        plant_seconds: 1.0,
        grow_seconds: 1.0,
        tend_boost: 1.0,
        spacing: 2,
        auto_pick: false,
    };
    for p in foot.tiles() {
        assert!(!w.can_plant(p, &def, &defs), "{p:?} is the site itself");
    }
    assert!(!w.can_plant(door, &def, &defs), "the door can't be planted on");
    // Within `spacing` of the site or its door, not just on them.
    assert!(!w.can_plant(at(foot.anchor.x - 1, foot.anchor.y), &def, &defs), "crowds the site");
    assert!(!w.can_plant(at(door.x - 1, door.y), &def, &defs), "crowds the door");
    // Clear of all of it, planting is fine again.
    assert!(w.can_plant(at(door.x + 3, door.y + 3), &def, &defs));
}

#[test]
fn planting_is_deterministic_per_seed_and_varies_between_seeds() {
    let grove = |seed: u64| {
        let (mut w, defs) = setup(Nation::Air, seed, &[at(0, 0)]);
        raise_orchard(&mut w, &defs, MID);
        run(&mut w, &defs, secs(90.0 * 8.0));
        fruit_trees(&w, &defs)
    };
    assert_eq!(grove(3), grove(3));
    assert!((4..12).any(|s| grove(s) != grove(3)), "the seed should change where trees go");
}

#[test]
fn a_planted_tree_is_a_sapling_until_it_first_ripens_then_a_plain_fruit_tree() {
    let (mut w, defs) = setup(Nation::Air, 1, &[at(0, 0)]);
    let def = orchard_def(&defs);
    raise_orchard(&mut w, &defs, MID);
    run(&mut w, &defs, secs(def.plant_seconds) + 2);
    let tree = fruit_trees(&w, &defs)[0];
    assert!(w.is_sapling(tree));

    // Saplings grow at the tended rate, so ripening takes grow_seconds / tend_boost.
    let grow = secs(def.grow_seconds / def.tend_boost);
    run(&mut w, &defs, grow - 20);
    assert!(w.is_sapling(tree) && w.is_resting(tree), "not ripe a little early");
    run(&mut w, &defs, 40);
    assert!(!w.is_sapling(tree) && !w.is_resting(tree), "ripe on schedule");
    assert_eq!(w.map[tree].resource, defs.resource_id("fruit_tree"));
}

#[test]
fn an_orchard_yields_harvestable_fruit_within_the_designed_time() {
    let (mut w, defs) = setup(Nation::Air, 2, &[at(0, 0), at(1, 0)]);
    let def = orchard_def(&defs);
    let fruit = defs.item_id("fruit").unwrap();
    raise_orchard(&mut w, &defs, MID);
    w.designate_stockpile(at(0, 14)).unwrap();

    // First fruit: one interval to plant it, then it grows at the tended rate, then someone picks
    // it up and carries it off. The slack covers walking and the picking work.
    let first_fruit = def.plant_seconds + def.grow_seconds / def.tend_boost;
    run_until(&mut w, &defs, secs(first_fruit + 120.0), |w| w.stored_total(fruit) > 0);
    assert!(w.stored_total(fruit) > 0, "units pick the orchard's fruit with no orders from us");
}

#[test]
fn the_existing_picking_job_works_a_planted_tree_by_hand() {
    // With `auto_pick` off, the tree is still an ordinary fruit tree: designating it works.
    let (mut w, mut defs) = setup(Nation::Air, 2, &[at(1, 1)]);
    let id = defs.building_id("orchard").unwrap();
    defs.buildings[id.0 as usize].orchard.as_mut().unwrap().auto_pick = false;
    let def = orchard_def(&defs);
    raise_orchard(&mut w, &defs, MID);
    run(&mut w, &defs, secs(def.plant_seconds) + 2);
    let tree = fruit_trees(&w, &defs)[0];
    assert!(!w.is_repeating_harvest(tree), "no standing order without auto_pick");
    run(&mut w, &defs, secs(def.grow_seconds / def.tend_boost) + 20);
    w.designate_harvest(tree, &defs).unwrap();
    let fruit = defs.item_id("fruit").unwrap();
    run_until(&mut w, &defs, 3000, |w| {
        w.ground_items().any(|(_, s)| s.item == fruit)
            || w.units.iter().any(|u| u.carrying.is_some())
    });
}

/// Clearing a tree an orchard planted (`docs/design/clearing.md`, issue #411/#424) needs no new
/// code in `tick_orchards`: it already drops a tree from `Orchard::trees` once its tile no
/// longer holds the planted resource.
#[test]
fn clearing_a_planted_tree_drops_it_from_the_orchard_on_the_next_tick() {
    let (mut w, defs) = setup(Nation::Air, 1, &[at(0, 0)]);
    let def = orchard_def(&defs);
    raise_orchard(&mut w, &defs, MID);
    run(&mut w, &defs, secs(def.plant_seconds) + 4);
    let trees = fruit_trees(&w, &defs);
    assert_eq!(trees.len(), 1);
    let tree = trees[0];
    assert_eq!(w.orchard_trees(MID), Some(1));

    // A sapling is resting, but a clear doesn't wait on that.
    let job = w.designate_clear(tree, &defs).unwrap();
    run_until(&mut w, &defs, 2000, |w| w.job(job).is_none());
    assert!(w.map[tree].resource.is_none());
    w.tick(&defs);
    assert_eq!(w.orchard_trees(MID), Some(0), "tick_orchards drops the cleared tree on its own");
}

#[test]
fn a_planted_tree_regrows_after_picking_faster_than_a_wild_one() {
    let (mut w, defs) = setup(Nation::Air, 2, &[at(0, 0)]);
    let def = orchard_def(&defs);
    let fruit_tree = defs.resource_id("fruit_tree").unwrap();
    let regrow = defs.harvest(fruit_tree).unwrap().regrow.unwrap();
    raise_orchard(&mut w, &defs, MID);
    // A wild tree beyond the orchard's reach, picked at the same moment as a planted one.
    let wild = at(0, 14);
    w.map[wild].resource = Some(fruit_tree);
    run(&mut w, &defs, secs(def.plant_seconds) + 2);
    let planted = fruit_trees(&w, &defs).into_iter().find(|&p| p != wild).unwrap();
    run(&mut w, &defs, secs(def.grow_seconds / def.tend_boost) + 20);
    assert!(!w.is_resting(planted));

    w.resting.insert(planted, regrow);
    w.resting.insert(wild, regrow);
    run(&mut w, &defs, secs(regrow / def.tend_boost) + 20);
    assert!(!w.is_resting(planted), "tended: back in {} s", regrow / def.tend_boost);
    assert!(w.is_resting(wild), "wild: still growing");
    assert!(!w.is_sapling(planted), "a picked tree is not a sapling again");
    run(&mut w, &defs, secs(regrow - regrow / def.tend_boost) + 20);
    assert!(!w.is_resting(wild), "wild: back after the full {regrow} s");
}

#[test]
fn a_standing_order_picks_each_crop_and_cancelling_it_stops_that() {
    let (mut w, defs) = setup(Nation::Air, 2, &[at(0, 0)]);
    let def = orchard_def(&defs);
    raise_orchard(&mut w, &defs, MID);
    run(&mut w, &defs, secs(def.plant_seconds) + 2);
    let tree = fruit_trees(&w, &defs)[0];
    assert!(w.is_repeating_harvest(tree), "planting leaves a standing order (#81)");
    w.cancel_harvest(tree).unwrap();
    assert!(!w.is_repeating_harvest(tree));
    run(&mut w, &defs, secs(def.grow_seconds));
    assert!(
        !w.jobs().iter().any(|j| j.kind == JobKind::Harvest { pos: tree }),
        "an order the player cancelled stays cancelled"
    );
    assert_eq!(w.map[tree].resource, defs.resource_id("fruit_tree"), "the tree itself stays");
}

#[test]
fn the_orchard_replants_when_a_tree_is_lost_and_waits_when_there_is_no_room() {
    let (mut w, defs) = setup(Nation::Air, 5, &[at(0, 0)]);
    let def = orchard_def(&defs);
    raise_orchard(&mut w, &defs, MID);
    run(&mut w, &defs, secs(def.plant_seconds) * (def.max_trees as u64 + 1));
    let lost = fruit_trees(&w, &defs)[0];
    w.map[lost].resource = None;
    assert_eq!(w.orchard_trees(MID), Some(def.max_trees as usize), "not noticed until it ticks");
    run(&mut w, &defs, 2);
    assert_eq!(w.orchard_trees(MID), Some(def.max_trees as usize - 1));
    run(&mut w, &defs, secs(def.plant_seconds) + 2);
    assert_eq!(w.orchard_trees(MID), Some(def.max_trees as usize));

    // An orchard hemmed in by water has no tile to plant on, and just keeps waiting.
    let (mut w, defs) = setup(Nation::Air, 5, &[at(0, 0)]);
    for p in w.map.iter().map(|(p, _)| p).collect::<Vec<_>>() {
        if p != MID && p.chebyshev(MID) <= def.radius as i32 {
            w.map[p].terrain = Terrain::Water;
        }
    }
    raise_orchard(&mut w, &defs, MID);
    run(&mut w, &defs, secs(def.plant_seconds) * 3);
    assert!(fruit_trees(&w, &defs).is_empty());
}

#[test]
fn an_orchard_built_by_the_settlers_starts_planting_when_it_is_finished() {
    let (mut w, defs) = setup(Nation::Air, 1, &[at(1, 1)]);
    let wood = defs.item_id("wood").unwrap();
    let id = defs.building_id("orchard").unwrap();
    let cost = defs.building(id).cost[0].count;
    w.designate_stockpile(at(0, 0)).unwrap();
    w.stored.insert(at(0, 0), Stack { item: wood, count: cost });
    let job = w.designate_build(MID, id, &defs).unwrap();
    assert_eq!(w.orchard_trees(MID), None, "a site plants nothing");

    run_until(&mut w, &defs, 6000, |w| w.job(job).is_none());
    assert_eq!(w.orchard_trees(MID), Some(0));
    let def = orchard_def(&defs);
    run(&mut w, &defs, secs(def.plant_seconds) + 20);
    assert_eq!(w.orchard_trees(MID), Some(1));
}

#[test]
fn only_air_can_build_an_orchard() {
    let defs = Defs::bundled();
    let orchards = defs.tech_id("orchards").unwrap();
    let id = defs.building_id("orchard").unwrap();
    for nation in Nation::ALL {
        let (mut w, _) = setup(nation, 1, &[at(1, 1)]);
        let result = w.designate_build(MID, id, &defs);
        if nation == Nation::Air {
            assert!(result.is_ok(), "Air starts with orchards");
        } else {
            assert_eq!(result, Err(BuildError::NotResearched(orchards)), "{nation:?}");
            assert!(
                !defs.techs_for(nation).any(|(t, _)| t == orchards),
                "{nation:?} can't research it either"
            );
        }
    }
}

#[test]
fn wild_fruit_picking_stays_open_to_every_nation_without_orchards() {
    let defs = Defs::bundled();
    let tree = defs.resource_id("fruit_tree").unwrap();
    let fruit = defs.item_id("fruit").unwrap();
    let orchards = defs.tech_id("orchards").unwrap();
    for nation in Nation::ALL {
        let (mut w, _) = setup(nation, 1, &[at(3, 3)]);
        assert!(!w.research().is_known(orchards) || nation == Nation::Air);
        w.map[at(5, 5)].resource = Some(tree);
        w.designate_harvest(at(5, 5), &defs).unwrap_or_else(|e| panic!("{nation:?}: {e:?}"));
        run_until(&mut w, &defs, 2000, |w| w.ground_items().any(|(_, s)| s.item == fruit));
        assert!(w.is_resting(at(5, 5)), "{nation:?} picked it and it is regrowing");
    }
}

#[test]
fn orchards_survive_a_save_and_the_grove_keeps_growing_the_same() {
    let (mut w, defs) = setup(Nation::Air, 9, &[at(0, 0)]);
    let def = orchard_def(&defs);
    raise_orchard(&mut w, &defs, MID);
    run(&mut w, &defs, secs(def.plant_seconds) * 3 + 5);
    let text = w.save(&defs).unwrap();
    let mut loaded = World::load(&text, &defs).unwrap();
    run(&mut w, &defs, secs(def.plant_seconds) * 4);
    run(&mut loaded, &defs, secs(def.plant_seconds) * 4);
    assert_eq!(fruit_trees(&w, &defs), fruit_trees(&loaded, &defs));
    assert_eq!(w.save(&defs).unwrap(), loaded.save(&defs).unwrap());
}

#[test]
fn a_save_from_before_orchards_planted_adopts_the_ones_it_already_has() {
    let (mut w, defs) = setup(Nation::Air, 9, &[at(0, 0)]);
    let id = defs.building_id("orchard").unwrap();
    let b = defs.building(id);
    // The building is on the map but nothing registered it, as in an old save.
    w.map[MID].building = Some(Building::new(id, b.passable, false));
    assert_eq!(w.orchard_trees(MID), None);
    w.adopt_orchards(&defs);
    assert_eq!(w.orchard_trees(MID), Some(0));
    w.adopt_orchards(&defs);
    run(&mut w, &defs, secs(orchard_def(&defs).plant_seconds) + 5);
    assert_eq!(w.orchard_trees(MID), Some(1), "adopting twice doesn't reset it");
}

// --- Water's Fishing Dock (#374): the orchard, planting on water ------------------------------

fn fishing_dock_def(defs: &Defs) -> OrchardDef {
    defs.building(defs.building_id("fishing_dock").unwrap()).orchard.expect("the dock plants")
}

/// Puts a finished Fishing Dock on `anchor`, as if built, with its real 2x2 footprint.
fn raise_fishing_dock(w: &mut World, defs: &Defs, anchor: Pos) {
    let id = defs.building_id("fishing_dock").unwrap();
    let def = defs.building(id);
    w.raise(Footprint::new(anchor, 2, 2), id, def);
    w.open_orchard(anchor, def.orchard.as_ref().unwrap());
}

fn fish_spots(w: &World, defs: &Defs) -> Vec<Pos> {
    let fish = defs.resource_id("fish_spot").unwrap();
    w.map.iter().filter(|(_, t)| t.resource == Some(fish)).map(|(p, _)| p).collect()
}

/// A small lake carved into the arena's land, far enough from the dock's own footprint (#374's
/// `spacing`) not to be excluded by it, and shaped so one tile -- `(4, 6)` -- never touches land
/// or coast, even though it is water, in reach, and otherwise exactly what a fish spot wants.
fn carve_lake(w: &mut World) {
    for y in 5..=7 {
        for x in 3..=5 {
            w.map[at(x, y)].terrain = Terrain::Water;
        }
    }
}

const LAKE_INTERIOR: Pos = Pos::new(4, 6);

#[test]
fn a_fishing_dock_plants_fish_spots_only_on_water_beside_the_shore_never_more_than_its_max() {
    let (mut w, defs) = setup(Nation::Water, 11, &[at(0, 0)]);
    let def = fishing_dock_def(&defs);
    carve_lake(&mut w);
    raise_fishing_dock(&mut w, &defs, MID);
    assert!(fish_spots(&w, &defs).is_empty());

    run(&mut w, &defs, secs(def.plant_seconds) * (def.max_trees as u64 + 3));
    let spots = fish_spots(&w, &defs);
    assert_eq!(spots.len() as u32, def.max_trees, "it tends no more than its maximum");
    assert_eq!(w.orchard_trees(MID), Some(def.max_trees as usize));
    for &p in &spots {
        assert_eq!(w.map[p].terrain, Terrain::Water, "{p:?} is a water tile");
        assert_ne!(p, LAKE_INTERIOR, "{p:?} never touches land or coast");
    }
}

#[test]
fn a_fishing_dock_never_plants_the_one_water_tile_that_does_not_touch_the_shore() {
    let (mut w, defs) = setup(Nation::Water, 1, &[at(0, 0)]);
    let def = fishing_dock_def(&defs);
    carve_lake(&mut w);
    raise_fishing_dock(&mut w, &defs, MID);
    assert!(w.can_plant(at(3, 6), &def, &defs), "touches land at (2, 6)");
    assert!(!w.can_plant(LAKE_INTERIOR, &def, &defs), "surrounded by water on every side");
}

#[test]
fn fish_spots_a_dock_plants_are_picked_as_they_ripen_with_no_orders_from_us() {
    let (mut w, defs) = setup(Nation::Water, 2, &[at(0, 0), at(1, 0)]);
    let def = fishing_dock_def(&defs);
    let fish = defs.item_id("fish").unwrap();
    carve_lake(&mut w);
    raise_fishing_dock(&mut w, &defs, MID);
    w.designate_stockpile(at(0, 14)).unwrap();

    let first_fish = def.plant_seconds + def.grow_seconds / def.tend_boost;
    run_until(&mut w, &defs, secs(first_fish + 180.0), |w| w.stored_total(fish) > 0);
    assert!(w.stored_total(fish) > 0, "fish spots are fished with no orders from us");
}

/// #490: a tree blocks movement for good, so an orchard never plants one in the only way through
/// a narrow place (here a one-tile corridor, every tile in reach of which would seal off the
/// ground past it), though it plants freely once the ground is wide enough to walk round it.
#[test]
fn an_orchard_never_plants_a_tree_that_seals_off_ground() {
    let defs = Defs::bundled();
    let def = orchard_def(&defs);
    let mut map = arena(SIZE);
    for p in map.positions().collect::<Vec<_>>() {
        if p.y != 7 {
            map[p].terrain = Terrain::Mountain;
        }
    }
    let mut w = World::with_map(7, map, Nation::Air, &[at(0, 7)], &defs);
    raise_orchard(&mut w, &defs, MID);
    run(&mut w, &defs, secs(def.plant_seconds) * (def.max_trees as u64 + 2));
    assert!(fruit_trees(&w, &defs).is_empty(), "{:?}", fruit_trees(&w, &defs));

    // Open the rows either side of the corridor and it plants as it always did.
    for p in w.map.positions().collect::<Vec<_>>() {
        w.map[p].terrain = Terrain::Land;
    }
    w.map_changed();
    run(&mut w, &defs, secs(def.plant_seconds) * (def.max_trees as u64 + 2));
    assert_eq!(fruit_trees(&w, &defs).len() as u32, def.max_trees);
}
