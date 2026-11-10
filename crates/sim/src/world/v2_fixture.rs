//! Writes `tests/fixtures/v2_buildings.ron` (#617), the save-format v2 fixture that pins
//! "buildings as records" (#604, `docs/design/buildings-as-records.md`) to today's behaviour.
//!
//! This is a unit test, not an integration test, because it must stand finished buildings the
//! way `run_build` leaves them (`raise`, the `open_*` calls, the fire), grant techs, light a timed
//! flame and put stacks on the ground, and none of that is public. It is `#[ignore]`d. The
//! fixture it writes is read, and checked, by `tests/save_v2_fixture.rs`, using public queries
//! only.
//!
//! ```text
//! FN_REGENERATE_FIXTURE=1 cargo test -p fn_sim --lib --profile ci regenerate_v2_fixture -- --ignored
//! ```
//!
//! **Do not regenerate it after the save format changes** (v3, issue #604 part 2). A file written
//! then would be v3, and no longer prove anything about v2 saves. This test, and the
//! `v2_buildings.after.txt` digest next to the fixture, are deleted with the format change, the
//! way #602 deleted the v1 generator. Ordinary content changes need no regeneration: saves are
//! remapped by name on load (#602).
//!
//! The layout, all on a 32x24 field of grass (hand-made, so not even mapgen is involved). The
//! positions are repeated in `tests/save_v2_fixture.rs`.
// Test code, not the sim: it writes its fixture. The lint is for the library (AGENTS.md rule 4).
#![allow(clippy::disallowed_methods)]

use super::*;
use crate::building::ONE_BY_ONE;
use crate::defs::{BiomeId, Terrain};
use crate::geom::{Footprint, Grid};
use crate::mapgen::Tile;

const WIDTH: i32 = 32;
const HEIGHT: i32 = 24;

fn at(x: i32, y: i32) -> Pos {
    Pos::new(x, y)
}

/// Stands a finished `name` on `anchor` exactly as `run_build` leaves it.
fn finish(w: &mut World, defs: &Defs, anchor: Pos, name: &str) {
    let id = defs.building_id(name).unwrap_or_else(|| panic!("no building {name}"));
    let def = defs.building(id);
    let (fw, fh) = def.footprint;
    if let Some(orchard) = &def.orchard {
        w.open_orchard(anchor, orchard);
    }
    if def.farm.is_some() {
        w.open_farm_plot(anchor);
    }
    if def.irrigation.is_some() {
        w.open_irrigation_channel(anchor);
    }
    if def.power.is_some() {
        w.open_power_source(anchor);
    }
    w.raise(Footprint::new(anchor, fw, fh), id, def);
    if def.chart.is_some() {
        w.open_chart_seats(anchor, def.seats);
    } else {
        w.open_study_seats(anchor, def.seats);
    }
    w.open_craft_jobs(anchor, def);
    w.light_building_fire(anchor, [fw, fh], def);
    if let Some(unlock) = def.unlocked_by {
        w.grant_tech(unlock);
    }
}

