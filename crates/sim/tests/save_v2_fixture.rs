//! A v2 save with every kind of building state in it, kept as text (#617), and the behaviour it
//! must keep producing.
//!
//! `docs/design/buildings-as-records.md` (#604) moves a finished building's state (craft batch,
//! chart progress, orchard, farm plot, fire, power and irrigation membership) from side tables
//! keyed by anchor into one record per building, and writes a new save format (v3). It promises
//! "no behaviour change". This file is the proof, in two halves:
//!
//! - `the_fixture_holds_every_kind_of_building_state`: the fixture really has all of it, in a
//!   non-default state, so the next test can't pass on an empty settlement.
//! - `the_fixture_plays_on_to_the_recorded_digest`: load the fixture, run 1,200 ticks, and
//!   compare a digest of the result with `fixtures/v2_buildings.after.txt`, which the build
//!   *before* the refactor wrote. The v3 build must reproduce it byte for byte.
//!
//! # The fixture and the digest
//!
//! `fixtures/v2_buildings.ron` is a 32x24 field of grass (hand-made, so no mapgen) with a bed, a
//! campfire (a building fire) and a timed flame, a 2x2 healing hut, a study hall with a scholar
//! seated, a Map Room charting, a forge with a smith mid-batch, a wind turbine powering it, an
//! orchard with a tree, a farm plot growing beside an irrigation channel, a stretch of rail, and
//! a part-delivered build site. A Fire settlement of six, saved at tick 901.
//!
//! It was written by `world::v2_fixture::regenerate_v2_fixture` (a `#[ignore]`d unit test in
//! `crates/sim/src/world/v2_fixture.rs`, which needs private access to stand buildings up). The
//! digest was written by `write_v2_digest` below. Both are one-shot, and **must not be
//! regenerated once the format has changed** (v3, #604 part 2): a file written then would no
//! longer prove anything about v2 saves or about the refactor. Both generators go away with the
//! format change, the way #602 removed the v1 one.
//!
//! The digest uses public queries only, and names content (items, buildings, techs) rather than
//! printing table indices. Ordinary content changes (#602: saves are remapped by name) therefore
//! neither break the fixture nor change the digest; `appending_content_changes_nothing` checks it.
//! Anything else that changes the digest changed the simulation.
//!
//! # A quirk the digest records
//!
//! `World::load` adopts orchards by scanning the map for tiles with an orchard on them
//! (`adopt_orchards`), so a 2x2 orchard comes back as four orchards, one per tile, each planting
//! and tending trees. The digest was taken from the loaded fixture, so it records that: its
//! `orchards` line lists four. A build that registers one orchard per building will not match it
//! until the digest is re-recorded from a v2 build that has the same fix.
// Test code, not the sim: it reads files and the environment. The lint is for the library
// (AGENTS.md rule 4).
#![allow(clippy::disallowed_methods)]

use std::fmt::Write;

use fn_sim::building::BuildingId;
use fn_sim::jobs::{ItemId, Stack};
use fn_sim::world::PlotStage;
use fn_sim::{Defs, Nation, Pos, SAVE_VERSION, World};

const FIXTURE: &str = include_str!("fixtures/v2_buildings.ron");
const AFTER: &str = include_str!("fixtures/v2_buildings.after.txt");

const WIDTH: i32 = 32;
const HEIGHT: i32 = 24;
/// Ticks the digest plays on for.
const TICKS: u64 = 1_200;

// Where `world::v2_fixture` put things (the same constants, repeated: that module is crate-private).
const BED: Pos = Pos::new(8, 6);
const CAMPFIRE: Pos = Pos::new(10, 6);
const HUT: Pos = Pos::new(12, 5);
const HALL: Pos = Pos::new(16, 4);
const MAP_ROOM: Pos = Pos::new(20, 4);
const FORGE: Pos = Pos::new(16, 12);
const TURBINE: Pos = Pos::new(18, 12);
const ORCHARD: Pos = Pos::new(22, 12);
const PLOT: Pos = Pos::new(6, 14);
const CHANNEL: Pos = Pos::new(7, 14);
const RAIL: [Pos; 4] = [Pos::new(10, 18), Pos::new(11, 18), Pos::new(12, 18), Pos::new(13, 18)];
const SITE: Pos = Pos::new(20, 18);
const FLAME: Pos = Pos::new(26, 20);

