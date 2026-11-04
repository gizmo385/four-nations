//! What the player is looking at: the cursor, the part of the map in view and the modal state.
//! Nothing here touches the terminal, so the render and input tests drive it directly.

use fn_sim::building::BuildingId;
use fn_sim::world::{Unit, UnitId};
use fn_sim::{Nation, Pos};
use fn_ui::build::Placement;
use fn_ui::saves::Slot;
use fn_ui::{Alerts, Session, Tool};

use crate::layout;

/// How close (in tiles) the cursor may come to the edge of the view before the view scrolls.
pub const SCROLL_MARGIN: i32 = 3;

/// How tall the selection bar is, in rows of the map area.
pub const BAR_HEIGHT: i32 = 7;

/// A y/n question that takes over the keys until it is answered.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Prompt {
    Quit,
    /// `R`: a new map (the next seed, the same nation).
    NewMap,
    /// `1`-`4`: the same map, played as this nation.
    Nation(Nation),
    /// `F9`: the quicksave replaces the game.
    QuickLoad,
    /// `Enter` in the saves screen: loads the named slot in place of the game.
    LoadSave(String),
    /// `S` in the saves screen: writes over the named slot.
    SaveOver(String),
}

/// A screen over the map area (the side panel stays).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Screen {
    Log,
    Help,
    /// `T`: the tech tree. It moves a card cursor ([`UiState::tech`]) and does not scroll.
    Research,
    /// `F6`: the save slots. It moves a highlight ([`UiState::save_cursor`]) over
    /// [`UiState::save_slots`] and does not scroll.
    Saves,
}

/// A modal dialog over the map (`crate::modal`, #285). Unlike a [`Screen`] it draws as a centred
/// box, not a full page, and it captures every key: no tool key, screen key or clock action
/// reaches `App::act` while one is open (`App::modal_key`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Modal {
    /// `U`: every building the nation can place or may yet unlock, highlighted one at a time
    /// ([`UiState::build_cursor`], `crate::build`). `Enter` on an unlocked one picks it and
    /// leaves the Build tool active with it; `Esc` leaves the tool and the choice as they were.
    Build,
}

/// The scroll of a screen that sticks to its end: the newest line of the log.
pub const PINNED: usize = usize::MAX;

#[derive(Clone, Debug, PartialEq)]
pub struct UiState {
    /// The tile under the keyboard cursor, always on the map.
    pub cursor: Pos,
    /// The map tile at the top-left of the view.
    pub view: Pos,
    /// How many tiles the view shows across and down (the map region of the terminal).
    viewport: (i32, i32),
    /// The map's width and height in tiles.
    bounds: (i32, i32),
    /// The active tool: `Select` until a tool key (`H` `X` `P` `B` `U`) picks another.
    pub tool: Tool,
    /// The building the Build tool places, kept across tool changes so `U` returns to it
    /// (`fn_ui::tools::switch_tool`; the client keeps the same choice in its own `BuildChoice`
    /// resource, since it belongs to the frontend, not the `Session`).
    pub choice: Option<BuildingId>,
    /// The rectangle's first corner, set by the tool's first `Enter`. The rectangle runs from
    /// here to the cursor; only a tool has one, and switching tools drops it.
    pub anchor: Option<Pos>,
    /// The last rectangle's result ("designate harvest: 12 done, 3 skipped"), shown in the Tool
    /// section until the tool changes.
    pub notice: String,
    /// A save or load result ("Quicksaved (tick 40)"): it takes the status line's key hints
    /// until the next key.
    pub status: String,
    pub prompt: Option<Prompt>,
    pub quit: bool,
    /// The unit the selection bar is about, if the bar is open.
    pub selected: Option<UnitId>,
    /// Whether the cursor tracks the selected unit (`F`).
    pub following: bool,
    /// The screen over the map, if one is open.
    pub screen: Option<Screen>,
    /// The first line of the open screen that shows ([`PINNED`] for the end of the log).
    pub scroll: usize,
    /// The modal dialog over the map, if one is open (#285).
    pub modal: Option<Modal>,
    /// The Build modal's highlight: an index into `fn_ui::build::picker_rows`.
    pub build_cursor: usize,
    /// The research screen's cursor: a column of the tech tree and a card in it.
    pub tech: (usize, usize),
    /// The saves screen's slots (newest first), read from disk when the screen opens or a save
    /// changes them ([`crate::app::App::refresh_saves`]), not on every frame.
    pub save_slots: Vec<Slot>,
    /// The saves screen's highlight: an index into [`Self::save_slots`].
    pub save_cursor: usize,
    /// Bend results, tech completions and deaths: the Alerts section and the message log.
    pub alerts: Alerts,
}

