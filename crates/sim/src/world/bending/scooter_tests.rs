//! Air Scooter (#143): targets, the speed burst it leaves on the caster, its expiry and recast,
//! dying with its carrier, and replay.

use super::*;
use crate::geom::Grid;
use crate::mapgen::Tile;

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
    World::with_map(3, field(defs), Nation::Air, spawns, defs)
}

fn scooter(defs: &Defs) -> AbilityId {
    defs.ability_id("air_scooter").unwrap()
}

fn wind(defs: &Defs) -> AbilityId {
    defs.ability_id("wind_carry").unwrap()
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

/// A world where `air_scooter` is known, so Air Scooter can be used.
fn scooter_world(spawns: &[Pos], defs: &Defs) -> World {
    let mut w = world(spawns, defs);
    w.research.grant(defs.tech_id("air_scooter").unwrap());
    w.refresh_modifiers(defs);
    w
}

/// Unit `i`'s effective move speed right now: its nation's modifier times its scooter
/// multiplier, if it is riding one (what `walk`, `wander` and `Stride` all use).
fn speed_of(w: &World, defs: &Defs, i: usize) -> f32 {
    let id = w.units[i].id;
    w.modifiers_for(&w.units[i], defs).move_speed * w.scooter_multiplier(id)
}

/// Casts Air Scooter with unit 0 (the only unit with chi) on `pos` and runs until it is done.
fn cast_scooter(w: &mut World, defs: &Defs, pos: Pos) {
    for u in &mut w.units {
        u.chi = 0.0;
    }
    w.units[0].chi = defs.chi.max;
    w.apply(Command::Bend { ability: scooter(defs), pos }, defs).unwrap();
    let events = w.bend_events().count();
    run_until(w, defs, 1000, |w| w.bend_events().count() > events);
    assert_eq!(w.bend_events().next_back().unwrap().outcome, BendOutcome::Done);
}

#[test]
fn air_scooter_is_locked_until_air_scooter_is_known() {
    let defs = Defs::bundled();
    let tech = defs.tech_id("air_scooter").unwrap();
    let ability = scooter(&defs);
    assert_eq!(defs.ability(ability).unlocked_by, Some(tech));
    assert_eq!(defs.ability(ability).nation, Nation::Air);

    let mut w = world(&[at(10, 4)], &defs);
    assert_eq!(w.check_ability(ability, &defs), Err(BendError::Locked { tech }));
    assert_eq!(
        w.apply(Command::Bend { ability, pos: at(11, 4) }, &defs),
        Err(CommandError::Bend(BendError::Locked { tech }))
    );
    assert!(w.command_log().is_empty() && w.jobs().is_empty());
    // Wind Carry, the signature ability, needs no tech.
    assert_eq!(w.check_ability(wind(&defs), &defs), Ok(()));

    w.research.grant(tech);
    assert_eq!(w.check_ability(ability, &defs), Ok(()));
    assert_eq!(w.apply(Command::Bend { ability, pos: at(11, 4) }, &defs), Ok(()));
}

#[test]
fn air_scooter_belongs_to_air_alone() {
    let defs = Defs::bundled();
    let ability = scooter(&defs);
    for nation in [Nation::Earth, Nation::Water, Nation::Fire] {
        let w = World::with_map(3, field(&defs), nation, &[at(10, 4)], &defs);
        assert_eq!(w.check_ability(ability, &defs), Err(BendError::WrongNation), "{nation:?}");
    }
}

#[test]
fn air_scooter_needs_somewhere_open_to_stand() {
    let defs = Defs::bundled();
    let mut w = scooter_world(&[at(10, 4)], &defs);
    w.map[at(2, 2)].terrain = Terrain::Water;
    w.map[at(3, 2)].terrain = Terrain::Mountain;
    w.map[at(6, 2)].resource = defs.resource_id("cabbage");
    w.map_changed();
    let ability = scooter(&defs);
    let bend = |w: &mut World, pos| w.apply(Command::Bend { ability, pos }, &defs);
    let refused = |why| Err(CommandError::Bend(BendError::InvalidTarget(why)));
    assert_eq!(bend(&mut w, at(8, 4)), Ok(()));
    for p in [at(2, 2), at(3, 2)] {
        assert_eq!(bend(&mut w, p), refused("needs open ground to stand on"), "{p:?}");
    }
    assert_eq!(bend(&mut w, at(6, 2)), refused("something is in the way"));
    assert_eq!(bend(&mut w, at(-1, 0)), refused("off the map"));
}

#[test]
fn riding_multiplies_move_speed_for_the_move_speed_scaled_time() {
    let defs = Defs::bundled();
    let mut w = scooter_world(&[at(10, 4), at(1, 1)], &defs);
    let base = w.modifiers().move_speed;
    assert_eq!(speed_of(&w, &defs, 0), base);

    cast_scooter(&mut w, &defs, at(12, 4));
    let def = defs.ability(scooter(&defs));
    assert!((w.units[0].chi - (defs.chi.max - def.chi)).abs() < 0.5);

    let id = w.units[0].id;
    let ride = w.rides[&id];
    assert_eq!(ride.multiplier, 2.5);
    // 2 minutes x Air's move_speed, which air_scooter has raised past the base 1.6.
    assert!(base > 1.6, "{base}");
    let left = (ride.until - w.tick) as f32 / TICKS_PER_SECOND as f32;
    assert!((left - 120.0 * base).abs() < 0.2, "{left}");

    // The rider moves faster; the other unit, which never cast it, doesn't.
    assert_eq!(speed_of(&w, &defs, 0), base * ride.multiplier);
    assert_eq!(speed_of(&w, &defs, 1), base);
}

#[test]
fn the_ride_fades_at_its_end_tick_and_a_recast_resets_it() {
    let defs = Defs::bundled();
    let mut w = scooter_world(&[at(10, 4)], &defs);
    cast_scooter(&mut w, &defs, at(10, 4));
    let id = w.units[0].id;
    let full = w.rides[&id].until - w.tick;

    // Half spent, then cast again: the timer is full once more, and it is still one ride.
    let half = full / 2;
    run_until(&mut w, &defs, 20_000, |w| w.rides[&id].until - w.tick <= half);
    assert!(w.rides[&id].until - w.tick < full / 2 + 1);
    cast_scooter(&mut w, &defs, at(10, 4));
    assert_eq!(w.rides.len(), 1);
    let renewed = w.rides[&id].until - w.tick;
    assert!(renewed.abs_diff(full) <= 1, "{renewed} vs {full}");

    // It lasts until its end tick and is gone on it.
    let until = w.rides[&id].until;
    run_until(&mut w, &defs, 20_000, |w| w.tick == until - 1);
    assert!(speed_of(&w, &defs, 0) > w.modifiers().move_speed);
    w.tick(&defs);
    assert!(w.rides.is_empty(), "the spent ride is dropped");
    assert_eq!(speed_of(&w, &defs, 0), w.modifiers().move_speed);
}

#[test]
fn the_ride_dies_with_its_carrier() {
    let defs = Defs::bundled();
    let mut w = scooter_world(&[at(10, 4), at(2, 2)], &defs);
    cast_scooter(&mut w, &defs, at(10, 4));
    assert_eq!(w.rides.len(), 1);
    w.units[0].needs.hunger = 0.0;
    w.units[0].needs.health = 0.001;
    w.tick(&defs);
    assert_eq!(w.units.len(), 1);
    assert!(w.rides.is_empty());
}

#[test]
fn a_ride_survives_a_save_and_old_saves_without_one_still_load() {
    let defs = Defs::bundled();
    let mut w = scooter_world(&[at(10, 4)], &defs);
    cast_scooter(&mut w, &defs, at(10, 4));
    let text = w.save(&defs).unwrap();
    let loaded = World::load(&text, &defs).unwrap();
    assert_eq!(loaded.rides, w.rides);
    // A save from before #143 has no `rides` field at all.
    let (head, body) = text.split_once('\n').unwrap();
    let old = World::load(&format!("{head}\n{}", strip_rides(body)), &defs).unwrap();
    assert!(old.rides.is_empty());
}

/// `body` with the `rides:{...}` field removed (RON struct fields are `name:value`).
fn strip_rides(body: &str) -> String {
    let start = body.find("rides:{").expect("the save lists rides");
    let mut depth = 0;
    let mut end = start;
    for (i, c) in body[start..].char_indices() {
        match c {
            '{' => depth += 1,
            '}' => {
                depth -= 1;
                if depth == 0 {
                    end = start + i + 1;
                    break;
                }
            }
            _ => {}
        }
    }
    let rest = body[end..].strip_prefix(',').unwrap_or(&body[end..]);
    format!("{}{}", &body[..start], rest)
}

fn scooter_snapshot(w: &World) -> String {
    let units: Vec<_> = w.units.iter().map(|u| (u.id, u.pos, u.chi.to_bits())).collect();
    format!(
        "tick {} {units:?} rides {:?} events {:?}",
        w.tick,
        w.rides,
        w.bend_events().collect::<Vec<_>>()
    )
}

fn play_scooter_session(defs: &Defs) -> World {
    let mut w = scooter_world(&[at(9, 4), at(3, 6)], defs);
    let ability = scooter(defs);
    for t in 0..8_000u64 {
        match t {
            0 => w.apply(Command::Bend { ability, pos: at(11, 4) }, defs).unwrap(),
            4000 => w.apply(Command::Bend { ability, pos: at(6, 6) }, defs).unwrap(),
            _ => {}
        }
        w.tick(defs);
    }
    w
}

#[test]
fn a_session_with_air_scooter_replays_identically() {
    let defs = Defs::bundled();
    let original = play_scooter_session(&defs);
    let log = original.command_log().to_vec();
    assert!(
        original.bend_events().count() >= 2,
        "the session should cast twice: {}",
        scooter_snapshot(&original)
    );
    let mut replayed = scooter_world(&[at(9, 4), at(3, 6)], &defs);
    replayed.replay(&log, original.tick, &defs);
    assert_eq!(scooter_snapshot(&replayed), scooter_snapshot(&original));
}
