//! The older modes of the `balance` binary (#300, #343, #348, #352, #526), kept as subcommands
//! when the binary moved here from `fn_sim` (#537): `sweep`, `supply`, `mastery` and `economy`.
//! They drive `fn_sim::balance::run` and `run_economy`, which stay, because their walk-time and
//! mastery outputs don't fit a `Player` (`fn_sim::balance::run_record`'s, which `verdict` uses).
//! The old flag spellings (`balance --supply`, `--mastery`, `--economy`, and bare sweep flags)
//! still reach these through `cli`.
//!
//! `sweep` runs the harness (`docs/design/difficulty.md`, "Measuring it") over every (preset,
//! nation, seed) a caller names, with no input and with the standard opening
//! (`fn_sim::autoplay::Opening`), on Small. It prints one row per run: the minute of the first
//! death, deaths and population at the end, food per head on each winter's first tick, and the
//! median path length walked per meal in the 15-minute windows at minute 15 and minute 90.
//!
//! ```text
//! cargo run -p fn_harness --bin balance -- sweep --seeds 1-8 --minutes 180 [--presets ..] [--nations ..]
//! ```
//!
//! `--presets` and `--nations` take comma-separated ids (default: every preset in
//! `data/difficulty.toml`'s order; every nation). `--seeds` takes a comma-separated list of
//! numbers and `A-B` ranges (default: `1-8`).
//!
//! A single run is cheap, but this default sweep is not: 3 presets x 4 nations x 8 seeds x 2 modes
//! is 192 runs of up to 180 minutes each. It's a dev tool, run on demand, never part of `cargo
//! xtask ci` (`crates/sim/tests/balance.rs` is the cheap, CI-sized version of this same check).
//!
//! `supply` is the supply survey instead (`docs/design/materials.md`, "What the map holds" and
//! "Measuring it", #348): how much of each named deposit is reachable from
//! `Survey::default_site` on Small, per seed, at each named radius.
//!
//! ```text
//! cargo run -p fn_harness --bin balance -- supply --seeds 1-8
//!     [--radii 20,40] [--resources stone,gold_vein,gold_seam] [--bore-limit 2]
//! ```
//!
//! This is cheap (one map generation and one BFS per seed): unlike the sweep above, it's fine to
//! run it often while retuning `resources.toml` or `jobs.toml`.
//!
//! `mastery` is the mastery-pacing survey (#343, `docs/design/endgame.md`, "Mastery pacing"): the
//! standard opening, plus up to `--bend-slots` bends kept designated near the anchor every
//! in-game minute (`fn_sim::autoplay::Opening::with_bending`), re-run fresh for every named
//! checkpoint in `--minutes` (one run per checkpoint, since one run is cheap). Each row is the
//! three highest settlers' bend counts at that checkpoint and the minute the `masters` sign
//! (`data/endeavor.toml`) was first met, if by then.
//!
//! ```text
//! cargo run -p fn_harness --bin balance -- mastery --seeds 7,42
//!     --minutes 30,60,90,120,150,180,240 [--nations fire,earth,water,air]
//!     [--preset normal] [--bend-slots 6] [--full-research]
//! ```
//!
//! `--full-research` swaps the short `RESEARCH_PLAN` for the settlement's whole visible tech
//! tree (`fn_sim::autoplay::Opening::with_full_research`, #352): off by default, since the
//! mastery sweep predates it, but a caller comparing Earth's and Air's pace with and without
//! their own branch researched (Bore Tunnel, Gliders) can turn it on either way.
//!
//! `economy` is the economy opening (#352, `docs/design/materials.md` section 7):
//! `fn_sim::autoplay::Economy`, which plays toward the Harmony Gate's signs (stone, ore,
//! population, buildings, the nation's branch) rather than just surviving. One row per season
//! reached, for every named `(nation, seed)`, then the minute each sign was first met, the minute
//! each reachable building first stood, and what recipe items and ore are left in stores (#376);
//! then (#434, `docs/design/pacing.md` "Measuring it") each tech's completion minute (`techs:`),
//! the decision timeline (`timeline:`: every tech completion that unlocks a building, ability or
//! job, plus each sign's first-met minute, in minute order), the longest gap between decisions
//! from minute 30 to Ready and how many gaps are over 15 minutes, and the idle share (the
//! fraction of units with no task) per 15-minute window. A trailing line per nation gives how
//! many of its seeds keep that longest gap at 15 minutes or less.
//!
//! ```text
//! cargo run -p fn_harness --bin balance -- economy --seeds 7,42 --minutes 180
//!     [--nations fire,earth,water,air] [--preset normal] [--no-farms] [--branch-first]
//! ```
//!
//! `--no-farms` tills no farm plots and raises no orchard (materials.md's own `--no-farms`,
//! owner question 2's check). `--branch-first` (`fn_sim::autoplay::Economy::with_branch_first`)
//! researches the nation's own branch before the shared tier, closer to how a person plays
//! (pacing.md's own "a player who goes branch first" note); the default order is unchanged.
//! `verdict` replaces these two flags with play styles (`data/harness.toml`).
//!
//! `--score` (#526, `docs/design/agent-hillclimb.md`) scores each run with
//! `fn_sim::score::Scorecard::compute` at the end of the run and prints a `score:` line per
//! `(nation, seed)`: the score, progress, care, deaths, close calls and the suffering share.
//! `--horizon-minutes M` sets the run's length (and so the horizon) in place of `--minutes`;
//! with neither, the horizon is the run's own length. The same scoring gives a track's par
//! (`fn_sim::balance::par`).
//!
//! ```text
//! cargo run -p fn_harness --bin balance -- economy --score --seeds 7 --nations water
//!     [--horizon-minutes 180]
//! ```

