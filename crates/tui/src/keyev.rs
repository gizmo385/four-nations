//! The key-event types every screen reads (#594, #857).
//!
//! The types are `fn_ui::keyev`'s, re-exported: frontend-neutral, with crossterm's names, fields
//! and the methods the game uses, so the game's key handling (`fn_ui::app::App`) runs on wasm32
//! (the web replayer, #595) and in a window (`fn_gui`) as well as in a terminal. On a native
//! build [`from_crossterm`] turns what crossterm reads into them; nothing else in this crate
//! touches crossterm's key types, so import key types from here, never from `crossterm::event`
//! directly, or the wasm build breaks (`cargo xtask ci` builds it).

pub use fn_ui::keyev::{KeyCode, KeyEvent, KeyEventKind, KeyEventState, KeyModifiers};

/// A key crossterm read, as the game's own key type. Keys the game never binds (media keys, lone
/// modifiers) become [`KeyCode::Null`].
#[cfg(not(target_arch = "wasm32"))]
pub fn from_crossterm(key: crossterm::event::KeyEvent) -> KeyEvent {
    use crossterm::event as ct;

    let code = match key.code {
        ct::KeyCode::Backspace => KeyCode::Backspace,
        ct::KeyCode::Enter => KeyCode::Enter,
        ct::KeyCode::Left => KeyCode::Left,
        ct::KeyCode::Right => KeyCode::Right,
        ct::KeyCode::Up => KeyCode::Up,
        ct::KeyCode::Down => KeyCode::Down,
        ct::KeyCode::Home => KeyCode::Home,
        ct::KeyCode::End => KeyCode::End,
        ct::KeyCode::PageUp => KeyCode::PageUp,
        ct::KeyCode::PageDown => KeyCode::PageDown,
        ct::KeyCode::Tab => KeyCode::Tab,
        ct::KeyCode::BackTab => KeyCode::BackTab,
        ct::KeyCode::Delete => KeyCode::Delete,
        ct::KeyCode::Insert => KeyCode::Insert,
        ct::KeyCode::F(n) => KeyCode::F(n),
        ct::KeyCode::Char(c) => KeyCode::Char(c),
        ct::KeyCode::Esc => KeyCode::Esc,
        _ => KeyCode::Null,
    };
    let mut modifiers = KeyModifiers::NONE;
    for (from, to) in [
        (ct::KeyModifiers::SHIFT, KeyModifiers::SHIFT),
        (ct::KeyModifiers::CONTROL, KeyModifiers::CONTROL),
        (ct::KeyModifiers::ALT, KeyModifiers::ALT),
        (ct::KeyModifiers::SUPER, KeyModifiers::SUPER),
        (ct::KeyModifiers::HYPER, KeyModifiers::HYPER),
        (ct::KeyModifiers::META, KeyModifiers::META),
    ] {
        if key.modifiers.contains(from) {
            modifiers |= to;
        }
    }
    let kind = match key.kind {
        ct::KeyEventKind::Press => KeyEventKind::Press,
        ct::KeyEventKind::Repeat => KeyEventKind::Repeat,
        ct::KeyEventKind::Release => KeyEventKind::Release,
    };
    let mut state = KeyEventState::NONE;
    for (from, to) in [
        (ct::KeyEventState::KEYPAD, KeyEventState::KEYPAD),
        (ct::KeyEventState::CAPS_LOCK, KeyEventState::CAPS_LOCK),
        (ct::KeyEventState::NUM_LOCK, KeyEventState::NUM_LOCK),
    ] {
        if key.state.contains(from) {
            state |= to;
        }
    }
    KeyEvent { code, modifiers, kind, state }
}
