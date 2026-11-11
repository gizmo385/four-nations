//! Every integration test of `fn_agent`, as one test binary.
//!
//! Each file in this folder is a module here, not a binary of its own: a binary per file meant
//! linking the whole crate stack once per file, on every build that touched the crate. To add a
//! test file, create it beside this one and add its `mod` line below; `cargo xtask ci` fails if a
//! file here isn't listed. One file's tests: `cargo test -p fn_agent --test it FILE::`.

mod build_id;
mod call_log;
mod carry_in;
mod concurrent_log;
mod frame_clamp;
mod horizon;
mod plan;
mod play;
mod porcelain;
mod probes;
mod reference_governor;
mod resume;
mod routine_bends;
mod run_script;
mod scorecard;
mod script_library;
mod sdk_reference;
mod trial;
mod watch_parity;
