use super::*;
use crate::building::Building;
use crate::defs::{BiomeId, Terrain};
use crate::geom::Grid;
use crate::mapgen::Tile;
use crate::world::{Command, Task};

/// A map of plain, walkable land.
fn arena(rows: &[&str]) -> TileMap {
    Grid::from_fn(rows[0].len() as i32, rows.len() as i32, |_| Tile {
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

fn setup(nation: Nation, rows: &[&str], units: &[Pos]) -> (World, Defs) {
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

/// Puts a finished study hall at `pos` (skipping the construction job itself, which `build.rs`
/// already covers) and opens its seats, the way `run_build` does the moment one completes.
fn place_study_hall(w: &mut World, defs: &Defs, pos: Pos) {
    let hall = defs.building_id("study_hall").expect("study hall");
    let def = defs.building(hall);
    w.map[pos].building = Some(Building { id: hall, passable: def.passable });
    w.open_study_seats(pos, def.seats);
}

#[test]
fn a_finished_study_hall_opens_one_job_per_seat() {
    let (mut w, defs) = setup(Nation::Earth, &GRID, &[at(2, 2)]);
    let seats = defs.building(defs.building_id("study_hall").unwrap()).seats;
    place_study_hall(&mut w, &defs, at(2, 2));
    assert_eq!(w.jobs().len(), seats as usize);

    // The unit starts right on the hall's tile, so it claims a seat on its very first tick.
    w.tick(&defs);
    assert!(matches!(w.units[0].task(), Some(Task::Study(_))), "{:?}", w.units[0].task());
}

#[test]
fn a_scholar_studying_adds_the_expected_knowledge_each_tick() {
    let (mut w, defs) = setup(Nation::Earth, &GRID, &[at(2, 2)]);
    place_study_hall(&mut w, &defs, at(2, 2));
    w.tick(&defs);
    assert!(matches!(w.units[0].task(), Some(Task::Study(_))));
    assert_eq!(w.modifiers().study, 1.0, "Earth has no study bonus yet");

    let passive = defs.research.passive_millis_per_tick;
    let study = defs.research.study_millis_per_tick;
    let minutes = 10;
    let ticks = u64::from(TICKS_PER_SECOND) * 60 * minutes;
    let before = w.research().pool_millis();
    for _ in 0..ticks {
        w.tick(&defs);
    }
    let gained = w.research().pool_millis() - before;
    assert_eq!(gained, ticks * (passive + study));
}

#[test]
fn a_hungry_scholar_leaves_to_eat_and_stops_producing() {
    let (mut w, defs) = setup(Nation::Earth, &GRID, &[at(2, 2)]);
    place_study_hall(&mut w, &defs, at(2, 2));
    w.tick(&defs);
    assert!(matches!(w.units[0].task(), Some(Task::Study(_))));

    // Food within reach, so the unit can actually act on its hunger.
    let fish = defs.item_id("fish").unwrap();
    w.ground.insert(at(0, 0), Stack { item: fish, count: 5 });
    w.units[0].needs.hunger = defs.needs.hunger.low;

    let before = w.research().pool_millis();
    w.tick(&defs);
    assert!(matches!(w.units[0].task(), Some(Task::Eat(_))), "{:?}", w.units[0].task());
    // Needs pre-empt the seat before it can add anything this tick: only the passive trickle
    // moves the pool.
    assert_eq!(w.research().pool_millis() - before, defs.research.passive_millis_per_tick);

    // It goes back to its seat once fed.
    run_until(&mut w, &defs, 2000, |w| matches!(w.units[0].task(), Some(Task::Study(_))));
}

#[test]
fn the_study_modifier_scales_how_fast_a_scholar_adds_knowledge() {
    let (mut w, defs) = setup(Nation::Air, &GRID, &[at(2, 2)]);
    place_study_hall(&mut w, &defs, at(2, 2));
    w.tick(&defs);
    assert!(matches!(w.units[0].task(), Some(Task::Study(_))));

    let passive = defs.research.passive_millis_per_tick;
    let base_study = defs.research.study_millis_per_tick;
    let before = w.research().pool_millis();
    w.tick(&defs);
    assert_eq!(w.research().pool_millis() - before, passive + base_study);

    // `meditation` has no prereqs and raises Air's `study` modifier to 1.2 (data/research.toml).
    // The scholar's own output, plus the passive trickle, pays for it.
    let meditation = defs.tech_id("meditation").unwrap();
    w.apply(Command::SetResearch { tech: meditation }, &defs).unwrap();
    run_until(&mut w, &defs, 10_000, |w| w.research().is_known(meditation));
    assert_eq!(w.modifiers().study, 1.2);

    let before = w.research().pool_millis();
    w.tick(&defs);
    let boosted_study = (base_study as f32 * 1.2).round() as u64;
    assert_ne!(boosted_study, base_study);
    assert_eq!(w.research().pool_millis() - before, passive + boosted_study);
}
