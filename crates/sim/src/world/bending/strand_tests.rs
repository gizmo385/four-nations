//! #490: nothing that appears on the map later may seal ground off for good. A resource blocks
//! movement for as long as it stands (and a renewable one is never taken off the map), so one
//! growing in a one-tile corridor, or a ruin uncovered there, would strand whoever is beyond it.

use super::*;
use crate::geom::{Footprint, Grid};
use crate::mapgen::Tile;

fn at(x: i32, y: i32) -> Pos {
    Pos::new(x, y)
}

fn biome(defs: &Defs, id: &str) -> BiomeId {
    BiomeId(defs.biomes.iter().position(|b| b.id == id).unwrap() as u8)
}

/// A 12x9 map of mountain with open plains only where `open` says. One unit, at (1, 4).
fn pocket(defs: &Defs, open: impl Fn(Pos) -> bool) -> World {
    let plains = biome(defs, "plains");
    let map = Grid::from_fn(12, 9, |p| Tile {
        elevation: 100,
        temperature: 128,
        moisture: 128,
        terrain: if open(p) { Terrain::Land } else { Terrain::Mountain },
        biome: plains,
        resource: None,
        building: None,
    });
    World::with_map(1, map, Nation::Water, &[at(1, 4)], defs)
}

fn patch(defs: &Defs, centre: Pos, sprouts: u32) -> Patch {
    Patch {
        centre,
        dries_at: 0,
        sprouts,
        resource: defs.resource_id("cabbage").unwrap(),
        biomes: vec![biome(defs, "plains")],
        fallow_ticks: 1,
    }
}

fn cabbages(w: &World, defs: &Defs) -> Vec<Pos> {
    let cabbage = defs.resource_id("cabbage");
    w.map
        .iter()
        .filter(|(_, t)| t.resource.is_some() && t.resource == cabbage)
        .map(|(p, _)| p)
        .collect()
}

/// Whether every open tile is still reachable from the unit's tile, `grown` tiles aside.
fn all_reachable(w: &World, grown: &[Pos]) -> bool {
    let flood = crate::flood::Flood::new(&w.map, at(1, 4), &crate::path::MoveRules::default());
    w.map
        .iter()
        .all(|(p, t)| t.terrain != Terrain::Land || grown.contains(&p) || flood.cost(p).is_some())
}

#[test]
fn a_sprout_never_seals_a_corridor() {
    let defs = Defs::bundled();
    // A corridor along row 4 from x = 1 to 10, one tile wide, under a patch centred on (3, 4).
    let mut w = pocket(&defs, |p| p.y == 4 && (1..=10).contains(&p.x));
    assert!(!w.sprout(&patch(&defs, at(3, 4), 2), &defs));
    assert!(cabbages(&w, &defs).is_empty());
    assert!(w.fallow_tiles().is_empty());
}

/// A two-wide stretch under the patch: sprouts with a way round still grow, but not enough to
/// close it (each is checked against the sprouts already placed in the same patch).
#[test]
fn a_sprout_may_grow_where_there_is_a_way_round_but_never_seals_the_last_one() {
    let defs = Defs::bundled();
    let mut w = pocket(&defs, |p| (1..=10).contains(&p.x) && (p.y == 4 || (p.y == 5 && p.x <= 6)));
    assert!(w.sprout(&patch(&defs, at(3, 4), 9), &defs));
    let grown = cabbages(&w, &defs);
    assert!(!grown.is_empty(), "there is a way round, so something grows");
    assert!(grown.len() < 6, "the six tiles in the patch can't all grow: {grown:?}");
    assert!(all_reachable(&w, &grown));
}

/// A sprout never lands on a finished building (a bed is walkable ground; a cabbage on it
/// blocks the tile) or a pending site, as the Water seed 3 settlement of #487 found out.
#[test]
fn a_sprout_never_lands_on_a_building_or_a_site() {
    let defs = Defs::bundled();
    let bed = defs.building_id("bed").unwrap();
    let mut w = pocket(&defs, |p| (1..=10).contains(&p.x) && (2..=7).contains(&p.y));
    w.raise(Footprint::single(at(3, 4)), bed, defs.building(bed));
    w.designate_build(at(2, 4), bed, &defs).unwrap();
    let patch = patch(&defs, at(3, 4), 7);
    assert!(!w.can_sprout(at(3, 4), &patch, &defs), "a finished bed");
    assert!(!w.can_sprout(at(2, 4), &patch, &defs), "a pending site");
    assert!(w.can_sprout(at(4, 4), &patch, &defs), "bare ground");
}

