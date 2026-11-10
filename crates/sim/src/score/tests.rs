use super::*;
use crate::mapgen::MapGenConfig;
use crate::nation::Nation;
use crate::needs::DeathCause;
use crate::world::Death;

/// The agent's turn: 20 in-game seconds (the doc's horizon of 540 turns is 180:00).
const TURN: u64 = 20 * 10;

fn near(a: f64, b: f64) {
    assert!((a - b).abs() < 1e-9, "{a} != {b}");
}

/// A run's raw numbers over the bundled endeavor: all 9 signs, Ready and 4 stages unmet, no care
/// events, a 100-turn horizon. Tests set what they need.
fn blank(defs: &Defs) -> Inputs {
    let signs = defs.endeavor.signs.iter().map(|s| (s.id.clone(), MilestoneKind::Sign, 1));
    let ready = [(READY_ID.to_owned(), MilestoneKind::Ready, 1)];
    let stages = defs.endeavor.stages.iter().map(|s| (s.id.clone(), MilestoneKind::Stage, 1));
    Inputs {
        start: 0,
        horizon: 100 * TURN,
        world_tick: 100 * TURN,
        milestones: signs
            .chain(ready)
            .chain(stages)
            .map(|(id, kind, target)| Milestone {
                name: id.clone(),
                id,
                kind,
                first_met: None,
                value: 0,
                target,
                met_now: false,
            })
            .collect(),
        ready_latched: false,
        deaths: 0,
        close_calls: 0,
        person_minutes: 1000.0,
        severe_any_minutes: 0.0,
        severe_minutes: Vec::new(),
    }
}

impl Inputs {
    fn milestone(&mut self, id: &str) -> &mut Milestone {
        self.milestones.iter_mut().find(|m| m.id == id).expect("a bundled milestone")
    }

    /// `id` first met at turn `t` and holding at `value` of `target`.
    fn met(&mut self, id: &str, t: u64, value: u32, target: u32) {
        let m = self.milestone(id);
        (m.first_met, m.value, m.target, m.met_now) = (Some(t * TURN), value, target, true);
    }

    /// `id` at `value` of `target`, not met.
    fn short(&mut self, id: &str, value: u32, target: u32) {
        let m = self.milestone(id);
        (m.value, m.target) = (value, target);
    }
}

fn row<'a>(card: &'a Scorecard, id: &str) -> &'a MilestoneRow {
    card.milestones.iter().find(|r| r.id == id).expect("a row for every milestone")
}

fn setup() -> (Defs, ScoreDef) {
    (Defs::bundled(), ScoreDef::bundled())
}

#[test]
fn bundled_score_data_matches_the_owner_decisions_and_the_endeavor() {
    let (defs, score) = setup();
    score.check(&defs).expect("no weight names a missing milestone");
    near(score.progress.late_credit, 0.6);
    near(score.progress.partial_credit, 0.5);
    near(score.progress.default_weight, 1.0);
    assert!(score.progress.weights.is_empty());
    near(score.care.death, 0.15);
    near(score.care.close_call, 0.03);
    near(score.care.suffering, 2.0);
    assert_eq!(score.care.severe, Severe::ALL);
    // Nine signs, Ready and four stages: the doc's 14.
    assert_eq!(milestone_ids(&defs).len(), 14);
}

#[test]
fn changing_a_weight_leaves_the_data_digest_alone() {
    let defs = Defs::bundled();
    let before = defs.id_hash();
    let heavier = include_str!("../../../../data/score.toml").replace(
        "default_weight = 1.0",
        "default_weight = 3.0\n\n[[progress.weight]]\nid = \"scrolls\"\nweight = 0.5",
    );
    let score = ScoreDef::from_toml(&heavier).expect("still valid");
    score.check(&defs).expect("scrolls is a sign");
    near(score.progress.weight_of("scrolls"), 0.5);
    near(score.progress.weight_of("ore"), 3.0);
    // Nothing in `Defs` reads the score data, so the digest cannot move.
    assert_eq!(defs.id_hash(), before);
    assert_eq!(Defs::bundled().id_hash(), before);
}

