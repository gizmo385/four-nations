//! Key scripts: the `--keys` of `--frame`, so an agent can play without a terminal
//! (docs/design/tui.md, "Frames for agents").
//!
//! A script is whitespace-separated tokens:
//!
//! * a single character is that key: `h`, `d`, `.`, `+`. An upper-case letter is Shift plus the
//!   letter, as a terminal sends it, so `S` moves the cursor 10 tiles and `s` moves it 1;
//! * `<name>` is a named key: `<enter>`, `<esc>`, `<space>`, `<tab>`, `<s-tab>`, `<up>`, `<down>`,
//!   `<left>`, `<right>`, `<pgup>`, `<pgdn>`, `<home>`, `<end>`, `<bs>`, `<del>`, `<f1>` to
//!   `<f12>`, and `<lt>` and `<gt>` for `<` and `>`. `s-` and `c-` in front add Shift or Ctrl:
//!   `<s-up>`, `<c-left>`, `<c-c>`. Any key may go in the brackets, so `<d>` is `d`;
//! * `<key*N>` presses the key N times: `<right*5>`;
//! * `<tick N>` runs N ticks of the world.
//!
//! A script never reads real time. Time passes only through `<tick N>` (and the `.` key while
//! paused), so a seed plus a script is one game: `h <right*5> <enter> <down*3> <enter> <tick 600>`
//! designates a harvest rectangle 6 wide and 4 tall, then lets the settlers work for a minute.

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

/// The most one `*N` may repeat, so a typo can't ask for a billion keys.
pub const MAX_REPEAT: u32 = 100_000;

/// One step of a script.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Step {
    Key(KeyEvent),
    /// Run this many ticks of the world, whatever the clock says.
    Tick(u32),
}

/// The steps of `script`, or a message saying which token is wrong.
pub fn parse(script: &str) -> Result<Vec<Step>, String> {
    let mut steps = Vec::new();
    for token in tokens(script)? {
        parse_token(&token, &mut steps).map_err(|e| format!("{token:?}: {e}"))?;
    }
    Ok(steps)
}

/// Splits on whitespace, except inside `<...>` (so `<tick 600>` is one token). A lone `<` is the
/// character.
fn tokens(script: &str) -> Result<Vec<String>, String> {
    let mut out = Vec::new();
    let mut chars = script.chars().peekable();
    while let Some(&c) = chars.peek() {
        if c.is_whitespace() {
            chars.next();
            continue;
        }
        let mut token = String::new();
        if c == '<' {
            chars.next();
            token.push('<');
            // `<` followed by a space or the end is the character itself.
            if chars.peek().is_none_or(|n| n.is_whitespace()) {
                out.push(token);
                continue;
            }
            loop {
                match chars.next() {
                    Some('>') => {
                        token.push('>');
                        break;
                    }
                    Some(c) => token.push(c),
                    None => return Err(format!("{token:?}: no closing >")),
                }
            }
        } else {
            while let Some(&c) = chars.peek() {
                if c.is_whitespace() {
                    break;
                }
                token.push(c);
                chars.next();
            }
        }
        out.push(token);
    }
    Ok(out)
}

fn parse_token(token: &str, steps: &mut Vec<Step>) -> Result<(), String> {
    let mut chars = token.chars();
    if let (Some(c), None) = (chars.next(), chars.next()) {
        steps.push(Step::Key(char_key(c)));
        return Ok(());
    }
    // More than one character: only a bracketed token is allowed.
    parse_bracketed(token, steps)
}

