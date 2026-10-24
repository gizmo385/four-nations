//! The terminal frontend.
//!
//!     cargo run -p fn_tui -- SEED NATION            # play in the terminal
//!     cargo run -p fn_tui -- SEED NATION --frame    # print one frame and exit (no TTY needed)
//!
//! See `fn_tui --help`.

use std::io::IsTerminal;
use std::process::ExitCode;

use fn_tui::app::App;
use fn_tui::cli::{self, Args};
use fn_tui::color::{ColorDepth, Palette};
use fn_tui::frame;
use fn_tui::new_session;

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
    let mut session = new_session(args.seed, args.nation);
    let palette = Palette::new(&session.defs, args.nation, depth, args.ascii);

    if args.frame {
        session.tick_n(args.ticks);
        print_frame(session, &palette, &args);
        return ExitCode::SUCCESS;
    }
    if !(std::io::stdout().is_terminal() && std::io::stdin().is_terminal()) {
        eprintln!(
            "fn_tui: this needs a terminal. To look at the game from a script or an agent, add \
             --frame: fn_tui {} {} --frame",
            args.seed,
            args.nation.as_str()
        );
        return ExitCode::from(2);
    }
    match fn_tui::run::run(session, &palette) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("fn_tui: {e}");
            ExitCode::FAILURE
        }
    }
}

/// `--frame`: one frame on stdout, with no raw mode and no alternate screen.
fn print_frame(session: fn_ui::Session, palette: &Palette, args: &Args) {
    let mut app = App::new(session, args.size);
    app.play(&args.keys);
    let buf = frame::render_app(&mut app, palette, args.size);
    let text = if args.ansi { frame::to_ansi(&buf) } else { frame::to_text(&buf) };
    print!("{text}");
}
