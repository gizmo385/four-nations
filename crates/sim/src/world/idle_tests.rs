//! Idle units stand still most of the time (issue #110).

use super::*;
use crate::defs::{BiomeId, Terrain};
use crate::geom::Footprint;
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

/// Finished 1x1 buildings laid on `tiles`: beds, or solid earth walls.
fn lay(w: &mut World, tiles: impl IntoIterator<Item = Pos>, passable: bool) {
    let defs = Defs::bundled();
    let id = defs.building_id(if passable { "bed" } else { "earth_wall" }).unwrap();
    assert_eq!(defs.building(id).passable, passable);
    for p in tiles {
        w.put_building(Footprint::single(p), id, &defs);
    }
}

/// #559: an idle unit only ever steps onto ground it could path onto. It used to wander onto a
/// finished solid building's tiles (a study hall, a mill), and a settler standing on one with
/// trees and walls round it had no way out for the pathfinder, so the run-health probe read it
/// as stranded.
#[test]
fn an_idle_unit_never_wanders_onto_a_solid_building() {
    let defs = Defs::bundled();
    for seed in 0..8 {
        let mut w = world(seed, &defs);
        // A 2x2 hall right beside the unit, and a tree on its other side.
        let hall = [Pos::new(11, 10), Pos::new(12, 10), Pos::new(11, 11), Pos::new(12, 11)];
        lay(&mut w, hall, false);
        w.map[Pos::new(9, 10)].resource = defs.resource_id("oak_tree");
        let mut visited = std::collections::BTreeSet::new();
        for _ in 0..6_000 {
            w.tick(&defs);
            visited.insert(w.units[0].pos);
            assert!(
                !hall.contains(&w.units[0].pos),
                "seed {seed}: wandered onto the hall at {:?}",
                w.units[0].pos
            );
        }
        assert!(visited.len() > 3, "seed {seed}: it still wanders ({visited:?})");
    }
}

/// The other half: a unit that already stands on a solid tile (a building finished under its
/// route) still wanders its way out, through the building's own tiles if every neighbour is
/// blocked, and does not come back in. Without the way out it would be stuck there for good.
#[test]
fn an_idle_unit_on_a_solid_building_wanders_out_and_stays_out() {
    let defs = Defs::bundled();
    for seed in 0..8 {
        let mut w = world(seed, &defs);
        let hall = Footprint::new(Pos::new(10, 10), 2, 2);
        lay(&mut w, hall.tiles(), false);
        let gap = Pos::new(12, 10);
        for p in hall.ring().filter(|&p| p != gap) {
            w.map[p].resource = defs.resource_id("oak_tree");
        }
        w.units[0].pos = Pos::new(10, 11);
        let mut out = None;
        for t in 0..6_000 {
            w.tick(&defs);
            if out.is_none() && !hall.contains(w.units[0].pos) {
                out = Some(t);
            }
            if out.is_some() {
                assert!(!hall.contains(w.units[0].pos), "seed {seed}: it walked back in");
            }
        }
        assert!(out.is_some(), "seed {seed}: still inside the hall");
    }
}

/// A passable building is ground like any other: an idle unit does wander over a bed.
#[test]
fn an_idle_unit_still_wanders_over_a_bed() {
    let defs = Defs::bundled();
    let mut w = world(1, &defs);
    let beds: Vec<Pos> = (9..=11).flat_map(|x| (9..=11).map(move |y| Pos::new(x, y))).collect();
    lay(&mut w, beds.iter().copied(), true);
    let mut on_bed = false;
    for _ in 0..6_000 {
        w.tick(&defs);
        on_bed |= beds.contains(&w.units[0].pos) && w.units[0].pos != Pos::new(10, 10);
    }
    assert!(on_bed);
}

/// What #559's `stranded` flags were: the probe is reading the position right. A settler on a bed
/// in the middle of a dense cluster of beds can walk to the water (a bed is passable, so no
/// cluster of them walls anyone in), while one standing on a solid hall tile with trees all
/// round the hall cannot. Only wandering could put a settler on the hall tile (the test above).
#[test]
fn reach_says_a_bed_cluster_is_open_and_a_ringed_hall_is_not() {
    let defs = Defs::bundled();
    let mut w = world(1, &defs);
    for y in 0..21 {
        w.map[Pos::new(20, y)].terrain = Terrain::Water;
    }
    let cluster: Vec<Pos> = (8..=12).flat_map(|x| (8..=12).map(move |y| Pos::new(x, y))).collect();
    lay(&mut w, cluster.iter().copied(), true);
    w.units[0].pos = Pos::new(10, 10);
    assert!(w.reach(UnitId(0)).unwrap().water, "beds are passable, so a cluster seals nobody");

    let mut w = world(1, &defs);
    for y in 0..21 {
        w.map[Pos::new(20, y)].terrain = Terrain::Water;
    }
    let hall = Footprint::new(Pos::new(10, 10), 2, 2);
    lay(&mut w, hall.tiles(), false);
    for p in hall.ring() {
        w.map[p].resource = defs.resource_id("oak_tree");
    }
    w.units[0].pos = Pos::new(10, 10);
    assert!(!w.reach(UnitId(0)).unwrap().water, "a unit on a solid tile ringed by trees is stuck");
    w.units[0].pos = Pos::new(5, 5);
    assert!(w.reach(UnitId(0)).unwrap().water, "the same map is open from outside");
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
