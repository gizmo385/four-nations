//! The web replayer (#595, docs/design/agent-play.md, "Watching on the web"): `fn_tui`'s
//! [`Replay`] compiled to wasm32, so a browser replays a `run.jsonl` the same way
//! `cargo xtask agent-run watch` does in a terminal -- by re-simulating it from its seed and
//! pressing the logged keys, not by playing back a recording.
//!
//! [`Replay`] and the timeline's [`scan`] live in `fn_tui::replay` (#995); this crate is the
//! page's side of them. [`abi`] is the thin C ABI the page calls (no wasm-bindgen, so the build
//! needs nothing beyond the `wasm32-unknown-unknown` target), and [`encode`] turns a frame into
//! the flat cell array the page draws onto a canvas.

pub mod abi;
pub mod encode;

pub use fn_tui::replay::{Replay, SNAPSHOT_EVERY, START_PACE, scan};

use fn_tui::keyev::{KeyCode, KeyEvent, KeyModifiers};

/// The key a page sends as `code` (a Unicode scalar, or one of these above the Unicode range)
/// and `mods` (bit 0 Shift, bit 1 Ctrl, bit 2 Alt). `None` for a code that's neither.
pub fn key_from_codes(code: u32, mods: u32) -> Option<KeyEvent> {
    const SPECIAL: u32 = 0x11_0000;
    let code = match code {
        c if c < SPECIAL => KeyCode::Char(char::from_u32(c)?),
        c => match c - SPECIAL {
            0 => KeyCode::Enter,
            1 => KeyCode::Esc,
            2 => KeyCode::Left,
            3 => KeyCode::Right,
            4 => KeyCode::Up,
            5 => KeyCode::Down,
            6 => KeyCode::Tab,
            7 => KeyCode::BackTab,
            8 => KeyCode::Backspace,
            9 => KeyCode::Home,
            10 => KeyCode::End,
            11 => KeyCode::PageUp,
            12 => KeyCode::PageDown,
            13 => KeyCode::Delete,
            _ => return None,
        },
    };
    let mut m = KeyModifiers::NONE;
    if mods & 1 != 0 {
        m |= KeyModifiers::SHIFT;
    }
    if mods & 2 != 0 {
        m |= KeyModifiers::CONTROL;
    }
    if mods & 4 != 0 {
        m |= KeyModifiers::ALT;
    }
    Some(KeyEvent::new(code, m))
}
