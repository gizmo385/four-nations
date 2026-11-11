//! The Badgermole Den (#697, docs/design/horizons-phase-2.md, "`badgermole_ways`"): a badgermole
//! bores designated tunnel with no chi, nearest its den first.

use super::*;
use crate::bending::AbilityId;
use crate::defs::{BiomeId, Terrain};
use crate::geom::Grid;
use crate::jobs::ItemId;
use crate::mapgen::Tile;
use crate::world::BendOutcome;

/// The den's anchor: it fills (8..=9, 4..=5), its door is (8, 6), and the rock starts at x = 10.
const DEN: Pos = Pos::new(8, 4);
const DOOR: Pos = Pos::new(8, 6);

fn at(x: i32, y: i32) -> Pos {
    Pos::new(x, y)
}

fn bore(defs: &Defs) -> AbilityId {
    defs.ability_id("bore_tunnel").unwrap()
}

fn item(defs: &Defs, id: &str) -> ItemId {
    defs.item_id(id).unwrap()
}

fn beast(w: &World) -> &Unit {
    w.beasts().next().expect("a beast")
}

/// Open ground in columns 0 to 9 and rock from column 10 east, 30 wide and 12 tall.
fn rock() -> TileMap {
    Grid::from_fn(30, 12, |p| Tile {
        elevation: 200,
        temperature: 128,
        moisture: 128,
        terrain: if p.x >= 10 { Terrain::Mountain } else { Terrain::Land },
        biome: BiomeId(0),
        resource: None,
        building: None,
    })
}

/// An Earth world with `badgermole_ways` and `tunneling` known, `settlers`, and a finished den
/// holding `food` cabbages, with its badgermole already at the door.
fn setup(settlers: &[Pos], food: u32, defs: &Defs) -> World {
    let mut w = World::with_map(1, rock(), Nation::Earth, settlers, defs);
    w.research.grant(defs.tech_id("tunneling").unwrap());
    w.research.grant(defs.tech_id("badgermole_ways").unwrap());
    w.refresh_modifiers(defs);
    w.raise(Footprint::new(DEN, 2, 2), defs.building_id("badgermole_den").unwrap(), defs);
    w.tick(defs);
    feed(&mut w, food, defs);
    w
}

fn feed(w: &mut World, food: u32, defs: &Defs) {
    let bunker = w.anchored_mut(DEN).and_then(|b| b.fuel.as_mut()).expect("the den has a bunker");
    bunker.count = food;
    bunker.item = Some(item(defs, "cabbage"));
}

/// Designates a dragged line of `len` tiles out from the rock's edge along row 6.
fn line(w: &mut World, len: i32, defs: &Defs) {
    for x in 10..10 + len {
        w.apply(Command::Bend { ability: bore(defs), pos: at(x, 6) }, defs).unwrap();
    }
}

fn run(w: &mut World, defs: &Defs, ticks: u32) {
    for _ in 0..ticks {
        w.tick(defs);
    }
}

fn done_tiles(w: &World) -> Vec<Pos> {
    w.bend_events().filter(|e| e.outcome == BendOutcome::Done).map(|e| e.pos).collect()
}

#[test]
fn the_data_is_the_designs() {
    let defs = Defs::bundled();
    let row = defs.beast(defs.beast_id("badgermole").unwrap());
    assert_eq!((row.nation, row.speed), (Nation::Earth, Some(0.8)));
    assert!(matches!(row.work, BeastWork::Bend { seconds, .. } if seconds == 120.0));
    let den = defs.building(row.home);
    assert_eq!(den.id, "badgermole_den");
    let fuel = den.fuel.as_ref().expect("it eats");
    assert_eq!((fuel.bunker, fuel.per_minute), (10, Some(0.2)));
    let tech = defs.tech(defs.tech_id("badgermole_ways").unwrap());
    assert_eq!((tech.cost, tech.core), (500, false));
}

