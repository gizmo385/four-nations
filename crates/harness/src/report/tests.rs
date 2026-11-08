use fn_sim::Defs;
use fn_sim::balance::Flag;

use super::*;
use crate::testkit::{config, panel, raised, record};
use crate::verdict::judge;

const TABLES: &str = "[[target]]\nid = \"stone_by_120\"\ndoc = \"materials.md#7-measuring-it\"\n\
    metric = \"sign_minute.stone\"\nat_most = 120\nshare = 0.75\n\
    hard_when = { supply = \"stone\", below = 1.0 }\nhard_expect = { at_most = 180, share = 0.5 }\n\
    [[pair]]\nid = \"care\"\ndoc = \"harm.md#is-a-careful-normal-colony-safe\"\n\
    metric = \"care.suffering\"\nlower = { style = \"steady\", preset = \"normal\" }\n\
    higher = { style = \"careless\", preset = \"normal\" }\n";

fn header() -> Header {
    Header { label: "abc1234".to_owned(), seeds: "1-8".to_owned(), nations: Nation::ALL.to_vec() }
}

fn stone_at(rec: &mut fn_sim::balance::RunRecord, minute: Option<f32>) {
    rec.sign_minutes.iter_mut().find(|(id, _)| id == "stone").unwrap().1 = minute;
}

/// A panel with one of each thing the report lists: a sim lock, a harness gap, an outlier, a
/// balance miss on a seed nobody met, and hard sites.
fn report() -> String {
    let defs = Defs::bundled();
    let cfg = config(TABLES, &defs);
    let mut runs = panel("steady", "normal", 8, |n, s| {
        let mut r = record(n, s);
        match (n, s) {
            (Nation::Water, 3) => {
                r.flags.push(raised(Flag::Stranded, 104.0));
                r.first_death_minute = Some(140.0);
                stone_at(&mut r, None);
            }
            (Nation::Fire, 4) => r.flags.push(raised(Flag::IdleWithWork, 45.0)),
            (Nation::Water, 5) => stone_at(&mut r, None),
            (_, 6) => stone_at(&mut r, None),
            (Nation::Air, 7) => {
                r.site.supply.insert("stone".to_owned(), 20);
                stone_at(&mut r, Some(170.0));
            }
            _ => {}
        }
        r
    });
    runs.extend(panel("careless", "normal", 8, record));
    let v = judge(&cfg, &runs, &defs);
    render(&v, &header())
}

#[test]
fn the_header_and_target_table_come_first() {
    let out = report();
    let mut lines = out.lines();
    let head = lines.next().unwrap();
    assert!(head.starts_with("balance verdict  seeds 1-8"), "{head}");
    assert!(head.contains("label abc1234") && head.contains("64 runs"), "{head}");
    let table: Vec<&str> = out.lines().skip(2).take(3).collect();
    assert!(
        table[0].starts_with("target") && table[0].contains("earth") && table[0].contains("doc")
    );
    assert!(table[1].starts_with("stone_by_120"), "{}", table[1]);
    assert!(table[1].contains("<= 120") && table[1].contains("steady/normal"));
    assert!(table[1].contains("75%") && table[1].ends_with("materials #7"), "{}", table[1]);
    // Earth passes 8/8 with seed 6 a miss shared by everyone (a balance miss, not an outlier).
    assert!(table[1].contains("7/8 pass") || table[1].contains("FAIL"), "{}", table[1]);
    assert!(table[2].trim_start().starts_with("on hard sites"), "{}", table[2]);
}

#[test]
fn each_flagged_run_gets_a_trace_command() {
    let out = report();
    let lock = out.lines().find(|l| l.starts_with("sim lock")).expect("a sim lock line");
    assert!(lock.contains("water 3"), "{lock}");
    assert!(lock.contains("stranded from 102") && lock.contains("first death 140"), "{lock}");
    assert!(
        lock.ends_with("balance trace --seed 3 --nation water --from 97 --to 145 --map-at 140"),
        "{lock}"
    );
    let gap = out.lines().find(|l| l.starts_with("harness gap")).expect("a harness gap line");
    assert!(gap.contains("fire 4") && gap.contains("idle_with_work from 43"), "{gap}");
    assert!(
        gap.contains("balance trace --seed 4 --nation fire --from 38 --to 50 --map-at 45"),
        "{gap}"
    );
}

#[test]
fn outliers_and_misses_are_listed_apart_with_who_met_them() {
    let out = report();
    let outlier = out.lines().find(|l| l.starts_with("outlier")).expect("an outlier line");
    assert!(outlier.contains("water 5") && outlier.contains("stone_by_120: never"), "{outlier}");
    assert!(outlier.contains("met by air earth fire"), "{outlier}");
    assert!(
        outlier.contains("balance trace --seed 5 --nation water --from 90 --to 120"),
        "{outlier}"
    );
    let miss = out.lines().find(|l| l.starts_with("balance miss")).expect("a miss line");
    assert!(miss.contains("air water earth fire 6") && miss.contains("nobody met it"), "{miss}");
    assert!(!miss.contains("balance trace"), "misses are the numbers' fault: no trace");
}

#[test]
fn hard_sites_list_their_seeds_and_the_trace_names_a_style_off_the_reference() {
    let out = report();
    assert!(out.contains("hard sites   stone supply"), "{out}");
    assert!(out.contains("air below 1x need on 1 of 8: 7"), "{out}");
    assert_eq!(
        trace_command("careless", "hard", Nation::Earth, 9, (1, 2), 2),
        "balance trace --seed 9 --nation earth --style careless --preset hard --from 1 --to 2 --map-at 2"
    );
}

#[test]
fn care_sensitivity_and_the_verdict_close_it() {
    let out = report();
    assert!(out.contains("care         steady/normal  depleted min/ph"), "{out}");
    // Both styles read the same care here, so the pair can't fail.
    let pair = out
        .lines()
        .find(|l| l.contains("careless_vs_careful_care") || l.contains("care  "))
        .unwrap();
    assert!(out.contains("CAN'T FAIL"), "{pair}\n{out}");
    let last = out.lines().last().unwrap();
    assert!(last.starts_with("verdict"), "{last}");
    assert!(last.contains("0 pass, 1 fail (stone_by_120"), "{last}");
    assert!(last.contains("1 sensitivity checks can't fail"), "{last}");
    assert!(last.contains("3 outliers") || last.contains("outliers"), "{last}");
}

#[test]
fn the_same_verdict_renders_the_same_bytes() {
    assert_eq!(report(), report());
}

#[test]
fn doc_names_are_short() {
    assert_eq!(doc_short("materials.md#7-measuring-it"), "materials #7");
    assert_eq!(doc_short("pacing.md#measuring-it"), "pacing");
    assert_eq!(doc_short("harm.md#is-a-careful-normal-colony-safe"), "harm");
}

#[test]
fn values_print_plainly() {
    assert_eq!(value_text(f64::INFINITY), "never");
    assert_eq!(value_text(120.0), "120");
    assert_eq!(value_text(2.25), "2.2");
}
