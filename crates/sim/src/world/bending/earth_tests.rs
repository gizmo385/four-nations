//! Raise Earth (#68): targets, the ford it leaves, reachability, chaining, and replay.

use super::*;
use crate::geom::Grid;
use crate::jobs::ItemId;
use crate::mapgen::Tile;

fn at(x: i32, y: i32) -> Pos {
    Pos::new(x, y)
}

/// A 13x7 plains field with `water` terrain in columns 5 to `last` (a river or lake spanning the
/// field top to bottom), cabbage on the east bank at (10, 3).
fn field(defs: &Defs, water: Terrain, last: i32) -> TileMap {
    let plains = BiomeId(defs.biomes.iter().position(|b| b.id == "plains").unwrap() as u8);
    let cabbage = defs.resource_id("cabbage");
    Grid::from_fn(13, 7, |p| Tile {
        elevation: 100,
        temperature: 128,
        moisture: 128,
        terrain: if (5..=last).contains(&p.x) { water } else { Terrain::Land },
        biome: plains,
        resource: if p == at(10, 3) { cabbage } else { None },
    })
}

fn world(nation: Nation, water: Terrain, last: i32, defs: &Defs) -> World {
    World::with_map(1, field(defs, water, last), nation, &[at(3, 3)], defs)
}

fn raise(defs: &Defs) -> AbilityId {
    defs.ability_id("raise_earth").unwrap()
}

fn cabbage_item(defs: &Defs) -> ItemId {
    defs.items.iter().position(|i| i.id == "cabbage").map(|i| ItemId(i as u16)).unwrap()
}

fn run_until(w: &mut World, defs: &Defs, limit: u64, mut done: impl FnMut(&World) -> bool) -> u64 {
    for n in 0..limit {
        if done(w) {
            return n;
        }
        w.tick(defs);
    }
    panic!("not done after {limit} ticks");
}

/// Raises `pos` and runs until the bend has finished, which it should have done successfully.
fn raise_at(w: &mut World, defs: &Defs, pos: Pos) {
    // Long runs get hungry and spend chi; neither is what is under test.
    w.units[0].needs = crate::needs::Needs::full(&defs.needs);
    w.units[0].chi = defs.chi.max;
    w.apply(Command::Bend { ability: raise(defs), pos }, defs).unwrap();
    let events = w.bend_events().len();
    run_until(w, defs, 2000, |w| w.bend_events().len() > events);
    assert_eq!(w.bend_events().last().unwrap().outcome, BendOutcome::Done);
}

#[test]
fn targets_are_river_or_lake_tiles_beside_dry_ground() {
    let defs = Defs::bundled();
    let mut w = world(Nation::Earth, Terrain::River, 5, &defs);
    w.map[at(8, 1)].terrain = Terrain::Water;
    w.map[at(7, 5)].terrain = Terrain::Lake;
    w.map[at(1, 5)].terrain = Terrain::Coast;
    w.map[at(1, 1)].terrain = Terrain::Mountain;
    w.map[at(5, 0)].terrain = Terrain::Ford;
    // A fishing spot on a river tile.
    w.map[at(5, 5)].resource = defs.resource_id("fish_spot");
    // A pool with no dry ground on any of its four sides (dry ground only touches diagonally).
    for p in [at(9, 5), at(10, 5), at(11, 5), at(9, 4), at(10, 4), at(11, 4)] {
        w.map[p].terrain = Terrain::Lake;
    }
    for p in [at(9, 6), at(10, 6), at(11, 6), at(8, 5), at(12, 5), at(8, 4), at(12, 4)] {
        w.map[p].terrain = Terrain::Lake;
    }
    w.map_changed();
    let ability = raise(&defs);
    let bend = |w: &mut World, pos| w.apply(Command::Bend { ability, pos }, &defs);
    let refused = |why| Err(CommandError::Bend(BendError::InvalidTarget(why)));

    assert_eq!(bend(&mut w, at(5, 3)), Ok(()), "river beside the bank");
    assert_eq!(bend(&mut w, at(5, 6)), Ok(()), "a tile at the map edge is fine");
    // The sea, the coast, rock, ordinary ground and an existing ford.
    assert_eq!(bend(&mut w, at(8, 1)), refused("the sea is out of reach"));
    assert_eq!(bend(&mut w, at(1, 5)), refused("the sea is out of reach"));
    assert_eq!(bend(&mut w, at(1, 1)), refused("needs a river or lake tile"));
    assert_eq!(bend(&mut w, at(3, 3)), refused("needs a river or lake tile"));
    assert_eq!(bend(&mut w, at(5, 0)), refused("already a ford"));
    assert_eq!(bend(&mut w, at(5, 5)), refused("something is in the way"));
    // Open water with dry ground only on its diagonals or not at all.
    assert_eq!(bend(&mut w, at(10, 5)), refused("needs dry ground beside it"));
    assert_eq!(bend(&mut w, at(-1, 0)), refused("off the map"));
    assert_eq!(w.command_log().len(), 2);
}

