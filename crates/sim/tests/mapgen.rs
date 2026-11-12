//! Map generation invariants and golden snapshots.
//!
//! Golden files live in `tests/golden/`. If you changed generation *on purpose*, regenerate them
//! with `FN_BLESS=1 cargo test -p fn_sim` and review the diff before committing.
// Test code, not the sim: it reads and blesses golden files. The lint in clippy.toml is for the library (AGENTS.md rule 4).
#![allow(clippy::disallowed_methods)]

use std::collections::VecDeque;

use fn_sim::defs::ResourceDef;
use fn_sim::{Defs, Grid, MapGenConfig, Pos, Terrain, TileMap, mapgen};

fn cfg(seed: u64) -> MapGenConfig {
    MapGenConfig { seed, width: 48, height: 24, ..Default::default() }
}

/// Big enough for rivers, climate bands and resource clusters to show up.
fn big(seed: u64) -> MapGenConfig {
    MapGenConfig { seed, width: 128, height: 96, ..Default::default() }
}

fn maps(cfgs: impl Iterator<Item = MapGenConfig>) -> Vec<(MapGenConfig, TileMap)> {
    let defs = Defs::bundled();
    cfgs.map(|c| {
        let m = mapgen::generate(&c, &defs);
        (c, m)
    })
    .collect()
}

#[test]
fn same_seed_same_map() {
    let defs = Defs::bundled();
    assert_eq!(mapgen::generate(&cfg(3), &defs), mapgen::generate(&cfg(3), &defs));
    assert_eq!(mapgen::generate(&big(3), &defs), mapgen::generate(&big(3), &defs));
}

#[test]
fn different_seed_different_map() {
    let defs = Defs::bundled();
    assert_ne!(mapgen::generate(&cfg(3), &defs), mapgen::generate(&cfg(4), &defs));
}

#[test]
fn border_is_water() {
    for (c, map) in maps((0..10).map(cfg).chain((0..3).map(big))) {
        for (p, t) in map.iter() {
            let edge = p.x < c.water_border
                || p.y < c.water_border
                || p.x >= c.width - c.water_border
                || p.y >= c.height - c.water_border;
            if edge {
                assert_eq!(t.terrain, Terrain::Water, "seed {} at {p:?}", c.seed);
            }
        }
    }
}

#[test]
fn land_fraction_matches_config() {
    // `land_fraction` sets sea level; landlocked basins below it then become land or lakes
    // (#410), so the share that isn't sea is the fraction plus a little.
    for land_fraction in [0.3, 0.5, 0.7] {
        for (c, map) in maps((0..4).map(|s| MapGenConfig { land_fraction, ..big(s) })) {
            let land = map.iter().filter(|(_, t)| t.terrain != Terrain::Water).count();
            let actual = land as f64 / (c.width * c.height) as f64;
            assert!(
                (land_fraction - 0.005..land_fraction + 0.04).contains(&actual),
                "seed {}: wanted {land_fraction} land, got {actual:.3}",
                c.seed
            );
        }
    }
}

#[test]
fn land_next_to_sea_or_lake_is_coast() {
    for (c, map) in maps((0..10).map(cfg).chain((0..3).map(big))) {
        for (p, t) in map.iter() {
            if t.terrain == Terrain::Land {
                assert!(
                    map.neighbors(p, true).all(|q| !map[q].terrain.has_shore()),
                    "seed {}: inland tile {p:?} touches open water",
                    c.seed
                );
            }
        }
    }
}

#[test]
fn mountains_are_a_minority_of_land() {
    for (c, map) in maps((0..4).map(big)) {
        let land = map.iter().filter(|(_, t)| t.terrain != Terrain::Water).count() as f64;
        let mountains = map.iter().filter(|(_, t)| t.terrain == Terrain::Mountain).count() as f64;
        // Rivers and lakes may replace a few of the `mountain_fraction` tiles.
        let share = mountains / land;
        assert!(share > c.mountain_fraction * 0.7 && share <= c.mountain_fraction + 0.001);
    }
}

#[test]
fn biomes_follow_the_climate_table() {
    let defs = Defs::bundled();
    for (c, map) in maps((0..4).map(big)) {
        for (p, t) in map.iter() {
            assert_eq!(
                t.biome,
                defs.classify_biome(t.temperature, t.moisture),
                "seed {} at {p:?}",
                c.seed
            );
        }
    }
}

