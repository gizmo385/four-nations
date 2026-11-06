use super::*;
use crate::defs::{BiomeId, Terrain};
use crate::geom::{Footprint, Grid};
use crate::mapgen::Tile;

/// A map from ASCII rows: `.` land, `~` water, `^` mountain, `T` oak tree, `s` stone, `c` cabbage,
/// `Y` fruit tree, `f` fishing spot (on water).
fn arena(defs: &Defs, rows: &[&str]) -> TileMap {
    Grid::from_fn(rows[0].len() as i32, rows.len() as i32, |p| {
        let c = rows[p.y as usize].as_bytes()[p.x as usize];
        let terrain = match c {
            b'~' | b'f' => Terrain::Water,
            b'^' => Terrain::Mountain,
            _ => Terrain::Land,
        };
        let resource = match c {
            b'T' => defs.resource_id("oak_tree"),
            b's' => defs.resource_id("stone"),
            b'c' => defs.resource_id("cabbage"),
            b'Y' => defs.resource_id("fruit_tree"),
            b'f' => defs.resource_id("fish_spot"),
            _ => None,
        };
        Tile {
            elevation: 100,
            temperature: 128,
            moisture: 128,
            terrain,
            biome: BiomeId(0),
            resource,
            building: None,
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
    // The harvest tally (#233) counts the tree's whole yield, and nothing else.
    assert_eq!(w.tallies().harvested(wood), 4);
    assert_eq!(w.tallies().harvested_items().count(), 1);
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
fn cancelling_a_stockpile_drops_what_it_held_on_the_ground_there() {
    let (mut w, defs) = setup(Nation::Earth, &WOODS, &[at(3, 2)]);
    let wood = defs.item_id("wood").unwrap();
    w.designate_stockpile(at(1, 2)).unwrap();
    w.designate_harvest(at(7, 2), &defs).unwrap();
    run_until(&mut w, &defs, 3000, |w| w.stored_total(wood) == 4);
    assert_eq!(w.cancel_stockpile(at(5, 5)), Err(DesignateError::NotDesignated));

    assert_eq!(w.cancel_stockpile(at(1, 2)), Ok(()));
    assert_eq!(w.stockpile_zones().count(), 0);
    assert_eq!(w.stored_total(wood), 0);
    // Nothing is lost: the stack lies on the tile it was stored on, with a haul queued.
    let on_ground: Vec<_> = w.ground_items().collect();
    assert_eq!(on_ground, [(at(1, 2), Stack { item: wood, count: 4 })]);
    assert!(w.jobs().iter().any(|j| j.kind == JobKind::Haul { pos: at(1, 2) }));
    assert_eq!(w.cancel_stockpile(at(1, 2)), Err(DesignateError::NotDesignated));

    // With another zone, normal hauling stores it there.
    w.designate_stockpile(at(2, 3)).unwrap();
    run_until(&mut w, &defs, 3000, |w| w.stored_at(at(2, 3)).is_some());
    assert_eq!(w.stored_at(at(2, 3)), Some(Stack { item: wood, count: 4 }));
    assert_eq!(w.ground_items().count(), 0);
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
fn a_study_seat_loses_to_a_farther_harvest_when_both_are_open() {
    // A study hall right beside the unit, with an open seat and something queued to research
    // (so claiming it is possible at all), plus a tree far across the map. Study is the lowest
    // claim priority (#284), so the unit should walk all the way to the tree rather than take
    // the seat at its elbow.
    let rows = ["..........", "..........", "..........", "..........", "..........", "........T."];
    let (mut w, defs) = setup(Nation::Earth, &rows, &[at(1, 1)]);
    let hall = defs.building_id("study_hall").unwrap();
    let def = defs.building(hall);
    w.map[at(2, 1)].building = Some(crate::building::Building::new(hall, def.passable, false));
    w.open_study_seats(at(2, 1), def.seats);
    let earth_shaping = defs.tech_id("earth_shaping").unwrap();
    w.apply(Command::SetResearch { tech: earth_shaping }, &defs).unwrap();

    let tree = w.designate_harvest(at(8, 5), &defs).unwrap();
    w.tick(&defs);
    assert_eq!(w.job(tree).unwrap().claimed_by, Some(UnitId(0)), "the farther harvest wins");
    assert!(
        w.jobs()
            .iter()
            .filter(|j| matches!(j.kind, JobKind::Study { .. }))
            .all(|j| j.claimed_by.is_none()),
        "the near seat stays open"
    );
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
    // Quarrying needs stoneworking, which only Earth starts with; the others get it for free here.
    w.research.grant(defs.tech_id("stoneworking").unwrap());
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
fn an_earth_unit_quarries_at_earths_earthworks_even_in_a_water_settlement() {
    // A unit's work speed follows its own nation's modifiers (#239), not the settlement's: an
    // Earth settler quarries at Earth's earthworks (2.5) wherever it lives.
    let (mut w, defs) = setup(Nation::Water, &["....", "..s.", "...."], &[at(1, 1)]);
    // Quarrying needs stoneworking; grant it so the settlement can designate the stone at all
    // (teaching across nations is out of scope, so this stays settlement-wide).
    w.research.grant(defs.tech_id("stoneworking").unwrap());
    w.units[0].nation = Some(Nation::Earth);
    w.designate_harvest(at(2, 1), &defs).unwrap();
    let ticks = run_until(&mut w, &defs, 5000, |w| w.map[at(2, 1)].resource.is_none());
    let expect = (12.0 / 2.5 * 10.0) as i64;
    assert!((ticks as i64 - expect).abs() <= 4, "{ticks}");
}

#[test]
fn quarrying_stone_needs_stoneworking_but_earth_starts_with_it() {
    let rows = ["....", "..s.", "...."];
    let stoneworking = Defs::bundled().tech_id("stoneworking").unwrap();
    // Earth knows stoneworking from tick 0.
    let (mut earth, defs) = setup(Nation::Earth, &rows, &[at(1, 1)]);
    assert!(earth.research().is_known(stoneworking));
    assert!(earth.designate_harvest(at(2, 1), &defs).is_ok());

    // Air doesn't: no designation, and nothing was queued by the refusal.
    let (mut air, defs) = setup(Nation::Air, &rows, &[at(1, 1)]);
    assert!(!air.research().is_known(stoneworking));
    assert_eq!(
        air.designate_harvest(at(2, 1), &defs),
        Err(DesignateError::NotResearched(stoneworking))
    );
    assert!(air.jobs().is_empty());

    // Once it has researched it (25 points at 3/min, so tick 5000), it can.
    air.apply(Command::SetResearch { tech: stoneworking }, &defs).unwrap();
    for _ in 0..5000 {
        air.tick(&defs);
    }
    assert!(air.research().is_known(stoneworking));
    assert!(air.designate_harvest(at(2, 1), &defs).is_ok());
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

/// #226: a unit inside a building isn't drawn out gradually like a walk; the tick a need turns
/// urgent, `consider_needs` steps it out before planning where the need takes it, so the route
/// it plans starts from the door it actually stands on.
#[test]
fn a_need_steps_a_unit_out_of_a_building_the_tick_it_becomes_urgent() {
    let rows = ["~~~~~~~~~", ".........", ".........", ".........", "........."];
    let anchor = at(3, 1);
    let foot = Footprint::new(anchor, 2, 2);
    let door = foot.entrance();
    let (mut w, mut defs) = setup(Nation::Earth, &rows, &[door]);
    let hall = defs.building_id("bed").unwrap();
    {
        let d = &mut defs.buildings[hall.0 as usize];
        d.footprint = (2, 2);
        d.bed = None;
        d.passable = false;
        d.capacity = 1;
    }
    w.raise(foot, hall, defs.building(hall));
    assert!(w.enter_building(0, anchor, &defs));

    w.units[0].needs.thirst = defs.needs.thirst.low;
    w.tick(&defs);
    assert!(w.units[0].inside().is_none(), "the thirst need stepped it out");
    assert_eq!(w.units[0].task(), Some(Task::Drink));
    assert!(w.occupants(anchor).next().is_none());
}

/// #293: `plan_drink` now finds the nearest tile beside water through `Flood::seek`, checking
/// only the tiles it actually visits instead of scanning the whole map for one. This pins that a
/// tied walking cost still breaks the same way as a full scan would: by position.
#[test]
fn thirsty_units_break_a_tied_distance_to_water_by_position() {
    let rows = ["~~~~~~~", "~......", "~......", "~......", "~......", "~......", "~......"];
    let (mut w, defs) = setup(Nation::Earth, &rows, &[at(3, 3)]);
    w.units[0].needs.thirst = defs.needs.thirst.low;
    w.tick(&defs);
    assert_eq!(w.units[0].task(), Some(Task::Drink));
    assert_eq!(
        w.units[0].route.as_ref().and_then(|r| r.first()),
        Some(&at(1, 3)),
        "(1,3) and (3,1) are both 2 tiles from water at the same cost; the lesser wins"
    );
}

/// #293: `plan_food` now checks `food_rank_at` only on the tiles `Flood::seek` actually visits,
/// instead of scanning `self.stored`/`self.ground`/the whole map up front and picking the best
/// by hand. This pins that an exact tie in walking cost still ranks a stockpile over a forage
/// (and, separately, breaks a tie within the same rank by position), same as before.
#[test]
fn hungry_units_prefer_a_stockpile_over_a_tied_distance_forage() {
    let rows = ["........", "........", "........", "......c.", "........", "........"];
    let (mut w, defs) = setup(Nation::Earth, &rows, &[at(3, 3)]);
    let fish = defs.item_id("fish").unwrap();
    w.designate_stockpile(at(1, 3)).unwrap();
    w.stored.insert(at(1, 3), Stack { item: fish, count: 2 });
    w.units[0].needs.hunger = 20.0;
    w.tick(&defs);
    // The stockpile (rank 0) at (1,3) and the cabbage's nearest stand, (5,3), are both 2 tiles
    // off at the same cost; the stockpile outranks foraging regardless.
    assert_eq!(w.units[0].task(), Some(Task::Eat(FoodSource::Stockpile(at(1, 3)))));
}

#[test]
fn hungry_units_break_a_tied_stockpile_distance_by_position() {
    let rows = ["........", "........", "........", "........", "........", "........"];
    let (mut w, defs) = setup(Nation::Earth, &rows, &[at(3, 3)]);
    let fish = defs.item_id("fish").unwrap();
    for pos in [at(1, 3), at(5, 3)] {
        w.designate_stockpile(pos).unwrap();
        w.stored.insert(pos, Stack { item: fish, count: 2 });
    }
    w.units[0].needs.hunger = 20.0;
    w.tick(&defs);
    assert_eq!(
        w.units[0].task(),
        Some(Task::Eat(FoodSource::Stockpile(at(1, 3)))),
        "both stockpiles are 2 tiles off at the same cost; the lesser position wins"
    );
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
    // The raw cabbage (30) is eaten, leaving the plant to regrow, and then it goes to drink.
    assert!(n.hunger > 30.0 && n.hunger < 45.0, "{n:?}");
    assert!(w.is_resting(at(6, 2)));
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
    assert_eq!(w.cancel_harvest(at(7, 2)), Ok(()));
    assert_eq!(w.cancel_harvest(at(7, 2)), Err(DesignateError::NotDesignated));
    w.tick(&defs);
    assert_eq!(w.units[0].task(), None);
    assert!(w.map[at(7, 2)].resource.is_some());
}

#[test]
fn units_still_starve_where_there_is_no_food_at_all() {
    // Water alone isn't enough.
    let rows = ["~~~~~~~~~", ".........", ".........", "........."];
    let (mut w, defs) = setup(Nation::Earth, &rows, &[at(1, 2), at(5, 2)]);
    run_until(&mut w, &defs, 100_000, |w| w.units.is_empty());
    assert_eq!(w.deaths.len(), 2);
    assert!(w.deaths.iter().all(|d| d.cause == DeathCause::Starvation));
}

#[test]
fn clearing_a_blocking_tree_makes_the_far_side_reachable() {
    // A column of trees splits the map in two, and the unit is on the left. The far tree can't
    // be reached until the one in the middle of the column is gone: the connectivity cache has
    // to notice that.
    let rows = ["...T...", "...T.T.", "...T..."];
    let (mut w, defs) = setup(Nation::Earth, &rows, &[at(1, 1)]);
    let far = w.designate_harvest(at(5, 1), &defs).unwrap();
    for _ in 0..50 {
        w.tick(&defs);
    }
    assert_eq!(w.job(far).unwrap().claimed_by, None, "the far tree is across the column");

    let gate = w.designate_harvest(at(3, 1), &defs).unwrap();
    run_until(&mut w, &defs, 3000, |w| w.job(gate).is_none());
    // The cache is rebuilt for the opened map, not left holding the old split.
    w.tick(&defs);
    w.tick(&defs);
    let fresh = Regions::compute(&w.map, &w.rules);
    assert_eq!(w.regions().clone(), fresh);
    assert!(fresh.connected(at(1, 1), at(4, 1)));
    run_until(&mut w, &defs, 3000, |w| w.map[at(5, 1)].resource.is_none());
}

// ---- renewable food (#56) ----

/// The seconds a harvest takes to ripen again.
fn regrow_of(defs: &Defs, resource: &str) -> f32 {
    defs.harvest(defs.resource_id(resource).unwrap()).unwrap().regrow.unwrap()
}

#[test]
fn a_renewable_resource_stays_after_harvest_and_ripens_on_schedule() {
    let rows = ["~~~~~~~~~", ".........", "....Y....", ".........", "........."];
    let (mut w, defs) = setup(Nation::Earth, &rows, &[at(1, 3)]);
    let fruit = defs.item_id("fruit").unwrap();
    let job = w.designate_harvest(at(4, 2), &defs).unwrap();
    run_until(&mut w, &defs, 2000, |w| w.job(job).is_none());
    // The tree is still there, bare, and the pick dropped beside it.
    assert!(w.map[at(4, 2)].resource.is_some());
    assert!(w.is_resting(at(4, 2)));
    assert_eq!(w.ground_items().map(|(_, s)| s.count).sum::<u32>(), 2);
    assert_eq!(w.ground_items().next().map(|(_, s)| s.item), Some(fruit));
    assert!(w.resting_left(at(4, 2)).unwrap() > 0.0, "the TUI's hover (#299) counts down from it");
    assert_eq!(w.resting_left(at(0, 0)), None, "not resting");

    // Resting trees can't be picked or foraged, though they can be queued.
    let again = w.designate_harvest(at(4, 2), &defs).unwrap();
    w.units[0].needs.hunger = 5.0;
    for _ in 0..50 {
        w.tick(&defs);
    }
    assert_eq!(w.job(again).unwrap().claimed_by, None);
    assert!(w.is_resting(at(4, 2)));

    // It ripens `regrow` seconds after the pick (the clock has run 50 ticks), and then the
    // queued job is claimed and done.
    let ticks = run_until(&mut w, &defs, 10_000, |w| !w.is_resting(at(4, 2)));
    let total = (regrow_of(&defs, "fruit_tree") * 10.0) as i64;
    assert!((ticks as i64 + 50 - total).abs() <= 5, "{ticks} ticks left of {total}");
    run_until(&mut w, &defs, 2000, |w| w.job(again).is_none());
    assert!(w.is_resting(at(4, 2)), "picked again");
}

#[test]
fn the_difficulty_presets_regrow_knob_scales_the_resting_time() {
    // The difficulty preset's `regrow` knob (#294): Hard's resting time is `regrow` times
    // identity's. Hard also turns `seasons` on (#295) and `strain` above 0 (#296), which would
    // otherwise mix their own factors into this; turned off here so this test is only about
    // `regrow`.
    let rows = ["~~~~~~~~~", ".........", "....Y....", ".........", "........."];
    let (mut w, mut defs) = setup(Nation::Earth, &rows, &[at(1, 3)]);
    let hard = defs.difficulty.presets.iter_mut().find(|p| p.id == "hard").unwrap();
    hard.seasons = false;
    hard.strain = 0.0;
    w.difficulty = Some("hard".into());
    let regrow = w.knobs(&defs).regrow;
    assert!(regrow > 1.0, "test assumes hard's regrow knob is above identity");
    let job = w.designate_harvest(at(4, 2), &defs).unwrap();
    run_until(&mut w, &defs, 2000, |w| w.job(job).is_none());
    assert!(w.is_resting(at(4, 2)));
    let ticks = run_until(&mut w, &defs, 10_000, |w| !w.is_resting(at(4, 2)));
    let total = (regrow_of(&defs, "fruit_tree") * regrow * 10.0) as i64;
    assert!((ticks as i64 - total).abs() <= 5, "{ticks} ticks vs {total} expected");
}

#[test]
fn hungry_units_forage_a_plant_again_once_it_has_regrown() {
    let rows = ["~~~~~~~~~", ".........", "......c..", ".........", "........."];
    let (mut w, defs) = setup(Nation::Earth, &rows, &[at(1, 3)]);
    let eat = |w: &mut World| {
        w.units[0].needs.hunger = 10.0;
        w.tick(&defs);
        assert!(matches!(w.units[0].task(), Some(Task::Eat(FoodSource::Forage(_)))));
        run_until(w, &defs, 2000, |w| w.units[0].task().is_none());
    };
    eat(&mut w);
    assert!(w.is_resting(at(6, 2)));
    // Nothing else to eat: it waits, hungry, instead of eating the resting plant.
    w.units[0].needs.hunger = 10.0;
    for _ in 0..100 {
        w.tick(&defs);
        assert!(!matches!(w.units[0].task(), Some(Task::Eat(_))));
    }
    let wait = (regrow_of(&defs, "cabbage") * 10.0) as u64;
    run_until(&mut w, &defs, wait, |w| !w.is_resting(at(6, 2)));
    eat(&mut w);
    assert!(w.map[at(6, 2)].resource.is_some());
}

#[test]
fn fishing_needs_the_tech_and_runs_on_water_work() {
    let rows = ["~~~~~~~~~", "~~~f~~~~~", ".........", ".........", "........."];
    // Earth doesn't know fishing: no designation, and a starving unit doesn't fish either.
    let (mut earth, defs) = setup(Nation::Earth, &rows, &[at(1, 3)]);
    let fishing = defs.tech_id("fishing").unwrap();
    assert_eq!(
        earth.designate_harvest(at(3, 1), &defs),
        Err(DesignateError::NotResearched(fishing))
    );
    earth.units[0].needs.hunger = 5.0;
    earth.tick(&defs);
    assert_eq!(earth.units[0].task(), None);

    // Water starts with fishing. Fishing is 8 work-seconds at a modifier of 1.0 and Water's
    // water_work is 2.0, so about 4 seconds.
    let (mut w, defs) = setup(Nation::Water, &rows, &[at(3, 2)]);
    let job = w.designate_harvest(at(3, 1), &defs).unwrap();
    let ticks = run_until(&mut w, &defs, 5000, |w| w.job(job).is_none());
    assert!((ticks as i64 - 40).abs() <= 4, "{ticks} ticks");
    assert!(w.is_resting(at(3, 1)));
    assert_eq!(w.ground_items().map(|(_, s)| s.count).sum::<u32>(), 2);
}

#[test]
fn a_resting_resource_stops_resting_if_it_is_cleared() {
    let rows = ["~~~~~~~~~", ".........", "......c..", "........."];
    let (mut w, defs) = setup(Nation::Earth, &rows, &[at(1, 3)]);
    let job = w.designate_harvest(at(6, 2), &defs).unwrap();
    run_until(&mut w, &defs, 2000, |w| w.job(job).is_none());
    assert!(w.is_resting(at(6, 2)));
    w.map[at(6, 2)].resource = None;
    w.map_changed();
    w.tick(&defs);
    assert!(!w.is_resting(at(6, 2)));
}

// ---- standing harvest orders (#81) ----

/// Whether a `Harvest` job for `pos` is in the queue (claimed or not). A finished harvest also
/// leaves behind an unrelated `Haul` job for the pick lying on the ground (no stockpile exists in
/// these tests), so `jobs().is_empty()` never becomes true on its own.
fn has_harvest_job(w: &World, pos: Pos) -> bool {
    w.jobs().iter().any(|j| j.kind == JobKind::Harvest { pos })
}

#[test]
fn a_repeating_harvest_re_queues_itself_every_time_it_ripens() {
    let rows = ["~~~~~~~~~", ".........", "......c..", "........."];
    let (mut w, defs) = setup(Nation::Earth, &rows, &[at(1, 3)]);
    let pos = at(6, 2);
    w.designate_repeating_harvest(pos, &defs).unwrap();
    assert!(w.is_repeating_harvest(pos));
    assert!(w.harvest_designations().any(|p| p == pos));

    // Worked once, same as a one-shot harvest: the job is gone and the plant rests.
    run_until(&mut w, &defs, 2000, |w| !has_harvest_job(w, pos));
    assert!(w.is_resting(pos));
    // No job sits around while it rests, but the standing order still shows.
    assert!(!has_harvest_job(&w, pos));
    assert!(w.is_repeating_harvest(pos));
    assert!(w.harvest_designations().any(|p| p == pos));

    // Once it ripens, a fresh job appears without the player doing anything.
    let wait = (regrow_of(&defs, "cabbage") * 10.0) as u64 + 30;
    run_until(&mut w, &defs, wait, |w| !w.is_resting(pos));
    assert!(has_harvest_job(&w, pos));

    // And it keeps going a second time too.
    run_until(&mut w, &defs, 2000, |w| !has_harvest_job(w, pos));
    assert!(w.is_resting(pos));
    run_until(&mut w, &defs, wait, |w| !w.is_resting(pos));
    assert!(has_harvest_job(&w, pos));
}

#[test]
fn a_one_shot_harvest_does_not_re_queue_when_it_ripens() {
    let rows = ["~~~~~~~~~", ".........", "......c..", "........."];
    let (mut w, defs) = setup(Nation::Earth, &rows, &[at(1, 3)]);
    let pos = at(6, 2);
    w.designate_harvest(pos, &defs).unwrap();
    assert!(!w.is_repeating_harvest(pos));
    run_until(&mut w, &defs, 2000, |w| !has_harvest_job(w, pos));
    assert!(w.is_resting(pos));
    let wait = (regrow_of(&defs, "cabbage") * 10.0) as u64 + 30;
    run_until(&mut w, &defs, wait, |w| !w.is_resting(pos));
    assert!(!has_harvest_job(&w, pos), "a one-shot harvest stays one-shot");
}

#[test]
fn marking_a_resting_resource_repeating_queues_the_job_once_it_ripens() {
    let rows = ["~~~~~~~~~", ".........", "......c..", "........."];
    let (mut w, defs) = setup(Nation::Earth, &rows, &[at(1, 3)]);
    let pos = at(6, 2);
    w.designate_harvest(pos, &defs).unwrap();
    run_until(&mut w, &defs, 2000, |w| !has_harvest_job(w, pos));
    assert!(w.is_resting(pos));

    // Marked repeating while it's still resting: no job yet, it just waits for the ripening.
    w.designate_repeating_harvest(pos, &defs).unwrap();
    assert!(!has_harvest_job(&w, pos));
    let wait = (regrow_of(&defs, "cabbage") * 10.0) as u64 + 30;
    run_until(&mut w, &defs, wait, |w| !w.is_resting(pos));
    assert!(has_harvest_job(&w, pos));
}

#[test]
fn canceling_a_repeating_harvest_clears_the_standing_order() {
    let rows = ["~~~~~~~~~", ".........", "......c..", "........."];
    let (mut w, defs) = setup(Nation::Earth, &rows, &[at(1, 3)]);
    let pos = at(6, 2);
    w.designate_repeating_harvest(pos, &defs).unwrap();
    run_until(&mut w, &defs, 2000, |w| !has_harvest_job(w, pos));
    assert!(w.is_resting(pos));

    w.cancel_harvest(pos).unwrap();
    assert!(!w.is_repeating_harvest(pos));
    assert!(!w.harvest_designations().any(|p| p == pos));

    let wait = (regrow_of(&defs, "cabbage") * 10.0) as u64 + 30;
    run_until(&mut w, &defs, wait, |w| !w.is_resting(pos));
    assert!(!has_harvest_job(&w, pos), "the standing order was cancelled, so nothing re-queues");

    // Cancelling again, now that there is neither a job nor an order, is refused.
    assert_eq!(w.cancel_harvest(pos), Err(DesignateError::NotDesignated));
}

#[test]
fn designating_repeating_harvest_twice_does_not_duplicate_the_job() {
    let rows = ["~~~~~~~~~", ".........", "......c..", "........."];
    let (mut w, defs) = setup(Nation::Earth, &rows, &[at(1, 3)]);
    let pos = at(6, 2);
    w.designate_repeating_harvest(pos, &defs).unwrap();
    w.designate_repeating_harvest(pos, &defs).unwrap();
    assert_eq!(w.jobs().len(), 1);
}

// ---- rail (#135) ----

/// Lays rail on `tiles`, as a finished rail building would.
fn lay_rail(w: &mut World, defs: &Defs, tiles: impl IntoIterator<Item = Pos>) {
    let id = defs.building_id("rail").expect("rail is defined");
    for p in tiles {
        w.map[p].building = Some(crate::building::Building::new(id, true, true));
    }
    w.map_changed();
}

/// Ticks for one unit to haul `count` wood from (3, 2) to a stockpile at (32, 2) along a flat
/// row, with rail laid on `rail`.
fn haul_ticks(rail: &[Pos], count: u32) -> u64 {
    let rows = ["...................................."; 5];
    let (mut w, defs) = setup(Nation::Earth, &rows, &[at(2, 2)]);
    let wood = defs.item_id("wood").unwrap();
    lay_rail(&mut w, &defs, rail.iter().copied());
    w.designate_stockpile(at(32, 2)).unwrap();
    w.ground.insert(at(3, 2), Stack { item: wood, count });
    w.ensure_haul_job(at(3, 2));
    run_until(&mut w, &defs, 4000, |w| w.stored_total(wood) == count)
}

#[test]
fn hauling_over_rail_beats_hauling_over_bare_ground() {
    let line: Vec<Pos> = (3..=32).map(|x| at(x, 2)).collect();
    let bare = haul_ticks(&[], 5);
    let railed = haul_ticks(&line, 5);
    assert!(railed < bare, "rail {railed} ticks, bare ground {bare}");
    // Both legs run along the rail now (29 steps of 6 ticks on bare ground, 3 on rail), so the
    // trip loses well over a third of its time.
    assert!(railed * 4 < bare * 3, "rail {railed} ticks, bare ground {bare}: not much faster");
}

#[test]
fn a_gap_in_the_rail_costs_the_time_it_covers() {
    let line = |gap: bool| -> Vec<Pos> {
        (3..=32).filter(|&x| !(gap && (14..=21).contains(&x))).map(|x| at(x, 2)).collect()
    };
    let whole = haul_ticks(&line(false), 5);
    let gapped = haul_ticks(&line(true), 5);
    let bare = haul_ticks(&[], 5);
    assert!(whole < gapped && gapped < bare, "{whole} < {gapped} < {bare}");
}

#[test]
fn every_unit_runs_along_rail_loaded_or_not() {
    let rows = ["..........", "..........", ".........."];
    let (mut w, defs) = setup(Nation::Earth, &rows, &[at(1, 1)]);
    lay_rail(&mut w, &defs, (1..=6).map(|x| at(x, 1)));
    let wood = defs.item_id("wood").unwrap();
    w.units[0].route = Some((2..=5).rev().map(|x| at(x, 1)).collect());
    // Rail is a road for everyone: half the time on a speed-2 rail, on foot or hauling.
    let on_foot = w.stride(UnitId(0), &defs).unwrap().needed;
    assert_eq!(on_foot, vec![BASE_TICKS_PER_STEP / 2.0; 4]);
    w.units[0].carrying = Some(Stack { item: wood, count: 3 });
    let hauling = w.stride(UnitId(0), &defs).unwrap().needed;
    assert_eq!(hauling, on_foot);
}

#[test]
fn rail_only_speeds_a_straight_step_between_two_rail_tiles() {
    let rows = ["....", "....", "...."];
    let (mut w, defs) = setup(Nation::Earth, &rows, &[at(1, 1)]);
    lay_rail(&mut w, &defs, [at(1, 1), at(2, 1), at(3, 2), at(2, 2)]);
    let wood = defs.item_id("wood").unwrap();
    w.units[0].carrying = Some(Stack { item: wood, count: 1 });
    let needed = |w: &mut World, from: Pos, to: Pos| {
        w.units[0].pos = from;
        w.units[0].route = Some(vec![to]);
        w.stride(UnitId(0), &defs).unwrap().needed[0]
    };
    // Rail to rail, straight: fast. Rail to bare ground, and bare ground to rail: no gain.
    assert_eq!(needed(&mut w, at(1, 1), at(2, 1)), BASE_TICKS_PER_STEP / 2.0);
    assert_eq!(needed(&mut w, at(1, 1), at(1, 0)), BASE_TICKS_PER_STEP);
    assert_eq!(needed(&mut w, at(1, 0), at(1, 1)), BASE_TICKS_PER_STEP);
    // A diagonal between two rail tiles is an ordinary diagonal.
    let diagonal = needed(&mut w, at(2, 1), at(3, 2));
    assert!(diagonal > BASE_TICKS_PER_STEP, "{diagonal}");
}

#[test]
fn a_loaded_unit_delivers_to_the_stockpile_the_rail_makes_nearer() {
    // Two stockpiles: one 144 cost away over bare ground, one at the end of a rail line that is
    // 180 away on foot but 90 with a cart. A unit with a stack in hand takes the rail one.
    let rows = ["...................."; 7];
    let (mut w, defs) = setup(Nation::Earth, &rows, &[at(1, 1)]);
    let wood = defs.item_id("wood").unwrap();
    w.designate_stockpile(at(13, 6)).unwrap();
    w.designate_stockpile(at(19, 1)).unwrap();
    lay_rail(&mut w, &defs, (1..=19).map(|x| at(x, 1)));
    w.ground.insert(at(1, 1), Stack { item: wood, count: 4 });
    w.ensure_haul_job(at(1, 1));
    run_until(&mut w, &defs, 2000, |w| w.stored_total(wood) == 4);
    assert_eq!(w.stored_at(at(19, 1)), Some(Stack { item: wood, count: 4 }));
    assert_eq!(w.stored_at(at(13, 6)), None);
}

/// Idle (#337): nothing designated at all is `NoWork`, not a bare "it's idle" with no reason.
#[test]
fn idle_reason_says_no_work_with_nothing_designated() {
    let (w, defs) = setup(Nation::Earth, &WOODS, &[at(3, 2)]);
    assert_eq!(w.idle_reason(UnitId(0), &defs), Some(IdleReason::NoWork));
    assert_eq!(w.idle_reason(UnitId(99), &defs), None, "no such unit");
}

/// A bend the unit's own nation can cast, but can't afford yet, reads `OutOfChi` rather than a
/// bare `Unreachable`: `claim_job` itself would take it the moment chi regenerates.
#[test]
fn idle_reason_flags_out_of_chi_for_a_bend_the_unit_cannot_yet_afford() {
    let (mut w, defs) = setup(Nation::Fire, &WOODS, &[at(3, 2)]);
    let hearthflame = defs.ability_id("hearthflame").unwrap();
    assert!(defs.ability(hearthflame).chi > 10.0, "the fixture needs headroom below the cost");
    w.units[0].chi = 10.0;
    let pos = w.units[0].pos;
    w.designate_bend(hearthflame, pos, &defs).unwrap();
    assert_eq!(w.idle_reason(UnitId(0), &defs), Some(IdleReason::OutOfChi));
}

/// A haul with nowhere to go because every stockpile tile already holds a different item reads
/// `StockpileFull` (#337, #378's own gap in the balance harness).
#[test]
fn idle_reason_flags_a_full_stockpile_blocking_a_haul() {
    let (mut w, defs) = setup(Nation::Earth, &WOODS, &[at(3, 2)]);
    let wood = defs.item_id("wood").unwrap();
    let stone = defs.item_id("stone").unwrap();
    w.designate_stockpile(at(1, 2)).unwrap();
    w.stored.insert(at(1, 2), Stack { item: stone, count: 1 });
    w.ground.insert(at(5, 2), Stack { item: wood, count: 4 });
    w.ensure_haul_job(at(5, 2));
    assert_eq!(w.idle_reason(UnitId(0), &defs), Some(IdleReason::StockpileFull));
}

/// A harvest stranded on an island no unit can ever reach reads `Unreachable`.
#[test]
fn idle_reason_flags_an_unreachable_harvest() {
    let rows = ["~~~~~~~~~", "~T~.....~", "~~~.....~", "~~~..T..~", "~~~~~~~~~"];
    let (mut w, defs) = setup(Nation::Earth, &rows, &[at(3, 2)]);
    w.designate_harvest(at(1, 1), &defs).unwrap();
    assert_eq!(w.idle_reason(UnitId(0), &defs), Some(IdleReason::Unreachable));
}

/// `World::job_unreachable` (#337): `None` before the connectivity cache has ever been needed,
/// then `true` only for the designation on the far side of water with no bank, `false` for an
/// ordinary one in reach -- the same rows `unreachable_jobs_are_skipped_without_blocking_others`
/// uses to pin `claim_job` itself.
#[test]
fn job_unreachable_names_a_stranded_designation_once_the_cache_is_warm() {
    let rows = ["~~~~~~~~~", "~T~.....~", "~~~.....~", "~~~..T..~", "~~~~~~~~~"];
    let (mut w, defs) = setup(Nation::Earth, &rows, &[at(3, 2)]);
    assert_eq!(w.job_unreachable(at(1, 1)), None, "the cache hasn't been needed yet");
    w.designate_harvest(at(1, 1), &defs).unwrap();
    w.designate_harvest(at(5, 3), &defs).unwrap();
    w.tick(&defs);
    assert_eq!(w.job_unreachable(at(1, 1)), Some(true));
    assert_eq!(w.job_unreachable(at(5, 3)), Some(false));
}
