use fn_sim::Defs;

use super::*;
use crate::runner::plan;

#[test]
fn tiers_parse_by_name() {
    for t in Tier::ALL {
        assert_eq!(Tier::parse(t.name()), Some(t));
    }
    assert_eq!(Tier::parse("slow"), None);
}

#[test]
fn quick_is_twelve_steady_normal_seeds_for_every_nation() {
    let defs = Defs::bundled();
    let cfg = Config::bundled(&defs);
    let seeds = crate::config::parse_seeds(&Tier::Quick.seeds(&cfg)).unwrap();
    assert_eq!(seeds, (1..=12).collect::<Vec<_>>());
    let jobs = plan(&cfg, &seeds, &Tier::Quick.filter());
    assert_eq!(jobs.len(), 48);
    assert!(jobs.iter().all(|j| j.style == "steady" && j.preset == "normal"));
}

/// The full tier is the doc's table: 32 seeds of `steady`, `careless`, `branch_first` and `idle`
/// on Normal, `steady` and `idle` on Hard and Sandbox at 12 seeds, all four nations; and the two
/// school styles at 32 seeds on Normal and Hard for the `[[fork]]` checks (horizons-phase-4.md).
#[test]
fn full_is_the_docs_table() {
    let defs = Defs::bundled();
    let cfg = Config::bundled(&defs);
    let seeds = crate::config::parse_seeds(&Tier::Full.seeds(&cfg)).unwrap();
    assert_eq!(seeds.len(), 32);
    let jobs = plan(&cfg, &seeds, &Tier::Full.filter());
    let count = |style: &str, preset: &str| {
        jobs.iter().filter(|j| j.style == style && j.preset == preset).count()
    };
    for style in ["steady", "careless", "branch_first", "idle"] {
        assert_eq!(count(style, "normal"), 4 * 32, "{style} on normal");
    }
    for (style, preset) in
        [("steady", "hard"), ("steady", "sandbox"), ("idle", "hard"), ("idle", "sandbox")]
    {
        assert_eq!(count(style, preset), 4 * 12, "{style} on {preset}");
    }
    for style in ["school_first", "school_second"] {
        for preset in ["normal", "hard"] {
            assert_eq!(count(style, preset), 4 * 32, "{style} on {preset}");
        }
    }
    // #751, #764: the United City runs, in both research orders, 420 minutes, every seed.
    for style in ["united", "united_branch_first"] {
        assert_eq!(count(style, "normal"), 4 * 32, "{style}");
        assert!(jobs.iter().filter(|j| j.style == style).all(|j| j.minutes == 420.0));
    }
    assert_eq!(jobs.len(), 4 * 128 + 4 * 48 + 4 * 128 + 2 * 4 * 32);
}

/// Quick's runs are runs the full tier makes too, which is what lets it pair with the baseline.
#[test]
fn quick_is_a_subset_of_full() {
    let defs = Defs::bundled();
    let cfg = Config::bundled(&defs);
    let seeds = |t: Tier| crate::config::parse_seeds(&t.seeds(&cfg)).unwrap();
    let full = plan(&cfg, &seeds(Tier::Full), &Tier::Full.filter());
    for q in plan(&cfg, &seeds(Tier::Quick), &Tier::Quick.filter()) {
        assert!(full.contains(&q), "{q:?} is not in the full tier");
    }
}

#[test]
fn holdout_shares_no_seed_with_the_panel() {
    let defs = Defs::bundled();
    let cfg = Config::bundled(&defs);
    let panel = crate::config::parse_seeds(&Tier::Full.seeds(&cfg)).unwrap();
    let holdout = crate::config::parse_seeds(&Tier::Holdout.seeds(&cfg)).unwrap();
    assert_eq!(holdout.len(), 32);
    assert!(holdout.iter().all(|s| !panel.contains(s)));
}
