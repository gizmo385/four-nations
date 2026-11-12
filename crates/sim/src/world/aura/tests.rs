use super::*;
use crate::defs::{BiomeId, Terrain};
use crate::geom::Grid;
use crate::mapgen::Tile;
use crate::world::Command;

fn field(width: i32, height: i32) -> TileMap {
    Grid::from_fn(width, height, |_| Tile {
        elevation: 100,
        temperature: 128,
        moisture: 128,
        terrain: Terrain::Land,
        biome: BiomeId(0),
        resource: None,
        building: None,
    })
}

fn at(x: i32, y: i32) -> Pos {
    Pos::new(x, y)
}

fn temple(defs: &Defs) -> BuildingId {
    defs.building_id("high_temple").unwrap()
}

/// The bundled data with the High Temple's aura replaced by `aura`.
fn with_temple_aura(aura: AuraDef) -> Defs {
    let mut defs = Defs::bundled();
    let id = temple(&defs);
    defs.buildings[id.0 as usize].aura = Some(aura);
    defs
}

fn put_temple(w: &mut World, defs: &Defs, anchor: Pos) {
    w.put_building(Footprint::new(anchor, 3, 3), temple(defs), defs);
}

#[test]
fn auras_are_validated() {
    let with = |aura: &str| {
        let old = "[building.aura]\nradius = 8\nchi_regen = 2.0\nstudy = 1.5\n";
        let mut files = crate::defs::BUNDLED_FILES;
        assert!(files[7].contains(old), "the temple's aura moved");
        let buildings = files[7].replacen(old, &format!("[building.aura]\n{aura}\n"), 1);
        files[7] = &buildings;
        Defs::from_files(files).map(|_| ())
    };
    for (aura, why) in [
        ("radius = 0\nstudy = 1.5", "radius must be above 0"),
        ("radius = 8\nstudy = 4.5", "must be 1.0 to 4"),
        ("radius = 8\nheal = 0.5", "must be 1.0 to 4"),
        ("radius = 8\nheal = 1.0", "needs a multiplier above 1.0"),
        ("radius = 8", "needs a multiplier above 1.0"),
        ("radius = 8\nstudy = 1.5\nwarmth = 2.0", "warmth"),
    ] {
        let e = with(aura).expect_err(aura).0;
        assert!(e.contains(why), "{aura}: {e}");
    }
    let defs = Defs::bundled();
    let def = defs.building(temple(&defs));
    assert_eq!(def.aura, Some(AuraDef { radius: 8, heal: 1.0, chi_regen: 2.0, study: 1.5 }));
    assert!(def.landmark);
    assert_eq!(def.footprint, (3, 3));
    assert_eq!(def.beside_word(), Some("rock"));
}

#[test]
fn the_best_value_per_stat_applies_and_auras_do_not_stack() {
    let defs = Defs::bundled();
    let mut w = World::with_map(1, field(40, 20), Nation::Air, &[at(0, 0)], &defs);
    assert_eq!(w.aura_at(at(10, 10), &defs), AuraEffect::default());
    put_temple(&mut w, &defs, at(10, 5));
    let near = w.aura_at(at(20, 6), &defs);
    assert_eq!(near, AuraEffect { heal: 1.0, chi_regen: 2.0, study_millis: 1500 });
    // 8 tiles from the footprint's edge (12, 7) reaches; 9 doesn't.
    assert_eq!(w.aura_at(at(20, 7), &defs).chi_regen, 2.0);
    assert_eq!(w.aura_at(at(21, 7), &defs), AuraEffect::default());

    // A second aura with a stronger chi and a heal: each stat takes its best, nothing adds.
    let mut defs = defs;
    let mut second = defs.building(temple(&defs)).clone();
    second.id = "test_shrine".into();
    second.landmark = false;
    second.aura = Some(AuraDef { radius: 8, heal: 2.0, chi_regen: 3.0, study: 1.0 });
    defs.buildings.push(second);
    let shrine = BuildingId((defs.buildings.len() - 1) as u16);
    w.put_building(Footprint::new(at(14, 12), 3, 3), shrine, &defs);
    let both = w.aura_at(at(13, 9), &defs);
    assert_eq!(both, AuraEffect { heal: 2.0, chi_regen: 3.0, study_millis: 1500 });
}

#[test]
fn chi_and_health_come_back_faster_in_reach() {
    let defs = with_temple_aura(AuraDef { radius: 4, heal: 2.0, chi_regen: 2.0, study: 1.0 });
    let after = |near: bool| {
        let mut w = World::with_map(1, field(40, 20), Nation::Air, &[at(5, 5)], &defs);
        if near {
            put_temple(&mut w, &defs, at(7, 7));
        }
        w.units[0].chi = 0.0;
        w.units[0].needs.health = 10.0;
        for _ in 0..600 {
            w.tick_needs(&defs);
        }
        (w.units[0].chi, w.units[0].needs.health - 10.0)
    };
    let (far_chi, far_heal) = after(false);
    let (near_chi, near_heal) = after(true);
    assert!(far_chi > 0.0 && far_heal > 0.0, "both regenerate on their own");
    assert!((near_chi - 2.0 * far_chi).abs() < 1e-3, "{near_chi} vs {far_chi}");
    assert!((near_heal - 2.0 * far_heal).abs() < 1e-3, "{near_heal} vs {far_heal}");
}

#[test]
fn a_landmark_is_one_per_settlement_and_never_comes_down() {
    let defs = Defs::bundled();
    let id = temple(&defs);
    let mut map = field(40, 20);
    for y in 0..20 {
        map[at(0, y)].terrain = Terrain::Mountain;
    }
    let mut w = World::with_map(1, map, Nation::Air, &[at(20, 10)], &defs);
    w.research.grant(defs.building(id).unlocked_by.unwrap());
    // Away from the rock: refused.
    let e = w.apply(Command::DesignateBuild { pos: at(10, 5), building: id }, &defs).unwrap_err();
    assert_eq!(e.to_string(), "must touch rock");
    w.apply(Command::DesignateBuild { pos: at(1, 2), building: id }, &defs).unwrap();
    // A second, even as a site only: refused.
    let e = w.apply(Command::DesignateBuild { pos: at(1, 10), building: id }, &defs).unwrap_err();
    assert_eq!(e.to_string(), "only one");
    // Finished, it still counts, and it can't be taken down.
    w.apply(Command::CancelBuild { pos: at(1, 2) }, &defs).unwrap();
    put_temple(&mut w, &defs, at(1, 2));
    let e = w.apply(Command::DesignateBuild { pos: at(1, 10), building: id }, &defs).unwrap_err();
    assert_eq!(e.to_string(), "only one");
    let e = w.apply(Command::Deconstruct { pos: at(2, 3) }, &defs).unwrap_err();
    assert_eq!(e.to_string(), "A landmark stays");
}
