//! The terminal frontend (docs/design/tui.md): a glyph map, a cursor, the Seal side panel and the
//! session clock, on ratatui and crossterm.
//!
//! It depends on `fn_sim` and `fn_ui` only, never Bevy (a test checks the dependency tree). The
//! game rules are in `fn_sim`, the frontend logic both UIs share in `fn_ui`; this crate draws and
//! reads keys.
//!
//! Rendering is a pure function into a ratatui `Buffer` ([`draw::draw`]), so `--frame`, the
//! golden frames and the interactive loop ([`run`]) all draw the same way, and nothing but
//! [`run`] touches the terminal or the wall clock.

pub mod app;
pub mod cli;
pub mod color;
pub mod draw;
pub mod frame;
pub mod glyphs;
pub mod input;
pub mod layout;
pub mod map;
pub mod panel;
pub mod run;
pub mod state;

use fn_sim::{MapGenConfig, Nation};
use fn_ui::Session;

/// A new game on the map for `seed`, the same size as the Bevy client's.
pub fn new_session(seed: u64, nation: Nation) -> Session {
    let map = MapGenConfig::default();
    Session::new(seed, nation, map.width, map.height)
}
