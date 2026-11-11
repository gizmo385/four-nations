//! Keys to actions, and actions to changes of the app (docs/design/tui.md, "Input").
//!
//! [`bindings`] says which crossterm key means which [`Action`]; [`map_key`] reads it. What each
//! key is *called* (its label, its short verb, its key help wording) is not here but in the one
//! table in `fn_ui::keys`, which the hint bar, the key help screen and `--help` are built from.
//! A test here walks every binding and every action `map_key` can return and fails if the table
//! doesn't name it, so a new key can't be added without its hint.

use std::sync::LazyLock;

use crate::keyev::{KeyCode, KeyEvent, KeyEventKind, KeyEventState, KeyModifiers};
use fn_ui::Tool;
pub use fn_ui::keys::{Action, Group, HelpRow, help_rows};

/// How far Shift moves the cursor.
pub const SHIFT_STEP: i32 = 10;

/// A key as the table names it. Shift is not part of it: it only makes a move ten tiles, and
/// letters ignore case.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Key {
    pub code: KeyCode,
    pub ctrl: bool,
    /// A numpad key, on a terminal that tells the numpad from the number row.
    pub keypad: bool,
}

impl Key {
    const fn plain(code: KeyCode) -> Self {
        Self { code, ctrl: false, keypad: false }
    }

    const fn ch(c: char) -> Self {
        Self::plain(KeyCode::Char(c))
    }

    const fn ctrl(code: KeyCode) -> Self {
        Self { code, ctrl: true, keypad: false }
    }

    const fn pad(c: char) -> Self {
        Self { code: KeyCode::Char(c), ctrl: false, keypad: true }
    }

    /// How the key is written in the help: "Enter", "W", "Ctrl+Up".
    pub fn name(&self) -> String {
        let base = match self.code {
            KeyCode::Char(' ') => "Space".to_owned(),
            KeyCode::Char(c) => c.to_ascii_uppercase().to_string(),
            KeyCode::Enter => "Enter".to_owned(),
            KeyCode::Esc => "Esc".to_owned(),
            KeyCode::Tab => "Tab".to_owned(),
            KeyCode::BackTab => "Shift+Tab".to_owned(),
            KeyCode::PageUp => "PgUp".to_owned(),
            KeyCode::PageDown => "PgDn".to_owned(),
            KeyCode::Up => "Up".to_owned(),
            KeyCode::Down => "Down".to_owned(),
            KeyCode::Left => "Left".to_owned(),
            KeyCode::Right => "Right".to_owned(),
            KeyCode::Home => "Home".to_owned(),
            KeyCode::End => "End".to_owned(),
            KeyCode::F(n) => format!("F{n}"),
            other => format!("{other:?}"),
        };
        match (self.ctrl, self.keypad) {
            (true, _) => format!("Ctrl+{base}"),
            (_, true) => format!("Num{base}"),
            _ => base,
        }
    }
}

/// One key and the action it asks for. Several keys can ask for the same one (`W` and Up).
#[derive(Clone, Copy, Debug)]
pub struct Binding {
    pub key: Key,
    pub action: Action,
}

/// Every key `map_key` answers to (Ctrl+C aside, which it matches by hand).
pub fn bindings() -> &'static [Binding] {
    static TABLE: LazyLock<Vec<Binding>> = LazyLock::new(build_table);
    &TABLE
}

