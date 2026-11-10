//! Bending-heavy runs, one per nation, with `World::check_invariants` (#600) every
//! `CHECK_EVERY` ticks: bends are where jobs, chi, patches and the tile map touch the most
//! tables, so a settler-versus-map disagreement (#559, #562, #582, #583) is likeliest to show up
//! here. `World::assert_invariants` is compiled in under `debug_assertions` (the `ci` profile has
//! them on) and does nothing in a release build.

use fn_sim::autoplay::Opening;
use fn_sim::embark::Survey;
use fn_sim::{Defs, MapGenConfig, Nation, World};

const SEED: u64 = 7;
const MINUTES: u64 = 90;

fn bending_run(nation: Nation) {
    let defs = Defs::bundled();
    let size = defs.embark.map_size("small").expect("bundled data always has a small map size");
    let map_cfg =
        MapGenConfig { seed: SEED, width: size.width, height: size.height, ..Default::default() };
    let survey = Survey::new(&map_cfg, &defs);
    let start = survey.default_site(&defs);
    let mut world = World::found(survey, nation, start, 6, Some("normal".to_owned()), &defs)
        .expect("Survey::default_site always names a site World::found accepts");
    let opening = Opening::with_bending(6, 6).with_full_research();
    for tick in 0..(MINUTES * 600) {
        if Opening::due(tick) {
            opening.step(&mut world, &defs);
        }
        world.tick(&defs);
        world.assert_invariants(&defs);
    }
    // The run did something: bends were designated, so the check had bend jobs to look at.
    assert!(!world.command_log().is_empty());
}

#[test]
fn a_bending_heavy_earth_run_stays_consistent() {
    bending_run(Nation::Earth);
}

#[test]
fn a_bending_heavy_water_run_stays_consistent() {
    bending_run(Nation::Water);
}

#[test]
fn a_bending_heavy_fire_run_stays_consistent() {
    bending_run(Nation::Fire);
}

#[test]
fn a_bending_heavy_air_run_stays_consistent() {
    bending_run(Nation::Air);
}
