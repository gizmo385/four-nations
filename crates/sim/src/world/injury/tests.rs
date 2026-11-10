//! Injury (issue #388): a unit hurt finishing a heavy pick (felling, quarrying, mining).

use super::*;
use crate::defs::{BiomeId, Terrain};
use crate::geom::Grid;
use crate::jobs::HarvestDef;
use crate::mapgen::Tile;
use crate::nation::Nation;

fn at(x: i32, y: i32) -> Pos {
    Pos::new(x, y)
}

/// A flat field of land, `w` wide.
fn field(w: i32, h: i32) -> TileMap {
    Grid::from_fn(w, h, |_| Tile {
        elevation: 100,
        temperature: 128,
        moisture: 128,
        terrain: Terrain::Land,
        biome: BiomeId(0),
        resource: None,
        building: None,
    })
}

fn world(seed: u64, nation: Nation, preset: &str) -> (World, Defs) {
    let defs = Defs::bundled();
    let mut w = World::with_map(seed, field(3, 3), nation, &[at(0, 0)], &defs);
    w.difficulty = Some(preset.into());
    (w, defs)
}

fn stone_harvest(defs: &Defs) -> &HarvestDef {
    let stone = defs.resource_id("stone").unwrap();
    defs.harvest(stone).unwrap()
}

/// Repeatedly rolls `h` for unit 0, topping its health back up between rolls so a hit never
/// pushes it below the floor and masks a later one; returns how many of `picks` hurt it.
fn count_injuries(w: &mut World, defs: &Defs, h: &HarvestDef, picks: u32) -> u32 {
    let mut hits = 0;
    for _ in 0..picks {
        w.units[0].needs.health = defs.needs.health.max;
        w.maybe_injure(0, at(0, 0), h, defs);
        if w.units[0].needs.health < defs.needs.health.max {
            hits += 1;
        }
    }
    hits
}

#[test]
fn harm_zero_draws_nothing() {
    // No difficulty preset at all: `Knobs::identity`'s `harm` is 0.0.
    let defs = Defs::bundled();
    let mut w = World::with_map(1, field(3, 3), Nation::Air, &[at(0, 0)], &defs);
    let h = stone_harvest(&defs);
    assert_eq!(count_injuries(&mut w, &defs, h, 5_000), 0);
    assert!(w.harm_events().next().is_none());

    // Sandbox: `harm` is explicitly 0.0 too.
    let (mut sandbox, defs) = world(1, Nation::Air, "sandbox");
    assert_eq!(count_injuries(&mut sandbox, &defs, h, 5_000), 0);
    assert!(sandbox.harm_events().next().is_none());
}

#[test]
fn a_harvest_with_no_risk_never_injures() {
    // A harvest with `risk` at 0.0 (the default; nothing in the bundled data is this low, so
    // this checks a `HarvestDef` built by hand, as `maybe_injure` would see it).
    let (mut w, defs) = world(1, Nation::Earth, "normal");
    let zero_risk = HarvestDef { risk: 0.0, ..stone_harvest(&defs).clone() };
    assert_eq!(count_injuries(&mut w, &defs, &zero_risk, 5_000), 0);
}

/// A seeded run injures a quarrier at the expected rate: Earth's earthworks (2.5) divides
/// stone's 0.02 risk down to 0.008 a pick, Air's (0.7) only down to about 0.0286 -- more than
/// three times Earth's. Over enough picks the two counts land in disjoint, wide bands around
/// their own expectations (80 and about 286 of 10,000), so this also pins the same seed to the
/// same count on every run.
#[test]
fn earths_earthworks_lowers_the_injury_rate() {
    let picks = 10_000;
    let h_def;
    let earth_hits = {
        let (mut w, defs) = world(7, Nation::Earth, "normal");
        h_def = stone_harvest(&defs).clone();
        count_injuries(&mut w, &defs, &h_def, picks)
    };
    let air_hits = {
        let (mut w, defs) = world(7, Nation::Air, "normal");
        count_injuries(&mut w, &defs, &h_def, picks)
    };
    assert!((40..=130).contains(&earth_hits), "earth: {earth_hits}");
    assert!((200..=370).contains(&air_hits), "air: {air_hits}");
    assert!(
        earth_hits < air_hits,
        "earth's earthworks should hurt it less than Air: \
             earth {earth_hits}, air {air_hits}"
    );

    // The same seed draws the same sequence, so the count is reproducible.
    let (mut w2, defs2) = world(7, Nation::Earth, "normal");
    assert_eq!(count_injuries(&mut w2, &defs2, &h_def, picks), earth_hits);
}