use fn_sim::balance::{
    EconomyConfig, EconomyReport, PEOPLE, RunConfig, RunReport, run, run_economy,
    run_economy_scored,
};
use fn_sim::defs::ResourceId;
use fn_sim::embark::Survey;
use fn_sim::score::{ScoreDef, Scorecard};
use fn_sim::{Defs, Nation};

/// `sweep`: the no-input and opening sweep over presets, nations and seeds.
pub fn run_sweep(args: &[String]) {
    let mut seeds = parse_seeds("1-8");
    let mut minutes: f32 = 180.0;
    let mut preset_ids: Option<Vec<String>> = None;
    let mut nation_ids: Option<Vec<String>> = None;

    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--seeds" => seeds = parse_seeds(&value(args, &mut i)),
            "--minutes" => minutes = value(args, &mut i).parse().expect("--minutes needs a number"),
            "--presets" => {
                preset_ids = Some(value(args, &mut i).split(',').map(str::to_owned).collect())
            }
            "--nations" => {
                nation_ids = Some(value(args, &mut i).split(',').map(str::to_owned).collect())
            }
            other => usage_error(&format!("unknown argument {other:?}")),
        }
        i += 1;
    }

    let defs = Defs::bundled();
    let size = defs.embark.map_size("small").expect("bundled data always has a small map size");

    let presets: Vec<String> = preset_ids
        .unwrap_or_else(|| defs.difficulty.presets.iter().map(|p| p.id.clone()).collect());
    for preset in &presets {
        if defs.difficulty.preset(preset).is_none() {
            usage_error(&format!("unknown preset {preset:?}"));
        }
    }
    let nations: Vec<Nation> = nation_ids
        .map(|ids| ids.iter().map(|id| nation_from_id(id)).collect())
        .unwrap_or_else(|| Nation::ALL.to_vec());

    println!(
        "{:<8} {:<6} {:>4} {:<8} {:>11} {:>6} {:>3}  {:<28} {:>7} {:>7}",
        "preset",
        "nation",
        "seed",
        "mode",
        "1st_death",
        "deaths",
        "pop",
        "winter_food/head",
        "walk15",
        "walk90"
    );
    for preset in &presets {
        for &nation in &nations {
            for &seed in &seeds {
                for opening in [false, true] {
                    let cfg = RunConfig {
                        seed,
                        nation,
                        preset: Some(preset.clone()),
                        minutes,
                        opening,
                        bend_slots: None,
                        full_research: false,
                        width: size.width,
                        height: size.height,
                    };
                    print_row(preset, nation, seed, opening, &run(&cfg, &defs));
                }
            }
        }
    }
}

