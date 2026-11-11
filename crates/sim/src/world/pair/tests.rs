use super::*;
use crate::building::PairDef;
use crate::defs::{BiomeId, Terrain};
use crate::geom::Grid;
use crate::mapgen::Tile;

const WIDTH: i32 = 80;

fn flat() -> TileMap {
    Grid::from_fn(WIDTH, 20, |_| Tile {
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

/// The partner positions: `A` and `B` are 25 apart, `C` is 5 from `A`, `D` is 65 from `A`,
/// `E` is 13 from `A` and 12 from `B`.
const A: Pos = Pos::new(5, 5);
const B: Pos = Pos::new(30, 5);
const C: Pos = Pos::new(10, 5);
const D: Pos = Pos::new(70, 5);
const E: Pos = Pos::new(18, 17);

/// The Study Hall (2x2, with a door) stands in for a pair building: the bundled data has none yet.
fn setup() -> (World, Defs, BuildingId) {
    let mut defs = Defs::bundled();
    let twin = defs.building_id("study_hall").unwrap();
    defs.buildings[twin.0 as usize].pair = Some(PairDef { reach: 48, min_tiles: 12 });
    let w = World::with_map(1, flat(), Nation::Earth, &[at(3, 15)], &defs);
    (w, defs, twin)
}

fn stand(w: &mut World, defs: &Defs, id: BuildingId, at: Pos) {
    w.put_building(Footprint::new(at, 2, 2), id, defs);
}

fn site(w: &mut World, defs: &Defs, id: BuildingId, at: Pos) {
    w.apply(Command::DesignateBuild { pos: at, building: id }, defs).unwrap();
}

fn link(w: &mut World, defs: &Defs, pos: Pos, to: Pos) -> Result<(), CommandError> {
    w.apply(Command::SetLink { pos, to }, defs)
}

fn refused(e: LinkError) -> Result<(), CommandError> {
    Err(CommandError::Link(e))
}

/// A column of sea at x = 20 splits the map in two.
fn split(w: &mut World) {
    for y in 0..20 {
        w.map[at(20, y)].terrain = Terrain::Water;
    }
    w.map_changed();
}

fn join(w: &mut World) {
    for y in 0..20 {
        w.map[at(20, y)].terrain = Terrain::Land;
    }
    w.map_changed();
}

#[test]
fn a_partner_must_be_a_pair_of_the_same_id_in_range() {
    let (mut w, defs, twin) = setup();
    for p in [A, B, C, D] {
        stand(&mut w, &defs, twin, p);
    }
    let campfire = defs.building_id("campfire").unwrap();
    w.put_building(Footprint::single(at(40, 15)), campfire, &defs);
    assert_eq!(link(&mut w, &defs, at(60, 15), B), refused(LinkError::NotLinkable));
    assert_eq!(link(&mut w, &defs, at(40, 15), B), refused(LinkError::NotLinkable), "no pair");
    assert_eq!(link(&mut w, &defs, A, at(40, 15)), refused(LinkError::NotPartner), "other id");
    assert_eq!(link(&mut w, &defs, A, at(50, 15)), refused(LinkError::NotPartner), "nothing");
    assert_eq!(link(&mut w, &defs, A, A), refused(LinkError::NotPartner), "itself");
    assert_eq!(link(&mut w, &defs, A, D), refused(LinkError::TooFar), "65 tiles");
    assert_eq!(link(&mut w, &defs, A, C), refused(LinkError::TooClose), "5 tiles");
    assert_eq!(w.anchored(A).unwrap().link, None, "refusals change nothing");
    assert_eq!(w.pair_status(A, &defs), Some(PairStatus::NoPartner));
    // Refusals read as fragments.
    assert_eq!(CommandError::Link(LinkError::TooClose).to_string(), "too close");
    assert_eq!(CommandError::Link(LinkError::NotPartner).to_string(), "not a partner");
    assert_eq!(CommandError::Link(LinkError::NotReachable).to_string(), "not reachable on foot");
    // The edges of the range are in: 12 tiles and 48 tiles.
    assert_eq!(link(&mut w, &defs, A, at(17, 5)), refused(LinkError::NotPartner));
    stand(&mut w, &defs, twin, at(17, 5));
    stand(&mut w, &defs, twin, at(53, 15));
    assert_eq!(link(&mut w, &defs, A, at(17, 5)), Ok(()), "12 away");
    assert_eq!(link(&mut w, &defs, A, at(53, 15)), Ok(()), "48 away");
}

#[test]
fn linking_sets_both_ends_and_lets_go_of_old_partners() {
    let (mut w, defs, twin) = setup();
    for p in [A, B, E] {
        stand(&mut w, &defs, twin, p);
    }
    link(&mut w, &defs, A, B).unwrap();
    assert_eq!(w.anchored(A).unwrap().link, Some(B));
    assert_eq!(w.anchored(B).unwrap().link, Some(A));
    assert_eq!(w.pair_status(A, &defs), Some(PairStatus::Linked { partner: B, distance: 25 }));
    assert_eq!(w.pair_status(B, &defs), Some(PairStatus::Linked { partner: A, distance: 25 }));
    assert_eq!(w.pair_partner(A, &defs), Some(B));
    // Linking again is harmless.
    link(&mut w, &defs, B, A).unwrap();
    assert_eq!(w.pair_partner(A, &defs), Some(B));
    // B takes E: A is let go.
    link(&mut w, &defs, B, E).unwrap();
    assert_eq!(w.anchored(A).unwrap().link, None);
    assert_eq!(w.pair_status(A, &defs), Some(PairStatus::NoPartner));
    assert_eq!(w.pair_partner(B, &defs), Some(E));
    assert_eq!(w.pair_partner(E, &defs), Some(B));
    // A takes E: B is let go in turn.
    link(&mut w, &defs, A, E).unwrap();
    assert_eq!(w.anchored(B).unwrap().link, None);
    assert_eq!(w.pair_partner(E, &defs), Some(A));
    assert_eq!(w.pair_partner(A, &defs), Some(E));
}

#[test]
fn a_building_that_is_not_a_pair_has_no_status() {
    let (w, defs, _) = setup();
    assert_eq!(w.pair_status(A, &defs), None);
    assert_eq!(w.pair_reach(A, &defs), None);
    let (w, defs, twin) = {
        let (mut w, defs, twin) = setup();
        stand(&mut w, &defs, twin, A);
        (w, defs, twin)
    };
    assert_eq!(w.pair_reach(A, &defs), Some((48, 12)));
    assert_eq!(w.anchored(A).unwrap().id, twin);
}

#[test]
fn a_site_may_be_linked_and_the_pair_works_once_both_stand() {
    let (mut w, defs, twin) = setup();
    site(&mut w, &defs, twin, A);
    site(&mut w, &defs, twin, B);
    link(&mut w, &defs, A, B).unwrap();
    assert_eq!(w.link_at(A), Some(B));
    assert_eq!(w.link_at(B), Some(A));
    assert_eq!(w.pair_status(A, &defs), Some(PairStatus::PartnerUnbuilt));
    assert_eq!(w.pair_partner(A, &defs), None);
    // A finishes: the finished end keeps its link, and B is still unbuilt.
    w.complete_site(A, &defs);
    assert_eq!(w.anchored(A).unwrap().link, Some(B));
    assert_eq!(w.pair_status(A, &defs), Some(PairStatus::PartnerUnbuilt));
    assert_eq!(w.pair_status(B, &defs), Some(PairStatus::PartnerUnbuilt), "a site isn't working");
    w.complete_site(B, &defs);
    assert_eq!(w.pair_status(A, &defs), Some(PairStatus::Linked { partner: B, distance: 25 }));
    assert_eq!(w.pair_status(B, &defs), Some(PairStatus::Linked { partner: A, distance: 25 }));
}

#[test]
fn a_site_links_to_a_finished_partner() {
    let (mut w, defs, twin) = setup();
    stand(&mut w, &defs, twin, A);
    site(&mut w, &defs, twin, B);
    link(&mut w, &defs, B, A).unwrap();
    assert_eq!(w.anchored(A).unwrap().link, Some(B));
    assert_eq!(w.pair_status(A, &defs), Some(PairStatus::PartnerUnbuilt));
    assert_eq!(link(&mut w, &defs, B, C), refused(LinkError::NotPartner), "no building at C");
}

#[test]
fn taking_down_or_cancelling_either_end_clears_the_other() {
    let (mut w, defs, twin) = setup();
    for p in [A, B, E] {
        stand(&mut w, &defs, twin, p);
    }
    link(&mut w, &defs, A, B).unwrap();
    let key = w.map[A].building.unwrap().key();
    w.remove_building(key, &defs).unwrap();
    assert_eq!(w.anchored(B).unwrap().link, None);
    assert_eq!(w.pair_status(B, &defs), Some(PairStatus::NoPartner));
    // A site cancelled likewise.
    site(&mut w, &defs, twin, A);
    link(&mut w, &defs, A, B).unwrap();
    w.apply(Command::CancelBuild { pos: A }, &defs).unwrap();
    assert_eq!(w.anchored(B).unwrap().link, None);
    // And a finished end whose partner is a site.
    site(&mut w, &defs, twin, A);
    link(&mut w, &defs, E, A).unwrap();
    let key = w.map[E].building.unwrap().key();
    w.remove_building(key, &defs).unwrap();
    assert_eq!(w.link_at(A), None);
    // Cancelling a linked site once more.
    link(&mut w, &defs, B, A).unwrap();
    w.apply(Command::CancelBuild { pos: A }, &defs).unwrap();
    assert_eq!(w.link_at(B), None);
}

#[test]
fn both_doors_must_be_in_one_region() {
    let (mut w, defs, twin) = setup();
    for p in [A, B] {
        stand(&mut w, &defs, twin, p);
    }
    split(&mut w);
    assert_eq!(link(&mut w, &defs, A, B), refused(LinkError::NotReachable));
    assert_eq!(w.anchored(A).unwrap().link, None);
    join(&mut w);
    link(&mut w, &defs, A, B).unwrap();
    assert!(matches!(w.pair_status(A, &defs), Some(PairStatus::Linked { .. })));
    // A later map change that splits them reports Cut off and does nothing.
    split(&mut w);
    assert_eq!(w.pair_status(A, &defs), Some(PairStatus::CutOff));
    assert_eq!(w.pair_status(B, &defs), Some(PairStatus::CutOff));
    assert_eq!(w.pair_partner(A, &defs), None);
    assert_eq!(w.anchored(A).unwrap().link, Some(B), "the link itself is kept");
    // The same answer once the regions cache is current.
    w.ensure_regions();
    assert_eq!(w.pair_status(A, &defs), Some(PairStatus::CutOff));
    // When the ground joins up again the pair works again.
    join(&mut w);
    assert_eq!(w.pair_status(A, &defs), Some(PairStatus::Linked { partner: B, distance: 25 }));
}

#[test]
fn a_door_that_cannot_be_stood_on_is_cut_off() {
    let (mut w, defs, twin) = setup();
    for p in [A, B] {
        stand(&mut w, &defs, twin, p);
    }
    link(&mut w, &defs, A, B).unwrap();
    // The door of the 2x2 at A is the tile below its left column.
    assert_eq!(Footprint::new(A, 2, 2).entrance(), at(5, 7));
    w.map[at(5, 7)].terrain = Terrain::Water;
    w.map_changed();
    assert_eq!(w.pair_status(A, &defs), Some(PairStatus::CutOff));
}

#[test]
fn links_survive_a_save_round_trip() {
    let (mut w, defs, twin) = setup();
    stand(&mut w, &defs, twin, A);
    site(&mut w, &defs, twin, B);
    stand(&mut w, &defs, twin, E);
    link(&mut w, &defs, A, B).unwrap();
    let w2 = World::load(&w.save(&defs).expect("saves"), &defs).expect("loads");
    assert_eq!(w2.anchored(A).unwrap().link, Some(B));
    assert_eq!(w2.link_at(B), Some(A), "a site's link too");
    assert_eq!(w2.pair_status(A, &defs), Some(PairStatus::PartnerUnbuilt));
    let mut w3 = w2;
    w3.complete_site(B, &defs);
    assert_eq!(w3.pair_status(A, &defs), Some(PairStatus::Linked { partner: B, distance: 25 }));
    let w4 = World::load(&w3.save(&defs).expect("saves"), &defs).expect("loads");
    assert_eq!(w4.pair_partner(B, &defs), Some(A));
}

#[test]
fn linking_is_deterministic() {
    let run = || {
        let (mut w, defs, twin) = setup();
        for p in [A, B, E] {
            stand(&mut w, &defs, twin, p);
        }
        site(&mut w, &defs, twin, at(60, 10));
        let _ = link(&mut w, &defs, A, D);
        link(&mut w, &defs, A, B).unwrap();
        link(&mut w, &defs, E, B).unwrap();
        link(&mut w, &defs, at(60, 10), E).unwrap();
        for _ in 0..50 {
            w.tick(&defs);
        }
        w.save(&defs).expect("saves")
    };
    assert_eq!(run(), run());
}
