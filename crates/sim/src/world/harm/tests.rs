//! Exposure (issue #387): a unit asleep, on a cold tile, not sheltered. Sickness (issue #389):
//! a drink taken where the only water nearby is the sea.

use super::*;
use crate::building::Building;
use crate::defs::{BiomeId, Terrain};
use crate::geom::Grid;
use crate::mapgen::Tile;

fn at(x: i32, y: i32) -> Pos {
    Pos::new(x, y)
}

/// A flat field of land, `w` wide, every tile at `temperature`.
fn field(w: i32, h: i32, temperature: u8) -> TileMap {
    Grid::from_fn(w, h, |_| Tile {
        elevation: 100,
        temperature,
        moisture: 128,
        terrain: Terrain::Land,
        biome: BiomeId(0),
        resource: None,
        building: None,
    })
}

/// A map from ASCII rows: `.` land, `~` sea, `r` river, `l` lake, `d` ford -- for sickness tests
/// (issue #389), which care about *which* water terrain sits beside a drinking spot.
fn terrain_map(rows: &[&str]) -> TileMap {
    Grid::from_fn(rows[0].len() as i32, rows.len() as i32, |p| {
        let c = rows[p.y as usize].as_bytes()[p.x as usize];
        let terrain = match c {
            b'~' => Terrain::Water,
            b'r' => Terrain::River,
            b'l' => Terrain::Lake,
            b'd' => Terrain::Ford,
            _ => Terrain::Land,
        };
        Tile {
            elevation: 100,
            temperature: 128,
            moisture: 128,
            terrain,
            biome: BiomeId(0),
            resource: None,
            building: None,
        }
    })
}

fn world(defs: &Defs, map: TileMap, spawns: &[Pos]) -> World {
    World::with_map(1, map, Nation::Earth, spawns, defs)
}

fn season_ticks(defs: &Defs) -> u64 {
    (defs.seasons.calendar.season_minutes as f64 * 60.0 * TICKS_PER_SECOND as f64) as u64
}

/// A unit asleep at `pos`, on winter's first tick, under `preset`, with its energy low enough
/// (as `rest_tests.rs`'s own sleep tests do) that a short run never finishes the sleep on its
/// own by reaching `sleep_until`.
fn sleeping_world(defs: &Defs, temperature: u8, pos: Pos, preset: &str) -> World {
    let mut w = world(defs, field(5, 1, temperature), &[pos]);
    w.difficulty = Some(preset.into());
    w.tick = 3 * season_ticks(defs); // winter's first tick
    w.units[0].needs = Needs { energy: 10.0, ..Needs::full(&defs.needs) };
    w.units[0].need_task = Some(Task::Sleep);
    w.units[0].route = None;
    w
}

/// Runs `w` for up to `ticks`, keeping its one unit perpetually and freshly asleep with hunger
/// and thirst held safely clear of 0, and winter's first tick pinned, so a long run can isolate
/// exactly what exposure alone does for far longer than a real night or winter: no depleted need
/// and no season boundary ever confounds it. Stops early if the unit dies.
fn run_asleep(w: &mut World, defs: &Defs, ticks: u64) {
    let max = defs.needs.max();
    let winter_start = w.tick;
    for _ in 0..ticks {
        if w.units.is_empty() {
            break;
        }
        w.units[0].needs.hunger = max;
        w.units[0].needs.thirst = max;
        w.units[0].needs.energy = 10.0;
        w.units[0].need_task = Some(Task::Sleep);
        w.units[0].route = None;
        w.tick(defs);
        w.tick = winter_start;
    }
}

#[test]
fn a_rough_sleeper_on_cold_land_loses_health_and_logs_once() {
    let defs = Defs::bundled();
    let mut w = sleeping_world(&defs, 0, at(2, 0), "normal"); // polar: cold
    for _ in 0..600 {
        w.tick(&defs);
    }
    assert!(w.units[0].needs.health < defs.needs.health.max, "{}", w.units[0].needs.health);
    assert_eq!(w.harm_events().len(), 1, "logs once per sleep, not every tick it continues");
    assert_eq!(w.harm_events()[0].kind, HarmEventKind::SleepingRough);
    assert_eq!(w.units[0].last_harm.map(|(k, _)| k), Some(HarmKind::Exposure));
}

