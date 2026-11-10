//! `cargo xtask agent-run [options]`     agent plays headless; watch it here
//! `cargo xtask agent-run --interactive [options]`  chat with the agent here; watch elsewhere
//! `cargo xtask agent-run --chunk-turns N [options]`  play as successive short claude sessions
//! `cargo xtask agent-run --scripts [options]`  play by writing and running scripts (#484)
//! `cargo xtask agent-run --track ID --attempts N [options]`  hill-climb a track (#530)
//! `cargo xtask agent-run --resume CAMPAIGN_DIR [--attempts N] [options]`  continue a campaign
//! `cargo xtask agent-run watch [options] [RUN_DIR]`  watch (or replay) a run; default: a list
//!                                      of your runs to pick from (#556), or the latest with no
//!                                      terminal. `watch --help` lists its options and keys,
//!                                      `watch --list` the recent runs (#510, #553).
//!
//! Starts an agent run (docs/design/agent-play.md): a Claude Code session plays one game
//! through `fn_agent serve` while you watch it in the normal TUI with `fn_tui --watch`. This is
//! #507's rewrite of the old `scripts/agent-run.sh`, which stays on disk only as a thin wrapper
//! that execs this.
//!
//! `agent-run --help` prints this text and exits 0, as does `watch --help` for watch's own.
//!
//! Options:
//!   --seed N           map seed (default: random, 1..=99999)
//!   --nation X         earth, fire, water or air (default: random)
//!   --difficulty ID    a data/difficulty.toml preset (default normal)
//!   --scenario ID      a data/scenarios.toml preset (#481) instead of --seed/--nation/
//!                      --difficulty, run through its own prelude first -- for testing the
//!                      midgame or late game without paying an agent to play the opening first.
//!                      Can't be combined with --seed, --nation or --difficulty.
//!                      (A scenario fixes its own seed and nation, so nothing is random.)
//!   --turns N          how many turns the prompt asks for (default 40)
//!   --turn-secs S      game seconds per end_turn (default: fn_agent's own, 20)
//!   --pace N           watch pace to start at: 1, 2, 5, 10, 20, 40, 100, 200 or 500 (default 20)
//!   --model M          passed to `claude --model`
//!   --prompt TEXT      replaces the default prompt (every chunk, under --chunk-turns or
//!                      --scripts, verbatim)
//!   --chunk-turns N    plays --turns as successive `claude -p` sessions of N turns each, every
//!                      one a fresh process against the same log (`fn_agent serve --resume`,
//!                      #341): headless, and not combined with --interactive or --scripts.
//!   --scripts          plays by writing and running scripts (docs/design/agent-scripting.md)
//!                      instead of acting turn by turn: adds `fn_agent serve`'s own `--scripts`,
//!                      and a prompt that teaches the trampoline loop. Chunked by round trips
//!                      (--chunk-calls) rather than game turns. Headless, and not combined with
//!                      --interactive or --chunk-turns.
//!   --chunk-calls N    with --scripts, ends a session after about N tool calls (default 30) and
//!                      resumes with `--resume`.
//!   --carry DIR        with --scripts, saves the run's final script library, lab notes and
//!                      plan to DIR, and loads whatever's already there into the first
//!                      session's own prompt.
//!   --playtest         plays by scripts like --scripts, but to learn about the game rather than
//!                      to score (#576): the prompt asks the model to say a short remark
//!                      (at most 30 words) before significant actions and when something
//!                      surprises it, and to write `game problem:` lines for anything that seems
//!                      wrong with the game itself. The remarks are read from `claude`'s
//!                      `--output-format stream-json --verbose` as they arrive and appended to
//!                      run.jsonl as `say` lines, so `agent-run watch` shows them live in the
//!                      feed, labelled "says": they are what the model chose to write, not its
//!                      reasoning. The header and the run browser mark the run a playtest, and its
//!                      score is not shown beside the scored runs'. At the end (and after every
//!                      chunk) game-problems.md lists the distinct `game problem:` lines from the
//!                      notes and remarks with the turn each was first said. Not combined with
//!                      --track, --resume, --interactive or --chunk-turns; --chunk-calls and
//!                      --carry apply as for --scripts. Campaign and --scripts prompts are not
//!                      changed by it.
//!   --max-usd N        the spend cap, in dollars, and required for every run that calls the
//!                      model (#568): the default mode, --chunk-turns, --scripts and a --track or
//!                      --resume campaign. Not for --interactive, which is a chat you are in
//!                      and can end (and `claude --max-budget-usd` only works with `-p`), so
//!                      it is refused there. Enforced, not just watched (#485): every `claude`
//!                      call gets `--max-budget-usd` of what is left, and a chunked run also
//!                      stops between chunks once it is spent. `--chunk-calls` is only a request
//!                      in the prompt: it is this cap, `claude`'s own, that limits a chunk.
//!                      A chunk whose cost can't be read stops the run (it would not be $0).
//!   --track ID         a hill-climbing campaign (docs/design/agent-hillclimb.md, "The attempt
//!                      loop"; #530) on a `data/tracks.toml` track: --attempts N scripted runs
//!                      (--scripts is implied) on the track's play_seed to its horizon, each
//!                      scored, then cleanly evaluated (#571): the best governor version on the
//!                      screen seeds, then on the gate seeds against the champion, paired. It
//!                      becomes the champion when the gain is clear (agent_eval/paired.rs);
//!                      later attempts start with the champion's scripts, plan and notes
//!                      preloaded (`fn_agent serve --carry-in`). The campaign folder holds
//!                      attempt-K/ run folders, ledger.json and champion/handoff.json, and the
//!                      ledger (owner-only columns included, for you only) is printed at the end.
//!                      Clean runs go in parallel: $FN_EVAL_THREADS, else half the cores.
//!                      The track sets the start, so --seed, --nation, --difficulty, --scenario,
//!                      --turns, --turn-secs, --prompt, --carry and --interactive don't apply.
//!                      --model, --max-usd and --chunk-calls do, per attempt; --max-usd also caps
//!                      the whole invocation's spend, and the loop stops once it is spent.
//!                      The live game stops at the track's horizon (`fn_agent serve
//!                      --horizon`, #569); an attempt ends there, or when a chunk changes
//!                      nothing (a plain --scripts or --chunk-turns run ends after two such
//!                      chunks in a row, #568).
//!   --attempts N       how many attempts to play (required with --track; default 1 with --resume)
//!   --resume DIR       continues the campaign in DIR: the track, ledger.json and champion are
//!                      read from the folder, new attempts are numbered after the last one, and
//!                      --attempts means N more, --max-usd this invocation's cap. Refused for a
//!                      folder with no ledger, or whose track's horizon or seeds no longer match
//!                      the tracks file (those scores wouldn't be comparable).
//!   --control GOV[@PLAN]  with --track or --resume: also play this fixed governor (and plan,
//!                      the `agent-compare` syntax) with no model on the track's seeds, and show
//!                      it in the owner's table beside par and the do-nothing control. Repeatable.
//!   --no-par           leave par out of the owner's table (par is what a stock scripted opening
//!                      reaches on this start, reading the world directly; computed once per
//!                      build and seed and kept outside the run folders; owner only, never in a
//!                      prompt)
//!   --no-controls      leave the do-nothing control (`loop { wait(10); }`) out of that table
//!   --tracks FILE      a tracks file other than data/tracks.toml (for tests)
//!   --claude-bin PATH  the `claude` binary to run (default: $FN_AGENT_CLAUDE_BIN, else
//!                      `claude` resolved on PATH) -- tests point this at a stub; the real
//!                      `claude` CLI is never invoked by a test.
//!
//! Seed and nation are random when you don't give them, so casual runs cover more ground (#549):
//! the picks are printed at start-up, marked "(random)", and land in the run folder's name, the
//! log header (`fn_agent serve --seed/--nation`) and the OTel attributes. To replay or compare,
//! pin them: `--seed 7 --nation earth` (the old defaults). The difficulty is never random: it stays
//! `normal`, the level the balance targets are set on. `agent-run watch` and `--scenario` runs
//! are unaffected.
//!
//! Each run gets its own folder under $FN_AGENT_RUNS (default ~/fn-agent-runs), outside the
//! repo. The agent gets only fn_agent's tools: that's the "in-game knowledge only" rule (#313,
//! #476), enforced by how `claude` is started (sandbox.rs, #567), not by the game: from an empty
//! temp directory, with no built-in tools, user or project settings, skills or Claude Code
//! prompt, and only the run's own MCP config. That is a set of flags, not a container; see
//! docs/design/agent-play.md, "The player's sandbox". Each session's transcript is copied into
//! the run folder's `transcripts/`. In the default mode, quitting the watcher stops the agent
//! too. The agent's own output goes to agent.log in the run folder. A --chunk-turns or
//! --scripts run also gets cost.json (round trips and tokens summed from every chunk's own
//! `claude -p --output-format json`, with a USD total; #485) and stop.json (why the loop ended:
//! done, cap, no_progress, unknown_cost or gave_up; #568); a --scripts run also gets
//! sdk-gaps.md. A --playtest run also gets game-problems.md.
//!
//! Every `claude` call this makes is tagged with OTEL_RESOURCE_ATTRIBUTES
//! (four_nations.run/.seed/.nation/.difficulty/.mode, plus .chunk on chunked runs; #494), kept
//! alongside anything already in the caller's own OTEL_RESOURCE_ATTRIBUTES rather than
//! replacing it.

use std::collections::BTreeMap;
use std::fs::{self, File, OpenOptions};
use std::io::{IsTerminal as _, Write as _};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::{Duration, Instant, SystemTime};

use serde::{Deserialize, Serialize};

use crate::Result;

mod campaign;
mod listing;
mod playtest;
mod sandbox;
mod versions;

fn say(msg: &str) {
    eprintln!("agent-run: {msg}");
}

fn help() -> String {
    include_str!("agent_run.rs")
        .lines()
        .skip(1)
        .take_while(|l| l.starts_with("//!"))
        .map(|l| l.trim_start_matches("//!"))
        .collect::<Vec<_>>()
        .join("\n")
}

// -------------------------------------------------------------------------------------------
// Entry point
// -------------------------------------------------------------------------------------------

pub fn run(args: &[&str]) -> Result {
    let repo = crate::root();

    if args.first() == Some(&"watch") {
        return run_watch(&repo, &args[1..]);
    }

    let mut opts = parse_args(args)?;
    if opts.help {
        println!("{}", help());
        return Ok(());
    }
    validate(&opts)?;
    // The player runs in another directory (#567), so nothing it is handed may be relative.
    opts.resume_dir = opts.resume_dir.take().map(|d| sandbox::absolute(&d));

    if !claude_available(&opts.claude_bin) {
        return Err(format!("{} (Claude Code) isn't on PATH", opts.claude_bin));
    }

    if opts.track.is_some() || opts.resume_dir.is_some() {
        return campaign::run(&repo, &opts);
    }

    if let Some(line) = start_line(&opts) {
        say(&line);
    }
    if opts.playtest {
        say("playtest: the model is asked to say what it thinks of the game, not to score; its \
             remarks go in the log and the watch feed, and the run isn't compared with scored runs");
    }

    cargo_build_release(&repo, &["fn_agent", "fn_tui"])?;

    let runs = runs_dir();
    let dir = make_run_dir(&runs, &opts)?;
    say(&format!("run folder: {}", dir.display()));

    let agent_bin = repo.join("target/release/fn_agent");
    let log = dir.join("run.jsonl");
    let mcp_config = dir.join("mcp.json");
    write_mcp_config(&mcp_config, &agent_bin, &serve_args(&opts, &log))?;

    let prompt = opts.prompt.clone().unwrap_or_else(|| default_prompt(opts.turns));
    let claude_args = base_claude_args(&mcp_config, opts.model.as_deref());

    if opts.scripts {
        run_scripts_chunked(&repo, &dir, &agent_bin, &opts, &claude_args)?;
        say(&format!(
            "scripted run finished; replay it with: cargo xtask agent-run watch {}",
            dir.display()
        ));
        say(&format!("cost: {0}/cost.json; SDK gaps: {0}/sdk-gaps.md", dir.display()));
        if opts.playtest {
            say(&format!("game problems the model flagged: {}/game-problems.md", dir.display()));
        }
        return Ok(());
    }
    if opts.chunk_turns != 0 {
        run_chunked(&repo, &dir, &agent_bin, &opts, &claude_args)?;
        say(&format!(
            "chunked run finished; replay it with: cargo xtask agent-run watch {}",
            dir.display()
        ));
        return Ok(());
    }
    if opts.interactive {
        say(&format!(
            "watch it from another terminal with: cargo xtask agent-run watch {}",
            dir.display()
        ));
        say(&format!("suggested first message: {prompt}"));
        let attrs = otel_attrs(&opts, &dir, "interactive", None);
        let session = sandbox::new_session_id();
        let _ = sandbox::claude_command(&opts.claude_bin, &dir, &session)?
            .args(&claude_args)
            .env("OTEL_RESOURCE_ATTRIBUTES", attrs)
            .status();
        sandbox::keep_transcript(&dir, &session, "interactive");
        finish_log(&dir, "agent session closed")?;
        return Ok(());
    }

    // One `claude -p` for the whole run, so claude's own cap is the whole cap (#568).
    let mut claude_args = claude_args;
    if let Some(cap) = opts.max_usd {
        claude_args.extend(budget_args(cap));
    }
    run_default(&repo, &dir, &opts, &prompt, &claude_args)
}

// -------------------------------------------------------------------------------------------
// Options
// -------------------------------------------------------------------------------------------

#[derive(Debug, Clone)]
struct Opts {
    seed: u64,
    nation: String,
    difficulty: String,
    scenario: Option<String>,
    turns: u32,
    turn_secs: Option<u32>,
    pace: u32,
    model: Option<String>,
    prompt: Option<String>,
    interactive: bool,
    chunk_turns: u32,
    scripts: bool,
    chunk_calls: u32,
    carry_dir: Option<PathBuf>,
    max_usd: Option<f64>,
    seed_given: bool,
    nation_given: bool,
    difficulty_given: bool,
    claude_bin: String,
    /// `-h`/`--help` was given: print the usage on stdout and exit 0 (#553).
    help: bool,
    /// `--track ID` (#530): a hill-climbing campaign on this `data/tracks.toml` track.
    track: Option<String>,
    /// `--attempts N` (#530): attempts to play, for `--track` and `--resume`.
    attempts: Option<u32>,
    /// `--resume DIR` (#530): continue the campaign in this folder.
    resume_dir: Option<PathBuf>,
    /// `--tracks FILE` (#530): a tracks file other than `data/tracks.toml`.
    tracks_file: Option<PathBuf>,
    /// `--control GOVERNOR.rhai[@PLAN.json]` (#572), repeatable: fixed strategies played with no
    /// model on the track's seeds and shown under the owner's table beside par.
    controls: Vec<String>,
    /// `--no-par` (#572): leave par out of the owner's table.
    no_par: bool,
    /// `--no-controls` (#572): leave the do-nothing control out of the owner's table.
    no_controls: bool,
    /// A campaign attempt's own run, which the track sets up rather than the flags above
    /// (#530): `fn_agent serve --map-size`, `--start`, `--prelude`, `--carry-in`, and the
    /// first chunk's prompt (`--prompt` replaces every chunk's).
    map_size: Option<String>,
    site: Option<String>,
    prelude: Option<String>,
    carry_in: Option<PathBuf>,
    first_prompt: Option<String>,
    /// The track's screen seeds and horizon (#561): `fn_agent serve --trial-seeds/--trial-turns`,
    /// which turns on the `test_governor` tool. Never a gate or owner-only seed.
    trial: Option<(Vec<u64>, u32)>,
    /// `--playtest` (#576): a scripted run played to learn about the game, not to score, whose
    /// model says what it thinks as it goes (`playtest.rs`). Implies `--scripts`.
    playtest: bool,
}

/// The nations `--nation` can name, and so the ones a random pick chooses between.
pub(crate) const NATIONS: [&str; 4] = ["earth", "fire", "water", "air"];

/// Random seeds are 1..=99999: short enough to type back in.
const MAX_RANDOM_SEED: u64 = 99_999;

/// A source of random numbers, injectable so tests stay deterministic.
type Entropy<'a> = &'a mut dyn FnMut() -> u64;

/// The real source: std only, no new crate. Hashes the wall clock, the pid and a per-process
/// counter (so two draws in the same nanosecond still differ). xtask isn't `fn_sim`, so its
/// determinism rules don't apply here.
fn system_entropy() -> u64 {
    use std::hash::{DefaultHasher, Hash, Hasher};
    use std::sync::atomic::AtomicU64;
    static DRAWS: AtomicU64 = AtomicU64::new(0);
    let nanos = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or_default();
    let mut h = DefaultHasher::new();
    nanos.hash(&mut h);
    std::process::id().hash(&mut h);
    DRAWS.fetch_add(1, Ordering::Relaxed).hash(&mut h);
    h.finish()
}

fn random_seed(entropy: Entropy) -> u64 {
    1 + entropy() % MAX_RANDOM_SEED
}

fn random_nation(entropy: Entropy) -> String {
    NATIONS[(entropy() % NATIONS.len() as u64) as usize].to_string()
}

/// The start-up line naming the game's seed and nation, marking each that was picked at random;
/// `None` for a scenario, which fixes its own.
fn start_line(o: &Opts) -> Option<String> {
    if o.scenario.is_some() {
        return None;
    }
    let mark = |given: bool| if given { "" } else { " (random)" };
    let mut line = format!(
        "seed {}{}, nation {}{}",
        o.seed,
        mark(o.seed_given),
        o.nation,
        mark(o.nation_given)
    );
    if !(o.seed_given && o.nation_given) {
        line.push_str(&format!("; pin it with --seed {} --nation {}", o.seed, o.nation));
    }
    Some(line)
}

fn take_value<'a>(args: &[&'a str], i: &mut usize, flag: &str) -> Result<&'a str> {
    *i += 1;
    args.get(*i).copied().ok_or_else(|| format!("{flag} needs a value"))
}

fn parse_args(args: &[&str]) -> Result<Opts> {
    parse_args_with(args, &mut system_entropy)
}

