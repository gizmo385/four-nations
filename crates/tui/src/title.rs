//! The title screen (#280): before a game exists, New game (pick a nation and a seed), Continue
//! (the newest loadable slot, hidden with none), Load (every slot, as the in-game saves screen
//! shows them) and Quit. [`crate::shell::Shell`] is the only thing that drives this: it holds
//! either this or the game, and switches between them on the outcome of a key.
//!
//! Like [`crate::state::UiState`], nothing here touches the terminal, so the keys and the golden
//! frames can drive it directly.

use std::path::Path;

use crossterm::event::{KeyCode, KeyEvent, KeyEventKind};
use fn_sim::{Defs, Nation, World};
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Style;

use crate::color::Palette;
use crate::draw::too_small_text;
use crate::layout::{MIN_HEIGHT, MIN_WIDTH};
use crate::widgets::{Painter, Row, TitleAt, Tone, draw_frame, wrap};

/// What `F6`'s own save-slot rows are drawn with, reused for the Load page.
use crate::saves as save_rows;

/// What `F5`, `F9` and the in-game `F6` say when there is no save directory (`app::NO_SAVE_DIR`'s
/// own wording, repeated here since the title screen has no `App` to read it from).
const NO_SAVE_DIR: &str = "Save failed: this platform has no data directory (set FN_SAVE_DIR)";

/// The longest a typed seed may be: `u64::MAX` is 20 digits.
const MAX_SEED_DIGITS: usize = 20;

/// One row of the main menu.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MenuItem {
    NewGame,
    /// Hidden unless [`TitleScreen::continue_slot`] has something to load.
    Continue,
    Load,
    Quit,
}

impl MenuItem {
    pub fn label(self) -> &'static str {
        match self {
            Self::NewGame => "New game",
            Self::Continue => "Continue",
            Self::Load => "Load",
            Self::Quit => "Quit",
        }
    }
}

/// What the screen is showing.
#[derive(Clone, Debug, PartialEq)]
pub enum Page {
    Menu,
    /// Picking a nation: `cursor` indexes [`Nation::ALL`].
    Nation {
        cursor: usize,
    },
    /// The chosen nation, typing a seed.
    Seed {
        nation: Nation,
        digits: String,
        edited: bool,
    },
    /// Every save, as the in-game saves screen (`F6`) shows them.
    Load,
}

/// What a key did, for [`crate::shell::Shell`] to act on.
#[derive(Debug)]
pub enum Outcome {
    None,
    /// Start a new game.
    Start {
        seed: u64,
        nation: Nation,
    },
    /// Continue or Load: a world ready to become the game.
    Loaded(Box<World>),
    /// The menu's own Quit: the whole program stops without asking again.
    Quit,
}

#[derive(Clone, Debug, PartialEq)]
pub struct TitleScreen {
    pub page: Page,
    /// The main menu's highlight.
    pub cursor: usize,
    /// The newest loadable save, if any (`MenuItem::Continue`'s target, and shown only then).
    pub continue_slot: Option<fn_ui::saves::Slot>,
    /// Every save, newest first, for the Load page.
    pub save_slots: Vec<fn_ui::saves::Slot>,
    pub load_cursor: usize,
    /// A load's failure, or a hint about a save with no directory; cleared on the next key.
    pub status: String,
    /// The seed a fresh New Game offers until the player types over it: bumped each time a new
    /// title screen starts (`Shell`'s own job, since only it may read the wall clock), so
    /// starting over and over doesn't always suggest the same seed.
    seed_hint: u64,
}

impl TitleScreen {
    pub fn new(defs: &Defs, saves: Option<&Path>, seed_hint: u64) -> Self {
        let save_slots = list_slots(defs, saves);
        let continue_slot = save_slots.iter().find(|s| s.loadable()).cloned();
        Self {
            page: Page::Menu,
            cursor: 0,
            continue_slot,
            save_slots,
            load_cursor: 0,
            status: String::new(),
            seed_hint,
        }
    }

