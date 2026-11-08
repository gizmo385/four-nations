//! `cargo xtask agent-run [options]`     agent plays headless; watch it here
//! `cargo xtask agent-run --interactive [options]`  chat with the agent here; watch elsewhere
//! `cargo xtask agent-run --chunk-turns N [options]`  play as successive short claude sessions
//! `cargo xtask agent-run --scripts [options]`  play by writing and running scripts (#484)
//! `cargo xtask agent-run watch [--no-build] [RUN_DIR]`  watch (or replay) a run; default: the
//!                                      latest. Builds fn_tui first unless `target/release/fn_tui`
//!                                      is already newer than everything under `crates/`, `data/`
//!                                      and `Cargo.lock`, or `--no-build` is given (#510).
//!
//! Starts an agent run (docs/design/agent-play.md): a Claude Code session plays one game
//! through `fn_agent serve` while you watch it in the normal TUI with `fn_tui --watch`. This is
//! #507's rewrite of the old `scripts/agent-run.sh`, which stays on disk only as a thin wrapper
//! that execs this.
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
//!   --pace N           watch pace to start at: 1, 2, 5, 10, 20 or 40 (default 20)
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
//!   --max-usd N        caps the run's own spend at N dollars (only with --chunk-turns or
//!                      --scripts; #485). Enforced, not just watched.
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
//! repo, so the agent can't read docs/ or data/. The agent gets only fn_agent's tools: that's
//! the "in-game knowledge only" rule (#313), enforced by claude's own `--allowedTools`/
//! `--disallowedTools`, not by the game. In the default mode, quitting the watcher stops the
//! agent too. The agent's own output goes to agent.log in the run folder. A --chunk-turns or
//! --scripts run also gets cost.json (round trips and tokens summed from every chunk's own
//! `claude -p --output-format json`, with a USD total; #485); a --scripts run also gets
//! sdk-gaps.md.
//!
//! Every `claude` call this makes is tagged with OTEL_RESOURCE_ATTRIBUTES
//! (four_nations.run/.seed/.nation/.difficulty/.mode, plus .chunk on chunked runs; #494), kept
//! alongside anything already in the caller's own OTEL_RESOURCE_ATTRIBUTES rather than
//! replacing it.

use std::collections::BTreeMap;
use std::fs::{self, File, OpenOptions};
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::{Duration, SystemTime};

use serde::{Deserialize, Serialize};

use crate::Result;

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
        let mut no_build = false;
        let mut positional = Vec::new();
        for a in &args[1..] {
            if *a == "--no-build" {
                no_build = true;
            } else {
                positional.push(*a);
            }
        }
        if positional.len() > 1 {
            return Err(help());
        }
        let runs = runs_dir();
        let dir = match positional.first() {
            Some(d) => PathBuf::from(d),
            None => latest_run_dir(&runs)?,
        };
        if !no_build {
            build_fn_tui_if_stale(&repo)?;
        }
        return watch_run(&repo, &dir, 20);
    }

    let opts = parse_args(args)?;
    validate(&opts)?;

    if !claude_available(&opts.claude_bin) {
        return Err(format!("{} (Claude Code) isn't on PATH", opts.claude_bin));
    }

    if let Some(line) = start_line(&opts) {
        say(&line);
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
        let _ = Command::new(&opts.claude_bin)
            .args(&claude_args)
            .env("OTEL_RESOURCE_ATTRIBUTES", attrs)
            .status();
        finish_log(&dir, "agent session closed")?;
        return Ok(());
    }

    run_default(&repo, &dir, &opts, &prompt, &claude_args)
}

// -------------------------------------------------------------------------------------------
// Options
// -------------------------------------------------------------------------------------------

#[derive(Debug)]
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
}

