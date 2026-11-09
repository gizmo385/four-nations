use std::collections::BTreeSet;
use std::path::PathBuf;

use fn_sim::balance::{Detectors, SiteProfile};
use fn_sim::embark::Survey;
use fn_sim::{Defs, Nation};

use super::*;

const STYLES: &str = "[[style]]\nid = \"steady\"\nopening = \"economy\"\n";

fn parse(extra: &str) -> Result<Config, ConfigError> {
    Config::parse(&format!("{STYLES}{extra}"), &Defs::bundled())
}

fn parse_err(extra: &str) -> String {
    parse(extra).expect_err("should not load").to_string()
}

const TARGET: &str = "[[target]]\nid = \"t\"\ndoc = \"materials.md#7-measuring-it\"\n\
    metric = \"sign_minute.stone\"\nat_most = 120\n";

#[test]
fn the_bundled_file_loads_with_every_table() {
    let defs = Defs::bundled();
    let cfg = Config::bundled(&defs);
    assert_eq!(cfg.panel.seeds, "1-32");
    assert_eq!(cfg.panel.threads, 4);
    assert_eq!(cfg.detectors, Detectors::default(), "the file's thresholds are the doc's");
    assert!(cfg.styles.get("steady").is_some());
    assert!(!cfg.pairs.is_empty());
    // The doc names these targets by id.
    for id in ["stone_by_120", "ore_by_120", "no_deaths_by_180", "careless_close_calls"] {
        assert!(cfg.target(id).is_some(), "{id}");
    }
    let stone = cfg.target("stone_by_120").unwrap();
    assert_eq!(stone.bar.at_most, Some(120.0));
    assert_eq!(stone.share, 0.85);
    assert_eq!(stone.hard_expect.unwrap().bar.at_most, Some(180.0));
}

#[test]
fn every_target_names_a_real_section_of_a_design_doc() {
    let defs = Defs::bundled();
    let cfg = Config::bundled(&defs);
    let docs = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../docs/design");
    let anchors = |file: &str| -> BTreeSet<String> {
        let text =
            std::fs::read_to_string(docs.join(file)).unwrap_or_else(|e| panic!("{file}: {e}"));
        text.lines()
            .filter(|l| l.starts_with('#'))
            .map(|l| {
                // GitHub's anchor: lowercase, letters, digits and hyphens kept, spaces to hyphens.
                l.trim_start_matches('#')
                    .trim()
                    .to_lowercase()
                    .chars()
                    .filter(|c| c.is_alphanumeric() || *c == '-' || *c == ' ')
                    .map(|c| if c == ' ' { '-' } else { c })
                    .collect()
            })
            .collect()
    };
    let docs_of = cfg.targets.iter().map(|t| &t.doc).chain(cfg.pairs.iter().map(|p| &p.doc));
    for doc in docs_of {
        let (file, anchor) = doc.split_once('#').expect("file.md#anchor");
        assert!(anchors(file).contains(anchor), "{doc}: no such heading in {file}");
    }
}

#[test]
fn the_hard_site_materials_are_what_the_site_profile_counts() {
    let defs = Defs::bundled();
    let size = defs.embark.map_size("small").unwrap();
    let survey = Survey::new(&Survey::config(1, size), &defs);
    let start = survey.default_site(&defs);
    let profile = SiteProfile::compute(&survey, start, Nation::Earth, &defs);
    let keys: Vec<&str> = profile.supply.keys().map(String::as_str).collect();
    let mut want = SUPPLY_SIGNS.to_vec();
    want.sort_unstable();
    assert_eq!(keys, want);
}

#[test]
fn defaults_come_from_the_panel_table() {
    let cfg =
        parse(&format!("[panel]\nshare = 0.9\nmin_runs = 5\nminutes = 120\n{TARGET}")).unwrap();
    let t = cfg.target("t").unwrap();
    assert_eq!((t.share, t.min_runs, t.minutes), (0.9, 5, 120.0));
    assert_eq!((t.style.as_str(), t.preset.as_str()), ("steady", "normal"));
    let own = parse(&format!("{TARGET}share = 0.5\nmin_runs = 2\nminutes = 60\n")).unwrap();
    let t = own.target("t").unwrap();
    assert_eq!((t.share, t.min_runs, t.minutes), (0.5, 2, 60.0));
}

#[test]
fn detector_thresholds_override_the_defaults_key_by_key() {
    let cfg = parse("[detectors]\nidle_minutes = 45\nrefused_streak = 5\n").unwrap();
    assert_eq!(cfg.detectors.idle_minutes, 45);
    assert_eq!(cfg.detectors.refused_streak, 5);
    assert_eq!(cfg.detectors.stranded_minutes, Detectors::default().stranded_minutes);
}

#[test]
fn strict_keys_everywhere() {
    assert!(parse_err("[detectors]\nidle_minute = 45\n").contains("idle_minute"));
    assert!(parse_err("[panel]\nseed = \"1-8\"\n").contains("seed"));
    assert!(parse_err(&format!("{TARGET}shares = 1\n")).contains("shares"));
    assert!(
        parse_err(&format!("{TARGET}hard_when = {{ supply = \"ore\", belowe = 1 }}\n"))
            .contains("belowe")
    );
    assert!(parse_err("[[targets]]\nid = \"x\"\n").contains("targets"));
}

