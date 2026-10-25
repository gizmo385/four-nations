//! Bore Tunnel and mining inside mountains (#132): the tech that unlocks it, its targets, the
//! tunnel it leaves, the richer yield of a vein beside one, and mining staying open to everyone.

use super::*;
use crate::geom::Grid;
use crate::jobs::ItemId;
use crate::mapgen::Tile;

fn at(x: i32, y: i32) -> Pos {
    Pos::new(x, y)
}

/// A 12x7 plains field: open ground in columns 0 to 3 and a mountain from column 4 east. A gold
/// vein sits on the mountain's edge at (4, 3), beside open ground, and another deep inside at
/// (8, 3); a stone outcrop on the edge at (4, 5).
fn field(defs: &Defs) -> TileMap {
    let plains = BiomeId(defs.biomes.iter().position(|b| b.id == "plains").unwrap() as u8);
    let gold = defs.resource_id("gold_vein");
    let stone = defs.resource_id("stone");
    Grid::from_fn(12, 7, |p| Tile {
        elevation: 200,
        temperature: 128,
        moisture: 128,
        terrain: if p.x >= 4 { Terrain::Mountain } else { Terrain::Land },
        biome: plains,
        resource: match (p.x, p.y) {
            (4, 3) | (8, 3) => gold,
            (4, 5) => stone,
            _ => None,
        },
        building: None,
    })
}

/// An Earth world on `field`, with `tunneling` known when `tunnels` is set.
fn world(nation: Nation, tunnels: bool, defs: &Defs) -> World {
    let mut w = World::with_map(1, field(defs), nation, &[at(2, 3)], defs);
    if tunnels {
        w.research.grant(tunneling(defs));
        w.refresh_modifiers(defs);
    }
    w
}

fn bore(defs: &Defs) -> AbilityId {
    defs.ability_id("bore_tunnel").unwrap()
}

fn tunneling(defs: &Defs) -> TechId {
    defs.tech_id("tunneling").unwrap()
}

fn item(defs: &Defs, id: &str) -> ItemId {
    defs.items.iter().position(|i| i.id == id).map(|i| ItemId(i as u16)).unwrap()
}

fn run_until(w: &mut World, defs: &Defs, limit: u64, mut done: impl FnMut(&World) -> bool) {
    for _ in 0..limit {
        if done(w) {
            return;
        }
        // Long runs get hungry; that is not what is under test.
        for u in &mut w.units {
            u.needs = crate::needs::Needs::full(&defs.needs);
        }
        w.tick(defs);
    }
    panic!("not done after {limit} ticks");
}

/// Bores `pos` and runs until the bend has finished, which it should have done successfully.
fn bore_at(w: &mut World, defs: &Defs, pos: Pos) {
    w.units[0].chi = defs.chi.max;
    w.apply(Command::Bend { ability: bore(defs), pos }, defs).unwrap();
    let events = w.bend_events().len();
    run_until(w, defs, 3000, |w| w.bend_events().len() > events);
    assert_eq!(w.bend_events().last().unwrap().outcome, BendOutcome::Done);
}

/// Mines the resource on `pos` and returns how many of `item` it dropped (nothing is
/// stockpiled, so the yield stays on the ground).
fn mine(w: &mut World, defs: &Defs, pos: Pos, item: ItemId) -> u32 {
    w.apply(Command::DesignateHarvest { pos }, defs).unwrap();
    run_until(w, defs, 4000, |w| w.map[pos].resource.is_none());
    w.ground_items().filter(|(_, s)| s.item == item).map(|(_, s)| s.count).sum()
}

#[test]
fn bore_tunnel_is_locked_until_tunneling_is_known() {
    let defs = Defs::bundled();
    let (ability, tech) = (bore(&defs), tunneling(&defs));
    assert_eq!(defs.ability(ability).unlocked_by, Some(tech));
    assert_eq!(defs.ability(ability).nation, Nation::Earth);

    let mut w = world(Nation::Earth, false, &defs);
    assert_eq!(w.check_ability(ability, &defs), Err(BendError::Locked { tech }));
    assert_eq!(
        w.apply(Command::Bend { ability, pos: at(4, 2) }, &defs),
        Err(CommandError::Bend(BendError::Locked { tech }))
    );
    assert!(w.command_log().is_empty() && w.jobs().is_empty());

    w.research.grant(tech);
    assert_eq!(w.check_ability(ability, &defs), Ok(()));
    assert_eq!(w.apply(Command::Bend { ability, pos: at(4, 2) }, &defs), Ok(()));
}

#[test]
fn bore_tunnel_belongs_to_earth_alone() {
    let defs = Defs::bundled();
    let tech = tunneling(&defs);
    for nation in [Nation::Water, Nation::Fire, Nation::Air] {
        assert!(defs.techs_for(nation).all(|(t, _)| t != tech), "{nation:?} reaches tunneling");
        let w = world(nation, false, &defs);
        assert_eq!(w.check_ability(bore(&defs), &defs), Err(BendError::WrongNation));
    }
}

