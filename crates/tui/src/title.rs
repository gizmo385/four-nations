//! The title screen (#280): before a game exists, New game (pick a nation and a seed), Continue
//! (the newest loadable slot, hidden with none), Load (every slot, as the in-game saves screen
//! shows them), Replays (the run browser on the agent runs folder, #841; hidden with no runs)
//! and Quit. [`crate::shell::Shell`] is the only thing that drives this: it holds
//! either this or the game, and switches between them on the outcome of a key.
//!
//! Like [`crate::state::UiState`], nothing here touches the terminal, so the keys and the golden
//! frames can drive it directly.

use fn_ui::saves::SaveStore;

use crate::keyev::{KeyCode, KeyEvent, KeyEventKind};
use fn_sim::embark::SurveyMode;
use fn_sim::{Defs, Nation, World};
use fn_ui::keys;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Style;

use crate::color::Palette;
use crate::draw::too_small_text;
use crate::layout::{MIN_HEIGHT, MIN_WIDTH};
use crate::widgets::{
    BarBody, Painter, Row, TitleAt, Tone, ToneStyle, draw_frame, draw_hint_bar, wrap,
};

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
    /// The run browser (#841). Hidden unless [`TitleScreen::replays`] says there are runs.
    Replays,
    Quit,
}

impl MenuItem {
    pub fn label(self) -> &'static str {
        match self {
            Self::NewGame => "New game",
            Self::Continue => "Continue",
            Self::Load => "Load",
            Self::Replays => "Replays",
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
    /// The chosen nation, with the seed, map size and survey rows (#290): `Enter` opens embark
    /// for them instead of starting the game straight away.
    Seed {
        nation: Nation,
        digits: String,
        edited: bool,
        /// Which row is highlighted: moved with up/down, changed with left/right (digits are
        /// typed only on [`SeedRow::Digits`]).
        row: SeedRow,
        /// An index into `Defs::embark`'s `map_sizes`.
        size_idx: usize,
        survey: SurveyMode,
        /// An index into `Defs::difficulty`'s `presets` (#298): the New game picker's choice of
        /// preset, in file order. Defaults to `Defs::difficulty`'s own `default`.
        difficulty_idx: usize,
    },
    /// Every save, as the in-game saves screen (`F6`) shows them.
    Load,
}

/// A row of the seed step (#290, docs/design/embark.md; Difficulty added by #298).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SeedRow {
    Digits,
    Size,
    Survey,
    Difficulty,
}

impl SeedRow {
    const ALL: [SeedRow; 4] =
        [SeedRow::Digits, SeedRow::Size, SeedRow::Survey, SeedRow::Difficulty];
}

/// What a key did, for [`crate::shell::Shell`] to act on.
#[derive(Debug)]
pub enum Outcome {
    None,
    /// `Enter` on the seed step: open embark for this seed, nation, size, survey mode and
    /// difficulty preset instead of starting the game straight away (#290, #298).
    Embark {
        seed: u64,
        nation: Nation,
        size: fn_sim::embark::MapSizeDef,
        mode: SurveyMode,
        /// The chosen preset's id (`data/difficulty.toml`), or `None` if the data has no
        /// presets at all.
        difficulty: Option<String>,
    },
    /// Continue or Load: a world ready to become the game.
    Loaded(Box<World>),
    /// Replays (#841): open the run browser. The title screen stays as it is underneath, so
    /// coming back finds the cursor still on Replays.
    Replays,
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
    /// Whether the agent runs folder holds anything to replay (#841): `Shell` decides, since
    /// only it knows the folder. Off by default, and always off on wasm32 (no run browser there).
    pub replays: bool,
    /// Whether the menu has a Quit row (#866): on by default, off in the browser, where there
    /// is nothing to quit to. `Shell` sets it (`Shell::with_quit`).
    pub quit: bool,
    /// The seed a fresh New Game offers until the player types over it: bumped each time a new
    /// title screen starts (`Shell`'s own job, since only it may read the wall clock), so
    /// starting over and over doesn't always suggest the same seed.
    seed_hint: u64,
}

