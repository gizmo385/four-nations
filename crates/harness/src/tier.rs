//! The cost tiers (`docs/design/balance-harness.md`, section 7; #538): which seeds and which part
//! of the panel a run covers. `cargo xtask balance` (#539) and `balance verdict --tier` both read
//! them from here.
//!
//! | Tier | Covers |
//! |---|---|
//! | `quick` | seeds 1-12, all nations, `steady`, Normal |
//! | `full` | the panel (`[panel] seeds`, 1-32) of everything the targets name |
//! | `holdout` | the full tier on seeds 1001-1032 |
//!
//! The quick tier is a subset of the full one, so a quick run pairs with the committed baseline
//! (the full tier's) run for run.

use crate::config::Config;
use crate::runner::Filter;

/// Quick's seeds.
pub const QUICK_SEEDS: &str = "1-12";
/// Holdout's seeds: none of the panel's, for a tuning pass that has iterated on the panel.
pub const HOLDOUT_SEEDS: &str = "1001-1032";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tier {
    Quick,
    Full,
    Holdout,
}

impl Tier {
    pub const ALL: [Tier; 3] = [Tier::Quick, Tier::Full, Tier::Holdout];

    pub fn name(self) -> &'static str {
        match self {
            Tier::Quick => "quick",
            Tier::Full => "full",
            Tier::Holdout => "holdout",
        }
    }

    pub fn parse(name: &str) -> Option<Tier> {
        Tier::ALL.into_iter().find(|t| t.name() == name)
    }

    /// The seed list, in `--seeds` syntax.
    pub fn seeds(self, config: &Config) -> String {
        match self {
            Tier::Quick => QUICK_SEEDS.to_owned(),
            Tier::Full => config.panel.seeds.clone(),
            Tier::Holdout => HOLDOUT_SEEDS.to_owned(),
        }
    }

    /// The part of the panel it runs.
    pub fn filter(self) -> Filter {
        match self {
            Tier::Quick => Filter {
                styles: Some(vec!["steady".to_owned()]),
                presets: Some(vec!["normal".to_owned()]),
                ..Filter::default()
            },
            Tier::Full | Tier::Holdout => Filter::default(),
        }
    }
}

#[cfg(test)]
mod tests;
