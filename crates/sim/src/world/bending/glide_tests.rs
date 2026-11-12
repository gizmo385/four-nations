//! Glide and lost ruins (#197): a second buried resource, found only by Air's Glide, each sense
//! finding only its own kind, and a buried resource staying hidden if the tile isn't free to take
//! it (#133's gold seams get the same rule).

use super::*;
use crate::geom::Grid;
use crate::mapgen::Tile;
use crate::world::work::DesignateError;

fn at(x: i32, y: i32) -> Pos {
    Pos::new(x, y)
}

/// A 30x7 plains field, all open land, with a lost ruin (buried) at (6, 3), within the glide's
/// reach of (3, 3), another far off at (25, 3), and a visible oak tree at (5, 3) blocking that
/// tile.
fn field(defs: &Defs) -> TileMap {
    let plains = BiomeId(defs.biomes.iter().position(|b| b.id == "plains").unwrap() as u8);
    let ruins = defs.resource_id("lost_ruins");
    let tree = defs.resource_id("oak_tree");
    Grid::from_fn(30, 7, |p| Tile {
        elevation: 200,
        temperature: 128,
        moisture: 128,
        terrain: Terrain::Land,
        biome: plains,
        resource: if p == at(6, 3) || p == at(25, 3) {
            ruins
        } else if p == at(5, 3) {
            tree
        } else {
            None
        },
        building: None,
    })
}

/// A world on `field`, with `techs` known.
fn world(nation: Nation, techs: &[&str], defs: &Defs) -> World {
    let mut w = World::with_map(1, field(defs), nation, &[at(1, 3)], defs);
    for t in techs {
        w.research.grant(defs.tech_id(t).unwrap());
    }
    w.refresh_modifiers(defs);
    w
}

fn glide(defs: &Defs) -> AbilityId {
    defs.ability_id("glide").unwrap()
}

/// Casts `ability` on `pos` with unit 0 and runs until it is done, which it should have been.
fn cast(w: &mut World, defs: &Defs, ability: AbilityId, pos: Pos) {
    w.apply(Command::Bend { ability, pos }, defs).unwrap();
    let events = w.bend_events().count();
    for _ in 0..3000 {
        if w.bend_events().count() > events {
            break;
        }
        w.units[0].needs = crate::needs::Needs::full(&defs.needs);
        w.units[0].chi = defs.chi.max;
        w.tick(defs);
    }
    assert_eq!(w.bend_events().next_back().map(|e| e.outcome), Some(BendOutcome::Done));
}

#[test]
fn a_ruin_is_buried_until_found() {
    let defs = Defs::bundled();
    let ruins = defs.resource_id("lost_ruins").unwrap();
    assert!(defs.resource(ruins).buried);
    let mut w = world(Nation::Air, &[], &defs);
    assert_eq!(w.buried_count(), 2);
    assert_eq!(w.map[at(6, 3)].resource, None, "nothing shows it");
    assert_eq!(
        w.apply(Command::DesignateHarvest { pos: at(6, 3) }, &defs),
        Err(CommandError::Designate(DesignateError::NothingToHarvest))
    );
}

#[test]
fn glide_is_locked_until_the_tech_and_belongs_to_air() {
    let defs = Defs::bundled();
    let (ability, tech) = (glide(&defs), defs.tech_id("gliders").unwrap());
    assert_eq!(defs.ability(ability).unlocked_by, Some(tech));
    let w = world(Nation::Air, &[], &defs);
    assert_eq!(w.check_ability(ability, &defs), Err(BendError::Locked { tech }));
    let w = world(Nation::Air, &["gliders"], &defs);
    assert_eq!(w.check_ability(ability, &defs), Ok(()));
    for nation in [Nation::Water, Nation::Fire, Nation::Earth] {
        let w = world(nation, &[], &defs);
        assert_eq!(w.check_ability(ability, &defs), Err(BendError::WrongNation), "{nation:?}");
    }
}

#[test]
fn targets_are_open_ground_to_stand_on() {
    let defs = Defs::bundled();
    let mut w = world(Nation::Air, &["gliders"], &defs);
    let ability = glide(&defs);
    let refused = |why| Err(CommandError::Bend(BendError::InvalidTarget(why)));
    assert_eq!(
        w.apply(Command::Bend { ability, pos: at(5, 3) }, &defs),
        refused("something is in the way")
    );
    assert_eq!(w.apply(Command::Bend { ability, pos: at(3, 3) }, &defs), Ok(()));
}

#[test]
fn the_glide_uncovers_what_is_in_reach_and_it_can_be_searched() {
    let defs = Defs::bundled();
    let ruins = defs.resource_id("lost_ruins").unwrap();
    let h = defs.harvest(ruins).unwrap();
    let mut w = world(Nation::Air, &["gliders"], &defs);
    cast(&mut w, &defs, glide(&defs), at(3, 3));
    assert_eq!(w.map[at(6, 3)].resource, Some(ruins), "in reach: uncovered");
    assert_eq!(w.map[at(25, 3)].resource, None, "out of reach (radius 14): still buried");
    assert_eq!(w.buried_count(), 1);

    w.apply(Command::DesignateHarvest { pos: at(6, 3) }, &defs).unwrap();
    for _ in 0..6000 {
        if w.map[at(6, 3)].resource.is_none() {
            break;
        }
        w.units[0].needs = crate::needs::Needs::full(&defs.needs);
        w.tick(&defs);
    }
    let found: u32 = w.ground_items().filter(|(_, s)| s.item == h.item).map(|(_, s)| s.count).sum();
    assert_eq!(found, h.count);
    assert_eq!(h.count, 2, "a ruin is worth more than one lone scroll");
}

