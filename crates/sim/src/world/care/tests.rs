//! `World::care` (issue #533, decision #520): hand-built worlds and units.

use super::*;
use crate::defs::{BiomeId, Terrain};
use crate::geom::{Grid, Pos};
use crate::mapgen::{Tile, TileMap};
use crate::nation::Nation;
use crate::world::World;

fn field() -> TileMap {
    Grid::from_fn(8, 4, |_| Tile {
        elevation: 100,
        temperature: 128,
        moisture: 128,
        terrain: Terrain::Land,
        biome: BiomeId(0),
        resource: None,
        building: None,
    })
}

/// A bare field with one settler, and nothing to eat or drink.
fn world(defs: &Defs) -> World {
    World::with_map(1, field(), Nation::Earth, &[Pos::new(2, 1)], defs)
}

/// A settler to feed `CareTally::observe` by hand, at `health`, everything else full.
fn settler(defs: &Defs, health: f32) -> Unit {
    let mut u = world(defs).units[0].clone();
    u.needs = Needs { health, ..Needs::full(&defs.needs) };
    u
}

fn at_health(tally: &mut CareTally, u: &mut Unit, defs: &Defs, health: f32) {
    u.needs.health = health;
    tally.observe(u, false, defs);
}

#[test]
fn a_fresh_world_has_counted_nothing() {
    let defs = Defs::bundled();
    assert_eq!(*world(&defs).care(), CareTally::default());
    assert_eq!(world(&defs).care().lowest_health(), None);
}

#[test]
fn person_minutes_add_up_per_settler_per_tick() {
    let defs = Defs::bundled();
    let mut w = world(&defs);
    for _ in 0..600 {
        w.tick(&defs);
    }
    // 600 ticks is a minute at 10 ticks a second, for the one settler.
    assert_eq!(w.care().person_minutes(), 1.0);
    assert_eq!(w.care().depleted_minutes(CareNeed::Hunger), 0.0);
    assert_eq!(w.care().cold_minutes(), 0.0);
    assert!(w.care().lowest_health().is_some_and(|h| h > 0.0));
}

#[test]
fn an_empty_need_accrues_depleted_minutes_at_the_expected_rate() {
    let defs = Defs::bundled();
    let mut w = world(&defs);
    w.units[0].needs.hunger = 0.0;
    for _ in 0..300 {
        w.tick(&defs);
    }
    let care = w.care();
    assert_eq!(care.depleted_minutes(CareNeed::Hunger), 0.5, "300 ticks is half a minute");
    assert_eq!(care.depleted_minutes(CareNeed::Thirst), 0.0);
    assert_eq!(care.depleted_minutes(CareNeed::Energy), 0.0);
    assert_eq!(care.person_minutes(), 0.5);
    // At 0, hunger is also well below half its `low`.
    assert_eq!(care.severe_minutes(Severe::Starving), 0.5);
    assert_eq!(care.severe_minutes(Severe::Parched), 0.0);
}

#[test]
fn severe_flags_follow_the_people_rows_bad_rule() {
    let defs = Defs::bundled();
    let low = defs.needs.hunger.low;
    let mut u = settler(&defs, defs.needs.health.max);
    let mut tally = CareTally::default();

    // Hungry (below `low`, not below half of it) isn't severe.
    u.needs.hunger = low * 0.75;
    tally.observe(&u, false, &defs);
    assert_eq!(tally.severe_minutes(Severe::Starving), 0.0);

    // Below half of `low` is Starving; a second flag at once counts for each.
    u.needs.hunger = low * 0.4;
    u.needs.thirst = 0.0;
    tally.observe(&u, false, &defs);
    let tick = 1.0 / (60.0 * f64::from(TICKS_PER_SECOND));
    assert_eq!(tally.severe_minutes(Severe::Starving), tick);
    assert_eq!(tally.severe_minutes(Severe::Parched), tick);
    assert_eq!(tally.severe_minutes(Severe::Exhausted), 0.0);
    assert_eq!(tally.severe_minutes(Severe::Dying), 0.0);
    assert_eq!(tally.depleted_minutes(CareNeed::Hunger), 0.0, "Starving isn't empty");
    assert_eq!(tally.depleted_minutes(CareNeed::Thirst), tick);

    // Dying is health below half of the People row's warning mark (25 of 100).
    let dying = defs.needs.health.max * HEALTH_LOW / 2.0;
    let mut u = settler(&defs, dying);
    tally.observe(&u, false, &defs);
    assert_eq!(tally.severe_minutes(Severe::Dying), 0.0, "exactly at the mark is still Hurt");
    u.needs.health = dying - 0.1;
    tally.observe(&u, false, &defs);
    assert_eq!(tally.severe_minutes(Severe::Dying), tick);
}

