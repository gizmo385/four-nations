//! Player commands (`World::apply`) and session replay.

use fn_sim::jobs::ItemId;
use fn_sim::world::{Command, CommandError, DesignateError, Recorded};
use fn_sim::{Defs, MapGenConfig, Nation, Pos, World};

fn fresh(defs: &Defs) -> World {
    let cfg = MapGenConfig { seed: 11, ..Default::default() };
    World::new(&cfg, Nation::Water, 4, defs)
}

/// Tiles within `radius` of `center` that the settlement can designate for harvest right now
/// (resources with a harvest entry whose tech, if any, is known), in map order.
fn harvestable(w: &World, defs: &Defs, center: Pos, radius: i32) -> Vec<Pos> {
    w.map
        .iter()
        .filter(|(p, t)| {
            (p.x - center.x).abs().max((p.y - center.y).abs()) <= radius
                && t.resource
                    .and_then(|r| defs.harvest(r))
                    .is_some_and(|h| w.is_unlocked(h.unlocked_by))
        })
        .map(|(p, _)| p)
        .collect()
}

/// Everything observable about a world through its public API.
fn snapshot(w: &World, defs: &Defs) -> String {
    let units: Vec<_> =
        w.units.iter().map(|u| (u.id, u.pos, u.needs, u.carrying, u.task())).collect();
    let stored: Vec<u32> =
        (0..defs.items.len()).map(|i| w.stored_total(ItemId(i as u16))).collect();
    let ground: Vec<_> = w.ground_items().collect();
    let zones: Vec<_> = w.stockpile_zones().collect();
    let buildings: Vec<_> =
        w.map.iter().filter_map(|(p, t)| t.building.map(|b| (p, b.id))).collect();
    let sites: Vec<_> = w.build_sites().collect();
    format!(
        "tick {} {units:?} {:?} {stored:?} {ground:?} {zones:?} {buildings:?} {sites:?} {:?} {:?}",
        w.tick,
        w.jobs(),
        w.deaths,
        w.research(),
    )
}

#[test]
fn applied_commands_change_the_world_and_are_logged_with_their_tick() {
    let defs = Defs::bundled();
    let mut w = fresh(&defs);
    let center = Pos::new(w.map.width() / 2, w.map.height() / 2);
    let target = harvestable(&w, &defs, center, 20)[0];
    for _ in 0..7 {
        w.tick(&defs);
    }
    w.apply(Command::DesignateHarvest { pos: target }, &defs).unwrap();
    assert_eq!(w.jobs().len(), 1);
    assert_eq!(
        w.command_log(),
        [Recorded { tick: 7, command: Command::DesignateHarvest { pos: target } }]
    );

    w.apply(Command::CancelHarvest { pos: target }, &defs).unwrap();
    assert!(w.jobs().is_empty());
    let tech = defs.tech_id("fire_making").unwrap();
    w.apply(Command::SetResearch { tech }, &defs).unwrap();
    assert_eq!(w.research().queue(), [tech]);
    assert_eq!(w.command_log().len(), 3);
}

#[test]
fn designate_repeating_harvest_is_logged_and_cleared_by_cancel() {
    let defs = Defs::bundled();
    let mut w = fresh(&defs);
    let center = Pos::new(w.map.width() / 2, w.map.height() / 2);
    let target = harvestable(&w, &defs, center, 20)[0];
    w.apply(Command::DesignateRepeatingHarvest { pos: target }, &defs).unwrap();
    assert_eq!(w.jobs().len(), 1);
    assert!(w.is_repeating_harvest(target));
    assert_eq!(
        w.command_log(),
        [Recorded { tick: 0, command: Command::DesignateRepeatingHarvest { pos: target } }]
    );

    // Cancelling drops the job and the standing order together.
    w.apply(Command::CancelHarvest { pos: target }, &defs).unwrap();
    assert!(w.jobs().is_empty());
    assert!(!w.is_repeating_harvest(target));
}

#[test]
fn cancel_stockpile_is_logged_and_rejected_where_there_is_no_zone() {
    let defs = Defs::bundled();
    let mut w = fresh(&defs);
    let pos = free_tile_near(&w, Pos::new(w.map.width() / 2, w.map.height() / 2), &[]);
    let cancel = Command::CancelStockpile { pos };
    assert_eq!(w.apply(cancel, &defs), Err(CommandError::Designate(DesignateError::NotDesignated)));
    w.apply(Command::DesignateStockpile { pos }, &defs).unwrap();
    assert_eq!(w.stockpile_zones().collect::<Vec<_>>(), [pos]);
    w.apply(cancel, &defs).unwrap();
    assert_eq!(w.stockpile_zones().count(), 0);
    // Only the accepted commands are logged.
    let logged: Vec<_> = w.command_log().iter().map(|r| r.command).collect();
    assert_eq!(logged, [Command::DesignateStockpile { pos }, cancel]);
}

