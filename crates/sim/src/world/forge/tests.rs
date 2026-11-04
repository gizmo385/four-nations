use super::*;
use crate::building::{Building, BuildingId};
use crate::defs::{BiomeId, Terrain};
use crate::geom::{Footprint, Grid};
use crate::mapgen::Tile;
use crate::world::Task;

/// A map of plain, walkable land.
fn arena(rows: &[&str]) -> TileMap {
    Grid::from_fn(rows[0].len() as i32, rows.len() as i32, |_| Tile {
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

fn setup(rows: &[&str], units: &[Pos]) -> (World, Defs) {
    let defs = Defs::bundled();
    let map = arena(rows);
    (World::with_map(1, map, Nation::Fire, units, &defs), defs)
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

/// Puts a finished building of `id` at `anchor` (skipping the construction job), covering its
/// real footprint, and opens its craft jobs the way `run_build` does the moment one completes.
fn place_building(w: &mut World, defs: &Defs, id: BuildingId, anchor: Pos) {
    let def = defs.building(id);
    let (width, height) = def.footprint;
    let foot = Footprint::new(anchor, width, height);
    for p in foot.tiles() {
        let part = [(p.x - anchor.x) as u8, (p.y - anchor.y) as u8];
        w.map[p].building = Some(Building {
            part,
            size: [width, height],
            ..Building::new(id, def.passable, false)
        });
    }
    w.map_changed();
    w.open_craft_jobs(anchor, def);
}

#[test]
fn a_finished_production_building_opens_one_craft_job_per_capacity() {
    let (mut w, defs) = setup(&grid(), &[at(1, 1)]);
    let smelter = defs.building_id("smelter").unwrap();
    let def = defs.building(smelter);
    place_building(&mut w, &defs, smelter, at(4, 4));
    assert_eq!(w.jobs().len(), def.capacity as usize);
    assert!(w.jobs().iter().all(|j| j.kind == crate::jobs::JobKind::Craft { pos: at(4, 4) }));
}

/// Places a finished `building`, stocks three batches' worth of its recipe's (single) input
/// right beside it, and checks the output appears and the input was spent: the shared check
/// behind the smelter (ore to metal), the brick kiln (stone to bricks) and the forge's smith job
/// (metal to tools).
fn runs_one_batch(building: &str) {
    let (mut w, defs) = setup(&grid(), &[at(1, 1)]);
    let id = defs.building_id(building).unwrap();
    let recipe =
        defs.building(id).recipe.clone().unwrap_or_else(|| panic!("{building} has a recipe"));
    assert_eq!(recipe.inputs.len(), 1, "every Fire production recipe today has one input");
    let anchor = at(4, 4);
    place_building(&mut w, &defs, id, anchor);

    // Plenty of input, right on the stockpile, for more than one batch.
    w.designate_stockpile(at(0, 0)).unwrap();
    let before = recipe.inputs[0].count * 3;
    w.stored.insert(at(0, 0), Stack { item: recipe.inputs[0].item, count: before });

    run_until(&mut w, &defs, 6000, |w| w.ground_items().any(|(_, s)| s.item == recipe.output.item));
    let (_, out) = w.ground_items().find(|(_, s)| s.item == recipe.output.item).unwrap();
    assert_eq!(out.count, recipe.output.count, "{building}: one batch's worth");
    assert!(w.stored_total(recipe.inputs[0].item) < before, "{building}: the input was spent");
}

#[test]
fn the_smelter_turns_ore_into_metal() {
    runs_one_batch("smelter");
}

#[test]
fn the_brick_kiln_turns_stone_into_bricks() {
    runs_one_batch("brick_kiln");
}

#[test]
fn the_forges_smith_job_turns_metal_into_tools() {
    runs_one_batch("forge");
}

#[test]
fn a_craft_waits_on_missing_inputs_instead_of_failing() {
    let (mut w, defs) = setup(&grid(), &[at(1, 1)]);
    let smelter = defs.building_id("smelter").unwrap();
    let anchor = at(4, 4);
    place_building(&mut w, &defs, smelter, anchor);
    // No ore anywhere: the job waits rather than erroring out or vanishing.
    for _ in 0..500 {
        w.tick(&defs);
    }
    assert_eq!(w.jobs().len(), 1);
    assert!(w.ground_items().next().is_none());
}

#[test]
fn a_crafter_works_inside_the_building() {
    let (mut w, defs) = setup(&grid(), &[at(1, 1)]);
    let smelter = defs.building_id("smelter").unwrap();
    let recipe = defs.building(smelter).recipe.clone().unwrap();
    let anchor = at(4, 4);
    place_building(&mut w, &defs, smelter, anchor);
    w.designate_stockpile(at(0, 0)).unwrap();
    w.stored.insert(at(0, 0), Stack { item: recipe.inputs[0].item, count: recipe.inputs[0].count });

    run_until(&mut w, &defs, 6000, |w| {
        matches!(w.units[0].task(), Some(Task::Craft(_))) && w.units[0].inside().is_some()
    });
}

/// A plain grid of land, big enough for a 2x2 building and a stockpile tile.
fn grid() -> [&'static str; 10] {
    [
        "..........",
        "..........",
        "..........",
        "..........",
        "..........",
        "..........",
        "..........",
        "..........",
        "..........",
        "..........",
    ]
}