impl TitleScreen {
    pub fn new(saves: Option<&dyn SaveStore>, seed_hint: u64) -> Self {
        let save_slots = list_slots(saves);
        let continue_slot = save_slots.iter().find(|s| s.loadable()).cloned();
        Self {
            page: Page::Menu,
            cursor: 0,
            continue_slot,
            save_slots,
            load_cursor: 0,
            status: String::new(),
            replays: false,
            quit: true,
            seed_hint,
        }
    }

    /// Shows or hides the Replays item (#841), keeping the cursor on the row it was on.
    pub fn set_replays(&mut self, replays: bool) {
        let on = self.items().get(self.cursor).copied();
        self.replays = replays;
        self.cursor = on.and_then(|i| self.items().iter().position(|&x| x == i)).unwrap_or(0);
    }

    /// [`Self::set_replays`], for building one.
    pub fn with_replays(mut self, replays: bool) -> Self {
        self.set_replays(replays);
        self
    }

    /// Shows or hides the Quit item (#866), keeping the cursor on the row it was on.
    pub fn set_quit(&mut self, quit: bool) {
        let on = self.items().get(self.cursor).copied();
        self.quit = quit;
        self.cursor = on.and_then(|i| self.items().iter().position(|&x| x == i)).unwrap_or(0);
    }

    /// [`Self::set_quit`], for building one.
    pub fn with_quit(mut self, quit: bool) -> Self {
        self.set_quit(quit);
        self
    }

    /// The menu rows that show right now: `Continue` only with somewhere to continue.
    pub fn items(&self) -> Vec<MenuItem> {
        let mut items = vec![MenuItem::NewGame];
        if self.continue_slot.is_some() {
            items.push(MenuItem::Continue);
        }
        items.push(MenuItem::Load);
        if self.replays {
            items.push(MenuItem::Replays);
        }
        if self.quit {
            items.push(MenuItem::Quit);
        }
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
    pub fn key(&mut self, key: KeyEvent, defs: &Defs, saves: Option<&dyn SaveStore>) -> Outcome {
        if key.kind == KeyEventKind::Release {
            return Outcome::None;
        }
        self.status.clear();
        match self.page.clone() {
            Page::Menu => self.key_menu(key, defs, saves),
            Page::Nation { cursor } => {
                self.key_nation(key, defs, cursor);
                Outcome::None
            }
            Page::Seed { nation, digits, edited, row, size_idx, survey, difficulty_idx } => self
                .key_seed(key, defs, nation, digits, edited, row, size_idx, survey, difficulty_idx),
            Page::Load => self.key_load(key, defs, saves),
        }
    }

    fn key_menu(&mut self, key: KeyEvent, defs: &Defs, saves: Option<&dyn SaveStore>) -> Outcome {
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

    fn choose(&mut self, item: MenuItem, defs: &Defs, saves: Option<&dyn SaveStore>) -> Outcome {
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
                self.save_slots = list_slots(saves);
                self.load_cursor = 0;
                self.page = Page::Load;
                Outcome::None
            }
            MenuItem::Replays => Outcome::Replays,
            MenuItem::Quit => Outcome::Quit,
        }
    }

    fn load_named(&mut self, name: &str, defs: &Defs, saves: Option<&dyn SaveStore>) -> Outcome {
        let Some(store) = saves else {
            self.status = NO_SAVE_DIR.to_owned();
            return Outcome::None;
        };
        match fn_ui::saves::load(store, name, defs) {
            Ok(world) => Outcome::Loaded(Box::new(world)),
            Err(why) => {
                self.status = format!("Load failed: {why}");
                Outcome::None
            }
        }
    }

