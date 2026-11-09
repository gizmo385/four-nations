use std::collections::BTreeMap;

use fn_sim::{Defs, Nation};

use super::*;
use crate::testkit::config;

const TABLES: &str = "\
    [[target]]\nid = \"stone\"\ndoc = \"materials.md#7-measuring-it\"\nmetric = \"sign_minute.stone\"\nat_most = 120\n\
    [[target]]\nid = \"deaths\"\ndoc = \"harm.md#x\"\nmetric = \"deaths_by.180\"\nat_most = 0\n\
    [noise]\n\"sign_minute.stone\" = 1.0\n";

fn cfg() -> Config {
    config(TABLES, &Defs::bundled())
}

/// A steady/normal entry with stone met at `stone` (`None` for never) and `deaths` deaths.
fn entry(nation: Nation, seed: u64, stone: Option<f64>, deaths: f64) -> Entry {
    Entry {
        style: "steady".to_owned(),
        preset: "normal".to_owned(),
        nation,
        seed,
        minutes: 180.0,
        flags: Vec::new(),
        metrics: BTreeMap::from([
            ("sign_minute.stone".to_owned(), stone.unwrap_or(f64::INFINITY)),
            ("deaths_by.180".to_owned(), deaths),
        ]),
    }
}

/// Seeds 1..=n of one nation, stone at `base + shift(seed)`.
fn set(n: u64, stone: impl Fn(u64) -> f64) -> Vec<Entry> {
    (1..=n).map(|s| entry(Nation::Earth, s, Some(stone(s)), 0.0)).collect()
}

fn row<'a>(c: &'a Comparison, metric: &str) -> &'a Row {
    c.rows.iter().find(|r| r.group.metric == metric).unwrap()
}

#[test]
fn counts_the_median_and_the_moved_rule() {
    // Ten runs: seeds 1-5 two minutes later, the rest unchanged.
    let then = set(10, |_| 100.0);
    let now = set(10, |s| if s <= 5 { 102.0 } else { 100.0 });
    let c = compare(&then, &now, &cfg());
    assert_eq!((c.paired_runs, c.only_then, c.only_now), (10, 0, 0));
    let r = row(&c, "sign_minute.stone");
    assert_eq!((r.paired, r.earlier, r.later, r.same), (10, 0, 5, 5));
    // Sorted deltas: five 0s then five 2s; the middle two are 0 and 2.
    assert_eq!(r.median, 1.0);
    assert_eq!(r.noise, 1.0);
    // The median is not past the noise, however lopsided the signs.
    assert!(!r.moved);

    // Seven of ten two minutes later: the median is 2, past the noise 1, and 7 >= 2 * 1.
    let now = set(10, |s| if s <= 7 { 102.0 } else { 100.0 });
    let c = compare(&then, &now, &cfg());
    let r = row(&c, "sign_minute.stone");
    assert_eq!((r.earlier, r.later, r.same, r.median), (0, 7, 3, 2.0));
    assert!(r.moved);
    assert_eq!(c.moved().count(), 1);
}

#[test]
fn a_median_past_the_noise_needs_twice_as_many_runs_one_way() {
    // Delta 5 on 5 runs, -5 on 4, 0 on 1: the median is 5 but 5 < 2 * 4.
    assert!(!is_moved(5.0, 4, 5, 1.0));
    assert!(is_moved(5.0, 2, 5, 1.0));
    assert!(is_moved(5.0, 0, 1, 1.0));
    // Earlier is a negative median and counts the other way round.
    assert!(is_moved(-5.0, 5, 2, 1.0));
    assert!(!is_moved(-5.0, 5, 3, 1.0));
    // Past the noise means strictly past, and a median of 0 never moves.
    assert!(!is_moved(1.0, 0, 9, 1.0));
    assert!(!is_moved(0.0, 3, 3, 0.0));
    assert!(is_moved(0.5, 0, 9, 0.0), "no noise entry: any movement counts");
}

#[test]
fn the_same_set_has_not_moved() {
    let a = set(8, |s| 90.0 + s as f64);
    let c = compare(&a, &a.clone(), &cfg());
    assert_eq!(c.moved().count(), 0);
    for r in &c.rows {
        assert_eq!((r.earlier, r.later, r.same, r.median), (0, 0, 8, 0.0));
    }
}

#[test]
fn an_event_that_never_happened_is_the_same_as_itself() {
    let then = vec![entry(Nation::Fire, 1, None, 0.0), entry(Nation::Fire, 2, None, 0.0)];
    let now = vec![entry(Nation::Fire, 1, None, 0.0), entry(Nation::Fire, 2, Some(150.0), 0.0)];
    let c = compare(&then, &now, &cfg());
    let r = row(&c, "sign_minute.stone");
    // Never-to-minute-150 is a move earlier, by an infinite amount.
    assert_eq!((r.earlier, r.later, r.same), (1, 0, 1));
    assert_eq!(r.median, f64::NEG_INFINITY);
    assert!(r.moved);
}

