//! The `balance` binary's command line (#537): a subcommand table, so a new one (`trace`, #535)
//! is one line in [`COMMANDS`] and a module.
//!
//! ```text
//! balance verdict [--tier quick|full|holdout] [--seeds 1-32] [--styles ..] [--presets ..]
//!                 [--nations ..] [--threads 4] [--label NAME] [--out DIR] [--minutes M]
//!                 [--against FILE] [--bless]
//! balance jitter [--tier ..] [--seeds ..] [--threads N] [--dry-run]
//! balance trace --seed S --nation N [--style ID] [--preset P] --from M --to M [--map-at M]
//! balance sweep | supply | mastery | economy ...      (the older modes, `legacy`)
//! ```
//!
//! `verdict` plays the panel `data/harness.toml`'s targets need, writes the run records to
//! `target/balance/LABEL.json`, prints the report to stdout (progress and timing go to stderr, so
//! two runs of a panel print the same bytes) and exits 1 if any target fails. `--against` pairs the
//! runs with an earlier set and `--bless` writes them as the new committed baseline (#538).

use std::path::PathBuf;
use std::process::ExitCode;
use std::time::Instant;

use fn_sim::{Defs, Nation};

use crate::baseline::{self, BASELINE_FILE, Set};
use crate::config::{Config, parse_seeds};
use crate::jitter;
use crate::legacy;
use crate::regress;
use crate::report::{self, Header};
use crate::runner::{self, Filter};
use crate::tier::Tier;
use crate::trace;
use crate::verdict;

type Handler = fn(&[String]) -> ExitCode;

/// Every subcommand: name, one-line summary, handler.
const COMMANDS: &[(&str, &str, Handler)] = &[
    ("verdict", "run the panel and judge it against data/harness.toml's targets", cmd_verdict),
    ("jitter", "replay the panel one tick late and write each metric's noise floor", cmd_jitter),
    ("trace", "rerun one run: per-minute probe rows and an ASCII crop of its map", trace::cmd),
    ("sweep", "the no-input and opening sweep over presets, nations and seeds", |a| {
        legacy::run_sweep(a);
        ExitCode::SUCCESS
    }),
    ("supply", "the supply survey: what the map holds within reach", |a| {
        legacy::run_supply(a);
        ExitCode::SUCCESS
    }),
    ("mastery", "mastery pacing: bends per settler at checkpoints", |a| {
        legacy::run_mastery(a);
        ExitCode::SUCCESS
    }),
    ("economy", "the economy opening: one row per season, signs, timeline, idle", |a| {
        legacy::run_economy_sweep(a);
        ExitCode::SUCCESS
    }),
];

/// The older flag spellings (`balance --supply ...`) and the subcommand each stands for.
const LEGACY_FLAGS: &[(&str, &str)] =
    &[("--supply", "supply"), ("--mastery", "mastery"), ("--economy", "economy")];

fn help() -> String {
    let mut out = String::from("usage: balance <command> [args]\n\ncommands:\n");
    for (name, summary, _) in COMMANDS {
        out.push_str(&format!("  {name:<9} {summary}\n"));
    }
    out.push_str("\n`balance <command> --help` for a command's options.\n");
    out
}

fn verdict_help() -> &'static str {
    "usage: balance verdict [--tier quick|full|holdout] [--seeds 1-32] [--styles ID,..]\n\
     \x20                      [--presets ID,..] [--nations ID,..] [--threads N] [--label NAME]\n\
     \x20                      [--out DIR] [--minutes M] [--against FILE] [--bless]\n\n\
     Runs the panel data/harness.toml's targets ask for, writes the run records to\n\
     DIR/LABEL.json (default target/balance/<git short hash>.json), prints the verdict and\n\
     exits 1 if a target fails.\n\n\
     --tier      quick (seeds 1-12, steady, Normal), full (the panel; the default) or holdout\n\
     \x20           (the full tier on seeds 1001-1032)\n\
     --against   pair the runs with an earlier set (crates/harness/baseline.json, or the\n\
     \x20           records of another run) and print, per metric, how many runs moved earlier,\n\
     \x20           later or not at all, the median delta and whether it moved past its noise.\n\
     \x20           Warns if the set was made under other data; it never refuses\n\
     --bless     write this run as the committed baseline (crates/harness/baseline.json).\n\
     \x20           Only for the unnarrowed full tier: no --tier, --seeds, --styles, --presets,\n\
     \x20           --nations or --minutes\n\
     --seeds     seed list (default: [panel] seeds, 1-32); targets may cap it with first_seeds\n\
     --styles    only these play styles' runs (other targets report no data)\n\
     --presets   only these difficulty presets' runs\n\
     --nations   only these nations\n\
     --threads   worker threads (default: [panel] threads, 4)\n\
     --minutes   run every run this long instead (a smoke test; later readings report no data)\n"
}

