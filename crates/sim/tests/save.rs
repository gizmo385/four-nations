//! Saving and loading (#7): a loaded world is the saved world, and carries on identically.

use fn_sim::world::Command;
use fn_sim::{Defs, MapGenConfig, Nation, Pos, SAVE_VERSION, SaveError, World};

/// A seed whose start area lets the Fire session in `play` keep a Hearthflame burning at tick
/// 1500 (it is seed-dependent). Map generation changes (#24, #225) reshuffle the start, so if
/// `the_saved_session_has_the_state_the_tests_need_to_cover` fails, try the next seed. Re-picked
/// by #383: gold veins no longer count toward `targets` (gold mining needs `prospecting` now),
/// which left seed 14's start without a harvest job still pending at tick 1500.
/// Re-picked by #582: settlers now spawn in one connected region, which moved the start of seed 6 (and others).
const SEED: u64 = 18;

fn fresh(nation: Nation, defs: &Defs) -> World {
    let cfg = MapGenConfig { seed: SEED, ..Default::default() };
    World::new(&cfg, nation, 4, defs)
}

/// A walkable, resource-free tile near `near` that isn't in `avoid`.
fn free_tile_near(w: &World, near: Pos, avoid: &[Pos]) -> Pos {
    w.map
        .iter()
        .filter(|(p, t)| {
            t.terrain.is_walkable()
                && t.resource.is_none()
                && (p.x - near.x).abs().max((p.y - near.y).abs()) <= 8
        })
        .map(|(p, _)| p)
        .find(|p| !avoid.contains(p))
        .expect("a free tile near home")
}

/// Plays `ticks` ticks of a busy session: harvests, a stockpile, a building under way, research,
/// and a few of the nation's bends, so most kinds of state are alive at the end.
fn play(nation: Nation, ticks: u64, defs: &Defs) -> World {
    let mut w = fresh(nation, defs);
    let center = Pos::new(w.map.width() / 2, w.map.height() / 2);
    let targets: Vec<Pos> = w
        .map
        .iter()
        .filter(|(p, t)| {
            (p.x - center.x).abs().max((p.y - center.y).abs()) <= 14
                && t.resource
                    .and_then(|r| defs.harvest(r))
                    .is_some_and(|h| w.is_unlocked(h.unlocked_by))
        })
        .map(|(p, _)| p)
        .collect();
    assert!(targets.len() >= 6, "seed has too little to harvest near the start");
    let home = w.units[0].pos;
    let bed = defs.building_id("bed").expect("starter building");
    let build_pos = free_tile_near(&w, home, &targets);
    let ability = defs
        .abilities
        .iter()
        .position(|a| a.nation == nation)
        .map(|i| fn_sim::bending::AbilityId(i as u16))
        .expect("every nation has an ability");

    for t in 0..ticks {
        match t {
            0 => {
                for p in &targets[..4] {
                    w.apply(Command::DesignateHarvest { pos: *p }, defs).unwrap();
                }
                w.apply(Command::DesignateRepeatingHarvest { pos: targets[5] }, defs).unwrap();
                for dy in -2..=2 {
                    for dx in -2..=2 {
                        let pos = Pos::new(home.x + dx, home.y + dy);
                        let _ = w.apply(Command::DesignateStockpile { pos }, defs);
                    }
                }
                w.apply(Command::DesignateBuild { pos: build_pos, building: bed }, defs).unwrap();
                let mut bends = 0;
                for dy in -6..=6 {
                    for dx in -6..=6 {
                        let pos = Pos::new(home.x + dx, home.y + dy);
                        if bends < 2 && w.apply(Command::Bend { ability, pos }, defs).is_ok() {
                            bends += 1;
                        }
                    }
                }
            }
            200 => {
                let tech = defs.tech_id("fire_making").unwrap();
                let _ = w.apply(Command::SetResearch { tech }, defs);
            }
            _ => {}
        }
        w.tick(defs);
    }
    w
}

#[test]
fn a_saved_world_loads_back_identically() {
    let defs = Defs::bundled();
    for nation in Nation::ALL {
        let w = play(nation, 1500, &defs);
        let text = w.save(&defs).unwrap();
        let loaded = World::load(&text, &defs).unwrap();
        // Saving is deterministic and covers all state, so equal text means equal worlds.
        assert_eq!(loaded.save(&defs).unwrap(), text, "{nation:?}");
        assert_eq!(loaded.tick, w.tick);
        assert_eq!(loaded.seed, w.seed);
        assert_eq!(loaded.nation, w.nation);
        assert_eq!(loaded.map, w.map);
        assert_eq!(loaded.command_log(), w.command_log());
        assert_eq!(loaded.research(), w.research());
        assert_eq!(loaded.units, w.units);
        assert_eq!(loaded.tallies(), w.tallies(), "{nation:?}");
    }
}

#[test]
fn the_saved_session_has_the_state_the_tests_need_to_cover() {
    // Guards the round-trip tests against saving worlds that never did anything.
    let defs = Defs::bundled();
    let w = play(Nation::Fire, 1500, &defs);
    assert!(!w.command_log().is_empty());
    assert!(w.stockpile_zones().count() > 0);
    assert!(!w.jobs().is_empty());
    assert!(!w.flames().is_empty(), "the Hearthflame should be burning");
    // Lifetime counts (#233) that the round trip has to keep.
    assert!(w.tallies().harvested_items().count() > 0, "something should have been harvested");
    assert!(w.units.iter().any(|u| u.bends() > 0), "the Hearthflame counts as a finished bend");
    // Units mid-task (and carrying things) at some point along the way, so saves taken then
    // exercise routes and claimed jobs.
    let busy = |ticks| {
        let w = play(Nation::Fire, ticks, &defs);
        w.units.iter().any(|u| u.task().is_some() || u.carrying.is_some())
    };
    assert!([50, 300, 700].into_iter().any(busy));
}

