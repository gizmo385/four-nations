//! egui's key events as the terminal's (`fn_ui::keyev`), so the shell can't tell a window from a
//! terminal. Printable characters come from egui's `Text` events (they carry the shifted
//! character and the keyboard layout); named keys, and letters held with Ctrl or Alt (which egui
//! sends no text for), come from `Key` events.

use eframe::egui::{self, Event, Key, Modifiers};
use fn_ui::keyev::{KeyCode, KeyEvent, KeyModifiers};

fn mods(m: Modifiers) -> KeyModifiers {
    let mut out = KeyModifiers::NONE;
    if m.shift {
        out |= KeyModifiers::SHIFT;
    }
    if m.ctrl || m.command && cfg!(not(target_os = "macos")) {
        out |= KeyModifiers::CONTROL;
    }
    if m.alt {
        out |= KeyModifiers::ALT;
    }
    out
}

/// A named key, or `None` for one that arrives as text instead.
fn named(key: Key) -> Option<KeyCode> {
    Some(match key {
        Key::Enter => KeyCode::Enter,
        Key::Escape => KeyCode::Esc,
        Key::ArrowLeft => KeyCode::Left,
        Key::ArrowRight => KeyCode::Right,
        Key::ArrowUp => KeyCode::Up,
        Key::ArrowDown => KeyCode::Down,
        Key::Tab => KeyCode::Tab,
        Key::Backspace => KeyCode::Backspace,
        Key::Delete => KeyCode::Delete,
        Key::Insert => KeyCode::Insert,
        Key::Home => KeyCode::Home,
        Key::End => KeyCode::End,
        Key::PageUp => KeyCode::PageUp,
        Key::PageDown => KeyCode::PageDown,
        Key::F1 => KeyCode::F(1),
        Key::F2 => KeyCode::F(2),
        Key::F3 => KeyCode::F(3),
        Key::F4 => KeyCode::F(4),
        Key::F5 => KeyCode::F(5),
        Key::F6 => KeyCode::F(6),
        Key::F7 => KeyCode::F(7),
        Key::F8 => KeyCode::F(8),
        Key::F9 => KeyCode::F(9),
        Key::F10 => KeyCode::F(10),
        Key::F11 => KeyCode::F(11),
        Key::F12 => KeyCode::F(12),
        _ => return None,
    })
}

/// The letter or digit a key names, for Ctrl and Alt chords.
fn chord_char(key: Key) -> Option<char> {
    let name = key.name();
    let mut chars = name.chars();
    let c = chars.next()?;
    (chars.next().is_none() && c.is_ascii_alphanumeric()).then(|| c.to_ascii_lowercase())
}

/// The terminal keys `event` stands for (none for most events).
pub fn translate(event: &Event) -> Option<KeyEvent> {
    match event {
        Event::Key { key, pressed: true, modifiers, .. } => {
            if let Some(code) = named(*key) {
                let code =
                    if code == KeyCode::Tab && modifiers.shift { KeyCode::BackTab } else { code };
                return Some(KeyEvent::new(code, mods(*modifiers)));
            }
            if modifiers.ctrl || modifiers.alt {
                return chord_char(*key).map(|c| KeyEvent::new(KeyCode::Char(c), mods(*modifiers)));
            }
            None
        }
        Event::Text(text) => {
            let mut chars = text.chars();
            let c = chars.next()?;
            if chars.next().is_some() {
                return None;
            }
            let m = if c.is_uppercase() { KeyModifiers::SHIFT } else { KeyModifiers::NONE };
            Some(KeyEvent::new(KeyCode::Char(c), m))
        }
        _ => None,
    }
}

/// Whether `key` is typing into a text field: a character without Ctrl or Alt, or `Backspace`.
pub fn is_typing(key: &KeyEvent) -> bool {
    match key.code {
        KeyCode::Backspace => true,
        KeyCode::Char(_) => !key.modifiers.intersects(KeyModifiers::CONTROL | KeyModifiers::ALT),
        _ => false,
    }
}

/// Every terminal key this frame's input holds, in order.
pub fn frame_keys(input: &egui::InputState) -> Vec<KeyEvent> {
    input.events.iter().filter_map(translate).collect()
}
