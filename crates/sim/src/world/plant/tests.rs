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

fn put(w: &mut World, defs: &Defs, id: &str, pos: Pos) {
    let id = defs.building_id(id).unwrap();
    let (fw, fh) = defs.building(id).footprint;
    w.put_building(Footprint::new(pos, fw, fh), id, defs);
}

#[test]
fn two_big_works_brown_out_a_lightning_station_and_one_taken_off_ends_it() {
    let defs = Defs::bundled();
    let mut w = World::with_map(1, field(40, 20), Nation::Fire, &[at(20, 12)], &defs);
    put(&mut w, &defs, "lightning_station", at(2, 2));
    put(&mut w, &defs, "steel_foundry", at(5, 2));
    put(&mut w, &defs, "steel_foundry", at(2, 5));
    w.research.grant(defs.tech_id("great_works").unwrap());
    let load = w.yard_loads(&defs)[&at(2, 2)];
    assert_eq!((load.draw, load.capacity), (8, Some(6)));
    assert_eq!(w.brownout(at(5, 2), &defs), 0.75, "6 of 8: every machine at three quarters");
    w.apply(crate::world::Command::SetAutomated { pos: at(2, 5), on: false }, &defs).unwrap();
    assert_eq!(w.yard_loads(&defs)[&at(2, 2)].draw, 4);
    assert_eq!(w.brownout(at(5, 2), &defs), 1.0);
}

#[test]
fn a_lightning_plant_takes_fire_notables_as_crew_and_each_adds_capacity() {
    let defs = Defs::bundled();
    let spawns = [at(20, 12), at(21, 12), at(22, 12)];
    let mut w = World::with_map(1, field(40, 20), Nation::Fire, &spawns, &defs);
    put(&mut w, &defs, "lightning_plant", at(2, 2));
    w.research.grant(defs.tech_id("lightning_plants").unwrap());
    let base = w.yard_loads(&defs)[&at(2, 2)];
    assert_eq!((base.capacity, base.crew), (Some(4), (0, 4)));
    let ids: Vec<UnitId> = w.units.iter().filter(|u| u.is_settler()).map(|u| u.id).collect();
    assert!(ids.len() >= 3, "a start has a few settlers");
    for u in w.units.iter_mut().filter(|u| ids[..3].contains(&u.id)) {
        u.standing.tier = Tier::Notable;
    }
    // A Water Notable can't crew Fire's plant.
    w.units.iter_mut().find(|u| u.id == ids[2]).unwrap().nation = Some(Nation::Water);
    assert_eq!(w.crews(&defs)[&at(2, 2)], ids[..2].to_vec());
    let crewed = w.yard_loads(&defs)[&at(2, 2)];
    assert_eq!((crewed.capacity, crewed.crew), (Some(10), (2, 4)));
}

#[test]
fn each_nation_has_a_plant_and_a_big_work_after_mechanisms() {
    let defs = Defs::bundled();
    for (tech, nation, plant, work) in [
        ("lightning_plants", Nation::Fire, "lightning_plant", "steel_foundry"),
        ("geothermal_vents", Nation::Earth, "geothermal_vent", "furniture_works"),
        ("hydro_mills", Nation::Water, "hydro_mill", "parka_works"),
        ("wind_farms", Nation::Air, "wind_farm", "herb_house"),
    ] {
        let t = defs.tech(defs.tech_id(tech).unwrap());
        assert_eq!(t.nation, Some(nation), "{tech}");
        assert!(t.requires.united, "{tech}");
        let p = defs.building(defs.building_id(plant).unwrap());
        let (by, a) = p.automate_upgrade.as_ref().expect(plant);
        assert_eq!(defs.tech(*by).id, tech);
        assert!(a.capacity > 0, "{plant}");
        let b = defs.building(defs.building_id(work).unwrap());
        assert_eq!((b.footprint, b.draw, b.uses_power), ((3, 3), 4, true), "{work}");
        assert!(b.recipe.is_some(), "{work}");
    }
}
