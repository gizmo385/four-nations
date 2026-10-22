//! Rivers and lakes from an elevation field.
//!
//! 1. **Depression filling** (priority-flood, Barnes et al. 2014, "+epsilon" variant): flood
//!    inward from the sea in elevation order, raising every pit just enough that each land
//!    tile has a strictly downhill path to the sea. Tiles raised by more than a threshold are
//!    lakes.
//! 2. **Flow**: each land tile drains to its lowest 8-neighbor on the filled surface.
//! 3. **Accumulation**: rain (tile moisture) is summed downstream, highest tiles first. Tiles
//!    whose catchment exceeds a threshold are rivers. Accumulation never decreases downstream,
//!    so every river tile flows, through rivers and lakes only, to the sea.

use std::cmp::Ordering;
use std::collections::BinaryHeap;

use crate::geom::{Grid, Pos};

/// Height added per step when flooding a flat or a pit, so flats still drain toward the outlet.
const EPSILON: f64 = 1e-7;

pub struct Hydrology {
    pub lake: Grid<bool>,
    pub river: Grid<bool>,
}

/// Min-heap entry ordered by height, then position (for deterministic tie-breaking).
struct Entry(f64, Pos);

impl Ord for Entry {
    fn cmp(&self, other: &Self) -> Ordering {
        // Reversed: BinaryHeap is a max-heap and we want the lowest first.
        other.0.total_cmp(&self.0).then_with(|| other.1.cmp(&self.1))
    }
}
impl PartialOrd for Entry {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}
impl PartialEq for Entry {
    fn eq(&self, other: &Self) -> bool {
        self.cmp(other) == Ordering::Equal
    }
}
impl Eq for Entry {}

/// Raises depressions so every non-sea tile drains to the sea. Returns the filled surface.
pub fn fill_depressions(elevation: &Grid<f64>, sea: &Grid<bool>) -> Grid<f64> {
    let mut filled = elevation.clone();
    let mut done = sea.clone();
    let mut heap: BinaryHeap<Entry> =
        sea.iter().filter(|(_, s)| **s).map(|(p, _)| Entry(elevation[p], p)).collect();
    // A map with no sea at all: drain off the edges instead.
    if heap.is_empty() {
        for p in elevation.positions() {
            if elevation.neighbors(p, false).count() < 4 {
                done[p] = true;
                heap.push(Entry(elevation[p], p));
            }
        }
    }
    while let Some(Entry(h, p)) = heap.pop() {
        for q in elevation.neighbors(p, true) {
            if done[q] {
                continue;
            }
            done[q] = true;
            filled[q] = elevation[q].max(h + EPSILON);
            heap.push(Entry(filled[q], q));
        }
    }
    filled
}

/// The lowest strictly-lower 8-neighbor on the filled surface, if any.
fn downstream(filled: &Grid<f64>, p: Pos) -> Option<Pos> {
    filled
        .neighbors(p, true)
        .filter(|&q| filled[q] < filled[p])
        .min_by(|&a, &b| filled[a].total_cmp(&filled[b]))
}

pub fn rivers_and_lakes(
    elevation: &Grid<f64>,
    sea: &Grid<bool>,
    rain: &Grid<f64>,
    river_threshold: f64,
    lake_depth: f64,
) -> Hydrology {
    let filled = fill_depressions(elevation, sea);
    let lake = filled.map_with(|p, &f| !sea[p] && f - elevation[p] > lake_depth);

    let mut order: Vec<Pos> = filled.positions().filter(|&p| !sea[p]).collect();
    order.sort_by(|&a, &b| filled[b].total_cmp(&filled[a]).then(a.cmp(&b)));
    let mut acc = rain.map_with(|p, &r| if sea[p] { 0.0 } else { r });
    for p in order {
        if let Some(d) = downstream(&filled, p) {
            acc[d] += acc[p];
        }
    }
    let river = acc.map_with(|p, &a| !sea[p] && !lake[p] && a >= river_threshold);
    Hydrology { lake, river }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pit_is_filled_and_drains() {
        // A bowl with a single low outlet on the left edge.
        let elev = Grid::from_fn(5, 5, |p| {
            if p == Pos::new(2, 2) {
                0.1
            } else if p.x == 0 && p.y == 2 {
                -1.0
            } else {
                1.0
            }
        });
        let sea = elev.map_with(|_, &e| e < 0.0);
        let filled = fill_depressions(&elev, &sea);
        assert!(filled[Pos::new(2, 2)] > 1.0);
        for p in elev.positions().filter(|&p| !sea[p]) {
            assert!(downstream(&filled, p).is_some(), "{p:?} has nowhere to drain");
        }
    }

    #[test]
    fn rivers_need_enough_rain() {
        // A slope down to the sea at x = 0.
        let elev = Grid::from_fn(20, 3, |p| f64::from(p.x) - 0.5);
        let sea = elev.map_with(|_, &e| e < 0.0);
        let rain = elev.map_with(|_, _| 1.0);
        let h = rivers_and_lakes(&elev, &sea, &rain, 10.0, 0.5);
        // Accumulation grows toward the sea, so only the low end is a river.
        assert!(h.river.iter().any(|(_, r)| *r));
        assert!(!h.river[Pos::new(19, 1)]);
        assert!(h.lake.iter().all(|(_, l)| !l));
    }
}