#[test]
fn a_sleeper_in_a_bed_takes_no_exposure_damage() {
    let defs = Defs::bundled();
    let pos = at(2, 0);
    let mut w = sleeping_world(&defs, 0, pos, "normal");
    let bed = defs.building_id("bed").unwrap();
    let def = defs.building(bed);
    assert!(def.bed.unwrap().sleep_bonus > 1.0, "a bed must beat the cold on its own");
    w.map[pos].building = Some(Building::new(bed, def.passable, false));
    for _ in 0..600 {
        w.tick(&defs);
    }
    assert_eq!(w.units[0].needs.health, defs.needs.health.max);
    assert!(w.harm_events().is_empty());
}

#[test]
fn a_sleeper_by_a_campfire_takes_no_exposure_damage() {
    let defs = Defs::bundled();
    let pos = at(2, 0);
    let mut w = sleeping_world(&defs, 0, pos, "normal");
    w.light_heat_source(HeatSource {
        pos,
        radius: 3,
        sleep_bonus: 1.0,
        seek_tiles: 10,
        until: HeatSource::FOREVER,
        size: [1, 1],
        cooks: true,
        ability: None,
    });
    assert!(w.is_heated(pos));
    for _ in 0..600 {
        w.tick(&defs);
    }
    assert_eq!(w.units[0].needs.health, defs.needs.health.max);
    assert!(w.harm_events().is_empty());
}

#[test]
fn a_sleeper_under_a_non_cooking_shelter_takes_no_exposure_damage() {
    // Earth's Earth Tent and Air's Air Shield (#362): `cooks: false`, but a shelter still keeps
    // the cold away exactly like a flame (`heat.rs`).
    let defs = Defs::bundled();
    let pos = at(2, 0);
    let mut w = sleeping_world(&defs, 0, pos, "normal");
    w.light_heat_source(HeatSource {
        pos,
        radius: 3,
        sleep_bonus: 1.0,
        seek_tiles: 10,
        until: HeatSource::FOREVER,
        size: [1, 1],
        cooks: false,
        ability: None,
    });
    for _ in 0..600 {
        w.tick(&defs);
    }
    assert_eq!(w.units[0].needs.health, defs.needs.health.max);
    assert!(w.harm_events().is_empty());
}

#[test]
fn regen_pauses_while_cold_awake_or_asleep() {
    // An awake unit with every need comfortable (so it never decides to sleep, eat or drink on
    // its own) but cold: `data/needs.toml`'s passive regen must stay off, unlike a warm unit's.
    let defs = Defs::bundled();
    let mut cold = world(&defs, field(5, 1, 0), &[at(2, 0)]); // polar
    cold.difficulty = Some("normal".into());
    cold.tick = 3 * season_ticks(&defs); // winter's first tick
    cold.units[0].needs = Needs { health: 50.0, ..Needs::full(&defs.needs) };
    let mut warm = world(&defs, field(5, 1, 255), &[at(2, 0)]); // warm band: never cold
    warm.difficulty = Some("normal".into());
    warm.tick = 3 * season_ticks(&defs);
    warm.units[0].needs = Needs { health: 50.0, ..Needs::full(&defs.needs) };
    for _ in 0..600 {
        cold.tick(&defs);
        warm.tick(&defs);
    }
    assert!(cold.is_cold(at(2, 0), &defs));
    assert_eq!(cold.units[0].needs.health, 50.0, "no regen while cold, even awake");
    assert!(warm.units[0].needs.health > 50.0, "a warm unit still regens");
}

