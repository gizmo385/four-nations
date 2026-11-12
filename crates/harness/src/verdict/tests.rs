use fn_sim::balance::Flag;

use super::*;
use crate::testkit::{config, panel, raised, record};

const STONE: &str = "[[target]]\nid = \"stone\"\ndoc = \"materials.md#7-measuring-it\"\n\
    metric = \"sign_minute.stone\"\nat_most = 120\nshare = 0.75\n";

const STONE_HARD: &str = "[[target]]\nid = \"stone\"\ndoc = \"materials.md#7-measuring-it\"\n\
    metric = \"sign_minute.stone\"\nat_most = 120\nshare = 0.75\n\
    hard_when = { supply = \"stone\", below = 1.0 }\n\
    hard_expect = { at_most = 180, share = 0.5 }\n";

fn stone_at(rec: &mut fn_sim::balance::RunRecord, minute: Option<f32>) {
    rec.sign_minutes.iter_mut().find(|(id, _)| id == "stone").expect("stone sign").1 = minute;
}

fn target_of<'a>(v: &'a Verdict, id: &str) -> &'a TargetVerdict {
    v.targets.iter().find(|t| t.target.id == id).expect("target")
}

fn earth(t: &TargetVerdict) -> &NationVerdict {
    t.nations.iter().find(|n| n.nation == Nation::Earth).expect("earth ran")
}

fn bucket_of(t: &TargetVerdict, nation: Nation, seed: u64) -> Bucket {
    t.runs.iter().find(|c| c.nation == nation && c.seed == seed).expect("classified").bucket
}

#[test]
fn a_clean_panel_passes_every_nation() {
    let defs = Defs::bundled();
    let cfg = config(STONE, &defs);
    let runs = panel("steady", "normal", 8, record);
    let v = judge(&cfg, &runs, &defs);
    let t = target_of(&v, "stone");
    assert_eq!(t.status, Status::Pass);
    assert_eq!(t.nations.len(), 4);
    assert!(t.nations.iter().all(|n| n.counted == 8 && n.passes == 8));
    assert!(!v.failed());
    assert!(v.flagged.is_empty());
}

#[test]
fn share_decides_between_pass_and_fail() {
    let defs = Defs::bundled();
    let cfg = config(STONE, &defs);
    // Earth: 6 of 8 meet it is 75%, which is the share; 5 of 8 is below it.
    let build = |misses: u64| {
        panel("steady", "normal", 8, move |n, s| {
            let mut r = record(n, s);
            if n == Nation::Earth && s <= misses {
                stone_at(&mut r, Some(150.0));
            }
            r
        })
    };
    let ok = judge(&cfg, &build(2), &defs);
    assert_eq!(earth(target_of(&ok, "stone")).status, Status::Pass);
    let bad = judge(&cfg, &build(3), &defs);
    let t = target_of(&bad, "stone");
    assert_eq!(earth(t).status, Status::Fail);
    assert_eq!(earth(t).passes, 5);
    assert_eq!(t.status, Status::Fail);
    assert_eq!(t.failing_nations(), vec![Nation::Earth]);
    assert!(bad.failed());
}

#[test]
fn too_few_counted_runs_is_no_data() {
    let defs = Defs::bundled();
    let cfg = config(STONE, &defs);
    let runs = panel("steady", "normal", 3, record);
    let v = judge(&cfg, &runs, &defs);
    let t = target_of(&v, "stone");
    assert_eq!(t.status, Status::NoData);
    assert_eq!(earth(t).counted, 3);
    assert!(!v.failed(), "no data is not a failure");
    // A target whose run was never made has no nations at all.
    let none = judge(&cfg, &[], &defs);
    assert_eq!(target_of(&none, "stone").status, Status::NoData);
}

