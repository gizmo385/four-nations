//! A* pathfinding on the tile grid.
//!
//! Units move 8-way. A diagonal step is only allowed when both tiles it squeezes between are
//! passable too ("no corner cutting"), so a path never slips through the gap where two walls
//! touch at a corner.
//!
//! Which tiles are passable, and how expensive they are, comes from a `MoveRules` value rather
//! than from `Terrain::is_walkable` directly. That keeps the search itself nation-agnostic: a
//! Water Tribe unit that can cross water just searches with different rules (AGENTS.md rule 7).
//!
//! **Rail** (#135): a straight step from one rail tile onto the next costs `MoveRules::rail_step`
//! instead of the usual `STRAIGHT * tile cost`, when the rules have rail switched on. Which units
//! get that is decided by which `MoveRules` the world hands them (`World::rules_for`), so this file
//! never asks who is walking. Because the cost of a step now depends on the tile it leaves as well
//! as the one it enters, searches ask `MoveRules::step_cost`, not `tile_cost`.
//!
//! Costs are integers, so results never depend on float rounding. Ties in the open set are
//! broken by a total order (`f`, then `h`, then tile index), so the same query on the same map
//! always returns the same path.

use std::cmp::Reverse;
use std::collections::BinaryHeap;

use crate::defs::Terrain;
use crate::geom::Pos;
use crate::mapgen::{Tile, TileMap};

/// Cost of one cardinal step onto a tile of cost 1.
pub(crate) const STRAIGHT: u32 = 10;
/// Cost of one diagonal step onto a tile of cost 1 (10 * sqrt(2), rounded).
pub(crate) const DIAGONAL: u32 = 14;

/// The terrains `MoveRules::terrain_cost` holds: every one but `Tunnel`, whose cost is a field
/// of its own so saves from before tunnels (#132), with seven entries here, still load.
const TERRAINS: usize = 7;

/// `t`'s entry in `MoveRules::terrain_cost`, or `None` for a tunnel.
fn terrain_index(t: Terrain) -> Option<usize> {
    Some(match t {
        Terrain::Water => 0,
        Terrain::Coast => 1,
        Terrain::Land => 2,
        Terrain::Mountain => 3,
        Terrain::River => 4,
        Terrain::Lake => 5,
        Terrain::Ford => 6,
        Terrain::Tunnel => return None,
    })
}

/// A tunnel is walked like open ground unless the rules say otherwise.
fn walk() -> Option<u32> {
    Some(1)
}

fn is_walk(cost: &Option<u32>) -> bool {
    *cost == walk()
}

/// Where a unit may walk and how much each tile costs to enter.
///
/// A terrain cost of `None` means impassable; `Some(c)` multiplies the step length by `c` (so 1
/// is normal speed, 2 is twice as slow). Costs must be at least 1.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct MoveRules {
    terrain_cost: [Option<u32>; TERRAINS],
    /// What entering a tunnel costs (#132): 1, like `Terrain::is_walkable` ground, for every
    /// rules value today. Left out of saves while it is 1, so they read as before tunnels.
    #[serde(default = "walk", skip_serializing_if = "is_walk")]
    tunnel_cost: Option<u32>,
    /// Whether tiles holding a resource (trees, rocks, ...) block movement.
    pub resources_block: bool,
    /// What a straight step from one rail tile onto the next one costs, in the units of
    /// `STRAIGHT` (so below `STRAIGHT` is a speed-up). 0 means these rules ignore rail. Saves
    /// from before rail existed have 0.
    #[serde(default)]
    rail_step: u32,
}

impl Default for MoveRules {
    /// Walking on foot: every `Terrain::is_walkable` tile at cost 1, resources block.
    fn default() -> Self {
        let mut terrain_cost = [None; TERRAINS];
        for t in [
            Terrain::Water,
            Terrain::Coast,
            Terrain::Land,
            Terrain::Mountain,
            Terrain::River,
            Terrain::Lake,
            Terrain::Ford,
        ] {
            terrain_cost[terrain_index(t).expect("not a tunnel")] = t.is_walkable().then_some(1);
        }
        let tunnel_cost = Terrain::Tunnel.is_walkable().then_some(1);
        Self { terrain_cost, tunnel_cost, resources_block: true, rail_step: 0 }
    }
}

impl MoveRules {
    /// Returns these rules with `terrain` costing `cost` (`None` = impassable).
    pub fn with_cost(mut self, terrain: Terrain, cost: Option<u32>) -> Self {
        assert!(cost != Some(0), "terrain costs must be >= 1");
        match terrain_index(terrain) {
            Some(i) => self.terrain_cost[i] = cost,
            None => self.tunnel_cost = cost,
        }
        self
    }