#[test]
fn bad_values_name_the_target() {
    let bad = |extra: &str| parse_err(&format!("{TARGET}{extra}"));
    assert!(bad("share = 0\n").contains("share"));
    assert!(bad("share = 1.5\n").contains("share"));
    assert!(bad("style = \"loud\"\n").contains("unknown style"));
    assert!(bad("preset = \"easy\"\n").contains("unknown preset"));
    assert!(bad("nations = [\"mud\"]\n").contains("unknown nation"));
    assert!(bad("first_seeds = 0\n").contains("first_seeds"));
    assert!(bad("hard_when = { supply = \"wood\", below = 1 }\n").contains("hard_when.supply"));
    assert!(bad("hard_when = { supply = \"ore\", below = 0 }\n").contains("below"));
    assert!(bad("hard_expect = { at_most = 5 }\n").contains("without hard_when"));
    assert!(parse_err(&TARGET.replace("sign_minute.stone", "sign_minute.mud")).contains("no sign"));
    assert!(
        parse_err(&TARGET.replace("at_most = 120", "at_most = 5\nat_least = 9")).contains("above")
    );
    assert!(parse_err(&TARGET.replace("at_most = 120\n", "")).contains("at_most"));
    assert!(parse_err(&TARGET.replace("materials.md#7-measuring-it", "materials")).contains("doc"));
    assert!(parse_err(&format!("{TARGET}{TARGET}")).contains("used twice"));
    assert!(parse_err("[panel]\nthreads = 0\n").contains("threads"));
    assert!(parse_err("[panel]\nseeds = \"1-\"\n").contains("seeds"));
}

#[test]
fn a_pair_names_two_different_runs() {
    let pair = |lo: &str, hi: &str| {
        format!(
            "[[pair]]\nid = \"p\"\ndoc = \"harm.md#x\"\nmetric = \"deaths\"\n\
             lower = {lo}\nhigher = {hi}\n"
        )
    };
    let steady = "{ style = \"steady\", preset = \"normal\" }";
    assert!(parse_err(&pair(steady, steady)).contains("same run"));
    assert!(
        parse_err(&pair(steady, "{ style = \"steady\", preset = \"easy\" }"))
            .contains("unknown preset")
    );
    assert!(parse(&pair(steady, "{ style = \"steady\", preset = \"hard\" }")).is_ok());
}

#[test]
fn a_bar_reads_an_event_that_never_happened_as_infinity() {
    let at_most = Bar { at_most: Some(120.0), at_least: None };
    let at_least = Bar { at_most: None, at_least: Some(180.0) };
    let band = Bar { at_most: Some(120.0), at_least: Some(45.0) };
    assert!(at_most.met(120.0) && !at_most.met(120.5) && !at_most.met(f64::INFINITY));
    assert!(at_least.met(f64::INFINITY) && !at_least.met(100.0));
    assert!(band.met(45.0) && band.met(120.0) && !band.met(44.9) && !band.met(121.0));
    assert_eq!(band.to_string(), "45..120");
    assert_eq!(at_most.to_string(), "<= 120");
}

#[test]
fn seed_lists_parse_like_the_binarys_flag() {
    assert_eq!(parse_seeds("1-4"), Some(vec![1, 2, 3, 4]));
    assert_eq!(parse_seeds("7,42"), Some(vec![7, 42]));
    assert_eq!(parse_seeds("1-3,2-5,9"), Some(vec![1, 2, 3, 4, 5, 9]));
    assert_eq!(parse_seeds("4-1"), None);
    assert_eq!(parse_seeds("a"), None);
    assert_eq!(parse_seeds(""), None);
}

#[test]
fn seed_lists_match_what_the_legacy_sweeps_expanded() {
    // `legacy.rs` once had its own parser: no dedup, no trimming, `4-1` an empty range. On its
    // defaults and on any list without repeats or inverted ranges it gave exactly this.
    assert_eq!(parse_seeds("1-8"), Some((1..=8).collect()));
    assert_eq!(parse_seeds("7,42"), Some(vec![7, 42]));
    assert_eq!(parse_seeds("5"), Some(vec![5]));
    assert_eq!(parse_seeds(" 3 - 4 , 9"), Some(vec![3, 4, 9]));
}

#[test]
fn noise_floors_load_by_metric_name() {
    let cfg =
        parse(&format!("{TARGET}[noise]\n\"sign_minute.stone\" = 1.5\n\"deaths_by.180\" = 0\n"))
            .unwrap();
    assert_eq!(cfg.noise_of("sign_minute.stone"), 1.5);
    assert_eq!(cfg.noise_of("deaths_by.180"), 0.0);
    assert_eq!(cfg.noise_of("pop_at.120"), 0.0, "no entry is no floor");
    assert!(parse(TARGET).unwrap().noise.is_empty());
    let defs = Defs::bundled();
    let bundled = Config::bundled(&defs);
    for name in bundled.noise.keys() {
        let read = bundled
            .targets
            .iter()
            .map(|t| &t.metric)
            .chain(bundled.pairs.iter().map(|p| &p.metric));
        assert!(read.into_iter().any(|m| m.name() == *name), "{name}: nothing reads it");
    }
}

#[test]
fn bad_noise_names_the_metric() {
    assert!(parse_err("[noise]\n\"sign_minute.nonsense\" = 1.0\n").contains("no sign"));
    assert!(parse_err("[noise]\n\"deaths\" = -1.0\n").contains("noise must be 0 or more"));
    assert!(parse_err("[noise]\ndeaths = \"lots\"\n").contains("invalid type"));
}