#[test]
fn severe_any_counts_a_settler_once_however_many_flags_they_show() {
    let defs = Defs::bundled();
    let tick = 1.0 / (60.0 * f64::from(TICKS_PER_SECOND));
    let mut u = settler(&defs, defs.needs.health.max);
    let mut tally = CareTally::default();

    tally.observe(&u, false, &defs);
    assert_eq!(tally.severe_any_minutes(), 0.0, "a comfortable settler isn't suffering");

    // Starving and Parched at once: each flag accrues one tick, and so does "any".
    u.needs.hunger = 0.0;
    u.needs.thirst = 0.0;
    tally.observe(&u, false, &defs);
    assert_eq!(tally.severe_minutes(Severe::Starving), tick);
    assert_eq!(tally.severe_minutes(Severe::Parched), tick);
    assert_eq!(tally.severe_any_minutes(), tick);

    // A dead tick isn't a minute lived, so it isn't suffering either.
    u.needs.health = 0.0;
    tally.observe(&u, false, &defs);
    assert_eq!(tally.severe_any_minutes(), tick);
    assert!(tally.severe_any_minutes() <= tally.person_minutes());

    // Nor is the Avatar a settler.
    let mut avatar = settler(&defs, 5.0);
    avatar.guest = Some(crate::world::Guest::Avatar);
    tally.observe(&avatar, false, &defs);
    assert_eq!(tally.severe_any_minutes(), tick);
}

#[test]
fn a_dip_counts_one_close_call_and_only_re_arms_above_the_higher_mark() {
    let defs = Defs::bundled();
    let mut u = settler(&defs, 100.0);
    let mut tally = CareTally::default();

    at_health(&mut tally, &mut u, &defs, 20.0);
    assert_eq!(tally.close_calls(), 0, "in the dip, not yet known to live");
    at_health(&mut tally, &mut u, &defs, 30.0);
    assert_eq!(tally.close_calls(), 1, "back above the dip mark, alive");
    assert_eq!(tally.lowest_health(), Some(20.0));

    // A second dip before recovering above 50 doesn't count.
    at_health(&mut tally, &mut u, &defs, 10.0);
    at_health(&mut tally, &mut u, &defs, 40.0);
    at_health(&mut tally, &mut u, &defs, 15.0);
    at_health(&mut tally, &mut u, &defs, 50.0);
    assert_eq!(tally.close_calls(), 1, "50 isn't above the re-arm mark");
    at_health(&mut tally, &mut u, &defs, 15.0);
    at_health(&mut tally, &mut u, &defs, 45.0);
    assert_eq!(tally.close_calls(), 1);
    assert_eq!(tally.lowest_health(), Some(10.0));

    // After recovering above 50, the next dip counts.
    at_health(&mut tally, &mut u, &defs, 51.0);
    at_health(&mut tally, &mut u, &defs, 24.9);
    at_health(&mut tally, &mut u, &defs, 25.0);
    assert_eq!(tally.close_calls(), 2);
}

