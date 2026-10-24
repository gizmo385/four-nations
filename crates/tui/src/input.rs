//! Keys to actions, and actions to changes of the app (docs/design/tui.md, "Input").
//!
//! Mapping a key is a pure function, so the tests cover the whole table. The selection and panel
//! keys (#99) and the research screen (#100) add their actions to
//! [`Action`] and their keys to [`map_key`].

use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyEventState, KeyModifiers};
use fn_ui::Tool;

/// How far Shift moves the cursor.
pub const SHIFT_STEP: i32 = 10;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action {
    /// Move the cursor `(dx, dy)` tiles; each of `dx` and `dy` is -1, 0 or 1, times `tiles`.
    Move {
        dx: i32,
        dy: i32,
        tiles: i32,
    },
    /// Jump the view a quarter screen (`dx`, `dy` in -1..=1) and carry the cursor.
    Jump {
        dx: i32,
        dy: i32,
    },
    /// `H` `X` `P` or `B`: switch to a designation tool (dropping the corner).
    PickTool(Tool),
    /// `Enter`: in a tool, set a corner or apply the rectangle. (Selecting is #99.)
    Confirm,
    /// `Esc`: drop the corner, or with none leave the tool.
    Cancel,
    TogglePause,
    Faster,
    Slower,
    /// One tick, while paused.
    Step,
    /// Ask whether to quit.
    AskQuit,
    /// Quit at once (Ctrl+C: raw mode turns the signal into a key).
    QuitNow,
}

/// What a key means when no prompt is open, if anything.
pub fn map_key(key: KeyEvent) -> Option<Action> {
    if key.kind == KeyEventKind::Release {
        return None;
    }
    let KeyEvent { code, modifiers, state, .. } = key;
    let ctrl = modifiers.contains(KeyModifiers::CONTROL);
    let shift = modifiers.contains(KeyModifiers::SHIFT);
    let step = |upper: bool| if upper { SHIFT_STEP } else { 1 };
    let walk = |dx, dy, upper| Some(Action::Move { dx, dy, tiles: step(upper) });

    match code {
        KeyCode::Char('c') | KeyCode::Char('C') if ctrl => Some(Action::QuitNow),
        // Ctrl and Alt chords are the terminal's and the shell's.
        KeyCode::Char(_) if ctrl || modifiers.contains(KeyModifiers::ALT) => None,

        KeyCode::Up | KeyCode::Down | KeyCode::Left | KeyCode::Right if ctrl => {
            let (dx, dy) = arrow(code);
            Some(Action::Jump { dx, dy })
        }
        KeyCode::Up | KeyCode::Down | KeyCode::Left | KeyCode::Right => {
            let (dx, dy) = arrow(code);
            walk(dx, dy, shift)
        }
        KeyCode::PageUp => Some(Action::Jump { dx: 0, dy: -1 }),
        KeyCode::PageDown => Some(Action::Jump { dx: 0, dy: 1 }),
        // Numpad 7 and 1 with NumLock off arrive as Home and End.
        KeyCode::Home => walk(-1, -1, shift),
        KeyCode::End => walk(-1, 1, shift),

        // W A S D move like the arrows; upper case (Shift, or Caps Lock) moves 10.
        KeyCode::Char(c @ ('w' | 'a' | 's' | 'd' | 'W' | 'A' | 'S' | 'D')) => {
            let (dx, dy) = match c.to_ascii_lowercase() {
                'w' => (0, -1),
                's' => (0, 1),
                'a' => (-1, 0),
                _ => (1, 0),
            };
            walk(dx, dy, c.is_ascii_uppercase() || shift)
        }

        // Numpad digits are only told apart from the number row on terminals that report
        // keypad keys (the kitty keyboard protocol); the number row is for later.
        KeyCode::Char(c @ '1'..='9') if state.contains(KeyEventState::KEYPAD) => {
            let (dx, dy) = match c {
                '7' => (-1, -1),
                '8' => (0, -1),
                '9' => (1, -1),
                '4' => (-1, 0),
                '6' => (1, 0),
                '1' => (-1, 1),
                '2' => (0, 1),
                '3' => (1, 1),
                _ => return None,
            };
            walk(dx, dy, shift)
        }

        // The tools, in either case. `U` (build) is not here: it needs a picker (follow-up).
        KeyCode::Char(c @ ('h' | 'H' | 'x' | 'X' | 'p' | 'P' | 'b' | 'B')) => {
            Tool::from_letter(c).map(Action::PickTool)
        }
        KeyCode::Enter => Some(Action::Confirm),
        KeyCode::Esc => Some(Action::Cancel),

        KeyCode::Char(' ') => Some(Action::TogglePause),
        KeyCode::Char('+' | '=') => Some(Action::Faster),
        KeyCode::Char('-' | '_') => Some(Action::Slower),
        KeyCode::Char('.') => Some(Action::Step),
        KeyCode::Char('q' | 'Q') => Some(Action::AskQuit),
        _ => None,
    }
}

