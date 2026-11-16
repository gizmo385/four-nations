//! A United City's balance with the spirits (docs/design/spirits.md, F6 of the factory endgame,
//! #1030; the numbers are `crate::spirits::SpiritsDef`). Once a minute after the gate, balance
//! (0 to 100) moves by calm minus unrest:
//!
//! - **Unrest**: the draw on every running yard (`World::yard_loads`), and trees felled since
//!   the last minute (`Tallies::school("trees_felled")`, counted where a tree's tile empties).
//! - **Calm**: a base, and the living trees on the map, up to a cap.
//!
//! No RNG. Before the gate nothing runs and nothing is written to a save.

use serde::{Deserialize, Serialize};

use super::*;

const MINUTE: u64 = 60 * TICKS_PER_SECOND as u64;

/// The last minute's terms, for the readout.
#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize)]
pub struct BalanceTerms {
    /// Unrest from machine draw.
    pub machines: f32,
    /// Unrest from trees felled.
    pub felled: f32,
    /// Calm that comes with time.
    pub base: f32,
    /// Calm from living trees.
    pub trees: f32,
}

impl BalanceTerms {
    /// Calm minus unrest: how far balance moved, before the clamp.
    pub fn net(&self) -> f32 {
        self.base + self.trees - self.machines - self.felled
    }
}

/// The balance and what moved it last (`World::balance`).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Spirits {
    /// `None` until the first minute after the gate: read as `SpiritsDef::start`.
    #[serde(default)]
    pub balance: Option<f32>,
    /// The `trees_felled` tally at the last minute.
    #[serde(default)]
    pub felled_seen: u32,
    #[serde(default)]
    pub last: Option<BalanceTerms>,
}

impl Spirits {
    pub fn is_default(&self) -> bool {
        *self == Self::default()
    }
}

impl World {
    /// The city's balance with the spirits, 0 to 100, once the gate stands: `None` before.
    pub fn balance(&self, defs: &Defs) -> Option<f32> {
        self.united().then(|| self.spirits.balance.unwrap_or(defs.spirits.start))
    }

    /// The last minute's terms, once a minute has passed since the gate.
    pub fn balance_terms(&self) -> Option<BalanceTerms> {
        self.spirits.last
    }

    /// Living trees on the map (`SpiritsDef::trees`).
    pub fn living_trees(&self, defs: &Defs) -> u32 {
        if defs.spirits.trees.is_empty() {
            return 0;
        }
        self.map
            .iter()
            .filter(|(_, t)| t.resource.is_some_and(|r| defs.spirits.trees.contains(&r)))
            .count() as u32
    }

    /// Counts a tree felled if the resource on `pos`, about to be cleared, is one.
    pub(super) fn note_felled(&mut self, pos: Pos, defs: &Defs) {
        if self
            .map
            .get(pos)
            .and_then(|t| t.resource)
            .is_some_and(|r| defs.spirits.trees.contains(&r))
        {
            self.tallies.count_school("trees_felled", 1);
        }
    }

    /// This minute's terms, as they stand now.
    pub fn balance_now(&self, defs: &Defs) -> BalanceTerms {
        let s = &defs.spirits;
        let draw: u32 = self.yard_loads(defs).values().map(|l| l.draw).sum();
        let felled = self.tallies.school("trees_felled").saturating_sub(self.spirits.felled_seen);
        let trees = self.living_trees(defs) / s.trees_per_calm;
        BalanceTerms {
            machines: draw as f32 * s.unrest_per_draw,
            felled: felled as f32 * s.unrest_per_tree,
            base: s.calm_base,
            trees: (trees as f32 * s.calm_per_trees).min(s.trees_calm_max),
        }
    }

    /// Once a minute in a United City: balance moves by calm minus unrest.
    pub(super) fn tick_spirits(&mut self, defs: &Defs) {
        if !self.tick.is_multiple_of(MINUTE) || !self.united() {
            return;
        }
        let terms = self.balance_now(defs);
        let now = self.spirits.balance.unwrap_or(defs.spirits.start);
        self.spirits.balance = Some((now + terms.net()).clamp(0.0, 100.0));
        self.spirits.felled_seen = self.tallies.school("trees_felled");
        self.spirits.last = Some(terms);
    }
}

#[cfg(test)]
mod tests;