#[test]
fn a_sim_lock_before_the_deadline_is_excluded_not_counted() {
    let defs = Defs::bundled();
    let cfg = config(STONE, &defs);
    let runs = panel("steady", "normal", 8, |n, s| {
        let mut r = record(n, s);
        if n == Nation::Water && s == 3 {
            stone_at(&mut r, None);
            r.flags.push(raised(Flag::Stranded, 104.0));
        }
        r
    });
    let v = judge(&cfg, &runs, &defs);
    let t = target_of(&v, "stone");
    assert_eq!(bucket_of(t, Nation::Water, 3), Bucket::SimLock);
    let water = t.nations.iter().find(|n| n.nation == Nation::Water).unwrap();
    assert_eq!((water.counted, water.passes, water.sim_locks), (7, 7, 1));
    assert_eq!(v.flagged.len(), 1);
    assert_eq!(v.flagged[0].bucket, Bucket::SimLock);
    assert_eq!(v.flagged[0].seed, 3);
}

#[test]
fn a_flag_after_the_deadline_leaves_the_run_counted() {
    let defs = Defs::bundled();
    let cfg = config(STONE, &defs);
    let runs = panel("steady", "normal", 8, |n, s| {
        let mut r = record(n, s);
        if n == Nation::Fire && s == 1 {
            r.flags.push(raised(Flag::Stranded, 121.0));
        }
        r
    });
    let v = judge(&cfg, &runs, &defs);
    let t = target_of(&v, "stone");
    assert_eq!(bucket_of(t, Nation::Fire, 1), Bucket::Balance);
    assert!(v.flagged.is_empty());
}

#[test]
fn a_harness_gap_is_excluded_and_a_sim_lock_outranks_it() {
    let defs = Defs::bundled();
    let cfg = config(STONE, &defs);
    let runs = panel("steady", "normal", 8, |n, s| {
        let mut r = record(n, s);
        if n == Nation::Air && s == 2 {
            r.flags.push(raised(Flag::ResearchIdle, 50.0));
        }
        if n == Nation::Air && s == 3 {
            r.flags.push(raised(Flag::ResearchIdle, 50.0));
            r.flags.push(raised(Flag::Stranded, 60.0));
        }
        r
    });
    let v = judge(&cfg, &runs, &defs);
    let t = target_of(&v, "stone");
    assert_eq!(bucket_of(t, Nation::Air, 2), Bucket::HarnessGap);
    assert_eq!(bucket_of(t, Nation::Air, 3), Bucket::SimLock);
    let c = t.runs.iter().find(|c| c.nation == Nation::Air && c.seed == 3).unwrap();
    assert_eq!(c.flags.len(), 1, "the sim lock bucket lists only the sim's flags");
    let air = t.nations.iter().find(|n| n.nation == Nation::Air).unwrap();
    assert_eq!((air.counted, air.sim_locks, air.harness_gaps), (6, 1, 1));
}

#[test]
fn a_flagged_run_that_passes_still_does_not_count() {
    let defs = Defs::bundled();
    let cfg = config(STONE, &defs);
    // Earth: 6 pass, 2 miss. Flag both misses and the target passes on the 6 alone; flag two
    // passes instead and the 4 left are 2 of 6 missing.
    let runs = |flag_misses: bool| {
        panel("steady", "normal", 8, move |n, s| {
            let mut r = record(n, s);
            if n == Nation::Earth && s <= 2 {
                stone_at(&mut r, Some(150.0));
                if flag_misses {
                    r.flags.push(raised(Flag::StoresFrozen, 15.0));
                }
            }
            if n == Nation::Earth && s >= 7 && !flag_misses {
                r.flags.push(raised(Flag::StoresFrozen, 15.0));
            }
            r
        })
    };
    let hidden = judge(&cfg, &runs(true), &defs);
    assert_eq!(earth(target_of(&hidden, "stone")).status, Status::Pass);
    assert_eq!(hidden.flagged.len(), 2, "excluded runs are always listed");
    let shown = judge(&cfg, &runs(false), &defs);
    let e = earth(target_of(&shown, "stone")).clone();
    assert_eq!((e.counted, e.passes, e.status), (6, 4, Status::Fail));
}

