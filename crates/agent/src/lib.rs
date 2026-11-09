//! `fn_agent`: an MCP server over stdio for the agent surface (docs/design/agent-play.md,
//! "Architecture"). An LLM agent (a Claude Code session, say) plays The Four Nations by
//! connecting to `fn_agent serve` as an MCP server -- no SDK, nothing in `fn_sim` -- the same
//! way it would connect to any other tool server.
//!
//! ```text
//! fn_agent serve --seed N --nation X [--start default|X,Y] [--difficulty ID] [--turn SECS]
//!                [--map-size ID] [--prelude OPENING:MINUTES] [--scripts] [--carry-in FILE]
//!                --log PATH
//! fn_agent keys LOG
//! fn_agent score LOG [--horizon TURNS] [--json] [--append]
//! ```
//!
//! `serve` wraps a [`fn_tui::surface::Host`] (the same one `fn_tui`'s own `--frame` and `--keys`
//! use) and speaks JSON-RPC 2.0 over stdin and stdout: `initialize`, `tools/list` and
//! `tools/call` ([`tools`]), hand-rolled rather than through an SDK ([`server`]). It writes its
//! run to `--log PATH` as JSON Lines as it goes, flushing each line ([`runlog`]), in the one
//! schema `fn_tui::surface::{RunLogHeader, LogLine}` defines -- the same shape `fn_tui --watch`
//! reads, so the two can't drift apart (#324). `fn_tui --watch run.jsonl` follows that file live
//! in a human's own terminal while `serve` is still writing it, or replays it once `serve` has
//! exited; `fn_agent keys LOG` reads a finished (or still growing) log the same way and prints
//! the `--keys` script that reproduces its final frame with `fn_tui SEED NATION --frame --keys
//! "..."`.
//!
//! `--scripts` (docs/design/agent-scripting.md) adds `run_script`: instead of deciding every
//! key itself, the model hands over a [`script`] written in embedded Rhai that plays the
//! routine work and only comes back once something worth a decision happens. Every key a
//! script presses is a normal `act`/`keys` call under the hood, logged the same way, so
//! `fn_tui --watch` and `fn_agent keys` read a scripted run exactly as they read an MCP-only
//! one. `save_script`, `scripts` and `notes` round this out with a session's own script
//! library, `memory` and lab notes, logged as `Script`/`Wake`/`Note` lines
//! (`fn_tui::surface::LogLine`) that `fn_agent serve --resume` reads back to restore them
//! exactly, without changing what the game itself replays to (#479).
//! `--carry-in FILE` (#530, [`carry`]) starts a fresh scripted session with a handoff's scripts,
//! notes and plan already in place, written to the log as ordinary lines.
//! `plan {set?, merge?}` (#524) holds the model's intent as data, read by scripts with `plan()`,
//! logged as a `Plan` line on every change and restored by `--resume` the same way.
//!
//! `fn_agent score LOG` (#525, docs/design/agent-hillclimb.md, "The scorecard") replays a finished
//! log to a horizon and prints its [`scorecard`]: progress, care and the diagnostics a climb can
//! act on. It reads the world through `fn_tui::surface::referee`, which no MCP tool handler may
//! name (the parity lint, `xtask/src/lint.rs`), so a score is for the end of an attempt and
//! `observe` carries none of it.
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

pub mod carry;
pub mod clean;
pub mod plan;
pub mod runlog;
pub mod scorecard;
pub mod script;
pub mod server;
pub mod tools;
pub mod trace;
pub mod trial;