    /// The menu rows that show right now: `Continue` only with somewhere to continue.
    pub fn items(&self) -> Vec<MenuItem> {
        let mut items = vec![MenuItem::NewGame];
        if self.continue_slot.is_some() {
            items.push(MenuItem::Continue);
        }
        items.push(MenuItem::Load);
        items.push(MenuItem::Quit);
        items
    }

    /// The nation whose colours the screen should wear right now: the one being picked or
    /// seeded, Earth's elsewhere (the menu and the load list name every nation themselves, so
    /// Earth is as good a default as any).
    pub fn display_nation(&self) -> Nation {
        match &self.page {
            Page::Nation { cursor } => Nation::ALL[*cursor % Nation::ALL.len()],
            Page::Seed { nation, .. } => *nation,
            Page::Menu | Page::Load => Nation::Earth,
        }
    }

    /// Handles one key, returning what `Shell` should do about it.
    pub fn key(&mut self, key: KeyEvent, defs: &Defs, saves: Option<&Path>) -> Outcome {
        if key.kind == KeyEventKind::Release {
            return Outcome::None;
        }
        self.status.clear();
        match self.page.clone() {
            Page::Menu => self.key_menu(key, defs, saves),
            Page::Nation { cursor } => {
                self.key_nation(key, cursor);
                Outcome::None
            }
            Page::Seed { nation, digits, edited } => self.key_seed(key, nation, digits, edited),
            Page::Load => self.key_load(key, defs, saves),
        }
    }

    fn key_menu(&mut self, key: KeyEvent, defs: &Defs, saves: Option<&Path>) -> Outcome {
        let items = self.items();
        match key.code {
            KeyCode::Up | KeyCode::Char('w' | 'W') => {
                self.cursor = step(items.len(), self.cursor, -1)
            }
            KeyCode::Down | KeyCode::Char('s' | 'S') => {
                self.cursor = step(items.len(), self.cursor, 1)
            }
            KeyCode::Enter => {
                let chosen = items[self.cursor];
                return self.choose(chosen, defs, saves);
            }
            _ => {}
        }
        Outcome::None
    }

    fn choose(&mut self, item: MenuItem, defs: &Defs, saves: Option<&Path>) -> Outcome {
        match item {
            MenuItem::NewGame => {
                self.page = Page::Nation { cursor: 0 };
                Outcome::None
            }
            MenuItem::Continue => match self.continue_slot.clone() {
                Some(slot) => self.load_named(&slot.name, defs, saves),
                None => Outcome::None,
            },
            MenuItem::Load => {
                self.save_slots = list_slots(defs, saves);
                self.load_cursor = 0;
                self.page = Page::Load;
                Outcome::None
            }
            MenuItem::Quit => Outcome::Quit,
        }
    }

    fn load_named(&mut self, name: &str, defs: &Defs, saves: Option<&Path>) -> Outcome {
        let Some(dir) = saves else {
            self.status = NO_SAVE_DIR.to_owned();
            return Outcome::None;
        };
        match fn_ui::saves::load(dir, name, defs) {
            Ok(world) => Outcome::Loaded(Box::new(world)),
            Err(why) => {
                self.status = format!("Load failed: {why}");
                Outcome::None
            }
        }
    }

    fn key_nation(&mut self, key: KeyEvent, cursor: usize) {
        let n = Nation::ALL.len() as i32;
        let mut cursor = cursor as i32;
        match key.code {
            KeyCode::Left | KeyCode::Up | KeyCode::Char('a' | 'A' | 'w' | 'W') => {
                cursor = (cursor - 1).rem_euclid(n);
            }
            KeyCode::Right | KeyCode::Down | KeyCode::Char('d' | 'D' | 's' | 'S') => {
                cursor = (cursor + 1).rem_euclid(n);
            }
            KeyCode::Char(c @ '1'..='4') => cursor = i32::from(c as u8 - b'1'),
            KeyCode::Enter => {
                let nation = Nation::ALL[cursor as usize];
                self.page =
                    Page::Seed { nation, digits: self.seed_hint.to_string(), edited: false };
                return;
            }
            KeyCode::Esc => {
                self.page = Page::Menu;
                return;
            }
            _ => {}
        }
        self.page = Page::Nation { cursor: cursor as usize };
    }

