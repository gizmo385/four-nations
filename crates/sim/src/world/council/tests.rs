use super::*;
use crate::defs::{BiomeId, Terrain};
use crate::geom::{Footprint, Grid};
use crate::mapgen::Tile;
use crate::standing::Tier;

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

fn fire(defs: &Defs) -> World {
    let spawns: Vec<Pos> = (0..8).map(|i| Pos::new(20 + i, 12)).collect();
    World::with_map(1, field(40, 20), Nation::Fire, &spawns, defs)
}

#[test]
fn the_council_waits_for_two_notables_of_every_nation() {
    let defs = Defs::bundled();
    let mut w = fire(&defs);
    let tech = defs.tech(defs.tech_id("republic_council").unwrap());
    let notables = |w: &World| {
        tech.requires
            .unmet(&w.facts(), &defs)
            .into_iter()
            .filter(|c| matches!(c, research::Condition::Notables { .. }))
            .count()
    };
    assert_eq!(notables(&w), 4, "no Notables yet");
    let nations = [Nation::Air, Nation::Water, Nation::Earth, Nation::Fire];
    for (i, u) in w.units.iter_mut().enumerate() {
        u.nation = Some(nations[i / 2]);
        u.standing.tier = Tier::Notable;
    }
    w.refresh_roster(&defs);
    assert_eq!(notables(&w), 0, "two of each");
    w.units[0].standing.tier = Tier::Citizen;
    w.refresh_roster(&defs);
    let unmet = tech.requires.unmet(&w.facts(), &defs);
    assert!(unmet.iter().any(|c| c.to_string() == "2 Air Notables (1 lives here)"), "{unmet:?}");
}

#[test]
fn once_the_hall_stands_each_seat_lends_its_nations_strongest_modifier() {
    let defs = Defs::bundled();
    let mut w = fire(&defs);
    let before = *w.modifiers();
    assert!(before.earthworks < 2.5 && before.water_work < 2.0);
    assert!(w.council_seats(&defs).is_empty());
    let hall = defs.building_id("council_hall").unwrap();
    w.put_building(Footprint::new(Pos::new(2, 2), 4, 4), hall, &defs);
    w.tick(&defs);
    let m = *w.modifiers();
    assert_eq!((m.earthworks, m.water_work, m.power_generation), (2.5, 2.0, 2.0));
    assert_eq!(m.heating, before.heating, "Fire's own strength stays");
    let seats: Vec<&str> = w.council_seats(&defs).iter().map(|s| s.1).collect();
    assert_eq!(seats, ["power_generation", "water_work", "earthworks", "heating"]);
    // A resident of another nation gets the lent strengths too.
    let mut water = w.units[0].clone();
    water.nation = Some(Nation::Water);
    assert_eq!(w.modifiers_for(&water, &defs).heating, 2.5);
}
