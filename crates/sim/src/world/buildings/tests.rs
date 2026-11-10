//! The building records (#604): their indexes, the load-time checks, and the v2 upgrade.

use super::*;
use crate::defs::{BiomeId, Terrain};
use crate::geom::Grid;

fn at(x: i32, y: i32) -> Pos {
    Pos::new(x, y)
}

fn field() -> TileMap {
    Grid::from_fn(24, 16, |_| Tile {
        elevation: 100,
        temperature: 128,
        moisture: 128,
        terrain: Terrain::Land,
        biome: BiomeId(0),
        resource: None,
        building: None,
    })
}

/// A world with a bed, a 2x2 orchard part-way to planting, a campfire and a farm plot.
fn busy(defs: &Defs) -> World {
    let mut w = World::with_map(5, field(), Nation::Air, &[at(0, 0)], defs);
    let id = |name: &str| defs.building_id(name).unwrap();
    w.put_building(Footprint::single(at(8, 2)), id("bed"), defs);
    w.put_building(Footprint::new(at(12, 6), 2, 2), id("orchard"), defs);
    w.put_building(Footprint::single(at(3, 9)), id("campfire"), defs);
    w.put_building(Footprint::single(at(5, 9)), id("farm_plot"), defs);
    w.anchored_mut(at(12, 6)).unwrap().orchard.as_mut().unwrap().plant_in = 1.5;
    w
}

/// `w` turned back into the shape a v2 body had (`save/v2.rs`): whole buildings on every tile,
/// state in tables keyed by anchor, building fires in `heat`, and no records.
fn as_v2(w: &mut World) {
    let records = std::mem::take(&mut w.buildings);
    let old = w.map.map_with(|p, t| v2::Tile {
        elevation: t.elevation,
        temperature: t.temperature,
        moisture: t.moisture,
        terrain: t.terrain,
        biome: t.biome,
        resource: t.resource,
        building: t.building.map(|tb| {
            let r = records.get(tb.key()).unwrap();
            v2::Building {
                id: r.id,
                passable: tb.passable(),
                rail: tb.rail(),
                part: [(p.x - r.anchor.x) as u8, (p.y - r.anchor.y) as u8],
                size: r.size,
            }
        }),
    });
    for r in records.records.values() {
        if let Some(c) = &r.craft {
            w.v2_crafts.insert(r.anchor, c.clone());
        }
        if let Some(c) = r.chart {
            w.v2_charts.insert(r.anchor, c);
        }
        if let Some(o) = &r.orchard {
            w.v2_orchards.insert(r.anchor, o.clone());
        }
        if let Some(p) = &r.plot {
            w.v2_farm_plots.insert(r.anchor, p.clone());
        }
        if let Some(h) = r.heat {
            w.heat.insert(r.anchor, h);
        }
    }
    w.v2_map = old;
    w.map = Grid::default();
}

#[test]
fn the_listing_is_row_major_and_the_kinds_are_in_pos_order() {
    let defs = Defs::bundled();
    let mut w = World::with_map(5, field(), Nation::Air, &[at(0, 0)], &defs);
    let plot = defs.building_id("farm_plot").unwrap();
    // Row-major, (5, 1) comes first; in `Pos` order (x first), (2, 3) does.
    w.put_building(Footprint::single(at(2, 3)), plot, &defs);
    w.put_building(Footprint::single(at(5, 1)), plot, &defs);
    let rows: Vec<Pos> = w.buildings().map(|b| b.anchor).collect();
    assert_eq!(rows, [at(5, 1), at(2, 3)]);
    let farms: Vec<Pos> = w.of_kind(Kind::Farm).map(|b| b.anchor).collect();
    assert_eq!(farms, [at(2, 3), at(5, 1)]);
    assert!(w.of_kind(Kind::Orchard).next().is_none());
    assert!(w.of_kind(Kind::Irrigation).next().is_none());
}

#[test]
fn put_building_fills_the_state_its_def_calls_for() {
    let defs = Defs::bundled();
    let w = busy(&defs);
    assert!(w.anchored(at(3, 9)).unwrap().heat.is_some_and(|h| h.is_permanent()));
    assert!(w.is_heated(at(3, 9)) && w.flames().is_empty());
    assert_eq!(w.plot_stage(at(5, 9)), Some(PlotStage::Empty));
    assert_eq!(w.orchard_trees(at(12, 6)), Some(0));
    assert!(w.anchored(at(8, 2)).unwrap().heat.is_none());
    // State is asked for by anchor: another tile of the orchard has none of its own.
    assert_eq!(w.orchard_trees(at(13, 7)), None);
    assert_eq!(w.building_record(at(13, 7)).map(|b| b.anchor), Some(at(12, 6)));
    w.check_buildings().unwrap();
}

