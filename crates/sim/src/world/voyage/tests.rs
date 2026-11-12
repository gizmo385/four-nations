//! The Airship Dock (#758): planning, loading, crewing, the voyage away, landing and the
//! invariants around units that are off the map.

use super::*;
use crate::defs::{BiomeId, Terrain};
use crate::geom::{Footprint, Grid};
use crate::mapgen::Tile;
use crate::world::fuel::Bunker;

fn at(x: i32, y: i32) -> Pos {
    Pos::new(x, y)
}

const DOCK: Pos = Pos::new(20, 10);

/// Earth's caravan, by deal: 0 is 30 food for 20 stone, 1 is 50 wood for 8 gold ore.
const FOOD_FOR_STONE: u8 = 0;
const WOOD_FOR_ORE: u8 = 1;

fn plan(deals: &[(u8, u8)]) -> [(u8, u8); MAX_VOYAGE_DEALS] {
    let mut out = [(0, 0); MAX_VOYAGE_DEALS];
    out[..deals.len()].copy_from_slice(deals);
    out
}

fn plan_cmd(nation: Nation, deals: &[(u8, u8)]) -> Command {
    Command::PlanVoyage { dock: DOCK, nation, deals: plan(deals) }
}

fn field() -> TileMap {
    Grid::from_fn(50, 40, |_| Tile {
        elevation: 100,
        temperature: 128,
        moisture: 128,
        terrain: Terrain::Land,
        biome: BiomeId(0),
        resource: None,
        building: None,
    })
}

/// A Fire settlement with `units` settlers, a finished Airship Dock at `DOCK` whose bunker holds
/// `fuel` wood, and stores of wood, food and stone. Needs don't decay, so the units don't wander
/// off to eat.
fn setup(units: i32, fuel: u32) -> (World, Defs) {
    let mut defs = Defs::bundled();
    defs.needs.hunger.decay_per_minute = 0.0;
    defs.needs.thirst.decay_per_minute = 0.0;
    defs.needs.energy.decay_per_minute = 0.0;
    let dock = defs.building_id("airship_dock").unwrap();
    let spawns: Vec<Pos> = (0..units).map(|n| at(16 + n, 16)).collect();
    let mut w = World::with_map(1, field(), Nation::Fire, &spawns, &defs);
    w.put_building(Footprint::new(DOCK, 3, 3), dock, &defs);
    w.anchored_mut(DOCK).unwrap().fuel = Some(Bunker { count: fuel, burned: 0, item: None });
    for (x, item) in [(14, "fish"), (15, "fish"), (16, "wood"), (17, "wood"), (18, "stone")] {
        let pos = at(x, 20);
        w.designate_stockpile(pos).unwrap();
        w.stored.insert(pos, Stack { item: defs.item_id(item).unwrap(), count: 200 });
    }
    // An empty one, for what the airship brings back.
    w.designate_stockpile(at(19, 20)).unwrap();
    (w, defs)
}

fn run_until(w: &mut World, defs: &Defs, limit: u64, mut done: impl FnMut(&World) -> bool) {
    for _ in 0..limit {
        if done(w) {
            return;
        }
        w.tick(defs);
    }
    panic!("not done after {limit} ticks");
}

/// Items of `item` in stores and on the ground.
fn total(w: &World, defs: &Defs, item: &str) -> u32 {
    let id = defs.item_id(item).unwrap();
    let sum =
        |it: Vec<&Stack>| it.into_iter().filter(|s| s.item == id).map(|s| s.count).sum::<u32>();
    sum(w.stored.values().collect()) + sum(w.ground.values().collect())
}

fn is_away(w: &World) -> bool {
    w.voyage_away(DOCK).is_some()
}

fn jobs_of(w: &World, f: impl Fn(JobKind) -> bool) -> usize {
    w.jobs.iter().filter(|j| f(j.kind)).count()
}

