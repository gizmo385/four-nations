//! One test per invariant: a healthy hand-built world passes, and breaking exactly that table
//! makes `check_invariants` name it.

use super::*;
use crate::building::Building;
use crate::defs::{BiomeId, Terrain};
use crate::geom::Grid;
use crate::mapgen::Tile;

fn arena() -> TileMap {
    Grid::from_fn(15, 15, |_| Tile {
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

/// Two settlers on an empty arena, which passes every check.
fn setup() -> (World, Defs) {
    let defs = Defs::bundled();
    let w = World::with_map(1, arena(), Nation::Earth, &[at(2, 2), at(3, 3)], &defs);
    w.check_invariants(&defs).expect("a fresh world is consistent");
    (w, defs)
}

/// Puts a finished building called `name` on `pos`, as if built.
fn raise(w: &mut World, defs: &Defs, name: &str, pos: Pos) {
    let id = defs.building_id(name).unwrap();
    w.map[pos].building = Some(Building::new(id, defs.building(id).passable, false));
}

fn err(w: &World, defs: &Defs) -> String {
    w.check_invariants(defs).expect_err("the world was broken by hand")
}

#[test]
fn units_out_of_order_fail() {
    let (mut w, defs) = setup();
    w.units.swap(0, 1);
    assert!(err(&w, &defs).contains("not sorted"));
}

#[test]
fn a_unit_off_the_map_fails() {
    let (mut w, defs) = setup();
    w.units[0].pos = at(40, 40);
    assert!(err(&w, &defs).contains("off the map"));
}

#[test]
fn a_claim_by_a_dead_unit_fails() {
    let (mut w, defs) = setup();
    w.add_job(JobKind::Study { pos: at(5, 5) });
    w.jobs[0].claimed_by = Some(UnitId(99));
    assert!(err(&w, &defs).contains("not alive"));
}

#[test]
fn a_claim_the_unit_does_not_hold_fails() {
    let (mut w, defs) = setup();
    w.add_job(JobKind::Study { pos: at(5, 5) });
    w.jobs[0].claimed_by = Some(w.units[0].id);
    assert!(err(&w, &defs).contains("whose job_task is"));
    w.units[0].job_task = Some(Task::Study(w.jobs[0].id));
    w.check_invariants(&defs).expect("now the unit holds what the job says");
}

#[test]
fn a_task_for_a_job_that_is_gone_fails() {
    let (mut w, defs) = setup();
    w.units[0].job_task = Some(Task::Harvest(JobId(7)));
    assert!(err(&w, &defs).contains("does not exist"));
}

#[test]
fn a_task_for_an_unclaimed_job_fails() {
    let (mut w, defs) = setup();
    let id = w.add_job(JobKind::Study { pos: at(5, 5) });
    w.units[0].job_task = Some(Task::Study(id));
    assert!(err(&w, &defs).contains("which is claimed by None"));
}

#[test]
fn a_bed_that_is_not_a_bed_fails() {
    let (mut w, defs) = setup();
    raise(&mut w, &defs, "bed", at(5, 5));
    w.units[0].bed = Some(at(5, 5));
    w.check_invariants(&defs).expect("a standing bed");
    w.units[0].bed = Some(at(6, 6));
    assert!(err(&w, &defs).contains("has bed"));
    raise(&mut w, &defs, "forge", at(8, 8));
    w.units[0].bed = Some(at(8, 8));
    assert!(err(&w, &defs).contains("has bed"));
}

#[test]
fn a_healing_building_that_is_not_one_fails() {
    let (mut w, defs) = setup();
    raise(&mut w, &defs, "healing_hut", at(5, 5));
    w.units[0].healing = Some(at(5, 5));
    w.check_invariants(&defs).expect("a standing healing hut");
    raise(&mut w, &defs, "bed", at(8, 8));
    w.units[0].healing = Some(at(8, 8));
    assert!(err(&w, &defs).contains("has healing"));
}

#[test]
fn inside_a_building_that_is_gone_fails() {
    let (mut w, defs) = setup();
    raise(&mut w, &defs, "study_hall", at(5, 5));
    w.units[0].inside = Some(at(5, 5));
    w.check_invariants(&defs).expect("a standing building");
    w.units[0].inside = Some(at(6, 6));
    assert!(err(&w, &defs).contains("has inside"));
}

#[test]
fn stored_off_a_stockpile_fails() {
    let (mut w, defs) = setup();
    let item = defs.item_id("wood").unwrap();
    w.stored.insert(at(4, 4), Stack { item, count: 3 });
    assert!(err(&w, &defs).contains("not a stockpile tile"));
    w.zones.insert(at(4, 4));
    w.check_invariants(&defs).expect("now it is a stockpile");
}

#[test]
fn a_haul_job_without_a_ground_stack_fails() {
    let (mut w, defs) = setup();
    w.add_job(JobKind::Haul { pos: at(4, 4) });
    assert!(err(&w, &defs).contains("no ground stack"));
    let item = defs.item_id("wood").unwrap();
    w.ground.insert(at(4, 4), Stack { item, count: 1 });
    w.check_invariants(&defs).expect("now there is a stack to haul");
}

#[test]
fn a_site_under_a_finished_building_fails() {
    let (mut w, defs) = setup();
    let bed = defs.building_id("bed").unwrap();
    w.apply(Command::DesignateBuild { pos: at(6, 6), building: bed }, &defs).expect("a bed site");
    w.check_invariants(&defs).expect("a plain site");
    raise(&mut w, &defs, "bed", at(6, 6));
    assert!(err(&w, &defs).contains("finished building"));
}

#[test]
fn a_craft_without_a_crafting_building_fails() {
    let (mut w, defs) = setup();
    w.crafts.insert(at(5, 5), CraftState::default());
    assert!(err(&w, &defs).starts_with("crafts"));
    raise(&mut w, &defs, "forge", at(5, 5));
    w.check_invariants(&defs).expect("a forge crafts");
}

#[test]
fn a_chart_without_a_map_room_fails() {
    let (mut w, defs) = setup();
    w.charts.insert(at(5, 5), 1.0);
    assert!(err(&w, &defs).starts_with("charts"));
    raise(&mut w, &defs, "map_room", at(5, 5));
    w.check_invariants(&defs).expect("a map room charts");
}

#[test]
fn a_farm_plot_without_a_plot_fails() {
    let (mut w, defs) = setup();
    w.open_farm_plot(at(5, 5));
    assert!(err(&w, &defs).starts_with("farm_plots"));
    raise(&mut w, &defs, "farm_plot", at(5, 5));
    w.check_invariants(&defs).expect("a farm plot farms");
}

#[test]
fn an_orchard_without_an_orchard_building_fails() {
    let (mut w, defs) = setup();
    let id = defs.building_id("orchard").unwrap();
    let orchard = defs.building(id).orchard.unwrap();
    w.open_orchard(at(5, 5), &orchard);
    assert!(err(&w, &defs).starts_with("orchards"));
    raise(&mut w, &defs, "orchard", at(5, 5));
    w.check_invariants(&defs).expect("an orchard orchards");
}

#[test]
fn a_power_source_without_a_generator_fails() {
    let (mut w, defs) = setup();
    w.open_power_source(at(5, 5));
    assert!(err(&w, &defs).starts_with("power_sources"));
    raise(&mut w, &defs, "wind_turbine", at(5, 5));
    w.check_invariants(&defs).expect("a turbine powers");
}

/// The invariant found these on `main` (#600): each took a ground stack away without the haul
/// job that pointed at it.
#[test]
fn eating_the_last_item_of_a_ground_stack_drops_its_haul_job() {
    let (mut w, defs) = setup();
    let item = defs.item_id("cabbage").unwrap();
    w.ground.insert(at(4, 4), Stack { item, count: 1 });
    w.add_job(JobKind::Haul { pos: at(4, 4) });
    w.take_food(0, FoodSource::Ground(at(4, 4)), item, &defs);
    w.check_invariants(&defs).expect("the stack and its haul job went together");
}

#[test]
fn fetching_the_last_of_a_ground_stack_for_a_build_drops_its_haul_job() {
    let (mut w, defs) = setup();
    let item = defs.item_id("wood").unwrap();
    w.ground.insert(at(4, 4), Stack { item, count: 2 });
    w.add_job(JobKind::Haul { pos: at(4, 4) });
    let took = w.take_for_build(0, build::Source::Ground(at(4, 4)), 5, &defs);
    assert_eq!(took, 2);
    w.check_invariants(&defs).expect("the stack and its haul job went together");
}

#[test]
fn raising_a_solid_building_on_a_ground_stack_moves_its_haul_job() {
    let (mut w, defs) = setup();
    let item = defs.item_id("wood").unwrap();
    w.ground.insert(at(6, 6), Stack { item, count: 2 });
    w.add_job(JobKind::Haul { pos: at(6, 6) });
    let id = defs.building_id("earth_wall").unwrap();
    assert!(!defs.building(id).passable);
    w.raise(crate::geom::Footprint::new(at(6, 6), 1, 1), id, defs.building(id));
    w.check_invariants(&defs).expect("the haul job followed the stack");
}

#[test]
fn assert_invariants_only_looks_every_check_every_ticks() {
    let (mut w, defs) = setup();
    w.units[0].pos = at(40, 40);
    w.tick = 1;
    w.assert_invariants(&defs);
    w.tick = CHECK_EVERY;
    let broken = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        w.assert_invariants(&defs);
    }));
    assert!(broken.is_err());
}
