use super::*;
use crate::bending::{AbilityId, Rank};
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
    w.put_building(crate::geom::Footprint::single(pos), id, defs);
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

/// Gives unit `i` `bends` finished bends, spread over the first `forms` of its own nation's
/// abilities (`nation_of`), so a mastery test can set bends and forms at once without going
/// through a real bend. `forms` must be 0, or at most that nation's own ability count.
fn give_bends(w: &mut World, defs: &Defs, i: usize, bends: u32, forms: u32) {
    w.units[i].bends = bends;
    w.units[i].bends_by.clear();
    if forms == 0 {
        return;
    }
    let nation = w.nation_of(&w.units[i]);
    let ids: Vec<AbilityId> = defs
        .abilities
        .iter()
        .enumerate()
        .filter(|(_, a)| a.nation == nation)
        .take(forms as usize)
        .map(|(idx, _)| AbilityId(idx as u16))
        .collect();
    assert_eq!(ids.len(), forms as usize, "{nation:?} needs at least {forms} abilities");
    let per = (bends / forms).max(1);
    for (n, &id) in ids.iter().enumerate() {
        let share = if n + 1 == ids.len() { bends - per * (forms - 1) } else { per };
        w.units[i].bends_by.insert(id, share);
    }
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
        "[[sign]]\nid = \"p\"\ngroup = \"people\"\nname = \"P\"\nshort = \"p\"\nkind = \"population\"\ntarget = 3\n",
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
    // A 2x1 bed (#207): both tiles point at it, but it is one bed.
    w.put_building(crate::geom::Footprint::new(at(3, 3), 2, 1), bed, &defs);
    assert_eq!(progress(&w, &defs, "beds"), (1, 2, false));
    assert_eq!(progress(&w, &defs, "building_kinds").0, 1);
}

#[test]
fn every_building_counts_only_kinds_distinct_and_finished() {
    let defs = Defs::bundled();
    let mut w = world(Nation::Earth, 1, &defs);
    // Earth reaches 12 kinds today (4 shared, the Herb Garden, the Map Room, earth_wall, rail,
    // the shared Workshop, the Mine Shaft, the Deep Mine and the Hot Spring Mill); spare = 1
    // makes the target 11 (`docs/design/materials.md` section 6, #272, #354; the Workshop, #369;
    // the mines, #373; the Map Room, #402; the Herb Garden, #390; the mill, #448).
    assert_eq!(progress(&w, &defs, "building_kinds"), (0, 11, false));
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
    // (#369), Map Room (#402) and Herb Garden (#390), the Mine Shaft and Deep Mine for Earth
    // (#373), the Healing Hut and Fishing Dock for Water (#375, #374), and the Tide Mill for
    // Water (#200), and the Hot Spring Mill for Earth (#448)): Air 11, Water 12, Earth 12 and Fire
    // 12 reachable, minus the bundled sign's `spare = 1`.
    assert_eq!(
        targets,
        [(Nation::Air, 10), (Nation::Water, 11), (Nation::Earth, 11), (Nation::Fire, 11)]
    );
}

/// The bundled data, plus a new building unlocked by a new core tech on `nation`'s own branch, with
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
         nation = \"{nation}\"\ncost = 10\ncore = true\n[tech.unlocks]\nbuildings = [\"test_building\"]\n"
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
    name = \"One of every building\"\nshort = \"kinds\"\nkind = \"every_building\"\nspare = 1\n";

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

/// The bundled files, with a non-core tech on every nation's branch, each unlocking a new
/// building: what each phase-1 tech of docs/design/horizons-phase-1.md adds.
fn defs_with_optional_techs() -> Defs {
    let mut files = crate::defs::BUNDLED_FILES;
    let mut research = files[3].to_string();
    let mut buildings = files[7].to_string();
    for n in Nation::ALL {
        let n = n.as_str();
        research.push_str(&format!(
            "\n[[tech]]\nid = \"opt_{n}\"\nname = \"Opt\"\ndescription = \"\"\nnation = \"{n}\"\n\
             cost = 10\n[tech.unlocks]\nbuildings = [\"opt_{n}_building\"]\n"
        ));
        buildings.push_str(&format!(
            "\n[[building]]\nid = \"opt_{n}_building\"\nname = \"Opt\"\nverb = \"Build\"\n\
             work = 10.0\npassable = true\n"
        ));
    }
    files[3] = &research;
    files[7] = &buildings;
    Defs::from_files(files).expect("test data is valid")
}

