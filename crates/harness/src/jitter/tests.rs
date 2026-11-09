use fn_sim::{Defs, Nation};

use super::*;
use crate::testkit::config;

fn entry(seed: u64, stone: f64, deaths: f64) -> Entry {
    Entry {
        style: "steady".to_owned(),
        preset: "normal".to_owned(),
        nation: Nation::Water,
        seed,
        minutes: 180.0,
        flags: Vec::new(),
        metrics: BTreeMap::from([
            ("sign_minute.stone".to_owned(), stone),
            ("deaths_by.180".to_owned(), deaths),
        ]),
    }
}

#[test]
fn noise_is_the_median_absolute_difference_between_twins() {
    // Stone moves by -3, 1, 0, 2, 0 (abs: 3 1 0 2 0 -> median 1); deaths never move.
    let plain: Vec<Entry> = (1..=5).map(|s| entry(s, 100.0, 0.0)).collect();
    let moved = [97.0, 101.0, 100.0, 102.0, 100.0];
    let twins: Vec<Entry> = (1..=5).map(|s| entry(s, moved[s as usize - 1], 0.0)).collect();
    let noise = noise_floor(&plain, &twins);
    assert_eq!(noise["sign_minute.stone"], 1.0);
    assert_eq!(noise["deaths_by.180"], 0.0);
    // Direction does not matter, and an even count averages the middle two.
    let twins: Vec<Entry> =
        (1..=4).map(|s| entry(s, 100.0 + [2.0, -2.0, 3.0, 0.0][s as usize - 1], 0.0)).collect();
    assert_eq!(noise_floor(&plain[..4], &twins)["sign_minute.stone"], 2.0);
}

#[test]
fn never_against_never_is_no_difference_and_never_against_a_minute_is_infinite() {
    let inf = f64::INFINITY;
    let plain = vec![entry(1, inf, 0.0), entry(2, inf, 0.0), entry(3, inf, 0.0)];
    let twins = vec![entry(1, inf, 0.0), entry(2, inf, 0.0), entry(3, 150.0, 0.0)];
    assert_eq!(noise_floor(&plain, &twins)["sign_minute.stone"], 0.0);
    let twins = vec![entry(1, inf, 0.0), entry(2, 150.0, 0.0), entry(3, 150.0, 0.0)];
    assert_eq!(noise_floor(&plain, &twins)["sign_minute.stone"], inf);
}

#[test]
fn only_runs_with_a_twin_and_a_reading_count() {
    let plain = vec![entry(1, 100.0, 0.0), entry(2, 100.0, 0.0)];
    let mut twin = entry(1, 104.0, 0.0);
    twin.metrics.remove("deaths_by.180");
    let twins = vec![twin, entry(9, 500.0, 0.0)];
    let noise = noise_floor(&plain, &twins);
    assert_eq!(noise["sign_minute.stone"], 4.0, "seed 9 has no plain twin");
    assert!(!noise.contains_key("deaths_by.180"), "no pair read it");
    assert!(noise_floor(&plain, &[]).is_empty());
}

#[test]
fn a_style_that_plays_nothing_has_no_twin_to_run() {
    let defs = Defs::bundled();
    let cfg = config(
        "[[target]]\nid = \"a\"\ndoc = \"harm.md#x\"\nmetric = \"deaths\"\nat_most = 1\n\
         [[target]]\nid = \"b\"\ndoc = \"harm.md#x\"\nstyle = \"idle\"\nmetric = \"first_death\"\nat_least = 45\n\
         [[target]]\nid = \"c\"\ndoc = \"harm.md#x\"\nstyle = \"careless\"\nmetric = \"deaths\"\nat_least = 1\n",
        &defs,
    );
    let jobs = jobs(&cfg, &[1, 2], &Filter::default());
    assert_eq!(jobs.len(), 16);
    assert!(jobs.iter().all(|j| j.style != "idle" && j.step_offset == 0));
}

const FILE: &str = "\
# header comment
[panel]
seeds = \"1-32\"

