//! Region connectivity on generated maps, and the unreachable-query budget from issue #42.
//!
//! The budget (< 5 ms) is only asserted in optimized builds; debug builds just check
//! correctness and print timings. Measure with:
//!
//!     cargo test --release -p fn_sim --test regions -- --nocapture

use std::hint::black_box;
use std::time::{Duration, Instant};

use fn_sim::path::{self, MoveRules};
use fn_sim::{Defs, Grid, MapGenConfig, Pos, Regions, Terrain, mapgen};

const BUDGET: Duration = Duration::from_millis(5);

/// Best of several runs, so a busy machine doesn't fail the budget by accident.
fn time<T>(f: impl Fn() -> T) -> (Duration, T) {
    let mut best = Duration::MAX;
    let mut out = None;
    for _ in 0..5 {
        let t = Instant::now();
        out = Some(f());
        best = best.min(t.elapsed());
    }
    (best, out.unwrap())
}

fn check_budget(what: &str, took: Duration) {
    println!("{what}: {took:?}");
    if !cfg!(debug_assertions) {
        assert!(took < BUDGET, "{what} took {took:?}, budget {BUDGET:?}");
    }
}

#[test]
fn connected_agrees_with_path_find_on_generated_maps() {
    let defs = Defs::bundled();
    for seed in [3, 11, 42] {
        let cfg = MapGenConfig { seed, width: 96, height: 96, ..Default::default() };
        let map = mapgen::generate(&cfg, &defs);
        let rules = MoveRules::default();
        let regions = Regions::compute(&map, &rules);
        assert!(regions.count() > 1, "seed {seed}: want several regions to tell apart");

        // Sample tiles across the map, including every region's first tile, and check every
        // pair among them (deterministic: no RNG).
        let mut sample: Vec<Pos> =
            map.positions().filter(|p| (p.x * 7 + p.y * 13) % 29 == 0).collect();
        for p in map.positions() {
            if regions.region(p).is_some_and(|r| (r.0 as usize).is_multiple_of(3))
                && sample.len() < 120
            {
                sample.push(p);
            }
        }
        let (mut yes, mut no) = (0, 0);
        for &a in &sample {
            for &b in &sample {
                let found = path::find(&map, a, b, &rules).is_some();
                assert_eq!(regions.connected(a, b), found, "seed {seed}: {a:?} -> {b:?}");
                if found {
                    yes += 1;
                } else {
                    no += 1;
                }
            }
        }
        assert!(yes > 0 && no > 0, "seed {seed}: sample must hit both answers ({yes}/{no})");
    }
}

#[test]
fn unreachable_query_on_an_open_256_map_fails_fast_with_regions() {
    let defs = Defs::bundled();
    let base = mapgen::generate(&MapGenConfig { seed: 1, ..Default::default() }, &defs);
    let land =
        *base.iter().find(|(_, t)| t.terrain == Terrain::Land && t.resource.is_none()).unwrap().1;
    // Fully open, with the target walled into a pocket: the worst case for `path::find`, which
    // explores all 65k tiles before giving up.
    let mut map = Grid::from_fn(256, 256, |_| land);
    let (from, to) = (Pos::new(0, 0), Pos::new(200, 200));
    for (dx, dy) in [(-1, -1), (0, -1), (1, -1), (-1, 0), (1, 0), (-1, 1), (0, 1), (1, 1)] {
        map[Pos::new(to.x + dx, to.y + dy)].terrain = Terrain::Mountain;
    }
    let rules = MoveRules::default();
    let regions = Regions::compute(&map, &rules);

    // Too quick for the clock to see one at a time, so time 10k and take the average.
    const N: u32 = 10_000;
    let (took, found) = time(|| {
        (0..N)
            .map(|_| regions.find(&map, black_box(from), black_box(to), &rules))
            .next_back()
            .unwrap()
    });
    assert_eq!(found, None);
    check_budget("unreachable query with regions (average)", took / N);

    let (took, connected) =
        time(|| (0..N).all(|_| !regions.connected(black_box(from), black_box(to))));
    assert!(connected, "never connected");
    check_budget("Regions::connected (average)", took / N);

    // What the check saves, and what keeping the cache current costs.
    let (took, searched) = time(|| path::find(&map, from, to, &rules));
    assert_eq!(searched, None);
    println!("same query, plain path::find (not budgeted): {took:?}");
    let (took, _) = time(|| Regions::compute(&map, &rules));
    check_budget("Regions::compute, 256x256", took);

    // Reachable targets still get their path.
    assert!(regions.find(&map, from, Pos::new(255, 255), &rules).is_some());
}
