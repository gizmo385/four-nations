//! Seasons, climate bands, frost and cold (#295).

use super::*;
use crate::defs::{BiomeId, Terrain};
use crate::geom::Grid;
use crate::mapgen::Tile;

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

fn at(x: i32, y: i32) -> Pos {
    Pos::new(x, y)
}

fn world(defs: &Defs, map: TileMap, spawns: &[Pos]) -> World {
    World::with_map(1, map, Nation::Earth, spawns, defs)
}

fn normal(defs: &Defs) -> World {
    let mut w = world(defs, field(5, 5, 128), &[]);
    w.difficulty = Some("normal".into());
    w
}

fn season_ticks(defs: &Defs) -> u64 {
    (defs.seasons.calendar.season_minutes as f64 * 60.0 * TICKS_PER_SECOND as f64) as u64
}

// ---- season / year / season_left: pure functions of the tick ----

#[test]
fn a_world_starts_on_springs_first_tick() {
    let defs = Defs::bundled();
    let w = world(&defs, field(3, 3, 128), &[]);
    assert_eq!(w.season_index(&defs), 0);
    assert_eq!(w.season(&defs).id, "spring");
    assert_eq!(w.year(&defs), 1);
}

#[test]
fn season_year_and_season_left_follow_the_tick_with_no_mutation() {
    let defs = Defs::bundled();
    let mut w = world(&defs, field(3, 3, 128), &[]);
    let per = season_ticks(&defs);
    assert_eq!(w.season_left(&defs), per as f32 / TICKS_PER_SECOND as f32);
    w.tick = per - 1;
    assert_eq!(w.season(&defs).id, "spring");
    assert!(w.season_left(&defs) <= 1.0 / TICKS_PER_SECOND as f32 + 0.001);
    w.tick = per;
    assert_eq!(w.season(&defs).id, "summer");
    w.tick = 3 * per;
    assert_eq!(w.season(&defs).id, "winter");
    assert_eq!(w.year(&defs), 1);
    w.tick = 4 * per;
    assert_eq!(w.season(&defs).id, "spring");
    assert_eq!(w.year(&defs), 2);
    w.tick = 4 * per + 3 * per;
    assert_eq!(w.season(&defs).id, "winter");
    assert_eq!(w.year(&defs), 2);
}

// ---- climate_at ----

#[test]
fn climate_at_reads_the_tiles_mapgen_temperature() {
    let defs = Defs::bundled();
    let w = world(&defs, field(3, 3, 0), &[]);
    assert_eq!(w.climate_at(at(1, 1), &defs).unwrap().id, "polar");
    assert_eq!(w.climate_at(at(50, 50), &defs), None, "off the map");

    let warm = world(&defs, field(3, 3, 255), &[]);
    assert_eq!(warm.climate_at(at(1, 1), &defs).unwrap().id, "warm");
}

// ---- growth_rate: season and climate (work.rs's hook) ----

/// Puts a resting (not yet ripe) fruit tree on `pos` with `left` seconds to go.
fn plant_resting(w: &mut World, defs: &Defs, pos: Pos, left: f32) {
    w.map[pos].resource = defs.resource_id("fruit_tree");
    w.resting.insert(pos, left);
}

#[test]
fn a_polar_fruit_tree_does_not_regrow_in_winter_while_a_warm_one_does() {
    let defs = Defs::bundled();
    let mut map = field(5, 1, 128);
    let (polar, warm) = (at(0, 0), at(4, 0));
    map[polar].temperature = 0;
    map[warm].temperature = 255;
    let mut w = world(&defs, map, &[]);
    w.difficulty = Some("normal".into());
    // Land exactly on winter's first tick.
    w.tick = 3 * season_ticks(&defs);
    plant_resting(&mut w, &defs, polar, 5.0);
    plant_resting(&mut w, &defs, warm, 5.0);

    for _ in 0..300 {
        w.tick(&defs);
    }
    assert!(w.is_resting(polar), "polar winter_growth is 0: it never ripens in winter");
    assert!(!w.is_resting(warm), "warm land keeps growing in winter");
}