#[test]
fn optional_techs_and_their_buildings_leave_the_gate_alone() {
    let defs = Defs::bundled();
    let defs2 = defs_with_optional_techs();
    assert_eq!(defs2.techs.len(), defs.techs.len() + 4);
    for nation in Nation::ALL {
        let targets = |d: &Defs| {
            let w = world(nation, 1, d);
            (progress(&w, d, "branch").1, progress(&w, d, "building_kinds").1)
        };
        assert_eq!(targets(&defs2), targets(&defs), "{nation:?}");
        // Knowing the optional tech doesn't move the branch sign either.
        let mut w = world(nation, 1, &defs2);
        let before = progress(&w, &defs2, "branch").0;
        w.research.grant(defs2.tech_id(&format!("opt_{}", nation.as_str())).unwrap());
        assert_eq!(progress(&w, &defs2, "branch").0, before, "{nation:?}");
    }
}

#[test]
fn a_save_with_the_sign_already_met_keeps_it_met() {
    let defs = Defs::bundled();
    let mut w = world(Nation::Fire, 1, &defs);
    // Fire's target is 11: raise 11 of its 12 reachable kinds, skipping the lightning station.
    let kinds = [
        "bed",
        "campfire",
        "study_hall",
        "farm_plot",
        "herb_garden",
        "forge",
        "brick_kiln",
        "smelter",
        "steam_engine",
        "workshop",
        "map_room",
    ];
    for (i, building) in kinds.iter().enumerate() {
        put(&mut w, &defs, building, at(i as i32, 1));
    }
    run(&mut w, &defs, 10);
    assert_eq!(progress(&w, &defs, "building_kinds"), (11, 11, true));
    assert!(
        w.endeavor_events().any(|e| e.kind == EndeavorEventKind::SignMet("building_kinds".into()))
    );

    let saved = w.save(&defs).expect("world saves");
    let w2 = World::load(&saved, &defs).expect("world loads");
    assert_eq!(progress(&w2, &defs, "building_kinds"), (11, 11, true));
    assert!(w2.endeavor_events().eq(w.endeavor_events()));
}

