use super::*;
use crate::defs::{BiomeId, Terrain};
use crate::geom::{Footprint, Grid};
use crate::mapgen::Tile;
use crate::world::farm::FarmPlot;

fn arena(temperature: u8) -> TileMap {
    Grid::from_fn(16, 16, |_| Tile {
        elevation: 100,
        temperature,
        moisture: 128,
        terrain: Terrain::Land,
        biome: BiomeId(0),
        resource: None,
        building: None,
    })
}

fn at(x: i32, y: i32) -> Pos {
    Pos::new(x, y)
}

/// The Hothouse's 2x2 footprint covers (5..=6, 5..=6), so with radius 2 it reaches x and y in
/// 3..=8.
const HOUSE: Pos = Pos::new(5, 5);
const NEAR: Pos = Pos::new(8, 6);
const FAR: Pos = Pos::new(9, 6);

fn season_ticks(defs: &Defs) -> u64 {
    (defs.seasons.calendar.season_minutes as f64 * 60.0 * TICKS_PER_SECOND as f64) as u64
}

/// A Fire world on a uniform band with seasons on, at the start of winter, with a finished,
/// empty Hothouse at `HOUSE`, and growing farm plots at `NEAR` and `FAR`.
fn setup(temperature: u8) -> (World, Defs) {
    let defs = Defs::bundled();
    let mut w = World::with_map(1, arena(temperature), Nation::Fire, &[at(0, 0)], &defs);
    w.difficulty = Some("normal".into());
    w.tick = 3 * season_ticks(&defs);
    assert!(w.season(&defs).winter, "the fourth season is winter");
    let house = defs.building_id("hothouse").unwrap();
    w.put_building(Footprint::new(HOUSE, 2, 2), house, &defs);
    let plot = defs.building_id("farm_plot").unwrap();
    for p in [NEAR, FAR] {
        w.put_building(Footprint::single(p), plot, &defs);
        set_stage(&mut w, p, PlotStage::Growing);
    }
    (w, defs)
}

fn set_stage(w: &mut World, pos: Pos, stage: PlotStage) {
    let plot: &mut FarmPlot = w.anchored_mut(pos).and_then(|b| b.plot.as_mut()).unwrap();
    plot.stage = stage;
    plot.growing_left = 30.0;
}

fn fill(w: &mut World, n: u32) {
    w.anchored_mut(HOUSE).and_then(|b| b.fuel.as_mut()).unwrap().count = n;
}

fn fuel(w: &World) -> u32 {
    w.anchored(HOUSE).and_then(|b| b.fuel).unwrap().count
}

/// A minute of fuel ticks, and one more: 2 a minute is 3333 micros a tick, rounded down, so a
/// bare 600 ticks fall 200 micros short of the second log.
fn minute(w: &mut World, defs: &Defs) {
    for _ in 0..601 {
        w.tick_fuel(defs);
    }
}

#[test]
fn the_hothouse_is_fires_and_burns_wood() {
    let defs = Defs::bundled();
    let def = defs.building(defs.building_id("hothouse").unwrap());
    assert_eq!(def.unlocked_by, defs.tech_id("hothouses"));
    let tech = defs.tech(defs.tech_id("hothouses").unwrap());
    assert_eq!(tech.nation, Some(Nation::Fire));
    assert!(!tech.core, "an optional horizons tech");
    let h = def.hothouse.unwrap();
    assert_eq!((h.radius, h.winter_growth), (2, 0.75));
    assert_eq!(defs.item(def.fuel.unwrap().item).id, "wood");
    assert_eq!(def.footprint, (2, 2));
}

#[test]
fn a_fuelled_hothouse_gives_plots_in_reach_its_winter_growth() {
    let (mut w, defs) = setup(128);
    let band = w.crop_growth_rate(NEAR, &defs);
    assert!(band < 0.75, "a temperate winter is slower than the Hothouse: {band}");
    // Empty, it does nothing.
    assert_eq!(w.hothouse_growth(NEAR, &defs), None);
    fill(&mut w, 10);
    assert_eq!(w.hothouse_growth(NEAR, &defs), Some(0.75));
    assert_eq!(w.hothouse_growth(FAR, &defs), None, "out of reach");
    let base = w.farm_def(NEAR, &defs).unwrap().crop_growth;
    assert!((w.crop_growth_rate(NEAR, &defs) - 0.75 * base).abs() < 1e-6);
    assert_eq!(w.crop_growth_rate(FAR, &defs), band);
    // A second Hothouse reaching the same plot doesn't stack.
    let house = defs.building_id("hothouse").unwrap();
    w.put_building(Footprint::new(at(9, 3), 2, 2), house, &defs);
    w.anchored_mut(at(9, 3)).and_then(|b| b.fuel.as_mut()).unwrap().count = 10;
    assert!((w.crop_growth_rate(NEAR, &defs) - 0.75 * base).abs() < 1e-6);
}

#[test]
fn outside_winter_or_with_seasons_off_it_does_nothing() {
    let (mut w, defs) = setup(128);
    fill(&mut w, 10);
    w.tick = 0;
    assert_eq!(w.hothouse_growth(NEAR, &defs), None);
    minute(&mut w, &defs);
    assert_eq!(fuel(&w), 10, "no burning outside winter");
    let (mut w, defs) = setup(128);
    fill(&mut w, 10);
    w.difficulty = None;
    assert_eq!(w.hothouse_growth(NEAR, &defs), None);
    minute(&mut w, &defs);
    assert_eq!(fuel(&w), 10, "no burning with seasons off");
}

#[test]
fn it_burns_only_in_winter_while_a_plot_in_reach_is_growing_or_ripe() {
    let (mut w, defs) = setup(128);
    fill(&mut w, 10);
    assert_eq!(w.hothouse_warming(HOUSE, &defs), Some(1));
    minute(&mut w, &defs);
    assert_eq!(fuel(&w), 8, "2 wood a minute while a plot grows");
    set_stage(&mut w, NEAR, PlotStage::Ripe);
    minute(&mut w, &defs);
    assert_eq!(fuel(&w), 6, "a ripe plot keeps it burning");
    // Only the plot out of reach still grows: nothing to warm.
    set_stage(&mut w, NEAR, PlotStage::Empty);
    assert_eq!(w.hothouse_warming(HOUSE, &defs), Some(0));
    minute(&mut w, &defs);
    assert_eq!(fuel(&w), 6);
}

#[test]
fn the_first_frost_spares_ripe_crops_in_reach_of_a_fuelled_hothouse() {
    // Polar: a frost band.
    let (mut w, defs) = setup(0);
    fill(&mut w, 10);
    set_stage(&mut w, NEAR, PlotStage::Ripe);
    set_stage(&mut w, FAR, PlotStage::Ripe);
    w.tick = 3 * season_ticks(&defs) - 1;
    w.tick(&defs);
    assert_eq!(w.plot_stage(NEAR), Some(PlotStage::Ripe), "spared");
    assert_eq!(w.plot_stage(FAR), Some(PlotStage::Empty), "taken");
}
