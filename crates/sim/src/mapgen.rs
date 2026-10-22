//! Procedural map generation.
//!
//! A port of the ClojureScript generator (`src/four_nations/model/map/*.cljs` on the `cljs`
//! branch), pass for pass:
//!
//! 1. random noise in `1..=255`, smoothed by repeatedly averaging each cell's neighbors;
//! 2. cells below the mean become water, the rest land (new: the highest cells become mountain);
//! 3. biomes are region-grown from random seed points;
//! 4. a water border is added, water randomly spreads into adjacent land, land fully surrounded
//!    by water is drowned, and land touching water becomes coast;
//! 5. resources are rolled per tile.
//!
//! Deliberate deviation: the cljs `subdivide-map` stopped one iteration early (`take-while` +
//! `last`), leaving some tiles without a biome. Here every tile always gets one.

use rand::RngExt;
use rand::seq::SliceRandom;

use crate::defs::{BiomeId, Defs, ResourceId, Terrain};
use crate::geom::{Grid, Pos};
use crate::rng;

#[derive(Debug, Clone, PartialEq)]
pub struct MapGenConfig {
    pub width: i32,
    pub height: i32,
    pub seed: u64,
    pub smoothing_passes: u32,
    /// Fraction of cells (by elevation rank) that become mountain, e.g. `0.06` = top 6%.
    pub mountain_fraction: f64,
    pub water_border: i32,
    pub water_spread_chance: f64,
    /// Number of biome seed points per biome definition.
    pub biome_seeds_per_biome: u32,
}

