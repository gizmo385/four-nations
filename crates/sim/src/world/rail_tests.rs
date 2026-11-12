//! `metal_city` (#722): Metal Rail carries power along a run, `also_on`, and the `boost` upgrade.

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

fn at(x: i32, y: i32) -> Pos {
    Pos::new(x, y)
}

#[test]
fn metal_rail_and_the_boost_upgrade_are_validated() {
    let research = |up: &str| {
        let mut files = crate::defs::BUNDLED_FILES;
        let r = format!(
            "{}\n[[tech]]\nid = \"t\"\nname = \"T\"\ndescription = \"\"\ncost = 10\n\
             [tech.unlocks]\n[[tech.unlocks.upgrades]]\n{up}\n",
            files[3]
        );
        files[3] = &r;
        Defs::from_files(files).map(|_| ())
    };
    for (up, why) in [
        ("building = \"bed\"\nboost = 1.5", "boost needs [building.power]"),
        ("building = \"wind_turbine\"\nboost = 4.5", "must be 1.0 to 4"),
        ("building = \"hot_spring_mill\"\nboost = 2.0", "already upgraded by \"metal_city\""),
    ] {
        let e = research(up).expect_err(up).0;
        assert!(e.contains(why), "{up}: {e}");
    }
    let buildings = |from: &str, to: &str| {
        let mut files = crate::defs::BUNDLED_FILES;
        assert!(files[7].contains(from), "{from} moved");
        let b = files[7].replacen(from, to, 1);
        files[7] = &b;
        Defs::from_files(files).map(|_| ())
    };
    let e = buildings("id = \"earth_wall\"\n", "id = \"earth_wall\"\nalso_on = [\"tunnel\"]\n")
        .unwrap_err()
        .0;
    assert!(e.contains("passable 1x1"), "{e}");
    let e = buildings("power_reach = 1", "power_reach = 9").unwrap_err().0;
    assert!(e.contains("power_reach must be 1 to 4"), "{e}");

    let defs = Defs::bundled();
    let rail = defs.building(defs.building_id("metal_rail").unwrap());
    let track = rail.track.expect("a track");
    assert_eq!((track.step_cost(), track.power_reach), (4, Some(1)));
    assert_eq!(rail.also_on, [Terrain::Tunnel]);
    let mill = defs.building(defs.building_id("hot_spring_mill").unwrap());
    assert_eq!(mill.boost_upgrade, Some((defs.tech_id("metal_city").unwrap(), 1.5, 1500)));
}

/// An Earth world with a Hot Spring Mill at (2, 2), a forge at (30, 4), and metal rail along
/// y = 3 from x = 4 to x = 29 (`gap` left out).
fn line(defs: &Defs, gap: Option<i32>) -> (World, Footprint) {
    let mut w = World::with_map(1, field(40, 10), Nation::Earth, &[at(20, 8)], defs);
    let id = |s: &str| defs.building_id(s).unwrap();
    w.put_building(Footprint::new(at(2, 2), 2, 2), id("hot_spring_mill"), defs);
    let (fw, fh) = defs.building(id("forge")).footprint;
    let forge = Footprint::new(at(30, 4), fw, fh);
    w.put_building(forge, id("forge"), defs);
    for x in 4..=29 {
        if Some(x) != gap {
            w.put_building(Footprint::single(at(x, 3)), id("metal_rail"), defs);
        }
    }
    (w, forge)
}

#[test]
fn a_live_run_powers_a_consumer_beside_it_at_the_source_boost() {
    let defs = Defs::bundled();
    let (mut w, forge) = line(&defs, None);
    assert_eq!(w.rail_runs(&defs).len(), 1);
    let p = w.power_for(forge, &defs).expect("powered through the rail");
    assert_eq!((p.source, p.boost), (at(2, 2), 1.2));
    w.research.grant(defs.tech_id("metal_city").unwrap());
    assert_eq!(w.power_for(forge, &defs).unwrap().boost, 1.5, "the boost upgrade");
    assert_eq!(w.power_at(at(15, 4), &defs).map(|p| p.boost), Some(1.5), "one tile off the run");
    assert_eq!(w.power_at(at(15, 6), &defs), None, "three tiles off it");
}

#[test]
fn a_cut_run_stops_powering_the_far_end() {
    let defs = Defs::bundled();
    let (w, forge) = line(&defs, Some(12));
    assert_eq!(w.rail_runs(&defs).len(), 2);
    assert_eq!(w.power_for(forge, &defs), None);
}

#[test]
fn metal_rail_can_be_laid_in_a_tunnel() {
    let defs = Defs::bundled();
    let mut map = field(20, 10);
    map[at(5, 5)].terrain = Terrain::Tunnel;
    let mut w = World::with_map(1, map, Nation::Earth, &[at(10, 8)], &defs);
    w.research.grant(defs.tech_id("metal_city").unwrap());
    let rail = defs.building_id("metal_rail").unwrap();
    w.check_build(at(5, 5), rail, &defs).expect("rail in a tunnel");
    let road = defs.building_id("rail").unwrap();
    if let Some(t) = defs.building(road).unlocked_by {
        w.research.grant(t);
    }
    assert!(w.check_build(at(5, 5), road, &defs).is_err(), "stone rail has no also_on");
}