fn print_row(preset: &str, nation: Nation, seed: u64, opening: bool, r: &RunReport) {
    let mode = if opening { "opening" } else { "no-input" };
    let first_death = r.first_death_minute.map_or("none".to_owned(), |m| format!("{m:.1}"));
    let winter_food = r
        .winter_food_per_head
        .iter()
        .map(|(year, food)| format!("y{year}:{food:.1}"))
        .collect::<Vec<_>>()
        .join(",");
    let walk = |m: Option<f32>| m.map_or("-".to_owned(), |m| format!("{m:.1}"));
    println!(
        "{preset:<8} {:<6} {seed:>4} {mode:<8} {first_death:>11} {:>6} {:>3}  {winter_food:<28} {:>7} {:>7}",
        nation.as_str(),
        r.deaths,
        r.population,
        walk(r.walk_minute_15),
        walk(r.walk_minute_90),
    );
}

/// `--supply`'s own loop: one `Survey` per seed, reported at every radius (#348).
pub fn run_supply(args: &[String]) {
    let mut seeds = parse_seeds("1-8");
    let mut radii: Vec<i32> = vec![20, 40];
    let mut resource_ids = vec!["stone".to_owned(), "gold_vein".to_owned(), "gold_seam".to_owned()];
    let mut bore_limit: u32 = 2;

    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--seeds" => seeds = parse_seeds(&value(args, &mut i)),
            "--radii" => {
                radii = value(args, &mut i)
                    .split(',')
                    .map(|s| {
                        s.parse().unwrap_or_else(|_| supply_usage_error("--radii must be numbers"))
                    })
                    .collect()
            }
            "--resources" => {
                resource_ids = value(args, &mut i).split(',').map(str::to_owned).collect()
            }
            "--bore-limit" => {
                bore_limit = value(args, &mut i)
                    .parse()
                    .unwrap_or_else(|_| supply_usage_error("--bore-limit needs a number"))
            }
            other => supply_usage_error(&format!("unknown argument {other:?}")),
        }
        i += 1;
    }

    let defs = Defs::bundled();
    let size = defs.embark.map_size("small").expect("bundled data always has a small map size");
    let resources: Vec<ResourceId> = resource_ids
        .iter()
        .map(|id| {
            let rid = defs
                .resource_id(id)
                .unwrap_or_else(|| supply_usage_error(&format!("unknown resource {id:?}")));
            if defs.harvest(rid).is_none() {
                supply_usage_error(&format!("{id:?} has no harvest yield in data/jobs.toml"));
            }
            rid
        })
        .collect();

    println!(
        "{:<6} {:>6} {:<12} {:>7} {:>9} {:>7}",
        "seed", "radius", "resource", "count", "tunnelled", "worth"
    );
    for &seed in &seeds {
        let cfg = Survey::config(seed, size);
        let survey = Survey::new(&cfg, &defs);
        let start = survey.default_site(&defs);
        for radius in survey.supply(start, &resources, &radii, bore_limit, &defs) {
            for deposit in &radius.deposits {
                println!(
                    "{seed:<6} {:>6} {:<12} {:>7} {:>9} {:>7}",
                    radius.radius,
                    defs.resource(deposit.resource).id,
                    deposit.count,
                    deposit.tunnelled,
                    deposit.worth,
                );
            }
        }
    }
}

fn supply_usage_error(msg: &str) -> ! {
    eprintln!("{msg}");
    eprintln!(
        "usage: balance supply [--seeds 1-8] [--radii 20,40] [--resources stone,gold_vein,gold_seam] [--bore-limit 2]"
    );
    std::process::exit(1);
}