#[test]
fn a_recovery_straight_past_the_re_arm_mark_counts_and_re_arms() {
    let defs = Defs::bundled();
    let mut u = settler(&defs, 100.0);
    let mut tally = CareTally::default();
    at_health(&mut tally, &mut u, &defs, 5.0);
    at_health(&mut tally, &mut u, &defs, 90.0); // a remedy and a rest in one step
    assert_eq!(tally.close_calls(), 1);
    at_health(&mut tally, &mut u, &defs, 5.0);
    at_health(&mut tally, &mut u, &defs, 90.0);
    assert_eq!(tally.close_calls(), 2);
}

#[test]
fn each_settler_has_their_own_episode() {
    let defs = Defs::bundled();
    let mut a = settler(&defs, 100.0);
    let mut b = settler(&defs, 100.0);
    b.id = UnitId(a.id.0 + 1);
    let mut tally = CareTally::default();
    at_health(&mut tally, &mut a, &defs, 10.0);
    at_health(&mut tally, &mut b, &defs, 10.0);
    at_health(&mut tally, &mut a, &defs, 30.0);
    at_health(&mut tally, &mut b, &defs, 30.0);
    assert_eq!(tally.close_calls(), 2);
}

#[test]
fn a_settler_who_dies_in_the_dip_adds_no_close_call() {
    let defs = Defs::bundled();
    let mut u = settler(&defs, 100.0);
    let mut tally = CareTally::default();
    at_health(&mut tally, &mut u, &defs, 15.0);
    at_health(&mut tally, &mut u, &defs, 0.0);
    assert_eq!(tally.close_calls(), 0);
    assert_eq!(tally.lowest_health(), Some(0.0));
    assert_eq!(tally.person_minutes(), 1.0 / 600.0, "the tick they died isn't a minute lived");
    // Nothing is left pending for the id either.
    assert!(tally.dipped.is_empty() && tally.counted.is_empty());
}

#[test]
fn a_death_in_a_world_adds_no_close_call() {
    let defs = Defs::bundled();
    let mut w = world(&defs);
    w.units[0].needs.health = 20.0;
    w.units[0].needs.hunger = 0.0;
    // At 0 hunger, the health drain outpaces everything, and nothing here can feed him.
    for _ in 0..20_000 {
        if w.units.is_empty() {
            break;
        }
        w.tick(&defs);
    }
    assert!(w.units.is_empty(), "the settler starved");
    assert_eq!(w.deaths().count(), 1);
    assert_eq!(w.care().close_calls(), 0);
    assert_eq!(w.care().lowest_health(), Some(0.0));
}

#[test]
fn the_avatar_is_not_a_settler() {
    let defs = Defs::bundled();
    let mut u = settler(&defs, 5.0);
    u.guest = Some(crate::world::Guest::Avatar);
    let mut tally = CareTally::default();
    tally.observe(&u, true, &defs);
    assert_eq!(tally, CareTally::default());
}

#[test]
fn the_avatar_leaving_counts_one_close_call() {
    let mut tally = CareTally::default();
    tally.count_avatar_left();
    assert_eq!(tally.close_calls(), 1);
}

#[test]
fn cold_ticks_count_only_when_exposure_applied() {
    let defs = Defs::bundled();
    let u = settler(&defs, 100.0);
    let mut tally = CareTally::default();
    tally.observe(&u, false, &defs);
    assert_eq!(tally.cold_minutes(), 0.0);
    tally.observe(&u, true, &defs);
    assert_eq!(tally.cold_minutes(), 1.0 / 600.0);
}

#[test]
fn the_tally_survives_a_save_round_trip() {
    let defs = Defs::bundled();
    let mut w = world(&defs);
    w.units[0].needs.hunger = 0.0;
    w.units[0].needs.health = 20.0;
    for _ in 0..50 {
        w.tick(&defs);
    }
    // Mid-dip, so the pending episode is part of what must survive.
    assert!(w.care().depleted_minutes(CareNeed::Hunger) > 0.0);
    assert!(!w.care.dipped.is_empty());
    let text = w.save(&defs).unwrap();
    let loaded = World::load(&text, &defs).unwrap();
    assert_eq!(loaded.care(), w.care());
}
