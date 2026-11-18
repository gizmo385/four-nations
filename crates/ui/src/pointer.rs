//! A pointer on the map (the window trials, #842 and #843; click semantics #878): the pointing
//! rules, in tiles and not pixels or cells, so every frontend (an egui window, the web canvas,
//! later the terminal) works out which tile is under its pointer and hands that in.
//!
//! - **Select, a click** points at the tile, then `Enter` if a unit can be selected there, else
//!   `Esc` if one is selected (deselect). A click on bare ground with nothing selected does
//!   nothing: `Esc` would open the game menu.
//! - **A tool, a click** is a one-tile apply: `Enter` `Enter` on that tile (with a corner already
//!   set, the click is the second corner: one `Enter`; the Gate and a chute's foot take one).
//! - **A tool, a drag** is the rectangle: `Enter` at the start, the cursor to the other corner,
//!   `Enter` on release.
//! - **A tool, a right click** is `Esc` (drop the corner, else leave the tool): [`cancel`].
//! - **A press with the tool list open** only closes the list, as any other key would.
//! - **An abandoned press** (a second finger on a touch screen) drops a rectangle's corner and
//!   applies nothing: [`Pointer::abandon`].
//!
//! **Confirm first** ([`Pointer::set_confirm`], [`tap`]; on touch screens): a finger can't hover
//! to see where a tool lands, and a pan that started as a one-finger drag used to issue the
//! rectangle it drew. So with confirm on, a tool's tap or drag stops one key short. A tap sets
//! the corner on its tile (a one-tile rectangle) and a drag leaves its rectangle drawn, anchor
//! set, cursor on the far corner; the Gate's tap only moves its ghost. Nothing is issued until
//! the `Enter` the window's confirm button stands for. A new press with a tool drops the waiting
//! rectangle (`Esc`) and starts again, so a wrong tap is undone by tapping where it should be.
//!
//! Everything goes through the keys themselves (`App::key`), so a click issues exactly the
//! command the keyboard path would and nothing here duplicates the tools' logic. While a drag is
//! under way the anchor is set, so the map draws the live rectangle in the tool's hue as it does
//! for the keyboard. The tests compare command logs (`crates/tui/tests/pointer.rs`).

use crate::tools::Tool;
use fn_sim::Pos;

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
    /// Stop one key short of applying (see the module docs): the window's confirm button
    /// sends the last `Enter`.
    confirm: bool,
}

/// Which pointer button a click is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Button {
    Primary,
    Secondary,
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

fn esc(app: &mut App) {
    app.key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
}

/// What a click on the tile the cursor was just put on does (see the module docs).
fn activate(app: &mut App) {
    if app.ui.linking.is_some() || app.ui.tool == Tool::Gate {
        return enter(app);
    }
    if app.ui.tool == Tool::Select {
        if !app.ui.selectable_here(&app.session).is_empty() {
            enter(app);
        } else if app.ui.selected.is_some() {
            esc(app);
        }
        return;
    }
    if app.ui.anchor.is_none() {
        enter(app);
    }
    enter(app);
}

/// Confirm first: a rectangle tool's waiting rectangle is dropped (`Esc` drops just the corner),
/// so the next press starts afresh.
fn drop_waiting(app: &mut App) {
    if app.ui.linking.is_none() && has_rectangle(app.ui.tool) && app.ui.anchor.is_some() {
        esc(app);
    }
}

/// A tap with confirm first, on the tile the cursor was just put on: as [`activate`] but one key
/// short with a tool. A rectangle tool sets the corner here (a one-tile rectangle waiting for
/// its `Enter`); the Gate only moves its ghost; Select and linking act at once, as they place
/// nothing.
fn activate_short(app: &mut App) {
    if app.ui.linking.is_some() || app.ui.tool == Tool::Select {
        return activate(app);
    }
    if app.ui.tool == Tool::Gate {
        return;
    }
    if app.ui.anchor.is_none() {
        enter(app);
    }
}

/// A tap on `pos` with confirm first (see the module docs): a waiting rectangle is dropped, then
/// the tap sets a new one-tile one (or moves the Gate's ghost, or selects in Select).
pub fn tap(app: &mut App, pos: Pos) {
    let mut p = Pointer::new();
    p.set_confirm(true);
    p.down(app, pos);
    p.up(app, pos);
}

/// A right click: `Esc` with a tool (drop the corner, else leave the tool). In Select it does
/// nothing (the right button pans there, and `Esc` would open the game menu).
pub fn cancel(app: &mut App) {
    if map_has_pointer(app) && (app.ui.tool != Tool::Select || app.ui.linking.is_some()) {
        esc(app);
        app.dirty = true;
    }
}

impl Pointer {
    pub fn new() -> Self {
        Self::default()
    }

    /// Turns confirm-first on or off (see the module docs).
    pub fn set_confirm(&mut self, on: bool) {
        self.confirm = on;
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
        // An open tool list takes the press as a key that closes it, and the press goes no
        // further (no click, no rectangle).
        if app.ui.tool_list.is_some() {
            self.down = None;
            self.dragging = false;
            esc(app);
            return;
        }
        if self.confirm {
            drop_waiting(app);
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

    /// The button came up on `pos`: a drag applies its rectangle; a press that never left its
    /// tile is a click ([`activate`]); a Select drag only moved the cursor.
    pub fn up(&mut self, app: &mut App, pos: Pos) {
        let Some(start) = self.down.take() else { return };
        if !map_has_pointer(app) {
            self.dragging = false;
            return;
        }
        app.ui.point_at(pos);
        if std::mem::take(&mut self.dragging) {
            // Confirm first: the rectangle stays drawn, waiting for its `Enter`.
            if app.ui.anchor.is_some() && !self.confirm {
                enter(app);
            }
        } else if pos == start {
            if self.confirm { activate_short(app) } else { activate(app) }
        }
        app.dirty = true;
    }
}

impl Pointer {
    /// Drops the press without applying anything (a second finger landed, #977): a rectangle
    /// being drawn loses its corner (`Esc`, as a right click would), and the release that
    /// follows does nothing.
    pub fn abandon(&mut self, app: &mut App) {
        let was_dragging = std::mem::take(&mut self.dragging);
        if self.down.take().is_some() && was_dragging && app.ui.anchor.is_some() {
            esc(app);
            app.dirty = true;
        }
    }
}

/// A whole drag from `from` to `to`, for tests and scripted callers.
pub fn drag(app: &mut App, from: Pos, to: Pos) {
    let mut p = Pointer::new();
    p.down(app, from);
    p.moved(app, to);
    p.up(app, to);
}

/// A click of `button` on `pos`: the primary one points and acts (see the module docs), the
/// secondary one is [`cancel`].
pub fn click_with(app: &mut App, pos: Pos, button: Button) {
    match button {
        Button::Primary => click(app, pos),
        Button::Secondary => cancel(app),
    }
}

/// A primary click on `pos`.
pub fn click(app: &mut App, pos: Pos) {
    let mut p = Pointer::new();
    p.down(app, pos);
    p.up(app, pos);
}