#[test]
fn a_loaded_world_carries_on_exactly_like_the_original() {
    let defs = Defs::bundled();
    for nation in Nation::ALL {
        let mut original = play(nation, 1500, &defs);
        let mut loaded = World::load(&original.save(&defs).unwrap(), &defs).unwrap();
        // Same new orders to both, then the same stretch of time: any state the save lost (an rng
        // position, a route, a counter) would make them drift apart.
        let center = Pos::new(original.map.width() / 2, original.map.height() / 2);
        let zone = free_tile_near(&original, center, &[]);
        for w in [&mut original, &mut loaded] {
            w.apply(Command::DesignateStockpile { pos: zone }, &defs).unwrap();
            for _ in 0..4000 {
                w.tick(&defs);
            }
        }
        assert_eq!(loaded.save(&defs).unwrap(), original.save(&defs).unwrap(), "{nation:?}");
    }
}

#[test]
fn a_loaded_world_can_be_saved_and_loaded_again() {
    let defs = Defs::bundled();
    let w = play(Nation::Water, 800, &defs);
    let once = World::load(&w.save(&defs).unwrap(), &defs).unwrap();
    let twice = World::load(&once.save(&defs).unwrap(), &defs).unwrap();
    assert_eq!(twice.save(&defs).unwrap(), w.save(&defs).unwrap());
}

#[test]
fn a_save_after_deaths_keeps_them() {
    let defs = Defs::bundled();
    let mut w = fresh(Nation::Earth, &defs);
    let id = w.units[1].id;
    w.units[1].needs.hunger = 0.0;
    w.units[1].needs.health = 0.01;
    w.tick(&defs);
    assert_eq!(w.deaths.len(), 1);
    let loaded = World::load(&w.save(&defs).unwrap(), &defs).unwrap();
    assert_eq!(loaded.deaths, w.deaths);
    assert!(loaded.unit(id).is_none());
    assert_eq!(loaded.units.len(), 3);
}

#[test]
fn inspect_reads_the_header_without_the_world() {
    let defs = Defs::bundled();
    let w = play(Nation::Air, 300, &defs);
    let info = fn_sim::save::inspect(&w.save(&defs).unwrap()).unwrap();
    assert_eq!(info.version, SAVE_VERSION);
    assert_eq!((info.seed, info.nation, info.tick), (SEED, Nation::Air, 300));
}

#[test]
fn text_that_is_not_a_save_is_refused() {
    let defs = Defs::bundled();
    for text in ["", "hello", "()", "(a: 1)", "[[ability]]\nid = \"x\""] {
        assert!(
            matches!(World::load(text, &defs), Err(SaveError::NotASave(_))),
            "{text:?} should not be a save"
        );
    }
}

#[test]
fn another_format_version_is_refused_before_the_body_is_read() {
    let defs = Defs::bundled();
    let text = fresh(Nation::Fire, &defs).save(&defs).unwrap();
    let marker = format!("version:{SAVE_VERSION}");
    assert!(text.contains(&marker), "unexpected header layout: {}", &text[..120]);
    // The body can be anything: a future format is refused on its version alone.
    let future = format!(
        "(magic:\"four-nations-save\",version:{},data_hash:0,seed:1,nation:fire,tick:0)\n(nonsense)",
        SAVE_VERSION + 1
    );
    assert_eq!(
        World::load(&future, &defs).err(),
        Some(SaveError::Version { found: SAVE_VERSION + 1, supported: SAVE_VERSION })
    );
}

#[test]
fn a_save_from_different_game_data_is_refused() {
    let defs = Defs::bundled();
    let text = fresh(Nation::Fire, &defs).save(&defs).unwrap();
    let tampered = text.replacen(&format!("data_hash:{}", defs.id_hash()), "data_hash:1", 1);
    assert_ne!(tampered, text);
    assert_eq!(World::load(&tampered, &defs).err(), Some(SaveError::DataMismatch));
}

#[test]
fn retuning_balance_numbers_does_not_change_the_data_hash() {
    // Saves survive rebalancing; only the id tables matter.
    let defs = Defs::bundled();
    let mut tuned = defs.clone();
    tuned.chi.max += 1.0;
    tuned.techs[0].cost += 1;
    assert_eq!(tuned.id_hash(), defs.id_hash());
    tuned.techs.pop();
    assert_ne!(tuned.id_hash(), defs.id_hash());
}

#[test]
fn damaged_saves_fail_cleanly_instead_of_panicking() {
    let defs = Defs::bundled();
    let text = play(Nation::Earth, 500, &defs).save(&defs).unwrap();
    // Truncated at a few places, and with a block of the body replaced.
    for cut in [text.len() / 4, text.len() / 2, text.len() - 5] {
        assert!(matches!(World::load(&text[..cut], &defs), Err(SaveError::Corrupt(_))), "{cut}");
    }
    let broken = text.replacen("units:[", "units:[Oops,", 1);
    assert!(matches!(World::load(&broken, &defs), Err(SaveError::Corrupt(_))));
}

#[test]
fn a_world_with_an_inconsistent_map_is_refused() {
    let defs = Defs::bundled();
    let text = fresh(Nation::Earth, &defs).save(&defs).unwrap();
    let w = World::load(&text, &defs).unwrap();
    let (width, height) = (w.map.width(), w.map.height());
    let lie = text.replacen(&format!("width:{width}"), &format!("width:{}", width + 1), 1);
    assert_ne!(lie, text);
    assert!(matches!(World::load(&lie, &defs), Err(SaveError::Corrupt(_))), "{height}");
}
