//! Draw (#435): the meter's pace, its terms, the 1.0 stop, grants and the gap between arrivals.

use super::*;
use crate::defs::{BiomeId, Terrain};
use crate::geom::{Footprint, Grid};
use crate::mapgen::Tile;

fn at(x: i32, y: i32) -> Pos {
    Pos::new(x, y)
}

/// A flat 21x9 field of land with one settler at (10, 4), as population growth's own tests use.
fn world(seed: u64, defs: &Defs) -> World {
    let field = Grid::from_fn(21, 9, |_| Tile {
        elevation: 100,
        temperature: 128,
        moisture: 128,
        terrain: Terrain::Land,
        biome: BiomeId(0),
        resource: None,
        building: None,
    });
    let mut w = World::with_map(seed, field, Nation::Earth, &[at(10, 4)], defs);
    // Frozen in place, so the settler can't wander and nothing but draw changes.
    w.units[0].idle_until = u64::MAX;
    w
}

fn place(w: &mut World, defs: &Defs, pos: Pos, building: &str) {
    let id = defs.building_id(building).unwrap();
    w.put_building(Footprint::single(pos), id, defs);
}

fn stock(w: &mut World, defs: &Defs, pos: Pos, item: &str, count: u32) {
    let item = defs.item_id(item).unwrap();
    w.stored.insert(pos, Stack { item, count });
}

fn term(r: &DrawReport, kind: DrawTermKind) -> DrawTerm {
    *r.terms.iter().find(|t| t.kind == kind).unwrap()
}

fn run(w: &mut World, defs: &Defs, ticks: u64) {
    for _ in 0..ticks {
        w.tick(defs);
    }
}

#[test]
fn a_bare_settlement_draws_at_base() {
    let defs = Defs::bundled();
    let mut w = world(1, &defs);
    let r = w.draw_multiplier(&defs);
    assert_eq!(r.milli, 600);
    assert!(!r.capped);
    assert!(term(&r, DrawTermKind::Base).is_met());
    for kind in [DrawTermKind::FoodKinds, DrawTermKind::WarmBeds, DrawTermKind::SpareBeds] {
        assert!(!term(&r, kind).is_met(), "{kind:?} is missing");
    }
    // One minute at 60% of one newcomer per 12 minutes: 0.05 of a newcomer.
    run(&mut w, &defs, 60 * u64::from(TICKS_PER_SECOND));
    assert_eq!(w.draw(), 50);
}

#[test]
fn each_bonus_raises_the_multiplier_and_the_cap_holds() {
    let mut defs = Defs::bundled();
    let mut w = world(1, &defs);

    // A second and third food kind with at least 10 stored each; a fourth stack under 10 doesn't
    // add a kind, and one kind alone counts for nothing.
    stock(&mut w, &defs, at(0, 0), "fish", 40);
    assert_eq!(w.draw_multiplier(&defs).milli, 600);
    stock(&mut w, &defs, at(1, 0), "fruit", 10);
    assert_eq!(w.draw_multiplier(&defs).milli, 800);
    stock(&mut w, &defs, at(2, 0), "cabbage", 9);
    assert_eq!(w.draw_multiplier(&defs).milli, 800, "9 cabbage isn't a kind yet");
    stock(&mut w, &defs, at(2, 0), "cabbage", 10);
    let r = w.draw_multiplier(&defs);
    assert_eq!(r.milli, 1000);
    assert!(term(&r, DrawTermKind::FoodKinds).is_met());

    // Spare beds: one settler, two beds is one free bed, which is no spare yet; each bed past
    // that adds 0.1, up to two.
    place(&mut w, &defs, at(4, 2), "bed");
    place(&mut w, &defs, at(5, 2), "bed");
    assert_eq!(w.draw_multiplier(&defs).milli, 1000);
    place(&mut w, &defs, at(6, 2), "bed");
    assert_eq!(w.draw_multiplier(&defs).milli, 1100);
    place(&mut w, &defs, at(7, 2), "bed");
    place(&mut w, &defs, at(8, 2), "bed");
    let r = w.draw_multiplier(&defs);
    assert_eq!(r.milli, 1200, "only two spare beds count");
    assert_eq!(term(&r, DrawTermKind::SpareBeds).count, 2);

    // Warm beds: a campfire among them. A bed out of its reach would spoil it.
    place(&mut w, &defs, at(6, 3), "campfire");
    assert!((4..=8).all(|x| w.is_heated(at(x, 2))));
    let r = w.draw_multiplier(&defs);
    assert_eq!(r.milli, 1400);
    assert!(r.terms.iter().all(DrawTerm::is_met));
    assert!(!r.capped, "1.4 is exactly the cap");
    place(&mut w, &defs, at(20, 8), "bed");
    assert!(!w.is_heated(at(20, 8)));
    assert!(!term(&w.draw_multiplier(&defs), DrawTermKind::WarmBeds).is_met());

    // The cap: with every bonus met, a lower `max` cuts the sum.
    defs.needs.growth.draw.max = 1.0;
    let r = w.draw_multiplier(&defs);
    assert_eq!(r.milli, 1000);
    assert!(r.capped);
}