#[test]
fn a_bend_leaves_a_permanent_ford_and_spends_the_chi() {
    let defs = Defs::bundled();
    let mut w = world(Nation::Earth, Terrain::River, 5, &defs);
    // Right beside the river, so the walk is a tick or two.
    w.units[0].pos = at(4, 3);
    w.apply(Command::Bend { ability: raise(&defs), pos: at(5, 3) }, &defs).unwrap();
    let taken = run_until(&mut w, &defs, 500, |w| !w.bend_events().is_empty());
    // 15 s at earthworks 2.5 is 6 s, or 60 ticks.
    assert!((60..=70).contains(&taken), "{taken}");
    assert_eq!(w.map[at(5, 3)].terrain, Terrain::Ford);
    assert_eq!(w.map[at(5, 2)].terrain, Terrain::River, "only the target changed");
    assert!((w.units[0].chi - 50.0).abs() < 0.1, "{}", w.units[0].chi);
    assert_eq!(w.bend_events()[0].outcome, BendOutcome::Done);
    // A ford can't be raised again, and nothing turns it back.
    let again = w.apply(Command::Bend { ability: raise(&defs), pos: at(5, 3) }, &defs);
    assert_eq!(again, Err(CommandError::Bend(BendError::InvalidTarget("already a ford"))));
    for _ in 0..3000 {
        w.tick(&defs);
    }
    assert_eq!(w.map[at(5, 3)].terrain, Terrain::Ford);
}

#[test]
fn other_nations_cannot_designate_it() {
    let defs = Defs::bundled();
    for nation in [Nation::Water, Nation::Fire, Nation::Air] {
        let mut w = world(nation, Terrain::River, 5, &defs);
        let r = w.apply(Command::Bend { ability: raise(&defs), pos: at(5, 3) }, &defs);
        assert_eq!(r, Err(CommandError::Bend(BendError::WrongNation)));
        assert!(w.command_log().is_empty());
        assert_eq!(w.map[at(5, 3)].terrain, Terrain::River);
    }
}

#[test]
fn a_raised_ford_makes_the_far_bank_reachable() {
    let defs = Defs::bundled();
    let item = cabbage_item(&defs);
    let mut w = world(Nation::Earth, Terrain::River, 5, &defs);
    w.apply(Command::DesignateStockpile { pos: at(2, 3) }, &defs).unwrap();
    w.apply(Command::DesignateHarvest { pos: at(10, 3) }, &defs).unwrap();
    // The river splits the field: the cabbage can't be reached however long it waits.
    for _ in 0..1500 {
        w.tick(&defs);
    }
    assert_eq!(w.stored_total(item), 0);
    assert!(w.map[at(10, 3)].resource.is_some());
    assert!(w.units[0].pos.x < 5, "still on the west bank: {:?}", w.units[0].pos);

    w.apply(Command::Bend { ability: raise(&defs), pos: at(5, 2) }, &defs).unwrap();
    // While it channels, push its retry timer far out: finishing the bend must wake it.
    run_until(&mut w, &defs, 1000, |w| w.jobs().iter().any(|j| j.claimed_by.is_some()));
    w.units[0].claim_retry_at = w.tick + 1_000_000;
    run_until(&mut w, &defs, 1000, |w| !w.bend_events().is_empty());
    assert_eq!(w.units[0].claim_retry_at, 0, "idle units are woken");

    run_until(&mut w, &defs, 6000, |w| w.stored_total(item) == 1);
}