fn make_world(defs: &Defs) -> World {
    let ruins = defs.resource_id("lost_ruins").expect("lost ruins");
    let map = Grid::from_fn(WIDTH, HEIGHT, |p| Tile {
        elevation: 128,
        temperature: 128,
        moisture: 128,
        terrain: Terrain::Land,
        biome: BiomeId(0),
        // A buried ruin for the Map Room to chart.
        resource: (p == at(28, 2)).then_some(ruins),
        building: None,
    });
    let spawns = [at(14, 8), at(15, 8), at(14, 9), at(15, 9), at(16, 8), at(16, 9)];
    let mut w = World::with_map(7, map, Nation::Fire, &spawns, defs);
    w.bury(defs);

    for x in 4..=6 {
        w.apply(Command::DesignateStockpile { pos: at(x, 4) }, defs).unwrap();
    }
    finish(&mut w, defs, at(8, 6), "bed");
    finish(&mut w, defs, at(10, 6), "campfire");
    finish(&mut w, defs, at(12, 5), "healing_hut");
    finish(&mut w, defs, at(16, 4), "study_hall");
    finish(&mut w, defs, at(20, 4), "map_room");
    finish(&mut w, defs, at(16, 12), "forge");
    finish(&mut w, defs, at(18, 12), "wind_turbine");
    finish(&mut w, defs, at(22, 12), "orchard");
    finish(&mut w, defs, at(6, 14), "farm_plot");
    finish(&mut w, defs, at(7, 14), "irrigation_channel");
    for x in 10..=13 {
        finish(&mut w, defs, at(x, 18), "rail");
    }
    // A timed Hearthflame, lit well past the end of any test.
    w.light_heat_source(HeatSource {
        pos: at(26, 20),
        radius: 3,
        sleep_bonus: 1.5,
        seek_tiles: 20,
        until: 6_000,
        size: ONE_BY_ONE,
        cooks: true,
        ability: None,
    });

    // The stockpile is full of other things, so the forge's tools stay on the ground (they would
    // count toward its stock limit and idle it before the orchard has grown a tree).
    let item = |name: &str| defs.item_id(name).unwrap_or_else(|| panic!("no item {name}"));
    for (x, name, count) in [(4, "gold_ore", 5), (5, "ice", 3), (6, "herbs", 2)] {
        w.stored.insert(at(x, 4), Stack { item: item(name), count });
    }

    // Supplies: metal for the forge, wood (but no stone) for the part-built windmill.
    w.place_ground(at(14, 14), Stack { item: item("metal"), count: 30 });
    w.place_ground(at(24, 18), Stack { item: item("wood"), count: 10 });

    // The research the scholar works toward, and a build site that gets some of its cost.
    let target = defs.tech_id("lightning_generation").expect("tech");
    w.apply(Command::SetResearch { tech: target }, defs).unwrap();
    let windmill = defs.building_id("windmill").unwrap();
    w.grant_tech(defs.building(windmill).unlocked_by.expect("windmill is locked"));
    w.apply(Command::DesignateBuild { pos: at(20, 18), building: windmill }, defs).unwrap();
    w.apply(Command::Sow { pos: at(6, 14) }, defs).unwrap();
    w.refresh_modifiers(defs);
    w
}

/// What the fixture promises that the world doesn't have yet.
fn missing(w: &World) -> Vec<&'static str> {
    let checks = [
        ("a scholar seated in the hall", w.units.iter().any(|u| u.inside() == Some(at(16, 4)))),
        (
            "a smith mid-batch in the forge",
            w.units.iter().any(|u| u.inside() == Some(at(16, 12))) && !w.crafts.is_empty(),
        ),
        (
            "some of the windmill's cost",
            w.build_site(at(20, 18)).is_some_and(|(_, d, _)| d.iter().sum::<u32>() > 0),
        ),
        ("chart progress", w.chart_progress(at(20, 4)) > 0.0),
        (
            "a plot about to ripen (so the digest sees it reaped)",
            w.plot_growth_left(at(6, 14)).is_some_and(|left| left < 100.0),
        ),
        ("an orchard tree", w.orchard_trees(at(22, 12)).is_some_and(|n| n > 0)),
    ];
    checks.into_iter().filter(|(_, ok)| !ok).map(|(what, _)| what).collect()
}

#[test]
#[ignore = "rewrites the checked-in v2 fixture; run by hand, and only while saves are v2"]
fn regenerate_v2_fixture() {
    assert!(
        std::env::var_os("FN_REGENERATE_FIXTURE").is_some(),
        "set FN_REGENERATE_FIXTURE=1 to confirm you mean to rewrite the fixture"
    );
    assert_eq!(crate::save::SAVE_VERSION, 2, "the v2 fixture can only be written by a v2 build");
    let defs = Defs::bundled();
    let mut w = make_world(&defs);
    let mut guard = 0;
    while !missing(&w).is_empty() {
        w.tick(&defs);
        guard += 1;
        assert!(guard < 6_000, "the fixture world never got busy; missing {:?}", missing(&w));
    }
    let text = w.save(&defs).unwrap();
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
    std::fs::write(dir.join("v2_buildings.ron"), text).unwrap();
}
