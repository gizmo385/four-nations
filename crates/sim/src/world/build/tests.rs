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
    setup_as(Nation::Earth, rows, units)
}

fn setup_as(nation: Nation, rows: &[&str], units: &[Pos]) -> (World, Defs) {
    let defs = Defs::bundled();
    let map = arena(rows);
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
    let (mut w, mut defs) = setup(&GRID, &[at(1, 1)]);
    let wood = defs.item_id("wood").unwrap();
    let bed = defs.building_id("bed").unwrap();
    // Beds and campfires can be walked onto, so make the bed a wall for this test.
    defs.buildings[bed.0 as usize].passable = false;
    let def = defs.building(bed).clone();
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

#[test]
fn another_nation_s_building_is_refused_because_its_tech_can_never_be_known() {
    let defs = Defs::bundled();
    let mut checked = 0;
    for nation in Nation::ALL {
        let (mut w, _) = setup_as(nation, &GRID, &[at(1, 1)]);
        for (i, def) in defs.buildings.iter().enumerate() {
            let Some(tech) = def.unlocked_by else { continue };
            if defs.tech(tech).nation.is_none_or(|n| n == nation) {
                continue;
            }
            assert_eq!(
                w.designate_build(at(2, 2), BuildingId(i as u16), &defs),
                Err(BuildError::NotResearched(tech)),
                "{nation:?} building {}",
                def.id
            );
            // And the tech can't be researched to get round it.
            assert_eq!(
                w.apply(Command::SetResearch { tech }, &defs),
                Err(CommandError::Research(ResearchError::WrongNation))
            );
            checked += 1;
        }
    }
    assert!(checked > 0);
}

#[test]
fn a_locked_building_needs_its_tech_but_fire_starts_with_it() {
    let defs = Defs::bundled();
    let campfire = defs.building_id("campfire").expect("added by issue #22");
    let fire_making = defs.tech_id("fire_making").unwrap();

    // Water doesn't start with `fire_making`: the campfire is locked.
    let (mut w, _) = setup_as(Nation::Water, &GRID, &[at(1, 1)]);
    assert_eq!(
        w.designate_build(at(2, 2), campfire, &defs),
        Err(BuildError::NotResearched(fire_making))
    );
    assert!(!w.research().is_known(fire_making));

    // Once it finishes researching `fire_making` (25 points at 3/min, so tick 5000), it can.
    w.apply(Command::SetResearch { tech: fire_making }, &defs).unwrap();
    for _ in 0..5000 {
        w.tick(&defs);
    }
    assert!(w.research().is_known(fire_making));
    assert!(w.designate_build(at(2, 2), campfire, &defs).is_ok());

    // Fire Nation starts with `fire_making`, so it can build one from tick 0.
    let (mut fire, _) = setup_as(Nation::Fire, &GRID, &[at(1, 1)]);
    assert!(fire.research().is_known(fire_making));
    assert!(fire.designate_build(at(2, 2), campfire, &defs).is_ok());
}

#[test]
fn a_site_reports_its_building_delivery_and_progress() {
    let (mut w, defs) = setup(&GRID, &[at(1, 1)]);
    let bed = defs.building_id("bed").unwrap();
    assert_eq!(w.build_site(at(2, 2)), None);
    w.designate_build(at(2, 2), bed, &defs).unwrap();
    let (building, delivered, progress) = w.build_site(at(2, 2)).expect("designated");
    assert_eq!(building, bed);
    assert_eq!(delivered, vec![0; defs.building(bed).cost.len()]);
    assert_eq!(progress, 0.0);
    w.cancel_build(at(2, 2), &defs).unwrap();
    assert_eq!(w.build_site(at(2, 2)), None);
}
