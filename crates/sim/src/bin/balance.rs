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

use fn_sim::balance::{RunConfig, RunReport, run};
use fn_sim::{Defs, Nation};

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
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