    fn key_seed(
        &mut self,
        key: KeyEvent,
        nation: Nation,
        mut digits: String,
        mut edited: bool,
    ) -> Outcome {
        match key.code {
            KeyCode::Char(c) if c.is_ascii_digit() => {
                if !edited {
                    digits.clear();
                    edited = true;
                }
                if digits.len() < MAX_SEED_DIGITS {
                    digits.push(c);
                }
            }
            KeyCode::Backspace => {
                edited = true;
                digits.pop();
            }
            KeyCode::Enter => {
                let seed =
                    if digits.is_empty() { self.seed_hint } else { digits.parse().unwrap_or(0) };
                return Outcome::Start { seed, nation };
            }
            KeyCode::Esc => {
                let cursor = Nation::ALL.iter().position(|&n| n == nation).unwrap_or(0);
                self.page = Page::Nation { cursor };
                return Outcome::None;
            }
            _ => {}
        }
        self.page = Page::Seed { nation, digits, edited };
        Outcome::None
    }

    fn key_load(&mut self, key: KeyEvent, defs: &Defs, saves: Option<&Path>) -> Outcome {
        match key.code {
            KeyCode::Up | KeyCode::Char('w' | 'W') => {
                self.load_cursor = step(self.save_slots.len(), self.load_cursor, -1);
            }
            KeyCode::Down | KeyCode::Char('s' | 'S') => {
                self.load_cursor = step(self.save_slots.len(), self.load_cursor, 1);
            }
            KeyCode::Enter => {
                if let Some(slot) = self.save_slots.get(self.load_cursor).cloned() {
                    if !slot.loadable() {
                        self.status = slot.describe(defs);
                        return Outcome::None;
                    }
                    return self.load_named(&slot.name, defs, saves);
                }
            }
            KeyCode::Esc => self.page = Page::Menu,
            _ => {}
        }
        Outcome::None
    }
}

/// Every save in `saves`, newest first, or none without a directory. A listing error is folded
/// into an empty list; it would otherwise have nowhere to show before the Load page is even open.
fn list_slots(defs: &Defs, saves: Option<&Path>) -> Vec<fn_ui::saves::Slot> {
    saves.map(|dir| fn_ui::saves::list(dir, defs).unwrap_or_default()).unwrap_or_default()
}

/// The highlight after a step of `dy` rows (its sign only), stopping at the edges, as
/// `crate::saves::step` does for the in-game screen.
fn step(len: usize, at: usize, dy: i32) -> usize {
    if len == 0 {
        return 0;
    }
    let last = len - 1;
    (at as i64 + i64::from(dy.signum())).clamp(0, last as i64) as usize
}

// -------------------------------------------------------------------------------------------
// Drawing
// -------------------------------------------------------------------------------------------

/// The footer hint for the frame around the title screen's box, by page.
fn footer(page: &Page) -> &'static str {
    match page {
        Page::Menu => "Arrows/W S choose  Enter select",
        Page::Nation { .. } => "Arrows/1-4 choose  Enter confirm  Esc back",
        Page::Seed { .. } => "Digits edit  Backspace delete  Enter start  Esc back",
        Page::Load => "Arrows/W S choose  Enter load  Esc back",
    }
}

/// The box the title screen draws in: centred, at most 64 columns wide.
fn box_area(area: Rect) -> Rect {
    let width = area.width.min(64);
    let height = area.height.min(22);
    Rect::new(area.x + (area.width - width) / 2, area.y + (area.height - height) / 2, width, height)
}

