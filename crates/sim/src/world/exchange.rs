//! The From newcomers list (docs/design/horizons-phase-6.md, "Borrowed techs: `teachable`" and
//! "Fusions", #753): what a United City can learn from the people of the other nations living
//! in it. Each other nation's teachable techs, with their borrowed cost and where they stand,
//! then the fusions. Empty until the Harmony Gate stands, so the pre-gate game shows nothing
//! new. The TUI and the agent surface both read it; nothing here is saved.

use crate::nation::Nation;
use crate::research::{Condition, TechId};

use super::*;

/// Why an entry is on the list.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExchangeKind {
    /// Another nation's `teachable` tech, learned at `borrow_cost` times its own.
    Borrowed { from: Nation },
    /// A fusion of two nations (`TechDef::fusion`).
    Fusion { pair: [Nation; 2] },
}

/// One thing the settlement still needs before an entry can be queued.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExchangeNeed {
    /// A prereq that isn't known yet (a borrowed tech's waived prereqs aren't listed).
    Tech(TechId),
    /// A `requires` condition, including the residents a borrowed tech implies.
    Condition(Condition),
}

/// Where an entry stands for this settlement.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExchangeStanding {
    /// Already learned.
    Known,
    /// In the research queue.
    Queued,
    /// Nothing is missing: it can be set as the target now.
    Available,
    /// Something is missing, in prereq order, then the conditions' (`Requires::unmet`'s) order.
    Locked(Vec<ExchangeNeed>),
}

/// One row of the From newcomers page.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExchangeEntry {
    pub tech: TechId,
    pub kind: ExchangeKind,
    /// Knowledge points this settlement pays, before the difficulty preset's `research_cost`:
    /// `cost x borrow_cost` for a borrowed tech, the tech's own `cost` for a fusion
    /// (`TechDef::cost_for`).
    pub cost: u32,
    pub standing: ExchangeStanding,
}

/// The From newcomers page: each other nation's teachable techs (nations in `Nation::ALL` order,
/// techs in file order), then the fusions in file order.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Exchange {
    pub entries: Vec<ExchangeEntry>,
}

impl Exchange {
    /// Whether there is nothing to learn: always before the gate.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// The borrowed techs of `nation`, in file order.
    pub fn borrowed_from(&self, nation: Nation) -> impl Iterator<Item = &ExchangeEntry> {
        self.entries.iter().filter(move |e| e.kind == ExchangeKind::Borrowed { from: nation })
    }

    /// The fusions, in file order.
    pub fn fusions(&self) -> impl Iterator<Item = &ExchangeEntry> {
        self.entries.iter().filter(|e| matches!(e.kind, ExchangeKind::Fusion { .. }))
    }
}

impl World {
    /// The From newcomers list for this settlement, empty until the gate is complete
    /// ([`united`](Self::united)). It lists the other nations' teachable techs even while none
    /// of their people live here: the standing says what is missing. A hidden teaching is never
    /// listed.
    pub fn exchange(&self, defs: &Defs) -> Exchange {
        if !self.united() {
            return Exchange::default();
        }
        let home = self.nation;
        let facts = self.facts();
        let research = self.research();
        let entry = |id: TechId, kind: ExchangeKind| {
            let tech = defs.tech(id);
            let standing = if research.is_known(id) {
                ExchangeStanding::Known
            } else if research.queue().contains(&id) {
                ExchangeStanding::Queued
            } else {
                let prereqs = match kind {
                    ExchangeKind::Borrowed { .. } => defs.borrow_prereqs(id),
                    ExchangeKind::Fusion { .. } => tech.prereqs.clone(),
                };
                let needs: Vec<ExchangeNeed> = prereqs
                    .into_iter()
                    .filter(|&p| !research.is_known(p))
                    .map(ExchangeNeed::Tech)
                    .chain(tech.unmet(home, &facts, defs).into_iter().map(ExchangeNeed::Condition))
                    .collect();
                if needs.is_empty() {
                    ExchangeStanding::Available
                } else {
                    ExchangeStanding::Locked(needs)
                }
            };
            ExchangeEntry {
                tech: id,
                kind,
                cost: tech.cost_for(home, defs.research.borrow_cost),
                standing,
            }
        };
        let mut entries = Vec::new();
        for from in Nation::ALL.into_iter().filter(|&n| n != home) {
            for (i, t) in defs.techs.iter().enumerate() {
                if t.teachable && t.nation == Some(from) && t.teaching.is_none() {
                    entries.push(entry(TechId(i as u16), ExchangeKind::Borrowed { from }));
                }
            }
        }
        for (i, t) in defs.techs.iter().enumerate() {
            if let Some(pair) = t.fusion {
                entries.push(entry(TechId(i as u16), ExchangeKind::Fusion { pair }));
            }
        }
        Exchange { entries }
    }
}

#[cfg(test)]
mod tests;
