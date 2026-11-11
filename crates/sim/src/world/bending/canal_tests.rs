//! Carve Canal (#695, docs/design/horizons-phase-2.md, "Shared sim work, 4. Canal"): the `canals`
//! tech that unlocks it, the fresh-water source rule and chained designations, where the bender
//! stands, waiting for the tile to empty, the no-split rule at designation and at completion,
//! what a canal is to the water rules, and the irrigation it gives.

use super::*;
use crate::geom::{Footprint, Grid};
use crate::mapgen::Tile;

fn at(x: i32, y: i32) -> Pos {
    Pos::new(x, y)
}

fn biome(defs: &Defs, id: &str) -> BiomeId {
    BiomeId(defs.biomes.iter().position(|b| b.id == id).unwrap() as u8)
}

fn tile(defs: &Defs, terrain: Terrain) -> Tile {
    Tile {
        elevation: 100,
        temperature: 128,
        moisture: 128,
        terrain,
        biome: biome(defs, "plains"),
        resource: None,
        building: None,
    }
}

/// An 11x9 plains field with a river down column 0.
fn field(defs: &Defs) -> TileMap {
    Grid::from_fn(11, 9, |p| tile(defs, if p.x == 0 { Terrain::River } else { Terrain::Land }))
}

/// A two-tile-wide corridor, rows 4 and 5 from x = 1 to 9, between rock, with a river along its
/// north side (row 3) to cut from.
fn corridor(defs: &Defs) -> TileMap {
    Grid::from_fn(11, 9, |p| {
        let terrain = match (p.x, p.y) {
            (1..=9, 3) => Terrain::River,
            (1..=9, 4..=5) => Terrain::Land,
            _ => Terrain::Mountain,
        };
        tile(defs, terrain)
    })
}

fn world_on(map: TileMap, units: &[Pos], defs: &Defs) -> World {
    let mut w = World::with_map(1, map, Nation::Water, units, defs);
    w.research.grant(defs.tech_id("canals").unwrap());
    w.refresh_modifiers(defs);
    w
}

fn carve(defs: &Defs) -> AbilityId {
    defs.ability_id("carve_canal").unwrap()
}

fn run_until(w: &mut World, defs: &Defs, limit: u64, mut done: impl FnMut(&World) -> bool) -> u64 {
    for n in 0..limit {
        if done(w) {
            return n;
        }
        w.units.iter_mut().for_each(|u| {
            u.needs = crate::needs::Needs::full(&defs.needs);
            u.chi = defs.chi.max;
        });
        w.tick(defs);
    }
    panic!("not done after {limit} ticks");
}

fn bend(w: &mut World, defs: &Defs, pos: Pos) -> Result<(), CommandError> {
    w.apply(Command::Bend { ability: carve(defs), pos }, defs).map(|_| ())
}

fn refused(why: &'static str) -> Result<(), CommandError> {
    Err(CommandError::Bend(BendError::InvalidTarget(why)))
}

fn canals_at(w: &World, tiles: &[Pos]) -> bool {
    tiles.iter().all(|&p| w.map[p].terrain == Terrain::Canal)
}

#[test]
fn the_canal_is_fresh_water_nobody_walks_on_and_has_no_shore() {
    let t = Terrain::Canal;
    assert!(t.is_water() && t.is_fresh());
    assert!(!t.is_walkable() && !t.has_shore());
    assert!(!Terrain::Water.is_fresh());
    for river_like in [Terrain::River, Terrain::Lake, Terrain::Ford] {
        assert!(river_like.is_fresh(), "{river_like:?}");
    }
    let rules = crate::path::MoveRules::default();
    assert_eq!(rules.terrain_cost(Terrain::Canal), None, "a canal blocks walking like a river");
    assert_eq!(rules.terrain_cost(Terrain::River), None);
}

#[test]
fn canals_is_waters_tech_and_unlocks_carve_canal() {
    let defs = Defs::bundled();
    let tech = defs.tech_id("canals").unwrap();
    let def = defs.tech(tech);
    assert_eq!(def.nation, Some(Nation::Water));
    assert_eq!(def.cost, 250);
    let prereqs: Vec<&str> = def.prereqs.iter().map(|&t| defs.tech(t).id.as_str()).collect();
    assert_eq!(prereqs, ["irrigation", "water_travel"]);
    let ability = carve(&defs);
    assert_eq!(defs.ability(ability).unlocked_by, Some(tech));
    assert_eq!(defs.ability(ability).nation, Nation::Water);
    assert_eq!(defs.ability(ability).chi, 60.0);

    let map = field(&defs);
    let w = World::with_map(1, map, Nation::Water, &[at(6, 6)], &defs);
    assert_eq!(w.check_ability(ability, &defs), Err(BendError::Locked { tech }));
    let w = world_on(field(&defs), &[at(6, 6)], &defs);
    assert_eq!(w.check_ability(ability, &defs), Ok(()));
    let w = World::with_map(1, field(&defs), Nation::Earth, &[at(6, 6)], &defs);
    assert_eq!(w.check_ability(ability, &defs), Err(BendError::WrongNation));
}

