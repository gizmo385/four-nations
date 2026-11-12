//! Saves survive content changes (#602, docs/design/save-compat.md): a save made before content
//! was appended, inserted in the middle or reordered loads and plays on, and one that uses
//! content which no longer exists fails naming it.
//!
//! Each case edits a copy of the `data/*.toml` text, builds `Defs` from it with
//! `Defs::from_files` (the same path `Defs::bundled` takes), and loads saves made under the
//! bundled data with it: the checked-in v1 fixture, and a v2 save of a busy world per nation.

use std::collections::BTreeSet;
use std::sync::OnceLock;

use fn_sim::autoplay::Opening;
use fn_sim::world::Command;
use fn_sim::{Defs, MapGenConfig, Nation, Pos, SaveError, World};

const FIXTURE: &str = include_str!("fixtures/v1_buildings.ron");

/// The 13 files `Defs::bundled` reads, in its order.
const FILES: [&str; 13] = [
    include_str!("../../../data/biomes.toml"),
    include_str!("../../../data/resources.toml"),
    include_str!("../../../data/nations.toml"),
    include_str!("../../../data/research.toml"),
    include_str!("../../../data/needs.toml"),
    include_str!("../../../data/jobs.toml"),
    include_str!("../../../data/bending.toml"),
    include_str!("../../../data/buildings.toml"),
    include_str!("../../../data/endeavor.toml"),
    include_str!("../../../data/embark.toml"),
    include_str!("../../../data/difficulty.toml"),
    include_str!("../../../data/seasons.toml"),
    include_str!("../../../data/harm.toml"),
];

fn bundled_text() -> [String; 13] {
    FILES.map(str::to_owned)
}

fn defs_from(files: &[String; 13]) -> Defs {
    Defs::from_files(std::array::from_fn(|i| files[i].as_str())).expect("edited data is valid")
}

/// An id table: which file holds it, its `[[table]]` name, and the entry each case copies.
struct Table {
    file: usize,
    name: &'static str,
    copy: &'static str,
}

/// The six content-id tables. Biomes, resources, buildings and abilities copy their first entry;
/// techs copy a root tech (no prereqs) so the copy may stand first; items copy wood (no
/// `stands_for`, which must be unique).
const TABLES: [Table; 6] = [
    Table { file: 0, name: "biome", copy: "tundra" },
    Table { file: 1, name: "resource", copy: "ancient_scroll" },
    Table { file: 3, name: "tech", copy: "fire_making" },
    Table { file: 5, name: "item", copy: "wood" },
    Table { file: 7, name: "building", copy: "bed" },
    Table { file: 6, name: "ability", copy: "raise_earth" },
];

/// The `[[name]]` blocks of `src`, as byte ranges. A block runs from its `[[name]]` line up to
/// the next line that opens another table, sub-tables of its own (`[name.x]`, `[[name.x]]`)
/// excepted, so a building keeps its `[[building.cost]]` and `[building.bed]` with it.
fn blocks(src: &str, name: &str) -> Vec<(usize, usize)> {
    let (head, sub, subs) = (format!("[[{name}]]"), format!("[{name}."), format!("[[{name}."));
    let mut out = Vec::new();
    let mut open: Option<usize> = None;
    let mut at = 0;
    for line in src.split_inclusive('\n') {
        let t = line.trim_start();
        if t.starts_with('[') && !t.starts_with(&sub) && !t.starts_with(&subs) {
            if let Some(start) = open.take() {
                out.push((start, at));
            }
            if t.trim_end() == head {
                open = Some(at);
            }
        }
        at += line.len();
    }
    if let Some(start) = open {
        out.push((start, src.len()));
    }
    out
}

fn block_id(block: &str) -> &str {
    block
        .lines()
        .find_map(|l| l.strip_prefix("id = \""))
        .and_then(|l| l.split('"').next())
        .expect("a block with an id")
}

/// `block` with its id replaced by `test_<name>`, ending in a newline. A tech's copy leaves out
/// its `[tech.unlocks]`: a building has exactly one tech.
fn renamed(block: &str, name: &str) -> String {
    let block = block.split("[tech.unlocks]").next().unwrap();
    let id = block_id(block);
    let mut text = block.replacen(&format!("id = \"{id}\""), &format!("id = \"test_{name}\""), 1);
    if !text.ends_with('\n') {
        text.push('\n');
    }
    text.push('\n');
    text
}

