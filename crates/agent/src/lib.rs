//! `fn_agent`: an MCP server over stdio for the agent surface (docs/design/agent-play.md,
//! "Architecture"). An LLM agent (a Claude Code session, say) plays The Four Nations by
//! connecting to `fn_agent serve` as an MCP server -- no SDK, nothing in `fn_sim` -- the same
//! way it would connect to any other tool server.
//!
//! ```text
//! fn_agent serve --seed N --nation X [--start default|X,Y] [--difficulty ID] [--turn SECS]
//!                --log PATH
//! fn_agent keys LOG
//! ```
//!
//! `serve` wraps a [`fn_tui::surface::Host`] (the same one `fn_tui`'s own `--frame` and `--keys`
//! use) and speaks JSON-RPC 2.0 over stdin and stdout: `initialize`, `tools/list` and
//! `tools/call` ([`tools`]), hand-rolled rather than through an SDK ([`server`]). It writes its
//! run to `--log PATH` as JSON Lines as it goes ([`runlog`]). `keys` reads a finished (or still
//! growing) log and prints the `--keys` script that reproduces its final frame with
//! `fn_tui SEED NATION --frame --keys "..."`.
//!
//! ## Sandboxing a racer (docs/design/agent-play.md, "Enforcing and testing parity")
//!
//! [`tools::list`] is deliberately the *entire* surface a sandboxed agent should have: no shell,
//! no file reads of `docs/` or `data/`, no web, nothing but these eight tools. **Granting only
//! them is a setting of whatever runs the agent, not something this crate or any test can
//! enforce.** For an agent harness that can restrict which tools or capabilities a model may
//! use (an MCP client's own tool allow-list, a sandboxed shell profile, or similar), the recipe
//! is:
//!
//! 1. Register `fn_agent serve ...` as the *only* MCP server the agent's harness connects to --
//!    don't also hand it a general filesystem or shell tool, which would let it read `docs/` or
//!    `data/`, or peek at another racer's log (#313's "What racers may know": in-game knowledge
//!    only).
//! 2. If the harness has its own notion of tool scoping on top of that (letting a model see only
//!    some of a server's tools), it doesn't need one here: every tool in [`tools::list`] is
//!    already safe to grant, by the parity rule the agent surface is built to (every one reads
//!    through `fn_ui`'s own view models or presses a key `Shell::key` would take from a human).
//! 3. Give the agent the race's briefing (the `briefing` tool, plus a short standard preamble)
//!    as its only instructions. Don't paste in this doc, `data/races.toml`, or any other rules
//!    text: that would make the run a test of the agent's reading comprehension rather than of
//!    the game.
//!
//! This is a statement about how you deploy the agent, same as "don't give it a shell" is: no
//! amount of testing inside this repository can prove an operator followed it.

pub mod runlog;
pub mod server;
pub mod tools;
