//! The roster counts research conditions read (`research::Facts::population`, `masters`,
//! `residents`; docs/design/horizons-phase-6.md, "Conditions: the rest of `requires`", #750).
//!
//! **Residents** are the units on the roster that are neither guests (the Avatar) nor beasts
//! (`Unit::is_settler`). Airship crew away on a voyage stay on the roster, so they still count.
//! The world fills the counts once a tick, before the research tick, by walking the units: a few
//! dozen rank checks. They are derived, never saved; `save::load` and `World::with_map` fill them
//! too, and `set_research` refreshes them so a command sees the roster as it is.

use super::*;

/// The counts `Facts` carries, by `Nation::index` of each unit's own nation (`World::nation_of`).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(super) struct Roster {
    pub(super) population: u16,
    pub(super) masters: [u16; 4],
    pub(super) residents: [u16; 4],
}

impl World {
    /// Counts the roster now (`Roster`): one pass over the units, no randomness.
    pub(crate) fn refresh_roster(&mut self, defs: &Defs) {
        let mut roster = Roster::default();
        for u in self.residents() {
            let n = self.nation_of(u).index();
            roster.population = roster.population.saturating_add(1);
            roster.residents[n] = roster.residents[n].saturating_add(1);
            if self.rank_of(u, defs).rank == Rank::Master {
                roster.masters[n] = roster.masters[n].saturating_add(1);
            }
        }
        self.roster = roster;
    }
}

/// `Facts` from its parts. A free function over the fields, not a method, so the research tick
/// can hold these facts while `research` is borrowed mutably.
pub(super) fn build_facts<'a>(
    tallies: &Tallies,
    log: &'a EndeavorLog,
    (kept, lost): (Vec<TechId>, Vec<TechId>),
    roster: &Roster,
    united: bool,
) -> research::Facts<'a> {
    research::Facts {
        scrolls_read: tallies.scrolls_read(),
        chapters_met: log.chapters_met().collect(),
        kept,
        lost,
        population: roster.population,
        masters: roster.masters,
        residents: roster.residents,
        united,
    }
}

#[cfg(test)]
mod tests;