#[derive(Debug, Clone, Copy)]
enum Case {
    Append,
    Insert,
    Reorder,
}

/// The id a reorder moves to the end: the first entry, or for techs the first leaf (named in no
/// other tech's block, so in no one's `prereqs`).
fn reordered_id(src: &str, t: &Table) -> String {
    let all = blocks(src, t.name);
    let text = |&(a, b): &(usize, usize)| &src[a..b];
    if t.name != "tech" {
        return block_id(text(&all[0])).to_owned();
    }
    all.iter()
        .map(text)
        .map(block_id)
        .find(|id| {
            let quoted = format!("\"{id}\"");
            all.iter().map(text).filter(|b| b.contains(&quoted)).count() == 1
        })
        .expect("a leaf tech")
        .to_owned()
}

fn edit(case: Case) -> [String; 13] {
    let mut files = bundled_text();
    for t in &TABLES {
        let src = &files[t.file];
        let all = blocks(src, t.name);
        assert!(all.len() >= 2, "{}: too few blocks", t.name);
        let text = |&(a, b): &(usize, usize)| src[a..b].to_owned();
        let find = |id: &str| {
            *all.iter().find(|r| block_id(&src[r.0..r.1]) == id).unwrap_or_else(|| {
                panic!("no {} {id}", t.name);
            })
        };
        let (first, last_end) = (all[0].0, all.last().unwrap().1);
        let edited = match case {
            Case::Append => {
                let copy = renamed(&text(&find(t.copy)), t.name);
                format!("{}{copy}{}", &src[..last_end], &src[last_end..])
            }
            Case::Insert => {
                let copy = renamed(&text(&find(t.copy)), t.name);
                format!("{}{copy}{}", &src[..first], &src[first..])
            }
            Case::Reorder => {
                let (a, b) = find(&reordered_id(src, t));
                let mut moved = text(&(a, b));
                if !moved.ends_with('\n') {
                    moved.push('\n');
                }
                format!("{}{}\n{moved}{}", &src[..a], &src[b..last_end], &src[last_end..])
            }
        };
        files[t.file] = edited;
    }
    files
}

/// The copy for `case`, checked to really differ from the bundled data in the way it claims.
fn copy(case: Case) -> Defs {
    let (bundled, copy) = (Defs::bundled(), defs_from(&edit(case)));
    let (b, c) = (&bundled, &copy);
    let names = |d: &Defs| -> [Vec<String>; 6] {
        [
            d.biomes.iter().map(|x| x.id.clone()).collect(),
            d.resources.iter().map(|x| x.id.clone()).collect(),
            d.techs.iter().map(|x| x.id.clone()).collect(),
            d.items.iter().map(|x| x.id.clone()).collect(),
            d.buildings.iter().map(|x| x.id.clone()).collect(),
            d.abilities.iter().map(|x| x.id.clone()).collect(),
        ]
    };
    for (t, (before, after)) in TABLES.iter().zip(names(b).into_iter().zip(names(c))) {
        let test = format!("test_{}", t.name);
        match case {
            Case::Append => {
                // Last of its `[[table]]` blocks, which for items isn't the end of the table:
                // foods and harvests add items after them.
                let at = after.iter().position(|x| *x == test).expect(t.name);
                assert!(at > 0, "{}", t.name);
                let rest: Vec<_> = after.iter().filter(|x| **x != test).cloned().collect();
                assert_eq!(rest, before, "{}", t.name);
            }
            Case::Insert => {
                assert_eq!(after.len(), before.len() + 1, "{}", t.name);
                assert_eq!(after[0], test, "{}", t.name);
            }
            Case::Reorder => {
                assert_eq!(after.len(), before.len(), "{}", t.name);
                assert_ne!(after, before, "{}", t.name);
                let (mut x, mut y) = (after.clone(), before.clone());
                x.sort();
                y.sort();
                assert_eq!(x, y, "{}", t.name);
            }
        }
    }
    if !matches!(case, Case::Append) {
        assert_ne!(b.building_id("bed"), c.building_id("bed"));
        assert_ne!(b.item_id("wood"), c.item_id("wood"));
    }
    copy
}

