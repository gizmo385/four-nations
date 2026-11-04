//! Metalbend (#134): the tech that unlocks it, who can cast it, what targets it accepts, the
//! haul job it takes over and gives back like Wind Carry, and the ore-to-metal conversion.

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

/// A 20x5 plains field, open ground throughout.
fn field(defs: &Defs) -> TileMap {
    let plains = biome(defs, "plains");
    Grid::from_fn(20, 5, |_| Tile {
        elevation: 100,
        temperature: 128,
        moisture: 128,
        terrain: Terrain::Land,
        biome: plains,
        resource: None,
        building: None,
    })
}

/// A world on `field`, with `metalbending` known when `known` is set.
fn world(nation: Nation, known: bool, defs: &Defs) -> World {
    let mut w = World::with_map(1, field(defs), nation, &[at(9, 2)], defs);
    if known {
        w.research.grant(defs.tech_id("metalbending").unwrap());
        w.refresh_modifiers(defs);
    }
    w
}

fn metalbend(defs: &Defs) -> AbilityId {
    defs.ability_id("metalbend").unwrap()
}

fn ore(defs: &Defs) -> ItemId {
    defs.item_id("gold_ore").unwrap()
}

fn metal(defs: &Defs) -> ItemId {
    defs.item_id("metal").unwrap()
}

fn run_until(w: &mut World, defs: &Defs, limit: u64, mut done: impl FnMut(&World) -> bool) -> u64 {
    for n in 0..limit {
        if done(w) {
            return n;
        }
        w.units[0].needs = crate::needs::Needs::full(&defs.needs);
        w.tick(defs);
    }
    panic!("not done after {limit} ticks");
}

/// Casts Metalbend on `pos`, with full chi, and runs until it finishes, which it should do
/// successfully.
fn bend(w: &mut World, defs: &Defs, pos: Pos) {
    w.units[0].chi = defs.chi.max;
    w.apply(Command::Bend { ability: metalbend(defs), pos }, defs).unwrap();
    let events = w.bend_events().len();
    run_until(w, defs, 3000, |w| w.bend_events().len() > events);
    assert_eq!(w.bend_events().last().unwrap().outcome, BendOutcome::Done);
}

#[test]
fn metalbend_is_locked_until_the_tech_and_belongs_to_earth() {
    let defs = Defs::bundled();
    let (ability, tech) = (metalbend(&defs), defs.tech_id("metalbending").unwrap());
    assert_eq!(defs.ability(ability).unlocked_by, Some(tech));
    let w = world(Nation::Earth, false, &defs);
    assert_eq!(w.check_ability(ability, &defs), Err(BendError::Locked { tech }));
    let w = world(Nation::Earth, true, &defs);
    assert_eq!(w.check_ability(ability, &defs), Ok(()));
    for nation in [Nation::Water, Nation::Fire, Nation::Air] {
        let w = world(nation, false, &defs);
        assert_eq!(w.check_ability(ability, &defs), Err(BendError::WrongNation), "{nation:?}");
    }
}

#[test]
fn targets_need_a_ground_stack_of_ore_with_enough_for_a_batch() {
    let defs = Defs::bundled();
    let (gold, metal_item) = (ore(&defs), metal(&defs));
    let mut w = world(Nation::Earth, true, &defs);
    let ability = metalbend(&defs);
    let refused = |why| Err(CommandError::Bend(BendError::InvalidTarget(why)));

    assert_eq!(
        w.apply(Command::Bend { ability, pos: at(10, 2) }, &defs),
        refused("needs ore to bend")
    );

    w.ground.insert(at(10, 2), Stack { item: metal_item, count: 5 });
    assert_eq!(
        w.apply(Command::Bend { ability, pos: at(10, 2) }, &defs),
        refused("that isn't ore")
    );

    w.ground.insert(at(10, 2), Stack { item: gold, count: 1 });
    assert_eq!(
        w.apply(Command::Bend { ability, pos: at(10, 2) }, &defs),
        refused("not enough ore for a batch")
    );

    w.ground.insert(at(10, 2), Stack { item: gold, count: 2 });
    assert_eq!(w.apply(Command::Bend { ability, pos: at(10, 2) }, &defs), Ok(()));
}

#[test]
fn other_nations_cannot_designate_it() {
    let defs = Defs::bundled();
    let gold = ore(&defs);
    for nation in [Nation::Water, Nation::Fire, Nation::Air] {
        let mut w = World::with_map(1, field(&defs), nation, &[at(9, 2)], &defs);
        w.ground.insert(at(10, 2), Stack { item: gold, count: 4 });
        w.ensure_haul_job(at(10, 2));
        let r = w.apply(Command::Bend { ability: metalbend(&defs), pos: at(10, 2) }, &defs);
        assert_eq!(r, Err(CommandError::Bend(BendError::WrongNation)));
    }
}

