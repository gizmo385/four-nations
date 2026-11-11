//! One-to-many pathfinding: the cost to every reachable tile from a single origin.
//!
//! `path::find` answers "how do I get from here to there"; the job system asks "which of these
//! twenty places is nearest, and can I reach any of them", which is one Dijkstra flood instead
//! of twenty A* searches. Steps, costs and the no-corner-cutting rule are the same as
//! `path::find`, so a path read out of a flood is a legal, cheapest path under the same
//! `MoveRules`. Ties are broken by tile index, so results are deterministic.

use std::cell::RefCell;
use std::cmp::Reverse;
use std::collections::{BinaryHeap, HashMap};
use std::sync::Arc;

use crate::geom::Pos;
use crate::mapgen::TileMap;
use crate::path::{DIAGONAL, DIAGONAL_SIDES, MoveRules, NEIGHBORS, STRAIGHT};

const UNSEEN: u32 = u32::MAX;

#[cfg(test)]
thread_local! {
    /// Test-only count of `Flood::new` calls, so a test can assert a cheap pre-check skips the
    /// flood instead of timing it (#322: a stuck build job used to pay for one every retry).
    pub(crate) static CALLS: std::cell::Cell<u32> = const { std::cell::Cell::new(0) };
}

/// What a flood needs to know about each tile of one map under one set of `MoveRules`: its
/// entry cost multiplier (`MoveRules::tile_cost`, `UNSEEN` for impassable) and whether it
/// holds rail. Built once per map and rules, it lets a flood step without looking tiles and
/// rules up again, and lets `World` share one between every flood until the map changes.
#[derive(Debug)]
pub struct StepGrid {
    width: i32,
    height: i32,
    cost: Vec<u32>,
    /// Each tile's track index (`TileBuilding::track`), 0 for none.
    track: Vec<u8>,
    /// `MoveRules::track_steps`.
    track_steps: Vec<u32>,
    /// Connected region per tile (`UNSEEN` for impassable ones), labelled by a 4-way fill the
    /// way `Regions` is (its module doc says why that is exactly what 8-way steps without
    /// corner-cutting reach). Lets a flood answer "unreachable" without searching for it.
    region: Vec<u32>,
}

impl StepGrid {
    pub fn new(map: &TileMap, rules: &MoveRules) -> Self {
        let mut cost = Vec::with_capacity((map.width() * map.height()) as usize);
        let mut track = Vec::with_capacity(cost.capacity());
        for (_, t) in map.iter() {
            cost.push(rules.tile_cost(t).unwrap_or(UNSEEN));
            track.push(t.building.map_or(0, |b| b.track()));
        }
        let (width, height) = (map.width(), map.height());
        let region = label_regions(width, height, &cost);
        let track_steps = rules.track_steps().to_vec();
        Self { width, height, cost, track, track_steps, region }
    }

    fn in_bounds(&self, p: Pos) -> bool {
        p.x >= 0 && p.y >= 0 && p.x < self.width && p.y < self.height
    }

    /// `MoveRules::step_cost` from tile `from` onto its neighbour `to` (both indices).
    fn step(&self, from: usize, to: usize, diagonal: bool) -> Option<u32> {
        let cost = self.cost[to];
        if cost == UNSEEN {
            return None;
        }
        if diagonal {
            return Some(DIAGONAL * cost);
        }
        let t = self.track[from];
        if t != 0
            && t == self.track[to]
            && let Some(&step) = self.track_steps.get(usize::from(t) - 1)
        {
            return Some(step.min(STRAIGHT * cost));
        }
        Some(STRAIGHT * cost)
    }
}