fn arrow(code: KeyCode) -> (i32, i32) {
    match code {
        KeyCode::Up => (0, -1),
        KeyCode::Down => (0, 1),
        KeyCode::Left => (-1, 0),
        _ => (1, 0),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    fn with(code: KeyCode, modifiers: KeyModifiers) -> KeyEvent {
        KeyEvent::new(code, modifiers)
    }

    const UP: Option<Action> = Some(Action::Move { dx: 0, dy: -1, tiles: 1 });
    const UP10: Option<Action> = Some(Action::Move { dx: 0, dy: -1, tiles: 10 });

    #[test]
    fn wasd_and_the_arrows_move_one_tile() {
        assert_eq!(map_key(key(KeyCode::Up)), UP);
        assert_eq!(map_key(key(KeyCode::Char('w'))), UP);
        assert_eq!(map_key(key(KeyCode::Down)), Some(Action::Move { dx: 0, dy: 1, tiles: 1 }));
        assert_eq!(
            map_key(key(KeyCode::Char('a'))),
            Some(Action::Move { dx: -1, dy: 0, tiles: 1 })
        );
        assert_eq!(map_key(key(KeyCode::Char('d'))), Some(Action::Move { dx: 1, dy: 0, tiles: 1 }));
    }

    #[test]
    fn shift_moves_ten_and_so_does_an_upper_case_letter() {
        assert_eq!(map_key(with(KeyCode::Up, KeyModifiers::SHIFT)), UP10);
        // Terminals send Shift+w as an upper-case W, with or without the Shift flag.
        assert_eq!(map_key(key(KeyCode::Char('W'))), UP10);
        assert_eq!(map_key(with(KeyCode::Char('W'), KeyModifiers::SHIFT)), UP10);
        assert_eq!(
            map_key(with(KeyCode::Right, KeyModifiers::SHIFT)),
            Some(Action::Move { dx: 1, dy: 0, tiles: 10 })
        );
    }

    #[test]
    fn ctrl_arrows_and_the_page_keys_jump() {
        assert_eq!(
            map_key(with(KeyCode::Left, KeyModifiers::CONTROL)),
            Some(Action::Jump { dx: -1, dy: 0 })
        );
        assert_eq!(map_key(key(KeyCode::PageUp)), Some(Action::Jump { dx: 0, dy: -1 }));
        assert_eq!(map_key(key(KeyCode::PageDown)), Some(Action::Jump { dx: 0, dy: 1 }));
    }

    #[test]
    fn the_numpad_moves_with_diagonals() {
        let pad = |c| KeyEvent {
            code: KeyCode::Char(c),
            modifiers: KeyModifiers::NONE,
            kind: KeyEventKind::Press,
            state: KeyEventState::KEYPAD,
        };
        assert_eq!(map_key(pad('7')), Some(Action::Move { dx: -1, dy: -1, tiles: 1 }));
        assert_eq!(map_key(pad('3')), Some(Action::Move { dx: 1, dy: 1, tiles: 1 }));
        assert_eq!(map_key(pad('8')), UP);
        assert_eq!(map_key(pad('5')), None);
        // NumLock off: 7 and 1 are Home and End.
        assert_eq!(map_key(key(KeyCode::Home)), Some(Action::Move { dx: -1, dy: -1, tiles: 1 }));
        assert_eq!(map_key(key(KeyCode::End)), Some(Action::Move { dx: -1, dy: 1, tiles: 1 }));
        // The number row is not the numpad.
        assert_eq!(map_key(key(KeyCode::Char('7'))), None);
    }

    #[test]
    fn ctrl_and_alt_chords_are_left_to_the_terminal() {
        assert_eq!(map_key(with(KeyCode::Char('s'), KeyModifiers::CONTROL)), None);
        assert_eq!(map_key(with(KeyCode::Char('d'), KeyModifiers::CONTROL)), None);
        assert_eq!(map_key(with(KeyCode::Char('w'), KeyModifiers::ALT)), None);
        assert_eq!(map_key(with(KeyCode::Char('c'), KeyModifiers::CONTROL)), Some(Action::QuitNow));
    }

    #[test]
    fn the_clock_keys() {
        assert_eq!(map_key(key(KeyCode::Char(' '))), Some(Action::TogglePause));
        assert_eq!(map_key(key(KeyCode::Char('+'))), Some(Action::Faster));
        assert_eq!(map_key(key(KeyCode::Char('='))), Some(Action::Faster));
        assert_eq!(map_key(key(KeyCode::Char('-'))), Some(Action::Slower));
        assert_eq!(map_key(key(KeyCode::Char('.'))), Some(Action::Step));
        assert_eq!(map_key(key(KeyCode::Char('q'))), Some(Action::AskQuit));
        assert_eq!(map_key(key(KeyCode::Char('Q'))), Some(Action::AskQuit));
        assert_eq!(map_key(key(KeyCode::F(5))), None);
    }

    #[test]
    fn the_tool_keys_ignore_case_and_enter_and_esc_drive_the_rectangle() {
        for (c, tool) in [
            ('h', Tool::Harvest),
            ('x', Tool::CancelHarvest),
            ('p', Tool::Stockpile),
            ('b', Tool::Bend),
        ] {
            let want = Some(Action::PickTool(tool));
            assert_eq!(map_key(key(KeyCode::Char(c))), want);
            assert_eq!(map_key(key(KeyCode::Char(c.to_ascii_uppercase()))), want);
            assert_eq!(
                map_key(with(KeyCode::Char(c.to_ascii_uppercase()), KeyModifiers::SHIFT)),
                want
            );
            assert_eq!(map_key(with(KeyCode::Char(c), KeyModifiers::CONTROL)), None);
        }
        // The build tool needs a picker, which comes with its own issue.
        assert_eq!(map_key(key(KeyCode::Char('u'))), None);
        assert_eq!(map_key(key(KeyCode::Enter)), Some(Action::Confirm));
        assert_eq!(map_key(key(KeyCode::Esc)), Some(Action::Cancel));
    }

    #[test]
    fn key_releases_do_nothing() {
        let release = KeyEvent {
            code: KeyCode::Up,
            modifiers: KeyModifiers::NONE,
            kind: KeyEventKind::Release,
            state: KeyEventState::NONE,
        };
        assert_eq!(map_key(release), None);
    }
}
