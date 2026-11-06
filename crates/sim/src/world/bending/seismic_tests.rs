//! Seismic Sense and buried gold seams (#133): what is buried stays hidden, the tech that unlocks
//! the sense, what it uncovers, and a Bore Tunnel striking a seam.

use super::*;
use crate::geom::Grid;
use crate::mapgen::Tile;
use crate::world::work::DesignateError;

fn at(x: i32, y: i32) -> Pos {
    Pos::new(x, y)
}

/// A 30x7 plains field: open ground in columns 0 to 3 and a mountain from column 4 east, with a
/// gold seam (buried) at (6, 3), within the sense's reach of (3, 3), and another far off at
/// (25, 3).
fn field(defs: &Defs) -> TileMap {
    let plains = BiomeId(defs.biomes.iter().position(|b| b.id == "plains").unwrap() as u8);
    let seam = defs.resource_id("gold_seam");
    Grid::from_fn(30, 7, |p| Tile {
        elevation: 200,
        temperature: 128,
        moisture: 128,
        terrain: if p.x >= 4 { Terrain::Mountain } else { Terrain::Land },
        biome: plains,
        resource: if p == at(6, 3) || p == at(25, 3) { seam } else { None },
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

fn sense(defs: &Defs) -> AbilityId {
    defs.ability_id("seismic_sense").unwrap()
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
fn a_seam_is_buried_until_found() {
    let defs = Defs::bundled();
    let seam = defs.resource_id("gold_seam").unwrap();
    assert!(defs.resource(seam).buried);
    let mut w = world(Nation::Earth, &[], &defs);
    assert_eq!(w.buried_count(), 2);
    assert_eq!(w.map[at(6, 3)].resource, None, "nothing shows it");
    assert_eq!(
        w.apply(Command::DesignateHarvest { pos: at(6, 3) }, &defs),
        Err(CommandError::Designate(DesignateError::NothingToHarvest))
    );
}

#[test]
fn seismic_sense_is_locked_until_the_tech_and_belongs_to_earth() {
    let defs = Defs::bundled();
    let (ability, tech) = (sense(&defs), defs.tech_id("seismic_sense").unwrap());
    assert_eq!(defs.ability(ability).unlocked_by, Some(tech));
    let w = world(Nation::Earth, &[], &defs);
    assert_eq!(w.check_ability(ability, &defs), Err(BendError::Locked { tech }));
    let w = world(Nation::Earth, &["seismic_sense"], &defs);
    assert_eq!(w.check_ability(ability, &defs), Ok(()));
    for nation in [Nation::Water, Nation::Fire, Nation::Air] {
        let w = world(nation, &[], &defs);
        assert_eq!(w.check_ability(ability, &defs), Err(BendError::WrongNation), "{nation:?}");
    }
}

#[test]
fn targets_are_open_ground_to_stand_on() {
    let defs = Defs::bundled();
    let mut w = world(Nation::Earth, &["seismic_sense"], &defs);
    let ability = sense(&defs);
    let refused = |why| Err(CommandError::Bend(BendError::InvalidTarget(why)));
    assert_eq!(
        w.apply(Command::Bend { ability, pos: at(5, 3) }, &defs),
        refused("needs open ground to stand on")
    );
    assert_eq!(w.apply(Command::Bend { ability, pos: at(3, 3) }, &defs), Ok(()));
}

#[test]
fn the_sense_uncovers_what_is_in_reach_and_it_can_be_mined() {
    let defs = Defs::bundled();
    let seam = defs.resource_id("gold_seam").unwrap();
    let h = defs.harvest(seam).unwrap();
    let mut w = world(Nation::Earth, &["seismic_sense", "prospecting"], &defs);
    cast(&mut w, &defs, sense(&defs), at(3, 3));
    assert_eq!(w.map[at(6, 3)].resource, Some(seam), "in reach: uncovered");
    assert_eq!(w.map[at(25, 3)].resource, None, "out of reach: still buried");
    assert_eq!(w.buried_count(), 1);

    // Bore to it and mine it: from inside the rock, for the tunnel yield.
    let bore = defs.ability_id("bore_tunnel").unwrap();
    w.research.grant(defs.tech_id("tunneling").unwrap());
    cast(&mut w, &defs, bore, at(4, 3));
    cast(&mut w, &defs, bore, at(5, 3));
    w.apply(Command::DesignateHarvest { pos: at(6, 3) }, &defs).unwrap();
    for _ in 0..6000 {
        if w.map[at(6, 3)].resource.is_none() {
            break;
        }
        w.units[0].needs = crate::needs::Needs::full(&defs.needs);
        w.tick(&defs);
    }
    let mined: u32 = w.ground_items().filter(|(_, s)| s.item == h.item).map(|(_, s)| s.count).sum();
    assert_eq!(mined, h.tunnel_count.unwrap());
    // #349 (docs/design/materials.md, section 1) brought the surface vein's yield up to match a
    // seam's: a seam's edge is in who can reach it (Seismic Sense, Earth only), not how rich it is.
    let vein = defs.harvest(defs.resource_id("gold_vein").unwrap()).unwrap();
    assert_eq!(h.count, vein.count);
    assert_eq!(h.tunnel_count, vein.tunnel_count);
}

#[test]
fn boring_into_a_buried_seam_strikes_it_instead() {
    let defs = Defs::bundled();
    let seam = defs.resource_id("gold_seam").unwrap();
    let mut w = world(Nation::Earth, &["tunneling"], &defs);
    let bore = defs.ability_id("bore_tunnel").unwrap();
    cast(&mut w, &defs, bore, at(4, 3));
    cast(&mut w, &defs, bore, at(5, 3));
    cast(&mut w, &defs, bore, at(6, 3));
    assert_eq!(w.map[at(6, 3)].resource, Some(seam), "struck");
    assert_eq!(w.map[at(6, 3)].terrain, Terrain::Mountain, "the rock stays");
}

#[test]
fn buried_seams_survive_a_save() {
    let defs = Defs::bundled();
    let mut w = world(Nation::Earth, &["seismic_sense"], &defs);
    let text = w.save(&defs).unwrap();
    let mut loaded = World::load(&text, &defs).unwrap();
    assert_eq!(loaded.buried_count(), 2);
    cast(&mut w, &defs, sense(&defs), at(3, 3));
    cast(&mut loaded, &defs, sense(&defs), at(3, 3));
    assert_eq!(loaded.map[at(6, 3)].resource, w.map[at(6, 3)].resource);
}