fn parse_bracketed(token: &str, steps: &mut Vec<Step>) -> Result<(), String> {
    let Some(inner) = token.strip_prefix('<').and_then(|t| t.strip_suffix('>')) else {
        return Err("a key is one character, or <name> in brackets (<enter>, <right*5>, \
                    <tick 600>)"
            .into());
    };
    let inner = inner.trim();
    if let Some(n) = inner.strip_prefix("tick").filter(|n| n.starts_with(char::is_whitespace)) {
        let n = n.trim();
        let n: u32 = n.parse().map_err(|_| format!("<tick N> wants a whole number, got {n:?}"))?;
        steps.push(Step::Tick(n));
        return Ok(());
    }
    let (name, count) = match inner.rsplit_once('*') {
        // `<*>` is the key `*`, not a repeat of nothing.
        Some((name, n)) if !name.is_empty() => {
            let count: u32 =
                n.trim().parse().map_err(|_| format!("*N wants a whole number, got {n:?}"))?;
            if !(1..=MAX_REPEAT).contains(&count) {
                return Err(format!("*N wants 1 to {MAX_REPEAT}, got {count}"));
            }
            (name.trim(), count)
        }
        _ => (inner, 1),
    };
    let key = named_key(name)?;
    steps.extend(std::iter::repeat_n(Step::Key(key), count as usize));
    Ok(())
}

/// A plain character as a terminal sends it: upper case carries Shift.
fn char_key(c: char) -> KeyEvent {
    let shift = if c.is_ascii_uppercase() { KeyModifiers::SHIFT } else { KeyModifiers::NONE };
    KeyEvent::new(KeyCode::Char(c), shift)
}

