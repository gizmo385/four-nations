//! Map generation invariants and golden snapshots.
//!
//! Golden files live in `tests/golden/`. If you changed generation *on purpose*, regenerate them
//! with `FN_BLESS=1 cargo test -p fn_sim` and review the diff before committing.

use fn_sim::{Defs, MapGenConfig, Terrain, mapgen};

fn cfg(seed: u64) -> MapGenConfig {
    MapGenConfig { seed, width: 48, height: 24, ..Default::default() }
}

#[test]
fn same_seed_same_map() {
    let defs = Defs::bundled();
    assert_eq!(mapgen::generate(&cfg(3), &defs), mapgen::generate(&cfg(3), &defs));
}

#[test]
fn different_seed_different_map() {
    let defs = Defs::bundled();
    assert_ne!(mapgen::generate(&cfg(3), &defs), mapgen::generate(&cfg(4), &defs));
}

#[test]
fn border_is_water() {
    let defs = Defs::bundled();
    for seed in 0..10 {
        let c = cfg(seed);
        let map = mapgen::generate(&c, &defs);
        for (p, t) in map.iter() {
            let edge = p.x < c.water_border
                || p.y < c.water_border
                || p.x >= c.width - c.water_border
                || p.y >= c.height - c.water_border;
            if edge {
                assert_eq!(t.terrain, Terrain::Water, "seed {seed} at {p:?}");
            }
        }
    }
}

#[test]
fn land_next_to_water_is_coast() {
    let defs = Defs::bundled();
    for seed in 0..10 {
        let map = mapgen::generate(&cfg(seed), &defs);
        for (p, t) in map.iter() {
            if t.terrain == Terrain::Land {
                assert!(
                    map.neighbors(p, true).all(|q| map[q].terrain != Terrain::Water),
                    "seed {seed}: inland tile {p:?} touches water"
                );
            }
        }
    }
}

#[test]
fn resources_respect_their_terrain_rules() {
    let defs = Defs::bundled();
    for seed in 0..10 {
        for (_, t) in mapgen::generate(&cfg(seed), &defs).iter() {
            if let Some(r) = t.resource {
                let def = defs.resource(r);
                if let Some(ts) = &def.terrains {
                    assert!(ts.contains(&t.terrain), "{} on {:?}", def.id, t.terrain);
                }
            }
        }
    }
}

#[test]
fn golden_ascii_seed_1() {
    let defs = Defs::bundled();
    let actual = mapgen::to_ascii(&mapgen::generate(&cfg(1), &defs), &defs);
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/golden/seed1_48x24.txt");
    if std::env::var_os("FN_BLESS").is_some() {
        std::fs::write(path, &actual).unwrap();
    }
    let expected = std::fs::read_to_string(path).expect("golden missing; run with FN_BLESS=1");
    assert_eq!(actual, expected, "map generation output changed; see module docs");
}
