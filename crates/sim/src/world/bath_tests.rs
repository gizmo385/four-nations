//! #741's two teachings: the Sun Stone (`[building.chi_regen]`) and the Bathhouse
//! (`[building.bath]`).

use super::*;
use crate::defs::{BiomeId, Terrain};
use crate::geom::{Footprint, Grid};
use crate::mapgen::Tile;

fn at(x: i32, y: i32) -> Pos {
    Pos::new(x, y)
}

fn field() -> TileMap {
    Grid::from_fn(40, 8, |_| Tile {
        elevation: 100,
        temperature: 128,
        moisture: 128,
        terrain: Terrain::Land,
        biome: BiomeId(0),
        resource: None,
        building: None,
    })
}

fn world(defs: &Defs, units: &[Pos]) -> World {
    let mut w = World::with_map(1, field(), Nation::Fire, units, defs);
    w.difficulty = Some("normal".into());
    w
}

fn put(w: &mut World, defs: &Defs, id: &str, anchor: Pos) {
    let b = defs.building_id(id).unwrap();
    let (fw, fh) = defs.building(b).footprint;
    w.put_building(Footprint::new(anchor, fw, fh), b, defs);
}

fn with_buildings(extra: &str) -> Result<Defs, crate::defs::DefsError> {
    let mut files = crate::defs::BUNDLED_FILES;
    let buildings = format!("{}\n{extra}\n", files[7]);
    files[7] = &buildings;
    Defs::from_files(files)
}

#[test]
fn the_stone_and_the_bath_are_data() {
    let defs = Defs::bundled();
    let stone = defs.building(defs.building_id("sun_stone").unwrap());
    let c = stone.chi_regen.as_ref().unwrap();
    assert_eq!((c.radius, c.factor), (8, 2.0));
    assert_eq!(c.seasons, ["spring", "summer"]);
    let bath = defs.building(defs.building_id("bathhouse").unwrap());
    let b = bath.bath.unwrap();
    assert_eq!((b.seek_energy_below, b.seek_tiles, b.minutes), (30.0, 30, 0.5));
    assert_eq!((b.warm_minutes, b.cooldown_minutes), (20.0, 20.0));
    assert_eq!(bath.beside, Some(vec![Terrain::Mountain, Terrain::Tunnel]));
    let springs = defs.tech(defs.tech_id("hot_springs").unwrap());
    assert!(springs.teaching.as_ref().unwrap().site.is_some());
    assert!(defs.tech(defs.tech_id("sun_stone").unwrap()).teaching.is_some());
}

#[test]
fn chi_regen_and_bath_numbers_are_validated() {
    let head =
        "[[building]]\nid = \"x\"\nname = \"X\"\nverb = \"Build\"\nwork = 1.0\npassable = false\n";
    for (body, why) in [
        (
            "[building.chi_regen]\nradius = 0\nfactor = 2.0\nseasons = [\"spring\"]",
            "radius must be 1 to 12",
        ),
        (
            "[building.chi_regen]\nradius = 4\nfactor = 1.0\nseasons = [\"spring\"]",
            "factor must be above 1",
        ),
        ("[building.chi_regen]\nradius = 4\nfactor = 2.0\nseasons = []", "at least one season"),
        (
            "[building.chi_regen]\nradius = 4\nfactor = 2.0\nseasons = [\"monsoon\"]",
            "unknown season",
        ),
        (
            "[building.bath]\nseek_energy_below = 30.0\nseek_tiles = 30\nminutes = 0.5\n\
             warm_minutes = 20.0\ncooldown_minutes = 20.0",
            "bath needs a capacity",
        ),
        (
            "capacity = 1\n[building.bath]\nseek_energy_below = 30.0\nseek_tiles = 30\n\
             minutes = 0.0\nwarm_minutes = 20.0\ncooldown_minutes = 20.0",
            "bath minutes must be positive",
        ),
    ] {
        let e = with_buildings(&format!("{head}{body}")).expect_err(body).0;
        assert!(e.contains(why), "{body}: {e}");
    }
}