#[test]
fn score_data_is_strict() {
    let good = include_str!("../../../../data/score.toml");
    let bad = |from: &str, to: &str| {
        ScoreDef::from_toml(&good.replace(from, to)).expect_err(to).to_string()
    };
    assert!(bad("late_credit = 0.6", "late_credit = 1.5").contains("late_credit"));
    assert!(bad("partial_credit = 0.5", "partial_credit = -0.1").contains("partial_credit"));
    assert!(bad("default_weight = 1.0", "default_weight = 0.0").contains("default_weight"));
    assert!(bad("death = 0.15", "death = -1.0").contains("death"));
    assert!(bad("\"dying\"", "\"fainting\"").contains("fainting"));
    assert!(bad("\"starving\", ", "\"dying\", ").contains("twice"));
    assert!(bad("[care]", "[care]\nsurprise = 1").contains("surprise"));
    assert!(bad("close_call = 0.03", "close_call_health = 25.0").contains("close_call"));
    let unknown = format!("{good}\n[[progress.weight]]\nid = \"nope\"\nweight = 2.0\n");
    let score = ScoreDef::from_toml(&unknown).expect("syntactically fine");
    assert!(score.check(&Defs::bundled()).unwrap_err().to_string().contains("nope"));
    let dup = format!(
        "{good}\n[[progress.weight]]\nid = \"ore\"\nweight = 2.0\n[[progress.weight]]\nid = \"ore\"\nweight = 1.0\n"
    );
    assert!(ScoreDef::from_toml(&dup).unwrap_err().to_string().contains("twice"));
}

#[test]
fn a_sign_met_at_turn_zero_earns_one_and_at_the_horizon_earns_late_credit() {
    let (defs, score) = setup();
    let mut run = blank(&defs);
    run.met("population", 0, 6, 6);
    run.met("stone", 100, 200, 200);
    run.met("branch", 50, 6, 6);
    let card = Scorecard::build(&score, run);
    near(row(&card, "population").credit, 1.0);
    near(row(&card, "stone").credit, 0.6);
    near(row(&card, "branch").credit, 0.8);
    assert!(!row(&card, "stone").slipped);
}

/// Turn 0 at `start` turns, horizon at 100 turns: the late-credit line runs from one to the other.
#[test]
fn credit_runs_from_the_start_to_the_horizon_and_a_prelude_milestone_earns_one() {
    let (defs, score) = setup();
    let mut run = blank(&defs);
    run.start = 40 * TURN;
    // Met during the prelude, and exactly at its end: both full credit.
    run.met("population", 3, 6, 6);
    run.met("branch", 40, 6, 6);
    // Halfway between the start (turn 40) and the horizon (turn 100): the midpoint, 0.8.
    run.met("stone", 70, 200, 200);
    // At the horizon: late credit.
    run.met("ore", 100, 6, 6);
    let card = Scorecard::build(&score, run);
    near(row(&card, "population").credit, 1.0);
    near(row(&card, "branch").credit, 1.0);
    near(row(&card, "stone").credit, 0.8);
    near(row(&card, "ore").credit, 0.6);
    assert_eq!(card.start_tick, 40 * TURN);
    // A first-met tick stays the world's own.
    assert_eq!(row(&card, "population").first_met, Some(3 * TURN));
}

#[test]
fn a_start_at_the_horizon_leaves_no_slope_to_divide_by() {
    let (defs, score) = setup();
    let mut run = blank(&defs);
    run.start = run.horizon;
    run.met("population", 100, 6, 6);
    let card = Scorecard::build(&score, run);
    near(row(&card, "population").credit, 1.0);
}

#[test]
fn compute_with_a_prelude_credits_its_milestones_in_full_and_counts_care_only_after_it() {
    let defs = Defs::bundled();
    let score = ScoreDef::bundled();
    let cfg = MapGenConfig { seed: 7, width: 64, height: 64, ..MapGenConfig::default() };
    let mut world = World::new(&cfg, Nation::Water, 20, &defs);
    for _ in 0..100 {
        world.tick(&defs);
    }
    let start = ScoreStart::of(&world);
    assert_eq!(start.tick, 100);
    for _ in 0..200 {
        world.tick(&defs);
    }
    let id = world.units[0].id;
    let died = |tick| Death {
        id,
        name: "Test".to_owned(),
        tick,
        cause: DeathCause::Starvation,
        weakened_by: None,
    };
    world.insert_event_for_test(died(50));
    world.insert_event_for_test(died(250));

    let from_zero = Scorecard::compute(&world, &defs, &score, &ScoreStart::default(), 300);
    let from_start = Scorecard::compute(&world, &defs, &score, &start, 300);
    assert_eq!(from_start.start_tick, 100);
    // The population sign was met on the first check, inside the prelude: full credit once turn
    // 0 is the prelude's end, a little less when measured from tick 0.
    near(row(&from_start, "population").credit, 1.0);
    assert!(row(&from_zero, "population").credit < 1.0);
    // Care counts from the start only: the prelude's person-time and its death don't.
    assert_eq!(from_zero.care.deaths, 2);
    assert_eq!(from_start.care.deaths, 1);
    let after = world.care().since(&start.care);
    near(from_start.care.person_minutes, after.person_minutes());
    assert!(from_start.care.person_minutes < from_zero.care.person_minutes);
    near(from_start.care.person_minutes, from_zero.care.person_minutes * 200.0 / 300.0);
}

