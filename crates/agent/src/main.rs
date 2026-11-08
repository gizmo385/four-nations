//! The CLI: `fn_agent serve ...` and `fn_agent keys LOG`. See the crate's own doc comment
//! (`fn_agent::...`) for the protocol and the sandboxing recipe.

use std::io::{BufReader, Write, stdin, stdout};
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use fn_agent::runlog::Writer;
use fn_agent::scorecard;
use fn_agent::script::{self, Limits as ScriptLimits};
use fn_agent::server;
use fn_agent::tools::Tools;
use fn_agent::trial::Trial;
use fn_tui::surface::{
    DEFAULT_TURN_SECONDS, EventKind, Host, LogLine, Prelude, PreludeOpening, RunLogHeader,
};
use fn_tui::{scenario, watch};
use signal_hook::consts::{SIGHUP, SIGINT, SIGTERM};
use signal_hook::iterator::Signals;

const USAGE: &str = "\
usage: fn_agent serve --seed N --nation X [--start default|X,Y] [--difficulty ID]
                       [--map-size ID] [--prelude OPENING:MINUTES] [--turn SECS] [--scripts]
                       [--carry-in FILE] [--trial-seeds LIST --trial-turns N] --log PATH
       fn_agent serve --scenario ID [--turn SECS] [--scripts] --log PATH
       fn_agent serve --resume LOG [--seed N] [--nation X] [--start default|X,Y]
                       [--difficulty ID] [--turn SECS] [--scripts]
                       [--trial-seeds LIST --trial-turns N]
       fn_agent play --script FILE [--scenario ID | --seed N --nation X
                      [--start default|X,Y] [--difficulty ID]] --turns N [--turn SECS]
                      [--wake-on LIST] [--plan FILE] --log PATH
       fn_agent play --script FILE [--plan FILE] --horizon TURNS [start flags as above]
                      [--map-size ID] [--prelude OPENING:MINUTES] [--turn SECS] --log PATH
       fn_agent keys LOG
       fn_agent score LOG [--horizon TURNS] [--json] [--append]

serve speaks MCP (JSON-RPC 2.0) over stdin and stdout: initialize, tools/list and tools/call. It
writes --log PATH as it goes; watch it live (or replay it after) with
    fn_tui --watch PATH
in another terminal.