/// `parse_args` with the random source for an unset `--seed`/`--nation` passed in.
fn parse_args_with(args: &[&str], entropy: Entropy) -> Result<Opts> {
    let mut o = Opts {
        seed: 0,
        nation: String::new(),
        difficulty: "normal".to_string(),
        scenario: None,
        turns: 40,
        turn_secs: None,
        pace: 20,
        model: None,
        prompt: None,
        interactive: false,
        chunk_turns: 0,
        scripts: false,
        chunk_calls: 30,
        carry_dir: None,
        max_usd: None,
        seed_given: false,
        nation_given: false,
        difficulty_given: false,
        claude_bin: std::env::var("FN_AGENT_CLAUDE_BIN").unwrap_or_else(|_| "claude".to_string()),
        help: false,
        track: None,
        attempts: None,
        resume_dir: None,
        tracks_file: None,
        controls: Vec::new(),
        no_par: false,
        no_controls: false,
        map_size: None,
        site: None,
        prelude: None,
        carry_in: None,
        first_prompt: None,
        trial: None,
        playtest: false,
    };

    let mut i = 0;
    while i < args.len() {
        match args[i] {
            "--seed" => {
                let v = take_value(args, &mut i, "--seed")?;
                o.seed = v.parse().map_err(|_| format!("--seed wants a number, got {v:?}"))?;
                o.seed_given = true;
            }
            "--nation" => {
                o.nation = take_value(args, &mut i, "--nation")?.to_string();
                o.nation_given = true;
            }
            "--difficulty" => {
                o.difficulty = take_value(args, &mut i, "--difficulty")?.to_string();
                o.difficulty_given = true;
            }
            "--scenario" => o.scenario = Some(take_value(args, &mut i, "--scenario")?.to_string()),
            "--turns" => {
                let v = take_value(args, &mut i, "--turns")?;
                o.turns = v.parse().map_err(|_| format!("--turns wants a number, got {v:?}"))?;
            }
            "--turn-secs" => {
                let v = take_value(args, &mut i, "--turn-secs")?;
                o.turn_secs =
                    Some(v.parse().map_err(|_| format!("--turn-secs wants a number, got {v:?}"))?);
            }
            "--pace" => {
                let v = take_value(args, &mut i, "--pace")?;
                o.pace = v.parse().map_err(|_| format!("--pace wants a number, got {v:?}"))?;
            }
            "--model" => o.model = Some(take_value(args, &mut i, "--model")?.to_string()),
            "--prompt" => o.prompt = Some(take_value(args, &mut i, "--prompt")?.to_string()),
            "--chunk-turns" => {
                let v = take_value(args, &mut i, "--chunk-turns")?;
                o.chunk_turns =
                    v.parse().map_err(|_| format!("--chunk-turns wants a number, got {v:?}"))?;
            }
            "--interactive" => o.interactive = true,
            "--scripts" => o.scripts = true,
            "--playtest" => {
                o.playtest = true;
                o.scripts = true;
            }
            "--chunk-calls" => {
                let v = take_value(args, &mut i, "--chunk-calls")?;
                o.chunk_calls =
                    v.parse().map_err(|_| format!("--chunk-calls wants a number, got {v:?}"))?;
            }
            "--carry" => o.carry_dir = Some(PathBuf::from(take_value(args, &mut i, "--carry")?)),
            "--max-usd" => {
                let v = take_value(args, &mut i, "--max-usd")?;
                if !is_plain_number(v) {
                    return Err(format!("--max-usd wants a plain number, got '{v}'"));
                }
                o.max_usd = Some(v.parse().expect("validated by is_plain_number"));
            }
            "--track" => o.track = Some(take_value(args, &mut i, "--track")?.to_string()),
            "--attempts" => {
                let v = take_value(args, &mut i, "--attempts")?;
                let n: u32 =
                    v.parse().map_err(|_| format!("--attempts wants a whole number, got {v:?}"))?;
                if n == 0 {
                    return Err("--attempts wants at least 1".into());
                }
                o.attempts = Some(n);
            }
            "--resume" => {
                o.resume_dir = Some(PathBuf::from(take_value(args, &mut i, "--resume")?));
            }
            "--tracks" => {
                o.tracks_file = Some(PathBuf::from(take_value(args, &mut i, "--tracks")?));
            }
            "--control" => o.controls.push(take_value(args, &mut i, "--control")?.to_string()),
            "--no-par" => o.no_par = true,
            "--no-controls" => o.no_controls = true,
            "--claude-bin" => o.claude_bin = take_value(args, &mut i, "--claude-bin")?.to_string(),
            "-h" | "--help" => {
                o.help = true;
                return Ok(o);
            }
            other => return Err(format!("unknown option {other} (see --help)")),
        }
        i += 1;
    }
    // A scenario fixes its own seed and nation, so there's nothing to pick (and the
    // placeholders are never read).
    if o.scenario.is_none() && o.track.is_none() && o.resume_dir.is_none() {
        if !o.seed_given {
            o.seed = random_seed(entropy);
        }
        if !o.nation_given {
            o.nation = random_nation(entropy);
        }
    }
    Ok(o)
}

fn is_plain_number(s: &str) -> bool {
    let mut parts = s.splitn(2, '.');
    let int_part = parts.next().unwrap_or("");
    if int_part.is_empty() || !int_part.bytes().all(|b| b.is_ascii_digit()) {
        return false;
    }
    match parts.next() {
        Some(frac) => !frac.is_empty() && frac.bytes().all(|b| b.is_ascii_digit()),
        None => true,
    }
}

fn validate(o: &Opts) -> Result {
    if o.playtest {
        validate_playtest(o)?;
    }
    if o.track.is_some() || o.resume_dir.is_some() {
        return validate_campaign(o);
    }
    if o.attempts.is_some() || o.tracks_file.is_some() {
        return Err("--attempts and --tracks only apply with --track or --resume".into());
    }
    if !o.controls.is_empty() || o.no_par || o.no_controls {
        return Err("--control, --no-par and --no-controls only apply with --track or --resume \
                     (they shape the owner's table of a campaign)"
            .into());
    }
    if o.scenario.is_some() && (o.seed_given || o.nation_given || o.difficulty_given) {
        return Err("--scenario can't be combined with --seed, --nation or --difficulty".into());
    }
    if o.chunk_turns != 0 && o.interactive {
        return Err("--interactive and --chunk-turns can't be combined".into());
    }
    if o.scripts && o.interactive {
        return Err("--interactive and --scripts can't be combined".into());
    }
    if o.scripts && o.chunk_turns != 0 {
        return Err("--chunk-turns and --scripts can't be combined; --scripts chunks by round \
                     trips (--chunk-calls)"
            .into());
    }
    if o.carry_dir.is_some() && !o.scripts {
        return Err("--carry only applies with --scripts".into());
    }
    check_cap(o)
}

/// What `--playtest` (#576) can't be combined with. A campaign plays for score with its scored
/// prompts, and a playtest is for learning about the game: running both in one invocation would
/// mix two aims, and a campaign's attempts are compared with each other. The model's remarks are
/// read from a stream that `--interactive` (a chat the owner is in) doesn't have, and the run is
/// always `--scripts`, chunked by `--chunk-calls`.
fn validate_playtest(o: &Opts) -> Result {
    if o.track.is_some() || o.resume_dir.is_some() {
        let mode = if o.track.is_some() { "--track" } else { "--resume" };
        return Err(format!(
            "--playtest can't be combined with {mode}: a campaign plays for score with scored \
             prompts and its attempts are compared with each other, while a playtest plays to \
             learn about the game and isn't compared with anything. Run them separately."
        ));
    }
    if o.interactive {
        return Err("--playtest and --interactive can't be combined: a playtest streams the \
                     model's remarks into the log, and --interactive is a chat you are in"
            .into());
    }
    if o.chunk_turns != 0 {
        return Err("--playtest and --chunk-turns can't be combined: a playtest plays by \
                     scripts, chunked by round trips (--chunk-calls)"
            .into());
    }
    Ok(())
}

/// The spend cap (#568): every run that calls the model has one. `--interactive` is the
/// exception: it is a chat the owner is sitting in and can end, and `claude --max-budget-usd`
/// only works with `-p`, so a cap there could not be kept; it is refused rather than ignored.
fn check_cap(o: &Opts) -> Result {
    if o.interactive {
        if o.max_usd.is_some() {
            return Err(
                "--max-usd doesn't apply to --interactive: `claude --max-budget-usd` only \
                        works with `claude -p`, and you are in the session and can end it"
                    .into(),
            );
        }
        return Ok(());
    }
    match o.max_usd {
        None => {
            Err("this run calls the model, so it needs a spend cap: add --max-usd N (dollars), \
                     e.g. --max-usd 5. A scripted game year costs about $0.5 on Sonnet and 3-4x \
                     that on Opus (docs/design/agent-scripting.md). --interactive needs none."
                .into())
        }
        Some(cap) if cap <= 0.0 => Err("--max-usd must be more than 0".into()),
        Some(_) => Ok(()),
    }
}

/// What a campaign (`--track`, `--resume`) can't be combined with: the track fixes the start
/// and the run is always `--scripts`, chunked by `--chunk-calls`.
fn validate_campaign(o: &Opts) -> Result {
    let mode = if o.track.is_some() { "--track" } else { "--resume" };
    if o.track.is_some() && o.resume_dir.is_some() {
        return Err("--track starts a campaign and --resume continues one: pick one".into());
    }
    if o.track.is_some() && o.attempts.is_none() {
        return Err("--track needs --attempts N (how many attempts to play)".into());
    }
    let clashes = [
        ("--seed", o.seed_given),
        ("--nation", o.nation_given),
        ("--difficulty", o.difficulty_given),
        ("--scenario", o.scenario.is_some()),
        ("--turn-secs", o.turn_secs.is_some()),
        ("--prompt", o.prompt.is_some()),
        ("--carry", o.carry_dir.is_some()),
        ("--interactive", o.interactive),
        ("--chunk-turns", o.chunk_turns != 0),
    ];
    for (flag, given) in clashes {
        if given {
            return Err(format!(
                "{flag} can't be combined with {mode}: the track sets the start and every \
                 attempt is a --scripts run"
            ));
        }
    }
    check_cap(o)
}

/// Whether `bin` (a bare name, resolved on `$PATH`, or a path containing a separator) exists --
/// the `command -v claude` check the old script made. Tests point `--claude-bin`/
/// `$FN_AGENT_CLAUDE_BIN` at a stub so this (and everything downstream) never touches the real
/// `claude` CLI.
fn claude_available(bin: &str) -> bool {
    if bin.contains('/') {
        return Path::new(bin).is_file();
    }
    let Some(path) = std::env::var_os("PATH") else { return false };
    std::env::split_paths(&path).any(|dir| dir.join(bin).is_file())
}

// -------------------------------------------------------------------------------------------
// Run folders
// -------------------------------------------------------------------------------------------

fn runs_dir() -> PathBuf {
    // Absolute, because the player runs in a different directory (#567).
    if let Some(p) = std::env::var_os("FN_AGENT_RUNS") {
        return sandbox::absolute(Path::new(&p));
    }
    let home = std::env::var_os("HOME").map(PathBuf::from).unwrap_or_else(|| PathBuf::from("."));
    sandbox::absolute(&home.join("fn-agent-runs"))
}

fn timestamp() -> String {
    let out = Command::new("date").arg("+%Y%m%d-%H%M%S").output().expect("date is on PATH");
    String::from_utf8_lossy(&out.stdout).trim().to_string()
}

fn make_run_dir(runs: &Path, o: &Opts) -> Result<PathBuf> {
    let name = match &o.scenario {
        Some(s) => format!("{}-scenario-{s}", timestamp()),
        None => format!("{}-{}-{}-{}", timestamp(), o.nation, o.seed, o.difficulty),
    };
    let dir = runs.join(name);
    fs::create_dir_all(&dir).map_err(|e| format!("creating {}: {e}", dir.display()))?;
    Ok(dir)
}

/// `YYYYMMDD-HHMMSS` at the front of a run folder's name (`timestamp()` writes it).
fn folder_stamp(name: &str) -> Option<&str> {
    let b = name.as_bytes();
    let ok = b.len() >= 15
        && b[..8].iter().all(u8::is_ascii_digit)
        && b[8] == b'-'
        && b[9..15].iter().all(u8::is_ascii_digit)
        && (b.len() == 15 || b[15] == b'-');
    ok.then(|| &name[..15])
}

/// Every run folder under `runs`, newest first by the date in its name (#585). Never by the
/// folder's modification time: that moves whenever anything is written into the folder, such as
/// the run browser's `score-cache.json`, and put old runs ahead of today's. A folder with no date
/// in its name goes after the dated ones, newest log first, and the name settles every tie.
fn run_dirs_newest(runs: &Path) -> Result<Vec<PathBuf>> {
    let entries = fs::read_dir(runs).map_err(|e| format!("no runs in {}: {e}", runs.display()))?;
    let key = |dir: &Path| {
        let name = dir.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        let stamp = folder_stamp(&name).map(str::to_owned);
        let log_time = if stamp.is_none() {
            dir.join("run.jsonl").metadata().and_then(|m| m.modified()).ok()
        } else {
            None
        };
        (stamp, log_time, name)
    };
    let mut dirs: Vec<(_, PathBuf)> = entries
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.is_dir())
        .map(|p| (key(&p), p))
        .collect();
    dirs.sort_by(|a, b| b.0.cmp(&a.0));
    Ok(dirs.into_iter().map(|(_, p)| p).collect())
}

fn latest_run_dir(runs: &Path) -> Result<PathBuf> {
    run_dirs_newest(runs)?
        .into_iter()
        .next()
        .ok_or_else(|| format!("no runs in {}", runs.display()))
}

// -------------------------------------------------------------------------------------------
// cargo / claude processes
// -------------------------------------------------------------------------------------------

/// Builds `pkgs` at the release profile, via a plain `cargo` resolved on `$PATH` (not the
/// `env!("CARGO")` xtask was invoked with): a test's own stub `cargo` ahead of it on `$PATH`
/// turns this into a no-op, the same way `scripts/test-agent-run-otel.sh` stubbed it, so a test
/// never waits on a real release build.
///
/// Not `-q`: when another cargo holds the target directory's build lock (a `ci` run, say),
/// cargo prints "Blocking waiting for file lock", and silencing that made the command look hung
/// (#510). The "building" line says so up front too.
pub(crate) fn cargo_build_release(repo: &Path, pkgs: &[&str]) -> Result {
    say(&format!(
        "building {} (release); if another cargo holds the build lock this waits for it",
        pkgs.join(" and ")
    ));
    let mut cmd = Command::new("cargo");
    cmd.args(["build", "--release"]);
    for p in pkgs {
        cmd.args(["-p", p]);
    }
    cmd.arg("--manifest-path").arg(repo.join("Cargo.toml"));
    let status = cmd.status().map_err(|e| e.to_string())?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("cargo build --release -p {} failed", pkgs.join(" -p ")))
    }
}

/// Whether `target/release/fn_tui` has to be rebuilt: it's missing, or some input is newer.
/// `newest_input` is `None` when there are no inputs to compare against (a built binary stands).
fn build_is_stale(bin_mtime: Option<SystemTime>, newest_input: Option<SystemTime>) -> bool {
    match (bin_mtime, newest_input) {
        (None, _) => true,
        (Some(_), None) => false,
        (Some(bin), Some(input)) => input > bin,
    }
}

/// The newest modification time of any file under `dir` (recursing), or `None` if there are
/// none. Unreadable entries are skipped.
fn newest_mtime(dir: &Path) -> Option<SystemTime> {
    let mut newest: Option<SystemTime> = None;
    let mut stack = vec![dir.to_path_buf()];
    while let Some(d) = stack.pop() {
        let Ok(entries) = fs::read_dir(&d) else { continue };
        for e in entries.filter_map(|e| e.ok()) {
            let Ok(meta) = e.metadata() else { continue };
            if meta.is_dir() {
                stack.push(e.path());
            } else if let Ok(t) = meta.modified() {
                newest = Some(newest.map_or(t, |n| n.max(t)));
            }
        }
    }
    newest
}

/// Builds fn_tui unless the existing release binary is newer than every file under `crates/`
/// and `data/` (and `Cargo.lock`), so watching a live run never waits on an unrelated cargo
/// command that holds the build lock (#510).
fn build_fn_tui_if_stale(repo: &Path) -> Result {
    let mtime = |p: PathBuf| p.metadata().and_then(|m| m.modified()).ok();
    let bin_mtime = mtime(repo.join("target/release/fn_tui"));
    let newest_input = ["crates", "data"]
        .iter()
        .filter_map(|d| newest_mtime(&repo.join(d)))
        .chain(mtime(repo.join("Cargo.lock")))
        .max();
    if build_is_stale(bin_mtime, newest_input) {
        cargo_build_release(repo, &["fn_tui"])
    } else {
        say("target/release/fn_tui is newer than crates/ and data/; skipping the build");
        Ok(())
    }
}

// -------------------------------------------------------------------------------------------
// `agent-run watch` (#510, #553)
// -------------------------------------------------------------------------------------------

/// `cargo xtask agent-run watch --help`. The keys are the ones `fn_tui --watch` handles
/// (crates/tui/src/watch.rs: `run`'s key match, `Watcher::key`, `Studio::key`, and free look's
/// `spectate::key`); the pace ladder and the studio threshold are `watch::PACES` and `[studio]`
/// in data/tui.toml, which a test below checks this text against.
const WATCH_HELP: &str = "\
usage: cargo xtask agent-run watch [options] [RUN_DIR]
       cargo xtask agent-run watch --list

Watches a run in the terminal (`fn_tui --watch`): a live one while the agent plays, or a
finished one replayed from its log. Never starts an agent.

With no RUN_DIR, in a terminal, it opens the run browser (below) so you can pick the run from a
list. With a RUN_DIR it opens that run directly. With no terminal (output piped, or run from a
script) or with --record, it takes the newest run, as it always did.

RUN_DIR   a run folder (the one holding run.jsonl), a campaign folder, or a .jsonl log such as
          a campaign's clean run (attempt-1/eval/v5-6/seed-7.jsonl). A bare folder name with no
          path is looked for under the runs folder. Default: the run browser in a terminal, else
          the newest folder under $FN_AGENT_RUNS (else ~/fn-agent-runs), by the date in its
          name; the folder opened is printed. A folder with no log yet is a run just starting,
          so this waits (up to a minute) for the log; a folder that doesn't exist fails at once.
          A campaign folder opens the run browser on that campaign, its attempts showing; with
          no terminal (or --record) it watches the campaign's newest attempt that has a log.

options:
  --list           list the newest runs and campaigns, newest first by the date in the folder
                   name (folder, nation, seed or scenario, turns, score if the log has one), each
                   campaign followed by its attempts (clean mean, kept or not, live turns, cost,
                   where the clean runs are), and exit; builds and opens nothing
  --no-build       don't build fn_tui first. Without it, a release fn_tui is built unless
                   target/release/fn_tui is newer than everything under crates/, data/ and
                   Cargo.lock
  --pace N         the pace to start at: 1, 2, 5, 10, 20, 40, 100, 200 or 500, or xN (default
                   20; fn_tui's own default is 1)
  --record OUT.cast  don't open the TUI: render the whole replay into an asciicast v2 file (the
                   same log gives a byte-identical file; it runs faster than real time). Turn
                   it into a GIF with `agg OUT.cast OUT.gif`
  --size WxH       the recording's size in cells (default 160x45), with --record
  --fps N          the recording's frames per second, 1 to 60 (default 15), with --record
  --ascii          draw with the ASCII glyph set
  --color MODE     truecolor, 256, 16 or never (default: from NO_COLOR, COLORTERM, WT_SESSION
                   and TERM; truecolor with --record)
  -h, --help       this text

Other fn_tui --watch flags (--frame, --at, --keys, --ansi) print one frame instead of opening
the watcher, so they are not passed on: run fn_tui --watch LOG --frame directly for those. The
run browser has a frame of its own: fn_tui --runs DIR --frame [--keys SCRIPT].