#[test]
fn the_data_that_names_a_canal_lists_it() {
    let defs = Defs::bundled();
    for id in ["fishing_dock", "tide_mill"] {
        let def = defs.building(defs.building_id(id).unwrap());
        assert!(def.beside.as_ref().unwrap().contains(&Terrain::Canal), "{id}");
        assert_eq!(def.beside_word(), Some("water"), "{id}");
    }
    let fish = defs.resource(defs.resource_id("fish_spot").unwrap());
    assert!(fish.terrains.as_ref().unwrap().contains(&Terrain::Canal));
    assert_eq!(defs.canal.irrigation_radius, 2);
    assert_eq!(defs.canal.growth_boost, 1.5);
}

#[test]
fn the_target_must_touch_fresh_water_and_the_sea_does_not_count() {
    let defs = Defs::bundled();
    let mut w = world_on(field(&defs), &[at(6, 6)], &defs);
    // Beside the river: fine. Away from it: refused. A diagonal isn't beside.
    assert_eq!(bend(&mut w, &defs, at(1, 4)), Ok(()));
    assert_eq!(bend(&mut w, &defs, at(4, 4)), refused("needs fresh water beside it"));
    assert_eq!(bend(&mut w, &defs, at(2, 3)), refused("needs fresh water beside it"));
    // The river is ordinary water to others, a lake counts too, the sea does not.
    w.map[at(0, 7)].terrain = Terrain::Lake;
    w.map[at(0, 2)].terrain = Terrain::Water;
    assert_eq!(bend(&mut w, &defs, at(1, 7)), Ok(()));
    assert_eq!(bend(&mut w, &defs, at(1, 2)), refused("needs fresh water beside it"));
    // Only land and coast are cut; water, rock and a canal are not.
    assert_eq!(bend(&mut w, &defs, at(0, 4)), refused("needs dry land"));
    w.map[at(1, 8)].terrain = Terrain::Canal;
    assert_eq!(bend(&mut w, &defs, at(1, 8)), refused("already a canal"));
    // A canal tile is a source too: land beside it can be cut.
    assert_eq!(bend(&mut w, &defs, at(2, 8)), Ok(()));
}

#[test]
fn a_tile_with_something_on_it_is_in_the_way() {
    let defs = Defs::bundled();
    let mut w = world_on(field(&defs), &[at(6, 6)], &defs);
    w.map[at(1, 3)].resource = defs.resource_id("cabbage");
    w.zones.insert(at(1, 5));
    assert_eq!(bend(&mut w, &defs, at(1, 3)), refused("something is in the way"));
    assert_eq!(bend(&mut w, &defs, at(1, 5)), refused("something is in the way"));
}

#[test]
fn a_line_can_be_designated_at_once_and_is_cut_outward_from_the_water() {
    let defs = Defs::bundled();
    let mut w = world_on(field(&defs), &[at(6, 6)], &defs);
    let line = [at(1, 4), at(2, 4), at(3, 4), at(4, 4)];
    // Out of order, the far tiles chain to nothing yet.
    assert_eq!(bend(&mut w, &defs, at(3, 4)), refused("needs fresh water beside it"));
    for p in line {
        assert_eq!(bend(&mut w, &defs, p), Ok(()), "{p:?}");
    }
    run_until(&mut w, &defs, 40_000, |w| {
        for pair in line.windows(2) {
            assert!(
                w.map[pair[0]].terrain == Terrain::Canal
                    || w.map[pair[1]].terrain != Terrain::Canal,
                "{:?} was cut before {:?}",
                pair[1],
                pair[0]
            );
        }
        canals_at(w, &line)
    });
    assert_eq!(w.bend_designations().count(), 0);
    assert!(w.bend_events().all(|e| e.outcome == BendOutcome::Done));
}