#[test]
fn the_branch_counts_only_the_settlements_own_nation() {
    let defs = Defs::bundled();
    let mut w = world(Nation::Water, 1, &defs);
    let branch: Vec<TechId> = defs
        .techs
        .iter()
        .enumerate()
        .filter(|(_, t)| t.core && t.nation == Some(Nation::Water))
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
        "[[sign]]\nid = \"stone\"\ngroup = \"stores\"\nname = \"S\"\nshort = \"s\"\nkind = \"stored\"\n\
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

/// Air's own mastery threshold (the bundled `master_bends_per_nation`, 16 bends in 3 forms; the
/// flat `master_bends` is 10) and the `masters` sign's own count of units at that rank (#360).
#[test]
fn masters_are_units_with_enough_finished_bends_and_forms() {
    let defs = Defs::bundled();
    let mut w = world(Nation::Air, 4, &defs);
    for (i, (bends, forms)) in [(16, 3), (40, 3), (15, 3), (0, 0)].into_iter().enumerate() {
        give_bends(&mut w, &defs, i, bends, forms);
    }
    assert_eq!(progress(&w, &defs, "masters"), (2, 3, false));
    give_bends(&mut w, &defs, 2, 16, 3);
    assert_eq!(progress(&w, &defs, "masters"), (3, 3, true));
}

/// A unit with enough bends but too few forms is never a master (#360): one ability spammed
/// over and over doesn't count, however many times.
#[test]
fn enough_bends_in_too_few_forms_is_not_a_master() {
    let defs = Defs::bundled();
    let mut w = world(Nation::Fire, 1, &defs);
    give_bends(&mut w, &defs, 0, 100, 1);
    assert_eq!(progress(&w, &defs, "masters"), (0, 3, false));
}

/// A United City newcomer (#240) is judged against their own nation's threshold, not the
/// settlement's: an Air settler in a Fire settlement masters at 16 bends in 3 forms, not 10.
#[test]
fn a_newcomers_own_nation_sets_their_mastery_threshold() {
    let defs = Defs::bundled();
    let mut w = world(Nation::Fire, 1, &defs);
    w.units[0].nation = Some(Nation::Air);
    give_bends(&mut w, &defs, 0, 10, 3);
    assert_eq!(progress(&w, &defs, "masters"), (0, 3, false), "10 masters Fire, not Air");
    give_bends(&mut w, &defs, 0, 16, 3);
    assert_eq!(progress(&w, &defs, "masters"), (1, 3, false));
}

/// `World::note_rank_up` fires `EndeavorEventKind::RankUp` the first tick a unit's bends and
/// forms reach its own nation's adept, then master, thresholds, never again for a rank already
/// reached, and a save/load in between doesn't refire either (#347, #360).
#[test]
fn note_rank_up_fires_once_per_rank_and_a_save_load_does_not_refire_it() {
    let defs = Defs::bundled();
    let mut w = world(Nation::Fire, 2, &defs);
    let id = w.units[0].id;
    let name = w.units[0].name.clone();

    give_bends(&mut w, &defs, 0, 4, 2);
    w.note_rank_up(0, &defs);
    assert!(w.endeavor_events().next().is_none(), "below the adept threshold (5 bends, 2 forms)");

    give_bends(&mut w, &defs, 0, 5, 2);
    w.note_rank_up(0, &defs);
    assert_eq!(
        w.endeavor_events().collect::<Vec<_>>(),
        [&EndeavorEvent {
            tick: w.tick,
            kind: EndeavorEventKind::RankUp {
                unit: id,
                name: name.clone(),
                rank: Rank::Adept,
                title: "Firebender".to_owned(),
            },
        }]
    );

    // More bends, still adept: no refire.
    w.units[0].bends += 1;
    w.note_rank_up(0, &defs);
    assert_eq!(w.endeavor_events().count(), 1);

    // Reaching master (10 bends, 3 forms) fires a second event.
    give_bends(&mut w, &defs, 0, 10, 3);
    w.note_rank_up(0, &defs);
    assert_eq!(
        w.endeavor_events().nth(1).unwrap(),
        &EndeavorEvent {
            tick: w.tick,
            kind: EndeavorEventKind::RankUp {
                unit: id,
                name: name.clone(),
                rank: Rank::Master,
                title: "Firebending Master".to_owned(),
            },
        }
    );

    // Calling again (more bends, still master) doesn't refire it.
    w.units[0].bends += 1;
    w.note_rank_up(0, &defs);
    assert_eq!(w.endeavor_events().count(), 2);

    // Neither does a save/load round trip.
    let saved = w.save(&defs).expect("world saves");
    let mut w2 = World::load(&saved, &defs).expect("world loads");
    w2.units[0].bends += 1;
    w2.note_rank_up(0, &defs);
    assert_eq!(w2.endeavor_events().count(), 2, "already credited before the save");

    // The second unit, jumping straight to master, still gets its own event.
    let id2 = w2.units[1].id;
    let name2 = w2.units[1].name.clone();
    give_bends(&mut w2, &defs, 1, 10, 3);
    w2.note_rank_up(1, &defs);
    assert_eq!(
        w2.endeavor_events().nth(2).unwrap(),
        &EndeavorEvent {
            tick: w2.tick,
            kind: EndeavorEventKind::RankUp {
                unit: id2,
                name: name2,
                rank: Rank::Master,
                title: "Firebending Master".to_owned(),
            },
        }
    );
}

/// `[mastery]` is foundational data, not tied to any one endeavor: `note_rank_up` fires even
/// without a `masters` sign in the data at all (unlike `EndeavorDef::master_threshold`, which
/// still reads `None` then, #360).
#[test]
fn note_rank_up_fires_even_without_a_masters_sign() {
    let defs = Defs::bundled().with_endeavor("[endeavor]\nname = \"Gate\"\n").unwrap();
    let mut w = world(Nation::Fire, 1, &defs);
    assert_eq!(defs.endeavor.master_threshold(Nation::Fire), None);
    give_bends(&mut w, &defs, 0, 10, 3);
    w.note_rank_up(0, &defs);
    assert_eq!(w.endeavor_events().count(), 1);
}

const STONE_SIGNS: &str = "
[[sign]]
id = \"people\"
group = \"people\"
name = \"People\"
short = \"people\"
kind = \"population\"
target = 1

[[sign]]
id = \"stone\"
group = \"stores\"
name = \"Stone\"
short = \"stone\"
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
        w.endeavor_events().collect::<Vec<_>>(),
        [&EndeavorEvent { tick: 10, kind: EndeavorEventKind::SignMet("people".into()) }]
    );

    store(&mut w, &defs, "stone", at(1, 5), 50);
    run(&mut w, &defs, 10);
    assert_eq!(*w.endeavor(), EndeavorState::Ready { tick: 40 });
    let tail: Vec<_> = w.endeavor_events().skip(1).map(|e| (e.tick, e.kind.clone())).collect();
    assert_eq!(
        tail,
        [(40, EndeavorEventKind::SignMet("stone".into())), (40, EndeavorEventKind::Ready)]
    );

    // The stone is used up: the sign drops, the latch holds, and nothing more is logged.
    w.stored.clear();
    run(&mut w, &defs, 100);
    assert!(!sign(&w, &defs, "stone").met);
    assert_eq!(*w.endeavor(), EndeavorState::Ready { tick: 40 });
    assert_eq!(w.endeavor_events().count(), 3);
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
    let met: Vec<_> = w.endeavor_events().map(|e| e.kind.clone()).collect();
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
    assert!(w.endeavor_events().next().is_none());
    assert!(w.endeavor_signs(&defs).is_empty());
}

