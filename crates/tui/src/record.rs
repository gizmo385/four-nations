//! `fn_tui --watch LOG --record OUT.cast` (#503, docs/design/agent-studio.md, "Recording"): a
//! replay rendered with no terminal at all, into an asciicast v2 file.
//!
//! The [`Watcher`] runs on a *virtual* clock: every frame is `advance(1 / fps)`, then a draw
//! through a ratatui `Terminal` over a `CrosstermBackend` that writes into memory. Ratatui sends
//! only the cells that changed since the last frame, so each frame's bytes are small; they become
//! one `"o"` event stamped with that frame's virtual time. Nothing here reads the wall clock, the
//! environment or a terminal (not even its size: the viewport is [`Viewport::Fixed`]), so the
//! same log, size, pace, fps and colour depth give a byte-identical file.
//!
//! Turning it into a GIF or video is the user's step (`agg OUT.cast OUT.gif`, or
//! `asciinema play OUT.cast`); nothing here shells out.

use std::cell::RefCell;
use std::io::{self, Write};
use std::path::Path;
use std::rc::Rc;

use ratatui::backend::CrosstermBackend;
use ratatui::layout::Rect;
use ratatui::{Terminal, TerminalOptions, Viewport};

use crate::color::{ColorDepth, Palette};
use crate::watch::{self, Watcher};

/// `--size` when `--record` doesn't give one: a studio-sized terminal
/// (`data/tui.toml`'s `[studio]` threshold fits it).
pub const DEFAULT_SIZE: (u16, u16) = (160, 45);
/// `--fps` when `--record` doesn't give one.
pub const DEFAULT_FPS: u32 = 15;
/// The most `--fps` takes.
pub const MAX_FPS: u32 = 60;

/// What to record.
#[derive(Clone, Debug)]
pub struct Options {
    pub size: (u16, u16),
    /// The watch pace to replay at, one of [`watch::PACES`]; `None` is the watcher's default.
    pub pace: Option<u32>,
    pub fps: u32,
    pub ascii: bool,
    pub depth: ColorDepth,
}

/// An in-memory terminal output that [`record_text`] can read back while the `Terminal` still
/// owns the backend.
#[derive(Clone, Default)]
struct Sink(Rc<RefCell<Vec<u8>>>);

impl Write for Sink {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.0.borrow_mut().extend_from_slice(buf);
        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

/// Records the run log at `log` to `out`.
pub fn record_file(log: &Path, out: &Path, options: &Options) -> Result<(), String> {
    let text = std::fs::read_to_string(log)
        .map_err(|e| format!("can't read the run log {}: {e}", log.display()))?;
    let cast = record_text(&text, options)?;
    std::fs::write(out, cast).map_err(|e| format!("can't write {}: {e}", out.display()))
}

/// The asciicast v2 text for the run log `text`: a header line, then one event line per frame
/// that changed anything. Stops once the log is fully replayed (its `end` line included, when it
/// has one).
pub fn record_text(text: &str, options: &Options) -> Result<String, String> {
    let (width, height) = options.size;
    let fps = options.fps.clamp(1, MAX_FPS);
    let (header, events) = watch::parse(text)?;
    let mut watcher = Watcher::new(&header)?;
    if let Some(pace) = options.pace {
        while watcher.pace() < pace {
            watcher.faster();
        }
    }
    watcher.feed(events);
    watcher.resize(width, height);
    watcher.set_frame_size(width, height);
    let palette = Palette::new(watcher.defs(), watcher.nation(), options.depth, options.ascii);

    // crossterm drops colour escapes when NO_COLOR is set; the palette already decides colour
    // (`--color never`), so the environment must not change the file.
    crossterm::style::force_color_output(true);
    let sink = Sink::default();
    let mut terminal = Terminal::with_options(
        CrosstermBackend::new(sink.clone()),
        TerminalOptions { viewport: Viewport::Fixed(Rect::new(0, 0, width, height)) },
    )
    .map_err(|e| e.to_string())?;

    let header_line = serde_json::json!({
        "version": 2,
        "width": width,
        "height": height,
        "title": format!("The Four Nations: seed {} {}", header.seed, header.nation.as_str()),
    });
    let mut out = header_line.to_string();
    out.push('\n');

    // The first event clears the screen and hides the cursor, so the file plays the same in a
    // terminal that wasn't blank.
    let mut frame_bytes = b"\x1b[2J\x1b[H\x1b[?25l".to_vec();
    let mut frame = 0u64;
    loop {
        terminal
            .draw(|f| {
                let buf = watcher.render(&palette, (width, height));
                f.buffer_mut().clone_from(&buf);
            })
            .map_err(|e| e.to_string())?;
        frame_bytes.append(&mut sink.0.borrow_mut());
        if !frame_bytes.is_empty() {
            push_event(&mut out, frame, fps, &frame_bytes);
            frame_bytes.clear();
        }
        if watcher.caught_up() {
            break;
        }
        watcher.advance(1.0 / f64::from(fps));
        frame += 1;
    }
    Ok(out)
}

/// One `[time, "o", data]` line. Time is integer microseconds formatted by hand, so no float
/// formatting is involved.
fn push_event(out: &mut String, frame: u64, fps: u32, bytes: &[u8]) {
    let micros = frame * 1_000_000 / u64::from(fps);
    let data = serde_json::Value::String(String::from_utf8_lossy(bytes).into_owned());
    out.push_str(&format!("[{}.{:06}, \"o\", {data}]\n", micros / 1_000_000, micros % 1_000_000));
}

#[cfg(test)]
mod tests {
    use fn_sim::Nation;