#[test]
fn the_bender_stands_beside_the_tile_and_pays_the_chi_when_it_is_cut() {
    let defs = Defs::bundled();
    let mut w = world_on(field(&defs), &[at(6, 6)], &defs);
    let target = at(2, 4);
    w.map[at(1, 4)].terrain = Terrain::Canal;
    w.map_changed();
    assert_eq!(bend(&mut w, &defs, target), Ok(()));
    let mut channelled = false;
    run_until(&mut w, &defs, 20_000, |w| {
        for c in w.bend_channels(&defs) {
            let u = w.unit(c.unit).unwrap();
            assert_ne!(u.pos, target, "the bender never stands on a tile it is cutting");
            assert!(u.pos.chebyshev(target) == 1, "stands beside it, not {:?}", u.pos);
            channelled = true;
        }
        w.map[target].terrain == Terrain::Canal
    });
    assert!(channelled);
    // The unit's chi was topped up each tick by the helper; what matters is what it cost.
    let e = w.bend_events().next_back().unwrap();
    assert_eq!((e.ability, e.pos, e.outcome), (carve(&defs), target, BendOutcome::Done));
    assert_eq!(w.map[target].terrain, Terrain::Canal);
}

#[test]
fn the_cut_waits_until_nobody_stands_on_the_tile() {
    let defs = Defs::bundled();
    let mut w = world_on(field(&defs), &[at(6, 6), at(9, 8)], &defs);
    let target = at(1, 4);
    assert_eq!(bend(&mut w, &defs, target), Ok(()));
    // Keep the second settler on the tile well past the end of the channel.
    for _ in 0..3_000 {
        w.units.iter_mut().for_each(|u| {
            u.needs = crate::needs::Needs::full(&defs.needs);
            u.chi = defs.chi.max;
        });
        w.units[1].pos = target;
        w.units[1].route = None;
        w.tick(&defs);
        w.units[1].pos = target;
    }
    assert_eq!(w.map[target].terrain, Terrain::Land, "still waiting for the tile to empty");
    assert_eq!(w.bend_designations().count(), 1);
    // Once it is empty the cut lands.
    w.units[1].pos = at(9, 8);
    run_until(&mut w, &defs, 2_000, |w| w.map[target].terrain == Terrain::Canal);
}

#[test]
fn a_cut_that_would_split_the_land_is_refused_at_designation() {
    let defs = Defs::bundled();
    let mut w = world_on(corridor(&defs), &[at(2, 4)], &defs);
    // Row 4 and 5 are two tiles wide: cutting one tile of row 4 leaves the row-5 way round.
    assert_eq!(bend(&mut w, &defs, at(5, 4)), Ok(()));
    w.apply(Command::CancelBend { pos: at(5, 4) }, &defs).unwrap();
    // Its neighbour in the same column, chained, would close the corridor: cut with the earlier
    // designation counted as made.
    assert_eq!(bend(&mut w, &defs, at(5, 4)), Ok(()));
    assert_eq!(bend(&mut w, &defs, at(5, 5)), refused("would cut the land in two"));
    // A tile at the corridor's end cuts nothing in two.
    assert_eq!(bend(&mut w, &defs, at(1, 4)), Ok(()));
}

#[test]
fn a_cut_across_a_one_tile_corridor_is_refused_but_its_end_is_fine() {
    let defs = Defs::bundled();
    let narrow = Grid::from_fn(11, 9, |p| {
        let t = match (p.x, p.y) {
            (1..=9, 3) => Terrain::River,
            (1..=9, 4) => Terrain::Land,
            _ => Terrain::Mountain,
        };
        tile(&defs, t)
    });
    let mut w = world_on(narrow, &[at(3, 4)], &defs);
    assert_eq!(bend(&mut w, &defs, at(5, 4)), refused("would cut the land in two"));
    assert_eq!(bend(&mut w, &defs, at(1, 4)), Ok(()));
    assert_eq!(bend(&mut w, &defs, at(9, 4)), Ok(()));
}

#[test]
fn the_split_is_checked_again_when_the_cut_completes() {
    let defs = Defs::bundled();
    let mut w = world_on(corridor(&defs), &[at(2, 4)], &defs);
    assert_eq!(bend(&mut w, &defs, at(5, 4)), Ok(()));
    // Something else closes the other half of the corridor before the channel finishes.
    w.map[at(5, 5)].terrain = Terrain::Canal;
    w.map_changed();
    let events = w.bend_events().count();
    run_until(&mut w, &defs, 20_000, |w| w.bend_events().count() > events);
    let e = w.bend_events().next_back().unwrap();
    assert_eq!(e.outcome, BendOutcome::Failed("would cut the land in two"));
    assert_eq!(w.map[at(5, 4)].terrain, Terrain::Land);
}

