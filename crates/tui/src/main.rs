//! The terminal frontend.
//!
//!     cargo run -p fn_tui                             # the title screen
//!     cargo run -p fn_tui -- SEED NATION               # play in the terminal, skipping it
//!     cargo run -p fn_tui -- SEED NATION --frame       # print one frame and exit (no TTY needed)
//!
//! See `fn_tui --help`.

use std::io::IsTerminal;
use std::process::ExitCode;
use std::time::{SystemTime, UNIX_EPOCH};

use fn_tui::cli::{self, Args};
use fn_tui::color::{ColorDepth, Palette};
use fn_tui::frame;
use fn_tui::keys::Step;
use fn_tui::shell::Shell;

fn main() -> ExitCode {
    let args = match cli::parse(std::env::args().skip(1)) {
        Ok(args) => args,
        Err(message) if message == cli::USAGE => {
            println!("{message}");
            return ExitCode::SUCCESS;
        }
        Err(message) => {
            eprintln!("fn_tui: {message}\n\n{}", cli::USAGE);
            return ExitCode::from(2);
        }
    };
    let depth = ColorDepth::detect(args.color, |k| std::env::var(k).ok());
    let saves = fn_ui::saves::default_save_dir();

    if let Some(path) = &args.watch {
        if args.frame {
            return match print_watch_frame(&args, path, depth) {
                Ok(()) => ExitCode::SUCCESS,
                Err(e) => {
                    eprintln!("fn_tui: {e}");
                    ExitCode::from(2)
                }
            };
        }
        if !(std::io::stdout().is_terminal() && std::io::stdin().is_terminal()) {
            eprintln!(
                "fn_tui: this needs a terminal. To look at a watched run from a script, add \
                 --frame."
            );
            return ExitCode::from(2);
        }
        return match fn_tui::watch::run(path, depth, args.ascii, args.pace) {
            Ok(()) => ExitCode::SUCCESS,
            Err(e) => {
                eprintln!("fn_tui: {e}");
                ExitCode::FAILURE
            }
        };
    }

    if args.frame {
        return match print_frame(&args, depth, saves) {
            Ok(()) => ExitCode::SUCCESS,
            Err(e) => {
                eprintln!("fn_tui: {e}");
                ExitCode::from(2)
            }
        };
    }
    if !(std::io::stdout().is_terminal() && std::io::stdin().is_terminal()) {
        eprintln!(
            "fn_tui: this needs a terminal. To look at the game from a script or an agent, add \
             --frame."
        );
        return ExitCode::from(2);
    }
    match fn_tui::run::run(&args, depth, saves, seed_hint()) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("fn_tui: {e}");
            ExitCode::FAILURE
        }
    }
}

/// A seed to offer a new game at the title screen, before the player has typed one: the wall
/// clock is read here and nowhere past this (docs/design/tui.md, "A deterministic clock"), since
/// `main` and `run` are the only edges of the program that are allowed to.
fn seed_hint() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_nanos() as u64)
}

/// `--frame`: one frame on stdout, with no raw mode and no alternate screen. With no `SEED`, the
/// frame is the title screen (#280); `--embark` and `--start` (#290) pick embark or a founded
/// game instead of the plain centre-spiral one; `--ticks` runs before the frame, and only means
/// anything for a game (there's nothing to tick at the title screen or in embark).
fn print_frame(
    args: &Args,
    depth: ColorDepth,
    saves: Option<std::path::PathBuf>,
) -> Result<(), String> {
    let mut shell = Shell::for_args(args, args.size, saves, seed_hint())?;
    let mut steps = Vec::new();
    if args.ticks > 0 {
        steps.push(Step::Tick(args.ticks));
    }
    steps.extend(args.keys.iter().copied());
    shell.play(&steps);
    let palette = Palette::new(shell.defs(), shell.nation(), depth, args.ascii);
    let buf = frame::render_shell(&mut shell, &palette, args.size);
    let text = if args.ansi { frame::to_ansi(&buf) } else { frame::to_text(&buf) };
    print!("{text}");
    Ok(())
}

/// `--watch LOG --frame [--at TURN]`: one frame of the watched session on stdout, no terminal
/// needed (#319's acceptance criteria: "so a test checks the frame at a given turn without a
/// TTY"). Reads as much of `path` as is on disk right now (it need not be finished) and replays
/// it all at once, up to the end of `--at`'s turn, or as far as it reaches without one.
fn print_watch_frame(args: &Args, path: &std::path::Path, depth: ColorDepth) -> Result<(), String> {
    let (header, events) = fn_tui::watch::read_file(path)?;
    let mut watcher = fn_tui::watch::Watcher::new(&header)?;
    watcher.feed(events);
    watcher.fast_forward_to_turn(args.at.unwrap_or(u32::MAX));
    let palette = Palette::new(watcher.defs(), watcher.nation(), depth, args.ascii);
    let buf = watcher.render(&palette, args.size);
    let text = if args.ansi { frame::to_ansi(&buf) } else { frame::to_text(&buf) };
    print!("{text}");
    Ok(())
}