#[test]
fn the_endeavor_is_deterministic() {
    // A real game on a generated map: settlers chop wood, and two signs track it.
    let defs = defs_with(
        "[[sign]]\nid = \"people\"\ngroup = \"people\"\nname = \"P\"\nshort = \"p\"\nkind = \"population\"\ntarget = 4\n\
         [[sign]]\nid = \"wood\"\ngroup = \"harvests\"\nname = \"W\"\nshort = \"w\"\nkind = \"tally\"\n\
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
        (w.endeavor().clone(), w.endeavor_events().cloned().collect::<Vec<_>>())
    };
    let (a, b) = (play(), play());
    assert_eq!(a, b);
    assert!(matches!(a.0, EndeavorState::Ready { .. }), "{a:?}");
}

/// The bundled chapter (#439, docs/design/pacing.md section 3).
fn road(defs: &Defs) -> &ChapterDef {
    defs.endeavor.chapters.iter().find(|c| c.id == "known_on_the_road").expect("bundled chapter")
}

/// Raises everything "Known on the road" asks for: 10 people, four kinds, 60 food.
fn meet_the_road(defs: &Defs) -> World {
    let mut w = world(Nation::Earth, 10, defs);
    for (b, x) in [("bed", 1), ("campfire", 3), ("herb_garden", 5), ("farm_plot", 7)] {
        put(&mut w, defs, b, at(x, 4));
    }
    store(&mut w, defs, "fish", at(10, 6), 60);
    w
}

#[test]
fn the_chapter_signs_stay_out_of_the_gates_list() {
    let defs = Defs::bundled();
    let chapter = road(&defs);
    let ids: Vec<&str> = chapter.signs.iter().map(|s| s.id.as_str()).collect();
    assert_eq!(ids, ["chapter_population", "chapter_kinds", "chapter_food"]);
    assert!(defs.endeavor.signs.iter().all(|s| !s.id.starts_with("chapter_")));
    let targets: Vec<Option<u32>> =
        chapter.signs.iter().map(|s| s.target_for(Nation::Earth)).collect();
    assert_eq!(targets, [Some(10), Some(4), Some(60)]);
}

#[test]
fn the_chapter_latches_once_all_its_signs_are_met_and_logs_one_event() {
    let defs = Defs::bundled();
    let mut w = meet_the_road(&defs);
    let chapter = road(&defs);
    assert!(w.chapter_signs(chapter, &defs).iter().all(|p| p.met));
    assert_eq!(w.chapter_met("known_on_the_road"), None, "not until the once-a-second check");
    assert_eq!(w.current_chapter(&defs).map(|c| c.id.as_str()), Some("known_on_the_road"));
    for _ in 0..TICKS_PER_SECOND {
        w.tick(&defs);
    }
    let met = w.chapter_met("known_on_the_road").expect("met");
    assert_eq!(met, u64::from(TICKS_PER_SECOND));
    assert!(w.current_chapter(&defs).is_none());

    // Eating the food doesn't undo it, and it never fires again.
    w.stored.clear();
    for _ in 0..3 * TICKS_PER_SECOND {
        w.tick(&defs);
    }
    assert_eq!(w.chapter_met("known_on_the_road"), Some(met));
    let chapter_events: Vec<_> = w
        .endeavor_events()
        .filter(|e| matches!(e.kind, EndeavorEventKind::ChapterMet(_)))
        .collect();
    assert_eq!(chapter_events.len(), 1);
    assert!(
        !w.endeavor_events().any(|e| matches!(&e.kind, EndeavorEventKind::SignMet(id)
            if id.starts_with("chapter_"))),
        "a chapter's signs log no SignMet of their own"
    );
    // The gate isn't Ready on the chapter's strength, and its count is unchanged.
    assert_eq!(*w.endeavor(), EndeavorState::Dormant);
}

