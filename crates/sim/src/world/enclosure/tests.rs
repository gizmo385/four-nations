use super::*;
use crate::building::BuildingKey;
use crate::defs::{BiomeId, Terrain};
use crate::geom::{Footprint, Grid};
use crate::mapgen::Tile;

fn field(width: i32, height: i32, temperature: u8) -> TileMap {
    Grid::from_fn(width, height, |_| Tile {
        elevation: 100,
        temperature,
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

/// An Earth world on a polar field with `agrarian_ring` known.
fn earth(defs: &Defs) -> World {
    let mut w = World::with_map(1, field(24, 20, 0), Nation::Earth, &[at(20, 18)], defs);
    w.research.grant(defs.tech_id("agrarian_ring").unwrap());
    w
}

/// Walls on the border of the square from `(lo, lo)` to `(hi, hi)`, except `skip`, keyed by
/// tile.
fn ring(w: &mut World, defs: &Defs, lo: i32, hi: i32, skip: &[Pos]) -> Vec<(Pos, BuildingKey)> {
    let wall = defs.building_id("earth_wall").unwrap();
    let mut keys = Vec::new();
    for y in lo..=hi {
        for x in lo..=hi {
            let p = at(x, y);
            let border = x == lo || x == hi || y == lo || y == hi;
            if border && !skip.contains(&p) {
                keys.push((p, w.put_building(Footprint::single(p), wall, defs)));
            }
        }
    }
    w.refresh_enclosure(defs);
    keys
}

#[test]
fn the_enclosure_table_is_validated() {
    let with = |enclosure: &str| {
        let mut files = crate::defs::BUNDLED_FILES;
        let start = files[11].find("[enclosure]").expect("the bundled [enclosure]");
        let seasons = format!("{}[enclosure]\n{enclosure}\n", &files[11][..start]);
        files[11] = &seasons;
        Defs::from_files(files).map(|_| ())
    };
    for (body, why) in [
        ("walls = []\nwinter_growth = 0.75\nfrost = false\ncold = false", "at least one"),
        ("walls = [\"nope\"]\nwinter_growth = 0.75\nfrost = false\ncold = false", "unknown"),
        ("walls = [\"waystation\"]\nwinter_growth = 0.75\nfrost = false\ncold = false", "1x1"),
        ("walls = [\"earth_wall\"]\nwinter_growth = 5.0\nfrost = false\ncold = false", "[0, 4]"),
    ] {
        let e = with(body).expect_err(body).0;
        assert!(e.contains(why), "{body}: {e}");
    }
    let defs = Defs::bundled();
    let e = defs.seasons.enclosure.as_ref().expect("[enclosure]");
    let names: Vec<&str> = e.walls.iter().map(|&b| defs.building(b).id.as_str()).collect();
    assert_eq!(names, ["earth_wall", "ring_gate"]);
    assert!(!e.frost && !e.cold && e.winter_growth == 0.75);
}

#[test]
fn a_closed_ring_shelters_its_inside_once_known() {
    let defs = Defs::bundled();
    let mut w = earth(&defs);
    let walls = ring(&mut w, &defs, 5, 10, &[]);
    assert!(w.is_sheltered(at(7, 7), &defs));
    assert_eq!(w.ring_size(at(6, 6), &defs), Some(16), "the 4x4 inside");
    assert!(!w.is_sheltered(at(5, 7), &defs), "a wall is a bound, not inside");
    assert!(!w.is_sheltered(at(3, 3), &defs));
    // A Ring Gate closes it as well as a wall does.
    let (p, key) = walls[3];
    w.remove_building(key, &defs);
    let gate = defs.building_id("ring_gate").unwrap();
    w.put_building(Footprint::single(p), gate, &defs);
    assert!(w.is_sheltered(at(7, 7), &defs), "fresh without a tick");

    let mut unknown = World::with_map(1, field(24, 20, 0), Nation::Earth, &[at(20, 18)], &defs);
    ring(&mut unknown, &defs, 5, 10, &[]);
    assert!(!unknown.is_sheltered(at(7, 7), &defs), "not before agrarian_ring");
}

#[test]
fn a_diagonal_gap_leaks() {
    let defs = Defs::bundled();
    let mut w = earth(&defs);
    ring(&mut w, &defs, 5, 10, &[at(5, 5)]);
    assert!(!w.is_sheltered(at(7, 7), &defs));
}

#[test]
fn removing_one_wall_clears_the_ring() {
    let defs = Defs::bundled();
    let mut w = earth(&defs);
    let walls = ring(&mut w, &defs, 5, 10, &[]);
    assert!(w.is_sheltered(at(7, 7), &defs));
    let (_, key) = walls.iter().find(|(p, _)| *p == at(5, 7)).copied().unwrap();
    w.remove_building(key, &defs);
    assert!(!w.is_sheltered(at(7, 7), &defs));
    w.tick(&defs);
    assert!(!w.is_sheltered(at(7, 7), &defs));
}

#[test]
fn a_ring_against_the_map_edge_or_closed_by_rock_does_not_shelter() {
    let defs = Defs::bundled();
    let mut w = earth(&defs);
    // Against the top-left corner: the edge tiles inside start the flood.
    let wall = defs.building_id("earth_wall").unwrap();
    for i in 0..=4 {
        w.put_building(Footprint::single(at(4, i)), wall, &defs);
        w.put_building(Footprint::single(at(i, 4)), wall, &defs);
    }
    w.refresh_enclosure(&defs);
    assert!(!w.is_sheltered(at(1, 1), &defs));

    // Rock in place of one side doesn't close it either (#716).
    let mut map = field(24, 20, 0);
    for y in 5..=10 {
        map[at(10, y)].terrain = Terrain::Mountain;
    }
    let mut w = World::with_map(1, map, Nation::Earth, &[at(20, 18)], &defs);
    w.research.grant(defs.tech_id("agrarian_ring").unwrap());
    let skip: Vec<Pos> = (5..=10).map(|y| at(10, y)).collect();
    ring(&mut w, &defs, 5, 10, &skip);
    assert!(!w.is_sheltered(at(7, 7), &defs));
}

#[test]
fn inside_a_ring_nobody_is_cold_and_winter_growth_has_a_floor() {
    let defs = Defs::bundled();
    let mut w = earth(&defs);
    w.difficulty = Some("normal".into());
    w.tick = 3 * defs.seasons.season_ticks(); // winter's first tick
    ring(&mut w, &defs, 5, 10, &[]);
    let (inside, outside) = (at(7, 7), at(15, 15));
    assert!(w.is_cold(outside, &defs) && !w.is_cold(inside, &defs));
    let out = w.season_growth_factor(outside, &defs, false);
    assert!(out < 0.75, "the polar band is harsher: {out}");
    assert_eq!(w.season_growth_factor(inside, &defs, false), 0.75);
    assert!(w.ring_spares_frost(inside, &defs) && !w.ring_spares_frost(outside, &defs));
}