/// Draws the title screen into `area` (the whole terminal): a centred Seal frame, the menu, the
/// nation picker, the seed field or the save list, depending on [`TitleScreen::page`].
pub fn draw_title(
    screen: &TitleScreen,
    defs: &Defs,
    palette: &Palette,
    area: Rect,
    buf: &mut Buffer,
) {
    if area.width < MIN_WIDTH || area.height < MIN_HEIGHT {
        buf.set_stringn(area.x, area.y, too_small_text(), usize::from(area.width), Style::new());
        return;
    }
    let marks = crate::widgets::Marks::new(palette.ascii);
    let title = format!(" {0} FOUR NATIONS {0} ", marks.seal);
    let outer = box_area(area);
    draw_frame(buf, outer, palette, &title, TitleAt::Centre, footer(&screen.page));
    let inner = Rect::new(outer.x + 2, outer.y + 1, outer.width - 4, outer.height - 2);

    if screen.page == Page::Load {
        return draw_load(screen, defs, palette, inner, buf);
    }
    let mut p = Painter::new(buf, inner, palette);
    match &screen.page {
        Page::Menu => draw_menu(screen, &mut p),
        Page::Nation { cursor } => draw_nation(defs, *cursor, &mut p),
        Page::Seed { nation, digits, .. } => draw_seed(defs, *nation, digits, &mut p),
        Page::Load => unreachable!("handled above"),
    }
}

fn draw_menu(screen: &TitleScreen, p: &mut Painter) {
    p.row(&Row::text("Welcome to the Four Nations.", Tone::Dim));
    p.blank();
    for (i, item) in screen.items().into_iter().enumerate() {
        let pointer = if i == screen.cursor { p.marks.pointer } else { " " };
        let tone = if i == screen.cursor { Tone::Title } else { Tone::Text };
        p.row(&Row::text(format!("{pointer} {}", item.label()), tone));
    }
    if !screen.status.is_empty() {
        p.blank();
        p.row(&Row::text(screen.status.clone(), Tone::Warn));
    }
}

fn draw_nation(defs: &Defs, cursor: usize, p: &mut Painter) {
    p.row(&Row::text("Choose a nation (arrows or 1-4):", Tone::Dim));
    p.blank();
    for (i, nation) in Nation::ALL.into_iter().enumerate() {
        let def = defs.nation(nation);
        let pointer = if i == cursor { p.marks.pointer } else { " " };
        let tone = if i == cursor { Tone::Title } else { Tone::Text };
        p.row(&Row::text(format!("{pointer} {}. {}", i + 1, def.name), tone));
    }
    p.blank();
    let chosen = defs.nation(Nation::ALL[cursor % Nation::ALL.len()]);
    let width = usize::from(p.inner.width);
    for line in wrap(&chosen.description, width, 4, p.marks.ellipsis) {
        p.row(&Row::text(line, Tone::Text));
    }
}

fn draw_seed(defs: &Defs, nation: Nation, digits: &str, p: &mut Painter) {
    let def = defs.nation(nation);
    p.row(&Row::text(format!("{} — pick a seed:", def.name), Tone::Title));
    p.blank();
    let width = usize::from(p.inner.width);
    for line in wrap(&def.description, width, 3, p.marks.ellipsis) {
        p.row(&Row::text(line, Tone::Dim));
    }
    p.blank();
    p.row(&Row::text(format!("Seed: {digits}"), Tone::Accent));
    p.blank();
    p.row(&Row::text("Type digits, Backspace to edit, Enter to start.", Tone::Dim));
}