#[test]
fn the_default_start_scores_exactly_as_a_world_with_no_prelude_did() {
    let defs = Defs::bundled();
    let score = ScoreDef::bundled();
    let cfg = MapGenConfig { seed: 7, width: 64, height: 64, ..MapGenConfig::default() };
    let mut world = World::new(&cfg, Nation::Water, 20, &defs);
    let fresh = ScoreStart::of(&world);
    assert_eq!(fresh, ScoreStart::default(), "a world at tick 0 has nothing tallied");
    for _ in 0..300 {
        world.tick(&defs);
    }
    assert_eq!(
        Scorecard::compute(&world, &defs, &score, &fresh, 300),
        Scorecard::compute(&world, &defs, &score, &ScoreStart::default(), 300)
    );
    assert_eq!(world.care().since(&CareTally::default()), *world.care());
}

#[test]
fn a_slipped_sign_earns_partial_credit_only() {
    let (defs, score) = setup();
    let mut run = blank(&defs);
    run.met("beds", 10, 6, 6);
    // More people arrived: three beds short at the horizon.
    let m = run.milestone("beds");
    (m.value, m.target, m.met_now) = (18, 21, false);
    let card = Scorecard::build(&score, run);
    let beds = row(&card, "beds");
    assert!(beds.slipped);
    assert_eq!(beds.first_met, Some(10 * TURN));
    near(beds.credit, 0.5 * 18.0 / 21.0);
}

#[test]
fn once_ready_latches_every_sign_keeps_its_first_met_credit() {
    let (defs, score) = setup();
    let mut run = blank(&defs);
    for (i, sign) in defs.endeavor.signs.iter().enumerate() {
        run.met(&sign.id, i as u64 * 10, 5, 5);
    }
    run.met(READY_ID, 90, 1, 1);
    run.ready_latched = true;
    // Stores were eaten afterwards: food no longer holds.
    let food = run.milestone("food");
    (food.value, food.met_now) = (0, false);
    let card = Scorecard::build(&score, run);
    let food = row(&card, "food");
    assert!(!food.slipped);
    near(food.credit, 1.0 - 0.4 * 40.0 / 100.0);
    // Ready itself and the four unbuilt stages: Ready at turn 90, stages nothing.
    near(row(&card, READY_ID).credit, 1.0 - 0.4 * 0.9);
    for stage in &defs.endeavor.stages {
        near(row(&card, &stage.id).credit, 0.0);
    }
    // Without the latch the same lapse earns only partial credit.
    let mut unlatched = blank(&defs);
    unlatched.met("food", 40, 0, 5);
    unlatched.milestone("food").met_now = false;
    let card = Scorecard::build(&score, unlatched);
    assert!(row(&card, "food").slipped);
    near(row(&card, "food").credit, 0.0);
}

#[test]
fn unmet_overshoot_and_stages_are_capped() {
    let (defs, score) = setup();
    let mut run = blank(&defs);
    run.short("ore", 39, 40);
    // Value past target but never met (the second-boundary check hasn't run): still capped.
    run.short("stone", 412, 200);
    run.short("scrolls", 0, 3);
    let card = Scorecard::build(&score, run);
    near(row(&card, "ore").credit, 0.5 * 39.0 / 40.0);
    near(row(&card, "stone").credit, 0.5);
    near(row(&card, "scrolls").credit, 0.0);
    // A target of 0 earns nothing rather than dividing by it.
    let mut run = blank(&defs);
    run.short("masters", 3, 0);
    near(row(&Scorecard::build(&score, run), "masters").credit, 0.0);
}

