use super::*;
use crate::defs::{BiomeId, Terrain};
use crate::geom::{Footprint, Grid};
use crate::jobs::JobKind;
use crate::mapgen::Tile;

/// A 30x10 slope falling 5 elevation per tile eastward, with a mountain column at x = 0.
fn slope() -> TileMap {
    Grid::from_fn(30, 10, |p| Tile {
        elevation: (200 - 5 * p.x) as u8,
        temperature: 128,
        moisture: 128,
        terrain: if p.x == 0 { Terrain::Mountain } else { Terrain::Land },
        biome: BiomeId(0),
        resource: None,
        building: None,
    })
}

fn at(x: i32, y: i32) -> Pos {
    Pos::new(x, y)
}

/// The chute's head, against the rock, and a valid foot 11 tiles east and 55 lower.
const HEAD: Pos = Pos::new(1, 5);
const FOOT: Pos = Pos::new(12, 5);

fn setup(unit: Pos) -> (World, Defs) {
    let defs = Defs::bundled();
    let mut w = World::with_map(1, slope(), Nation::Earth, &[unit], &defs);
    w.put_building(Footprint::single(HEAD), defs.building_id("chute").unwrap(), &defs);
    (w, defs)
}

fn run_until(w: &mut World, defs: &Defs, limit: u64, mut done: impl FnMut(&World) -> bool) {
    for _ in 0..limit {
        if done(w) {
            return;
        }
        w.tick(defs);
    }
    panic!("not done after {limit} ticks");
}

#[test]
fn the_chute_is_earths_and_cut_into_rock() {
    let defs = Defs::bundled();
    let def = defs.building(defs.building_id("chute").unwrap());
    assert_eq!(def.unlocked_by, defs.tech_id("omashu_chutes"));
    let tech = defs.tech(defs.tech_id("omashu_chutes").unwrap());
    assert_eq!(tech.nation, Some(Nation::Earth));
    assert!(!tech.core, "an optional horizons tech");
    assert_eq!(def.chute, Some(crate::building::ChuteDef { reach: 16, drop: 8 }));
    assert_eq!(def.beside, Some(vec![Terrain::Mountain, Terrain::Tunnel]));
}

#[test]
fn a_foot_must_be_open_in_reach_and_downhill() {
    let (mut w, defs) = setup(at(5, 8));
    let set = |w: &mut World, pos: Pos, to: Pos| w.apply(Command::SetLink { pos, to }, &defs);
    let refused = |e: LinkError| Err(CommandError::Link(e));
    assert_eq!(set(&mut w, at(3, 3), FOOT), refused(LinkError::NotAChute));
    assert_eq!(set(&mut w, HEAD, at(40, 5)), refused(LinkError::OffMap));
    assert_eq!(set(&mut w, HEAD, at(0, 5)), refused(LinkError::NotOpen), "rock");
    assert_eq!(set(&mut w, HEAD, HEAD), refused(LinkError::NotOpen), "itself");
    assert_eq!(set(&mut w, HEAD, at(18, 5)), refused(LinkError::TooFar), "17 tiles");
    // One tile east is 5 lower, short of the 8 drop.
    assert_eq!(set(&mut w, HEAD, at(2, 5)), refused(LinkError::NotDownhill));
    assert_eq!(w.anchored(HEAD).unwrap().link, None, "refusals change nothing");
    assert_eq!(w.chute_foot(HEAD, &defs), None, "no foot: it does nothing");
    set(&mut w, HEAD, FOOT).unwrap();
    assert_eq!(w.chute_foot(HEAD, &defs), Some(FOOT));
    // Refusals read as fragments.
    assert_eq!(CommandError::Link(LinkError::TooFar).to_string(), "foot out of reach");
    // A wall on the foot later stops the chute.
    let wall = defs.building_id("earth_wall").unwrap();
    w.put_building(Footprint::single(FOOT), wall, &defs);
    assert_eq!(w.chute_foot(HEAD, &defs), None);
}