    fn key_nation(&mut self, key: KeyEvent, defs: &Defs, cursor: usize) {
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
                self.page = Page::Seed {
                    nation,
                    digits: self.seed_hint.to_string(),
                    edited: false,
                    row: SeedRow::Digits,
                    size_idx: 0,
                    survey: SurveyMode::Exact,
                    difficulty_idx: default_difficulty_idx(defs),
                };
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

    /// Four rows (#290, #298): the seed's own digits, then Size, Survey and Difficulty, moved
    /// between with up/down and changed with left/right. `Enter` opens embark from any of them.
    #[allow(clippy::too_many_arguments)]
    fn key_seed(
        &mut self,
        key: KeyEvent,
        defs: &Defs,
        nation: Nation,
        mut digits: String,
        mut edited: bool,
        mut row: SeedRow,
        mut size_idx: usize,
        mut survey: SurveyMode,
        mut difficulty_idx: usize,
    ) -> Outcome {
        let sizes = defs.embark.map_sizes.len().max(1);
        let presets = defs.difficulty.presets.len().max(1);
        match key.code {
            KeyCode::Char(c) if c.is_ascii_digit() && row == SeedRow::Digits => {
                if !edited {
                    digits.clear();
                    edited = true;
                }
                if digits.len() < MAX_SEED_DIGITS {
                    digits.push(c);
                }
            }
            KeyCode::Backspace if row == SeedRow::Digits => {
                edited = true;
                digits.pop();
            }
            KeyCode::Up | KeyCode::Char('w' | 'W') => row = step_row(row, -1),
            KeyCode::Down | KeyCode::Char('s' | 'S') => row = step_row(row, 1),
            KeyCode::Left | KeyCode::Char('a' | 'A') if row == SeedRow::Size => {
                size_idx = (size_idx + sizes - 1) % sizes;
            }
            KeyCode::Right | KeyCode::Char('d' | 'D') if row == SeedRow::Size => {
                size_idx = (size_idx + 1) % sizes;
            }
            KeyCode::Left | KeyCode::Right | KeyCode::Char('a' | 'A' | 'd' | 'D')
                if row == SeedRow::Survey =>
            {
                survey = toggle_survey(survey);
            }
            KeyCode::Left | KeyCode::Char('a' | 'A') if row == SeedRow::Difficulty => {
                difficulty_idx = (difficulty_idx + presets - 1) % presets;
            }
            KeyCode::Right | KeyCode::Char('d' | 'D') if row == SeedRow::Difficulty => {
                difficulty_idx = (difficulty_idx + 1) % presets;
            }
            KeyCode::Enter => {
                let seed =
                    if digits.is_empty() { self.seed_hint } else { digits.parse().unwrap_or(0) };
                let size = defs
                    .embark
                    .map_sizes
                    .get(size_idx)
                    .cloned()
                    .or_else(|| defs.embark.map_sizes.first().cloned())
                    .unwrap_or(fn_sim::embark::MapSizeDef {
                        id: "small".into(),
                        name: "Small".into(),
                        width: 192,
                        height: 128,
                        continent_frequency: None,
                    });
                let difficulty = defs.difficulty.presets.get(difficulty_idx).map(|p| p.id.clone());
                return Outcome::Embark { seed, nation, size, mode: survey, difficulty };
            }
            KeyCode::Esc => {
                let cursor = Nation::ALL.iter().position(|&n| n == nation).unwrap_or(0);
                self.page = Page::Nation { cursor };
                return Outcome::None;
            }
            _ => {}
        }
        self.page = Page::Seed { nation, digits, edited, row, size_idx, survey, difficulty_idx };
        Outcome::None
    }

    fn key_load(&mut self, key: KeyEvent, defs: &Defs, saves: Option<&dyn SaveStore>) -> Outcome {
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

/// Every save in `saves`, newest first, or none without a store. A listing error is folded
/// into an empty list; it would otherwise have nowhere to show before the Load page is even open.
fn list_slots(saves: Option<&dyn SaveStore>) -> Vec<fn_ui::saves::Slot> {
    saves.map(|store| fn_ui::saves::list(store).unwrap_or_default()).unwrap_or_default()
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

/// The seed step's next row after a step of `dy` (its sign only), stopping at the edges (#290).
fn step_row(row: SeedRow, dy: i32) -> SeedRow {
    let at = SeedRow::ALL.iter().position(|&r| r == row).unwrap_or(0);
    let last = SeedRow::ALL.len() - 1;
    SeedRow::ALL[(at as i64 + i64::from(dy.signum())).clamp(0, last as i64) as usize]
}

fn toggle_survey(mode: SurveyMode) -> SurveyMode {
    match mode {
        SurveyMode::Exact => SurveyMode::Rough,
        SurveyMode::Rough => SurveyMode::Exact,
    }
}

/// The seed step's hint for whichever row is highlighted (#290, #298).
fn seed_row_hint(
    row: SeedRow,
    defs: &Defs,
    size_idx: usize,
    survey: SurveyMode,
    difficulty_idx: usize,
) -> String {
    match row {
        SeedRow::Digits => "Type digits, Backspace to edit.".to_owned(),
        SeedRow::Size => match defs.embark.map_sizes.get(size_idx) {
            Some(s) => format!("{}: {}x{} tiles.", s.name, s.width, s.height),
            None => String::new(),
        },
        SeedRow::Survey => match survey {
            SurveyMode::Exact => "Exact: you'll see exactly what's at every site.".to_owned(),
            SurveyMode::Rough => {
                "Rough: you won't know exactly what's there until you arrive.".to_owned()
            }
        },
        SeedRow::Difficulty => match defs.difficulty.presets.get(difficulty_idx) {
            Some(p) => p.hint.clone(),
            None => String::new(),
        },
    }
}

/// The preset `Page::Seed` offers until the player changes it: `Defs::difficulty`'s own
/// `default`, or the first preset if `default` somehow names none, or 0 with no presets at all
/// (data without `Defs::with_difficulty`, in tests).
pub(crate) fn default_difficulty_idx(defs: &Defs) -> usize {
    defs.difficulty.presets.iter().position(|p| p.id == defs.difficulty.default).unwrap_or(0)
}

// -------------------------------------------------------------------------------------------
// Drawing
// -------------------------------------------------------------------------------------------

/// The hint bar's mode for `page` (`fn_ui::keys`): each page's keys are the table's, drawn on the
/// screen's bottom row, never in the box (#686).
pub fn hint_mode(page: &Page) -> keys::Mode {
    match page {
        Page::Menu => keys::Mode::Title,
        Page::Nation { .. } => keys::Mode::TitleNation,
        Page::Seed { .. } => keys::Mode::TitleSeed,
        Page::Load => keys::Mode::TitleLoad,
    }
}

/// The title box's widest.
const BOX_WIDTH: u16 = 64;
/// Lines a page reserves for its wrapped description or hint, so the box keeps one height while
/// the cursor moves between rows whose text wraps to different lengths.
const NATION_DESCRIPTION_LINES: usize = 4;
const SEED_HINT_LINES: usize = 3;
/// The save list's own rows, which scroll inside whatever height the box gives them.
const LOAD_ROWS: u16 = 20;

/// The rows `page` draws inside the frame: what it shows, not a fixed 22, so a short page gets a
/// short box instead of one mostly empty.
fn content_rows(screen: &TitleScreen) -> u16 {
    let rows = match &screen.page {
        // The welcome line and a blank, the items, then room for a status line and its blank.
        Page::Menu => 2 + screen.items().len() + 2,
        // The prompt and a blank, one row per nation, a blank, then the description.
        Page::Nation { .. } => 2 + Nation::ALL.len() + 1 + NATION_DESCRIPTION_LINES,
        // The heading and a blank, the four rows, a blank, then the hint.
        Page::Seed { .. } => 2 + SeedRow::ALL.len() + 1 + SEED_HINT_LINES,
        Page::Load => return LOAD_ROWS,
    };
    u16::try_from(rows).unwrap_or(u16::MAX)
}

/// The box the title screen draws in: centred, at most [`BOX_WIDTH`] columns wide and as tall
/// as the page's content plus its frame.
fn box_area(area: Rect, rows: u16) -> Rect {
    let width = area.width.min(BOX_WIDTH);
    let height = area.height.min(rows.saturating_add(2));
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
    // The last row is the hint bar's; the box is centred over the rows above it.
    let above = Rect::new(area.x, area.y, area.width, area.height - 1);
    let outer = box_area(above, content_rows(screen));
    draw_frame(buf, outer, palette, &title, TitleAt::Centre, "");
    let bar = Rect::new(area.x, area.bottom() - 1, area.width, 1);
    draw_hint_bar(buf, bar, palette, hint_mode(&screen.page), BarBody::Keys, false, false);
    let inner = Rect::new(outer.x + 2, outer.y + 1, outer.width - 4, outer.height - 2);

    if screen.page == Page::Load {
        return draw_load(screen, defs, palette, inner, buf);
    }
    let mut p = Painter::new(buf, inner, palette);
    match &screen.page {
        Page::Menu => draw_menu(screen, &mut p),
        Page::Nation { cursor } => draw_nation(defs, *cursor, &mut p),
        Page::Seed { nation, digits, row, size_idx, survey, difficulty_idx, .. } => {
            draw_seed(defs, *nation, digits, *row, *size_idx, *survey, *difficulty_idx, &mut p)
        }
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
    for line in wrap(&chosen.description, width, NATION_DESCRIPTION_LINES, p.marks.ellipsis) {
        p.row(&Row::text(line, Tone::Text));
    }
}

/// The seed step (#290, #298): the seed, then Size, Survey and Difficulty, each moved to with
/// up/down and changed with left/right, with the highlighted row's hint below.
#[allow(clippy::too_many_arguments)]
fn draw_seed(
    defs: &Defs,
    nation: Nation,
    digits: &str,
    row: SeedRow,
    size_idx: usize,
    survey: SurveyMode,
    difficulty_idx: usize,
    p: &mut Painter,
) {
    let def = defs.nation(nation);
    p.row(&Row::text(format!("{} — new game:", def.name), Tone::Title));
    p.blank();
    let pointer = p.marks.pointer;
    let mark = |r: SeedRow| if r == row { pointer } else { " " };
    let tone = |r: SeedRow| if r == row { Tone::Title } else { Tone::Text };
    p.row(&Row::text(format!("{} Seed: {digits}", mark(SeedRow::Digits)), tone(SeedRow::Digits)));
    let size_name = defs.embark.map_sizes.get(size_idx).map_or("Small", |s| s.name.as_str());
    p.row(&Row::text(format!("{} Size: {size_name}", mark(SeedRow::Size)), tone(SeedRow::Size)));
    let survey_name = match survey {
        SurveyMode::Exact => "Exact",
        SurveyMode::Rough => "Rough",
    };
    p.row(&Row::text(
        format!("{} Survey: {survey_name}", mark(SeedRow::Survey)),
        tone(SeedRow::Survey),
    ));
    let difficulty_name =
        defs.difficulty.presets.get(difficulty_idx).map_or("Normal", |p| p.name.as_str());
    p.row(&Row::text(
        format!("{} Difficulty: {difficulty_name}", mark(SeedRow::Difficulty)),
        tone(SeedRow::Difficulty),
    ));
    p.blank();
    // Wrapped, not cut off: a difficulty's hint runs well past one line. The keys live in the
    // hint bar (`hint_mode`) and the seed row's own hint says how to edit the digits, so the
    // page doesn't repeat them.
    let hint = seed_row_hint(row, defs, size_idx, survey, difficulty_idx);
    let width = usize::from(p.inner.width);
    for line in wrap(&hint, width, SEED_HINT_LINES, p.marks.ellipsis) {
        p.row(&Row::text(line, Tone::Dim));
    }
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

/// Over the embark screen: `Q` asks whether to go back to the title screen or exit the process
/// (#280). A game's Quit… box asks both itself (#703). The keys are the hint bar's `EXIT` mode
/// (`fn_ui::keys`), drawn on the screen's bottom row, not in the box.
pub fn draw_exit_prompt(palette: &Palette, area: Rect, buf: &mut Buffer) {
    let width = area.width.min(50);
    let height = 3u16.min(area.height);
    let outer = Rect::new(
        area.x + (area.width - width) / 2,
        area.y + (area.height - height) / 2,
        width,
        height,
    );
    draw_frame(buf, outer, palette, " QUIT? ", TitleAt::Centre, "");
    if outer.height >= 3 {
        let inner = Rect::new(outer.x + 2, outer.y + 1, outer.width - 4, outer.height - 2);
        let mut p = Painter::new(buf, inner, palette);
        p.row(&Row::text("Go back to the title screen, or exit?", Tone::Text));
    }
    let bar = Rect::new(area.x, area.bottom().saturating_sub(1), area.width, 1);
    draw_hint_bar(buf, bar, palette, keys::Mode::Exit, BarBody::Keys, false, false);
}

#[cfg(test)]
mod tests {
    use crate::keyev::{KeyEventState, KeyModifiers};
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
        let screen = TitleScreen::new(None, 1);
        assert_eq!(
            screen.items(),
            [MenuItem::NewGame, MenuItem::Load, MenuItem::Quit],
            "no save directory: nothing to continue"
        );
    }

    #[test]
    fn quit_is_hidden_when_the_host_cannot_quit() {
        let mut screen = TitleScreen::new(None, 1);
        screen.cursor = screen.items().len() - 1;
        screen.set_quit(false);
        assert_eq!(screen.items(), [MenuItem::NewGame, MenuItem::Load]);
        assert!(screen.cursor < screen.items().len(), "the cursor stays on a row");
        let screen = TitleScreen::new(None, 1).with_replays(true).with_quit(false);
        assert_eq!(screen.items(), [MenuItem::NewGame, MenuItem::Load, MenuItem::Replays]);
    }

    #[test]
    fn replays_shows_between_load_and_quit_only_when_there_are_runs() {
        let hidden = TitleScreen::new(None, 1);
        assert!(!hidden.items().contains(&MenuItem::Replays), "no runs: no Replays");
        let mut shown = TitleScreen::new(None, 1).with_replays(true);
        assert_eq!(
            shown.items(),
            [MenuItem::NewGame, MenuItem::Load, MenuItem::Replays, MenuItem::Quit]
        );
        press(&mut shown, KeyCode::Down);
        press(&mut shown, KeyCode::Down);
        assert!(matches!(press(&mut shown, KeyCode::Enter), Outcome::Replays));
        assert_eq!(shown.page, Page::Menu, "the menu stays put underneath the browser");
        assert_eq!(shown.items()[shown.cursor], MenuItem::Replays);
    }

    #[test]
    fn the_menu_cursor_stops_at_the_edges() {
        let mut screen = TitleScreen::new(None, 1);
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
        let mut screen = TitleScreen::new(None, 42);
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
        assert!(matches!(outcome, Outcome::Embark { seed: 42, nation: Nation::Fire, .. }));
    }

    #[test]
    fn typing_digits_replaces_the_default_seed_and_backspace_edits_it() {
        let mut screen = TitleScreen::new(None, 999);
        screen.page = Page::Seed {
            nation: Nation::Water,
            digits: "999".into(),
            edited: false,
            row: SeedRow::Digits,
            size_idx: 0,
            survey: SurveyMode::Exact,
            difficulty_idx: 0,
        };
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
        assert!(matches!(outcome, Outcome::Embark { seed: 75, nation: Nation::Water, .. }));
    }

    /// The Size and Survey rows (#290): up/down moves between the three rows, left/right
    /// changes the highlighted one, and `Enter` carries the choice into the embark outcome.
    #[test]
    fn size_and_survey_rows_move_with_up_down_and_change_with_left_right() {
        let defs = Defs::bundled();
        let mut screen = TitleScreen::new(None, 7);
        press(&mut screen, KeyCode::Enter); // New game
        press(&mut screen, KeyCode::Enter); // Air, to the seed page
        press(&mut screen, KeyCode::Down); // Size row
        press(&mut screen, KeyCode::Down); // Survey row
        if let Page::Seed { row, .. } = &screen.page {
            assert_eq!(*row, SeedRow::Survey);
        } else {
            panic!("not on the seed page");
        }
        press(&mut screen, KeyCode::Left);
        if let Page::Seed { survey, .. } = &screen.page {
            assert_eq!(*survey, SurveyMode::Rough);
        } else {
            panic!("not on the seed page");
        }
        press(&mut screen, KeyCode::Up); // back to Size
        press(&mut screen, KeyCode::Right);
        if let Page::Seed { size_idx, .. } = &screen.page {
            assert_eq!(*size_idx, 1);
        } else {
            panic!("not on the seed page");
        }
        let outcome = press(&mut screen, KeyCode::Enter);
        match outcome {
            Outcome::Embark {
                seed: 7, nation: Nation::Air, size, mode: SurveyMode::Rough, ..
            } => {
                assert_eq!(size.id, defs.embark.map_sizes[1].id);
            }
            other => panic!("{other:?}"),
        }
    }

    /// The Difficulty row (#298): it defaults to `data/difficulty.toml`'s own `default`, cycles
    /// in file order with left/right and wraps, and `Enter` carries its id into the outcome.
    #[test]
    fn difficulty_row_defaults_cycles_in_file_order_and_wraps_into_the_outcome() {
        let defs = Defs::bundled();
        let mut screen = TitleScreen::new(None, 7);
        press(&mut screen, KeyCode::Enter); // New game
        press(&mut screen, KeyCode::Enter); // Air, to the seed page
        if let Page::Seed { difficulty_idx, .. } = &screen.page {
            let default = defs.difficulty.presets[*difficulty_idx].id.as_str();
            assert_eq!(default, defs.difficulty.default, "starts on the data's own default");
        } else {
            panic!("not on the seed page");
        }
        for _ in 0..3 {
            press(&mut screen, KeyCode::Down); // Size, Survey, Difficulty
        }
        if let Page::Seed { row, .. } = &screen.page {
            assert_eq!(*row, SeedRow::Difficulty);
        } else {
            panic!("not on the seed page");
        }
        press(&mut screen, KeyCode::Left); // normal (idx 1) -> sandbox (idx 0)
        if let Page::Seed { difficulty_idx, .. } = &screen.page {
            assert_eq!(*difficulty_idx, 0);
        } else {
            panic!("not on the seed page");
        }
        press(&mut screen, KeyCode::Left); // wraps to the last preset
        if let Page::Seed { difficulty_idx, .. } = &screen.page {
            assert_eq!(*difficulty_idx, defs.difficulty.presets.len() - 1);
        } else {
            panic!("not on the seed page");
        }
        let outcome = press(&mut screen, KeyCode::Enter);
        match outcome {
            Outcome::Embark { difficulty: Some(id), .. } => {
                assert_eq!(id, defs.difficulty.presets.last().unwrap().id);
            }
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn escape_steps_back_a_page_at_a_time() {
        let mut screen = TitleScreen::new(None, 1);
        press(&mut screen, KeyCode::Enter); // New game
        press(&mut screen, KeyCode::Enter); // Air, to the seed page
        press(&mut screen, KeyCode::Esc);
        assert!(matches!(screen.page, Page::Nation { cursor: 0 }));
        press(&mut screen, KeyCode::Esc);
        assert_eq!(screen.page, Page::Menu);
    }

    #[test]
    fn an_empty_seed_before_any_typing_falls_back_to_the_hint() {
        let mut screen = TitleScreen::new(None, 7);
        press(&mut screen, KeyCode::Enter); // New game
        press(&mut screen, KeyCode::Enter); // Air, to the seed page
        let outcome = press(&mut screen, KeyCode::Enter); // Enter alone: the hint
        assert!(matches!(outcome, Outcome::Embark { seed: 7, nation: Nation::Air, .. }));
    }

    #[test]
    fn quit_from_the_menu_quits_at_once_with_no_further_question() {
        let mut screen = TitleScreen::new(None, 1);
        for _ in 0..3 {
            press(&mut screen, KeyCode::Down);
        }
        assert_eq!(screen.items()[screen.cursor], MenuItem::Quit);
        assert!(matches!(press(&mut screen, KeyCode::Enter), Outcome::Quit));
    }

    #[test]
    fn with_no_save_directory_load_says_so_and_continue_does_nothing() {
        let mut screen = TitleScreen::new(None, 1);
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
        let mut screen = TitleScreen::new(None, 1);
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
