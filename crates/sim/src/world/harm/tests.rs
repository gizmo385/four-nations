//! Exposure (issue #387): a unit asleep, on a cold tile, not sheltered.

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