/// A ruin buried in the one way through a narrow place stays buried, however it is asked to
/// come up (a Map Room's chart, Seismic Sense, ...: all go through `free_to_uncover`), until
/// there is a way round it. On open ground it comes up as it always did.
#[test]
fn a_ruin_in_the_only_corridor_stays_buried() {
    let defs = Defs::bundled();
    let ruin = ResourceId(defs.resources.iter().position(|r| r.buried).unwrap() as u16);
    let mut w = pocket(&defs, |p| p.y == 4 && (1..=10).contains(&p.x));
    w.buried.insert(at(5, 4), ruin);
    assert!(!w.free_to_uncover(at(5, 4)));
    assert!(!w.uncover(at(5, 4)));
    assert_eq!(w.map[at(5, 4)].resource, None);
    assert_eq!(w.buried_count(), 1);

    // Widen the corridor beside it, and it can come up.
    for x in 4..=6 {
        w.map[at(x, 5)].terrain = Terrain::Land;
    }
    assert!(w.uncover(at(5, 4)));
    assert_eq!(w.map[at(5, 4)].resource, Some(ruin));
}

/// `would_cut_off`: open ground, a dead end and a tile nothing walks on strand nothing; a tile
/// in a one-wide gap does.
#[test]
fn would_cut_off_reads_the_ground_round_a_tile() {
    let defs = Defs::bundled();
    let corridor = pocket(&defs, |p| p.y == 4 && (1..=10).contains(&p.x));
    assert!(corridor.would_cut_off(at(5, 4)), "mid-corridor");
    assert!(!corridor.would_cut_off(at(10, 4)), "the dead end has nothing beyond it");
    assert!(!corridor.would_cut_off(at(1, 4)), "nor has the start");
    assert!(!corridor.would_cut_off(at(5, 3)), "rock isn't walkable, so blocking it is moot");

    let field = pocket(&defs, |p| (1..=10).contains(&p.x) && (1..=7).contains(&p.y));
    assert!(!field.would_cut_off(at(5, 4)), "open field");
    assert!(!field.would_cut_off(at(1, 1)), "a corner");
    assert!(!field.would_cut_off(at(1, 4)), "the edge of a field");

    // An hourglass: two chambers joined by one tile.
    let hourglass = pocket(&defs, |p| {
        (1..=9).contains(&p.x)
            && ((1..=3).contains(&p.y) || (p == at(5, 4)) || (5..=7).contains(&p.y))
    });
    assert!(hourglass.would_cut_off(at(5, 4)));
    assert!(hourglass.would_cut_off(at(5, 3)), "the neck's only door into the upper chamber");
    assert!(!hourglass.would_cut_off(at(2, 2)), "the chamber's own floor has a way round");
}

/// A ring of ground broken into several runs isn't enough to cut anything off if a way round
/// exists further out: a pillar in a wide corridor can be blocked, since there is room past it.
#[test]
fn would_cut_off_lets_a_wider_detour_through() {
    let defs = Defs::bundled();
    // Two lanes (rows 3 and 5) round a solid row-4 wall, joined at both ends by columns 1 and 10.
    let mut w = pocket(&defs, |p| {
        (1..=10).contains(&p.x) && ((3..=5).contains(&p.y) && (p.y != 4 || p.x == 1 || p.x == 10))
    });
    assert!(!w.would_cut_off(at(1, 4)), "the lanes still meet at the far end");
    assert!(!w.would_cut_off(at(5, 3)), "the other lane is still there");
    // Once one end is closed, the other is the lanes' only link.
    w.map[at(1, 4)].resource = Some(ResourceId(0));
    assert!(w.would_cut_off(at(10, 4)));
}