/// `StepGrid::region`: a 4-way flood fill over the passable tiles, in row-major order.
fn label_regions(width: i32, height: i32, cost: &[u32]) -> Vec<u32> {
    let mut label = vec![UNSEEN; cost.len()];
    let mut stack = Vec::new();
    let mut next = 0;
    for start in 0..cost.len() {
        if cost[start] == UNSEEN || label[start] != UNSEEN {
            continue;
        }
        label[start] = next;
        stack.push(start);
        while let Some(i) = stack.pop() {
            let (x, y) = (i as i32 % width, i as i32 / width);
            for (nx, ny) in [(x + 1, y), (x - 1, y), (x, y + 1), (x, y - 1)] {
                if nx < 0 || ny < 0 || nx >= width || ny >= height {
                    continue;
                }
                let n = (ny * width + nx) as usize;
                if cost[n] != UNSEEN && label[n] == UNSEEN {
                    label[n] = next;
                    stack.push(n);
                }
            }
        }
        next += 1;
    }
    label
}

/// Cheapest costs from `origin` to every tile it can reach.
///
/// The Dijkstra search runs lazily: asking for a tile's cost (or a path to it) continues the
/// search just until that tile is settled, so a caller that only asks about nearby tiles never
/// pays for the rest of the map. Tiles settle in exactly the order a search to the end would
/// settle them, so every answer is the one a complete flood gives.
#[derive(Debug, Clone)]
pub struct Flood {
    width: i32,
    origin: Pos,
    grid: Arc<StepGrid>,
    search: RefCell<Search>,
    /// `within`'s bound while it is answering; `UNSEEN` (no bound) otherwise.
    limit: std::cell::Cell<u32>,
    /// The origin's region (`StepGrid::region`) when the origin is passable: every tile it can
    /// reach is in it, so any other tile is unreachable without a search. `None` for an
    /// impassable origin, which can step into more than one.
    home: Option<u32>,
}

#[derive(Debug, Clone)]
struct Search {
    cost: Vec<u32>,
    came_from: Vec<u32>,
    closed: Vec<bool>,
    open: BinaryHeap<Reverse<(u32, usize)>>,
    /// Set by `seek`, which stops early on purpose: no more expanding, and `cost` answers from
    /// whatever it reached, settled or not, as it always has.
    frozen: bool,
}

impl Flood {
    /// Floods outward from `origin`, which counts as reachable even if its own tile is
    /// impassable (a unit may be standing somewhere it could no longer walk onto).
    pub fn new(map: &TileMap, origin: Pos, rules: &MoveRules) -> Self {
        Self::over(Arc::new(StepGrid::new(map, rules)), origin)
    }

    /// `new` over a `StepGrid` already built for the map and rules (`World` keeps one until the
    /// map changes).
    pub fn over(grid: Arc<StepGrid>, origin: Pos) -> Self {
        #[cfg(test)]
        CALLS.with(|c| c.set(c.get() + 1));
        let len = grid.cost.len();
        let mut search = Search {
            cost: vec![UNSEEN; len],
            came_from: vec![u32::MAX; len],
            closed: vec![false; len],
            open: BinaryHeap::new(),
            frozen: false,
        };
        if grid.in_bounds(origin) {
            let i = (origin.y * grid.width + origin.x) as usize;
            search.cost[i] = 0;
            search.open.push(Reverse((0u32, i)));
        }
        let limit = std::cell::Cell::new(UNSEEN);
        let home = grid
            .in_bounds(origin)
            .then(|| grid.region[(origin.y * grid.width + origin.x) as usize])
            .filter(|&r| r != UNSEEN);
        Self { width: grid.width, origin, grid, search: RefCell::new(search), limit, home }
    }

