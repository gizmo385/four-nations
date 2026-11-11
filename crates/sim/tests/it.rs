//! Every integration test of `fn_sim`, as one test binary.
//!
//! Each file in this folder is a module here, not a binary of its own: a binary per file meant
//! linking the whole crate stack once per file, on every build that touched the crate. To add a
//! test file, create it beside this one and add its `mod` line below; `cargo xtask ci` fails if a
//! file here isn't listed. One file's tests: `cargo test -p fn_sim --test it FILE::`.

mod balance;
mod commands;
mod embark;
mod food;
mod invariants;
mod jobs;
mod mapgen;
mod path;
mod regions;
mod research_air;
mod research_earth;
mod research_fire;
mod research_water;
mod run_record;
mod save;
mod save_compat;
mod save_fixture;
mod save_v2_fixture;
mod save_v3_fixture;