#[test]
fn a_dragged_line_is_bored_in_order_from_the_den_with_no_chi() {
    let defs = Defs::bundled();
    let mut w = setup(&[], 10, &defs);
    assert_eq!(w.boring_left(DEN, &defs), Some(0));
    line(&mut w, 5, &defs);
    assert_eq!(w.boring_left(DEN, &defs), Some(5));
    run(&mut w, &defs, 9_000);
    let want: Vec<Pos> = (10..15).map(|x| at(x, 6)).collect();
    assert_eq!(done_tiles(&w), want, "bored outward, one real tile at a time");
    for p in &want {
        assert_eq!(w.map[*p].terrain, Terrain::Tunnel);
    }
    assert_eq!((w.boring_left(DEN, &defs), w.bored_by(DEN, &defs)), (Some(0), Some(5)));
    let mole = beast(&w);
    assert_eq!(mole.chi, 0.0, "no chi was spent");
    assert_eq!((mole.bends(), mole.forms()), (0, 0), "no mastery counts it");
    // Five tiles at 120 s each from a standing start: 6000 ticks, and a few steps more.
    let last = w.bend_events().last().unwrap().tick;
    assert!((6_000..8_000).contains(&last), "a tile takes 120 s, finished at {last}");
    // 5 tiles in 10 minutes at 0.2 a minute eats 2 of the 10.
    let left = w.anchored(DEN).unwrap().fuel.unwrap().count;
    assert!((7..=8).contains(&left), "it ate while it worked: {left}");
    w.assert_invariants(&defs);
}

#[test]
fn a_tile_takes_the_rows_seconds_and_not_the_nation_s_modifier() {
    let defs = Defs::bundled();
    let mut w = setup(&[], 10, &defs);
    w.apply(Command::Bend { ability: bore(&defs), pos: at(10, 6) }, &defs).unwrap();
    let mut channelling = None;
    for _ in 0..3_000 {
        w.tick(&defs);
        if let Some(c) = w.bend_channels(&defs).first() {
            channelling = Some(c.remaining_secs);
            break;
        }
    }
    let remaining = channelling.expect("it starts channelling");
    assert!((115.0..=120.0).contains(&remaining), "{remaining} s left of 120");
    run(&mut w, &defs, 1_300);
    assert_eq!(done_tiles(&w), [at(10, 6)]);
}

#[test]
fn it_bores_the_ready_tile_nearest_its_den() {
    let defs = Defs::bundled();
    let mut w = setup(&[], 10, &defs);
    // Two ready tiles on the rock's edge: (10, 9) is farther from the door (8, 6) than (10, 7).
    for p in [at(10, 9), at(10, 7)] {
        w.apply(Command::Bend { ability: bore(&defs), pos: p }, &defs).unwrap();
    }
    run(&mut w, &defs, 6_000);
    assert_eq!(done_tiles(&w), [at(10, 7), at(10, 9)]);
}

#[test]
fn a_settler_with_chi_may_still_claim_the_same_designations() {
    let defs = Defs::bundled();
    // The den has no food: the badgermole rests, so the settler is the only hand.
    let mut w = setup(&[at(9, 8)], 0, &defs);
    assert!(w.beast_hungry(beast(&w), &defs));
    line(&mut w, 2, &defs);
    w.units.iter_mut().find(|u| u.is_settler()).unwrap().chi = defs.chi.max;
    let mut spent = false;
    for _ in 0..3_000 {
        for u in w.units.iter_mut().filter(|u| u.is_settler()) {
            u.needs = crate::needs::Needs::full(&defs.needs);
        }
        w.tick(&defs);
        spent |= w.units.iter().any(|u| u.is_settler() && u.chi < defs.chi.max - 10.0);
    }
    assert_eq!(done_tiles(&w), [at(10, 6), at(11, 6)]);
    assert!(spent, "the settler paid chi");
    assert_eq!(w.units.iter().find(|u| u.is_settler()).unwrap().bends(), 2);
    assert_eq!(w.bored_by(DEN, &defs), Some(0), "the badgermole bored none");
}

