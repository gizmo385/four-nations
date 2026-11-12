//! #740's three teachings: `sandbending` (sand sailers and the Sandstone Works),
//! `plantbending` (Regrow) and `spirit_projection` (Project).

use super::*;
use crate::bending::Effect;
use crate::defs::{BiomeId, Terrain};
use crate::geom::Grid;
use crate::mapgen::Tile;
use crate::path::{MoveRules, path_cost};

fn at(x: i32, y: i32) -> Pos {
    Pos::new(x, y)
}

fn biome(defs: &Defs, id: &str) -> BiomeId {
    defs.biome_id(id).unwrap()
}

/// A 12x6 field of plains, with columns `x >= 6` desert.
fn field(defs: &Defs) -> TileMap {
    let (plains, desert) = (biome(defs, "plains"), biome(defs, "desert"));
    Grid::from_fn(12, 6, |p| Tile {
        elevation: 100,
        temperature: 128,
        moisture: 128,
        terrain: Terrain::Land,
        biome: if p.x >= 6 { desert } else { plains },
        resource: None,
        building: None,
    })
}

fn world(nation: Nation, techs: &[&str], defs: &Defs) -> World {
    let mut w = World::with_map(1, field(defs), nation, &[at(1, 2)], defs);
    for t in techs {
        w.research.grant(defs.tech_id(t).unwrap());
    }
    w.refresh_modifiers(defs);
    w
}

fn with_research(extra: &str) -> Result<Defs, crate::defs::DefsError> {
    let mut files = crate::defs::BUNDLED_FILES;
    let research = format!("{}\n{extra}\n", files[3]);
    files[3] = &research;
    Defs::from_files(files)
}

#[test]
fn biome_speed_is_validated() {
    let tech = |speed: &str| {
        format!(
            "[[tech]]\nid = \"t\"\nname = \"T\"\ndescription = \"\"\ncost = 10\n\
             [tech.unlocks]\nbiome_speed = {{ {speed} }}"
        )
    };
    for (speed, why) in [
        ("lava = 2.0", "unknown biome \"lava\""),
        ("desert = 0.0", "must be in (0, 4]"),
        ("desert = 4.5", "must be in (0, 4]"),
    ] {
        let e = with_research(&tech(speed)).expect_err(speed).0;
        assert!(e.contains(why), "{speed}: {e}");
    }
    assert!(with_research(&tech("swamp = 0.5")).is_ok(), "a slowdown is fine");
}

#[test]
fn the_three_teachings_are_data() {
    let defs = Defs::bundled();
    for (id, nation) in [
        ("sandbending", Nation::Earth),
        ("plantbending", Nation::Water),
        ("spirit_projection", Nation::Air),
    ] {
        let t = defs.tech(defs.tech_id(id).unwrap());
        assert!(t.teaching.is_some(), "{id}");
        assert_eq!(t.nation, Some(nation), "{id}");
        assert_eq!(t.cost, 250, "{id}");
    }
    let sand = defs.tech(defs.tech_id("sandbending").unwrap());
    assert_eq!(sand.unlocks.biome_speed.get("desert"), Some(&2.0));
    let site = sand.teaching.as_ref().unwrap().site.as_ref().expect("a site rule");
    assert_eq!((site.within, site.min_tiles), (40, 40));
    let works = defs.building(defs.building_id("sandstone_works").unwrap());
    assert_eq!(works.on_biomes, ["desert"]);
    let recipe = works.recipe.as_ref().expect("a source recipe");
    assert!(recipe.inputs.is_empty(), "stone by labour, from nothing but sand");
    assert_eq!(recipe.output.item, defs.item_id("stone").unwrap());
    assert_eq!(recipe.modifier.as_deref(), Some("earthworks"));
}

/// Sand sailers: a step onto desert costs half once `sandbending` is known, and routes plan
/// with it (#740).
#[test]
fn sand_sailers_cross_desert_twice_as_fast() {
    let defs = Defs::bundled();
    let line: Vec<Pos> = (6..12).map(|x| at(x, 2)).collect();
    let mut w = world(Nation::Earth, &[], &defs);
    let rules = w.rules_for(&w.units[0]).clone();
    let before = path_cost(&w.map, &line, &rules);
    let onto_plains = path_cost(&w.map, &[at(1, 2), at(2, 2)], &rules);
    w.research.grant(defs.tech_id("sandbending").unwrap());
    w.refresh_modifiers(&defs);
    let rules = w.rules_for(&w.units[0]).clone();
    assert_eq!(path_cost(&w.map, &line, &rules) * 2, before, "half the cost on desert");
    assert_eq!(path_cost(&w.map, &[at(1, 2), at(2, 2)], &rules), onto_plains, "plains unchanged");
    // A save loads with it, though the speed itself isn't saved.
    let saved = w.save(&defs).unwrap();
    let back = World::load(&saved, &defs).unwrap();
    assert_eq!(back.rules_for(&back.units[0]), &rules);
    // Plain rules with every biome at 1000 are plain walking.
    assert_eq!(MoveRules::default().with_biome_speeds(vec![1000; 6]), MoveRules::default());
}

#[test]
fn the_sandstone_works_must_stand_on_desert() {
    let defs = Defs::bundled();
    let works = defs.building_id("sandstone_works").unwrap();
    let w = world(Nation::Earth, &["sandbending"], &defs);
    let desert = biome(&defs, "desert");
    assert_eq!(
        w.check_build(at(5, 2), works, &defs),
        Err(BuildError::CannotBuild { at: at(5, 2), why: Blocker::OnBiome(desert) }),
        "half on plains"
    );
    assert_eq!(w.check_build(at(8, 2), works, &defs), Ok(()));
}

fn regrow(defs: &Defs) -> AbilityId {
    defs.ability_id("regrow").unwrap()
}

/// Casts `ability` on `pos` with unit 0 and runs until it is done.
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
fn regrow_clears_the_strain_in_a_three_by_three_patch() {
    let defs = Defs::bundled();
    assert_eq!(defs.ability(regrow(&defs)).effect, Effect::Unstrain { radius: 1 });
    let mut w = world(Nation::Water, &["plantbending"], &defs);
    assert_eq!(
        w.apply(Command::Bend { ability: regrow(&defs), pos: at(3, 3) }, &defs),
        Err(CommandError::Bend(BendError::InvalidTarget("nothing here is tired")))
    );
    for p in [at(2, 2), at(4, 4), at(5, 3), at(3, 1)] {
        w.strain.insert(p, 50.0);
    }
    cast(&mut w, &defs, regrow(&defs), at(3, 3));
    assert_eq!(w.strain_at(at(2, 2)), None);
    assert_eq!(w.strain_at(at(4, 4)), None);
    assert!(w.strain_at(at(5, 3)).is_some(), "two tiles off: outside the patch");
    assert!(w.strain_at(at(3, 1)).is_some(), "two tiles off: outside the patch");
}

/// Project finds every buried kind, so one added later can't be forgotten here.
#[test]
fn project_reveals_every_buried_resource() {
    let defs = Defs::bundled();
    let project = defs.ability(defs.ability_id("project").unwrap());
    let Effect::Reveal { radius, finds } = &project.effect else { panic!("a reveal") };
    assert_eq!(*radius, 20);
    let buried: Vec<ResourceId> = (0..defs.resources.len())
        .map(|i| ResourceId(i as u16))
        .filter(|&r| defs.resource(r).buried)
        .collect();
    let mut found = finds.clone();
    found.sort_by_key(|r| r.0);
    assert_eq!(found, buried);
}
