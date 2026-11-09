//! Draw Water (#364): the tech that unlocks it, who can cast it, targets (an empty `Land` tile,
//! with no bank needed since it is already walkable), the temporary `Ford` it leaves and its
//! revert, and that nothing it does strands or kills a settler standing right on the pool.

use super::*;
use crate::geom::Grid;
use crate::mapgen::Tile;

fn at(x: i32, y: i32) -> Pos {
    Pos::new(x, y)
}

fn biome(defs: &Defs, id: &str) -> BiomeId {
    BiomeId(defs.biomes.iter().position(|b| b.id == id).unwrap() as u8)
}

/// An 11x9 plains field, open ground throughout except a river at x = 0 and a mountain block in
/// the corner (3..=5, 0..=2).
fn field(defs: &Defs) -> TileMap {
    let plains = biome(defs, "plains");
    Grid::from_fn(11, 9, |p| Tile {
        elevation: 100,
        temperature: 128,
        moisture: 128,
        terrain: match p {
            _ if p.x == 0 => Terrain::River,
            _ if (3..=5).contains(&p.x) && (0..=2).contains(&p.y) => Terrain::Mountain,
            _ => Terrain::Land,
        },
        biome: plains,
        resource: None,
        building: None,
    })
}

/// A world on `field`, with `irrigation` known when `known` is set.
fn world(nation: Nation, known: bool, defs: &Defs) -> World {
    let mut w = World::with_map(1, field(defs), nation, &[at(6, 6)], defs);
    if known {
        w.research.grant(defs.tech_id("irrigation").unwrap());
        w.refresh_modifiers(defs);
    }
    w
}

fn draw_water(defs: &Defs) -> AbilityId {
    defs.ability_id("draw_water").unwrap()
}

fn run_until(w: &mut World, defs: &Defs, limit: u64, mut done: impl FnMut(&World) -> bool) -> u64 {
    for n in 0..limit {
        if done(w) {
            return n;
        }
        w.units.iter_mut().for_each(|u| u.needs = crate::needs::Needs::full(&defs.needs));
        w.tick(defs);
    }
    panic!("not done after {limit} ticks");
}

/// Casts Draw Water on `pos`, with full chi, and runs until it finishes, which it should do
/// successfully.
fn draw(w: &mut World, defs: &Defs, pos: Pos) {
    for u in &mut w.units {
        u.chi = defs.chi.max;
    }
    w.apply(Command::Bend { ability: draw_water(defs), pos }, defs).unwrap();
    let events = w.bend_events().len();
    run_until(w, defs, 3000, |w| w.bend_events().len() > events);
    assert_eq!(w.bend_events().last().unwrap().outcome, BendOutcome::Done);
}

#[test]
fn draw_water_is_locked_until_irrigation_and_belongs_to_water() {
    let defs = Defs::bundled();
    let (ability, tech) = (draw_water(&defs), defs.tech_id("irrigation").unwrap());
    assert_eq!(defs.ability(ability).unlocked_by, Some(tech));
    assert_eq!(defs.ability(ability).nation, Nation::Water);
    let w = world(Nation::Water, false, &defs);
    assert_eq!(w.check_ability(ability, &defs), Err(BendError::Locked { tech }));
    let w = world(Nation::Water, true, &defs);
    assert_eq!(w.check_ability(ability, &defs), Ok(()));
    for nation in [Nation::Earth, Nation::Fire, Nation::Air] {
        let w = world(nation, false, &defs);
        assert_eq!(w.check_ability(ability, &defs), Err(BendError::WrongNation), "{nation:?}");
    }
}

