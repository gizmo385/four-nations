//! `southern_ways` (#727): shore fishing, free Waystations and their shelter from the cold.

use super::*;
use crate::defs::{BiomeId, Terrain};
use crate::geom::{Footprint, Grid};
use crate::jobs::JobKind;
use crate::mapgen::Tile;
use crate::needs::Needs;
use crate::world::Command;

/// Land, with a sea column at `x = 0..sea` and the given mapgen temperature.
fn coast(width: i32, height: i32, sea: i32, temperature: u8) -> TileMap {
    Grid::from_fn(width, height, |p| Tile {
        elevation: 100,
        temperature,
        moisture: 128,
        terrain: if p.x < sea { Terrain::Water } else { Terrain::Land },
        biome: BiomeId(0),
        resource: None,
        building: None,
    })
}

fn at(x: i32, y: i32) -> Pos {
    Pos::new(x, y)
}

fn grant(w: &mut World, defs: &Defs, tech: &str) {
    w.research.grant(defs.tech_id(tech).unwrap());
}

#[test]
fn free_and_shelter_upgrades_are_validated() {
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
        ("building = \"bed\"\nfree = false", "free can only be true"),
        ("building = \"bed\"\nshelter = 13", "must be 1 to 12"),
        ("building = \"bed\"\nshelter = 0", "must be 1 to 12"),
        ("building = \"bed\"\nfree = true\nshelter = 3", "exactly one property"),
        ("building = \"waystation\"\nfree = true", "already upgraded by \"southern_ways\""),
        ("building = \"waystation\"\nshelter = 2", "already upgraded by \"southern_ways\""),
    ] {
        let e = with(up).expect_err(up).0;
        assert!(e.contains(why), "{up}: {e}");
    }
    let defs = Defs::bundled();
    let tech = defs.tech_id("southern_ways").unwrap();
    let way = defs.building(defs.building_id("waystation").unwrap());
    assert_eq!(way.free_upgrade, Some(tech));
    assert_eq!(way.shelter_upgrade, Some((tech, 6)));
    let shore = defs.shore_fishing.as_ref().expect("[shore_fishing]");
    assert_eq!((defs.item(shore.harvest.item).id.as_str(), shore.harvest.count), ("fish", 1));
    assert_eq!(defs.school(defs.techs[tech.0 as usize].school.unwrap()).id, "water_pole");
}

#[test]
fn shore_fishing_needs_the_tech_and_a_walkable_neighbour() {
    let defs = Defs::bundled();
    let mut w = World::with_map(1, coast(20, 8, 4, 128), Nation::Water, &[at(10, 4)], &defs);
    let edge = at(3, 4);
    let deep = at(1, 4);
    assert!(w.harvest_at(edge, &defs).is_none());
    assert!(w.apply(Command::DesignateHarvest { pos: edge }, &defs).is_err(), "not yet known");
    grant(&mut w, &defs, "southern_ways");
    assert_eq!(w.harvest_at(edge, &defs).map(|h| h.verb.as_str()), Some("Fish"));
    assert!(w.harvest_at(deep, &defs).is_none(), "no walkable neighbour");
    assert!(w.apply(Command::DesignateHarvest { pos: deep }, &defs).is_err());
    assert!(w.harvest_at(at(6, 4), &defs).is_none(), "land is not shore water");
    w.apply(Command::DesignateHarvest { pos: edge }, &defs).unwrap();
}

#[test]
fn a_standing_shore_order_keeps_fishing_and_never_runs_out() {
    let defs = Defs::bundled();
    let mut w = World::with_map(1, coast(20, 8, 4, 128), Nation::Water, &[at(6, 4)], &defs);
    grant(&mut w, &defs, "southern_ways");
    let edge = at(3, 4);
    w.apply(Command::DesignateRepeatingHarvest { pos: edge }, &defs).unwrap();
    let fish = defs.item_id("fish").unwrap();
    let caught = |w: &World| -> u32 {
        w.ground.values().chain(w.stored.values()).filter(|s| s.item == fish).map(|s| s.count).sum()
    };
    for _ in 0..20_000 {
        w.tick(&defs);
        if caught(&w) >= 3 {
            break;
        }
    }
    assert!(caught(&w) >= 3, "three catches from one tile");
    assert_eq!(w.map[edge].terrain, Terrain::Water);
    assert!(w.is_repeating_harvest(edge));
    assert!(w.jobs.iter().any(|j| j.kind == JobKind::Harvest { pos: edge }) || caught(&w) >= 3);
}