#[test]
fn with_seasons_off_climate_makes_no_difference_to_growth() {
    // Identity (#294/#295): no difficulty preset, or one with `seasons` off, plays exactly as
    // before this issue, whatever the season or the tile's climate.
    let defs = Defs::bundled();
    let mut map = field(3, 1, 128);
    map[at(0, 0)].temperature = 0; // polar
    let mut w = world(&defs, map, &[]);
    assert_eq!(w.difficulty, None);
    w.tick = 3 * season_ticks(&defs); // what would be winter, if seasons were on
    plant_resting(&mut w, &defs, at(0, 0), 5.0);
    for _ in 0..100 {
        w.tick(&defs);
    }
    assert!(!w.is_resting(at(0, 0)), "identity growth proceeds at the plain rate regardless");
}

// ---- frost ----

fn fruit_tree(defs: &Defs) -> Option<crate::defs::ResourceId> {
    defs.resource_id("fruit_tree")
}

#[test]
fn the_first_frost_sends_ripe_plants_on_frost_bands_back_to_resting() {
    let defs = Defs::bundled();
    let mut map = field(3, 1, 0); // polar: frost = true
    map[at(0, 0)].resource = fruit_tree(&defs);
    let mut w = world(&defs, map, &[]);
    w.difficulty = Some("normal".into());
    w.tick = 3 * season_ticks(&defs) - 1; // the tick before winter
    assert!(!w.is_resting(at(0, 0)), "ripe, not resting, going into winter");
    w.tick(&defs); // crosses into winter: the first frost
    assert!(w.is_resting(at(0, 0)), "frost struck the ripe plant");
    let regrow = defs.harvest(fruit_tree(&defs).unwrap()).unwrap().regrow.unwrap();
    assert_eq!(*w.resting.get(&at(0, 0)).unwrap(), regrow, "its full regrow time");
    assert!(
        w.season_events().iter().any(|e| matches!(e.kind, SeasonEventKind::Frost { count: 1 })),
        "{:?}",
        w.season_events()
    );
}

#[test]
fn frost_hardy_harvests_and_already_resting_plants_are_untouched_by_frost() {
    let defs = Defs::bundled();
    let mut map = field(3, 1, 0); // polar
    map[at(0, 0)].resource = defs.resource_id("fish_spot");
    let mut w = world(&defs, map, &[]);
    w.difficulty = Some("normal".into());
    w.tick = 3 * season_ticks(&defs) - 1;
    w.tick(&defs);
    assert!(!w.is_resting(at(0, 0)), "fishing is frost_hardy");
}

#[test]
fn frost_is_skipped_when_the_effective_severity_is_zero() {
    let mut defs = Defs::bundled();
    defs.difficulty.presets.iter_mut().find(|p| p.id == "normal").unwrap().winter_severity = 0.0;
    let mut map = field(3, 1, 0);
    map[at(0, 0)].resource = fruit_tree(&defs);
    let mut w = world(&defs, map, &[]);
    w.difficulty = Some("normal".into());
    w.tick = 3 * season_ticks(&defs) - 1;
    w.tick(&defs);
    assert!(!w.is_resting(at(0, 0)), "no frost at severity 0");
    assert!(!w.season_events().iter().any(|e| matches!(e.kind, SeasonEventKind::Frost { .. })));
}

#[test]
fn a_season_change_event_fires_at_every_boundary_and_a_warning_before_winter() {
    let defs = Defs::bundled();
    let mut w = normal(&defs);
    let per = season_ticks(&defs);
    let warn = (defs.seasons.calendar.winter_warning_minutes as f64
        * 60.0
        * TICKS_PER_SECOND as f64) as u64;
    w.tick = per - 1;
    w.tick(&defs); // into summer
    assert!(
        w.season_events()
            .iter()
            .any(|e| e.kind == SeasonEventKind::Changed { season: 1 } && e.tick == per)
    );
    w.tick = 3 * per - warn - 1;
    w.tick(&defs);
    assert!(w.season_events().iter().any(|e| e.kind == SeasonEventKind::WinterWarning));
}

