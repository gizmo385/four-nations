use fn_sim::Nation;
use fn_sim::balance::{Autopsy, EconomySeason, RunRecord};

use super::*;
use crate::testkit::record;

fn parse(name: &str) -> Metric {
    Metric::parse(name, &Defs::bundled()).unwrap_or_else(|e| panic!("{name}: {e}"))
}

fn read(name: &str, rec: &RunRecord) -> Option<f64> {
    parse(name).read(rec, &Defs::bundled())
}

fn season(
    year: u64,
    season: usize,
    minute: f32,
    population: u32,
    wood: u32,
    food: f32,
) -> EconomySeason {
    EconomySeason {
        year,
        season,
        minute,
        stone_stored: 0,
        ore_mined: 0,
        wood_stored: wood,
        food_per_head: food,
        population,
        signs_met: Vec::new(),
    }
}

#[test]
fn every_name_parses_and_a_bad_one_says_why() {
    for name in [
        "sign_minute.stone",
        "ready_minute",
        "first_death",
        "deaths",
        "deaths_by.180",
        "pop_at.120",
        "wood_at.120",
        "food_per_head.winter.2",
        "longest_gap",
        "gaps_over_15",
        "care.close_calls",
        "care.close_calls_per_10ph",
        "care.depleted_per_ph",
        "care.suffering",
    ] {
        parse(name);
    }
    let defs = Defs::bundled();
    let err = |name: &str| Metric::parse(name, &defs).expect_err(name);
    assert!(err("sign_minute.nonsense").contains("no sign"));
    assert!(err("sign_minute").contains("unknown metric"));
    assert!(err("pop_at.soon").contains("whole number"));
    assert!(err("food_per_head.summer.2").contains("winter"));
    assert!(err("deaths.5").contains("unknown metric"));
    assert!(err("care.love").contains("unknown metric"));
}

#[test]
fn an_unmet_sign_or_no_death_reads_as_infinity() {
    let mut rec = record(Nation::Earth, 1);
    assert_eq!(read("sign_minute.stone", &rec), Some(100.0));
    rec.sign_minutes.iter_mut().find(|(id, _)| id == "stone").unwrap().1 = None;
    assert_eq!(read("sign_minute.stone", &rec), Some(f64::INFINITY));
    assert_eq!(read("first_death", &rec), Some(f64::INFINITY));
    rec.first_death_minute = Some(61.5);
    assert_eq!(read("first_death", &rec), Some(61.5));
}

#[test]
fn ready_ignores_masters_and_scrolls_and_waits_for_the_rest() {
    let mut rec = record(Nation::Earth, 1);
    // The default record has masters unmet, which Ready ignores; population is the last at 110.
    assert_eq!(read("ready_minute", &rec), Some(110.0));
    rec.sign_minutes.iter_mut().find(|(id, _)| id == "ore").unwrap().1 = None;
    assert_eq!(read("ready_minute", &rec), Some(f64::INFINITY));
}

#[test]
fn season_rows_answer_population_wood_and_winter_food() {
    let defs = Defs::bundled();
    let winter = defs.seasons.seasons.iter().position(|s| s.winter).expect("a winter");
    let mut rec = record(Nation::Earth, 1);
    rec.seasons = vec![
        season(1, 0, 0.0, 6, 10, 2.5),
        season(2, 0, 60.0, 9, 80, 3.0),
        season(2, winter, 105.0, 12, 120, 4.5),
        season(3, 0, 120.0, 15, 200, 5.0),
    ];
    assert_eq!(read("pop_at.120", &rec), Some(15.0));
    assert_eq!(read("wood_at.120", &rec), Some(200.0));
    assert_eq!(read("pop_at.60", &rec), Some(9.0));
    assert_eq!(read("pop_at.90", &rec), None, "no row there: the record can't say");
    assert_eq!(read("food_per_head.winter.2", &rec), Some(4.5));
    assert_eq!(read("food_per_head.winter.3", &rec), None);
    // The run ended before the minute asked about.
    rec.minutes = 100.0;
    assert_eq!(read("pop_at.120", &rec), None);
}

#[test]
fn deaths_by_a_minute_come_from_the_autopsies() {
    let mut rec = record(Nation::Earth, 1);
    let dead = |minute: f32| Autopsy {
        name: "x".to_owned(),
        minute,
        cause: fn_sim::needs::DeathCause::Starvation,
        weakened_by: None,
        minutes_since_ate: 0.0,
        minutes_since_drank: 0.0,
        last_minutes: Vec::new(),
        unreachable_minutes: 0,
        food_nearby: 0,
        water_reachable: true,
        stockpile_reachable: None,
        home_reachable: true,
    };
    rec.autopsies = vec![dead(40.0), dead(150.0)];
    rec.deaths = 2;
    assert_eq!(read("deaths_by.100", &rec), Some(1.0));
    assert_eq!(read("deaths_by.180", &rec), Some(2.0));
    assert_eq!(read("deaths", &rec), Some(2.0));
}

#[test]
fn care_rates_are_per_person_hour() {
    let mut rec = record(Nation::Earth, 1);
    // 1800 person-minutes is 30 person-hours.
    rec.care.close_calls = 3;
    rec.care.depleted_minutes = [30.0, 15.0, 0.0];
    rec.care.severe_any_minutes = 180.0;
    assert_eq!(read("care.close_calls", &rec), Some(3.0));
    assert_eq!(read("care.close_calls_per_10ph", &rec), Some(1.0));
    assert_eq!(read("care.depleted_per_ph", &rec), Some(1.5));
    assert_eq!(read("care.suffering", &rec), Some(0.1));
    rec.care.person_minutes = 0.0;
    assert_eq!(read("care.suffering", &rec), Some(0.0));
}

#[test]
fn a_gap_reads_its_length() {
    let mut rec = record(Nation::Earth, 1);
    assert_eq!(read("longest_gap", &rec), Some(0.0));
    rec.longest_gap = Some((40.0, 58.5));
    rec.gaps_over_15 = 2;
    assert_eq!(read("longest_gap", &rec), Some(18.5));
    assert_eq!(read("gaps_over_15", &rec), Some(2.0));
}

#[test]
fn deadlines_follow_what_the_target_asks() {
    let defs = Defs::bundled();
    let rec = record(Nation::Earth, 1);
    let at_most = Bar { at_most: Some(120.0), at_least: None };
    let none = Bar { at_most: None, at_least: Some(1.0) };
    assert_eq!(parse("sign_minute.stone").deadline(&at_most, &rec, &defs), 120.0);
    assert_eq!(parse("first_death").deadline(&none, &rec, &defs), 180.0);
    assert_eq!(parse("pop_at.120").deadline(&none, &rec, &defs), 120.0);
    assert_eq!(parse("deaths_by.150").deadline(&at_most, &rec, &defs), 150.0);
    assert_eq!(parse("longest_gap").deadline(&at_most, &rec, &defs), 180.0);
}