#[test]
fn weights_change_the_progress_but_not_the_credits() {
    let (defs, score) = setup();
    let mut run = blank(&defs);
    run.met("population", 0, 6, 6);
    let even = Scorecard::build(&score, blank_with(&defs, &run));
    near(even.progress, 1.0 / 14.0);
    let heavy = ScoreDef::from_toml(&include_str!("../../../../data/score.toml").replace(
        "default_weight = 1.0",
        "default_weight = 1.0\n\n[[progress.weight]]\nid = \"population\"\nweight = 6.0",
    ))
    .unwrap();
    let card = Scorecard::build(&heavy, run);
    near(card.weight_total, 19.0);
    near(card.progress, 6.0 / 19.0);
}

/// A copy of `run` (`Inputs` isn't `Clone` because `compute` never needs one).
fn blank_with(defs: &Defs, run: &Inputs) -> Inputs {
    let mut copy = blank(defs);
    for (to, from) in copy.milestones.iter_mut().zip(&run.milestones) {
        (to.first_met, to.value, to.target, to.met_now) =
            (from.first_met, from.value, from.target, from.met_now);
    }
    copy
}

#[test]
fn deaths_and_close_calls_reduce_care_and_it_floors_at_zero() {
    let (defs, score) = setup();
    let care = |deaths: u32, close_calls: u32, severe: f64| {
        let mut run = blank(&defs);
        (run.deaths, run.close_calls, run.severe_any_minutes) = (deaths, close_calls, severe);
        Scorecard::build(&score, run).care
    };
    near(care(0, 0, 0.0).value, 1.0);
    near(care(2, 0, 0.0).value, 0.70);
    near(care(0, 3, 0.0).value, 0.91);
    // 5% of person-time suffering costs 0.10.
    let suffer = care(0, 0, 50.0);
    near(suffer.suffering, 0.05);
    near(suffer.value, 0.90);
    near(care(1, 1, 50.0).value, 1.0 - 0.15 - 0.03 - 0.10);
    near(care(7, 0, 0.0).value, 0.0);
    near(care(0, 0, 1000.0).value, 0.0);
    near(care(3, 20, 500.0).value, 0.0);
    // No person-time at all is not a division by zero.
    let mut run = blank(&defs);
    (run.person_minutes, run.severe_any_minutes) = (0.0, 0.0);
    near(Scorecard::build(&score, run).care.suffering, 0.0);
}

#[test]
fn care_multiplies_progress_so_two_deaths_cost_thirty_percent() {
    let (defs, score) = setup();
    let mut run = blank(&defs);
    run.met("population", 0, 6, 6);
    let safe = Scorecard::build(&score, blank_with(&defs, &run)).score;
    run.deaths = 2;
    let grieving = Scorecard::build(&score, run).score;
    near(grieving, safe * 0.70);
    // And a run with no progress can't buy a score by being safe.
    near(Scorecard::build(&score, blank(&defs)).score, 0.0);
}

/// The doc's worked example (agent-hillclimb.md): Water, seed 7633, Normal, horizon 540 turns.
#[test]
fn the_worked_example_scores_256() {
    let (defs, score) = setup();
    let mut run = blank(&defs);
    run.horizon = 540 * TURN;
    run.world_tick = 528 * TURN;
    run.met("population", 360, 21, 16);
    run.met("stone", 417, 412, 200);
    run.met("branch", 438, 6, 6);
    run.short("ore", 39, 40);
    run.short("building_kinds", 10, 11);
    run.met("beds", 13, 18, 21);
    run.milestone("beds").met_now = false;
    run.short("food", 34, 150);
    run.short("masters", 0, 3);
    run.short("scrolls", 0, 3);
    let card = Scorecard::build(&score, run);

    let expected = [
        ("population", 0.733),
        ("stone", 0.691),
        ("branch", 0.676),
        ("ore", 0.488),
        ("building_kinds", 0.455),
        ("beds", 0.429),
        ("food", 0.113),
        ("masters", 0.0),
        ("scrolls", 0.0),
        (READY_ID, 0.0),
    ];
    for (id, credit) in expected {
        assert!((row(&card, id).credit - credit).abs() < 0.0015, "{id}: {}", row(&card, id).credit);
    }
    assert!(row(&card, "beds").slipped);
    near(card.weight_total, 14.0);
    assert!((card.credit_total - 3.58).abs() < 0.01, "{}", card.credit_total);
    assert!((card.progress - 0.256).abs() < 0.001, "{}", card.progress);
    near(card.care.value, 1.0);
    assert_eq!(card.score.round(), 256.0);
    assert!(!card.past_horizon, "stopped early at turn 528 of 540");
}