#[test]
fn targets_must_be_empty_land_with_no_bank_needed() {
    let defs = Defs::bundled();
    let mut w = world(Nation::Water, true, &defs);
    let tree = defs.resource_id("oak_tree").unwrap();
    w.map[at(7, 7)].resource = Some(tree);
    w.zones.insert(at(8, 7));
    let wood = defs.item_id("wood").unwrap();
    w.ground.insert(at(9, 7), Stack { item: wood, count: 1 });
    let ability = draw_water(&defs);
    let bend = |w: &mut World, pos| w.apply(Command::Bend { ability, pos }, &defs);
    let refused = |why| Err(CommandError::Bend(BendError::InvalidTarget(why)));

    // An ordinary open land tile, with nobody beside it to call a bank: Draw Water's target is
    // already walkable, so there's no bank or chain requirement the way Raise Earth has one.
    assert_eq!(bend(&mut w, at(4, 4)), Ok(()));
    w.apply(Command::CancelBend { pos: at(4, 4) }, &defs).unwrap();
    // The mountain block at (3..=5, 0..=2) is surrounded by dry land, but it isn't dry land.
    assert_eq!(bend(&mut w, at(4, 1)), refused("needs dry land"));
    assert_eq!(bend(&mut w, at(0, 4)), refused("needs dry land"));
    // A resource, a stockpile tile and a ground stack are all "something in the way".
    assert_eq!(bend(&mut w, at(7, 7)), refused("something is in the way"));
    assert_eq!(bend(&mut w, at(8, 7)), refused("something is in the way"));
    assert_eq!(bend(&mut w, at(9, 7)), refused("something is in the way"));
    assert_eq!(bend(&mut w, at(-1, 0)), refused("off the map"));

    // Once a pool exists, it can't be re-targeted until it reverts.
    draw(&mut w, &defs, at(4, 4));
    assert_eq!(bend(&mut w, at(4, 4)), refused("already a ford"));
}

#[test]
fn a_bend_leaves_a_temporary_ford_that_drains_back_to_land() {
    let defs = Defs::bundled();
    let mut w = world(Nation::Water, true, &defs);
    let pos = at(4, 4);
    draw(&mut w, &defs, pos);
    assert_eq!(w.map[pos].terrain, Terrain::Ford);
    assert_eq!(w.map[at(4, 3)].terrain, Terrain::Land, "only the target changed");
    assert!((w.units[0].chi - 70.0).abs() < 0.1, "{}", w.units[0].chi);

    // 10 minutes x water_work (2.0 base x irrigation's 1.15 = 2.3) is 23 minutes, or 13800 ticks.
    let factor = w.modifiers().water_work;
    assert!((factor - 2.3).abs() < 1e-4, "{factor}");
    let full = minutes_to_ticks_min1(10.0 * factor);
    assert_eq!(full, 13_800);
    let cast_at = w.tick;
    run_until(&mut w, &defs, full + 10, |w| w.map[pos].terrain == Terrain::Land);
    assert!(w.tick.abs_diff(cast_at + full) <= 1, "{}", w.tick - cast_at);
    // It stays Land afterwards: nothing keeps reverting it back and forth.
    for _ in 0..50 {
        w.tick(&defs);
    }
    assert_eq!(w.map[pos].terrain, Terrain::Land);
}

#[test]
fn the_magnitude_modifier_scales_revert_minutes() {
    let fast = "
        [[ability]]
        id = 'fast_draw'
        nation = 'water'
        name = 'Fast draw'
        verb = 'Draw'
        chi = 10.0
        work = 0.0
        magnitude_modifier = 'water_work'
        [ability.effect]
        kind = 'reshape'
        from = ['land']
        to = 'ford'
        revert_minutes = 2.0
    ";
    let defs = crate::bending::testing::load(fast, "").unwrap();
    let mut w = world(Nation::Water, false, &defs);
    let ability = defs.ability_id("fast_draw").unwrap();
    let pos = at(4, 4);
    w.apply(Command::Bend { ability, pos }, &defs).unwrap();
    run_until(&mut w, &defs, 200, |w| !w.bend_events().is_empty());
    let factor = w.modifiers().water_work;
    assert!((factor - 2.0).abs() < 1e-4, "{factor}");
    let cast_at = w.tick;
    run_until(&mut w, &defs, 5000, |w| w.map[pos].terrain == Terrain::Land);
    let took = w.tick - cast_at;
    let want = minutes_to_ticks_min1(2.0 * factor);
    assert!(took.abs_diff(want) <= 1, "{took} vs {want}");
}

