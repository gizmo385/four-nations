//! The interactive loop, and the only place that reads the wall clock. `fn_sim` and `fn_ui` never
//! see real time: it only decides *how many* ticks run between inputs (`Clock::advance`).
//!
//! Each pass of the loop:
//!
//! 1. waits for input, for no longer than the next frame is due;
//! 2. handles every pending key and resize (`App::key`), so any command a key issues is applied
//!    between ticks, never in the middle of one;
//! 3. runs the ticks the elapsed time is worth;
//! 4. redraws if anything changed, at most 30 times a second.

use std::io::{self, Stdout, Write};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use crossterm::event::{
    self, Event, KeyboardEnhancementFlags, PopKeyboardEnhancementFlags,
    PushKeyboardEnhancementFlags,
};
use crossterm::terminal::{
    EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
    supports_keyboard_enhancement,
};
use crossterm::{execute, terminal};
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;

use crate::cli::Args;
use crate::color::{ColorDepth, Palette};
use crate::shell::{self, Shell};

/// The fastest the screen redraws.
const FRAME: Duration = Duration::from_millis(33);

/// Whether the keyboard enhancement flags were pushed, so restoring pops them only then.
static ENHANCED: AtomicBool = AtomicBool::new(false);

/// Puts the terminal back the way we found it. Safe to call twice, and from a panic hook.
pub(crate) fn restore() {
    let mut out = io::stdout();
    if ENHANCED.swap(false, Ordering::SeqCst) {
        let _ = execute!(out, PopKeyboardEnhancementFlags);
    }
    let _ = disable_raw_mode();
    let _ = execute!(out, LeaveAlternateScreen, crossterm::cursor::Show);
    let _ = out.flush();
}

/// Restores the terminal when dropped, so an error return leaves a usable shell too.
pub(crate) struct Restore;

impl Drop for Restore {
    fn drop(&mut self) {
        restore();
    }
}

pub(crate) fn setup() -> io::Result<Terminal<CrosstermBackend<Stdout>>> {
    // A panic restores the terminal before its message prints, so the message is readable.
    let previous = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        restore();
        previous(info);
    }));
    enable_raw_mode()?;
    let mut out = io::stdout();
    execute!(out, EnterAlternateScreen)?;
    // Where the terminal can tell the numpad from the number row (the kitty protocol), ask it to.
    if supports_keyboard_enhancement().unwrap_or(false)
        && execute!(
            out,
            PushKeyboardEnhancementFlags(KeyboardEnhancementFlags::DISAMBIGUATE_ESCAPE_CODES)
        )
        .is_ok()
    {
        ENHANCED.store(true, Ordering::SeqCst);
    }
    Terminal::new(CrosstermBackend::new(out))
}

/// Plays the title screen and then a game in the terminal until the player quits for good
/// (#280). `args` resolves (`Shell::for_args`) to the title screen, a game or embark (`--embark`,
/// `--start`, #290), skipping the title screen when `SEED NATION` is given; `seed_hint` seeds the
/// title screen's own default seed, the one thing here that is not already decided before the
/// terminal is touched. A new nation (Switch nation…, or a new game chosen at the title screen) needs
/// new colours, so the palette is rebuilt for it at the same depth.
///
/// `runs` is the agent runs folder (#841): the title screen's Replays item opens the run browser
/// on it, nested on this same terminal (`crate::browser::run_in`), and its `q` comes back here
/// to the title screen as it was.
pub fn run(
    args: &Args,
    depth: ColorDepth,
    saves: Option<PathBuf>,
    runs: Option<PathBuf>,
    seed_hint: u64,
) -> io::Result<()> {
    let _restore = Restore;
    let mut terminal = setup()?;
    let (w, h) = terminal::size()?;
    let mut shell = Shell::for_args(args, (w, h), saves, seed_hint)
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidInput, e))?
        .with_runs(runs);
    let mut nation = shell.nation();
    let mut palette = Palette::new(shell.defs(), nation, depth, args.ascii);

    let mut last_tick = Instant::now();
    let mut last_draw: Option<Instant> = None;
    while !shell.quit {
        if shell.dirty && last_draw.is_none_or(|t| t.elapsed() >= FRAME) {
            if shell.nation() != nation {
                nation = shell.nation();
                palette = Palette::new(shell.defs(), nation, palette.depth, palette.ascii);
            }
            terminal.draw(|f| {
                let area = f.area();
                shell::draw(&mut shell, &palette, f.buffer_mut());
                if let Some(at) = shell::terminal_cursor(&shell, area) {
                    f.set_cursor_position(at);
                }
            })?;
            shell.dirty = false;
            last_draw = Some(Instant::now());
        }

        // Wake for the next frame; paused with nothing to draw, only input wakes us (and a toast
        // that is still ageing).
        let wait = match (shell.dirty, shell.paused() && !shell.has_toasts()) {
            (true, _) => FRAME.saturating_sub(last_draw.map_or(FRAME, |t| t.elapsed())),
            (false, false) => FRAME,
            (false, true) => Duration::from_secs(3600),
        };
        let mut pending = event::poll(wait)?;
        // Time spent paused isn't owed to the sim once it runs again.
        let was_paused = shell.paused();
        while pending {
            match event::read()? {
                Event::Key(key) => shell.key(crate::keyev::from_crossterm(key)),
                Event::Resize(w, h) => shell.resize(w, h),
                _ => {}
            }
            if let Some(dir) = shell.take_replays() {
                let cli = crate::browser::Cli::for_dir(dir, args.ascii);
                crate::browser::run_in(&mut terminal, &cli, depth)?;
                // The browser drew over everything, and the terminal may have changed size.
                terminal.clear()?;
                let (w, h) = terminal::size()?;
                shell.resize(w, h);
                last_tick = Instant::now();
            }
            pending = event::poll(Duration::ZERO)?;
        }

        let now = Instant::now();
        let elapsed = now.duration_since(last_tick).as_secs_f64();
        last_tick = now;
        if was_paused {
            shell.age(elapsed);
        } else {
            shell.advance(elapsed);
        }
    }
    Ok(())
}