#[test]
fn draw_stops_at_one_without_a_bed_and_a_late_bed_brings_one_newcomer() {
    let defs = Defs::bundled();
    let mut w = world(1, &defs);
    stock(&mut w, &defs, at(0, 0), "fish", defs.needs.growth.food_per_newcomer * 4);
    w.draw = 990;
    run(&mut w, &defs, 600);
    assert_eq!(w.draw(), DRAW_ONE, "it stops at 1.0 with no bed");
    assert_eq!(w.arrivals().count(), 0);

    // Three beds at once: room for two newcomers, but draw holds only one.
    for x in [4, 5, 6] {
        place(&mut w, &defs, at(x, 2), "bed");
    }
    run(&mut w, &defs, 60 * u64::from(TICKS_PER_SECOND));
    assert_eq!(w.arrivals().count(), 1, "one newcomer, not several");
    assert!(w.draw() < 200);
}

#[test]
fn add_draw_stops_at_bank_and_arrivals_keep_the_gap() {
    let defs = Defs::bundled();
    let mut w = world(1, &defs);
    w.add_draw(5000, &defs);
    assert_eq!(w.draw(), 2000, "bank is 2.0");

    for x in [4, 5, 6, 7] {
        place(&mut w, &defs, at(x, 2), "bed");
    }
    stock(&mut w, &defs, at(0, 0), "fish", defs.needs.growth.food_per_newcomer * 4);
    run(&mut w, &defs, 5 * 60 * u64::from(TICKS_PER_SECOND));
    let ticks: Vec<u64> = w.arrivals().map(|a| a.tick).collect();
    assert_eq!(ticks.len(), 2, "two newcomers from the grant: {ticks:?}");
    let gap = (defs.needs.growth.draw.min_gap_minutes * 60.0) as u64 * u64::from(TICKS_PER_SECOND);
    assert_eq!(ticks[1] - ticks[0], gap, "the second waits out the gap");
    assert_eq!(w.last_arrival(), Some(ticks[1]));
}

#[test]
fn two_runs_from_one_seed_give_the_same_arrival_ticks() {
    let defs = Defs::bundled();
    let setup = || {
        let mut w = world(7, &defs);
        for x in [4, 5, 6, 7, 8] {
            place(&mut w, &defs, at(x, 2), "bed");
        }
        stock(&mut w, &defs, at(0, 0), "fish", 200);
        stock(&mut w, &defs, at(1, 0), "fruit", 50);
        w.draw = 900;
        w
    };
    let (mut a, mut b) = (setup(), setup());
    run(&mut a, &defs, 6000);
    run(&mut b, &defs, 6000);
    let ticks = |w: &World| w.arrivals().map(|x| x.tick).collect::<Vec<_>>();
    assert!(!ticks(&a).is_empty());
    assert_eq!(ticks(&a), ticks(&b));
    assert_eq!(a.draw(), b.draw());
}

// --- Steered draw (#761) ---

/// A world with beds and food to spare, so only draw decides when a newcomer joins.
fn roomy(seed: u64, defs: &Defs) -> World {
    let mut w = world(seed, defs);
    for x in [3, 4, 5, 6, 7, 8] {
        place(&mut w, defs, at(x, 2), "bed");
    }
    stock(&mut w, defs, at(0, 0), "fish", defs.needs.growth.food_per_newcomer * 20);
    w
}

fn newcomer_nations(w: &World) -> Vec<Nation> {
    w.units.iter().filter(|u| u.is_settler()).skip(1).map(|u| w.nation_of(u)).collect()
}

#[test]
fn steered_draw_brings_a_newcomer_of_that_nation_in_order() {
    let defs = Defs::bundled();
    let mut w = roomy(1, &defs);
    w.add_steered_draw(Nation::Fire, 1000, &defs);
    w.add_steered_draw(Nation::Air, 1000, &defs);
    assert_eq!(w.steered_draw(), [(Nation::Fire, 1000), (Nation::Air, 1000)]);
    assert_eq!(w.draw(), 2000);
    run(&mut w, &defs, 5 * 60 * u64::from(TICKS_PER_SECOND));
    assert_eq!(newcomer_nations(&w), [Nation::Fire, Nation::Air], "in the order granted");
    assert!(w.steered_draw().is_empty(), "all of it spent");
}

