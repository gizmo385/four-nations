use std::path::PathBuf;

use fn_sim::{Defs, Nation};

use super::*;
use crate::runner;
use crate::testkit::{config, panel, record};
use crate::tier::Tier;

const TABLES: &str = "\
    [[target]]\nid = \"stone\"\ndoc = \"materials.md#7-measuring-it\"\nmetric = \"sign_minute.stone\"\nat_most = 120\n\
    [[target]]\nid = \"masters\"\ndoc = \"materials.md#7-measuring-it\"\nmetric = \"sign_minute.masters\"\nat_most = 150\n\
    [[target]]\nid = \"pop\"\ndoc = \"materials.md#7-measuring-it\"\nmetric = \"pop_at.120\"\nat_least = 14\n\
    [[target]]\nid = \"more_stone\"\ndoc = \"materials.md#7-measuring-it\"\nmetric = \"sign_minute.stone\"\nat_most = 100\n";

fn runs() -> Vec<Run> {
    panel("steady", "normal", 3, |n, s| {
        let mut r = record(n, s);
        // Stone at a fraction that f32 can't hold exactly; the masters sign is never met.
        r.sign_minutes[1].1 = Some(100.0 + s as f32 / 10.0);
        r
    })
}

fn set() -> (Set, Config, Defs) {
    let defs = Defs::bundled();
    let cfg = config(TABLES, &defs);
    (Set::from_runs(&runs(), &cfg, &defs), cfg, defs)
}

#[test]
fn a_run_boils_down_to_its_key_flags_and_readings() {
    let (set, _, _) = set();
    assert_eq!(set.entries.len(), 12);
    let e = &set.entries[0];
    assert_eq!(e.key(), ("steady", "normal", "air", 1));
    assert_eq!(e.minutes, 180.0);
    // Each distinct metric once, however many targets read it; masters never met is infinity; a
    // reading the record can't give (pop at 120 with no season rows) is left out.
    assert_eq!(
        e.metrics.keys().map(String::as_str).collect::<Vec<_>>(),
        ["sign_minute.masters", "sign_minute.stone"]
    );
    assert_eq!(e.metrics["sign_minute.stone"], 100.099_998, "rounded, not 100.0999984741211");
    assert_eq!(e.metrics["sign_minute.masters"], f64::INFINITY);
}

#[test]
fn the_file_round_trips_exactly() {
    let (set, cfg, defs) = set();
    let json = set.to_json();
    assert_eq!(Set::parse(&json, &cfg, &defs).unwrap(), set);
    assert_eq!(Set::parse(&json, &cfg, &defs).unwrap().to_json(), json);
    assert!(json.contains("\"sign_minute.masters\":\"never\""), "{json}");
    assert!(json.contains("\"sign_minute.stone\":100.099998"), "{json}");
}

#[test]
fn the_file_is_one_run_per_line() {
    let (set, _, _) = set();
    let json = set.to_json();
    let lines: Vec<&str> = json.lines().collect();
    assert_eq!(lines.len(), 12 + 6, "{json}");
    assert!(lines[0] == "{" && lines[lines.len() - 1] == "}");
    assert!(lines[1].starts_with("\"format\": 1"));
    assert!(lines[2].starts_with("\"data\": \""));
    assert_eq!(lines[3], "\"runs\": [");
    assert!(lines[4].starts_with("{\"style\":\"steady\""));
    assert!(lines[4].ends_with("},") && lines[15].ends_with('}'));
}

/// Blessing twice must give the same bytes: nothing depends on the order the runs came back in.
#[test]
fn the_bytes_do_not_depend_on_the_order_of_the_runs() {
    let defs = Defs::bundled();
    let cfg = config(TABLES, &defs);
    let forward = runs();
    let mut backward = runs();
    backward.reverse();
    assert_eq!(
        Set::from_runs(&forward, &cfg, &defs).to_json(),
        Set::from_runs(&backward, &cfg, &defs).to_json()
    );
}

#[test]
fn a_file_of_full_records_reads_the_same_runs() {
    let (set, cfg, defs) = set();
    let full = runner::to_json(&runs());
    let from_full = Set::parse(&full, &cfg, &defs).unwrap();
    assert_eq!(from_full.digest, None);
    assert_eq!(from_full.entries, set.entries);
}

