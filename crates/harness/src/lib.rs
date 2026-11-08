//! The balance harness (`docs/design/balance-harness.md`, section 8): the crate that reads
//! `data/harness.toml` and owns the `balance` binary, which `fn_sim` never does because it does
//! file I/O and threads.
//!
//! - `style` (#536): the play styles, mapped onto `fn_sim::autoplay::Player`.
//! - `config`, `metric` (#537): the panel, detector thresholds, targets and sensitivity pairs.
//! - `runner`: which runs a panel needs, the runs on threads, the records as JSON.
//! - `verdict`, `report`: every run into a bucket, every target into pass, fail or no data, and
//!   the text the binary prints.
//! - `trace` (#535): one run's per-minute probe rows and an ASCII crop of its map.
//! - `cli`, `legacy`: the subcommands (`verdict`, `trace`, and the older `sweep`, `supply`, `mastery`,
//!   `economy`).

pub mod cli;
pub mod config;
pub mod legacy;
pub mod metric;
pub mod report;
pub mod runner;
pub mod style;
pub mod trace;
pub mod verdict;

#[cfg(test)]
mod testkit;

pub use config::{Config, ConfigError};
pub use style::{Play, Style, StyleError, Styles};
pub use verdict::{Bucket, Status, Verdict, judge};