impl UiState {
    /// The cursor on the first settler, with the view centred on it, for a terminal of
    /// `screen` columns by rows.
    pub fn new(session: &Session, screen: (u16, u16)) -> Self {
        let map = &session.world.map;
        let cursor = session
            .world
            .units
            .first()
            .map_or(Pos::new(map.width() / 2, map.height() / 2), |u| u.pos);
        let mut ui = Self {
            cursor,
            view: Pos::new(0, 0),
            viewport: layout::viewport(screen.0, screen.1),
            bounds: (map.width(), map.height()),
            tool: Tool::Select,
            choice: None,
            anchor: None,
            notice: String::new(),
            status: String::new(),
            prompt: None,
            quit: false,
            selected: None,
            following: false,
            screen: None,
            scroll: 0,
            modal: None,
            build_cursor: 0,
            tech: (0, 0),
            save_slots: Vec::new(),
            save_cursor: 0,
            alerts: Alerts::default(),
        };
        ui.center_view();
        ui
    }

    /// The map region of the terminal, in tiles.
    pub fn viewport(&self) -> (i32, i32) {
        self.viewport
    }

    /// The part of [`Self::viewport`] the selection bar leaves free: the view keeps the cursor
    /// and the selected unit inside it.
    pub fn visible(&self) -> (i32, i32) {
        let (w, h) = self.viewport;
        if self.selected.is_some() { (w, (h - BAR_HEIGHT).max(1)) } else { (w, h) }
    }

    /// The terminal changed size: the view keeps the cursor in sight at the new size.
    pub fn resize(&mut self, width: u16, height: u16) {
        self.viewport = layout::viewport(width, height);
        self.follow();
    }

    /// Moves the cursor `(dx, dy)` tiles, stopping at the map edge, and scrolls the view if the
    /// cursor came within [`SCROLL_MARGIN`] of its edge.
    pub fn move_cursor(&mut self, dx: i32, dy: i32) {
        self.following = false;
        self.cursor = self.clamp_to_map(Pos::new(self.cursor.x + dx, self.cursor.y + dy));
        self.follow();
    }

    /// Jumps the view `(dx, dy)` quarter screens and carries the cursor the same distance
    /// (Ctrl+arrows, PageUp and PageDown).
    pub fn jump(&mut self, dx: i32, dy: i32) {
        self.following = false;
        let (w, h) = self.visible();
        let (dx, dy) = (dx * (w / 4).max(1), dy * (h / 4).max(1));
        self.view = self.clamp_view(Pos::new(self.view.x + dx, self.view.y + dy));
        self.cursor = self.clamp_to_map(Pos::new(self.cursor.x + dx, self.cursor.y + dy));
        self.follow();
    }

    /// Switches to `tool`, dropping the corner and the last result. The building choice is not
    /// a tool's own state (`fn_ui::tools::switch_tool` resolves it): it outlives the switch, so
    /// coming back to Build returns to it.
    pub fn set_tool(&mut self, tool: Tool) {
        self.tool = tool;
        self.anchor = None;
        self.notice.clear();
    }

    /// What the active tool's hint says: generic for most tools, as `fn_ui`'s own
    /// `Tool::hint` has it (it already names the ability for `Bend`, from the session's own
    /// choice). `Build`'s choice belongs to the frontend, not the `Session`, so it's named here:
    /// the building it would place, or the tool's plain label with none placeable.
    pub fn tool_hint(&self, session: &Session) -> String {
        if self.tool != Tool::Build {
            return self.tool.hint(session);
        }
        let rows = fn_ui::build::picker_rows(session);
        match fn_ui::build::valid_choice(&rows, self.choice)
            .and_then(|id| rows.iter().find(|r| r.id == id))
        {
            Some(row) => format!("build {}: Enter to place (U: pick a building)", row.name),
            None => self.tool.hint(session),
        }
    }

