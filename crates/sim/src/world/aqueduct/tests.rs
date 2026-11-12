use super::*;
use crate::defs::{BiomeId, Terrain};
use crate::geom::{Footprint, Grid};
use crate::mapgen::Tile;
use crate::world::Task;

const W: i32 = 40;
const H: i32 = 12;

/// A 40x12 field of land with a river down column 0.
fn field() -> TileMap {
    Grid::from_fn(W, H, |p| Tile {
        elevation: 100,
        temperature: 128,
        moisture: 128,
        terrain: if p.x == 0 { Terrain::River } else { Terrain::Land },
        biome: BiomeId(0),
        resource: None,
        building: None,
    })
}

fn at(x: i32, y: i32) -> Pos {
    Pos::new(x, y)
}

fn setup(units: &[Pos]) -> (World, Defs) {
    let defs = Defs::bundled();
    (World::with_map(1, field(), Nation::Earth, units, &defs), defs)
}

fn raise(w: &mut World, defs: &Defs, name: &str, pos: Pos) {
    let id = defs.building_id(name).unwrap();
    w.put_building(Footprint::single(pos), id, defs);
}

/// A finished aqueduct on every tile of the row `y`, `from..=to`.
fn line(w: &mut World, defs: &Defs, y: i32, from: i32, to: i32) {
    for x in from..=to {
        raise(w, defs, "aqueduct", at(x, y));
    }
}

fn aqueduct_def(defs: &Defs) -> crate::building::AqueductDef {
    defs.building(defs.building_id("aqueduct").unwrap()).aqueduct.clone().expect("an aqueduct")
}

#[test]
fn the_aqueduct_and_its_tech_are_data() {
    let defs = Defs::bundled();
    let id = defs.building_id("aqueduct").unwrap();
    let def = defs.building(id);
    assert!(def.passable, "people walk under the arches");
    assert_eq!(def.footprint, (1, 1));
    assert_eq!(def.work, 12.0);
    let a = aqueduct_def(&defs);
    assert_eq!(a.source, [Terrain::River, Terrain::Lake, Terrain::Canal]);
    assert_eq!((a.irrigation_radius, a.growth_boost), (2, 1.5));
    assert_eq!(def.cost.len(), 1);
    assert_eq!(def.cost[0].count, 3);
    assert_eq!(def.cost[0].item, defs.item_id("stone").unwrap());
    // Only the aqueduct carries water.
    assert_eq!(defs.buildings.iter().filter(|b| b.aqueduct.is_some()).count(), 1);

    let tech = defs.tech(defs.tech_id("aqueducts").unwrap());
    assert_eq!(tech.fusion, Some([Nation::Earth, Nation::Water]));
    let masonry = defs.tech_id("masonry").unwrap();
    let farming = defs.tech_id("farming").unwrap();
    assert_eq!(tech.prereqs, [masonry, farming]);
    assert_eq!(tech.cost, 1000);
    assert_eq!(tech.unlocks.buildings, ["aqueduct"]);
    assert!(!tech.core);
}

#[test]
fn a_tile_beside_a_source_flows_and_so_does_the_line_it_feeds() {
    let (mut w, defs) = setup(&[at(20, 8)]);
    // (1..=6, 5) touches the river at (0, 5) only through its first tile.
    line(&mut w, &defs, 5, 1, 6);
    for x in 1..=6 {
        assert!(w.is_flowing(at(x, 5), &defs), "tile {x} flows");
    }
    // A run with no source beside it is dry, flowing nowhere.
    line(&mut w, &defs, 9, 10, 14);
    for x in 10..=14 {
        assert!(!w.is_flowing(at(x, 9), &defs), "tile {x} is dry");
    }
    // Not an aqueduct at all.
    assert!(!w.is_flowing(at(3, 3), &defs));
}

#[test]
fn a_diagonal_touch_does_not_feed_a_tile() {
    let (mut w, defs) = setup(&[at(20, 8)]);
    // (1, 5) touches the river at (0, 5) by an edge; (1, 6)'s column neighbour (0, 6) is river,
    // so lift the test off the bank: a lone tile at (2, 5) is two from the river.
    raise(&mut w, &defs, "aqueduct", at(2, 5));
    assert!(!w.is_flowing(at(2, 5), &defs));
    // A tile whose only river contact is diagonal: bend the river off column 0 at row 7.
    w.map[at(0, 7)].terrain = Terrain::Land;
    w.map[at(0, 8)].terrain = Terrain::Land;
    w.map[at(0, 6)].terrain = Terrain::River;
    w.map_changed();
    raise(&mut w, &defs, "aqueduct", at(1, 7));
    assert!(!w.is_flowing(at(1, 7), &defs), "(0, 6) is diagonal to (1, 7)");
    raise(&mut w, &defs, "aqueduct", at(1, 6));
    assert!(w.is_flowing(at(1, 6), &defs));
    assert!(w.is_flowing(at(1, 7), &defs), "fed through (1, 6) now");
}

