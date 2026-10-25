use super::*;
use crate::building::Building;
use crate::defs::{BiomeId, Terrain};
use crate::geom::Grid;
use crate::mapgen::Tile;

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

/// The bundled data with `signs` as the endeavor's whole list.
fn defs_with(signs: &str) -> Defs {
    Defs::bundled()
        .with_endeavor(&format!("[endeavor]\nname = \"Gate\"\n{signs}"))
        .expect("test signs are valid")
}

/// A bare 12x8 field with `units` units of `nation` along the top row.
fn world(nation: Nation, units: i32, defs: &Defs) -> World {
    let spawns: Vec<Pos> = (0..units).map(|x| at(x, 0)).collect();
    World::with_map(1, field(12, 8), nation, &spawns, defs)
}

/// Puts a finished building straight onto `pos`, skipping construction (`build.rs` covers that).
fn put(w: &mut World, defs: &Defs, building: &str, pos: Pos) {
    let id = defs.building_id(building).expect("building exists");
    w.map[pos].building = Some(Building::new(id, defs.building(id).passable, false));
    w.map_changed();
}

fn store(w: &mut World, defs: &Defs, item: &str, pos: Pos, count: u32) {
    let item = defs.item_id(item).expect("item exists");
    w.stored.insert(pos, Stack { item, count });
}

/// The bundled sign `id`'s progress in `w`.
fn sign<'d>(w: &World, defs: &'d Defs, id: &str) -> SignProgress<'d> {
    w.endeavor_signs(defs).into_iter().find(|p| p.id == id).expect("bundled sign")
}

fn progress(w: &World, defs: &Defs, id: &str) -> (u32, u32, bool) {
    let p = sign(w, defs, id);
    (p.value, p.target, p.met)
}

fn run(w: &mut World, defs: &Defs, ticks: u64) {
    for _ in 0..ticks {
        w.tick(defs);
    }
}

#[test]
fn signs_are_listed_in_file_order() {
    let defs = Defs::bundled();
    let w = world(Nation::Earth, 3, &defs);
    let ids: Vec<&str> = w.endeavor_signs(&defs).iter().map(|p| p.id).collect();
    let data: Vec<&str> = defs.endeavor.signs.iter().map(|s| s.id.as_str()).collect();
    assert_eq!(ids, data);
}

#[test]
fn population_counts_living_units() {
    let defs = Defs::bundled();
    let mut w = world(Nation::Earth, 3, &defs);
    assert_eq!(progress(&w, &defs, "population"), (3, 16, false));
    w.units.truncate(1);
    assert_eq!(progress(&w, &defs, "population").0, 1);
    let defs = defs_with(
        "[[sign]]\nid = \"p\"\ngroup = \"people\"\nname = \"P\"\nkind = \"population\"\ntarget = 3\n",
    );
    let w = world(Nation::Earth, 3, &defs);
    assert_eq!(progress(&w, &defs, "p"), (3, 3, true));
}

#[test]
fn beds_for_all_needs_a_bed_per_person_and_somebody_to_sleep() {
    let defs = Defs::bundled();
    let mut w = world(Nation::Fire, 2, &defs);
    assert_eq!(progress(&w, &defs, "beds"), (0, 2, false));
    put(&mut w, &defs, "bed", at(3, 3));
    // A building that isn't a bed doesn't count.
    put(&mut w, &defs, "campfire", at(5, 3));
    assert_eq!(progress(&w, &defs, "beds"), (1, 2, false));
    put(&mut w, &defs, "bed", at(4, 3));
    assert_eq!(progress(&w, &defs, "beds"), (2, 2, true));
    put(&mut w, &defs, "bed", at(6, 3));
    assert_eq!(progress(&w, &defs, "beds"), (3, 2, true));
    // Nobody left: no one has a bed, so the sign isn't met however many stand empty.
    w.units.clear();
    assert_eq!(progress(&w, &defs, "beds"), (3, 0, false));
}

#[test]
fn a_building_over_several_tiles_counts_once() {
    let defs = Defs::bundled();
    let mut w = world(Nation::Fire, 2, &defs);
    let bed = defs.building_id("bed").unwrap();
    // A 2x1 bed (#207): both tiles carry it, but it is one bed.
    for (x, part) in [(3, [0, 0]), (4, [1, 0])] {
        let b = Building { part, size: [2, 1], ..Building::new(bed, true, false) };
        w.map[at(x, 3)].building = Some(b);
    }
    assert_eq!(progress(&w, &defs, "beds"), (1, 2, false));
    assert_eq!(progress(&w, &defs, "building_kinds").0, 1);
}