/// A busy world per nation, played under the bundled data: the scripted opening with bending and
/// the whole tech tree queued, then a fresh build site. Built once per test process (nextest
/// runs each test in its own). A 64x64 map: what's under test is how ids map, not the map's
/// size, and every save and load of a small-map world costs an eighth as much.
fn busy_worlds() -> &'static [World] {
    static WORLDS: OnceLock<Vec<World>> = OnceLock::new();
    WORLDS.get_or_init(|| {
        let defs = Defs::bundled();
        Nation::ALL
            .into_iter()
            .enumerate()
            .map(|(i, nation)| {
                let cfg = MapGenConfig {
                    seed: 30 + i as u64,
                    width: 64,
                    height: 64,
                    ..Default::default()
                };
                let mut w = World::new(&cfg, nation, 4, &defs);
                let opening = Opening::with_bending(4, 2).with_full_research();
                for tick in 0..6000 {
                    if Opening::due(tick) {
                        opening.step(&mut w, &defs);
                    }
                    w.tick(&defs);
                }
                // A site left unbuilt: a bed on free ground near the settlement.
                let home = w.settlement_anchor();
                let bed = defs.building_id("bed").unwrap();
                let site = w
                    .map
                    .iter()
                    .filter(|(p, t)| {
                        t.terrain.is_walkable()
                            && t.resource.is_none()
                            && (p.x - home.x).abs().max((p.y - home.y).abs()) <= 10
                    })
                    .map(|(p, _)| p)
                    .find(|&pos| {
                        w.clone()
                            .apply(Command::DesignateBuild { pos, building: bed }, &defs)
                            .is_ok()
                    })
                    .expect("room for a bed");
                w.apply(Command::DesignateBuild { pos: site, building: bed }, &defs).unwrap();
                w
            })
            .collect()
    })
}

#[test]
fn the_busy_worlds_exercise_every_id_bearing_structure() {
    let defs = Defs::bundled();
    for w in busy_worlds() {
        let n = w.nation;
        let starting = &defs.nation(n).starting_techs;
        assert!(w.research().known_techs().any(|t| !starting.contains(&t)), "{n:?}: a tech done");
        assert!(!w.research().queue().is_empty(), "{n:?}: research queued");
        assert!(w.build_sites().count() > 0, "{n:?}: a build site");
        assert!(w.map.iter().any(|(_, t)| t.building.is_some()), "{n:?}: a building");
        assert!(w.tallies().harvested_items().count() > 0, "{n:?}: harvests tallied");
        assert!(w.units.iter().any(|u| u.bends() > 0), "{n:?}: bends counted");
        assert!(!w.jobs().is_empty(), "{n:?}: jobs");
    }
}

/// What a world holds, by name: buildings and sites by position, known techs, the queue, and
/// harvest tallies. Equal under two data sets means every id was mapped to the same content.
fn by_name(w: &World, defs: &Defs) -> String {
    let buildings: Vec<(Pos, &str)> = w
        .map
        .iter()
        .filter_map(|(p, _)| w.building_at(p).map(|b| (p, defs.building(b).id.as_str())))
        .collect();
    let sites: Vec<(Pos, &str)> =
        w.build_sites().map(|(f, b, _)| (f.anchor, defs.building(b).id.as_str())).collect();
    let known: BTreeSet<&str> =
        w.research().known_techs().map(|t| defs.tech(t).id.as_str()).collect();
    let queue: Vec<&str> = w.research().queue().iter().map(|&t| defs.tech(t).id.as_str()).collect();
    let harvested: BTreeSet<(&str, u32)> =
        w.tallies().harvested_items().map(|(i, n)| (defs.item(i).id.as_str(), n)).collect();
    let biomes: Vec<&str> = w.map.iter().map(|(_, t)| defs.biome(t.biome).id.as_str()).collect();
    let resources: Vec<Option<&str>> =
        w.map.iter().map(|(_, t)| t.resource.map(|r| defs.resource(r).id.as_str())).collect();
    format!(
        "{buildings:?}\n{sites:?}\n{known:?}\n{queue:?}\n{harvested:?}\n{biomes:?}\n{resources:?}"
    )
}

