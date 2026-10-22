//! Bending through the world: chi, designation errors, claiming, channeling, events and replay.
//!
//! The real effects aren't built yet, so these use the test-only `probe` effect from
//! `bending::testing`: it accepts walkable ground and does nothing but spend the chi.

use super::*;
use crate::bending::testing::probe_defs;
use crate::defs::{BiomeId, Terrain};
use crate::geom::Grid;
use crate::mapgen::Tile;

fn arena(width: i32, height: i32) -> TileMap {
    Grid::from_fn(width, height, |_| Tile {
        elevation: 100,
        temperature: 128,
        moisture: 128,
        terrain: Terrain::Land,
        biome: BiomeId(0),
        resource: None,
    })
}

fn at(x: i32, y: i32) -> Pos {
    Pos::new(x, y)
}

/// A bare 9x5 field, with `nation` units at `spawns`, and a probe that takes `work` seconds.
fn setup(nation: Nation, spawns: &[Pos], work: f32, extra: &str) -> (World, Defs) {
    let defs = probe_defs(work, extra);
    (World::with_map(1, arena(9, 5), nation, spawns, &defs), defs)
}

fn id(defs: &Defs, ability: &str) -> AbilityId {
    defs.ability_id(ability).unwrap()
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

#[test]
fn units_start_with_full_chi_and_regenerate_awake_and_asleep() {
    let (mut w, defs) = setup(Nation::Earth, &[at(3, 3)], 2.0, "");
    assert_eq!(w.units[0].chi, 100.0);

    w.units[0].chi = 0.0;
    for _ in 0..600 {
        w.tick(&defs);
    }
    assert!((w.units[0].chi - 2.0).abs() < 0.01, "{}", w.units[0].chi);

    // Asleep it regenerates four times as fast (8 a minute).
    w.units[0].chi = 0.0;
    w.units[0].need_task = Some(Task::Sleep);
    w.units[0].needs.energy = 10.0;
    for _ in 0..600 {
        w.tick(&defs);
    }
    assert!(w.units[0].is_sleeping());
    assert!((w.units[0].chi - 8.0).abs() < 0.01, "{}", w.units[0].chi);
}

#[test]
fn the_chi_regen_modifier_scales_regeneration_and_chi_never_passes_max() {
    let (mut w, defs) = setup(Nation::Earth, &[at(3, 3)], 2.0, "");
    w.modifiers.chi_regen = 1.25;
    w.units[0].chi = 0.0;
    for _ in 0..600 {
        w.tick(&defs);
    }
    assert!((w.units[0].chi - 2.5).abs() < 0.01, "{}", w.units[0].chi);
    w.units[0].chi = 99.999;
    for _ in 0..10 {
        w.tick(&defs);
    }
    assert_eq!(w.units[0].chi, 100.0);
}

#[test]
fn bend_commands_that_cannot_be_given_are_refused_and_not_logged() {
    let (mut w, defs) = setup(Nation::Earth, &[at(3, 3)], 2.0, "");
    let probe = id(&defs, "probe");
    w.map[at(8, 4)].terrain = Terrain::Mountain;
    w.map_changed();
    let tech = defs.tech_id("probe_lore").unwrap();
    for (command, error) in [
        (Command::Bend { ability: AbilityId(999), pos: at(1, 1) }, BendError::UnknownAbility),
        (
            Command::Bend { ability: id(&defs, "probe_water"), pos: at(1, 1) },
            BendError::WrongNation,
        ),
        (
            Command::Bend { ability: id(&defs, "probe_locked"), pos: at(1, 1) },
            BendError::Locked { tech },
        ),
        (Command::Bend { ability: probe, pos: at(-1, 0) }, BendError::InvalidTarget("off the map")),
        (
            Command::Bend { ability: probe, pos: at(8, 4) },
            BendError::InvalidTarget("probe needs walkable ground"),
        ),
        (
            // The real abilities refuse every target until their effects land.
            Command::Bend { ability: id(&defs, "raise_earth"), pos: at(1, 1) },
            BendError::InvalidTarget("not implemented yet"),
        ),
        (Command::CancelBend { pos: at(1, 1) }, BendError::NotDesignated),
    ] {
        assert_eq!(w.apply(command, &defs), Err(CommandError::Bend(error)), "{command:?}");
    }
    assert!(w.jobs().is_empty());
    assert!(w.command_log().is_empty());

    w.apply(Command::Bend { ability: probe, pos: at(1, 1) }, &defs).unwrap();
    assert_eq!(
        w.apply(Command::Bend { ability: probe, pos: at(1, 1) }, &defs),
        Err(CommandError::Bend(BendError::AlreadyDesignated))
    );
    assert_eq!(w.bend_designations().collect::<Vec<_>>(), [(probe, at(1, 1))]);
    assert_eq!(w.command_log().len(), 1);
}

#[test]
fn an_ability_unlocks_when_its_tech_is_learned() {
    let (mut w, defs) = setup(Nation::Earth, &[at(3, 3)], 2.0, "");
    let locked = id(&defs, "probe_locked");
    let tech = defs.tech_id("probe_lore").unwrap();
    assert_eq!(w.check_ability(locked, &defs), Err(BendError::Locked { tech }));
    w.set_research(tech, &defs).unwrap();
    // 60 points at 3 a minute is twenty minutes.
    run_until(&mut w, &defs, 12_100, |w| w.research().is_known(tech));
    assert_eq!(w.check_ability(locked, &defs), Ok(()));
    w.apply(Command::Bend { ability: locked, pos: at(1, 1) }, &defs).unwrap();
}

#[test]
fn a_bend_spends_chi_only_when_it_completes() {
    let (mut w, defs) = setup(Nation::Earth, &[at(1, 2)], 2.0, "");
    let probe = id(&defs, "probe");
    w.apply(Command::Bend { ability: probe, pos: at(6, 2) }, &defs).unwrap();
    // While it walks and channels the meter stays full.
    run_until(&mut w, &defs, 400, |w| {
        if w.bend_events().is_empty() {
            assert_eq!(w.units[0].chi, 100.0);
        }
        !w.bend_events().is_empty()
    });
    let [event] = w.bend_events() else { panic!("one event") };
    assert_eq!(
        *event,
        BendEvent {
            tick: w.tick,
            unit: w.units[0].id,
            ability: probe,
            pos: at(6, 2),
            outcome: BendOutcome::Done
        }
    );
    assert_eq!(w.units[0].chi, 60.0);
    assert!(w.jobs().is_empty());
    assert_eq!(w.units[0].task(), None);
    // It stood on the target to do it.
    assert_eq!(w.units[0].pos, at(6, 2));
}

#[test]
fn the_speed_modifier_divides_the_channel_time() {
    let ticks = |extra: &str| {
        let (mut w, defs) = setup(Nation::Earth, &[at(3, 2)], 10.0, extra);
        w.apply(Command::Bend { ability: id(&defs, "probe"), pos: at(3, 2) }, &defs).unwrap();
        let rate = w.modifiers().earthworks;
        (run_until(&mut w, &defs, 1000, |w| !w.bend_events().is_empty()), rate)
    };
    let (plain, rate) = ticks("");
    let (fast, _) = ticks("speed_modifier = 'earthworks'");
    assert_eq!(rate, 2.5);
    // Ten seconds is a hundred ticks; at 2.5 it is forty.
    assert!(plain.abs_diff(100) <= 3, "{plain}");
    assert!(fast.abs_diff(40) <= 3, "{fast}");
}

#[test]
fn a_unit_without_the_chi_waits_until_it_has_enough() {
    let (mut w, defs) = setup(Nation::Earth, &[at(3, 2)], 0.0, "");
    w.apply(Command::Bend { ability: id(&defs, "probe"), pos: at(5, 2) }, &defs).unwrap();
    w.units[0].chi = 39.0;
    for _ in 0..300 {
        w.tick(&defs);
        assert_eq!(w.jobs()[0].claimed_by, None);
    }
    assert!(w.bend_events().is_empty());
    w.units[0].chi = 40.0;
    run_until(&mut w, &defs, 400, |w| !w.bend_events().is_empty());
    assert_eq!(w.bend_events()[0].outcome, BendOutcome::Done);
    assert!(w.units[0].chi < 1.0);
}

#[test]
fn a_unit_of_another_nation_does_not_take_the_job() {
    let (mut w, defs) = setup(Nation::Earth, &[at(3, 2)], 0.0, "");
    w.apply(Command::Bend { ability: id(&defs, "probe"), pos: at(5, 2) }, &defs).unwrap();
    w.units[0].nation = Nation::Water;
    for _ in 0..200 {
        w.tick(&defs);
    }
    assert_eq!(w.jobs().len(), 1);
    assert_eq!(w.jobs()[0].claimed_by, None);
    assert!(w.bend_events().is_empty());
}

#[test]
fn a_need_pre_empts_a_bend_and_its_progress_is_kept() {
    let (mut w, defs) = setup(Nation::Earth, &[at(3, 2)], 10.0, "");
    w.apply(Command::Bend { ability: id(&defs, "probe"), pos: at(3, 2) }, &defs).unwrap();
    run_until(&mut w, &defs, 100, |w| w.jobs()[0].progress > 2.0);
    // Too tired to go on: it sleeps, still holding the job.
    w.units[0].needs.energy = defs.needs.energy.low - 1.0;
    w.tick(&defs);
    assert_eq!(w.units[0].task(), Some(Task::Sleep));
    let kept = w.jobs()[0].progress;
    assert!(kept > 2.0);
    for _ in 0..50 {
        w.tick(&defs);
        assert_eq!(w.jobs()[0].progress, kept);
        assert_eq!(w.jobs()[0].claimed_by, Some(w.units[0].id));
    }
    w.units[0].needs.energy = 95.0;
    run_until(&mut w, &defs, 300, |w| !w.bend_events().is_empty());
    assert_eq!(w.bend_events()[0].outcome, BendOutcome::Done);
    // Only the remainder was left to channel after waking: 10 s total, 2+ done before sleep.
    assert!(w.tick < 100 + 55 + 90, "{}", w.tick);
}

#[test]
fn a_target_that_became_invalid_fails_without_spending_chi() {
    let (mut w, defs) = setup(Nation::Earth, &[at(1, 2)], 2.0, "");
    let probe = id(&defs, "probe");
    w.apply(Command::Bend { ability: probe, pos: at(6, 2) }, &defs).unwrap();
    // The ground turns to rock before anyone gets there.
    w.map[at(6, 2)].terrain = Terrain::Mountain;
    w.map_changed();
    run_until(&mut w, &defs, 400, |w| !w.bend_events().is_empty());
    assert_eq!(w.bend_events()[0].outcome, BendOutcome::Failed("probe needs walkable ground"));
    assert_eq!(w.units[0].chi, 100.0);
    assert!(w.jobs().is_empty());
    assert_eq!(w.units[0].task(), None);
}

#[test]
fn cancelling_releases_the_unit_and_logs_no_event() {
    let (mut w, defs) = setup(Nation::Earth, &[at(1, 2)], 5.0, "");
    w.apply(Command::Bend { ability: id(&defs, "probe"), pos: at(6, 2) }, &defs).unwrap();
    run_until(&mut w, &defs, 100, |w| matches!(w.units[0].task(), Some(Task::Bend(_))));
    w.apply(Command::CancelBend { pos: at(6, 2) }, &defs).unwrap();
    assert!(w.jobs().is_empty());
    w.tick(&defs);
    assert_eq!(w.units[0].task(), None);
    for _ in 0..300 {
        w.tick(&defs);
    }
    assert!(w.bend_events().is_empty());
    assert_eq!(w.units[0].chi, 100.0);
}

/// Everything observable about a world through its public API.
fn snapshot(w: &World) -> String {
    let units: Vec<_> =
        w.units.iter().map(|u| (u.id, u.pos, u.chi.to_bits(), u.needs, u.task())).collect();
    format!("tick {} {units:?} {:?} {:?}", w.tick, w.jobs(), w.bend_events())
}

fn play_session(defs: &Defs) -> World {
    let probe = id(defs, "probe");
    let mut w = World::with_map(1, arena(9, 5), Nation::Earth, &[at(1, 2), at(7, 2)], defs);
    for t in 0..1500u64 {
        match t {
            0 => {
                w.apply(Command::Bend { ability: probe, pos: at(6, 2) }, defs).unwrap();
                w.apply(Command::Bend { ability: probe, pos: at(2, 4) }, defs).unwrap();
            }
            40 => w.apply(Command::CancelBend { pos: at(2, 4) }, defs).unwrap(),
            100 => {
                w.apply(Command::Bend { ability: probe, pos: at(4, 0) }, defs).unwrap();
                w.apply(Command::Bend { ability: probe, pos: at(0, 0) }, defs).unwrap();
            }
            _ => {}
        }
        w.tick(defs);
    }
    w
}

#[test]
fn a_session_with_bend_commands_replays_identically() {
    let defs = probe_defs(3.0, "");
    let original = play_session(&defs);
    let log = original.command_log().to_vec();
    assert_eq!(log.len(), 5);
    assert!(
        original.bend_events().len() >= 2 && original.units.iter().any(|u| u.chi < 100.0),
        "the session should have bent something, or the test proves little"
    );

    let mut replayed = World::with_map(1, arena(9, 5), Nation::Earth, &[at(1, 2), at(7, 2)], &defs);
    replayed.replay(&log, original.tick, &defs);
    assert_eq!(snapshot(&replayed), snapshot(&original));
    assert_eq!(replayed.command_log(), original.command_log());
}

#[test]
fn replay_without_the_bend_commands_diverges() {
    // Guards the test above against comparing two worlds that never did anything.
    let defs = probe_defs(3.0, "");
    let original = play_session(&defs);
    let mut log = original.command_log().to_vec();
    log.retain(|r| !matches!(r.command, Command::CancelBend { .. }));
    let mut replayed = World::with_map(1, arena(9, 5), Nation::Earth, &[at(1, 2), at(7, 2)], &defs);
    replayed.replay(&log, original.tick, &defs);
    assert_ne!(snapshot(&replayed), snapshot(&original));
}