#[test]
fn building_kinds_counts_distinct_finished_buildings() {
    let defs = Defs::bundled();
    let mut w = world(Nation::Earth, 1, &defs);
    assert_eq!(progress(&w, &defs, "building_kinds"), (0, 8, false));
    put(&mut w, &defs, "bed", at(3, 3));
    put(&mut w, &defs, "bed", at(4, 3));
    put(&mut w, &defs, "campfire", at(5, 3));
    assert_eq!(progress(&w, &defs, "building_kinds").0, 2);
    // A site under construction isn't raised yet.
    w.apply(
        Command::DesignateBuild {
            pos: at(8, 5),
            building: defs.building_id("study_hall").unwrap(),
        },
        &defs,
    )
    .unwrap();
    assert_eq!(progress(&w, &defs, "building_kinds").0, 2);
}

#[test]
fn the_branch_counts_only_the_settlements_own_nation() {
    let defs = Defs::bundled();
    let mut w = world(Nation::Water, 1, &defs);
    let branch: Vec<TechId> = defs
        .techs
        .iter()
        .enumerate()
        .filter(|(_, t)| t.nation == Some(Nation::Water))
        .map(|(i, _)| TechId(i as u16))
        .collect();
    let total = branch.len() as u32;
    assert!(total > 0);
    let (known, target, met) = progress(&w, &defs, "branch");
    assert_eq!((target, met), (total, false));
    // Shared techs and other nations' techs don't move it.
    w.research.grant(defs.tech_id("shelter").unwrap());
    w.research.grant(defs.tech_id("lightning_generation").unwrap());
    assert_eq!(progress(&w, &defs, "branch").0, known);
    for &t in &branch {
        w.research.grant(t);
    }
    assert_eq!(progress(&w, &defs, "branch"), (total, total, true));
}

#[test]
fn stored_sums_the_listed_items_and_food_counts_every_edible_item() {
    let defs = Defs::bundled();
    let mut w = world(Nation::Earth, 1, &defs);
    store(&mut w, &defs, "stone", at(1, 5), 120);
    store(&mut w, &defs, "stone", at(2, 5), 90);
    store(&mut w, &defs, "wood", at(3, 5), 50);
    assert_eq!(progress(&w, &defs, "stone"), (210, 200, true));
    assert_eq!(progress(&w, &defs, "food").0, 0);
    store(&mut w, &defs, "fish", at(4, 5), 30);
    store(&mut w, &defs, "fruit", at(5, 5), 20);
    store(&mut w, &defs, "cabbage", at(6, 5), 5);
    assert_eq!(progress(&w, &defs, "food"), (55, 150, false));
}

#[test]
fn per_nation_targets_apply_to_their_nation_only() {
    let defs = defs_with(
        "[[sign]]\nid = \"stone\"\ngroup = \"stores\"\nname = \"S\"\nkind = \"stored\"\n\
         items = [\"stone\"]\ntarget = 200\nper_nation = { air = 150 }\n",
    );
    for (nation, target) in [(Nation::Air, 150), (Nation::Earth, 200)] {
        let mut w = world(nation, 1, &defs);
        store(&mut w, &defs, "stone", at(1, 5), 160);
        assert_eq!(progress(&w, &defs, "stone"), (160, target, target == 150), "{nation:?}");
    }
}

#[test]
fn tallies_feed_the_ore_and_scroll_signs() {
    let defs = Defs::bundled();
    let mut w = world(Nation::Earth, 1, &defs);
    let ore = defs.item_id("gold_ore").unwrap();
    w.tallies.count_harvest(ore, 39);
    w.tallies.count_harvest(defs.item_id("stone").unwrap(), 500);
    assert_eq!(progress(&w, &defs, "ore"), (39, 40, false));
    w.tallies.count_harvest(ore, 1);
    assert_eq!(progress(&w, &defs, "ore"), (40, 40, true));
    w.tallies.count_scrolls(2);
    assert_eq!(progress(&w, &defs, "scrolls"), (2, 3, false));
}

#[test]
fn masters_are_units_with_enough_finished_bends() {
    let defs = Defs::bundled();
    let mut w = world(Nation::Air, 4, &defs);
    for (u, bends) in w.units.iter_mut().zip([25, 40, 24, 0]) {
        u.bends = bends;
    }
    assert_eq!(progress(&w, &defs, "masters"), (2, 3, false));
    w.units[2].bends = 25;
    assert_eq!(progress(&w, &defs, "masters"), (3, 3, true));
}

const STONE_SIGNS: &str = "
[[sign]]
id = \"people\"
group = \"people\"
name = \"People\"
kind = \"population\"
target = 1

