//! `sun_warriors` (#724): campfires that heal and restore chi, and a Hothouse that needs no fuel.

use super::*;
use crate::defs::{BiomeId, Terrain};
use crate::geom::{Footprint, Grid};
use crate::jobs::JobKind;
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
fn aura_and_fuel_free_upgrades_are_validated() {
    let with = |up: &str| {
        let mut files = crate::defs::BUNDLED_FILES;
        let research = format!(
            "{}\n[[tech]]\nid = \"t\"\nname = \"T\"\ndescription = \"\"\ncost = 10\n\
             [tech.unlocks]\n[[tech.unlocks.upgrades]]\n{up}\n",
            files[3]
        );
        files[3] = &research;
        Defs::from_files(files).map(|_| ())
    };
    for (up, why) in [
        ("building = \"bed\"\naura = { heal = 2.0 }", "aura needs [building.heat]"),
        ("building = \"campfire\"\naura = { heal = 5.0 }", "must be 1.0 to 4"),
        ("building = \"campfire\"\naura = { heal = 1.0 }", "needs a multiplier above 1.0"),
        ("building = \"bed\"\nfuel_free = true", "no [building.fuel]"),
        ("building = \"tea_house\"\nfuel_free = false", "fuel_free can only be true"),
        ("building = \"hothouse\"\nfuel_free = true", "already upgraded by \"sun_warriors\""),
    ] {
        let e = with(up).expect_err(up).0;
        assert!(e.contains(why), "{up}: {e}");
    }
    let defs = Defs::bundled();
    let tech = defs.tech_id("sun_warriors").unwrap();
    let fire = defs.building(defs.building_id("campfire").unwrap());
    let (t, aura) = fire.heat_aura_upgrade.expect("campfire aura");
    assert_eq!((t, aura.heal, aura.chi_regen, aura.study), (tech, 2.0, 1.5, 1.0));
    let hothouse = defs.building(defs.building_id("hothouse").unwrap());
    assert_eq!(hothouse.fuel_free_upgrade, Some(tech));
    assert_eq!(defs.school(defs.techs[tech.0 as usize].school.unwrap()).id, "fire_purpose");
}

#[test]
fn a_campfire_heals_and_restores_chi_in_its_heat_reach_once_known() {
    let defs = Defs::bundled();
    let mut w = World::with_map(1, field(30, 10), Nation::Fire, &[at(25, 8)], &defs);
    let fire = defs.building_id("campfire").unwrap();
    w.put_building(Footprint::single(at(5, 5)), fire, &defs);
    let reach = w.building_record(at(5, 5)).unwrap().heat.unwrap().radius as i32;
    let (near, far) = (at(5 + reach, 5), at(6 + reach, 5));
    assert_eq!(w.aura_at(near, &defs), aura::AuraEffect::default());
    w.research.grant(defs.tech_id("sun_warriors").unwrap());
    let a = w.aura_at(near, &defs);
    assert_eq!((a.heal, a.chi_regen, a.study_millis), (2.0, 1.5, 1000));
    assert_eq!(w.aura_at(far, &defs), aura::AuraEffect::default());
}

#[test]
fn a_fuel_free_hothouse_works_empty_and_posts_no_refuel() {
    let defs = Defs::bundled();
    let mut w = World::with_map(1, field(30, 10), Nation::Fire, &[at(25, 8)], &defs);
    let id = defs.building_id("hothouse").unwrap();
    w.put_building(Footprint::new(at(5, 5), 2, 2), id, &defs);
    w.tick(&defs);
    assert!(w.jobs.iter().any(|j| j.kind == JobKind::Refuel { pos: at(5, 5) }), "empty: refuel");
    let b = w.building_record(at(5, 5)).unwrap();
    assert!(!w.fuel_lit(b, &defs));
    w.research.grant(defs.tech_id("sun_warriors").unwrap());
    let b = w.building_record(at(5, 5)).unwrap();
    assert!(w.fuel_lit(b, &defs), "lit with nothing in it");
    for _ in 0..50 {
        w.tick(&defs);
    }
    assert!(!w.jobs.iter().any(|j| j.kind == JobKind::Refuel { pos: at(5, 5) }), "no haul");
}
