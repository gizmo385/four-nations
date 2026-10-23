//! Hearthflame (#70): targets, burning time, restoking, cooked meals, warm and sought-out
//! sleep, and replay.

use super::*;
use crate::geom::Grid;
use crate::jobs::{ItemId, Stack};
use crate::mapgen::Tile;
use crate::needs::Needs;

fn at(x: i32, y: i32) -> Pos {
    Pos::new(x, y)
}

fn biome(defs: &Defs, id: &str) -> BiomeId {
    BiomeId(defs.biomes.iter().position(|b| b.id == id).unwrap() as u8)
}

/// A 21x9 plains field.
fn field(defs: &Defs) -> TileMap {
    let plains = biome(defs, "plains");
    Grid::from_fn(21, 9, |_| Tile {
        elevation: 100,
        temperature: 128,
        moisture: 128,
        terrain: Terrain::Land,
        biome: plains,
        resource: None,
        building: None,
    })
}

fn world(spawns: &[Pos], defs: &Defs) -> World {
    World::with_map(3, field(defs), Nation::Fire, spawns, defs)
}

fn hearth(defs: &Defs) -> AbilityId {
    defs.ability_id("hearthflame").unwrap()
}

fn cabbage(defs: &Defs) -> ItemId {
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

/// A flame lit directly, with the Hearthflame numbers but any reach and time.
fn lit(pos: Pos, until: u64, seek_tiles: u32) -> HeatSource {
    HeatSource { pos, radius: 3, sleep_bonus: 1.5, seek_tiles, until }
}

/// Kindles `pos` with the first unit and runs until the bend is done.
fn kindle(w: &mut World, defs: &Defs, pos: Pos) {
    w.units[0].chi = defs.chi.max;
    w.apply(Command::Bend { ability: hearth(defs), pos }, defs).unwrap();
    let events = w.bend_events().len();
    run_until(w, defs, 1000, |w| w.bend_events().len() > events);
    assert_eq!(w.bend_events().last().unwrap().outcome, BendOutcome::Done);
}

#[test]
fn a_flame_needs_an_empty_open_tile() {
    let defs = Defs::bundled();
    let mut w = world(&[at(10, 4)], &defs);
    w.map[at(2, 2)].terrain = Terrain::Water;
    w.map[at(3, 2)].terrain = Terrain::Mountain;
    w.map[at(4, 2)].terrain = Terrain::River;
    w.map[at(5, 2)].terrain = Terrain::Coast;
    w.map[at(6, 2)].resource = defs.resource_id("cabbage");
    w.map_changed();
    let ability = hearth(&defs);
    let bend = |w: &mut World, pos| w.apply(Command::Bend { ability, pos }, &defs);
    let refused = |why| Err(CommandError::Bend(BendError::InvalidTarget(why)));
    assert_eq!(bend(&mut w, at(8, 4)), Ok(()));
    assert_eq!(bend(&mut w, at(5, 2)), Ok(()));
    for p in [at(2, 2), at(3, 2), at(4, 2)] {
        assert_eq!(bend(&mut w, p), refused("needs open ground to kindle"), "{p:?}");
    }
    assert_eq!(bend(&mut w, at(6, 2)), refused("something is in the way"));
    assert_eq!(bend(&mut w, at(-1, 0)), refused("off the map"));
    assert_eq!(bend(&mut w, at(8, 4)), Err(CommandError::Bend(BendError::AlreadyDesignated)));
}

#[test]
fn a_flame_burns_for_the_heating_scaled_time_then_goes_out() {
    let defs = Defs::bundled();
    let mut w = world(&[at(10, 4)], &defs);
    assert!(w.flames().is_empty());
    kindle(&mut w, &defs, at(11, 4));
    let flames = w.flames();
    assert_eq!(flames.len(), 1);
    assert_eq!((flames[0].pos, flames[0].radius), (at(11, 4), 3));
    // 8 minutes x Fire's heating of 2.5.
    let factor = w.modifiers().heating;
    assert!((factor - 2.5).abs() < 1e-6);
    assert!((flames[0].seconds_left - 480.0 * factor).abs() < 0.2, "{}", flames[0].seconds_left);
    assert!(w.is_lit(at(11, 4)) && !w.is_lit(at(10, 4)));

    // It burns until its end tick and is gone on it.
    let until = w.heat[&at(11, 4)].until;
    run_until(&mut w, &defs, 20_000, |w| w.tick == until - 1);
    assert_eq!(w.flames().len(), 1);
    assert!(w.is_heated(at(11, 4)));
    w.tick(&defs);
    assert!(w.flames().is_empty());
    assert!(!w.is_heated(at(11, 4)));
    assert!(w.heat.is_empty(), "the spent flame is dropped");
}

#[test]
fn a_flame_does_not_block_movement() {
    let defs = Defs::bundled();
    let mut w = world(&[at(10, 4)], &defs);
    kindle(&mut w, &defs, at(11, 4));
    assert!(w.walkable(at(11, 4)));
    assert_eq!(w.map[at(11, 4)].resource, None);
    let flood = w.flood_from(0);
    assert!(flood.cost(at(11, 4)).is_some());
    assert!(flood.cost(at(12, 4)).is_some());
}

#[test]
fn bending_a_lit_flame_restokes_it() {
    let defs = Defs::bundled();
    let mut w = world(&[at(10, 4)], &defs);
    kindle(&mut w, &defs, at(11, 4));
    let full = w.flames()[0].seconds_left;
    // Half burnt.
    let half = (full * TICKS_PER_SECOND as f32 / 2.0) as u64;
    run_until(&mut w, &defs, 20_000, |w| w.heat[&at(11, 4)].until - w.tick <= half);
    assert!(w.flames()[0].seconds_left < full * 0.51);
    // The same bend again: the timer is full once more, and it is still one flame.
    w.units[0].needs = Needs::full(&defs.needs);
    w.units[0].chi = defs.ability(hearth(&defs)).chi;
    kindle(&mut w, &defs, at(11, 4));
    let flames = w.flames();
    assert_eq!(flames.len(), 1);
    assert!((flames[0].seconds_left - full).abs() < 1.0, "{}", flames[0].seconds_left);
}

#[test]
fn a_meal_within_a_lit_flame_is_cooked_and_elsewhere_it_is_raw() {
    let defs = Defs::bundled();
    let food = defs.foods.iter().find(|f| f.id == "cabbage").unwrap();
    let (raw, cooked) = (food.hunger, food.cooked_hunger.unwrap());
    assert!(cooked > raw);
    // Two hungry units each hold a cabbage: one stands in the flame's reach, one far from it.
    let mut w = world(&[at(11, 4), at(2, 4)], &defs);
    w.heat.insert(at(10, 4), lit(at(10, 4), 100_000, 0));
    let before = defs.needs.hunger.low - 5.0;
    for u in &mut w.units {
        u.needs.hunger = before;
        u.carrying = Some(Stack { item: cabbage(&defs), count: 1 });
    }
    run_until(&mut w, &defs, 200, |w| w.units.iter().all(|u| u.carrying.is_none()));
    // A few ticks of decay are in the figure, so compare with a little room.
    let gained = |i: usize| w.units[i].needs.hunger - before;
    assert!((gained(0) - cooked).abs() < 0.5, "near the flame: {}", gained(0));
    assert!((gained(1) - raw).abs() < 0.5, "away from it: {}", gained(1));
    assert!(gained(0) > gained(1) + 10.0);
}

#[test]
fn an_expired_flame_no_longer_cooks() {
    let defs = Defs::bundled();
    let mut w = world(&[at(11, 4)], &defs);
    w.heat.insert(at(10, 4), lit(at(10, 4), 1, 0));
    run_until(&mut w, &defs, 5, |w| w.flames().is_empty());
    w.units[0].needs.hunger = defs.needs.hunger.low - 5.0;
    w.units[0].carrying = Some(Stack { item: cabbage(&defs), count: 1 });
    let before = w.units[0].needs.hunger;
    run_until(&mut w, &defs, 200, |w| w.units[0].carrying.is_none());
    assert!((w.units[0].needs.hunger - before - 30.0).abs() < 0.5);
}

/// Energy regained over a minute of sleep for a unit at `pos`.
fn minute_of_sleep(w: &mut World, defs: &Defs, pos: Pos) -> f32 {
    w.units[0].pos = pos;
    w.units[0].need_task = Some(Task::Sleep);
    w.units[0].route = None;
    w.units[0].needs = Needs { energy: 10.0, ..Needs::full(&defs.needs) };
    for _ in 0..600 {
        w.tick(defs);
    }
    assert!(w.units[0].is_sleeping());
    w.units[0].needs.energy - 10.0
}

#[test]
fn sleep_in_a_flames_reach_is_faster_and_flames_do_not_stack() {
    let defs = Defs::bundled();
    let base = defs.needs.general.sleep_energy_per_minute;
    let mut w = world(&[at(2, 4)], &defs);
    assert!((minute_of_sleep(&mut w, &defs, at(2, 4)) - base).abs() < 0.01);

    w.heat.insert(at(10, 4), lit(at(10, 4), 100_000, 0));
    let warm = minute_of_sleep(&mut w, &defs, at(13, 4));
    assert!((warm - base * 1.5).abs() < 0.01, "{warm}");
    // One tile past the radius is back to the base rate.
    assert!((minute_of_sleep(&mut w, &defs, at(14, 4)) - base).abs() < 0.01);

    // A second flame over the same spot adds nothing.
    w.heat.insert(at(12, 4), lit(at(12, 4), 100_000, 0));
    let twice = minute_of_sleep(&mut w, &defs, at(11, 4));
    assert!((twice - warm).abs() < 0.001, "{twice} vs {warm}");
}

#[test]
fn a_unit_about_to_sleep_walks_into_a_flame_within_seek_range() {
    let defs = Defs::bundled();
    let mut w = world(&[at(1, 4)], &defs);
    w.heat.insert(at(10, 4), lit(at(10, 4), 100_000, 20));
    w.units[0].needs.energy = defs.needs.energy.low - 1.0;
    run_until(&mut w, &defs, 400, |w| w.units[0].is_sleeping());
    let p = w.units[0].pos;
    assert!(w.is_heated(p), "{p:?}");
    // The nearest warm tile, not the flame itself.
    assert_eq!(p, at(7, 4));
    assert_eq!(w.units[0].task(), Some(Task::Sleep));
    // While it walked it was awake: energy only fell until now.
    assert!(w.units[0].needs.energy < defs.needs.energy.low);
}

#[test]
fn a_unit_sleeps_where_it_is_if_the_flame_is_out_of_seek_range() {
    let defs = Defs::bundled();
    let mut w = world(&[at(1, 4)], &defs);
    // Seven tiles of path to the nearest warm tile, and the flame only calls from five.
    w.heat.insert(at(11, 4), lit(at(11, 4), 100_000, 5));
    w.units[0].needs.energy = defs.needs.energy.low - 1.0;
    run_until(&mut w, &defs, 100, |w| w.units[0].is_sleeping());
    assert_eq!(w.units[0].pos, at(1, 4));
    // Already warm: it doesn't move either.
    let mut w = world(&[at(9, 4)], &defs);
    w.heat.insert(at(11, 4), lit(at(11, 4), 100_000, 5));
    w.units[0].needs.energy = defs.needs.energy.low - 1.0;
    run_until(&mut w, &defs, 100, |w| w.units[0].is_sleeping());
    assert_eq!(w.units[0].pos, at(9, 4));
}

#[test]
fn nobody_seeks_a_flame_it_cannot_reach() {
    let defs = Defs::bundled();
    let mut w = world(&[at(1, 4)], &defs);
    for y in 0..9 {
        w.map[at(5, y)].terrain = Terrain::Water;
    }
    w.map_changed();
    w.heat.insert(at(10, 4), lit(at(10, 4), 100_000, 30));
    w.units[0].needs.energy = defs.needs.energy.low - 1.0;
    run_until(&mut w, &defs, 100, |w| w.units[0].is_sleeping());
    assert_eq!(w.units[0].pos, at(1, 4));
}

/// Everything observable about the flames, through the public API.
fn snapshot(w: &World) -> String {
    let units: Vec<_> = w
        .units
        .iter()
        .map(|u| (u.id, u.pos, u.chi.to_bits(), u.needs.hunger.to_bits(), u.needs.energy.to_bits()))
        .collect();
    format!(
        "tick {} {units:?} flames {:?} {:?} heated {}",
        w.tick,
        w.flames(),
        w.bend_events(),
        w.is_heated(at(11, 4)),
    )
}

/// Two Fire units, both about to get hungry and tired with a cabbage in hand, so that the flame
/// lit in the session is there when they eat and sleep.
fn session_world(defs: &Defs) -> World {
    let mut w = World::with_map(5, field(defs), Nation::Fire, &[at(9, 4), at(3, 6)], defs);
    for u in &mut w.units {
        u.needs.hunger = defs.needs.hunger.low + 3.0;
        u.needs.energy = defs.needs.energy.low + 3.0;
        u.carrying = Some(Stack { item: cabbage(defs), count: 1 });
    }
    w
}

fn play_session(defs: &Defs) -> World {
    let ability = hearth(defs);
    let mut w = session_world(defs);
    for t in 0..12_000u64 {
        match t {
            0 => {
                w.apply(Command::Bend { ability, pos: at(11, 4) }, defs).unwrap();
                w.apply(Command::Bend { ability, pos: at(15, 7) }, defs).unwrap();
            }
            30 => w.apply(Command::CancelBend { pos: at(15, 7) }, defs).unwrap(),
            // Restoke well before it would go out.
            6000 => w.apply(Command::Bend { ability, pos: at(11, 4) }, defs).unwrap(),
            _ => {}
        }
        w.tick(defs);
    }
    w
}

#[test]
fn a_session_with_hearthflame_replays_identically() {
    let defs = Defs::bundled();
    let original = play_session(&defs);
    let log = original.command_log().to_vec();
    assert!(log.len() >= 4);
    assert!(
        original.bend_events().len() >= 2 && !original.flames().is_empty(),
        "the session should kindle and restoke a flame, or the test proves little: {}",
        snapshot(&original)
    );
    let mut replayed = session_world(&defs);
    replayed.replay(&log, original.tick, &defs);
    assert_eq!(snapshot(&replayed), snapshot(&original));
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
    assert_ne!(snapshot(&replayed), snapshot(&original));
}
