//! Idle units stand still most of the time (issue #110).

use super::*;
use crate::defs::{BiomeId, Terrain};
use crate::geom::Grid;
use crate::mapgen::Tile;

fn field() -> TileMap {
    Grid::from_fn(21, 21, |_| Tile {
        elevation: 100,
        temperature: 128,
        moisture: 128,
        terrain: Terrain::Land,
        biome: BiomeId(0),
        resource: None,
        building: None,
    })
}

fn world(seed: u64, defs: &Defs) -> World {
    World::with_map(seed, field(), Nation::Earth, &[Pos::new(10, 10)], defs)
}

/// The unit's position after each of `ticks` ticks.
fn path(w: &mut World, defs: &Defs, ticks: u64) -> Vec<Pos> {
    (0..ticks)
        .map(|_| {
            w.tick(defs);
            w.units[0].pos
        })
        .collect()
}

#[test]
fn an_idle_unit_moves_on_only_a_minority_of_ticks() {
    let defs = Defs::bundled();
    let mut w = world(1, &defs);
    let start = w.units[0].pos;
    let ticks = 600;
    let positions = path(&mut w, &defs, ticks);
    let moved =
        std::iter::once(&start).chain(&positions).zip(&positions).filter(|(a, b)| a != b).count();
    assert!(moved > 0, "it still wanders now and then");
    assert!(moved * 4 < ticks as usize, "moved on {moved} of {ticks} ticks");
}

#[test]
fn the_same_seed_gives_the_same_idle_path_and_another_seed_differs() {
    let defs = Defs::bundled();
    let a = path(&mut world(5, &defs), &defs, 600);
    let b = path(&mut world(5, &defs), &defs, 600);
    let c = path(&mut world(6, &defs), &defs, 600);
    assert_eq!(a, b);
    assert_ne!(a, c);
}

#[test]
fn a_unit_standing_still_takes_a_new_job_on_the_next_tick() {
    let defs = Defs::bundled();
    let mut w = world(1, &defs);
    let tree = Pos::new(12, 10);
    w.map[tree].resource = defs.resource_id("oak_tree");
    // Mid-pause: it must not wait the pause out.
    w.units[0].idle_until = w.tick + 10_000;
    let job = w.designate_harvest(tree, &defs).unwrap();
    w.tick(&defs);
    assert_eq!(w.job(job).unwrap().claimed_by, Some(UnitId(0)));
}

#[test]
fn a_standing_unit_still_acts_on_a_low_need() {
    let defs = Defs::bundled();
    let mut w = world(1, &defs);
    w.units[0].idle_until = w.tick + 10_000;
    w.units[0].needs.energy = 1.0;
    w.tick(&defs);
    assert_eq!(w.units[0].task(), Some(Task::Sleep));
}

/// #226: a unit hidden inside a building never wanders off its door on its own, even though
/// `wander` doesn't otherwise know why it's standing still.
#[test]
fn a_unit_inside_a_building_never_wanders() {
    let defs = Defs::bundled();
    let mut w = world(1, &defs);
    let door = w.units[0].pos;
    w.units[0].inside = Some(Pos::new(3, 3));
    for _ in 0..600 {
        w.tick(&defs);
    }
    assert_eq!(w.units[0].pos, door, "a hidden unit's pos never moves on its own");
    assert_eq!(w.units[0].inside, Some(Pos::new(3, 3)));
}

#[test]
fn old_saves_without_the_idle_fields_still_load() {
    let defs = Defs::bundled();
    let w = world(1, &defs);
    let text = ron::to_string(&w.units[0]).unwrap();
    assert!(text.contains("idle_until:0") && text.contains("wander_left:0"), "{text}");
    let old = text.replace("idle_until:0,", "").replace("wander_left:0,", "");
    let u: Unit = ron::from_str(&old).unwrap();
    assert_eq!((u.idle_until, u.wander_left), (0, 0));
}
