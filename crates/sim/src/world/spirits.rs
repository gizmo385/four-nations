//! A United City's balance with the spirits (docs/design/spirits.md, F6 of the factory endgame,
//! #1030; the numbers are `crate::spirits::SpiritsDef`). Once a minute after the gate, balance
//! (0 to 100) moves by calm minus unrest:
//!
//! - **Unrest**: the draw on every running yard (`World::yard_loads`), and trees felled since
//!   the last minute (`Tallies::school("trees_felled")`, counted where a tree's tile empties).
//! - **Calm**: a base, and the living trees on the map, up to a cap.
//!
//! While balance is low, **spirit vines** grow (F6b): every `vine_seconds` one appears on the
//! free land tile nearest the busiest yard (the highest draw, then the first by anchor), among
//! the tiles touching that yard's machines or an existing vine within 3 tiles of them. A vine is
//! a map resource, cut like any other. An automated machine whose edge touches one runs at
//! `vine_choke` of its speed (`World::choked`).
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
        // Only trees felled since the gate count: the first minute after it charges none.
        let felled = if self.spirits.balance.is_some() {
            self.tallies.school("trees_felled").saturating_sub(self.spirits.felled_seen)
        } else {
            0
        };
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

impl World {
    /// Whether the building anchored on `pos` has a spirit vine against its edge (F6b).
    pub fn choked(&self, pos: Pos, defs: &Defs) -> bool {
        let Some(vine) = defs.spirits.vines else { return false };
        let Some(b) = self.anchored(pos) else { return false };
        b.footprint().ring().any(|p| self.map.get(p).is_some_and(|t| t.resource == Some(vine)))
    }

    /// Whether a vine may grow on `pos`: open land with nothing on it and nobody standing there.
    fn vine_room(&self, pos: Pos) -> bool {
        self.map.get(pos).is_some_and(|t| {
            t.terrain == crate::defs::Terrain::Land && t.resource.is_none() && t.building.is_none()
        }) && self.site_at(pos).is_none()
            && !self.zones.contains(&pos)
            && !self.ground.contains_key(&pos)
            && self.units_at(pos).next().is_none()
    }

    /// Where the next vine grows, if one is due to: the free tile nearest the busiest yard.
    pub(super) fn next_vine(&self, defs: &Defs) -> Option<Pos> {
        let vine = defs.spirits.vines?;
        let loads = self.yard_loads(defs);
        let (&yard, _) = loads
            .iter()
            .filter(|(_, l)| l.draw > 0)
            .max_by(|a, b| a.1.draw.cmp(&b.1.draw).then(b.0.cmp(a.0)))?;
        let yard_foot = self.footprint_at(yard)?;
        let machines: Vec<crate::geom::Footprint> = self
            .buildings()
            .filter(|b| !b.manual && self.yard_of(b.anchor, defs).is_some_and(|(y, _)| y == yard))
            .map(|b| b.footprint())
            .collect();
        let near = |p: Pos, d: i32| machines.iter().any(|f| f.distance(p) <= d);
        let (mut lo, mut hi) = (Pos::new(i32::MAX, i32::MAX), Pos::new(i32::MIN, i32::MIN));
        for f in &machines {
            for t in f.tiles() {
                lo = Pos::new(lo.x.min(t.x - 4), lo.y.min(t.y - 4));
                hi = Pos::new(hi.x.max(t.x + 4), hi.y.max(t.y + 4));
            }
        }
        let is_vine = |p: Pos| self.map.get(p).is_some_and(|t| t.resource == Some(vine));
        // Never on a door: a machine choked is slowed, never shut off from its supply.
        let doors: std::collections::BTreeSet<Pos> =
            self.buildings().map(|b| b.footprint().entrance()).collect();
        (lo.y..=hi.y)
            .flat_map(|y| (lo.x..=hi.x).map(move |x| Pos::new(x, y)))
            .filter(|&p| self.vine_room(p) && !doors.contains(&p))
            .filter(|&p| {
                near(p, 1)
                    || (near(p, 3)
                        && [(1, 0), (-1, 0), (0, 1), (0, -1)]
                            .iter()
                            .any(|&(dx, dy)| is_vine(Pos::new(p.x + dx, p.y + dy))))
            })
            .min_by_key(|&p| (yard_foot.distance(p), p.y, p.x))
    }

    /// Grows a vine when one is due (F6b).
    pub(super) fn tick_vines(&mut self, defs: &Defs) {
        let s = &defs.spirits;
        let (Some(vine), Some(balance)) = (s.vines, self.balance(defs)) else { return };
        if balance >= s.vine_below {
            return;
        }
        let secs = if balance < s.vine_fast_below { s.vine_seconds / 2.0 } else { s.vine_seconds };
        let every = (secs * TICKS_PER_SECOND as f32).round().max(1.0) as u64;
        if !self.tick.is_multiple_of(every) {
            return;
        }
        if let Some(p) = self.next_vine(defs) {
            self.map[p].resource = Some(vine);
            self.map_changed();
        }
    }
}

#[cfg(test)]
mod tests;
