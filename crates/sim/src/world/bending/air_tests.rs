//! Wind Carry (#71): targets, the haul job it takes over and gives back, sending the whole
//! stack over anything, overflow, and replay.

use super::*;
use crate::geom::Grid;
use crate::jobs::ItemId;
use crate::mapgen::Tile;

fn at(x: i32, y: i32) -> Pos {
    Pos::new(x, y)
}

fn biome(defs: &Defs, id: &str) -> BiomeId {
    BiomeId(defs.biomes.iter().position(|b| b.id == id).unwrap() as u8)
}

/// A `width` x `height` plains field. If `river` is `Some(x)`, that column is `Terrain::River`
/// (no ford anywhere), splitting the field in two.
fn field(defs: &Defs, width: i32, height: i32, river: Option<i32>) -> TileMap {
    let plains = biome(defs, "plains");
    Grid::from_fn(width, height, |p| Tile {
        elevation: 100,
        temperature: 128,
        moisture: 128,
        terrain: if river == Some(p.x) { Terrain::River } else { Terrain::Land },
        biome: plains,
        resource: None,
        building: None,
    })
}

fn world(spawns: &[Pos], width: i32, height: i32, river: Option<i32>, defs: &Defs) -> World {
    World::with_map(1, field(defs, width, height, river), Nation::Air, spawns, defs)
}

fn wind(defs: &Defs) -> AbilityId {
    defs.ability_id("wind_carry").unwrap()
}