#[test]
fn unmatched_and_differently_long_runs_are_not_paired() {
    let then = set(4, |_| 100.0);
    let mut now = set(5, |_| 100.0);
    now[0].minutes = 30.0;
    let c = compare(&then, &now, &cfg());
    assert_eq!(c.paired_runs, 3);
    assert_eq!(c.length_differs, 1);
    assert_eq!(c.only_now, 1);
    assert_eq!(c.only_then, 0, "the short run still matched a key");
    let c = compare(&then[..2], &now, &cfg());
    assert_eq!((c.paired_runs, c.only_now), (1, 3));
    let c = compare(&then, &now[..2], &cfg());
    assert_eq!(c.only_then, 2);
}

#[test]
fn only_the_groups_that_ran_get_a_row_and_pairing_is_by_style_and_preset() {
    let then = set(3, |_| 100.0);
    let mut hard = set(3, |_| 100.0);
    for e in &mut hard {
        e.preset = "hard".to_owned();
        e.metrics.insert("sign_minute.stone".to_owned(), 140.0);
    }
    let mut now = then.clone();
    now.extend(hard); // hard runs have no baseline
    let c = compare(&then, &now, &cfg());
    assert_eq!(c.paired_runs, 3);
    assert_eq!(c.only_now, 3);
    // Both targets are steady/normal, which ran; nothing was asked of steady/hard.
    assert_eq!(c.rows.len(), 2);
    let c = compare(&then, &[], &cfg());
    assert!(c.rows.is_empty());
}

#[test]
fn flags_that_come_or_go_are_counted() {
    let then = set(4, |_| 100.0);
    let mut now = then.clone();
    now[0].flags = vec!["stuck_need".to_owned()];
    now[1].flags = vec!["refused".to_owned()];
    let mut then = then;
    then[1].flags = vec!["refused".to_owned()];
    then[2].flags = vec!["overgrow".to_owned()];
    let c = compare(&then, &now, &cfg());
    assert_eq!((c.newly_flagged, c.unflagged), (1, 1));
}

#[test]
fn the_median_of_even_and_infinite_lists() {
    assert_eq!(median(&[]), 0.0);
    assert_eq!(median(&[3.0, 1.0, 2.0]), 2.0);
    assert_eq!(median(&[4.0, 1.0, 2.0, 3.0]), 2.5);
    assert_eq!(median(&[f64::NEG_INFINITY, f64::INFINITY]), 0.0);
    assert_eq!(median(&[1.0, f64::INFINITY]), f64::INFINITY);
    assert_eq!(median(&[f64::INFINITY, f64::INFINITY]), f64::INFINITY);
}

#[test]
fn groups_come_from_targets_and_pairs_once_each() {
    let defs = Defs::bundled();
    let cfg = Config::bundled(&defs);
    let g = groups(&cfg);
    for (i, a) in g.iter().enumerate() {
        assert!(g[i + 1..].iter().all(|b| a != b), "{a:?} twice");
    }
    assert!(g.contains(&Group {
        metric: "sign_minute.stone".to_owned(),
        style: "steady".to_owned(),
        preset: "normal".to_owned()
    }));
    // A pair's sides are read too.
    assert!(g.iter().any(|x| x.metric == "care.suffering" && x.style == "careless"));
}

#[test]
fn the_stale_note_warns_without_refusing() {
    assert_eq!(stale_note(Some("abc"), "abc"), None);
    let different = stale_note(Some("abc"), "def").unwrap();
    assert!(different.contains("abc") && different.contains("def"), "{different}");
    assert!(different.contains("other data"), "{different}");
    assert!(stale_note(None, "def").unwrap().contains("no data digest"));
}

#[test]
fn the_text_names_the_moved_metrics() {
    let then = set(10, |_| 100.0);
    let now = set(10, |s| if s <= 8 { 105.0 } else { 100.0 });
    let c = compare(&then, &now, &cfg());
    let text = render(&c, "baseline.json", Some("be careful"));
    assert!(text.starts_with("against      baseline.json: 10 runs paired"), "{text}");
    assert!(text.contains("WARNING    be careful"), "{text}");
    let line = text.lines().find(|l| l.contains("sign_minute.stone")).unwrap();
    assert!(
        line.contains("MOVED") && line.contains("later   8") && line.contains("+5.00"),
        "{line}"
    );
    let quiet = text.lines().find(|l| l.contains("deaths_by.180")).unwrap();
    assert!(!quiet.contains("MOVED") && quiet.contains("median       0"), "{quiet}");
    assert!(text.contains("1 of 2 readings moved: sign_minute.stone (steady/normal)"), "{text}");
    assert_eq!(text, render(&c, "baseline.json", Some("be careful")), "pure");
}
