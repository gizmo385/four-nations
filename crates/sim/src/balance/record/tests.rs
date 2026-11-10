use std::collections::BTreeMap;

use super::*;
use crate::balance::{Probe, Sample};
use crate::defs::{BiomeId, Terrain};
use crate::geom::{Grid, Pos};
use crate::mapgen::{Tile, TileMap};
use crate::needs::DeathCause;
use crate::world::{Command, UnitId};

fn spec(seed: u64, nation: Nation, minutes: f32, defs: &Defs) -> RunSpec {
    RunSpec::new(seed, nation, minutes, defs)
}

/// The same spec, run twice, gives an equal record (module doc).
#[test]
fn two_runs_of_the_same_spec_give_identical_records() {
    let defs = Defs::bundled();
    let s = spec(7, Nation::Earth, 20.0, &defs);
    assert_eq!(run(&s, &defs), run(&s, &defs));
    let idle = RunSpec { player: Player::Idle, ..s };
    assert_eq!(run(&idle, &defs), run(&idle, &defs));
}

/// A record is JSON-able, and says what the doc says it holds.
#[test]
fn a_record_serializes_with_its_parts() {
    let defs = Defs::bundled();
    let r = run(&spec(7, Nation::Fire, 20.0, &defs), &defs);
    let json = serde_json::to_value(&r).unwrap();
    for key in
        ["site", "seasons", "sign_minutes", "timeline", "care", "deaths", "flags", "autopsies"]
    {
        assert!(json.get(key).is_some(), "no {key} in {json}");
    }
    assert!(json["site"]["supply"]["stone"].is_number());
    assert!(json["care"]["person_minutes"].as_f64().unwrap() > 0.0);
}

/// A record read back from its JSON is the record (a baseline or a saved panel is read this way).
#[test]
fn a_record_survives_its_json() {
    let defs = Defs::bundled();
    let r = run(&spec(7, Nation::Fire, 60.0, &defs), &defs);
    let back: RunRecord = serde_json::from_str(&serde_json::to_string(&r).unwrap()).unwrap();
    assert_eq!(back, r);
}

/// An offset of 0 is the plain run, and an offset of 1 is a different run of the same game: the
/// opening decides one tick later, so something downstream shifts (#538).
#[test]
fn a_step_offset_of_zero_is_the_plain_run() {
    let defs = Defs::bundled();
    let plain = spec(7, Nation::Earth, 25.0, &defs);
    let zero = RunSpec { step_offset: 0, ..plain.clone() };
    let a = run(&plain, &defs);
    assert_eq!(run(&zero, &defs), a);
    let one = RunSpec { step_offset: 1, ..plain };
    let b = run(&one, &defs);
    assert_ne!(a, b, "a one-tick delay changed nothing in 25 minutes");
    assert_eq!((a.seed, a.nation, a.minutes), (b.seed, b.nation, b.minutes));
}

/// `run_economy` is this run's economy half: the same numbers.
#[test]
fn the_economy_report_is_the_records_economy_half() {
    let defs = Defs::bundled();
    let size = defs.embark.map_size("small").unwrap();
    let cfg = super::super::EconomyConfig {
        seed: 7,
        nation: Nation::Water,
        preset: Some("normal".to_owned()),
        minutes: 30.0,
        farms: true,
        branch_first: false,
        width: size.width,
        height: size.height,
    };
    let record = run(&spec(7, Nation::Water, 30.0, &defs), &defs);
    assert_eq!(super::super::run_economy(&cfg, &defs), record.economy_report());
}

/// A style is a field: no farms and an idle settlement are different runs.
#[test]
fn the_style_changes_the_run() {
    let defs = Defs::bundled();
    let s = spec(7, Nation::Earth, 20.0, &defs);
    let idle = RunSpec { player: Player::Idle, ..s.clone() };
    let (played, left_alone) = (run(&s, &defs), run(&idle, &defs));
    assert_ne!(played.seasons, left_alone.seasons);
    assert!(played.timeline.len() > left_alone.timeline.len());
}