#[test]
fn the_airship_dock_and_airships_are_the_fire_air_fusion() {
    let defs = Defs::bundled();
    let def = defs.building(defs.building_id("airship_dock").unwrap());
    let a = def.airship.expect("an airship");
    assert_eq!((a.crew, a.hold, a.deals, a.fuel_per_voyage), (2, 60, 2, 20));
    assert_eq!(a.away_ticks, 15 * 60 * u64::from(TICKS_PER_SECOND));
    assert_eq!(def.footprint, (3, 3));
    assert_eq!(def.fuel.as_ref().map(|f| f.bunker), Some(20));
    let tech = defs.tech(defs.tech_id("airships").unwrap());
    assert_eq!(tech.fusion, Some([Nation::Fire, Nation::Air]));
    assert_eq!(tech.requires.population, Some(24));
    assert_eq!(tech.prereqs, [defs.tech_id("cartography").unwrap()]);
}

#[test]
fn an_airship_needs_a_bunker_and_sane_numbers() {
    use crate::defs::BUNDLED_FILES;
    let load = |from: &str, to: &str| {
        let mut files = BUNDLED_FILES;
        let buildings = files[7].replacen(from, to, 1);
        files[7] = &buildings;
        Defs::from_files(files).err().map(|e| e.0)
    };
    assert_eq!(load("crew = 2", "crew = 2"), None, "an unchanged file loads");
    for (from, to, needle) in [
        ("crew = 2", "crew = 0", "crew"),
        ("hold = 60\ndeals = 2", "hold = 0\ndeals = 2", "hold"),
        ("deals = 2\naway", "deals = 0\naway", "deals"),
        ("deals = 2\naway", "deals = 5\naway", "deals"),
        ("away_minutes = 15.0", "away_minutes = 0.0", "away_minutes"),
        ("fuel_per_voyage = 20", "fuel_per_voyage = 21", "fuel_per_voyage"),
        ("fuel_per_voyage = 20", "fuel_per_voyage = 0", "fuel_per_voyage"),
    ] {
        let err = load(from, to).unwrap_or_else(|| panic!("{to} should be refused"));
        assert!(err.contains(needle) && err.contains("airship"), "{to}: {err}");
    }
}

#[test]
fn a_voyage_is_refused_with_a_reason() {
    let (mut w, defs) = setup(3, 20);
    let refused = |w: &mut World, cmd| match w.apply(cmd, &defs) {
        Err(CommandError::Voyage(e)) => e,
        other => panic!("expected a voyage error, got {other:?}"),
    };
    let off = Command::PlanVoyage { dock: at(1, 1), nation: Nation::Earth, deals: plan(&[(0, 1)]) };
    assert_eq!(refused(&mut w, off), VoyageError::NoDock);
    assert_eq!(refused(&mut w, plan_cmd(Nation::Earth, &[])), VoyageError::NoDeals);
    assert_eq!(refused(&mut w, plan_cmd(Nation::Earth, &[(99, 1)])), VoyageError::NoSuchDeal);
    assert_eq!(
        refused(&mut w, plan_cmd(Nation::Earth, &[(FOOD_FOR_STONE, 3)])),
        VoyageError::TooManyTimes { most: 2 }
    );
    assert_eq!(
        refused(&mut w, plan_cmd(Nation::Earth, &[(FOOD_FOR_STONE, 1), (FOOD_FOR_STONE, 1)])),
        VoyageError::Duplicate
    );
    assert_eq!(
        refused(&mut w, plan_cmd(Nation::Earth, &[(0, 1), (1, 1), (2, 1)])),
        VoyageError::TooManyDeals { most: 2 }
    );
    // 30 food and 50 wood: 80 items in a hold of 60.
    assert_eq!(
        refused(&mut w, plan_cmd(Nation::Earth, &[(FOOD_FOR_STONE, 1), (WOOD_FOR_ORE, 1)])),
        VoyageError::TooBig { out: 80, hold: 60 }
    );
    assert_eq!(refused(&mut w, Command::CancelVoyage { dock: DOCK }), VoyageError::NothingPlanned);
    assert!(w.command_log().is_empty(), "refusals aren't logged");
    assert_eq!(w.voyage_status(DOCK, &defs), Some(VoyageStatus::Docked));
    // A valid plan, then a second is refused until the first is called off.
    w.apply(plan_cmd(Nation::Earth, &[(WOOD_FOR_ORE, 1)]), &defs).unwrap();
    assert_eq!(refused(&mut w, plan_cmd(Nation::Water, &[(0, 1)])), VoyageError::Planned);
    assert_eq!(VoyageError::Away.to_string(), "The airship is away");
}

