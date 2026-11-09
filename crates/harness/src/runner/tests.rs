use fn_sim::Defs;

use super::*;
use crate::testkit::config;

const TABLES: &str = "\
    [[target]]\nid = \"a\"\ndoc = \"materials.md#7-measuring-it\"\nmetric = \"sign_minute.stone\"\nat_most = 120\n\
    [[target]]\nid = \"b\"\ndoc = \"materials.md#7-measuring-it\"\nmetric = \"sign_minute.ore\"\nat_most = 120\nminutes = 240\n\
    [[target]]\nid = \"c\"\ndoc = \"harm.md#x\"\npreset = \"hard\"\nmetric = \"deaths\"\nat_most = 2\nfirst_seeds = 2\n\
    [[target]]\nid = \"d\"\ndoc = \"harm.md#x\"\nstyle = \"careless\"\nmetric = \"deaths\"\nat_least = 1\n\
    [[pair]]\nid = \"p\"\ndoc = \"harm.md#x\"\nmetric = \"deaths\"\n\
    lower = { style = \"steady\", preset = \"normal\" }\nhigher = { style = \"steady\", preset = \"hard\" }\nfirst_seeds = 3\n";

fn plan_for(filter: &Filter) -> Vec<Job> {
    let defs = Defs::bundled();
    plan(&config(TABLES, &defs), &[1, 2, 3, 4], filter)
}

fn count(jobs: &[Job], style: &str, preset: &str) -> usize {
    jobs.iter().filter(|j| j.style == style && j.preset == preset).count()
}

#[test]
fn the_plan_covers_what_targets_and_pairs_name() {
    let jobs = plan_for(&Filter::default());
    // steady/normal: targets a and b and the pair's low side, all seeds, 4 nations.
    assert_eq!(count(&jobs, "steady", "normal"), 16);
    // steady/hard: target c wants 2 seeds, the pair 3: the more demanding wins.
    assert_eq!(count(&jobs, "steady", "hard"), 12);
    assert_eq!(count(&jobs, "careless", "normal"), 16);
    assert_eq!(jobs.len(), 44);
}

#[test]
fn a_run_is_as_long_as_the_longest_target_that_reads_it() {
    let jobs = plan_for(&Filter::default());
    let minutes = |style: &str, preset: &str| {
        jobs.iter().find(|j| j.style == style && j.preset == preset).unwrap().minutes
    };
    assert_eq!(minutes("steady", "normal"), 240.0);
    assert_eq!(minutes("steady", "hard"), 180.0);
}

#[test]
fn the_plan_is_sorted_by_style_preset_nation_seed() {
    let jobs = plan_for(&Filter::default());
    let keys: Vec<_> = jobs
        .iter()
        .map(|j| (j.style.clone(), j.preset.clone(), j.nation.as_str(), j.seed))
        .collect();
    let mut sorted = keys.clone();
    sorted.sort();
    assert_eq!(keys, sorted);
}

#[test]
fn filters_narrow_the_plan() {
    let only_steady =
        plan_for(&Filter { styles: Some(vec!["steady".to_owned()]), ..Filter::default() });
    assert_eq!(only_steady.len(), 28);
    let hard = plan_for(&Filter { presets: Some(vec!["hard".to_owned()]), ..Filter::default() });
    assert!(hard.iter().all(|j| j.preset == "hard"));
    let earth = plan_for(&Filter { nations: Some(vec![Nation::Earth]), ..Filter::default() });
    assert!(earth.iter().all(|j| j.nation == Nation::Earth));
    assert_eq!(earth.len(), 11);
    let short = plan_for(&Filter { minutes: Some(5.0), ..Filter::default() });
    assert!(short.iter().all(|j| j.minutes == 5.0));
}

#[test]
fn seeds_are_taken_in_the_order_given() {
    let defs = Defs::bundled();
    let jobs = plan(&config(TABLES, &defs), &[9, 3, 5, 1], &Filter::default());
    let hard: Vec<u64> = jobs
        .iter()
        .filter(|j| j.preset == "hard" && j.nation == Nation::Air)
        .map(|j| j.seed)
        .collect();
    assert_eq!(hard, vec![3, 5, 9], "the panel's first three, sorted into the output");
}

#[test]
fn the_records_do_not_depend_on_the_thread_count() {
    let defs = Defs::bundled();
    let cfg = config(TABLES, &defs);
    let filter = Filter {
        styles: Some(vec!["steady".to_owned()]),
        presets: Some(vec!["normal".to_owned()]),
        nations: Some(vec![Nation::Earth, Nation::Water]),
        minutes: Some(3.0),
    };
    let jobs = plan(&cfg, &[3, 1, 2], &filter);
    assert_eq!(jobs.len(), 6);
    let one = run_all(&jobs, &cfg, &defs, 1, &|_, _| {});
    let three = run_all(&jobs, &cfg, &defs, 3, &|_, _| {});
    assert_eq!(to_json(&one), to_json(&three));
    let order: Vec<(&str, u64)> =
        one.iter().map(|r| (r.record.nation.as_str(), r.record.seed)).collect();
    assert_eq!(
        order,
        vec![("earth", 1), ("earth", 2), ("earth", 3), ("water", 1), ("water", 2), ("water", 3)]
    );
}

#[test]
fn json_is_an_array_with_a_run_per_line() {
    let defs = Defs::bundled();
    let cfg = config(TABLES, &defs);
    let job = Job {
        style: "steady".to_owned(),
        preset: "normal".to_owned(),
        nation: Nation::Earth,
        seed: 7,
        minutes: 2.0,
        step_offset: 0,
    };
    let run = run_job(&job, &cfg, &defs);
    assert_eq!(run.record.seed, 7);
    let json = to_json(&[run.clone(), run]);
    let lines: Vec<&str> = json.lines().collect();
    assert_eq!(lines.len(), 4);
    assert_eq!((lines[0], lines[3]), ("[", "]"));
    assert!(lines[1].ends_with("},") && lines[2].ends_with('}'));
    let parsed: serde_json::Value = serde_json::from_str(&json).expect("valid JSON");
    assert_eq!(parsed[0]["style"], "steady");
    assert_eq!(parsed[0]["record"]["seed"], 7);
    assert_eq!(parsed[0]["record"]["nation"], "earth");
}

#[test]
fn a_style_runs_the_player_its_data_says() {
    let defs = Defs::bundled();
    let cfg = config(TABLES, &defs);
    let job = |style: &str| Job {
        style: style.to_owned(),
        preset: "normal".to_owned(),
        nation: Nation::Earth,
        seed: 7,
        minutes: 10.0,
        step_offset: 0,
    };
    let careless = run_job(&job("careless"), &cfg, &defs);
    assert!(careless.record.style.contains("farms: false"), "{}", careless.record.style);
    let idle = run_job(&job("idle"), &cfg, &defs);
    assert_eq!(idle.record.style, "Idle");
}