/// Runs the binary: `args` is `std::env::args()` without the program name.
pub fn main(args: Vec<String>) -> ExitCode {
    let Some(first) = args.first().map(String::as_str) else {
        eprint!("{}", help());
        return ExitCode::from(2);
    };
    if matches!(first, "-h" | "--help" | "help") {
        print!("{}", help());
        return ExitCode::SUCCESS;
    }
    if let Some((_, name)) = LEGACY_FLAGS.iter().find(|(flag, _)| *flag == first) {
        return dispatch(name, &args[1..]);
    }
    if first.starts_with("--") {
        // The sweep was the binary's only mode once: `balance --seeds 1-8 ...`.
        return dispatch("sweep", &args);
    }
    dispatch(first, &args[1..])
}

fn dispatch(name: &str, rest: &[String]) -> ExitCode {
    match COMMANDS.iter().find(|(n, _, _)| *n == name) {
        Some((_, _, handler)) => handler(rest),
        None => {
            eprintln!("unknown command {name:?}\n\n{}", help());
            ExitCode::from(2)
        }
    }
}

/// What `balance verdict` was asked for.
#[derive(Debug, Default, PartialEq)]
struct VerdictArgs {
    seeds: Option<String>,
    styles: Option<Vec<String>>,
    presets: Option<Vec<String>>,
    nations: Option<Vec<String>>,
    threads: Option<usize>,
    label: Option<String>,
    out: Option<PathBuf>,
    minutes: Option<f32>,
    tier: Option<String>,
    against: Option<PathBuf>,
    bless: bool,
    help: bool,
}

fn parse_verdict_args(args: &[String]) -> Result<VerdictArgs, String> {
    let mut out = VerdictArgs::default();
    let mut it = args.iter();
    while let Some(flag) = it.next() {
        let mut value = || it.next().cloned().ok_or(format!("{flag} needs a value"));
        let list = |v: String| v.split(',').map(str::to_owned).collect::<Vec<_>>();
        match flag.as_str() {
            "--seeds" => out.seeds = Some(value()?),
            "--styles" => out.styles = Some(list(value()?)),
            "--presets" => out.presets = Some(list(value()?)),
            "--nations" => out.nations = Some(list(value()?)),
            "--threads" => {
                out.threads = Some(
                    value()?.parse().map_err(|_| "--threads needs a whole number".to_owned())?,
                )
            }
            "--label" => out.label = Some(value()?),
            "--out" => out.out = Some(PathBuf::from(value()?)),
            "--minutes" => {
                out.minutes =
                    Some(value()?.parse().map_err(|_| "--minutes needs a number".to_owned())?)
            }
            "--tier" => out.tier = Some(value()?),
            "--against" => out.against = Some(PathBuf::from(value()?)),
            "--bless" => out.bless = true,
            "-h" | "--help" => out.help = true,
            other => return Err(format!("unknown argument {other:?}")),
        }
    }
    Ok(out)
}

/// `--tier NAME`, or `None` if it wasn't given.
fn parse_tier(name: &Option<String>) -> Result<Option<Tier>, String> {
    name.as_deref()
        .map(|n| {
            Tier::parse(n).ok_or_else(|| {
                let known: Vec<_> = Tier::ALL.iter().map(|t| t.name()).collect();
                format!("unknown tier {n:?} (known: {})", known.join(", "))
            })
        })
        .transpose()
}

/// Why `--bless` can't be used with these arguments, if it can't: a baseline written from a
/// narrowed run would silently drop the rest of the panel from every later comparison.
fn bless_refusal(a: &VerdictArgs, tier: Option<Tier>) -> Option<String> {
    let narrowed: Vec<&str> = [
        ("--seeds", a.seeds.is_some()),
        ("--styles", a.styles.is_some()),
        ("--presets", a.presets.is_some()),
        ("--nations", a.nations.is_some()),
        ("--minutes", a.minutes.is_some()),
        ("--tier", tier.is_some_and(|t| t != Tier::Full)),
    ]
    .into_iter()
    .filter_map(|(flag, given)| given.then_some(flag))
    .collect();
    (a.bless && !narrowed.is_empty()).then(|| {
        format!(
            "--bless writes the whole full tier, so it can't be combined with {}",
            narrowed.join(" or ")
        )
    })
}

/// The short hash of HEAD, or `local` if git can't say.
fn git_label() -> String {
    std::process::Command::new("git")
        .args(["rev-parse", "--short", "HEAD"])
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_owned())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "local".to_owned())
}

/// The committed baseline: `crates/harness/baseline.json`, wherever the binary is run from.
fn baseline_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(BASELINE_FILE)
}

/// `data/harness.toml` of this workspace, which `balance jitter` writes `[noise]` into.
fn harness_toml_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../data/harness.toml")
}

