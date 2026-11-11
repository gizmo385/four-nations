//! A pointer on the map (the window trials, #842 and #843): a click moves the cursor, and a drag
//! with a designate tool active makes the same rectangle the keyboard does. It is expressed in
//! tiles, not pixels or cells, so every frontend (an egui window, the web canvas, later the
//! terminal) works out which tile is under its pointer and hands that in.
//!
//! The rectangle goes through the keys themselves (`Enter` at one corner, the cursor to the
//! other, `Enter` again, `App::confirm`), so a drag issues exactly the command the keyboard would
//! and nothing here duplicates the tools' logic. While a drag is under way the anchor is set, so
//! the map draws the live rectangle in the tool's hue as it does for the keyboard.

use fn_sim::Pos;
use fn_ui::tools::Tool;

use crate::app::App;
use crate::keyev::{KeyCode, KeyEvent, KeyModifiers};

/// One press of the pointer, from button down to button up.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Pointer {
    /// The tile the button went down on, while it is down.
    down: Option<Pos>,
    /// Whether this press has become a rectangle (the pointer left its first tile with a tool
    /// that has one).
    dragging: bool,
}

/// Whether the map is what a pointer on it should act on: a game with no screen, modal or
/// prompt over the map.
pub fn map_has_pointer(app: &App) -> bool {
    app.ui.screen.is_none() && app.ui.modal.is_none() && app.ui.prompt.is_none()
}

/// Whether `tool` makes a rectangle (Select picks a unit and the Gate is placed in one go).
fn has_rectangle(tool: Tool) -> bool {
    !matches!(tool, Tool::Select | Tool::Gate)
}

fn enter(app: &mut App) {
    app.key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
}

impl Pointer {
    pub fn new() -> Self {
        Self::default()
    }

    /// Whether a drag is making a rectangle right now.
    pub fn dragging(&self) -> bool {
        self.dragging
    }

    /// The button went down on `pos`.
    pub fn down(&mut self, app: &mut App, pos: Pos) {
        if !map_has_pointer(app) {
            return;
        }
        self.down = Some(pos);
        self.dragging = false;
        app.ui.point_at(pos);
        app.dirty = true;
    }

    /// The pointer moved to `pos` with the button down: past its first tile with a rectangle
    /// tool, the press becomes a drag (the anchor is set where it went down).
    pub fn moved(&mut self, app: &mut App, pos: Pos) {
        let Some(start) = self.down else { return };
        if !map_has_pointer(app) {
            return;
        }
        if !self.dragging && pos != start && has_rectangle(app.ui.tool) && app.ui.linking.is_none()
        {
            app.ui.anchor = None;
            app.ui.point_at(start);
            enter(app);
            self.dragging = true;
        }
        app.ui.point_at(pos);
        app.dirty = true;
    }

    /// The button came up on `pos`: a drag applies its rectangle; a click has already moved
    /// the cursor.
    pub fn up(&mut self, app: &mut App, pos: Pos) {
        if self.down.take().is_none() {
            return;
        }
        if !map_has_pointer(app) {
            self.dragging = false;
            return;
        }
        app.ui.point_at(pos);
        if std::mem::take(&mut self.dragging) && app.ui.anchor.is_some() {
            enter(app);
        }
        app.dirty = true;
    }
}

/// A whole drag from `from` to `to`, for tests and scripted callers.
pub fn drag(app: &mut App, from: Pos, to: Pos) {
    let mut p = Pointer::new();
    p.down(app, from);
    p.moved(app, to);
    p.up(app, to);
}

/// A click on `pos`.
pub fn click(app: &mut App, pos: Pos) {
    let mut p = Pointer::new();
    p.down(app, pos);
    p.up(app, pos);
}