#[test]
fn a_wide_lake_is_crossed_one_tile_at_a_time() {
    let defs = Defs::bundled();
    let mut w = world(Nation::Earth, Terrain::Lake, 7, &defs);
    let ability = raise(&defs);
    let refused = Err(CommandError::Bend(BendError::InvalidTarget("needs dry ground beside it")));
    // Only the shore tiles are valid to begin with; the middle of the lake is not.
    assert_eq!(w.apply(Command::Bend { ability, pos: at(6, 3) }, &defs), refused);
    assert_eq!(w.apply(Command::Bend { ability, pos: at(7, 3) }, &defs), Ok(()));
    w.apply(Command::CancelBend { pos: at(7, 3) }, &defs).unwrap();

    raise_at(&mut w, &defs, at(5, 3));
    assert_eq!(w.map[at(5, 3)].terrain, Terrain::Ford);
    // Now the next tile out is valid, and the one after is still not.
    assert_eq!(w.apply(Command::Bend { ability, pos: at(6, 3) }, &defs), Ok(()));
    w.apply(Command::CancelBend { pos: at(6, 3) }, &defs).unwrap();
    raise_at(&mut w, &defs, at(6, 3));
    raise_at(&mut w, &defs, at(7, 3));
    // A chain of fords joins the banks, so the cabbage can be fetched.
    let item = cabbage_item(&defs);
    w.apply(Command::DesignateStockpile { pos: at(2, 3) }, &defs).unwrap();
    w.apply(Command::DesignateHarvest { pos: at(10, 3) }, &defs).unwrap();
    run_until(&mut w, &defs, 8000, |w| w.stored_total(item) == 1);
}

/// Everything observable about the world's terrain, through its public API.
fn snapshot(w: &World, defs: &Defs) -> String {
    let units: Vec<_> = w.units.iter().map(|u| (u.id, u.pos, u.chi.to_bits(), u.task())).collect();
    let fords: Vec<_> =
        w.map.iter().filter(|(_, t)| t.terrain == Terrain::Ford).map(|(p, _)| p).collect();
    format!(
        "tick {} {units:?} fords {fords:?} {:?} stored {}",
        w.tick,
        w.bend_events(),
        w.stored_total(cabbage_item(defs)),
    )
}

fn session_world(defs: &Defs) -> World {
    World::with_map(11, field(defs, Terrain::Lake, 6), Nation::Earth, &[at(3, 3), at(4, 5)], defs)
}

fn play_session(defs: &Defs) -> World {
    let ability = raise(defs);
    let mut w = session_world(defs);
    let mut stage = 0;
    for t in 0..9000u64 {
        match t {
            0 => {
                w.apply(Command::DesignateStockpile { pos: at(2, 3) }, defs).unwrap();
                w.apply(Command::Bend { ability, pos: at(5, 3) }, defs).unwrap();
                w.apply(Command::Bend { ability, pos: at(5, 5) }, defs).unwrap();
            }
            30 => w.apply(Command::CancelBend { pos: at(5, 5) }, defs).unwrap(),
            _ => {}
        }
        // Each bend opens up the next: chain across, then fetch the far side's cabbage.
        let done = w.bend_events().iter().filter(|e| e.outcome == BendOutcome::Done).count();
        if done >= 1 && stage == 0 {
            stage = 1;
            w.apply(Command::Bend { ability, pos: at(6, 3) }, defs).unwrap();
        } else if done >= 2 && stage == 1 {
            stage = 2;
            w.apply(Command::DesignateHarvest { pos: at(10, 3) }, defs).unwrap();
        }
        w.tick(defs);
    }
    w
}

#[test]
fn a_session_with_raise_earth_replays_identically() {
    let defs = Defs::bundled();
    let original = play_session(&defs);
    let log = original.command_log().to_vec();
    assert!(log.len() >= 5);
    assert!(
        original.bend_events().len() >= 2 && original.stored_total(cabbage_item(&defs)) > 0,
        "the session should raise a causeway and cross it, or the test proves little: {}",
        snapshot(&original, &defs)
    );
    let mut replayed = session_world(&defs);
    replayed.replay(&log, original.tick, &defs);
    assert_eq!(snapshot(&replayed, &defs), snapshot(&original, &defs));
    assert_eq!(replayed.command_log(), original.command_log());
}

#[test]
fn replay_without_the_bend_commands_diverges() {
    let defs = Defs::bundled();
    let original = play_session(&defs);
    let mut log = original.command_log().to_vec();
    log.retain(|r| !matches!(r.command, Command::Bend { .. } | Command::CancelBend { .. }));
    let mut replayed = session_world(&defs);
    replayed.replay(&log, original.tick, &defs);
    assert_ne!(snapshot(&replayed, &defs), snapshot(&original, &defs));
}