#[test]
fn waystations_placed_after_the_tech_have_no_bill_and_give_none_back() {
    let defs = Defs::bundled();
    let way = defs.building_id("waystation").unwrap();
    let mut w = World::with_map(1, coast(60, 12, 0, 128), Nation::Water, &[at(2, 2)], &defs);
    grant(&mut w, &defs, "waystations");
    let before = at(10, 5);
    w.apply(Command::DesignateBuild { pos: before, building: way }, &defs).unwrap();
    assert!(w.next_shortfall(before, &defs).is_some(), "placed before: billed");
    grant(&mut w, &defs, "southern_ways");
    assert!(w.next_shortfall(before, &defs).is_some(), "a waiting site keeps its bill");
    let after = at(30, 5);
    w.apply(Command::DesignateBuild { pos: after, building: way }, &defs).unwrap();
    assert!(w.site_is_free(after) && !w.site_is_free(before));
    assert_eq!(w.next_shortfall(after, &defs), None, "placed after: only work");

    // Cancelled, it drops nothing; finished and taken down, nothing either.
    w.apply(Command::CancelBuild { pos: after }, &defs).unwrap();
    assert!(w.ground.is_empty());
    w.apply(Command::DesignateBuild { pos: after, building: way }, &defs).unwrap();
    w.jobs.retain(|j| j.kind != JobKind::Build { pos: before });
    w.apply(Command::CancelBuild { pos: before }, &defs).unwrap();
    w.ground.clear();
    for _ in 0..20_000 {
        w.tick(&defs);
        if w.building_at(after).is_some() {
            break;
        }
    }
    assert_eq!(w.building_at(after), Some(way), "built with work alone");
    w.apply(Command::Deconstruct { pos: after }, &defs).unwrap();
    for _ in 0..20_000 {
        w.tick(&defs);
        if w.building_at(after).is_none() {
            break;
        }
    }
    assert_eq!(w.building_at(after), None);
    assert!(w.ground.is_empty(), "a free building gives back no cost: {:?}", w.ground);
}

#[test]
fn a_waystation_keeps_the_cold_off_within_six_tiles_once_known() {
    let defs = Defs::bundled();
    let pos = at(9, 2);
    let mut w = World::with_map(1, coast(30, 5, 0, 0), Nation::Water, &[pos], &defs);
    w.difficulty = Some("normal".into());
    w.tick =
        3 * (defs.seasons.calendar.season_minutes as f64 * 60.0 * TICKS_PER_SECOND as f64) as u64; // winter's first tick
    w.units[0].needs = Needs::full(&defs.needs);
    let way = defs.building_id("waystation").unwrap();
    w.put_building(Footprint::new(at(1, 1), 2, 2), way, &defs);
    assert!(w.is_cold(pos, &defs), "the camp alone doesn't heat");
    grant(&mut w, &defs, "southern_ways");
    // The footprint's edge is x = 2: 6 tiles out is x = 8, 7 is x = 9.
    assert!(!w.is_cold(at(8, 2), &defs) && w.camp_shelters(at(8, 2), &defs));
    assert!(w.is_cold(pos, &defs), "7 tiles out is past it");
}

/// #731: a hungry settler with no stores forages fish straight from the shore, which has no
/// resource on it (eating read the tile's resource and panicked).
#[test]
fn a_hungry_settler_can_eat_from_the_shore() {
    let defs = Defs::bundled();
    let mut w = World::with_map(1, coast(20, 8, 4, 128), Nation::Water, &[at(5, 4)], &defs);
    grant(&mut w, &defs, "southern_ways");
    w.units[0].needs = Needs { hunger: 10.0, ..Needs::full(&defs.needs) };
    let start = w.units[0].needs.hunger;
    for _ in 0..TICKS_PER_SECOND * 120 {
        w.tick(&defs);
    }
    assert!(w.units[0].needs.hunger > start, "ate: {}", w.units[0].needs.hunger);
}
