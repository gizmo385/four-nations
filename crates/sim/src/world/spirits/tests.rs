use super::*;
use crate::defs::{BiomeId, Terrain};
use crate::geom::{Footprint, Grid};
use crate::mapgen::Tile;

fn field(width: i32, height: i32) -> TileMap {
    Grid::from_fn(width, height, |_| Tile {
        elevation: 100,
        temperature: 128,
        moisture: 128,
        terrain: Terrain::Land,
        biome: BiomeId(0),
        resource: None,
        building: None,
    })
}

/// A Fire world with `trees` oaks along the bottom rows, its gate standing if `united`.
fn city(defs: &Defs, trees: i32, united: bool) -> World {
    let mut map = field(40, 20);
    let oak = defs.resource_id("oak_tree").unwrap();
    for i in 0..trees {
        map.get_mut(Pos::new(i % 40, 19 - i / 40)).unwrap().resource = Some(oak);
    }
    let mut w = World::with_map(1, map, Nation::Fire, &[Pos::new(20, 8)], defs);
    if united {
        w.endeavor = EndeavorState::Complete { tick: 0 };
    }
    w
}

fn minute(w: &mut World, defs: &Defs) {
    w.tick = (w.tick / MINUTE + 1) * MINUTE;
    w.tick_spirits(defs);
}

#[test]
fn before_the_gate_there_is_no_balance_and_nothing_is_saved() {
    let defs = Defs::bundled();
    let mut w = city(&defs, 0, false);
    minute(&mut w, &defs);
    assert_eq!(w.balance(&defs), None);
    assert!(w.spirits.is_default());
}

#[test]
fn machine_draw_pulls_balance_down_and_trees_hold_it_up() {
    let defs = Defs::bundled();
    let mut w = city(&defs, 0, true);
    assert_eq!(w.balance(&defs), Some(100.0), "full at the gate");
    let id = |s: &str| defs.building_id(s).unwrap();
    w.put_building(Footprint::new(Pos::new(2, 2), 2, 2), id("lightning_station"), &defs);
    w.put_building(Footprint::new(Pos::new(5, 2), 3, 3), id("steel_foundry"), &defs);
    w.put_building(Footprint::new(Pos::new(2, 5), 3, 3), id("steel_foundry"), &defs);
    w.research.grant(defs.tech_id("great_works").unwrap());
    minute(&mut w, &defs);
    // Draw 8 at 0.25 is 2.0 unrest, against 1.0 calm.
    let terms = w.balance_terms().unwrap();
    assert_eq!((terms.machines, terms.base, terms.trees), (2.0, 1.0, 0.0));
    assert_eq!(w.balance(&defs), Some(99.0));
    // Fifty trees give another 1.0 calm, and the city holds.
    let mut green = city(&defs, 50, true);
    green.buildings = w.buildings.clone();
    green.research = w.research.clone();
    for (p, t) in w.map.iter() {
        if t.building.is_some() {
            green.map.get_mut(p).unwrap().building = t.building;
        }
    }
    minute(&mut green, &defs);
    assert_eq!(green.balance_terms().unwrap().trees, 1.0);
    assert_eq!(green.balance(&defs), Some(100.0));
}

#[test]
fn a_felled_tree_unsettles_the_spirits_once() {
    let defs = Defs::bundled();
    let mut w = city(&defs, 3, true);
    w.spirits.balance = Some(50.0);
    let tree = Pos::new(0, 19);
    w.note_felled(tree, &defs);
    w.note_felled(Pos::new(10, 10), &defs); // no tree there
    minute(&mut w, &defs);
    let terms = w.balance_terms().unwrap();
    assert_eq!(terms.felled, 1.0);
    assert_eq!(w.balance(&defs), Some(50.0), "1.0 calm, 1.0 unrest");
    minute(&mut w, &defs);
    assert_eq!(w.balance_terms().unwrap().felled, 0.0, "counted once");
    assert_eq!(w.balance(&defs), Some(51.0));
}