#[test]
fn a_chapter_waits_for_all_its_signs_at_once() {
    let defs = Defs::bundled();
    let mut w = meet_the_road(&defs);
    w.stored.clear();
    store(&mut w, &defs, "fish", at(10, 6), 59);
    for _ in 0..2 * TICKS_PER_SECOND {
        w.tick(&defs);
    }
    assert_eq!(w.chapter_met("known_on_the_road"), None, "59 food is one short");
    let p = w.chapter_signs(road(&defs), &defs);
    assert_eq!(p.iter().filter(|p| p.met).count(), 2);
}

/// #707: `Command::SetResearch` on a tech that `requires` a chapter or scrolls is refused until
/// the world's facts (`World::facts`) meet it.
#[test]
fn research_conditions_read_the_chapters_met_and_the_scrolls_read() {
    let mut files = crate::defs::BUNDLED_FILES;
    let research = format!(
        "{}\n[[tech]]\nid = \"gated\"\nname = \"Gated\"\ndescription = \"\"\ncost = 10\n\
         requires = {{ scrolls_read = 2, chapter = \"known_on_the_road\" }}\n",
        files[3]
    );
    files[3] = &research;
    let defs = Defs::from_files(files).expect("a chapter that exists loads");
    let gated = defs.tech_id("gated").unwrap();
    let set = |w: &mut World| w.apply(Command::SetResearch { tech: gated }, &defs);
    let refused = |w: &mut World| match set(w) {
        Err(CommandError::Research(e)) => e.to_string(),
        other => panic!("expected a research refusal, got {other:?}"),
    };

    let mut w = meet_the_road(&defs);
    assert_eq!(refused(&mut w), "needs 2 scrolls read");
    w.tallies.count_scrolls(2);
    assert_eq!(w.facts().scrolls_read, 2);
    assert_eq!(refused(&mut w), "needs Known on the road", "the chapter latches on the next check");
    for _ in 0..TICKS_PER_SECOND {
        w.tick(&defs);
    }
    assert_eq!(w.facts().chapters_met, ["known_on_the_road"]);
    set(&mut w).expect("both conditions are met");
    assert_eq!(w.research().queue().last(), Some(&gated));
}

#[test]
fn the_chapter_survives_a_save() {
    let defs = Defs::bundled();
    let mut w = meet_the_road(&defs);
    for _ in 0..TICKS_PER_SECOND {
        w.tick(&defs);
    }
    let loaded = World::load(&w.save(&defs).unwrap(), &defs).unwrap();
    assert_eq!(loaded.chapter_met("known_on_the_road"), w.chapter_met("known_on_the_road"));
}

// --- The United City's population milestones (#442) ---

/// Grows `w` to `n` settlers by cloning its first unit under fresh ids.
fn grow_to(w: &mut World, n: usize) {
    while w.settlers() < n {
        let mut unit = w.units[0].clone();
        unit.id = UnitId(w.next_unit);
        w.next_unit += 1;
        w.units.push(unit);
    }
}

fn milestones_fired(w: &World) -> Vec<u32> {
    w.endeavor_events()
        .filter_map(|e| match e.kind {
            EndeavorEventKind::PopulationMilestone(n) => Some(n),
            _ => None,
        })
        .collect()
}

#[test]
fn the_bundled_milestones_are_24_32_and_every_8_after() {
    let m = Defs::bundled().endeavor.milestones;
    assert_eq!(m.list.iter().map(|m| m.population).collect::<Vec<_>>(), [24, 32]);
    assert_eq!(m.repeat.as_ref().map(|r| r.every), Some(8));
    let next: Vec<Option<u32>> = [0, 23, 24, 31, 32, 39, 40, 100].map(|p| m.next_after(p)).to_vec();
    assert_eq!(
        next,
        [Some(24), Some(24), Some(32), Some(32), Some(40), Some(40), Some(48), Some(104)]
    );
    assert_eq!(m.toast(24).as_deref(), Some("The United City passes 24 souls."));
    assert_eq!(m.toast(48).as_deref(), Some("The United City passes 48 souls."));
    assert_eq!(m.toast(30), None, "not a milestone");
    assert_eq!([24, 32, 40, 48].map(|n| m.previous(n)), [0, 24, 32, 40]);
}

