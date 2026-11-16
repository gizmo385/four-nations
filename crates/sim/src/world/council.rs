//! The Republic Council (docs/design/factory-endgame.md, "The goal: the Republic Council",
//! #1030). The Council Hall (`BuildingDef::council`) is the era's capstone: its tech asks for
//! Notables of every nation living in the city, and its bill is steel, brick and every nation's
//! goods. Once one stands, the council sits: each nation's seat lends that nation's strongest
//! work modifier (`Modifiers::strongest`, from `data/nations.toml`) to the whole city, raising
//! the city's own value to it where it is lower. Fire's heating, Earth's earthworks, Water's
//! water work, Air's power reach.
//!
//! Nothing is stored: the seats follow the hall, so taking it down (it is a landmark, so it
//! can't be) or loading a save works it out afresh.

use super::*;

impl World {
    /// Whether a finished Council Hall stands.
    pub fn council_seated(&self) -> bool {
        self.has_kind(Kind::Council)
    }

    /// Each nation's seat while the council sits, in nation order: the nation, the modifier it
    /// lends, and the value. Empty before the hall stands.
    pub fn council_seats(&self, defs: &Defs) -> Vec<(Nation, &'static str, f32)> {
        if !self.council_seated() {
            return Vec::new();
        }
        Nation::ALL
            .iter()
            .map(|&n| {
                let (name, v) = defs.nation(n).modifiers.strongest();
                (n, name, v)
            })
            .collect()
    }

    /// Raises `m` to every seat's lent modifier.
    pub(super) fn lend_council_seats(&self, m: &mut Modifiers, defs: &Defs) {
        for (_, name, v) in self.council_seats(defs) {
            m.raise(name, v);
        }
    }
}

#[cfg(test)]
mod tests;
