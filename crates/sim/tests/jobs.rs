//! The job system on generated maps: survival without fire (owner requirement from #3) and
//! determinism.

use fn_sim::world::{Command, Task};
use fn_sim::{Defs, MapGenConfig, Nation, Pos, World};

const MINUTE: u64 = 600; // ticks

fn world(nation: Nation, seed: u64) -> (World, Defs) {
    let defs = Defs::bundled();
    let cfg = MapGenConfig { seed, ..Default::default() };
    (World::new(&cfg, nation, 4, &defs), defs)
}

/// How many of a unit's tiles away the nearest cabbage is, for diagnosing a failing seed.
fn cabbages(w: &World, defs: &Defs) -> usize {
    let cabbage = defs.resource_id("cabbage").unwrap();
    w.map.iter().filter(|(_, t)| t.resource == Some(cabbage)).count()
}

/// Nobody may starve because their nation lacks fire: a settlement with no cooking and no
/// designations at all, only wild food within reach, lives through its first in-game hour.
#[test]
fn nations_without_fire_survive_the_first_hour_on_raw_food() {
    for nation in [Nation::Earth, Nation::Water, Nation::Air] {
        let (mut w, defs) = world(nation, 7);
        let fire = defs.tech_id("fire_making").unwrap();
        assert!(!w.research().is_known(fire), "{nation:?} must start without fire");
        let mut ate = 0;
        let mut was_hungry = [false; 4];
        for _ in 0..60 * MINUTE {
            w.tick(&defs);
            for u in &w.units {
                let hungry = matches!(u.task(), Some(Task::Eat(_)));
                if was_hungry[u.id.0 as usize] && !hungry {
                    ate += 1;
                }
                was_hungry[u.id.0 as usize] = hungry;
            }
        }
        assert!(
            w.deaths.is_empty(),
            "{nation:?}: {:?} ({} cabbages)",
            w.deaths,
            cabbages(&w, &defs)
        );
        // An hour is 240 hunger a unit; one cabbage is 30. Everyone ate repeatedly.
        assert!(ate >= 4 * 5, "{nation:?}: only {ate} meals");
        for u in &w.units {
            assert!(u.needs.health >= 50.0, "{nation:?} {}: {:?}", u.name, u.needs);
        }
    }
}

/// Same seed, same designations: same world, tick for tick.
#[test]
fn the_job_system_is_deterministic() {
    let run = || {
        let (mut w, defs) = world(Nation::Earth, 3);
        let center = Pos::new(w.map.width() / 2, w.map.height() / 2);
        // Designate every harvestable tree and rock within a few tiles of the start.
        let near: Vec<Pos> = w
            .map
            .iter()
            .filter(|(p, t)| {
                let d = (p.x - center.x).abs().max((p.y - center.y).abs());
                d <= 12 && t.resource.is_some_and(|r| defs.harvest(r).is_some())
            })
            .map(|(p, _)| p)
            .collect();
        assert!(!near.is_empty());
        for p in &near {
            w.apply(Command::DesignateHarvest { pos: *p }, &defs).unwrap();
        }
        // A stockpile on the open tiles around the first unit.
        let home = w.units[0].pos;
        for dy in -3..=3 {
            for dx in -3..=3 {
                let pos = Pos::new(home.x + dx, home.y + dy);
                let _ = w.apply(Command::DesignateStockpile { pos }, &defs);
            }
        }
        assert!(w.stockpile_zones().count() >= 3);
        for _ in 0..15 * MINUTE {
            w.tick(&defs);
        }
        let units: Vec<_> =
            w.units.iter().map(|u| (u.pos, u.needs, u.carrying, u.task())).collect();
        let stored: Vec<_> = defs
            .items
            .iter()
            .enumerate()
            .map(|(i, _)| w.stored_total(fn_sim::jobs::ItemId(i as u16)))
            .collect();
        (units, stored, w.jobs().to_vec(), w.deaths.clone())
    };
    let (a, b) = (run(), run());
    assert_eq!(a, b);
    assert!(a.1.iter().any(|&n| n > 0), "nothing was stockpiled in 15 minutes");
}