fn build_table() -> Vec<Binding> {
    use KeyCode::{
        BackTab, Down, End, Enter, Esc, F, Home, Left, PageDown, PageUp, Right, Tab, Up,
    };
    let mut t = Vec::new();
    let mut add = |key, action| t.push(Binding { key, action });
    let step = |dx, dy| Action::Move { dx, dy, tiles: 1 };

    for (code, dx, dy) in [(Up, 0, -1), (Down, 0, 1), (Left, -1, 0), (Right, 1, 0)] {
        add(Key::plain(code), step(dx, dy));
    }
    for (c, dx, dy) in [('w', 0, -1), ('a', -1, 0), ('s', 0, 1), ('d', 1, 0)] {
        add(Key::ch(c), step(dx, dy));
    }
    // The numpad, diagonals too. Numpad 7 and 1 with NumLock off arrive as Home and End.
    for (c, dx, dy) in [
        ('7', -1, -1),
        ('8', 0, -1),
        ('9', 1, -1),
        ('4', -1, 0),
        ('6', 1, 0),
        ('1', -1, 1),
        ('2', 0, 1),
        ('3', 1, 1),
    ] {
        add(Key::pad(c), step(dx, dy));
    }
    add(Key::plain(Home), step(-1, -1));
    add(Key::plain(End), step(-1, 1));

    for (code, dx, dy) in [(Up, 0, -1), (Down, 0, 1), (Left, -1, 0), (Right, 1, 0)] {
        add(Key::ctrl(code), Action::Jump { dx, dy });
    }
    add(Key::plain(PageUp), Action::Jump { dx: 0, dy: -1 });
    add(Key::plain(PageDown), Action::Jump { dx: 0, dy: 1 });

    add(Key::plain(Enter), Action::Confirm);
    add(Key::plain(Esc), Action::Cancel);
    add(Key::plain(Tab), Action::NextUnit);
    add(Key::plain(BackTab), Action::PrevUnit);
    add(Key::ch('f'), Action::Follow);

    for tool in Tool::ALL {
        // Select is Esc; every other tool has a letter.
        if let Some(letter) = tool.letter() {
            add(Key::ch(letter.to_ascii_lowercase()), Action::PickTool(tool));
        }
    }
    add(Key::ch('o'), Action::ToggleRepeat);

    add(Key::ch(' '), Action::TogglePause);
    add(Key::ch('+'), Action::Faster);
    add(Key::ch('='), Action::Faster);
    add(Key::ch('-'), Action::Slower);
    add(Key::ch('_'), Action::Slower);
    add(Key::ch('.'), Action::Step);

    add(Key::ch('t'), Action::ShowResearch);
    add(Key::ch('l'), Action::ShowLog);
    add(Key::ch('?'), Action::ShowHelp);
    add(Key::ch('g'), Action::ShowEndeavor);
    add(Key::ch('n'), Action::ShowNation);
    // F5/F9, the usual quicksave/quickload keys.
    add(Key::plain(F(5)), Action::QuickSave);
    add(Key::plain(F(9)), Action::AskQuickLoad);
    add(Key::plain(F(6)), Action::ShowSaves);
    t
}

/// Whether `action` may reach the watch spectator's own `UiState` in free look (#330,
/// `crate::spectate`): camera and cursor movement, browsing a unit or a read-only screen, never
/// anything that would change the agent's own game. Every [`Action`] variant is matched by name,
/// with no wildcard arm, so a new one fails to compile here until it's judged -- that's this
/// crate's one enforcement that a new binding never reaches a spectator unclassified, since a
/// test can't see a variant `map_key` never returns.
///
/// `Action::PickTool` is judged by `tool`, not blanket: Build's own key only opens the
/// browse-only picker (#330's acceptance criteria, "Build list (U, browse only)"); it never
/// places anything by itself (`crate::spectate` refuses `Enter` inside it), so it alone is
/// allowed, and any other (including a tool added after this was written) is refused, the safe
/// default. `Action::Confirm` is allowed in general (mostly "select a unit to see its bar"); the
/// Research screen's own exception (`Enter` there sets the agent's research target) is judged by
/// `crate::spectate`, which knows which screen is open and this function doesn't.
pub fn spectator_allowed(action: Action) -> bool {
    match action {
        Action::Move { .. } | Action::Jump { .. } => true,
        Action::PickTool(Tool::Build) => true,
        Action::PickTool(_) => false,
        Action::ToggleRepeat => false,
        Action::Confirm => true,
        Action::Cancel => false,
        Action::NextUnit | Action::PrevUnit | Action::Follow => true,
        Action::ShowResearch
        | Action::ShowLog
        | Action::ShowHelp
        | Action::ShowEndeavor
        | Action::ShowNation => true,
        Action::ShowSaves => false,
        Action::QuickSave | Action::AskQuickLoad => false,
        Action::TogglePause | Action::Faster | Action::Slower | Action::Step => false,
        Action::QuitNow => false,
    }
}