#[test]
fn a_harvest_with_no_modifier_rolls_risk_directly() {
    // Felling (no modifier) uses `risk` unscaled, whichever nation does it.
    let picks = 10_000;
    let (mut w, defs) = world(3, Nation::Earth, "normal");
    let oak = defs.resource_id("oak_tree").unwrap();
    let h = defs.harvest(oak).unwrap().clone();
    assert_eq!(h.modifier, None);
    let hits = count_injuries(&mut w, &defs, &h, picks);
    // Expectation: 10,000 x 0.01 = 100. A wide band, same reasoning as the quarrying test.
    assert!((40..=180).contains(&hits), "{hits}");
}

#[test]
fn an_injury_sets_last_harm_and_logs_an_event() {
    let (mut w, defs) = world(7, Nation::Air, "normal");
    let h = stone_harvest(&defs).clone();
    // Air's rate on stone is high enough that a handful of picks reliably hits at least once.
    for _ in 0..200 {
        w.units[0].needs.health = defs.needs.health.max;
        w.maybe_injure(0, at(2, 1), &h, &defs);
        if w.units[0].needs.health < defs.needs.health.max {
            break;
        }
    }
    assert!(w.units[0].needs.health < defs.needs.health.max, "expected at least one hit");
    assert_eq!(w.units[0].last_harm.map(|(k, _)| k), Some(HarmKind::Injury));
    assert_eq!(w.harm_events().count(), 1);
    assert_eq!(w.harm_events().next().unwrap().kind, HarmEventKind::Injured);
    assert_eq!(w.harm_events().next().unwrap().pos, at(2, 1));
}

#[test]
fn an_injury_floors_at_10_on_normal_but_never_kills() {
    let (mut w, defs) = world(7, Nation::Air, "normal");
    let h = stone_harvest(&defs).clone();
    let just_above_floor = defs.harm.settings.floor + 1.0;
    // Force a hit: Air's chance on stone is well under 1, so roll until one lands, keeping
    // health pinned just above the floor so a single hit would cross it if unfloored.
    for _ in 0..100_000 {
        w.units[0].needs.health = just_above_floor;
        w.maybe_injure(0, at(0, 0), &h, &defs);
        if w.units[0].needs.health != just_above_floor {
            break;
        }
    }
    assert_eq!(w.units[0].needs.health, defs.harm.settings.floor);
    assert!(w.deaths().next().is_none());
}

#[test]
fn an_injury_can_kill_on_hard() {
    let (mut w, defs) = world(7, Nation::Air, "hard");
    let h = stone_harvest(&defs).clone();
    let scale = w.knobs(&defs).harm;
    // One hit's worth of (scaled) damage, less a hair: a hit from here takes health to 0 only if
    // `harm_kills` really does let it go below the floor.
    let lethal_health = defs.harm.injury.damage * scale - 1.0;
    for _ in 0..100_000 {
        w.units[0].needs.health = lethal_health;
        w.maybe_injure(0, at(0, 0), &h, &defs);
        if w.units[0].needs.health <= 0.0 {
            break;
        }
    }
    assert_eq!(w.units[0].needs.health, 0.0, "harm_kills lets an injury take health to 0");
}

#[test]
fn a_unit_below_fit_for_heavy_is_unfit_only_while_harm_is_in_play() {
    let (mut w, defs) = world(1, Nation::Earth, "normal");
    w.units[0].needs.health = 70.0;
    assert!(!w.fit_for_heavy_work(&w.units[0], &defs));
    w.units[0].needs.health = 80.0;
    assert!(w.fit_for_heavy_work(&w.units[0], &defs));

    // No difficulty preset at all: harm is off, so the health check never applies.
    let mut off = World::with_map(1, field(3, 3), Nation::Earth, &[at(0, 0)], &defs);
    off.units[0].needs.health = 10.0;
    assert!(off.fit_for_heavy_work(&off.units[0], &defs));
}
