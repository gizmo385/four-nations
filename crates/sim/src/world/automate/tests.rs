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
    let (tech, a) = station.automate_upgrade.clone().expect("great_works");
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

/// A `nation` world with its power `source` at (2, 2) and the recipe building `machine` at
/// `pos`, `terrain` laid on each of `tiles` first, and `mechanisms` known (#1037).
fn machine_yard(
    defs: &Defs,
    nation: Nation,
    source: &str,
    machine: &str,
    pos: Pos,
    tiles: &[(Pos, Terrain)],
) -> World {
    let mut map = field(40, 20);
    for &(p, terrain) in tiles {
        map.get_mut(p).unwrap().terrain = terrain;
    }
    let mut w = World::with_map(1, map, nation, &[at(20, 12)], defs);
    let id = |s: &str| defs.building_id(s).unwrap();
    let size = |s: &str| defs.building(id(s)).footprint;
    let (sw, sh) = size(source);
    w.put_building(Footprint::new(at(2, 2), sw, sh), id(source), defs);
    let (mw, mh) = size(machine);
    w.put_building(Footprint::new(pos, mw, mh), id(machine), defs);
    w
}

/// A Fire yard with a Smelter at (5, 2) touching a Steelworks at (7, 2), both automated, and
/// ore and wood in stores (#1038).
fn smelter_row(defs: &Defs) -> World {
    let mut w = World::with_map(1, field(40, 20), Nation::Fire, &[at(20, 12), at(21, 12)], defs);
    let id = |s: &str| defs.building_id(s).unwrap();
    w.put_building(Footprint::new(at(2, 2), 2, 2), id("lightning_station"), defs);
    w.put_building(Footprint::new(at(5, 2), 2, 2), id("smelter"), defs);
    w.put_building(Footprint::new(at(7, 2), 3, 2), id("steelworks"), defs);
    for (x, item) in [(20, "gold_ore"), (21, "wood")] {
        w.designate_stockpile(at(x, 10)).unwrap();
        w.stored.insert(at(x, 10), Stack { item: defs.item_id(item).unwrap(), count: 60 });
    }
    w.research.grant(defs.tech_id("great_works").unwrap());
    w
}

#[test]
fn mechanisms_runs_each_nations_machines_at_its_own_rate() {
    let defs = Defs::bundled();
    let mechanisms = defs.tech_id("mechanisms").unwrap();
    // Air: the Wind Turbine runs a Tea Works anywhere in its wide reach, slowest.
    let mut air = machine_yard(&defs, Nation::Air, "wind_turbine", "tea_works", at(8, 2), &[]);
    assert_eq!(air.automated(at(8, 2), &defs), None, "not before the tech");
    air.research.grant(mechanisms);
    assert_eq!(air.automated(at(8, 2), &defs), Some(0.5));
    // Water: the Tide Mill runs a Smokehouse only along the water.
    let shore = [(at(9, 2), Terrain::River)];
    for (tiles, want) in [(&[][..], None), (&shore[..], Some(0.75))] {
        let mut w = machine_yard(&defs, Nation::Water, "tide_mill", "smokehouse", at(7, 2), tiles);
        w.research.grant(mechanisms);
        assert_eq!(w.automated(at(7, 2), &defs), want, "{tiles:?}");
    }
    // Earth: the Hot Spring Mill runs a Workshop only at the rock face.
    let rock = [(at(5, 4), Terrain::Mountain)];
    for (tiles, want) in [(&[][..], None), (&rock[..], Some(0.6))] {
        let mut w =
            machine_yard(&defs, Nation::Earth, "hot_spring_mill", "workshop", at(5, 2), tiles);
        w.research.grant(mechanisms);
        assert_eq!(w.automated(at(5, 2), &defs), want, "{tiles:?}");
    }
}

#[test]
fn mechanisms_is_shared_waits_for_the_gate_and_bills_steel() {
    let defs = Defs::bundled();
    let t = defs.tech(defs.tech_id("mechanisms").unwrap());
    assert!(t.nation.is_none() && t.requires.united);
    assert!(t.cost_items.iter().any(|c| defs.item(c.item).id == "steel"));
    // Fire's own yard keeps Great Works, at the best rate.
    let station = defs.building(defs.building_id("lightning_station").unwrap());
    let (tech, a) = station.automate_upgrade.as_ref().unwrap();
    assert_eq!((defs.tech(*tech).id.as_str(), a.rate), ("great_works", 1.0));
}

#[test]
fn a_smelter_hands_its_metal_straight_to_the_touching_steelworks() {
    let defs = Defs::bundled();
    let mut w = smelter_row(&defs);
    let metal = defs.item_id("metal").unwrap();
    assert_eq!(w.touching_takers(at(5, 2), metal, &defs), vec![at(7, 2)]);
    let mut metal_loose = false;
    for _ in 0..30_000 {
        w.tick(&defs);
        metal_loose |= w.ground.values().chain(w.stored.values()).any(|s| s.item == metal);
        if w.tallies().school("steel_made") > 0 {
            break;
        }
    }
    assert!(w.tallies().school("steel_made") > 0, "the Steelworks made steel");
    assert!(w.tallies().school("passed_across") >= 2, "the metal went across");
    assert!(!metal_loose, "no metal was ever hauled or stored");
}