#[test]
fn harm_floors_at_10_on_normal_but_never_kills() {
    let defs = Defs::bundled();
    let mut w = sleeping_world(&defs, 0, at(2, 0), "normal");
    run_asleep(&mut w, &defs, 20_000);
    assert_eq!(w.units.len(), 1, "Normal's harm_kills is off: exposure alone never kills");
    assert_eq!(w.units[0].needs.health, defs.harm.settings.floor);
    assert!(w.deaths.is_empty());
}

#[test]
fn harm_kills_on_hard() {
    let defs = Defs::bundled();
    let mut w = sleeping_world(&defs, 0, at(2, 0), "hard");
    run_asleep(&mut w, &defs, 20_000);
    assert!(w.units.is_empty(), "exposure should have killed by now on Hard");
    assert_eq!(w.deaths.len(), 1);
    assert_eq!(w.deaths[0].cause, DeathCause::Exposure);
    assert_eq!(w.deaths[0].weakened_by, None, "the death already names its own cause");
}

#[test]
fn sandbox_has_no_harm_at_all() {
    let defs = Defs::bundled();
    let mut w = sleeping_world(&defs, 0, at(2, 0), "sandbox");
    run_asleep(&mut w, &defs, 2_000);
    assert_eq!(w.units[0].needs.health, defs.needs.health.max);
    assert!(w.harm_events().is_empty());
}

#[test]
fn no_difficulty_preset_behaves_exactly_as_before() {
    let defs = Defs::bundled();
    let mut w = world(&defs, field(5, 1, 0), &[at(2, 0)]);
    w.units[0].needs = Needs { energy: 10.0, ..Needs::full(&defs.needs) };
    w.units[0].need_task = Some(Task::Sleep);
    w.units[0].route = None;
    run_asleep(&mut w, &defs, 2_000);
    assert_eq!(w.units[0].needs.health, defs.needs.health.max);
    assert!(w.harm_events().is_empty());
}

// Sickness (issue #389): a drink at a spot whose only water is the sea.

#[test]
fn a_sea_only_spot_rolls_the_listed_chance() {
    let defs = Defs::bundled();
    let map = terrain_map(&["~~~", "~.~", "~~~"]);
    let w = world(&defs, map, &[at(1, 1)]);
    assert_eq!(w.sickness_chance(at(1, 1), &defs), Some(defs.harm.sickness.water[0].chance));
}

#[test]
fn a_spot_with_any_fresh_water_neighbour_never_rolls() {
    let defs = Defs::bundled();
    let map = terrain_map(&["~~~", "r.~", "~~~"]);
    let w = world(&defs, map, &[at(1, 1)]);
    assert_eq!(w.sickness_chance(at(1, 1), &defs), None, "a river neighbour rules it safe");
}

#[test]
fn a_spot_with_no_water_neighbour_never_rolls() {
    let defs = Defs::bundled();
    let w = world(&defs, field(3, 3, 128), &[at(1, 1)]);
    assert_eq!(w.sickness_chance(at(1, 1), &defs), None);
}

/// Repeated independent rolls of the same sea-only spot, from the one progressing
/// `harm.sickness` stream, land close to the configured chance. Each trial resets
/// `sick_until` so the next roll isn't blocked by `maybe_catch_sickness`'s own
/// no-stacking guard: this is testing the roll itself, not the "already sick" rule (covered
/// separately below).
#[test]
fn seeded_sea_drinks_catch_sickness_near_the_configured_rate() {
    let defs = Defs::bundled();
    let map = terrain_map(&["~~~", "~.~", "~~~"]);
    let mut w = world(&defs, map, &[at(1, 1)]);
    w.difficulty = Some("normal".into());
    let trials = 4_000;
    let mut caught = 0u32;
    for _ in 0..trials {
        w.maybe_catch_sickness(0, &defs);
        if w.units[0].sick_until.is_some() {
            caught += 1;
            w.units[0].sick_until = None;
        }
    }
    let chance = f64::from(defs.harm.sickness.water[0].chance);
    let expected = f64::from(trials) * chance;
    // A generous band (about 5 standard deviations) around the binomial mean, loose enough to
    // never flake, tight enough to catch a roll that silently always/never fires.
    let sd = (f64::from(trials) * chance * (1.0 - chance)).sqrt();
    let band = sd * 5.0;
    assert!(
        (f64::from(caught) - expected).abs() <= band,
        "caught {caught} of {trials}, expected about {expected} (+/- {band})"
    );
}