#[test]
fn the_plan_lists_its_deals_in_file_order_and_each_nation_may_be_picked() {
    let (mut w, defs) = setup(3, 20);
    w.apply(plan_cmd(Nation::Earth, &[(2, 1), (0, 1)]), &defs).unwrap();
    let p = w.voyage_plan(DOCK).unwrap();
    assert_eq!(p.deals.iter().map(|d| d.deal).collect::<Vec<_>>(), [0, 2]);
    assert!(!p.united);
    // Kin: the settlement's own nation is a destination too.
    let (mut w, _) = setup(3, 20);
    w.apply(plan_cmd(Nation::Fire, &[(0, 1)]), &defs).unwrap();
    assert_eq!(w.voyage_plan(DOCK).unwrap().nation, Nation::Fire);
}

#[test]
fn haulers_load_the_goods_then_a_crew_sails_and_the_voyage_lands_on_a_fixed_tick() {
    let (mut w, defs) = setup(3, 20);
    let wood_before = total(&w, &defs, "wood") + 20;
    w.apply(plan_cmd(Nation::Earth, &[(WOOD_FOR_ORE, 1)]), &defs).unwrap();
    assert_eq!(
        w.voyage_status(DOCK, &defs),
        Some(VoyageStatus::Loading { held: 0, out: 50 }),
        "a Load job for the slot is open at once"
    );
    assert_eq!(jobs_of(&w, |k| matches!(k, JobKind::Load { .. })), 1);
    run_until(&mut w, &defs, 30_000, |w| {
        w.voyage_plan(DOCK).is_some_and(|p| !p.deals[0].held.is_empty())
    });
    run_until(&mut w, &defs, 20_000, is_away);

    let away = w.voyage_away(DOCK).unwrap().clone();
    assert_eq!(away.crew.len(), 2);
    assert_eq!(w.units.len(), 1, "two of the three are off the map");
    assert!(w.units.iter().all(|u| away.crew.iter().all(|c| c.id != u.id)));
    assert_eq!(away.lands, away.sailed + 15 * 60 * u64::from(TICKS_PER_SECOND));
    assert_eq!(w.anchored(DOCK).unwrap().bunker().unwrap().count, 0, "20 wood burned");
    assert!(w.voyage_plan(DOCK).is_none(), "the plan went with the airship");
    assert_eq!(jobs_of(&w, |k| matches!(k, JobKind::Load { .. } | JobKind::Crew { .. })), 0);
    assert_eq!(total(&w, &defs, "wood"), wood_before - 70, "50 out, 20 burned");
    assert_eq!(total(&w, &defs, "gold_ore"), 0, "nothing back yet");
    w.check_invariants(&defs).expect("a world with crew away is consistent");
    assert!(matches!(w.voyage_status(DOCK, &defs), Some(VoyageStatus::Away { .. })));

    // Away, the crew still count as residents: the world's count, the roster research reads,
    // and the settlement's own people.
    w.tick(&defs);
    assert_eq!(w.population(), 3);
    assert_eq!(w.settlers(), 3);
    assert_eq!(w.residents().count(), 3);
    assert_eq!((w.roster.population, w.roster.residents[Nation::Fire.index()]), (3, 3));

    // Home on the very tick it was fixed for.
    run_until(&mut w, &defs, 20_000, |w| w.tick + 1 == away.lands);
    assert!(is_away(&w));
    w.tick(&defs);
    assert!(!is_away(&w), "landed on {}", away.lands);
    assert_eq!(w.tick, away.lands);
    assert_eq!(w.units.len(), 3);
    assert!(w.units.windows(2).all(|p| p[0].id < p[1].id), "back in id order");
    for c in &away.crew {
        let u = w.units.iter().find(|u| u.id == c.id).expect("back on the map");
        assert_eq!(u.needs, c.needs, "needs were frozen");
        assert_eq!(u.chi, c.chi);
    }
    w.check_invariants(&defs).unwrap();
    // The get side spilled beside the dock, then ordinary hauls bring it home.
    assert_eq!(total(&w, &defs, "gold_ore"), 8);
    assert!(w.ground.values().any(|s| s.item == defs.item_id("gold_ore").unwrap()));
    run_until(&mut w, &defs, 30_000, |w| {
        w.stored.values().any(|s| s.item == defs.item_id("gold_ore").unwrap())
    });
    // And it can sail again.
    assert_eq!(w.voyage_status(DOCK, &defs), Some(VoyageStatus::Docked));
    w.apply(plan_cmd(Nation::Earth, &[(WOOD_FOR_ORE, 1)]), &defs).unwrap();
}