#[test]
fn poles_are_cold_and_the_equator_is_warm() {
    let defs = Defs::bundled();
    let tundra = defs.biome_id("tundra").unwrap();
    for (c, map) in maps((0..4).map(big)) {
        let row_mean = |y: i32| {
            (0..c.width).map(|x| f64::from(map[Pos::new(x, y)].temperature)).sum::<f64>()
                / f64::from(c.width)
        };
        assert!(row_mean(3) < row_mean(c.height / 2) - 100.0, "seed {}", c.seed);
        assert!(row_mean(c.height - 4) < row_mean(c.height / 2) - 100.0, "seed {}", c.seed);
        // No tundra in the tropics, at least at sea level.
        let mid = c.height / 2;
        for x in 0..c.width {
            let t = map[Pos::new(x, mid)];
            if t.terrain != Terrain::Mountain {
                assert_ne!(t.biome, tundra, "seed {}: tundra at the equator", c.seed);
            }
        }
    }
}

/// Tiles reachable from open water (the sea, or a lake: a landlocked basin lake is a sink like
/// the sea, #410) through rivers, fords and lakes (8-connected).
fn drains_to_sea(map: &TileMap) -> Grid<bool> {
    let mut seen = Grid::from_fn(map.width(), map.height(), |p| {
        matches!(map[p].terrain, Terrain::Water | Terrain::Lake)
    });
    let mut queue: VecDeque<Pos> = map.positions().filter(|&p| seen[p]).collect();
    while let Some(p) = queue.pop_front() {
        for q in map.neighbors(p, true) {
            if !seen[q] && matches!(map[q].terrain, Terrain::River | Terrain::Lake | Terrain::Ford)
            {
                seen[q] = true;
                queue.push_back(q);
            }
        }
    }
    seen
}

#[test]
fn rivers_reach_the_sea() {
    let mut rivers = 0;
    for (c, map) in maps((0..6).map(big)) {
        let drained = drains_to_sea(&map);
        for (p, t) in map.iter() {
            if t.terrain == Terrain::River {
                rivers += 1;
                assert!(drained[p], "seed {}: river at {p:?} doesn't reach the sea", c.seed);
            }
        }
    }
    assert!(rivers > 100, "expected rivers on full-size maps, got {rivers} tiles");
}

/// The map sizes the New game flow offers (`data/embark.toml`).
fn embark_sizes(seeds: std::ops::Range<u64>) -> Vec<(MapGenConfig, TileMap)> {
    let defs = Defs::bundled();
    let cfgs: Vec<MapGenConfig> = defs
        .embark
        .map_sizes
        .iter()
        .flat_map(|size| seeds.clone().map(move |s| fn_sim::embark::Survey::config(s, size)))
        .collect();
    maps(cfgs.into_iter())
}

/// 8-connected components of tiles `member` accepts, as their sizes.
fn component_sizes(map: &TileMap, member: impl Fn(Terrain) -> bool) -> Vec<usize> {
    let mut seen = Grid::from_fn(map.width(), map.height(), |_| false);
    let mut sizes = Vec::new();
    for start in map.positions() {
        if seen[start] || !member(map[start].terrain) {
            continue;
        }
        seen[start] = true;
        let mut queue = VecDeque::from([start]);
        let mut n = 0;
        while let Some(p) = queue.pop_front() {
            n += 1;
            for q in map.neighbors(p, true) {
                if !seen[q] && member(map[q].terrain) {
                    seen[q] = true;
                    queue.push_back(q);
                }
            }
        }
        sizes.push(n);
    }
    sizes
}

/// Sea is only sea connected to the map border (#410): landlocked basins became lakes or land.
#[test]
fn all_sea_reaches_the_border() {
    for (c, map) in embark_sizes(0..3) {
        let seas = component_sizes(&map, |t| t == Terrain::Water);
        assert_eq!(seas.len(), 1, "seed {} {}x{}: sea bodies {seas:?}", c.seed, c.width, c.height);
    }
}

#[test]
fn no_lake_is_smaller_than_the_minimum() {
    for (c, map) in embark_sizes(0..3) {
        let lakes = component_sizes(&map, |t| t == Terrain::Lake);
        assert!(!lakes.is_empty(), "seed {}: no lakes at all", c.seed);
        assert!(
            lakes.iter().all(|&n| n >= c.min_lake_tiles),
            "seed {} {}x{}: lakes {lakes:?}",
            c.seed,
            c.width,
            c.height
        );
    }
}