the run browser (`watch` with no RUN_DIR, in a terminal):
  One row a run folder, newest first with the newest selected: its start date and time, nation,
  seed or scenario, difficulty, mode (scripts or by turn), model, length (turns and game time),
  score, signs met, deaths, cost and state (live, finished or stopped early). A value the run
  doesn't have shows `-`; a run with an unreadable or cut-off log still has a row, saying why
  (`no log`, `empty log`, `unreadable`, `cut off`; the line below the list has the whole reason,
  and Enter on one that can't be opened says it instead of opening it). A terminal narrower
  than the full row drops the columns from the right and never wraps or cuts the run's name.
  SCORE@540 is the score at 540 turns, computed by the same code as `fn_agent score LOG
  --horizon 540`: the log's own `score` line (from `fn_agent score --append`) when it has one
  (shown as 41.3@N when it was made at another horizon), else replayed in the background, so
  the list opens at once with `...` in the score, signs and deaths cells and fills them in.
  The result is cached in score-cache.json in the run folder (the log is never written to);
  it is replayed again when the log has grown, and ignored after a change to the game's data.
  A live run's turn count moves while the browser is open.
  A campaign (a hill-climb: `agent-run --track ID --attempts N`) is a row of its own, marked +
  (folded) or - (open): its track, attempts kept of attempts made, best clean mean (CLEAN), cost
  and state, which for a running one says what it is doing (`live: a3 evaluating`, or playing,
  or testing). Opened, it shows its attempts oldest first: the live game's turns, cost and the
  score it played to, the clean mean (CLEAN) and whether the attempt was kept (KEPT). Under an
  attempt, once opened, are its clean evaluation runs of the chosen governor, one per seed,
  with the seed's tier (screen, gate, owner, or train and held-out in older ledgers) and the
  score the ledger recorded, or replayed in the background if it has none. An attempt with no
  live turns says so and offers those instead. Both ledger layouts show; a missing file is `-`.
  up or k, down or j   move the selection; PageUp, PageDown, Home or g, End or G go further
  enter          open the selected run, attempt (if it has live turns) or clean run in the
                 watcher (its keys, below); q there returns here on the same row. On a campaign
                 it opens or folds the row
  right or l     open a campaign or attempt (when open already, go into it); left or h folds it,
                 or goes up to the row it belongs to; space does one or the other
  s              switch the order between date (newest first) and score (highest first)
  n              show one nation's runs: each press goes on to the next nation, then to all
  q              quit

keys inside the watcher:
  space          pause or resume the replay
  + or =         faster, one step up the pace ladder (1, 2, 5, 10, 20, 40, 100, 200, 500)
  - or _         slower, one step down it
  .              step: finish the act in progress, or start the next one, ignoring the pace
  ]  }           skip ahead to the end of the next turn, or ten turns
  [  {           go back to the end of the previous turn, or ten turns (not in the run
                 browser's --frame)
  >              skip to the agent's next move, past the turns where it does nothing (in a
                 script run, to where the script wakes the agent)
  v              switch between Follow (the agent's own view) and Free look
  1  2  3        zoom the code, game or feed pane; again to return to the split. Below the
                 studio size these switch the tab instead
  0              back to the split (below the studio size, only flashes a reminder)
  j  k           scroll the code pane a line down or up; J  K half a pane
  q              quit (a run started by `agent-run` stops its agent too; `watch` never does;
                 from the run browser, back to the list)

free look (any of these switch to it from Follow; read-only, the replay never changes):
  arrows, WASD   move the cursor (Shift: 10 tiles); Ctrl+arrows or PageUp/PageDown jump
  Tab, Shift+Tab cycle through units; F follows the selected one; Enter selects
  N  T  G  L  ?  open the Nation, Research, Endeavor, message log and key help screens
  U              browse the Build list (browse only)

The studio (the code pane beside the game and the activity feed) needs a terminal of at least
160x45; smaller terminals show the three panes as tabs, 1 2 3 picking the tab.";

/// The pace ladder `fn_tui --pace` accepts (`watch::PACES` in crates/tui/src/watch.rs; a test
/// reads that source to make sure this copy hasn't drifted).
const WATCH_PACES: [u32; 9] = [1, 2, 5, 10, 20, 40, 100, 200, 500];

/// `fn_tui --fps` accepts 1 to this (`record::MAX_FPS`).
const WATCH_MAX_FPS: u32 = 60;

/// The pace `agent-run watch` starts at when not told.
const DEFAULT_WATCH_PACE: u32 = 20;

/// How many runs `watch --list` shows.
const LIST_LIMIT: usize = 20;

/// How long a watch waits for a run's log to appear, in half seconds.
const LOG_WAIT_TRIES: u32 = 120;

/// What `agent-run watch` was asked to do.
#[derive(Debug, Clone, PartialEq)]
enum WatchCmd {
    Help,
    List,
    Watch(WatchOpts),
}

/// `agent-run watch`'s options, and (via [`WatchOpts::live`]) what a live run's own watcher uses.
#[derive(Debug, Clone, PartialEq)]
struct WatchOpts {
    no_build: bool,
    pace: u32,
    record: Option<PathBuf>,
    fps: Option<u32>,
    size: Option<String>,
    ascii: bool,
    color: Option<String>,
    dir: Option<String>,
}

impl WatchOpts {
    /// The watcher a live run opens: just its pace.
    fn live(pace: u32) -> Self {
        WatchOpts {
            no_build: false,
            pace,
            record: None,
            fps: None,
            size: None,
            ascii: false,
            color: None,
            dir: None,
        }
    }
}

fn watch_hint(msg: &str) -> String {
    format!("{msg} (see `cargo xtask agent-run watch --help`)")
}

/// Parses what follows `agent-run watch`. Pure: no filesystem, no processes. The values are
/// checked the way `fn_tui` checks them, so a bad one fails here at once instead of after the
/// build.
fn parse_watch(args: &[&str]) -> Result<WatchCmd> {
    if args.iter().any(|a| matches!(*a, "-h" | "--help")) {
        return Ok(WatchCmd::Help);
    }
    let mut o = WatchOpts::live(DEFAULT_WATCH_PACE);
    let mut list = false;
    let mut i = 0;
    while i < args.len() {
        // `--flag=value` and `--flag value` both work, as in fn_tui.
        let (flag, inline) = match args[i].split_once('=') {
            Some((f, v)) if f.starts_with("--") => (f, Some(v)),
            _ => (args[i], None),
        };
        let mut value = |name: &str| -> Result<String> {
            if let Some(v) = inline {
                return Ok(v.to_string());
            }
            i += 1;
            match args.get(i) {
                Some(v) if !v.starts_with("--") => Ok(v.to_string()),
                _ => Err(watch_hint(&format!("{name} needs a value"))),
            }
        };
        match flag {
            "--list" => list = true,
            "--no-build" => o.no_build = true,
            "--ascii" => o.ascii = true,
            "--pace" => {
                let v = value("--pace")?;
                o.pace = v
                    .trim_start_matches(['x', 'X'])
                    .parse()
                    .ok()
                    .filter(|n| WATCH_PACES.contains(n))
                    .ok_or_else(|| {
                        watch_hint(&format!("--pace wants one of {WATCH_PACES:?}, got {v:?}"))
                    })?;
            }
            "--record" => o.record = Some(PathBuf::from(value("--record")?)),
            "--fps" => {
                let v = value("--fps")?;
                o.fps =
                    Some(v.parse().ok().filter(|n| (1..=WATCH_MAX_FPS).contains(n)).ok_or_else(
                        || watch_hint(&format!("--fps wants 1 to {WATCH_MAX_FPS}, got {v:?}")),
                    )?);
            }
            "--size" => {
                let v = value("--size")?;
                let ok = v.split_once(['x', 'X']).is_some_and(|(w, h)| {
                    w.parse::<u16>().is_ok_and(|w| w > 0) && h.parse::<u16>().is_ok_and(|h| h > 0)
                });
                if !ok {
                    return Err(watch_hint(&format!(
                        "--size wants WxH (such as 160x45), got {v:?}"
                    )));
                }
                o.size = Some(v);
            }
            "--color" => {
                let v = value("--color")?;
                if !["truecolor", "256", "16", "never"].contains(&v.as_str()) {
                    return Err(watch_hint(&format!(
                        "--color wants truecolor, 256, 16 or never, got {v:?}"
                    )));
                }
                o.color = Some(v);
            }
            f if f.starts_with('-') && f.len() > 1 => {
                return Err(watch_hint(&format!("unknown option {f} for watch")));
            }
            dir => {
                if let Some(first) = &o.dir {
                    return Err(watch_hint(&format!(
                        "watch takes one RUN_DIR, got {first:?} and {dir:?}"
                    )));
                }
                o.dir = Some(dir.to_string());
            }
        }
        i += 1;
    }
    if list {
        if o != WatchOpts::live(DEFAULT_WATCH_PACE) {
            return Err(watch_hint("--list can't be combined with other options or a RUN_DIR"));
        }
        return Ok(WatchCmd::List);
    }
    if o.record.is_none() {
        if o.fps.is_some() {
            return Err(watch_hint("--fps only means something with --record"));
        }
        if o.size.is_some() {
            return Err(watch_hint("--size only means something with --record"));
        }
    }
    Ok(WatchCmd::Watch(o))
}

/// Which run folder `o` names: the RUN_DIR as a path, else (a bare name) a folder of that name
/// under `runs`, else the newest folder under `runs`. Checked now, before any build: a missing
/// path or a plain file is an error naming it. The `bool` is whether it is the "latest" default.
fn resolve_watch_dir(o: &WatchOpts, runs: &Path) -> Result<(PathBuf, bool)> {
    let Some(given) = &o.dir else { return Ok((latest_run_dir(runs)?, true)) };
    let mut path = PathBuf::from(given);
    if !path.exists() && path.components().count() == 1 && runs.join(given).is_dir() {
        path = runs.join(given);
    }
    if !path.exists() {
        return Err(format!(
            "no run folder at {} (`cargo xtask agent-run watch --list` shows the recent ones)",
            path.display()
        ));
    }
    // A log file opens as it is: `fn_tui --watch` reads any `.jsonl`, such as the clean
    // evaluation logs of a campaign (`attempt-1/eval/v5-6/seed-7.jsonl`).
    let is_log = path.is_file() && path.extension().is_some_and(|e| e == "jsonl");
    if !path.is_dir() && !is_log {
        return Err(format!(
            "{} isn't a directory; RUN_DIR is the folder holding run.jsonl (or a .jsonl log)",
            path.display()
        ));
    }
    Ok((path, false))
}

/// The `fn_tui` command line (after the program name) that opens `log` the way `o` asks.
fn fn_tui_args(log: &Path, o: &WatchOpts) -> Vec<String> {
    let mut args = vec![
        "--watch".to_string(),
        log.display().to_string(),
        "--pace".to_string(),
        o.pace.to_string(),
    ];
    if let Some(out) = &o.record {
        args.push("--record".to_string());
        args.push(out.display().to_string());
    }
    if let Some(fps) = o.fps {
        args.push("--fps".to_string());
        args.push(fps.to_string());
    }
    if let Some(size) = &o.size {
        args.push("--size".to_string());
        args.push(size.clone());
    }
    if o.ascii {
        args.push("--ascii".to_string());
    }
    if let Some(color) = &o.color {
        args.push("--color".to_string());
        args.push(color.clone());
    }
    args
}

/// Whether a `watch` opens the run browser: nothing was named, nothing is being recorded, and
/// there is a terminal to draw it in (#556). Otherwise it opens the newest run, as before.
fn wants_browser(o: &WatchOpts, terminal: bool) -> bool {
    terminal && o.dir.is_none() && o.record.is_none()
}

/// Whether `dir` is a campaign folder (#585): it has no `run.jsonl` of its own, and a
/// `ledger.json` or `attempt-N` folders.
fn is_campaign_dir(dir: &Path) -> bool {
    dir.is_dir()
        && !dir.join("run.jsonl").is_file()
        && (dir.join("ledger.json").is_file() || !attempt_dirs(dir).is_empty())
}

/// A campaign's `attempt-K` folders, by K.
fn attempt_dirs(campaign: &Path) -> Vec<(u32, PathBuf)> {
    let mut v: Vec<(u32, PathBuf)> = fs::read_dir(campaign)
        .into_iter()
        .flatten()
        .filter_map(|e| e.ok())
        .filter(|e| e.path().is_dir())
        .filter_map(|e| {
            let k = e.file_name().to_string_lossy().strip_prefix("attempt-")?.parse().ok()?;
            Some((k, e.path()))
        })
        .collect();
    v.sort();
    v
}

/// The `fn_tui` command line (after the program name) that opens the run browser on `runs`, on
/// `focus` (a campaign, say) if given.
fn fn_tui_runs_args(runs: &Path, o: &WatchOpts, focus: Option<&Path>) -> Vec<String> {
    let mut args = vec![
        "--runs".to_string(),
        runs.display().to_string(),
        "--pace".to_string(),
        o.pace.to_string(),
    ];
    if let Some(focus) = focus {
        args.push("--focus".to_string());
        args.push(focus.display().to_string());
    }
    if o.ascii {
        args.push("--ascii".to_string());
    }
    if let Some(color) = &o.color {
        args.push("--color".to_string());
        args.push(color.clone());
    }
    args
}

fn run_watch(repo: &Path, args: &[&str]) -> Result {
    let opts = match parse_watch(args)? {
        WatchCmd::Help => {
            println!("{WATCH_HELP}");
            return Ok(());
        }
        WatchCmd::List => {
            let runs = runs_dir();
            println!("{}", format_run_list(&runs, &list_runs(&runs, LIST_LIMIT)));
            return Ok(());
        }
        WatchCmd::Watch(o) => o,
    };
    let runs = runs_dir();
    let (dir, latest) = resolve_watch_dir(&opts, &runs)?;
    let terminal = std::io::stdin().is_terminal() && std::io::stdout().is_terminal();
    if wants_browser(&opts, terminal) {
        say(&format!("opening the run browser on {}", runs.display()));
        if !opts.no_build {
            build_fn_tui_if_stale(repo)?;
        }
        let bin = repo.join("target/release/fn_tui");
        let status = Command::new(bin)
            .args(fn_tui_runs_args(&runs, &opts, None))
            .status()
            .map_err(|e| e.to_string())?;
        return if status.success() { Ok(()) } else { Err(format!("fn_tui --runs: {status}")) };
    }
    // A campaign folder has no log of its own (#585): in a terminal the browser opens on it, with
    // its attempts showing; otherwise its newest attempt with a log is watched, as the newest run
    // would be.
    let mut dir = dir;
    if is_campaign_dir(&dir) && opts.record.is_none() {
        if terminal {
            let abs = fs::canonicalize(&dir).unwrap_or_else(|_| dir.clone());
            let parent = abs.parent().map_or_else(|| runs.clone(), Path::to_path_buf);
            say(&format!("{} is a campaign: opening the run browser on it", dir.display()));
            if !opts.no_build {
                build_fn_tui_if_stale(repo)?;
            }
            let bin = repo.join("target/release/fn_tui");
            let status = Command::new(bin)
                .args(fn_tui_runs_args(&parent, &opts, Some(&abs)))
                .status()
                .map_err(|e| e.to_string())?;
            return if status.success() { Ok(()) } else { Err(format!("fn_tui --runs: {status}")) };
        }
        let newest = attempt_dirs(&dir)
            .into_iter()
            .rev()
            .find(|(_, d)| d.join("run.jsonl").metadata().is_ok_and(|m| m.len() > 0));
        let Some((k, attempt)) = newest else {
            return Err(format!(
                "{} is a campaign folder with no attempt log yet (its attempts are attempt-K \
                 folders); run `watch` in a terminal to browse it",
                dir.display()
            ));
        };
        say(&format!(
            "{} is a campaign: watching its newest attempt with a log (attempt {k})",
            dir.display()
        ));
        dir = attempt;
    }
    say(&format!("watching {}{}", dir.display(), if latest { " (the latest run)" } else { "" }));
    if !opts.no_build {
        build_fn_tui_if_stale(repo)?;
    }
    watch_run(repo, &dir, &opts)
}

fn watch_run(repo: &Path, dir: &Path, o: &WatchOpts) -> Result {
    // A `.jsonl` named as the RUN_DIR is the log itself.
    let log = if dir.is_file() { dir.to_path_buf() } else { dir.join("run.jsonl") };
    let has_log = || log.is_file() && log.metadata().map(|m| m.len() > 0).unwrap_or(false);
    if !has_log() {
        say(&format!(
            "no log in {} yet: waiting up to {} seconds for the run to start writing it \
             (Ctrl-C to stop)",
            dir.display(),
            LOG_WAIT_TRIES / 2
        ));
    }
    let started = Instant::now();
    let mut tries = 0;
    while !has_log() {
        if tries >= LOG_WAIT_TRIES {
            return Err(format!(
                "no log at {} yet (did the agent start? see {}/agent.log)",
                log.display(),
                dir.display()
            ));
        }
        thread::sleep(Duration::from_millis(500));
        tries += 1;
        if tries % 20 == 0 && !has_log() {
            say(&format!("still waiting for {} ({}s)", log.display(), started.elapsed().as_secs()));
        }
    }
    let bin = repo.join("target/release/fn_tui");
    Command::new(bin).args(fn_tui_args(&log, o)).status().map_err(|e| e.to_string())?;
    Ok(())
}

// -------------------------------------------------------------------------------------------
// `agent-run watch --list` (#553)
// -------------------------------------------------------------------------------------------

/// One line of `watch --list`: what a run folder's log says about it.
#[derive(Debug, Clone, PartialEq)]
struct RunRow {
    name: String,
    nation: String,
    /// `seed 7633`, or `scenario earth-year-two`.
    start: String,
    turns: u32,
    score: Option<f64>,
    /// The header says it was a playtest (#576): its score isn't shown beside the scored runs'.
    playtest: bool,
    /// A campaign's row has no turns of its own (#585), and its attempts hang under it.
    campaign: bool,
    /// What a campaign or one of its attempts adds beyond the columns.
    detail: Option<String>,
}

/// Reads `dir`'s `run.jsonl` for its header, turn count and last score. Parses only the three
/// kinds of line it needs: a long run's log is mostly script code. A folder with no log (or an
/// unreadable one) still gets a row, with dashes.
fn run_row(dir: &Path) -> RunRow {
    let name = dir.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    let mut row = RunRow {
        name: name.clone(),
        nation: "-".to_string(),
        start: "-".to_string(),
        turns: 0,
        score: None,
        playtest: false,
        campaign: false,
        detail: None,
    };
    let Ok(text) = fs::read_to_string(dir.join("run.jsonl")) else { return row };
    for line in text.lines() {
        if line.starts_with(r#"{"type":"header""#) {
            if let Ok(LogLine::Header(h)) = serde_json::from_str::<LogLine>(line) {
                row.nation = h.nation;
                row.playtest = h.playtest;
                // The header only says a scenario's prelude ran; the folder name has its id.
                let scenario = name.split_once("-scenario-").map(|(_, id)| id);
                row.start = match (h.prelude, scenario) {
                    (Some(_), Some(id)) => format!("scenario {id}"),
                    _ => format!("seed {}", h.seed),
                };
            }
        } else if line.starts_with(r#"{"type":"turn""#) {
            if let Ok(LogLine::Turn { index, .. }) = serde_json::from_str::<LogLine>(line) {
                row.turns = row.turns.max(index);
            }
        } else if line.starts_with(r#"{"type":"score""#)
            && let Ok(LogLine::Score { score, .. }) = serde_json::from_str::<LogLine>(line)
        {
            row.score = Some(score);
        }
    }
    row
}

/// The newest `limit` runs and campaigns under `runs`, newest first, each campaign followed by
/// its attempts (#585). A missing or empty folder is no rows.
fn list_runs(runs: &Path, limit: usize) -> Vec<RunRow> {
    let now = SystemTime::now();
    let mut rows = Vec::new();
    for dir in run_dirs_newest(runs).unwrap_or_default().iter().take(limit) {
        if is_campaign_dir(dir) {
            rows.extend(listing::campaign_rows(dir, now));
        } else {
            rows.push(run_row(dir));
        }
    }
    rows
}

fn format_run_list(runs: &Path, rows: &[RunRow]) -> String {
    if rows.is_empty() {
        return format!("no runs in {}", runs.display());
    }
    let width = |f: fn(&RunRow) -> usize, head: &str| {
        rows.iter().map(f).chain([head.len()]).max().unwrap_or(0)
    };
    let name_w = width(|r| r.name.len(), "RUN");
    let nation_w = width(|r| r.nation.len(), "NATION");
    let start_w = width(|r| r.start.len(), "START");
    let details = rows.iter().any(|r| r.detail.is_some());
    let line = |run: &str, nation: &str, start: &str, turns: &str, score: &str, detail: &str| {
        let text = format!(
            "{run:name_w$}  {nation:nation_w$}  {start:start_w$}  {turns:>5}  {score:>5}  {detail}"
        );
        text.trim_end().to_string()
    };
    let head_detail = if details { "DETAIL" } else { "" };
    let mut out = vec![line("RUN", "NATION", "START", "TURNS", "SCORE", head_detail)];
    for r in rows {
        let score = if r.playtest {
            "playtest".to_string()
        } else {
            r.score.map_or("-".to_string(), |s| format!("{s:.1}"))
        };
        let turns = if r.campaign { "-".to_string() } else { r.turns.to_string() };
        out.push(line(
            &r.name,
            &r.nation,
            &r.start,
            &turns,
            &score,
            r.detail.as_deref().unwrap_or(""),
        ));
    }
    out.push(format!(
        "newest first by the date in the name, in {}; watch one with: cargo xtask agent-run \
         watch NAME",
        runs.display()
    ));
    if details {
        out.push(
            "a campaign opens in the run browser (its attempts and their clean runs); a clean \
             run opens with: cargo xtask agent-run watch PATH/seed-N.jsonl"
                .to_string(),
        );
    }
    out.join("\n")
}

fn base_claude_args(mcp_config: &Path, model: Option<&str>) -> Vec<String> {
    let mut args = vec![
        "--strict-mcp-config".to_string(),
        "--mcp-config".to_string(),
        sandbox::absolute(mcp_config).display().to_string(),
        "--allowedTools".to_string(),
        "mcp__four-nations__*".to_string(),
        "--disallowedTools".to_string(),
        "Bash,Read,Edit,Write,Glob,Grep,WebFetch,WebSearch,NotebookEdit,Agent".to_string(),
    ];
    // The clean sandbox (#567): no built-in tools, settings, skills or Claude Code prompt. The
    // cwd and session id are set where each call is made.
    args.extend(sandbox::flags());
    if let Some(m) = model {
        args.push("--model".to_string());
        args.push(m.to_string());
    }
    args
}

/// One `claude -p PROMPT ... --output-format json` call: stdout (the JSON result) goes to
/// `out_json`, stderr is appended to `dir/agent.log`. How it went is in the returned
/// [`ChunkRun`], which `settle_chunk` turns into the run's cost, its report, and whether the
/// loop can go on (#568).
fn run_claude_p(
    opts: &Opts,
    prompt: &str,
    call_args: &[String],
    mode: &str,
    chunk: u32,
    dir: &Path,
    out_json: &Path,
) -> Result<ChunkRun> {
    if opts.playtest {
        // The only run that reads `claude`'s output as it goes (#576), for the model's remarks.
        return playtest::run_claude_stream(opts, prompt, call_args, mode, chunk, dir, out_json);
    }
    let stdout_file =
        File::create(out_json).map_err(|e| format!("creating {}: {e}", out_json.display()))?;
    let stderr_file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(dir.join("agent.log"))
        .map_err(|e| e.to_string())?;
    let attrs = otel_attrs(opts, dir, mode, Some(chunk));
    let session = sandbox::new_session_id();
    let status = sandbox::claude_command(&opts.claude_bin, dir, &session)?
        .arg("-p")
        .arg(prompt)
        .args(call_args)
        .args(["--output-format", "json"])
        .env("OTEL_RESOURCE_ATTRIBUTES", attrs)
        .stdout(stdout_file)
        .stderr(stderr_file)
        .status();
    sandbox::keep_transcript(dir, &session, &format!("chunk-{chunk}"));
    let failure = match status {
        Ok(s) if s.success() => None,
        Ok(s) => Some(format!("claude ended with {s}")),
        Err(e) => Some(format!("claude could not be run: {e}")),
    };
    Ok(ChunkRun { failure, result: read_claude_result(out_json) })
}

fn append_log(path: &Path, text: &str) -> Result {
    let mut f =
        OpenOptions::new().create(true).append(true).open(path).map_err(|e| e.to_string())?;
    f.write_all(text.as_bytes()).map_err(|e| e.to_string())
}

/// The run's background headless agent (default mode): spawns `claude -p PROMPT` with its
/// output going to `dir/agent.log`, then runs the live watcher (`fn_tui --watch`) in the
/// foreground. Quitting the watcher kills the agent; the agent finishing on its own writes the
/// log's `end` line right away, live, whether or not anyone is still watching (mirrors the old
/// script's background subshell + `trap ... EXIT`).
fn run_default(
    repo: &Path,
    dir: &Path,
    opts: &Opts,
    prompt: &str,
    claude_args: &[String],
) -> Result {
    let agent_log = dir.join("agent.log");
    let stdout_file = File::create(&agent_log).map_err(|e| e.to_string())?;
    let stderr_file = stdout_file.try_clone().map_err(|e| e.to_string())?;
    let attrs = otel_attrs(opts, dir, "mcp", None);
    let session = sandbox::new_session_id();
    let mut child = sandbox::claude_command(&opts.claude_bin, dir, &session)?
        .arg("-p")
        .arg(prompt)
        .args(claude_args)
        .env("OTEL_RESOURCE_ATTRIBUTES", attrs)
        .stdout(stdout_file)
        .stderr(stderr_file)
        .spawn()
        .map_err(|e| e.to_string())?;
    let pid = child.id();

    let killing = Arc::new(AtomicBool::new(false));
    let alive = Arc::new(AtomicBool::new(true));
    let dir_owned = dir.to_path_buf();
    let session_for_thread = session.clone();
    let killing_for_thread = killing.clone();
    let alive_for_thread = alive.clone();
    let handle = thread::spawn(move || {
        let _ = child.wait();
        sandbox::keep_transcript(&dir_owned, &session_for_thread, "mcp");
        alive_for_thread.store(false, Ordering::SeqCst);
        let reason = if killing_for_thread.load(Ordering::SeqCst) {
            "stopped by watcher"
        } else {
            "agent finished"
        };
        let _ = finish_log(&dir_owned, reason);
    });

    say("agent running; quit the watcher to stop it");
    build_fn_tui_if_stale(repo)?;
    let watch_result = watch_run(repo, dir, &WatchOpts::live(opts.pace));

    if alive.load(Ordering::SeqCst) {
        say("stopping the agent");
        killing.store(true, Ordering::SeqCst);
        let _ = Command::new("pkill").args(["-P", &pid.to_string()]).status();
        let _ = Command::new("kill").arg(pid.to_string()).status();
    } else {
        say(&format!("agent finished; its summary is in {}/agent.log", dir.display()));
    }
    let _ = handle.join();
    watch_result
}

// -------------------------------------------------------------------------------------------
// fn_agent CLI arguments
// -------------------------------------------------------------------------------------------

fn serve_args(o: &Opts, log: &Path) -> Vec<String> {
    let mut args = vec!["serve".to_string()];
    if let Some(scenario) = &o.scenario {
        args.push("--scenario".to_string());
        args.push(scenario.clone());
    } else {
        args.push("--seed".to_string());
        args.push(o.seed.to_string());
        args.push("--nation".to_string());
        args.push(o.nation.clone());
        args.push("--difficulty".to_string());
        args.push(o.difficulty.clone());
    }
    args.push("--log".to_string());
    args.push(log.display().to_string());
    if let Some(t) = o.turn_secs {
        args.push("--turn".to_string());
        args.push(t.to_string());
    }
    if o.scripts {
        args.push("--scripts".to_string());
    }
    // The header carries it from here on, so a resumed chunk (`resume_args`) needn't repeat it.
    if o.playtest {
        args.push("--playtest".to_string());
    }
    for (flag, value) in [("--map-size", &o.map_size), ("--prelude", &o.prelude)] {
        if let Some(v) = value {
            args.push(flag.to_string());
            args.push(v.clone());
        }
    }
    if let Some(site) = &o.site {
        args.push("--start".to_string());
        args.push(site.clone());
    }
    if let Some(carry_in) = &o.carry_in {
        args.push("--carry-in".to_string());
        args.push(carry_in.display().to_string());
    }
    args.extend(trial_args(o));
    args
}

/// `--trial-seeds LIST --trial-turns N` for a run that offers `test_governor` (#561), else
/// nothing, and `--horizon N` with them: a campaign's live game stops at the track's horizon
/// (#569). A resumed chunk needs them again: `serve --resume` rebuilds the game from the log's
/// header but not the tool.
fn trial_args(o: &Opts) -> Vec<String> {
    match &o.trial {
        Some((seeds, turns)) if o.scripts => {
            let seeds: Vec<String> = seeds.iter().map(u64::to_string).collect();
            vec![
                "--trial-seeds".to_string(),
                seeds.join(","),
                "--trial-turns".to_string(),
                turns.to_string(),
                "--horizon".to_string(),
                turns.to_string(),
            ]
        }
        _ => Vec::new(),
    }
}

fn resume_args(log: &Path, with_scripts: bool) -> Vec<String> {
    let mut args = vec!["serve".to_string(), "--resume".to_string(), log.display().to_string()];
    if with_scripts {
        args.push("--scripts".to_string());
    }
    args
}

fn write_mcp_config(path: &Path, command: &Path, args: &[String]) -> Result {
    let value = serde_json::json!({
        "mcpServers": {
            "four-nations": {
                "command": sandbox::absolute(command).display().to_string(),
                "args": args,
            }
        }
    });
    let text = serde_json::to_string_pretty(&value).map_err(|e| e.to_string())? + "\n";
    fs::write(path, text).map_err(|e| format!("writing {}: {e}", path.display()))
}

// -------------------------------------------------------------------------------------------
// Prompts
// -------------------------------------------------------------------------------------------

fn default_prompt(n: u32) -> String {
    format!(
        "You're playing The Four Nations, a colony game, through the four-nations tools. Call \
         briefing first, then observe. Keep everyone alive and grow the settlement. Give orders \
         with act (add a short note saying why) and let time pass with end_turn. Play {n} turns, \
         then summarise how it went."
    )
}

/// A resumed chunk's own prompt (#313, #341's acceptance criteria: in-game terms only): the
/// previous chunk's own final report, handed back as a short reminder.
fn handoff_prompt(n: u32, done_turns: u32, report: &str) -> String {
    format!(
        "You're continuing the same settlement from where you left off (turn {done_turns} so \
         far), not starting a new one. Here is your own last report:\n\n{report}\n\nCall \
         observe to see where things stand now, then carry on: give orders with act (add a \
         short note saying why) and let time pass with end_turn. Play {n} more turns, then \
         summarise how it went."
    )
}

/// The layout rules every script-mode prompt carries (#557): the same file `fn_agent`'s
/// `save_script` tool description is built from (`crates/agent/src/script/style.rs`'s
/// `LAYOUT_RULES`), so the two can't drift apart. Any other script-mode prompt (an attempt
/// prompt, say) adds it by calling this.
fn script_style_passage() -> &'static str {
    include_str!("../../crates/agent/src/script/layout_rules.txt").trim()
}

/// `--scripts`' own first prompt (docs/design/agent-scripting.md, "The trampoline"; #531,
/// docs/design/agent-hillclimb.md, "Within a run: a governor and a plan").
fn default_script_prompt(total: u32, calls: u32) -> String {
    format!(
        "You're playing The Four Nations, a colony game, by writing and running scripts through \
         the four-nations tools, rather than acting turn by turn. Call briefing first (it says \
         how the run is scored: play to that), then observe, and read sdk_reference. Run the \
         whole settlement from one saved script named \"governor\": a single loop of duties \
         (water, food, beds, stockpile, research, what to build next) that reads what to aim \
         for from plan(). Keep your intent in the plan tool (goal, order, targets, build \
         queue) and change it there: when something needs doing, edit the plan and let the \
         governor do it. Revise the governor with save_script rather than replacing it or \
         starting another one, and only when it can't do something at all. Start it small, run \
         it with run_script for just 3-5 turns to see what it does, then for a season, and \
         once that goes well, for a year. Use inline run_script code only to look (it is \
         read-only: observe, look, cells and the like), never to act. {style} Keep everyone alive and \
         grow the settlement, toward about {total} turns in all. After every run_script call, \
         use notes to record what you learned so it survives into the next stretch, and a line \
         starting \"SDK gap:\" for anything you wished the tools could do. This session is one \
         chunk of that: after roughly {calls} tool calls, stop and summarise how it went so the \
         next session can carry on.",
        style = script_style_passage(),
    )
}

/// A resumed `--scripts` chunk's own prompt (#341's rule in script-mode terms, #484's acceptance
/// criteria: the library and notes rather than a prose report; #531 adds the plan and the
/// governor rules).
///
/// A campaign attempt (#569) passes `campaign`: `campaign::handoff_note`'s account of where the
/// attempt stands and what decides it. It takes the place of "turn N of about T so far", which
/// read as a fresh start when a chunk had spent itself editing and testing and N was 0.
fn scripts_handoff_prompt(
    total: u32,
    done_turns: u32,
    calls: u32,
    listing: &str,
    campaign: Option<&str>,
) -> String {
    let (place, note) = match campaign {
        Some(note) => (String::new(), format!("\n\n{note}\n\n")),
        None => (format!(" (turn {done_turns} of about {total} so far)"), " ".to_owned()),
    };
    format!(
        "You're continuing the same settlement from where you left off{place}, not starting a \
         new one.{note}Here is your own plan, script library \
         and lab notes:\n\n{listing}\n\nCarry on the same way. One saved script, \"governor\", \
         runs the whole settlement from plan(): change what you want with the plan tool, and \
         revise the governor with save_script (don't replace it or start another) only when it \
         can't do something at all. Use inline run_script code only to look; it is read-only. \
         {style} Record what you learn in notes (and any SDK gap), and let time pass with \
         run_script. After roughly {calls} more tool calls, stop and summarise how it went.",
        style = script_style_passage(),
    )
}

// -------------------------------------------------------------------------------------------
// OTel resource attributes (#494)
// -------------------------------------------------------------------------------------------

/// Percent-encodes one OTEL_RESOURCE_ATTRIBUTES value per the OTel spec (comma-separated
/// `key=value`): anything outside `[A-Za-z0-9._~-]` is escaped, matching Python's
/// `urllib.parse.quote(s, safe="")`.
fn otel_encode(s: &str) -> String {
    let mut out = String::new();
    for b in s.bytes() {
        let c = b as char;
        if c.is_ascii_alphanumeric() || matches!(c, '-' | '.' | '_' | '~') {
            out.push(c);
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}

/// The OTEL_RESOURCE_ATTRIBUTES value for one `claude` call (#494): `mode` is
/// interactive|mcp|mcp-chunked|scripts, `chunk` is this call's 1-based chunk number (omitted
/// outside chunked modes). Appends to whatever's already in the caller's own
/// OTEL_RESOURCE_ATTRIBUTES rather than replacing it.
fn otel_attrs(o: &Opts, dir: &Path, mode: &str, chunk: Option<u32>) -> String {
    let run_name = dir.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    let mut attrs = format!("four_nations.run={}", otel_encode(&run_name));
    if let Some(scenario) = &o.scenario {
        attrs.push_str(&format!(",four_nations.scenario={}", otel_encode(scenario)));
    } else {
        attrs.push_str(&format!(",four_nations.seed={}", otel_encode(&o.seed.to_string())));
        attrs.push_str(&format!(",four_nations.nation={}", otel_encode(&o.nation)));
        attrs.push_str(&format!(",four_nations.difficulty={}", otel_encode(&o.difficulty)));
    }
    attrs.push_str(&format!(",four_nations.mode={}", otel_encode(mode)));
    if let Some(c) = chunk {
        attrs.push_str(&format!(",four_nations.chunk={c}"));
    }
    match std::env::var("OTEL_RESOURCE_ATTRIBUTES") {
        Ok(existing) if !existing.is_empty() => format!("{existing},{attrs}"),
        _ => attrs,
    }
}

// -------------------------------------------------------------------------------------------
// The run log (run.jsonl): `fn_runfmt`'s lines, with the nation and wake kinds as plain strings
// (#606), so xtask reads exactly what `fn_agent` writes without `fn_sim` or `fn_tui`.
// -------------------------------------------------------------------------------------------

use fn_runfmt::LogLine;

/// What a session's work in `run.jsonl` adds up to, for telling a chunk that changed something
/// from one that didn't (#569): game turns, saved scripts, plan edits, notes and `test_governor`
/// calls that ran a test (not a repeat of an earlier one, #639). A chunk spent editing and testing adds to it without playing a turn.
fn work_marks(lines: &[LogLine]) -> usize {
    lines
        .iter()
        .filter(|l| match l {
            LogLine::Turn { .. }
            | LogLine::Script { .. }
            | LogLine::Plan { .. }
            | LogLine::Note { .. } => true,
            // A repeat of an earlier test is not work (#639): it changed nothing.
            LogLine::Call { .. } => l.is_fresh_test(),
            _ => false,
        })
        .count()
}

/// How many tests the log holds (the tool's own limit is per attempt); repeats don't count.
fn tests_used(lines: &[LogLine]) -> usize {
    lines.iter().filter(|l| l.is_fresh_test()).count()
}

fn read_log_lines(path: &Path) -> std::io::Result<Vec<LogLine>> {
    let text = fs::read_to_string(path)?;
    Ok(text
        .lines()
        .filter(|l| !l.trim().is_empty())
        .filter_map(|l| serde_json::from_str::<LogLine>(l).ok())
        .collect())
}

/// How many turns `run.jsonl` has completed so far (the highest `turn` line's own `index`, 0
/// with none yet or no log file).
fn turns_done(dir: &Path) -> Result<u32> {
    let log = dir.join("run.jsonl");
    if !log.is_file() {
        return Ok(0);
    }
    let lines = read_log_lines(&log).map_err(|e| e.to_string())?;
    Ok(lines
        .iter()
        .filter_map(|l| if let LogLine::Turn { index, .. } = l { Some(*index) } else { None })
        .max()
        .unwrap_or(0))
}

/// Claude Code stops its MCP servers with a signal when it exits, so `fn_agent` rarely gets to
/// write the log's `end` line itself. This writes it once `claude` has exited, so the watcher
/// shows "run finished" instead of waiting forever. A log that already ends with one, or
/// doesn't exist yet, is left alone.
///
/// It first waits [`finish_log_settle`] for the MCP server's last writes to land.
fn finish_log(dir: &Path, reason: &str) -> Result {
    thread::sleep(finish_log_settle());
    end_log(dir, reason)
}

/// How long [`finish_log`] lets a stopping `fn_agent` finish writing before it reads the log: a
/// second. `$FN_AGENT_FINISH_LOG_SETTLE_MS` overrides it; the integration tests' stub agents
/// write nothing after they exit, so they set 0 instead of waiting a second per chunk.
fn finish_log_settle() -> Duration {
    std::env::var("FN_AGENT_FINISH_LOG_SETTLE_MS")
        .ok()
        .and_then(|s| s.parse().ok())
        .map_or(Duration::from_secs(1), Duration::from_millis)
}

/// [`finish_log`] without the wait.
fn end_log(dir: &Path, reason: &str) -> Result {
    let log = dir.join("run.jsonl");
    let lines = match read_log_lines(&log) {
        Ok(l) => l,
        Err(_) => return Ok(()),
    };
    if lines.is_empty() || matches!(lines.last(), Some(LogLine::End { .. })) {
        return Ok(());
    }
    let tick = lines
        .iter()
        .rev()
        .find_map(|l| if let LogLine::Turn { tick, .. } = l { Some(*tick) } else { None })
        .unwrap_or(0);
    let mut f =
        OpenOptions::new().create(true).append(true).open(&log).map_err(|e| e.to_string())?;
    let line = serde_json::json!({"type": "end", "tick": tick, "reason": reason});
    writeln!(f, "{line}").map_err(|e| e.to_string())
}

// -------------------------------------------------------------------------------------------
// The script library and lab notes (docs/design/agent-scripting.md, "Memory across sessions")
// -------------------------------------------------------------------------------------------

pub(crate) use fn_runfmt::{Handoff, ScriptEntry};

/// Replays `lines`' own `script`/`turn`/`wake`/`note`/`plan` lines (the same ones
/// `fn_agent::tools::rebuild_session` reads on `--resume`) into a script library (by name), a
/// list of lab notes in log order, and the latest non-empty plan (#524).
fn parse_library(
    lines: &[LogLine],
) -> (BTreeMap<String, ScriptEntry>, Vec<String>, Option<serde_json::Value>) {
    let mut scripts: BTreeMap<String, ScriptEntry> = BTreeMap::new();
    let mut notes = Vec::new();
    let mut plan = None;
    let mut pending: Option<String> = None;
    let mut turns_run: u32 = 0;
    for line in lines {
        match line {
            LogLine::Script { name, version, code, turns, .. } => {
                if *version >= 1 {
                    let e = scripts.entry(name.clone()).or_default();
                    e.version = *version;
                    e.code = code.clone();
                }
                pending = if *version >= 1 && *turns >= 1 { Some(name.clone()) } else { None };
                turns_run = 0;
            }
            LogLine::Turn { .. } => turns_run += 1,
            LogLine::Wake { reason, .. } => {
                if let Some(name) = &pending
                    && let Some(e) = scripts.get_mut(name)
                {
                    e.last_outcome = Some(reason.clone());
                    e.last_turns_run = Some(turns_run);
                }
                pending = None;
                turns_run = 0;
            }
            LogLine::Note { text } => notes.push(text.clone()),
            LogLine::Plan { plan: p } => {
                plan = p.as_object().is_some_and(|m| !m.is_empty()).then(|| p.clone());
            }
            _ => {}
        }
    }
    (scripts, notes, plan)
}

/// A short, human-readable summary (names, versions, last outcome; no code) for a resumed
/// `--scripts` chunk's own prompt -- `--resume` already restores the real library and memory, so
/// this is only orientation text, not a restore payload.
fn library_notes_listing(log: &Path) -> Result<String> {
    let lines = read_log_lines(log).map_err(|e| e.to_string())?;
    let (scripts, notes, plan) = parse_library(&lines);
    let mut out = Vec::new();
    match &plan {
        Some(plan) => out.push(format!("Plan:\n{plan}")),
        None => out.push("Plan: none set yet.".to_string()),
    }
    out.push(String::new());
    if scripts.is_empty() {
        out.push("Scripts: none saved yet.".to_string());
    } else {
        out.push("Scripts:".to_string());
        for (name, s) in &scripts {
            match &s.last_outcome {
                Some(outcome) => out.push(format!(
                    "- {name} v{} (last: {outcome}, {} turns)",
                    s.version,
                    s.last_turns_run.unwrap_or(0)
                )),
                None => out.push(format!("- {name} v{} (not run yet)", s.version)),
            }
        }
    }
    out.push(String::new());
    if notes.is_empty() {
        out.push("Notes: none yet.".to_string());
    } else {
        out.push("Notes:".to_string());
        out.extend(notes.iter().map(|n| format!("- {n}")));
    }
    Ok(out.join("\n"))
}

/// `--carry DIR`'s own save (#484, #476 "option A"): writes this run's final script library and
/// lab notes to `DIR/handoff.json`, read back by `carry_block` on a later run.
fn carry_save(log: &Path, carry_dir: &Path) -> Result {
    fs::create_dir_all(carry_dir).map_err(|e| e.to_string())?;
    let lines = read_log_lines(log).unwrap_or_default();
    let (scripts, notes, plan) = parse_library(&lines);
    let handoff = Handoff { scripts, notes, plan };
    let text = serde_json::to_string(&handoff).map_err(|e| e.to_string())?;
    fs::write(carry_dir.join("handoff.json"), text).map_err(|e| e.to_string())?;
    say(&format!("carried scripts, notes and plan to {}/handoff.json", carry_dir.display()));
    Ok(())
}

/// `--carry DIR`'s own load: the text a first `--scripts` prompt is given ahead of
/// `default_script_prompt`, asking the model to restore `DIR/handoff.json`'s own scripts (each
/// with its exact name and code, via `save_script`) and notes before doing anything else. `None`
/// with no `--carry`, or with nothing saved there yet (a first run).
fn carry_block(carry_dir: &Path) -> Result<Option<String>> {
    let path = carry_dir.join("handoff.json");
    let Ok(text) = fs::read_to_string(&path) else { return Ok(None) };
    if text.trim().is_empty() {
        return Ok(None);
    }
    let data: Handoff = serde_json::from_str(&text).map_err(|e| e.to_string())?;
    if data.scripts.is_empty() && data.notes.is_empty() && data.plan.is_none() {
        return Ok(None);
    }
    let mut out = vec![
        "You have saved work from a previous settlement: your own script library and lab \
         notes, carried over. Restore them before anything else."
            .to_string(),
    ];
    for (name, s) in &data.scripts {
        out.push(format!(
            "\nScript `{name}` (version {}): call save_script with this exact name and code to \
             restore it.\n```\n{}\n```",
            s.version, s.code
        ));
    }
    if !data.notes.is_empty() {
        out.push(format!(
            "\nYour lab notes from before (call notes to add them back):\n{}",
            data.notes.iter().map(|n| format!("- {n}")).collect::<Vec<_>>().join("\n")
        ));
    }
    if let Some(plan) = &data.plan {
        out.push(format!(
            "\nYour plan from before: call plan with set = this exact JSON to restore it, then \
             revise it for this settlement.\n```\n{plan}\n```"
        ));
    }
    Ok(Some(out.join("\n")))
}

/// `dir/sdk-gaps.md` (#484): every lab note whose text mentions an "SDK gap" (the convention
/// `default_script_prompt` asks for), pulled from the finished run's own log, for the owner to
/// turn into issues.
fn write_sdk_gaps(dir: &Path) -> Result {
    let lines = read_log_lines(&dir.join("run.jsonl")).unwrap_or_default();
    let gaps: Vec<String> = lines
        .into_iter()
        .filter_map(|l| match l {
            LogLine::Note { text } if text.to_lowercase().contains("sdk gap") => Some(text),
            _ => None,
        })
        .collect();
    let mut out = String::from("# SDK gaps\n\n");
    if gaps.is_empty() {
        out.push_str("None noted this run.\n");
    } else {
        for g in gaps {
            out.push_str(&format!("- {g}\n"));
        }
    }
    fs::write(dir.join("sdk-gaps.md"), out).map_err(|e| e.to_string())
}

// -------------------------------------------------------------------------------------------
// Reading `claude`'s result, and cost tracking (#485, #568)
// -------------------------------------------------------------------------------------------
//
// Everything about what a `claude -p --output-format json` call left behind is here: parsing the
// result (`read_claude_result`), the cost in it (`ClaudeResult::cost`), what a call's end means
// (`ChunkRun`, `settle_chunk`) and the running totals (`cost.json`).

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
struct Usage {
    #[serde(default)]
    input_tokens: u64,
    #[serde(default)]
    cache_creation_input_tokens: u64,
    #[serde(default)]
    cache_read_input_tokens: u64,
    #[serde(default)]
    output_tokens: u64,
}

/// A `claude -p ... --output-format json` call's own result shape: just the fields this cares
/// about. A field that is missing reads as zero or empty, except the cost: a result with no
/// `total_cost_usd` has an unknown cost, not a free one (#568).
#[derive(Debug, Default, Clone, Deserialize)]
struct ClaudeResult {
    #[serde(default)]
    num_turns: u64,
    #[serde(default)]
    usage: Usage,
    #[serde(default)]
    total_cost_usd: Option<f64>,
    #[serde(default)]
    result: String,
}

impl ClaudeResult {
    /// The call's cost in dollars, or `None` when the result doesn't say (or says something that
    /// isn't an amount of money).
    fn cost(&self) -> Option<f64> {
        self.total_cost_usd.filter(|c| c.is_finite() && *c >= 0.0)
    }
}

/// Parses `path` as a `claude -p --output-format json` result: `None` if the file is missing or
/// isn't a JSON object of that shape (a crashed or killed `claude` leaves nothing, or half a
/// line).
fn read_claude_result(path: &Path) -> Option<ClaudeResult> {
    serde_json::from_str(&fs::read_to_string(path).ok()?).ok()
}

/// How one chunk's `claude` call went (#568).
#[derive(Debug)]
struct ChunkRun {
    /// Set when `claude` didn't exit 0 (killed, nonzero status, or never started): what happened.
    failure: Option<String>,
    /// What it printed, if that parsed.
    result: Option<ClaudeResult>,
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
struct CostTotals {
    #[serde(default)]
    round_trips: u64,
    #[serde(default)]
    input_tokens: u64,
    #[serde(default)]
    cache_creation_input_tokens: u64,
    #[serde(default)]
    cache_read_input_tokens: u64,
    #[serde(default)]
    output_tokens: u64,
    #[serde(default)]
    usd: f64,
    /// Chunks whose cost couldn't be read (#568). Their spend is not in `usd`, so a total with
    /// any of these is a floor, not the cost; the run stops at the first one.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    unknown_chunks: Vec<u32>,
}

/// Reads and parses `path` as JSON, or `T::default()` if it's missing or isn't valid JSON.
fn read_json_or_default<T: Default + for<'de> Deserialize<'de>>(path: &Path) -> T {
    fs::read_to_string(path).ok().and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_default()
}

/// Adds one chunk's result into `dir/cost.json`'s running total: round trips (`num_turns`),
/// input/cache/output tokens, and USD. A chunk with no result, or none with a readable cost, is
/// listed in `unknown_chunks` and adds nothing to `usd`. Updated after every chunk, not just at
/// the end, so an interrupted run still reports what it spent so far.
fn update_cost(dir: &Path, chunk: u32, result: Option<&ClaudeResult>) -> Result {
    let cost_path = dir.join("cost.json");
    let mut totals: CostTotals = read_json_or_default(&cost_path);
    if let Some(data) = result {
        totals.round_trips += data.num_turns;
        totals.input_tokens += data.usage.input_tokens;
        totals.cache_creation_input_tokens += data.usage.cache_creation_input_tokens;
        totals.cache_read_input_tokens += data.usage.cache_read_input_tokens;
        totals.output_tokens += data.usage.output_tokens;
    }
    match result.and_then(ClaudeResult::cost) {
        Some(usd) => totals.usd += usd,
        None => totals.unknown_chunks.push(chunk),
    }
    let text = serde_json::to_string_pretty(&totals).map_err(|e| e.to_string())? + "\n";
    fs::write(&cost_path, text).map_err(|e| format!("writing {}: {e}", cost_path.display()))
}

/// What a finished chunk comes to (#568).
struct Settled {
    /// The chunk's own report, or a line saying it left none, for the next chunk's handoff.
    report: String,
    /// Why the loop can't go on, when it can't: the chunk's cost is unknown, so the cap can no
    /// longer be vouched for.
    stop: Option<String>,
}

/// Books a finished chunk, in one place for every chunked loop: reports a `claude` that failed
/// (on stderr and in agent.log, never silently), adds the cost to `cost.json`, and says to stop
/// if the cost can't be read. A nonzero exit whose result still carries a cost is charged and
/// reported but doesn't stop the run by itself: that is also how `claude` may end a chunk that
/// hit `--max-budget-usd`, and the cap and no-progress rules bound what repeats.
fn settle_chunk(dir: &Path, chunk: u32, run: &ChunkRun) -> Result<Settled> {
    let agent_log = dir.join("agent.log");
    if let Some(why) = &run.failure {
        say(&format!("chunk {chunk}: {why}; see {}", agent_log.display()));
        append_log(&agent_log, &format!("\n--- chunk {chunk} failed: {why} ---\n"))?;
    }
    update_cost(dir, chunk, run.result.as_ref())?;
    let known = run.result.as_ref().and_then(ClaudeResult::cost).is_some();
    let stop = (!known).then(|| {
        let what = match &run.failure {
            Some(why) => format!("{why} and left no result with a cost"),
            None => "claude exited 0 but left no result with a total_cost_usd".to_string(),
        };
        format!(
            "chunk {chunk}: {what} (chunk-{chunk}.json). What it spent is unknown, not $0, so \
             the spend cap can't be checked: stopping. cost.json lists it under unknown_chunks."
        )
    });
    let said = run.result.as_ref().map(|r| r.result.as_str()).unwrap_or_default();
    let report = if !said.is_empty() {
        said.to_string()
    } else if let Some(why) = &run.failure {
        format!("(this chunk's session failed: {why}; see {}/agent.log)", dir.display())
    } else {
        format!("(this chunk's session ended without a report; see {}/agent.log)", dir.display())
    };
    append_log(&agent_log, &format!("\n--- chunk {chunk} report ---\n{report}\n"))?;
    Ok(Settled { report, stop })
}

/// The USD `dir/cost.json` has recorded so far (0 with no file yet).
fn cost_so_far(dir: &Path) -> f64 {
    read_json_or_default::<CostTotals>(&dir.join("cost.json")).usd
}

/// `--max-usd N`'s own remaining budget for the next `claude -p` call: N minus `cost_so_far`,
/// floored at 0.
fn budget_remaining(dir: &Path, max_usd: f64) -> f64 {
    (max_usd - cost_so_far(dir)).max(0.0)
}

/// `claude`'s own spend cap for one call. This, not `--chunk-calls` (a request in the prompt),
/// is what limits what a chunk can spend: `claude` stops once a response takes it past the
/// amount, so a runaway chunk overshoots by at most what was in flight (#568).
fn budget_args(usd: f64) -> [String; 2] {
    ["--max-budget-usd".to_string(), usd.to_string()]
}

/// A chunk's `claude` arguments: the run's, plus a cap of what's left of `--max-usd`.
fn chunk_call_args(o: &Opts, dir: &Path, claude_args: &[String]) -> Vec<String> {
    let mut args = claude_args.to_vec();
    if let Some(max_usd) = o.max_usd {
        args.extend(budget_args(budget_remaining(dir, max_usd)));
    }
    args
}

// -------------------------------------------------------------------------------------------
// Why a chunked loop ended (#568)
// -------------------------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum StopReason {
    /// The target turns were played (or a campaign attempt reached its horizon).
    Done,
    /// `--max-usd` is spent.
    Cap,
    /// Chunks in a row changed nothing.
    NoProgress,
    /// A chunk's cost couldn't be read; the run stops rather than assume $0. An error.
    UnknownCost,
    /// The backstop on chunk count.
    GaveUp,
}

impl StopReason {
    fn as_str(self) -> &'static str {
        match self {
            Self::Done => "done",
            Self::Cap => "cap",
            Self::NoProgress => "no_progress",
            Self::UnknownCost => "unknown_cost",
            Self::GaveUp => "gave_up",
        }
    }
}

/// Writes `dir/stop.json`: why the loop ended, so a run folder (or a campaign's attempt folder)
/// says it without the terminal output.
fn write_stop(dir: &Path, reason: StopReason, chunk: u32, turns: u32, detail: &str) -> Result {
    let text = serde_json::to_string_pretty(&serde_json::json!({
        "reason": reason.as_str(),
        "chunk": chunk,
        "turns": turns,
        "detail": detail,
    }))
    .map_err(|e| e.to_string())?
        + "\n";
    fs::write(dir.join("stop.json"), text).map_err(|e| format!("writing stop.json: {e}"))
}

/// A chunked run's no-progress rule (#568, building on #569's): a chunk that adds nothing to
/// `run.jsonl`'s `work_marks` (no turn, script save, plan edit, note or test) is idle, and
/// `limit` idle chunks in a row end the run. An editing-only chunk is work, so this does not
/// bound a model that edits forever: `--max-usd` does.
struct Idle {
    limit: u32,
    streak: u32,
}

impl Idle {
    /// A campaign attempt (`o.trial`) ends on the first idle chunk (#569: that is how an attempt
    /// that has done what it will do finishes). An ordinary run has no such natural end, so it
    /// gets one idle chunk of grace (a session that spent itself thinking, or hit a transient
    /// error), and stops on the second.
    fn for_run(o: &Opts) -> Self {
        Self { limit: if o.trial.is_some() { 1 } else { 2 }, streak: 0 }
    }

    /// Records a chunk that went from `before` to `after` work marks; true when the run should
    /// stop now.
    fn chunk_done(&mut self, before: usize, after: usize) -> bool {
        self.streak = if after > before { 0 } else { self.streak + 1 };
        self.streak >= self.limit
    }
}

/// How much work `log` holds (see [`work_marks`]), 0 with no log yet.
fn logged_work(log: &Path) -> usize {
    read_log_lines(log).map_or(0, |l| work_marks(&l))
}

// -------------------------------------------------------------------------------------------
// Chunked loops
// -------------------------------------------------------------------------------------------

/// `--chunk-turns N`: plays `opts.turns` as successive `claude -p` sessions of at most N turns
/// each, every one but the first resuming `run.jsonl` (`fn_agent serve --resume`, #341) instead
/// of starting a fresh game, so no single session's context grows past one chunk's worth.
///
/// Ends (and says why, in `stop.json`) when the turns are played, `--max-usd` is spent, two
/// chunks in a row change nothing, a chunk's cost can't be read (an error) or the chunk-count
/// backstop is hit.
fn run_chunked(
    repo: &Path,
    dir: &Path,
    agent_bin: &Path,
    o: &Opts,
    claude_args: &[String],
) -> Result {
    let target = o.turns;
    let size = o.chunk_turns;
    let mut n: u32 = 1;
    // Generous: the exact chunk count, plus headroom for a chunk that undershoots and needs
    // another attempt, so a stuck run still gives up instead of looping forever.
    let max_chunks = target.div_ceil(size) + 5;
    say(&format!(
        "chunked run: {target} turns total, {size} per chunk, log at {}/run.jsonl",
        dir.display()
    ));
    let mcp_config = dir.join("mcp.json");
    let log = dir.join("run.jsonl");
    let mut report = String::new();
    let mut idle = Idle::for_run(o);
    let mut last_chunk = 0;
    let (reason, detail) = loop {
        let done_turns = turns_done(dir)?;
        if done_turns >= target {
            let detail = format!("reached turn {done_turns} of {target} turns; done");
            say(&detail);
            break (StopReason::Done, detail);
        }
        if n > max_chunks {
            let detail = format!(
                "giving up after {max_chunks} chunks (only reached turn {done_turns} of \
                 {target}); see {}/agent.log",
                dir.display()
            );
            say(&detail);
            break (StopReason::GaveUp, detail);
        }
        if let Some(detail) = cap_reached(o, dir, done_turns, target) {
            say(&detail);
            break (StopReason::Cap, detail);
        }
        let turns_left = target - done_turns;
        let this_chunk = turns_left.min(size);

        if n == 1 {
            write_mcp_config(&mcp_config, agent_bin, &serve_args(o, &log))?;
        } else {
            write_mcp_config(&mcp_config, agent_bin, &resume_args(&log, false))?;
        }

        let this_prompt = match (&o.prompt, n) {
            (Some(p), _) => p.clone(),
            (None, 1) => default_prompt(this_chunk),
            (None, _) => handoff_prompt(this_chunk, done_turns, &report),
        };
        let marks_before = logged_work(&log);
        let call_args = chunk_call_args(o, dir, claude_args);

        say(&format!(
            "chunk {n}: turns {}-{} of {target}",
            done_turns + 1,
            done_turns + this_chunk
        ));
        let chunk_json = dir.join(format!("chunk-{n}.json"));
        let run = run_claude_p(o, &this_prompt, &call_args, "mcp-chunked", n, dir, &chunk_json)?;
        last_chunk = n;
        let settled = settle_chunk(dir, n, &run)?;
        report = settled.report;
        finish_log(dir, &format!("chunk {n} finished"))?;
        if let Some(detail) = settled.stop {
            say(&detail);
            break (StopReason::UnknownCost, detail);
        }
        if idle.chunk_done(marks_before, logged_work(&log)) {
            let detail = format!(
                "stopping: {} chunk(s) in a row changed nothing (no turn, script, plan, note or \
                 test); at turn {} of {target}. See {}/agent.log",
                idle.streak,
                turns_done(dir)?,
                dir.display()
            );
            say(&detail);
            break (StopReason::NoProgress, detail);
        }
        n += 1;
    };
    write_stop(dir, reason, last_chunk, turns_done(dir)?, &detail)?;
    let _ = repo;
    if reason == StopReason::UnknownCost { Err(detail) } else { Ok(()) }
}

/// The `--max-usd` check between chunks: a line saying so once the cap is spent.
fn cap_reached(o: &Opts, dir: &Path, done_turns: u32, target: u32) -> Option<String> {
    let max_usd = o.max_usd?;
    let spent = cost_so_far(dir);
    (spent >= max_usd).then(|| {
        format!(
            "stopping: hit the ${max_usd} cap (${spent} spent so far, turn {done_turns} of \
             {target})"
        )
    })
}

/// `--scripts`: plays `opts.turns` as successive `claude -p` sessions chunked by round trips
/// rather than game turns (`--chunk-calls`, #484, docs/design/agent-scripting.md). Every chunk
/// but the first resumes `run.jsonl` (`fn_agent serve --scripts --resume`), which restores the
/// real script library and memory on its own.
///
/// Ends like [`run_chunked`] does. A campaign attempt (`o.trial`) differs in two ways: it is
/// done at its horizon too, and one idle chunk ends it rather than two.
fn run_scripts_chunked(
    repo: &Path,
    dir: &Path,
    agent_bin: &Path,
    o: &Opts,
    claude_args: &[String],
) -> Result {
    let target = o.turns;
    let calls = o.chunk_calls;
    let mut n: u32 = 1;
    // Turns per chunk vary a lot with how mature the library is, so this backstop can't be
    // sized from `calls` the way `run_chunked`'s is from `size`: one extra chunk per turn, plus
    // headroom. The cap and the no-progress rule are what bound a run in practice.
    let max_chunks = target + 10;
    // A campaign attempt (`o.trial` is only ever set by one, #561): its live game stops at the
    // horizon (`--horizon`, #569), and what finishes it is not only the turn counter.
    let campaign = o.trial.is_some();
    say(&format!(
        "scripted run: {target} turns total, about {calls} tool calls per chunk, log at \
         {}/run.jsonl",
        dir.display()
    ));
    let mcp_config = dir.join("mcp.json");
    let log = dir.join("run.jsonl");
    let mut idle = Idle::for_run(o);
    let mut last_chunk = 0;
    let (reason, detail) = loop {
        let done_turns = turns_done(dir)?;
        if done_turns >= target {
            let detail = format!("reached turn {done_turns} of {target} turns; done");
            say(&detail);
            break (StopReason::Done, detail);
        }
        if n > max_chunks {
            let detail = format!(
                "giving up after {max_chunks} chunks (only reached turn {done_turns} of \
                 {target}); see {}/agent.log",
                dir.display()
            );
            say(&detail);
            break (StopReason::GaveUp, detail);
        }
        if let Some(detail) = cap_reached(o, dir, done_turns, target) {
            say(&detail);
            break (StopReason::Cap, detail);
        }

        if n == 1 {
            write_mcp_config(&mcp_config, agent_bin, &serve_args(o, &log))?;
        } else {
            let mut args = resume_args(&log, true);
            args.extend(trial_args(o));
            write_mcp_config(&mcp_config, agent_bin, &args)?;
        }

        let this_prompt = if let Some(p) = &o.prompt {
            p.clone()
        } else if n == 1 && o.first_prompt.is_some() {
            o.first_prompt.clone().unwrap_or_default()
        } else if n == 1 {
            let base = if o.playtest {
                playtest::first_prompt(target, calls)
            } else {
                default_script_prompt(target, calls)
            };
            match &o.carry_dir {
                Some(carry_dir) => match carry_block(carry_dir)? {
                    Some(block) => format!("{block}\n\n{base}"),
                    None => base,
                },
                None => base,
            }
        } else if o.playtest {
            let listing = library_notes_listing(&log)?;
            playtest::handoff_prompt(target, done_turns, calls, &listing)
        } else {
            let listing = library_notes_listing(&log)?;
            let note = campaign.then(|| {
                let tests = read_log_lines(&log).map_or(0, |l| tests_used(&l));
                campaign::handoff_note(target, done_turns, tests)
            });
            scripts_handoff_prompt(target, done_turns, calls, &listing, note.as_deref())
        };
        let marks_before = logged_work(&log);
        let call_args = chunk_call_args(o, dir, claude_args);

        say(&format!(
            "chunk {n}: about {calls} tool calls, past turn {done_turns} of {target} so far"
        ));
        let chunk_json = dir.join(format!("chunk-{n}.json"));
        let mode = if o.playtest { "playtest" } else { "scripts" };
        let run = run_claude_p(o, &this_prompt, &call_args, mode, n, dir, &chunk_json)?;
        last_chunk = n;
        let settled = settle_chunk(dir, n, &run)?;
        finish_log(dir, &format!("chunk {n} finished"))?;
        if o.playtest {
            // After every chunk, so a run that stops early has it too.
            playtest::write_game_problems(dir)?;
        }
        if let Some(detail) = settled.stop {
            say(&detail);
            break (StopReason::UnknownCost, detail);
        }
        // A campaign attempt is finished when the live game is at its horizon (the loop's own
        // check, once the chunk that got there has had its finishing calls) or when a chunk
        // changed nothing: no turn, no saved script, plan edit, note or test. A chunk that
        // played no turns but did something is not finished, so an attempt may spend whole
        // chunks editing and testing; the turn counter alone ended it (or restarted it) before.
        // An ordinary run stops after two such chunks in a row (#568).
        if idle.chunk_done(marks_before, logged_work(&log)) {
            let detail = if campaign {
                format!(
                    "chunk {n} changed nothing (no turn, script, plan, note or test); \
                     the attempt is finished"
                )
            } else {
                format!(
                    "stopping: {} chunk(s) in a row changed nothing (no turn, script, plan, \
                     note or test); at turn {} of {target}. See {}/agent.log",
                    idle.streak,
                    turns_done(dir)?,
                    dir.display()
                )
            };
            say(&detail);
            break (StopReason::NoProgress, detail);
        }
        n += 1;
    };
    write_stop(dir, reason, last_chunk, turns_done(dir)?, &detail)?;
    write_sdk_gaps(dir)?;
    if o.playtest {
        playtest::write_game_problems(dir)?;
    }
    if let Some(carry_dir) = &o.carry_dir {
        carry_save(&log, carry_dir)?;
    }
    let _ = repo;
    if reason == StopReason::UnknownCost { Err(detail) } else { Ok(()) }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ----- argument parsing and validation -----

    #[test]
    fn defaults_match_the_old_script() {
        // Everything but the seed and nation (#549: those are random now, see below).
        let o = parse_args(&[]).unwrap();
        assert_eq!(o.difficulty, "normal");
        assert_eq!(o.turns, 40);
        assert_eq!(o.pace, 20);
        assert_eq!(o.chunk_calls, 30);
        assert!(!o.interactive);
        assert!(!o.scripts);
        assert_eq!(o.chunk_turns, 0);
        assert!(o.max_usd.is_none());
    }

    /// A random source that yields `vals` in order, then zeros.
    fn fixed<const N: usize>(vals: [u64; N]) -> impl FnMut() -> u64 {
        let mut it = vals.into_iter();
        move || it.next().unwrap_or(0)
    }

    #[test]
    fn unset_seed_and_nation_are_random_and_marked() {
        let o = parse_args_with(&[], &mut fixed([48_212, 1])).unwrap();
        assert_eq!(o.seed, 48_213);
        assert_eq!(o.nation, "fire");
        assert!(!o.seed_given && !o.nation_given);
        assert_eq!(o.difficulty, "normal");
        let line = start_line(&o).unwrap();
        assert!(line.starts_with("seed 48213 (random), nation fire (random)"), "{line}");
        assert!(line.contains("--seed 48213 --nation fire"), "{line}");
    }

    #[test]
    fn random_picks_cover_the_range_and_all_four_nations() {
        // The ends of the seed range, and each nation index.
        assert_eq!(random_seed(&mut fixed([0])), 1);
        assert_eq!(random_seed(&mut fixed([MAX_RANDOM_SEED - 1])), MAX_RANDOM_SEED);
        assert_eq!(random_seed(&mut fixed([MAX_RANDOM_SEED])), 1);
        let nations: Vec<String> = (0..4).map(|i| random_nation(&mut fixed([i]))).collect();
        assert_eq!(nations, ["earth", "fire", "water", "air"]);
    }

    #[test]
    fn explicit_seed_and_nation_win_and_draw_nothing() {
        let mut draws = 0;
        let mut counting = || {
            draws += 1;
            5
        };
        let o = parse_args_with(&["--seed", "7", "--nation", "earth"], &mut counting).unwrap();
        assert_eq!((o.seed, o.nation.as_str()), (7, "earth"));
        assert_eq!(draws, 0);
        assert_eq!(start_line(&o).unwrap(), "seed 7, nation earth");
        // One given, one random.
        let o = parse_args_with(&["--nation", "air"], &mut fixed([9])).unwrap();
        assert_eq!((o.seed, o.nation.as_str()), (10, "air"));
        assert_eq!(
            start_line(&o).unwrap().split(';').next().unwrap(),
            "seed 10 (random), nation air"
        );
    }

    #[test]
    fn scenario_runs_pick_nothing_at_random() {
        let mut draws = 0;
        let mut counting = || {
            draws += 1;
            5
        };
        let o = parse_args_with(&["--scenario", "earth-year-two", "--max-usd", "1"], &mut counting)
            .unwrap();
        assert_eq!(draws, 0);
        assert!(validate(&o).is_ok());
        assert!(start_line(&o).is_none());
    }

    #[test]
    fn random_picks_reach_the_run_folder_serve_args_and_otel_attributes() {
        let o = parse_args_with(&["--difficulty", "hard"], &mut fixed([48_212, 1])).unwrap();
        let runs = std::env::temp_dir().join(format!("fn-agent-run-549-{}", std::process::id()));
        let dir = make_run_dir(&runs, &o).unwrap();
        let name = dir.file_name().unwrap().to_str().unwrap().to_string();
        assert!(name.ends_with("-fire-48213-hard"), "{name}");
        let _ = fs::remove_dir_all(&runs);

        let serve = serve_args(&o, Path::new("/x/run.jsonl"));
        let pair = |flag: &str| {
            let at = serve.iter().position(|a| a == flag).unwrap();
            serve[at + 1].clone()
        };
        assert_eq!(pair("--seed"), "48213");
        assert_eq!(pair("--nation"), "fire");

        let attrs = otel_attrs(&o, &dir, "interactive", None);
        assert!(attrs.contains("four_nations.seed=48213,four_nations.nation=fire"), "{attrs}");
    }

    #[test]
    fn system_entropy_varies_between_draws() {
        let draws: std::collections::HashSet<u64> = (0..16).map(|_| system_entropy()).collect();
        assert!(draws.len() > 1);
    }

    #[test]
    fn unknown_flag_is_rejected() {
        assert!(parse_args(&["--nope"]).is_err());
    }

    #[test]
    fn scenario_cannot_combine_with_seed_nation_or_difficulty() {
        let o = parse_args(&["--scenario", "earth-year-two", "--seed", "3"]).unwrap();
        assert!(validate(&o).is_err());
        let o = parse_args(&["--scenario", "earth-year-two", "--max-usd", "1"]).unwrap();
        assert!(validate(&o).is_ok());
    }

    #[test]
    fn interactive_chunk_turns_and_scripts_are_mutually_exclusive() {
        assert!(validate(&parse_args(&["--interactive", "--chunk-turns", "5"]).unwrap()).is_err());
        assert!(validate(&parse_args(&["--interactive", "--scripts"]).unwrap()).is_err());
        assert!(validate(&parse_args(&["--chunk-turns", "5", "--scripts"]).unwrap()).is_err());
    }

    #[test]
    fn carry_requires_scripts() {
        assert!(validate(&parse_args(&["--carry", "/tmp/x"]).unwrap()).is_err());
        assert!(
            validate(&parse_args(&["--carry", "/tmp/x", "--scripts", "--max-usd", "1"]).unwrap())
                .is_ok()
        );
    }

    #[test]
    fn max_usd_wants_a_plain_number_above_zero() {
        assert!(parse_args(&["--max-usd", "abc"]).is_err());
        assert!(parse_args(&["--max-usd", "-1"]).is_err());
        let zero = validate(&parse_args(&["--max-usd", "0", "--scripts"]).unwrap());
        assert!(zero.unwrap_err().contains("more than 0"));
    }

    /// #568: every run that calls the model needs a cap, and says how to give one.
    #[test]
    fn every_model_run_needs_a_spend_cap() {
        for args in [
            &[][..],
            &["--scripts"],
            &["--chunk-turns", "3"],
            &["--track", "t", "--attempts", "1"],
            &["--resume", "d"],
        ] {
            let err = validate(&parse_args(args).unwrap()).unwrap_err();
            assert!(err.contains("--max-usd N"), "{args:?}: {err}");
        }
        for args in [
            &["--max-usd", "5"][..],
            &["--max-usd", "5", "--scripts"],
            &["--max-usd", "5.50", "--chunk-turns", "3"],
            &["--max-usd", "5", "--track", "t", "--attempts", "1"],
            &["--max-usd", "5", "--resume", "d"],
        ] {
            assert!(validate(&parse_args(args).unwrap()).is_ok(), "{args:?}");
        }
    }

    /// #568: an interactive chat is the one exemption, and a cap there is refused, not ignored.
    #[test]
    fn interactive_needs_no_cap_and_takes_none() {
        assert!(validate(&parse_args(&["--interactive"]).unwrap()).is_ok());
        let err = validate(&parse_args(&["--interactive", "--max-usd", "5"]).unwrap());
        assert!(err.unwrap_err().contains("only works with `claude -p`"));
    }

    #[test]
    fn help_flag_prints_usage_text_and_succeeds() {
        // `--help` is a request, not an error (#553): stdout and exit 0, wherever it appears.
        for args in [&["--help"][..], &["-h"], &["--seed", "7", "--help"]] {
            assert!(parse_args(args).unwrap().help, "{args:?}");
            assert!(run(args).is_ok(), "{args:?}");
        }
        let text = help();
        assert!(text.contains("cargo xtask agent-run"));
        assert!(text.contains("watch --help"), "the top-level help points at watch's own");
    }

    /// Exercised without ever spawning a process (real or stub): a bogus name is never found, a
    /// name dropped into a directory added to `$PATH` is. Deliberately not an integration test
    /// that runs the real binary without a stub on `$PATH` -- that could end up resolving (and
    /// this crate must never invoke) a real `claude` if one happens to be installed.
    #[test]
    fn claude_available_checks_path_without_ever_invoking_anything() {
        assert!(!claude_available("fn-agent-run-test-definitely-not-a-real-binary"));

        let dir = tempdir();
        let fake = dir.join("claude");
        fs::write(&fake, "#!/bin/sh\n").unwrap();
        let old_path = std::env::var_os("PATH");
        let new_path = match &old_path {
            Some(p) => {
                let mut paths = vec![dir.clone()];
                paths.extend(std::env::split_paths(p));
                std::env::join_paths(paths).unwrap()
            }
            None => dir.clone().into_os_string(),
        };
        // SAFETY: tests in this module don't run this one concurrently with another that reads
        // `$PATH`.
        unsafe { std::env::set_var("PATH", &new_path) };
        let found = claude_available("claude");
        match old_path {
            Some(p) => unsafe { std::env::set_var("PATH", p) },
            None => unsafe { std::env::remove_var("PATH") },
        }
        assert!(found);
        fs::remove_dir_all(&dir).ok();
    }

    // ----- OTel attributes (#494) -----

    #[test]
    fn otel_encode_escapes_outside_the_unreserved_set() {
        assert_eq!(otel_encode("abc-._~123"), "abc-._~123");
        assert_eq!(otel_encode("a b,c"), "a%20b%2Cc");
    }

    #[test]
    fn otel_attrs_cover_seed_nation_difficulty_and_mode_with_no_chunk() {
        let o = parse_args(&["--seed", "11", "--nation", "fire", "--difficulty", "hard"]).unwrap();
        let dir = PathBuf::from("/tmp/x/20260101-000000-fire-11-hard");
        let attrs = otel_attrs(&o, &dir, "interactive", None);
        assert_eq!(
            attrs,
            "four_nations.run=20260101-000000-fire-11-hard,four_nations.seed=11,\
             four_nations.nation=fire,four_nations.difficulty=hard,four_nations.mode=interactive"
        );
    }

    #[test]
    fn otel_attrs_use_scenario_instead_of_seed_nation_difficulty() {
        let o = parse_args(&["--scenario", "earth-year-two", "--max-usd", "1"]).unwrap();
        let dir = PathBuf::from("/tmp/x/20260101-000000-scenario-earth-year-two");
        let attrs = otel_attrs(&o, &dir, "scripts", Some(2));
        assert_eq!(
            attrs,
            "four_nations.run=20260101-000000-scenario-earth-year-two,\
             four_nations.scenario=earth-year-two,four_nations.mode=scripts,\
             four_nations.chunk=2"
        );
    }

    #[test]
    fn otel_attrs_append_to_an_existing_caller_value_rather_than_replace_it() {
        // SAFETY: tests in this module don't run this one concurrently with another that reads
        // the same env var.
        unsafe { std::env::set_var("OTEL_RESOURCE_ATTRIBUTES", "caller.tag=keep-me") };
        let o =
            parse_args(&["--seed", "1", "--nation", "water", "--difficulty", "normal"]).unwrap();
        let dir = PathBuf::from("/tmp/x/run");
        let attrs = otel_attrs(&o, &dir, "mcp", None);
        unsafe { std::env::remove_var("OTEL_RESOURCE_ATTRIBUTES") };
        assert!(attrs.starts_with("caller.tag=keep-me,four_nations.run="));
    }

    // ----- the run log -----

    fn write_lines(path: &Path, lines: &[&str]) {
        fs::write(path, lines.join("\n") + "\n").unwrap();
    }

    #[test]
    fn turns_done_reads_the_highest_turn_index() {
        let dir = tempdir();
        write_lines(
            &dir.join("run.jsonl"),
            &[
                r#"{"type":"header"}"#,
                r#"{"type":"turn","index":1,"tick":200}"#,
                r#"{"type":"turn","index":2,"tick":400}"#,
            ],
        );
        assert_eq!(turns_done(&dir).unwrap(), 2);
        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn turns_done_is_zero_with_no_log_yet() {
        let dir = tempdir();
        assert_eq!(turns_done(&dir).unwrap(), 0);
        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn finish_log_appends_an_end_line_with_the_last_turns_tick() {
        let dir = tempdir();
        let log = dir.join("run.jsonl");
        write_lines(&log, &[r#"{"type":"turn","index":1,"tick":200}"#]);
        end_log(&dir, "agent finished").unwrap();
        let text = fs::read_to_string(&log).unwrap();
        let lines: Vec<&str> = text.lines().collect();
        assert_eq!(lines.len(), 2);
        let last: serde_json::Value = serde_json::from_str(lines[1]).unwrap();
        assert_eq!(last["type"], "end");
        assert_eq!(last["tick"], 200);
        assert_eq!(last["reason"], "agent finished");
        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn finish_log_is_a_no_op_once_an_end_line_is_already_there() {
        let dir = tempdir();
        let log = dir.join("run.jsonl");
        write_lines(
            &log,
            &[
                r#"{"type":"turn","index":1,"tick":200}"#,
                r#"{"type":"end","tick":200,"reason":"x"}"#,
            ],
        );
        end_log(&dir, "agent finished").unwrap();
        let text = fs::read_to_string(&log).unwrap();
        assert_eq!(text.lines().count(), 2);
        fs::remove_dir_all(&dir).ok();
    }

    /// #511: a mid-log `end` (a chunk's session that was stopped, then resumed) is not the run's
    /// end; only the last line being `end` is.
    #[test]
    fn finish_log_still_ends_a_log_whose_only_end_line_is_mid_log() {
        let dir = tempdir();
        let log = dir.join("run.jsonl");
        write_lines(
            &log,
            &[
                r#"{"type":"turn","index":1,"tick":200}"#,
                r#"{"type":"end","tick":200,"reason":"stopped by SIGTERM"}"#,
                r#"{"type":"turn","index":2,"tick":400}"#,
            ],
        );
        end_log(&dir, "agent finished").unwrap();
        let text = fs::read_to_string(&log).unwrap();
        let lines: Vec<&str> = text.lines().collect();
        assert_eq!(lines.len(), 4);
        let last: serde_json::Value = serde_json::from_str(lines[3]).unwrap();
        assert_eq!(last["type"], "end");
        assert_eq!(last["tick"], 400);
        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn finish_log_is_a_no_op_with_no_log_file_at_all() {
        let dir = tempdir();
        end_log(&dir, "agent finished").unwrap();
        assert!(!dir.join("run.jsonl").exists());
        fs::remove_dir_all(&dir).ok();
    }

    // ----- script library, carry and sdk gaps -----

    #[test]
    fn library_notes_listing_reports_scripts_and_their_last_outcome() {
        let dir = tempdir();
        let log = dir.join("run.jsonl");
        write_lines(
            &log,
            &[
                r#"{"type":"script","name":"haul_wood","version":1,"code":"x","turns":3,"wake_on":[]}"#,
                r#"{"type":"turn","index":1,"tick":200}"#,
                r#"{"type":"turn","index":2,"tick":400}"#,
                r#"{"type":"wake","reason":"turns ended","why":null,"turn":2,"memory":{}}"#,
                r#"{"type":"note","text":"SDK gap: no way to query stockpiles directly"}"#,
            ],
        );
        let listing = library_notes_listing(&log).unwrap();
        assert!(listing.contains("haul_wood v1 (last: turns ended, 2 turns)"));
        assert!(listing.contains("SDK gap: no way to query stockpiles directly"));
        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn library_notes_listing_with_nothing_saved_yet() {
        let dir = tempdir();
        let log = dir.join("run.jsonl");
        write_lines(&log, &[r#"{"type":"header"}"#]);
        let listing = library_notes_listing(&log).unwrap();
        assert!(listing.contains("Scripts: none saved yet."));
        assert!(listing.contains("Notes: none yet."));
        assert!(listing.contains("Plan: none set yet."));
        fs::remove_dir_all(&dir).ok();
    }

    /// #531: the handoff listing shows the current (latest) plan next to scripts and notes.
    #[test]
    fn library_notes_listing_shows_the_current_plan() {
        let dir = tempdir();
        let log = dir.join("run.jsonl");
        write_lines(
            &log,
            &[
                r#"{"type":"plan","plan":{"goal":"old"}}"#,
                r#"{"type":"script","name":"governor","version":1,"code":"x","turns":0,"wake_on":[]}"#,
                r#"{"type":"plan","plan":{"goal":"nine signs","order":["beds","stone"]}}"#,
                r#"{"type":"note","text":"beds first"}"#,
            ],
        );
        let listing = library_notes_listing(&log).unwrap();
        assert!(listing.contains("Plan:\n{\"goal\":\"nine signs\""), "{listing}");
        assert!(!listing.contains("old"), "{listing}");
        assert!(listing.contains("governor v1"));
        assert!(listing.contains("beds first"));
        fs::remove_dir_all(&dir).ok();
    }

    /// #531: both script-mode prompts teach one governor run from plan(), plan edits for
    /// intent, revising rather than replacing, read-only inline probes, and notes.
    #[test]
    fn script_prompts_teach_the_governor_and_the_plan() {
        let first = default_script_prompt(540, 30);
        let handoff = scripts_handoff_prompt(540, 100, 30, "(listing)", None);
        assert!(handoff.contains("(listing)"));
        for prompt in [&first, &handoff] {
            for phrase in [
                "\"governor\"",
                "plan()",
                "plan tool",
                "revise the governor",
                "Use inline run_script code only to look",
                "read-only",
                "notes",
                "SDK gap",
            ] {
                let hit =
                    prompt.contains(phrase) || prompt.contains(&phrase.replace("revise", "Revise"));
                assert!(hit, "missing {phrase:?} in: {prompt}");
            }
        }
        // #557: the layout rules reach both, as one shared passage.
        for prompt in [&first, &handoff] {
            assert!(prompt.contains(script_style_passage()), "{prompt}");
            for phrase in [
                "one statement per line",
                "4-space indent",
                "under 80 characters",
                "own named fn",
                "change only what needs changing",
            ] {
                assert!(prompt.contains(phrase), "missing {phrase:?} in: {prompt}");
            }
        }
        assert!(first.contains("how the run is scored"), "{first}");
        assert!(!first.contains("for one routine job"), "the per-job wording is gone: {first}");
    }

    #[test]
    fn carry_save_then_carry_block_round_trips_a_script_and_a_note() {
        let dir = tempdir();
        let log = dir.join("run.jsonl");
        write_lines(
            &log,
            &[
                r#"{"type":"script","name":"haul_wood","version":2,"code":"run()","turns":1,"wake_on":[]}"#,
                r#"{"type":"note","text":"going well"}"#,
            ],
        );
        let carry_dir = dir.join("carry");
        carry_save(&log, &carry_dir).unwrap();
        let block = carry_block(&carry_dir).unwrap().expect("something to carry");
        assert!(block.contains("Script `haul_wood` (version 2)"));
        assert!(block.contains("run()"));
        assert!(block.contains("going well"));
        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn carry_save_then_carry_block_round_trips_the_latest_plan() {
        let dir = tempdir();
        let log = dir.join("run.jsonl");
        write_lines(
            &log,
            &[
                r#"{"type":"plan","plan":{"goal":"old"}}"#,
                r#"{"type":"plan","plan":{"goal":"nine signs","order":["beds","stone"]}}"#,
            ],
        );
        let carry_dir = dir.join("carry");
        carry_save(&log, &carry_dir).unwrap();
        let saved = fs::read_to_string(carry_dir.join("handoff.json")).unwrap();
        let handoff: serde_json::Value = serde_json::from_str(&saved).unwrap();
        assert_eq!(
            handoff["plan"],
            serde_json::json!({"goal":"nine signs","order":["beds","stone"]})
        );
        let block = carry_block(&carry_dir).unwrap().expect("a plan alone is worth carrying");
        assert!(block.contains("call plan with set"));
        assert!(block.contains(r#""order":["beds","stone"]"#));
        assert!(!block.contains("old"));
        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn carry_with_no_plan_set_has_no_plan_block() {
        let dir = tempdir();
        let log = dir.join("run.jsonl");
        write_lines(
            &log,
            &[r#"{"type":"note","text":"going well"}"#, r#"{"type":"plan","plan":{}}"#],
        );
        let carry_dir = dir.join("carry");
        carry_save(&log, &carry_dir).unwrap();
        let block = carry_block(&carry_dir).unwrap().unwrap();
        assert!(!block.contains("call plan"));
        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn carry_block_is_none_with_nothing_saved_yet() {
        let dir = tempdir();
        assert!(carry_block(&dir.join("never-written")).unwrap().is_none());
        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn write_sdk_gaps_collects_only_notes_mentioning_sdk_gap() {
        let dir = tempdir();
        write_lines(
            &dir.join("run.jsonl"),
            &[
                r#"{"type":"note","text":"everyone is happy"}"#,
                r#"{"type":"note","text":"SDK gap: can't query stockpiles"}"#,
            ],
        );
        write_sdk_gaps(&dir).unwrap();
        let text = fs::read_to_string(dir.join("sdk-gaps.md")).unwrap();
        assert!(text.contains("can't query stockpiles"));
        assert!(!text.contains("everyone is happy"));
        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn write_sdk_gaps_notes_when_there_are_none() {
        let dir = tempdir();
        write_lines(&dir.join("run.jsonl"), &[r#"{"type":"note","text":"all fine"}"#]);
        write_sdk_gaps(&dir).unwrap();
        let text = fs::read_to_string(dir.join("sdk-gaps.md")).unwrap();
        assert!(text.contains("None noted this run."));
        fs::remove_dir_all(&dir).ok();
    }

    // ----- cost tracking -----

    fn result_json(dir: &Path, name: &str, text: &str) -> Option<ClaudeResult> {
        let path = dir.join(name);
        fs::write(&path, text).unwrap();
        read_claude_result(&path)
    }

    #[test]
    fn update_cost_sums_across_chunks() {
        let dir = tempdir();
        let chunk1 = result_json(
            &dir,
            "chunk-1.json",
            r#"{"num_turns":3,"usage":{"input_tokens":100,"output_tokens":20},"total_cost_usd":0.05,"result":"ok"}"#,
        );
        update_cost(&dir, 1, chunk1.as_ref()).unwrap();
        let chunk2 = result_json(
            &dir,
            "chunk-2.json",
            r#"{"num_turns":2,"usage":{"input_tokens":50,"output_tokens":10},"total_cost_usd":0.03,"result":"ok again"}"#,
        );
        update_cost(&dir, 2, chunk2.as_ref()).unwrap();
        let totals: CostTotals = read_json_or_default(&dir.join("cost.json"));
        assert_eq!(totals.round_trips, 5);
        assert_eq!(totals.input_tokens, 150);
        assert_eq!(totals.output_tokens, 30);
        assert!((totals.usd - 0.08).abs() < 1e-9);
        assert!(totals.unknown_chunks.is_empty());
        assert!(!fs::read_to_string(dir.join("cost.json")).unwrap().contains("unknown"));
        fs::remove_dir_all(&dir).ok();
    }

    /// #568: a result with no readable cost is unknown, never a free chunk.
    #[test]
    fn a_cost_that_cannot_be_read_is_unknown_not_zero() {
        let dir = tempdir();
        assert!(result_json(&dir, "a.json", "not json").is_none());
        assert!(read_claude_result(&dir.join("missing.json")).is_none());
        for text in
            [r#"{"result":"report"}"#, r#"{"total_cost_usd":null}"#, r#"{"total_cost_usd":-1}"#]
        {
            let r = result_json(&dir, "b.json", text).expect("it parses");
            assert_eq!(r.cost(), None, "{text}");
        }
        assert_eq!(
            result_json(&dir, "c.json", r#"{"total_cost_usd":0}"#).unwrap().cost(),
            Some(0.0)
        );

        // The chunk is listed, and adds nothing to `usd`.
        update_cost(&dir, 4, None).unwrap();
        let r = result_json(&dir, "d.json", r#"{"total_cost_usd":0.5}"#);
        update_cost(&dir, 5, r.as_ref()).unwrap();
        let totals: CostTotals = read_json_or_default(&dir.join("cost.json"));
        assert_eq!(totals.unknown_chunks, [4]);
        assert!((totals.usd - 0.5).abs() < 1e-9);
        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn settle_chunk_stops_only_for_an_unreadable_cost() {
        let dir = tempdir();
        let ok = ChunkRun {
            failure: None,
            result: result_json(&dir, "1.json", r#"{"total_cost_usd":0.2,"result":"done it"}"#),
        };
        let settled = settle_chunk(&dir, 1, &ok).unwrap();
        assert_eq!((settled.report.as_str(), settled.stop), ("done it", None));

        // Failed, but the result has its cost (what a budget stop may look like): charged,
        // reported, not a reason to stop by itself.
        let failed_known = ChunkRun {
            failure: Some("claude ended with exit status: 1".into()),
            result: result_json(&dir, "2.json", r#"{"total_cost_usd":0.3}"#),
        };
        let settled = settle_chunk(&dir, 2, &failed_known).unwrap();
        assert!(settled.stop.is_none());
        assert!(settled.report.contains("exit status: 1"), "{}", settled.report);
        assert!((cost_so_far(&dir) - 0.5).abs() < 1e-9);

        let failed = ChunkRun { failure: Some("claude ended with signal: 9".into()), result: None };
        let stop = settle_chunk(&dir, 3, &failed).unwrap().stop.unwrap();
        assert!(stop.contains("signal: 9") && stop.contains("unknown, not $0"), "{stop}");
        let silent = ChunkRun { failure: None, result: result_json(&dir, "4.json", "{}") };
        let stop = settle_chunk(&dir, 4, &silent).unwrap().stop.unwrap();
        assert!(stop.contains("exited 0 but left no result"), "{stop}");
        assert!((cost_so_far(&dir) - 0.5).abs() < 1e-9, "unknown chunks add nothing");
        let agent_log = fs::read_to_string(dir.join("agent.log")).unwrap();
        assert!(agent_log.contains("chunk 3 failed: claude ended with signal: 9"), "{agent_log}");
        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn cost_so_far_and_budget_remaining_with_no_cost_json_yet() {
        let dir = tempdir();
        assert_eq!(cost_so_far(&dir), 0.0);
        assert_eq!(budget_remaining(&dir, 5.0), 5.0);
        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn budget_remaining_floors_at_zero() {
        let dir = tempdir();
        fs::write(dir.join("cost.json"), r#"{"usd": 7.5}"#).unwrap();
        assert_eq!(budget_remaining(&dir, 5.0), 0.0);
        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn budget_args_are_claudes_own_flag() {
        assert_eq!(budget_args(5.0), ["--max-budget-usd", "5"]);
        assert_eq!(budget_args(0.25), ["--max-budget-usd", "0.25"]);
    }

    /// #568: an ordinary run gives one idle chunk grace; a campaign attempt does not (#569).
    #[test]
    fn idle_chunks_end_an_ordinary_run_on_the_second_and_an_attempt_on_the_first() {
        let mut ordinary = Idle::for_run(&parse_args(&["--scripts"]).unwrap());
        assert!(!ordinary.chunk_done(3, 3), "one idle chunk");
        assert!(!ordinary.chunk_done(3, 4), "work resets the streak");
        assert!(!ordinary.chunk_done(4, 4));
        assert!(ordinary.chunk_done(4, 4), "two in a row");
        let mut attempt = Idle::for_run(&Opts {
            trial: Some((vec![1], 2)),
            ..parse_args(&["--scripts"]).unwrap()
        });
        assert!(attempt.chunk_done(3, 3));
    }

    // ----- --playtest (#576) -----

    /// FNV-1a, for pinning a prompt's text without copying all of it into the test.
    fn fnv(text: &str) -> u64 {
        text.bytes()
            .fold(0xcbf2_9ce4_8422_2325, |h, b| (h ^ u64::from(b)).wrapping_mul(0x0100_0000_01b3))
    }

    /// Campaign and scored `--scripts` prompts must not change because a playtest exists: their
    /// text, with the shared layout passage (pinned by its own tests) taken out, is pinned.
    /// If this fails after you edited one of them on purpose, update the number; if you did not,
    /// something is leaking playtest wording into a scored run.
    #[test]
    fn the_scored_prompts_are_byte_for_byte_what_they_were() {
        let style = script_style_passage();
        let first = default_script_prompt(540, 30).replace(style, "<STYLE>");
        let handoff =
            scripts_handoff_prompt(540, 100, 30, "<LISTING>", None).replace(style, "<STYLE>");
        let campaign = scripts_handoff_prompt(540, 100, 30, "<LISTING>", Some("<NOTE>"))
            .replace(style, "<STYLE>");
        assert_eq!(
            [fnv(&first), fnv(&handoff), fnv(&campaign)],
            PINNED_SCORED_PROMPTS,
            "first:\n{first}\n\nhandoff:\n{handoff}\n\ncampaign:\n{campaign}"
        );
        for prompt in [&first, &handoff, &campaign] {
            for playtest_word in ["playtest", "game problem", "remark"] {
                assert!(
                    !prompt.to_lowercase().contains(playtest_word),
                    "{playtest_word}: {prompt}"
                );
            }
        }
        // And the playtest's prompts are their own.
        assert_ne!(first, playtest::first_prompt(540, 30).replace(style, "<STYLE>"));
    }

    const PINNED_SCORED_PROMPTS: [u64; 3] =
        [529_216_101_422_545_671, 2_747_889_883_185_109_194, 17_751_177_510_856_055_539];

    #[test]
    fn playtest_implies_scripts_and_is_refused_with_a_campaign_a_chat_or_chunk_turns() {
        let o = parse_args(&["--playtest", "--max-usd", "3"]).unwrap();
        assert!(o.playtest && o.scripts);
        validate(&o).unwrap();
        assert!(!parse_args(&["--scripts", "--max-usd", "3"]).unwrap().playtest);

        for (extra, want) in [
            (&["--track", "t", "--attempts", "1"][..], "--playtest can't be combined with --track"),
            (&["--resume", "dir"][..], "--playtest can't be combined with --resume"),
            (&["--interactive"][..], "--playtest and --interactive"),
            (&["--chunk-turns", "5"][..], "--playtest and --chunk-turns"),
        ] {
            let mut args = vec!["--playtest", "--max-usd", "3"];
            args.extend_from_slice(extra);
            let err = validate(&parse_args(&args).unwrap()).unwrap_err();
            assert!(err.contains(want), "{extra:?}: {err}");
        }
        // It still needs a cap: it calls the model.
        let err = validate(&parse_args(&["--playtest"]).unwrap()).unwrap_err();
        assert!(err.contains("--max-usd"), "{err}");
    }

    #[test]
    fn only_a_playtest_asks_fn_agent_for_the_playtest_header() {
        let log = Path::new("/tmp/run.jsonl");
        let plain = serve_args(&parse_args(&["--scripts", "--max-usd", "1"]).unwrap(), log);
        assert!(!plain.contains(&"--playtest".to_string()), "{plain:?}");
        let play = serve_args(&parse_args(&["--playtest", "--max-usd", "1"]).unwrap(), log);
        assert!(
            play.contains(&"--playtest".to_string()) && play.contains(&"--scripts".to_string())
        );
        // A resumed chunk reads it from the header.
        assert!(!resume_args(log, true).contains(&"--playtest".to_string()));
    }

    #[test]
    fn the_run_list_marks_a_playtest_instead_of_scoring_it() {
        let rows = |playtest: bool| {
            let row = RunRow {
                name: "r".into(),
                nation: "earth".into(),
                start: "seed 1".into(),
                turns: 3,
                score: Some(55.5),
                playtest,
                campaign: false,
                detail: None,
            };
            format_run_list(Path::new("/runs"), &[row])
        };
        assert!(rows(true).contains("playtest") && !rows(true).contains("55.5"));
        assert!(rows(false).contains("55.5") && !rows(false).contains("playtest"));
    }

    // ----- fn_agent CLI arguments and mcp.json -----

    #[test]
    fn serve_args_use_scenario_or_seed_nation_difficulty() {
        let o = parse_args(&["--scenario", "earth-year-two", "--max-usd", "1"]).unwrap();
        let args = serve_args(&o, Path::new("/tmp/run.jsonl"));
        assert_eq!(args, vec!["serve", "--scenario", "earth-year-two", "--log", "/tmp/run.jsonl"]);

        let o = parse_args(&["--seed", "3", "--nation", "water", "--turn-secs", "10"]).unwrap();
        let args = serve_args(&o, Path::new("/tmp/run.jsonl"));
        assert_eq!(
            args,
            vec![
                "serve",
                "--seed",
                "3",
                "--nation",
                "water",
                "--difficulty",
                "normal",
                "--log",
                "/tmp/run.jsonl",
                "--turn",
                "10"
            ]
        );
    }

    #[test]
    fn write_mcp_config_round_trips_through_serde_json() {
        let dir = tempdir();
        let path = dir.join("mcp.json");
        write_mcp_config(
            &path,
            Path::new("/bin/fn_agent"),
            &["serve".to_string(), "--log".to_string(), "/tmp/run.jsonl".to_string()],
        )
        .unwrap();
        let value: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
        assert_eq!(value["mcpServers"]["four-nations"]["command"], "/bin/fn_agent");
        assert_eq!(value["mcpServers"]["four-nations"]["args"][0], "serve");
        fs::remove_dir_all(&dir).ok();
    }

    // ----- the watch build skip (#510) -----

    #[test]
    fn build_is_stale_only_when_missing_or_an_input_is_newer() {
        let t = |s: u64| Some(SystemTime::UNIX_EPOCH + Duration::from_secs(s));
        assert!(build_is_stale(None, t(10)), "no binary: build");
        assert!(build_is_stale(None, None), "no binary and no inputs: build");
        assert!(build_is_stale(t(10), t(20)), "source newer than the binary: build");
        assert!(!build_is_stale(t(20), t(10)), "binary newer than every input: skip");
        assert!(!build_is_stale(t(10), t(10)), "same instant: skip");
        assert!(!build_is_stale(t(10), None), "nothing to compare against: skip");
    }

    #[test]
    fn newest_mtime_walks_nested_dirs() {
        let dir = tempdir();
        assert_eq!(newest_mtime(&dir), None);
        fs::create_dir_all(dir.join("a/b")).unwrap();
        let set = |p: PathBuf, secs: u64| {
            let f = File::create(&p).unwrap();
            f.set_modified(SystemTime::UNIX_EPOCH + Duration::from_secs(secs)).unwrap();
        };
        set(dir.join("old.rs"), 100);
        set(dir.join("a/b/new.rs"), 300);
        set(dir.join("a/mid.rs"), 200);
        assert_eq!(newest_mtime(&dir), Some(SystemTime::UNIX_EPOCH + Duration::from_secs(300)));
        fs::remove_dir_all(&dir).ok();
    }

    // ----- agent-run watch (#553) -----

    fn watch_opts(args: &[&str]) -> WatchOpts {
        match parse_watch(args).unwrap() {
            WatchCmd::Watch(o) => o,
            other => panic!("expected a watch, got {other:?}"),
        }
    }

    #[test]
    fn watch_help_is_a_request_wherever_it_appears() {
        for args in [&["--help"][..], &["-h"], &["--bogus", "--help"], &["somewhere", "-h"]] {
            assert_eq!(parse_watch(args).unwrap(), WatchCmd::Help, "{args:?}");
        }
        // And running it succeeds without a repo to build in or a TUI to open.
        assert!(run_watch(Path::new("/nonexistent"), &["--help"]).is_ok());
        assert!(run(&["watch", "-h"]).is_ok());
    }

    #[test]
    fn watch_help_names_every_part_of_the_issue() {
        for want in [
            "usage: cargo xtask agent-run watch",
            "RUN_DIR",
            "--list",
            "--no-build",
            "--pace",
            "--record",
            "--size",
            "--fps",
            "--ascii",
            "--color",
            "160x45",
            "space",
            "+ or =",
            "- or _",
            "  .  ",
            "  ]  ",
            "  v  ",
            "1  2  3",
            "  0  ",
            "j  k",
            "  q  ",
            "free look",
            "fn-agent-runs",
        ] {
            assert!(WATCH_HELP.contains(want), "watch --help should mention {want:?}");
        }
    }

    /// The help's facts come from `fn_tui`: its pace ladder, its keys and the studio's size.
    /// Reading the sources keeps this copy from drifting.
    #[test]
    fn watch_help_matches_what_fn_tui_does() {
        let watch_rs = include_str!("../../crates/tui/src/watch.rs");
        let ladder = WATCH_PACES.map(|p| p.to_string()).join(", ");
        assert!(
            watch_rs
                .contains(&format!("pub const PACES: [u32; {}] = [{ladder}];", WATCH_PACES.len())),
            "WATCH_PACES must match watch::PACES"
        );
        for handler in [
            "KeyCode::Char(' ') => watcher.toggle_pause()",
            "KeyCode::Char('+' | '=') => watcher.faster()",
            "KeyCode::Char('-' | '_') => watcher.slower()",
            "KeyCode::Char('.') => watcher.step_one()",
            "KeyCode::Char(']') => watcher.fast_forward_to_turn(watcher.turn() + 1)",
            "KeyCode::Char('q' | 'Q') => quit = true",
            "KeyCode::Char('v' | 'V') => self.toggle_view()",
            "KeyCode::Char('1') => return self.press_pane(Pane::Code)",
            "KeyCode::Char('2') => return self.press_pane(Pane::Game)",
            "KeyCode::Char('3') => return self.press_pane(Pane::Feed)",
            "KeyCode::Char('0') => return self.press_zero()",
        ] {
            assert!(watch_rs.contains(handler), "watch.rs no longer has {handler:?}");
        }
        let studio_rs = include_str!("../../crates/tui/src/studio.rs");
        for scroll in ["'j'", "'k'", "'J'", "'K'"] {
            assert!(studio_rs.contains(&format!("KeyCode::Char({scroll}) => self.scroll_by")));
        }
        let tui_toml = include_str!("../../data/tui.toml");
        assert!(tui_toml.contains("min_width = 160"), "the studio threshold moved");
        assert!(tui_toml.contains("min_height = 45"), "the studio threshold moved");
        let cli_rs = include_str!("../../crates/tui/src/cli.rs");
        assert!(cli_rs.contains("(1..=record::MAX_FPS)"));
        assert!(
            include_str!("../../crates/tui/src/record.rs")
                .contains(&format!("pub const MAX_FPS: u32 = {WATCH_MAX_FPS};"))
        );
    }

    #[test]
    fn watch_defaults() {
        let o = watch_opts(&[]);
        assert_eq!(o, WatchOpts::live(20));
        assert_eq!(o.dir, None);
        assert!(!o.no_build);
        assert!(watch_opts(&["--no-build", "some/dir"]).no_build);
        assert_eq!(watch_opts(&["some/dir"]).dir.as_deref(), Some("some/dir"));
    }

    #[test]
    fn watch_pace_takes_what_fn_tui_takes() {
        for (arg, want) in
            [("1", 1), ("2", 2), ("5", 5), ("10", 10), ("20", 20), ("40", 40), ("500", 500)]
        {
            assert_eq!(watch_opts(&["--pace", arg]).pace, want);
            assert_eq!(watch_opts(&["--pace", &format!("x{arg}")]).pace, want, "the xN form");
        }
        assert_eq!(watch_opts(&["--pace=5"]).pace, 5, "the = form");
        for bad in ["3", "0", "fast", "x", "-1", "1000"] {
            let err = parse_watch(&["--pace", bad]).unwrap_err();
            assert!(err.contains("--pace wants one of"), "{bad}: {err}");
            assert!(err.contains("watch --help"), "{err}");
        }
        assert!(parse_watch(&["--pace"]).unwrap_err().contains("--pace needs a value"));
    }

    #[test]
    fn watch_unknown_flags_fail_naming_the_flag() {
        for flag in ["--bogus", "-x", "--frame", "--keys", "--at", "--ansi"] {
            let err = parse_watch(&[flag]).unwrap_err();
            assert!(err.contains(flag), "{err}");
            assert!(err.contains("watch --help"), "{err}");
        }
        // A typo'd flag is not mistaken for a run folder.
        assert!(parse_watch(&["--no-bild", "dir"]).unwrap_err().contains("--no-bild"));
    }

    #[test]
    fn watch_takes_one_run_dir() {
        let err = parse_watch(&["a", "b"]).unwrap_err();
        assert!(err.contains("one RUN_DIR") && err.contains("\"a\"") && err.contains("\"b\""));
    }

    #[test]
    fn watch_record_options_pass_through_to_fn_tui() {
        let o = watch_opts(&[
            "run1", "--pace", "x10", "--record", "out.cast", "--fps", "30", "--size", "100x30",
            "--ascii", "--color", "never",
        ]);
        let args = fn_tui_args(Path::new("/r/run1/run.jsonl"), &o);
        assert_eq!(
            args,
            [
                "--watch",
                "/r/run1/run.jsonl",
                "--pace",
                "10",
                "--record",
                "out.cast",
                "--fps",
                "30",
                "--size",
                "100x30",
                "--ascii",
                "--color",
                "never"
            ]
        );
        // Plain: the log and the pace, 20 unless told.
        let plain = fn_tui_args(Path::new("/r/run.jsonl"), &watch_opts(&[]));
        assert_eq!(plain, ["--watch", "/r/run.jsonl", "--pace", "20"]);
    }

    #[test]
    fn watch_flags_that_only_mean_something_with_record_say_so() {
        assert!(parse_watch(&["--fps", "30"]).unwrap_err().contains("--fps only means"));
        assert!(parse_watch(&["--size", "80x24"]).unwrap_err().contains("--size only means"));
        assert!(parse_watch(&["--record", "o.cast", "--fps", "30"]).is_ok());
        for bad in ["0", "61", "x", "-3"] {
            assert!(parse_watch(&["--record", "o.cast", "--fps", bad]).is_err(), "{bad}");
        }
        for bad in ["small", "80", "0x24", "80x0", "axb"] {
            assert!(parse_watch(&["--record", "o.cast", "--size", bad]).is_err(), "{bad}");
        }
        assert!(parse_watch(&["--color", "mauve"]).unwrap_err().contains("--color wants"));
    }

    #[test]
    fn watch_list_stands_alone() {
        assert_eq!(parse_watch(&["--list"]).unwrap(), WatchCmd::List);
        for extra in [&["--pace", "5"][..], &["--no-build"], &["dir"], &["--record", "o.cast"]] {
            let mut args = vec!["--list"];
            args.extend(extra);
            let err = parse_watch(&args).unwrap_err();
            assert!(err.contains("--list can't be combined"), "{args:?}: {err}");
        }
    }

    #[test]
    fn a_bad_watch_fails_before_any_build() {
        // The repo doesn't exist, so a build (or a launch) would fail with a different message:
        // these errors can only come from the arguments, ahead of both.
        let repo = Path::new("/nonexistent/repo");
        let err = run_watch(repo, &["--bogus"]).unwrap_err();
        assert!(err.contains("--bogus"), "{err}");
        let err = run_watch(repo, &["/nonexistent/run-folder"]).unwrap_err();
        assert!(err.contains("/nonexistent/run-folder"), "{err}");
    }

    #[test]
    fn resolve_watch_dir_checks_the_folder_up_front() {
        let runs = tempdir();
        let with_dir = |d: &str| WatchOpts { dir: Some(d.to_string()), ..WatchOpts::live(20) };

        // Nothing there: names the path.
        let missing = runs.join("nope");
        let err = resolve_watch_dir(&with_dir(missing.to_str().unwrap()), &runs).unwrap_err();
        assert!(err.contains(missing.to_str().unwrap()), "{err}");

        // A file is not a run folder.
        let file = runs.join("file.txt");
        fs::write(&file, "x").unwrap();
        let err = resolve_watch_dir(&with_dir(file.to_str().unwrap()), &runs).unwrap_err();
        assert!(err.contains(file.to_str().unwrap()) && err.contains("isn't a directory"), "{err}");

        // A folder with no log yet is a run just starting: accepted (the wait comes later).
        let fresh = runs.join("20261108-120000-fire-7-normal");
        fs::create_dir_all(&fresh).unwrap();
        let (dir, latest) = resolve_watch_dir(&with_dir(fresh.to_str().unwrap()), &runs).unwrap();
        assert_eq!((dir.as_path(), latest), (fresh.as_path(), false));

        // A bare name is looked for under the runs folder; one that isn't there still fails.
        let (dir, _) =
            resolve_watch_dir(&with_dir("20261108-120000-fire-7-normal"), &runs).unwrap();
        assert_eq!(dir, fresh);
        assert!(resolve_watch_dir(&with_dir("no-such-run"), &runs).is_err());

        // No RUN_DIR: the newest, and it says so.
        let (dir, latest) = resolve_watch_dir(&WatchOpts::live(20), &runs).unwrap();
        assert!(latest);
        assert!(dir.is_dir());
        fs::remove_dir_all(&runs).ok();
    }

    fn write_run(runs: &Path, name: &str, lines: &[&str], age_secs: u64) -> PathBuf {
        let dir = runs.join(name);
        fs::create_dir_all(&dir).unwrap();
        if !lines.is_empty() {
            fs::write(dir.join("run.jsonl"), lines.join("\n") + "\n").unwrap();
        }
        let when = SystemTime::now() - Duration::from_secs(age_secs);
        File::open(&dir).unwrap().set_modified(when).unwrap();
        dir
    }

    const HEADER: &str = r#"{"type":"header","seed":7633,"nation":"water","turn_seconds":20,"ticks_per_turn":200,"map_size":null,"difficulty":"normal","site":null,"rough":false,"data_digest":1,"crate_version":"0.1.0","prelude":null}"#;
    const SCENARIO_HEADER: &str = r#"{"type":"header","seed":7,"nation":"earth","turn_seconds":20,"ticks_per_turn":200,"map_size":"small","difficulty":"normal","site":null,"rough":false,"data_digest":1,"crate_version":"0.1.0","prelude":{"opening":"economy","minutes":60.0}}"#;
    const TURN_1: &str = r#"{"type":"turn","index":1,"tick":200}"#;
    const TURN_2: &str = r#"{"type":"turn","index":2,"tick":400}"#;
    const END: &str = r#"{"type":"end","tick":400,"reason":"agent finished"}"#;
    const SCORE: &str = r#"{"type":"score","score":41.25,"progress":30.0,"care":0.9,"horizon":2}"#;

    #[test]
    fn list_runs_summarises_each_run_newest_first() {
        let runs = tempdir();
        write_run(
            &runs,
            "20261101-000000-water-7633-normal",
            &[HEADER, TURN_1, TURN_2, END, SCORE],
            300,
        );
        write_run(
            &runs,
            "20261102-000000-scenario-earth-year-two",
            &[SCENARIO_HEADER, TURN_1],
            200,
        );
        write_run(&runs, "20261103-000000-fire-9-normal", &[], 100);
        // A file in the runs folder is not a run.
        fs::write(runs.join("notes.txt"), "x").unwrap();

        let rows = list_runs(&runs, 20);
        let names: Vec<&str> = rows.iter().map(|r| r.name.as_str()).collect();
        assert_eq!(
            names,
            [
                "20261103-000000-fire-9-normal",
                "20261102-000000-scenario-earth-year-two",
                "20261101-000000-water-7633-normal"
            ]
        );
        assert_eq!(rows[0].nation, "-", "no log yet");
        assert_eq!(rows[0].turns, 0);
        assert_eq!(rows[1].nation, "earth");
        assert_eq!(rows[1].start, "scenario earth-year-two");
        assert_eq!(rows[1].turns, 1);
        assert_eq!(rows[1].score, None);
        assert_eq!(rows[2].nation, "water");
        assert_eq!(rows[2].start, "seed 7633");
        assert_eq!(rows[2].turns, 2);
        assert_eq!(rows[2].score, Some(41.25));

        assert_eq!(list_runs(&runs, 2).len(), 2, "the limit applies");

        let text = format_run_list(&runs, &rows);
        let lines: Vec<&str> = text.lines().collect();
        assert!(lines[0].starts_with("RUN") && lines[0].ends_with("SCORE"), "{text}");
        assert!(lines[1].contains("fire-9-normal"), "{text}");
        assert!(lines[3].contains("water") && lines[3].contains("seed 7633"), "{text}");
        assert!(lines[3].ends_with("41.2") || lines[3].ends_with("41.3"), "{text}");
        assert!(text.contains("agent-run watch NAME"), "{text}");
        fs::remove_dir_all(&runs).ok();
    }

    /// #585: the order is the date in the folder name. Writing into a folder (the run browser's
    /// `score-cache.json`) moves its modification time, and once put old runs above today's.
    #[test]
    fn lists_and_the_latest_run_go_by_the_date_in_the_name_not_the_folders_mtime() {
        let runs = tempdir();
        // The oldest name has the newest folder time.
        write_run(&runs, "20261104-000000-earth-7-normal", &[HEADER, TURN_1], 10);
        write_run(&runs, "20261107-000000-earth-7-normal", &[HEADER, TURN_1], 500);
        write_run(&runs, "20261108-000000-earth-7-normal", &[HEADER, TURN_1], 900);
        let names = |runs: &Path| -> Vec<String> {
            list_runs(runs, 20).into_iter().map(|r| r.name[..8].to_string()).collect()
        };
        assert_eq!(names(&runs), ["20261108", "20261107", "20261104"]);
        assert!(latest_run_dir(&runs).unwrap().ends_with("20261108-000000-earth-7-normal"));

        // A cache write into the oldest folder makes it the most recently modified, and changes
        // nothing about the order.
        fs::write(runs.join("20261104-000000-earth-7-normal/score-cache.json"), "{}").unwrap();
        assert_eq!(names(&runs), ["20261108", "20261107", "20261104"]);

        // A folder with no date in its name goes after the dated ones, newest log first.
        write_run(&runs, "scratch-a", &[HEADER], 100);
        write_run(&runs, "scratch-b", &[HEADER], 50);
        let all: Vec<String> = list_runs(&runs, 20).into_iter().map(|r| r.name).collect();
        assert_eq!(all[3..], ["scratch-b", "scratch-a"]);

        assert_eq!(folder_stamp("20261108-142213-water-7633-normal"), Some("20261108-142213"));
        assert_eq!(folder_stamp("20261108-142213"), Some("20261108-142213"));
        assert_eq!(folder_stamp("20261108-1422"), None);
        assert_eq!(folder_stamp("water-7633"), None);
        fs::remove_dir_all(&runs).ok();
    }

    /// A campaign in the first ledger shape (train and held-out) and one in the seed-tier shape.
    fn write_campaign(runs: &Path, name: &str, tiered: bool) -> PathBuf {
        let dir = runs.join(name);
        fs::create_dir_all(dir.join("attempt-1/eval/v3-2")).unwrap();
        fs::create_dir_all(dir.join("attempt-2")).unwrap();
        let (track, row) = if tiered {
            (
                r#"{"id":"water-3h","nation":"water","screen_seeds":[1],"gate_seeds":[2],"owner_seeds":[3]}"#,
                r#"{"attempt":1,"governor_version":3,"played":210.5,"screen":[],"screen_mean":275.0,"gate_mean":250.0,"kept":true,"cost_usd":0.5}"#,
            )
        } else {
            (
                r#"{"id":"water-3h","nation":"water","train_seeds":[1],"holdout_seeds":[2]}"#,
                r#"{"attempt":1,"governor_version":16,"played":355.3,"clean":[],"train_mean":102.3,"held_out":[],"kept":true,"cost_usd":0.5}"#,
            )
        };
        fs::write(dir.join("ledger.json"), format!(r#"{{"track":{track},"rows":[{row}]}}"#))
            .unwrap();
        fs::write(dir.join("attempt-1/run.jsonl"), [HEADER, TURN_1, TURN_2, END].join("\n") + "\n")
            .unwrap();
        fs::write(
            dir.join("attempt-1/eval/versions.json"),
            r#"{"pairs":[{"version":2,"chosen":false},{"version":3,"chosen":true}]}"#,
        )
        .unwrap();
        for seed in [1, 2] {
            fs::write(dir.join(format!("attempt-1/eval/v3-2/seed-{seed}.jsonl")), "x").unwrap();
        }
        // The second attempt has no ledger row and no live turns.
        fs::write(dir.join("attempt-2/run.jsonl"), HEADER.to_string() + "\n").unwrap();
        fs::write(dir.join("attempt-2/cost.json"), r#"{"usd":0.25}"#).unwrap();
        dir
    }

    #[test]
    fn list_shows_campaigns_with_their_attempts_in_either_ledger_shape() {
        let runs = tempdir();
        write_campaign(&runs, "20261108-100000-campaign-water-3h", false);
        write_campaign(&runs, "20261108-200000-campaign-water-3h", true);
        write_run(&runs, "20261108-150000-water-7633-normal", &[HEADER, TURN_1], 100);
        let rows = list_runs(&runs, 20);
        let names: Vec<&str> = rows.iter().map(|r| r.name.trim()).collect();
        assert_eq!(
            names,
            [
                "20261108-200000-campaign-water-3h",
                "attempt-1",
                "attempt-2",
                "20261108-150000-water-7633-normal",
                "20261108-100000-campaign-water-3h",
                "attempt-1",
                "attempt-2",
            ]
        );
        // The tiered campaign: best clean mean is the screen mean, and the attempts say what
        // they did.
        assert!(rows[0].campaign && rows[0].turns == 0 && rows[0].start == "water-3h");
        assert_eq!(rows[0].score, Some(275.0));
        let detail = rows[0].detail.as_deref().unwrap();
        assert!(detail.contains("2 attempt(s), 1 kept") && detail.contains("$0.75"), "{detail}");
        assert!(detail.contains("live: attempt 2 playing"), "just written, no row yet: {detail}");
        assert_eq!((rows[1].turns, rows[1].score), (2, Some(275.0)));
        let d1 = rows[1].detail.as_deref().unwrap();
        assert!(d1.contains("kept") && d1.contains("played 210.5"), "{d1}");
        assert!(d1.contains("2 clean run(s) in eval/v3-2"), "only the chosen pair: {d1}");
        let d2 = rows[2].detail.as_deref().unwrap();
        assert!(d2.contains("no ledger row yet") && d2.contains("no live turns"), "{d2}");
        assert_eq!(rows[2].turns, 0);
        // The first shape reads its train mean.
        assert_eq!(rows[4].score, Some(102.3));
        assert_eq!(rows[5].start, "v16");

        let text = format_run_list(&runs, &rows);
        assert!(text.lines().next().unwrap().ends_with("DETAIL"), "{text}");
        let camp = text.lines().nth(1).unwrap();
        assert!(camp.contains("water-3h") && camp.contains(" -  "), "no turns of its own: {camp}");
        assert!(text.contains("  attempt-1"), "attempts are indented under it:\n{text}");
        assert!(text.contains("watch PATH/seed-N.jsonl"), "{text}");
        assert!(text.lines().all(|l| l == l.trim_end()), "no trailing spaces:\n{text}");
        fs::remove_dir_all(&runs).ok();
    }

    #[test]
    fn a_campaign_with_damaged_files_still_lists_with_dashes() {
        let runs = tempdir();
        let dir = runs.join("20261108-100000-campaign-water-3h");
        fs::create_dir_all(dir.join("attempt-1")).unwrap();
        fs::write(dir.join("ledger.json"), "{ nope").unwrap();
        let rows = list_runs(&runs, 20);
        assert_eq!(rows.len(), 2);
        assert_eq!(
            (rows[0].nation.as_str(), rows[0].start.as_str(), rows[0].score),
            ("-", "-", None)
        );
        assert_eq!(
            (rows[1].nation.as_str(), rows[1].start.as_str(), rows[1].score),
            ("-", "-", None)
        );
        assert!(format_run_list(&runs, &rows).contains("attempt-1"));
        fs::remove_dir_all(&runs).ok();
    }

    #[test]
    fn watch_takes_a_campaign_folder_or_a_log_file() {
        let runs = tempdir();
        let with_dir =
            |d: &Path| WatchOpts { dir: Some(d.display().to_string()), ..WatchOpts::live(20) };
        let campaign = write_campaign(&runs, "20261108-100000-campaign-water-3h", true);
        assert!(is_campaign_dir(&campaign));
        assert!(!is_campaign_dir(&campaign.join("attempt-1")), "an attempt is a plain run");
        let (dir, latest) = resolve_watch_dir(&with_dir(&campaign), &runs).unwrap();
        assert_eq!((dir.as_path(), latest), (campaign.as_path(), false));
        assert_eq!(attempt_dirs(&campaign).iter().map(|(k, _)| *k).collect::<Vec<_>>(), [1, 2]);

        // A clean evaluation log opens as it is; some other file is still refused.
        let log = campaign.join("attempt-1/eval/v3-2/seed-1.jsonl");
        let (dir, _) = resolve_watch_dir(&with_dir(&log), &runs).unwrap();
        assert_eq!(dir, log);
        let args = fn_tui_args(&log, &watch_opts(&[]));
        assert_eq!(args[..2], ["--watch", log.to_str().unwrap()]);
        fs::write(runs.join("notes.txt"), "x").unwrap();
        assert!(resolve_watch_dir(&with_dir(&runs.join("notes.txt")), &runs).is_err());

        // The browser opens on the campaign.
        let args = fn_tui_runs_args(&runs, &watch_opts(&[]), Some(&campaign));
        assert_eq!(args[..2], ["--runs", runs.to_str().unwrap()]);
        assert!(args.windows(2).any(|w| w[0] == "--focus" && w[1] == campaign.to_str().unwrap()));
        assert!(!fn_tui_runs_args(&runs, &watch_opts(&[]), None).contains(&"--focus".to_string()));
        assert!(WATCH_HELP.contains("campaign folder") && WATCH_HELP.contains("right or l"));
        fs::remove_dir_all(&runs).ok();
    }

    #[test]
    fn list_with_no_runs_says_so() {
        let runs = tempdir();
        assert_eq!(
            format_run_list(&runs, &list_runs(&runs, 20)),
            format!("no runs in {}", runs.display())
        );
        let gone = runs.join("gone");
        assert!(list_runs(&gone, 20).is_empty(), "a missing runs folder is no runs, not an error");
        fs::remove_dir_all(&runs).ok();
    }

    // ----- helpers -----

    fn tempdir() -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "fn_agent_run_test_{}_{}",
            std::process::id(),
            unique_id()
        ));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn unique_id() -> u64 {
        use std::sync::atomic::{AtomicU64, Ordering};
        static NEXT: AtomicU64 = AtomicU64::new(0);
        NEXT.fetch_add(1, Ordering::SeqCst)
    }
}