#[test]
fn a_struck_seam_behaves_as_it_does_for_a_bender() {
    let defs = Defs::bundled();
    let mut w = setup(&[], 10, &defs);
    let gold = defs.resource_id("gold_vein").unwrap();
    w.buried.insert(at(10, 6), gold);
    w.apply(Command::Bend { ability: bore(&defs), pos: at(10, 6) }, &defs).unwrap();
    run(&mut w, &defs, 2_500);
    assert_eq!(w.map[at(10, 6)].resource, Some(gold), "the seam is uncovered");
    assert_eq!(w.map[at(10, 6)].terrain, Terrain::Mountain, "the rock stays");
    assert_eq!(w.bored_by(DEN, &defs), Some(1));
    assert_eq!(w.boring_left(DEN, &defs), Some(0));
}

#[test]
fn a_hungry_badgermole_stops() {
    let defs = Defs::bundled();
    let mut w = setup(&[], 0, &defs);
    line(&mut w, 6, &defs);
    run(&mut w, &defs, 3_000);
    assert!(done_tiles(&w).is_empty(), "it bores nothing on an empty stomach");
    assert!(beast(&w).job_task.is_none());
    assert_eq!(beast(&w).pos, DOOR);
    assert_eq!(w.boring_left(DEN, &defs), Some(6));
    // One cabbage burns out in 5 minutes at 0.2 a minute: it finishes the tile it holds when
    // the bunker runs dry, then rests, with the line unfinished.
    feed(&mut w, 1, &defs);
    run(&mut w, &defs, 9_000);
    let bored = done_tiles(&w).len();
    assert!((2..6).contains(&bored), "fed once, it bored {bored}");
    assert!(w.beast_hungry(beast(&w), &defs));
    assert!(beast(&w).job_task.is_none());
    assert_eq!(w.boring_left(DEN, &defs), Some(6 - bored));
    // Fed again, it finishes the line.
    feed(&mut w, 5, &defs);
    run(&mut w, &defs, 9_000);
    assert_eq!(done_tiles(&w).len(), 6);
}

#[test]
fn taking_the_den_down_leaves_the_designations_for_a_bender() {
    let defs = Defs::bundled();
    let mut w = setup(&[], 10, &defs);
    line(&mut w, 2, &defs);
    run(&mut w, &defs, 300);
    let key = w.map[DEN].building.unwrap().key();
    w.remove_building(key, &defs).expect("it stood");
    run(&mut w, &defs, 2_000);
    assert_eq!(w.beasts().count(), 0);
    assert_eq!(w.boring_left(DEN, &defs), None);
    assert!(done_tiles(&w).is_empty());
    assert_eq!(w.bend_designations().count(), 2);
    w.assert_invariants(&defs);
}

#[test]
fn boring_is_deterministic_and_survives_a_save() {
    let defs = Defs::bundled();
    let make = || {
        let mut w = setup(&[at(2, 2)], 10, &defs);
        line(&mut w, 4, &defs);
        w
    };
    let (mut a, mut b) = (make(), make());
    run(&mut a, &defs, 2_000);
    run(&mut b, &defs, 1_000);
    // Saved and loaded mid-bore, `b` carries on exactly as `a` did.
    let mut b = World::load(&b.save(&defs).expect("saves"), &defs).expect("loads");
    run(&mut b, &defs, 1_000);
    assert_eq!(a.units, b.units);
    run(&mut a, &defs, 6_000);
    run(&mut b, &defs, 6_000);
    assert_eq!(a.units, b.units);
    assert_eq!(done_tiles(&a), done_tiles(&b));
    assert_eq!(a.bored_by(DEN, &defs), b.bored_by(DEN, &defs));
    assert!(a.bored_by(DEN, &defs).unwrap() > 0);
}
