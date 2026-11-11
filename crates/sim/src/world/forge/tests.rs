use std::collections::HashMap;

use super::*;
use crate::building::{BuildingDef, BuildingId, PowerDef};
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
    w.put_building(Footprint::new(anchor, width, height), id, defs);
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

/// Places a finished `building`, stocks three batches' worth of every one of its recipe's
/// inputs right beside it, and checks the output appears and every input was spent: the shared
/// check behind the smelter (ore and wood to metal), the brick kiln (stone and wood to bricks)
/// and the forge's smith job (metal to tools).
fn runs_one_batch(building: &str) {
    let (mut w, defs) = setup(&grid(), &[at(1, 1)]);
    let id = defs.building_id(building).unwrap();
    let recipe =
        defs.building(id).recipe.clone().unwrap_or_else(|| panic!("{building} has a recipe"));
    let anchor = at(4, 4);
    place_building(&mut w, &defs, id, anchor);

    // Plenty of every input, right on the stockpile, for more than one batch.
    let before: Vec<(ItemId, u32)> = recipe
        .inputs
        .iter()
        .enumerate()
        .map(|(n, input)| {
            let zone = at(n as i32, 0);
            w.designate_stockpile(zone).unwrap();
            let count = input.count * 3;
            w.stored.insert(zone, Stack { item: input.item, count });
            (input.item, count)
        })
        .collect();

    run_until(&mut w, &defs, 6000, |w| w.ground_items().any(|(_, s)| s.item == recipe.output.item));
    let (_, out) = w.ground_items().find(|(_, s)| s.item == recipe.output.item).unwrap();
    assert_eq!(out.count, recipe.output.count, "{building}: one batch's worth");
    for (item, count) in before {
        assert!(w.stored_total(item) < count, "{building}: the input was spent");
    }
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

/// The shared Workshop (`carpentry`, issue #369): everyone's route to tools, from wood instead
/// of the forge's metal.
#[test]
fn the_workshops_job_turns_wood_into_tools() {
    runs_one_batch("workshop");
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
    for (n, input) in recipe.inputs.iter().enumerate() {
        let zone = at(n as i32, 0);
        w.designate_stockpile(zone).unwrap();
        w.stored.insert(zone, Stack { item: input.item, count: input.count });
    }

    run_until(&mut w, &defs, 6000, |w| {
        matches!(w.units[0].task(), Some(Task::Craft(_))) && w.units[0].inside().is_some()
    });
}

/// A crafter never finishes a batch once stone and wood are gone (issue #350: with no hauler, no
/// output ever makes it into the stockpile, so `craft_idle` would otherwise go on forever).
/// Pre-stocking every input, one unit alone (the crafter) can only ever deliver one batch's
/// worth of output to the ground, where `craft_idle`'s `stored_total` doesn't see it: this
/// checks the crafter still idles once asked to start the next batch, with its inputs untouched.
#[test]
fn a_kiln_idles_at_its_stock_limit_instead_of_starting_a_batch() {
    let (mut w, defs) = setup(&grid(), &[at(1, 1)]);
    let kiln = defs.building_id("brick_kiln").unwrap();
    let recipe = defs.building(kiln).recipe.clone().unwrap();
    let anchor = at(4, 4);
    place_building(&mut w, &defs, kiln, anchor);

    // Plenty of stone and wood for many more batches than the stock limit allows.
    for (n, input) in recipe.inputs.iter().enumerate() {
        let zone = at(n as i32, 0);
        w.designate_stockpile(zone).unwrap();
        w.stored.insert(zone, Stack { item: input.item, count: input.count * 10 });
    }
    // The output is already at the limit, as if it had all been hauled away already.
    let brick_zone = at(5, 0);
    w.designate_stockpile(brick_zone).unwrap();
    w.stored.insert(brick_zone, Stack { item: recipe.output.item, count: recipe.stock_limit });

    assert!(w.craft_idle(anchor, &recipe), "idle from the start, with no batch under way");
    let stone_before = w.stored_total(recipe.inputs[0].item);
    for _ in 0..2000 {
        w.tick(&defs);
    }
    assert_eq!(w.stored_total(recipe.output.item), recipe.stock_limit, "no new bricks");
    assert_eq!(w.stored_total(recipe.inputs[0].item), stone_before, "no stone was spent idling");
    assert!(w.ground_items().next().is_none(), "nothing fetched for a batch that never starts");

    // Taking bricks below the limit lets it resume and spend stone again.
    w.stored.remove(&brick_zone);
    assert!(!w.craft_idle(anchor, &recipe), "resumed once stock dropped below the limit");
    run_until(&mut w, &defs, 6000, |w| w.stored_total(recipe.inputs[0].item) < stone_before);
}

/// A smelter with gold ore and wood stocked well past its needs never eats more than the stock
/// limit's worth, plus at most one batch already under way when the hauler's delivery first
/// brings the stockpile to the limit (issue #350, raised to 6 by #369; `craft_idle` only sees
/// what's *stockpiled*, not what's on the ground waiting for a haul, so one extra batch can
/// already be committed the instant the limit is reached): 6 metal (plus at most one more) at
/// 2 ore a batch, however long it runs with a hauler to carry the metal into the stockpile.
#[test]
fn a_smelter_never_eats_more_ore_than_its_stock_limit_allows() {
    let (mut w, defs) = setup(&grid(), &[at(1, 1), at(2, 1)]);
    let smelter = defs.building_id("smelter").unwrap();
    let recipe = defs.building(smelter).recipe.clone().unwrap();
    let ore = defs.item_id("gold_ore").unwrap();
    let anchor = at(4, 4);
    place_building(&mut w, &defs, smelter, anchor);

    // Plenty of ore and wood, well past what the stock limit allows the smelter to use.
    for (n, input) in recipe.inputs.iter().enumerate() {
        let zone = at(n as i32, 0);
        w.designate_stockpile(zone).unwrap();
        w.stored.insert(zone, Stack { item: input.item, count: input.count * 20 });
    }
    let ore_count = recipe.inputs.iter().find(|c| c.item == ore).unwrap().count;
    let ore_before = ore_count * 20;
    // Somewhere for the second unit to haul metal to, so `stored_total` ever sees it.
    w.designate_stockpile(at(5, 0)).unwrap();

    run_until(&mut w, &defs, 40_000, |w| w.stored_total(recipe.output.item) >= recipe.stock_limit);
    // Give any batch already committed when the limit was first reached time to finish and be
    // hauled in, then settle: no more ore is spent however long we keep ticking after that.
    for _ in 0..3000 {
        w.tick(&defs);
    }
    let settled = w.stored_total(recipe.output.item);
    let ceiling = recipe.stock_limit + recipe.output.count;
    assert!(settled <= ceiling, "at most one batch past the limit, got {settled}");
    for _ in 0..3000 {
        w.tick(&defs);
    }
    assert_eq!(w.stored_total(recipe.output.item), settled, "settled, no further batches");
    let ore_spent = ore_before - w.stored_total(ore);
    let max_spend = ore_count * ceiling;
    assert!(
        ore_spent <= max_spend,
        "never eats more ore than the stock limit (plus one batch) allows"
    );
}

/// The shared Workshop (#369) makes tools from wood and stops once about 6 are stored, the same
/// way the forge and the smelter stop at their own limits (#350). As with the smelter below,
/// `craft_idle` only sees what's stockpiled, not what's on the ground waiting for a haul, so one
/// extra batch can already be committed the instant the limit is reached.
#[test]
fn a_workshop_makes_tools_from_wood_and_stops_at_its_stock_limit() {
    let (mut w, defs) = setup(&grid(), &[at(1, 1), at(2, 1)]);
    let workshop = defs.building_id("workshop").unwrap();
    let recipe = defs.building(workshop).recipe.clone().unwrap();
    let wood = defs.item_id("wood").unwrap();
    let anchor = at(4, 4);
    place_building(&mut w, &defs, workshop, anchor);

    // Plenty of wood, well past what the stock limit allows the Workshop to use.
    for (n, input) in recipe.inputs.iter().enumerate() {
        let zone = at(n as i32, 0);
        w.designate_stockpile(zone).unwrap();
        w.stored.insert(zone, Stack { item: input.item, count: input.count * 20 });
    }
    let wood_count = recipe.inputs.iter().find(|c| c.item == wood).unwrap().count;
    let wood_before = wood_count * 20;
    // Somewhere for the second unit to haul tools to, so `stored_total` ever sees them.
    w.designate_stockpile(at(5, 0)).unwrap();

    run_until(&mut w, &defs, 30_000, |w| w.stored_total(recipe.output.item) >= recipe.stock_limit);
    // Give any batch already committed when the limit was first reached time to finish and be
    // hauled in, then settle: no more wood is spent however long we keep ticking after that.
    for _ in 0..3000 {
        w.tick(&defs);
    }
    let settled = w.stored_total(recipe.output.item);
    let ceiling = recipe.stock_limit + recipe.output.count;
    assert!(settled <= ceiling, "at most one batch past the limit, got {settled}");
    for _ in 0..3000 {
        w.tick(&defs);
    }
    assert_eq!(w.stored_total(recipe.output.item), settled, "settled, no further batches");
    let wood_spent = wood_before - w.stored_total(wood);
    assert!(wood_spent <= wood_count * ceiling, "never spends more wood than the limit allows");
}

/// Earth's Mine Shaft (issue #373): a source recipe (`input = []`) still needs a worker inside,
/// and still takes `work` seconds -- it just has nothing to fetch first.
#[test]
fn a_mine_shaft_cuts_stone_with_no_input_to_fetch() {
    let (mut w, defs) = setup(&grid(), &[at(1, 1)]);
    let shaft = defs.building_id("mine_shaft").unwrap();
    let recipe = defs.building(shaft).recipe.clone().unwrap();
    assert!(recipe.inputs.is_empty(), "a source recipe: nothing to fetch");
    let anchor = at(4, 4);
    place_building(&mut w, &defs, shaft, anchor);

    // The crafter heads straight inside: there's no input trip to make first.
    run_until(&mut w, &defs, 200, |w| w.units[0].inside().is_some());
    run_until(&mut w, &defs, 6000, |w| w.ground_items().any(|(_, s)| s.item == recipe.output.item));
    let (_, out) = w.ground_items().find(|(_, s)| s.item == recipe.output.item).unwrap();
    assert_eq!(out.count, recipe.output.count);
}

/// A no-input recipe idles at its `stock_limit` exactly like any other (#350): the mechanic
/// doesn't care whether there was anything to fetch, only whether a batch is already under way
/// and how much output is already stored. Earth's data leaves the Mine Shaft's limit unset (it
/// never runs out on its own), so this sets one for the test.
#[test]
fn a_mine_shafts_source_recipe_idles_at_its_stock_limit_like_any_other() {
    let (mut w, mut defs) = setup(&grid(), &[at(1, 1)]);
    let shaft = defs.building_id("mine_shaft").unwrap();
    defs.buildings[shaft.0 as usize].recipe.as_mut().unwrap().stock_limit = 3;
    let recipe = defs.building(shaft).recipe.clone().unwrap();
    let anchor = at(4, 4);
    place_building(&mut w, &defs, shaft, anchor);

    // The output is already at the limit, as if it had all been hauled away already.
    let zone = at(5, 0);
    w.designate_stockpile(zone).unwrap();
    w.stored.insert(zone, Stack { item: recipe.output.item, count: recipe.stock_limit });
    assert!(w.craft_idle(anchor, &recipe), "idle from the start, with no batch under way");
    for _ in 0..2000 {
        w.tick(&defs);
    }
    assert_eq!(w.stored_total(recipe.output.item), recipe.stock_limit, "no new stone");
    assert!(w.ground_items().next().is_none(), "nothing worked for a batch that never starts");

    // Below the limit, a batch resumes: the crafter still has to walk in and spend the work.
    w.stored.remove(&zone);
    assert!(!w.craft_idle(anchor, &recipe));
    run_until(&mut w, &defs, 6000, |w| w.ground_items().any(|(_, s)| s.item == recipe.output.item));
}

/// Earth's Deep Mine (issue #373): its gold ore comes from a recipe, not a harvest, so it never
/// touches the "Ore mined" tally (`data/endeavor.toml`) -- the same way Metalbend's own metal
/// doesn't (`World::tallies`, `world/work.rs`'s `count_harvest`).
#[test]
fn a_deep_mines_ore_does_not_add_to_the_ore_mined_tally() {
    let (mut w, defs) = setup(&grid(), &[at(1, 1)]);
    let mine = defs.building_id("deep_mine").unwrap();
    let recipe = defs.building(mine).recipe.clone().unwrap();
    let ore = defs.item_id("gold_ore").unwrap();
    assert_eq!(recipe.output.item, ore);
    let anchor = at(4, 4);
    place_building(&mut w, &defs, mine, anchor);

    run_until(&mut w, &defs, 12_000, |w| w.ground_items().any(|(_, s)| s.item == ore));
    assert_eq!(w.tallies().harvested(ore), 0, "a recipe output isn't a harvest");
}

/// A 1x1, passable building whose only trait is `[building.power]` (issue #428): which real
/// buildings are sources is out of scope here (#255, #204, #200), so this is a test-only one.
fn test_source(defs: &mut Defs, radius: u32, boost: f32) -> BuildingId {
    let id = BuildingId(defs.buildings.len() as u16);
    defs.buildings.push(BuildingDef {
        id: "test_power_source".into(),
        name: "Test Power Source".into(),
        verb: "Build".into(),
        cost: Vec::new(),
        work: 1.0,
        passable: true,
        footprint: (1, 1),
        seats: 0,
        capacity: 0,
        bed: None,
        heat: None,
        track: None,
        orchard: None,
        recipe: None,
        farm: None,
        irrigation: None,
        healing: None,
        power: Some(PowerDef { radius, boost, boost_millis: (boost * 1000.0).round() as u32 }),
        chart: None,
        uses_power: false,
        names: HashMap::new(),
        fuel: None,
        outpost: None,
        houses_beast: false,
        pickers: None,
        hothouse: None,
        chute: None,
        pair: None,
        glide: None,
        balloon: None,
        unlocked_by: None,
        seat_upgrades: Vec::new(),
        endeavor: false,
        beside: None,
    });
    id
}

/// A Workshop within a test source's reach finishes a batch in `work / boost` seconds, not
/// `work` (issue #428): the boost scales the recipe's work rate, while the haul before it
/// (fetching wood for the batch) is unaffected and identical between the two runs, so the *gap*
/// between a powered and an unpowered run isolates exactly the time the boost saved.
#[test]
fn a_powered_workshop_finishes_a_batch_faster_by_exactly_its_boost() {
    let run = |source: Option<(u32, f32)>| {
        let mut defs = Defs::bundled();
        let anchor = at(4, 4);
        let mut w = World::with_map(1, arena(&grid()), Nation::Water, &[at(1, 1)], &defs);
        let workshop = defs.building_id("workshop").unwrap();
        place_building(&mut w, &defs, workshop, anchor);
        let recipe = defs.building(workshop).recipe.clone().unwrap();
        for (n, input) in recipe.inputs.iter().enumerate() {
            let zone = at(n as i32, 0);
            w.designate_stockpile(zone).unwrap();
            w.stored.insert(zone, Stack { item: input.item, count: input.count * 3 });
        }
        if let Some((radius, boost)) = source {
            let id = test_source(&mut defs, radius, boost);
            w.put_building(Footprint::single(at(8, 8)), id, &defs);
        }
        let ticks = run_until(&mut w, &defs, 6000, |w| {
            w.ground_items().any(|(_, s)| s.item == recipe.output.item)
        });
        (ticks, recipe.work)
    };

    let (unpowered_ticks, work) = run(None);
    let boost = 2.0;
    let (powered_ticks, _) = run(Some((6, boost)));

    assert!(powered_ticks < unpowered_ticks, "the powered run finishes sooner");
    // Water's `construction` is 1.0, so the plain run's work takes exactly `work` seconds; the
    // haul beforehand is identical either way, so the gap between the two runs is exactly the
    // time the boost saved.
    let expected_gap = (work - work / boost) * TICKS_PER_SECOND as f32;
    let gap = (unpowered_ticks - powered_ticks) as f32;
    assert!((gap - expected_gap).abs() <= 2.0, "gap {gap}, expected close to {expected_gap}");
}

/// A finished Tide Mill (issue #200) in reach of a Workshop boosts its crafting by exactly the
/// mill's 1.5x `boost`, the same mechanism `a_powered_workshop_finishes_a_batch_faster_by_
/// exactly_its_boost` checks with a test-only source, now exercised through real data.
#[test]
fn a_tide_mill_speeds_up_a_workshop_in_its_reach_by_its_own_boost() {
    let run = |with_mill: bool| {
        let defs = Defs::bundled();
        let anchor = at(4, 4);
        let mut w = World::with_map(1, arena(&grid()), Nation::Water, &[at(1, 1)], &defs);
        let workshop = defs.building_id("workshop").unwrap();
        place_building(&mut w, &defs, workshop, anchor);
        let recipe = defs.building(workshop).recipe.clone().unwrap();
        for (n, input) in recipe.inputs.iter().enumerate() {
            let zone = at(n as i32, 0);
            w.designate_stockpile(zone).unwrap();
            w.stored.insert(zone, Stack { item: input.item, count: input.count * 3 });
        }
        if with_mill {
            let mill = defs.building_id("tide_mill").unwrap();
            place_building(&mut w, &defs, mill, at(8, 8));
        }
        let ticks = run_until(&mut w, &defs, 6000, |w| {
            w.ground_items().any(|(_, s)| s.item == recipe.output.item)
        });
        (ticks, recipe.work)
    };

    let (unpowered_ticks, work) = run(false);
    let (powered_ticks, _) = run(true);

    assert!(powered_ticks < unpowered_ticks, "the powered run finishes sooner");
    let boost = defs_bundled_tide_mill_boost();
    let expected_gap = (work - work / boost) * TICKS_PER_SECOND as f32;
    let gap = (unpowered_ticks - powered_ticks) as f32;
    assert!((gap - expected_gap).abs() <= 2.0, "gap {gap}, expected close to {expected_gap}");
}

/// Earth's Hot Spring Mill (issue #448): at Earth's 0.8 `power_generation` its radius of 2 still
/// reaches 2 tiles (round(1.6)), so a Mine Shaft with a footprint tile 2 tiles from the mill cuts
/// stone 1.2x as fast; one whose nearest tile is 3 away runs at its plain rate.
#[test]
fn a_hot_spring_mill_powers_a_mine_shaft_within_2_tiles_but_not_3() {
    // Ticks until the first stone, with the mill's anchor `mill_x` tiles right of the shaft's
    // (the shaft covers x 2..=3, the mill x mill_x..=mill_x+1), or no mill.
    let run = |mill_x: Option<i32>| {
        let defs = Defs::bundled();
        let mut w = World::with_map(1, arena(&grid()), Nation::Earth, &[at(1, 1)], &defs);
        assert_eq!(w.modifiers().power_generation, 0.8);
        let shaft = defs.building_id("mine_shaft").unwrap();
        place_building(&mut w, &defs, shaft, at(2, 4));
        if let Some(x) = mill_x {
            let mill = defs.building_id("hot_spring_mill").unwrap();
            place_building(&mut w, &defs, mill, at(x, 4));
        }
        let stone = defs.building(shaft).recipe.as_ref().unwrap().output.item;
        let ticks =
            run_until(&mut w, &defs, 20_000, |w| w.ground_items().any(|(_, s)| s.item == stone));
        let rate = w.modifiers().get("earthworks").unwrap_or(1.0);
        (ticks, rate)
    };

    // The shaft's nearest tile is x 3, so a mill anchored at x 5 is 2 tiles away and one at x 6
    // is 3.
    let (plain, rate) = run(None);
    let (two_away, _) = run(Some(5));
    let (three_away, _) = run(Some(6));
    assert_eq!(three_away, plain, "3 tiles away: out of reach");
    let work = defs_bundled_recipe_work("mine_shaft") / rate;
    let expected_gap = (work - work / 1.2) * TICKS_PER_SECOND as f32;
    let gap = (plain - two_away) as f32;
    assert!((gap - expected_gap).abs() <= 2.0, "gap {gap}, expected close to {expected_gap}");
}

fn defs_bundled_recipe_work(id: &str) -> f32 {
    let defs = Defs::bundled();
    defs.building(defs.building_id(id).unwrap()).recipe.as_ref().unwrap().work
}

/// The Tide Mill's `boost`, read from data instead of hard-coded, so this test keeps working if
/// #432's balance pass changes it.
fn defs_bundled_tide_mill_boost() -> f32 {
    let defs = Defs::bundled();
    let mill = defs.building_id("tide_mill").unwrap();
    defs.building(mill).power.unwrap().boost
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
