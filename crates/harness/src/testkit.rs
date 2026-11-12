//! Hand-built runs for the harness's own tests: a `RunRecord` is plain data, so a verdict test
//! writes the numbers it wants instead of playing a game.

use std::collections::BTreeMap;

use fn_sim::balance::{CareSummary, Flag, Raised, RunRecord, SiteProfile};
use fn_sim::{Defs, Nation};

use crate::config::Config;
use crate::verdict::Run;

/// A small config over the bundled data: two styles (`steady`, `idle`), one panel default of
/// `min_runs = 4`, and whatever `tables` adds (targets, pairs).
pub fn config(tables: &str, defs: &Defs) -> Config {
    let text = format!(
        "[[style]]\nid = \"steady\"\nopening = \"economy\"\n\
         [[style]]\nid = \"careless\"\nopening = \"economy\"\nfarms = false\n\
         [[style]]\nid = \"idle\"\nopening = \"none\"\n\
         [panel]\nmin_runs = 4\n{tables}"
    );
    Config::parse(&text, defs).unwrap_or_else(|e| panic!("test config: {e}"))
}

/// An uneventful 180-minute record: stone and ore met at minute 100, nobody died, plenty of both
/// materials in reach, no flags.
pub fn record(nation: Nation, seed: u64) -> RunRecord {
    RunRecord {
        seed,
        nation,
        preset: Some("normal".to_owned()),
        minutes: 180.0,
        style: "test".to_owned(),
        site: SiteProfile {
            anchor: (0, 0),
            supply: BTreeMap::from([("stone".to_owned(), 500), ("ore".to_owned(), 500)]),
            tunnels: false,
            fresh_water_steps: Some(0),
            food_sources: 10,
            open_tiles: 100,
        },
        seasons: Vec::new(),
        sign_minutes: vec![
            ("population".to_owned(), Some(110.0)),
            ("stone".to_owned(), Some(100.0)),
            ("ore".to_owned(), Some(100.0)),
            ("masters".to_owned(), None),
        ],
        timeline: Vec::new(),
        first_built: Vec::new(),
        recipe_stores: Vec::new(),
        techs: Vec::new(),
        longest_gap: None,
        gaps_over_15: 0,
        idle_share: Vec::new(),
        care: CareSummary {
            person_minutes: 1800.0,
            depleted_minutes: [0.0; 3],
            severe_minutes: [0.0; 4],
            severe_any_minutes: 0.0,
            cold_minutes: 0.0,
            close_calls: 0,
            lowest_health: Some(100.0),
        },
        first_death_minute: None,
        paid_in: Vec::new(),
        deaths: 0,
        population: 12,
        autopsies: Vec::new(),
        flags: Vec::new(),
        stage_minutes: Vec::new(),
        school_tallies: Vec::new(),
        teachings: Default::default(),
    }
}

/// `flag` raised at `minute`, its condition begun two minutes before.
pub fn raised(flag: Flag, minute: f32) -> Raised {
    Raised { flag, minute, since: (minute - 2.0).max(0.0) }
}

/// A run of `style` on `preset` from `record`.
pub fn run(style: &str, preset: &str, record: RunRecord) -> Run {
    Run { style: style.to_owned(), preset: preset.to_owned(), record }
}

/// Seeds `1..=seeds` for each of the four nations, every run built by `make`.
pub fn panel(
    style: &str,
    preset: &str,
    seeds: u64,
    make: impl Fn(Nation, u64) -> RunRecord,
) -> Vec<Run> {
    let mut runs = Vec::new();
    for &nation in &Nation::ALL {
        for seed in 1..=seeds {
            runs.push(run(style, preset, make(nation, seed)));
        }
    }
    runs
}