    use super::*;
    use crate::surface::Host;

    /// A short run log: a few acts and turns.
    fn log_text() -> String {
        let mut h = Host::new(7, Nation::Earth, None, None, None, false).unwrap();
        h.press_keys("h <right*3> <enter>", Some("harvest".into()), None).unwrap();
        h.end_turn(1);
        h.press_keys("<left*2>", None, None).unwrap();
        h.end_turn(2);
        h.log().to_jsonl(&h.header())
    }

    fn options() -> Options {
        Options {
            size: (160, 45),
            pace: Some(10),
            fps: 15,
            ascii: false,
            depth: ColorDepth::TrueColor,
        }
    }

    #[test]
    fn two_recordings_of_one_log_are_byte_identical() {
        let text = log_text();
        let a = record_text(&text, &options()).unwrap();
        let b = record_text(&text, &options()).unwrap();
        assert_eq!(a, b);
    }

    #[test]
    fn the_header_matches_the_size_and_every_event_is_well_formed() {
        let cast = record_text(&log_text(), &Options { size: (120, 40), ..options() }).unwrap();
        let mut lines = cast.lines();
        let header: serde_json::Value = serde_json::from_str(lines.next().unwrap()).unwrap();
        assert_eq!(header["version"], 2);
        assert_eq!((header["width"].as_u64(), header["height"].as_u64()), (Some(120), Some(40)));
        let mut last = -1.0;
        let mut n = 0;
        for line in lines {
            let event: serde_json::Value = serde_json::from_str(line).unwrap();
            let (t, kind, data) = (event[0].as_f64().unwrap(), &event[1], &event[2]);
            assert_eq!(kind, "o");
            assert!(data.as_str().is_some_and(|s| !s.is_empty()));
            assert!(t > last, "events are in strictly increasing time order");
            last = t;
            n += 1;
        }
        assert!(n > 1, "a replay with acts is more than one frame");
    }

    #[test]
    fn it_ends_when_the_log_is_replayed_and_a_faster_pace_is_shorter() {
        let text = log_text();
        let slow = record_text(&text, &Options { pace: Some(1), ..options() }).unwrap();
        let fast = record_text(&text, &Options { pace: Some(40), ..options() }).unwrap();
        assert!(fast.lines().count() < slow.lines().count());
    }

    #[test]
    fn the_first_event_clears_the_screen() {
        let cast = record_text(&log_text(), &options()).unwrap();
        let first: serde_json::Value = serde_json::from_str(cast.lines().nth(1).unwrap()).unwrap();
        assert_eq!(first[0].as_f64(), Some(0.0));
        assert!(first[2].as_str().unwrap().starts_with("\x1b[2J\x1b[H\x1b[?25l"));
    }

    /// Colour is the palette's call, not the environment's: `--color never` has no colour
    /// escapes, truecolor has 24-bit ones.
    #[test]
    fn colour_follows_the_depth_option() {
        let text = log_text();
        let plain = record_text(&text, &Options { depth: ColorDepth::Never, ..options() }).unwrap();
        let rich = record_text(&text, &options()).unwrap();
        assert!(!plain.contains("38;2;"));
        assert!(rich.contains("38;2;"));
    }
}