#[test]
fn a_style_with_no_player_has_no_harness_gaps() {
    let defs = Defs::bundled();
    let idle = "[[target]]\nid = \"first\"\ndoc = \"difficulty.md#measuring-it\"\nstyle = \"idle\"\n\
        metric = \"first_death\"\nat_least = 45\nat_most = 120\nshare = 0.75\n";
    let cfg = config(idle, &defs);
    let runs = panel("idle", "normal", 8, |n, s| {
        let mut r = record(n, s);
        r.first_death_minute = Some(90.0);
        // `research_idle` fires in every no-input run: nobody is there to research.
        r.flags.push(raised(Flag::ResearchIdle, 10.0));
        if n == Nation::Earth && s == 1 {
            r.flags.push(raised(Flag::StuckNeed, 20.0));
        }
        r
    });
    let v = judge(&cfg, &runs, &defs);
    let t = target_of(&v, "first");
    assert_eq!(t.status, Status::Pass);
    assert_eq!(bucket_of(t, Nation::Fire, 1), Bucket::Balance);
    assert_eq!(bucket_of(t, Nation::Earth, 1), Bucket::SimLock, "the sim's flags still apply");
}

#[test]
fn a_hard_site_is_judged_against_the_softer_bar_and_counted_apart() {
    let defs = Defs::bundled();
    let cfg = config(STONE_HARD, &defs);
    let runs = panel("steady", "normal", 8, |n, s| {
        let mut r = record(n, s);
        if n == Nation::Earth && s <= 4 {
            // Hard: well under the stone sign's need. Half meet the soft bar of 180.
            r.site.supply.insert("stone".to_owned(), 10);
            stone_at(&mut r, if s <= 2 { Some(170.0) } else { None });
        }
        r
    });
    let v = judge(&cfg, &runs, &defs);
    let t = target_of(&v, "stone");
    let e = earth(t);
    assert_eq!((e.hard, e.hard_passes), (4, 2));
    assert_eq!(e.hard_status, Some(Status::Pass), "2 of 4 is the hard share, 50%");
    // They never count toward the normal share: 4 feasible runs left, all passing.
    assert_eq!((e.counted, e.passes), (4, 4));
    assert_eq!(bucket_of(t, Nation::Earth, 1), Bucket::HardSite);
    assert!(t.runs.iter().find(|c| c.nation == Nation::Earth && c.seed == 1).unwrap().met);
    assert_eq!(t.hard_status, Some(Status::NoData), "the other nations have no hard sites");
    let sites = &v.hard_sites[0];
    assert_eq!(sites.supply, "stone");
    let (_, seeds, total) = sites.nations.iter().find(|(n, _, _)| *n == Nation::Earth).unwrap();
    assert_eq!((seeds.clone(), *total), (vec![1, 2, 3, 4], 8));
}

#[test]
fn a_flag_outranks_the_hard_site_bucket() {
    let defs = Defs::bundled();
    let cfg = config(STONE_HARD, &defs);
    let runs = panel("steady", "normal", 8, |n, s| {
        let mut r = record(n, s);
        if n == Nation::Earth && s == 1 {
            r.site.supply.insert("stone".to_owned(), 10);
            r.flags.push(raised(Flag::StoresFrozen, 15.0));
        }
        r
    });
    let v = judge(&cfg, &runs, &defs);
    assert_eq!(bucket_of(target_of(&v, "stone"), Nation::Earth, 1), Bucket::HarnessGap);
}