/// `target/balance` of this workspace, wherever the binary is run from.
fn default_out() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../target/balance")
}

fn cmd_verdict(args: &[String]) -> ExitCode {
    match run_verdict(args) {
        Ok(code) => code,
        Err(msg) => {
            eprintln!("{msg}\n\n{}", verdict_help());
            ExitCode::from(2)
        }
    }
}

fn run_verdict(args: &[String]) -> Result<ExitCode, String> {
    let a = parse_verdict_args(args)?;
    if a.help {
        print!("{}", verdict_help());
        return Ok(ExitCode::SUCCESS);
    }
    let defs = Defs::bundled();
    let config = Config::bundled(&defs);
    let tier = parse_tier(&a.tier)?;
    if let Some(why) = bless_refusal(&a, tier) {
        return Err(why);
    }

    let seeds_text = a.seeds.clone().unwrap_or_else(|| match tier {
        Some(t) => t.seeds(&config),
        None => config.panel.seeds.clone(),
    });
    let seeds = parse_seeds(&seeds_text)
        .filter(|s| !s.is_empty())
        .ok_or_else(|| format!("--seeds {seeds_text:?} is not a seed list like 1-32 or 7,42"))?;
    let nations: Option<Vec<Nation>> = a
        .nations
        .as_ref()
        .map(|ids| {
            ids.iter()
                .map(|id| {
                    Nation::ALL
                        .iter()
                        .copied()
                        .find(|n| n.as_str() == id)
                        .ok_or_else(|| format!("unknown nation {id:?}"))
                })
                .collect::<Result<Vec<_>, _>>()
        })
        .transpose()?;
    for (what, list, known) in [
        ("style", &a.styles, config.styles.iter().map(|s| s.id.clone()).collect::<Vec<_>>()),
        (
            "preset",
            &a.presets,
            defs.difficulty.presets.iter().map(|p| p.id.clone()).collect::<Vec<_>>(),
        ),
    ] {
        for id in list.iter().flatten() {
            if !known.contains(id) {
                return Err(format!("unknown {what} {id:?} (known: {})", known.join(", ")));
            }
        }
    }
    if a.threads == Some(0) {
        return Err("--threads must be at least 1".to_owned());
    }
    let threads = a.threads.unwrap_or(config.panel.threads);

    let from_tier = tier.map(Tier::filter).unwrap_or_default();
    let filter = Filter {
        styles: a.styles.clone().or(from_tier.styles),
        presets: a.presets.clone().or(from_tier.presets),
        nations: nations.clone(),
        minutes: a.minutes,
    };
    // Read the comparison set before spending minutes on the runs, so a bad path fails at once.
    let against = a
        .against
        .as_ref()
        .map(|path| {
            let text = std::fs::read_to_string(path)
                .map_err(|e| format!("can't read {}: {e}", path.display()))?;
            Set::parse(&text, &config, &defs).map_err(|e| format!("{}: {e}", path.display()))
        })
        .transpose()?;
    let jobs = runner::plan(&config, &seeds, &filter);
    if jobs.is_empty() {
        return Err("nothing to run: no target uses those styles, presets and nations".to_owned());
    }

    let started = Instant::now();
    eprintln!("balance verdict: {} runs on {threads} threads", jobs.len());
    let tenth = jobs.len().div_ceil(10).max(1);
    let progress = move |done: usize, total: usize| {
        if done.is_multiple_of(tenth) || done == total {
            eprintln!("  {done}/{total} runs, {:.0}s", started.elapsed().as_secs_f32());
        }
    };
    let runs = runner::run_all(&jobs, &config, &defs, threads, &progress);

    let label = a.label.clone().unwrap_or_else(git_label);
    let dir = a.out.clone().unwrap_or_else(default_out);
    let path = runner::write_json(&runs, &dir, &label)
        .map_err(|e| format!("can't write {}: {e}", dir.display()))?;

    let verdict = verdict::judge(&config, &runs, &defs);
    let header = Header {
        label,
        seeds: seeds_text,
        nations: nations.unwrap_or_else(|| Nation::ALL.to_vec()),
    };
    print!("{}", report::render(&verdict, &header));

    let now = (against.is_some() || a.bless).then(|| Set::from_runs(&runs, &config, &defs));
    if let (Some(then), Some(now), Some(path)) = (&against, &now, &a.against) {
        let comparison = regress::compare(&then.entries, &now.entries, &config);
        let digest = baseline::data_digest(&config);
        let note = regress::stale_note(then.digest.as_deref(), &digest);
        print!("{}", regress::render(&comparison, &path.display().to_string(), note.as_deref()));
    }
    if let (true, Some(now)) = (a.bless, &now) {
        let path = baseline_path();
        std::fs::write(&path, now.to_json())
            .map_err(|e| format!("can't write {}: {e}", path.display()))?;
        eprintln!("blessed {} runs: {}", now.entries.len(), path.display());
    }
    eprintln!("records: {}  ({:.1}s)", path.display(), started.elapsed().as_secs_f32());
    Ok(if verdict.failed() { ExitCode::from(1) } else { ExitCode::SUCCESS })
}