#[test]
fn routed_to_stores_the_metal_goes_out_the_door_instead() {
    let defs = Defs::bundled();
    let mut w = smelter_row(&defs);
    w.apply(Command::SetShare { pos: at(5, 2), to: SplitTo::Stores, weight: 1 }, &defs).unwrap();
    assert_eq!(w.split_of(at(5, 2)), &[Share { to: SplitTo::Stores, weight: 1 }]);
    let metal = defs.item_id("metal").unwrap();
    let mut metal_out = false;
    for _ in 0..20_000 {
        w.tick(&defs);
        metal_out |= w.ground.values().chain(w.stored.values()).any(|s| s.item == metal);
        if metal_out {
            break;
        }
    }
    assert!(metal_out, "the Smelter's metal left by the door");
    assert_eq!(w.tallies().school("passed_across"), 0);
    let e = w.apply(Command::SetShare { pos: at(30, 15), to: SplitTo::Stores, weight: 1 }, &defs);
    assert!(e.is_err(), "no building there");
    let far = SplitTo::Machine(at(30, 15));
    let e = w.apply(Command::SetShare { pos: at(5, 2), to: far, weight: 1 }, &defs).unwrap_err();
    assert_eq!(e.to_string(), "doesn't touch a machine that takes it");
}

#[test]
fn a_split_deals_the_output_by_its_shares() {
    let defs = Defs::bundled();
    let mut w = smelter_row(&defs);
    let smelter = at(5, 2);
    assert_eq!(
        w.split_destinations(smelter, &defs),
        vec![SplitTo::Machine(at(7, 2)), SplitTo::Stores]
    );
    for (to, weight) in [(SplitTo::Machine(at(7, 2)), 2), (SplitTo::Stores, 1)] {
        w.apply(Command::SetShare { pos: smelter, to, weight }, &defs).unwrap();
    }
    for _ in 0..30_000 {
        w.tick(&defs);
        if w.tallies().school("automated_batches") >= 9 {
            break;
        }
    }
    let across = w.tallies().school("passed_across");
    assert!(across >= 2, "some metal went across: {across}");
    let out = w.tallies().school("automated_batches") - w.tallies().school("steel_made");
    assert!(out > 0, "some batches were the smelter's");
    // A share of 0 takes a destination off; with none left the default is back.
    for to in [SplitTo::Machine(at(7, 2)), SplitTo::Stores] {
        w.apply(Command::SetShare { pos: smelter, to, weight: 0 }, &defs).unwrap();
    }
    assert!(w.split_of(smelter).is_empty());
}

#[test]
fn shares_of_two_to_one_send_two_across_for_each_one_out_the_door() {
    let defs = Defs::bundled();
    let mut w = smelter_row(&defs);
    let (smelter, works) = (at(5, 2), at(7, 2));
    let metal = defs.item_id("metal").unwrap();
    for (to, weight) in [(SplitTo::Machine(works), 2), (SplitTo::Stores, 1)] {
        w.apply(Command::SetShare { pos: smelter, to, weight }, &defs).unwrap();
    }
    // The Steelworks' batch takes 2 metal: two units go across, the third is the door's.
    let left = w.pass_across(smelter, Stack { item: metal, count: 3 }, &defs);
    assert_eq!(left.count, 1);
    assert_eq!(w.batch_room(works, metal, &defs), 0);
    // With its batch full, the machine's turns fall through to stores.
    let left = w.pass_across(smelter, Stack { item: metal, count: 3 }, &defs);
    assert_eq!(left.count, 3);
}

/// Ticks `w` until a machine has been tended or `ticks` pass.
fn run_smelter(w: &mut World, defs: &Defs, ticks: u32) {
    for _ in 0..ticks {
        w.tick(defs);
        if w.tallies().school("tended") > 0 {
            break;
        }
    }
}

#[test]
fn a_united_city_s_machine_stops_after_its_batches_until_a_tool_is_brought() {
    assert!(Defs::bundled().jobs.tend_every > 0, "the data wears machines");
    // Two batches a tending, so the settlers are still fed when the test is done.
    let mut defs = Defs::bundled();
    defs.jobs.tend_every = 2;
    let tool = defs.item_id("tool").unwrap();
    // No tool in stores: the Smelter stops for good after `every` batches.
    let mut w = smelter_row(&defs);
    w.complete_endeavor(&defs);
    run_smelter(&mut w, &defs, 12_000);
    assert!(w.needs_tending(at(5, 2), &defs), "worn out");
    let stuck = w.tallies().school("automated_batches");
    for _ in 0..3_000 {
        w.tick(&defs);
    }
    assert_eq!(w.tallies().school("automated_batches"), stuck, "nothing runs while worn");
    // A tool comes from stores, is used up, and the machine runs again.
    w.designate_stockpile(at(22, 10)).unwrap();
    w.stored.insert(at(22, 10), Stack { item: tool, count: 3 });
    run_smelter(&mut w, &defs, 6_000);
    assert!(!w.units.is_empty());
    assert_eq!(w.tallies().school("tended"), 1);
    assert!(!w.needs_tending(at(5, 2), &defs));
    let tools: u32 = w
        .ground
        .values()
        .chain(w.stored.values())
        .filter(|s| s.item == tool)
        .map(|s| s.count)
        .sum();
    assert_eq!(tools, 2, "one tool used up");
}

#[test]
fn before_the_gate_machines_never_wear() {
    let mut defs = Defs::bundled();
    defs.jobs.tend_every = 2;
    let mut w = smelter_row(&defs);
    for _ in 0..12_000 {
        w.tick(&defs);
    }
    assert!(w.tallies().school("automated_batches") > defs.jobs.tend_every);
    assert!(!w.needs_tending(at(5, 2), &defs));
}
