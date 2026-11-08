use super::*;

/// A calm minute: six settlers, all busy, nothing wrong.
fn calm(minute: u32) -> Sample {
    Sample { minute, settlers: 6, stores: u64::from(minute), ..Sample::default() }
}

/// Feeds `watch` minutes 0..`minutes`, each built by `shape`.
fn feed(watch: &mut Watch, minutes: u32, shape: impl Fn(&mut Sample)) {
    for minute in 0..minutes {
        let mut s = calm(minute);
        shape(&mut s);
        watch.observe(&s);
    }
}

fn run(minutes: u32, shape: impl Fn(&mut Sample)) -> Watch {
    let mut watch = Watch::new(Detectors::default());
    feed(&mut watch, minutes, shape);
    watch
}

fn raised(watch: &Watch, flag: Flag) -> Option<(f32, f32)> {
    watch.first(flag).map(|r| (r.minute, r.since))
}

#[test]
fn a_calm_run_raises_nothing() {
    assert_eq!(run(200, |_| {}).raised(), Vec::new());
}

#[test]
fn stranded_fires_after_three_minutes_running() {
    // Stranded from minute 10: the third minute running is 12, and it began at 10.
    let w = run(30, |s| s.longest_stranded = s.minute.saturating_sub(9));
    assert_eq!(raised(&w, Flag::Stranded), Some((12.0, 10.0)));
}

#[test]
fn stranded_stays_quiet_below_three_minutes() {
    let w = run(30, |s| s.longest_stranded = if s.minute % 3 == 0 { 2 } else { 0 });
    assert_eq!(raised(&w, Flag::Stranded), None);
}

#[test]
fn stuck_need_fires_after_three_minutes_running() {
    let w = run(30, |s| s.longest_stuck = s.minute.saturating_sub(4));
    assert_eq!(raised(&w, Flag::StuckNeed), Some((7.0, 5.0)));
}

#[test]
fn stuck_need_stays_quiet_below_three_minutes() {
    assert_eq!(raised(&run(30, |s| s.longest_stuck = 2), Flag::StuckNeed), None);
}

fn idle_with_work(s: &mut Sample) {
    s.idle = 4;
    s.material_unmet = true;
}

#[test]
fn idle_with_work_fires_after_half_an_hour_mostly_idle() {
    // Four of six is over 60%: the thirtieth minute running is 29 since minute 0.
    let w = run(60, idle_with_work);
    assert_eq!(raised(&w, Flag::IdleWithWork), Some((29.0, 0.0)));
}

#[test]
fn idle_with_work_needs_the_material_unmet_and_in_reach() {
    let w = run(60, |s| {
        idle_with_work(s);
        s.material_unmet = false;
    });
    assert_eq!(raised(&w, Flag::IdleWithWork), None);
}

#[test]
fn idle_with_work_needs_most_of_the_settlement_idle() {
    // Three of six is 50%, under the 60% line.
    let w = run(60, |s| {
        idle_with_work(s);
        s.idle = 3;
    });
    assert_eq!(raised(&w, Flag::IdleWithWork), None);
}

#[test]
fn idle_with_work_restarts_when_work_resumes() {
    // Idle for 29 minutes, one busy minute, then 29 more: never 30 running.
    let w = run(59, |s| {
        if s.minute != 29 {
            idle_with_work(s);
        }
    });
    assert_eq!(raised(&w, Flag::IdleWithWork), None);
}

#[test]
fn stores_frozen_fires_after_half_an_hour_unchanged() {
    // Stores last changed at minute 9 (the fingerprint is the minute until then), so they have
    // been the same since 9, and the thirtieth unchanged comparison is minute 39.
    let w = run(60, |s| {
        s.material_unmet = true;
        s.stores = u64::from(s.minute.min(9));
    });
    assert_eq!(raised(&w, Flag::StoresFrozen), Some((39.0, 9.0)));
}