/// Every save under test, as text written under the bundled data: the v1 fixture as it is, and
/// a v2 save of each busy world.
fn saves() -> Vec<(String, String)> {
    let defs = Defs::bundled();
    let mut out = vec![("v1 fixture".to_owned(), FIXTURE.to_owned())];
    for w in busy_worlds() {
        out.push((format!("{:?}", w.nation), w.save(&defs).unwrap()));
    }
    out
}

fn check(case: Case) {
    let bundled = Defs::bundled();
    let copy = copy(case);
    for (name, text) in saves() {
        let what = format!("{case:?}, {name}");
        // S0: the save as the bundled data reads and writes it (the fixture comes back as v2).
        let original = World::load(&text, &bundled).unwrap_or_else(|e| panic!("{what}: {e}"));
        let s0 = original.save(&bundled).unwrap();

        // Round trip through the copy and back, byte for byte.
        let under_copy = World::load(&s0, &copy).unwrap_or_else(|e| panic!("{what}: {e}"));
        assert_eq!(by_name(&under_copy, &copy), by_name(&original, &bundled), "{what}");
        let s1 = under_copy.save(&copy).unwrap();
        let s2 = World::load(&s1, &bundled).unwrap().save(&bundled).unwrap();
        assert!(s2 == s0, "{what}: the round trip through the edited data changed the save");

        // The original text (v1 for the fixture) loads under the copy just the same.
        let direct = World::load(&text, &copy).unwrap_or_else(|e| panic!("{what}: {e}"));
        assert_eq!(direct.save(&copy).unwrap(), s1, "{what}");

        // Play on under the copy, then save and load again.
        let mut w = under_copy;
        let start = w.tick;
        for _ in 0..600 {
            w.tick(&copy);
        }
        assert_eq!(w.tick, start + 600);
        let again = World::load(&w.save(&copy).unwrap(), &copy).unwrap();
        assert_eq!(again.tick, w.tick, "{what}");
    }
}

#[test]
fn a_save_loads_and_plays_on_after_content_is_appended() {
    check(Case::Append);
}

#[test]
fn a_save_loads_and_plays_on_after_content_is_inserted() {
    check(Case::Insert);
}

#[test]
fn a_save_loads_and_plays_on_after_content_is_reordered() {
    check(Case::Reorder);
}

#[test]
fn a_save_that_uses_removed_content_names_it() {
    // Made under the append copy with its extra building designated, loaded under the bundled
    // data, which doesn't have it.
    let appended = copy(Case::Append);
    let mut w = World::load(&busy_worlds()[0].save(&Defs::bundled()).unwrap(), &appended).unwrap();
    let test = appended.building_id("test_building").unwrap();
    let home = w.settlement_anchor();
    let near: Vec<Pos> = w
        .map
        .iter()
        .map(|(p, _)| p)
        .filter(|p| (p.x - home.x).abs().max((p.y - home.y).abs()) <= 12)
        .collect();
    let pos = near
        .into_iter()
        .find(|&pos| w.apply(Command::DesignateBuild { pos, building: test }, &appended).is_ok())
        .expect("room for the test building");
    assert!(w.build_site(pos).is_some());
    let text = w.save(&appended).unwrap();
    let err = World::load(&text, &Defs::bundled()).unwrap_err();
    assert_eq!(err, SaveError::MissingContent { kind: "building", id: "test_building".into() });
    assert!(err.to_string().contains("building \"test_building\""), "{err}");
    // Content the save lists but never uses doesn't matter.
    let unused = World::load(&busy_worlds()[1].save(&Defs::bundled()).unwrap(), &appended).unwrap();
    assert!(World::load(&unused.save(&appended).unwrap(), &Defs::bundled()).is_ok());
}

#[test]
fn a_v1_save_with_unknown_data_is_a_data_mismatch() {
    let defs = Defs::bundled();
    let header = FIXTURE.lines().next().unwrap();
    let hash = header.split("data_hash:").nth(1).unwrap().split(',').next().unwrap();
    let other = FIXTURE.replacen(&format!("data_hash:{hash}"), "data_hash:1", 1);
    assert_eq!(World::load(&other, &defs).err(), Some(SaveError::DataMismatch));
    assert_eq!(fn_sim::save::inspect(&other).err(), Some(SaveError::DataMismatch));
}

