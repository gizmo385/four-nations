//! Cut Ice (#364): the tech that unlocks it, who can cast it, the `draw_item` target (a river,
//! lake or sea tile beside passable ground), and that the ice lands on the bender's own tile
//! instead of the water. An igloo built from that ice closes the loop.

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

/// An 11x7 plains field with a river down column 5, a lake tile at (2, 2) and sea at (8, 0).
fn field(defs: &Defs) -> TileMap {
    let plains = biome(defs, "plains");
    Grid::from_fn(11, 7, |p| Tile {
        elevation: 100,
        temperature: 128,
        moisture: 128,
        terrain: match p {
            _ if p.x == 5 => Terrain::River,
            _ if p == at(2, 2) => Terrain::Lake,
            _ if p == at(8, 0) => Terrain::Water,
            _ => Terrain::Land,
        },
        biome: plains,
        resource: None,
        building: None,
    })
}

/// A world on `field`, with `ice_construction` known when `known` is set.
fn world(nation: Nation, known: bool, defs: &Defs) -> World {
    let mut w = World::with_map(1, field(defs), nation, &[at(3, 3)], defs);
    if known {
        w.research.grant(defs.tech_id("ice_construction").unwrap());
        w.refresh_modifiers(defs);
    }
    w
}

fn cut_ice(defs: &Defs) -> AbilityId {
    defs.ability_id("cut_ice").unwrap()
}

fn ice_item(defs: &Defs) -> ItemId {
    defs.item_id("ice").unwrap()
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

/// Casts Cut Ice on `pos`, with full chi, and runs until it finishes, which it should do
/// successfully.
fn cut(w: &mut World, defs: &Defs, pos: Pos) {
    for u in &mut w.units {
        u.chi = defs.chi.max;
    }
    w.apply(Command::Bend { ability: cut_ice(defs), pos }, defs).unwrap();
    let events = w.bend_events().len();
    run_until(w, defs, 3000, |w| w.bend_events().len() > events);
    assert_eq!(w.bend_events().last().unwrap().outcome, BendOutcome::Done);
}

#[test]
fn cut_ice_is_locked_until_ice_construction_and_belongs_to_water() {
    let defs = Defs::bundled();
    let (ability, tech) = (cut_ice(&defs), defs.tech_id("ice_construction").unwrap());
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
fn targets_are_fresh_water_or_sea_beside_passable_ground() {
    let defs = Defs::bundled();
    let mut w = world(Nation::Water, true, &defs);
    // No dry ground on any of the lake's four orthogonal sides (diagonal doesn't count).
    for p in [at(2, 1), at(3, 2), at(2, 3), at(1, 2)] {
        w.map[p].terrain = Terrain::Lake;
    }
    w.map_changed();
    let ability = cut_ice(&defs);
    let bend = |w: &mut World, pos| w.apply(Command::Bend { ability, pos }, &defs);
    let refused = |why| Err(CommandError::Bend(BendError::InvalidTarget(why)));

    assert_eq!(bend(&mut w, at(5, 3)), Ok(()), "river beside dry land");
    assert_eq!(bend(&mut w, at(8, 0)), Ok(()), "sea beside dry land");
    assert_eq!(bend(&mut w, at(3, 3)), refused("needs river, lake or sea"));
    assert_eq!(bend(&mut w, at(2, 2)), refused("needs dry ground beside it"));
    assert_eq!(bend(&mut w, at(-1, 0)), refused("off the map"));
}

#[test]
fn a_bend_drops_four_ice_on_the_tile_the_bender_actually_stood_on() {
    let defs = Defs::bundled();
    let mut w = world(Nation::Water, true, &defs);
    // The unit starts right beside the river, so it stands at (4, 3) to cut (5, 3).
    w.units[0].pos = at(4, 3);
    let ice = ice_item(&defs);
    cut(&mut w, &defs, at(5, 3));
    assert_eq!(w.map[at(5, 3)].terrain, Terrain::River, "the water itself is untouched");
    assert_eq!(w.units[0].pos, at(4, 3), "stood right where it already was");
    assert_eq!(w.ground.get(&at(4, 3)), Some(&Stack { item: ice, count: 4 }));
    assert!((w.units[0].chi - 70.0).abs() < 0.1, "{}", w.units[0].chi);
}

#[test]
fn an_igloo_builds_from_the_ice_cut_ice_drops() {
    let defs = Defs::bundled();
    let mut w = world(Nation::Water, true, &defs);
    let igloo = defs.building_id("igloo").unwrap();
    let cost = defs.building(igloo).cost[0].count;
    assert_eq!((defs.building(igloo).cost[0].item, cost), (ice_item(&defs), 4));

    w.units[0].pos = at(4, 3);
    cut(&mut w, &defs, at(5, 3));
    let stockpile = at(1, 1);
    w.apply(Command::DesignateStockpile { pos: stockpile }, &defs).unwrap();
    run_until(&mut w, &defs, 2000, |w| w.stored_total(ice_item(&defs)) == cost);

    let anchor = at(0, 4);
    w.apply(Command::DesignateBuild { pos: anchor, building: igloo }, &defs).unwrap();
    run_until(&mut w, &defs, 5000, |w| w.building_at(anchor) == Some(igloo));
    assert_eq!(w.stored_total(ice_item(&defs)), 0, "the igloo's ice is spent, not wood");
}
