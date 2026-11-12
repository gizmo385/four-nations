//! Spirit water (#739): the spring drop, the stock limit, and a vial saving a settler.

use super::*;
use crate::defs::{BiomeId, Terrain};
use crate::geom::{Footprint, Grid};
use crate::mapgen::Tile;
use crate::nation::Nation;

fn field() -> TileMap {
    Grid::from_fn(12, 8, |_| Tile {
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

fn world(defs: &Defs) -> World {
    World::with_map(1, field(), Nation::Water, &[at(1, 1)], defs)
}

/// Puts a finished Healing Hut at `anchor`.
fn hut(w: &mut World, defs: &Defs, anchor: Pos) {
    let id = defs.building_id("healing_hut").unwrap();
    let (fw, fh) = defs.building(id).footprint;
    w.put_building(Footprint::new(anchor, fw, fh), id, defs);
}

fn vial(defs: &Defs) -> ItemId {
    defs.item_id("spirit_vial").unwrap()
}

fn knows_spirit_water(w: &mut World, defs: &Defs) {
    w.research.grant(defs.tech_id("spirit_water").unwrap());
}

#[test]
fn spirit_water_is_data() {
    let defs = Defs::bundled();
    let hut = defs.building(defs.building_id("healing_hut").unwrap());
    let h = hut.healing.as_ref().unwrap();
    assert_eq!((h.vials_per_year, h.vial_item, h.vial_stock), (0, Some(vial(&defs)), 3));
    assert_eq!(h.revive_to, 30.0);
    let tech = defs.tech_id("spirit_water").unwrap();
    assert_eq!(hut.vial_upgrade, Some((tech, 1)));
    assert!(defs.tech(tech).teaching.is_some());
    let hall = defs.building(defs.building_id("study_hall").unwrap());
    assert_eq!(hall.spirit_seats, 0);
    assert_eq!(hall.spirit_upgrade, Some((defs.tech_id("spirit_library").unwrap(), 1)));
}

#[test]
fn spirit_seats_and_vials_are_validated() {
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
        ("building = \"bed\"\nspirit_seats = 1", "no study seats"),
        ("building = \"map_room\"\nspirit_seats = 1", "no study seats"),
        ("building = \"study_hall\"\nspirit_seats = 3", "must be 1 to 4"),
        ("building = \"study_hall\"\nspirit_seats = 1", "already upgraded by \"spirit_library\""),
        ("building = \"bed\"\nvials_per_year = 1", "no vial_item"),
        ("building = \"healing_hut\"\nvials_per_year = 9", "must be 1 to 4"),
        ("building = \"healing_hut\"\nvials_per_year = 1", "already upgraded by \"spirit_water\""),
    ] {
        let e = with(up).expect_err(up).0;
        assert!(e.contains(why), "{up}: {e}");
    }
}

#[test]
fn a_hut_fills_a_vial_on_the_first_tick_of_spring_only_once_known() {
    let defs = Defs::bundled();
    let mut w = world(&defs);
    // Seasons run only under a difficulty preset.
    w.difficulty = Some("normal".into());
    hut(&mut w, &defs, at(6, 3));
    let year = defs.seasons.season_ticks() * defs.seasons.seasons.len() as u64;
    w.tick = year - 1;
    w.tick(&defs);
    assert_eq!(w.vials_held(vial(&defs)), 0, "not known: no vial");

    knows_spirit_water(&mut w, &defs);
    w.tick = 2 * year - 2;
    w.tick(&defs);
    assert_eq!(w.vials_held(vial(&defs)), 0, "the last tick of winter: not yet");
    w.tick(&defs);
    assert_eq!(w.tick, 2 * year);
    assert_eq!(w.vials_held(vial(&defs)), 1, "spring's first tick");
    w.tick(&defs);
    assert_eq!(w.vials_held(vial(&defs)), 1, "once a year");
}

#[test]
fn no_vial_drops_past_the_stock_limit_and_only_one_hut_fills() {
    let defs = Defs::bundled();
    let mut w = world(&defs);
    knows_spirit_water(&mut w, &defs);
    hut(&mut w, &defs, at(6, 3));
    hut(&mut w, &defs, at(9, 3));
    for want in [1, 2, 3, 3] {
        w.tick_vials(&defs);
        assert_eq!(w.vials_held(vial(&defs)), want);
    }
}

#[test]
fn a_stored_vial_saves_a_settler_who_would_have_starved() {
    let defs = Defs::bundled();
    let mut w = world(&defs);
    w.stored.insert(at(4, 6), Stack { item: vial(&defs), count: 2 });
    w.stored.insert(at(3, 6), Stack { item: vial(&defs), count: 1 });
    let id = w.units[0].id;
    w.units[0].needs.hunger = 0.0;
    w.units[0].needs.health = 0.0001;
    w.tick(&defs);
    let u = w.units.iter().find(|u| u.id == id).expect("saved");
    assert!(u.needs.health > 29.0 && u.needs.health <= 30.0, "{}", u.needs.health);
    assert!(u.needs.hunger > 29.0 && u.needs.hunger <= 30.0, "{}", u.needs.hunger);
    assert_eq!(w.stored_at(at(3, 6)), None, "the lowest Pos stack goes first");
    assert_eq!(w.stored_total(vial(&defs)), 2);
    assert_eq!(w.harm_events().filter(|e| e.kind == HarmEventKind::Revived).count(), 1);
    assert!(w.deaths().next().is_none());
}

#[test]
fn with_no_vial_the_settler_dies() {
    let defs = Defs::bundled();
    let mut w = world(&defs);
    // A vial on the ground, not yet stored, doesn't count.
    w.place_ground(at(8, 6), Stack { item: vial(&defs), count: 1 });
    w.units[0].needs.hunger = 0.0;
    w.units[0].needs.health = 0.0001;
    w.tick(&defs);
    assert!(w.units.is_empty());
    assert_eq!(w.harm_events().filter(|e| e.kind == HarmEventKind::Revived).count(), 0);
}