#[test]
fn a_v2_save_with_another_data_hash_still_loads() {
    // The hash is information only now.
    let defs = Defs::bundled();
    let text = busy_worlds()[2].save(&defs).unwrap();
    let tampered = text.replacen(&format!("data_hash:{}", defs.id_hash()), "data_hash:1", 1);
    assert_ne!(tampered, text);
    let w = World::load(&tampered, &defs).unwrap();
    assert_eq!(w.save(&defs).unwrap(), text);
}

#[test]
fn a_save_carrying_the_retired_unit_variant_loads_and_drops_it() {
    // `Unit::variant` was removed in #607; a save written before still names it on every unit.
    let defs = Defs::bundled();
    let text = busy_worlds()[2].save(&defs).unwrap();
    assert!(!text.contains("variant:"), "new saves no longer write it");
    let old = text.replace(",needs:(", ",variant:1,needs:(");
    assert_ne!(old, text);
    let w = World::load(&old, &defs).unwrap();
    assert_eq!(w.save(&defs).unwrap(), text);
    // The checked-in fixture's units carry it too.
    assert!(FIXTURE.contains("variant:"));
    World::load(FIXTURE, &defs).unwrap();
}

// ---- Great ruins in a pre-phase-5 save (#736) ----

/// A save from before phase 5 (no `teachings`, and no great ruins: its data had no promote pass)
/// gets its great ruins on load, among its still-buried lost ruins, the same ones every time.
#[test]
fn a_pre_phase_5_save_gets_great_ruins_from_its_buried_lost_ruins() {
    let mut files = bundled_text();
    let promote = files[1].find("[resource.promote]").expect("great_ruins promotes");
    files[1].truncate(promote);
    let old = defs_from(&files);
    let defs = Defs::bundled();
    let (lost, great) =
        (defs.resource_id("lost_ruins").unwrap(), defs.resource_id("great_ruins").unwrap());
    let cfg = MapGenConfig { seed: 7, width: 192, height: 128, ..MapGenConfig::default() };
    let world = World::new(&cfg, Nation::Air, 3, &old);
    assert!(world.buried().all(|(_, r)| r != great), "the old data placed no great ruins");
    let before: BTreeSet<Pos> =
        world.buried().filter(|&(_, r)| r == lost).map(|(p, _)| p).collect();
    let text = world.save(&old).unwrap();
    let at = text.find("teachings:").expect("a phase-5 save writes teachings");
    let (head, body) = text.split_at(at);
    // Drop the `teachings:(...)` field, as a save written before it existed.
    let close = body.find(')').unwrap() + 1;
    let rest = body[close..].strip_prefix(',').unwrap_or(&body[close..]);
    let old_save = format!("{head}{rest}");

    let a = World::load(&old_save, &defs).unwrap();
    let b = World::load(&old_save, &defs).unwrap();
    assert!(a.great_ruins_placed());
    let greats = |w: &World| -> Vec<Pos> {
        w.buried().filter(|&(_, r)| r == great).map(|(p, _)| p).collect()
    };
    assert_eq!(greats(&a), greats(&b), "the upgrade is deterministic");
    assert_eq!(greats(&a).len(), 2, "Small gets 2");
    assert!(greats(&a).iter().all(|p| before.contains(p)), "only buried lost ruins promote");
    let after: BTreeSet<Pos> =
        a.buried().filter(|&(_, r)| r == lost || r == great).map(|(p, _)| p).collect();
    assert_eq!(after, before, "nothing else moved");
    // A new save of the upgraded world loads as it is: no second promotion.
    let again = World::load(&a.save(&defs).unwrap(), &defs).unwrap();
    assert_eq!(greats(&again), greats(&a));
    // And a world made with today's data put the same ruins there at mapgen.
    let fresh = World::new(&cfg, Nation::Air, 3, &defs);
    assert_eq!(BTreeSet::from_iter(greats(&fresh)), BTreeSet::from_iter(greats(&a)));
}