fn id(defs: &Defs, building: &str) -> BuildingId {
    defs.building_id(building).unwrap_or_else(|| panic!("no building {building}"))
}

fn loaded(defs: &Defs) -> World {
    World::load(FIXTURE, defs).expect("the fixture loads")
}

/// Whether the body of the saved file has a non-empty `field` (a `World` field with a name no
/// public query can reach: the craft batches, irrigation channels and power sources).
fn body_has(field: &str) -> bool {
    let body = FIXTURE.lines().nth(2).expect("a body line");
    body.contains(&format!("{field}:{{(")) || body.contains(&format!("{field}:[("))
}

#[test]
fn the_fixture_is_a_v2_save() {
    let info = fn_sim::save::inspect(FIXTURE).unwrap();
    assert_eq!(info.version, 2);
    assert_eq!((info.seed, info.nation), (7, Nation::Fire));
    assert!(info.tick > 0);
    assert_eq!(FIXTURE.lines().count(), 3, "header, id tables, body");
    assert!(FIXTURE.len() < 150_000, "keep the fixture small: {} bytes", FIXTURE.len());
}

#[test]
fn the_fixture_holds_every_kind_of_building_state() {
    let defs = Defs::bundled();
    let w = loaded(&defs);
    assert_eq!((w.map.width(), w.map.height()), (WIDTH, HEIGHT));
    assert_eq!(w.units.len(), 6);

    // Every finished building is on the map, with its footprint.
    for (pos, name, size) in [
        (BED, "bed", (1, 1)),
        (CAMPFIRE, "campfire", (1, 1)),
        (HUT, "healing_hut", (2, 2)),
        (HALL, "study_hall", (2, 2)),
        (MAP_ROOM, "map_room", (2, 2)),
        (FORGE, "forge", (2, 2)),
        (TURBINE, "wind_turbine", (1, 1)),
        (ORCHARD, "orchard", (2, 2)),
        (PLOT, "farm_plot", (1, 1)),
        (CHANNEL, "irrigation_channel", (1, 1)),
    ] {
        assert_eq!(w.building_at(pos), Some(id(&defs, name)), "{name}");
        let foot = w.footprint_at(pos).expect(name);
        assert_eq!((foot.anchor, (foot.w, foot.h)), (pos, size), "{name}");
    }
    // A multi-tile building answers from every tile of its footprint.
    assert_eq!(w.building_at(Pos::new(HUT.x + 1, HUT.y + 1)), Some(id(&defs, "healing_hut")));
    for pos in RAIL {
        assert_eq!(w.building_at(pos), Some(id(&defs, "rail")), "rail at {pos:?}");
    }

    // A building fire (the campfire) and a timed flame, which are different things.
    assert!(w.is_heated(CAMPFIRE) && w.is_heated(Pos::new(CAMPFIRE.x + 1, CAMPFIRE.y)));
    assert!(!w.is_heated(Pos::new(0, 0)));
    let flames = w.flames();
    assert_eq!(flames.len(), 1, "only the timed flame is listed");
    assert_eq!(flames[0].pos, FLAME);
    assert!(flames[0].seconds_left > 0.0);
    assert!(w.is_heated(FLAME));

    // A study hall with a scholar seated, and the research it feeds.
    assert!(w.units.iter().any(|u| u.inside() == Some(HALL)), "a scholar in the hall");
    assert!(w.study_seats_filled() > 0);
    assert!(!w.research().queue().is_empty(), "a research target");
    assert!(w.research().known_techs().count() > 1);

    // A Map Room with chart progress.
    assert!(w.chart_progress(MAP_ROOM) > 0.0, "chart progress {}", w.chart_progress(MAP_ROOM));
    assert!(w.buried_count() > 0, "a ruin left to chart");

    // A forge mid-batch: a smith inside, and the batch in the side table.
    assert!(w.units.iter().any(|u| u.inside() == Some(FORGE)), "a smith in the forge");
    assert!(body_has("crafts"), "a craft batch under way");

    // A wind turbine powering the forge.
    let power = w.power_for(w.footprint_at(FORGE).unwrap(), &defs).expect("the forge is powered");
    assert_eq!(power.source, TURBINE);
    assert!(power.boost > 1.0);
    assert!(body_has("power_sources"));

    // An orchard with a tree, a farm plot growing, and the channel in reach of the plot.
    assert!(w.orchard_trees(ORCHARD).is_some_and(|n| n > 0), "an orchard tree");
    assert!(w.orchards().any(|(p, n)| p == ORCHARD && n > 0));
    assert_eq!(w.plot_stage(PLOT), Some(PlotStage::Growing));
    assert!(w.plot_growth_left(PLOT).is_some_and(|left| left > 0.0));
    assert!(body_has("irrigation_channels"));

    // A build site with some of its cost delivered.
    let sites: Vec<_> = w.build_sites().collect();
    assert_eq!(sites.len(), 1);
    assert_eq!((sites[0].0.anchor, sites[0].1), (SITE, id(&defs, "windmill")));
    let (_, delivered, _) = w.build_site(SITE).unwrap();
    assert!(delivered.iter().sum::<u32>() > 0, "some of the cost is in: {delivered:?}");

    // Items on the ground and in the stockpile.
    assert!(w.ground_items().count() > 0);
    assert!(w.stockpile_zones().filter_map(|p| w.stored_at(p)).count() > 0);
}