fn draw_load(screen: &TitleScreen, defs: &Defs, palette: &Palette, inner: Rect, buf: &mut Buffer) {
    if screen.save_slots.is_empty() {
        let mut p = Painter::new(buf, inner, palette);
        p.row(&Row::text("No saves yet.", Tone::Dim));
        return;
    }
    save_rows::draw(&screen.save_slots, screen.load_cursor, defs, palette, inner, buf);
    if !screen.status.is_empty() {
        let y = inner.bottom().saturating_sub(1);
        let style = Tone::Warn.style(palette);
        buf.set_stringn(inner.x, y, &screen.status, usize::from(inner.width), style);
    }
}

/// Over the game: `Q` then `y` asks whether to go back to the title screen or exit the process
/// (#280's acceptance criteria), since quitting no longer always means stopping `fn_tui`.
pub fn draw_exit_prompt(palette: &Palette, area: Rect, buf: &mut Buffer) {
    let width = area.width.min(50);
    let height = 5u16.min(area.height);
    let outer = Rect::new(
        area.x + (area.width - width) / 2,
        area.y + (area.height - height) / 2,
        width,
        height,
    );
    draw_frame(buf, outer, palette, " QUIT? ", TitleAt::Centre, "");
    if outer.height < 3 {
        return;
    }
    let inner = Rect::new(outer.x + 2, outer.y + 1, outer.width - 4, outer.height - 2);
    let mut p = Painter::new(buf, inner, palette);
    p.row(&Row::text("Go back to the title screen, or exit?", Tone::Text));
    p.blank();
    p.row(&Row::text("T title   Q/Enter exit   Esc stay", Tone::Dim));
}

#[cfg(test)]
mod tests {
    use crossterm::event::{KeyEventState, KeyModifiers};
    use fn_sim::Defs;

    use super::*;

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    fn press(screen: &mut TitleScreen, code: KeyCode) -> Outcome {
        let defs = Defs::bundled();
        screen.key(key(code), &defs, None)
    }

    #[test]
    fn with_no_saves_continue_is_hidden_and_the_menu_has_the_other_three() {
        let defs = Defs::bundled();
        let screen = TitleScreen::new(&defs, None, 1);
        assert_eq!(
            screen.items(),
            [MenuItem::NewGame, MenuItem::Load, MenuItem::Quit],
            "no save directory: nothing to continue"
        );
    }

    #[test]
    fn the_menu_cursor_stops_at_the_edges() {
        let defs = Defs::bundled();
        let mut screen = TitleScreen::new(&defs, None, 1);
        assert_eq!(screen.cursor, 0);
        press(&mut screen, KeyCode::Up);
        assert_eq!(screen.cursor, 0, "stops at the top");
        for _ in 0..5 {
            press(&mut screen, KeyCode::Down);
        }
        assert_eq!(screen.cursor, screen.items().len() - 1, "stops at the bottom");
    }

    #[test]
    fn new_game_goes_through_a_nation_then_a_seed_then_starts() {
        let defs = Defs::bundled();
        let mut screen = TitleScreen::new(&defs, None, 42);
        assert!(matches!(press(&mut screen, KeyCode::Enter), Outcome::None)); // New game (cursor 0)
        assert!(matches!(screen.page, Page::Nation { cursor: 0 }));
        assert_eq!(screen.display_nation(), Nation::Air);

        press(&mut screen, KeyCode::Char('3'));
        assert!(matches!(screen.page, Page::Nation { cursor: 2 }));
        assert_eq!(screen.display_nation(), Nation::Earth);

        press(&mut screen, KeyCode::Right);
        assert!(matches!(screen.page, Page::Nation { cursor: 3 }));
        press(&mut screen, KeyCode::Right);
        assert!(matches!(screen.page, Page::Nation { cursor: 0 }), "wraps round");

        press(&mut screen, KeyCode::Char('4'));
        assert!(matches!(screen.page, Page::Nation { cursor: 3 }));
        let outcome = press(&mut screen, KeyCode::Enter);
        assert!(matches!(screen.page, Page::Seed { nation: Nation::Fire, .. }));
        assert!(matches!(outcome, Outcome::None));

        // The default seed is offered until the player types: Enter alone uses it.
        if let Page::Seed { digits, .. } = &screen.page {
            assert_eq!(digits, "42");
        }
        let outcome = press(&mut screen, KeyCode::Enter);
        assert!(matches!(outcome, Outcome::Start { seed: 42, nation: Nation::Fire }));
    }

