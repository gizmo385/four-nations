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
use fn_ui::Session;
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;

use crate::app::App;
use crate::color::Palette;
use crate::draw::{draw, terminal_cursor};

/// The fastest the screen redraws.
const FRAME: Duration = Duration::from_millis(33);

/// Whether the keyboard enhancement flags were pushed, so restoring pops them only then.
static ENHANCED: AtomicBool = AtomicBool::new(false);

/// Puts the terminal back the way we found it. Safe to call twice, and from a panic hook.
fn restore() {
    let mut out = io::stdout();
    if ENHANCED.swap(false, Ordering::SeqCst) {
        let _ = execute!(out, PopKeyboardEnhancementFlags);
    }
    let _ = disable_raw_mode();
    let _ = execute!(out, LeaveAlternateScreen, crossterm::cursor::Show);
    let _ = out.flush();
}

/// Restores the terminal when dropped, so an error return leaves a usable shell too.
struct Restore;

impl Drop for Restore {
    fn drop(&mut self) {
        restore();
    }
}

fn setup() -> io::Result<Terminal<CrosstermBackend<Stdout>>> {
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

/// Plays `session` in the terminal until the player quits. A new nation (`1`-`4`) needs new
/// colours, so the palette is rebuilt for it at the same depth.
pub fn run(session: Session, mut palette: Palette) -> io::Result<()> {
    let _restore = Restore;
    let mut terminal = setup()?;
    let (w, h) = terminal::size()?;
    let mut app = App::new(session, (w, h));
    let mut nation = app.session.world.nation;

    let mut last_tick = Instant::now();
    let mut last_draw: Option<Instant> = None;
    while !app.ui.quit {
        if app.dirty && last_draw.is_none_or(|t| t.elapsed() >= FRAME) {
            if app.session.world.nation != nation {
                nation = app.session.world.nation;
                palette = Palette::new(&app.session.defs, nation, palette.depth, palette.ascii);
            }
            terminal.draw(|f| {
                let area = f.area();
                draw(&app.session, &app.ui, &palette, f.buffer_mut());
                if let Some(at) = terminal_cursor(&app.ui, area) {
                    f.set_cursor_position(at);
                }
            })?;
            app.dirty = false;
            last_draw = Some(Instant::now());
        }

        // Wake for the next frame; paused with nothing to draw, only input wakes us (and a toast
        // that is still ageing).
        let wait = match (app.dirty, app.session.clock.paused && !app.has_toasts()) {
            (true, _) => FRAME.saturating_sub(last_draw.map_or(FRAME, |t| t.elapsed())),
            (false, false) => FRAME,
            (false, true) => Duration::from_secs(3600),
        };
        let mut pending = event::poll(wait)?;
        // Time spent paused isn't owed to the sim once it runs again.
        let was_paused = app.session.clock.paused;
        while pending {
            match event::read()? {
                Event::Key(key) => app.key(key),
                Event::Resize(w, h) => app.resize(w, h),
                _ => {}
            }
            pending = event::poll(Duration::ZERO)?;
        }

        let now = Instant::now();
        let elapsed = now.duration_since(last_tick).as_secs_f64();
        last_tick = now;
        if was_paused {
            app.age(elapsed);
        } else {
            app.advance(elapsed);
        }
    }
    Ok(())
}