    /// Continues the search until tile `target` is settled, or until everything reachable is.
    fn settle(&self, target: usize) {
        let mut guard = self.search.borrow_mut();
        let s = &mut *guard;
        if s.frozen {
            return;
        }
        let g = &*self.grid;
        let w = g.width;
        let limit = self.limit.get();
        while !s.closed[target] {
            if s.open.peek().is_some_and(|Reverse((c, _))| *c > limit) {
                return;
            }
            let Some(Reverse((c, i))) = s.open.pop() else { return };
            if s.closed[i] {
                continue;
            }
            s.closed[i] = true;
            let (x, y) = (i as i32 % w, i as i32 / w);
            let costs = NEIGHBORS.map(|(dx, dy)| {
                let n = Pos::new(x + dx, y + dy);
                if !g.in_bounds(n) {
                    return None;
                }
                g.step(i, (n.y * w + n.x) as usize, dx != 0 && dy != 0)
            });
            for (k, (dx, dy)) in NEIGHBORS.into_iter().enumerate() {
                let Some(step) = costs[k] else { continue };
                if k >= 4 {
                    let (h, v) = DIAGONAL_SIDES[k - 4];
                    if costs[h].is_none() || costs[v].is_none() {
                        continue;
                    }
                }
                let n = ((y + dy) * w + x + dx) as usize;
                let nc = c + step;
                if !s.closed[n] && nc < s.cost[n] {
                    s.cost[n] = nc;
                    s.came_from[n] = i as u32;
                    s.open.push(Reverse((nc, n)));
                }
            }
        }
    }

    /// Floods from `origin` like `new`, but stops once the nearest tile `probe` accepts has been
    /// confirmed, instead of visiting every reachable tile (#293). `probe` is called on every
    /// tile as it's visited (nearest-cost-first, same order `new` fills `cost` in) with its
    /// walking cost `c`; returning `Some((k, out))` offers `out` as an answer with tie-break key
    /// `k`, compared as `(c, k, tile)` against every other tile `probe` has accepted so far, so
    /// two tiles at the same cost and the same `k` break the tie the same way `new`'s own tile
    /// order would (least `Pos`); `k` lets a caller rank matches further, the way `plan_food`
    /// ranks a stockpile over the ground over foraging. `probe` can read live state no static
    /// target list would (a tile's neighbours, a bed's current occupancy), so this never needs a
    /// second pass over a precomputed set of candidates, only over however much of the map it
    /// actually has to visit before the nearest match is confirmed.
    ///
    /// Because every step costs at least 1 (`MoveRules`'s costs are never 0), every tile at the
    /// cheapest matching cost is already in the open set by the time any of them is popped, so
    /// nothing cheaper is missed by stopping there; this flood just never grows past it. The
    /// returned `Flood` only has `cost`/`path_to` for tiles visited before that point, which is
    /// enough to retrace a path to whatever was found.
    pub fn seek<K: Ord, Out>(
        map: &TileMap,
        origin: Pos,
        rules: &MoveRules,
        mut probe: impl FnMut(Pos, u32) -> Option<(K, Out)>,
    ) -> (Self, Option<(Pos, Out)>) {
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
        let mut best: Option<(u32, K, Pos)> = None;
        let mut answer: Option<Out> = None;
        while let Some(Reverse((c, i))) = open.pop() {
            if best.as_ref().is_some_and(|(bc, _, _)| c > *bc) {
                break;
            }
            if closed[i] {
                continue;
            }
            closed[i] = true;
            let p = pos(i);
            if let Some((k, out)) = probe(p, c) {
                let key = (c, k, p);
                if best.as_ref().is_none_or(|b| key < *b) {
                    best = Some(key);
                    answer = Some(out);
                }
            }
            if best.as_ref().is_none_or(|(bc, _, _)| c < *bc) {
                let here = &map[p];
                let costs = NEIGHBORS.map(|(dx, dy)| {
                    let t = map.get(Pos::new(p.x + dx, p.y + dy))?;
                    rules.step_cost(here, t, dx != 0 && dy != 0)
                });
                for (k, (dx, dy)) in NEIGHBORS.into_iter().enumerate() {
                    let Some(step) = costs[k] else { continue };
                    if k >= 4 {
                        let (h, v) = DIAGONAL_SIDES[k - 4];
                        if costs[h].is_none() || costs[v].is_none() {
                            continue;
                        }
                    }
                    let n = idx(Pos::new(p.x + dx, p.y + dy));
                    let nc = c + step;
                    if !closed[n] && nc < cost[n] {
                        cost[n] = nc;
                        came_from[n] = i as u32;
                        open.push(Reverse((nc, n)));
                    }
                }
            }
        }
        let search = Search { cost, came_from, closed, open: BinaryHeap::new(), frozen: true };
        // Frozen: it never steps again, so its grid needs no tiles.
        let grid = Arc::new(StepGrid {
            width: w,
            height: map.height(),
            cost: Vec::new(),
            track: Vec::new(),
            track_steps: Vec::new(),
            region: Vec::new(),
        });
        let limit = std::cell::Cell::new(UNSEEN);
        let flood =
            Self { width: w, origin, grid, search: RefCell::new(search), limit, home: None };
        (flood, best.map(|(_, _, p)| p).zip(answer))
    }