#[test]
fn targets_are_bare_mountain_tiles_beside_open_ground() {
    let defs = Defs::bundled();
    let mut w = world(Nation::Earth, true, &defs);
    w.map[at(5, 0)].terrain = Terrain::Tunnel;
    w.map_changed();
    let ability = bore(&defs);
    let bend = |w: &mut World, pos| w.apply(Command::Bend { ability, pos }, &defs);
    let refused = |why| Err(CommandError::Bend(BendError::InvalidTarget(why)));

    assert_eq!(bend(&mut w, at(4, 2)), Ok(()), "rock beside open ground");
    assert_eq!(bend(&mut w, at(6, 0)), Ok(()), "rock beside a tunnel");
    assert_eq!(bend(&mut w, at(2, 2)), refused("needs a mountain tile"));
    assert_eq!(bend(&mut w, at(5, 0)), refused("already a tunnel"));
    assert_eq!(bend(&mut w, at(4, 5)), refused("something is in the way"));
    assert_eq!(bend(&mut w, at(6, 3)), refused("needs dry ground beside it"));
    assert_eq!(w.command_log().len(), 2);
}

#[test]
fn a_bore_leaves_a_walkable_tunnel_and_spends_the_chi() {
    let defs = Defs::bundled();
    let mut w = world(Nation::Earth, true, &defs);
    w.units[0].pos = at(3, 2);
    bore_at(&mut w, &defs, at(4, 2));
    assert_eq!(w.map[at(4, 2)].terrain, Terrain::Tunnel);
    assert_eq!(w.map[at(5, 2)].terrain, Terrain::Mountain, "only the target changed");
    let cost = defs.ability(bore(&defs)).chi;
    assert!((w.units[0].chi - (defs.chi.max - cost)).abs() < 0.5, "{}", w.units[0].chi);
    assert!(w.rules.passable(&w.map[at(4, 2)]), "a tunnel is walked like open ground");
    // Nothing can be built in one.
    let bed = defs.building_id("bed").unwrap();
    assert!(w.apply(Command::DesignateBuild { building: bed, pos: at(4, 2) }, &defs).is_err());
}

#[test]
fn a_tunnel_reaches_a_vein_deep_inside_the_mountain() {
    let defs = Defs::bundled();
    let gold = item(&defs, "gold_ore");
    let mut w = world(Nation::Earth, true, &defs);
    w.apply(Command::DesignateHarvest { pos: at(8, 3) }, &defs).unwrap();
    for _ in 0..1500 {
        w.tick(&defs);
    }
    assert!(w.map[at(8, 3)].resource.is_some(), "the deep vein can't be reached through rock");

    // Bore in a tile at a time, each from the last.
    for x in 4..=7 {
        let pos = at(x, 2);
        bore_at(&mut w, &defs, pos);
    }
    run_until(&mut w, &defs, 6000, |w| w.map[at(8, 3)].resource.is_none());
    let mined: u32 = w.ground_items().filter(|(_, s)| s.item == gold).map(|(_, s)| s.count).sum();
    let h = defs.harvest(defs.resource_id("gold_vein").unwrap()).unwrap();
    assert_eq!(mined, h.tunnel_count.unwrap(), "mined from inside the mountain");
}

#[test]
fn the_same_vein_yields_more_mined_from_a_tunnel() {
    let defs = Defs::bundled();
    for (resource, pos, item_id) in
        [("gold_vein", at(4, 3), "gold_ore"), ("stone", at(4, 5), "stone")]
    {
        let h = defs.harvest(defs.resource_id(resource).unwrap()).unwrap();
        let item = item(&defs, item_id);

        let mut plain = world(Nation::Earth, true, &defs);
        assert_eq!(mine(&mut plain, &defs, pos, item), h.count, "{resource} from outside");

        // A tunnel bored beside it, one tile north.
        let mut tunnelled = world(Nation::Earth, true, &defs);
        bore_at(&mut tunnelled, &defs, Pos::new(pos.x, pos.y - 1));
        let more = mine(&mut tunnelled, &defs, pos, item);
        assert_eq!(more, h.tunnel_count.unwrap(), "{resource} from a tunnel");
        assert!(more > h.count, "{resource}");
    }
}

#[test]
fn mining_stays_open_to_every_nation_without_tunneling() {
    let defs = Defs::bundled();
    let tech = tunneling(&defs);
    for resource in ["gold_vein", "stone"] {
        let h = defs.harvest(defs.resource_id(resource).unwrap()).unwrap();
        assert_ne!(h.unlocked_by, Some(tech), "{resource} isn't gated behind tunneling");
    }
    for nation in Nation::ALL {
        let mut w = world(nation, false, &defs);
        // Stone is stoneworking's (a shared tech Earth starts with); gold needs nothing.
        w.research.grant(defs.tech_id("stoneworking").unwrap());
        assert!(!w.research.is_known(tech));
        for pos in [at(4, 3), at(4, 5)] {
            let r = w.apply(Command::DesignateHarvest { pos }, &defs);
            assert!(r.is_ok(), "{nation:?} at {pos:?}: {r:?}");
        }
        let gold = item(&defs, "gold_ore");
        run_until(&mut w, &defs, 6000, |w| w.map[at(4, 3)].resource.is_none());
        let mined: u32 =
            w.ground_items().filter(|(_, s)| s.item == gold).map(|(_, s)| s.count).sum();
        assert_eq!(mined, 1, "{nation:?}");
    }
}