/// `name`: optional `s-` and `c-` prefixes, then a key name or one character.
fn named_key(name: &str) -> Result<KeyEvent, String> {
    let mut rest = name;
    let mut modifiers = KeyModifiers::NONE;
    loop {
        let lower = rest.to_ascii_lowercase();
        if lower.starts_with("s-") && rest.len() > 2 {
            modifiers |= KeyModifiers::SHIFT;
        } else if lower.starts_with("c-") && rest.len() > 2 {
            modifiers |= KeyModifiers::CONTROL;
        } else {
            break;
        }
        rest = &rest[2..];
    }
    let lower = rest.to_ascii_lowercase();
    let code = match lower.as_str() {
        "enter" | "return" => KeyCode::Enter,
        "esc" | "escape" => KeyCode::Esc,
        "space" => KeyCode::Char(' '),
        "tab" if modifiers.contains(KeyModifiers::SHIFT) => KeyCode::BackTab,
        "tab" => KeyCode::Tab,
        "up" => KeyCode::Up,
        "down" => KeyCode::Down,
        "left" => KeyCode::Left,
        "right" => KeyCode::Right,
        "pgup" | "pageup" => KeyCode::PageUp,
        "pgdn" | "pagedown" => KeyCode::PageDown,
        "home" => KeyCode::Home,
        "end" => KeyCode::End,
        "bs" | "backspace" => KeyCode::Backspace,
        "del" | "delete" => KeyCode::Delete,
        "lt" => KeyCode::Char('<'),
        "gt" => KeyCode::Char('>'),
        f if f.starts_with('f') && f.len() > 1 && f[1..].bytes().all(|b| b.is_ascii_digit()) => {
            match f[1..].parse() {
                Ok(n @ 1..=12) => KeyCode::F(n),
                _ => return Err(format!("no key named {rest:?} (F1 to F12)")),
            }
        }
        _ => {
            let mut chars = rest.chars();
            match (chars.next(), chars.next()) {
                (Some(c), None) => {
                    // With Ctrl the key is the lower-case letter; with Shift, the upper-case.
                    let c = if modifiers.contains(KeyModifiers::CONTROL) {
                        c.to_ascii_lowercase()
                    } else if modifiers.contains(KeyModifiers::SHIFT) {
                        c.to_ascii_uppercase()
                    } else {
                        c
                    };
                    let shift =
                        c.is_ascii_uppercase() && !modifiers.contains(KeyModifiers::CONTROL);
                    if shift {
                        modifiers |= KeyModifiers::SHIFT;
                    }
                    KeyCode::Char(c)
                }
                _ => return Err(format!("no key named {rest:?}")),
            }
        }
    };
    Ok(KeyEvent::new(code, modifiers))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(code: KeyCode, modifiers: KeyModifiers) -> Step {
        Step::Key(KeyEvent::new(code, modifiers))
    }

    fn plain(c: char) -> Step {
        key(KeyCode::Char(c), KeyModifiers::NONE)
    }

    #[test]
    fn the_documented_example_parses() {
        let steps = parse("h <right*5> <enter> <down*3> <enter> <tick 600>").unwrap();
        let mut want = vec![plain('h')];
        want.extend([key(KeyCode::Right, KeyModifiers::NONE); 5]);
        want.push(key(KeyCode::Enter, KeyModifiers::NONE));
        want.extend([key(KeyCode::Down, KeyModifiers::NONE); 3]);
        want.push(key(KeyCode::Enter, KeyModifiers::NONE));
        want.push(Step::Tick(600));
        assert_eq!(steps, want);
    }

    #[test]
    fn a_character_is_that_key_and_upper_case_carries_shift() {
        assert_eq!(
            parse("d S . + 7").unwrap(),
            vec![
                plain('d'),
                key(KeyCode::Char('S'), KeyModifiers::SHIFT),
                plain('.'),
                plain('+'),
                plain('7'),
            ]
        );
    }

    #[test]
    fn named_keys_and_modifiers() {
        let one = |s: &str| {
            let mut steps = parse(s).unwrap();
            assert_eq!(steps.len(), 1, "{s}");
            steps.remove(0)
        };
        assert_eq!(one("<esc>"), key(KeyCode::Esc, KeyModifiers::NONE));
        assert_eq!(one("<Space>"), plain(' '));
        assert_eq!(one("<s-up>"), key(KeyCode::Up, KeyModifiers::SHIFT));
        assert_eq!(one("<c-left>"), key(KeyCode::Left, KeyModifiers::CONTROL));
        assert_eq!(one("<s-tab>"), key(KeyCode::BackTab, KeyModifiers::SHIFT));
        assert_eq!(one("<pgdn>"), key(KeyCode::PageDown, KeyModifiers::NONE));
        assert_eq!(one("<c-c>"), key(KeyCode::Char('c'), KeyModifiers::CONTROL));
        assert_eq!(one("<s-w>"), key(KeyCode::Char('W'), KeyModifiers::SHIFT));
        assert_eq!(one("<d>"), plain('d'));
        assert_eq!(one("<f5>"), key(KeyCode::F(5), KeyModifiers::NONE));
        assert_eq!(one("<lt>"), plain('<'));
        assert_eq!(one("<*>"), plain('*'));
        assert_eq!(one("<s-right*1>"), key(KeyCode::Right, KeyModifiers::SHIFT));
    }

    #[test]
    fn repeats_and_ticks() {
        assert_eq!(parse("<d*3>").unwrap(), vec![plain('d'); 3]);
        assert_eq!(parse("<tick 600>").unwrap(), vec![Step::Tick(600)]);
        assert_eq!(parse("<tick   0>").unwrap(), vec![Step::Tick(0)]);
    }

    #[test]
    fn whitespace_of_any_kind_separates_and_a_lone_angle_bracket_is_a_key() {
        assert_eq!(parse("  a\n\tb  ").unwrap(), vec![plain('a'), plain('b')]);
        assert_eq!(parse("").unwrap(), vec![]);
        assert_eq!(parse("< >").unwrap(), vec![plain('<'), plain('>')]);
        assert_eq!(parse("<").unwrap(), vec![plain('<')]);
    }

    #[test]
    fn mistakes_name_the_token() {
        for (script, want) in [
            ("hello", "\"hello\""),
            ("<bogus>", "no key named"),
            ("<enter", "no closing >"),
            ("<tick>", "no key named"),
            ("<tick x>", "whole number"),
            ("<tick -1>", "whole number"),
            ("<right*0>", "1 to"),
            ("<right*x>", "whole number"),
            ("<right*999999999>", "1 to"),
            ("<f13>", "F1 to F12"),
            ("<>", "no key named"),
        ] {
            let err = parse(script).unwrap_err();
            assert!(err.contains(want), "{script:?}: {err}");
        }
    }
}