/// A name for what `item` is: the content's own id, never its index.
fn item_name(defs: &Defs, item: ItemId) -> &str {
    &defs.item(item).id
}

fn stack(defs: &Defs, s: Stack) -> String {
    format!("{}x{}", item_name(defs, s.item), s.count)
}

/// Every finished building, in `World::buildings` order. The one place the digest touches that
/// query's item type, which #604 part 2 changes (to a record with `footprint()` and `id`): adapt
/// this and nothing else.
fn listing(w: &World) -> Vec<(fn_sim::geom::Footprint, BuildingId)> {
    w.buildings().collect()
}

/// The world as a text a person can diff: units, jobs, tallies, research, stacks and every
/// tile's building state, from public queries only. Names stand for ids, so appending content
/// (or reordering it) doesn't change the text; floats are rounded to three places.
fn digest(w: &World, defs: &Defs) -> String {
    let mut out = String::new();
    let o = &mut out;
    writeln!(o, "tick {}", w.tick).unwrap();
    writeln!(o, "deaths {} buried {}", w.deaths.len(), w.buried_count()).unwrap();

    writeln!(o, "\n[units]").unwrap();
    for u in &w.units {
        let task = u.task().map(|t| format!("{t:?}")).unwrap_or_default();
        // The variant only: `Eat(..)` and friends carry content ids, and the jobs are listed below.
        let task = task.split('(').next().unwrap_or_default();
        let carrying = u.carrying.map(|s| stack(defs, s)).unwrap_or_else(|| "-".into());
        writeln!(
            o,
            "{} {} ({},{}) inside {:?} task [{}] carrying {} hunger {:.3} thirst {:.3} energy {:.3} \
             health {:.3} chi {:.3}",
            u.id.0,
            u.name,
            u.pos.x,
            u.pos.y,
            u.inside().map(|p| (p.x, p.y)),
            task,
            carrying,
            u.needs.hunger,
            u.needs.thirst,
            u.needs.energy,
            u.needs.health,
            u.chi,
        )
        .unwrap();
    }

    writeln!(o, "\n[jobs]").unwrap();
    for j in w.jobs() {
        writeln!(o, "{} {:?} by {:?} progress {:.3}", j.id.0, j.kind, j.claimed_by, j.progress)
            .unwrap();
    }

    writeln!(o, "\n[tallies]").unwrap();
    let t = w.tallies();
    let mut harvested: Vec<_> =
        t.harvested_items().map(|(i, n)| format!("{}={n}", item_name(defs, i))).collect();
    harvested.sort();
    writeln!(o, "harvested {}", harvested.join(" ")).unwrap();
    writeln!(o, "scrolls {} crops {}", t.scrolls_read(), t.crops_harvested()).unwrap();

    writeln!(o, "\n[research]").unwrap();
    let r = w.research();
    let tech = |t| defs.tech(t).id.clone();
    let mut known: Vec<_> = r.known_techs().map(tech).collect();
    known.sort();
    writeln!(o, "pool {} known {}", r.pool_millis(), known.join(",")).unwrap();
    writeln!(o, "queue {}", r.queue().iter().map(|&t| tech(t)).collect::<Vec<_>>().join(","))
        .unwrap();
    for c in r.completions() {
        writeln!(o, "completed {} at {}", tech(c.tech), c.tick).unwrap();
    }

    writeln!(o, "\n[stacks]").unwrap();
    for (p, s) in w.ground_items() {
        writeln!(o, "ground ({},{}) {}", p.x, p.y, stack(defs, s)).unwrap();
    }
    for p in w.stockpile_zones() {
        let held = w.stored_at(p).map(|s| stack(defs, s)).unwrap_or_else(|| "-".into());
        writeln!(o, "stored ({},{}) {held}", p.x, p.y).unwrap();
    }

    writeln!(o, "\n[sites]").unwrap();
    for (foot, building, progress) in w.build_sites() {
        let (_, delivered, _) = w.build_site(foot.anchor).unwrap();
        writeln!(
            o,
            "{} at ({},{}) {}x{} progress {progress:.3} delivered {delivered:?}",
            defs.building(building).id,
            foot.anchor.x,
            foot.anchor.y,
            foot.w,
            foot.h
        )
        .unwrap();
    }

    writeln!(o, "\n[buildings]").unwrap();
    for (foot, building) in listing(w) {
        writeln!(
            o,
            "{} at ({},{}) {}x{} chart {:.3} trees {:?} plot {:?}/{:?} power {:?}",
            defs.building(building).id,
            foot.anchor.x,
            foot.anchor.y,
            foot.w,
            foot.h,
            w.chart_progress(foot.anchor),
            w.orchard_trees(foot.anchor),
            w.plot_stage(foot.anchor),
            w.plot_growth_left(foot.anchor).map(|g| format!("{g:.3}")),
            w.power_for(foot, defs).map(|p| (p.source.x, p.source.y, format!("{:.3}", p.boost))),
        )
        .unwrap();
    }
    writeln!(o, "orchards {:?}", w.orchards().collect::<Vec<_>>()).unwrap();
    writeln!(o, "charts {:?}", w.chart_events()).unwrap();
    writeln!(o, "flames {:?}", w.flames()).unwrap();

    // Every tile, by public query: what stands on it, what grows on it, whether a source warms
    // it and whether a power source reaches it.
    writeln!(o, "\n[map]").unwrap();
    for (p, tile) in w.map.iter() {
        let building = w.building_at(p).map(|b| defs.building(b).id.as_str());
        let resource = tile.resource.map(|r| defs.resource(r).id.as_str());
        let sapling = w.is_sapling(p);
        if building.is_some() || resource.is_some() || sapling {
            let foot = w.footprint_at(p).map(|f| (f.anchor.x, f.anchor.y, f.w, f.h));
            writeln!(
                o,
                "({},{}) building {building:?} footprint {foot:?} resource {resource:?} \
                 sapling {sapling}",
                p.x, p.y,
            )
            .unwrap();
        }
    }
    for (title, pred) in [
        ("heated", &(|p: Pos| w.is_heated(p)) as &dyn Fn(Pos) -> bool),
        ("cooking", &|p| w.is_cooking(p)),
        ("powered", &|p| w.power_at(p, defs).is_some()),
    ] {
        writeln!(o, "{title}").unwrap();
        for y in 0..HEIGHT {
            let row: String =
                (0..WIDTH).map(|x| if pred(Pos::new(x, y)) { '#' } else { '.' }).collect();
            writeln!(o, "{row}").unwrap();
        }
    }
    out
}

