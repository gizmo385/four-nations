use super::*;
use crate::defs::{BiomeId, Terrain};
use crate::geom::Grid;
use crate::mapgen::Tile;

/// A map from ASCII rows: `.` land, `~` water.
fn arena(rows: &[&str]) -> TileMap {
    Grid::from_fn(rows[0].len() as i32, rows.len() as i32, |p| {
        let terrain = match rows[p.y as usize].as_bytes()[p.x as usize] {
            b'~' => Terrain::Water,
            _ => Terrain::Land,
        };
        Tile {
            elevation: 100,
            temperature: 128,
            moisture: 128,
            terrain,
            biome: BiomeId(0),
            resource: None,
            building: None,
        }
    })
}

fn at(x: i32, y: i32) -> Pos {
    Pos::new(x, y)
}

fn setup(rows: &[&str], units: &[Pos]) -> (World, Defs) {
    let defs = Defs::bundled();
    let map = arena(rows);
    (World::with_map(1, map, Nation::Earth, units, &defs), defs)
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

const GRID: [&str; 5] = [".........", ".........", ".........", ".........", "........."];

#[test]
fn designating_is_checked() {
    let (mut w, defs) = setup(&GRID, &[at(1, 1)]);
    let bed = defs.building_id("bed").unwrap();
    assert_eq!(w.designate_build(at(99, 0), bed, &defs), Err(BuildError::OutOfBounds));
    assert_eq!(
        w.designate_build(at(2, 2), BuildingId(999), &defs),
        Err(BuildError::UnknownBuilding)
    );
    assert!(w.designate_build(at(2, 2), bed, &defs).is_ok());
    assert_eq!(w.designate_build(at(2, 2), bed, &defs), Err(BuildError::CannotBuild));
    assert_eq!(w.cancel_build(at(9, 9), &defs), Err(BuildError::NotDesignated));
}

#[test]
fn a_build_waits_until_a_unit_delivers_its_cost_then_finishes_the_work() {
    let (mut w, defs) = setup(&GRID, &[at(1, 1)]);
    let wood = defs.item_id("wood").unwrap();
    let bed = defs.building_id("bed").unwrap();
    let def = defs.building(bed).clone();
    w.designate_stockpile(at(0, 0)).unwrap();
    w.stored.insert(at(0, 0), Stack { item: wood, count: def.cost[0].count });
    let job = w.designate_build(at(5, 2), bed, &defs).unwrap();
    assert_eq!(w.build_sites().count(), 1);

    run_until(&mut w, &defs, 4000, |w| w.job(job).is_none());
    assert_eq!(w.building_at(at(5, 2)), Some(bed));
    assert_eq!(w.build_sites().count(), 0);
    // The cost was spent, not left lying around.
    assert_eq!(w.stored_total(wood), 0);
    assert_eq!(w.ground_items().count(), 0);
    assert_eq!(w.units[0].task(), None);
}

#[test]
fn a_build_waits_on_missing_materials_instead_of_failing() {
    let (mut w, defs) = setup(&GRID, &[at(1, 1)]);
    let bed = defs.building_id("bed").unwrap();
    let job = w.designate_build(at(5, 2), bed, &defs).unwrap();
    // No materials exist anywhere: the job waits (claim/abandon/retry) rather than erroring out
    // or vanishing.
    for _ in 0..500 {
        w.tick(&defs);
    }
    assert!(w.job(job).is_some(), "the job waits rather than failing outright");
    assert!(w.building_at(at(5, 2)).is_none());
    assert_eq!(w.build_sites().count(), 1);
}

#[test]
fn a_finished_building_blocks_pathing_when_its_def_says_so() {
    let (mut w, defs) = setup(&GRID, &[at(1, 1)]);
    let wood = defs.item_id("wood").unwrap();
    let bed = defs.building_id("bed").unwrap();
    let def = defs.building(bed).clone();
    assert!(!def.passable, "the starter building blocks movement");
    w.designate_stockpile(at(0, 0)).unwrap();
    w.stored.insert(at(0, 0), Stack { item: wood, count: def.cost[0].count });
    let job = w.designate_build(at(5, 2), bed, &defs).unwrap();
    run_until(&mut w, &defs, 4000, |w| w.job(job).is_none());
    assert!(w.map[at(5, 2)].building.is_some());
    assert!(!w.rules.passable(&w.map[at(5, 2)]), "a non-passable building blocks its own tile");
}

#[test]
fn cancelling_a_build_drops_delivered_materials_and_frees_the_unit() {
    let (mut w, defs) = setup(&GRID, &[at(1, 1)]);
    let bed = defs.building_id("bed").unwrap();
    let job = w.designate_build(at(5, 2), bed, &defs).unwrap();
    // Simulate a partial delivery: three of the five wood the bed costs have reached the site.
    w.sites.get_mut(&at(5, 2)).expect("just designated").delivered[0] = 3;

    w.cancel_build(at(5, 2), &defs).unwrap();
    assert!(w.job(job).is_none());
    assert!(w.building_at(at(5, 2)).is_none());
    assert!(w.build_sites().next().is_none());
    // What had been delivered is now on the ground somewhere, to be hauled again.
    assert_eq!(w.ground_items().map(|(_, s)| s.count).sum::<u32>(), 3);
}
