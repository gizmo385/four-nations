//! The key-event types every frontend hands to the game (#594, #857).
//!
//! They are frontend-neutral: the same shapes crossterm names (`KeyCode`, `KeyEvent`,
//! `KeyModifiers`, ...) and the methods the game uses, but owned here, so the game's key handling
//! (`crate::app::App::key`, `crate::input`, `crate::pointer`) depends on no terminal library and
//! runs on wasm32. Only the keys the game binds are listed. A terminal frontend converts what it
//! reads (`fn_tui::keyev::from_crossterm`), a window frontend builds events from its own key
//! codes (`fn_gui::keys`).

use std::fmt;
use std::ops::{BitOr, BitOrAssign};

/// A key, as crossterm names it. Only the keys the game binds are listed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum KeyCode {
    Backspace,
    Enter,
    Left,
    Right,
    Up,
    Down,
    Home,
    End,
    PageUp,
    PageDown,
    Tab,
    BackTab,
    Delete,
    Insert,
    F(u8),
    Char(char),
    Null,
    Esc,
}

impl fmt::Display for KeyCode {
    /// The same names crossterm's `Display` prints.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            KeyCode::Backspace => write!(f, "Backspace"),
            KeyCode::Enter => write!(f, "Enter"),
            KeyCode::Left => write!(f, "Left"),
            KeyCode::Right => write!(f, "Right"),
            KeyCode::Up => write!(f, "Up"),
            KeyCode::Down => write!(f, "Down"),
            KeyCode::Home => write!(f, "Home"),
            KeyCode::End => write!(f, "End"),
            KeyCode::PageUp => write!(f, "Page Up"),
            KeyCode::PageDown => write!(f, "Page Down"),
            KeyCode::Tab => write!(f, "Tab"),
            KeyCode::BackTab => write!(f, "Back Tab"),
            KeyCode::Delete => write!(f, "Del"),
            KeyCode::Insert => write!(f, "Ins"),
            KeyCode::F(n) => write!(f, "F{n}"),
            KeyCode::Char(' ') => write!(f, "Space"),
            KeyCode::Char(c) => write!(f, "{c}"),
            KeyCode::Null => write!(f, "Null"),
            KeyCode::Esc => write!(f, "Esc"),
        }
    }
}

/// A small bit set, shaped like crossterm's `bitflags` type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Default)]
pub struct KeyModifiers(u8);

impl KeyModifiers {
    pub const NONE: Self = Self(0);
    pub const SHIFT: Self = Self(0b0001);
    pub const CONTROL: Self = Self(0b0010);
    pub const ALT: Self = Self(0b0100);
    pub const SUPER: Self = Self(0b0000_1000);
    pub const HYPER: Self = Self(0b0001_0000);
    pub const META: Self = Self(0b0010_0000);

    pub const fn empty() -> Self {
        Self::NONE
    }
    pub const fn bits(self) -> u8 {
        self.0
    }
    pub const fn is_empty(self) -> bool {
        self.0 == 0
    }
    pub const fn contains(self, other: Self) -> bool {
        self.0 & other.0 == other.0
    }
    pub const fn intersects(self, other: Self) -> bool {
        self.0 & other.0 != 0
    }
    pub fn insert(&mut self, other: Self) {
        self.0 |= other.0;
    }
    pub fn remove(&mut self, other: Self) {
        self.0 &= !other.0;
    }
}

impl BitOr for KeyModifiers {
    type Output = Self;
    fn bitor(self, rhs: Self) -> Self {
        Self(self.0 | rhs.0)
    }
}

impl BitOrAssign for KeyModifiers {
    fn bitor_assign(&mut self, rhs: Self) {
        self.0 |= rhs.0;
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum KeyEventKind {
    Press,
    Repeat,
    Release,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Default)]
pub struct KeyEventState(u8);

impl KeyEventState {
    pub const NONE: Self = Self(0);
    pub const KEYPAD: Self = Self(0b0001);
    pub const CAPS_LOCK: Self = Self(0b0010);
    pub const NUM_LOCK: Self = Self(0b0100);

    pub const fn empty() -> Self {
        Self::NONE
    }
    pub const fn is_empty(self) -> bool {
        self.0 == 0
    }
    pub const fn contains(self, other: Self) -> bool {
        self.0 & other.0 == other.0
    }
}

impl BitOr for KeyEventState {
    type Output = Self;
    fn bitor(self, rhs: Self) -> Self {
        Self(self.0 | rhs.0)
    }
}

impl BitOrAssign for KeyEventState {
    fn bitor_assign(&mut self, rhs: Self) {
        self.0 |= rhs.0;
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct KeyEvent {
    pub code: KeyCode,
    pub modifiers: KeyModifiers,
    pub kind: KeyEventKind,
    pub state: KeyEventState,
}

impl KeyEvent {
    pub const fn new(code: KeyCode, modifiers: KeyModifiers) -> Self {
        Self::new_with_kind(code, modifiers, KeyEventKind::Press)
    }
    pub const fn new_with_kind(code: KeyCode, modifiers: KeyModifiers, kind: KeyEventKind) -> Self {
        Self { code, modifiers, kind, state: KeyEventState::NONE }
    }
    pub const fn is_press(&self) -> bool {
        matches!(self.kind, KeyEventKind::Press)
    }
    pub const fn is_release(&self) -> bool {
        matches!(self.kind, KeyEventKind::Release)
    }
}

impl From<KeyCode> for KeyEvent {
    fn from(code: KeyCode) -> Self {
        Self::new(code, KeyModifiers::NONE)
    }
}