#[test]
fn a_voyage_takes_up_to_the_hold_and_brings_every_deal_back() {
    let (mut w, defs) = setup(3, 20);
    // Two food-for-stone deals' worth: 60 food out, 40 stone back.
    w.apply(plan_cmd(Nation::Earth, &[(FOOD_FOR_STONE, 2)]), &defs).unwrap();
    run_until(&mut w, &defs, 40_000, is_away);
    let away = w.voyage_away(DOCK).unwrap();
    assert_eq!(away.back, [Stack { item: defs.item_id("stone").unwrap(), count: 40 }]);
    assert_eq!(away.knowledge, 0);
}

#[test]
fn air_knowledge_comes_home_as_research_points() {
    let (mut w, defs) = setup(3, 20);
    // Air's second deal: 30 wood for 30 knowledge.
    let deal = &defs.trade.caravan(Nation::Air, false).unwrap().deals[1];
    assert_eq!((deal.knowledge, deal.get.len()), (30, 0));
    w.apply(plan_cmd(Nation::Air, &[(1, 1)]), &defs).unwrap();
    run_until(&mut w, &defs, 40_000, is_away);
    assert_eq!(w.voyage_away(DOCK).unwrap().knowledge, 30);
    assert!(w.voyage_away(DOCK).unwrap().back.is_empty());
}

#[test]
fn cancelling_before_it_sails_drops_what_was_loaded_by_the_door() {
    let (mut w, defs) = setup(3, 20);
    w.apply(plan_cmd(Nation::Earth, &[(WOOD_FOR_ORE, 1)]), &defs).unwrap();
    run_until(&mut w, &defs, 8000, |w| {
        w.voyage_plan(DOCK)
            .is_some_and(|p| p.deals[0].held.iter().map(|s| s.count).sum::<u32>() > 0)
    });
    let wood = total(&w, &defs, "wood");
    let loaded: u32 = w.voyage_plan(DOCK).unwrap().deals[0].held.iter().map(|s| s.count).sum();
    w.apply(Command::CancelVoyage { dock: DOCK }, &defs).unwrap();
    assert!(w.voyage_plan(DOCK).is_none());
    assert_eq!(jobs_of(&w, |k| matches!(k, JobKind::Load { .. } | JobKind::Crew { .. })), 0);
    let door = w.anchored(DOCK).unwrap().footprint().entrance();
    let near_door: u32 = w
        .ground
        .iter()
        .filter(|(p, s)| p.chebyshev(door) <= 2 && s.item == defs.item_id("wood").unwrap())
        .map(|(_, s)| s.count)
        .sum();
    assert!(near_door >= loaded, "{near_door} beside the door, {loaded} loaded");
    // Nothing was lost, and nobody is left holding a job that is gone.
    assert!(total(&w, &defs, "wood") >= wood);
    for _ in 0..200 {
        w.tick(&defs);
    }
    w.check_invariants(&defs).unwrap();
    assert!(w.units.iter().all(|u| !matches!(u.task(), Some(Task::Load(_) | Task::Crew(_)))));
}

#[test]
fn the_airship_waits_for_fuel_and_crew_jobs_open_once_it_is_loaded_and_fuelled() {
    let (mut w, defs) = setup(3, 5);
    // Load it by hand, so the only thing missing is fuel.
    w.apply(plan_cmd(Nation::Earth, &[(WOOD_FOR_ORE, 1)]), &defs).unwrap();
    let wood = defs.item_id("wood").unwrap();
    w.anchored_mut(DOCK).unwrap().voyage.as_mut().unwrap().deals[0].held =
        vec![Stack { item: wood, count: 50 }];
    w.jobs.retain(|j| !matches!(j.kind, JobKind::Load { .. }));
    // Nothing in stores to burn, so no refuel can finish it either.
    w.stored.clear();
    for _ in 0..60 {
        w.tick(&defs);
    }
    assert_eq!(w.voyage_status(DOCK, &defs), Some(VoyageStatus::NeedsFuel { have: 5, need: 20 }));
    assert_eq!(jobs_of(&w, |k| matches!(k, JobKind::Crew { .. })), 0);
    w.anchored_mut(DOCK).unwrap().fuel = Some(Bunker { count: 20, burned: 0, item: None });
    for _ in 0..60 {
        w.tick(&defs);
        if is_away(&w) {
            break;
        }
    }
    assert!(
        is_away(&w) || matches!(w.voyage_status(DOCK, &defs), Some(VoyageStatus::Boarding { .. }))
    );
    assert!(jobs_of(&w, |k| matches!(k, JobKind::Crew { .. })) <= 2);
}