    /// The building the Build tool's ghosts would place (`fn_ui::build::valid_choice`): `None`
    /// outside the Build tool, or with nothing placeable yet.
    pub fn ghost_building(&self, session: &Session) -> Option<BuildingId> {
        if self.tool != Tool::Build {
            return None;
        }
        fn_ui::build::valid_choice(&fn_ui::build::picker_rows(session), self.choice)
    }

    /// The Build tool's ghosts (`fn_ui::build::ghosts`, #212): the footprint under the cursor
    /// with no corner set, or one per stamp of the rectangle being dragged. Empty outside the
    /// Build tool, or with nothing chosen yet.
    pub fn ghosts(&self, session: &Session) -> Vec<Placement> {
        match self.ghost_building(session) {
            Some(b) => fn_ui::build::ghosts(session, self.anchor, self.cursor, b),
            None => Vec::new(),
        }
    }

    /// The corners of the rectangle being dragged: the anchor and the cursor, if a tool has an
    /// anchor.
    pub fn rect(&self) -> Option<(Pos, Pos)> {
        (self.tool != Tool::Select).then_some(self.anchor).flatten().map(|a| (a, self.cursor))
    }

    /// The rectangle being dragged as (width, height) in tiles.
    pub fn rect_size(&self) -> Option<(i32, i32)> {
        self.rect().map(|(a, b)| ((a.x - b.x).abs() + 1, (a.y - b.y).abs() + 1))
    }

    /// Whether `pos` is inside the rectangle being dragged.
    pub fn in_rect(&self, pos: Pos) -> bool {
        self.rect().is_some_and(|(a, b)| {
            (a.x.min(b.x)..=a.x.max(b.x)).contains(&pos.x)
                && (a.y.min(b.y)..=a.y.max(b.y)).contains(&pos.y)
        })
    }

    fn clamp_to_map(&self, p: Pos) -> Pos {
        Pos::new(p.x.clamp(0, self.bounds.0 - 1), p.y.clamp(0, self.bounds.1 - 1))
    }

    /// `view` kept within the map: the view never shows past an edge, and a map smaller than
    /// the view sits at the origin.
    fn clamp_view(&self, view: Pos) -> Pos {
        let max = |bound: i32, shown: i32| (bound - shown).max(0);
        let shown = self.visible();
        Pos::new(
            view.x.clamp(0, max(self.bounds.0, shown.0)),
            view.y.clamp(0, max(self.bounds.1, shown.1)),
        )
    }

    /// Scrolls just far enough that the cursor is [`SCROLL_MARGIN`] tiles inside the view's edges
    /// (less in a view too small for that), then keeps the view on the map.
    fn follow(&mut self) {
        let scroll = |view: i32, cursor: i32, shown: i32| {
            let margin = SCROLL_MARGIN.min((shown - 1) / 2).max(0);
            if cursor < view + margin {
                cursor - margin
            } else if cursor > view + shown - 1 - margin {
                cursor - (shown - 1 - margin)
            } else {
                view
            }
        };
        let shown = self.visible();
        let view = Pos::new(
            scroll(self.view.x, self.cursor.x, shown.0),
            scroll(self.view.y, self.cursor.y, shown.1),
        );
        self.view = self.clamp_view(view);
    }

    fn center_view(&mut self) {
        let shown = self.visible();
        let view = Pos::new(self.cursor.x - shown.0 / 2, self.cursor.y - shown.1 / 2);
        self.view = self.clamp_view(view);
    }

    /// Where on the map region (column, row) the cursor is drawn, if it's in view.
    pub fn cursor_cell(&self) -> Option<(i32, i32)> {
        let (x, y) = (self.cursor.x - self.view.x, self.cursor.y - self.view.y);
        let shown = self.visible();
        ((0..shown.0).contains(&x) && (0..shown.1).contains(&y)).then_some((x, y))
    }