// ---- cold ----

/// A unit at `pos` that won't act on its own (no job, no need low enough to chase), with full
/// needs, in winter, under Normal.
fn cold_test_world(defs: &Defs, temperature: u8, pos: Pos) -> World {
    let mut w = world(defs, field(5, 1, temperature), &[pos]);
    w.difficulty = Some("normal".into());
    w.tick = 3 * season_ticks(defs); // winter's first tick
    w.units[0].needs = Needs::full(&defs.needs);
    w.units[0].need_task = None;
    w.units[0].job_task = None;
    w
}

#[test]
fn a_unit_on_a_cold_band_tile_in_winter_loses_hunger_and_energy_faster_unless_heated() {
    let defs = Defs::bundled();
    let mut w = cold_test_world(&defs, 0, at(2, 0)); // polar: cold = true
    let mut baseline = cold_test_world(&defs, 255, at(2, 0)); // warm: not cold

    for _ in 0..600 {
        w.tick(&defs);
        baseline.tick(&defs);
    }
    assert!(w.units[0].needs.hunger < baseline.units[0].needs.hunger, "cold drains hunger faster");
    assert!(w.units[0].needs.energy < baseline.units[0].needs.energy, "and energy");
    assert_eq!(w.units[0].needs.thirst, baseline.units[0].needs.thirst, "never thirst");
}

/// `World::is_cold` agrees with `tick_needs`'s own check (the TUI's hover, #299): on, in
/// winter, on a cold band, unheated; off a heat source, off that band, outside winter, or with
/// seasons off altogether.
#[test]
fn is_cold_matches_the_tick_needs_check() {
    let defs = Defs::bundled();
    let pos = at(2, 0);
    let w = cold_test_world(&defs, 0, pos); // polar, winter, Normal
    assert!(w.is_cold(pos, &defs));

    let warm_band = cold_test_world(&defs, 255, pos); // warm band: never cold
    assert!(!warm_band.is_cold(pos, &defs));

    let mut not_winter = cold_test_world(&defs, 0, pos);
    not_winter.tick = 0; // spring
    assert!(!not_winter.is_cold(pos, &defs));

    let mut sandbox = cold_test_world(&defs, 0, pos);
    sandbox.difficulty = Some("sandbox".into()); // seasons off
    assert!(!sandbox.is_cold(pos, &defs));

    let mut heated = cold_test_world(&defs, 0, pos);
    heated.light_heat_source(HeatSource {
        pos,
        radius: 3,
        sleep_bonus: 1.0,
        seek_tiles: 10,
        until: HeatSource::FOREVER,
        size: [1, 1],
        cooks: true,
        ability: None,
    });
    assert!(!heated.is_cold(pos, &defs));
}

#[test]
fn a_campfire_in_range_negates_cold() {
    let defs = Defs::bundled();
    let pos = at(2, 0);
    let mut w = cold_test_world(&defs, 0, pos); // polar
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
    let mut unheated = cold_test_world(&defs, 0, pos);

    for _ in 0..600 {
        w.tick(&defs);
        unheated.tick(&defs);
    }
    assert!(w.units[0].needs.hunger > unheated.units[0].needs.hunger, "heat beats cold");
    assert!(w.units[0].needs.energy > unheated.units[0].needs.energy);
}

// ---- determinism ----

#[test]
fn two_runs_over_a_full_year_serialize_identically() {
    let defs = Defs::bundled();
    let run = || {
        let mut w = world(&defs, field(9, 9, 64), &[at(4, 4), at(5, 5)]);
        w.difficulty = Some("normal".into());
        let per = season_ticks(&defs);
        for _ in 0..4 * per {
            w.tick(&defs);
        }
        w.save(&defs).unwrap()
    };
    assert_eq!(run(), run());
}
