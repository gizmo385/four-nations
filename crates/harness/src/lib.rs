//! The balance harness (`docs/design/balance-harness.md`, section 8): the crate that reads
//! `data/harness.toml`, which `fn_sim` never does. So far it holds only the play styles
//! (`style`, #536); the runner, targets and verdict come with #537.

pub mod style;

pub use style::{Play, Style, StyleError, Styles};