/// The profile follows the nation: only Earth tunnels, and what it reaches by boring is never
/// less than what the others reach on the same seed.
#[test]
fn site_profiles_count_a_tunnelling_nation_s_reach() {
    let defs = Defs::bundled();
    let size = defs.embark.map_size("small").unwrap();
    for seed in 1..=8 {
        let survey = Survey::new(&Survey::config(seed, size), &defs);
        let start = survey.default_site(&defs);
        let profiles: Vec<(Nation, SiteProfile)> = Nation::ALL
            .iter()
            .map(|&n| (n, SiteProfile::compute(&survey, start, n, &defs)))
            .collect();
        for (nation, p) in &profiles {
            assert_eq!(p.tunnels, *nation == Nation::Earth, "{nation:?}");
            assert_eq!(p.anchor, (start.x, start.y));
            assert_eq!(p.stone() > 0, p.supply["stone"] > 0);
        }
        let ore = |n: Nation| profiles.iter().find(|(m, _)| *m == n).unwrap().1.ore();
        assert!(ore(Nation::Earth) >= ore(Nation::Water), "seed {seed}");
        // The rest of the profile belongs to the site, not the nation.
        let rest = |p: &SiteProfile| (p.fresh_water_steps, p.food_sources, p.open_tiles);
        assert!(profiles.windows(2).all(|w| rest(&w[0].1) == rest(&w[1].1)), "seed {seed}");
    }
}

/// Seed 3's default site, the one the doc's Water examples sit on: stone-rich (#512), with ore
/// to spare, water within reach and room to move. #410 dropped the pond beside it (and the fish
/// in it), so the walk to water grew from 1 step to 13 and there is no food source nearby.
#[test]
fn seed_3_is_a_stone_rich_site_with_room() {
    let defs = Defs::bundled();
    let size = defs.embark.map_size("small").unwrap();
    let survey = Survey::new(&Survey::config(3, size), &defs);
    let start = survey.default_site(&defs);
    let p = SiteProfile::compute(&survey, start, Nation::Water, &defs);
    assert_eq!(p.stone(), 696);
    assert!(p.ore() >= 40, "{p:?}");
    assert!(p.fresh_water_steps.is_some_and(|s| s <= defs.embark.outlook.water_far), "{p:?}");
    assert!(p.open_tiles > 30, "{p:?}");
}

/// Care comes from `World::care()`: the same minutes, rounded the same way.
#[test]
fn the_care_summary_reads_the_worlds_tally() {
    let defs = Defs::bundled();
    let (r, world) = run_world(&spec(7, Nation::Earth, 20.0, &defs), &defs);
    assert_eq!(r.care, CareSummary::of(&world));
    // Six settlers for twenty minutes, plus whoever arrived on the way.
    assert!(r.care.person_minutes >= 6.0 * 20.0 - 1.0, "{:?}", r.care);
}

/// Every death in a run gets an autopsy, in order, with its cause and minute. With nobody
/// playing on Hard (no starting stores), seed 1 on Earth starves two settlers by minute 70.
#[test]
fn every_death_gets_an_autopsy() {
    let defs = Defs::bundled();
    let s = RunSpec {
        preset: Some("hard".to_owned()),
        player: Player::Idle,
        ..spec(1, Nation::Earth, 70.0, &defs)
    };
    let (r, world) = run_world(&s, &defs);
    assert!(r.deaths > 0, "expected someone to die with nobody playing on Hard: {r:?}");
    assert_eq!(r.autopsies.len(), world.deaths().count());
    for (a, d) in r.autopsies.iter().zip(world.deaths()) {
        assert_eq!(
            (a.name.as_str(), a.cause, a.weakened_by),
            (d.name.as_str(), d.cause, d.weakened_by)
        );
        assert!((a.minute - ticks_to_minutes(d.tick)).abs() < 1e-3);
        assert!(a.last_minutes.len() <= 10 && !a.last_minutes.is_empty(), "{a:?}");
        assert!(a.minutes_since_ate >= 0.0 && a.minutes_since_ate <= a.minute, "{a:?}");
        assert!(a.minutes_since_drank >= 0.0 && a.minutes_since_drank <= a.minute, "{a:?}");
        assert!(a.summary().starts_with(&format!("{:?}", a.cause)));
    }
    assert_eq!(r.first_death_minute, Some(r.autopsies[0].minute));
}

