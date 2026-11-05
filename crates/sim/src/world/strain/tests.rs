//! Strain: tiring wild land (#296).

use super::*;
use crate::building::Building;
use crate::defs::{BiomeId, Terrain};
use crate::geom::Grid;
use crate::mapgen::Tile;

/// A map from ASCII rows: `.` land, `~` water (so a unit worked for hours on end still has
/// something to drink), `Y` fruit tree.
fn arena(defs: &Defs, rows: &[&str]) -> TileMap {
    Grid::from_fn(rows[0].len() as i32, rows.len() as i32, |p| {
        let c = rows[p.y as usize].as_bytes()[p.x as usize];
        let terrain = if c == b'~' { Terrain::Water } else { Terrain::Land };
        let resource = match c {
            b'Y' => defs.resource_id("fruit_tree"),
            _ => None,
        };
        Tile {
            elevation: 100,
            temperature: 128,
            moisture: 128,
            terrain,
            biome: BiomeId(0),
            resource,
            building: None,
        }
    })
}

fn at(x: i32, y: i32) -> Pos {
    Pos::new(x, y)
}

fn setup(preset: Option<&str>, rows: &[&str], units: &[Pos]) -> (World, Defs) {
    let defs = Defs::bundled();
    let map = arena(&defs, rows);
    let mut w = World::with_map(1, map, Nation::Earth, units, &defs);
    w.difficulty = preset.map(String::from);
    (w, defs)
}

/// Turns `seasons` off on preset `id`, so a test about strain alone isn't also measuring the
/// season's own growth factor (`growth_rate`, #295): the doc's equilibrium numbers are strain's
/// alone.
fn without_seasons(defs: &mut Defs, id: &str) {
    defs.difficulty.presets.iter_mut().find(|p| p.id == id).unwrap().seasons = false;
}

/// Stops hunger, thirst and energy from decaying, so the lone unit in a many-cycle test never
/// has to break off picking to eat, drink or sleep: that would perturb the pick timing this is
/// actually about. Health still regenerates and nothing else about needs changes.
fn immortal(defs: &mut Defs) {
    defs.needs.hunger.decay_per_minute = 0.0;
    defs.needs.thirst.decay_per_minute = 0.0;
    defs.needs.energy.decay_per_minute = 0.0;
}

fn run_until(w: &mut World, defs: &Defs, limit: u64, mut done: impl FnMut(&World) -> bool) -> u64 {
    for n in 0..limit {
        if done(w) {
            return n;
        }
        w.tick(defs);
    }
    panic!("not done after {limit} ticks");
}

/// `data/jobs.toml`'s fruit tree's own regrow time, in seconds.
fn regrow_seconds(defs: &Defs) -> f32 {
    let r = defs.resource_id("fruit_tree").unwrap();
    defs.harvest(r).unwrap().regrow.unwrap()
}

const TREE: Pos = Pos::new(4, 2);
const ROWS: [&str; 5] = ["~........", "~........", "~...Y....", "~........", "~........"];

#[test]
fn repeated_picking_lengthens_regrowth_and_resting_recovers_it() {
    let (mut w, defs) = setup(Some("normal"), &ROWS, &[at(3, 2)]);
    assert_eq!(w.strain_at(TREE), None, "untouched land starts untired");

    let job = w.designate_harvest(TREE, &defs).unwrap();
    run_until(&mut w, &defs, 2000, |w| w.job(job).is_none());
    assert!(w.is_resting(TREE));
    let first_strain = w.strain_at(TREE).expect("a pick adds strain");
    assert!(first_strain > 0.0);
    let first_rest = run_until(&mut w, &defs, 100_000, |w| !w.is_resting(TREE));

    // Picking again, before strain has fully faded, adds more: the tile is worse off than after
    // the first pick, and rests longer.
    let job = w.designate_harvest(TREE, &defs).unwrap();
    run_until(&mut w, &defs, 2000, |w| w.job(job).is_none());
    let second_strain = w.strain_at(TREE).expect("still tired");
    assert!(second_strain > first_strain, "{second_strain} should exceed {first_strain}");
    let second_rest = run_until(&mut w, &defs, 100_000, |w| !w.is_resting(TREE));
    assert!(second_rest > first_rest, "{second_rest} should exceed {first_rest}: longer regrowth");

    // Left alone (ripe, unpicked) the strain fades back toward 0 and the entry is dropped.
    run_until(&mut w, &defs, 400_000, |w| w.strain_at(TREE).is_none());
}