#[test]
fn a_hauler_sends_a_stack_down_the_chute_when_that_is_the_shorter_way() {
    // The stack lies beside the head; the only stockpile is beside the foot, 12 tiles off.
    let (mut w, defs) = setup(at(3, 5));
    let stone = defs.item_id("stone").unwrap();
    w.apply(Command::SetLink { pos: HEAD, to: FOOT }, &defs).unwrap();
    w.designate_stockpile(at(13, 5)).unwrap();
    w.ground.insert(at(2, 6), Stack { item: stone, count: 4 });
    w.ensure_haul_job(at(2, 6));
    run_until(&mut w, &defs, 600, |w| w.units[0].job_task == Some(Task::Chute(HEAD)));
    // It drops in at the head and lands at the foot on the same tick, as a ground stack.
    run_until(&mut w, &defs, 600, |w| w.ground.contains_key(&FOOT));
    assert_eq!(w.ground[&FOOT], Stack { item: stone, count: 4 });
    assert!(w.units[0].carrying.is_none());
    assert!(w.jobs.iter().any(|j| j.kind == JobKind::Haul { pos: FOOT }), "an ordinary haul");
    run_until(&mut w, &defs, 2000, |w| w.stored_total(stone) == 4);
}

#[test]
fn a_hauler_walks_when_stores_are_nearer_than_the_chute_route() {
    let (mut w, defs) = setup(at(6, 5));
    let stone = defs.item_id("stone").unwrap();
    w.apply(Command::SetLink { pos: HEAD, to: FOOT }, &defs).unwrap();
    w.designate_stockpile(at(7, 5)).unwrap();
    w.ground.insert(at(6, 6), Stack { item: stone, count: 4 });
    w.ensure_haul_job(at(6, 6));
    for _ in 0..600 {
        assert_ne!(w.units[0].job_task, Some(Task::Chute(HEAD)));
        w.tick(&defs);
    }
    assert_eq!(w.stored_total(stone), 4);
}

#[test]
fn the_link_survives_a_save_round_trip() {
    let (mut w, defs) = setup(at(5, 8));
    w.apply(Command::SetLink { pos: HEAD, to: FOOT }, &defs).unwrap();
    let saved = w.save(&defs).expect("saves");
    let w2 = World::load(&saved, &defs).expect("loads");
    assert_eq!(w2.anchored(HEAD).unwrap().link, Some(FOOT));
    assert_eq!(w2.chute_foot(HEAD, &defs), Some(FOOT));
}

#[test]
fn a_site_takes_its_foot_and_hands_it_to_the_finished_chute() {
    let defs = Defs::bundled();
    let mut w = World::with_map(1, slope(), Nation::Earth, &[at(3, 6)], &defs);
    let site = at(1, 3);
    let chute = defs.building_id("chute").unwrap();
    // Straight onto the map as a site: designating needs the tech, which isn't the point here.
    w.sites.insert(site, super::super::build::Site::gate(chute, [1, 1]));
    w.apply(Command::SetLink { pos: site, to: at(12, 3) }, &defs).unwrap();
    assert_eq!(w.link_at(site), Some(at(12, 3)));
    assert_eq!(w.chute_foot(site, &defs), None, "a site sends nothing");
    assert_eq!(
        w.apply(Command::SetLink { pos: site, to: at(2, 3) }, &defs),
        Err(CommandError::Link(LinkError::NotDownhill))
    );
}

#[test]
fn the_chute_counts_what_it_sends() {
    let (mut w, defs) = setup(at(3, 5));
    let stone = defs.item_id("stone").unwrap();
    w.apply(Command::SetLink { pos: HEAD, to: FOOT }, &defs).unwrap();
    w.designate_stockpile(at(13, 5)).unwrap();
    w.ground.insert(at(2, 6), Stack { item: stone, count: 4 });
    w.ensure_haul_job(at(2, 6));
    run_until(&mut w, &defs, 1200, |w| !w.chute_sent(HEAD).is_empty());
    assert_eq!(w.chute_sent(HEAD), [Stack { item: stone, count: 4 }]);
}