#[test]
fn the_docs_other_comparisons_hold() {
    let (defs, score) = setup();
    let base = |beds_held: bool| {
        let mut run = blank(&defs);
        run.horizon = 540 * TURN;
        run.met("population", 360, 21, 16);
        run.met("stone", 417, 412, 200);
        run.met("branch", 438, 6, 6);
        run.short("ore", 39, 40);
        run.short("building_kinds", 10, 11);
        run.met("beds", 13, 18, 21);
        run.milestone("beds").met_now = beds_held;
        if beds_held {
            run.milestone("beds").value = 21;
        }
        run.short("food", 34, 150);
        run
    };
    // Beds held at the end: 296.
    let held = Scorecard::build(&score, base(true));
    assert_eq!(held.score.round(), 296.0);
    // Two deaths in the evidence run: 30% of whatever it achieved.
    let alive = Scorecard::build(&score, base(false)).score;
    let mut grieving = base(false);
    grieving.deaths = 2;
    near(Scorecard::build(&score, grieving).score, alive * 0.7);
}

#[test]
fn compute_reads_the_worlds_events_deaths_and_care_and_cuts_off_at_the_horizon() {
    let defs = Defs::bundled();
    let score = ScoreDef::bundled();
    let cfg = MapGenConfig { seed: 7, width: 64, height: 64, ..MapGenConfig::default() };
    let mut world = World::new(&cfg, Nation::Water, 5, &defs);
    for _ in 0..300 {
        world.tick(&defs);
    }
    let id = world.units[0].id;
    let died = |tick| Death {
        id,
        name: "Test".to_owned(),
        tick,
        cause: DeathCause::Starvation,
        weakened_by: None,
    };
    world.insert_event_for_test(died(100));
    world.insert_event_for_test(died(250));

    let at_300 = Scorecard::compute(&world, &defs, &score, &ScoreStart::default(), 300);
    assert_eq!(at_300.care.deaths, 2);
    assert!(!at_300.past_horizon);
    assert_eq!(at_300.world_tick, 300);
    assert_eq!(at_300.milestones.len(), 14);
    near(at_300.care.person_minutes, world.care().person_minutes());
    near(at_300.care.suffering, world.care().severe_any_minutes() / world.care().person_minutes());
    assert_eq!(at_300.care.severe_minutes.len(), 4);

    // Scored at an earlier horizon, the later death is ignored and the card says the rest is
    // from a later moment.
    let at_200 = Scorecard::compute(&world, &defs, &score, &ScoreStart::default(), 200);
    assert_eq!(at_200.care.deaths, 1);
    assert!(at_200.past_horizon);
    assert_eq!(
        Scorecard::compute(&world, &defs, &score, &ScoreStart::default(), 300),
        at_300,
        "deterministic"
    );
    // Population is met from the start (5 of the target): sign rows follow `endeavor_signs`.
    let pop = world.endeavor_signs(&defs).into_iter().find(|p| p.id == "population").unwrap();
    assert_eq!(row(&at_300, "population").value, pop.value);
    assert_eq!(row(&at_300, "population").target, pop.target);
}

#[test]
fn compute_ignores_events_after_the_horizon() {
    let defs = Defs::bundled();
    let score = ScoreDef::bundled();
    let cfg = MapGenConfig { seed: 7, width: 64, height: 64, ..MapGenConfig::default() };
    let mut world = World::new(&cfg, Nation::Water, 20, &defs);
    for _ in 0..30 {
        world.tick(&defs);
    }
    let met_at = world
        .endeavor_events()
        .find_map(|e| match &e.kind {
            EndeavorEventKind::SignMet(id) if id == "population" => Some(e.tick),
            _ => None,
        })
        .expect("20 settlers meet the population sign on the first check");
    let after = Scorecard::compute(&world, &defs, &score, &ScoreStart::default(), met_at + 100);
    assert_eq!(row(&after, "population").first_met, Some(met_at));
    let before = Scorecard::compute(&world, &defs, &score, &ScoreStart::default(), met_at - 1);
    assert_eq!(row(&before, "population").first_met, None);
}
