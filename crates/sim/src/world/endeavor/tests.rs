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
    w.building_index.insert((pos.y, pos.x), pos);
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
    w.building_index.insert((3, 3), at(3, 3));
    assert_eq!(progress(&w, &defs, "beds"), (1, 2, false));
    assert_eq!(progress(&w, &defs, "building_kinds").0, 1);
}

#[test]
fn every_building_counts_only_kinds_distinct_and_finished() {
    let defs = Defs::bundled();
    let mut w = world(Nation::Earth, 1, &defs);
    // Earth reaches 9 kinds today (4 shared, earth_wall, rail, the shared Workshop, the Mine
    // Shaft and the Deep Mine); spare = 1 makes the target 8 (`docs/design/materials.md` section
    // 6, #272, #354; the Workshop, #369; the mines, #373).
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
fn every_building_ignores_kinds_this_nation_cant_reach() {
    let defs = Defs::bundled();
    let mut w = world(Nation::Earth, 1, &defs);
    put(&mut w, &defs, "bed", at(3, 3));
    assert_eq!(progress(&w, &defs, "building_kinds").0, 1);
    // The orchard is Air's own kind; even standing here (never possible through normal play) it
    // doesn't count toward Earth's target.
    put(&mut w, &defs, "orchard", at(5, 3));
    assert_eq!(progress(&w, &defs, "building_kinds").0, 1);
}

#[test]
fn every_building_targets_are_computed_per_nation() {
    let defs = Defs::bundled();
    let targets: Vec<(Nation, u32)> = Nation::ALL
        .iter()
        .map(|&n| (n, progress(&world(n, 1, &defs), &defs, "building_kinds").1))
        .collect();
    // Today's numbers (`docs/design/materials.md` section 6, raised by the shared Workshop
    // (#369), the Mine Shaft and Deep Mine for Earth (#373), and the Healing Hut and Fishing
    // Dock for Water (#375, #374)): Air 9, Water 9, Earth 9 and Fire 10 reachable, minus the
    // bundled sign's `spare = 1`.
    assert_eq!(
        targets,
        [(Nation::Air, 8), (Nation::Water, 8), (Nation::Earth, 8), (Nation::Fire, 9)]
    );
}

/// The bundled data, plus a new building unlocked by a new tech on `nation`'s own branch, with
/// `signs` as the endeavor's whole list: proves the `every_building` target tracks content added
/// to one nation's branch without moving anyone else's (#354, and the designer work of #357).
fn defs_with_extra_building(nation: &str, signs: &str) -> Defs {
    const BIOMES: &str = include_str!("../../../../../data/biomes.toml");
    const RESOURCES: &str = include_str!("../../../../../data/resources.toml");
    const NATIONS: &str = include_str!("../../../../../data/nations.toml");
    const RESEARCH: &str = include_str!("../../../../../data/research.toml");
    const NEEDS: &str = include_str!("../../../../../data/needs.toml");
    const JOBS: &str = include_str!("../../../../../data/jobs.toml");
    const BENDING: &str = include_str!("../../../../../data/bending.toml");
    const BUILDINGS: &str = include_str!("../../../../../data/buildings.toml");
    let research = format!(
        "{RESEARCH}\n[[tech]]\nid = \"test_tech\"\nname = \"Test Tech\"\ndescription = \"\"\n\
         nation = \"{nation}\"\ncost = 10\n[tech.unlocks]\nbuildings = [\"test_building\"]\n"
    );
    let buildings = format!(
        "{BUILDINGS}\n[[building]]\nid = \"test_building\"\nname = \"Test Building\"\n\
         verb = \"Build\"\nwork = 10.0\npassable = true\n"
    );
    Defs::from_toml(BIOMES, RESOURCES, NATIONS, &research, NEEDS, JOBS, BENDING, &buildings)
        .and_then(|d| d.with_endeavor(&format!("[endeavor]\nname = \"Gate\"\n{signs}")))
        .expect("test data is valid")
}

/// The bundled `every_building` sign (the one real sign the test data above needs).
const EVERY_BUILDING_SIGN: &str = "[[sign]]\nid = \"building_kinds\"\ngroup = \"buildings\"\n\
    name = \"One of every building\"\nkind = \"every_building\"\nspare = 1\n";

#[test]
fn adding_a_building_to_one_nations_branch_raises_only_that_nations_target() {
    let defs = Defs::bundled();
    let before: Vec<(Nation, u32)> = Nation::ALL
        .iter()
        .map(|&n| (n, progress(&world(n, 1, &defs), &defs, "building_kinds").1))
        .collect();

    let defs2 = defs_with_extra_building("earth", EVERY_BUILDING_SIGN);
    let after: Vec<(Nation, u32)> = Nation::ALL
        .iter()
        .map(|&n| (n, progress(&world(n, 1, &defs2), &defs2, "building_kinds").1))
        .collect();

    for ((nation, before), (_, after)) in before.iter().zip(after.iter()) {
        let want = if *nation == Nation::Earth { before + 1 } else { *before };
        assert_eq!(*after, want, "{nation:?}");
    }
}

#[test]
fn a_save_with_the_sign_already_met_keeps_it_met() {
    let defs = Defs::bundled();
    let mut w = world(Nation::Fire, 1, &defs);
    // Fire's target is 9: raise 9 of its 10 reachable kinds, skipping the lightning station.
    let kinds = [
        "bed",
        "campfire",
        "study_hall",
        "farm_plot",
        "forge",
        "brick_kiln",
        "smelter",
        "steam_engine",
        "workshop",
    ];
    for (i, building) in kinds.iter().enumerate() {
        put(&mut w, &defs, building, at(i as i32, 1));
    }
    run(&mut w, &defs, 10);
    assert_eq!(progress(&w, &defs, "building_kinds"), (9, 9, true));
    assert!(
        w.endeavor_events()
            .iter()
            .any(|e| e.kind == EndeavorEventKind::SignMet("building_kinds".into()))
    );

    let saved = w.save(&defs).expect("world saves");
    let w2 = World::load(&saved, &defs).expect("world loads");
    assert_eq!(progress(&w2, &defs, "building_kinds"), (9, 9, true));
    assert_eq!(w2.endeavor_events(), w.endeavor_events());
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
fn the_stone_sign_counts_brick_too() {
    // Brick stands in for stone one for one (issue #371), and `data/endeavor.toml`'s stone sign
    // lists both, so brick gets the settlement there just as stone does.
    let defs = Defs::bundled();
    let mut w = world(Nation::Fire, 1, &defs);
    store(&mut w, &defs, "stone", at(1, 5), 150);
    assert_eq!(progress(&w, &defs, "stone"), (150, 200, false));
    store(&mut w, &defs, "brick", at(2, 5), 50);
    assert_eq!(progress(&w, &defs, "stone"), (200, 200, true));
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

/// Air's own mastery threshold (the bundled `bends_per_nation`, 16; the flat `bends` is 10).
#[test]
fn masters_are_units_with_enough_finished_bends() {
    let defs = Defs::bundled();
    let mut w = world(Nation::Air, 4, &defs);
    for (u, bends) in w.units.iter_mut().zip([16, 40, 15, 0]) {
        u.bends = bends;
    }
    assert_eq!(progress(&w, &defs, "masters"), (2, 3, false));
    w.units[2].bends = 16;
    assert_eq!(progress(&w, &defs, "masters"), (3, 3, true));
}

/// A United City newcomer (#240) is judged against their own nation's threshold, not the
/// settlement's: an Air settler in a Fire settlement masters at 16 bends, not 10.
#[test]
fn a_newcomers_own_nation_sets_their_mastery_threshold() {
    let defs = Defs::bundled();
    let mut w = world(Nation::Fire, 1, &defs);
    w.units[0].nation = Some(Nation::Air);
    w.units[0].bends = 10;
    assert_eq!(progress(&w, &defs, "masters"), (0, 3, false), "10 masters Fire, not Air");
    w.units[0].bends = 16;
    assert_eq!(progress(&w, &defs, "masters"), (1, 3, false));
}

/// `World::note_mastery` fires `EndeavorEventKind::Mastered` the first tick a unit's bends reach
/// its own nation's threshold, never again for that unit, and a save/load in between doesn't
/// refire it either (#347).
#[test]
fn note_mastery_fires_once_per_unit_and_a_save_load_does_not_refire_it() {
    let defs = Defs::bundled();
    let mut w = world(Nation::Fire, 2, &defs);
    w.units[0].bends = 9;
    w.note_mastery(0, &defs);
    assert!(w.endeavor_events().is_empty(), "not at the threshold yet");

    w.units[0].bends = 10;
    w.note_mastery(0, &defs);
    let id = w.units[0].id;
    let name = w.units[0].name.clone();
    assert_eq!(
        w.endeavor_events(),
        [EndeavorEvent {
            tick: w.tick,
            kind: EndeavorEventKind::Mastered { unit: id, name: name.clone() },
        }]
    );

    // Calling again (more bends, still master) doesn't refire it.
    w.units[0].bends += 1;
    w.note_mastery(0, &defs);
    assert_eq!(w.endeavor_events().len(), 1);

    // Neither does a save/load round trip.
    let saved = w.save(&defs).expect("world saves");
    let mut w2 = World::load(&saved, &defs).expect("world loads");
    w2.units[0].bends += 1;
    w2.note_mastery(0, &defs);
    assert_eq!(w2.endeavor_events().len(), 1, "already credited before the save");

    // The second unit still gets its own event once it reaches the threshold.
    w2.units[1].bends = 10;
    w2.note_mastery(1, &defs);
    assert_eq!(w2.endeavor_events().len(), 2);
}

/// Without a masters sign in the data, `note_mastery` never fires (`EndeavorDef::master_threshold`
/// is `None`).
#[test]
fn note_mastery_does_nothing_without_a_masters_sign() {
    let defs = Defs::bundled().with_endeavor("[endeavor]\nname = \"Gate\"\n").unwrap();
    let mut w = world(Nation::Fire, 1, &defs);
    w.units[0].bends = 1000;
    w.note_mastery(0, &defs);
    assert!(w.endeavor_events().is_empty());
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