#[test]
fn a_building_fire_replaces_a_flame_on_its_anchor() {
    let defs = Defs::bundled();
    let mut w = World::with_map(5, field(), Nation::Fire, &[at(0, 0)], &defs);
    let flame = HeatSource {
        pos: at(4, 4),
        radius: 2,
        sleep_bonus: 1.2,
        seek_tiles: 10,
        until: 10_000,
        size: [1, 1],
        cooks: true,
        ability: None,
    };
    w.light_heat_source(flame);
    w.light_heat_source(HeatSource { pos: at(6, 4), ..flame });
    w.put_building(Footprint::single(at(4, 4)), defs.building_id("campfire").unwrap(), &defs);
    w.put_building(Footprint::single(at(6, 4)), defs.building_id("bed").unwrap(), &defs);
    assert!(!w.is_lit(at(4, 4)), "the campfire's own fire took the flame's place");
    assert!(w.is_lit(at(6, 4)), "a bed keeps no fire, so the flame on it burns on");
}

#[test]
fn a_v2_world_upgrades_to_the_same_records() {
    let defs = Defs::bundled();
    let w = busy(&defs);
    let mut old = w.clone();
    as_v2(&mut old);
    assert!(old.check_buildings().is_err(), "the shims are still full");
    old.upgrade_from_v2().unwrap();
    old.reindex_buildings(&defs);
    old.validate(&defs).unwrap();
    assert_eq!(old.buildings().count(), w.buildings().count());
    for b in w.buildings() {
        assert_eq!(old.anchored(b.anchor), Some(b));
        for p in b.footprint().tiles() {
            assert_eq!(old.building_record(p).map(|r| r.anchor), Some(b.anchor), "{p:?}");
            let passable = |w: &World| w.map[p].building.map(|t| t.passable());
            assert_eq!(passable(&old), passable(&w), "{p:?}");
        }
    }
    assert!(old.heat.is_empty(), "the campfire's fire moved into its record");
    assert_eq!(old.buildings.next, 4);
    // Keys are handed out row-major on upgrade: the bed's row comes first.
    assert_eq!(old.map[at(8, 2)].building.unwrap().key(), BuildingKey(0));
}

#[test]
fn the_upgrade_drops_state_anchored_where_no_building_stands() {
    // A 2x2 orchard saved with an entry on every tile (#624) is one orchard, the anchor's.
    let defs = Defs::bundled();
    let mut old = busy(&defs);
    as_v2(&mut old);
    let orchard = old.v2_orchards[&at(12, 6)].clone();
    old.v2_orchards.insert(at(13, 7), Orchard { plant_in: 9.0, ..orchard });
    old.v2_crafts.insert(at(20, 1), CraftState::default());
    old.upgrade_from_v2().unwrap();
    old.reindex_buildings(&defs);
    old.validate(&defs).unwrap();
    assert_eq!(old.orchards().collect::<Vec<_>>(), [(at(12, 6), 0)]);
    assert_eq!(old.anchored(at(12, 6)).unwrap().orchard.as_ref().unwrap().plant_in, 1.5);
}

#[test]
fn a_part_with_no_anchor_is_corrupt() {
    let defs = Defs::bundled();
    let mut old = busy(&defs);
    as_v2(&mut old);
    old.v2_map[at(12, 6)].building = None;
    assert!(old.upgrade_from_v2().unwrap_err().contains("no matching anchor"));
}

#[test]
fn validate_refuses_tiles_and_records_that_disagree() {
    let defs = Defs::bundled();
    let w = busy(&defs);
    w.validate(&defs).unwrap();
    // A tile pointing at a record that doesn't cover it.
    let mut bad = w.clone();
    let key = bad.map[at(8, 2)].building.unwrap().key();
    bad.map[at(0, 15)].building = Some(TileBuilding::new(key, true, false));
    assert!(bad.validate(&defs).is_err());
    // A footprint tile that doesn't point back.
    let mut bad = w.clone();
    bad.map[at(13, 7)].building = None;
    assert!(bad.validate(&defs).is_err());
    // A key past `next`.
    let mut bad = w.clone();
    bad.buildings.next = 1;
    assert!(bad.validate(&defs).is_err());
}

#[test]
fn the_invariant_refuses_indexes_that_drifted_from_the_records() {
    let defs = Defs::bundled();
    let w = busy(&defs);
    w.check_building_records(&defs).unwrap();
    let mut bad = w.clone();
    bad.buildings.kinds.clear();
    assert!(bad.check_building_records(&defs).unwrap_err().contains("per-kind"));
    let mut bad = w.clone();
    bad.buildings.rows.clear();
    assert!(bad.check_building_records(&defs).unwrap_err().contains("listing"));
}