[[sign]]
id = \"stone\"
group = \"stores\"
name = \"Stone\"
kind = \"stored\"
items = [\"stone\"]
target = 50
";

#[test]
fn every_sign_met_at_once_latches_ready_and_eating_stores_does_not_undo_it() {
    let defs = defs_with(STONE_SIGNS);
    let mut w = world(Nation::Earth, 1, &defs);
    run(&mut w, &defs, 30);
    assert_eq!(*w.endeavor(), EndeavorState::Dormant);
    // The population sign was met at the first check, and only logged once.
    assert_eq!(
        w.endeavor_events(),
        [EndeavorEvent { tick: 10, kind: EndeavorEventKind::SignMet("people".into()) }]
    );

    store(&mut w, &defs, "stone", at(1, 5), 50);
    run(&mut w, &defs, 10);
    assert_eq!(*w.endeavor(), EndeavorState::Ready { tick: 40 });
    let tail: Vec<_> = w.endeavor_events()[1..].iter().map(|e| (e.tick, e.kind.clone())).collect();
    assert_eq!(
        tail,
        [(40, EndeavorEventKind::SignMet("stone".into())), (40, EndeavorEventKind::Ready)]
    );

    // The stone is used up: the sign drops, the latch holds, and nothing more is logged.
    w.stored.clear();
    run(&mut w, &defs, 100);
    assert!(!sign(&w, &defs, "stone").met);
    assert_eq!(*w.endeavor(), EndeavorState::Ready { tick: 40 });
    assert_eq!(w.endeavor_events().len(), 3);
}

#[test]
fn signs_met_one_at_a_time_are_not_ready_and_each_is_logged_once() {
    let defs = defs_with(STONE_SIGNS);
    let mut w = world(Nation::Earth, 1, &defs);
    store(&mut w, &defs, "stone", at(1, 5), 50);
    w.units.clear();
    run(&mut w, &defs, 10);
    // Stone is met but nobody lives here: not every sign at once.
    assert_eq!(*w.endeavor(), EndeavorState::Dormant);
    w.stored.clear();
    run(&mut w, &defs, 20);
    store(&mut w, &defs, "stone", at(1, 5), 60);
    run(&mut w, &defs, 20);
    let met: Vec<_> = w.endeavor_events().iter().map(|e| e.kind.clone()).collect();
    assert_eq!(met, [EndeavorEventKind::SignMet("stone".into())], "logged the first time only");
    assert_eq!(*w.endeavor(), EndeavorState::Dormant);
}

#[test]
fn checks_run_once_a_second() {
    let defs = defs_with(STONE_SIGNS);
    let mut w = world(Nation::Earth, 1, &defs);
    run(&mut w, &defs, 10);
    store(&mut w, &defs, "stone", at(1, 5), 50);
    run(&mut w, &defs, 9);
    assert_eq!(*w.endeavor(), EndeavorState::Dormant, "tick 19 is not a check");
    run(&mut w, &defs, 1);
    assert_eq!(*w.endeavor(), EndeavorState::Ready { tick: 20 });
}

#[test]
fn data_without_signs_never_becomes_ready() {
    let defs = crate::bending::testing::probe_defs(1.0, "");
    assert!(defs.endeavor.signs.is_empty());
    let mut w = world(Nation::Earth, 1, &defs);
    run(&mut w, &defs, 50);
    assert_eq!(*w.endeavor(), EndeavorState::Dormant);
    assert!(w.endeavor_events().is_empty());
    assert!(w.endeavor_signs(&defs).is_empty());
}

#[test]
fn the_endeavor_is_deterministic() {
    // A real game on a generated map: settlers chop wood, and two signs track it.
    let defs = defs_with(
        "[[sign]]\nid = \"people\"\ngroup = \"people\"\nname = \"P\"\nkind = \"population\"\ntarget = 4\n\
         [[sign]]\nid = \"wood\"\ngroup = \"harvests\"\nname = \"W\"\nkind = \"tally\"\n\
         tally = \"harvested\"\nitems = [\"wood\"]\ntarget = 8\n",
    );
    let play = || {
        let cfg = MapGenConfig { seed: 7, ..Default::default() };
        let mut w = World::new(&cfg, Nation::Earth, 4, &defs);
        let home = w.units[0].pos;
        let trees: Vec<Pos> = w
            .map
            .iter()
            .filter(|(p, t)| {
                (p.x - home.x).abs().max((p.y - home.y).abs()) <= 12
                    && t.resource.and_then(|r| defs.harvest(r)).is_some_and(|h| {
                        defs.item(h.item).id == "wood" && w.is_unlocked(h.unlocked_by)
                    })
            })
            .map(|(p, _)| p)
            .take(4)
            .collect();
        assert!(trees.len() >= 2, "seed 7 has trees near the start");
        for pos in trees {
            w.apply(Command::DesignateHarvest { pos }, &defs).unwrap();
        }
        run(&mut w, &defs, 4000);
        (w.endeavor().clone(), w.endeavor_events().to_vec())
    };
    let (a, b) = (play(), play());
    assert_eq!(a, b);
    assert!(matches!(a.0, EndeavorState::Ready { .. }), "{a:?}");
}
