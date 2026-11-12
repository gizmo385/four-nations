//! Sheltered rings (docs/design/horizons-phase-4.md, `agrarian_ring`, #721).
//!
//! A tile is a **bound** if a finished building named in `[enclosure] walls` stands on it. One
//! flood starts from every map-edge tile that isn't a bound and steps in eight directions through
//! every tile that isn't a bound, whatever its terrain: mountains and water don't close a ring
//! (#716). Every other tile that isn't a bound is **sheltered**, and belongs to one ring (an
//! eight-connected pocket the flood never reached). A diagonal step in a wall is a gap, the same
//! gap a walker could slip through.
//!
//! The answer is derived and never saved: `World::enclosure` holds it, rebuilt at the start of a
//! tick when a bound has gone up or come down since (`World::walls_changed`), when the feature is
//! learned, and after load. A query between rebuilds that finds no fresh answer floods on the
//! spot, so it is always right.

use crate::geom::Grid;

use super::*;

/// One flood's answer: per tile, the ring it is in (an index into `sizes`), or none.
#[derive(Debug, Clone, Default)]
pub(super) struct Enclosure {
    /// The feature was known when this was flooded; `None` below means "not known".
    rings: Option<Grid<Option<u32>>>,
    /// Tiles in each ring.
    sizes: Vec<u32>,
}

impl World {
    /// Whether `pos` is inside a closed ring while `sheltered_rings` is known.
    pub fn is_sheltered(&self, pos: Pos, defs: &Defs) -> bool {
        self.ring_size(pos, defs).is_some()
    }

    /// The tiles in the ring sheltering `pos`, for the cursor panel's `Sheltered: inside a ring
    /// of 256 tiles`; `None` if nothing shelters it.
    pub fn ring_size(&self, pos: Pos, defs: &Defs) -> Option<u32> {
        let known = self.has_feature(crate::research::Feature::ShelteredRings, defs)
            && defs.seasons.enclosure.is_some();
        if !known {
            return None;
        }
        let read = |e: &Enclosure| {
            let ring = (*e.rings.as_ref()?.get(pos)?)?;
            Some(e.sizes[ring as usize])
        };
        match &self.enclosure {
            Some(e) if e.rings.is_some() => read(e),
            _ => read(&self.flood_enclosure(defs)),
        }
    }

    /// No unit on `pos` is cold: it's sheltered, and `[enclosure] cold` is false.
    pub(super) fn ring_keeps_cold_off(&self, pos: Pos, defs: &Defs) -> bool {
        defs.seasons.enclosure.as_ref().is_some_and(|e| !e.cold) && self.is_sheltered(pos, defs)
    }

    /// The first frost spares a ripe crop on `pos`: it's sheltered, and `[enclosure] frost` is
    /// false.
    pub(super) fn ring_spares_frost(&self, pos: Pos, defs: &Defs) -> bool {
        defs.seasons.enclosure.as_ref().is_some_and(|e| !e.frost) && self.is_sheltered(pos, defs)
    }

    /// The winter growth floor on `pos`, if it's sheltered.
    pub(super) fn ring_winter_growth(&self, pos: Pos, defs: &Defs) -> Option<f32> {
        let e = defs.seasons.enclosure.as_ref()?;
        self.is_sheltered(pos, defs).then_some(e.winter_growth)
    }

    /// A finished building `id` went up or came down: if it closes rings, the answer is stale.
    pub(super) fn walls_changed(&mut self, id: crate::building::BuildingId, defs: &Defs) {
        if defs.seasons.enclosure.as_ref().is_some_and(|e| e.walls.contains(&id)) {
            self.enclosure = None;
        }
    }

    /// Rebuilds `enclosure` if it is missing or was flooded before the feature was known (or
    /// after it stopped being). Called once a tick.
    pub(super) fn refresh_enclosure(&mut self, defs: &Defs) {
        let known = self.has_feature(crate::research::Feature::ShelteredRings, defs)
            && defs.seasons.enclosure.is_some();
        let fresh = self.enclosure.as_ref().is_some_and(|e| e.rings.is_some() == known);
        if !fresh {
            self.enclosure =
                Some(if known { self.flood_enclosure(defs) } else { Enclosure::default() });
        }
    }

    /// The flood itself (module docs).
    fn flood_enclosure(&self, defs: &Defs) -> Enclosure {
        let Some(def) = defs.seasons.enclosure.as_ref() else { return Enclosure::default() };
        let (w, h) = (self.map.width(), self.map.height());
        let bound = Grid::from_fn(w, h, |p| {
            self.map[p].building.is_some()
                && self.building_at(p).is_some_and(|id| def.walls.contains(&id))
        });
        // 0: not yet reached; 1: outside (reached from the edge); 2: a bound.
        let mut seen = Grid::from_fn(w, h, |p| if bound[p] { 2u8 } else { 0 });
        let mut stack: Vec<Pos> = Vec::new();
        for p in seen.positions() {
            let edge = p.x == 0 || p.y == 0 || p.x == w - 1 || p.y == h - 1;
            if edge && seen[p] == 0 {
                seen[p] = 1;
                stack.push(p);
            }
        }
        while let Some(p) = stack.pop() {
            for n in eight(p) {
                if seen.get(n) == Some(&0) {
                    seen[n] = 1;
                    stack.push(n);
                }
            }
        }
        let mut rings: Grid<Option<u32>> = Grid::from_fn(w, h, |_| None);
        let mut sizes = Vec::new();
        for start in seen.positions() {
            if seen[start] != 0 || rings[start].is_some() {
                continue;
            }
            let id = sizes.len() as u32;
            let mut count = 0;
            rings[start] = Some(id);
            stack.push(start);
            while let Some(p) = stack.pop() {
                count += 1;
                for n in eight(p) {
                    if seen.get(n) == Some(&0) && rings[n].is_none() {
                        rings[n] = Some(id);
                        stack.push(n);
                    }
                }
            }
            sizes.push(count);
        }
        Enclosure { rings: Some(rings), sizes }
    }
}

/// The eight tiles around `p`.
fn eight(p: Pos) -> impl Iterator<Item = Pos> {
    (-1..=1)
        .flat_map(|dy| (-1..=1).map(move |dx| (dx, dy)))
        .filter(|&d| d != (0, 0))
        .map(move |(dx, dy)| Pos::new(p.x + dx, p.y + dy))
}

#[cfg(test)]
mod tests;
