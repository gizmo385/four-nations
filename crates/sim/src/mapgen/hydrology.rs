//! Rivers and lakes from an elevation field.
//!
//! 1. **Depression filling** (priority-flood, Barnes et al. 2014, "+epsilon" variant): flood
//!    inward from the sea in elevation order, raising every pit just enough that each land
//!    tile has a strictly downhill path to the sea. Landlocked sea basins the caller turned
//!    into lakes are sinks too, so their lake stays the size of the old basin instead of
//!    filling up to its spill point. Tiles raised by more than a threshold are lakes, unless
//!    their lake is smaller than `Params::min_lake_tiles` (issue #410): tiny ponds read as
//!    clutter, so they stay land (or river, where enough water runs through them).
//! 2. **Flow**: each land tile drains to its lowest 8-neighbor on the filled surface.
//! 3. **Accumulation**: rain (tile moisture) is summed downstream, highest tiles first. Tiles
//!    whose catchment exceeds a threshold are rivers. Accumulation never decreases downstream,
//!    so every river tile flows, through rivers and lakes only, to the sea or a basin lake.

use std::cmp::Ordering;
use std::collections::{BinaryHeap, VecDeque};

use crate::geom::{Grid, Pos};

/// Height added per step when flooding a flat or a pit, so flats still drain toward the outlet.
const EPSILON: f64 = 1e-7;

pub struct Params {
    /// Rain (summed moisture of upstream tiles) needed to form a river.
    pub river_threshold: f64,
    /// Minimum depth of a filled depression for it to become a lake.
    pub lake_depth: f64,
    /// Smallest lake kept, in 8-connected tiles.
    pub min_lake_tiles: usize,
}

pub struct Hydrology {
    /// Lakes, including the landlocked basins passed in.
    pub lake: Grid<bool>,
    pub river: Grid<bool>,
    /// Rain accumulated at each tile from everything upstream: a proxy for river size.
    pub flow: Grid<f64>,
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

/// Rivers and lakes for land draining to `sea`. `basins` are landlocked sea basins that become
/// lakes: they are sinks like the sea, and part of the returned `lake`.
pub fn rivers_and_lakes(
    elevation: &Grid<f64>,
    sea: &Grid<bool>,
    basins: &Grid<bool>,
    rain: &Grid<f64>,
    params: &Params,
) -> Hydrology {
    let sink = sea.map_with(|p, &s| s || basins[p]);
    let filled = fill_depressions(elevation, &sink);
    let ponds =
        filled.map_with(|p, &f| basins[p] || (!sink[p] && f - elevation[p] > params.lake_depth));
    // Basins are already at least `min_lake_tiles`, so any component holding one survives.
    let lake = without_small_components(&ponds, params.min_lake_tiles);

    let mut order: Vec<Pos> = filled.positions().filter(|&p| !sink[p]).collect();
    order.sort_by(|&a, &b| filled[b].total_cmp(&filled[a]).then(a.cmp(&b)));
    let mut acc = rain.map_with(|p, &r| if sink[p] { 0.0 } else { r });
    for p in order {
        if let Some(d) = downstream(&filled, p) {
            acc[d] += acc[p];
        }
    }
    let river = acc.map_with(|p, &a| !sink[p] && !lake[p] && a >= params.river_threshold);
    Hydrology { lake, river, flow: acc }
}

/// `mask` without its 8-connected components of fewer than `min` tiles.
pub fn without_small_components(mask: &Grid<bool>, min: usize) -> Grid<bool> {
    let mut out = mask.clone();
    let mut seen = mask.map_with(|_, _| false);
    let mut component = Vec::new();
    for start in mask.positions() {
        if !mask[start] || seen[start] {
            continue;
        }
        seen[start] = true;
        component.clear();
        let mut queue = VecDeque::from([start]);
        while let Some(p) = queue.pop_front() {
            component.push(p);
            for q in mask.neighbors(p, true) {
                if mask[q] && !seen[q] {
                    seen[q] = true;
                    queue.push_back(q);
                }
            }
        }
        if component.len() < min {
            for &p in &component {
                out[p] = false;
            }
        }
    }
    out
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
        let none = sea.map_with(|_, _| false);
        let params = Params { river_threshold: 10.0, lake_depth: 0.5, min_lake_tiles: 16 };
        let h = rivers_and_lakes(&elev, &sea, &none, &rain, &params);
        // Accumulation grows toward the sea, so only the low end is a river.
        assert!(h.river.iter().any(|(_, r)| *r));
        assert!(!h.river[Pos::new(19, 1)]);
        assert!(h.lake.iter().all(|(_, l)| !l));
    }

    /// A 12x12 plateau at height 1 draining off the left edge, with two pits: a 2x2 one and a
    /// 5x5 one.
    fn two_pits() -> (Grid<f64>, Grid<bool>) {
        let elev = Grid::from_fn(12, 12, |p| {
            if p.x == 0 {
                -1.0
            } else if ((2..4).contains(&p.x) && (2..4).contains(&p.y))
                || ((5..10).contains(&p.x) && (5..10).contains(&p.y))
            {
                0.0
            } else {
                1.0 + f64::from(p.x) * 0.01
            }
        });
        let sea = elev.map_with(|_, &e| e < -0.5);
        (elev, sea)
    }

    fn lake_components(lake: &Grid<bool>) -> Vec<usize> {
        let mut seen = lake.map_with(|_, _| false);
        let mut sizes = Vec::new();
        for start in lake.positions().filter(|&p| lake[p]) {
            if seen[start] {
                continue;
            }
            seen[start] = true;
            let mut queue = VecDeque::from([start]);
            let mut n = 0;
            while let Some(p) = queue.pop_front() {
                n += 1;
                for q in lake.neighbors(p, true) {
                    if lake[q] && !seen[q] {
                        seen[q] = true;
                        queue.push_back(q);
                    }
                }
            }
            sizes.push(n);
        }
        sizes
    }

    #[test]
    fn lakes_smaller_than_the_minimum_are_dropped() {
        let (elev, sea) = two_pits();
        let none = sea.map_with(|_, _| false);
        let rain = elev.map_with(|_, _| 0.0);
        let params = Params { river_threshold: 1e9, lake_depth: 0.5, min_lake_tiles: 16 };
        let h = rivers_and_lakes(&elev, &sea, &none, &rain, &params);
        assert_eq!(lake_components(&h.lake), vec![25]);
        assert!(!h.lake[Pos::new(2, 2)], "the 4-tile pit should be land");

        let keep_all = Params { min_lake_tiles: 1, ..params };
        let h = rivers_and_lakes(&elev, &sea, &none, &rain, &keep_all);
        let mut sizes = lake_components(&h.lake);
        sizes.sort();
        assert_eq!(sizes, vec![4, 25]);
    }

    #[test]
    fn a_basin_is_a_sink_and_a_lake() {
        // Mark the big pit's middle 3x3 as a landlocked basin: its rim is no longer filled.
        let (elev, sea) = two_pits();
        let basins = elev.map_with(|p, _| (6..9).contains(&p.x) && (6..9).contains(&p.y));
        let rain = elev.map_with(|_, _| 1.0);
        let params = Params { river_threshold: 1e9, lake_depth: 0.5, min_lake_tiles: 9 };
        let h = rivers_and_lakes(&elev, &sea, &basins, &rain, &params);
        assert_eq!(lake_components(&h.lake), vec![9]);
        assert!(basins.iter().all(|(p, &b)| b == h.lake[p]));
        assert_eq!(h.flow[Pos::new(7, 7)], 0.0, "sinks collect nothing");
    }
}