#[test]
fn designating_takes_over_the_haul_job() {
    let defs = Defs::bundled();
    let gold = ore(&defs);
    let mut w = world(Nation::Earth, true, &defs);
    w.ground.insert(at(10, 2), Stack { item: gold, count: 4 });
    w.ensure_haul_job(at(10, 2));
    assert!(w.jobs().iter().any(|j| matches!(j.kind, JobKind::Haul { .. })));

    w.apply(Command::Bend { ability: metalbend(&defs), pos: at(10, 2) }, &defs).unwrap();
    assert!(
        !w.jobs().iter().any(|j| matches!(j.kind, JobKind::Haul { .. })),
        "the haul job is gone"
    );
}

#[test]
fn cancelling_gives_the_haul_job_back() {
    let defs = Defs::bundled();
    let gold = ore(&defs);
    let mut w = world(Nation::Earth, true, &defs);
    w.ground.insert(at(10, 2), Stack { item: gold, count: 4 });
    w.ensure_haul_job(at(10, 2));
    w.apply(Command::Bend { ability: metalbend(&defs), pos: at(10, 2) }, &defs).unwrap();
    assert!(!w.jobs().iter().any(|j| matches!(j.kind, JobKind::Haul { .. })));

    w.apply(Command::CancelBend { pos: at(10, 2) }, &defs).unwrap();
    assert!(w.jobs().iter().any(|j| j.kind == JobKind::Haul { pos: at(10, 2) }));
    assert_eq!(w.ground_items().next(), Some((at(10, 2), Stack { item: gold, count: 4 })));
}

#[test]
fn a_failed_bend_gives_the_haul_job_back() {
    let defs = Defs::bundled();
    let gold = ore(&defs);
    let mut w = world(Nation::Earth, true, &defs);
    w.ground.insert(at(10, 2), Stack { item: gold, count: 4 });
    w.ensure_haul_job(at(10, 2));
    w.units[0].chi = defs.chi.max;
    w.apply(Command::Bend { ability: metalbend(&defs), pos: at(10, 2) }, &defs).unwrap();
    // The ore gets carried off from under the bend before the channel finishes (shouldn't
    // happen in practice, since designating took over the haul job, but proves the safety net).
    w.ground.remove(&at(10, 2));

    let events = w.bend_events().len();
    run_until(&mut w, &defs, 3000, |w| w.bend_events().len() > events);
    assert_eq!(w.bend_events().last().unwrap().outcome, BendOutcome::Failed("needs ore to bend"));
    assert_eq!(w.units[0].chi, defs.chi.max, "nothing was spent");
}

#[test]
fn a_bend_converts_the_stack_at_the_smelters_ratio() {
    let defs = Defs::bundled();
    let (gold, metal_item) = (ore(&defs), metal(&defs));
    let mut w = world(Nation::Earth, true, &defs);
    w.ground.insert(at(10, 2), Stack { item: gold, count: 6 });
    w.ensure_haul_job(at(10, 2));
    bend(&mut w, &defs, at(10, 2));
    assert_eq!(
        w.ground_items().collect::<Vec<_>>(),
        [(at(10, 2), Stack { item: metal_item, count: 3 })]
    );
    // The metal is new ground, not the ore's old haul job: it still needs hauling to a stockpile.
    assert!(
        w.jobs().iter().any(|j| j.kind == JobKind::Haul { pos: at(10, 2) }),
        "metal needs hauling"
    );
}

#[test]
fn an_odd_pile_leaves_the_remainder_as_ore_with_a_fresh_haul_job() {
    let defs = Defs::bundled();
    let (gold, metal_item) = (ore(&defs), metal(&defs));
    let mut w = world(Nation::Earth, true, &defs);
    w.ground.insert(at(10, 2), Stack { item: gold, count: 5 });
    w.ensure_haul_job(at(10, 2));
    bend(&mut w, &defs, at(10, 2));
    // 5 ore at a ratio of 2 makes 2 metal and leaves 1 ore behind.
    assert_eq!(w.ground.get(&at(10, 2)), Some(&Stack { item: gold, count: 1 }));
    assert!(
        w.jobs().iter().any(|j| j.kind == JobKind::Haul { pos: at(10, 2) }),
        "ore needs hauling"
    );
    let metal_at: Vec<_> = w.ground_items().filter(|(_, s)| s.item == metal_item).collect();
    assert_eq!(metal_at.iter().map(|(_, s)| s.count).sum::<u32>(), 2);
}

#[test]
fn a_unit_can_bend_it_only_with_metalbending_known_and_enough_chi() {
    let defs = Defs::bundled();
    let gold = ore(&defs);
    let mut w = world(Nation::Earth, true, &defs);
    w.ground.insert(at(10, 2), Stack { item: gold, count: 4 });
    w.ensure_haul_job(at(10, 2));
    let ability = metalbend(&defs);
    w.units[0].chi = defs.ability(ability).chi - 1.0;
    assert!(!w.unit_can_bend(0, ability, &defs), "not enough chi");
    w.units[0].chi = defs.chi.max;
    assert!(w.unit_can_bend(0, ability, &defs));
}