/// A map from ASCII rows (`~` water, `.` land, `T` tree), with a unit per spawn.
fn arena(defs: &Defs, rows: &[&str]) -> TileMap {
    Grid::from_fn(rows[0].len() as i32, rows.len() as i32, |p| {
        let c = rows[p.y as usize].as_bytes()[p.x as usize];
        Tile {
            elevation: 100,
            temperature: 128,
            moisture: 128,
            terrain: if c == b'~' { Terrain::Water } else { Terrain::Land },
            biome: BiomeId(0),
            resource: (c == b'T').then(|| defs.resource_id("oak_tree")).flatten(),
            building: None,
        }
    })
}

fn bare_site() -> SiteProfile {
    SiteProfile {
        anchor: (0, 0),
        supply: BTreeMap::new(),
        tunnels: false,
        fresh_water_steps: None,
        food_sources: 0,
        open_tiles: 0,
    }
}

/// Samples `world` once a minute for `minutes` minutes and returns the last sample.
fn sample_for(world: &mut World, defs: &Defs, minutes: u32) -> Sample {
    let mut probe = Probe::new(world, defs, &bare_site());
    let mut last = Sample::default();
    for _ in 0..minutes {
        last = probe.sample(world, defs);
        for _ in 0..600 {
            world.tick(defs);
        }
    }
    last
}

/// Work it can't reach doesn't strand a settler: it can still drink, store and rejoin the others,
/// so it is idle (`World::idle_reason` says `Unreachable` for plenty that is no barrier, a build
/// waiting for wood among it), not cut off.
#[test]
fn unreachable_work_alone_does_not_strand_a_settler() {
    let defs = Defs::bundled();
    let rows = ["~~~~~~~~~", "~T~.....~", "~~~.....~", "~~~..T..~", "~~~~~~~~~"];
    let mut world =
        World::with_map(1, arena(&defs, &rows), Nation::Earth, &[Pos::new(3, 2)], &defs);
    world.apply(Command::DesignateHarvest { pos: Pos::new(1, 1) }, &defs).unwrap();
    assert_eq!(world.idle_reason(UnitId(0), &defs), Some(crate::world::IdleReason::Unreachable));
    let s = sample_for(&mut world, &defs, 6);
    assert_eq!((s.longest_stranded, s.idle, s.settlers), (0, 1, 1));
}

/// The same settler with only reachable work is never stranded.
#[test]
fn a_settler_with_reachable_work_is_not_stranded() {
    let defs = Defs::bundled();
    let rows = ["~~~~~~~~~", "~T~.....~", "~~~.....~", "~~~..T..~", "~~~~~~~~~"];
    let mut world =
        World::with_map(1, arena(&defs, &rows), Nation::Earth, &[Pos::new(3, 2)], &defs);
    world.apply(Command::DesignateHarvest { pos: Pos::new(5, 3) }, &defs).unwrap();
    let s = sample_for(&mut world, &defs, 5);
    assert_eq!(s.longest_stranded, 0, "{s:?}");
}

/// A settler on land with no way to water is stranded even with nothing designated: the other
/// half of the `stranded` rule.
#[test]
fn a_settler_with_no_path_to_water_is_stranded() {
    let defs = Defs::bundled();
    let rows = ["........", "........", "........"];
    let mut world =
        World::with_map(1, arena(&defs, &rows), Nation::Earth, &[Pos::new(3, 1)], &defs);
    let reach = world.reach(UnitId(0)).unwrap();
    assert!(!reach.water);
    assert_eq!(reach.stockpile, None);
    assert_eq!(sample_for(&mut world, &defs, 4).longest_stranded, 4);
}

/// Cut off from a marked stockpile (a strait between), though water is at hand.
#[test]
fn a_settler_cut_off_from_the_stockpile_is_stranded() {
    let defs = Defs::bundled();
    let rows = ["......~.", "......~.", "......~."];
    let mut world =
        World::with_map(1, arena(&defs, &rows), Nation::Earth, &[Pos::new(1, 1)], &defs);
    world.apply(Command::DesignateStockpile { pos: Pos::new(7, 1) }, &defs).unwrap();
    let reach = world.reach(UnitId(0)).unwrap();
    assert_eq!((reach.water, reach.stockpile), (true, Some(false)));
    assert!(sample_for(&mut world, &defs, 4).longest_stranded >= 3);
    // And one that can walk to it is fine.
    let mut near = World::with_map(1, arena(&defs, &rows), Nation::Earth, &[Pos::new(1, 1)], &defs);
    near.apply(Command::DesignateStockpile { pos: Pos::new(3, 1) }, &defs).unwrap();
    assert_eq!(near.reach(UnitId(0)).unwrap().stockpile, Some(true));
    assert_eq!(sample_for(&mut near, &defs, 4).longest_stranded, 0);
}

