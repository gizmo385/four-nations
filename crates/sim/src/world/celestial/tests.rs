//! Celestial days (#760): the day, the rise, the regen, the events and the Almanac lines.

use super::*;
use crate::defs::{BiomeId, Terrain};
use crate::geom::Grid;
use crate::mapgen::Tile;

fn field() -> TileMap {
    Grid::from_fn(12, 8, |_| Tile {
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

/// A Water settlement of four: two Water residents, a Fire resident and the Avatar.
fn world(defs: &Defs) -> World {
    let mut w =
        World::with_map(1, field(), Nation::Water, &[at(1, 1), at(2, 1), at(3, 1), at(4, 1)], defs);
    w.units[2].nation = Some(Nation::Fire);
    w.units[3].guest = Some(Guest::Avatar);
    w
}

fn known(w: &mut World, defs: &Defs) {
    w.research.grant(defs.tech_id("star_charts").unwrap());
}

fn mins(m: f32) -> u64 {
    (m * 60.0) as u64 * TICKS_PER_SECOND as u64
}

fn season_start(defs: &Defs, season: &str) -> u64 {
    let i = defs.seasons.seasons.iter().position(|s| s.id == season).unwrap();
    i as u64 * defs.seasons.season_ticks()
}

/// The first tick of the harvest moon (Water, autumn minute 5) in year 1.
fn moon(defs: &Defs) -> u64 {
    season_start(defs, "autumn") + mins(5.0)
}

fn drain_chi(w: &mut World) {
    for u in &mut w.units {
        u.chi = 0.0;
    }
}

/// Sets the tick so the next `tick` is `t`.
fn arrive_at(w: &mut World, t: u64) {
    w.tick = t - 1;
}

fn rises(w: &World) -> Vec<(CelestialDayId, u32)> {
    w.events()
        .iter()
        .filter_map(|e| match e {
            Event::Season(SeasonEvent {
                kind: SeasonEventKind::CelestialRise { day, units },
                ..
            }) => Some((*day, *units)),
            _ => None,
        })
        .collect()
}

fn warnings(w: &World) -> Vec<(u64, CelestialDayId)> {
    w.events()
        .iter()
        .filter_map(|e| match e {
            Event::Season(SeasonEvent {
                tick,
                kind: SeasonEventKind::CelestialWarning { day },
            }) => Some((*tick, *day)),
            _ => None,
        })
        .collect()
}

fn day_id(defs: &Defs, id: &str) -> CelestialDayId {
    CelestialDayId(defs.seasons.celestial.days.iter().position(|d| d.id == id).unwrap() as u8)
}

// ---- data ----

#[test]
fn star_charts_is_data() {
    let defs = Defs::bundled();
    let tech = defs.tech_id("star_charts").unwrap();
    assert_eq!(defs.feature_tech(Feature::CelestialDays), Some(tech));
    let def = defs.tech(tech);
    assert_eq!(def.cost, 500);
    assert_eq!(def.nation, None, "shared");
    let prereqs: Vec<&str> = def.prereqs.iter().map(|&p| defs.tech(p).id.as_str()).collect();
    assert_eq!(prereqs, ["calendar", "cartography"]);
}

#[test]
fn each_nation_has_one_day_in_its_own_season() {
    let defs = Defs::bundled();
    let c = &defs.seasons.celestial;
    assert_eq!(c.warning_minutes, 2.0);
    let of = |n: Nation| {
        let d: Vec<_> = c.days.iter().filter(|d| d.nation == n).collect();
        assert_eq!(d.len(), 1);
        (d[0].id.as_str(), defs.seasons.seasons[d[0].season].id.as_str())
    };
    assert_eq!(of(Nation::Air), ("equinox_winds", "spring"));
    assert_eq!(of(Nation::Fire), ("summer_solstice", "summer"));
    assert_eq!(of(Nation::Water), ("harvest_moon", "autumn"));
    assert_eq!(of(Nation::Earth), ("deep_earth_day", "winter"));
    for d in &c.days {
        assert_eq!((d.at_minutes, d.minutes, d.refill, d.chi_regen), (5.0, 5.0, 1.0, 2.0));
    }
}

const SEASONS: &str = include_str!("../../../../../data/seasons.toml");

#[test]
fn celestial_validation() {
    let defs = Defs::bundled();
    let bad = |from: &str, to: &str| {
        assert!(SEASONS.contains(from), "{from}");
        defs.clone().with_seasons(&SEASONS.replacen(from, to, 1)).unwrap_err().0
    };
    assert!(bad("season = \"spring\"", "season = \"monsoon\"").contains("unknown season"));
    assert!(bad("nation = \"fire\"", "nation = \"air\"").contains("already has a day"));
    assert!(bad("at_minutes = 5.0", "at_minutes = 11.0").contains("fit in its season"));
    assert!(bad("\nminutes = 5.0", "\nminutes = 0.5").contains("minutes must be 1 to"));
    assert!(bad("\nminutes = 5.0", "\nminutes = 16.0").contains("minutes must be 1 to"));
    assert!(bad("refill = 1.0", "refill = 1.5").contains("refill must be 0 to 1"));
    assert!(bad("refill = 1.0", "refill = -0.1").contains("refill must be 0 to 1"));
    assert!(bad("chi_regen = 2.0", "chi_regen = 0.5").contains("chi_regen must be 1 to 4"));
    assert!(bad("chi_regen = 2.0", "chi_regen = 5.0").contains("chi_regen must be 1 to 4"));
    assert!(bad("warning_minutes = 2.0", "warning_minutes = 0.0").contains("warning_minutes"));
    assert!(bad("id = \"harvest_moon\"", "id = \"equinox_winds\"").contains("duplicate id"));
    assert!(bad("[celestial]\n", "[celestia]\n").contains("celestia"));
}

// ---- the day is a function of the tick ----

#[test]
fn the_day_comes_from_the_tick_alone_and_only_once_known() {
    let defs = Defs::bundled();
    let mut w = world(&defs);
    w.tick = moon(&defs) + 10;
    assert_eq!(w.celestial_day(&defs), None, "unknown before star_charts");
    known(&mut w, &defs);
    assert_eq!(w.celestial_day(&defs).unwrap().id, "harvest_moon");
    w.tick = moon(&defs) - 1;
    assert_eq!(w.celestial_day(&defs), None);
    w.tick = moon(&defs) + mins(5.0) - 1;
    assert_eq!(w.celestial_day(&defs).unwrap().id, "harvest_moon");
    w.tick = moon(&defs) + mins(5.0);
    assert_eq!(w.celestial_day(&defs), None);
    // A year later it comes again.
    w.tick = moon(&defs) + defs.seasons.year_ticks() + 1;
    assert_eq!(w.celestial_day(&defs).unwrap().id, "harvest_moon");
    // Each other nation's day is in its own season.
    w.tick = season_start(&defs, "spring") + mins(6.0);
    assert_eq!(w.celestial_day(&defs).unwrap().id, "equinox_winds");
    w.tick = season_start(&defs, "winter") + mins(7.0);
    assert_eq!(w.celestial_day(&defs).unwrap().id, "deep_earth_day");
}

#[test]
fn next_celestial_counts_to_the_nations_next_day() {
    let defs = Defs::bundled();
    let mut w = world(&defs);
    known(&mut w, &defs);
    let year = defs.seasons.year_ticks();
    w.tick = moon(&defs) - 100;
    let water = day_id(&defs, "harvest_moon");
    assert_eq!(w.next_celestial(Nation::Water, &defs), Some((water, 100)));
    // On the day itself, the next is a year off.
    w.tick = moon(&defs);
    assert_eq!(w.next_celestial(Nation::Water, &defs), Some((water, year)));
    w.tick = moon(&defs) + 1;
    assert_eq!(w.next_celestial(Nation::Water, &defs), Some((water, year - 1)));
    let mut w2 = world(&defs);
    w2.tick = moon(&defs) - 100;
    assert_eq!(w2.next_celestial(Nation::Water, &defs), None, "unknown");
}

// ---- the rise ----

#[test]
fn the_rise_fills_residents_of_the_nation_but_not_the_avatar_or_others() {
    let defs = Defs::bundled();
    let mut w = world(&defs);
    known(&mut w, &defs);
    drain_chi(&mut w);
    let avatar_before = w.units[3].chi;
    arrive_at(&mut w, moon(&defs));
    w.tick(&defs);
    assert_eq!(w.tick, moon(&defs));
    let max = defs.chi.max;
    assert!(w.units[0].chi >= max - 0.1, "{}", w.units[0].chi);
    assert!(w.units[1].chi >= max - 0.1);
    assert!(w.units[2].chi < 1.0, "a Fire resident isn't Water's: {}", w.units[2].chi);
    assert!(w.units[3].chi <= avatar_before + 0.1, "the Avatar's meter is his own");
    assert_eq!(rises(&w), vec![(day_id(&defs, "harvest_moon"), 2)]);
}

#[test]
fn a_unit_already_above_the_refill_keeps_its_chi() {
    let defs =
        Defs::bundled().with_seasons(&SEASONS.replacen("refill = 1.0", "refill = 0.5", 3)).unwrap();
    // The first three days were changed; the Water day is the third in the file.
    assert_eq!(defs.seasons.celestial.days[2].refill, 0.5);
    let mut w = world(&defs);
    known(&mut w, &defs);
    w.units[0].chi = 0.0;
    w.units[1].chi = 80.0;
    arrive_at(&mut w, moon(&defs));
    w.tick(&defs);
    let half = defs.chi.max * 0.5;
    assert!((w.units[0].chi - half).abs() < 0.1, "{}", w.units[0].chi);
    assert!(w.units[1].chi >= 80.0, "kept: {}", w.units[1].chi);
}

#[test]
fn the_rise_fires_once_and_only_while_known_and_only_with_a_resident() {
    let defs = Defs::bundled();
    let mut w = world(&defs);
    arrive_at(&mut w, moon(&defs));
    drain_chi(&mut w);
    w.tick(&defs);
    assert!(rises(&w).is_empty(), "no star_charts");
    assert!(w.units[0].chi < 1.0);

    let mut w = world(&defs);
    known(&mut w, &defs);
    arrive_at(&mut w, moon(&defs));
    for _ in 0..50 {
        w.tick(&defs);
    }
    assert_eq!(rises(&w).len(), 1);

    // Earth's day: no Earth resident, so no rise.
    let mut w = world(&defs);
    known(&mut w, &defs);
    arrive_at(&mut w, season_start(&defs, "winter") + mins(5.0));
    w.tick(&defs);
    assert!(rises(&w).is_empty());
    // Fire has one resident.
    let mut w = world(&defs);
    known(&mut w, &defs);
    arrive_at(&mut w, season_start(&defs, "summer") + mins(5.0));
    w.tick(&defs);
    assert_eq!(rises(&w), vec![(day_id(&defs, "summer_solstice"), 1)]);
}

#[test]
fn learned_mid_day_there_is_no_refill_but_the_regen_applies() {
    let defs = Defs::bundled();
    let mut w = world(&defs);
    drain_chi(&mut w);
    // Learned two minutes into the day.
    w.tick = moon(&defs) + mins(2.0);
    known(&mut w, &defs);
    for _ in 0..100 {
        w.tick(&defs);
    }
    assert!(rises(&w).is_empty());
    let on = w.units[0].chi;
    assert!(on > 0.0 && on < 20.0, "{on}");
}

// ---- the regen ----

/// Chi that units 0 (Water), 2 (Fire) and 3 (Avatar) regain over `n` ticks of `tick_needs`
/// starting at `t`, from empty.
fn gain(defs: &Defs, t: u64, n: u64) -> (f32, f32, f32) {
    let mut w = world(defs);
    known(&mut w, defs);
    w.tick = t;
    drain_chi(&mut w);
    for _ in 0..n {
        w.tick_needs(defs);
    }
    (w.units[0].chi, w.units[2].chi, w.units[3].chi)
}

#[test]
fn chi_regen_runs_at_the_days_multiple_for_its_nation_only() {
    let defs = Defs::bundled();
    let (water_on, fire_on, avatar_on) = gain(&defs, moon(&defs) + 100, 600);
    let (water_off, fire_off, avatar_off) = gain(&defs, season_start(&defs, "autumn") + 100, 600);
    assert!(water_off > 0.0);
    assert!((water_on / water_off - 2.0).abs() < 0.01, "2x on the day: {water_on} vs {water_off}");
    assert!((fire_on - fire_off).abs() < 1e-3, "Fire isn't Water's: {fire_on} vs {fire_off}");
    assert!((avatar_on - avatar_off).abs() < 1e-3, "the Avatar is outside: {avatar_on}");
}

#[test]
fn the_day_ends_and_the_regen_returns_to_normal() {
    let defs = Defs::bundled();
    let (after, _, _) = gain(&defs, moon(&defs) + mins(5.0), 600);
    let (base, _, _) = gain(&defs, season_start(&defs, "autumn") + 100, 600);
    assert!((after - base).abs() < 1e-3);
}

// ---- seasons off, nothing saved ----

#[test]
fn it_works_with_seasons_off_and_on_alike() {
    let defs = Defs::bundled();
    for preset in [None, Some("normal")] {
        let mut w = world(&defs);
        w.difficulty = preset.map(Into::into);
        known(&mut w, &defs);
        drain_chi(&mut w);
        arrive_at(&mut w, moon(&defs));
        w.tick(&defs);
        assert!(w.units[0].chi >= defs.chi.max - 0.1, "{preset:?}");
        assert_eq!(rises(&w).len(), 1, "{preset:?}");
    }
}

#[test]
fn nothing_is_saved_for_the_days() {
    let defs = Defs::bundled();
    let mut a = world(&defs);
    known(&mut a, &defs);
    a.tick = moon(&defs) + 40;
    let bytes = a.save(&defs).unwrap();
    let text = bytes.clone();
    assert!(!text.contains("harvest_moon") && !text.contains("celestial"));
    let b = World::load(&text, &defs).unwrap();
    assert_eq!(b.celestial_day(&defs).unwrap().id, "harvest_moon");
}

// ---- the warning ----

#[test]
fn the_warning_fires_two_minutes_ahead_once() {
    let defs = Defs::bundled();
    let mut w = world(&defs);
    known(&mut w, &defs);
    let warn_at = moon(&defs) - mins(2.0);
    arrive_at(&mut w, warn_at - 5);
    for _ in 0..(mins(2.0) + 100) {
        w.tick(&defs);
    }
    assert_eq!(warnings(&w), vec![(warn_at, day_id(&defs, "harvest_moon"))]);
}

#[test]
fn no_warning_without_the_feature_or_a_resident() {
    let defs = Defs::bundled();
    let mut w = world(&defs);
    arrive_at(&mut w, moon(&defs) - mins(2.0));
    w.tick(&defs);
    assert!(warnings(&w).is_empty(), "unknown");
    let mut w = world(&defs);
    known(&mut w, &defs);
    // Earth has no resident.
    let earth = season_start(&defs, "winter") + mins(5.0) - mins(2.0);
    arrive_at(&mut w, earth);
    w.tick(&defs);
    assert!(warnings(&w).is_empty());
}

// ---- the Almanac ----

#[test]
fn the_almanac_lists_the_running_day_then_the_rest_soonest_first() {
    let defs = Defs::bundled();
    let mut w = world(&defs);
    w.research.grant(defs.tech_id("calendar").unwrap());
    assert_eq!(w.almanac(&defs).unwrap().celestial, vec![], "before star_charts");
    known(&mut w, &defs);
    // Residents: Water 2, Fire 1. Air and Earth have none, so they get no line.
    let (water, fire) = (day_id(&defs, "harvest_moon"), day_id(&defs, "summer_solstice"));
    w.tick = 10;
    assert_eq!(
        w.almanac(&defs).unwrap().celestial,
        vec![
            (fire, Timing::In { ticks: season_start(&defs, "summer") + mins(5.0) - 10 }, 1),
            (water, Timing::In { ticks: moon(&defs) - 10 }, 2),
        ]
    );
    // The harvest moon running: it leads, with its time left; Fire's next is next year.
    w.tick = moon(&defs) + mins(1.0);
    let lines = w.almanac(&defs).unwrap().celestial;
    assert_eq!(lines[0], (water, Timing::Now { left: mins(4.0) }, 2));
    assert_eq!(lines.len(), 2);
    assert_eq!(lines[1].0, fire);
    let Timing::In { ticks } = lines[1].1 else { panic!("{lines:?}") };
    assert_eq!(
        ticks,
        defs.seasons.year_ticks() + season_start(&defs, "summer") + mins(5.0) - w.tick
    );
}

#[test]
fn a_nation_with_a_resident_gets_a_line() {
    let defs = Defs::bundled();
    let mut w = world(&defs);
    w.research.grant(defs.tech_id("calendar").unwrap());
    known(&mut w, &defs);
    w.units[0].nation = Some(Nation::Air);
    w.units[1].nation = Some(Nation::Earth);
    w.tick = 0;
    let lines = w.almanac(&defs).unwrap().celestial;
    assert_eq!(lines.len(), 3, "air, fire, earth: {lines:?}");
    let benders: u16 = lines.iter().map(|l| l.2).sum();
    assert_eq!(benders, 3, "the Avatar is no resident");
}
