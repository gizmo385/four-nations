//! The terminal frontend (docs/design/tui.md): a glyph map, a cursor, the Seal side panel and the
//! session clock, on ratatui and crossterm.
//!
//! It depends on `fn_sim` and `fn_ui` only, never a game engine. The game rules are in `fn_sim`,
//! the shared frontend logic in `fn_ui` (including `App`, `UiState` and key handling, #857); this
//! crate draws them in ratatui cells and turns crossterm's keys into `fn_ui`'s.
//!
//! Rendering is a pure function into a ratatui `Buffer` ([`draw::draw`]), so `--frame`, the
//! golden frames and the interactive loop ([`run`]) all draw the same way, and nothing but
//! [`run`] touches the terminal or the wall clock.

pub mod abilities;
pub mod ask;
pub mod banner;
pub mod browser;
pub mod build;
pub mod buildid;
pub mod cache;
pub mod cli;
pub mod color;
pub mod draw;
pub mod embark;
pub mod feed;
pub mod frame;
pub mod glyphs;
pub mod inspector;
pub mod keyev;
pub mod layout;
pub mod legend;
pub mod map;
pub mod menu;
pub mod modal;
pub mod msglog;
pub mod nation;
pub mod page;
pub mod panel;
pub mod record;
pub mod research;
#[cfg(not(target_arch = "wasm32"))]
pub mod run;
pub mod runs;
pub mod saves;
pub mod scenario;
pub mod screens;
pub mod shell;
pub mod spectate;
pub mod studio;
pub mod surface;
pub mod title;
pub mod traders;
pub mod watch;
pub mod widgets;

// The game's key handling lives in `fn_ui` (#857); these keep the old paths.
pub use fn_ui::app;
pub use fn_ui::input;
pub use fn_ui::pointer;
pub use fn_ui::screens::endeavor;
pub use fn_ui::script as keys;
pub use fn_ui::state;

use fn_sim::{MapGenConfig, Nation};
use fn_ui::Session;

/// A new game on the map for `seed`, at the default size.
pub fn new_session(seed: u64, nation: Nation) -> Session {
    let map = MapGenConfig::default();
    Session::new(seed, nation, map.width, map.height)
}