/// `--mastery`'s own loop: one run per `(nation, seed, minutes checkpoint)`, re-run fresh for
/// each checkpoint (#343).
pub fn run_mastery(args: &[String]) {
    let mut seeds = parse_seeds("7,42");
    let mut checkpoints: Vec<f32> = vec![30.0, 60.0, 90.0, 120.0, 150.0, 180.0, 240.0];
    let mut nation_ids: Option<Vec<String>> = None;
    let mut preset = "normal".to_owned();
    let mut bend_slots: u32 = PEOPLE;
    let mut full_research = false;

    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--seeds" => seeds = parse_seeds(&value(args, &mut i)),
            "--minutes" => {
                checkpoints = value(args, &mut i)
                    .split(',')
                    .map(|s| {
                        s.parse()
                            .unwrap_or_else(|_| mastery_usage_error("--minutes must be numbers"))
                    })
                    .collect()
            }
            "--nations" => {
                nation_ids = Some(value(args, &mut i).split(',').map(str::to_owned).collect())
            }
            "--preset" => preset = value(args, &mut i),
            "--bend-slots" => {
                bend_slots = value(args, &mut i)
                    .parse()
                    .unwrap_or_else(|_| mastery_usage_error("--bend-slots needs a number"))
            }
            "--full-research" => full_research = true,
            other => mastery_usage_error(&format!("unknown argument {other:?}")),
        }
        i += 1;
    }

    let defs = Defs::bundled();
    let size = defs.embark.map_size("small").expect("bundled data always has a small map size");
    if defs.difficulty.preset(&preset).is_none() {
        mastery_usage_error(&format!("unknown preset {preset:?}"));
    }
    let nations: Vec<Nation> = nation_ids
        .map(|ids| ids.iter().map(|id| nation_from_id(id)).collect())
        .unwrap_or_else(|| Nation::ALL.to_vec());

    println!(
        "{:<6} {:>4} {:>7}  {:>6} {:>6} {:>6}  {:>8}",
        "nation", "seed", "minute", "1st", "2nd", "3rd", "masters"
    );
    for &nation in &nations {
        for &seed in &seeds {
            for &minutes in &checkpoints {
                let cfg = RunConfig {
                    seed,
                    nation,
                    preset: Some(preset.clone()),
                    minutes,
                    opening: true,
                    bend_slots: Some(bend_slots),
                    full_research,
                    width: size.width,
                    height: size.height,
                };
                print_mastery_row(nation, seed, minutes, &run(&cfg, &defs));
            }
        }
    }
}

fn print_mastery_row(nation: Nation, seed: u64, minutes: f32, r: &RunReport) {
    let bends = |i: usize| r.top_bends.get(i).copied().unwrap_or(0);
    let masters = r.masters_minute.map_or("none".to_owned(), |m| format!("{m:.1}"));
    println!(
        "{:<6} {seed:>4} {minutes:>7.1}  {:>6} {:>6} {:>6}  {masters:>8}",
        nation.as_str(),
        bends(0),
        bends(1),
        bends(2),
    );
}

fn mastery_usage_error(msg: &str) -> ! {
    eprintln!("{msg}");
    eprintln!(
        "usage: balance mastery [--seeds 7,42] [--minutes 30,60,90,120,150,180,240] [--nations fire,earth,water,air] [--preset normal] [--bend-slots 6]"
    );
    std::process::exit(1);
}

fn value(args: &[String], i: &mut usize) -> String {
    let flag = args[*i].clone();
    *i += 1;
    args.get(*i).cloned().unwrap_or_else(|| usage_error(&format!("{flag} needs a value")))
}

fn parse_seeds(s: &str) -> Vec<u64> {
    s.split(',')
        .flat_map(|part| match part.split_once('-') {
            Some((a, b)) => {
                let a: u64 = a.parse().unwrap_or_else(|_| usage_error("--seeds must be numbers"));
                let b: u64 = b.parse().unwrap_or_else(|_| usage_error("--seeds must be numbers"));
                a..=b
            }
            None => {
                let n: u64 =
                    part.parse().unwrap_or_else(|_| usage_error("--seeds must be numbers"));
                n..=n
            }
        })
        .collect()
}