    /// The selected unit, if the bar is open and the unit is alive.
    pub fn selected_unit<'a>(&self, session: &'a Session) -> Option<&'a Unit> {
        let id = self.selected?;
        session.world.units.iter().find(|u| u.id == id)
    }

    /// Opens the bar on `id` (or closes it). The view shifts so the cursor stays above the bar.
    pub fn select(&mut self, id: Option<UnitId>) {
        self.selected = id;
        if id.is_none() {
            self.following = false;
        }
        self.follow();
    }

    /// `Enter` in Select mode: selects the unit under the cursor; with several there, the next
    /// one after the selected. Nothing happens on a tile with no unit.
    pub fn select_here(&mut self, session: &Session) {
        let here: Vec<UnitId> =
            session.world.units.iter().filter(|u| u.pos == self.cursor).map(|u| u.id).collect();
        let next = match here.iter().position(|&id| Some(id) == self.selected) {
            Some(i) => here[(i + 1) % here.len()],
            None => match here.first() {
                Some(&id) => id,
                None => return,
            },
        };
        self.select(Some(next));
    }

    /// `Tab` (`step` 1) and Shift+`Tab` (`step` -1): the next or previous unit in the order of
    /// the people list, wrapping round; the cursor goes to it. With none selected, Tab starts at
    /// the first and Shift+Tab at the last.
    pub fn cycle(&mut self, session: &Session, step: i32) {
        let units = &session.world.units;
        if units.is_empty() {
            return;
        }
        let n = units.len() as i32;
        let at = self.selected.and_then(|id| units.iter().position(|u| u.id == id));
        let to = match (at, step.signum()) {
            (Some(i), s) => (i as i32 + s).rem_euclid(n),
            (None, s) if s < 0 => n - 1,
            (None, _) => 0,
        } as usize;
        let unit = &units[to];
        self.selected = Some(unit.id);
        self.cursor = self.clamp_to_map(unit.pos);
        self.follow();
    }

    /// `F`: starts or stops following the selected unit. Only a selected unit can be followed.
    pub fn toggle_follow(&mut self, session: &Session) {
        if self.selected_unit(session).is_none() {
            return;
        }
        self.following = !self.following;
        self.track(session);
    }

    /// After the world moved: a selected unit that died closes the bar, and a followed one drags
    /// the cursor (and the view) along.
    pub fn track(&mut self, session: &Session) {
        let Some(unit) = self.selected_unit(session) else {
            if self.selected.is_some() {
                self.select(None);
            }
            return;
        };
        if self.following {
            let pos = self.clamp_to_map(unit.pos);
            if pos != self.cursor {
                self.cursor = pos;
                self.follow();
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use fn_sim::Nation;

    use super::*;

    /// A 100x80 map (the world's own size doesn't matter here) on a 120x40 terminal: an 88x39
    /// view.
    fn ui() -> UiState {
        let mut ui = UiState::new(&Session::new(7, Nation::Earth, 100, 80), (120, 40));
        ui.cursor = Pos::new(50, 40);
        ui.center_view();
        ui
    }

    #[test]
    fn a_new_view_is_centred_on_the_cursor_and_stays_on_the_map() {
        let s = Session::new(7, Nation::Earth, 100, 80);
        let ui = UiState::new(&s, (120, 40));
        assert_eq!(ui.cursor, s.world.units[0].pos);
        assert!(ui.view.x >= 0 && ui.view.x <= 100 - 88);
        assert!(ui.view.y >= 0 && ui.view.y <= 80 - 39);
        assert!(ui.cursor_cell().is_some());
    }

    #[test]
    fn the_cursor_moves_inside_the_view_without_scrolling_it() {
        let mut ui = ui();
        let view = ui.view;
        ui.move_cursor(3, -2);
        assert_eq!(ui.cursor, Pos::new(53, 38));
        assert_eq!(ui.view, view);
    }

    #[test]
    fn the_view_scrolls_when_the_cursor_comes_within_three_tiles_of_an_edge() {
        let mut ui = ui();
        ui.view = Pos::new(10, 10);
        ui.cursor = Pos::new(10 + 3, 10 + 3);
        ui.move_cursor(0, 0);
        assert_eq!(ui.view, Pos::new(10, 10), "exactly 3 tiles in is still fine");
        ui.move_cursor(-1, -1);
        assert_eq!(ui.view, Pos::new(9, 9), "2 tiles in scrolls one tile");
        // The far edge: the last column in view is x + 87, and 3 tiles in from it is x + 84.
        ui.cursor = Pos::new(9 + 84, 9 + 35);
        ui.move_cursor(0, 0);
        assert_eq!(ui.view, Pos::new(9, 9));
        ui.move_cursor(1, 1);
        assert_eq!(ui.view, Pos::new(10, 10));
    }

    #[test]
    fn the_view_never_scrolls_past_the_map() {
        let mut ui = ui();
        ui.move_cursor(-500, -500);
        assert_eq!(ui.cursor, Pos::new(0, 0));
        assert_eq!(ui.view, Pos::new(0, 0));
        ui.move_cursor(500, 500);
        assert_eq!(ui.cursor, Pos::new(99, 79));
        assert_eq!(ui.view, Pos::new(100 - 88, 80 - 39));
    }

    #[test]
    fn a_jump_moves_the_view_and_carries_the_cursor_a_quarter_screen() {
        let mut ui = ui();
        ui.view = Pos::new(6, 20);
        ui.cursor = Pos::new(40, 40);
        ui.jump(-1, 0);
        // A quarter of 88 columns.
        assert_eq!((ui.view.x, ui.cursor.x), (0, 18), "the view stops at the map edge");
        ui.jump(0, 1);
        assert_eq!((ui.view.y, ui.cursor.y), (20 + 9, 40 + 9), "a quarter of 39 rows");
        assert_eq!(ui.cursor_cell(), Some((18, 40 + 9 - 29)));
    }

    #[test]
    fn the_rectangle_runs_from_the_anchor_to_the_cursor_whichever_way_it_was_dragged() {
        let mut ui = ui();
        assert_eq!((ui.rect(), ui.rect_size()), (None, None));
        ui.anchor = Some(Pos::new(50, 40));
        assert_eq!(ui.rect(), None, "Select mode has no rectangle");
        ui.tool = Tool::Harvest;
        assert_eq!(ui.rect_size(), Some((1, 1)), "one tile is Enter twice");
        ui.move_cursor(-4, 2);
        assert_eq!(ui.rect(), Some((Pos::new(50, 40), Pos::new(46, 42))));
        assert_eq!(ui.rect_size(), Some((5, 3)));
        assert!(ui.in_rect(Pos::new(46, 40)) && ui.in_rect(Pos::new(50, 42)));
        assert!(!ui.in_rect(Pos::new(45, 40)) && !ui.in_rect(Pos::new(50, 43)));
        ui.set_tool(Tool::Stockpile);
        assert_eq!((ui.anchor, ui.rect(), ui.notice.as_str()), (None, None, ""));
    }

    #[test]
    fn a_map_smaller_than_the_view_stays_put() {
        let mut ui = UiState::new(&Session::new(7, Nation::Earth, 48, 32), (120, 40));
        assert_eq!(ui.view, Pos::new(0, 0));
        ui.move_cursor(100, 100);
        assert_eq!(ui.view, Pos::new(0, 0));
        ui.jump(1, 1);
        assert_eq!(ui.view, Pos::new(0, 0));
        assert_eq!(ui.cursor, Pos::new(47, 31));
    }

    #[test]
    fn resizing_keeps_the_cursor_in_sight() {
        let mut ui = ui();
        ui.cursor = Pos::new(50, 40);
        ui.resize(80, 24);
        let (w, h) = ui.viewport();
        assert_eq!((w, h), (48, 23));
        let (cx, cy) = ui.cursor_cell().expect("still in view");
        assert!((SCROLL_MARGIN..w - SCROLL_MARGIN).contains(&cx));
        assert!((SCROLL_MARGIN..h - SCROLL_MARGIN).contains(&cy));
    }

    /// Every other tool's hint is `fn_ui`'s own; the Build tool's names the chosen building,
    /// since the choice belongs to `UiState`, not the `Session` (#275).
    #[test]
    fn the_build_tool_s_hint_names_the_chosen_building() {
        let session = Session::new(7, Nation::Earth, 48, 32);
        let mut ui = UiState::new(&session, (120, 40));
        ui.set_tool(Tool::Harvest);
        assert_eq!(ui.tool_hint(&session), Tool::Harvest.hint(&session));

        // With no choice yet it falls back to the first building the nation can place (Bed).
        ui.set_tool(Tool::Build);
        assert_eq!(ui.tool_hint(&session), "build Bed: Enter to place (U: pick a building)");
        let bed = session.defs.building_id("bed").unwrap();
        ui.choice = Some(bed);
        assert_eq!(ui.tool_hint(&session), "build Bed: Enter to place (U: pick a building)");
    }
}
