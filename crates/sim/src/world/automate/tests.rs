use super::*;
use crate::defs::{BiomeId, Terrain};
use crate::geom::{Footprint, Grid};
use crate::mapgen::Tile;
use crate::world::Command;

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

/// A Fire world with a Lightning Station at (2, 2) and a forge at `forge`, with 10 metal in a
/// stockpile at (20, 10).
fn yard(defs: &Defs, forge: Pos) -> World {
    let mut w = World::with_map(1, field(40, 20), Nation::Fire, &[at(20, 12)], defs);
    let id = |s: &str| defs.building_id(s).unwrap();
    w.put_building(Footprint::new(at(2, 2), 2, 2), id("lightning_station"), defs);
    w.put_building(Footprint::new(forge, 2, 2), id("forge"), defs);
    w.designate_stockpile(at(20, 10)).unwrap();
    w.stored.insert(at(20, 10), Stack { item: defs.item_id("metal").unwrap(), count: 10 });
    w
}

#[test]
fn automate_is_validated() {
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
        ("building = \"bed\"\nautomate = { rate = 1.0, radius = 4 }", "needs [building.power]"),
        ("building = \"wind_turbine\"\nautomate = { rate = 2.0, radius = 4 }", "0.1 to 1.0"),
        ("building = \"wind_turbine\"\nautomate = { rate = 1.0, radius = 9 }", "1 to 8"),
        (
            "building = \"lightning_station\"\nautomate = { rate = 0.5, radius = 2 }",
            "already upgraded by \"great_works\"",
        ),
    ] {
        let e = with(up).expect_err(up).0;
        assert!(e.contains(why), "{up}: {e}");
    }
    let defs = Defs::bundled();
    let station = defs.building(defs.building_id("lightning_station").unwrap());
    let (tech, a) = station.automate_upgrade.expect("great_works");
    assert_eq!((defs.tech(tech).id.as_str(), a.rate, a.radius), ("great_works", 1.0, 4));
}

#[test]
fn a_forge_in_the_yard_runs_with_no_crafter_and_a_supply_haul_feeds_it() {
    let defs = Defs::bundled();
    let forge = at(7, 2); // 3 tiles from the station's edge at x = 3
    let mut w = yard(&defs, forge);
    assert_eq!(w.automated(forge, &defs), None, "not before the tech");
    w.research.grant(defs.tech_id("great_works").unwrap());
    assert_eq!(w.automated(forge, &defs), Some(1.0));
    let tool = defs.item_id("tool").unwrap();
    let tools = |w: &World| -> u32 {
        w.ground.values().chain(w.stored.values()).filter(|s| s.item == tool).map(|s| s.count).sum()
    };
    let mut crafted_by_hand = false;
    for _ in 0..12_000 {
        w.tick(&defs);
        crafted_by_hand |= w.units.iter().any(|u| u.inside == Some(forge));
        if tools(&w) >= 2 {
            break;
        }
    }
    assert!(tools(&w) >= 2, "a batch came out");
    assert!(!crafted_by_hand, "nobody worked the bench");
}

#[test]
fn set_automated_toggles_and_is_refused_before_the_tech_and_outside_the_yard() {
    let defs = Defs::bundled();
    let (near, far) = (at(7, 2), at(20, 2));
    let mut w = yard(&defs, near);
    w.put_building(Footprint::new(far, 2, 2), defs.building_id("forge").unwrap(), &defs);
    let e = w.apply(Command::SetAutomated { pos: near, on: false }, &defs).unwrap_err();
    assert_eq!(e.to_string(), "needs Great Works");
    w.research.grant(defs.tech_id("great_works").unwrap());
    let e = w.apply(Command::SetAutomated { pos: far, on: true }, &defs).unwrap_err();
    assert!(e.to_string().starts_with("not in a ") && e.to_string().ends_with("'s yard"), "{e}");
    assert_eq!(w.automated(far, &defs), None);
    w.apply(Command::SetAutomated { pos: near, on: false }, &defs).unwrap();
    assert_eq!(w.automated(near, &defs), None, "back to a crafter");
    w.apply(Command::SetAutomated { pos: near, on: true }, &defs).unwrap();
    assert_eq!(w.automated(near, &defs), Some(1.0));
}
