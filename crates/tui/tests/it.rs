//! Every integration test of `fn_tui`, as one test binary.
//!
//! Each file in this folder is a module here, not a binary of its own: a binary per file meant
//! linking the whole crate stack once per file, on every build that touched the crate. To add a
//! test file, create it beside this one and add its `mod` line below; `cargo xtask ci` fails if a
//! file here isn't listed. One file's tests: `cargo test -p fn_tui --test it FILE::`.

mod abilities;
mod agent_parity;
mod beasts_canals;
mod binary;
mod browser;
mod browser_campaigns;
mod chute;
mod embark;
mod fast;
mod footprints;
mod frames;
mod full_screen;
mod fusions;
mod hints;
mod key_help;
mod layout_properties;
mod menu;
mod message_log;
mod meter_and_clock;
mod nation;
mod newcomers;
mod pairs;
mod panel_alerts;
mod panels;
mod phase3;
mod pointer;
mod rail;
mod reference_goldens;
mod replays;
mod research;
mod runs_dir;
mod save_list;
mod saves;
mod scenario;
mod school_readouts;
mod schools;
mod teachings;
mod title;
mod tool_list;
mod traders;
mod watch;