/// The Sun Stone doubles chi regen in its reach in spring and summer, and two don't stack.
#[test]
fn a_sun_stone_doubles_chi_regen_in_reach_in_its_seasons() {
    let defs = Defs::bundled();
    let mut w = world(&defs, &[at(12, 3), at(30, 3)]);
    put(&mut w, &defs, "sun_stone", at(5, 3));
    assert_eq!(w.season(&defs).id, "spring");
    assert_eq!(w.chi_stone_at(at(13, 3), &defs), 2.0, "8 tiles off");
    assert_eq!(w.chi_stone_at(at(14, 3), &defs), 1.0, "9 tiles off");
    put(&mut w, &defs, "sun_stone", at(7, 3));
    assert_eq!(w.chi_stone_at(at(12, 3), &defs), 2.0, "two stones don't stack");
    // Chi comes back twice as fast for the settler in reach.
    for u in &mut w.units {
        u.chi = 0.0;
    }
    for _ in 0..30 {
        w.tick(&defs);
    }
    let (near, far) = (w.units[0].chi, w.units[1].chi);
    assert!(far > 0.0);
    assert!((near / far - 2.0).abs() < 0.01, "{near} vs {far}");
    // Autumn: nothing.
    let per = defs.seasons.season_ticks();
    w.tick = 2 * per;
    assert_eq!(w.season(&defs).id, "autumn");
    assert_eq!(w.chi_stone_at(at(12, 3), &defs), 1.0);
}

/// A tired settler bathes instead of sleeping: half a minute inside, then full energy and
/// warm for 20 minutes, and no second bath for 20.
#[test]
fn a_tired_settler_bathes_and_comes_out_rested_and_warm() {
    let defs = Defs::bundled();
    let mut w = world(&defs, &[at(2, 3)]);
    put(&mut w, &defs, "bathhouse", at(6, 2));
    w.units[0].needs.energy = 20.0;
    let mut saw_bath = false;
    for _ in 0..2000 {
        w.tick(&defs);
        if matches!(w.units[0].need_task, Some(Task::Bath(_))) {
            saw_bath = true;
        }
        if w.units[0].warm_until > 0 {
            break;
        }
    }
    assert!(saw_bath, "it went to the bath");
    let u = &w.units[0];
    assert!(u.needs.energy > defs.needs.max() - 1.0, "full energy: {}", u.needs.energy);
    assert_eq!(u.warm_until, w.tick + minutes_to_ticks_min1(20.0));
    assert_eq!(u.bath_ready_at, w.tick + minutes_to_ticks_min1(20.0));
    assert_eq!(u.need_task, None);
    assert_eq!(u.bath, None, "the claim is let go");
    // Tired again inside the cooldown: no bath, so it sleeps.
    w.units[0].needs.energy = 20.0;
    assert!(w.plan_bath(0, &defs).is_none());
}

#[test]
fn a_warm_settler_takes_no_exposure() {
    let defs = Defs::bundled();
    let mut w = world(&defs, &[at(2, 3)]);
    let mut u = w.units.remove(0);
    u.need_task = Some(Task::Sleep);
    u.route = None;
    assert!(w.apply_exposure(&mut u, true, &defs), "cold and sleeping rough");
    u.warm_until = w.tick + 10;
    let before = u.needs.health;
    assert!(!w.apply_exposure(&mut u, true, &defs), "warm from a bath");
    assert_eq!(u.needs.health, before);
}

#[test]
fn the_bathhouse_must_touch_rock() {
    let defs = Defs::bundled();
    let mut w = world(&defs, &[at(2, 3)]);
    w.research.grant(defs.tech_id("hot_springs").unwrap());
    let bath = defs.building_id("bathhouse").unwrap();
    assert_eq!(
        w.check_build(at(10, 3), bath, &defs),
        Err(BuildError::CannotBuild { at: at(10, 3), why: Blocker::Beside("rock") })
    );
}
