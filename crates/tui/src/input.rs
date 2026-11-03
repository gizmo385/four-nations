//! Keys to actions, and actions to changes of the app (docs/design/tui.md, "Input").
//!
//! One table, [`bindings`], says which key means which action and what to call it. Mapping a key
//! ([`map_key`]) reads it, and so does the key help screen ([`help_rows`]), so the help can't drift
//! from what the keys do. The research screen's key (`T`) is one more row; inside it the same arrows
//! and `Enter` mean "choose a tech".

use std::sync::LazyLock;

use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyEventState, KeyModifiers};
use fn_sim::Nation;
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
    /// `Enter`: in Select mode, select the unit under the cursor (again: the next one there); in
    /// a tool, set a corner or apply the rectangle.
    Confirm,
    /// `Esc`: close the screen; in a tool, drop the corner or with none leave the tool; in Select
    /// mode, close the selection bar.
    Cancel,
    /// `Tab` and Shift+`Tab`: the next and the previous unit, and the cursor goes to it.
    NextUnit,
    PrevUnit,
    /// `F`: the cursor tracks the selected unit.
    Follow,
    /// `T`: the research screen.
    ShowResearch,
    /// `L` and `?`: the message log and the key help.
    ShowLog,
    ShowHelp,
    /// `R`: a new map, after a y/n prompt.
    AskNewMap,
    /// `1`-`4`: switch nation, after a y/n prompt.
    AskNation(Nation),
    /// `F5`: save the game to the quicksave slot.
    QuickSave,
    /// `F9`: replace the game with the quicksave, after a y/n prompt.
    AskQuickLoad,
    /// `F6`: the saves screen (named slots).
    ShowSaves,
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

/// Where the key help lists a binding.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Group {
    Cursor,
    Units,
    Tools,
    Time,
    Game,
}

impl Group {
    pub const ALL: [Group; 5] = [Self::Cursor, Self::Units, Self::Tools, Self::Time, Self::Game];

    pub fn title(self) -> &'static str {
        match self {
            Self::Cursor => "Cursor and view",
            Self::Units => "Units",
            Self::Tools => "Designating",
            Self::Time => "Time",
            Self::Game => "Screens and game",
        }
    }
}

/// One key, what it does, and how the help words that. Bindings that follow each other with the
/// same `what` are one line of the help, and one with an empty `what` is not listed (a second
/// spelling of a key that is).
#[derive(Clone, Debug)]
pub struct Binding {
    pub key: Key,
    pub action: Action,
    pub group: Group,
    pub what: String,
}

/// Every key the game answers to, in the order the help lists them.
pub fn bindings() -> &'static [Binding] {
    static TABLE: LazyLock<Vec<Binding>> = LazyLock::new(build_table);
    &TABLE
}

fn build_table() -> Vec<Binding> {
    use KeyCode::{
        BackTab, Down, End, Enter, Esc, F, Home, Left, PageDown, PageUp, Right, Tab, Up,
    };
    let mut t = Vec::new();
    let mut add = |group, what: &str, key, action| {
        t.push(Binding { key, action, group, what: what.to_owned() });
    };
    let step = |dx, dy| Action::Move { dx, dy, tiles: 1 };

    let walk = "Move the cursor 1 tile (Shift: 10)";
    for (code, dx, dy) in [(Up, 0, -1), (Down, 0, 1), (Left, -1, 0), (Right, 1, 0)] {
        add(Group::Cursor, walk, Key::plain(code), step(dx, dy));
    }
    for (c, dx, dy) in [('w', 0, -1), ('a', -1, 0), ('s', 0, 1), ('d', 1, 0)] {
        add(Group::Cursor, walk, Key::ch(c), step(dx, dy));
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
        add(Group::Cursor, "", Key::pad(c), step(dx, dy));
    }
    add(Group::Cursor, "", Key::plain(Home), step(-1, -1));
    add(Group::Cursor, "", Key::plain(End), step(-1, 1));

    let jump = "Jump the view a quarter screen";
    for (code, dx, dy) in [(Up, 0, -1), (Down, 0, 1), (Left, -1, 0), (Right, 1, 0)] {
        add(Group::Cursor, jump, Key::ctrl(code), Action::Jump { dx, dy });
    }
    add(Group::Cursor, jump, Key::plain(PageUp), Action::Jump { dx: 0, dy: -1 });
    add(Group::Cursor, jump, Key::plain(PageDown), Action::Jump { dx: 0, dy: 1 });

    add(
        Group::Units,
        "Select the unit under the cursor; again: the next one there. In a tool: set a corner, \
         then apply",
        Key::plain(Enter),
        Action::Confirm,
    );
    add(
        Group::Units,
        "Close the screen or the bar. In a tool: drop the corner, then leave the tool",
        Key::plain(Esc),
        Action::Cancel,
    );
    add(Group::Units, "Select the next unit (Select mode)", Key::plain(Tab), Action::NextUnit);
    add(
        Group::Units,
        "Select the previous unit (Select mode)",
        Key::plain(BackTab),
        Action::PrevUnit,
    );
    add(Group::Units, "Follow the selected unit (Select mode)", Key::ch('f'), Action::Follow);

    for tool in Tool::ALL {
        // Select is Esc. Build needs a building picker, which has no key yet.
        let Some(letter) = tool.letter().filter(|_| tool != Tool::Build) else { continue };
        let what = format!("{} tool: {}", tool.name(), tool.label());
        add(Group::Tools, &what, Key::ch(letter.to_ascii_lowercase()), Action::PickTool(tool));
    }

    add(Group::Time, "Pause or run", Key::ch(' '), Action::TogglePause);
    add(Group::Time, "Faster", Key::ch('+'), Action::Faster);
    add(Group::Time, "Faster", Key::ch('='), Action::Faster);
    add(Group::Time, "Slower", Key::ch('-'), Action::Slower);
    add(Group::Time, "Slower", Key::ch('_'), Action::Slower);
    add(Group::Time, "One tick, while paused", Key::ch('.'), Action::Step);

    add(
        Group::Game,
        "Research: arrows W A S D choose a tech, Enter sets it as the target",
        Key::ch('t'),
        Action::ShowResearch,
    );
    add(Group::Game, "Message log", Key::ch('l'), Action::ShowLog);
    add(Group::Game, "Key help", Key::ch('?'), Action::ShowHelp);
    add(Group::Game, "New map (asks first)", Key::ch('r'), Action::AskNewMap);
    let switch = "Switch nation: 1 Air, 2 Water, 3 Earth, 4 Fire (asks first)";
    for (i, nation) in Nation::ALL.into_iter().enumerate() {
        let digit = char::from(b'1' + i as u8);
        add(Group::Game, switch, Key::ch(digit), Action::AskNation(nation));
    }
    // The Bevy client's quicksave keys, so a player of both doesn't learn two.
    add(Group::Game, "Quicksave", Key::plain(F(5)), Action::QuickSave);
    add(Group::Game, "Quickload (asks first)", Key::plain(F(9)), Action::AskQuickLoad);
    add(
        Group::Game,
        "Saves: arrows/W S choose a slot, Enter loads it, Shift+S saves over it (both ask \
         first), N a new one",
        Key::plain(F(6)),
        Action::ShowSaves,
    );
    add(Group::Game, "Quit (asks first)", Key::ch('q'), Action::AskQuit);
    t
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

/// One line of the key help: the keys, and what they do.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HelpRow {
    pub group: Group,
    pub keys: String,
    pub what: String,
}