--scenario ID (docs/design/agent-scripting.md, \"Scenario starts\") starts a data/scenarios.toml
preset instead of --seed/--nation/--start/--difficulty, run through its own prelude first if it
names one -- the same world fn_tui --scenario ID starts. Can't be combined with --seed, --nation,
--start, --difficulty or --resume.

--resume LOG rebuilds the session LOG's own header describes, replays LOG's keys and turns back
into it (so the resumed session picks up exactly where the last one left off), and keeps
appending to LOG from there -- for playing a long run as a chain of short, cheap sessions instead
of one that re-reads a growing log every turn. Any of --seed, --nation, --start, --difficulty or
--turn given alongside --resume must match LOG's own header, or serve refuses to start.

--carry-in FILE (docs/design/agent-hillclimb.md, \"The attempt loop\"; needs --scripts) starts a
fresh session with a strategy already loaded: FILE is a handoff (JSON with scripts, notes and
plan, as `cargo xtask agent-run --carry` writes), written to the log as script, note and plan
lines before the first call, so the library, lab notes and plan are in place with no retyping.
--map-size and --prelude take what `play` takes, so a track's start can be played as well as
evaluated. None of the three combine with --resume or --scenario.

--trial-seeds LIST --trial-turns N (#561; needs --scripts, and works with --resume) adds the
test_governor tool: a clean test of the saved governor and the plan from turn 0, on a copy of the
world of its own, on one of the LIST seeds (comma-separated; the harness passes a track's train
seeds and nothing else) for up to N turns. It reports the stop reason, turn and score the clean
evaluation (play --horizon) would, and is limited to a few calls per attempt.

--scripts (docs/design/agent-scripting.md) adds the run_script tool: a sandboxed Rhai runner
over the same Host, with limits from data/agent.toml's [scripts] table. Works with --scenario
too: the runner sees whatever world the prelude left, the same as any other start.

play (docs/design/agent-scripting.md, \"Headless play\") runs FILE's own code against a fresh
Host through the same plumbing and porcelain run_script uses, with no model: one run_script call
as a batch job. It stops at the first wake (an error, wake(msg), or a --wake-on event during a
wait) or once --turns runs out, writes --log PATH the same way serve does, and prints the wake
report as JSON on stdout. It exits 0 for done or budget, nonzero for anything else (error,
runaway, asked or event), so a shell loop (scripts/agent-run.sh, say) can tell which runs need a
look. --wake-on is a comma-separated list from death, arrival, tech, endeavor, season, stockpile,
bend_failed, need (data/agent.toml's own default_wake with none given). --plan FILE (a JSON
object, within data/agent.toml's plan_bytes) is the plan that plan() returns, as the plan tool would
have set it.

play --horizon TURNS (docs/design/agent-hillclimb.md, \"Across runs: attempts and a champion\") is
the clean evaluation of a strategy (a track, data/tracks.toml, is read by `cargo xtask agent-eval`,
which passes its start here; --map-size small, medium or large goes with --start X,Y, and --prelude
economy:60 or opening:60 runs that autoplay opening for that many game minutes before turn 0, as a
scenario's prelude does): instead of stopping at the first wake it runs to exactly TURNS
turns, with no model. Event wakes are ignored. When the script returns or calls wake() it runs again
with its memory kept (one end_turn is forced first if the run used no turn, so it can't spin). After
an error or a runaway the remaining turns run with no orders. The log has a wake line per stop. The
JSON on stdout is the last run's report plus restarts, stopped_by (the first error or runaway, or
null) and scorecard (what `fn_agent score LOG --horizon TURNS --json` prints for the log). It exits
0 unless a script stopped_by an error or a runaway. Can't be combined with --turns or --wake-on.

score replays LOG up to --horizon TURNS (the log's own last turn by default; it stops there even if
the log goes on) and prints its scorecard (docs/design/agent-hillclimb.md, \"The scorecard\"): the
score, progress by milestone, care, and diagnostics (idle share, research, script churn). --json
prints the same fields as JSON. --append adds a `score` line to LOG, which replay, keys, --resume
and --watch all ignore. A log made against different data is refused.

keys prints the run at LOG as a --keys script, as a full fn_tui command line: --scenario ID when
the log's own header matches a bundled preset, else SEED NATION --start ... --difficulty ....";

fn main() -> ExitCode {
    let mut args = std::env::args().skip(1);
    let result = match args.next().as_deref() {
        Some("serve") => serve(args.collect()).map(|()| ExitCode::SUCCESS),
        Some("keys") => keys(args.collect()).map(|()| ExitCode::SUCCESS),
        Some("play") => play(args.collect()),
        Some("score") => score(args.collect()).map(|()| ExitCode::SUCCESS),
        _ => Err(USAGE.to_owned()),
    };
    match result {
        Ok(code) => code,
        Err(e) => {
            eprintln!("fn_agent: {e}");
            ExitCode::FAILURE
        }
    }
}

#[derive(Debug)]
struct ServeArgs {
    seed: Option<u64>,
    nation: Option<String>,
    /// Whether `--start` was given at all, so a resumed session only checks it against the
    /// log's own header when the caller actually asked for a particular one.
    start_given: bool,
    start: Option<(i32, i32)>,
    difficulty: Option<String>,
    /// `--map-size ID` (#530): as `play --map-size`, read with `--start X,Y` only.
    map_size: Option<String>,
    /// `--prelude OPENING:MINUTES` (#530): as `play --prelude`.
    prelude: Option<Prelude>,
    /// `--carry-in FILE` (#530): a handoff to preload (`fn_agent::carry`).
    carry_in: Option<String>,
    /// `--scenario ID` (#481): a `data/scenarios.toml` preset's id, in place of `--seed`,
    /// `--nation`, `--start` and `--difficulty`.
    scenario: Option<String>,
    turn_seconds: Option<u32>,
    log: Option<String>,
    resume: Option<String>,
    scripts: bool,
    /// `--trial-seeds LIST` and `--trial-turns N` (#561): `test_governor`'s train seeds and
    /// horizon; given together or not at all.
    trial_seeds: Option<Vec<u64>>,
    trial_turns: Option<u32>,
}

fn parse_serve_args(args: Vec<String>) -> Result<ServeArgs, String> {
    let mut a = ServeArgs {
        seed: None,
        nation: None,
        start_given: false,
        start: None,
        difficulty: None,
        map_size: None,
        prelude: None,
        carry_in: None,
        scenario: None,
        turn_seconds: None,
        log: None,
        resume: None,
        scripts: false,
        trial_seeds: None,
        trial_turns: None,
    };
    let mut it = args.into_iter();
    while let Some(flag) = it.next() {
        let mut value = || it.next().ok_or_else(|| format!("{flag} needs a value"));
        match flag.as_str() {
            "--seed" => a.seed = Some(value()?.parse::<u64>().map_err(|e| e.to_string())?),
            "--nation" => a.nation = Some(value()?),
            "--start" => {
                let v = value()?;
                a.start_given = true;
                a.start = match v.as_str() {
                    "default" => None,
                    other => Some(parse_pos(other)?),
                };
            }
            "--difficulty" => a.difficulty = Some(value()?),
            "--map-size" => a.map_size = Some(value()?),
            "--prelude" => a.prelude = Some(parse_prelude(&value()?)?),
            "--carry-in" => a.carry_in = Some(value()?),
            "--scenario" => a.scenario = Some(value()?),
            "--turn" => a.turn_seconds = Some(value()?.parse::<u32>().map_err(|e| e.to_string())?),
            "--log" => a.log = Some(value()?),
            "--resume" => a.resume = Some(value()?),
            "--scripts" => a.scripts = true,
            "--trial-seeds" => {
                let v = value()?;
                let seeds: Result<Vec<u64>, _> = v.split(',').map(|s| s.trim().parse()).collect();
                a.trial_seeds =
                    Some(seeds.map_err(|e| {
                        format!("--trial-seeds wants seeds like 7,42, got {v:?}: {e}")
                    })?);
            }
            "--trial-turns" => {
                a.trial_turns = Some(
                    value()?
                        .parse::<u32>()
                        .map_err(|e| format!("--trial-turns wants a turn count: {e}"))?,
                );
            }
            other => return Err(format!("unknown flag {other:?}")),
        }
    }
    if a.trial_seeds.is_some() != a.trial_turns.is_some() {
        return Err("--trial-seeds and --trial-turns go together".to_owned());
    }
    if a.trial_seeds.is_some() && !a.scripts {
        return Err("--trial-seeds needs --scripts: it tests the saved governor".to_owned());
    }
    if a.scenario.is_some()
        && (a.seed.is_some()
            || a.nation.is_some()
            || a.start_given
            || a.difficulty.is_some()
            || a.resume.is_some())
    {
        return Err("--scenario can't be combined with --seed, --nation, --start, \
                     --difficulty or --resume: the preset already sets them"
            .to_owned());
    }
    let fresh_only = [
        ("--map-size", a.map_size.is_some()),
        ("--prelude", a.prelude.is_some()),
        ("--carry-in", a.carry_in.is_some()),
    ];
    for (flag, given) in fresh_only {
        if given && (a.scenario.is_some() || a.resume.is_some()) {
            return Err(format!(
                "{flag} starts a fresh session of its own: it can't be combined with --scenario \
                 or --resume"
            ));
        }
    }
    if a.carry_in.is_some() && !a.scripts {
        return Err("--carry-in needs --scripts: it preloads the script library".to_owned());
    }
    Ok(a)
}

/// `X,Y`: a tile position, for `--start`.
fn parse_pos(s: &str) -> Result<(i32, i32), String> {
    let (x, y) =
        s.split_once(',').ok_or_else(|| format!("--start wants default or X,Y, got {s:?}"))?;
    let (x, y) = (x.trim().parse().ok(), y.trim().parse().ok());
    match (x, y) {
        (Some(x), Some(y)) => Ok((x, y)),
        _ => Err(format!("--start wants default or X,Y, got {s:?}")),
    }
}

fn serve(args: Vec<String>) -> Result<(), String> {
    let a = parse_serve_args(args)?;
    match a.resume.clone() {
        Some(path) => serve_resume(&a, &path),
        None => serve_fresh(&a),
    }
}

fn serve_fresh(a: &ServeArgs) -> Result<(), String> {
    let log = a.log.as_deref().ok_or("--log is required")?;
    let host = match &a.scenario {
        Some(id) => Host::from_scenario(id)?,
        None => {
            let nation_str = a.nation.as_deref().ok_or("--nation is required")?;
            let nation = fn_tui::cli::parse_nation(nation_str).ok_or_else(|| {
                format!("unknown nation {nation_str:?} (air, water, earth or fire)")
            })?;
            let seed = a.seed.ok_or("--seed is required")?;
            let host =
                Host::new(seed, nation, a.map_size.clone(), a.difficulty.clone(), a.start, false)?;
            match a.prelude {
                Some(prelude) => host.with_prelude(prelude),
                None => host,
            }
        }
    };
    let host = host.with_turn_seconds(a.turn_seconds.unwrap_or(DEFAULT_TURN_SECONDS));
    let writer = Writer::create(Path::new(log)).map_err(|e| e.to_string())?;
    let mut tools = with_scripts(Tools::new(host, writer)?, a)?;
    if let Some(path) = &a.carry_in {
        let handoff = std::fs::read_to_string(path)
            .map_err(|e| format!("can't read --carry-in {path:?}: {e}"))?;
        tools.carry_in(&handoff)?;
    }
    run_tools(tools, Path::new(log))
}

/// Turns `run_script` on when `--scripts` was given (`Tools::with_scripts`), with the limits
/// `data/agent.toml` ships, and `test_governor` with it when `--trial-seeds` was (#561).
fn with_scripts<W: Write>(tools: Tools<W>, a: &ServeArgs) -> Result<Tools<W>, String> {
    if !a.scripts {
        return Ok(tools);
    }
    let tools = tools.with_scripts(ScriptLimits::bundled());
    match (&a.trial_seeds, a.trial_turns) {
        (Some(seeds), Some(turns)) => Ok(tools.with_trial(Trial::new(seeds.clone(), turns)?)),
        _ => Ok(tools),
    }
}

/// `--resume LOG`: rebuilds `LOG`'s own session (`Host::resume`) from its header and lines,
/// refuses to start if any flag given alongside `--resume` disagrees with that header
/// (`check_resume_args`), then keeps appending to the same file (`Writer::append`) -- so a run
/// can be played as a chain of short sessions, and `fn_tui --watch`/`fn_agent keys` still read
/// it as one continuous run (#341). The log is append-only (#511): an `end` line the last session
/// left stays where it is, and means "a session ended" (only a log's *last* line being `end`
/// means the run did), because rewriting the file under a live `fn_tui --watch` desynced its tail.
fn serve_resume(a: &ServeArgs, path: &str) -> Result<(), String> {
    if let Some(log) = &a.log
        && log != path
    {
        return Err(format!(
            "--log {log:?} doesn't match --resume {path:?}; a resumed session appends to the \
             log it resumes, not a different one"
        ));
    }
    let (header, lines) = watch::read_file(Path::new(path))?;
    check_resume_args(&header, a)?;
    let host = Host::resume(&header, &lines)?;
    let writer = Writer::append(Path::new(path)).map_err(|e| e.to_string())?;
    let tools = with_scripts(Tools::resume(host, writer, &lines), a)?;
    run_tools(tools, Path::new(path))
}

/// Refuses `a` if any of `--seed`, `--nation`, `--difficulty`, `--turn` or `--start` it was
/// actually given disagrees with `header` -- the resumed session always uses `header`'s own
/// values (`Host::resume`), so a mismatched flag is a caller's mistake worth catching rather
/// than silently ignoring.
fn check_resume_args(header: &RunLogHeader, a: &ServeArgs) -> Result<(), String> {
    if let Some(seed) = a.seed
        && seed != header.seed
    {
        return Err(format!("--seed {seed} doesn't match the log's own seed {}", header.seed));
    }
    if let Some(nation_str) = &a.nation {
        let nation = fn_tui::cli::parse_nation(nation_str)
            .ok_or_else(|| format!("unknown nation {nation_str:?} (air, water, earth or fire)"))?;
        if nation != header.nation {
            return Err(format!(
                "--nation {nation_str:?} doesn't match the log's own nation {:?}",
                header.nation.as_str()
            ));
        }
    }
    if let Some(difficulty) = &a.difficulty
        && Some(difficulty) != header.difficulty.as_ref()
    {
        return Err(format!(
            "--difficulty {difficulty:?} doesn't match the log's own difficulty {:?}",
            header.difficulty
        ));
    }
    if let Some(turn_seconds) = a.turn_seconds
        && turn_seconds != header.turn_seconds
    {
        return Err(format!(
            "--turn {turn_seconds} doesn't match the log's own turn length {}",
            header.turn_seconds
        ));
    }
    if a.start_given && a.start != header.site {
        return Err(format!("--start doesn't match the log's own start {:?}", header.site));
    }
    Ok(())
}

fn run_tools<W: Write>(mut tools: Tools<W>, log_path: &Path) -> Result<(), String> {
    install_signal_end_handler(log_path.to_path_buf());
    let input = BufReader::new(stdin());
    let reason = server::serve(input, stdout(), &mut tools);
    tools.end(&reason)?;
    Ok(())
}

/// #329: Claude Code (and anything else driving `fn_agent serve` as an MCP server) stops it with
/// a signal rather than closing stdin, so [`server::serve`]'s own loop -- blocked reading the
/// next request -- never gets a chance to return and let [`run_tools`] write the run log's `end`
/// line itself. This spawns a background thread that waits for SIGTERM, SIGINT or SIGHUP and,
/// the moment one arrives, writes that line to `log_path` directly ([`write_end_for_signal`])
/// and exits the whole process (there is nothing left worth returning to: the main thread is
/// stuck in a blocking read that a signal alone won't interrupt). `scripts/agent-run.sh`'s own
/// `finish_log` fallback (b134fc7) stays for a log an older build left without one, or one this
/// handler couldn't flush in time, but normally has nothing left to do once this fires first.
///
/// Best-effort: if the signal handler itself can't be installed (an unsupported platform, say),
/// this does nothing and `fn_agent serve` behaves exactly as it did before #329 -- the shell
/// fallback is still there.
fn install_signal_end_handler(log_path: PathBuf) {
    let Ok(mut signals) = Signals::new([SIGTERM, SIGINT, SIGHUP]) else { return };
    std::thread::spawn(move || {
        if let Some(signal) = signals.forever().next() {
            let _ = write_end_for_signal(&log_path, &signal_name(signal));
            std::process::exit(128 + signal);
        }
    });
}

/// SIGTERM, SIGINT or SIGHUP's own name, for the run log's `end` line (`write_end_for_signal`)
/// -- the three [`install_signal_end_handler`] watches for; anything else (not reachable through
/// it today) falls back to the raw number.
fn signal_name(signal: i32) -> String {
    match signal {
        SIGTERM => "SIGTERM".to_owned(),
        SIGINT => "SIGINT".to_owned(),
        SIGHUP => "SIGHUP".to_owned(),
        other => format!("signal {other}"),
    }
}

/// Appends `log_path`'s own `end` line (#327's shape) naming `signal`, with the world's own
/// tick as of the last `turn` line already in the log (0 if there isn't one yet) -- the same
/// tick `scripts/agent-run.sh`'s `finish_log` fallback computes, since neither it nor this can
/// reach the live `Host` a signal interrupted mid-read. Does nothing if `log_path` already ends
/// with an `end` line (a race with `run_tools`'s own clean shutdown, or a second signal), so a
/// run never ends up with two.
fn write_end_for_signal(log_path: &Path, signal: &str) -> Result<(), String> {
    let (_, lines) = watch::read_file(log_path)?;
    if matches!(lines.last(), Some(LogLine::End { .. })) {
        return Ok(());
    }
    let tick = lines
        .iter()
        .rev()
        .find_map(|l| match l {
            LogLine::Turn { tick, .. } => Some(*tick),
            _ => None,
        })
        .unwrap_or(0);
    let mut writer = Writer::append(log_path).map_err(|e| e.to_string())?;
    writer.write_end(tick, &format!("stopped by {signal}")).map_err(|e| e.to_string())
}

// -------------------------------------------------------------------------------------------
// play (#483, docs/design/agent-scripting.md, "Headless play")
// -------------------------------------------------------------------------------------------

#[derive(Debug)]
struct PlayArgs {
    script: Option<String>,
    seed: Option<u64>,
    nation: Option<String>,
    start_given: bool,
    start: Option<(i32, i32)>,
    /// `--map-size ID` (#529): a `data/embark.toml` map size, read with `--start X,Y`.
    map_size: Option<String>,
    /// `--prelude OPENING:MINUTES` (#529): a scenario-style autoplay opening before turn 0.
    prelude: Option<Prelude>,
    difficulty: Option<String>,
    scenario: Option<String>,
    turn_seconds: Option<u32>,
    turns: Option<u32>,
    /// `--horizon TURNS` (#528): the clean evaluation, run to exactly this many turns.
    horizon: Option<u32>,
    /// `--plan FILE` (#528): the plan `plan()` returns.
    plan: Option<String>,
    wake_on: Option<Vec<EventKind>>,
    log: Option<String>,
}

fn parse_play_args(args: Vec<String>) -> Result<PlayArgs, String> {
    let mut a = PlayArgs {
        script: None,
        seed: None,
        nation: None,
        start_given: false,
        start: None,
        map_size: None,
        prelude: None,
        difficulty: None,
        scenario: None,
        turn_seconds: None,
        turns: None,
        horizon: None,
        plan: None,
        wake_on: None,
        log: None,
    };
    let mut it = args.into_iter();
    while let Some(flag) = it.next() {
        let mut value = || it.next().ok_or_else(|| format!("{flag} needs a value"));
        match flag.as_str() {
            "--script" => a.script = Some(value()?),
            "--seed" => a.seed = Some(value()?.parse::<u64>().map_err(|e| e.to_string())?),
            "--nation" => a.nation = Some(value()?),
            "--start" => {
                let v = value()?;
                a.start_given = true;
                a.start = match v.as_str() {
                    "default" => None,
                    other => Some(parse_pos(other)?),
                };
            }
            "--difficulty" => a.difficulty = Some(value()?),
            "--map-size" => a.map_size = Some(value()?),
            "--prelude" => a.prelude = Some(parse_prelude(&value()?)?),
            "--scenario" => a.scenario = Some(value()?),
            "--turn" => a.turn_seconds = Some(value()?.parse::<u32>().map_err(|e| e.to_string())?),
            "--turns" => a.turns = Some(value()?.parse::<u32>().map_err(|e| e.to_string())?),
            "--horizon" => {
                let v = value()?;
                a.horizon = Some(
                    v.parse::<u32>().map_err(|e| format!("--horizon wants a turn count: {e}"))?,
                );
            }
            "--plan" => a.plan = Some(value()?),
            "--wake-on" => {
                let v = value()?;
                let kinds: Result<Vec<EventKind>, String> =
                    v.split(',').map(|s| parse_event_kind(s.trim())).collect();
                a.wake_on = Some(kinds?);
            }
            "--log" => a.log = Some(value()?),
            other => return Err(format!("unknown flag {other:?}")),
        }
    }
    if a.horizon.is_some() && (a.turns.is_some() || a.wake_on.is_some()) {
        return Err("--horizon runs to a fixed turn and ignores event wakes: it can't be \
                     combined with --turns or --wake-on"
            .to_owned());
    }
    if a.horizon == Some(0) {
        return Err("--horizon wants at least one turn".to_owned());
    }
    if a.scenario.is_some()
        && (a.seed.is_some()
            || a.nation.is_some()
            || a.start_given
            || a.difficulty.is_some()
            || a.map_size.is_some()
            || a.prelude.is_some())
    {
        return Err("--scenario can't be combined with --seed, --nation, --start, --difficulty, \
                     --map-size or --prelude: the preset already sets them"
            .to_owned());
    }
    Ok(a)
}

/// `--prelude OPENING:MINUTES`: `economy` or `opening` (a scenario's `prelude.opening`), then the
/// game minutes it runs before turn 0.
fn parse_prelude(s: &str) -> Result<Prelude, String> {
    let bad = || format!("--prelude wants OPENING:MINUTES (economy:60, say), got {s:?}");
    let (opening, minutes) = s.split_once(':').ok_or_else(bad)?;
    let opening = match opening {
        "economy" => PreludeOpening::Economy,
        "opening" => PreludeOpening::Opening,
        _ => return Err(bad()),
    };
    let minutes: f32 = minutes.parse().map_err(|_| bad())?;
    if !minutes.is_finite() || minutes <= 0.0 {
        return Err(bad());
    }
    Ok(Prelude { opening, minutes })
}

/// `--wake-on`'s own vocabulary: the same word [`EventKind::as_str`] gives back, so a list a
/// caller types and a wake report's own `since_start.events` keys read the same way.
fn parse_event_kind(s: &str) -> Result<EventKind, String> {
    match s {
        "death" => Ok(EventKind::Death),
        "arrival" => Ok(EventKind::Arrival),
        "tech" => Ok(EventKind::Tech),
        "endeavor" => Ok(EventKind::Endeavor),
        "season" => Ok(EventKind::Season),
        "stockpile" => Ok(EventKind::Stockpile),
        "bend_failed" => Ok(EventKind::BendFailed),
        "need" => Ok(EventKind::Need),
        other => Err(format!(
            "unknown --wake-on kind {other:?} (want death, arrival, tech, endeavor, season, \
             stockpile, bend_failed or need)"
        )),
    }
}

/// `fn_agent play`: runs `--script FILE`'s own code through [`script::run`] against a fresh
/// [`Host`], exactly as one `run_script` call would, with no model and no MCP loop. Writes
/// `--log PATH` the same shape `serve` does (a `Header` line, the run's own `Script` line, every
/// `Keys`/`Turn` line the script's own `act`/`keys`/`wait` calls made, a `Wake` line, then an
/// `End` line) and prints the wake report as JSON on stdout. The caller's own exit code comes
/// from `outcome.reason`, not from whether this function could even start the run: a bad flag,
/// an unreadable script file or a log that couldn't be created is `Err`, reported the same way
/// every other subcommand's own usage mistake is (`main`'s `eprintln!("fn_agent: {e}")`).
fn play(args: Vec<String>) -> Result<ExitCode, String> {
    let a = parse_play_args(args)?;
    let script_path = a.script.as_deref().ok_or("--script is required")?;
    if a.turns.is_none() && a.horizon.is_none() {
        return Err("--turns (or --horizon) is required".to_owned());
    }
    let log_path = a.log.as_deref().ok_or("--log is required")?;
    let code = std::fs::read_to_string(script_path)
        .map_err(|e| format!("can't read --script {script_path:?}: {e}"))?;

    let host = match &a.scenario {
        Some(id) => Host::from_scenario(id)?,
        None => {
            let nation_str = a.nation.as_deref().ok_or("--nation is required")?;
            let nation = fn_tui::cli::parse_nation(nation_str).ok_or_else(|| {
                format!("unknown nation {nation_str:?} (air, water, earth or fire)")
            })?;
            let seed = a.seed.ok_or("--seed is required")?;
            let host =
                Host::new(seed, nation, a.map_size.clone(), a.difficulty.clone(), a.start, false)?;
            match a.prelude {
                Some(prelude) => host.with_prelude(prelude),
                None => host,
            }
        }
    };
    let mut host = host.with_turn_seconds(a.turn_seconds.unwrap_or(DEFAULT_TURN_SECONDS));

    let mut writer = Writer::create(Path::new(log_path)).map_err(|e| e.to_string())?;
    writer.write_header(&host.header()).map_err(|e| e.to_string())?;

    let limits = ScriptLimits::bundled();
    let tag = "play".to_owned();

    let plan = match &a.plan {
        Some(path) => read_plan(path, limits.plan_bytes)?,
        None => serde_json::json!({}),
    };
    if a.plan.is_some() {
        writer.write_line(&LogLine::Plan { plan: plan.clone() }).map_err(|e| e.to_string())?;
    }

    if let Some(horizon) = a.horizon {
        return play_to_horizon(
            &mut host,
            &mut writer,
            &code,
            &tag,
            &plan,
            horizon,
            &limits,
            log_path,
        );
    }
    let turns = a.turns.expect("checked above: --turns or --horizon");
    let wake_on = a.wake_on.unwrap_or_else(|| limits.default_wake.clone());

    writer
        .write_line(&LogLine::Script {
            name: tag.clone(),
            version: 0,
            code: code.clone(),
            turns,
            wake_on: wake_on.clone(),
        })
        .map_err(|e| e.to_string())?;

    let memory = serde_json::json!({});
    let request = script::Request {
        code: &code,
        turns: Some(turns),
        note: Some(&tag),
        version: 0,
        wake_on: &wake_on,
        memory: &memory,
        plan: &plan,
        probe: false,
    };
    let outcome = script::run(&mut host, request, &limits);

    // The script's own `Keys`/`Turn` lines, made while it ran: a fresh `Host` writes nothing to
    // its own `RunLog` before this call, so every one of them is new.
    for line in &host.log().lines() {
        writer.write_line(line).map_err(|e| e.to_string())?;
    }

    writer
        .write_line(&LogLine::Wake {
            reason: outcome.reason.to_owned(),
            why: outcome.why.clone(),
            turn: outcome.observe.turn,
            memory: outcome.memory.clone(),
        })
        .map_err(|e| e.to_string())?;
    writer.write_end(host.tick(), "script finished").map_err(|e| e.to_string())?;

    println!("{}", serde_json::to_string(&outcome).map_err(|e| e.to_string())?);

    // "done" and "budget" are a clean stop; everything else ("error", "runaway", "asked" or
    // "event") is the script asking for a look, the shell-loop signal the acceptance criteria
    // names (docs/design/agent-scripting.md, "Headless play").
    Ok(match outcome.reason {
        "done" | "budget" => ExitCode::SUCCESS,
        _ => ExitCode::FAILURE,
    })
}

/// `--plan FILE`: a JSON object within `cap` bytes, checked the way the `plan` tool checks a `set`.
fn read_plan(path: &str, cap: usize) -> Result<serde_json::Value, String> {
    let text =
        std::fs::read_to_string(path).map_err(|e| format!("can't read --plan {path:?}: {e}"))?;
    let value: serde_json::Value =
        serde_json::from_str(&text).map_err(|e| format!("--plan {path:?} isn't JSON: {e}"))?;
    fn_agent::plan::apply(&serde_json::json!({}), Some(value), None, cap)
        .map_err(|e| format!("--plan {path:?}: {e}"))
}

/// `play --horizon TURNS` (#528): [`fn_agent::clean::run`], then the log's `End` line and the
/// report on stdout -- the last script run's own report plus `restarts`, `stopped_by` and the
/// `scorecard` at the horizon. The scorecard is `fn_agent score LOG --horizon TURNS --json` on
/// the log just written, so the two can never disagree.
#[allow(clippy::too_many_arguments)]
fn play_to_horizon(
    host: &mut Host,
    writer: &mut Writer<std::io::BufWriter<std::fs::File>>,
    code: &str,
    tag: &str,
    plan: &serde_json::Value,
    horizon: u32,
    limits: &ScriptLimits,
    log_path: &str,
) -> Result<ExitCode, String> {
    let run = fn_agent::clean::run(host, code, tag, plan, horizon, limits, |line| {
        writer.write_line(line).map_err(|e| e.to_string())
    })?;
    writer.write_end(host.tick(), "horizon reached").map_err(|e| e.to_string())?;

    let (header, lines) = watch::read_file(Path::new(log_path))?;
    let report = scorecard::build(&header, &lines, Some(horizon))?;

    let mut out = serde_json::to_value(&run.last).map_err(|e| e.to_string())?;
    let fields = out.as_object_mut().expect("an Outcome serializes as an object");
    // The last run's own `observe` predates any forced or coasting turn after it.
    fields.insert(
        "observe".to_owned(),
        serde_json::to_value(host.observe()).map_err(|e| e.to_string())?,
    );
    fields.insert("turns".to_owned(), run.turns.into());
    fields.insert("restarts".to_owned(), run.restarts.into());
    fields.insert(
        "stopped_by".to_owned(),
        run.stopped_by.as_ref().map_or(serde_json::Value::Null, fn_agent::clean::StoppedBy::json),
    );
    fields.insert("scorecard".to_owned(), report.json());
    println!("{}", serde_json::to_string(&out).map_err(|e| e.to_string())?);

    Ok(if run.stopped_by.is_some() { ExitCode::FAILURE } else { ExitCode::SUCCESS })
}

// -------------------------------------------------------------------------------------------
// score (#525, docs/design/agent-hillclimb.md, "The scorecard")
// -------------------------------------------------------------------------------------------

#[derive(Debug, Default, PartialEq, Eq)]
struct ScoreArgs {
    log: Option<String>,
    horizon: Option<u32>,
    json: bool,
    append: bool,
}

fn parse_score_args(args: Vec<String>) -> Result<ScoreArgs, String> {
    let mut a = ScoreArgs::default();
    let mut it = args.into_iter();
    while let Some(arg) = it.next() {
        match arg.as_str() {
            "--horizon" => {
                let v = it.next().ok_or("--horizon needs a value")?;
                a.horizon = Some(
                    v.parse::<u32>().map_err(|e| format!("--horizon wants a turn count: {e}"))?,
                );
            }
            "--json" => a.json = true,
            "--append" => a.append = true,
            flag if flag.starts_with("--") => return Err(format!("unknown flag {flag:?}")),
            path if a.log.is_none() => a.log = Some(path.to_owned()),
            extra => return Err(format!("unexpected argument {extra:?}")),
        }
    }
    Ok(a)
}

/// `fn_agent score LOG`: replays the log to its horizon and prints the scorecard
/// (`fn_agent::scorecard`), as text or `--json`. With `--append` it then adds a `score` line to
/// the log (after anything already there, an `end` line included): the line every reader of the
/// log ignores when replaying.
fn score(args: Vec<String>) -> Result<(), String> {
    let a = parse_score_args(args)?;
    let path = a.log.ok_or("fn_agent score needs a LOG path")?;
    let (header, lines) = watch::read_file(Path::new(&path))?;
    let report = scorecard::build(&header, &lines, a.horizon)?;
    if a.json {
        println!("{}", serde_json::to_string_pretty(&report.json()).map_err(|e| e.to_string())?);
    } else {
        print!("{}", report.text());
    }
    if a.append {
        let mut writer = Writer::append(Path::new(&path)).map_err(|e| e.to_string())?;
        writer.write_line(&report.score_line()).map_err(|e| e.to_string())?;
    }
    Ok(())
}

/// Prints the run at `LOG` as a full `fn_tui` command line: reads the same
/// `fn_tui::surface::RunLogHeader`/`LogLine` schema `fn_tui --watch` does (#324), so a log
/// `fn_agent serve` is still writing reads back here exactly as it would there.
fn keys(args: Vec<String>) -> Result<(), String> {
    let path = args.into_iter().next().ok_or("fn_agent keys needs a LOG path")?;
    let (header, lines) = watch::read_file(Path::new(&path))?;
    let script = watch::to_keys_script(&header, &lines);
    println!("{}", command_line(&header, &script));
    Ok(())
}

/// The `fn_tui` invocation that replays `header`'s run: `--scenario ID` when `header`'s own
/// seed, nation, map size, difficulty, site and prelude exactly match a bundled preset
/// (`fn_tui::scenario::id_for`, #481), else `SEED NATION` plus whatever `--start` and
/// `--difficulty` the header itself says -- either way followed by `--frame --keys "..."`.
fn command_line(header: &RunLogHeader, script: &str) -> String {
    let start = match scenario::id_for(header) {
        Some(id) => format!("--scenario {id}"),
        None => {
            let mut s = format!("{} {}", header.seed, header.nation.as_str());
            if let Some((x, y)) = header.site {
                s.push_str(&format!(" --start {x},{y}"));
            }
            if let Some(d) = &header.difficulty {
                s.push_str(&format!(" --difficulty {d}"));
            }
            s
        }
    };
    format!("fn_tui {start} --frame --keys \"{script}\"")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn header() -> RunLogHeader {
        RunLogHeader {
            seed: 7,
            nation: fn_tui::cli::parse_nation("earth").unwrap(),
            turn_seconds: 20,
            ticks_per_turn: 200,
            map_size: None,
            difficulty: Some("normal".into()),
            site: Some((5, 5)),
            rough: false,
            data_digest: 42,
            crate_version: "0.1.0".into(),
            prelude: None,
        }
    }

    fn resume_args(path: &str) -> ServeArgs {
        ServeArgs {
            seed: None,
            nation: None,
            start_given: false,
            start: None,
            difficulty: None,
            map_size: None,
            prelude: None,
            carry_in: None,
            scenario: None,
            turn_seconds: None,
            log: None,
            resume: Some(path.to_owned()),
            scripts: false,
            trial_seeds: None,
            trial_turns: None,
        }
    }

    fn serve_flags(args: &[&str]) -> Result<ServeArgs, String> {
        parse_serve_args(args.iter().map(|s| (*s).to_owned()).collect())
    }

    /// `--trial-seeds` and `--trial-turns` (#561) come as a pair, need `--scripts`, and work on a
    /// fresh session and a resumed one alike (a chunked attempt keeps the tool).
    #[test]
    fn trial_flags_come_as_a_pair_with_scripts() {
        let a = serve_flags(&[
            "--resume",
            "r.jsonl",
            "--scripts",
            "--trial-seeds",
            "7633, 7,42",
            "--trial-turns",
            "540",
        ])
        .unwrap();
        assert_eq!((a.trial_seeds, a.trial_turns), (Some(vec![7633, 7, 42]), Some(540)));
        let lone = serve_flags(&["--scripts", "--trial-seeds", "7"]).unwrap_err();
        assert!(lone.contains("go together"), "{lone}");
        let no_scripts = serve_flags(&["--trial-seeds", "7", "--trial-turns", "5"]).unwrap_err();
        assert!(no_scripts.contains("--scripts"), "{no_scripts}");
        let bad = serve_flags(&["--scripts", "--trial-seeds", "7,x", "--trial-turns", "5"]);
        assert!(bad.unwrap_err().contains("--trial-seeds"));
    }

    /// `--carry-in`, `--map-size` and `--prelude` (#530) belong to a fresh session: `--carry-in`
    /// needs `--scripts`, and none combine with `--resume` or `--scenario`.
    #[test]
    fn carry_in_map_size_and_prelude_are_for_fresh_scripted_sessions() {
        let a = serve_flags(&[
            "--seed",
            "7",
            "--nation",
            "water",
            "--scripts",
            "--carry-in",
            "h.json",
            "--map-size",
            "small",
            "--prelude",
            "economy:60",
            "--log",
            "l.jsonl",
        ])
        .unwrap();
        assert_eq!(a.carry_in.as_deref(), Some("h.json"));
        assert_eq!(a.map_size.as_deref(), Some("small"));
        assert!(a.prelude.is_some());
        let no_scripts = serve_flags(&["--seed", "7", "--carry-in", "h.json"]).unwrap_err();
        assert!(no_scripts.contains("--scripts"), "{no_scripts}");
        for flags in [["--carry-in", "h.json"], ["--map-size", "small"], ["--prelude", "economy:5"]]
        {
            for other in [["--resume", "r.jsonl"], ["--scenario", "earth-year-two"]] {
                let mut all = vec!["--scripts"];
                all.extend(flags);
                all.extend(other);
                let err = serve_flags(&all).unwrap_err();
                assert!(err.contains(flags[0]) && err.contains("fresh"), "{all:?}: {err}");
            }
        }
    }

    /// `--resume LOG` alone, with no other flags, never fails the header check: nothing was
    /// given to disagree with.
    #[test]
    fn resume_with_no_other_flags_matches_any_header() {
        check_resume_args(&header(), &resume_args("run.jsonl")).unwrap();
    }

    /// Every flag `--resume` accepts alongside it is checked against the log's own header, and a
    /// mismatch is refused with a message naming which flag disagreed.
    #[test]
    fn resume_refuses_every_mismatched_flag() {
        let cases: Vec<(ServeArgs, &str)> = vec![
            (ServeArgs { seed: Some(99), ..resume_args("run.jsonl") }, "--seed"),
            (ServeArgs { nation: Some("fire".into()), ..resume_args("run.jsonl") }, "--nation"),
            (
                ServeArgs { difficulty: Some("hard".into()), ..resume_args("run.jsonl") },
                "--difficulty",
            ),
            (ServeArgs { turn_seconds: Some(99), ..resume_args("run.jsonl") }, "--turn"),
            (
                ServeArgs { start_given: true, start: Some((1, 1)), ..resume_args("run.jsonl") },
                "--start",
            ),
        ];
        for (args, flag) in cases {
            let err = match check_resume_args(&header(), &args) {
                Ok(()) => panic!("{flag} should have been refused"),
                Err(e) => e,
            };
            assert!(err.contains(flag), "{flag}: {err}");
        }
    }

    /// Flags that agree with the header (including `--start default` when the header's own site
    /// is `None`) are accepted.
    #[test]
    fn resume_accepts_matching_flags() {
        let mut h = header();
        h.site = None;
        let args = ServeArgs {
            seed: Some(7),
            nation: Some("earth".into()),
            difficulty: Some("normal".into()),
            turn_seconds: Some(20),
            start_given: true,
            start: None,
            ..resume_args("run.jsonl")
        };
        check_resume_args(&h, &args).unwrap();
    }

    /// `--scenario` can't be combined with anything that would also pick a seed, nation, site,
    /// difficulty or a different session to resume (#481).
    #[test]
    fn scenario_cant_be_combined_with_seed_nation_start_difficulty_or_resume() {
        let base = vec!["--scenario".to_owned(), "earth-year-two".to_owned()];
        for extra in [
            vec!["--seed".to_owned(), "7".to_owned()],
            vec!["--nation".to_owned(), "earth".to_owned()],
            vec!["--start".to_owned(), "default".to_owned()],
            vec!["--difficulty".to_owned(), "hard".to_owned()],
            vec!["--resume".to_owned(), "run.jsonl".to_owned()],
        ] {
            let mut args = base.clone();
            args.extend(extra);
            let err = match parse_serve_args(args) {
                Ok(_) => panic!("should have been refused"),
                Err(e) => e,
            };
            assert!(err.contains("can't be combined with"), "{err}");
        }
        // Alone (plus --turn and --log), it's fine.
        let a = parse_serve_args(vec![
            "--scenario".to_owned(),
            "earth-year-two".to_owned(),
            "--turn".to_owned(),
            "10".to_owned(),
            "--log".to_owned(),
            "run.jsonl".to_owned(),
        ])
        .unwrap();
        assert_eq!(a.scenario.as_deref(), Some("earth-year-two"));
    }

    /// `fn_agent keys`' own command line names `--scenario ID` when the log's header matches a
    /// bundled preset exactly (#481), falling back to `SEED NATION` otherwise.
    #[test]
    fn keys_command_line_names_a_matching_scenario() {
        let scenario = &scenario::bundled()[0];
        let scenario_header = Host::from_scenario(&scenario.id).unwrap().header();
        let line = command_line(&scenario_header, "h <tick 10>");
        assert!(line.contains(&format!("--scenario {}", scenario.id)), "{line}");
        assert!(line.contains("--frame --keys \"h <tick 10>\""), "{line}");

        let plain = header();
        let line = command_line(&plain, "h <tick 10>");
        assert!(line.contains("7 earth"), "{line}");
        assert!(line.contains("--start 5,5"), "{line}");
        assert!(line.contains("--difficulty normal"), "{line}");
        assert!(!line.contains("--scenario"), "{line}");
    }

    /// `--log` given alongside `--resume` must name the same file; a different one is almost
    /// certainly a mistake (watching the wrong run, or clobbering one), caught in `serve_resume`
    /// before anything is read or written.
    #[test]
    fn serve_resume_refuses_a_log_flag_that_disagrees_with_resume() {
        let a = ServeArgs { log: Some("other.jsonl".into()), ..resume_args("run.jsonl") };
        let err = match serve_resume(&a, "run.jsonl") {
            Ok(()) => panic!("a mismatched --log should be refused"),
            Err(e) => e,
        };
        assert!(err.contains("--log") && err.contains("--resume"), "{err}");
    }

    /// #329's own mapping from a signal `install_signal_end_handler` watches for to the name
    /// `write_end_for_signal` writes into the log's `end` line.
    #[test]
    fn signal_name_names_the_three_signals_the_handler_watches_for() {
        assert_eq!(signal_name(SIGTERM), "SIGTERM");
        assert_eq!(signal_name(SIGINT), "SIGINT");
        assert_eq!(signal_name(SIGHUP), "SIGHUP");
    }

    /// A unique scratch path per test (shared with `runlog`'s own tests' pattern), so parallel
    /// tests in this module don't race on the same file.
    fn scratch_log(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "fn_agent_main_test_{}_{}.jsonl",
            std::process::id(),
            name
        ))
    }

    /// `write_end_for_signal` (the handler's own logic, #329's acceptance criteria: a test may
    /// drive this directly rather than an actual signal) appends an `end` line naming the
    /// signal, with the tick of the last `turn` line already in the log.
    #[test]
    fn write_end_for_signal_appends_an_end_line_naming_the_signal_and_the_last_turn() {
        let path = scratch_log("writes_end");
        let mut w = Writer::create(&path).unwrap();
        w.write_header(&header()).unwrap();
        w.write_line(&LogLine::Turn { index: 1, tick: 200 }).unwrap();
        w.write_line(&LogLine::Turn { index: 2, tick: 400 }).unwrap();
        drop(w);

        write_end_for_signal(&path, "SIGTERM").unwrap();

        let (_, lines) = watch::read_file(&path).unwrap();
        std::fs::remove_file(&path).ok();
        assert_eq!(
            lines.last(),
            Some(&LogLine::End { tick: 400, reason: "stopped by SIGTERM".to_owned() })
        );
    }

    /// A log with no `turn` line yet gets `tick: 0` -- the same default
    /// `scripts/agent-run.sh`'s `finish_log` fallback uses.
    #[test]
    fn write_end_for_signal_defaults_to_tick_zero_with_no_turn_yet() {
        let path = scratch_log("no_turns_yet");
        let mut w = Writer::create(&path).unwrap();
        w.write_header(&header()).unwrap();
        drop(w);

        write_end_for_signal(&path, "SIGINT").unwrap();

        let (_, lines) = watch::read_file(&path).unwrap();
        std::fs::remove_file(&path).ok();
        assert_eq!(
            lines.last(),
            Some(&LogLine::End { tick: 0, reason: "stopped by SIGINT".to_owned() })
        );
    }

    /// A log that already ends with an `end` line (a race with `run_tools`'s own clean
    /// shutdown, or a second signal) is left alone: never two `end` lines.
    #[test]
    fn write_end_for_signal_does_nothing_if_the_log_already_ended() {
        let path = scratch_log("already_ended");
        let mut w = Writer::create(&path).unwrap();
        w.write_header(&header()).unwrap();
        w.write_line(&LogLine::Turn { index: 1, tick: 200 }).unwrap();
        w.write_end(200, "stdin closed").unwrap();
        drop(w);

        write_end_for_signal(&path, "SIGHUP").unwrap();

        let (_, lines) = watch::read_file(&path).unwrap();
        std::fs::remove_file(&path).ok();
        assert_eq!(lines.len(), 2, "{lines:?}");
        assert_eq!(
            lines.last(),
            Some(&LogLine::End { tick: 200, reason: "stdin closed".to_owned() })
        );
    }

    /// #511: a served session stopped by the #329 signal path and then resumed (as
    /// `serve_resume` does: `Writer::append`, no rewrite) never moves a byte of what a tail
    /// following the file has already read, so the tail gets every line whole, `end` in the
    /// middle included.
    #[test]
    fn a_tail_follows_a_session_stopped_by_a_signal_and_resumed_line_for_line() {
        use serde_json::json;

        let path = scratch_log("tail_across_resume");
        let host =
            Host::new(7, fn_tui::cli::parse_nation("fire").unwrap(), None, None, None, false)
                .unwrap();
        let mut tail_lines = Vec::new();

        let mut tools = Tools::new(host, Writer::create(&path).unwrap()).unwrap();
        let mut tail = watch::LogTail::open(&path).unwrap();
        tools.call("end_turn", json!({"turns": 1})).unwrap();
        tail_lines.extend(tail.poll().unwrap());
        // The signal arrives while the session is blocked reading stdin: the handler appends the
        // `end` line through its own file handle.
        write_end_for_signal(&path, "SIGTERM").unwrap();
        tail_lines.extend(tail.poll().unwrap());
        drop(tools);

        let (header, lines) = watch::read_file(&path).unwrap();
        let host = Host::resume(&header, &lines).unwrap();
        let mut tools = Tools::resume(host, Writer::append(&path).unwrap(), &lines);
        tail_lines.extend(tail.poll().unwrap());
        tools.call("end_turn", json!({"turns": 1})).unwrap();
        tail_lines.extend(tail.poll().unwrap());
        tools.end("stdin closed").unwrap();
        tail_lines.extend(tail.poll().unwrap());

        let on_disk = std::fs::read_to_string(&path).unwrap();
        std::fs::remove_file(&path).ok();
        let want: Vec<&str> = on_disk.lines().collect();
        assert_eq!(tail_lines, want, "the tail should have seen every line whole");
        let parsed: Vec<LogLine> =
            tail_lines.iter().map(|l| serde_json::from_str(l).unwrap()).collect();
        let ends = parsed.iter().filter(|l| matches!(l, LogLine::End { .. })).count();
        assert_eq!(ends, 2, "one per session: {parsed:?}");
        assert!(matches!(parsed.last(), Some(LogLine::End { .. })));
        assert!(
            parsed.iter().position(|l| matches!(l, LogLine::End { .. })).unwrap()
                < parsed.len() - 2,
            "the first end sits mid-log, with the resumed session's lines after it"
        );
    }

    // -----------------------------------------------------------------------------------------
    // play (#483)
    // -----------------------------------------------------------------------------------------

    fn play_args(extra: &[&str]) -> Vec<String> {
        let mut v = vec![
            "--script".to_owned(),
            "lib.rhai".to_owned(),
            "--seed".to_owned(),
            "7".to_owned(),
            "--nation".to_owned(),
            "earth".to_owned(),
            "--turns".to_owned(),
            "10".to_owned(),
            "--log".to_owned(),
            "run.jsonl".to_owned(),
        ];
        v.extend(extra.iter().map(|s| (*s).to_owned()));
        v
    }

    /// `--map-size` and `--prelude OPENING:MINUTES` (#529) parse; a bad prelude is refused, and a
    /// scenario can't be combined with either.
    #[test]
    fn parse_play_args_reads_map_size_and_prelude() {
        let a = parse_play_args(play_args(&["--map-size", "medium", "--prelude", "economy:60"]))
            .unwrap();
        assert_eq!(a.map_size.as_deref(), Some("medium"));
        assert_eq!(a.prelude, Some(Prelude { opening: PreludeOpening::Economy, minutes: 60.0 }));
        assert_eq!(
            parse_prelude("opening:12.5").unwrap(),
            Prelude { opening: PreludeOpening::Opening, minutes: 12.5 }
        );
        for bad in ["economy", "economy:", "economy:0", "economy:-1", "sprint:5", ":5"] {
            assert!(parse_prelude(bad).unwrap_err().contains("--prelude"), "{bad}");
        }
        for clash in [["--map-size", "small"], ["--prelude", "economy:60"]] {
            let mut args: Vec<String> = ["--scenario", "air-year-two"].map(str::to_owned).to_vec();
            args.extend(["--script", "s.rhai", "--turns", "5", "--log", "l"].map(str::to_owned));
            args.extend(clash.map(str::to_owned));
            assert!(parse_play_args(args).is_err(), "{clash:?}");
        }
    }

    /// Every flag `play` takes (`--script`, `--seed`, `--nation`, `--start`, `--difficulty`,
    /// `--turn`, `--turns`, `--wake-on`, `--log`) parses into [`PlayArgs`].
    #[test]
    fn parse_play_args_reads_every_flag() {
        let a = parse_play_args(play_args(&[
            "--start",
            "5,5",
            "--difficulty",
            "hard",
            "--turn",
            "30",
            "--wake-on",
            "death,need",
        ]))
        .unwrap();
        assert_eq!(a.script.as_deref(), Some("lib.rhai"));
        assert_eq!(a.seed, Some(7));
        assert_eq!(a.nation.as_deref(), Some("earth"));
        assert_eq!(a.start, Some((5, 5)));
        assert_eq!(a.difficulty.as_deref(), Some("hard"));
        assert_eq!(a.turn_seconds, Some(30));
        assert_eq!(a.turns, Some(10));
        assert_eq!(a.wake_on, Some(vec![EventKind::Death, EventKind::Need]));
        assert_eq!(a.log.as_deref(), Some("run.jsonl"));
    }

    /// `--wake-on` wants a known event-kind word, comma-separated; an unknown one is refused
    /// naming it, not silently dropped.
    #[test]
    fn parse_play_args_refuses_an_unknown_wake_on_kind() {
        let err = parse_play_args(play_args(&["--wake-on", "death,nope"])).unwrap_err();
        assert!(err.contains("nope"), "{err}");
    }

    /// `--scenario` can't be combined with `--seed`, `--nation`, `--start` or `--difficulty`,
    /// the same rule `serve` enforces (its own preset already sets them).
    #[test]
    fn parse_play_args_refuses_scenario_combined_with_seed_or_nation() {
        let args = vec![
            "--script".to_owned(),
            "lib.rhai".to_owned(),
            "--scenario".to_owned(),
            "earth-year-two".to_owned(),
            "--seed".to_owned(),
            "7".to_owned(),
            "--turns".to_owned(),
            "10".to_owned(),
            "--log".to_owned(),
            "run.jsonl".to_owned(),
        ];
        let err = parse_play_args(args).unwrap_err();
        assert!(err.contains("can't be combined with"), "{err}");
    }

    /// `play` refuses to start at all without `--script`, `--turns` or `--log`, before it ever
    /// touches a `Host` or opens a file.
    #[test]
    fn play_refuses_to_start_without_script_turns_or_log() {
        let base = |missing: &str| -> Vec<String> {
            play_args(&[]).into_iter().filter(|a| a != missing).collect()
        };
        for (flag, value) in [("--script", "lib.rhai"), ("--turns", "10"), ("--log", "run.jsonl")] {
            let mut args = base(flag);
            // Dropping the flag alone would leave its value as a stray positional; drop both.
            if let Some(pos) = args.iter().position(|a| a == value) {
                args.remove(pos);
            }
            let err = match play(args) {
                Ok(_) => panic!("{flag} missing should have been refused"),
                Err(e) => e,
            };
            assert!(err.contains(flag), "{flag}: {err}");
        }
    }

    /// `--horizon` and `--plan` (#528) parse; `--horizon` stands in for `--turns` and can't be
    /// combined with it or with `--wake-on`.
    #[test]
    fn parse_play_args_reads_horizon_and_plan() {
        let without_turns: Vec<String> = {
            let mut v = play_args(&[]);
            let at = v.iter().position(|a| a == "--turns").unwrap();
            v.drain(at..at + 2);
            v
        };
        let mut args = without_turns.clone();
        args.extend(["--horizon", "540", "--plan", "plan.json"].map(str::to_owned));
        let a = parse_play_args(args).unwrap();
        assert_eq!((a.horizon, a.turns), (Some(540), None));
        assert_eq!(a.plan.as_deref(), Some("plan.json"));

        for clash in [["--turns", "5"], ["--wake-on", "death"]] {
            let mut args = without_turns.clone();
            args.extend(["--horizon", "540"].map(str::to_owned));
            args.extend(clash.map(str::to_owned));
            let err = parse_play_args(args).unwrap_err();
            assert!(err.contains("--horizon"), "{err}");
        }
        let mut args = without_turns;
        args.extend(["--horizon", "0"].map(str::to_owned));
        assert!(parse_play_args(args).is_err());
    }

    /// `--wake-on`'s own vocabulary round-trips: every word [`EventKind::as_str`] gives back
    /// parses back to the same kind.
    #[test]
    fn parse_event_kind_round_trips_every_event_kind_word() {
        for kind in [
            EventKind::Death,
            EventKind::Arrival,
            EventKind::Tech,
            EventKind::Endeavor,
            EventKind::Season,
            EventKind::Stockpile,
            EventKind::BendFailed,
            EventKind::Need,
        ] {
            assert_eq!(parse_event_kind(kind.as_str()), Ok(kind));
        }
        assert!(parse_event_kind("nope").is_err());
    }
}