#[test]
fn a_lake_and_a_canal_feed_a_line_too() {
    let (mut w, defs) = setup(&[at(20, 8)]);
    w.map[at(20, 3)].terrain = Terrain::Lake;
    w.map[at(30, 3)].terrain = Terrain::Canal;
    w.map[at(25, 3)].terrain = Terrain::Water;
    w.map_changed();
    raise(&mut w, &defs, "aqueduct", at(20, 4));
    raise(&mut w, &defs, "aqueduct", at(30, 4));
    raise(&mut w, &defs, "aqueduct", at(25, 4));
    assert!(w.is_flowing(at(20, 4), &defs), "a lake");
    assert!(w.is_flowing(at(30, 4), &defs), "a canal");
    assert!(!w.is_flowing(at(25, 4), &defs), "the sea is not a source");
}

#[test]
fn the_bitmap_follows_a_tile_going_up_and_coming_down() {
    let (mut w, defs) = setup(&[at(20, 8)]);
    line(&mut w, &defs, 5, 1, 6);
    w.tick(&defs);
    assert!(w.flowing.is_some(), "the tick rebuilds the bitmap");
    assert!(w.is_flowing(at(6, 5), &defs));

    // Cut the run at (3, 5): the far half runs dry at once, before any tick.
    let key = w.map[at(3, 5)].building.unwrap().key();
    w.remove_building(key, &defs).unwrap();
    assert!(w.flowing.is_none(), "stale until the next tick");
    assert!(w.is_flowing(at(2, 5), &defs));
    assert!(!w.is_flowing(at(4, 5), &defs), "cut off from the river");
    assert!(!w.is_flowing(at(6, 5), &defs));
    w.tick(&defs);
    assert!(w.flowing.is_some());
    assert!(!w.is_flowing(at(6, 5), &defs));

    // Raising it again reconnects the run.
    raise(&mut w, &defs, "aqueduct", at(3, 5));
    assert!(w.is_flowing(at(6, 5), &defs));
}

#[test]
fn nothing_is_saved_and_a_loaded_world_floods_again() {
    let (mut w, defs) = setup(&[at(20, 8)]);
    line(&mut w, &defs, 5, 1, 4);
    w.tick(&defs);
    let text = w.save(&defs).unwrap();
    let loaded = World::load(&text, &defs).unwrap();
    assert!(loaded.flowing.is_none(), "the bitmap is derived, not saved");
    assert!(loaded.is_flowing(at(4, 5), &defs));
    let mut loaded = loaded;
    loaded.tick(&defs);
    assert!(loaded.flowing.is_some(), "rebuilt on the first tick");
}

#[test]
fn a_flowing_tile_is_a_drinking_place() {
    // The river is 30 tiles from the unit; the aqueduct reaches to 5 tiles from it.
    let (mut w, defs) = setup(&[at(30, 5)]);
    line(&mut w, &defs, 5, 1, 24);
    w.units[0].needs.thirst = defs.needs.thirst.low - 1.0;
    w.tick(&defs);
    assert_eq!(w.units[0].task(), Some(Task::Drink));
    let stand = *w.units[0].route.as_ref().and_then(|r| r.first()).expect("a route");
    assert!(
        (stand.x - 24).abs() <= 1 && (stand.y - 5).abs() <= 1,
        "drinks beside the end of the line, not at the river: {stand:?}"
    );
}

#[test]
fn a_dry_aqueduct_is_no_drinking_place() {
    let defs = Defs::bundled();
    let mut map = field();
    for y in 0..H {
        map[at(0, y)].terrain = Terrain::Land;
    }
    let mut w = World::with_map(1, map, Nation::Earth, &[at(10, 5)], &defs);
    line(&mut w, &defs, 5, 8, 12);
    w.units[0].needs.thirst = defs.needs.thirst.low - 1.0;
    w.tick(&defs);
    assert_eq!(w.units[0].task(), None, "no water anywhere: nowhere to drink");
}