#[test]
fn a_baseline_is_far_smaller_than_the_records_it_summarises() {
    let (set, _, _) = set();
    assert!(set.to_json().len() < runner::to_json(&runs()).len() / 3);
}

#[test]
fn bad_files_say_what_is_wrong() {
    let (_, cfg, defs) = set();
    let err = |text: &str| Set::parse(text, &cfg, &defs).expect_err(text);
    assert!(err("nonsense").contains("not a baseline file"));
    assert!(err("[1, 2]").contains("not a list of run records"));
    assert!(err("{\"format\": 9, \"data\": \"x\", \"runs\": []}").contains("baseline format 9"));
    let odd = "{\"format\": 1, \"data\": \"x\", \"runs\": [{\"style\":\"s\",\"preset\":\"p\",\
        \"nation\":\"air\",\"seed\":1,\"minutes\":1.0,\"flags\":[],\"metrics\":{\"m\":\"soon\"}}]}";
    assert!(err(odd).contains("not a number"));
}

#[test]
fn quantizing_rounds_to_a_millionth_and_keeps_infinity() {
    assert_eq!(quantize(100.099_998_474_121_1), 100.099_998);
    assert_eq!(quantize(0.123_456_789), 0.123_457);
    assert_eq!(quantize(f64::INFINITY), f64::INFINITY);
    assert_eq!(quantize(0.0), 0.0);
}

#[test]
fn the_digest_follows_the_styles_and_not_the_targets() {
    let defs = Defs::bundled();
    let a = config(TABLES, &defs);
    assert_eq!(data_digest(&a), data_digest(&config(TABLES, &defs)));
    assert_eq!(data_digest(&a).len(), 16);
    let fewer = config(&TABLES[..TABLES.find("[[target]]\nid = \"masters\"").unwrap()], &defs);
    assert_eq!(data_digest(&a), data_digest(&fewer), "targets judge runs, they don't change them");
    let noisy = config(&format!("{TABLES}[noise]\n\"sign_minute.stone\" = 3.0\n"), &defs);
    assert_eq!(data_digest(&a), data_digest(&noisy), "re-jittering must not make a baseline stale");
    let text = format!(
        "[[style]]\nid = \"steady\"\nopening = \"economy\"\nfarms = false\n\
         [[style]]\nid = \"careless\"\nopening = \"economy\"\nfarms = false\n\
         [[style]]\nid = \"idle\"\nopening = \"none\"\n[panel]\nmin_runs = 4\n{TABLES}"
    );
    let other_style = Config::parse(&text, &defs).unwrap();
    assert_ne!(data_digest(&a), data_digest(&other_style));
}

#[test]
fn flags_are_kept_by_name() {
    let defs = Defs::bundled();
    let cfg = config(TABLES, &defs);
    let mut rec = record(Nation::Fire, 5);
    rec.flags.push(crate::testkit::raised(fn_sim::balance::Flag::Refused, 40.0));
    let run = crate::testkit::run("steady", "normal", rec);
    let e = Entry::of(&run, &metric_list(&cfg), &defs);
    assert_eq!(e.flags, ["refused"]);
}

fn committed() -> (Set, Config, Defs) {
    let defs = Defs::bundled();
    let cfg = Config::bundled(&defs);
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(BASELINE_FILE);
    let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    let set = Set::parse(&text, &cfg, &defs).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    (set, cfg, defs)
}

/// The committed baseline loads, is in canonical form, and has a run for everything the quick tier
/// plays, so `--tier quick --against baseline.json` pairs every run it makes.
#[test]
fn the_committed_baseline_covers_the_quick_tier() {
    let (set, cfg, _) = committed();
    assert!(set.digest.is_some(), "a blessed baseline records its data digest");
    let seeds = crate::config::parse_seeds(&Tier::Quick.seeds(&cfg)).unwrap();
    for job in runner::plan(&cfg, &seeds, &Tier::Quick.filter()) {
        let key = (job.style.as_str(), job.preset.as_str(), job.nation.as_str(), job.seed);
        let e = set.entries.iter().find(|e| e.key() == key);
        let e = e.unwrap_or_else(|| {
            panic!("baseline has no run for {key:?}; `balance verdict --bless`")
        });
        assert_eq!(e.minutes, job.minutes, "{key:?}");
    }
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(BASELINE_FILE);
    assert_eq!(std::fs::read_to_string(path).unwrap(), set.to_json(), "not in canonical form");
}