    #[test]
    fn typing_digits_replaces_the_default_seed_and_backspace_edits_it() {
        let defs = Defs::bundled();
        let mut screen = TitleScreen::new(&defs, None, 999);
        screen.page = Page::Seed { nation: Nation::Water, digits: "999".into(), edited: false };
        press(&mut screen, KeyCode::Char('7'));
        if let Page::Seed { digits, edited, .. } = &screen.page {
            assert_eq!(digits, "7", "the first digit clears the default");
            assert!(edited);
        } else {
            panic!("still on the seed page");
        }
        press(&mut screen, KeyCode::Char('3'));
        press(&mut screen, KeyCode::Backspace);
        press(&mut screen, KeyCode::Char('5'));
        let outcome = press(&mut screen, KeyCode::Enter);
        assert!(matches!(outcome, Outcome::Start { seed: 75, nation: Nation::Water }));
    }

    #[test]
    fn escape_steps_back_a_page_at_a_time() {
        let defs = Defs::bundled();
        let mut screen = TitleScreen::new(&defs, None, 1);
        press(&mut screen, KeyCode::Enter); // New game
        press(&mut screen, KeyCode::Enter); // Air, to the seed page
        press(&mut screen, KeyCode::Esc);
        assert!(matches!(screen.page, Page::Nation { cursor: 0 }));
        press(&mut screen, KeyCode::Esc);
        assert_eq!(screen.page, Page::Menu);
    }

    #[test]
    fn an_empty_seed_before_any_typing_falls_back_to_the_hint() {
        let defs = Defs::bundled();
        let mut screen = TitleScreen::new(&defs, None, 7);
        press(&mut screen, KeyCode::Enter); // New game
        press(&mut screen, KeyCode::Enter); // Air, to the seed page
        let outcome = press(&mut screen, KeyCode::Enter); // Enter alone: the hint
        assert!(matches!(outcome, Outcome::Start { seed: 7, nation: Nation::Air }));
    }

    #[test]
    fn quit_from_the_menu_quits_at_once_with_no_further_question() {
        let defs = Defs::bundled();
        let mut screen = TitleScreen::new(&defs, None, 1);
        for _ in 0..3 {
            press(&mut screen, KeyCode::Down);
        }
        assert_eq!(screen.items()[screen.cursor], MenuItem::Quit);
        assert!(matches!(press(&mut screen, KeyCode::Enter), Outcome::Quit));
    }

    #[test]
    fn with_no_save_directory_load_says_so_and_continue_does_nothing() {
        let defs = Defs::bundled();
        let mut screen = TitleScreen::new(&defs, None, 1);
        // Load is the second item without a Continue slot.
        press(&mut screen, KeyCode::Down);
        assert_eq!(screen.items()[screen.cursor], MenuItem::Load);
        press(&mut screen, KeyCode::Enter);
        assert_eq!(screen.page, Page::Load);
        press(&mut screen, KeyCode::Enter); // nothing to load
        assert!(screen.status.is_empty());
        press(&mut screen, KeyCode::Esc);
        assert_eq!(screen.page, Page::Menu);
    }

    #[test]
    fn a_release_event_does_nothing() {
        let defs = Defs::bundled();
        let mut screen = TitleScreen::new(&defs, None, 1);
        let release = KeyEvent {
            code: KeyCode::Down,
            modifiers: KeyModifiers::NONE,
            kind: KeyEventKind::Release,
            state: KeyEventState::NONE,
        };
        assert!(matches!(screen.key(release, &defs, None), Outcome::None));
        assert_eq!(screen.cursor, 0);
    }
}
