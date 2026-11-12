//! Frontend logic for the terminal UI (docs/design/tui.md).
//!
//! The sim holds the rules; this crate holds what a *frontend* does with them that isn't drawing:
//! which command each tool issues, how a task or a tile reads in English, what the side panel and
//! the selection bar show, the alert log and the clock that turns real time into ticks, and the
//! game's UI state with the one place keys change it (#857): [`app::App`] (the session and what
//! the player sees of it), [`state::UiState`], key handling ([`keyev`], [`input`], [`script`]),
//! the pointer ([`mod@pointer`]) and what each full screen says and how its cursor moves
//! ([`screens`], [`menu`], [`ask`]). A frontend feeds it keys or a pointer and draws what it
//! holds: `fn_tui` in character cells, `fn_gui` in a window. Text is toned [`rows::Row`]s, not
//! colours: how a [`rows::Tone`] looks is the frontend's.
//!
//! What keeps it honest:
//!
//! * It depends on `fn_sim` only: no engine, no terminal library, no ratatui or crossterm types in
//!   any signature. No wall-clock reads: callers pass elapsed time in (`Clock::advance`,
//!   `Alerts::update`). No I/O either, except in `saves`, the save files the terminal UI reads and
//!   writes (the sim itself stays free of I/O); `App` reaches the disk only through it.
//! * It changes the world only through `World::apply` and `World::tick`, and it never decides
//!   something the sim decides. It doesn't pre-filter tiles a command would accept or reject; it
//!   formats and maps.
//! * Wording that depends on how the player acts (a mouse versus a keyboard) is an enum here
//!   (`inspect::Hint`) that the frontend words itself.
//!
//! Sprites, entities, the camera, layout pixels and anything that touches a window stay in the
//! frontend.

pub mod alerts;
pub mod app;
pub mod ask;
pub mod bend;
pub mod build;
pub mod chute;
pub mod clock;
pub mod code;
pub mod embark;
pub mod endeavor;
pub mod gate;
pub mod input;
pub mod inspect;
pub mod inspector;
pub mod keyev;
pub mod keys;
pub mod layout;
pub mod link;
pub mod menu;
pub mod meter;
pub mod msglog;
pub mod page;
pub mod panel;
pub mod pointer;
pub mod recipe;
pub mod research;
pub mod rows;
pub mod saves;
pub mod screens;
pub mod script;
#[cfg(feature = "serde")]
mod ser;
pub mod session;
pub mod sparkline;
pub mod state;
pub mod teaching;
pub mod text;
pub mod theme;
pub mod time;
pub mod toolpanel;
pub mod tools;
pub mod trade;
pub mod view;

pub use alerts::Alerts;
pub use clock::Clock;
pub use session::{STARTING_UNITS, Session};
pub use tools::Tool;

/// A new game on the default-size map for `seed`, for the tests of what moved here from
/// `fn_tui` (#857), which called `fn_tui::new_session`.
#[cfg(test)]
pub(crate) fn new_session(seed: u64, nation: fn_sim::Nation) -> Session {
    let map = fn_sim::MapGenConfig::default();
    Session::new(seed, nation, map.width, map.height)
}
