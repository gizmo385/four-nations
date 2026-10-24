//! What the player is looking at: the cursor, the part of the map in view and the modal state.
//! Nothing here touches the terminal, so the render and input tests drive it directly.

use fn_sim::Pos;
use fn_ui::{Session, Tool};

use crate::layout;

/// How close (in tiles) the cursor may come to the edge of the view before the view scrolls.
pub const SCROLL_MARGIN: i32 = 3;

/// A y/n question that takes over the keys until it is answered.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Prompt {
    Quit,
}

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
    /// The tool the cursor tooltip words its hints for. `Select` until the designation tools
    /// (#98) bring a way to change it.
    pub tool: Tool,
    pub prompt: Option<Prompt>,
    pub quit: bool,
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
            prompt: None,
            quit: false,
        };
        ui.center_view();
        ui
    }

    pub fn viewport(&self) -> (i32, i32) {
        self.viewport
    }

    /// The terminal changed size: the view keeps the cursor in sight at the new size.
    pub fn resize(&mut self, width: u16, height: u16) {
        self.viewport = layout::viewport(width, height);
        self.follow();
    }

    /// Moves the cursor `(dx, dy)` tiles, stopping at the map edge, and scrolls the view if the
    /// cursor came within [`SCROLL_MARGIN`] of its edge.
    pub fn move_cursor(&mut self, dx: i32, dy: i32) {
        self.cursor = self.clamp_to_map(Pos::new(self.cursor.x + dx, self.cursor.y + dy));
        self.follow();
    }

    /// Jumps the view `(dx, dy)` quarter screens and carries the cursor the same distance
    /// (Ctrl+arrows, PageUp and PageDown).
    pub fn jump(&mut self, dx: i32, dy: i32) {
        let (w, h) = self.viewport;
        let (dx, dy) = (dx * (w / 4).max(1), dy * (h / 4).max(1));
        self.view = self.clamp_view(Pos::new(self.view.x + dx, self.view.y + dy));
        self.cursor = self.clamp_to_map(Pos::new(self.cursor.x + dx, self.cursor.y + dy));
        self.follow();
    }

    fn clamp_to_map(&self, p: Pos) -> Pos {
        Pos::new(p.x.clamp(0, self.bounds.0 - 1), p.y.clamp(0, self.bounds.1 - 1))
    }

    /// `view` kept within the map: the view never shows past an edge, and a map smaller than
    /// the view sits at the origin.
    fn clamp_view(&self, view: Pos) -> Pos {
        let max = |bound: i32, shown: i32| (bound - shown).max(0);
        Pos::new(
            view.x.clamp(0, max(self.bounds.0, self.viewport.0)),
            view.y.clamp(0, max(self.bounds.1, self.viewport.1)),
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
        let view = Pos::new(
            scroll(self.view.x, self.cursor.x, self.viewport.0),
            scroll(self.view.y, self.cursor.y, self.viewport.1),
        );
        self.view = self.clamp_view(view);
    }

    fn center_view(&mut self) {
        let view =
            Pos::new(self.cursor.x - self.viewport.0 / 2, self.cursor.y - self.viewport.1 / 2);
        self.view = self.clamp_view(view);
    }

    /// Where on the map region (column, row) the cursor is drawn, if it's in view.
    pub fn cursor_cell(&self) -> Option<(i32, i32)> {
        let (x, y) = (self.cursor.x - self.view.x, self.cursor.y - self.view.y);
        ((0..self.viewport.0).contains(&x) && (0..self.viewport.1).contains(&y)).then_some((x, y))
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
}