    /// `seek`, but for the common case of a handful of known `targets` instead of a predicate
    /// over the whole map: a bed's door, say, rather than "any tile beside water". `accept` is
    /// only called for a tile that's one of `targets`, with its payload and the now-known
    /// walking cost `c`, to decide whether it still qualifies (within a bed's `seek_tiles`, and
    /// not already full) and, if so, its tie-break key and answer.
    pub fn nearest<V, K: Ord, Out>(
        map: &TileMap,
        origin: Pos,
        rules: &MoveRules,
        targets: &HashMap<Pos, V>,
        mut accept: impl FnMut(Pos, u32, &V) -> Option<(K, Out)>,
    ) -> (Self, Option<(Pos, Out)>) {
        Self::seek(map, origin, rules, |p, c| targets.get(&p).and_then(|v| accept(p, c, v)))
    }

    /// Runs `f` with every `cost` (and `path_to`) answering `None` for a tile that costs more
    /// than `limit`, reachable or not, so the search never grows past `limit` while `f` runs.
    /// Tiles within it answer exactly as they would without one.
    /// For a caller that only wants something cheaper than what it already has
    /// (`World::pick_job`).
    pub fn within<R>(&self, limit: u32, f: impl FnOnce() -> R) -> R {
        let before = self.limit.replace(limit.min(self.limit.get()));
        let out = f();
        self.limit.set(before);
        out
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
        let i = (p.y * self.width + p.x) as usize;
        if i >= self.search.borrow().cost.len() {
            return None;
        }
        if self.home.is_some_and(|h| self.grid.region[i] != h) {
            return None;
        }
        self.settle(i);
        let s = self.search.borrow();
        match s.cost[i] {
            c if c != UNSEEN && c <= self.limit.get() && (s.closed[i] || s.frozen) => Some(c),
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
        let s = self.search.borrow();
        while pos(cur) != self.origin {
            cur = s.came_from[cur] as usize;
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

    /// `seek` with a live predicate (the shape `plan_drink` uses: "is this tile beside water")
    /// agrees with flooding everywhere and picking the least `(cost, tile)` among every tile
    /// that passes the same predicate, by hand -- for every origin on a map with mountains in
    /// the way and some matching tiles tied in cost.
    #[test]
    fn seek_matches_a_full_flood_and_a_by_hand_minimum_over_a_live_predicate() {
        let m = map(&["........", ".~~~~~..", "...#....", ".#.#.~~.", "...#....", "........"]);
        let rules = MoveRules::default().with_cost(Terrain::Mountain, Some(3));
        let beside_water = |p: Pos| m.neighbors(p, true).any(|q| m[q].terrain == Terrain::Water);
        for (origin, _) in m.iter() {
            let full = Flood::new(&m, origin, &rules);
            let by_hand = m
                .positions()
                .filter(|&p| m[p].terrain != Terrain::Water && beside_water(p))
                .filter_map(|p| full.cost(p).map(|c| (c, p)))
                .min();
            let (_, found) = Flood::seek(&m, origin, &rules, |p, c| {
                (m[p].terrain != Terrain::Water && beside_water(p)).then_some(((), (p, c)))
            });
            match by_hand {
                Some((c, p)) => {
                    let (_, (fp, fc)) = found.expect("by-hand found one");
                    assert_eq!((fc, fp), (c, p), "origin {origin:?}");
                }
                None => assert!(found.is_none(), "origin {origin:?}: by-hand found none"),
            }
        }
    }

    /// `nearest` among a handful of targets agrees with flooding everywhere and picking the
    /// least `(cost, target_position)` by hand, for every origin on a map with some
    /// unreachable targets (behind mountains) and some tied in cost.
    #[test]
    fn nearest_matches_a_full_flood_and_a_by_hand_minimum() {
        let m = map(&["........", ".~~~~~..", "...#....", ".#.#.~~.", "...#....", "........"]);
        let rules = MoveRules::default().with_cost(Terrain::Mountain, Some(3));
        let targets: HashMap<Pos, ()> = [(0, 0), (7, 0), (0, 5), (7, 5), (3, 2), (5, 3), (2, 0)]
            .into_iter()
            .map(|(x, y)| (Pos::new(x, y), ()))
            .collect();
        for (origin, _) in m.iter() {
            let full = Flood::new(&m, origin, &rules);
            let by_hand = targets.keys().filter_map(|&p| full.cost(p).map(|c| (c, p))).min();
            let (_, found) = Flood::nearest(&m, origin, &rules, &targets, |_, c, ()| Some(((), c)));
            match by_hand {
                Some((c, p)) => {
                    let (fp, fc) = found.expect("by-hand found one");
                    assert_eq!((fc, fp), (c, p), "origin {origin:?}");
                }
                None => assert!(found.is_none(), "origin {origin:?}: by-hand found none"),
            }
        }
    }

    /// `accept` can reject a target once its cost is known (a bed out of its `seek_tiles`, say),
    /// in which case the next-nearest one that still qualifies wins.
    #[test]
    fn nearest_skips_targets_accept_rejects_once_their_cost_is_known() {
        let m = map(&["........."]);
        let targets: HashMap<Pos, u32> =
            [(Pos::new(2, 0), 1u32), (Pos::new(6, 0), 5u32)].into_iter().collect();
        let (_, found) = Flood::nearest(&m, Pos::new(0, 0), &MoveRules::default(), &targets, {
            |_, c, &max| (c <= max * path::STRAIGHT).then_some(((), ()))
        });
        // (2,0) costs 2*STRAIGHT, over its own max of 1*STRAIGHT: rejected. (6,0) costs
        // 6*STRAIGHT, within its max of 5*STRAIGHT... also rejected (6 > 5). Lower the bar so
        // one qualifies.
        assert!(found.is_none());
        let targets: HashMap<Pos, u32> =
            [(Pos::new(2, 0), 1u32), (Pos::new(6, 0), 8u32)].into_iter().collect();
        let (flood, found) = Flood::nearest(&m, Pos::new(0, 0), &MoveRules::default(), &targets, {
            |_, c, &max| (c <= max * path::STRAIGHT).then_some(((), ()))
        });
        let (p, ()) = found.expect("the farther target qualifies");
        assert_eq!(p, Pos::new(6, 0));
        assert_eq!(flood.path_to(p).map(|p| p.len()), Some(7));
    }

    /// Among several targets tied in cost, `nearest` picks the one with the least `(K, Pos)`,
    /// the same tie-break a hand-written `.min()` over `(cost, key, pos)` would.
    #[test]
    fn nearest_breaks_cost_ties_by_key_then_position() {
        let m = map(&["........."]);
        // (2,0) and (6,0) are both unreachable-free and strictly farther from (4,0) than (3,0)
        // and (5,0), which tie in cost from it.
        let targets: HashMap<Pos, u8> =
            [(Pos::new(3, 0), 1u8), (Pos::new(5, 0), 0u8)].into_iter().collect();
        let (_, found) = Flood::nearest(&m, Pos::new(4, 0), &MoveRules::default(), &targets, {
            |_, c, &rank| Some((rank, c))
        });
        let (p, _) = found.unwrap();
        assert_eq!(p, Pos::new(5, 0), "lower rank wins the cost tie");
    }
}
