//! Aqueducts (`[building.aqueduct]`, docs/design/horizons-phase-6.md, "`aqueducts`", #755).
//!
//! A finished aqueduct tile is **flowing** when it is 4-adjacent to one of its def's `source`
//! terrains, or to a flowing aqueduct tile. One flood over the aqueduct tiles finds them all:
//! every 4-connected run that touches a source runs from end to end, and a run cut off from one
//! is **dry**. A flowing tile:
//!
//! - is a drinking place: a thirsty unit drinks standing beside it (8 neighbours, as at a river),
//!   and the water is fresh, so it never rolls sickness (`plan_drink`, `sickness_chance`,
//!   `Flow::beside`);
//! - irrigates farm plots within `irrigation_radius`, under the best-boost rule
//!   (`World::irrigation_rate` takes the best of the channel, a canal, an aqueduct and rain).
//!
//! It is not a bank: the Fishing Dock and Tide Mill still need real water.
//!
//! The answer is a bitmap, derived and never saved: `World::flowing` holds it, dropped whenever
//! the map is edited (`World::map_changed`: an aqueduct finishing or coming down, a canal
//! carved beside a run) and rebuilt at the start of the next tick, and after load. A query
//! between rebuilds that finds no fresh answer floods on the spot, so it is always right.

use std::borrow::Cow;

use crate::geom::Grid;

use super::buildings::Kind;
use super::*;

/// The flowing tiles a query reads: nothing with no aqueduct standing.
pub(super) struct Flow<'a>(Option<Cow<'a, Grid<bool>>>);

impl Flow<'_> {
    /// Whether `p` has a flowing aqueduct tile among its 8 neighbours: a spot to drink.
    pub(super) fn beside(&self, map: &TileMap, p: Pos) -> bool {
        self.0.as_ref().is_some_and(|g| map.neighbors(p, true).any(|q| g[q]))
    }
}

/// One run of aqueduct, as the cursor panel reads it (`World::aqueduct_flow`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AqueductFlow {
    /// The run carries water: `tiles` finished tiles, fed at `from`, the first tile (row by row)
    /// that touches a source.
    Flowing { tiles: u32, from: Pos },
    /// No tile of the run touches a source: `Dry: no river upstream`.
    Dry,
}

impl World {
    /// Whether the finished aqueduct tile on `pos` is flowing.
    pub fn is_flowing(&self, pos: Pos, defs: &Defs) -> bool {
        self.flow(defs).0.is_some_and(|g| g.get(pos).copied().unwrap_or(false))
    }

    /// The flowing bitmap: the cached one if fresh, else a new flood, and nothing with no
    /// aqueduct standing.
    pub(super) fn flow(&self, defs: &Defs) -> Flow<'_> {
        if !self.has_kind(Kind::Aqueduct) {
            return Flow(None);
        }
        Flow(Some(match &self.flowing {
            Some(g) => Cow::Borrowed(g),
            None => Cow::Owned(self.flood_flowing(defs)),
        }))
    }

    /// Rebuilds the bitmap if the map was edited since the last flood. Once a tick.
    pub(super) fn refresh_aqueducts(&mut self, defs: &Defs) {
        if self.flowing.is_none() && self.has_kind(Kind::Aqueduct) {
            self.flowing = Some(self.flood_flowing(defs));
        }
    }

    /// Whether the aqueduct tile on `pos` touches a source of its def.
    fn touches_source(&self, pos: Pos, defs: &Defs) -> bool {
        let Some(def) = self.building_at(pos).and_then(|b| defs.building(b).aqueduct.as_ref())
        else {
            return false;
        };
        self.map.neighbors(pos, false).any(|q| def.source.contains(&self.map[q].terrain))
    }

    /// The flood itself (module docs).
    fn flood_flowing(&self, defs: &Defs) -> Grid<bool> {
        let mut flowing = Grid::from_fn(self.map.width(), self.map.height(), |_| false);
        let mut stack: Vec<Pos> = self
            .of_kind(Kind::Aqueduct)
            .map(|b| b.anchor)
            .filter(|&p| self.touches_source(p, defs))
            .collect();
        for &p in &stack {
            flowing[p] = true;
        }
        while let Some(p) = stack.pop() {
            for n in self.map.neighbors(p, false) {
                if !flowing[n] && self.is_aqueduct(n, defs) {
                    flowing[n] = true;
                    stack.push(n);
                }
            }
        }
        flowing
    }

    fn is_aqueduct(&self, pos: Pos, defs: &Defs) -> bool {
        self.building_at(pos).is_some_and(|b| defs.building(b).aqueduct.is_some())
    }

    /// The boost the aqueducts give the farm plot on `pos`: the best `growth_boost` among
    /// flowing tiles within their own `irrigation_radius`, else 1.0. One of the readings
    /// `World::irrigation_rate` takes the best of.
    pub(super) fn aqueduct_rate(&self, pos: Pos, defs: &Defs) -> f32 {
        let flow = self.flow(defs);
        let Some(g) = flow.0.as_ref() else { return 1.0 };
        let mut best = 1.0_f32;
        for b in self.of_kind(Kind::Aqueduct) {
            let Some(def) = defs.building(b.id).aqueduct.as_ref() else { continue };
            if b.anchor.chebyshev(pos) <= def.irrigation_radius as i32 && g[b.anchor] {
                best = best.max(def.growth_boost);
            }
        }
        best
    }

    /// The run of aqueduct holding the finished tile on `pos`: flowing (how many tiles, and where
    /// it is fed) or dry. `None` if `pos` holds no aqueduct. For the cursor panel's `Flowing: 20
    /// tiles from (40, 61)` and `Dry: no river upstream`.
    pub fn aqueduct_flow(&self, pos: Pos, defs: &Defs) -> Option<AqueductFlow> {
        if !self.is_aqueduct(pos, defs) {
            return None;
        }
        let mut seen = std::collections::BTreeSet::from([pos]);
        let mut stack = vec![pos];
        while let Some(p) = stack.pop() {
            for n in self.map.neighbors(p, false) {
                if self.is_aqueduct(n, defs) && seen.insert(n) {
                    stack.push(n);
                }
            }
        }
        let from = seen
            .iter()
            .copied()
            .filter(|&p| self.touches_source(p, defs))
            .min_by_key(|p| (p.y, p.x));
        Some(match from {
            Some(from) => AqueductFlow::Flowing { tiles: seen.len() as u32, from },
            None => AqueductFlow::Dry,
        })
    }
}

#[cfg(test)]
mod tests;
