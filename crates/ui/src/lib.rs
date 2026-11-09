//! Frontend logic for the terminal UI (docs/design/tui.md).
//!
//! The sim holds the rules; this crate holds what a *frontend* does with them that isn't drawing:
//! which command each tool issues, how a task or a tile reads in English, what the side panel and
//! the selection bar show, the alert log and the clock that turns real time into ticks.
//!
//! What keeps it honest:
//!
//! * It depends on `fn_sim` only. No engine and no wall-clock reads: callers pass elapsed time in
//!   (`Clock::advance`, `Alerts::update`). No I/O either, except in `saves`, the save files the
//!   terminal UI reads and writes (the sim itself stays free of I/O).
//! * It changes the world only through `World::apply` and `World::tick`, and it never decides
//!   something the sim decides. It doesn't pre-filter tiles a command would accept or reject; it
//!   formats and maps.
//! * Wording that depends on how the player acts (a mouse versus a keyboard) is an enum here
//!   (`inspect::Hint`) that the frontend words itself.
//!
//! Sprites, entities, the camera, layout pixels and anything that touches a window stay in the
//! frontend.

pub mod alerts;
pub mod bend;
pub mod build;
pub mod clock;
pub mod code;
pub mod endeavor;
pub mod inspect;
pub mod research;
pub mod saves;
pub mod session;
pub mod sparkline;
pub mod text;
pub mod theme;
pub mod tools;
pub mod view;

pub use alerts::Alerts;
pub use clock::Clock;
pub use session::{STARTING_UNITS, Session};
pub use tools::Tool;