fn nation_from_id(id: &str) -> Nation {
    Nation::ALL
        .into_iter()
        .find(|n| n.as_str() == id)
        .unwrap_or_else(|| usage_error(&format!("unknown nation {id:?}")))
}

fn usage_error(msg: &str) -> ! {
    eprintln!("{msg}");
    eprintln!(
        "usage: balance sweep [--seeds 1-8] [--minutes 180] [--presets sandbox,normal,hard] [--nations air,water,earth,fire]"
    );
    std::process::exit(1);
}

/// `--economy`'s own loop: one run per `(nation, seed)`, driving `fn_sim::autoplay::Economy`
/// (#352, `docs/design/materials.md` section 7). Each run reports one row per season reached,
/// plus a trailing row with the minute every sign was first met (or "none").
pub fn run_economy_sweep(args: &[String]) {
    let mut seeds = parse_seeds("7,42");
    let mut minutes: f32 = 180.0;
    let mut nation_ids: Option<Vec<String>> = None;
    let mut preset = "normal".to_owned();
    let mut farms = true;
    let mut branch_first = false;
    let mut score = false;
    let mut horizon_minutes: Option<f32> = None;

    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--seeds" => seeds = parse_seeds(&value(args, &mut i)),
            "--minutes" => minutes = value(args, &mut i).parse().expect("--minutes needs a number"),
            "--score" => score = true,
            "--horizon-minutes" => {
                horizon_minutes =
                    Some(value(args, &mut i).parse().unwrap_or_else(|_| {
                        economy_usage_error("--horizon-minutes needs a number")
                    }))
            }
            "--nations" => {
                nation_ids = Some(value(args, &mut i).split(',').map(str::to_owned).collect())
            }
            "--preset" => preset = value(args, &mut i),
            "--no-farms" => farms = false,
            "--branch-first" => branch_first = true,
            other => economy_usage_error(&format!("unknown argument {other:?}")),
        }
        i += 1;
    }

    if horizon_minutes.is_some() && !score {
        economy_usage_error("--horizon-minutes only goes with --score");
    }
    if let Some(horizon) = horizon_minutes {
        minutes = horizon;
    }
    let score_def = score.then(ScoreDef::bundled);

    let defs = Defs::bundled();
    let size = defs.embark.map_size("small").expect("bundled data always has a small map size");
    if defs.difficulty.preset(&preset).is_none() {
        economy_usage_error(&format!("unknown preset {preset:?}"));
    }
    let nations: Vec<Nation> = nation_ids
        .map(|ids| ids.iter().map(|id| nation_from_id(id)).collect())
        .unwrap_or_else(|| Nation::ALL.to_vec());

    println!(
        "{:<6} {:>4} {:>4} {:>7}  {:>6} {:>6} {:>6} {:>6} {:>4}  signs",
        "nation", "seed", "year", "minute", "stone", "ore", "wood", "food/h", "pop"
    );
    for &nation in &nations {
        // How many of this nation's seeds keep the longest decision gap (minute 30 to Ready)
        // at or under 15 minutes (`docs/design/pacing.md`'s own pacing target; #434's per-nation
        // summary line).
        let mut gap_ok = 0usize;
        for &seed in &seeds {
            let cfg = EconomyConfig {
                seed,
                nation,
                preset: Some(preset.clone()),
                minutes,
                farms,
                branch_first,
                width: size.width,
                height: size.height,
            };
            let (report, card) = match &score_def {
                Some(score_def) => {
                    let (report, card) = run_economy_scored(&cfg, &defs, score_def);
                    (report, Some(card))
                }
                None => (run_economy(&cfg, &defs), None),
            };
            for season in &report.seasons {
                println!(
                    "{:<6} {seed:>4} {:>4} {:>7.1}  {:>6} {:>6} {:>6} {:>6.1} {:>4}  {}",
                    nation.as_str(),
                    season.year,
                    season.minute,
                    season.stone_stored,
                    season.ore_mined,
                    season.wood_stored,
                    season.food_per_head,
                    season.population,
                    season.signs_met.join(","),
                );
            }
            print_sign_minutes(nation, seed, &report);
            print_timeline(nation, seed, &report);
            print_idle_share(nation, seed, &report);
            if let Some(card) = &card {
                print_score(nation, seed, card);
            }
            if report.longest_gap.is_none_or(|(start, end)| end - start <= 15.0) {
                gap_ok += 1;
            }
        }
        println!(
            "{:<6} longest decision gap 15 min or less on {gap_ok}/{} seeds",
            nation.as_str(),
            seeds.len()
        );
    }
}