    /// Returns these rules with rail switched on: a straight step between two rail tiles costs
    /// `step` (at least 1, and `STRAIGHT` or less) whatever the ground under them.
    pub fn with_rail(mut self, step: u32) -> Self {
        assert!((1..=STRAIGHT).contains(&step), "a rail step costs 1..={STRAIGHT}, not {step}");
        self.rail_step = step;
        self
    }

    /// The cost of a rail step, or 0 if these rules ignore rail.
    pub fn rail_step(&self) -> u32 {
        self.rail_step
    }

    pub fn terrain_cost(&self, terrain: Terrain) -> Option<u32> {
        match terrain_index(terrain) {
            Some(i) => self.terrain_cost[i],
            None => self.tunnel_cost,
        }
    }

    /// The cost multiplier for entering `tile`, or `None` if it can't be entered.
    pub fn tile_cost(&self, tile: &Tile) -> Option<u32> {
        if self.resources_block && tile.resource.is_some() {
            return None;
        }
        if tile.building.is_some_and(|b| !b.passable()) {
            return None;
        }
        self.terrain_cost(tile.terrain)
    }

    pub fn passable(&self, tile: &Tile) -> bool {
        self.tile_cost(tile).is_some()
    }

    /// The cost of one step from `from` onto the adjacent tile `to`, in the units `find` and
    /// `Flood` minimise (`STRAIGHT` or `DIAGONAL` times the tile cost, or `rail_step` along a
    /// track), or `None` if `to` can't be entered. Rail only speeds a straight step: a diagonal
    /// between two rail tiles is an ordinary diagonal, because a track doesn't run that way.
    pub fn step_cost(&self, from: &Tile, to: &Tile, diagonal: bool) -> Option<u32> {
        let cost = self.tile_cost(to)?;
        if diagonal {
            return Some(DIAGONAL * cost);
        }
        let rail = |t: &Tile| t.building.is_some_and(|b| b.rail());
        if self.rail_step > 0 && rail(from) && rail(to) {
            return Some(self.rail_step.min(STRAIGHT * cost));
        }
        Some(STRAIGHT * cost)
    }

    /// The cheapest a straight step can cost, in `STRAIGHT` units: the heuristic's scale, so it
    /// never overestimates. The cheapest passable terrain, or the rail if that is cheaper.
    fn min_step(&self) -> u32 {
        let costs = self.terrain_cost.iter().chain([&self.tunnel_cost]).flatten();
        let ground = costs.copied().min().unwrap_or(1) * STRAIGHT;
        if self.rail_step > 0 { ground.min(self.rail_step) } else { ground }
    }
}

/// Octile distance in step-cost units: the exact cost of the best path on an open, cost-1 map.
fn octile(a: Pos, b: Pos) -> u32 {
    let dx = a.x.abs_diff(b.x);
    let dy = a.y.abs_diff(b.y);
    let (lo, hi) = (dx.min(dy), dx.max(dy));
    DIAGONAL * lo + STRAIGHT * (hi - lo)
}

/// Cardinals first (indices 0..4), then diagonals.
pub(crate) const NEIGHBORS: [(i32, i32); 8] =
    [(0, -1), (1, 0), (0, 1), (-1, 0), (1, -1), (1, 1), (-1, 1), (-1, -1)];
/// For each diagonal in `NEIGHBORS[4..]`, the indices of the two cardinals it squeezes between.
pub(crate) const DIAGONAL_SIDES: [(usize, usize); 4] = [(1, 0), (1, 2), (3, 2), (3, 0)];

/// Packs an open-set entry as `f | h | index`, so integer order is (f, h, index) order.
fn key(f: u32, h: u32, index: usize) -> u128 {
    (u128::from(f) << 64) | (u128::from(h) << 32) | index as u128
}

