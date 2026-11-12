//! Procedural map generation (v2, issue #16).
//!
//! The approach follows Red Blob Games' "Making maps with noise functions": separate
//! continuous fields for elevation, temperature and moisture, with terrain and biomes derived
//! from them rather than painted at random. Passes, each with its own RNG stream:
//!
//! 1. **Elevation** (`mapgen.elevation`): fractal noise (fBm), domain-warped for ragged coasts,
//!    plus ridged noise for mountain *ranges*, pulled down toward the map edges by a falloff
//!    mask so land never touches the border.
//! 2. **Sea level** by percentile, so `land_fraction` of the map is above it whatever the noise
//!    did. Only below-sea-level tiles connected to the map border stay sea (issue #410):
//!    landlocked basins of at least `min_lake_tiles` become lakes and smaller ones become land,
//!    so continents aren't pocked with sand-ringed puddles of "sea". Mountains are the highest
//!    `mountain_fraction` of land.
//! 3. **Climate** (`mapgen.climate`): temperature from latitude (poles at the top and bottom),
//!    minus altitude; moisture from noise plus nearness to water.
//! 4. **Hydrology** (see `hydrology`): fill depressions into lakes (dropping any smaller than
//!    `min_lake_tiles`), route rain downhill, and turn high-flow tiles into rivers. Moisture is then recomputed so rivers green their
//!    valleys.
//!    **River crossings** (see `crossings`, issue #30): rivers are not walkable, so the
//!    narrowest river tiles that would otherwise split a landmass become `Terrain::Ford`.
//!    Chosen over bridges-as-a-build-job (the start of the game would be unplayable on a split
//!    continent) and slow-river movement cost (rivers should stay a real obstacle).
//! 5. **Biomes** from `data/biomes.toml`'s temperature x moisture table.
//! 6. **Resources** (`mapgen.resources.<id>`): per-tile rolls whose chance follows a per-resource
//!    noise field (clumped groves), moisture, and an optional minimum spacing. Every resource has
//!    its own streams keyed by its id, so content changes don't reshuffle other resources
//!    (issue #225, see `add_resources`).
//! 7. **Promotion** (`mapgen.great_ruins`, #736): a resource with `[resource.promote]` takes over
//!    a few of another resource's tiles, picked by `pick_promoted`. It only rewrites tiles that
//!    already hold the `from` resource, so nothing else on the map moves.
//!
//! All arithmetic is plain `f64` `+ - * /` and `floor` (no `powf`/`exp`/FMA), so maps are
//! identical on every platform. See `noise` for why the noise is in-house.

mod crossings;
mod hydrology;

use std::collections::VecDeque;

use rand::RngExt;
use rand::seq::SliceRandom;

use crate::building::TileBuilding;
use crate::defs::{BiomeId, Defs, PromoteDef, ResourceDef, ResourceId, Terrain};
use crate::geom::{Grid, Pos};
use crate::noise::{Fbm, lerp, smoothstep};
use crate::rng;

#[derive(Debug, Clone, PartialEq)]
pub struct MapGenConfig {
    pub width: i32,
    pub height: i32,
    pub seed: u64,
    /// Fraction of the map above sea level. Landlocked basins below it become land or lakes
    /// (issue #410), so the share that isn't sea (`Terrain::Water`) comes out a little higher:
    /// up to about 3 points on the default map.
    pub land_fraction: f64,
    /// Fraction of non-sea tiles (by elevation rank) that become mountain.
    pub mountain_fraction: f64,
    /// Roughly how many continent-sized noise features fit across the map's long side.
    pub continent_frequency: f64,
    /// How far (in noise units) coordinates are displaced before sampling elevation. Higher
    /// values give more ragged, organic coastlines.
    pub warp_strength: f64,
    /// Weight of ridged noise (mountain ranges) added to inland elevation.
    pub ridge_strength: f64,
    /// How strongly elevation is lowered toward the map edges (0 = not at all).
    pub edge_falloff: f64,
    /// Rings of tiles at the map edge that are always sea.
    pub water_border: i32,
    /// Rain (summed moisture of upstream tiles) needed to form a river.
    pub river_threshold: f64,
    /// Minimum depth (in elevation units, sea-to-peak is roughly 1) of a filled depression for
    /// it to become a lake.
    pub lake_depth: f64,
    /// Smallest lake kept, in 8-connected tiles: smaller filled depressions and landlocked sea
    /// basins become land (issue #410, settled on #407).
    pub min_lake_tiles: usize,
}