fn print_sign_minutes(nation: Nation, seed: u64, report: &EconomyReport) {
    let signs = report
        .sign_minutes
        .iter()
        .map(|(id, m)| format!("{id}={}", m.map_or("none".to_owned(), |m| format!("{m:.1}"))))
        .collect::<Vec<_>>()
        .join(" ");
    println!("{:<6} {seed:>4} signs: {signs}", nation.as_str());
    let first_death = report.first_death_minute.map_or("none".to_owned(), |m| format!("{m:.1}"));
    println!("{:<6} {seed:>4} deaths: {} (first @ {first_death})", nation.as_str(), report.deaths);
    let built = report
        .first_built
        .iter()
        .map(|(id, m)| format!("{id}={}", m.map_or("none".to_owned(), |m| format!("{m:.1}"))))
        .collect::<Vec<_>>()
        .join(" ");
    println!("{:<6} {seed:>4} built: {built}", nation.as_str());
    let stores = report
        .recipe_stores
        .iter()
        .map(|(id, n)| format!("{id}={n}"))
        .collect::<Vec<_>>()
        .join(" ");
    println!("{:<6} {seed:>4} in stores at the end: {stores}", nation.as_str());
}

/// `techs:`, `timeline:` and the longest-gap line (module doc; #434, `docs/design/pacing.md`
/// "Measuring it").
fn print_timeline(nation: Nation, seed: u64, report: &EconomyReport) {
    let techs = report
        .techs
        .iter()
        .map(|(id, minute)| format!("{id}={minute:.1}"))
        .collect::<Vec<_>>()
        .join(" ");
    println!("{:<6} {seed:>4} techs: {techs}", nation.as_str());
    let timeline = report
        .timeline
        .iter()
        .map(|e| format!("{}@{:.1}", e.label, e.minute))
        .collect::<Vec<_>>()
        .join(" | ");
    println!("{:<6} {seed:>4} timeline: {timeline}", nation.as_str());
    let gap = match report.longest_gap {
        Some((start, end)) => format!("{start:.1}-{end:.1} ({:.1} min)", end - start),
        None => "none".to_owned(),
    };
    println!(
        "{:<6} {seed:>4} longest gap: {gap}, gaps over 15 min: {}",
        nation.as_str(),
        report.gaps_over_15
    );
}

/// The `score:` line (module doc; #526): the run's `Scorecard` in one row.
fn print_score(nation: Nation, seed: u64, card: &Scorecard) {
    let c = &card.care;
    println!(
        "{:<6} {seed:>4} score: {:.1} (progress {:.3} x care {:.2}) deaths {} close calls {} suffering {:.1}%",
        nation.as_str(),
        card.score,
        card.progress,
        c.value,
        c.deaths,
        c.close_calls,
        c.suffering * 100.0,
    );
}

/// The idle line: the share of unit samples with no task, per 15-minute window (module doc;
/// #434).
fn print_idle_share(nation: Nation, seed: u64, report: &EconomyReport) {
    let idle = report
        .idle_share
        .iter()
        .map(|&(start, share)| format!("{start:.0}-{:.0}={:.0}%", start + 15.0, share * 100.0))
        .collect::<Vec<_>>()
        .join(" ");
    println!("{:<6} {seed:>4} idle: {idle}", nation.as_str());
}

fn economy_usage_error(msg: &str) -> ! {
    eprintln!("{msg}");
    eprintln!(
        "usage: balance economy [--seeds 7,42] [--minutes 180] [--nations fire,earth,water,air] [--preset normal] [--no-farms] [--branch-first] [--score [--horizon-minutes M]]"
    );
    std::process::exit(1);
}
