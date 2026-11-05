//! The CLI: `fn_agent serve ...` and `fn_agent keys LOG`. See the crate's own doc comment
//! (`fn_agent::...`) for the protocol and the sandboxing recipe.

use std::fs;
use std::io::{BufReader, stdin, stdout};
use std::process::ExitCode;

use fn_agent::runlog::{self, Writer};
use fn_agent::server;
use fn_agent::tools::Tools;
use fn_tui::surface::{DEFAULT_TURN_SECONDS, Host};

const USAGE: &str = "\
usage: fn_agent serve --seed N --nation X [--start default|X,Y] [--difficulty ID]
                       [--turn SECS] --log PATH
       fn_agent keys LOG

serve speaks MCP (JSON-RPC 2.0) over stdin and stdout: initialize, tools/list and tools/call.
keys prints the run at LOG as a --keys script (fn_tui SEED NATION --frame --keys \"...\").";

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
    seed: u64,
    nation: String,
    start: Option<(i32, i32)>,
    difficulty: Option<String>,
    turn_seconds: u32,
    log: String,
}

fn parse_serve_args(args: Vec<String>) -> Result<ServeArgs, String> {
    let (mut seed, mut nation, mut start, mut difficulty, mut log) = (None, None, None, None, None);
    let mut turn_seconds = DEFAULT_TURN_SECONDS;
    let mut it = args.into_iter();
    while let Some(flag) = it.next() {
        let mut value = || it.next().ok_or_else(|| format!("{flag} needs a value"));
        match flag.as_str() {
            "--seed" => seed = Some(value()?.parse::<u64>().map_err(|e| e.to_string())?),
            "--nation" => nation = Some(value()?),
            "--start" => {
                let v = value()?;
                start = match v.as_str() {
                    "default" => None,
                    other => Some(parse_pos(other)?),
                };
            }
            "--difficulty" => difficulty = Some(value()?),
            "--turn" => turn_seconds = value()?.parse::<u32>().map_err(|e| e.to_string())?,
            "--log" => log = Some(value()?),
            other => return Err(format!("unknown flag {other:?}")),
        }
    }
    Ok(ServeArgs {
        seed: seed.ok_or("--seed is required")?,
        nation: nation.ok_or("--nation is required")?,
        start,
        difficulty,
        turn_seconds,
        log: log.ok_or("--log is required")?,
    })
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
    let nation = fn_tui::cli::parse_nation(&a.nation)
        .ok_or_else(|| format!("unknown nation {:?} (air, water, earth or fire)", a.nation))?;
    let host = Host::new(a.seed, nation, None, a.difficulty, a.start, false)?
        .with_turn_seconds(a.turn_seconds);
    let writer = Writer::create(std::path::Path::new(&a.log)).map_err(|e| e.to_string())?;
    let mut tools = Tools::new(host, writer);
    tools.write_header(a.seed, &a.nation)?;

    let input = BufReader::new(stdin());
    server::serve(input, stdout(), &mut tools);
    Ok(())
}

fn keys(args: Vec<String>) -> Result<(), String> {
    let path = args.into_iter().next().ok_or("fn_agent keys needs a LOG path")?;
    let text = fs::read_to_string(&path).map_err(|e| format!("{path}: {e}"))?;
    let events = runlog::read(text.as_bytes())?;
    println!("{}", runlog::to_keys_script(&events)?);
    Ok(())
}