impl Default for MapGenConfig {
    fn default() -> Self {
        Self {
            width: 192,
            height: 128,
            seed: 0,
            land_fraction: 0.5,
            mountain_fraction: 0.08,
            continent_frequency: 5.0,
            warp_strength: 0.35,
            ridge_strength: 0.6,
            edge_falloff: 0.6,
            water_border: 2,
            river_threshold: 45.0,
            lake_depth: 0.02,
            min_lake_tiles: 16,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Tile {
    /// Elevation rescaled to `0..=255` over the map (lowest sea floor to highest peak).
    pub elevation: u8,
    /// Quantized temperature, `0..=255` (see `defs::climate_unit`). 0 is polar.
    pub temperature: u8,
    /// Quantized moisture, `0..=255`. 0 is arid.
    pub moisture: u8,
    pub terrain: Terrain,
    pub biome: BiomeId,
    pub resource: Option<ResourceId>,
    /// The finished building on this tile (`Command::DesignateBuild`, issue #64), if any: its
    /// record's key and what pathfinding needs (`World::buildings`, #604).
    pub building: Option<TileBuilding>,
}

pub type TileMap = Grid<Tile>;

pub fn generate(cfg: &MapGenConfig, defs: &Defs) -> TileMap {
    let elevation = elevation(cfg);
    let (below_sea, sea_level) = sea_by_percentile(&elevation, cfg);
    let (sea, basins) = split_landlocked(&below_sea, cfg.min_lake_tiles);
    // Open water of either kind: what mountains are ranked against and rain is measured from.
    let water = sea.map_with(|p, &s| s || basins[p]);
    let mountain = mountains(&elevation, &water, cfg.mountain_fraction);

    let mut climate_rng = rng::stream(cfg.seed, "mapgen.climate");
    let moisture_noise = rank_normalize(&noise_field(cfg, &Fbm::new(&mut climate_rng, 4), 2.0));
    let temperature_noise = noise_field(cfg, &Fbm::new(&mut climate_rng, 3), 1.5);

    // Rain for river routing comes from noise and distance to the sea; the final moisture also
    // counts rivers and lakes.
    let rain = moisture(&moisture_noise, &water);
    let hydro = hydrology::rivers_and_lakes(
        &elevation,
        &sea,
        &basins,
        &rain,
        &hydrology::Params {
            river_threshold: cfg.river_threshold,
            lake_depth: cfg.lake_depth,
            min_lake_tiles: cfg.min_lake_tiles,
        },
    );
    let terrain = elevation.map_with(|p, _| {
        if sea[p] {
            Terrain::Water
        } else if hydro.lake[p] {
            Terrain::Lake
        } else if hydro.river[p] {
            Terrain::River
        } else if mountain[p] {
            Terrain::Mountain
        } else {
            Terrain::Land
        }
    });
    let mut terrain = add_coastline(&terrain);
    crossings::add_fords(&mut terrain, &hydro.flow);
    let fresh_or_sea = terrain.map_with(|_, t| t.is_water());
    let moisture = moisture(&moisture_noise, &fresh_or_sea);
    let temperature = temperature(cfg, &elevation, sea_level, &temperature_noise);

    let (lo, hi) = min_max(&elevation);
    let mut map = Grid::from_fn(cfg.width, cfg.height, |p| {
        let (t, m) = (quantize(temperature[p]), quantize(moisture[p]));
        Tile {
            elevation: quantize((elevation[p] - lo) / (hi - lo).max(f64::MIN_POSITIVE)),
            temperature: t,
            moisture: m,
            terrain: terrain[p],
            biome: defs.classify_biome(t, m),
            resource: None,
            building: None,
        }
    });
    add_resources(&mut map, cfg, defs);
    promote_resources(&mut map, cfg.seed, defs);
    map
}

/// Promotes tiles for every resource with `[resource.promote]` (great ruins, #736): picks among
/// the `from` resource's tiles with `pick_promoted` and rewrites only those tiles' resource. The
/// pass's only write, so every other tile and resource stays exactly where `add_resources` put
/// it.
fn promote_resources(map: &mut TileMap, seed: u64, defs: &Defs) {
    for (i, r) in defs.resources.iter().enumerate() {
        let Some(promote) = &r.promote else { continue };
        let from = defs.resource_id(&promote.from).expect("validated");
        let candidates: Vec<Pos> =
            map.iter().filter(|(_, t)| t.resource == Some(from)).map(|(p, _)| p).collect();
        for p in pick_promoted(map, &candidates, promote, seed, &r.id) {
            map[p].resource = Some(ResourceId(i as u16));
        }
    }
}

/// Picks which of `candidates` (the `from` resource's tiles, in scan order) a promotion takes
/// (docs/design/horizons-phase-5.md, "What mapgen does"). Used by mapgen and by the old-save
/// upgrade (`World::place_great_ruins`), which passes the still-buried tiles instead.
///
/// 1. Shuffle the candidates with `mapgen.<id>`, then stable-sort by size (Land tiles in the box
///    of `size_radius` around each), largest first. The shuffle only breaks ties.
/// 2. Take them in that order, skipping one closer than `spacing` to one already taken, until
///    `count` are taken.
/// 3. Fewer than `min`? Fill from the skipped ones in the same order, ignoring spacing.
///
/// Returns the picked tiles in pick order.
pub fn pick_promoted(
    map: &TileMap,
    candidates: &[Pos],
    promote: &PromoteDef,
    seed: u64,
    id: &str,
) -> Vec<Pos> {
    let count = promote.count(map.width() as u32, map.height() as u32);
    let mut order: Vec<Pos> = candidates.to_vec();
    order.shuffle(&mut rng::stream(seed, &format!("mapgen.{id}")));
    let r = promote.size_radius as i32;
    let size = |p: Pos| -> usize {
        (-r..=r)
            .flat_map(|dy| (-r..=r).map(move |dx| Pos::new(p.x + dx, p.y + dy)))
            .filter(|&q| map.get(q).is_some_and(|t| t.terrain == Terrain::Land))
            .count()
    };
    let mut sized: Vec<(usize, Pos)> = order.into_iter().map(|p| (size(p), p)).collect();
    sized.sort_by_key(|&(s, _)| std::cmp::Reverse(s));
    let mut taken: Vec<Pos> = Vec::new();
    let mut skipped: Vec<Pos> = Vec::new();
    for &(_, p) in &sized {
        if taken.len() >= count {
            break;
        }
        if taken.iter().any(|&q| p.chebyshev(q) < promote.spacing as i32) {
            skipped.push(p);
        } else {
            taken.push(p);
        }
    }
    let min = promote.min as usize;
    for p in skipped {
        if taken.len() >= min.min(count) {
            break;
        }
        taken.push(p);
    }
    taken
}

fn quantize(v: f64) -> u8 {
    (v.clamp(0.0, 1.0) * 255.0).round() as u8
}

fn min_max(g: &Grid<f64>) -> (f64, f64) {
    g.iter().fold((f64::INFINITY, f64::NEG_INFINITY), |(lo, hi), (_, &v)| (lo.min(v), hi.max(v)))
}

/// Noise coordinates for tile `p` at `frequency` features across the long side.
fn noise_coords(cfg: &MapGenConfig, p: Pos, frequency: f64) -> (f64, f64) {
    let scale = frequency / f64::from(cfg.width.max(cfg.height));
    (f64::from(p.x) * scale, f64::from(p.y) * scale)
}

fn noise_field(cfg: &MapGenConfig, fbm: &Fbm, frequency: f64) -> Grid<f64> {
    Grid::from_fn(cfg.width, cfg.height, |p| {
        let (u, v) = noise_coords(cfg, p, frequency);
        fbm.get(u, v)
    })
}

/// Chebyshev distance from `p` to the nearest edge, in tiles.
fn edge_distance(cfg: &MapGenConfig, p: Pos) -> i32 {
    p.x.min(p.y).min(cfg.width - 1 - p.x).min(cfg.height - 1 - p.y)
}

/// Elevation of the deep sea that the edge falloff pulls toward. Below any noise value.
const DEEP: f64 = -1.5;

fn elevation(cfg: &MapGenConfig) -> Grid<f64> {
    /// Tiles over which elevation fades into `DEEP` inside the forced-sea border.
    const RIM: f64 = 3.0;
    let mut rng = rng::stream(cfg.seed, "mapgen.elevation");
    let base = Fbm::new(&mut rng, 6);
    let warp_x = Fbm::new(&mut rng, 3);
    let warp_y = Fbm::new(&mut rng, 3);
    let ridges = Fbm::new(&mut rng, 4);
    let span = |n: i32| f64::from((n - 1).max(1));

    Grid::from_fn(cfg.width, cfg.height, |p| {
        let d = edge_distance(cfg, p);
        if d < cfg.water_border {
            return DEEP;
        }
        let (u, v) = noise_coords(cfg, p, cfg.continent_frequency);
        // Domain warp at twice the base frequency: bends coastlines without moving continents.
        let (wu, wv) = (u * 2.0 + 5.2, v * 2.0 + 1.3);
        let (u, v) = (
            u + cfg.warp_strength * warp_x.get(wu, wv),
            v + cfg.warp_strength * warp_y.get(wu, wv),
        );
        let e = base.get(u, v);
        // Ridges only rise where the base is already land-ish, so ranges sit inside continents.
        // Sharpen the ridged noise so only its crests count.
        let crest = smoothstep(0.55, 0.95, ridges.ridged(u * 1.5, v * 1.5));
        let e = e + cfg.ridge_strength * crest * smoothstep(-0.05, 0.25, e);
        // "Square bump" falloff: 0 in the middle, 1 at the edges, so land thins out toward the
        // edges without a rectangular outline. Fourth powers keep the middle 60% nearly flat,
        // so the polar latitudes still have land.
        let nx = 2.0 * f64::from(p.x) / span(cfg.width) - 1.0;
        let ny = 2.0 * f64::from(p.y) / span(cfg.height) - 1.0;
        let (nx4, ny4) = (nx * nx * nx * nx, ny * ny * ny * ny);
        let e = e - cfg.edge_falloff * (1.0 - (1.0 - nx4) * (1.0 - ny4));
        // A short rim just inside the forced-sea border so land never ends in a straight cliff.
        lerp(DEEP, e, smoothstep(0.0, RIM, f64::from(d - cfg.water_border)))
    })
}

/// Marks the lowest `1 - land_fraction` of tiles (plus the border) as sea, and returns the
/// sea level: the lowest non-sea elevation.
fn sea_by_percentile(elevation: &Grid<f64>, cfg: &MapGenConfig) -> (Grid<bool>, f64) {
    let ranked = rank_order(elevation);
    let n = ranked.len();
    let sea_count = ((n as f64) * (1.0 - cfg.land_fraction.clamp(0.0, 1.0))).round() as usize;
    let mut sea = elevation.map_with(|_, _| false);
    for &p in &ranked[..sea_count] {
        sea[p] = true;
    }
    let sea_level = ranked.get(sea_count).map_or(f64::INFINITY, |&p| elevation[p]);
    (sea, sea_level)
}

/// Splits below-sea-level tiles into the sea proper (8-connected to the map border) and the
/// landlocked basins of at least `min_tiles` that become lakes. Smaller basins are in neither:
/// they become land.
fn split_landlocked(below_sea: &Grid<bool>, min_tiles: usize) -> (Grid<bool>, Grid<bool>) {
    let mut sea = below_sea.map_with(|_, _| false);
    let mut queue: VecDeque<Pos> = below_sea
        .positions()
        .filter(|&p| below_sea[p] && below_sea.neighbors(p, false).count() < 4)
        .collect();
    for &p in &queue {
        sea[p] = true;
    }
    while let Some(p) = queue.pop_front() {
        for q in below_sea.neighbors(p, true) {
            if below_sea[q] && !sea[q] {
                sea[q] = true;
                queue.push_back(q);
            }
        }
    }
    let landlocked = below_sea.map_with(|p, &b| b && !sea[p]);
    let basins = hydrology::without_small_components(&landlocked, min_tiles);
    (sea, basins)
}

/// Positions sorted by value ascending, ties broken by position.
fn rank_order(g: &Grid<f64>) -> Vec<Pos> {
    let mut v: Vec<Pos> = g.positions().collect();
    v.sort_by(|&a, &b| g[a].total_cmp(&g[b]).then(a.cmp(&b)));
    v
}

/// Replaces each value by its rank in `[0, 1]`: a uniform distribution with the same shape.
fn rank_normalize(g: &Grid<f64>) -> Grid<f64> {
    let ranked = rank_order(g);
    let denom = (ranked.len().max(2) - 1) as f64;
    let mut out = g.map_with(|_, _| 0.0);
    for (i, &p) in ranked.iter().enumerate() {
        out[p] = i as f64 / denom;
    }
    out
}

fn mountains(elevation: &Grid<f64>, sea: &Grid<bool>, fraction: f64) -> Grid<bool> {
    let mut land: Vec<Pos> = elevation.positions().filter(|&p| !sea[p]).collect();
    land.sort_by(|&a, &b| elevation[b].total_cmp(&elevation[a]).then(a.cmp(&b)));
    let count = ((land.len() as f64) * fraction.clamp(0.0, 1.0)).round() as usize;
    let mut out = elevation.map_with(|_, _| false);
    for &p in &land[..count] {
        out[p] = true;
    }
    out
}

/// Multi-source BFS: Chebyshev distance (8-connected steps) to the nearest `source` tile.
fn distance_to(source: &Grid<bool>) -> Grid<u32> {
    let mut dist = source.map_with(|_, &s| if s { 0 } else { u32::MAX });
    let mut queue: VecDeque<Pos> = source.iter().filter(|(_, s)| **s).map(|(p, _)| p).collect();
    while let Some(p) = queue.pop_front() {
        for q in source.neighbors(p, true) {
            if dist[q] == u32::MAX {
                dist[q] = dist[p] + 1;
                queue.push_back(q);
            }
        }
    }
    dist
}

/// Moisture in `[0, 1]`: 60% noise (uniform), 40% nearness to `water`.
fn moisture(noise: &Grid<f64>, water: &Grid<bool>) -> Grid<f64> {
    /// Distance, in tiles, at which water's influence halves.
    const HALF_DISTANCE: f64 = 6.0;
    let dist = distance_to(water);
    noise.map_with(|p, &n| {
        let near = if dist[p] == u32::MAX {
            0.0
        } else {
            1.0 / (1.0 + f64::from(dist[p]) / HALF_DISTANCE)
        };
        0.6 * n + 0.4 * near
    })
}

/// Temperature in `[0, 1]`: latitude (hot equator, frozen top and bottom), cooled by altitude
/// above sea level and nudged by noise so climate bands aren't ruler-straight.
fn temperature(
    cfg: &MapGenConfig,
    elevation: &Grid<f64>,
    sea_level: f64,
    noise: &Grid<f64>,
) -> Grid<f64> {
    let (_, peak) = min_max(elevation);
    let relief = (peak - sea_level).max(f64::MIN_POSITIVE);
    let half = f64::from(cfg.height - 1).max(1.0) / 2.0;
    elevation.map_with(|p, &e| {
        let latitude = 1.0 - (f64::from(p.y) - half).abs() / half;
        let altitude = ((e - sea_level) / relief).clamp(0.0, 1.0);
        (latitude + 0.1 * noise[p] - 0.35 * altitude).clamp(0.0, 1.0)
    })
}

fn add_coastline(terrain: &Grid<Terrain>) -> Grid<Terrain> {
    terrain.map_with(|p, &t| {
        let shore = terrain.neighbors(p, true).any(|q| terrain[q].has_shore());
        if t == Terrain::Land && shore { Terrain::Coast } else { t }
    })
}

/// Places resources. Each resource is decided on its own, then they are merged by priority.
///
/// **Per-resource streams (issue #225).** Resource `r` rolls from `mapgen.resources.<id>` and
/// draws its cluster noise from `mapgen.resource_clusters.<id>`, keyed by the resource's *id*,
/// never its position in `data/resources.toml`. It draws one roll per tile, always, so the stream
/// stays position-stable when eligibility changes. Adding, removing or reordering a resource
/// therefore leaves every other resource's candidate tiles exactly as they were (it can still
/// win or lose an overlapping tile, see below). New content should follow the same rule: any
/// new randomness for a resource goes in a stream named after its id.
///
/// Nothing draws from these streams afterwards: this is the last mapgen pass, and the world's
/// unit placement and later play use their own named streams (`world.*`, `bending.*`).
///
/// **Priority.** Two resources can both want a tile (a fir and a scroll in the tundra). File
/// order in `data/resources.toml` is priority: the first resource whose candidate set contains
/// the tile gets it. Spacing is checked against a resource's own candidates, not against the
/// tiles it finally holds, so a higher-priority resource taking a tile never frees a neighbour
/// for a lower one.
fn add_resources(map: &mut TileMap, cfg: &MapGenConfig, defs: &Defs) {
    for (i, r) in defs.resources.iter().enumerate() {
        let id = ResourceId(i as u16);
        for p in resource_candidates(map, cfg, defs, r) {
            if map[p].resource.is_none() {
                map[p].resource = Some(id);
            }
        }
    }
}

/// Every tile resource `r` would grow on if nothing else competed for it, in scan order.
fn resource_candidates(
    map: &TileMap,
    cfg: &MapGenConfig,
    defs: &Defs,
    r: &ResourceDef,
) -> Vec<Pos> {
    /// Grove/outcrop size: noise features per this many tiles, independent of map size.
    const CLUSTER_TILES: f64 = 14.0;
    let mut rng = rng::stream(cfg.seed, &format!("mapgen.resources.{}", r.id));
    let clusters = (r.clustering > 0.0).then(|| {
        let mut cluster_rng = rng::stream(cfg.seed, &format!("mapgen.resource_clusters.{}", r.id));
        Fbm::new(&mut cluster_rng, 3)
    });
    let biomes: Option<Vec<BiomeId>> = r
        .biomes
        .as_ref()
        .map(|bs| bs.iter().map(|b| defs.biome_id(b).expect("validated")).collect());
    let mut taken = Grid::from_fn(map.width(), map.height(), |_| false);
    let mut found = Vec::new();

    for p in map.positions() {
        let tile = map[p];
        let roll: f64 = rng.random();
        let terrain_ok = r.terrains.as_ref().is_none_or(|ts| ts.contains(&tile.terrain));
        let biome_ok = biomes.as_ref().is_none_or(|bs| bs.contains(&tile.biome));
        let beside_ok = r
            .beside
            .as_ref()
            .is_none_or(|ts| map.neighbors(p, true).any(|q| ts.contains(&map[q].terrain)));
        if !(terrain_ok && biome_ok && beside_ok) {
            continue;
        }
        let mut chance = r.chance;
        if let Some(clusters) = &clusters {
            let (u, v) = (f64::from(p.x) / CLUSTER_TILES, f64::from(p.y) / CLUSTER_TILES);
            let patch = smoothstep(-0.15, 0.15, clusters.get(u, v));
            chance *= lerp(1.0, 2.0 * patch, r.clustering);
        }
        if r.moisture_scaled {
            chance *= 0.5 + crate::defs::climate_unit(tile.moisture);
        }
        if roll < chance && r.min_spacing.is_none_or(|s| !within(&taken, p, s)) {
            taken[p] = true;
            found.push(p);
        }
    }
    found
}

/// Is there a marked tile closer than `spacing` (Chebyshev) to `p`?
fn within(marked: &Grid<bool>, p: Pos, spacing: u32) -> bool {
    let r = spacing as i32 - 1;
    (-r..=r)
        .any(|dy| (-r..=r).any(|dx| marked.get(Pos::new(p.x + dx, p.y + dy)).is_some_and(|&m| m)))
}

/// The glyph `to_ascii` draws for one tile: its resource's, else its terrain's, else (plain
/// land) its biome's.
pub fn tile_glyph(t: &Tile, defs: &Defs) -> char {
    match (t.resource, t.terrain) {
        (Some(r), _) => defs.resource(r).glyph,
        (None, Terrain::Water) => '~',
        (None, Terrain::Lake) => '-',
        (None, Terrain::River | Terrain::Canal) => '=',
        (None, Terrain::Ford) => '+',
        (None, Terrain::Coast) => '.',
        (None, Terrain::Mountain) => '^',
        (None, Terrain::Tunnel) => '_',
        (None, Terrain::Land) => defs.biome(t.biome).glyph,
    }
}

/// Renders the map as text: one glyph per tile, rows separated by newlines.
pub fn to_ascii(map: &TileMap, defs: &Defs) -> String {
    let mut out = String::with_capacity(((map.width() + 1) * map.height()) as usize);
    for (p, t) in map.iter() {
        out.push(tile_glyph(t, defs));
        if p.x == map.width() - 1 {
            out.push('\n');
        }
    }
    out
}