#[test]
fn water_beside_an_aqueduct_is_fresh_and_never_makes_anyone_sick() {
    let defs = Defs::bundled();
    let mut map = field();
    // Sea at (10, 3): the tile (11, 3) is beside it, and the run (12, 2), (13, 2) ends at a
    // river at (14, 2).
    map[at(10, 3)].terrain = Terrain::Water;
    map[at(14, 2)].terrain = Terrain::River;
    let mut w = World::with_map(1, map, Nation::Earth, &[at(20, 8)], &defs);
    assert!(w.sickness_chance(at(11, 3), &defs).is_some(), "sea water alone rolls sickness");
    line(&mut w, &defs, 2, 12, 13);
    assert!(w.is_flowing(at(12, 2), &defs));
    assert_eq!(w.sickness_chance(at(11, 3), &defs), None);
}

#[test]
fn a_flowing_aqueduct_irrigates_plots_in_reach_and_a_dry_one_does_not() {
    let (mut w, defs) = setup(&[at(30, 9)]);
    let a = aqueduct_def(&defs);
    let r = a.irrigation_radius as i32;
    line(&mut w, &defs, 5, 1, 10);
    line(&mut w, &defs, 10, 20, 24);
    // Within the radius of the flowing run, at its far end.
    assert_eq!(w.irrigation_rate(at(10, 5 - r), &defs), a.growth_boost);
    assert_eq!(w.irrigation_rate(at(10 + r, 5 + r), &defs), a.growth_boost);
    // One tile past the radius.
    assert_eq!(w.irrigation_rate(at(10, 5 - r - 1), &defs), 1.0);
    // Beside the dry run: nothing.
    assert_eq!(w.irrigation_rate(at(22, 10 - r), &defs), 1.0);
    // Cutting the run dries what is downstream of the cut.
    let key = w.map[at(5, 5)].building.unwrap().key();
    w.remove_building(key, &defs).unwrap();
    assert_eq!(w.irrigation_rate(at(10, 5 - r), &defs), 1.0);
    assert_eq!(w.irrigation_rate(at(2, 5 - r), &defs), a.growth_boost);
}

#[test]
fn irrigation_takes_the_best_boost_never_the_sum() {
    let mut defs = Defs::bundled();
    let id = defs.building_id("aqueduct").unwrap();
    defs.buildings[id.0 as usize].aqueduct.as_mut().unwrap().growth_boost = 3.0;
    let mut w = World::with_map(1, field(), Nation::Earth, &[at(30, 9)], &defs);
    let plot = at(10, 5);
    raise(&mut w, &defs, "irrigation_channel", at(10, 6));
    let channel = w.irrigation_rate(plot, &defs);
    assert!(channel > 1.0 && channel < 3.0);
    line(&mut w, &defs, 5, 1, 6);
    assert_eq!(w.irrigation_rate(plot, &defs), 1.0_f32.max(channel), "aqueduct out of reach");
    raise(&mut w, &defs, "aqueduct", at(7, 5));
    assert_eq!(w.irrigation_rate(plot, &defs), 1.0_f32.max(channel), "three tiles is too far");
    raise(&mut w, &defs, "aqueduct", at(8, 5));
    assert_eq!(w.irrigation_rate(plot, &defs), 3.0, "the best of the two");
    raise(&mut w, &defs, "aqueduct", at(9, 5));
    assert_eq!(w.irrigation_rate(plot, &defs), 3.0, "two tiles do not stack");
}

#[test]
fn the_cursor_reads_a_run_as_flowing_or_dry() {
    let (mut w, defs) = setup(&[at(30, 9)]);
    line(&mut w, &defs, 7, 1, 5);
    line(&mut w, &defs, 7, 8, 10);
    assert_eq!(
        w.aqueduct_flow(at(4, 7), &defs),
        Some(AqueductFlow::Flowing { tiles: 5, from: at(1, 7) })
    );
    assert_eq!(w.aqueduct_flow(at(9, 7), &defs), Some(AqueductFlow::Dry));
    assert_eq!(w.aqueduct_flow(at(15, 7), &defs), None);
}

#[test]
fn an_aqueduct_never_blocks_a_walk() {
    let (mut w, defs) = setup(&[at(5, 0)]);
    line(&mut w, &defs, 5, 1, W - 1);
    assert!(w.map[at(20, 5)].building.is_some());
    assert!(w.rules.passable(&w.map[at(20, 5)]), "raised on arches");
}
