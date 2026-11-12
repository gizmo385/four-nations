//! `steam_power` and the `bank` upgrade (#757, docs/design/horizons-phase-6.md, "`steam_power`"):
//! a power source on the bank reaches further and burns no fuel.

use super::*;
use crate::building::FuelDef;
use crate::defs::{BUNDLED_FILES, BiomeId, DefsError, Terrain};
use crate::geom::{Footprint, Grid};
use crate::jobs::JobKind;
use crate::mapgen::Tile;

fn at(x: i32, y: i32) -> Pos {
    Pos::new(x, y)
}

/// A 40x12 field with a strip of `bank` terrain along the row `y = 0`.
fn shore(bank: Terrain) -> TileMap {
    Grid::from_fn(40, 12, |p| Tile {
        elevation: 100,
        temperature: 128,
        moisture: 128,
        terrain: if p.y == 0 { bank } else { Terrain::Land },
        biome: BiomeId(0),
        resource: None,
        building: None,
    })
}

fn with_upgrade(up: &str) -> Result<Defs, DefsError> {
    let mut files = BUNDLED_FILES;
    let research = format!(
        "{}\n[[tech]]\nid = \"t\"\nname = \"T\"\ndescription = \"\"\ncost = 10\n\
         [tech.unlocks]\n[[tech.unlocks.upgrades]]\n{up}\n",
        files[3]
    );
    files[3] = &research;
    Defs::from_files(files)
}

#[test]
fn the_bank_upgrade_is_validated() {
    for (up, why) in [
        (
            "building = \"bed\"\nbank = { beside = [\"water\"], radius = 4 }",
            "bank needs [building.power]",
        ),
        ("building = \"tide_mill\"\nbank = { beside = [], radius = 4 }", "at least one terrain"),
        ("building = \"tide_mill\"\nbank = { beside = [\"water\"], radius = 0 }", "1 to 12"),
        ("building = \"tide_mill\"\nbank = { beside = [\"water\"], radius = 13 }", "1 to 12"),
        ("building = \"tide_mill\"\nbank = { beside = [\"moat\"], radius = 4 }", "moat"),
        (
            "building = \"steam_engine\"\nbank = { beside = [\"water\"], radius = 4 }",
            "already upgraded by \"steam_power\"",
        ),
        (
            "building = \"tide_mill\"\nboost = 2.0\nbank = { beside = [\"water\"], radius = 4 }",
            "exactly one property",
        ),
    ] {
        let e = with_upgrade(up).map(|_| ()).expect_err(up).0;
        assert!(e.contains(why), "{up}: {e}");
    }
    // A power source other than the Steam Engine takes one too.
    with_upgrade("building = \"tide_mill\"\nbank = { beside = [\"water\"], radius = 6 }")
        .expect("a bank on the Tide Mill");
}

#[test]
fn the_bundled_steam_power_is_the_fire_water_fusion() {
    let defs = Defs::bundled();
    let id = defs.tech_id("steam_power").unwrap();
    let t = defs.tech(id);
    assert_eq!(t.fusion, Some([Nation::Fire, Nation::Water]));
    assert_eq!((t.cost, t.prereqs.clone()), (1000, vec![defs.tech_id("steam_engines").unwrap()]));
    let tools = defs.item_id("tool").unwrap();
    assert_eq!(t.cost_items.iter().map(|s| (s.item, s.count)).collect::<Vec<_>>(), [(tools, 6)]);
    assert_eq!(t.requires.masters, Some(1));
    let engine = defs.building(defs.building_id("steam_engine").unwrap());
    let (tech, bank) = engine.bank_upgrade.as_ref().expect("the engine's bank upgrade");
    assert_eq!(*tech, id);
    assert_eq!(bank.radius, 4);
    // The Tide Mill's banks, canals included.
    let mill = defs.building(defs.building_id("tide_mill").unwrap());
    assert_eq!(Some(&bank.beside), mill.beside.as_ref());
    assert!(bank.beside.contains(&Terrain::Canal));
}

/// The engine's reach in tiles at the settlement's current `power_generation`.
fn reach(w: &World, defs: &Defs, pos: Pos) -> u32 {
    w.power_def(pos, defs).unwrap().radius_for(w.modifiers().power_generation)
}

