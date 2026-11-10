//! Weld (Fire) and Lift (Air): the `build` effect (#363, docs/design/bending.md, "More
//! abilities and the route to mastery"). Both add `work_seconds` x their own magnitude modifier
//! to an ordinary build site's banked work. Work banked before the site's materials arrive is
//! kept, and the site finishes as soon as both are complete.

use super::*;
use crate::defs::{BiomeId, Terrain};
use crate::geom::Grid;
use crate::jobs::{ItemId, JobId, Stack};
use crate::mapgen::Tile;

fn at(x: i32, y: i32) -> Pos {
    Pos::new(x, y)
}

/// Wide and tall enough for a study hall (2x2), a 4x4 gate footprint and a stockpile, all apart.
const GRID: [&str; 8] = [
    "............",
    "............",
    "............",
    "............",
    "............",
    "............",
    "............",
    "............",
];

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

fn setup(nation: Nation, units: &[Pos]) -> (World, Defs) {
    let defs = Defs::bundled();
    let map = arena(&GRID);
    (World::with_map(1, map, nation, units, &defs), defs)
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

/// `nation`, its ability and the tech that unlocks it, in the order the AC lists them.
const BUILDERS: [(Nation, &str, &str); 2] =
    [(Nation::Fire, "weld", "forging"), (Nation::Air, "lift", "windmills")];

/// Where the tests put a study hall site, the stockpile that supplies its wood, and a second
/// settler (the bender, apart from the builder who claims the build job first: #284's job
/// priority ties break on job id, and the site is always designated first).
const ANCHOR: Pos = Pos::new(4, 2);
const STOCKPILE: Pos = Pos::new(1, 6);
const SPAWNS: [Pos; 2] = [Pos::new(0, 0), Pos::new(0, 1)];

/// Grants the tech, fills every unit's chi and casts `ability` on `pos`, running until it lands,
/// which it should. Filling every unit's chi (rather than picking one) leaves it to the usual
/// job-claiming rules to pick the bender, same as the player would see.
fn cast(w: &mut World, defs: &Defs, ability: &str, tech: &str, pos: Pos) {
    w.research.grant(defs.tech_id(tech).unwrap());
    w.refresh_modifiers(defs);
    for u in &mut w.units {
        u.chi = defs.chi.max;
    }
    let id = defs.ability_id(ability).unwrap();
    w.apply(Command::Bend { ability: id, pos }, defs).unwrap();
    let events = w.bend_events().count();
    run_until(w, defs, 2000, |w| w.bend_events().count() > events);
    assert_eq!(w.bend_events().next_back().unwrap().outcome, BendOutcome::Done, "{ability}");
}

/// Designates a study hall at `ANCHOR`, with its wood already stocked.
fn designate_supplied_study_hall(w: &mut World, defs: &Defs) -> (JobId, ItemId, u32) {
    let study_hall = defs.building_id("study_hall").unwrap();
    let wood = defs.item_id("wood").unwrap();
    let cost = defs.building(study_hall).cost[0].count;
    w.designate_stockpile(STOCKPILE).unwrap();
    w.stored.insert(STOCKPILE, Stack { item: wood, count: cost });
    let job = w.designate_build(ANCHOR, study_hall, defs).unwrap();
    (job, wood, cost)
}

/// A weld (or lift) on a site whose materials are already delivered finishes it sooner than
/// without one (AC): it banks real work seconds straight onto the site.
#[test]
fn a_weld_on_a_supplied_site_finishes_it_sooner_than_without_one() {
    let defs = Defs::bundled();
    let study_hall = defs.building_id("study_hall").unwrap();

    for (nation, ability, tech) in BUILDERS {
        let (mut base, _) = setup(nation, &SPAWNS);
        let (base_job, ..) = designate_supplied_study_hall(&mut base, &defs);
        let base_ticks = run_until(&mut base, &defs, 8000, |w| w.job(base_job).is_none());

        let (mut bent, _) = setup(nation, &SPAWNS);
        let (bent_job, ..) = designate_supplied_study_hall(&mut bent, &defs);
        cast(&mut bent, &defs, ability, tech, ANCHOR);
        let bent_ticks = run_until(&mut bent, &defs, 8000, |w| w.job(bent_job).is_none());

        assert!(
            bent_ticks < base_ticks,
            "{ability}: {bent_ticks} should be fewer ticks than {base_ticks}"
        );
        assert_eq!(bent.building_at(ANCHOR), Some(study_hall), "{ability}");
    }
}

/// A weld (or lift) before the materials arrive banks progress, and the building completes once
/// the last material arrives (AC): the bend lands on an unsupplied site, work accrues there with
/// nothing yet delivered, and only once the hauler finishes the delivery does the building
/// finish too.
#[test]
fn a_weld_before_delivery_banks_progress_until_the_last_material_arrives() {
    let defs = Defs::bundled();
    let study_hall = defs.building_id("study_hall").unwrap();

    for (nation, ability, tech) in BUILDERS {
        let (mut w, _) = setup(nation, &SPAWNS);
        let study_hall_def = defs.building(study_hall);
        let wood = defs.item_id("wood").unwrap();
        let cost = study_hall_def.cost[0].count;
        let job = w.designate_build(ANCHOR, study_hall, &defs).unwrap();

        cast(&mut w, &defs, ability, tech, ANCHOR);
        let (_, _, progress) = w.build_site(ANCHOR).expect("still a site: no materials yet");
        assert!(progress > 0.0, "{ability}: work should be banked before materials arrive");
        assert!(w.building_at(ANCHOR).is_none(), "{ability}: can't finish without materials");

        // Now the materials show up; the normal hauler-and-builder flow should finish the
        // building, picking up right where the banked work left off.
        w.designate_stockpile(STOCKPILE).unwrap();
        w.stored.insert(STOCKPILE, Stack { item: wood, count: cost });
        run_until(&mut w, &defs, 8000, |w| w.job(job).is_none());
        assert_eq!(w.building_at(ANCHOR), Some(study_hall), "{ability}");
    }
}

/// A weld (or lift) on the gate site, or on a finished building, is refused with a reason (AC).
#[test]
fn a_weld_on_the_gate_or_a_finished_building_is_refused() {
    let defs = Defs::bundled();
    let study_hall = defs.building_id("study_hall").unwrap();

    for (nation, ability, tech) in BUILDERS {
        let (mut w, _) = setup(nation, &[at(0, 0)]);
        w.research.grant(defs.tech_id(tech).unwrap());
        w.refresh_modifiers(&defs);
        let id = defs.ability_id(ability).unwrap();

        // A finished building: deliver its cost and let it complete, then weld it.
        let (job, ..) = designate_supplied_study_hall(&mut w, &defs);
        run_until(&mut w, &defs, 8000, |w| w.job(job).is_none());
        assert_eq!(w.building_at(ANCHOR), Some(study_hall));
        assert!(
            w.apply(Command::Bend { ability: id, pos: ANCHOR }, &defs).is_err(),
            "{ability}: a finished building should refuse a build bend"
        );

        // The gate's own site (#236): `gate.rs` owns its stages, so a build bend must refuse it.
        w.endeavor = EndeavorState::Ready { tick: w.tick };
        let gate_anchor = at(7, 3);
        w.apply(Command::BeginEndeavor { anchor: gate_anchor }, &defs).expect("gate placed");
        assert!(
            w.apply(Command::Bend { ability: id, pos: gate_anchor }, &defs).is_err(),
            "{ability}: the gate's site should refuse a build bend"
        );
    }
}