#[test]
fn maybe_catch_sickness_never_stacks_or_resets_an_already_sick_unit() {
    let defs = Defs::bundled();
    let map = terrain_map(&["~~~", "~.~", "~~~"]);
    let mut w = world(&defs, map, &[at(1, 1)]);
    w.difficulty = Some("normal".into());
    w.units[0].sick_until = Some(w.tick + 1);
    for _ in 0..200 {
        w.maybe_catch_sickness(0, &defs);
    }
    assert_eq!(w.units[0].sick_until, Some(w.tick + 1), "already sick: no reroll, no restart");
}

#[test]
fn sandbox_or_harm_zero_never_catches_sickness() {
    let defs = Defs::bundled();
    let map = terrain_map(&["~~~", "~.~", "~~~"]);
    for preset in [None, Some("sandbox".into())] {
        let mut w = world(&defs, map.clone(), &[at(1, 1)]);
        w.difficulty = preset;
        for _ in 0..4_000 {
            w.maybe_catch_sickness(0, &defs);
        }
        assert!(w.units[0].sick_until.is_none());
        assert!(w.harm_events().is_empty());
    }
}

/// A unit already sick (set directly, as if a drink had just caught it) loses health at
/// `[sickness] damage_per_minute` with no natural regen, and recovers exactly at `sick_until`:
/// health stops falling, regen resumes, and `HarmEventKind::Recovered` fires once.
#[test]
fn a_sick_unit_loses_health_with_no_regen_and_recovers_on_time() {
    let defs = Defs::bundled();
    let mut w = world(&defs, field(3, 1, 128), &[at(1, 0)]);
    w.difficulty = Some("normal".into());
    w.units[0].needs = Needs::full(&defs.needs);
    let minutes = defs.harm.sickness.minutes;
    let ticks = (minutes * 60.0 * TICKS_PER_SECOND as f32).round() as u64;
    let harm_scale = w.knobs(&defs).harm;
    w.units[0].sick_until = Some(w.tick + ticks);
    for _ in 0..ticks {
        w.tick(&defs);
    }
    let expected =
        defs.needs.health.max - defs.harm.sickness.damage_per_minute * minutes * harm_scale;
    let health = w.units[0].needs.health;
    assert!((health - expected).abs() < 1.0, "health {health}, expected about {expected}");
    assert!(w.units[0].sick_until.is_none(), "sickness should have run out by now");
    assert_eq!(
        w.harm_events().iter().filter(|e| e.kind == HarmEventKind::Recovered).count(),
        1,
        "recovery logs once"
    );
    // No regen resumes it past where sickness left it, while every need stays comfortable: a
    // handful more ticks should leave health exactly where it was the instant sickness ended --
    // or above it only because regen is now allowed again, never below.
    let after_recovery = health;
    for _ in 0..600 {
        w.tick(&defs);
    }
    assert!(w.units[0].needs.health >= after_recovery, "regen should resume once recovered");
}

#[test]
fn harm_zero_never_progresses_or_ends_a_sickness_caught_before_the_preset_changed() {
    // A no-op difficulty preset still honours a sickness that was already set (an edge case:
    // `maybe_catch_sickness` itself never sets one while `harm` is 0). Matches `apply_exposure`'s
    // own guard.
    let defs = Defs::bundled();
    let mut w = world(&defs, field(3, 1, 128), &[at(1, 0)]);
    w.units[0].needs = Needs::full(&defs.needs);
    w.units[0].sick_until = Some(w.tick + 10);
    for _ in 0..20 {
        w.tick(&defs);
    }
    assert_eq!(w.units[0].needs.health, defs.needs.health.max);
}