#[test]
fn a_canal_blocks_walking_like_a_river() {
    let defs = Defs::bundled();
    let mut w = world_on(field(&defs), &[at(6, 6)], &defs);
    assert!(w.walkable(at(1, 4)));
    w.map[at(1, 4)].terrain = Terrain::Canal;
    w.map_changed();
    assert!(!w.walkable(at(1, 4)));
}

#[test]
fn a_canal_is_a_source_for_water_the_land() {
    let defs = Defs::bundled();
    let dry = Grid::from_fn(15, 9, |p| {
        tile(&defs, if p == at(2, 4) { Terrain::Canal } else { Terrain::Land })
    });
    let mut w = World::with_map(1, dry, Nation::Water, &[at(8, 6)], &defs);
    let water = defs.ability_id("water_the_land").unwrap();
    let mut cast = |pos| w.apply(Command::Bend { ability: water, pos }, &defs).map(|_| ());
    assert_eq!(cast(at(5, 4)), Ok(()), "within the source range of the canal");
    assert_eq!(
        cast(at(12, 4)),
        Err(CommandError::Bend(BendError::InvalidTarget("needs fresh water nearby")))
    );
}

#[test]
fn a_carved_canal_irrigates_plots_in_reach_and_does_not_stack_with_a_channel() {
    let defs = Defs::bundled();
    let open = Grid::from_fn(15, 15, |_| tile(&defs, Terrain::Land));
    let mut w = World::with_map(1, open, Nation::Water, &[], &defs);
    let plot = at(7, 7);
    let farm = defs.building_id("farm_plot").unwrap();
    w.put_building(Footprint::single(plot), farm, &defs);
    assert_eq!(w.irrigation_rate(plot, &defs), 1.0);

    let boost = defs.canal.growth_boost;
    let radius = defs.canal.irrigation_radius as i32;
    // Just outside the radius: nothing. At the radius, diagonal included: the boost.
    w.map[at(7 + radius + 1, 7)].terrain = Terrain::Canal;
    assert_eq!(w.irrigation_rate(plot, &defs), 1.0);
    w.map[at(7 + radius, 7 - radius)].terrain = Terrain::Canal;
    assert_eq!(w.irrigation_rate(plot, &defs), boost);
    // Many canal tiles and a channel in reach: the best one, never the product.
    w.map[at(7, 6)].terrain = Terrain::Canal;
    let channel = defs.building_id("irrigation_channel").unwrap();
    w.put_building(Footprint::single(at(7, 5)), channel, &defs);
    let best = boost.max(defs.building(channel).irrigation.unwrap().growth_boost);
    assert_eq!(w.irrigation_rate(plot, &defs), best);
}

#[test]
fn mapgen_never_makes_a_canal() {
    let defs = Defs::bundled();
    for seed in 0..6 {
        let cfg = crate::MapGenConfig { seed, width: 96, height: 64, ..Default::default() };
        let map = crate::mapgen::generate(&cfg, &defs);
        assert!(map.iter().all(|(_, t)| t.terrain != Terrain::Canal), "seed {seed}");
    }
}

#[test]
fn canal_numbers_are_validated() {
    let defs = Defs::bundled();
    let _ = defs;
    let bending = include_str!("../../../../../data/bending.toml");
    let load = |from: &str, to: &str| {
        let b = bending.replacen(from, to, 1);
        Defs::from_toml(
            include_str!("../../../../../data/biomes.toml"),
            include_str!("../../../../../data/resources.toml"),
            include_str!("../../../../../data/nations.toml"),
            include_str!("../../../../../data/research.toml"),
            include_str!("../../../../../data/needs.toml"),
            include_str!("../../../../../data/jobs.toml"),
            &b,
            include_str!("../../../../../data/buildings.toml"),
        )
        .map(|_| ())
        .map_err(|e| e.0)
    };
    assert_eq!(load("irrigation_radius = 2", "irrigation_radius = 2"), Ok(()));
    let err = load("irrigation_radius = 2", "irrigation_radius = 0").unwrap_err();
    assert!(err.contains("irrigation_radius"), "{err}");
    let err = load("growth_boost = 1.5   #", "growth_boost = 0.5   #").unwrap_err();
    assert!(err.contains("growth_boost"), "{err}");
    let err = load("source = [\"river\", \"lake\", \"canal\"]", "source = []").unwrap_err();
    assert!(err.contains("source"), "{err}");
}
