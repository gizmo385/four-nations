//! egui key events become the terminal's.

use eframe::egui::{Event, Key, Modifiers};
use fn_gui::keys::translate;
use fn_ui::keyev::{KeyCode, KeyModifiers};

fn key(k: Key, m: Modifiers) -> Event {
    Event::Key { key: k, physical_key: None, pressed: true, repeat: false, modifiers: m }
}

#[test]
fn named_keys_text_and_ctrl_chords_translate() {
    let enter = translate(&key(Key::Enter, Modifiers::NONE)).unwrap();
    assert_eq!(enter.code, KeyCode::Enter);
    assert_eq!(translate(&key(Key::F5, Modifiers::NONE)).unwrap().code, KeyCode::F(5));
    let back = translate(&key(Key::Tab, Modifiers::SHIFT)).unwrap();
    assert_eq!(back.code, KeyCode::BackTab);

    // A plain letter arrives as text, not as a key, so it isn't pressed twice.
    assert!(translate(&key(Key::H, Modifiers::NONE)).is_none());
    let h = translate(&Event::Text("h".into())).unwrap();
    assert_eq!((h.code, h.modifiers), (KeyCode::Char('h'), KeyModifiers::NONE));
    let big = translate(&Event::Text("S".into())).unwrap();
    assert_eq!((big.code, big.modifiers), (KeyCode::Char('S'), KeyModifiers::SHIFT));

    let ctrl_c = translate(&key(Key::C, Modifiers::CTRL)).unwrap();
    assert_eq!((ctrl_c.code, ctrl_c.modifiers), (KeyCode::Char('c'), KeyModifiers::CONTROL));
    let ctrl_left = translate(&key(Key::ArrowLeft, Modifiers::CTRL)).unwrap();
    assert_eq!((ctrl_left.code, ctrl_left.modifiers), (KeyCode::Left, KeyModifiers::CONTROL));
}