/// Seismic Sense feels only gold seams and Glide spots only ruins, even standing right on top of
/// the other nation's buried find (#197's `Effect::Reveal::finds`).
#[test]
fn each_sense_finds_only_its_own_kind() {
    let defs = Defs::bundled();
    let plains = BiomeId(defs.biomes.iter().position(|b| b.id == "plains").unwrap() as u8);
    let ruins = defs.resource_id("lost_ruins").unwrap();
    let seam = defs.resource_id("gold_seam").unwrap();
    let map = Grid::from_fn(10, 5, |p| Tile {
        elevation: 200,
        temperature: 128,
        moisture: 128,
        terrain: if p == at(5, 2) { Terrain::Mountain } else { Terrain::Land },
        biome: plains,
        resource: if p == at(3, 2) {
            Some(ruins)
        } else if p == at(5, 2) {
            Some(seam)
        } else {
            None
        },
        building: None,
    });
    let mut w = World::with_map(1, map, Nation::Air, &[at(1, 2)], &defs);
    w.research.grant(defs.tech_id("gliders").unwrap());
    w.refresh_modifiers(&defs);
    assert_eq!(w.buried_count(), 2);

    cast(&mut w, &defs, glide(&defs), at(1, 2));
    assert_eq!(w.map[at(3, 2)].resource, Some(ruins), "glide finds the ruin");
    assert_eq!(w.map[at(5, 2)].resource, None, "glide leaves the seam buried");
    assert_eq!(w.buried_count(), 1);
}

#[test]
fn a_buried_ruin_under_a_stockpile_zone_stays_hidden_until_the_zone_is_cleared() {
    let defs = Defs::bundled();
    let ruins = defs.resource_id("lost_ruins").unwrap();
    let mut w = world(Nation::Air, &["gliders"], &defs);
    w.apply(Command::DesignateStockpile { pos: at(6, 3) }, &defs).unwrap();
    cast(&mut w, &defs, glide(&defs), at(3, 3));
    assert_eq!(w.map[at(6, 3)].resource, None, "the zone is using the tile");
    assert_eq!(w.buried_count(), 2, "still buried, to be found later");

    w.apply(Command::CancelStockpile { pos: at(6, 3) }, &defs).unwrap();
    cast(&mut w, &defs, glide(&defs), at(3, 3));
    assert_eq!(w.map[at(6, 3)].resource, Some(ruins), "the tile is free now");
    assert_eq!(w.buried_count(), 1);
}

#[test]
fn buried_ruins_survive_a_save() {
    let defs = Defs::bundled();
    let mut w = world(Nation::Air, &["gliders"], &defs);
    let text = w.save(&defs).unwrap();
    let mut loaded = World::load(&text, &defs).unwrap();
    assert_eq!(loaded.buried_count(), 2);
    cast(&mut w, &defs, glide(&defs), at(3, 3));
    cast(&mut loaded, &defs, glide(&defs), at(3, 3));
    assert_eq!(loaded.map[at(6, 3)].resource, w.map[at(6, 3)].resource);
}

/// Great ruins (#736): Glide spots them as it does lost ruins, and each find is logged; searching
/// one drops its two scrolls and, as its own stack, a teaching scroll (`also`).
#[test]
fn glide_spots_great_ruins_and_a_search_drops_a_teaching_scroll() {
    let defs = Defs::bundled();
    let plains = BiomeId(defs.biomes.iter().position(|b| b.id == "plains").unwrap() as u8);
    let ruins = defs.resource_id("lost_ruins").unwrap();
    let great = defs.resource_id("great_ruins").unwrap();
    let map = Grid::from_fn(12, 5, |p| Tile {
        elevation: 200,
        temperature: 128,
        moisture: 128,
        terrain: Terrain::Land,
        biome: plains,
        resource: if p == at(4, 2) {
            Some(great)
        } else if p == at(6, 2) {
            Some(ruins)
        } else {
            None
        },
        building: None,
    });
    let mut w = World::with_map(1, map, Nation::Air, &[at(1, 2)], &defs);
    w.research.grant(defs.tech_id("gliders").unwrap());
    w.refresh_modifiers(&defs);
    assert_eq!(w.buried_count(), 2);

    cast(&mut w, &defs, glide(&defs), at(1, 2));
    assert_eq!(w.map[at(4, 2)].resource, Some(great), "glide finds the great ruin");
    assert_eq!(w.map[at(6, 2)].resource, Some(ruins));
    let finds: Vec<(Pos, ResourceId)> = w.find_events().map(|e| (e.pos, e.resource)).collect();
    assert_eq!(finds, vec![(at(4, 2), great), (at(6, 2), ruins)]);
    assert!(w.find_events().all(|e| e.ability == glide(&defs)));

    w.apply(Command::DesignateHarvest { pos: at(4, 2) }, &defs).unwrap();
    for _ in 0..20_000 {
        if w.map[at(4, 2)].resource.is_none() {
            break;
        }
        w.units[0].needs = crate::needs::Needs::full(&defs.needs);
        w.tick(&defs);
    }
    assert_eq!(w.map[at(4, 2)].resource, None, "searched");
    let count = |item: &str| -> u32 {
        let id = defs.item_id(item).unwrap();
        let ground: u32 =
            w.ground_items().filter(|(_, s)| s.item == id).map(|(_, s)| s.count).sum();
        let carried: u32 = w
            .units
            .iter()
            .filter_map(|u| u.carrying)
            .filter(|s| s.item == id)
            .map(|s| s.count)
            .sum();
        ground + carried
    };
    assert_eq!(count("ancient_scroll"), 2, "the lost ruin's two scrolls");
    assert_eq!(count("teaching_scroll"), 1, "and a teaching scroll");
}