/// The digest of the fixture after `TICKS` ticks under `defs`.
fn played_digest(defs: &Defs) -> String {
    let mut w = loaded(defs);
    let start = w.tick;
    for _ in 0..TICKS {
        w.tick(defs);
    }
    assert_eq!(w.tick, start + TICKS);
    digest(&w, defs)
}

#[test]
fn the_fixture_plays_on_to_the_recorded_digest() {
    let got = played_digest(&Defs::bundled());
    if got != AFTER {
        let line = got.lines().zip(AFTER.lines()).position(|(a, b)| a != b);
        let at = line.unwrap_or_else(|| got.lines().count().min(AFTER.lines().count()));
        panic!(
            "the simulation of the v2 fixture changed (first difference at line {}).\n\
             recorded: {:?}\n\
             got:      {:?}\n\
             This digest was written before buildings became records (#604); until that \
             refactor is done it pins today's behaviour. If you changed the rules on purpose, \
             say so in the commit and re-record `fixtures/v2_buildings.after.txt` with \
             `FN_REGENERATE_FIXTURE=1 cargo test -p fn_sim --test save_v2_fixture -- --ignored \
             write_v2_digest`; never after the v3 format is in.",
            at + 1,
            AFTER.lines().nth(at),
            got.lines().nth(at),
        );
    }
}