impl Default for MapGenConfig {
    fn default() -> Self {
        Self {
            width: 64,
            height: 48,
            seed: 0,
            smoothing_passes: 15,
            mountain_fraction: 0.06,
            water_border: 2,
            water_spread_chance: 0.1,
            biome_seeds_per_biome: 5,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Tile {
    /// Smoothed noise value, a rough "elevation" in `1..=255`.
    pub elevation: u8,
    pub terrain: Terrain,
    pub biome: BiomeId,
    pub resource: Option<ResourceId>,
}

pub type TileMap = Grid<Tile>;

pub fn generate(cfg: &MapGenConfig, defs: &Defs) -> TileMap {
    let elevation = smoothed_noise(cfg);
    let terrain = base_terrain(&elevation, cfg.mountain_fraction);
    let biomes = assign_biomes(cfg, defs.biomes.len());

    let mut terrain = add_water_border(terrain, cfg.water_border);
    terrain = spread_water(&terrain, cfg);
    terrain = drown_surrounded_land(&terrain);
    terrain = add_coastline(&terrain);

    let mut map = Grid::from_fn(cfg.width, cfg.height, |p| Tile {
        elevation: elevation[p],
        terrain: terrain[p],
        biome: biomes[p],
        resource: None,
    });
    add_resources(&mut map, cfg, defs);
    map
}

fn smoothed_noise(cfg: &MapGenConfig) -> Grid<u8> {
    let mut rng = rng::stream(cfg.seed, "mapgen.noise");
    let mut g: Grid<u8> = Grid::from_fn(cfg.width, cfg.height, |_| rng.random_range(1..=255));
    for _ in 0..cfg.smoothing_passes {
        g = g.map_with(|p, _| {
            let (sum, n) =
                g.neighbors(p, true).fold((0u32, 0u32), |(s, n), q| (s + u32::from(g[q]), n + 1));
            (sum / n) as u8
        });
    }
    g
}

fn base_terrain(elevation: &Grid<u8>, mountain_fraction: f64) -> Grid<Terrain> {
    let mut values: Vec<u8> = elevation.iter().map(|(_, v)| *v).collect();
    let mean = values.iter().map(|v| u32::from(*v)).sum::<u32>() / values.len() as u32;
    values.sort_unstable();
    let cutoff_index = ((values.len() as f64) * (1.0 - mountain_fraction)) as usize;
    let mountain_cutoff = values.get(cutoff_index).copied().unwrap_or(u8::MAX);

    elevation.map_with(|_, &v| {
        if u32::from(v) < mean {
            Terrain::Water
        } else if mountain_fraction > 0.0 && v >= mountain_cutoff {
            Terrain::Mountain
        } else {
            Terrain::Land
        }
    })
}

fn assign_biomes(cfg: &MapGenConfig, biome_count: usize) -> Grid<BiomeId> {
    let mut rng = rng::stream(cfg.seed, "mapgen.biomes");
    let mut groups: Grid<Option<BiomeId>> = Grid::from_fn(cfg.width, cfg.height, |_| None);
    let seeds = (cfg.biome_seeds_per_biome as usize * biome_count).max(1);
    for _ in 0..seeds {
        let p = Pos::new(rng.random_range(0..cfg.width), rng.random_range(0..cfg.height));
        groups[p] = Some(BiomeId(rng.random_range(0..biome_count) as u8));
    }

    let mut order: Vec<Pos> = groups.positions().collect();
    while groups.iter().any(|(_, g)| g.is_none()) {
        // Like the cljs version, each pass reads the previous pass's grid, so growth is uniform.
        order.shuffle(&mut rng);
        let prev = groups.clone();
        for &p in &order {
            if prev[p].is_some() {
                continue;
            }
            let candidates: Vec<BiomeId> =
                prev.neighbors(p, true).filter_map(|q| prev[q]).collect();
            if !candidates.is_empty() {
                groups[p] = Some(candidates[rng.random_range(0..candidates.len())]);
            }
        }
    }
    groups.map_with(|_, g| g.expect("loop exits only when fully assigned"))
}

fn add_water_border(mut terrain: Grid<Terrain>, border: i32) -> Grid<Terrain> {
    let (w, h) = (terrain.width(), terrain.height());
    for p in terrain.positions() {
        if p.x < border || p.y < border || p.x >= w - border || p.y >= h - border {
            terrain[p] = Terrain::Water;
        }
    }
    terrain
}

fn touches_water(terrain: &Grid<Terrain>, p: Pos) -> bool {
    terrain.neighbors(p, true).any(|q| terrain[q].is_water())
}

fn spread_water(terrain: &Grid<Terrain>, cfg: &MapGenConfig) -> Grid<Terrain> {
    let mut rng = rng::stream(cfg.seed, "mapgen.spread_water");
    terrain.map_with(|p, &t| {
        // Roll for every cell, not just eligible ones, so the stream is position-stable.
        let roll: f64 = rng.random();
        if t == Terrain::Land && touches_water(terrain, p) && roll < cfg.water_spread_chance {
            Terrain::Water
        } else {
            t
        }
    })
}

fn drown_surrounded_land(terrain: &Grid<Terrain>) -> Grid<Terrain> {
    terrain.map_with(|p, &t| {
        if !t.is_water() && terrain.neighbors(p, true).all(|q| terrain[q].is_water()) {
            Terrain::Water
        } else {
            t
        }
    })
}

fn add_coastline(terrain: &Grid<Terrain>) -> Grid<Terrain> {
    terrain.map_with(|p, &t| {
        if t == Terrain::Land && touches_water(terrain, p) { Terrain::Coast } else { t }
    })
}

fn add_resources(map: &mut TileMap, cfg: &MapGenConfig, defs: &Defs) {
    let mut rng = rng::stream(cfg.seed, "mapgen.resources");
    let biome_filters: Vec<Option<Vec<BiomeId>>> = defs
        .resources
        .iter()
        .map(|r| {
            r.biomes
                .as_ref()
                .map(|bs| bs.iter().map(|b| defs.biome_id(b).expect("validated")).collect())
        })
        .collect();

    for p in map.positions() {
        let tile = map[p];
        for (i, r) in defs.resources.iter().enumerate() {
            let roll: f64 = rng.random();
            let terrain_ok = r.terrains.as_ref().is_none_or(|ts| ts.contains(&tile.terrain));
            let biome_ok = biome_filters[i].as_ref().is_none_or(|bs| bs.contains(&tile.biome));
            if terrain_ok && biome_ok && roll < r.chance {
                map[p].resource = Some(ResourceId(i as u16));
                break;
            }
        }
    }
}

/// Renders the map as text: one glyph per tile, rows separated by newlines.
pub fn to_ascii(map: &TileMap, defs: &Defs) -> String {
    let mut out = String::with_capacity(((map.width() + 1) * map.height()) as usize);
    for (p, t) in map.iter() {
        let glyph = match (t.resource, t.terrain) {
            (Some(r), _) => defs.resource(r).glyph,
            (None, Terrain::Water) => '~',
            (None, Terrain::Coast) => '.',
            (None, Terrain::Mountain) => '^',
            (None, Terrain::Land) => defs.biome(t.biome).glyph,
        };
        out.push(glyph);
        if p.x == map.width() - 1 {
            out.push('\n');
        }
    }
    out
}
