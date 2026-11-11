//! The Stable and `herding` (#696): a tech for Earth, Fire and Water, never Air, and one building
//! that houses each of the three nations' own beast.

use super::*;
use crate::beast::BeastWork;
use crate::defs::{BiomeId, Terrain};
use crate::geom::Grid;
use crate::mapgen::Tile;
use crate::research::ResearchError;
use crate::world::build::BuildError;
use crate::world::command::{Command, CommandError};

const STABLE: Pos = Pos::new(4, 6);
const DOOR: Pos = Pos::new(4, 8);

/// Earth, Fire and Water, with the beast each houses, the speed its row says and its carry.
const HERDERS: [(Nation, &str, &str, f32, u32); 3] = [
    (Nation::Earth, "ostrich_horse", "Ostrich Horse", 1.4, 10),
    (Nation::Fire, "komodo_rhino", "Komodo Rhino", 0.8, 12),
    (Nation::Water, "buffalo_yak", "Buffalo Yak", 1.0, 10),
];

fn field() -> TileMap {
    Grid::from_fn(30, 14, |_| Tile {
        elevation: 100,
        temperature: 128,
        moisture: 128,
        terrain: Terrain::Land,
        biome: BiomeId(0),
        resource: None,
        building: None,
    })
}

fn world(nation: Nation, defs: &Defs) -> World {
    World::with_map(1, field(), nation, &[Pos::new(0, 0)], defs)
}

#[test]
fn herding_is_for_earth_fire_and_water() {
    let defs = Defs::bundled();
    let herding = defs.tech_id("herding").unwrap();
    let def = defs.tech(herding);
    assert_eq!((def.cost, def.core), (250, false));
    assert_eq!(def.nation, None);
    assert_eq!(def.nations, [Nation::Earth, Nation::Fire, Nation::Water]);
    assert!(!def.is_shared());
    assert_eq!(def.prereqs, [defs.tech_id("farming").unwrap()]);
    for nation in Nation::ALL {
        let sees = defs.techs_for(nation).any(|(id, _)| id == herding);
        assert_eq!(sees, nation != Nation::Air, "{nation:?}");
        assert_eq!(def.open_to(nation), nation != Nation::Air, "{nation:?}");
    }
}

#[test]
fn air_cannot_research_herding_or_raise_a_stable() {
    let defs = Defs::bundled();
    let herding = defs.tech_id("herding").unwrap();
    let stable = defs.building_id("stable").unwrap();
    assert_eq!(defs.building(stable).unlocked_by, Some(herding));
    let mut w = world(Nation::Air, &defs);
    let refused = w.apply(Command::SetResearch { tech: herding }, &defs);
    assert_eq!(refused, Err(CommandError::Research(ResearchError::WrongNation)));
    assert_eq!(ResearchError::WrongNation.to_string(), "that tech belongs to another nation");
    assert!(w.research().queue().is_empty());
    assert_eq!(
        w.designate_build(Pos::new(2, 2), stable, &defs),
        Err(BuildError::NotResearched(herding))
    );
    // Even finished, a stable houses nothing of Air's: no row names it.
    w.raise(Footprint::new(STABLE, 2, 2), stable, &defs);
    w.tick(&defs);
    assert_eq!(w.beasts().count(), 0);
}

#[test]
fn each_herding_nation_researches_it_and_builds_a_stable() {
    let defs = Defs::bundled();
    let herding = defs.tech_id("herding").unwrap();
    let farming = defs.tech_id("farming").unwrap();
    let stable = defs.building_id("stable").unwrap();
    for (nation, ..) in HERDERS {
        let mut w = world(nation, &defs);
        assert_eq!(
            w.designate_build(Pos::new(2, 2), stable, &defs),
            Err(BuildError::NotResearched(herding)),
            "{nation:?}"
        );
        w.apply(Command::SetResearch { tech: herding }, &defs).unwrap();
        // farming comes first, in id order, and herding is the last of the queue.
        assert_eq!(w.research().queue().last(), Some(&herding), "{nation:?}");
        assert!(w.research().queue().contains(&farming), "{nation:?}");
        w.research.grant(farming);
        w.research.grant(herding);
        assert!(w.designate_build(Pos::new(2, 2), stable, &defs).is_ok(), "{nation:?}");
    }
}

#[test]
fn each_stable_houses_its_nation_s_beast() {
    let defs = Defs::bundled();
    let stable = defs.building_id("stable").unwrap();
    for (nation, id, name, speed, carry) in HERDERS {
        let mut w = world(nation, &defs);
        w.raise(Footprint::new(STABLE, 2, 2), stable, &defs);
        w.tick(&defs);
        assert_eq!(w.beasts().count(), 1, "{nation:?}");
        let b = w.beasts().next().unwrap().clone();
        assert_eq!((b.name.as_str(), b.pos), (name, DOOR), "{nation:?}");
        assert_eq!(b.beast.map(|x| (x.id, x.home)), Some((defs.beast_id(id).unwrap(), STABLE)));
        assert_eq!(w.unit_speed(&b, &defs), speed, "{nation:?}");
        let row = defs.beast(defs.beast_id(id).unwrap());
        assert_eq!((row.nation, row.home), (nation, stable), "{nation:?}");
        let BeastWork::Haul { carry: c, min_trip } = row.work else { panic!("{id} hauls") };
        assert_eq!((c, min_trip), (carry, 10), "{nation:?}");
        // Taking the stable down sends it back to the wild.
        let key = w.building_record(STABLE).unwrap();
        let key = w.map[key.anchor].building.unwrap().key();
        w.remove_building(key, &defs).expect("it stood");
        w.tick(&defs);
        assert_eq!(w.beasts().count(), 0, "{nation:?}");
        let events: Vec<String> = w.beast_events().map(BeastEvent::describe).collect();
        assert_eq!(events, [format!("The {} returns to the wild.", name.to_lowercase())]);
    }
}

#[test]
fn a_stable_eats_015_food_a_minute() {
    let defs = Defs::bundled();
    let stable = defs.building_id("stable").unwrap();
    let fuel = defs.building(stable).fuel.as_ref().unwrap();
    assert_eq!((fuel.per_minute, fuel.bunker), (Some(0.15), 8));
    let names: Vec<&str> = fuel.items.iter().map(|&i| defs.item(i).id.as_str()).collect();
    assert_eq!(names, ["cabbage", "fruit", "fish"]);
    for (nation, ..) in HERDERS {
        let mut w = world(nation, &defs);
        w.raise(Footprint::new(STABLE, 2, 2), stable, &defs);
        w.tick(&defs);
        let b = w.anchored_mut(STABLE).and_then(|b| b.fuel.as_mut()).unwrap();
        b.count = 8;
        b.item = Some(defs.item_id("cabbage").unwrap());
        // Forty minutes burn six items: 40 x 0.15.
        for _ in 0..(40 * 60 * TICKS_PER_SECOND) {
            w.tick_fuel(&defs);
        }
        assert_eq!(w.anchored(STABLE).unwrap().fuel.unwrap().count, 2, "{nation:?}");
    }
}