/// Finds a cheapest path from `from` to `to`, both included. `Some(vec![from])` if they are the
/// same tile, `None` if either end is out of bounds or impassable, or `to` is unreachable.
pub fn find(map: &TileMap, from: Pos, to: Pos, rules: &MoveRules) -> Option<Vec<Pos>> {
    let cost_at = |p: Pos| map.get(p).and_then(|t| rules.tile_cost(t));
    cost_at(from)?;
    cost_at(to)?;
    if from == to {
        return Some(vec![from]);
    }

    let w = map.width();
    let len = (w * map.height()) as usize;
    let idx = |p: Pos| (p.y * w + p.x) as usize;
    let pos = |i: usize| Pos::new(i as i32 % w, i as i32 / w);
    let min_step = rules.min_step();
    let h = |p: Pos| octile(p, to) * min_step / STRAIGHT;

    const UNSEEN: u32 = u32::MAX;
    let mut g = vec![UNSEEN; len];
    let mut came_from = vec![u32::MAX; len];
    let mut closed = vec![false; len];
    // Min-heap on (f, h, index), packed into one integer: a total order, so pop order (and
    // the path) is deterministic, and one integer compares faster than a tuple.
    let mut open = BinaryHeap::new();

    let start = idx(from);
    let goal = idx(to);
    g[start] = 0;
    open.push(Reverse(key(h(from), h(from), start)));

    while let Some(Reverse(k)) = open.pop() {
        let i = k as u32 as usize;
        if closed[i] {
            continue; // Stale entry: a cheaper route was found after this was pushed.
        }
        if i == goal {
            let mut path = vec![to];
            let mut cur = i;
            while cur != start {
                cur = came_from[cur] as usize;
                path.push(pos(cur));
            }
            path.reverse();
            return Some(path);
        }
        closed[i] = true;
        let p = pos(i);
        // Look each neighbor up once; the diagonal checks below reuse the cardinal results.
        let here = &map[p];
        let costs = NEIGHBORS.map(|(dx, dy)| {
            let t = map.get(Pos::new(p.x + dx, p.y + dy))?;
            rules.step_cost(here, t, dx != 0 && dy != 0)
        });
        for (k, (dx, dy)) in NEIGHBORS.into_iter().enumerate() {
            let Some(step) = costs[k] else { continue };
            if k >= 4 {
                // No corner cutting: both orthogonal tiles beside the diagonal must be passable.
                let (horizontal, vertical) = DIAGONAL_SIDES[k - 4];
                if costs[horizontal].is_none() || costs[vertical].is_none() {
                    continue;
                }
            }
            let n = Pos::new(p.x + dx, p.y + dy);
            let ni = idx(n);
            if closed[ni] {
                continue;
            }
            let ng = g[i] + step;
            if ng < g[ni] {
                g[ni] = ng;
                came_from[ni] = i as u32;
                let hn = h(n);
                open.push(Reverse(key(ng + hn, hn, ni)));
            }
        }
    }
    None
}