#[test]
fn rejected_commands_change_nothing_and_are_not_logged() {
    let defs = Defs::bundled();
    let mut w = fresh(&defs);
    let before = snapshot(&w, &defs);
    let off_map = Pos::new(-1, 0);
    let lightning = defs.tech_id("lightning_generation").unwrap();
    for (command, error) in [
        (
            Command::DesignateHarvest { pos: off_map },
            CommandError::Designate(DesignateError::OutOfBounds),
        ),
        (
            Command::DesignateRepeatingHarvest { pos: off_map },
            CommandError::Designate(DesignateError::OutOfBounds),
        ),
        (
            Command::CancelHarvest { pos: Pos::new(0, 0) },
            CommandError::Designate(DesignateError::NotDesignated),
        ),
        (
            Command::DesignateStockpile { pos: off_map },
            CommandError::Designate(DesignateError::OutOfBounds),
        ),
        // Lightning is the fire nation's tech.
        (
            Command::SetResearch { tech: lightning },
            CommandError::Research(fn_sim::research::ResearchError::WrongNation),
        ),
    ] {
        assert_eq!(w.apply(command, &defs), Err(error), "{command:?}");
    }
    assert_eq!(snapshot(&w, &defs), before);
    assert!(w.command_log().is_empty());

    // Designating twice: the second is refused and not logged.
    let center = Pos::new(w.map.width() / 2, w.map.height() / 2);
    let target = harvestable(&w, &defs, center, 20)[0];
    w.apply(Command::DesignateHarvest { pos: target }, &defs).unwrap();
    assert_eq!(
        w.apply(Command::DesignateHarvest { pos: target }, &defs),
        Err(CommandError::Designate(DesignateError::AlreadyDesignated))
    );
    assert_eq!(w.command_log().len(), 1);
}

/// A walkable, resource-free tile near `near` that isn't in `avoid`, for a build designation.
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

/// Plays a scripted session: commands at several ticks, including a cancel and a research
/// change, and a building (a bed) put up from wood the harvests bring in.
fn play_session(defs: &Defs) -> World {
    let mut w = fresh(defs);
    let center = Pos::new(w.map.width() / 2, w.map.height() / 2);
    let targets = harvestable(&w, defs, center, 14);
    assert!(targets.len() >= 6, "seed has too little to harvest near the start");
    let home = w.units[0].pos;
    let bed = defs.building_id("bed").expect("starter building");
    let build_pos = free_tile_near(&w, home, &targets);
    for t in 0..9000u64 {
        match t {
            0 => {
                for p in &targets[..4] {
                    w.apply(Command::DesignateHarvest { pos: *p }, defs).unwrap();
                }
                for dy in -2..=2 {
                    for dx in -2..=2 {
                        // Some of these tiles aren't open ground; those are simply refused.
                        let pos = Pos::new(home.x + dx, home.y + dy);
                        let _ = w.apply(Command::DesignateStockpile { pos }, defs);
                    }
                }
                w.apply(Command::DesignateBuild { pos: build_pos, building: bed }, defs).unwrap();
            }
            300 => {
                w.apply(Command::CancelHarvest { pos: targets[1] }, defs).unwrap();
                for p in &targets[4..] {
                    w.apply(Command::DesignateHarvest { pos: *p }, defs).unwrap();
                }
            }
            1200 => {
                let tech = defs.tech_id("water_travel").unwrap();
                w.apply(Command::SetResearch { tech }, defs).unwrap();
            }
            _ => {}
        }
        w.tick(defs);
    }
    w
}

#[test]
fn replaying_the_command_log_reproduces_the_session() {
    let defs = Defs::bundled();
    let original = play_session(&defs);
    let log = original.command_log().to_vec();
    assert!(log.len() > 6);
    assert!(log.windows(2).all(|w| w[0].tick <= w[1].tick));
    assert!(original.stockpile_zones().count() > 0);
    assert!(
        (0..defs.items.len()).any(|i| original.stored_total(ItemId(i as u16)) > 0),
        "the session should have stockpiled something, or the test proves little"
    );
    let bed = defs.building_id("bed").unwrap();
    assert!(
        original.map.iter().any(|(_, t)| t.building.is_some_and(|b| b.id == bed)),
        "the designated bed should have been built during the session"
    );

    let mut replayed = fresh(&defs);
    replayed.replay(&log, original.tick, &defs);
    assert_eq!(snapshot(&replayed, &defs), snapshot(&original, &defs));
    assert_eq!(replayed.command_log(), original.command_log());
}

#[test]
fn replay_with_a_different_log_diverges() {
    // Guards the test above against comparing two worlds that never did anything.
    let defs = Defs::bundled();
    let original = play_session(&defs);
    let mut log = original.command_log().to_vec();
    log.retain(|r| !matches!(r.command, Command::DesignateStockpile { .. }));
    let mut replayed = fresh(&defs);
    replayed.replay(&log, original.tick, &defs);
    assert_ne!(snapshot(&replayed, &defs), snapshot(&original, &defs));
}
