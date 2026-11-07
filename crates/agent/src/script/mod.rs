//! The embedded script runner (docs/design/agent-scripting.md, "Architecture", "The SDK",
//! "`run_script`"): a sandboxed `rhai::Engine` with a thin plumbing layer 1:1 with
//! `fn_tui::surface::Host`, and [`runner::run`], the one entry point the `run_script` tool
//! (`crate::tools`) calls.
//!
//! The engine ([`runner::run`]) is built with no module resolver (Cargo's `no_module` feature:
//! `import` fails to parse, so a script can't read another file) and no time functions
//! (`no_time`: `timestamp()` doesn't exist), and registers no RNG of its own, so a script can't
//! read `docs/` or `data/`, look ahead of the clock, or get a different answer from the same
//! state twice. `print` and `debug` are routed to the script's own log ([`runner::Outcome`]),
//! never to stdout, which is the MCP channel `fn_agent serve` speaks over.
//!
//! `wake_on` events, `events()` and `wake(msg)` (#478, docs/design/agent-scripting.md, "The
//! trampoline", "The wake report") let a script end its own run on something worth the model's
//! attention, and give the model back a compact report instead of a turn-by-turn transcript.
//!
//! [`library::Library`] is the saved script library `save_script`/`run_script {name}`/
//! `scripts {}` share (#479); [`runner::run`]'s own `memory` parameter and `Outcome::memory`
//! are the persisted map `fn_agent::tools::Tools` carries across `run_script` calls in a
//! session, capped by `Limits::memory_bytes`.
//!
//! [`prelude::SOURCE`] (#480) is the porcelain layer, written in Rhai on top of the plumbing:
//! [`runner::run`] merges its compiled AST into every call ahead of the caller's own code, and
//! `crate::tools`'s own `sdk_reference` tool reads [`prelude::reference`] and
//! [`bindings::plumbing_reference`] back to describe both layers from the code itself.

pub(crate) mod bindings;
pub mod library;
pub mod limits;
pub mod prelude;
pub mod runner;

pub use library::Library;
pub use limits::Limits;
pub use runner::{Outcome, Request, run};
