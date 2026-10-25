//! Glide and lost ruins (#197): ruins stay hidden, the tech that unlocks the glide, what it spots
//! (ruins only, not seams), and searching a ruin for scrolls.

use super::*;
use crate::geom::Grid;
use crate::mapgen::Tile;
use crate::world::work::DesignateError;

fn at(x: i32, y: i32) -> Pos {
    Pos::new(x, y)
}

/// A 40x7 plains field, open ground but for a mountain in columns 30 to 39, with lost ruins
/// (buried) at (12, 3), within the glide's reach of (1, 3), and at (20, 3), out of it, and a gold
/// seam at (14, 3) in a lone rock.
fn field(defs: &Defs) -> TileMap {
    let plains = BiomeId(defs.biomes.iter().position(|b| b.id == "plains").unwrap() as u8);
    let (ruins, seam) = (defs.resource_id("lost_ruins"), defs.resource_id("gold_seam"));
    Grid::from_fn(40, 7, |p| Tile {
        elevation: 200,
        temperature: 128,
        moisture: 128,
        terrain: if p == at(14, 3) { Terrain::Mountain } else { Terrain::Land },
        biome: plains,
        resource: if p == at(12, 3) || p == at(20, 3) {
            ruins
        } else if p == at(14, 3) {
            seam
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
    let events = w.bend_events().len();
    for _ in 0..3000 {
        if w.bend_events().len() > events {
            break;
        }
        w.units[0].needs = crate::needs::Needs::full(&defs.needs);
        w.units[0].chi = defs.chi.max;
        w.tick(defs);
    }
    assert_eq!(w.bend_events().last().map(|e| e.outcome), Some(BendOutcome::Done));
}

#[test]
fn ruins_are_buried_until_spotted() {
    let defs = Defs::bundled();
    let ruins = defs.resource_id("lost_ruins").unwrap();
    assert!(defs.resource(ruins).buried);
    let mut w = world(Nation::Air, &[], &defs);
    assert_eq!(w.buried_count(), 3, "two ruins and a seam");
    assert_eq!(w.map[at(12, 3)].resource, None, "nothing shows it");
    assert_eq!(
        w.apply(Command::DesignateHarvest { pos: at(12, 3) }, &defs),
        Err(CommandError::Designate(DesignateError::NothingToHarvest))
    );
}

#[test]
fn glide_is_locked_until_gliders_and_belongs_to_air() {
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
fn takeoff_is_from_open_ground() {
    let defs = Defs::bundled();
    let mut w = world(Nation::Air, &["gliders"], &defs);
    let ability = glide(&defs);
    assert_eq!(
        w.apply(Command::Bend { ability, pos: at(14, 3) }, &defs),
        Err(CommandError::Bend(BendError::InvalidTarget("needs open ground to stand on")))
    );
    assert_eq!(w.apply(Command::Bend { ability, pos: at(2, 3) }, &defs), Ok(()));
}

#[test]
fn the_glide_spots_ruins_in_reach_but_not_seams_and_they_hold_scrolls() {
    let defs = Defs::bundled();
    let ruins = defs.resource_id("lost_ruins").unwrap();
    let h = defs.harvest(ruins).unwrap();
    assert_eq!(h.item, defs.item_id("ancient_scroll").unwrap());
    let mut w = world(Nation::Air, &["gliders"], &defs);
    cast(&mut w, &defs, glide(&defs), at(1, 3));
    assert_eq!(w.map[at(12, 3)].resource, Some(ruins), "in reach: spotted");
    assert_eq!(w.map[at(20, 3)].resource, None, "out of reach: still hidden");
    assert_eq!(w.map[at(14, 3)].resource, None, "the rock keeps its seam");
    assert_eq!(w.buried_count(), 2);

    w.apply(Command::DesignateHarvest { pos: at(12, 3) }, &defs).unwrap();
    for _ in 0..6000 {
        if w.map[at(12, 3)].resource.is_none() {
            break;
        }
        w.units[0].needs = crate::needs::Needs::full(&defs.needs);
        w.tick(&defs);
    }
    assert_eq!(w.map[at(12, 3)].resource, None, "searched, and gone");
    let found: u32 = w.ground_items().filter(|(_, s)| s.item == h.item).map(|(_, s)| s.count).sum();
    assert_eq!(found, h.count);
}

#[test]
fn seismic_sense_feels_seams_but_not_ruins() {
    let defs = Defs::bundled();
    let mut w = world(Nation::Earth, &["seismic_sense"], &defs);
    cast(&mut w, &defs, defs.ability_id("seismic_sense").unwrap(), at(13, 3));
    assert_eq!(w.map[at(14, 3)].resource, defs.resource_id("gold_seam"));
    assert_eq!(w.map[at(12, 3)].resource, None, "ruins are seen from the air, not felt");
}

#[test]
fn a_ruin_under_someone_stays_hidden_until_the_next_glide() {
    let defs = Defs::bundled();
    let ruins = defs.resource_id("lost_ruins").unwrap();
    let mut w = world(Nation::Air, &["gliders"], &defs);
    w.units[0].pos = at(12, 3);
    w.apply(Command::Bend { ability: glide(&defs), pos: at(12, 3) }, &defs).unwrap();
    let events = w.bend_events().len();
    for _ in 0..3000 {
        if w.bend_events().len() > events {
            break;
        }
        w.units[0].chi = defs.chi.max;
        w.tick(&defs);
    }
    assert_eq!(w.map[at(12, 3)].resource, None, "the glider stood on it");
    cast(&mut w, &defs, glide(&defs), at(2, 3));
    assert_eq!(w.map[at(12, 3)].resource, Some(ruins));
}
