//! The file formats the agent tooling passes around, defined once (#606): the run log
//! ([`RunLogHeader`], [`LogLine`]), the handoff ([`Handoff`]) and the campaign ledger
//! ([`campaign`]). `serde` and `serde_json` are the only dependencies, so `xtask` (which writes
//! and reads these files without `fn_sim`, `fn_tui`, ratatui or crossterm), `fn_agent`, `fn_tui`
//! and the wasm replayer all use the same types and cannot drift apart.
//!
//! Two fields of the run log carry game types that this crate cannot name: the header's
//! `nation` (`fn_sim::Nation`) and a `script` line's `wake_on` (`fn_tui::surface::EventKind`).
//! [`RunLogHeader`] and [`LogLine`] are generic over them (`N` and `E`), defaulting to `String`,
//! their JSON form. `fn_tui` fixes them to the typed enums (`surface::LogLine` is
//! `LogLine<Nation, EventKind>`); a reader that only counts lines, such as `xtask`, uses the
//! plain strings.

pub mod campaign;
mod handoff;
mod log;

pub use handoff::{Handoff, ScriptEntry};
pub use log::{LogLine, Prelude, PreludeOpening, RunLogHeader, run_end_reason};