#[test]
fn a_miss_the_other_nations_did_not_share_is_an_outlier() {
    let defs = Defs::bundled();
    let cfg = config(STONE, &defs);
    let runs = panel("steady", "normal", 8, |n, s| {
        let mut r = record(n, s);
        // Seed 18-style: water misses where the other three meet it.
        if n == Nation::Water && s == 5 {
            stone_at(&mut r, None);
        }
        // Seed 6: water and fire miss, so only two others (earth, air) met it: still an outlier.
        // Seed 7: three nations miss, one other met it: a balance miss.
        if s == 6 && matches!(n, Nation::Water | Nation::Fire) {
            stone_at(&mut r, None);
        }
        if s == 7 && n != Nation::Earth {
            stone_at(&mut r, None);
        }
        r
    });
    let v = judge(&cfg, &runs, &defs);
    let t = target_of(&v, "stone");
    assert_eq!(bucket_of(t, Nation::Water, 5), Bucket::Outlier);
    let c = t.runs.iter().find(|c| c.nation == Nation::Water && c.seed == 5).unwrap();
    assert!(!c.met);
    assert_eq!(c.met_by.len(), 3);
    assert_eq!(bucket_of(t, Nation::Water, 6), Bucket::Outlier);
    assert_eq!(bucket_of(t, Nation::Fire, 6), Bucket::Outlier);
    assert_eq!(bucket_of(t, Nation::Water, 7), Bucket::Balance);
    assert_eq!(bucket_of(t, Nation::Earth, 7), Bucket::Balance, "a pass is a balance run");
    assert_eq!(v.outliers().len(), 3);
    // An outlier counts as a miss: water is at 5 of 8 here, under the 75% share.
    let water = t.nations.iter().find(|n| n.nation == Nation::Water).unwrap();
    assert_eq!((water.counted, water.passes, water.outliers), (8, 5, 2));
}

#[test]
fn a_flagged_run_is_never_an_outlier() {
    let defs = Defs::bundled();
    let cfg = config(STONE, &defs);
    let runs = panel("steady", "normal", 8, |n, s| {
        let mut r = record(n, s);
        if n == Nation::Water && s == 5 {
            stone_at(&mut r, None);
            r.flags.push(raised(Flag::StoresFrozen, 15.0));
        }
        r
    });
    let v = judge(&cfg, &runs, &defs);
    assert_eq!(bucket_of(target_of(&v, "stone"), Nation::Water, 5), Bucket::HarnessGap);
    assert!(v.outliers().is_empty());
}

#[test]
fn a_run_too_short_for_the_reading_has_no_data() {
    let defs = Defs::bundled();
    let cfg = config(
        "[[target]]\nid = \"d\"\ndoc = \"harm.md#is-a-careful-normal-colony-safe\"\n\
         metric = \"deaths_by.180\"\nat_most = 0\n",
        &defs,
    );
    let runs = panel("steady", "normal", 8, |n, s| {
        let mut r = record(n, s);
        r.minutes = 60.0;
        r
    });
    let v = judge(&cfg, &runs, &defs);
    let t = target_of(&v, "d");
    assert!(t.runs.is_empty());
    assert_eq!(t.status, Status::NoData);
}

#[test]
fn a_sensitivity_pair_that_separates_is_fine() {
    let defs = Defs::bundled();
    let pair = "[[pair]]\nid = \"care\"\ndoc = \"harm.md#is-a-careful-normal-colony-safe\"\n\
        metric = \"care.suffering\"\nlower = { style = \"steady\", preset = \"normal\" }\n\
        higher = { style = \"careless\", preset = \"normal\" }\n";
    let cfg = config(pair, &defs);
    let mut runs = panel("steady", "normal", 8, record);
    runs.extend(panel("careless", "normal", 8, |n, s| {
        let mut r = record(n, s);
        r.care.severe_any_minutes = 90.0;
        r
    }));
    let v = judge(&cfg, &runs, &defs);
    let p = &v.pairs[0];
    assert_eq!((p.paired, p.separated, p.status), (32, 32, PairStatus::Separates));
    assert!((p.mean_higher - 0.05).abs() < 1e-9 && p.mean_lower == 0.0);
}