fn wood_item(defs: &Defs) -> ItemId {
    defs.item_id("wood").unwrap()
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

/// Bends Wind Carry on `pos` and runs until it finishes, which it should do successfully.
fn blow(w: &mut World, defs: &Defs, pos: Pos) {
    w.apply(Command::Bend { ability: wind(defs), pos }, defs).unwrap();
    let events = w.bend_events().count();
    run_until(w, defs, 2000, |w| w.bend_events().count() > events);
    assert_eq!(w.bend_events().next_back().unwrap().outcome, BendOutcome::Done);
}

#[test]
fn targets_need_a_ground_stack_and_a_stockpile_with_room_in_reach() {
    let defs = Defs::bundled();
    let wood = wood_item(&defs);
    let mut w = world(&[at(9, 2)], 40, 5, None, &defs);
    w.ground.insert(at(10, 2), Stack { item: wood, count: 5 });
    w.ensure_haul_job(at(10, 2));
    let ability = wind(&defs);
    let bend = |w: &mut World, pos| w.apply(Command::Bend { ability, pos }, &defs);
    let refused = |why| Err(CommandError::Bend(BendError::InvalidTarget(why)));

    assert_eq!(bend(&mut w, at(11, 2)), refused("needs a stack to blow"));
    assert_eq!(bend(&mut w, at(10, 2)), refused("no stockpile with room in reach"));

    // A full stockpile nearby doesn't count.
    w.zones.insert(at(15, 2));
    w.stored.insert(at(15, 2), Stack { item: wood, count: defs.jobs.stockpile_stack });
    assert_eq!(bend(&mut w, at(10, 2)), refused("no stockpile with room in reach"));

    // 8 tiles x power_generation 2.0 is 16: right at the edge, this one has room.
    w.zones.insert(at(26, 2));
    assert_eq!(bend(&mut w, at(10, 2)), Ok(()));
    w.apply(Command::CancelBend { pos: at(10, 2) }, &defs).unwrap();

    // One tile further out is out of reach.
    w.zones.remove(&at(26, 2));
    w.zones.insert(at(27, 2));
    assert_eq!(bend(&mut w, at(10, 2)), refused("no stockpile with room in reach"));
    assert_eq!(bend(&mut w, at(-1, 0)), refused("off the map"));
}

#[test]
fn other_nations_cannot_designate_it() {
    let defs = Defs::bundled();
    let wood = wood_item(&defs);
    for nation in [Nation::Water, Nation::Fire, Nation::Earth] {
        let mut w = World::with_map(1, field(&defs, 40, 5, None), nation, &[at(9, 2)], &defs);
        w.ground.insert(at(10, 2), Stack { item: wood, count: 3 });
        w.ensure_haul_job(at(10, 2));
        w.zones.insert(at(14, 2));
        let r = w.apply(Command::Bend { ability: wind(&defs), pos: at(10, 2) }, &defs);
        assert_eq!(r, Err(CommandError::Bend(BendError::WrongNation)));
    }
}

#[test]
fn designating_releases_a_hauler_that_has_not_arrived_yet() {
    let defs = Defs::bundled();
    let wood = wood_item(&defs);
    let mut w = world(&[at(1, 2), at(38, 2)], 40, 5, None, &defs);
    w.apply(Command::DesignateStockpile { pos: at(20, 2) }, &defs).unwrap();
    w.ground.insert(at(10, 2), Stack { item: wood, count: 5 });
    w.ensure_haul_job(at(10, 2));
    w.wake_idle();
    run_until(&mut w, &defs, 50, |w| w.jobs().iter().any(|j| j.claimed_by.is_some()));
    let job_id = w.jobs()[0].id;
    assert_eq!(w.units[0].task(), Some(Task::Haul(job_id)), "unit 0 is on its way");
    assert_eq!(
        w.ground_items().next(),
        Some((at(10, 2), Stack { item: wood, count: 5 })),
        "not picked up yet"
    );

    w.apply(Command::Bend { ability: wind(&defs), pos: at(10, 2) }, &defs).unwrap();
    assert!(
        !w.jobs().iter().any(|j| matches!(j.kind, JobKind::Haul { .. })),
        "the haul job is gone"
    );
    assert_eq!(w.units[0].task(), None, "released right away");
}

/// #386: a hauler whose claim gets taken over by Wind Carry while a need has already pre-empted
/// it (hunger, thirst or energy, `work.rs`'s "needs pre-empt jobs") must not lose its need task's
/// own route. Before the fix, `take_haul_job` cleared `route` unconditionally, as if the unit
/// were still walking the haul -- but with a need in charge, `route` belongs to that instead
/// (the haul's `job_task` is only set aside, not dropped). The unit was left with `need_task`
/// still `Drink` and `route` suddenly `None`, which `walk` never reports as arrived, so it never
/// drank and starved beside water with plenty to drink.
#[test]
fn designating_over_a_need_preempted_hauler_does_not_strand_its_route() {
    let defs = Defs::bundled();
    let wood = wood_item(&defs);
    // A river at x=5 blocks nothing here (everyone stays east of it): it's only there to give
    // the unit somewhere to drink, close enough that one tick after the need takes over still
    // leaves a route in progress.
    let mut w = world(&[at(9, 2)], 40, 5, Some(5), &defs);
    w.apply(Command::DesignateStockpile { pos: at(14, 2) }, &defs).unwrap();
    w.ground.insert(at(10, 2), Stack { item: wood, count: 5 });
    w.ensure_haul_job(at(10, 2));
    w.wake_idle();
    run_until(&mut w, &defs, 50, |w| w.jobs().iter().any(|j| j.claimed_by.is_some()));
    let job_id = w.jobs()[0].id;
    assert_eq!(w.units[0].task(), Some(Task::Haul(job_id)), "unit 0 is on its way");

    // Thirst alone drops to `low`: hunger and energy stay full, so only Drink pre-empts.
    w.units[0].needs.thirst = defs.needs.thirst.low - 1.0;
    w.tick(&defs);
    assert_eq!(w.units[0].need_task, Some(Task::Drink), "thirst pre-empted the haul");
    assert_eq!(w.units[0].job_task, Some(Task::Haul(job_id)), "the haul is only set aside");
    assert!(w.units[0].route.is_some(), "a route to drink was planned");

    // Wind Carry takes the haul's ground stack over mid-need.
    w.apply(Command::Bend { ability: wind(&defs), pos: at(10, 2) }, &defs).unwrap();
    assert_eq!(w.units[0].job_task, None, "the dead haul claim is dropped");
    assert_eq!(w.units[0].need_task, Some(Task::Drink), "still mid-need");
    assert!(w.units[0].route.is_some(), "the need's own route survives the haul's cancellation");

    run_until(&mut w, &defs, 2000, |w| w.units[0].need_task.is_none());
    assert!(w.units[0].needs.thirst > defs.needs.thirst.low, "it actually drank: {w:?}");
    assert!(
        w.deaths().next().is_none(),
        "nobody should have starved or dehydrated: {:?}",
        w.deaths().collect::<Vec<_>>()
    );
}

#[test]
fn cancelling_a_wind_carry_gives_the_haul_job_back() {
    let defs = Defs::bundled();
    let wood = wood_item(&defs);
    let mut w = world(&[at(9, 2)], 40, 5, None, &defs);
    w.apply(Command::DesignateStockpile { pos: at(20, 2) }, &defs).unwrap();
    w.ground.insert(at(10, 2), Stack { item: wood, count: 5 });
    w.ensure_haul_job(at(10, 2));
    w.apply(Command::Bend { ability: wind(&defs), pos: at(10, 2) }, &defs).unwrap();
    assert!(!w.jobs().iter().any(|j| matches!(j.kind, JobKind::Haul { .. })));

    w.apply(Command::CancelBend { pos: at(10, 2) }, &defs).unwrap();
    assert!(w.jobs().iter().any(|j| j.kind == JobKind::Haul { pos: at(10, 2) }));
    assert_eq!(w.ground_items().next(), Some((at(10, 2), Stack { item: wood, count: 5 })));
}

#[test]
fn a_failed_wind_carry_gives_the_haul_job_back() {
    let defs = Defs::bundled();
    let wood = wood_item(&defs);
    let mut w = world(&[at(9, 2)], 40, 5, None, &defs);
    w.apply(Command::DesignateStockpile { pos: at(20, 2) }, &defs).unwrap();
    w.ground.insert(at(10, 2), Stack { item: wood, count: 5 });
    w.ensure_haul_job(at(10, 2));
    w.apply(Command::Bend { ability: wind(&defs), pos: at(10, 2) }, &defs).unwrap();
    // The only stockpile in reach fills up (someone else's delivery) before the channel ends.
    w.stored.insert(at(20, 2), Stack { item: wood, count: defs.jobs.stockpile_stack });

    let events = w.bend_events().count();
    run_until(&mut w, &defs, 1000, |w| w.bend_events().count() > events);
    assert_eq!(
        w.bend_events().next_back().unwrap().outcome,
        BendOutcome::Failed("no stockpile with room in reach")
    );
    assert_eq!(w.units[0].chi, defs.chi.max, "nothing was spent");
    assert_eq!(
        w.ground_items().next(),
        Some((at(10, 2), Stack { item: wood, count: 5 })),
        "still there"
    );
    assert!(
        w.jobs().iter().any(|j| j.kind == JobKind::Haul { pos: at(10, 2) }),
        "haul job restored"
    );
}

#[test]
fn a_wind_carry_sends_the_whole_stack_ignoring_carry_capacity() {
    let defs = Defs::bundled();
    let wood = wood_item(&defs);
    let mut w = world(&[at(9, 2)], 40, 5, None, &defs);
    w.apply(Command::DesignateStockpile { pos: at(20, 2) }, &defs).unwrap();
    // Air's carry_capacity (0.8) moves 4 items a trip; a dozen in one bend proves it is ignored.
    w.ground.insert(at(10, 2), Stack { item: wood, count: 12 });
    w.ensure_haul_job(at(10, 2));
    blow(&mut w, &defs, at(10, 2));
    assert_eq!(w.stored_total(wood), 12);
    assert_eq!(w.ground_items().count(), 0);
    assert!(!w.jobs().iter().any(|j| matches!(j.kind, JobKind::Haul { .. })));
}

#[test]
fn what_does_not_fit_stays_on_the_ground_with_a_fresh_haul_job() {
    let defs = Defs::bundled();
    let wood = wood_item(&defs);
    let cap = defs.jobs.stockpile_stack;
    let mut w = world(&[at(9, 2)], 40, 5, None, &defs);
    w.apply(Command::DesignateStockpile { pos: at(20, 2) }, &defs).unwrap();
    w.stored.insert(at(20, 2), Stack { item: wood, count: cap - 2 });
    w.ground.insert(at(10, 2), Stack { item: wood, count: 5 });
    w.ensure_haul_job(at(10, 2));
    blow(&mut w, &defs, at(10, 2));
    assert_eq!(w.stored_at(at(20, 2)), Some(Stack { item: wood, count: cap }));
    assert_eq!(w.ground_items().collect::<Vec<_>>(), [(at(10, 2), Stack { item: wood, count: 3 })]);
    assert_eq!(
        w.jobs().iter().filter(|j| j.kind == JobKind::Haul { pos: at(10, 2) }).count(),
        1,
        "the leftover gets a haul job"
    );
}

#[test]
fn equally_near_stockpiles_tie_by_position() {
    let defs = Defs::bundled();
    let wood = wood_item(&defs);
    let mut w = world(&[at(9, 2)], 40, 5, None, &defs);
    // Both 4 tiles from the stack (Chebyshev); the lower position wins.
    w.zones.insert(at(14, 2));
    w.zones.insert(at(6, 2));
    w.ground.insert(at(10, 2), Stack { item: wood, count: 3 });
    w.ensure_haul_job(at(10, 2));
    blow(&mut w, &defs, at(10, 2));
    assert_eq!(w.stored_at(at(6, 2)), Some(Stack { item: wood, count: 3 }));
    assert_eq!(w.stored_at(at(14, 2)), None);
}

#[test]
fn wind_carry_crosses_a_river_with_no_ford() {
    let defs = Defs::bundled();
    let wood = wood_item(&defs);
    // A river at x = 7 splits the field; nothing fords it.
    let mut w = world(&[at(2, 2)], 20, 5, Some(7), &defs);
    w.apply(Command::DesignateStockpile { pos: at(15, 2) }, &defs).unwrap();
    w.ground.insert(at(3, 2), Stack { item: wood, count: 3 });
    w.ensure_haul_job(at(3, 2));
    blow(&mut w, &defs, at(3, 2));
    assert_eq!(w.stored_total(wood), 3);
    assert_eq!(w.map[at(7, 2)].terrain, Terrain::River, "no ford appears");
}

/// Everything observable about the world's ground and stockpiles, through its public API.
fn snapshot(w: &World, defs: &Defs) -> String {
    let units: Vec<_> = w.units.iter().map(|u| (u.id, u.pos, u.chi.to_bits(), u.task())).collect();
    let ground: Vec<_> = w.ground_items().collect();
    format!(
        "tick {} {units:?} ground {ground:?} {:?} stored {}",
        w.tick,
        w.bend_events().collect::<Vec<_>>(),
        w.stored_total(wood_item(defs)),
    )
}

/// A 40x5 plains field with oak trees at (10, 2) and (25, 2).
fn session_field(defs: &Defs) -> TileMap {
    let plains = biome(defs, "plains");
    let tree = defs.resource_id("oak_tree");
    Grid::from_fn(40, 5, |p| Tile {
        elevation: 100,
        temperature: 128,
        moisture: 128,
        terrain: Terrain::Land,
        biome: plains,
        resource: if p == at(10, 2) || p == at(25, 2) { tree } else { None },
        building: None,
    })
}

fn session_world(defs: &Defs) -> World {
    World::with_map(11, session_field(defs), Nation::Air, &[at(9, 2), at(24, 2)], defs)
}

fn play_session(defs: &Defs) -> World {
    let ability = wind(defs);
    let mut w = session_world(defs);
    let mut stage = 0;
    let mut cancel_at: Option<u64> = None;
    for t in 0..6000u64 {
        if t == 0 {
            w.apply(Command::DesignateStockpile { pos: at(20, 2) }, defs).unwrap();
            w.apply(Command::DesignateHarvest { pos: at(10, 2) }, defs).unwrap();
            w.apply(Command::DesignateHarvest { pos: at(25, 2) }, defs).unwrap();
        }
        // Blow each chopped pile as soon as it lands, then cancel the second a few ticks later.
        if stage == 0 && w.ground_items().any(|(p, _)| p == at(10, 2)) {
            stage = 1;
            w.apply(Command::Bend { ability, pos: at(10, 2) }, defs).unwrap();
        } else if stage == 1 && w.ground_items().any(|(p, _)| p == at(25, 2)) {
            stage = 2;
            w.apply(Command::Bend { ability, pos: at(25, 2) }, defs).unwrap();
            cancel_at = Some(t + 5);
        } else if cancel_at == Some(t) {
            w.apply(Command::CancelBend { pos: at(25, 2) }, defs).unwrap();
            cancel_at = None;
        }
        w.tick(defs);
    }
    w
}

#[test]
fn a_session_with_wind_carry_replays_identically() {
    let defs = Defs::bundled();
    let original = play_session(&defs);
    let log = original.command_log().to_vec();
    assert!(log.len() >= 5);
    assert!(
        original.bend_events().any(|e| e.outcome == BendOutcome::Done)
            && original.stored_total(wood_item(&defs)) > 0,
        "the session should blow a pile into the stockpile, or the test proves little: {}",
        snapshot(&original, &defs)
    );
    let mut replayed = session_world(&defs);
    replayed.replay(&log, original.tick, &defs);
    assert_eq!(snapshot(&replayed, &defs), snapshot(&original, &defs));
    assert_eq!(replayed.command_log(), original.command_log());
}

#[test]
fn replay_without_the_bend_commands_diverges() {
    let defs = Defs::bundled();
    let original = play_session(&defs);
    let mut log = original.command_log().to_vec();
    log.retain(|r| !matches!(r.command, Command::Bend { .. } | Command::CancelBend { .. }));
    let mut replayed = session_world(&defs);
    replayed.replay(&log, original.tick, &defs);
    assert_ne!(snapshot(&replayed, &defs), snapshot(&original, &defs));
}

#[test]
fn a_bend_is_a_channel_only_while_the_unit_stands_at_it_and_works() {
    let defs = Defs::bundled();
    let wood = wood_item(&defs);
    let mut w = world(&[at(3, 2)], 40, 5, None, &defs);
    w.zones.insert(at(14, 2));
    w.ground.insert(at(10, 2), Stack { item: wood, count: 5 });
    w.ensure_haul_job(at(10, 2));
    assert!(w.bend_channels(&defs).is_empty(), "nothing designated");

    w.apply(Command::Bend { ability: wind(&defs), pos: at(10, 2) }, &defs).unwrap();
    // The unit walks over first: not a channel yet.
    w.tick(&defs);
    assert!(w.bend_channels(&defs).is_empty(), "still walking");
    run_until(&mut w, &defs, 500, |w| !w.bend_channels(&defs).is_empty());

    let first = w.bend_channels(&defs)[0];
    assert_eq!((first.unit, first.ability, first.pos), (w.units[0].id, wind(&defs), at(10, 2)));
    let work = defs.ability(wind(&defs)).work;
    assert!(first.remaining_secs > 0.0 && first.remaining_secs <= work, "{first:?}");

    // The time left counts down with the world's ticks, and stops with them.
    w.tick(&defs);
    let later = w.bend_channels(&defs)[0].remaining_secs;
    assert!(later < first.remaining_secs, "{later} < {}", first.remaining_secs);
    assert_eq!(w.bend_channels(&defs)[0].remaining_secs, later, "no tick, no change");

    // A need pre-empts the channel: it is set aside, so it isn't one.
    w.units[0].need_task = Some(Task::Sleep);
    assert!(w.bend_channels(&defs).is_empty());
    w.units[0].need_task = None;
    assert_eq!(w.bend_channels(&defs).len(), 1);

    // Finishing ends the channel and logs the event on the same tick.
    let events = w.bend_events().count();
    run_until(&mut w, &defs, 200, |w| w.bend_events().count() > events);
    assert!(w.bend_channels(&defs).is_empty());
    assert_eq!(w.bend_events().next_back().unwrap().outcome, BendOutcome::Done);
}

#[test]
fn the_event_says_where_the_stack_went_so_the_client_can_fly_it() {
    let defs = Defs::bundled();
    let wood = wood_item(&defs);
    let cap = defs.jobs.stockpile_stack;
    let mut w = world(&[at(9, 2)], 40, 5, None, &defs);
    w.zones.insert(at(14, 2));
    w.stored.insert(at(14, 2), Stack { item: wood, count: cap - 3 });
    w.ground.insert(at(10, 2), Stack { item: wood, count: 5 });
    w.ensure_haul_job(at(10, 2));
    blow(&mut w, &defs, at(10, 2));
    // Only what fits is moved; the rest stays on the ground.
    let event = *w.bend_events().next_back().unwrap();
    assert_eq!(event.moved, Some(Moved { to: at(14, 2), item: wood, count: 3 }));
    assert_eq!(w.ground_items().next(), Some((at(10, 2), Stack { item: wood, count: 2 })));
}

#[test]
fn only_wind_carry_moves_a_stack() {
    let defs = Defs::bundled();
    let mut w = World::with_map(1, field(&defs, 40, 5, None), Nation::Fire, &[at(9, 2)], &defs);
    let ability = defs.ability_id("hearthflame").unwrap();
    w.apply(Command::Bend { ability, pos: at(10, 2) }, &defs).unwrap();
    run_until(&mut w, &defs, 500, |w| w.bend_events().next().is_some());
    assert_eq!(w.bend_events().next().unwrap().outcome, BendOutcome::Done);
    assert_eq!(w.bend_events().next().unwrap().moved, None);
}