#[test]
fn an_engine_on_the_bank_reaches_as_far_as_a_tide_mill_and_one_inland_does_not() {
    let defs = Defs::bundled();
    let engine = defs.building_id("steam_engine").unwrap();
    let tech = defs.tech_id("steam_power").unwrap();
    for bank in [Terrain::Water, Terrain::River, Terrain::Lake, Terrain::Canal] {
        let mut w = World::with_map(1, shore(bank), Nation::Water, &[at(30, 8)], &defs);
        // Footprint rows 1-2: the ring takes in the water on row 0. Rows 6-7 touch nothing.
        w.put_building(Footprint::new(at(5, 1), 2, 2), engine, &defs);
        w.put_building(Footprint::new(at(15, 6), 2, 2), engine, &defs);
        let own = defs.building(engine).power.unwrap().radius;
        let pg = w.modifiers().power_generation;
        assert_eq!(reach(&w, &defs, at(5, 1)), (own as f32 * pg).round() as u32);
        // Before the tech nothing changes.
        assert!(!w.on_bank(w.building_record(at(5, 1)).unwrap(), &defs), "{bank:?}");
        w.research.grant(tech);
        assert!(w.on_bank(w.building_record(at(5, 1)).unwrap(), &defs), "{bank:?}");
        assert!(!w.on_bank(w.building_record(at(15, 6)).unwrap(), &defs), "{bank:?} inland");
        assert_eq!(reach(&w, &defs, at(5, 1)), (4.0 * pg).round() as u32, "{bank:?}");
        assert_eq!(reach(&w, &defs, at(15, 6)), (own as f32 * pg).round() as u32, "{bank:?}");
    }
}

#[test]
fn the_banks_reach_powers_a_consumer_the_plain_one_misses() {
    let defs = Defs::bundled();
    let engine = defs.building_id("steam_engine").unwrap();
    let hall = defs.building_id("workshop").unwrap();
    let mut w = World::with_map(1, shore(Terrain::Lake), Nation::Water, &[at(30, 8)], &defs);
    w.put_building(Footprint::new(at(5, 1), 2, 2), engine, &defs);
    // The plain reach is 3 tiles (Water's `power_generation` is 1.0), the bank's is 4: a
    // workshop 4 tiles off the engine's edge is out of reach, then in.
    let site = Footprint::new(at(10, 1), 2, 2);
    w.put_building(site, hall, &defs);
    assert_eq!(w.power_at(at(10, 1), &defs), None);
    w.research.grant(defs.tech_id("steam_power").unwrap());
    let p = w.power_at(at(10, 1), &defs).expect("in reach on the bank");
    assert_eq!(p.source, at(5, 1));
}

#[test]
fn the_base_radius_becomes_the_larger_of_its_own_and_the_banks() {
    let mut defs = Defs::bundled();
    let id = defs.building_id("steam_engine").unwrap();
    // A bank smaller than the engine's own radius changes nothing.
    defs.buildings[id.0 as usize].bank_upgrade.as_mut().unwrap().1.radius = 2;
    let mut w = World::with_map(1, shore(Terrain::Water), Nation::Fire, &[at(30, 8)], &defs);
    w.put_building(Footprint::new(at(5, 1), 2, 2), id, &defs);
    let before = reach(&w, &defs, at(5, 1));
    w.research.grant(defs.tech_id("steam_power").unwrap());
    assert!(w.on_bank(w.building_record(at(5, 1)).unwrap(), &defs));
    assert_eq!(reach(&w, &defs, at(5, 1)), before);
}

#[test]
fn an_engine_on_the_bank_posts_no_refuel_haul() {
    let mut defs = Defs::bundled();
    let id = defs.building_id("steam_engine").unwrap();
    // #433 gives the engine a bunker; this stands in for it.
    let wood = defs.item_id("wood").unwrap();
    defs.buildings[id.0 as usize].fuel = Some(FuelDef {
        item: wood,
        items: vec![wood],
        bunker: 6,
        per_minute: Some(1.0),
        micros_per_tick: 1000,
    });
    let refuels = |w: &World, p: Pos| w.jobs.iter().any(|j| j.kind == JobKind::Refuel { pos: p });
    for known in [false, true] {
        let mut w = World::with_map(1, shore(Terrain::Water), Nation::Fire, &[at(30, 8)], &defs);
        w.put_building(Footprint::new(at(5, 1), 2, 2), id, &defs); // on the bank
        w.put_building(Footprint::new(at(15, 6), 2, 2), id, &defs); // inland
        if known {
            w.research.grant(defs.tech_id("steam_power").unwrap());
        }
        let (bank, inland) =
            (w.building_record(at(5, 1)).unwrap(), w.building_record(at(15, 6)).unwrap());
        assert_eq!(w.fuel_lit(bank, &defs), known, "lit on the bank with an empty bunker");
        assert!(!w.fuel_lit(inland, &defs), "inland still needs fuel");
        w.tick(&defs);
        assert_eq!(refuels(&w, at(5, 1)), !known, "known: {known}");
        assert!(refuels(&w, at(15, 6)), "inland always asks, known: {known}");
    }
}