#[test]
fn the_pool_is_drinkable_and_lets_water_the_land_reach_away_from_any_river() {
    let defs = Defs::bundled();
    let mut w = world(Nation::Water, true, &defs);
    let water_the_land = defs.ability_id("water_the_land").unwrap();
    let far = at(6, 6); // more than source_range (4) tiles from the river at x = 0.
    let refused = Err(CommandError::Bend(BendError::InvalidTarget("needs fresh water nearby")));
    assert_eq!(w.apply(Command::Bend { ability: water_the_land, pos: far }, &defs), refused);

    // Draw a pool within range of `far` but still far from the river.
    let pool = at(5, 6);
    draw(&mut w, &defs, pool);
    assert_eq!(w.map[pool].terrain, Terrain::Ford);
    assert_eq!(
        w.apply(Command::Bend { ability: water_the_land, pos: far }, &defs),
        Ok(()),
        "the pool counts as fresh water for Water the Land's source_range"
    );
    w.apply(Command::CancelBend { pos: far }, &defs).unwrap();

    // It's drinkable too: a thirsty unit beside it walks there to drink, the same as a river.
    assert!(w.map.neighbors(at(4, 6), true).any(|q| w.map[q].terrain.is_water()));
}

#[test]
fn a_settler_standing_on_the_pool_is_not_stranded_or_killed_when_it_reverts() {
    let defs = Defs::bundled();
    let mut w = world(Nation::Water, true, &defs);
    let pos = at(4, 4);
    draw(&mut w, &defs, pos);
    assert_eq!(w.map[pos].terrain, Terrain::Ford);
    let cast_at = w.tick;
    let full = minutes_to_ticks_min1(10.0 * w.modifiers().water_work);

    // Plant the settler right on the pool and warm the connectivity cache while it's still a
    // ford, so a stale "unreachable" answer from before the revert would be visible below.
    w.units[0].pos = pos;
    w.units[0].route = None;
    w.ensure_regions();
    assert_eq!(w.job_unreachable(at(9, 8)), Some(false));

    // Jump straight to the tick before the pool reverts and plant the settler there again (an
    // idle unit wanders on its own, which isn't what's under test here), then run the one real
    // tick that reverts it: `World::tick` runs `tick_reverts` before units act, so this is the
    // settler's first chance to move off the tile, not its last chance to avoid the revert.
    w.tick = cast_at + full - 1;
    w.units[0].pos = pos;
    w.tick(&defs);
    assert_eq!(w.map[pos].terrain, Terrain::Land, "reverted on schedule");
    assert!(w.unit(w.units[0].id).is_some(), "the settler standing on the pool survives");
    assert!(w.deaths.is_empty());
    assert!(w.map[pos].terrain.is_walkable(), "still walkable Land after reverting");

    // The regions cache was marked dirty by the revert (not left stale), and a far tile is still
    // reachable once it's recomputed: the whole map was always one open region either way.
    w.ensure_regions();
    assert_eq!(w.job_unreachable(at(9, 8)), Some(false));

    // The settler on the reverted tile can still be given and finish ordinary work.
    let oak = defs.resource_id("oak_tree").unwrap();
    w.map[at(9, 8)].resource = Some(oak);
    w.map_changed();
    w.apply(Command::DesignateStockpile { pos: at(1, 1) }, &defs).unwrap();
    w.apply(Command::DesignateHarvest { pos: at(9, 8) }, &defs).unwrap();
    let wood = defs.item_id("wood").unwrap();
    run_until(&mut w, &defs, 3000, |w| w.stored_total(wood) > 0);
}
