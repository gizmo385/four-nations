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
//! Out of scope here (left to #478-#480, docs/design/agent-scripting.md's own follow-up list):
//! wake rules and `events()`, the saved script library and `memory`, and the porcelain prelude.

mod bindings;
pub mod limits;
pub mod runner;

pub use limits::Limits;
pub use runner::{Outcome, run};