fn jitter_help() -> &'static str {
    "usage: balance jitter [--tier quick|full|holdout] [--seeds 1-32] [--threads N] [--dry-run]\n\n\
     Plays every run of the panel twice, once as usual and once with each of the opening's\n\
     steps one tick late, and takes each metric's median absolute difference between a run\n\
     and its twin as that metric's noise. Writes the values to data/harness.toml's [noise]\n\
     table (comments and layout stay as they are) unless --dry-run. Styles that play nothing\n\
     (idle) have no steps to delay and are left out. Takes about twice a full verdict run.\n"
}

fn cmd_jitter(args: &[String]) -> ExitCode {
    match run_jitter(args) {
        Ok(()) => ExitCode::SUCCESS,
        Err(msg) => {
            eprintln!("{msg}\n\n{}", jitter_help());
            ExitCode::from(2)
        }
    }
}

fn run_jitter(args: &[String]) -> Result<(), String> {
    let (mut tier, mut seeds, mut threads, mut dry_run) = (None, None, None, false);
    let mut it = args.iter();
    while let Some(flag) = it.next() {
        let mut value = || it.next().cloned().ok_or(format!("{flag} needs a value"));
        match flag.as_str() {
            "--tier" => tier = Some(value()?),
            "--seeds" => seeds = Some(value()?),
            "--threads" => {
                threads =
                    Some(value()?.parse::<usize>().map_err(|_| "--threads needs a whole number")?)
            }
            "--dry-run" => dry_run = true,
            "-h" | "--help" => {
                print!("{}", jitter_help());
                return Ok(());
            }
            other => return Err(format!("unknown argument {other:?}")),
        }
    }
    let defs = Defs::bundled();
    let config = Config::bundled(&defs);
    let tier = parse_tier(&tier)?.unwrap_or(Tier::Full);
    let seeds_text = seeds.unwrap_or_else(|| tier.seeds(&config));
    let seeds = parse_seeds(&seeds_text)
        .filter(|s| !s.is_empty())
        .ok_or_else(|| format!("--seeds {seeds_text:?} is not a seed list like 1-32 or 7,42"))?;
    if threads == Some(0) {
        return Err("--threads must be at least 1".to_owned());
    }
    let threads = threads.unwrap_or(config.panel.threads);

    let plain = jitter::jobs(&config, &seeds, &tier.filter());
    if plain.is_empty() {
        return Err("nothing to run".to_owned());
    }
    let twins: Vec<_> = plain.iter().map(|j| runner::Job { step_offset: 1, ..j.clone() }).collect();

    let started = Instant::now();
    eprintln!(
        "balance jitter: {} runs ({} twice) on {threads} threads",
        2 * plain.len(),
        plain.len()
    );
    let tenth = plain.len().div_ceil(10).max(1);
    let progress = move |done: usize, total: usize| {
        if done.is_multiple_of(tenth) || done == total {
            eprintln!("  {done}/{total} runs, {:.0}s", started.elapsed().as_secs_f32());
        }
    };
    let plain_runs = runner::run_all(&plain, &config, &defs, threads, &progress);
    eprintln!("  twins:");
    let twin_runs = runner::run_all(&twins, &config, &defs, threads, &progress);
    let metrics = baseline::metric_list(&config);
    let entries = |runs: &[verdict::Run]| -> Vec<baseline::Entry> {
        runs.iter().map(|r| baseline::Entry::of(r, &metrics, &defs)).collect()
    };
    let noise = jitter::noise_floor(&entries(&plain_runs), &entries(&twin_runs));

    println!("noise        median absolute difference between a run and its one-tick-late twin");
    println!("             ({} runs each, seeds {seeds_text}, {} tier)", plain.len(), tier.name());
    for (name, v) in &noise {
        println!("  {name:<28} {v}");
    }
    if dry_run {
        eprintln!(
            "--dry-run: {} not written ({:.0}s)",
            harness_toml_path().display(),
            started.elapsed().as_secs_f32()
        );
        return Ok(());
    }
    let path = harness_toml_path();
    let text = std::fs::read_to_string(&path)
        .map_err(|e| format!("can't read {}: {e}", path.display()))?;
    std::fs::write(&path, jitter::set_noise(&text, &noise))
        .map_err(|e| format!("can't write {}: {e}", path.display()))?;
    eprintln!("wrote [noise] to {} ({:.0}s)", path.display(), started.elapsed().as_secs_f32());
    Ok(())
}

#[cfg(test)]
mod tests;
