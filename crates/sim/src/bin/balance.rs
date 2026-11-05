//! Runs the balance harness (#300, `docs/design/difficulty.md`, "Measuring it") over every
//! (preset, nation, seed) a caller names, with no input and with the standard opening
//! (`fn_sim::autoplay::Opening`), on Small. Prints one row per run: the minute of the first
//! death, deaths and population at the end, food per head on each winter's first tick, and the
//! median path length walked per meal in the 15-minute windows at minute 15 and minute 90.
//!
//!     cargo run -p fn_sim --bin balance -- --seeds 1-8 --minutes 180 [--presets ..] [--nations ..]
//!
//! `--presets` and `--nations` take comma-separated ids (default: every preset in
//! `data/difficulty.toml`'s order; every nation). `--seeds` takes a comma-separated list of
//! numbers and `A-B` ranges (default: `1-8`).
//!
//! A single run is cheap (`fn_sim::balance::run`'s own doc), but this binary's default sweep is
//! not: 3 presets x 4 nations x 8 seeds x 2 modes is 192 runs of up to 180 minutes each. It's a
//! dev tool, run on demand, never part of `cargo xtask ci` (`crates/sim/tests/balance.rs` is the
//! cheap, CI-sized version of this same check).
//!
//! `--supply` switches to the supply survey instead (`docs/design/materials.md`, "What the map
//! holds" and "Measuring it", #348): how much of each named deposit is reachable from
//! `Survey::default_site` on Small, per seed, at each named radius.
//!
//!     cargo run -p fn_sim --bin balance -- --supply --seeds 1-8
//!         [--radii 20,40] [--resources stone,gold_vein,gold_seam] [--bore-limit 2]
//!
//! This is cheap (one map generation and one BFS per seed): unlike the sweep above, it's fine to
//! run it often while retuning `resources.toml` or `jobs.toml`.
//!
//! `--mastery` switches to the mastery-pacing survey instead (#343, `docs/design/endgame.md`,
//! "Mastery pacing"): the standard opening, plus up to `--bend-slots` bends kept designated near
//! the anchor every in-game minute (`fn_sim::autoplay::Opening::with_bending`), re-run fresh for
//! every named checkpoint in `--minutes` (one run per checkpoint, since one run is cheap). Each
//! row is the three highest settlers' bend counts at that checkpoint and the minute the
//! `masters` sign (`data/endeavor.toml`) was first met, if by then.
//!
//!     cargo run -p fn_sim --bin balance -- --mastery --seeds 7,42
//!         --minutes 30,60,90,120,150,180,240 [--nations fire,earth,water,air]
//!         [--preset normal] [--bend-slots 6]

use fn_sim::balance::{PEOPLE, RunConfig, RunReport, run};
use fn_sim::defs::ResourceId;
use fn_sim::embark::Survey;
use fn_sim::{Defs, Nation};

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.first().map(String::as_str) == Some("--supply") {
        run_supply(&args[1..]);
        return;
    }
    if args.first().map(String::as_str) == Some("--mastery") {
        run_mastery(&args[1..]);
        return;
    }
    let mut seeds = parse_seeds("1-8");
    let mut minutes: f32 = 180.0;
    let mut preset_ids: Option<Vec<String>> = None;
    let mut nation_ids: Option<Vec<String>> = None;

    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--seeds" => seeds = parse_seeds(&value(&args, &mut i)),
            "--minutes" => {
                minutes = value(&args, &mut i).parse().expect("--minutes needs a number")
            }
            "--presets" => {
                preset_ids = Some(value(&args, &mut i).split(',').map(str::to_owned).collect())
            }
            "--nations" => {
                nation_ids = Some(value(&args, &mut i).split(',').map(str::to_owned).collect())
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
fn run_supply(args: &[String]) {
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
        "usage: balance --supply [--seeds 1-8] [--radii 20,40] [--resources stone,gold_vein,gold_seam] [--bore-limit 2]"
    );
    std::process::exit(1);
}

/// `--mastery`'s own loop: one run per `(nation, seed, minutes checkpoint)`, re-run fresh for
/// each checkpoint (#343).
fn run_mastery(args: &[String]) {
    let mut seeds = parse_seeds("7,42");
    let mut checkpoints: Vec<f32> = vec![30.0, 60.0, 90.0, 120.0, 150.0, 180.0, 240.0];
    let mut nation_ids: Option<Vec<String>> = None;
    let mut preset = "normal".to_owned();
    let mut bend_slots: u32 = PEOPLE;

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
        "usage: balance --mastery [--seeds 7,42] [--minutes 30,60,90,120,150,180,240] [--nations fire,earth,water,air] [--preset normal] [--bend-slots 6]"
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
        "usage: balance [--seeds 1-8] [--minutes 180] [--presets sandbox,normal,hard] [--nations air,water,earth,fire]"
    );
    std::process::exit(1);
}