/// #410's target: at most a fifth of the lake bodies a default map had before it. Seeds 0-19
/// averaged 39.0 lakes per map then (34.7 of them 12 tiles or fewer).
#[test]
fn default_maps_have_few_lakes() {
    const BEFORE_410: f64 = 39.0;
    let all = maps((0..20).map(|seed| MapGenConfig { seed, ..Default::default() }));
    let lakes: usize =
        all.iter().map(|(_, m)| component_sizes(m, |t| t == Terrain::Lake).len()).sum();
    let mean = lakes as f64 / all.len() as f64;
    assert!(mean <= BEFORE_410 / 5.0, "{mean} lakes per map");
}

#[test]
fn rivers_and_lakes_are_not_walkable() {
    for t in [Terrain::Water, Terrain::River, Terrain::Lake, Terrain::Mountain] {
        assert!(!t.is_walkable(), "{t:?}");
    }
    assert!(Terrain::River.is_water() && Terrain::Lake.is_water());
    // A ford is the exception: shallow water you can wade.
    assert!(Terrain::Ford.is_walkable() && Terrain::Ford.is_water());
}

/// Labels 4-connected regions of tiles for which `member` holds. 0 means "not a member".
fn regions(map: &TileMap, member: impl Fn(Terrain) -> bool) -> Grid<usize> {
    let mut label = Grid::from_fn(map.width(), map.height(), |_| 0);
    let mut count = 0;
    for start in map.positions() {
        if label[start] != 0 || !member(map[start].terrain) {
            continue;
        }
        count += 1;
        label[start] = count;
        let mut queue = VecDeque::from([start]);
        while let Some(p) = queue.pop_front() {
            for q in map.neighbors(p, false) {
                if label[q] == 0 && member(map[q].terrain) {
                    label[q] = count;
                    queue.push_back(q);
                }
            }
        }
    }
    label
}

#[test]
fn rivers_do_not_disconnect_land() {
    // A landmass is land held together by rivers and fords too (not lakes or sea); within it,
    // every walkable tile must reach every other on foot, 4-connected (the strictest step rule).
    let mut fords = 0;
    for (c, map) in
        maps((0..8).map(big).chain((0..3).map(|s| MapGenConfig { seed: s, ..Default::default() })))
    {
        let landmass = regions(&map, |t| {
            matches!(t, Terrain::Land | Terrain::Coast | Terrain::Ford | Terrain::River)
        });
        let walkable = regions(&map, Terrain::is_walkable);
        let mut walk_region_of: std::collections::HashMap<usize, usize> = Default::default();
        for p in map.positions().filter(|&p| walkable[p] != 0) {
            let first = *walk_region_of.entry(landmass[p]).or_insert(walkable[p]);
            assert_eq!(first, walkable[p], "seed {}: {p:?} is cut off by a river", c.seed);
        }
        fords += map.iter().filter(|(_, t)| t.terrain == Terrain::Ford).count();
    }
    assert!(fords > 0, "expected some fords on full-size maps");
}

#[test]
fn fords_are_single_river_tiles_and_stay_connected_to_the_sea() {
    for (c, map) in maps((0..6).map(big)) {
        let drained = drains_to_sea(&map);
        for (p, t) in map.iter() {
            if t.terrain == Terrain::Ford {
                assert!(t.resource.is_none(), "seed {}: resource on ford {p:?}", c.seed);
                assert!(drained[p], "seed {}: ford at {p:?} doesn't reach the sea", c.seed);
            }
        }
    }
}

#[test]
fn resources_respect_their_rules() {
    let defs = Defs::bundled();
    for (_, map) in maps((0..10).map(cfg).chain((0..3).map(big))) {
        for (p, t) in map.iter() {
            if let Some(r) = t.resource {
                let def = defs.resource(r);
                if let Some(ts) = &def.terrains {
                    assert!(ts.contains(&t.terrain), "{} on {:?}", def.id, t.terrain);
                }
                if let Some(ts) = &def.beside {
                    assert!(
                        map.neighbors(p, true).any(|q| ts.contains(&map[q].terrain)),
                        "{} at {p:?} isn't beside {ts:?}",
                        def.id
                    );
                }
                if let Some(bs) = &def.biomes {
                    let biome = &defs.biome(t.biome).id;
                    assert!(bs.contains(biome), "{} in {biome}", def.id);
                }
            }
        }
    }
}

/// Renewable food (#56) exists on real maps: fishing spots in the water, fruit trees on land.
#[test]
fn renewable_food_sources_appear_on_generated_maps() {
    let defs = Defs::bundled();
    let (fish, fruit) =
        (defs.resource_id("fish_spot").unwrap(), defs.resource_id("fruit_tree").unwrap());
    for (c, map) in maps((0..3).map(big)) {
        let count = |id| map.iter().filter(|(_, t)| t.resource == Some(id)).count();
        assert!(count(fish) >= 10, "seed {}: {} fishing spots", c.seed, count(fish));
        assert!(count(fruit) >= 5, "seed {}: {} fruit trees", c.seed, count(fruit));
    }
}