#[test]
fn grants_of_one_nation_join_and_an_arrival_spends_from_the_front() {
    let defs = Defs::bundled();
    let mut w = roomy(1, &defs);
    w.add_steered_draw(Nation::Water, 500, &defs);
    w.add_steered_draw(Nation::Water, 250, &defs);
    w.add_steered_draw(Nation::Fire, 250, &defs);
    w.add_steered_draw(Nation::Air, 250, &defs);
    assert_eq!(
        w.steered_draw(),
        [(Nation::Water, 750), (Nation::Fire, 250), (Nation::Air, 250)],
        "two grants of one caravan in a row are one run"
    );
    // The arrival is Water's and spends 1000 from the front: Water's 750 and Fire's 250.
    w.tick = 0;
    w.tick_draw(&defs);
    assert_eq!(newcomer_nations(&w), [Nation::Water]);
    assert_eq!(w.steered_draw(), [(Nation::Air, 250)]);
    assert_eq!(w.draw(), 250);
}

#[test]
fn steered_draw_never_passes_the_bank() {
    let defs = Defs::bundled();
    let mut w = world(1, &defs);
    w.add_steered_draw(Nation::Fire, 1500, &defs);
    w.add_steered_draw(Nation::Air, 1500, &defs);
    assert_eq!(w.draw(), 2000, "bank is 2.0");
    assert_eq!(w.steered_draw(), [(Nation::Fire, 1500), (Nation::Air, 500)]);
    // A grant with the bank full steers nothing.
    w.add_steered_draw(Nation::Water, 1000, &defs);
    assert_eq!(w.steered_draw(), [(Nation::Fire, 1500), (Nation::Air, 500)]);
    // Plain draw is unsteered.
    let mut w = world(1, &defs);
    w.add_draw(1000, &defs);
    assert!(w.steered_draw().is_empty());
}

#[test]
fn a_steer_waits_until_someone_can_join() {
    let defs = Defs::bundled();
    // No bed, so nobody joins: the steer waits.
    let mut w = world(1, &defs);
    w.add_steered_draw(Nation::Fire, 1000, &defs);
    run(&mut w, &defs, 60 * u64::from(TICKS_PER_SECOND));
    assert_eq!(w.arrivals().count(), 0);
    assert_eq!(w.steered_draw(), [(Nation::Fire, 1000)]);
}

#[test]
fn a_steered_arrival_leaves_the_weighted_stream_alone() {
    let defs = Defs::bundled();
    let mut steered = roomy(3, &defs);
    steered.endeavor = EndeavorState::Complete { tick: 0 };
    steered.add_steered_draw(Nation::Air, 1000, &defs);
    let rng_before = steered.newcomer_rng.clone();
    run(&mut steered, &defs, 60 * u64::from(TICKS_PER_SECOND));
    assert_eq!(newcomer_nations(&steered), [Nation::Air]);
    assert_eq!(steered.newcomer_rng, rng_before);

    // The same grant as plain draw is drawn by weight, which does use the stream.
    let mut plain = roomy(3, &defs);
    plain.endeavor = EndeavorState::Complete { tick: 0 };
    plain.add_draw(1000, &defs);
    run(&mut plain, &defs, 60 * u64::from(TICKS_PER_SECOND));
    assert_eq!(plain.arrivals().count(), 1);
    assert_ne!(plain.newcomer_rng, rng_before);
}

#[test]
fn steered_draw_is_deterministic_and_survives_a_save() {
    let defs = Defs::bundled();
    let setup = || {
        let mut w = roomy(5, &defs);
        w.endeavor = EndeavorState::Complete { tick: 0 };
        w.add_steered_draw(Nation::Fire, 1000, &defs);
        w.add_steered_draw(Nation::Water, 1000, &defs);
        w
    };
    let (mut a, mut b) = (setup(), setup());
    // Saved after the first arrival, with the second still queued, then resumed.
    run(&mut a, &defs, 100);
    assert_eq!(a.steered_draw(), [(Nation::Water, 1000)]);
    let mut c = World::load(&a.save(&defs).unwrap(), &defs).unwrap();
    assert_eq!(c.steered_draw(), a.steered_draw());
    run(&mut b, &defs, 5000);
    run(&mut a, &defs, 4900);
    run(&mut c, &defs, 4900);
    assert_eq!(newcomer_nations(&b), [Nation::Fire, Nation::Water]);
    assert_eq!(newcomer_nations(&a), newcomer_nations(&b));
    assert_eq!(newcomer_nations(&c), newcomer_nations(&b));
    assert_eq!(a.save(&defs).unwrap(), c.save(&defs).unwrap());
}
