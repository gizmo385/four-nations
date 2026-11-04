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

    if args.frame {
        print_frame(&args, depth, saves);
        return ExitCode::SUCCESS;
    }
    if !(std::io::stdout().is_terminal() && std::io::stdin().is_terminal()) {
        eprintln!(
            "fn_tui: this needs a terminal. To look at the game from a script or an agent, add \
             --frame."
        );
        return ExitCode::from(2);
    }
    match fn_tui::run::run(args.seed.zip(args.nation), depth, args.ascii, saves, seed_hint()) {
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
/// frame is the title screen (#280); `--ticks` only means anything with one.
fn print_frame(args: &Args, depth: ColorDepth, saves: Option<std::path::PathBuf>) {
    let mut shell = match (args.seed, args.nation) {
        (Some(seed), Some(nation)) => {
            let mut session = fn_tui::new_session(seed, nation);
            session.tick_n(args.ticks);
            Shell::new_game(session, args.size, saves)
        }
        _ => Shell::new_title(args.size, saves, seed_hint()),
    };
    shell.play(&args.keys);
    let palette = Palette::new(shell.defs(), shell.nation(), depth, args.ascii);
    let buf = frame::render_shell(&mut shell, &palette, args.size);
    let text = if args.ansi { frame::to_ansi(&buf) } else { frame::to_text(&buf) };
    print!("{text}");
}
