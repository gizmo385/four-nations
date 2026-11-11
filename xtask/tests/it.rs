//! Every integration test of `xtask`, as one test binary.
//!
//! Each file in this folder is a module here, not a binary of its own: a binary per file meant
//! linking the whole crate stack once per file, on every build that touched the crate. To add a
//! test file, create it beside this one and add its `mod` line below; `cargo xtask ci` fails if a
//! file here isn't listed. One file's tests: `cargo test -p xtask --test it FILE::`.

mod agent_campaign;
mod agent_eval;
mod agent_run;
