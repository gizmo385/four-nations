//! One-to-many pathfinding: the cost to every reachable tile from a single origin.
//!
//! `path::find` answers "how do I get from here to there"; the job system asks "which of these
//! twenty places is nearest, and can I reach any of them", which is one Dijkstra flood instead
//! of twenty A* searches. Steps, costs and the no-corner-cutting rule are the same as
//! `path::find`, so a path read out of a flood is a legal, cheapest path under the same
//! `MoveRules`. Ties are broken by tile index, so results are deterministic.

use std::cmp::Reverse;
use std::collections::BinaryHeap;

use crate::geom::Pos;
use crate::mapgen::TileMap;
use crate::path::{DIAGONAL, DIAGONAL_SIDES, MoveRules, NEIGHBORS, STRAIGHT};

const UNSEEN: u32 = u32::MAX;

/// Cheapest costs from `origin` to every tile it can reach.
#[derive(Debug, Clone)]
pub struct Flood {
    width: i32,
    origin: Pos,
    cost: Vec<u32>,
    came_from: Vec<u32>,
}

impl Flood {
    /// Floods outward from `origin`, which counts as reachable even if its own tile is
    /// impassable (a unit may be standing somewhere it could no longer walk onto).
    pub fn new(map: &TileMap, origin: Pos, rules: &MoveRules) -> Self {
        let (w, len) = (map.width(), (map.width() * map.height()) as usize);
        let idx = |p: Pos| (p.y * w + p.x) as usize;
        let pos = |i: usize| Pos::new(i as i32 % w, i as i32 / w);
        let mut cost = vec![UNSEEN; len];
        let mut came_from = vec![u32::MAX; len];
        let mut closed = vec![false; len];
        let mut open = BinaryHeap::new();
        if map.in_bounds(origin) {
            cost[idx(origin)] = 0;
            open.push(Reverse((0u32, idx(origin))));
        }
        while let Some(Reverse((c, i))) = open.pop() {
            if closed[i] {
                continue;
            }
            closed[i] = true;
            let p = pos(i);
            let costs = NEIGHBORS.map(|(dx, dy)| {
                let n = Pos::new(p.x + dx, p.y + dy);
                map.get(n).and_then(|t| rules.tile_cost(t))
            });
            for (k, (dx, dy)) in NEIGHBORS.into_iter().enumerate() {
                let Some(tile_cost) = costs[k] else { continue };
                let step = if k < 4 {
                    STRAIGHT
                } else {
                    let (h, v) = DIAGONAL_SIDES[k - 4];
                    if costs[h].is_none() || costs[v].is_none() {
                        continue;
                    }
                    DIAGONAL
                };
                let n = idx(Pos::new(p.x + dx, p.y + dy));
                let nc = c + step * tile_cost;
                if !closed[n] && nc < cost[n] {
                    cost[n] = nc;
                    came_from[n] = i as u32;
                    open.push(Reverse((nc, n)));
                }
            }
        }
        Self { width: w, origin, cost, came_from }
    }

    pub fn origin(&self) -> Pos {
        self.origin
    }

    /// The cost of the cheapest path to `p` (same units as `path::path_cost`), or `None` if it
    /// can't be reached.
    pub fn cost(&self, p: Pos) -> Option<u32> {
        if p.x < 0 || p.y < 0 || p.x >= self.width {
            return None;
        }
        match self.cost.get((p.y * self.width + p.x) as usize) {
            Some(&c) if c != UNSEEN => Some(c),
            _ => None,
        }
    }

    /// The cheapest path from the origin to `p`, both included.
    pub fn path_to(&self, p: Pos) -> Option<Vec<Pos>> {
        self.cost(p)?;
        let w = self.width;
        let pos = |i: usize| Pos::new(i as i32 % w, i as i32 / w);
        let mut path = vec![p];
        let mut cur = (p.y * w + p.x) as usize;
        while pos(cur) != self.origin {
            cur = self.came_from[cur] as usize;
            path.push(pos(cur));
        }
        path.reverse();
        Some(path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::defs::{BiomeId, Terrain};
    use crate::geom::Grid;
    use crate::mapgen::Tile;
    use crate::path;

    fn map(rows: &[&str]) -> TileMap {
        Grid::from_fn(rows[0].len() as i32, rows.len() as i32, |p| Tile {
            elevation: 0,
            temperature: 0,
            moisture: 0,
            terrain: if rows[p.y as usize].as_bytes()[p.x as usize] == b'#' {
                Terrain::Mountain
            } else if rows[p.y as usize].as_bytes()[p.x as usize] == b'~' {
                Terrain::Water
            } else {
                Terrain::Land
            },
            biome: BiomeId(0),
            resource: None,
            building: None,
        })
    }

    #[test]
    fn flood_costs_match_astar_everywhere() {
        let m = map(&["........", ".~~~~~..", "...#....", ".#.#.~~.", "...#....", "........"]);
        let rules = MoveRules::default().with_cost(Terrain::Mountain, Some(3));
        let from = Pos::new(0, 0);
        let flood = Flood::new(&m, from, &rules);
        for (p, _) in m.iter() {
            let astar =
                path::find(&m, from, p, &rules).map(|path| path::path_cost(&m, &path, &rules));
            assert_eq!(flood.cost(p), astar, "{p:?}");
            if let Some(path) = flood.path_to(p) {
                assert_eq!(path.first(), Some(&from));
                assert_eq!(path::path_cost(&m, &path, &rules), flood.cost(p).unwrap());
            }
        }
    }

    #[test]
    fn unreachable_and_out_of_bounds_tiles_have_no_cost() {
        let m = map(&["..~..", "..~.."]);
        let flood = Flood::new(&m, Pos::new(0, 0), &MoveRules::default());
        assert_eq!(flood.cost(Pos::new(4, 0)), None);
        assert_eq!(flood.cost(Pos::new(-1, 0)), None);
        assert_eq!(flood.cost(Pos::new(9, 9)), None);
        assert_eq!(flood.path_to(Pos::new(4, 1)), None);
        assert_eq!(flood.cost(Pos::new(1, 1)), Some(14));
    }
}