#[test]
fn nothing_sails_short_of_a_whole_crew() {
    // One settler only: the second place in the crew is never filled.
    let (mut w, defs) = setup(1, 20);
    w.apply(plan_cmd(Nation::Earth, &[(WOOD_FOR_ORE, 1)]), &defs).unwrap();
    for _ in 0..12_000 {
        w.tick(&defs);
    }
    assert!(!is_away(&w), "one resident is not a crew of two");
    assert!(matches!(
        w.voyage_status(DOCK, &defs),
        Some(VoyageStatus::Boarding { aboard: 1, crew: 2 })
    ));
    assert_eq!(w.units.len(), 1);
}

#[test]
fn the_dock_cannot_be_taken_down_or_called_back_while_the_airship_is_away() {
    let (mut w, defs) = setup(3, 20);
    w.apply(plan_cmd(Nation::Earth, &[(WOOD_FOR_ORE, 1)]), &defs).unwrap();
    run_until(&mut w, &defs, 40_000, is_away);
    let err = w.apply(Command::Deconstruct { pos: DOCK }, &defs).unwrap_err();
    assert_eq!(err, CommandError::Designate(DesignateError::AirshipAway));
    assert_eq!(err.to_string(), "The airship is away");
    // Any tile of it, not only the anchor.
    assert!(w.apply(Command::Deconstruct { pos: at(22, 12) }, &defs).is_err());
    assert_eq!(
        w.apply(Command::CancelVoyage { dock: DOCK }, &defs),
        Err(CommandError::Voyage(VoyageError::Away))
    );
    assert_eq!(
        w.apply(plan_cmd(Nation::Earth, &[(WOOD_FOR_ORE, 1)]), &defs),
        Err(CommandError::Voyage(VoyageError::Away))
    );
    let lands = w.voyage_away(DOCK).unwrap().lands;
    run_until(&mut w, &defs, 40_000, |w| w.tick >= lands);
    w.apply(Command::Deconstruct { pos: DOCK }, &defs).expect("home, it can come down");
}

#[test]
fn a_dock_taken_down_with_a_voyage_loaded_gives_the_goods_back() {
    let (mut w, defs) = setup(3, 20);
    w.apply(plan_cmd(Nation::Earth, &[(WOOD_FOR_ORE, 1)]), &defs).unwrap();
    let wood = defs.item_id("wood").unwrap();
    w.anchored_mut(DOCK).unwrap().voyage.as_mut().unwrap().deals[0].held =
        vec![Stack { item: wood, count: 30 }];
    let key = w.map[DOCK].building.unwrap().key();
    let record = w.remove_building(key, &defs).unwrap();
    assert_eq!(record.voyage_goods(), [Stack { item: wood, count: 30 }]);
    assert_eq!(jobs_of(&w, |k| matches!(k, JobKind::Load { .. } | JobKind::Crew { .. })), 0);
}

