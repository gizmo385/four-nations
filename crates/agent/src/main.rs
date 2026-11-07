//! The CLI: `fn_agent serve ...` and `fn_agent keys LOG`. See the crate's own doc comment
//! (`fn_agent::...`) for the protocol and the sandboxing recipe.

use std::io::{BufReader, Write, stdin, stdout};
use std::path::Path;
use std::process::ExitCode;

use fn_agent::runlog::Writer;
use fn_agent::script::Limits as ScriptLimits;
use fn_agent::server;
use fn_agent::tools::Tools;
use fn_tui::surface::{DEFAULT_TURN_SECONDS, Host, LogLine, RunLogHeader};
use fn_tui::{scenario, watch};

const USAGE: &str = "\
usage: fn_agent serve --seed N --nation X [--start default|X,Y] [--difficulty ID]
                       [--turn SECS] [--scripts] --log PATH
       fn_agent serve --scenario ID [--turn SECS] [--scripts] --log PATH
       fn_agent serve --resume LOG [--seed N] [--nation X] [--start default|X,Y]
                       [--difficulty ID] [--turn SECS] [--scripts]
       fn_agent keys LOG

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

--scripts (docs/design/agent-scripting.md) adds the run_script tool: a sandboxed Rhai runner
over the same Host, with limits from data/agent.toml's [scripts] table. Works with --scenario
too: the runner sees whatever world the prelude left, the same as any other start.

keys prints the run at LOG as a --keys script, as a full fn_tui command line: --scenario ID when
the log's own header matches a bundled preset, else SEED NATION --start ... --difficulty ....";

fn main() -> ExitCode {
    let mut args = std::env::args().skip(1);
    let result = match args.next().as_deref() {
        Some("serve") => serve(args.collect()),
        Some("keys") => keys(args.collect()),
        _ => Err(USAGE.to_owned()),
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("fn_agent: {e}");
            ExitCode::FAILURE
        }
    }
}

struct ServeArgs {
    seed: Option<u64>,
    nation: Option<String>,
    /// Whether `--start` was given at all, so a resumed session only checks it against the
    /// log's own header when the caller actually asked for a particular one.
    start_given: bool,
    start: Option<(i32, i32)>,
    difficulty: Option<String>,
    /// `--scenario ID` (#481): a `data/scenarios.toml` preset's id, in place of `--seed`,
    /// `--nation`, `--start` and `--difficulty`.
    scenario: Option<String>,
    turn_seconds: Option<u32>,
    log: Option<String>,
    resume: Option<String>,
    scripts: bool,
}

fn parse_serve_args(args: Vec<String>) -> Result<ServeArgs, String> {
    let mut a = ServeArgs {
        seed: None,
        nation: None,
        start_given: false,
        start: None,
        difficulty: None,
        scenario: None,
        turn_seconds: None,
        log: None,
        resume: None,
        scripts: false,
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
            "--scenario" => a.scenario = Some(value()?),
            "--turn" => a.turn_seconds = Some(value()?.parse::<u32>().map_err(|e| e.to_string())?),
            "--log" => a.log = Some(value()?),
            "--resume" => a.resume = Some(value()?),
            "--scripts" => a.scripts = true,
            other => return Err(format!("unknown flag {other:?}")),
        }
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
            Host::new(seed, nation, None, a.difficulty.clone(), a.start, false)?
        }
    };
    let host = host.with_turn_seconds(a.turn_seconds.unwrap_or(DEFAULT_TURN_SECONDS));
    let writer = Writer::create(Path::new(log)).map_err(|e| e.to_string())?;
    let tools = with_scripts(Tools::new(host, writer)?, a);
    run_tools(tools)
}

/// Turns `run_script` on when `--scripts` was given (`Tools::with_scripts`), with the limits
/// `data/agent.toml` ships.
fn with_scripts<W: Write>(tools: Tools<W>, a: &ServeArgs) -> Tools<W> {
    if a.scripts { tools.with_scripts(ScriptLimits::bundled()) } else { tools }
}

/// `--resume LOG`: rebuilds `LOG`'s own session (`Host::resume`) from its header and lines,
/// refuses to start if any flag given alongside `--resume` disagrees with that header
/// (`check_resume_args`), then drops `LOG`'s `end` line (if it has one) and keeps appending to
/// the same file (`Writer::recreate_with`) -- so a run can be played as a chain of short
/// sessions, and `fn_tui --watch`/`fn_agent keys` still read it as one continuous run (#341).
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
    let kept: Vec<LogLine> =
        lines.into_iter().filter(|l| !matches!(l, LogLine::End { .. })).collect();
    let writer =
        Writer::recreate_with(Path::new(path), &header, &kept).map_err(|e| e.to_string())?;
    let tools = with_scripts(Tools::resume(host, writer), a);
    run_tools(tools)
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

fn run_tools<W: Write>(mut tools: Tools<W>) -> Result<(), String> {
    let input = BufReader::new(stdin());
    let reason = server::serve(input, stdout(), &mut tools);
    tools.end(&reason)?;
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
            scenario: None,
            turn_seconds: None,
            log: None,
            resume: Some(path.to_owned()),
            scripts: false,
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
}
