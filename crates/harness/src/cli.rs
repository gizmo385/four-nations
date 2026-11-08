//! The `balance` binary's command line (#537): a subcommand table, so a new one (`trace`, #535)
//! is one line in [`COMMANDS`] and a module.
//!
//! ```text
//! balance verdict [--seeds 1-32] [--styles ..] [--presets ..] [--nations ..]
//!                 [--threads 4] [--label NAME] [--out DIR] [--minutes M]
//! balance trace --seed S --nation N [--style ID] [--preset P] --from M --to M [--map-at M]
//! balance sweep | supply | mastery | economy ...      (the older modes, `legacy`)
//! ```
//!
//! `verdict` plays the panel `data/harness.toml`'s targets need, writes the run records to
//! `target/balance/LABEL.json`, prints the report to stdout (progress and timing go to stderr, so
//! two runs of a panel print the same bytes) and exits 1 if any target fails.

use std::path::PathBuf;
use std::process::ExitCode;
use std::time::Instant;

use fn_sim::{Defs, Nation};

use crate::config::{Config, parse_seeds};
use crate::legacy;
use crate::report::{self, Header};
use crate::runner::{self, Filter};
use crate::trace;
use crate::verdict;

type Handler = fn(&[String]) -> ExitCode;

/// Every subcommand: name, one-line summary, handler.
const COMMANDS: &[(&str, &str, Handler)] = &[
    ("verdict", "run the panel and judge it against data/harness.toml's targets", cmd_verdict),
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
    "usage: balance verdict [--seeds 1-32] [--styles ID,..] [--presets ID,..] [--nations ID,..]\n\
     \x20                      [--threads N] [--label NAME] [--out DIR] [--minutes M]\n\n\
     Runs the panel data/harness.toml's targets ask for, writes the run records to\n\
     DIR/LABEL.json (default target/balance/<git short hash>.json), prints the verdict and\n\
     exits 1 if a target fails.\n\n\
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
            "-h" | "--help" => out.help = true,
            other => return Err(format!("unknown argument {other:?}")),
        }
    }
    Ok(out)
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

    let seeds_text = a.seeds.clone().unwrap_or_else(|| config.panel.seeds.clone());
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

    let filter = Filter {
        styles: a.styles.clone(),
        presets: a.presets.clone(),
        nations: nations.clone(),
        minutes: a.minutes,
    };
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
    eprintln!("records: {}  ({:.1}s)", path.display(), started.elapsed().as_secs_f32());
    Ok(if verdict.failed() { ExitCode::from(1) } else { ExitCode::SUCCESS })
}

#[cfg(test)]
mod tests;
