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
        food_stored: (food * population as f32) as u32,
        herbs_stored: 12,
        scrolls_copied: 7,
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
        "food_at.180",
        "herbs_at.120",
        "copies_by.240",
        "food_per_head.winter.2",
        "longest_gap",
        "gaps_over_15",
        "care.close_calls",
        "care.close_calls_per_10ph",
        "care.depleted_per_ph",
        "care.suffering",
        "stage_minute.3",
        "school_minute",
        "tally.ice_paid",
    ] {
        assert_eq!(parse(name).name(), name, "name() is parse()'s inverse");
    }
    let defs = Defs::bundled();
    let err = |name: &str| Metric::parse(name, &defs).expect_err(name);
    assert!(err("sign_minute.nonsense").contains("no sign"));
    assert!(err("sign_minute").contains("unknown metric"));
    assert!(err("pop_at.soon").contains("whole number"));
    assert!(err("food_per_head.summer.2").contains("winter"));
    assert!(err("deaths.5").contains("unknown metric"));
    assert!(err("care.love").contains("unknown metric"));
    assert!(err("stage_minute.0").contains("no stage"));
    assert!(err("stage_minute.9").contains("no stage"));
    assert!(err("tally.gold").contains("no tally"));
}

/// horizons-phase-4.md, "Measuring it" (#731): the stage minute, the first school learned and a
/// school's tally.
#[test]
fn stage_school_and_tally_read_off_the_record() {
    let mut rec = record(Nation::Earth, 1);
    rec.stage_minutes = vec![
        ("foundation".to_owned(), Some(150.0)),
        ("pillars".to_owned(), Some(190.0)),
        ("arch".to_owned(), None),
        ("rite".to_owned(), None),
    ];
    assert_eq!(read("stage_minute.2", &rec), Some(190.0));
    assert_eq!(read("stage_minute.3", &rec), Some(f64::INFINITY), "unfinished");
    assert_eq!(read("school_minute", &rec), Some(f64::INFINITY), "no school yet");
    rec.techs = vec![("fire_making".to_owned(), 6.0), ("metal_city".to_owned(), 171.5)];
    assert_eq!(read("school_minute", &rec), Some(171.5));
    assert_eq!(read("tally.rail_powered", &rec), None, "an older record has none");
    rec.school_tallies = vec![("rail_powered".to_owned(), 2)];
    assert_eq!(read("tally.rail_powered", &rec), Some(2.0));
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
    assert_eq!(read("food_at.120", &rec), Some(75.0), "5 a head for 15 people");
    assert_eq!(read("herbs_at.120", &rec), Some(12.0));
    assert_eq!(read("copies_by.120", &rec), Some(7.0));
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

/// horizons-phase-6.md, "Measuring it" (#751): `gate_minute`, and `after_gate.NAME` counting a
/// minute metric from it.
#[test]
fn the_gate_minute_and_metrics_counted_from_it() {
    let mut rec = record(Nation::Earth, 1);
    assert_eq!(read("gate_minute", &rec), Some(f64::INFINITY), "no gate stood");
    assert_eq!(read("after_gate.first_offer_minute", &rec), None, "no gate, no reading");
    rec.gate_minute = Some(240.0);
    rec.teachings.offers = vec![300.0];
    assert_eq!(read("gate_minute", &rec), Some(240.0));
    assert_eq!(read("after_gate.first_offer_minute", &rec), Some(60.0));
    assert_eq!(read("after_gate.sign_minute.stone", &rec), Some(-140.0), "before the gate");
    rec.teachings.offers.clear();
    assert_eq!(read("after_gate.first_offer_minute", &rec), Some(f64::INFINITY));

    let defs = Defs::bundled();
    assert_eq!(parse("after_gate.first_offer_minute").name(), "after_gate.first_offer_minute");
    let err = Metric::parse("after_gate.deaths", &defs).expect_err("not a minute");
    assert!(err.contains("not a minute"), "{err}");
    assert!(Metric::parse("after_gate.nonsense", &defs).is_err());

    // The deadline of an `after_gate` bar counts from the gate too.
    rec.minutes = 420.0;
    let bar = Bar { at_most: Some(90.0), at_least: None };
    let m = parse("after_gate.first_offer_minute");
    assert_eq!(m.deadline(&bar, &rec, &defs), 330.0);
    rec.gate_minute = None;
    assert_eq!(m.deadline(&bar, &rec, &defs), rec.minutes);
}