#[test]
fn the_invariants_walk_the_voyages() {
    let (mut w, defs) = setup(3, 20);
    w.apply(plan_cmd(Nation::Earth, &[(WOOD_FOR_ORE, 1)]), &defs).unwrap();
    run_until(&mut w, &defs, 40_000, is_away);
    w.check_invariants(&defs).unwrap();
    let good = w.voyages.clone();
    // A unit both on the map and away.
    let mut bad = w.clone();
    let twin = good[0].crew[0].clone();
    bad.units.push(twin);
    bad.units.sort_by_key(|u| u.id);
    assert!(bad.check_invariants(&defs).unwrap_err().contains("both on the map and away"));
    // Crew out of id order.
    let mut bad = w.clone();
    bad.voyages[0].crew.reverse();
    assert!(bad.check_invariants(&defs).unwrap_err().contains("crew not sorted"));
    // An airship with nobody aboard.
    let mut bad = w.clone();
    bad.voyages[0].crew.clear();
    assert!(bad.check_invariants(&defs).unwrap_err().contains("no crew"));
    // Away holding a task.
    let mut bad = w.clone();
    bad.voyages[0].crew[0].job_task = Some(Task::Deliver);
    assert!(bad.check_invariants(&defs).unwrap_err().contains("holding a task"));
    // Voyages out of dock order.
    let mut bad = w.clone();
    let mut second = bad.voyages[0].clone();
    second.dock = at(1, 1);
    second.crew.clear();
    bad.voyages.push(second);
    assert!(bad.check_invariants(&defs).is_err());
}

#[test]
fn a_voyage_survives_a_save_and_lands_the_same() {
    let (mut w, defs) = setup(3, 20);
    w.apply(plan_cmd(Nation::Earth, &[(WOOD_FOR_ORE, 1)]), &defs).unwrap();
    // Mid-loading, then away.
    for _ in 0..400 {
        w.tick(&defs);
    }
    let mut loading = World::load(&w.save(&defs).expect("saves"), &defs).expect("loads");
    assert_eq!(loading.voyage_plan(DOCK), w.voyage_plan(DOCK));
    run_until(&mut w, &defs, 40_000, is_away);
    let mut away = World::load(&w.save(&defs).expect("saves"), &defs).expect("loads");
    assert_eq!(away.voyages, w.voyages);
    assert_eq!(away.save(&defs).unwrap(), w.save(&defs).unwrap());
    away.check_invariants(&defs).unwrap();
    assert_eq!(away.population(), 3);
    let lands = w.voyage_away(DOCK).unwrap().lands;
    for _ in w.tick..lands {
        w.tick(&defs);
        away.tick(&defs);
    }
    assert_eq!(away.save(&defs).unwrap(), w.save(&defs).unwrap());
    assert!(!is_away(&away));
    // The one loaded from mid-loading carries on to sail too.
    run_until(&mut loading, &defs, 40_000, is_away);
}

#[test]
fn a_voyage_is_deterministic_and_replays_from_its_command_log() {
    let run = || {
        let (mut w, defs) = setup(3, 20);
        w.apply(plan_cmd(Nation::Earth, &[(WOOD_FOR_ORE, 1)]), &defs).unwrap();
        let sailed = {
            run_until(&mut w, &defs, 40_000, is_away);
            w.tick
        };
        let lands = w.voyage_away(DOCK).unwrap().lands;
        for _ in 0..(lands - sailed + 600) {
            w.tick(&defs);
        }
        (w.save(&defs).expect("saves"), w.command_log().to_vec(), sailed, lands)
    };
    let (a, log, sailed, lands) = run();
    let (b, _, sailed_b, lands_b) = run();
    assert_eq!(a, b);
    assert_eq!((sailed, lands), (sailed_b, lands_b));
    // The log alone reproduces it: the voyage is a function of the commands.
    let (mut w, defs) = setup(3, 20);
    w.replay(&log, lands + 600, &defs);
    assert_eq!(w.save(&defs).unwrap(), a);
}

#[test]
fn the_crew_s_needs_are_frozen_while_they_are_away() {
    let (mut w, mut defs) = setup(3, 20);
    w.apply(plan_cmd(Nation::Earth, &[(WOOD_FOR_ORE, 1)]), &defs).unwrap();
    run_until(&mut w, &defs, 40_000, is_away);
    let crew = w.voyage_away(DOCK).unwrap().crew.clone();
    let stayed = w.units[0].needs;
    // Hunger starts to bite once they are out of sight.
    defs.needs.hunger.decay_per_minute = 30.0;
    for _ in 0..3000 {
        w.tick(&defs);
    }
    assert!(is_away(&w));
    assert_eq!(w.voyage_away(DOCK).unwrap().crew, crew, "they can't starve, die or leave");
    assert_ne!(w.units[0].needs, stayed, "the one who stayed is not frozen");
}