/// The nations `--nation` can name, and so the ones a random pick chooses between.
const NATIONS: [&str; 4] = ["earth", "fire", "water", "air"];

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
            "--claude-bin" => o.claude_bin = take_value(args, &mut i, "--claude-bin")?.to_string(),
            "-h" | "--help" => return Err(help()),
            other => return Err(format!("unknown option {other} (see --help)")),
        }
        i += 1;
    }
    // A scenario fixes its own seed and nation, so there's nothing to pick (and the
    // placeholders are never read).
    if o.scenario.is_none() {
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
    if o.max_usd.is_some() && !o.scripts && o.chunk_turns == 0 {
        return Err("--max-usd only applies with --chunk-turns or --scripts".into());
    }
    Ok(())
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
    if let Some(p) = std::env::var_os("FN_AGENT_RUNS") {
        return PathBuf::from(p);
    }
    let home = std::env::var_os("HOME").map(PathBuf::from).unwrap_or_else(|| PathBuf::from("."));
    home.join("fn-agent-runs")
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

fn latest_run_dir(runs: &Path) -> Result<PathBuf> {
    let entries = fs::read_dir(runs).map_err(|e| format!("no runs in {}: {e}", runs.display()))?;
    let mut dirs: Vec<(std::time::SystemTime, PathBuf)> = entries
        .filter_map(|e| e.ok())
        .filter(|e| e.path().is_dir())
        .filter_map(|e| e.metadata().ok().and_then(|m| m.modified().ok()).map(|t| (t, e.path())))
        .collect();
    dirs.sort_by_key(|(t, _)| std::cmp::Reverse(*t));
    dirs.into_iter().next().map(|(_, p)| p).ok_or_else(|| format!("no runs in {}", runs.display()))
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
fn cargo_build_release(repo: &Path, pkgs: &[&str]) -> Result {
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

fn watch_run(repo: &Path, dir: &Path, pace: u32) -> Result {
    let log = dir.join("run.jsonl");
    let mut tries = 0;
    while !(log.is_file() && log.metadata().map(|m| m.len() > 0).unwrap_or(false)) {
        if tries >= 120 {
            return Err(format!(
                "no log at {} yet (did the agent start? see {}/agent.log)",
                log.display(),
                dir.display()
            ));
        }
        thread::sleep(Duration::from_millis(500));
        tries += 1;
    }
    let bin = repo.join("target/release/fn_tui");
    Command::new(bin)
        .arg("--watch")
        .arg(&log)
        .arg("--pace")
        .arg(pace.to_string())
        .status()
        .map_err(|e| e.to_string())?;
    Ok(())
}

fn base_claude_args(mcp_config: &Path, model: Option<&str>) -> Vec<String> {
    let mut args = vec![
        "--strict-mcp-config".to_string(),
        "--mcp-config".to_string(),
        mcp_config.display().to_string(),
        "--allowedTools".to_string(),
        "mcp__four-nations__*".to_string(),
        "--disallowedTools".to_string(),
        "Bash,Read,Edit,Write,Glob,Grep,WebFetch,WebSearch,NotebookEdit,Agent".to_string(),
    ];
    if let Some(m) = model {
        args.push("--model".to_string());
        args.push(m.to_string());
    }
    args
}

/// One `claude -p PROMPT ... --output-format json` call: stdout (the JSON result) goes to
/// `out_json`, stderr is appended to `dir/agent.log`. The exit status is ignored (`|| true` in
/// the old script): a chunk that errors out still gets its (empty) result parsed and the loop
/// moves on.
fn run_claude_p(
    opts: &Opts,
    prompt: &str,
    call_args: &[String],
    mode: &str,
    chunk: u32,
    dir: &Path,
    out_json: &Path,
) -> Result {
    let stdout_file =
        File::create(out_json).map_err(|e| format!("creating {}: {e}", out_json.display()))?;
    let stderr_file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(dir.join("agent.log"))
        .map_err(|e| e.to_string())?;
    let attrs = otel_attrs(opts, dir, mode, Some(chunk));
    let _ = Command::new(&opts.claude_bin)
        .arg("-p")
        .arg(prompt)
        .args(call_args)
        .args(["--output-format", "json"])
        .env("OTEL_RESOURCE_ATTRIBUTES", attrs)
        .stdout(stdout_file)
        .stderr(stderr_file)
        .status();
    Ok(())
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
    let mut child = Command::new(&opts.claude_bin)
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
    let killing_for_thread = killing.clone();
    let alive_for_thread = alive.clone();
    let handle = thread::spawn(move || {
        let _ = child.wait();
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
    let watch_result = watch_run(repo, dir, opts.pace);

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
    args
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
                "command": command.display().to_string(),
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

/// `--scripts`' own first prompt (docs/design/agent-scripting.md, "The trampoline").
fn default_script_prompt(total: u32, calls: u32) -> String {
    format!(
        "You're playing The Four Nations, a colony game, by writing and running scripts through \
         the four-nations tools, rather than acting turn by turn. Call briefing first, then \
         observe. Save a short, focused script with save_script for one routine job, then run \
         it with run_script for just 3-5 turns to see what it does. Once a script proves \
         itself, run it for a season, and once that goes well, for a year. Keep everyone alive \
         and grow the settlement, toward about {total} turns in all. After every run_script \
         call, use notes to record what happened, and a line starting \"SDK gap:\" for anything \
         you wished the tools could do. This session is one chunk of that: after roughly {calls} \
         tool calls, stop and summarise how it went so the next session can carry on."
    )
}

/// A resumed `--scripts` chunk's own prompt (#341's rule in script-mode terms, #484's acceptance
/// criteria: the library and notes rather than a prose report).
fn scripts_handoff_prompt(total: u32, done_turns: u32, calls: u32, listing: &str) -> String {
    format!(
        "You're continuing the same settlement from where you left off (turn {done_turns} of \
         about {total} so far), not starting a new one. Here is your own script library and lab \
         notes:\n\n{listing}\n\nCarry on the same way: run or improve a script with run_script \
         and save_script, record outcomes and any SDK gap in notes, and let time pass. After \
         roughly {calls} more tool calls, stop and summarise how it went."
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
// The run log (run.jsonl): a minimal parallel of fn_tui::surface::LogLine, read-only, just the
// variants this needs (`#[serde(other)]` swallows Header/Keys/Look/Inspect/Call untouched).
// Depending on fn_tui for the real type would pull in fn_sim, ratatui and crossterm just to
// parse five JSON shapes; copying them here is cheaper and keeps xtask's own build light.
// -------------------------------------------------------------------------------------------

#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum LogLine {
    Turn {
        index: u32,
        tick: u64,
    },
    End,
    Script {
        name: String,
        version: u32,
        code: String,
        turns: u32,
    },
    Wake {
        reason: String,
    },
    Note {
        text: String,
    },
    Plan {
        plan: serde_json::Value,
    },
    #[serde(other)]
    Other,
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
fn finish_log(dir: &Path, reason: &str) -> Result {
    thread::sleep(Duration::from_secs(1));
    let log = dir.join("run.jsonl");
    let lines = match read_log_lines(&log) {
        Ok(l) => l,
        Err(_) => return Ok(()),
    };
    if lines.is_empty() || matches!(lines.last(), Some(LogLine::End)) {
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

#[derive(Debug, Default, Clone, PartialEq, Serialize, Deserialize)]
struct ScriptEntry {
    version: u32,
    code: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    last_outcome: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    last_turns_run: Option<u32>,
}

#[derive(Debug, Default, Serialize, Deserialize)]
struct Handoff {
    #[serde(default)]
    scripts: BTreeMap<String, ScriptEntry>,
    #[serde(default)]
    notes: Vec<String>,
    /// The `plan` tool's latest map (#524); absent when none was ever set.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    plan: Option<serde_json::Value>,
}

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
            LogLine::Script { name, version, code, turns } => {
                if *version >= 1 {
                    let e = scripts.entry(name.clone()).or_default();
                    e.version = *version;
                    e.code = code.clone();
                }
                pending = if *version >= 1 && *turns >= 1 { Some(name.clone()) } else { None };
                turns_run = 0;
            }
            LogLine::Turn { .. } => turns_run += 1,
            LogLine::Wake { reason } => {
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
    let (scripts, notes, _plan) = parse_library(&lines);
    let mut out = Vec::new();
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
// Cost tracking (#485)
// -------------------------------------------------------------------------------------------

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
/// about, same leniency as the old script's `try/except` (a missing or unparsable file reads as
/// all zeros, never an error).
#[derive(Debug, Default, Clone, Deserialize)]
struct ClaudeResult {
    #[serde(default)]
    num_turns: u64,
    #[serde(default)]
    usage: Usage,
    #[serde(default)]
    total_cost_usd: f64,
    #[serde(default)]
    result: String,
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
}

/// Reads and parses `path` as JSON, or `T::default()` if it's missing or isn't valid JSON --
/// the same leniency the old script's Python `try/except (FileNotFoundError, JSONDecodeError)`
/// had.
fn read_json_or_default<T: Default + for<'de> Deserialize<'de>>(path: &Path) -> T {
    fs::read_to_string(path).ok().and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_default()
}

/// Adds one chunk's own `claude -p --output-format json` result (read from `json_path`) into
/// `dir/cost.json`'s running total: round trips (`num_turns`), input/cache/output tokens, and
/// USD. Updated after every chunk, not just at the end, so an interrupted run still reports what
/// it spent so far.
fn update_cost(dir: &Path, json_path: &Path) -> Result {
    let data: ClaudeResult = read_json_or_default(json_path);
    let cost_path = dir.join("cost.json");
    let mut totals: CostTotals = read_json_or_default(&cost_path);
    totals.round_trips += data.num_turns;
    totals.input_tokens += data.usage.input_tokens;
    totals.cache_creation_input_tokens += data.usage.cache_creation_input_tokens;
    totals.cache_read_input_tokens += data.usage.cache_read_input_tokens;
    totals.output_tokens += data.usage.output_tokens;
    totals.usd += data.total_cost_usd;
    let text = serde_json::to_string_pretty(&totals).map_err(|e| e.to_string())? + "\n";
    fs::write(&cost_path, text).map_err(|e| format!("writing {}: {e}", cost_path.display()))
}

/// Extracts a `claude -p --output-format json` call's own `result` text (its final report).
/// Empty (not an error) with no result yet, or if `claude` never wrote valid JSON.
fn chunk_result(json_path: &Path) -> String {
    read_json_or_default::<ClaudeResult>(json_path).result
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

// -------------------------------------------------------------------------------------------
// Chunked loops
// -------------------------------------------------------------------------------------------

/// `--chunk-turns N`: plays `opts.turns` as successive `claude -p` sessions of at most N turns
/// each, every one but the first resuming `run.jsonl` (`fn_agent serve --resume`, #341) instead
/// of starting a fresh game, so no single session's context grows past one chunk's worth.
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
    loop {
        let done_turns = turns_done(dir)?;
        if done_turns >= target {
            say(&format!("reached turn {done_turns} of {target} turns; done"));
            break;
        }
        if n > max_chunks {
            say(&format!(
                "giving up after {max_chunks} chunks (only reached turn {done_turns} of \
                 {target}); see {}/agent.log",
                dir.display()
            ));
            break;
        }
        if let Some(max_usd) = o.max_usd {
            let spent = cost_so_far(dir);
            if spent >= max_usd {
                say(&format!(
                    "stopping: hit the ${max_usd} cap (${spent} spent so far, turn {done_turns} \
                     of {target})"
                ));
                break;
            }
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

        let mut call_args = claude_args.to_vec();
        if let Some(max_usd) = o.max_usd {
            call_args.push("--max-budget-usd".to_string());
            call_args.push(budget_remaining(dir, max_usd).to_string());
        }

        say(&format!(
            "chunk {n}: turns {}-{} of {target}",
            done_turns + 1,
            done_turns + this_chunk
        ));
        let chunk_json = dir.join(format!("chunk-{n}.json"));
        run_claude_p(o, &this_prompt, &call_args, "mcp-chunked", n, dir, &chunk_json)?;
        let result = chunk_result(&chunk_json);
        report = if result.is_empty() {
            format!(
                "(this chunk's session ended without a report; see {}/agent.log)",
                dir.display()
            )
        } else {
            result
        };
        append_log(&dir.join("agent.log"), &format!("\n--- chunk {n} report ---\n{report}\n"))?;
        update_cost(dir, &chunk_json)?;
        finish_log(dir, &format!("chunk {n} finished"))?;
        n += 1;
    }
    let _ = repo;
    Ok(())
}

/// `--scripts`: plays `opts.turns` as successive `claude -p` sessions chunked by round trips
/// rather than game turns (`--chunk-calls`, #484, docs/design/agent-scripting.md). Every chunk
/// but the first resumes `run.jsonl` (`fn_agent serve --scripts --resume`), which restores the
/// real script library and memory on its own.
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
    // headroom.
    let max_chunks = target + 10;
    say(&format!(
        "scripted run: {target} turns total, about {calls} tool calls per chunk, log at \
         {}/run.jsonl",
        dir.display()
    ));
    let mcp_config = dir.join("mcp.json");
    let log = dir.join("run.jsonl");
    loop {
        let done_turns = turns_done(dir)?;
        if done_turns >= target {
            say(&format!("reached turn {done_turns} of {target} turns; done"));
            break;
        }
        if n > max_chunks {
            say(&format!(
                "giving up after {max_chunks} chunks (only reached turn {done_turns} of \
                 {target}); see {}/agent.log",
                dir.display()
            ));
            break;
        }
        if let Some(max_usd) = o.max_usd {
            let spent = cost_so_far(dir);
            if spent >= max_usd {
                say(&format!(
                    "stopping: hit the ${max_usd} cap (${spent} spent so far, turn {done_turns} \
                     of {target})"
                ));
                break;
            }
        }

        if n == 1 {
            write_mcp_config(&mcp_config, agent_bin, &serve_args(o, &log))?;
        } else {
            write_mcp_config(&mcp_config, agent_bin, &resume_args(&log, true))?;
        }

        let this_prompt = if let Some(p) = &o.prompt {
            p.clone()
        } else if n == 1 {
            let base = default_script_prompt(target, calls);
            match &o.carry_dir {
                Some(carry_dir) => match carry_block(carry_dir)? {
                    Some(block) => format!("{block}\n\n{base}"),
                    None => base,
                },
                None => base,
            }
        } else {
            let listing = library_notes_listing(&log)?;
            scripts_handoff_prompt(target, done_turns, calls, &listing)
        };

        let mut call_args = claude_args.to_vec();
        if let Some(max_usd) = o.max_usd {
            call_args.push("--max-budget-usd".to_string());
            call_args.push(budget_remaining(dir, max_usd).to_string());
        }

        say(&format!(
            "chunk {n}: about {calls} tool calls, past turn {done_turns} of {target} so far"
        ));
        let chunk_json = dir.join(format!("chunk-{n}.json"));
        run_claude_p(o, &this_prompt, &call_args, "scripts", n, dir, &chunk_json)?;
        let result = chunk_result(&chunk_json);
        let result = if result.is_empty() {
            format!(
                "(this chunk's session ended without a report; see {}/agent.log)",
                dir.display()
            )
        } else {
            result
        };
        append_log(&dir.join("agent.log"), &format!("\n--- chunk {n} report ---\n{result}\n"))?;
        update_cost(dir, &chunk_json)?;
        finish_log(dir, &format!("chunk {n} finished"))?;
        n += 1;
    }
    write_sdk_gaps(dir)?;
    if let Some(carry_dir) = &o.carry_dir {
        carry_save(&log, carry_dir)?;
    }
    let _ = repo;
    Ok(())
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
        let o = parse_args_with(&["--scenario", "earth-year-two"], &mut counting).unwrap();
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
        let o = parse_args(&["--scenario", "earth-year-two"]).unwrap();
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
        assert!(validate(&parse_args(&["--carry", "/tmp/x", "--scripts"]).unwrap()).is_ok());
    }

    #[test]
    fn max_usd_requires_a_plain_number_and_a_chunked_mode() {
        assert!(parse_args(&["--max-usd", "abc"]).is_err());
        assert!(parse_args(&["--max-usd", "-1"]).is_err());
        assert!(validate(&parse_args(&["--max-usd", "5"]).unwrap()).is_err());
        assert!(validate(&parse_args(&["--max-usd", "5", "--scripts"]).unwrap()).is_ok());
        assert!(
            validate(&parse_args(&["--max-usd", "5.50", "--chunk-turns", "3"]).unwrap()).is_ok()
        );
    }

    #[test]
    fn help_flag_prints_usage_text() {
        let err = parse_args(&["--help"]).unwrap_err();
        assert!(err.contains("cargo xtask agent-run"));
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
        let o = parse_args(&["--scenario", "earth-year-two"]).unwrap();
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
        finish_log(&dir, "agent finished").unwrap();
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
        finish_log(&dir, "agent finished").unwrap();
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
        finish_log(&dir, "agent finished").unwrap();
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
        finish_log(&dir, "agent finished").unwrap();
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
        fs::remove_dir_all(&dir).ok();
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

    #[test]
    fn update_cost_sums_across_chunks() {
        let dir = tempdir();
        let chunk1 = dir.join("chunk-1.json");
        fs::write(
            &chunk1,
            r#"{"num_turns":3,"usage":{"input_tokens":100,"output_tokens":20},"total_cost_usd":0.05,"result":"ok"}"#,
        )
        .unwrap();
        update_cost(&dir, &chunk1).unwrap();
        let chunk2 = dir.join("chunk-2.json");
        fs::write(
            &chunk2,
            r#"{"num_turns":2,"usage":{"input_tokens":50,"output_tokens":10},"total_cost_usd":0.03,"result":"ok again"}"#,
        )
        .unwrap();
        update_cost(&dir, &chunk2).unwrap();
        let totals: CostTotals = read_json_or_default(&dir.join("cost.json"));
        assert_eq!(totals.round_trips, 5);
        assert_eq!(totals.input_tokens, 150);
        assert_eq!(totals.output_tokens, 30);
        assert!((totals.usd - 0.08).abs() < 1e-9);
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
    fn chunk_result_reads_the_result_field_or_is_empty_without_one() {
        let dir = tempdir();
        let path = dir.join("chunk-1.json");
        fs::write(&path, r#"{"result":"all good"}"#).unwrap();
        assert_eq!(chunk_result(&path), "all good");
        fs::write(&path, "not json").unwrap();
        assert_eq!(chunk_result(&path), "");
        assert_eq!(chunk_result(&dir.join("missing.json")), "");
        fs::remove_dir_all(&dir).ok();
    }

    // ----- fn_agent CLI arguments and mcp.json -----

    #[test]
    fn serve_args_use_scenario_or_seed_nation_difficulty() {
        let o = parse_args(&["--scenario", "earth-year-two"]).unwrap();
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