/// What a key means when no prompt is open, if anything.
pub fn map_key(key: KeyEvent) -> Option<Action> {
    if key.kind == KeyEventKind::Release {
        return None;
    }
    let KeyEvent { code, modifiers, state, .. } = key;
    let ctrl = modifiers.contains(KeyModifiers::CONTROL);
    let alt = modifiers.contains(KeyModifiers::ALT);
    let upper = matches!(code, KeyCode::Char(c) if c.is_ascii_uppercase());
    let shift = modifiers.contains(KeyModifiers::SHIFT) || upper;

    if ctrl && matches!(code, KeyCode::Char('c' | 'C')) {
        return Some(Action::QuitNow);
    }
    // Ctrl and Alt chords are the terminal's and the shell's.
    if matches!(code, KeyCode::Char(_)) && (ctrl || alt) {
        return None;
    }
    let code = match code {
        KeyCode::Char(c) => KeyCode::Char(c.to_ascii_lowercase()),
        KeyCode::Tab if shift => KeyCode::BackTab,
        other => other,
    };
    let want = Key { code, ctrl, keypad: state.contains(KeyEventState::KEYPAD) };
    let binding = bindings().iter().find(|b| b.key == want)?;
    Some(match binding.action {
        // Shift (or an upper-case letter, or Caps Lock) makes a move ten tiles.
        Action::Move { dx, dy, tiles } if shift => {
            Action::Move { dx, dy, tiles: tiles * SHIFT_STEP }
        }
        other => other,
    })
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
        // The number row is not the numpad: its digits do nothing (1 to 4 once switched nation,
        // #655 took that away).
        assert_eq!(map_key(key(KeyCode::Char('7'))), None);
        assert_eq!(map_key(key(KeyCode::Char('1'))), None);
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
        // `Q` and `R` are not keys any more (#655): quit and new map live in the game menu.
        assert_eq!(map_key(key(KeyCode::Char('q'))), None);
        assert_eq!(map_key(key(KeyCode::Char('Q'))), None);
        assert_eq!(map_key(key(KeyCode::Char('r'))), None);
        assert_eq!(map_key(key(KeyCode::F(1))), None);
    }

    #[test]
    fn the_tool_keys_ignore_case_and_enter_and_esc_drive_the_rectangle() {
        for (c, tool) in [
            ('h', Tool::Harvest),
            ('x', Tool::CancelHarvest),
            ('p', Tool::Stockpile),
            ('b', Tool::Bend),
            ('u', Tool::Build),
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
        assert_eq!(map_key(key(KeyCode::Enter)), Some(Action::Confirm));
        assert_eq!(map_key(key(KeyCode::Esc)), Some(Action::Cancel));
    }

    #[test]
    fn the_selection_and_panel_keys() {
        assert_eq!(map_key(key(KeyCode::Tab)), Some(Action::NextUnit));
        assert_eq!(map_key(key(KeyCode::BackTab)), Some(Action::PrevUnit));
        assert_eq!(map_key(with(KeyCode::Tab, KeyModifiers::SHIFT)), Some(Action::PrevUnit));
        assert_eq!(map_key(key(KeyCode::Char('f'))), Some(Action::Follow));
        assert_eq!(map_key(key(KeyCode::Char('F'))), Some(Action::Follow));
        assert_eq!(map_key(key(KeyCode::Char('t'))), Some(Action::ShowResearch));
        assert_eq!(map_key(key(KeyCode::Char('T'))), Some(Action::ShowResearch));
        assert_eq!(map_key(key(KeyCode::Char('l'))), Some(Action::ShowLog));
        assert_eq!(map_key(key(KeyCode::Char('L'))), Some(Action::ShowLog));
        // A terminal sends `?` as Shift+/, so it may carry the Shift flag.
        assert_eq!(map_key(key(KeyCode::Char('?'))), Some(Action::ShowHelp));
        assert_eq!(map_key(with(KeyCode::Char('?'), KeyModifiers::SHIFT)), Some(Action::ShowHelp));
        assert_eq!(map_key(key(KeyCode::Char('5'))), None);
        assert_eq!(map_key(key(KeyCode::F(5))), Some(Action::QuickSave));
        assert_eq!(map_key(key(KeyCode::F(9))), Some(Action::AskQuickLoad));
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

    #[test]
    fn no_key_is_bound_twice() {
        let table = bindings();
        for (i, a) in table.iter().enumerate() {
            for b in &table[i + 1..] {
                assert_ne!(a.key, b.key, "{} is bound twice", a.key.name());
            }
        }
    }

    #[test]
    fn every_listed_binding_maps_back_to_its_action() {
        // The table is what map_key reads, so a key typed as the help names it does its action.
        for b in bindings() {
            let mods = if b.key.ctrl { KeyModifiers::CONTROL } else { KeyModifiers::NONE };
            let state = if b.key.keypad { KeyEventState::KEYPAD } else { KeyEventState::NONE };
            let event =
                KeyEvent { code: b.key.code, modifiers: mods, kind: KeyEventKind::Press, state };
            assert_eq!(map_key(event), Some(b.action), "{}", b.key.name());
        }
    }

    /// `spectator_allowed`'s own table (#330's acceptance criteria): camera, cursor, browsing a
    /// unit and every read-only screen are allowed; every mutating action -- tools other than
    /// Build's browse-only picker, standing orders, cancel, saves and loads, a new map, a
    /// nation switch, the game's own pause/speed and quit -- is refused. Exhaustiveness (no new
    /// binding reaches a spectator unclassified) is enforced by `spectator_allowed`'s match
    /// having no wildcard arm, which fails to compile, not merely a test, once an `Action`
    /// variant is added without being judged there.
    #[test]
    fn spectator_allowed_matches_the_acceptance_criteria() {
        let allowed = [
            Action::Move { dx: 1, dy: 0, tiles: 1 },
            Action::Jump { dx: 0, dy: 1 },
            Action::PickTool(Tool::Build),
            Action::Confirm,
            Action::NextUnit,
            Action::PrevUnit,
            Action::Follow,
            Action::ShowResearch,
            Action::ShowLog,
            Action::ShowHelp,
            Action::ShowEndeavor,
            Action::ShowNation,
        ];
        for a in allowed {
            assert!(spectator_allowed(a), "{a:?} should be allowed");
        }
        let refused = [
            Action::PickTool(Tool::Harvest),
            Action::PickTool(Tool::CancelHarvest),
            Action::PickTool(Tool::Clear),
            Action::PickTool(Tool::Stockpile),
            Action::PickTool(Tool::Bend),
            Action::PickTool(Tool::Sow),
            Action::PickTool(Tool::Select),
            Action::ToggleRepeat,
            Action::Cancel,
            Action::ShowSaves,
            Action::QuickSave,
            Action::AskQuickLoad,
            Action::TogglePause,
            Action::Faster,
            Action::Slower,
            Action::Step,
            Action::QuitNow,
        ];
        for a in refused {
            assert!(!spectator_allowed(a), "{a:?} should be refused");
        }
    }

    /// Every entry of the real table resolves through `spectator_allowed` without needing a
    /// wildcard here either, so a binding that only shows up through `bindings()` (never
    /// constructed by hand above) is still exercised.
    #[test]
    fn every_binding_is_classified_for_the_spectator() {
        for b in bindings() {
            let _ = spectator_allowed(b.action);
        }
    }

    /// The tokens a key label in `fn_ui::keys`'s table stands for: "Arrows or WASD" covers Up,
    /// Down, Left, Right, W, A, S and D; "Ctrl+Arrows or PgUp/PgDn" covers Ctrl+arrows, PgUp and
    /// PgDn; "1-4" covers 1 to 4; "+ or =" covers + and =.
    fn label_tokens(label: &str) -> Vec<String> {
        let mut out = Vec::new();
        for word in label.split([' ', '/']).filter(|w| *w != "or") {
            match word {
                "Arrows" => out.extend(["Up", "Down", "Left", "Right"].map(String::from)),
                "Ctrl+Arrows" => out
                    .extend(["Ctrl+Up", "Ctrl+Down", "Ctrl+Left", "Ctrl+Right"].map(String::from)),
                "WASD" => out.extend(["W", "A", "S", "D"].map(String::from)),
                "1-4" => out.extend(["1", "2", "3", "4"].map(String::from)),
                other => out.push(other.to_owned()),
            }
        }
        out
    }

    /// A binding handled here and missing from `fn_ui::keys`'s table would have no hint and no
    /// key help row (#651): every action `map_key` can return is named by an entry, and every
    /// key it is listed under is in that entry's label.
    #[test]
    fn every_key_map_key_handles_is_in_the_key_table() {
        let table = fn_ui::keys::table();
        let mut actions: Vec<Action> = bindings().iter().map(|b| b.action).collect();
        actions.push(Action::QuitNow);
        for action in actions {
            assert!(
                table.iter().any(|e| e.actions.iter().any(|a| a.same_kind(action))),
                "{action:?} is handled but no entry of fn_ui::keys's table names it"
            );
        }
        for b in bindings() {
            // The numpad and Home/End are second spellings of the arrows, not listed.
            if b.key.keypad || matches!(b.key.code, KeyCode::Home | KeyCode::End) {
                continue;
            }
            let name = b.key.name();
            let listed = table.iter().any(|e| {
                e.help.is_some()
                    && e.actions.iter().any(|a| a.same_kind(b.action))
                    && label_tokens(&e.keys).contains(&name)
            });
            assert!(listed, "{name} ({:?}) is in no key help label", b.action);
        }
        // Ctrl+C is matched by hand in `map_key`.
        assert!(table.iter().any(|e| e.keys == "Ctrl+C" && e.actions == [Action::QuitNow]));
    }

    /// The other way round: a label in the table that no key reaches is a hint that lies.
    #[test]
    fn every_key_label_in_the_table_is_a_key_that_works() {
        let known: Vec<String> = bindings()
            .iter()
            .map(|b| b.key.name())
            .chain(["Ctrl+C".to_owned(), "Shift+S".to_owned(), "Shift+Tab".to_owned()])
            // The message log's filters (#661): the number row, read by `App::key`.
            .chain(["1-5".to_owned()])
            .collect();
        for e in fn_ui::keys::table() {
            // `Type` (the name prompt) is every character, not one key.
            if e.keys == "Type" {
                continue;
            }
            for token in label_tokens(&e.keys) {
                assert!(known.contains(&token), "{token:?} in {:?} is bound to no key", e.keys);
            }
        }
    }

    #[test]
    fn the_help_lists_every_tool_and_group_from_the_table() {
        let rows = help_rows();
        for tool in [Tool::Harvest, Tool::CancelHarvest, Tool::Stockpile, Tool::Bend, Tool::Build] {
            let row = rows
                .iter()
                .find(|r| r.what.starts_with(tool.name()))
                .unwrap_or_else(|| panic!("{} missing", tool.name()));
            assert_eq!(row.keys, tool.key(), "from the tool table");
        }
        let walk = rows.iter().find(|r| r.what.starts_with("Move 1 tile")).unwrap();
        assert_eq!(walk.keys, "Arrows or WASD");
        let jump = rows.iter().find(|r| r.what.starts_with("Jump")).unwrap();
        assert_eq!(jump.keys, "Ctrl+Arrows");
        assert!(rows.iter().any(|r| r.keys == "PgUp/PgDn" && r.what.starts_with("Jump")));
        // New map, switch nation and quit live in the game menu only (#655): no key row.
        for gone in ["New map", "Switch nation", "Quit (asks"] {
            assert!(rows.iter().all(|r| !r.what.starts_with(gone)), "{gone} has a key row");
        }
        let save = rows.iter().find(|r| r.what == "Quicksave").unwrap();
        assert_eq!(save.keys, "F5");
        let load = rows.iter().find(|r| r.what.starts_with("Quickload")).unwrap();
        assert_eq!(load.keys, "F9");
        for group in Group::ALL {
            assert!(rows.iter().any(|r| r.group == group), "{group:?} has no rows");
        }
        assert!(rows.iter().all(|r| !r.keys.is_empty() && !r.what.is_empty()));
    }
}