#[test]
fn a_sensitivity_pair_that_does_not_separate_cant_fail() {
    let defs = Defs::bundled();
    let pair = "[[pair]]\nid = \"care\"\ndoc = \"harm.md#is-a-careful-normal-colony-safe\"\n\
        metric = \"care.close_calls_per_10ph\"\nlower = { style = \"steady\", preset = \"normal\" }\n\
        higher = { style = \"careless\", preset = \"normal\" }\n";
    let cfg = config(pair, &defs);
    // Both colonies read the same everywhere but on 2 of 32 paired runs: 6%, under the 25% share.
    let mut runs = panel("steady", "normal", 8, record);
    runs.extend(panel("careless", "normal", 8, |n, s| {
        let mut r = record(n, s);
        if n == Nation::Earth && s <= 2 {
            r.care.close_calls = 1;
        }
        r
    }));
    let v = judge(&cfg, &runs, &defs);
    assert_eq!(v.pairs[0].status, PairStatus::CantFail);
    assert_eq!((v.pairs[0].paired, v.pairs[0].separated), (32, 2));

    // One side missing entirely: no data, not "can't fail".
    let only_steady: Vec<Run> = runs.iter().filter(|r| r.style == "steady").cloned().collect();
    assert_eq!(judge(&cfg, &only_steady, &defs).pairs[0].status, PairStatus::NoData);
}

#[test]
fn a_flagged_run_does_not_pair() {
    let defs = Defs::bundled();
    let pair = "[[pair]]\nid = \"care\"\ndoc = \"harm.md#is-a-careful-normal-colony-safe\"\n\
        metric = \"care.suffering\"\nlower = { style = \"steady\", preset = \"normal\" }\n\
        higher = { style = \"careless\", preset = \"normal\" }\n";
    let cfg = config(pair, &defs);
    let mut runs = panel("steady", "normal", 8, record);
    runs.extend(panel("careless", "normal", 8, |n, s| {
        let mut r = record(n, s);
        r.care.severe_any_minutes = 90.0;
        if n == Nation::Earth {
            r.flags.push(raised(Flag::Stranded, 30.0));
        }
        r
    }));
    let v = judge(&cfg, &runs, &defs);
    assert_eq!(v.pairs[0].paired, 24);
}

#[test]
fn care_rates_pool_each_nations_runs() {
    let defs = Defs::bundled();
    let cfg = config(STONE, &defs);
    let runs = panel("steady", "normal", 2, |n, s| {
        let mut r = record(n, s);
        if n == Nation::Water {
            r.care.depleted_minutes = [15.0, 0.0, 0.0];
            r.care.close_calls = 1;
        }
        r
    });
    let v = judge(&cfg, &runs, &defs);
    let row = &v.care[0];
    let water = row.nations.iter().find(|n| n.0 == Nation::Water).unwrap();
    // 30 depleted minutes over 3600 person-minutes (60 person-hours); 2 close calls.
    assert!((water.1 - 0.5).abs() < 1e-9, "{water:?}");
    assert!((water.2 - 2.0 * 10.0 / 60.0).abs() < 1e-9);
    let earth = row.nations.iter().find(|n| n.0 == Nation::Earth).unwrap();
    assert_eq!((earth.1, earth.2), (0.0, 0.0));
}

#[test]
fn runs_of_another_style_or_preset_are_not_this_targets() {
    let defs = Defs::bundled();
    let cfg = config(STONE, &defs);
    let mut runs = panel("careless", "normal", 8, record);
    runs.extend(panel("steady", "hard", 8, record));
    let v = judge(&cfg, &runs, &defs);
    assert!(target_of(&v, "stone").runs.is_empty());
}

#[test]
fn a_target_limited_to_some_nations_leaves_the_rest_out() {
    let defs = Defs::bundled();
    let cfg = config(&format!("{STONE}nations = [\"earth\", \"fire\"]\n"), &defs);
    let runs = panel("steady", "normal", 8, record);
    let v = judge(&cfg, &runs, &defs);
    let nations: Vec<Nation> = target_of(&v, "stone").nations.iter().map(|n| n.nation).collect();
    assert_eq!(nations, vec![Nation::Earth, Nation::Fire]);
}

