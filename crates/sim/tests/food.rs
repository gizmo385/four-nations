//! Renewable food (#56): fishing, fruit trees and regrowing cabbages keep a settlement fed long
//! past the point where finite wild food ran out (about two in-game hours on seed 7).

use fn_sim::world::Command;
use fn_sim::{Defs, MapGenConfig, Nation, Pos, World};

const MINUTE: u64 = 600; // ticks
const HOUR: u64 = 60 * MINUTE;

fn world(nation: Nation, seed: u64) -> (World, Defs) {
    let defs = Defs::bundled();
    let cfg = MapGenConfig { seed, ..Default::default() };
    (World::new(&cfg, nation, 4, &defs), defs)
}

/// Designates every food source within `radius` tiles of `home` that the nation can work and
/// that isn't already designated.
fn designate_food(w: &mut World, defs: &Defs, home: Pos, radius: i32) {
    let spots: Vec<Pos> = w
        .map
        .iter()
        .filter(|(p, t)| {
            let near = (p.x - home.x).abs().max((p.y - home.y).abs()) <= radius;
            near && t
                .resource
                .and_then(|r| defs.harvest(r))
                .is_some_and(|h| defs.food_of(h.item).is_some())
        })
        .map(|(p, _)| p)
        .collect();
    for pos in spots {
        // Resting plants queue up; fish for nations without fishing are refused.
        let _ = w.apply(Command::DesignateHarvest { pos }, defs);
    }
}

/// A non-Fire settlement that designates food work and keeps a stockpile eats for hours. (Fire is
/// left out on purpose: wild food alone has to carry the nations without cooking.) One test per
/// nation so the harness runs these long soaks in parallel.
#[test]
fn designated_food_work_feeds_earth_for_hours() {
    designated_food_work_feeds_a_settlement_for_hours(Nation::Earth);
}

#[test]
fn designated_food_work_feeds_water_for_hours() {
    designated_food_work_feeds_a_settlement_for_hours(Nation::Water);
}

#[test]
fn designated_food_work_feeds_air_for_hours() {
    designated_food_work_feeds_a_settlement_for_hours(Nation::Air);
}

fn designated_food_work_feeds_a_settlement_for_hours(nation: Nation) {
    let (mut w, defs) = world(nation, 7);
    let home = w.units[0].pos;
    for dy in -2..=2 {
        for dx in -2..=2 {
            let pos = Pos::new(home.x + dx, home.y + dy);
            let _ = w.apply(Command::DesignateStockpile { pos }, &defs);
        }
    }
    let food: Vec<_> = defs.foods.iter().map(|f| defs.item_id(&f.id).unwrap()).collect();
    let mut stocked = 0;
    for minute in 0..3 * 60 {
        designate_food(&mut w, &defs, home, 40);
        for _ in 0..MINUTE {
            w.tick(&defs);
        }
        stocked = stocked.max(food.iter().map(|&f| w.stored_total(f)).sum::<u32>());
        assert!(
            w.deaths().next().is_none(),
            "{nation:?} lost {:?} at minute {minute}",
            w.deaths().collect::<Vec<_>>()
        );
    }
    assert_eq!(w.units.len(), 4);
    assert!(stocked > 0, "{nation:?} never stockpiled any food");
    for u in &w.units {
        assert!(u.needs.health >= 50.0, "{nation:?} {}: {:?}", u.name, u.needs);
    }
}

/// With no designations at all, hungry units forage plants that have regrown, so they live far
/// longer than the two hours wild food used to last.
#[test]
fn foraging_alone_outlasts_the_old_two_hour_limit_earth() {
    foraging_alone_outlasts_the_old_two_hour_limit(Nation::Earth);
}

#[test]
fn foraging_alone_outlasts_the_old_two_hour_limit_water() {
    foraging_alone_outlasts_the_old_two_hour_limit(Nation::Water);
}

fn foraging_alone_outlasts_the_old_two_hour_limit(nation: Nation) {
    let (mut w, defs) = world(nation, 7);
    for _ in 0..3 * HOUR {
        w.tick(&defs);
    }
    assert!(w.deaths().next().is_none(), "{nation:?}: {:?}", w.deaths().collect::<Vec<_>>());
}
