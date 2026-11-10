//! The C ABI the page (`web/replay.js`) calls (#595). There is one replay at a time, held here.
//! Hand-written rather than wasm-bindgen so the build needs only the wasm32 target, not a
//! matching `wasm-bindgen-cli`, and the page's glue is a few dozen lines it owns.
//!
//! Text goes in through [`fw_input`] (the page writes UTF-8 into the buffer it returns) and out
//! through [`fw_text_ptr`]/[`fw_text_len`] (set by whichever call last produced text). A frame
//! comes back from [`fw_render`] as a pointer to [`crate::encode`]'s cell array, valid until the
//! next call. A panic traps; the page then reads its message with [`fw_panic`].
//!
//! The timeline's scan ([`crate::scan`]) has calls of its own (`fw_scan_*`, [`fw_markers`]). The
//! page makes them on a second instance, in a worker, so the scan never holds up the replay.

use std::cell::RefCell;

use crate::scan::Scan;
use crate::{Replay, encode, key_from_codes};

#[derive(Default)]
struct State {
    replay: Option<Replay>,
    scan: Option<Scan>,
    input: Vec<u8>,
    text: String,
    frame: Vec<u8>,
}

thread_local! {
    static STATE: RefCell<State> = RefCell::new(State::default());
    static PANIC: RefCell<String> = const { RefCell::new(String::new()) };
}

fn with<R>(f: impl FnOnce(&mut State) -> R) -> R {
    STATE.with(|s| f(&mut s.borrow_mut()))
}

fn with_replay<R: Default>(f: impl FnOnce(&mut Replay) -> R) -> R {
    with(|s| s.replay.as_mut().map(f).unwrap_or_default())
}

/// A buffer of `len` bytes for the page to write a log's text into before [`fw_open`].
#[unsafe(no_mangle)]
pub extern "C" fn fw_input(len: usize) -> *mut u8 {
    std::panic::set_hook(Box::new(|info| PANIC.with(|p| *p.borrow_mut() = info.to_string())));
    with(|s| {
        s.input = vec![0; len];
        s.input.as_mut_ptr()
    })
}

/// Opens the log in the input buffer. 1 on success; 0 on failure, with the reason as the text.
#[unsafe(no_mangle)]
pub extern "C" fn fw_open() -> u32 {
    with(|s| {
        let input = std::mem::take(&mut s.input);
        let result = std::str::from_utf8(&input)
            .map_err(|e| format!("the log isn't UTF-8: {e}"))
            .and_then(Replay::new);
        match result {
            Ok(replay) => {
                s.text = replay.build().unwrap_or("").to_owned();
                s.replay = Some(replay);
                1
            }
            Err(e) => {
                s.text = e;
                s.replay = None;
                0
            }
        }
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn fw_text_ptr() -> *const u8 {
    with(|s| s.text.as_ptr())
}

#[unsafe(no_mangle)]
pub extern "C" fn fw_text_len() -> usize {
    with(|s| s.text.len())
}

/// Puts the last panic's message in the text; its length.
#[unsafe(no_mangle)]
pub extern "C" fn fw_panic() -> usize {
    let message = PANIC.with(|p| p.borrow().clone());
    // A panic can leave `STATE` borrowed; there's nothing left to save then.
    STATE.with(|s| match s.try_borrow_mut() {
        Ok(mut s) => {
            s.text = message;
            s.text.len()
        }
        Err(_) => 0,
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn fw_advance(real_secs: f64) {
    with_replay(|r| r.advance(real_secs));
}

/// One key: `code` and `mods` as [`key_from_codes`] reads them.
#[unsafe(no_mangle)]
pub extern "C" fn fw_key(code: u32, mods: u32) {
    if let Some(key) = key_from_codes(code, mods) {
        with_replay(|r| r.key(key));
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn fw_seek(turn: u32) {
    with_replay(|r| r.seek(turn));
}

/// One turn of a seek; 1 while there's further to go.
#[unsafe(no_mangle)]
pub extern "C" fn fw_seek_step() -> u32 {
    with_replay(|r| u32::from(r.seek_step()))
}

/// The frame at `width` x `height`, as [`encode`]'s cell array of `width * height` cells.
#[unsafe(no_mangle)]
pub extern "C" fn fw_render(width: u16, height: u16) -> *const u8 {
    with(|s| {
        if let Some(replay) = s.replay.as_mut() {
            let buf = replay.render(width, height);
            encode::encode(&buf, &mut s.frame);
        } else {
            s.frame.clear();
        }
        s.frame.as_ptr()
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn fw_turn() -> u32 {
    with_replay(|r| r.turn())
}

#[unsafe(no_mangle)]
pub extern "C" fn fw_total_turns() -> u32 {
    with_replay(|r| r.total_turns())
}

/// Bit 0 paused, bit 1 seeking, bit 2 at the end of the log.
#[unsafe(no_mangle)]
pub extern "C" fn fw_flags() -> u32 {
    with_replay(|r| {
        u32::from(r.paused()) | (u32::from(r.seeking()) << 1) | (u32::from(r.at_end()) << 2)
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn fw_pace() -> u32 {
    with_replay(|r| r.pace())
}

/// Opens the log in the input buffer for a scan, as [`fw_open`] does for a replay: 1 on
/// success; 0 on failure, with the reason as the text.
#[unsafe(no_mangle)]
pub extern "C" fn fw_scan_open() -> u32 {
    with(|s| {
        let input = std::mem::take(&mut s.input);
        let result = std::str::from_utf8(&input)
            .map_err(|e| format!("the log isn't UTF-8: {e}"))
            .and_then(Scan::new);
        match result {
            Ok(scan) => {
                s.scan = Some(scan);
                1
            }
            Err(e) => {
                s.text = e;
                s.scan = None;
                0
            }
        }
    })
}

/// One more turn of the scan; 1 while there's further to go.
#[unsafe(no_mangle)]
pub extern "C" fn fw_scan_step() -> u32 {
    with(|s| s.scan.as_mut().map_or(0, |scan| u32::from(scan.step())))
}

/// The turn the scan has reached.
#[unsafe(no_mangle)]
pub extern "C" fn fw_scan_turn() -> u32 {
    with(|s| s.scan.as_ref().map_or(0, Scan::turn))
}

/// How many markers the scan has found.
#[unsafe(no_mangle)]
pub extern "C" fn fw_marker_count() -> u32 {
    with(|s| s.scan.as_ref().map_or(0, |scan| scan.markers().len() as u32))
}

/// Puts the markers from the `from`th on in the text, one a line as `turn<TAB>kind<TAB>text`
/// (`kind` as [`crate::scan::MarkerKind::as_str`] names it); the text's length.
#[unsafe(no_mangle)]
pub extern "C" fn fw_markers(from: u32) -> usize {
    with(|s| {
        let mut text = String::new();
        if let Some(scan) = &s.scan {
            for m in scan.markers().iter().skip(from as usize) {
                let line = m.text.replace(['\t', '\n'], " ");
                text.push_str(&format!("{}\t{}\t{line}\n", m.turn, m.kind.as_str()));
            }
        }
        s.text = text;
        s.text.len()
    })
}