#[test]
fn stores_frozen_stays_quiet_while_stores_move() {
    // Changes every 20 minutes, so never 30 unchanged.
    let w = run(120, |s| {
        s.material_unmet = true;
        s.stores = u64::from(s.minute / 20);
    });
    assert_eq!(raised(&w, Flag::StoresFrozen), None);
}

#[test]
fn stores_frozen_stays_quiet_when_no_sign_needs_the_material() {
    let w = run(120, |s| s.stores = 7);
    assert_eq!(raised(&w, Flag::StoresFrozen), None);
}

#[test]
fn research_idle_fires_after_ten_minutes_with_a_tech_available() {
    let w = run(30, |s| s.research_waiting = s.minute >= 5);
    assert_eq!(raised(&w, Flag::ResearchIdle), Some((14.0, 5.0)));
}

#[test]
fn research_idle_stays_quiet_when_something_is_queued_in_time() {
    // Nothing set for nine minutes at a time, never ten.
    let w = run(100, |s| s.research_waiting = s.minute % 10 != 9);
    assert_eq!(raised(&w, Flag::ResearchIdle), None);
}

#[test]
fn chi_idle_fires_once_half_the_time_since_the_first_bend_is_idle() {
    // Bending is known from minute 10; from then on a bender is always at full chi with nothing
    // designated. Judged after 20 minutes of bending: minute 29.
    let w = run(60, |s| {
        s.bending_known = s.minute >= 10;
        s.chi_idle = s.bending_known;
    });
    assert_eq!(raised(&w, Flag::ChiIdle), Some((29.0, 10.0)));
}

#[test]
fn chi_idle_stays_quiet_for_a_bender_that_is_kept_busy() {
    // Idle one minute in four.
    let w = run(100, |s| {
        s.bending_known = true;
        s.chi_idle = s.minute % 4 == 0;
    });
    assert_eq!(raised(&w, Flag::ChiIdle), None);
}

#[test]
fn chi_idle_stays_quiet_before_any_bend_is_known() {
    let w = run(100, |s| s.chi_idle = true);
    assert_eq!(raised(&w, Flag::ChiIdle), None);
}

#[test]
fn refused_fires_at_twenty_in_a_row() {
    let w = run(10, |s| s.refused_streak = if s.minute >= 4 { 20 } else { 19 });
    assert_eq!(raised(&w, Flag::Refused), Some((4.0, 4.0)));
}

#[test]
fn refused_stays_quiet_below_twenty() {
    assert_eq!(raised(&run(10, |s| s.refused_streak = 19), Flag::Refused), None);
}

fn crowded(s: &mut Sample) {
    s.population_target = 10;
    s.population = 13;
}

#[test]
fn overgrow_fires_when_over_the_population_sign_with_food_unmet() {
    assert_eq!(raised(&run(3, crowded), Flag::Overgrow), Some((0.0, 0.0)));
}

#[test]
fn overgrow_stays_quiet_when_food_is_met_or_the_excess_is_small() {
    let fed = run(3, |s| {
        crowded(s);
        s.food_met = true;
    });
    assert_eq!(raised(&fed, Flag::Overgrow), None);
    let snug = run(3, |s| {
        crowded(s);
        s.population = 12; // 20% over, under the 25% line
    });
    assert_eq!(raised(&snug, Flag::Overgrow), None);
    // No population sign in the data: nothing to be over.
    assert_eq!(raised(&run(3, |s| s.population = 50), Flag::Overgrow), None);
}

#[test]
fn a_flag_keeps_its_first_minute() {
    let w = run(100, |s| s.refused_streak = 25);
    assert_eq!(raised(&w, Flag::Refused), Some((0.0, 0.0)));
    assert_eq!(w.raised().len(), 1);
}

#[test]
fn flags_are_classed_sim_lock_or_harness_gap() {
    let locks: Vec<Flag> =
        Flag::ALL.into_iter().filter(|f| f.class() == FlagClass::SimLock).collect();
    assert_eq!(locks, [Flag::Stranded, Flag::StuckNeed]);
}
