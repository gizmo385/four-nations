use std::collections::HashMap;

use super::*;
use crate::building::{BuildingDef, BuildingId, PowerDef};
use crate::defs::{BiomeId, Terrain};
use crate::geom::{Footprint, Grid};
use crate::mapgen::Tile;
use crate::world::Command;

fn arena(size: i32) -> TileMap {
    Grid::from_fn(size, size, |_| Tile {
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

/// A 1x1, passable building whose only trait is `[building.power]`, since which real buildings
/// are sources is out of scope here (#255, #204, #200): these tests define their own.
fn test_source(defs: &mut Defs, radius: u32, boost: f32) -> BuildingId {
    let id = BuildingId(defs.buildings.len() as u16);
    defs.buildings.push(BuildingDef {
        id: "test_power_source".into(),
        name: "Test Power Source".into(),
        verb: "Build".into(),
        cost: Vec::new(),
        work: 1.0,
        passable: true,
        footprint: (1, 1),
        seats: 0,
        spirit_seats: 0,
        capacity: 0,
        bed: None,
        heat: None,
        track: None,
        orchard: None,
        recipe: None,
        alt_recipes: Vec::new(),
        farm: None,
        irrigation: None,
        aqueduct: None,
        airship: None,
        healing: None,
        chi_regen: None,
        bath: None,
        power: Some(PowerDef { radius, boost, boost_millis: (boost * 1000.0).round() as u32 }),
        chart: None,
        archive: None,
        uses_power: false,
        names: HashMap::new(),
        fuel: None,
        outpost: None,
        houses_beast: false,
        pickers: None,
        hothouse: None,
        tea_house: None,
        pai_sho: None,
        chute: None,
        pair: None,
        glide: None,
        balloon: None,
        unlocked_by: None,
        seat_upgrades: Vec::new(),
        free_upgrade: None,
        shelter_upgrade: None,
        spirit_upgrade: None,
        vial_upgrade: None,
        endeavor: false,
        beside: None,
        on_biomes: Vec::new(),
        also_on: Vec::new(),
        boost_upgrade: None,
        heat_aura_upgrade: None,
        fuel_free_upgrade: None,
        automate_upgrade: None,
        bank_upgrade: None,
        aura: None,
        landmark: false,
    });
    id
}

/// Puts a finished source of `id` on `pos`, as if built.
fn raise_source(w: &mut World, defs: &Defs, id: BuildingId, pos: Pos) {
    w.put_building(Footprint::single(pos), id, defs);
}

#[test]
fn a_tile_within_reach_is_powered_at_the_sources_boost() {
    let mut defs = Defs::bundled();
    let source = test_source(&mut defs, 3, 1.5);
    let mut w = World::with_map(1, arena(12), Nation::Water, &[at(0, 0)], &defs);
    raise_source(&mut w, &defs, source, at(5, 5));

    assert_eq!(w.power_at(at(5, 5), &defs), Some(PowerAt { source: at(5, 5), boost: 1.5 }));
    assert_eq!(w.power_at(at(8, 5), &defs).map(|p| p.boost), Some(1.5), "3 tiles away: in reach");
    assert!(w.power_at(at(9, 5), &defs).is_none(), "4 tiles away: out of reach");
}

#[test]
fn a_footprint_is_powered_when_any_tile_of_it_is_in_reach() {
    let mut defs = Defs::bundled();
    let source = test_source(&mut defs, 3, 1.5);
    let mut w = World::with_map(1, arena(12), Nation::Water, &[at(0, 0)], &defs);
    raise_source(&mut w, &defs, source, at(5, 5));

    // A 2x2 whose anchor (1, 5) is 4 tiles away, but whose far column (2, 5) is 3 away.
    let two_by_two = Footprint::new(at(1, 5), 2, 2);
    assert!(w.power_at(at(1, 5), &defs).is_none(), "the anchor alone is out of reach");
    assert_eq!(w.power_for(two_by_two, &defs), Some(PowerAt { source: at(5, 5), boost: 1.5 }));
    assert!(w.power_for(Footprint::new(at(0, 5), 2, 2), &defs).is_none());
}

#[test]
fn two_sources_in_reach_give_the_best_boost_not_the_product() {
    let mut defs = Defs::bundled();
    let weak = test_source(&mut defs, 3, 1.5);
    let strong = test_source(&mut defs, 3, 2.0);
    let mut w = World::with_map(1, arena(12), Nation::Water, &[at(0, 0)], &defs);
    raise_source(&mut w, &defs, weak, at(2, 2));
    raise_source(&mut w, &defs, strong, at(6, 6));

    // (4, 4) sits 2 tiles from each, inside both reaches.
    let p = w.power_at(at(4, 4), &defs).expect("in reach of both");
    assert_eq!(p.boost, 2.0, "the best of 1.5 and 2.0, not their product 3.0");
    assert_eq!(p.source, at(6, 6));
}

#[test]
fn researching_a_power_generation_tech_grows_the_reach() {
    let mut defs = Defs::bundled();
    let source = test_source(&mut defs, 4, 1.5);
    let mut w = World::with_map(1, arena(20), Nation::Water, &[at(0, 0)], &defs);
    raise_source(&mut w, &defs, source, at(10, 10));

    // Water's base `power_generation` is 1.0 (data/nations.toml): the reach is the radius as-is.
    assert_eq!(w.modifiers().power_generation, 1.0);
    let edge = at(15, 10); // 5 tiles from the source: just outside a reach of 4
    assert!(w.power_at(edge, &defs).is_none());

    // `tide_mills` (prereqs: `farming`, `irrigation`) raises Water's `power_generation` to 1.25
    // (data/research.toml): round(4 * 1.25) = 5, so the same tile is now in reach.
    w.grant_tech(defs.tech_id("farming").unwrap());
    w.grant_tech(defs.tech_id("irrigation").unwrap());
    let tide_mills = defs.tech_id("tide_mills").unwrap();
    w.apply(Command::SetResearch { tech: tide_mills }, &defs).unwrap();
    w.research.bank_for_test(300_000);
    w.tick(&defs);
    assert!(w.research().is_known(tide_mills));
    assert_eq!(w.modifiers().power_generation, 1.25);

    assert_eq!(w.power_at(edge, &defs).map(|p| p.boost), Some(1.5), "now in reach");
}

/// The Wind Turbine (issue #204), on real data: once Air knows `windmills` and `wind_turbines`,
/// `power_generation` is 2.0 (base) * 1.2 * 1.25 = 3.0, so the turbine's radius of 3 reaches 9
/// tiles (docs/design/late-tree.md, "The sources"). A Workshop within that reach runs at the
/// turbine's 1.5x `boost`; one just outside doesn't.
#[test]
fn a_wind_turbine_powers_a_workshop_within_9_tiles_but_not_at_10() {
    let defs = Defs::bundled();
    let mut w = World::with_map(1, arena(24), Nation::Air, &[at(0, 0)], &defs);
    assert_eq!(w.modifiers().power_generation, 2.0);

    w.grant_tech(defs.tech_id("gliders").unwrap());
    w.grant_tech(defs.tech_id("farming").unwrap());
    let wind_turbines = defs.tech_id("wind_turbines").unwrap();
    w.apply(Command::SetResearch { tech: wind_turbines }, &defs).unwrap();
    // The queue is `windmills` then `wind_turbines`: `add_toward_target`'s cap tracks only the
    // tech at the front, so each needs its own bank-and-tick, the way natural passive income
    // would pay for one before moving to the next.
    for _ in 0..4 {
        if w.research().is_known(wind_turbines) {
            break;
        }
        w.research.bank_for_test(300_000);
        w.tick(&defs);
    }
    assert!(w.research().is_known(wind_turbines));
    assert_eq!(w.modifiers().power_generation, 3.0);

    let turbine = defs.building_id("wind_turbine").unwrap();
    let workshop_footprint = crate::geom::Footprint::new(at(0, 0), 2, 2);

    // Nearest workshop tile is (1, 0): a turbine 10 tiles east sits 9 away (within the reach of
    // 9), one 11 tiles east sits 10 away (just outside it).
    raise_source(&mut w, &defs, turbine, at(10, 0));
    assert_eq!(w.power_boost(workshop_footprint, &defs), 1.5, "9 tiles away: in reach");

    let near = w.map[at(10, 0)].building.unwrap().key();
    w.remove_building(near, &defs).unwrap();
    raise_source(&mut w, &defs, turbine, at(11, 0));
    assert_eq!(w.power_boost(workshop_footprint, &defs), 1.0, "10 tiles away: out of reach");
}

#[test]
fn a_powered_layout_survives_a_save_and_load_round_trip() {
    let mut defs = Defs::bundled();
    let source = test_source(&mut defs, 3, 1.5);
    let mut w = World::with_map(1, arena(12), Nation::Water, &[at(0, 0)], &defs);
    raise_source(&mut w, &defs, source, at(5, 5));

    let text = w.save(&defs).unwrap();
    let loaded = World::load(&text, &defs).unwrap();
    assert_eq!(
        loaded.power_at(at(5, 5), &defs),
        Some(PowerAt { source: at(5, 5), boost: 1.5 }),
        "a power source reaches the same tiles once loaded"
    );
    assert_eq!(w.save(&defs).unwrap(), loaded.save(&defs).unwrap());
}

#[test]
fn a_2x2_power_building_is_one_source() {
    let defs = Defs::bundled();
    let id = defs.building_id("steam_engine").unwrap();
    let mut w = World::with_map(1, arena(12), Nation::Water, &[at(0, 0)], &defs);
    w.put_building(Footprint::new(at(5, 5), 2, 2), id, &defs);
    let sources = |w: &World| w.of_kind(Kind::Power).map(|b| b.anchor).collect::<Vec<_>>();
    assert_eq!(sources(&w), vec![at(5, 5)]);
    let loaded = World::load(&w.save(&defs).unwrap(), &defs).unwrap();
    assert_eq!(sources(&loaded), vec![at(5, 5)]);
}