#[test]
fn a_tree_picked_whenever_it_ripens_settles_near_the_docs_equilibrium() {
    // docs/design/difficulty.md, "Strain: tiring land": picked on sight every time it ripens, a
    // fruit tree settles at strain about 2.3 and a cycle of about 20 minutes with the shipped
    // numbers.
    let (mut w, mut defs) = setup(Some("normal"), &ROWS, &[at(4, 2)]);
    without_seasons(&mut defs, "normal");
    immortal(&mut defs);
    w.designate_repeating_harvest(TREE, &defs).unwrap();

    let mut prev_harvest_tick = 0u64;
    let mut last_cycle = 0u64;
    let mut last_strain = 0.0f32;
    for _ in 0..40 {
        // Wait for this pick to finish (strain was just added and resting just began).
        run_until(&mut w, &defs, 50_000, |w| w.is_resting(TREE));
        last_strain = w.strain_at(TREE).expect("just picked");
        last_cycle = w.tick - prev_harvest_tick;
        prev_harvest_tick = w.tick;
        // Wait for it to ripen again, so the next loop's "wait to be picked" doesn't fire at
        // once on the resting flag that's already true.
        run_until(&mut w, &defs, 50_000, |w| !w.is_resting(TREE));
    }

    assert!((2.0..=2.6).contains(&last_strain), "strain settled at {last_strain}, expected ~2.3");
    let twenty_minutes = (20.0 * 60.0 * TICKS_PER_SECOND as f32) as u64;
    let tolerance = twenty_minutes / 10;
    assert!(
        last_cycle.abs_diff(twenty_minutes) <= tolerance,
        "cycle settled at {last_cycle} ticks, expected close to {twenty_minutes}"
    );
}

#[test]
fn hard_kills_an_over_picked_tree_and_normal_never_does() {
    let (mut w, mut defs) = setup(Some("hard"), &ROWS, &[at(4, 2)]);
    without_seasons(&mut defs, "hard");
    immortal(&mut defs);
    // The shipped numbers converge toward strain 3 under Hard's always-repicked equilibrium (the
    // tuning pass, out of scope here, is what will pick exact numbers); lower the kill threshold
    // so this test exercises the kill mechanism itself quickly and deterministically, without
    // depending on where that tuning lands.
    defs.jobs.strain_kill_at = 2.5;
    w.designate_repeating_harvest(TREE, &defs).unwrap();

    let mut killed = false;
    for _ in 0..10 {
        run_until(&mut w, &defs, 50_000, |w| w.is_resting(TREE) || w.map[TREE].resource.is_none());
        if w.map[TREE].resource.is_none() {
            killed = true;
            break;
        }
        run_until(&mut w, &defs, 50_000, |w| !w.is_resting(TREE));
    }
    assert!(killed, "an over-picked tree should die on Hard");
    assert_eq!(w.strain_at(TREE), None, "the dead tile holds no strain");
    assert_eq!(w.strain_events().len(), 1);
    assert_eq!(w.strain_events()[0].pos, TREE);
    assert_eq!(
        w.strain_events()[0].resource,
        defs.resource_id("fruit_tree").unwrap(),
        "the TUI's alert (#299) names what it was"
    );

    let (mut w, mut defs) = setup(Some("normal"), &ROWS, &[at(4, 2)]);
    without_seasons(&mut defs, "normal");
    immortal(&mut defs);
    defs.jobs.strain_kill_at = 2.5;
    w.designate_repeating_harvest(TREE, &defs).unwrap();
    for _ in 0..10 {
        run_until(&mut w, &defs, 50_000, |w| w.is_resting(TREE));
        assert!(w.map[TREE].resource.is_some(), "Normal never kills an over-picked tree");
        run_until(&mut w, &defs, 50_000, |w| !w.is_resting(TREE));
    }
    assert!(w.strain_events().is_empty());
}

#[test]
fn strain_zero_matches_todays_behaviour_byte_for_byte() {
    for preset in [None, Some("sandbox")] {
        let (mut w, defs) = setup(preset, &ROWS, &[at(3, 2)]);
        let regrow = regrow_seconds(&defs);
        for _ in 0..3 {
            let job = w.designate_harvest(TREE, &defs).unwrap();
            run_until(&mut w, &defs, 2000, |w| w.job(job).is_none());
            assert_eq!(w.strain_at(TREE), None, "{preset:?}: strain never builds");
            let rest = run_until(&mut w, &defs, 100_000, |w| !w.is_resting(TREE));
            let expected = (regrow * w.knobs(&defs).regrow * TICKS_PER_SECOND as f32) as i64;
            assert!(
                (rest as i64 - expected).abs() <= 2,
                "{preset:?}: {rest} vs {expected} expected, with no strain multiplier"
            );
        }
        assert!(w.strain_events().is_empty());
    }
}

#[test]
fn an_orchards_radius_keeps_its_wild_trees_from_tiring() {
    let defs = Defs::bundled();
    let map = arena(&defs, &ROWS);
    let mut w = World::with_map(1, map, Nation::Air, &[at(3, 2)], &defs);
    w.difficulty = Some("normal".into());
    let id = defs.building_id("orchard").unwrap();
    let def = defs.building(id);
    // Well within the orchard's radius (4) of the fruit tree at TREE.
    let orchard_pos = at(1, 2);
    w.map[orchard_pos].building = Some(Building::new(id, def.passable, false));
    w.open_orchard(orchard_pos, def.orchard.as_ref().unwrap());
    assert!(w.tend_rate(TREE, &defs) > 1.0, "test assumes the tree is in reach");

    for _ in 0..3 {
        let job = w.designate_harvest(TREE, &defs).unwrap();
        run_until(&mut w, &defs, 2000, |w| w.job(job).is_none());
        assert_eq!(w.strain_at(TREE), None, "a tended tile never tires");
        run_until(&mut w, &defs, 100_000, |w| !w.is_resting(TREE));
    }
}