/// Total cost of `path` under `rules`, in the same units `find` minimizes. Panics if the path
/// isn't a sequence of legal steps.
pub fn path_cost(map: &TileMap, path: &[Pos], rules: &MoveRules) -> u32 {
    path.windows(2)
        .map(|w| {
            let (a, b) = (w[0], w[1]);
            let (dx, dy) = (b.x - a.x, b.y - a.y);
            assert!(dx.abs() <= 1 && dy.abs() <= 1 && (dx, dy) != (0, 0), "bad step {a:?}->{b:?}");
            rules
                .step_cost(&map[a], &map[b], dx != 0 && dy != 0)
                .expect("path enters an impassable tile")
        })
        .sum()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::defs::BiomeId;
    use crate::geom::Grid;

    /// Builds a map from ASCII rows: `.` land, `#` mountain, `~` water, `,` coast.
    fn map(rows: &[&str]) -> TileMap {
        let h = rows.len() as i32;
        let w = rows[0].len() as i32;
        Grid::from_fn(w, h, |p| {
            let terrain = match rows[p.y as usize].as_bytes()[p.x as usize] {
                b'.' => Terrain::Land,
                b'#' => Terrain::Mountain,
                b'~' => Terrain::Water,
                b',' => Terrain::Coast,
                c => panic!("bad map char {}", c as char),
            };
            Tile {
                elevation: 0,
                temperature: 0,
                moisture: 0,
                terrain,
                biome: BiomeId(0),
                resource: None,
                building: None,
            }
        })
    }

    fn assert_legal(m: &TileMap, path: &[Pos], rules: &MoveRules) {
        for w in path.windows(2) {
            let (a, b) = (w[0], w[1]);
            assert!(rules.passable(&m[b]), "{b:?} impassable");
            let (dx, dy) = (b.x - a.x, b.y - a.y);
            assert!(dx.abs() <= 1 && dy.abs() <= 1 && (dx, dy) != (0, 0));
            if dx != 0 && dy != 0 {
                assert!(rules.passable(&m[Pos::new(a.x + dx, a.y)]), "corner cut at {a:?}");
                assert!(rules.passable(&m[Pos::new(a.x, a.y + dy)]), "corner cut at {a:?}");
            }
        }
    }

    #[test]
    fn straight_and_diagonal_on_open_ground() {
        let m = map(&[".....", ".....", ".....", ".....", "....."]);
        let r = MoveRules::default();
        let p = find(&m, Pos::new(0, 0), Pos::new(4, 0), &r).unwrap();
        assert_eq!(p.len(), 5);
        let p = find(&m, Pos::new(0, 0), Pos::new(4, 4), &r).unwrap();
        assert_eq!(p, (0..5).map(|i| Pos::new(i, i)).collect::<Vec<_>>());
        assert_eq!(path_cost(&m, &p, &r), 4 * DIAGONAL);
    }

    #[test]
    fn same_tile_is_a_one_step_path() {
        let m = map(&["..", ".."]);
        let p = Pos::new(1, 1);
        assert_eq!(find(&m, p, p, &MoveRules::default()), Some(vec![p]));
    }

    #[test]
    fn unreachable_target_returns_none() {
        let m = map(&["..#..", "..#..", "..#.."]);
        let r = MoveRules::default();
        assert_eq!(find(&m, Pos::new(0, 1), Pos::new(4, 1), &r), None);
        // Impassable or out-of-bounds endpoints.
        assert_eq!(find(&m, Pos::new(0, 1), Pos::new(2, 1), &r), None);
        assert_eq!(find(&m, Pos::new(2, 1), Pos::new(0, 1), &r), None);
        assert_eq!(find(&m, Pos::new(0, 1), Pos::new(9, 1), &r), None);
    }

    #[test]
    fn no_corner_cutting() {
        // The only diagonal gap is between two walls touching at a corner.
        let m = map(&[
            ".#...", //
            "#....", //
            ".....",
        ]);
        let r = MoveRules::default();
        let p = find(&m, Pos::new(0, 0), Pos::new(1, 1), &r);
        assert_eq!(p, None, "(0,0) is sealed in unless it cuts the corner");

        let m = map(&[
            "...", //
            ".#.", //
            "...",
        ]);
        let p = find(&m, Pos::new(0, 1), Pos::new(1, 0), &r).unwrap();
        // Must go via (0,0): (0,1)->(1,0) diagonally would clip the wall at (1,1).
        assert_eq!(p, vec![Pos::new(0, 1), Pos::new(0, 0), Pos::new(1, 0)]);
        assert_legal(&m, &p, &r);
    }

    #[test]
    fn routes_around_walls() {
        let m = map(&[
            ".......", //
            ".#####.", //
            ".....#.", //
            "####.#.", //
            ".......",
        ]);
        let r = MoveRules::default();
        let p = find(&m, Pos::new(0, 2), Pos::new(6, 2), &r).unwrap();
        assert_legal(&m, &p, &r);
        assert_eq!(p.first(), Some(&Pos::new(0, 2)));
        assert_eq!(p.last(), Some(&Pos::new(6, 2)));
    }

    #[test]
    fn terrain_costs_change_the_route() {
        // Straight across the coast strip is shorter, but coast costs 5 here, so go around.
        let m = map(&[
            ".......", //
            ",,,,,,,", //
            ",,,,,,,", //
            ".......",
        ]);
        let cheap = MoveRules::default();
        let slow = MoveRules::default().with_cost(Terrain::Coast, Some(5));
        let a = Pos::new(0, 1);
        let b = Pos::new(6, 1);
        let direct = find(&m, a, b, &cheap).unwrap();
        assert!(direct.iter().all(|p| p.y == 1));
        let around = find(&m, a, b, &slow).unwrap();
        assert!(around.iter().any(|p| p.y == 0), "{around:?}");
        assert!(path_cost(&m, &around, &slow) < path_cost(&m, &direct, &slow));
    }

    #[test]
    fn rules_can_open_water() {
        // A Water Tribe-style rule set: sea is passable, so the island is reachable.
        let m = map(&["..~~..", "..~~..", "..~~.."]);
        let a = Pos::new(0, 1);
        let b = Pos::new(5, 1);
        assert_eq!(find(&m, a, b, &MoveRules::default()), None);
        let swim = MoveRules::default().with_cost(Terrain::Water, Some(2));
        let p = find(&m, a, b, &swim).unwrap();
        assert_legal(&m, &p, &swim);
    }

    #[test]
    fn resources_block_unless_rules_say_otherwise() {
        let mut m = map(&["...", "...", "..."]);
        for y in 0..3 {
            m[Pos::new(1, y)].resource = Some(crate::defs::ResourceId(0));
        }
        let mut r = MoveRules::default();
        assert_eq!(find(&m, Pos::new(0, 1), Pos::new(2, 1), &r), None);
        r.resources_block = false;
        assert!(find(&m, Pos::new(0, 1), Pos::new(2, 1), &r).is_some());
    }

    #[test]
    fn a_building_blocks_or_allows_pathing_per_its_def() {
        use crate::building::TileBuilding;
        let mut m = map(&["...", "...", "..."]);
        let r = MoveRules::default();
        for y in 0..3 {
            m[Pos::new(1, y)].building = Some(TileBuilding::wall(false));
        }
        assert_eq!(find(&m, Pos::new(0, 1), Pos::new(2, 1), &r), None);
        for y in 0..3 {
            m[Pos::new(1, y)].building = Some(TileBuilding::wall(true));
        }
        assert!(find(&m, Pos::new(0, 1), Pos::new(2, 1), &r).is_some());
    }

    #[test]
    fn optimal_and_deterministic_with_many_ties() {
        // An open field has many equal-cost paths; the same one must come back every time.
        let m = map(&["........"; 8]);
        let r = MoveRules::default();
        let (a, b) = (Pos::new(0, 2), Pos::new(7, 5));
        let p = find(&m, a, b, &r).unwrap();
        assert_eq!(path_cost(&m, &p, &r), octile(a, b));
        for _ in 0..10 {
            assert_eq!(find(&m, a, b, &r).as_ref(), Some(&p));
        }
    }

    /// Marks `tiles` of a map as rail.
    fn lay_rail(m: &mut TileMap, tiles: impl IntoIterator<Item = Pos>) {
        use crate::building::{BuildingKey, TileBuilding};
        for p in tiles {
            m[p].building = Some(TileBuilding::new(BuildingKey::NONE, true, true));
        }
    }

    #[test]
    fn rail_makes_a_longer_route_cheaper_and_only_for_rules_that_ask() {
        // The rail runs up, along the top and back down; the direct line is 20 steps.
        let mut m = map(&[
            ".....................",
            ".....................",
            ".....................",
            ".....................",
        ]);
        let rail: Vec<Pos> = (0..=3)
            .map(|y| Pos::new(0, y))
            .chain((1..=20).map(|x| Pos::new(x, 0)))
            .chain((1..=3).map(|y| Pos::new(20, y)))
            .collect();
        lay_rail(&mut m, rail);
        let (a, b) = (Pos::new(0, 3), Pos::new(20, 3));

        let foot = MoveRules::default();
        let direct = find(&m, a, b, &foot).unwrap();
        assert_eq!(path_cost(&m, &direct, &foot), 20 * STRAIGHT, "rail means nothing on foot");
        assert!(direct.iter().all(|p| p.y == 3));

        let cart = MoveRules::default().with_rail(STRAIGHT / 2);
        let railed = find(&m, a, b, &cart).unwrap();
        assert!(railed.contains(&Pos::new(10, 0)), "the cart takes the rail: {railed:?}");
        assert_eq!(path_cost(&m, &railed, &cart), (3 + 20 + 3) * STRAIGHT / 2);
        assert!(path_cost(&m, &railed, &cart) < path_cost(&m, &direct, &cart));
        assert_eq!(find(&m, a, b, &cart), find(&m, a, b, &cart), "deterministic");
    }

    #[test]
    fn a_flood_agrees_with_a_search_when_rail_is_on() {
        use crate::flood::Flood;
        let mut m = map(&["........", ".~~~~~..", "...#....", ".#.#.~~.", "...#....", "........"]);
        lay_rail(&mut m, (0..8).map(|x| Pos::new(x, 0)).chain((0..6).map(|y| Pos::new(7, y))));
        let rules = MoveRules::default().with_cost(Terrain::Mountain, Some(3)).with_rail(4);
        let flood = Flood::new(&m, Pos::new(0, 0), &rules);
        for (p, _) in m.iter() {
            let astar =
                find(&m, Pos::new(0, 0), p, &rules).map(|path| path_cost(&m, &path, &rules));
            assert_eq!(flood.cost(p), astar, "{p:?}");
        }
    }

    #[test]
    fn rail_ignores_the_ground_under_it() {
        let mut m = map(&[",,,,,"]);
        lay_rail(&mut m, (0..5).map(|x| Pos::new(x, 0)));
        let rules = MoveRules::default().with_cost(Terrain::Coast, Some(3)).with_rail(5);
        let p = find(&m, Pos::new(0, 0), Pos::new(4, 0), &rules).unwrap();
        assert_eq!(path_cost(&m, &p, &rules), 4 * 5, "a rail bed is level");
    }
}
