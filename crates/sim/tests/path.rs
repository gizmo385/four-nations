//! Pathfinding on generated maps, plus the 256x256 performance budget from issue #2.
//!
//! The budget (< 5 ms per query) is only asserted in optimized builds; debug builds just check
//! correctness and print timings. Measure with:
//!
//!     cargo test --release -p fn_sim --test path -- --nocapture
// Test code, not the sim: it times queries against the perf budget. The lint in clippy.toml is for the library (AGENTS.md rule 4).
#![allow(clippy::disallowed_types, clippy::disallowed_methods)]

use std::time::{Duration, Instant};

use fn_sim::path::{self, MoveRules};
use fn_sim::{Defs, Grid, MapGenConfig, Pos, Terrain, TileMap, mapgen};

const BUDGET: Duration = Duration::from_millis(5);

/// 4-connected BFS over passable tiles from `start`: every reached tile, nearest first. With
/// no corner cutting, 8-way reachability is exactly 4-way reachability, so this is the region
/// `path::find` can reach.
fn flood(map: &TileMap, start: Pos, rules: &MoveRules, seen: &mut Grid<bool>) -> Vec<Pos> {
    seen[start] = true;
    let mut order = vec![start];
    let mut i = 0;
    while let Some(&p) = order.get(i) {
        i += 1;
        for n in map.neighbors(p, false) {
            if !seen[n] && rules.passable(&map[n]) {
                seen[n] = true;
                order.push(n);
            }
        }
    }
    order
}

fn farthest(map: &TileMap, start: Pos, rules: &MoveRules) -> Pos {
    let mut seen = Grid::from_fn(map.width(), map.height(), |_| false);
    *flood(map, start, rules, &mut seen).last().unwrap()
}

/// Passable regions, largest first (ties by first tile, for determinism).
fn regions(map: &TileMap, rules: &MoveRules) -> Vec<Vec<Pos>> {
    let mut seen = Grid::from_fn(map.width(), map.height(), |_| false);
    let mut out = Vec::new();
    for p in map.positions() {
        if !seen[p] && rules.passable(&map[p]) {
            out.push(flood(map, p, rules, &mut seen));
        }
    }
    out.sort_by_key(|r| (std::cmp::Reverse(r.len()), r[0]));
    out
}

/// Best of several runs, so a busy machine doesn't fail the budget by accident.
fn time(f: impl Fn() -> Option<Vec<Pos>>) -> (Duration, Option<Vec<Pos>>) {
    let mut best = Duration::MAX;
    let mut out = None;
    for _ in 0..5 {
        let t = Instant::now();
        out = f();
        best = best.min(t.elapsed());
    }
    (best, out)
}

fn check_budget(what: &str, took: Duration) {
    println!("{what}: {took:?}");
    if !cfg!(debug_assertions) {
        assert!(took < BUDGET, "{what} took {took:?}, budget {BUDGET:?}");
    }
}

fn legal(map: &TileMap, p: &[Pos], rules: &MoveRules) -> bool {
    p.windows(2).all(|w| {
        let (a, b) = (w[0], w[1]);
        let (dx, dy) = (b.x - a.x, b.y - a.y);
        dx.abs() <= 1
            && dy.abs() <= 1
            && rules.passable(&map[b])
            && (dx == 0
                || dy == 0
                || (rules.passable(&map[Pos::new(a.x + dx, a.y)])
                    && rules.passable(&map[Pos::new(a.x, a.y + dy)])))
    })
}

#[test]
fn long_path_across_a_generated_256_map() {
    let defs = Defs::bundled();
    let cfg = MapGenConfig { seed: 11, width: 256, height: 256, ..Default::default() };
    let map = mapgen::generate(&cfg, &defs);
    let rules = MoveRules::default();

    // Approximate the diameter of the largest walkable region (double BFS sweep).
    let regions = regions(&map, &rules);
    let a = farthest(&map, regions[0][0], &rules);
    let b = farthest(&map, a, &rules);

    let (took, p) = time(|| path::find(&map, a, b, &rules));
    let p = p.expect("BFS says b is reachable from a");
    assert_eq!((p[0], *p.last().unwrap()), (a, b));
    assert!(legal(&map, &p, &rules));
    println!("generated 256x256: {a:?} -> {b:?}, {} steps", p.len());
    check_budget("generated 256x256, long path", took);

    // Same query again gives the same path.
    assert_eq!(path::find(&map, a, b, &rules), Some(p));

    // Worst case for A*: a walkable but unreachable target, so the whole region is explored.
    let other = regions[1][0];
    let (took, p) = time(|| path::find(&map, a, other, &rules));
    assert_eq!(p, None);
    println!("largest region: {} tiles", regions[0].len());
    check_budget("generated 256x256, unreachable target", took);
}

#[test]
fn serpentine_256_map() {
    // Walls every other row with alternating gaps: the only route snakes through every row.
    let defs = Defs::bundled();
    let base = mapgen::generate(&MapGenConfig { seed: 1, ..Default::default() }, &defs);
    let land =
        *base.iter().find(|(_, t)| t.terrain == Terrain::Land && t.resource.is_none()).unwrap().1;
    let map = Grid::from_fn(256, 256, |p| {
        let wall = p.y % 2 == 1 && if p.y % 4 == 1 { p.x != 255 } else { p.x != 0 };
        let mut t = land;
        if wall {
            t.terrain = Terrain::Mountain;
        }
        t
    });
    let rules = MoveRules::default();
    let (took, p) = time(|| path::find(&map, Pos::new(0, 0), Pos::new(0, 254), &rules));
    let p = p.unwrap();
    assert!(legal(&map, &p, &rules));
    assert!(p.len() > 127 * 255, "path must snake through every row, got {}", p.len());
    check_budget("serpentine 256x256", took);

    // Sealed-off target behind a full wall: explores every open tile, then gives up.
    let mut sealed = map.clone();
    for x in 0..256 {
        sealed[Pos::new(x, 253)].terrain = Terrain::Mountain;
    }
    let (took, p) = time(|| path::find(&sealed, Pos::new(0, 0), Pos::new(0, 254), &rules));
    assert_eq!(p, None);
    check_budget("serpentine 256x256, unreachable", took);

    // Open field, corner to corner: the heuristic is exact, so this is a straight dive.
    let mut open = Grid::from_fn(256, 256, |_| land);
    let (took, p) = time(|| path::find(&open, Pos::new(0, 0), Pos::new(255, 255), &rules));
    assert_eq!(p.map(|p| p.len()), Some(256));
    check_budget("open 256x256, corner to corner", took);

    // Open field, target boxed in: all 65k tiles are explored before giving up. This is the
    // pathological case (~2 heap operations per tile) and is reported, not held to the budget:
    // callers should avoid it with a reachability check (#42).
    for (dx, dy) in [(-1, -1), (0, -1), (1, -1), (-1, 0), (1, 0), (-1, 1), (0, 1), (1, 1)] {
        open[Pos::new(200 + dx, 200 + dy)].terrain = Terrain::Mountain;
    }
    let (took, p) = time(|| path::find(&open, Pos::new(0, 0), Pos::new(200, 200), &rules));
    assert_eq!(p, None);
    println!("open 256x256, boxed-in target (not budgeted): {took:?}");
}