// horizons-phase-4.md, "Measuring it": the `[[fork]]` check (#731).
const FORK: &str = "[[fork]]\nid = \"both\"\ndoc = \"horizons-phase-4.md#measuring-it\"\n\
    metric = \"stage_minute.3\"\na = { style = \"steady\", preset = \"normal\" }\n\
    b = { style = \"careless\", preset = \"normal\" }\ntie = 5.0\nmax_share = 0.75\n";

fn arch_at(nation: Nation, seed: u64, minute: Option<f32>) -> fn_sim::balance::RunRecord {
    let mut r = record(nation, seed);
    r.stage_minutes = ["foundation", "pillars", "arch", "rite"]
        .iter()
        .map(|s| (s.to_string(), if *s == "arch" { minute } else { None }))
        .collect();
    r
}

/// Earth's `a` side finishes the arch at `a_minute(seed)`, everyone else and `b` at 200.
fn fork_panel(a_minute: impl Fn(u64) -> Option<f32>) -> Vec<Run> {
    let mut runs = panel("steady", "normal", 8, |n, s| {
        arch_at(n, s, if n == Nation::Earth { a_minute(s) } else { Some(200.0) })
    });
    runs.extend(panel("careless", "normal", 8, |n, s| arch_at(n, s, Some(200.0))));
    runs
}

fn fork_earth(v: &Verdict) -> &ForkNation {
    v.forks[0].nations.iter().find(|n| n.nation == Nation::Earth).expect("earth judged")
}

#[test]
fn a_fork_passes_while_neither_side_wins_too_often() {
    let defs = Defs::bundled();
    let cfg = config(FORK, &defs);
    // Earth's `a` wins 4 seeds, loses 2 and ties 2 (within 5 minutes).
    let minute = |s: u64| {
        Some(match s {
            1..=4 => 150.0,
            5 | 6 => 250.0,
            _ => 203.0,
        })
    };
    let v = judge(&cfg, &fork_panel(minute), &defs);
    let e = fork_earth(&v);
    assert_eq!((e.paired, e.a_wins, e.b_wins, e.ties), (8, 4, 2, 2));
    assert_eq!(e.status, Status::Pass);
    assert!(!v.failed());
}

#[test]
fn a_fork_fails_when_one_side_wins_more_than_its_share() {
    let defs = Defs::bundled();
    let cfg = config(FORK, &defs);
    // 7 of 8 is past 75%.
    let v = judge(&cfg, &fork_panel(|s| Some(if s <= 7 { 150.0 } else { 200.0 })), &defs);
    assert_eq!(fork_earth(&v).a_wins, 7);
    assert_eq!(fork_earth(&v).status, Status::Fail);
    assert_eq!(v.forks[0].status, Status::Fail);
    assert!(v.failed());
}

#[test]
fn a_fork_neither_side_reaches_is_no_data() {
    let defs = Defs::bundled();
    let cfg = config(FORK, &defs);
    let mut runs = panel("steady", "normal", 8, |n, s| arch_at(n, s, None));
    runs.extend(panel("careless", "normal", 8, |n, s| arch_at(n, s, None)));
    let v = judge(&cfg, &runs, &defs);
    let e = fork_earth(&v);
    assert_eq!((e.paired, e.ties), (8, 8), "two runs that never got there tie");
    assert_eq!(e.status, Status::NoData);
    // One side getting there is a win, though: seed 8's `a` never does, so `b` wins it.
    let v = judge(&cfg, &fork_panel(|s| (s <= 7).then_some(150.0)), &defs);
    assert_eq!((fork_earth(&v).a_wins, fork_earth(&v).b_wins), (7, 1));
}
