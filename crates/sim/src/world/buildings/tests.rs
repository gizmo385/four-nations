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
                rail: tb.track() != 0,
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
    bad.map[at(0, 15)].building = Some(TileBuilding::new(key, true, 0));
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

/// Unit `i` stood on the door of the building anchored at `anchor` and stepped inside.
fn seat_inside(w: &mut World, i: usize, anchor: Pos) {
    let door = w.footprint_at(anchor).unwrap().entrance();
    w.units[i].pos = door;
    w.units[i].inside = Some(anchor);
}

/// Anything in the world, bar the tiles, that still names `anchor`: a unit's place inside, bed,
/// healing spot or task, or an open job.
fn refers_to(w: &World, anchor: Pos) -> Option<String> {
    for u in &w.units {
        if u.inside == Some(anchor) || u.bed == Some(anchor) || u.healing == Some(anchor) {
            return Some(format!("unit {} is bound to it", u.id.0));
        }
        if u.need_task == Some(Task::Healing(anchor)) {
            return Some(format!("unit {} is healing in it", u.id.0));
        }
    }
    let at_anchor = |k: &JobKind| {
        matches!(k, JobKind::Study { pos } | JobKind::Chart { pos } | JobKind::Craft { pos }
            | JobKind::Sow { pos } | JobKind::Reap { pos } if *pos == anchor)
    };
    w.jobs.iter().find(|j| at_anchor(&j.kind)).map(|j| format!("job {} is still open", j.id.0))
}

#[test]
fn removing_a_building_in_use_leaves_nothing_pointing_at_it() {
    let defs = Defs::bundled();
    let mut tested = 0;
    for def in defs.buildings.iter().filter(|d| !d.endeavor) {
        let name = &def.id;
        let id = defs.building_id(name).unwrap();
        let anchor = at(10, 6);
        let foot = Footprint::new(anchor, def.footprint.0, def.footprint.1);
        let units = [at(1, 1), at(2, 1), at(3, 1), at(4, 1)];
        let mut w = World::with_map(5, field(), Nation::Earth, &units, &defs);
        // Another building elsewhere, which must be left alone.
        let campfire = defs.building_id("campfire").unwrap();
        let bystander = w.put_building(Footprint::single(at(1, 12)), campfire, &defs);
        w.raise(foot, id, &defs);
        let key = w.map[anchor].building.unwrap().key();

        // In use, each only where the def has a use for it: a scholar seated or a crafter
        // mid-batch, a sleeper in the bed, a patient healing, a farmhand with a Sow job, and
        // otherwise a unit inside.
        let seat = w.jobs.iter().find_map(|j| match j.kind {
            JobKind::Study { pos } if pos == anchor => Some((j.id, Task::Study(j.id))),
            JobKind::Chart { pos } if pos == anchor => Some((j.id, Task::Chart(j.id))),
            JobKind::Craft { pos } if pos == anchor => Some((j.id, Task::Craft(j.id))),
            _ => None,
        });
        if let Some((job, task)) = seat {
            let j = w.job_index(job).unwrap();
            w.jobs[j].claimed_by = Some(w.units[0].id);
            w.units[0].job_task = Some(task);
            seat_inside(&mut w, 0, anchor);
        }
        if let Some(recipe) = &def.recipe {
            w.anchored_mut(anchor).unwrap().craft =
                Some(CraftState { delivered: vec![1; recipe.inputs.len()], progress: 2.5 });
        }
        if def.bed.is_some() {
            w.units[1].bed = Some(anchor);
            w.units[1].need_task = Some(Task::Sleep);
            seat_inside(&mut w, 1, anchor);
        }
        if def.healing.is_some() {
            w.units[2].healing = Some(anchor);
            w.units[2].need_task = Some(Task::Healing(anchor));
            seat_inside(&mut w, 2, anchor);
        }
        if def.farm.is_some() {
            let job = w.add_job(JobKind::Sow { pos: anchor });
            let j = w.job_index(job).unwrap();
            w.jobs[j].claimed_by = Some(w.units[3].id);
            w.units[3].job_task = Some(Task::Sow(job));
        } else {
            seat_inside(&mut w, 3, anchor);
        }
        w.validate(&defs).unwrap();
        w.check_invariants(&defs).unwrap_or_else(|e| panic!("{name} before: {e}"));
        let before = w.map_rev;

        let record = w.remove_building(key, &defs).unwrap_or_else(|| panic!("{name}"));
        assert_eq!(record.anchor, anchor, "{name}");
        assert_eq!(record.craft.is_some(), def.recipe.is_some(), "{name}: the batch comes back");
        w.validate(&defs).unwrap_or_else(|e| panic!("{name}: {e}"));
        w.check_invariants(&defs).unwrap_or_else(|e| panic!("{name}: {e}"));
        assert!(w.map_rev > before, "{name}: the claim memo must be told");
        assert!(w.buildings().all(|b| b.anchor != anchor), "{name}: still listed");
        assert_eq!(w.buildings().count(), 1, "{name}: the bystander stays");
        assert!(w.buildings.get(bystander).is_some(), "{name}");
        assert!(foot.tiles().all(|p| w.map[p].building.is_none()), "{name}: a tile points on");
        assert_eq!(refers_to(&w, anchor), None, "{name}");
        assert!(w.remove_building(key, &defs).is_none(), "{name}: removed twice");
        for u in &w.units {
            assert!(u.job_task.is_none() && u.need_task.is_none(), "{name}: {:?}", u.task());
        }

        // Putting one back on the same anchor starts from nothing, under a new key.
        let again = w.put_building(foot, id, &defs);
        assert_ne!(again, key, "{name}: keys aren't reused");
        let fresh = w.anchored(anchor).unwrap();
        assert!(fresh.craft.is_none() && fresh.chart.is_none(), "{name}");
        w.check_invariants(&defs).unwrap_or_else(|e| panic!("{name}: {e}"));
        tested += 1;
    }
    assert!(tested >= 10, "only {tested} kinds tried");
}