/// Ancient scrolls (#24) are rare: about 3 to 6 on a default-size (192x128) map, so on average
/// a handful, never none or a crowd. Averaged over seeds because each map is one roll of the dice.
#[test]
fn ancient_scrolls_are_rare_finds() {
    let defs = Defs::bundled();
    let scroll = defs.resource_id("ancient_scroll").unwrap();
    let counts: Vec<usize> = (1..=12)
        .map(|seed| {
            let c = MapGenConfig { seed, width: 192, height: 128, ..Default::default() };
            let map = mapgen::generate(&c, &defs);
            map.iter().filter(|(_, t)| t.resource == Some(scroll)).count()
        })
        .collect();
    let mean = counts.iter().sum::<usize>() as f64 / counts.len() as f64;
    assert!((3.0..=6.0).contains(&mean), "mean {mean} scrolls per map: {counts:?}");
    assert!(counts.iter().all(|&n| n <= 10), "{counts:?}");
    assert!(counts.iter().filter(|&&n| n >= 1).count() >= 10, "{counts:?}");
}

#[test]
fn spaced_resources_keep_their_distance() {
    let defs = Defs::bundled();
    let spaced: Vec<_> = defs
        .resources
        .iter()
        .enumerate()
        .filter_map(|(i, r)| r.min_spacing.map(|s| (i, s as i32)))
        .collect();
    assert!(!spaced.is_empty(), "test needs a resource with min_spacing");
    for (c, map) in maps((0..3).map(big)) {
        for (i, spacing) in &spaced {
            let at: Vec<Pos> = map
                .iter()
                .filter(|(_, t)| t.resource.is_some_and(|r| r.0 as usize == *i))
                .map(|(p, _)| p)
                .collect();
            assert!(!at.is_empty());
            for (k, a) in at.iter().enumerate() {
                for b in &at[k + 1..] {
                    let d = (a.x - b.x).abs().max((a.y - b.y).abs());
                    assert!(d >= *spacing, "seed {}: {a:?} and {b:?} too close", c.seed);
                }
            }
        }
    }
}

#[test]
fn trees_grow_in_groves() {
    // With clustering, a tree's neighbors are much likelier to be trees than a random eligible
    // tile is: forests have clearings instead of uniform speckle.
    let defs = Defs::bundled();
    let fir = defs.resource_id("fir_tree").unwrap();
    let def = defs.resource(fir);
    let biomes: Vec<_> = def.biomes.iter().flatten().map(|b| defs.biome_id(b).unwrap()).collect();
    for (c, map) in maps((0..3).map(big)) {
        let eligible: Vec<Pos> = map
            .iter()
            .filter(|(_, t)| t.terrain == Terrain::Land && biomes.contains(&t.biome))
            .map(|(p, _)| p)
            .collect();
        let is_fir = |p: Pos| map[p].resource == Some(fir);
        let base = eligible.iter().filter(|&&p| is_fir(p)).count() as f64 / eligible.len() as f64;
        let (mut pairs, mut both) = (0, 0);
        for &p in eligible.iter().filter(|&&p| is_fir(p)) {
            for q in map.neighbors(p, false).filter(|q| eligible.contains(q)) {
                pairs += 1;
                both += usize::from(is_fir(q));
            }
        }
        let neighbor = both as f64 / pairs as f64;
        assert!(neighbor > base * 1.2, "seed {}: {neighbor:.3} vs base {base:.3}", c.seed);
    }
}

fn golden(name: &str, c: &MapGenConfig) {
    // FN_GOLDEN=skip (cargo xtask ci --fast --no-goldens): every golden comparison is a no-op
    // that passes, so agents can run the gate without caring whether goldens are stale.
    if std::env::var("FN_GOLDEN").ok().as_deref() == Some("skip") {
        return;
    }
    let defs = Defs::bundled();
    let actual = mapgen::to_ascii(&mapgen::generate(c, &defs), &defs);
    let path = format!("{}/tests/golden/{name}", env!("CARGO_MANIFEST_DIR"));
    if std::env::var_os("FN_BLESS").is_some() {
        std::fs::write(&path, &actual).unwrap();
    }
    let expected = std::fs::read_to_string(&path).expect("golden missing; run with FN_BLESS=1");
    assert_eq!(actual, expected, "map generation output changed; see module docs");
}

#[test]
fn golden_ascii_seed_1() {
    golden("seed1_48x24.txt", &cfg(1));
}