#[test]
fn the_repeat_is_optional_and_the_milestones_are_checked() {
    let load = |tail: &str| {
        let src = include_str!("../../../../../data/endeavor.toml");
        let cut = src.find("# The United City's population milestones").unwrap();
        let end = src.find("[[sign]]\nid = \"chapter_population\"").unwrap();
        let src = format!("{}{tail}\n{}", &src[..cut], &src[end..]);
        Defs::bundled().with_endeavor(&src).map(|d| d.endeavor.milestones)
    };
    let m = load("[[milestone]]\npopulation = 24\ntoast = \"A\"\n").unwrap();
    assert_eq!((m.next_after(0), m.next_after(24)), (Some(24), None), "no repeat: 24 only");
    let rel = "[milestone_repeat]\nevery = 5\ntoast = \"B {population}\"\n";
    let m = load(&format!("[[milestone]]\npopulation = 24\ntoast = \"A\"\n{rel}")).unwrap();
    assert_eq!((m.next_after(24), m.toast(29).as_deref()), (Some(29), Some("B 29")));
    let none = load("").unwrap();
    assert_eq!(none.next_after(0), None);
    let e = |t: &str| load(t).expect_err("rejected").0;
    assert!(e(rel).contains("at least one [[milestone]]"));
    assert!(
        e("[[milestone]]\npopulation = 24\ntoast = \"A\"\n[[milestone]]\npopulation = 24\ntoast = \"B\"\n")
            .contains("increase")
    );
    assert!(e("[[milestone]]\npopulation = 0\ntoast = \"A\"\n").contains("population"));
    assert!(e("[[milestone]]\npopulation = 3\ntoast = \" \"\n").contains("toast"));
    let zero = "[[milestone]]\npopulation = 3\ntoast = \"A\"\n[milestone_repeat]\nevery = 0\ntoast = \"B\"\n";
    assert!(e(zero).contains("every"));
}

#[test]
fn a_milestone_fires_once_in_order_and_only_in_the_united_city() {
    let defs = Defs::bundled();
    let mut w = world(Nation::Earth, 3, &defs);
    grow_to(&mut w, 26);
    w.tick_milestones(&defs);
    assert!(milestones_fired(&w).is_empty(), "the gate doesn't stand yet");

    w.endeavor = EndeavorState::Complete { tick: 0 };
    assert_eq!(w.next_milestone(&defs), Some(24));
    w.tick_milestones(&defs);
    w.tick_milestones(&defs);
    assert_eq!(milestones_fired(&w), [24], "once, though 26 stays above it");
    assert_eq!(w.next_milestone(&defs), Some(32));

    // Dropping below and coming back doesn't fire it again.
    w.units.truncate(10);
    w.tick_milestones(&defs);
    grow_to(&mut w, 25);
    w.tick_milestones(&defs);
    assert_eq!(milestones_fired(&w), [24]);

    // A jump past two at once fires both, in order; the repeat then carries on without end.
    grow_to(&mut w, 41);
    w.tick_milestones(&defs);
    assert_eq!(milestones_fired(&w), [24, 32, 40]);
    assert_eq!(w.next_milestone(&defs), Some(48));
    grow_to(&mut w, 48);
    w.tick_milestones(&defs);
    assert_eq!(milestones_fired(&w), [24, 32, 40, 48]);
}

#[test]
fn a_fired_milestone_survives_a_save_and_the_event_is_the_toast() {
    let defs = Defs::bundled();
    let mut w = world(Nation::Earth, 3, &defs);
    grow_to(&mut w, 25);
    w.endeavor = EndeavorState::Complete { tick: 0 };
    w.tick_milestones(&defs);
    let loaded = World::load(&w.save(&defs).unwrap(), &defs).unwrap();
    assert_eq!(loaded.next_milestone(&defs), Some(32), "24 doesn't fire again");
    assert_eq!(milestones_fired(&loaded), [24]);
}

#[test]
fn the_avatar_does_not_count_toward_a_milestone() {
    let defs = Defs::bundled();
    let mut w = world(Nation::Earth, 3, &defs);
    grow_to(&mut w, 23);
    w.endeavor = EndeavorState::Complete { tick: 0 };
    let mut guest = w.units[0].clone();
    guest.guest = Some(Guest::Avatar);
    w.units.push(guest);
    assert_eq!((w.population(), w.settlers()), (24, 23));
    w.tick_milestones(&defs);
    assert!(milestones_fired(&w).is_empty());
}
