use super::*;
use crate::defs::{BiomeId, Terrain};
use crate::geom::Grid;
use crate::mapgen::Tile;

/// A map from ASCII rows: `.` land, `~` water, `^` mountain, `T` oak tree, `s` stone, `c` cabbage.
fn arena(defs: &Defs, rows: &[&str]) -> TileMap {
    Grid::from_fn(rows[0].len() as i32, rows.len() as i32, |p| {
        let c = rows[p.y as usize].as_bytes()[p.x as usize];
        let terrain = match c {
            b'~' => Terrain::Water,
            b'^' => Terrain::Mountain,
            _ => Terrain::Land,
        };
        let resource = match c {
            b'T' => defs.resource_id("oak_tree"),
            b's' => defs.resource_id("stone"),
            b'c' => defs.resource_id("cabbage"),
            _ => None,
        };
        Tile {
            elevation: 100,
            temperature: 128,
            moisture: 128,
            terrain,
            biome: BiomeId(0),
            resource,
        }
    })
}

fn at(x: i32, y: i32) -> Pos {
    Pos::new(x, y)
}

fn setup(nation: Nation, rows: &[&str], units: &[Pos]) -> (World, Defs) {
    let defs = Defs::bundled();
    let map = arena(&defs, rows);
    (World::with_map(1, map, nation, units, &defs), defs)
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

const WOODS: [&str; 5] = [".........", ".........", ".......T.", ".........", "........."];

#[test]
fn chop_a_tree_and_haul_the_wood_to_the_stockpile() {
    let (mut w, defs) = setup(Nation::Earth, &WOODS, &[at(3, 2)]);
    let wood = defs.item_id("wood").unwrap();
    w.designate_stockpile(at(1, 2)).unwrap();
    w.designate_stockpile(at(1, 3)).unwrap();
    let job = w.designate_harvest(at(7, 2), &defs).unwrap();
    w.tick(&defs);
    assert_eq!(w.job(job).unwrap().claimed_by, Some(UnitId(0)));
    run_until(&mut w, &defs, 3000, |w| w.stored_total(wood) == 4);
    assert!(w.map[at(7, 2)].resource.is_none(), "the tree is gone");
    assert!(w.jobs().is_empty());
    assert_eq!(w.ground_items().count(), 0);
    let stacks: Vec<_> = [at(1, 2), at(1, 3)].iter().filter_map(|&p| w.stored_at(p)).collect();
    assert_eq!(stacks, [Stack { item: wood, count: 4 }]);
    assert_eq!(w.units[0].carrying, None);
    assert_eq!(w.units[0].task(), None);
}

#[test]
fn wood_waits_on_the_ground_until_a_stockpile_exists() {
    let (mut w, defs) = setup(Nation::Earth, &WOODS, &[at(3, 2)]);
    let wood = defs.item_id("wood").unwrap();
    w.designate_harvest(at(7, 2), &defs).unwrap();
    run_until(&mut w, &defs, 3000, |w| w.ground_items().count() == 1);
    // A haul job exists but nobody can do it.
    for _ in 0..200 {
        w.tick(&defs);
    }
    assert_eq!(w.jobs().len(), 1);
    assert_eq!(w.jobs()[0].claimed_by, None);
    let on_ground: Vec<_> = w.ground_items().collect();
    assert_eq!(on_ground, [(at(7, 2), Stack { item: wood, count: 4 })]);
    w.designate_stockpile(at(2, 2)).unwrap();
    run_until(&mut w, &defs, 3000, |w| w.stored_total(wood) == 4);
}

#[test]
fn units_claim_the_nearest_reachable_job_not_the_oldest() {
    let rows = ["..........", "..........", "..........", "..T......T"];
    let (mut w, defs) = setup(Nation::Earth, &rows, &[at(1, 1)]);
    let far = w.designate_harvest(at(9, 3), &defs).unwrap();
    let near = w.designate_harvest(at(2, 3), &defs).unwrap();
    w.tick(&defs);
    assert_eq!(w.job(near).unwrap().claimed_by, Some(UnitId(0)));
    assert_eq!(w.job(far).unwrap().claimed_by, None);
}

#[test]
fn nearest_means_walking_distance_so_a_wall_makes_a_tree_far() {
    // The unit at (2,2) is two tiles from the tree at (4,2) as the crow flies, but a wall
    // between them means a walk round the end of it (about 70). The tree at (2,7) is 40 away.
    let rows = [
        "...^...", "...^...", "...^T..", "...^...", "...^...", ".......", ".......", "..T....",
        ".......",
    ];
    let (mut w, defs) = setup(Nation::Earth, &rows, &[at(2, 2)]);
    let behind_wall = w.designate_harvest(at(4, 2), &defs).unwrap();
    let in_the_open = w.designate_harvest(at(2, 7), &defs).unwrap();
    w.tick(&defs);
    assert_eq!(w.job(in_the_open).unwrap().claimed_by, Some(UnitId(0)));
    assert_eq!(w.job(behind_wall).unwrap().claimed_by, None);
}

#[test]
fn only_one_unit_takes_a_job() {
    let (mut w, defs) = setup(Nation::Earth, &WOODS, &[at(0, 0), at(8, 4)]);
    let job = w.designate_harvest(at(7, 2), &defs).unwrap();
    w.tick(&defs);
    // The first unit in id order gets it, even though the second is nearer: it asked first.
    assert_eq!(w.job(job).unwrap().claimed_by, Some(UnitId(0)));
    assert_eq!(w.units[1].task(), None);
}

#[test]
fn unreachable_jobs_are_skipped_without_blocking_others() {
    let rows = ["~~~~~~~~~", "~T~.....~", "~~~.....~", "~~~..T..~", "~~~~~~~~~"];
    let (mut w, defs) = setup(Nation::Earth, &rows, &[at(3, 2)]);
    let island = w.designate_harvest(at(1, 1), &defs).unwrap();
    let reachable = w.designate_harvest(at(5, 3), &defs).unwrap();
    w.tick(&defs);
    assert_eq!(w.job(reachable).unwrap().claimed_by, Some(UnitId(0)));
    run_until(&mut w, &defs, 2000, |w| w.job(reachable).is_none());
    for _ in 0..300 {
        w.tick(&defs);
    }
    assert_eq!(w.job(island).unwrap().claimed_by, None);
    assert!(w.map[at(1, 1)].resource.is_some());
}

/// Ticks from designation until the resource on `target` is cleared, with one unit beside it.
fn ticks_to_clear(nation: Nation, rows: &[&str], target: Pos) -> u64 {
    let (mut w, defs) = setup(nation, rows, &[at(1, 1)]);
    w.designate_harvest(target, &defs).unwrap();
    run_until(&mut w, &defs, 5000, |w| w.map[target].resource.is_none())
}

#[test]
fn work_speed_uses_the_nation_modifier_the_harvest_names() {
    let quarry = ["....", "..s.", "...."];
    let earth = ticks_to_clear(Nation::Earth, &quarry, at(2, 1));
    let air = ticks_to_clear(Nation::Air, &quarry, at(2, 1));
    // Stone is 12 work-seconds at modifier 1.0; earthworks is 2.5 for Earth and 0.7 for Air, and
    // there are 10 ticks to a second. A couple of ticks go on claiming and arriving.
    let expect = |m: f32| (12.0 / m * 10.0) as i64;
    assert!((earth as i64 - expect(2.5)).abs() <= 4, "earth {earth}");
    assert!((air as i64 - expect(0.7)).abs() <= 4, "air {air}");
    assert!(earth * 3 < air);

    // Chopping names no modifier, so every nation takes the same time: 8 seconds.
    let chop = ["....", "..T.", "...."];
    let times: Vec<u64> = Nation::ALL.iter().map(|&n| ticks_to_clear(n, &chop, at(2, 1))).collect();
    assert!(times.iter().all(|&t| (t as i64 - 80).abs() <= 4), "{times:?}");
}

#[test]
fn carry_capacity_sets_how_much_a_trip_moves() {
    for (nation, trip) in [(Nation::Earth, 7), (Nation::Air, 4)] {
        let (mut w, defs) = setup(nation, &WOODS, &[at(1, 2)]);
        let wood = defs.item_id("wood").unwrap();
        w.designate_stockpile(at(0, 2)).unwrap();
        w.ground.insert(at(4, 2), Stack { item: wood, count: 10 });
        w.ensure_haul_job(at(4, 2));
        let mut biggest = 0;
        run_until(&mut w, &defs, 4000, |w| {
            biggest = biggest.max(w.units[0].carrying.map_or(0, |s| s.count));
            w.stored_total(wood) == 10
        });
        assert_eq!(biggest, trip, "{nation:?}");
    }
}

#[test]
fn a_low_need_pauses_the_job_and_it_resumes_where_it_left_off() {
    let rows = ["~~~~~~~~~", ".........", "....T....", ".........", "........."];
    let (mut w, defs) = setup(Nation::Earth, &rows, &[at(3, 2)]);
    let job = w.designate_harvest(at(4, 2), &defs).unwrap();
    run_until(&mut w, &defs, 500, |w| w.job(job).unwrap().progress > 2.0);
    let before = w.job(job).unwrap().progress;
    // Thirsty: the shore (row 1) is a step away.
    w.units[0].needs.thirst = defs.needs.thirst.low;
    w.tick(&defs);
    assert_eq!(w.units[0].task(), Some(Task::Drink));
    // The job is set aside, not dropped.
    assert_eq!(w.job(job).unwrap().claimed_by, Some(UnitId(0)));
    run_until(&mut w, &defs, 500, |w| w.units[0].task() != Some(Task::Drink));
    assert!(w.units[0].needs.thirst > defs.needs.thirst.low + 40.0);
    let resumed = w.job(job).unwrap().progress;
    assert!(resumed >= before && resumed < before + 1.0, "no work done while drinking");
    run_until(&mut w, &defs, 2000, |w| w.job(job).is_none());
    assert!(w.map[at(4, 2)].resource.is_none());
    assert_eq!(w.ground_items().next().map(|(_, s)| s.count), Some(4));
}

#[test]
fn the_most_urgent_need_goes_first_and_a_hungry_unit_forages_without_fire() {
    let rows = ["~~~~~~~~~", ".........", "......c..", ".........", "........."];
    let (mut w, defs) = setup(Nation::Earth, &rows, &[at(1, 3)]);
    let fire = defs.tech_id("fire_making").unwrap();
    assert!(!w.research().is_known(fire));
    w.units[0].needs.hunger = 10.0;
    w.units[0].needs.thirst = 38.0;
    w.tick(&defs);
    // Hunger is a quarter of the way up to `low`, thirst almost all the way: eat first.
    assert!(matches!(w.units[0].task(), Some(Task::Eat(FoodSource::Forage(_)))));
    run_until(&mut w, &defs, 2000, |w| w.units[0].task().is_none());
    let n = w.units[0].needs;
    // The raw cabbage (30) is eaten and gone, and then it goes to drink.
    assert!(n.hunger > 30.0 && n.hunger < 45.0, "{n:?}");
    assert!(w.map[at(6, 2)].resource.is_none());
    run_until(&mut w, &defs, 2000, |w| w.units[0].needs.thirst > 80.0);
}

#[test]
fn hungry_units_eat_stockpiled_food_before_foraging() {
    let rows = ["~~~~~~~~~", ".........", ".c.......", ".........", "........."];
    let (mut w, defs) = setup(Nation::Water, &rows, &[at(7, 3)]);
    let fish = defs.item_id("fish").unwrap();
    w.designate_stockpile(at(6, 3)).unwrap();
    w.stored.insert(at(6, 3), Stack { item: fish, count: 2 });
    w.units[0].needs.hunger = 20.0;
    w.tick(&defs);
    assert_eq!(w.units[0].task(), Some(Task::Eat(FoodSource::Stockpile(at(6, 3)))));
    run_until(&mut w, &defs, 500, |w| w.units[0].task().is_none());
    assert_eq!(w.stored_total(fish), 1);
    assert!(w.units[0].needs.hunger >= 59.0, "{:?}", w.units[0].needs);
    assert!(w.map[at(1, 2)].resource.is_some(), "the cabbage was left alone");
}

#[test]
fn a_tired_unit_sleeps_until_rested_then_wakes() {
    let (mut w, defs) = setup(Nation::Fire, &WOODS, &[at(3, 2)]);
    w.units[0].needs.energy = defs.needs.energy.low;
    w.tick(&defs);
    assert!(w.units[0].is_sleeping());
    let start = w.units[0].needs.energy;
    for _ in 0..100 {
        w.tick(&defs);
    }
    assert!(w.units[0].needs.energy > start);
    run_until(&mut w, &defs, 10 * 60 * 10, |w| !w.units[0].is_sleeping());
    assert!(w.units[0].needs.energy >= defs.jobs.sleep_until);
}

#[test]
fn a_dying_unit_gives_its_job_and_load_back() {
    let (mut w, defs) = setup(Nation::Earth, &WOODS, &[at(3, 2), at(4, 4)]);
    let wood = defs.item_id("wood").unwrap();
    w.designate_stockpile(at(0, 0)).unwrap();
    let job = w.designate_harvest(at(7, 2), &defs).unwrap();
    run_until(&mut w, &defs, 500, |w| w.job(job).unwrap().progress > 1.0);
    let progress = w.job(job).unwrap().progress;
    w.units[0].needs.hunger = 0.0;
    w.units[0].needs.health = 0.001;
    w.units[0].carrying = Some(Stack { item: wood, count: 3 });
    w.tick(&defs);
    assert_eq!(w.units.len(), 1);
    assert_eq!(w.ground_items().map(|(_, s)| s.count).sum::<u32>(), 3);
    // The other unit can take the job on, and the work already done isn't lost.
    let j = w.job(job).unwrap();
    assert!(j.progress >= progress);
    assert!(j.claimed_by.is_none_or(|u| u == UnitId(1)));
}

#[test]
fn designating_is_checked_and_cancelling_frees_the_unit() {
    let (mut w, defs) = setup(Nation::Earth, &WOODS, &[at(3, 2)]);
    assert_eq!(w.designate_harvest(at(0, 0), &defs), Err(DesignateError::NothingToHarvest));
    assert_eq!(w.designate_harvest(at(99, 0), &defs), Err(DesignateError::OutOfBounds));
    assert!(w.designate_harvest(at(7, 2), &defs).is_ok());
    assert_eq!(w.designate_harvest(at(7, 2), &defs), Err(DesignateError::AlreadyDesignated));
    assert_eq!(w.designate_stockpile(at(7, 2)), Err(DesignateError::CannotStockpile));
    w.tick(&defs);
    assert!(w.units[0].task().is_some());
    assert!(w.cancel_harvest(at(7, 2)));
    assert!(!w.cancel_harvest(at(7, 2)));
    w.tick(&defs);
    assert_eq!(w.units[0].task(), None);
    assert!(w.map[at(7, 2)].resource.is_some());
}

#[test]
fn units_still_starve_where_there_is_no_food_at_all() {
    // Water alone isn't enough. (Wild food is finite; renewable food comes with fishing and
    // farming.)
    let rows = ["~~~~~~~~~", ".........", ".........", "........."];
    let (mut w, defs) = setup(Nation::Earth, &rows, &[at(1, 2), at(5, 2)]);
    run_until(&mut w, &defs, 100_000, |w| w.units.is_empty());
    assert_eq!(w.deaths.len(), 2);
    assert!(w.deaths.iter().all(|d| d.cause == DeathCause::Starvation));
}