/// `food_near` counts stockpiled and ripe wild food inside the radius and nothing outside it.
#[test]
fn food_near_counts_what_lies_within_the_radius() {
    let defs = Defs::bundled();
    let cabbage = defs.resource_id("cabbage").unwrap();
    let yield_of = defs.harvest(cabbage).unwrap().count;
    // Cabbages at x = 5 (within 15 of x = 2) and x = 30 (outside).
    let mut map =
        arena(&defs, &["~".repeat(40).as_str(), format!("~{}~", ".".repeat(38)).as_str()]);
    for x in [5, 30] {
        map[Pos::new(x, 1)].resource = Some(cabbage);
    }
    let world = World::with_map(1, map, Nation::Earth, &[Pos::new(2, 1)], &defs);
    assert_eq!(world.food_near(Pos::new(2, 1), 15, &defs), yield_of);
    assert_eq!(world.food_near(Pos::new(2, 1), 2, &defs), 0);
    assert_eq!(world.food_near(Pos::new(30, 1), 15, &defs), yield_of);
    assert_eq!(world.food_near(Pos::new(18, 1), 15, &defs), 2 * yield_of);
}

/// Refusals: the routine ones a stateless player gets don't build a streak; a command that was
/// wrong does, and an accepted one ends it.
#[test]
fn only_wrong_commands_build_a_refusal_streak() {
    let defs = Defs::bundled();
    let rows = ["~~~~~~", "~..T.~", "~~~~~~"];
    let mut world =
        World::with_map(1, arena(&defs, &rows), Nation::Earth, &[Pos::new(1, 1)], &defs);
    let tree = Pos::new(3, 1);
    world.apply(Command::DesignateHarvest { pos: tree }, &defs).unwrap();
    for _ in 0..30 {
        assert!(world.apply(Command::DesignateHarvest { pos: tree }, &defs).is_err());
    }
    assert_eq!((world.refusals().total(), world.refusals().streak()), (30, 0));
    for _ in 0..5 {
        assert!(world.apply(Command::DesignateHarvest { pos: Pos::new(0, 0) }, &defs).is_err());
    }
    assert_eq!(world.refusals().streak(), 5);
    world.apply(Command::DesignateStockpile { pos: Pos::new(2, 1) }, &defs).unwrap();
    assert_eq!(world.refusals().streak(), 0);
}

/// A settler on dry land, with no water to reach and no food to find, dies unfed; its autopsy
/// says so: no path to water, nothing to eat nearby, nothing eaten or drunk since it arrived.
#[test]
fn an_autopsy_names_the_water_and_food_it_never_had() {
    let defs = Defs::bundled();
    let rows = ["........", "........", "........"];
    let mut world =
        World::with_map(1, arena(&defs, &rows), Nation::Earth, &[Pos::new(3, 1)], &defs);
    let mut probe = Probe::new(&world, &defs, &bare_site());
    let mut minutes = 0u16;
    while world.deaths().next().is_none() {
        assert!(minutes < 300, "nobody died in five hours");
        probe.sample(&world, &defs);
        for _ in 0..600 {
            world.tick(&defs);
        }
        minutes += 1;
    }
    let autopsies = probe.autopsies(&world, &defs);
    assert_eq!(autopsies.len(), 1);
    let a = &autopsies[0];
    assert!(matches!(a.cause, DeathCause::Dehydration | DeathCause::Starvation), "{a:?}");
    assert!(!a.water_reachable, "{a:?}");
    assert_eq!((a.stockpile_reachable, a.food_nearby), (None, 0));
    assert!(a.minutes_since_drank >= f32::from(minutes) - 2.0, "{a:?}");
    assert!(a.minutes_since_ate >= f32::from(minutes) - 2.0, "{a:?}");
    assert!(a.summary().contains("no path to water"), "{}", a.summary());
    // Asked again, there is nothing new to report.
    assert!(probe.autopsies(&world, &defs).is_empty());
}
