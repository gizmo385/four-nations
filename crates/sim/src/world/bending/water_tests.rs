//! Water the Land (#69): targets, wet patches, sprouting, fallow, and replay.

use super::*;
use crate::bending::testing;
use crate::geom::Grid;
use crate::jobs::ItemId;
use crate::mapgen::Tile;

fn at(x: i32, y: i32) -> Pos {
    Pos::new(x, y)
}

fn biome(defs: &Defs, id: &str) -> BiomeId {
    BiomeId(defs.biomes.iter().position(|b| b.id == id).unwrap() as u8)
}

/// An 11x9 plains field with a river down its west edge (x = 0).
fn field(defs: &Defs) -> TileMap {
    let plains = biome(defs, "plains");
    Grid::from_fn(11, 9, |p| Tile {
        elevation: 100,
        temperature: 128,
        moisture: 128,
        terrain: if p.x == 0 { Terrain::River } else { Terrain::Land },
        biome: plains,
        resource: None,
        building: None,
    })
}

fn world(seed: u64, defs: &Defs) -> World {
    World::with_map(seed, field(defs), Nation::Water, &[at(4, 4)], defs)
}

fn water(defs: &Defs) -> AbilityId {
    defs.ability_id("water_the_land").unwrap()
}

fn cabbage_item(defs: &Defs) -> ItemId {
    defs.items.iter().position(|i| i.id == "cabbage").map(|i| ItemId(i as u16)).unwrap()
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

/// Waters `pos` and runs until the patch has dried and sprouted.
fn water_and_dry(w: &mut World, defs: &Defs, pos: Pos) {
    // Long runs get hungry and spend chi; neither is what is under test.
    w.units[0].needs = crate::needs::Needs::full(&defs.needs);
    w.units[0].chi = defs.chi.max;
    w.apply(Command::Bend { ability: water(defs), pos }, defs).unwrap();
    let events = w.bend_events().count();
    run_until(w, defs, 1000, |w| w.bend_events().count() > events);
    assert_eq!(w.bend_events().next_back().unwrap().outcome, BendOutcome::Done);
    run_until(w, defs, 4000, |w| w.wet_tiles().is_empty());
}

fn cabbages(w: &World, defs: &Defs) -> Vec<Pos> {
    let cabbage = defs.resource_id("cabbage");
    w.map
        .iter()
        .filter(|(_, t)| t.resource.is_some() && t.resource == cabbage)
        .map(|(p, _)| p)
        .collect()
}

#[test]
fn targets_must_be_open_ground_near_fresh_water() {
    let defs = Defs::bundled();
    let mut w = world(1, &defs);
    w.map[at(8, 8)].terrain = Terrain::Mountain;
    w.map[at(9, 1)].terrain = Terrain::Water;
    w.map[at(6, 6)].terrain = Terrain::Lake;
    w.map[at(1, 7)].terrain = Terrain::Coast;
    w.map[at(10, 6)].terrain = Terrain::Ford;
    w.map_changed();
    let ability = water(&defs);
    let bend = |w: &mut World, pos| w.apply(Command::Bend { ability, pos }, &defs);
    let refused = |why| Err(CommandError::Bend(BendError::InvalidTarget(why)));
    // Four tiles from the river is in range, five is not.
    assert_eq!(bend(&mut w, at(4, 1)), Ok(()));
    assert_eq!(bend(&mut w, at(5, 1)), refused("needs fresh water nearby"));
    // Rock and water are not ground.
    assert_eq!(bend(&mut w, at(8, 8)), refused("needs open ground to water"));
    assert_eq!(bend(&mut w, at(9, 1)), refused("needs open ground to water"));
    assert_eq!(bend(&mut w, at(0, 3)), refused("needs open ground to water"));
    // A lake counts, in any direction, and so does a coast tile.
    assert_eq!(bend(&mut w, at(9, 8)), Ok(()));
    assert_eq!(bend(&mut w, at(1, 7)), Ok(()));
    // A ford counts.
    assert_eq!(bend(&mut w, at(10, 1)), refused("needs fresh water nearby"));
    assert_eq!(bend(&mut w, at(10, 2)), Ok(()));
    assert_eq!(bend(&mut w, at(-1, 0)), refused("off the map"));
    assert_eq!(w.command_log().len(), 4);
}

#[test]
fn a_patch_is_wet_for_six_minutes_then_two_tiles_sprout() {
    let defs = Defs::bundled();
    let mut w = world(1, &defs);
    w.apply(Command::Bend { ability: water(&defs), pos: at(3, 4) }, &defs).unwrap();
    run_until(&mut w, &defs, 1000, |w| w.bend_events().next().is_some());
    let done = w.tick;
    let wet = w.wet_tiles();
    assert_eq!(wet.len(), 9);
    for &(p, seconds) in &wet {
        assert!((p.x - 3).abs() <= 1 && (p.y - 4).abs() <= 1, "{p:?}");
        assert!((seconds - 360.0).abs() < 0.2, "{seconds}");
    }
    assert!(wet.windows(2).all(|p| p[0].0 < p[1].0), "position order");
    assert!(cabbages(&w, &defs).is_empty());
    assert!(w.fallow_tiles().is_empty());

    // Almost dry, nothing yet; then exactly two cabbages inside the patch.
    run_until(&mut w, &defs, 4000, |w| w.tick == done + 3599);
    assert!(cabbages(&w, &defs).is_empty());
    assert_eq!(w.wet_tiles().len(), 9);
    w.tick(&defs);
    assert!(w.wet_tiles().is_empty());
    let sprouted = cabbages(&w, &defs);
    assert_eq!(sprouted.len(), 2);
    assert!(sprouted.iter().all(|p| (p.x - 3).abs() <= 1 && (p.y - 4).abs() <= 1));
    // Those two are fallow for half an hour.
    let fallow = w.fallow_tiles();
    assert_eq!(fallow.iter().map(|f| f.0).collect::<Vec<_>>(), sprouted);
    assert!(fallow.iter().all(|f| (f.1 - 1800.0).abs() < 0.2));
    // They are not resting: they can be gathered straight away.
    assert!(sprouted.iter().all(|&p| !w.is_resting(p)));
}

#[test]
fn the_same_seed_sprouts_the_same_tiles_and_seeds_differ() {
    let defs = Defs::bundled();
    let sprouts = |seed| {
        let mut w = world(seed, &defs);
        water_and_dry(&mut w, &defs, at(3, 4));
        cabbages(&w, &defs)
    };
    assert_eq!(sprouts(5), sprouts(5));
    let all: Vec<_> = (1..=8).map(sprouts).collect();
    assert!(all.iter().any(|s| *s != all[0]), "the stream should depend on the seed");
}

#[test]
fn a_fallow_tile_does_not_sprout_again_until_its_time_ends() {
    let defs = Defs::bundled();
    let mut w = world(3, &defs);
    let mut seen: Vec<Pos> = Vec::new();
    // Nine tiles, two per watering: the fourth watering finds only three that never sprouted,
    // and the fifth finds one.
    for (round, expect) in [2, 2, 2, 2, 1].into_iter().enumerate() {
        water_and_dry(&mut w, &defs, at(3, 4));
        let now = cabbages(&w, &defs);
        assert_eq!(now.len(), expect, "round {round}");
        assert!(now.iter().all(|p| !seen.contains(p)), "round {round}: {now:?} after {seen:?}");
        // Clear them, so only the fallow time keeps a tile from sprouting again.
        for &p in &now {
            w.map[p].resource = None;
        }
        w.map_changed();
        seen.extend(now);
    }
    // Every tile of the patch is fallow now.
    assert_eq!(w.fallow_tiles().len(), 9);
}

#[test]
fn fallow_ends_after_thirty_minutes_and_the_tile_sprouts_again() {
    let defs = Defs::bundled();
    let mut w = world(3, &defs);
    water_and_dry(&mut w, &defs, at(3, 4));
    let first = cabbages(&w, &defs);
    assert_eq!(first.len(), 2);
    let sprouted_at = w.tick;
    run_until(&mut w, &defs, 18_100, |w| w.fallow_tiles().is_empty());
    assert!(w.tick.abs_diff(sprouted_at + 18_000) <= 1, "{}", w.tick - sprouted_at);
    // Clear the old ones and water until they sprout again: with the other tiles also fallow-free
    // by now, any of the nine may be chosen, so just check the fallow list is rebuilt.
    for &p in &first {
        w.map[p].resource = None;
    }
    w.map_changed();
    water_and_dry(&mut w, &defs, at(3, 4));
    assert_eq!(w.fallow_tiles().len(), 2);
}

#[test]
fn only_empty_fertile_unused_tiles_sprout() {
    let defs = Defs::bundled();
    let mut base = world(2, &defs);
    let desert = biome(&defs, "desert");
    let tree = defs.resource_id("oak_tree").unwrap();
    let wood = defs.items.iter().position(|i| i.id == "wood").map(|i| ItemId(i as u16)).unwrap();
    // Of the nine tiles (2..=4, 3..=5): desert, tree, stockpile and ground stack leave five.
    base.map[at(2, 3)].biome = desert;
    base.map[at(3, 3)].resource = Some(tree);
    base.zones.insert(at(4, 3));
    base.ground.insert(at(2, 4), Stack { item: wood, count: 1 });
    base.map_changed();
    let free = [at(3, 4), at(4, 4), at(2, 5), at(3, 5), at(4, 5)];
    for seed in 1..=6 {
        let mut w = base.clone();
        w.sprout_rng = crate::rng::stream(seed, "bending.sprout");
        water_and_dry(&mut w, &defs, at(3, 4));
        let sprouted = cabbages(&w, &defs);
        assert_eq!(sprouted.len(), 2);
        assert!(sprouted.iter().all(|p| free.contains(p)), "{sprouted:?}");
    }
    // A desert patch never sprouts and leaves no fallow.
    let mut dry = world(2, &defs);
    for y in 0..9 {
        for x in 0..11 {
            dry.map[at(x, y)].biome = desert;
        }
    }
    water_and_dry(&mut dry, &defs, at(3, 4));
    assert!(cabbages(&dry, &defs).is_empty());
    assert!(dry.fallow_tiles().is_empty());
}

/// #226: a unit standing on a tile only because it is hidden inside a building there (its door)
/// doesn't hold the tile the way a unit that is actually standing on the map does.
#[test]
fn a_unit_inside_a_building_does_not_block_its_door_from_sprouting() {
    let mut defs = Defs::bundled();
    let hall = defs.building_id("bed").unwrap();
    {
        let def = &mut defs.buildings[hall.0 as usize];
        def.footprint = (2, 2);
        def.bed = None;
        def.passable = false;
        def.capacity = 1;
    }
    let mut w = world(3, &defs);
    // Anchored on (3, 1): its footprint (rows 1-2) sits clear of the 3x3 patch centred on
    // (3, 4) (rows 3-5), but its door, the ring tile below it, lands inside that patch.
    let anchor = at(3, 1);
    let foot = crate::geom::Footprint::new(anchor, 2, 2);
    w.raise(foot, hall, &defs);
    let door = foot.entrance();
    assert_eq!(door, at(3, 3));
    w.units[0].pos = door;
    assert!(w.enter_building(0, anchor, &defs));

    let patch = Patch {
        centre: at(3, 4),
        dries_at: 0,
        sprouts: 2,
        resource: defs.resource_id("cabbage").unwrap(),
        biomes: vec![biome(&defs, "plains")],
        fallow_ticks: 1,
    };
    assert!(w.can_sprout(door, &patch, &defs), "the hidden unit doesn't hold its door");

    // The same tile, visibly occupied instead, does block it, as it always has.
    w.leave_building(0);
    assert!(!w.can_sprout(door, &patch, &defs));
}

#[test]
fn a_patch_at_the_edge_and_shore_is_clipped() {
    let defs = Defs::bundled();
    let mut w = world(1, &defs);
    // Centre (1, 0): the river column and the map's top row are not wettable.
    w.apply(Command::Bend { ability: water(&defs), pos: at(1, 0) }, &defs).unwrap();
    run_until(&mut w, &defs, 1000, |w| w.bend_events().next().is_some());
    let wet: Vec<Pos> = w.wet_tiles().iter().map(|t| t.0).collect();
    assert_eq!(wet, [at(1, 0), at(1, 1), at(2, 0), at(2, 1)]);
}

#[test]
fn sprouted_cabbage_is_gathered_like_wild_cabbage() {
    let defs = Defs::bundled();
    let mut w = world(1, &defs);
    water_and_dry(&mut w, &defs, at(3, 4));
    let sprouted = cabbages(&w, &defs);
    let item = cabbage_item(&defs);
    w.apply(Command::DesignateStockpile { pos: at(8, 4) }, &defs).unwrap();
    w.apply(Command::DesignateHarvest { pos: sprouted[0] }, &defs).unwrap();
    run_until(&mut w, &defs, 3000, |w| w.stored_total(item) == 1);
    // Worked, so it rests like any cabbage (regrow data).
    assert!(w.is_resting(sprouted[0]));
    assert!(!w.is_resting(sprouted[1]));
}

#[test]
fn nothing_sprouts_under_a_unit() {
    let defs = Defs::bundled();
    let mut w = world(1, &defs);
    water_and_dry(&mut w, &defs, at(3, 4));
    assert!(w.units.iter().all(|u| w.map[u.pos].resource.is_none()));
}

#[test]
fn the_magnitude_modifier_scales_how_long_a_patch_stays_wet() {
    let slow = "
        [[ability]]
        id = 'slow_water'
        nation = 'water'
        name = 'Slow water'
        verb = 'Water'
        chi = 10.0
        work = 0.0
        magnitude_modifier = 'water_work'
        [ability.effect]
        kind = 'irrigate'
        source_range = 4
        wet_minutes = 6.0
        sprouts = 2
        sprout_resource = 'cabbage'
        biomes = ['plains']
        fallow_minutes = 30.0
    ";
    let defs = testing::load(slow, "").unwrap();
    let mut w = world(1, &defs);
    let ability = defs.ability_id("slow_water").unwrap();
    w.apply(Command::Bend { ability, pos: at(3, 4) }, &defs).unwrap();
    run_until(&mut w, &defs, 200, |w| w.bend_events().next().is_some());
    let factor = w.modifiers().water_work;
    assert!(factor > 1.0);
    let left = w.wet_tiles()[0].1;
    assert!((left - 360.0 * factor).abs() < 0.2, "{left}");
}

/// Everything observable about the world's water, through its public API.
fn snapshot(w: &World, defs: &Defs) -> String {
    let units: Vec<_> = w.units.iter().map(|u| (u.id, u.pos, u.chi.to_bits(), u.task())).collect();
    let resources: Vec<_> =
        w.map.iter().filter(|(_, t)| t.resource.is_some()).map(|(p, _)| p).collect();
    format!(
        "tick {} {units:?} {resources:?} wet {:?} fallow {:?} {:?} stored {}",
        w.tick,
        w.wet_tiles(),
        w.fallow_tiles(),
        w.bend_events().collect::<Vec<_>>(),
        w.stored_total(cabbage_item(defs)),
    )
}

fn session_world(defs: &Defs) -> World {
    World::with_map(11, field(defs), Nation::Water, &[at(4, 4), at(5, 6)], defs)
}

fn play_session(defs: &Defs) -> World {
    let ability = water(defs);
    let mut w = session_world(defs);
    for t in 0..9000u64 {
        match t {
            0 => {
                w.apply(Command::Bend { ability, pos: at(3, 4) }, defs).unwrap();
                w.apply(Command::Bend { ability, pos: at(2, 7) }, defs).unwrap();
                w.apply(Command::DesignateStockpile { pos: at(8, 4) }, defs).unwrap();
            }
            50 => w.apply(Command::CancelBend { pos: at(2, 7) }, defs).unwrap(),
            2000 => w.apply(Command::Bend { ability, pos: at(3, 5) }, defs).unwrap(),
            4100 => {
                // Gather whatever has sprouted.
                let sprouted: Vec<Pos> =
                    w.map.iter().filter(|(_, t)| t.resource.is_some()).map(|(p, _)| p).collect();
                for pos in sprouted {
                    w.apply(Command::DesignateHarvest { pos }, defs).unwrap();
                }
            }
            _ => {}
        }
        w.tick(defs);
    }
    w
}

#[test]
fn a_session_with_water_the_land_replays_identically() {
    let defs = Defs::bundled();
    let original = play_session(&defs);
    let log = original.command_log().to_vec();
    assert!(log.len() >= 5);
    assert!(
        original.bend_events().count() >= 2
            && !original.fallow_tiles().is_empty()
            && original.stored_total(cabbage_item(&defs)) > 0,
        "the session should water, sprout and gather, or the test proves little: {}",
        snapshot(&original, &defs)
    );
    let mut replayed = session_world(&defs);
    replayed.replay(&log, original.tick, &defs);
    assert_eq!(snapshot(&replayed, &defs), snapshot(&original, &defs));
    assert_eq!(replayed.command_log(), original.command_log());
}

#[test]
fn replay_without_the_harvest_commands_diverges() {
    let defs = Defs::bundled();
    let original = play_session(&defs);
    let mut log = original.command_log().to_vec();
    log.retain(|r| !matches!(r.command, Command::DesignateHarvest { .. }));
    let mut replayed = session_world(&defs);
    replayed.replay(&log, original.tick, &defs);
    assert_ne!(snapshot(&replayed, &defs), snapshot(&original, &defs));
}