# --- Noise floors ---
# Why these exist.
[noise]
# the first one
\"sign_minute.stone\" = 0.5   # measured once
\"deaths_by.180\"=2
# a note between entries

\"pop_at.120\" = 1.0

# --- Targets ---
[[target]]
id = \"t\"
";

fn values(list: &[(&str, f64)]) -> BTreeMap<String, f64> {
    list.iter().map(|(k, v)| ((*k).to_owned(), *v)).collect()
}

#[test]
fn set_noise_changes_only_the_values_it_names() {
    let out = set_noise(FILE, &values(&[("sign_minute.stone", 1.5), ("deaths_by.180", 0.0)]));
    let expect = FILE
        .replace(
            "\"sign_minute.stone\" = 0.5   # measured once",
            "\"sign_minute.stone\" = 1.5   # measured once",
        )
        .replace("\"deaths_by.180\"=2", "\"deaths_by.180\"= 0.0");
    assert_eq!(out, expect);
}

#[test]
fn set_noise_adds_new_metrics_after_the_last_entry_in_order() {
    let out = set_noise(FILE, &values(&[("wood_at.120", 3.0), ("longest_gap", 0.25)]));
    let expect = FILE.replace(
        "\"pop_at.120\" = 1.0\n",
        "\"pop_at.120\" = 1.0\n\"longest_gap\" = 0.25\n\"wood_at.120\" = 3.0\n",
    );
    assert_eq!(out, expect);
}

#[test]
fn set_noise_is_idempotent_and_the_result_loads() {
    let v = values(&[("sign_minute.stone", 2.0), ("deaths", 0.0)]);
    let once = set_noise(FILE, &v);
    assert_eq!(set_noise(&once, &v), once);
    let toml: toml::Value = toml::from_str(&once).unwrap();
    assert_eq!(toml["noise"]["sign_minute.stone"].as_float(), Some(2.0));
    assert_eq!(toml["noise"]["pop_at.120"].as_float(), Some(1.0));
    assert_eq!(toml["target"][0]["id"].as_str(), Some("t"));
}

#[test]
fn an_empty_table_is_filled_under_its_header_and_a_missing_one_is_added() {
    let empty = "[panel]\nseeds = \"1-4\"\n[noise]\n\n# Targets\n[[target]]\nid = \"t\"\n";
    let out = set_noise(empty, &values(&[("deaths", 1.0)]));
    assert_eq!(
        out,
        "[panel]\nseeds = \"1-4\"\n[noise]\n\"deaths\" = 1.0\n\n# Targets\n[[target]]\nid = \"t\"\n"
    );
    let none = set_noise("[panel]\nseeds = \"1-4\"\n", &values(&[("deaths", 1.0)]));
    assert_eq!(none, "[panel]\nseeds = \"1-4\"\n\n[noise]\n\"deaths\" = 1.0\n");
}

/// Writing nothing leaves the file exactly as it was.
#[test]
fn set_noise_with_no_values_is_the_identity() {
    assert_eq!(set_noise(FILE, &BTreeMap::new()), FILE);
}

/// The bundled file takes a write without disturbing a byte outside `[noise]`, and still loads.
#[test]
fn the_bundled_file_keeps_its_comments_through_a_write() {
    let text = include_str!("../../../../data/harness.toml");
    let v = values(&[("sign_minute.stone", 1.0), ("care.suffering", 0.0)]);
    let out = set_noise(text, &v);
    let lines: Vec<&str> = out.lines().collect();
    let before: Vec<&str> = text.lines().collect();
    for line in &before {
        if !line.starts_with('"') {
            assert!(lines.contains(line), "lost {line:?}");
        }
    }
    let defs = Defs::bundled();
    let cfg = Config::parse(&out, &defs).unwrap();
    assert_eq!(cfg.noise_of("sign_minute.stone"), 1.0);
    assert_eq!(cfg.targets.len(), crate::config::Config::bundled(&defs).targets.len());
}