#[test]
fn the_digest_has_something_to_say() {
    // A guard against a digest recorded from an empty or stalled world.
    for needle in [
        "[units]",
        "study_hall at (16,4) 2x2",
        "map_room at (20,4) 2x2",
        "forge at (16,12) 2x2",
        "orchard at (22,12) 2x2",
        "plot Some(",
        "power Some((18, 12",
        "Craft",
        "Study",
        "Chart",
    ] {
        assert!(AFTER.contains(needle), "the recorded digest lacks {needle:?}");
    }
    assert!(AFTER.lines().count() > 150, "{} lines", AFTER.lines().count());
    assert!(AFTER.contains("heated\n") && AFTER.contains('#'));
}

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

#[test]
fn appending_content_changes_nothing() {
    // #602: an ordinary content change must not break the fixture or move the digest. Append a
    // building and a tech to the end of their tables and play the same 1,200 ticks.
    let mut files = FILES.map(str::to_owned);
    files[3].push_str(
        "\n[[tech]]\nid = \"test_tech\"\nname = \"Test\"\ndescription = \"\"\ncost = 10\n",
    );
    files[7].push_str(
        "\n[[building]]\nid = \"test_building\"\nname = \"Test\"\nverb = \"Build\"\nwork = 20.0\n\
         passable = true\n[building.bed]\nsleep_bonus = 1.4\nseek_tiles = 30\n\
         [[building.cost]]\nitem = \"wood\"\ncount = 5\n",
    );
    let defs = Defs::from_files(std::array::from_fn(|i| files[i].as_str())).unwrap();
    assert!(defs.building_id("test_building").is_some() && defs.tech_id("test_tech").is_some());
    assert_ne!(defs.id_hash(), Defs::bundled().id_hash());
    assert_eq!(played_digest(&defs), AFTER);
}

/// Records `fixtures/v2_buildings.after.txt` from the fixture. One-shot: see the module docs.
#[test]
#[ignore = "rewrites the checked-in digest; run by hand, and only while saves are v2"]
fn write_v2_digest() {
    assert!(
        std::env::var_os("FN_REGENERATE_FIXTURE").is_some(),
        "set FN_REGENERATE_FIXTURE=1 to confirm you mean to rewrite the digest"
    );
    assert_eq!(SAVE_VERSION, 2, "the digest records the pre-refactor (v2) build");
    let text = played_digest(&Defs::bundled());
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/v2_buildings.after.txt");
    std::fs::write(path, text).unwrap();
}