/// The key help, generated from [`bindings`]: bindings that do the same thing share a row, and
/// the four arrows (or Ctrl+arrows) are written as one.
pub fn help_rows() -> Vec<HelpRow> {
    let mut rows: Vec<(Group, String, Vec<Key>)> = Vec::new();
    for b in bindings().iter().filter(|b| !b.what.is_empty()) {
        match rows.last_mut() {
            Some((g, what, keys)) if *g == b.group && *what == b.what => keys.push(b.key),
            _ => rows.push((b.group, b.what.clone(), vec![b.key])),
        }
    }
    rows.into_iter()
        .map(|(group, what, keys)| HelpRow { group, keys: join_keys(&keys), what })
        .collect()
}

/// The keys' names in a row, with Up Down Left Right (and their Ctrl versions) as "Arrows".
fn join_keys(keys: &[Key]) -> String {
    let all_four = |ctrl: bool| {
        [KeyCode::Up, KeyCode::Down, KeyCode::Left, KeyCode::Right]
            .iter()
            .all(|&code| keys.iter().any(|k| k.code == code && k.ctrl == ctrl && !k.keypad))
    };
    let mut names: Vec<String> = Vec::new();
    for k in keys {
        let arrow = matches!(k.code, KeyCode::Up | KeyCode::Down | KeyCode::Left | KeyCode::Right);
        let name = match (arrow, k.ctrl) {
            (true, false) if all_four(false) => "Arrows".to_owned(),
            (true, true) if all_four(true) => "Ctrl+Arrows".to_owned(),
            _ => k.name(),
        };
        if !names.contains(&name) {
            names.push(name);
        }
    }
    names.join(" ")
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
        // The number row is not the numpad: 7 does nothing, and 1 to 4 switch nation.
        assert_eq!(map_key(key(KeyCode::Char('7'))), None);
        assert_eq!(map_key(key(KeyCode::Char('1'))), Some(Action::AskNation(Nation::Air)));
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
        assert_eq!(map_key(key(KeyCode::F(1))), None);
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
        assert_eq!(map_key(key(KeyCode::Char('r'))), Some(Action::AskNewMap));
        assert_eq!(map_key(key(KeyCode::Char('R'))), Some(Action::AskNewMap));
        for (i, nation) in Nation::ALL.into_iter().enumerate() {
            let digit = char::from(b'1' + i as u8);
            assert_eq!(map_key(key(KeyCode::Char(digit))), Some(Action::AskNation(nation)));
        }
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

    #[test]
    fn the_help_lists_every_tool_and_group_from_the_table() {
        let rows = help_rows();
        for tool in [Tool::Harvest, Tool::CancelHarvest, Tool::Stockpile, Tool::Bend] {
            let row = rows
                .iter()
                .find(|r| r.what.starts_with(tool.name()))
                .unwrap_or_else(|| panic!("{} missing", tool.name()));
            assert_eq!(row.keys, tool.key(), "from the tool table");
        }
        let walk = rows.iter().find(|r| r.what.starts_with("Move the cursor")).unwrap();
        assert_eq!(walk.keys, "Arrows W A S D");
        let jump = rows.iter().find(|r| r.what.starts_with("Jump")).unwrap();
        assert_eq!(jump.keys, "Ctrl+Arrows PgUp PgDn");
        let nation = rows.iter().find(|r| r.what.starts_with("Switch nation")).unwrap();
        assert_eq!(nation.keys, "1 2 3 4");
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