#[test]
fn golden_ascii_seed_2_large() {
    golden("seed2_120x50.txt", &MapGenConfig { seed: 2, width: 120, height: 50, ..cfg(2) });
}

// ---- Per-resource RNG streams (#225) ----

/// A test-only resource that grows on coast tiles, which no bundled resource uses, so it never
/// competes for a tile with the originals: any change to them could only come from shared RNG.
fn coast_weed() -> ResourceDef {
    ResourceDef {
        id: "coast_weed".into(),
        name: "Coast Weed".into(),
        glyph: 'w',
        chance: 0.4,
        terrains: Some(vec![Terrain::Coast]),
        biomes: None,
        clustering: 0.5,
        moisture_scaled: false,
        min_spacing: None,
        beside: None,

        buried: false,
    }
}

/// Where each resource ended up, keyed by id and sorted so list order doesn't matter.
fn placements(defs: &Defs, c: &MapGenConfig) -> Vec<(String, Vec<Pos>)> {
    let map = mapgen::generate(c, defs);
    let mut out: Vec<(String, Vec<Pos>)> =
        defs.resources.iter().map(|r| (r.id.clone(), Vec::new())).collect();
    for (p, t) in map.iter() {
        if let Some(r) = t.resource {
            out[r.0 as usize].1.push(p);
        }
    }
    out.sort();
    out
}

fn without(mut p: Vec<(String, Vec<Pos>)>, id: &str) -> Vec<(String, Vec<Pos>)> {
    p.retain(|(i, _)| i != id);
    p
}

fn position(defs: &Defs, id: &str) -> usize {
    defs.resources.iter().position(|r| r.id == id).unwrap()
}

#[test]
fn adding_a_resource_does_not_move_the_others() {
    let base = Defs::bundled();
    let mut appended = base.clone();
    appended.resources.push(coast_weed());
    let mut prepended = base.clone();
    prepended.resources.insert(0, coast_weed());
    for seed in [1, 2, 3, 7, 42] {
        let c = big(seed);
        let before = placements(&base, &c);
        assert!(before.iter().filter(|(_, v)| !v.is_empty()).count() >= 5, "seed {seed}");
        let after = placements(&appended, &c);
        let weeds = after.iter().find(|(id, _)| id == "coast_weed").unwrap();
        assert!(!weeds.1.is_empty(), "seed {seed}: the test resource must actually place");
        assert_eq!(before, without(after, "coast_weed"), "seed {seed}: appended");
        let front = without(placements(&prepended, &c), "coast_weed");
        assert_eq!(before, front, "seed {seed}: inserted at the front");
    }
}

#[test]
fn removing_or_reordering_resources_does_not_move_the_others() {
    let mut base = Defs::bundled();
    base.resources.push(coast_weed());
    // Oaks and firs grow in disjoint biomes, and fish spots in water, so swapping them or
    // moving them to the front changes priority only where nothing overlaps.
    let mut swapped = base.clone();
    let (oak, fir) = (position(&base, "oak_tree"), position(&base, "fir_tree"));
    swapped.resources.swap(oak, fir);
    let mut fish_first = base.clone();
    let fish = fish_first.resources.remove(position(&base, "fish_spot"));
    fish_first.resources.insert(0, fish);
    let mut no_fish = base.clone();
    no_fish.resources.remove(position(&base, "fish_spot"));
    for seed in [1, 2, 3, 7, 42] {
        let c = big(seed);
        let before = placements(&base, &c);
        assert_eq!(before, placements(&swapped, &c), "seed {seed}: oak and fir swapped");
        assert_eq!(before, placements(&fish_first, &c), "seed {seed}: fish first");
        let rest = without(before, "fish_spot");
        assert_eq!(rest, placements(&no_fish, &c), "seed {seed}: fish removed");
    }
}

#[test]
fn list_order_is_priority_where_resources_overlap() {
    // Gold is listed before stone and both want mountains. Swapping them hands the contested
    // tiles to stone, but gold never gains a tile it did not have before.
    let base = Defs::bundled();
    let mut flipped = base.clone();
    flipped.resources.swap(position(&base, "gold_vein"), position(&base, "stone"));
    let c = big(1);
    let get = |d: &Defs| placements(d, &c).into_iter().find(|(i, _)| i == "gold_vein").unwrap().1;
    let (before, after) = (get(&base), get(&flipped));
    assert!(!before.is_empty() && after.len() < before.len(), "stone should win some tiles");
    assert!(after.iter().all(|p| before.contains(p)), "no new gold tiles appear");
}
