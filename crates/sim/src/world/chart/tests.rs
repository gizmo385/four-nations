use super::*;
use crate::defs::{BiomeId, Terrain};
use crate::geom::{Footprint, Grid};
use crate::mapgen::Tile;
use crate::world::{Command, Task};

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

fn setup(nation: Nation, rows: &[&str], units: &[Pos]) -> (World, Defs) {
    let defs = Defs::bundled();
    let map = arena(rows);
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

const GRID: [&str; 10] = [
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
];

/// Puts a finished Map Room at `anchor` (skipping the construction job, which `build.rs` already
/// covers), covering its real footprint, and opens its one Chart seat, the way `run_build` does
/// the moment one finishes.
fn place_map_room(w: &mut World, defs: &Defs, anchor: Pos) {
    let room = defs.building_id("map_room").expect("map room");
    let def = defs.building(room);
    let (width, height) = def.footprint;
    w.put_building(Footprint::new(anchor, width, height), room, defs);
    w.open_chart_seats(anchor, def.seats);
}

/// Unit `i` holds a Chart seat and has walked in through the door: charting, not still walking.
fn seated(w: &World, i: usize) -> bool {
    matches!(w.units[i].task(), Some(Task::Chart(_))) && w.units[i].inside().is_some()
}

fn ruins(defs: &Defs) -> crate::defs::ResourceId {
    defs.resource_id("lost_ruins").expect("lost_ruins")
}

#[test]
fn a_finished_map_room_opens_one_chart_seat() {
    let (mut w, defs) = setup(Nation::Earth, &GRID, &[at(1, 2)]);
    let seats = defs.building(defs.building_id("map_room").unwrap()).seats;
    assert_eq!(seats, 1);
    place_map_room(&mut w, &defs, at(2, 2));
    assert_eq!(w.jobs().len(), seats as usize);
    assert!(matches!(w.jobs()[0].kind, JobKind::Chart { .. }));
}

#[test]
fn no_seat_is_claimed_with_nothing_within_range_to_chart() {
    let (mut w, defs) = setup(Nation::Earth, &GRID, &[at(1, 2)]);
    place_map_room(&mut w, &defs, at(2, 2));
    for _ in 0..200 {
        w.tick(&defs);
    }
    assert!(w.units[0].task().is_none(), "{:?}", w.units[0].task());
}

#[test]
fn a_chart_seat_loses_to_a_farther_harvest_when_both_are_open() {
    // A Map Room right beside the unit, with something in range to chart, plus a tree far
    // across the map. Chart is the same low priority as Study (#402, docs/design/
    // cartography.md), so the unit should walk all the way to the tree rather than take the
    // seat at its elbow.
    let (mut w, defs) = setup(Nation::Earth, &GRID, &[at(1, 1)]);
    w.buried.insert(at(4, 4), ruins(&defs));
    place_map_room(&mut w, &defs, at(2, 1));
    w.map[at(8, 9)].resource = defs.resource_id("oak_tree");

    let tree = w.designate_harvest(at(8, 9), &defs).unwrap();
    w.tick(&defs);
    assert_eq!(w.job(tree).unwrap().claimed_by, Some(UnitId(0)), "the farther harvest wins");
    assert!(
        w.jobs()
            .iter()
            .filter(|j| matches!(j.kind, JobKind::Chart { .. }))
            .all(|j| j.claimed_by.is_none()),
        "the near seat stays open"
    );
}

#[test]
fn a_settler_charting_adds_progress_at_the_study_modifier_and_it_survives_a_meal() {
    let (mut w, defs) = setup(Nation::Earth, &GRID, &[at(1, 2)]);
    w.buried.insert(at(8, 8), ruins(&defs));
    place_map_room(&mut w, &defs, at(2, 2));
    let anchor = at(2, 2);

    // Walks to the room's door and steps inside before it can add anything.
    run_until(&mut w, &defs, 200, |w| seated(w, 0));
    assert_eq!(w.modifiers().study, 1.0, "Earth has no study bonus yet");

    let before = w.chart_progress(anchor);
    w.tick(&defs);
    assert_eq!(w.chart_progress(anchor) - before, 1.0, "one tick of progress, unmodified");

    // A hungry settler steps out to eat (needs pre-empt a seat, mirroring a study hall's).
    let fish = defs.item_id("fish").unwrap();
    w.ground.insert(at(0, 0), Stack { item: fish, count: 5 });
    w.units[0].needs.hunger = defs.needs.hunger.low;
    let banked = w.chart_progress(anchor);
    w.tick(&defs);
    assert!(matches!(w.units[0].task(), Some(Task::Eat(_))), "{:?}", w.units[0].task());
    assert!(w.units[0].inside().is_none(), "stepped outside to eat");
    assert_eq!(w.chart_progress(anchor), banked, "progress waits on the building, not lost");

    // It goes back once fed, and charting resumes from where it was.
    run_until(&mut w, &defs, 2000, |w| seated(w, 0));
    assert_eq!(w.units[0].inside(), Some(anchor));
    assert!(w.chart_progress(anchor) >= banked);
}

#[test]
fn chart_standing_reads_progress_and_closes_with_nothing_left() {
    use super::ChartStanding;
    let (mut w, defs) = setup(Nation::Earth, &GRID, &[at(1, 2)]);
    let anchor = at(2, 2);
    assert_eq!(w.chart_standing(anchor, &defs), None, "no Map Room yet");
    w.buried.insert(at(8, 8), ruins(&defs));
    place_map_room(&mut w, &defs, anchor);
    assert_eq!(w.chart_standing(anchor, &defs), Some(ChartStanding::Charting { percent: 0 }));

    let def = defs.building(defs.building_id("map_room").unwrap());
    let target = minutes_to_ticks_min1(def.chart.as_ref().unwrap().minutes) as f32;
    w.anchored_mut(anchor).unwrap().chart = Some(target * 0.4);
    assert_eq!(w.chart_standing(anchor, &defs), Some(ChartStanding::Charting { percent: 40 }));
    w.anchored_mut(anchor).unwrap().chart = Some(target);
    assert_eq!(
        w.chart_standing(anchor, &defs),
        Some(ChartStanding::Charting { percent: 99 }),
        "a full chart is found at once, never shown as done"
    );

    w.buried.clear();
    assert_eq!(w.chart_standing(anchor, &defs), Some(ChartStanding::NothingLeft));
}

#[test]
fn the_study_modifier_scales_how_fast_a_settler_charts() {
    // No Map Room yet, so there is nothing to pull the lone unit away from idling while
    // meditation (no prereqs, raises Air's `study` to 1.2) is learned on the passive trickle
    // alone (3/min, data/research.toml; 60 points in 20 minutes).
    let (mut w, defs) = setup(Nation::Air, &GRID, &[at(1, 2)]);
    let meditation = defs.tech_id("meditation").unwrap();
    w.apply(Command::SetResearch { tech: meditation }, &defs).unwrap();
    run_until(&mut w, &defs, 15_000, |w| w.research().is_known(meditation));
    assert_eq!(w.modifiers().study, 1.2);

    // Only now does the Map Room (and something to chart) appear, so the seat can't have been
    // claimed and emptied already.
    w.buried.insert(at(8, 8), ruins(&defs));
    place_map_room(&mut w, &defs, at(2, 2));
    let anchor = at(2, 2);
    run_until(&mut w, &defs, 200, |w| seated(w, 0));
    let before = w.chart_progress(anchor);
    w.tick(&defs);
    assert_eq!(w.chart_progress(anchor) - before, 1.2);
}

#[test]
fn a_finished_chart_uncovers_the_nearest_reachable_ruin_before_a_closer_unreachable_one() {
    // A river wall the full height of the map cuts it in two, with no ford: the right side is
    // unreachable from the room on the left, however close a ruin there sits.
    let (mut w, defs) = setup(Nation::Earth, &GRID, &[at(1, 1)]);
    for y in 0..GRID.len() as i32 {
        w.map[at(5, y)].terrain = Terrain::River;
    }
    place_map_room(&mut w, &defs, at(1, 3));

    let closer_but_unreachable = at(6, 3); // just past the river: nearer, but cut off
    let farther_but_reachable = at(1, 9); // our own side: farther, but a settler gets there
    w.buried.insert(closer_but_unreachable, ruins(&defs));
    w.buried.insert(farther_but_reachable, ruins(&defs));

    run_until(&mut w, &defs, 200, |w| seated(w, 0));
    run_until(&mut w, &defs, 20_000, |w| !w.chart_events().is_empty());
    let ev = w.chart_events()[0];
    match ev.kind {
        ChartEventKind::Found { pos, reachable } => {
            assert_eq!(pos, farther_but_reachable, "the reachable one wins despite being farther");
            assert!(reachable);
        }
        other => panic!("expected a find, got {other:?}"),
    }
}

#[test]
fn a_ruin_under_something_else_is_skipped() {
    let (mut w, defs) = setup(Nation::Earth, &GRID, &[at(1, 1)]);
    place_map_room(&mut w, &defs, at(1, 3));
    let blocked = at(4, 4);
    let open = at(7, 7);
    w.buried.insert(blocked, ruins(&defs));
    w.buried.insert(open, ruins(&defs));
    // A ground stack sits on the closer ruin: not free to uncover (`free_to_uncover`).
    w.ground.insert(blocked, Stack { item: defs.item_id("wood").unwrap(), count: 1 });

    run_until(&mut w, &defs, 200, |w| seated(w, 0));
    run_until(&mut w, &defs, 20_000, |w| !w.chart_events().is_empty());
    assert_eq!(
        w.chart_events()[0].kind,
        ChartEventKind::Found { pos: open, reachable: true },
        "the blocked ruin is skipped"
    );
    assert_eq!(w.map[blocked].resource, None, "left buried");
}

#[test]
fn a_finished_chart_is_remembered_across_a_save_without_its_room_or_its_ruins() {
    let (mut w, defs) = setup(Nation::Earth, &GRID, &[at(1, 1)]);
    place_map_room(&mut w, &defs, at(1, 3));
    let only = at(7, 7);
    w.buried.insert(only, ruins(&defs));
    assert!(!w.has_charted());
    assert!(!w.charted_at(only));

    run_until(&mut w, &defs, 200, |w| seated(w, 0));
    run_until(&mut w, &defs, 20_000, |w| !w.chart_events().is_empty());
    assert!(w.has_charted());
    assert!(w.charted_at(only));
    assert!(!w.charted_at(at(2, 2)), "only the charted ruin's own tile");

    // The ruin is recovered and the Map Room taken down: the chart still happened.
    w.map[only].resource = None;
    let room = w.map[at(1, 3)].building.unwrap().key();
    w.remove_building(room, &defs).unwrap();
    assert!(w.buildings().next().is_none());
    let text = w.save(&defs).unwrap();
    let loaded = World::load(&text, &defs).unwrap();
    assert!(loaded.buildings().next().is_none());
    assert!(loaded.has_charted());
    assert!(loaded.charted_at(only));
}

#[test]
fn every_ruin_charted_closes_the_seat_and_logs_none_left() {
    let (mut w, defs) = setup(Nation::Earth, &GRID, &[at(1, 1)]);
    place_map_room(&mut w, &defs, at(1, 3));
    let only = at(7, 7);
    w.buried.insert(only, ruins(&defs));

    run_until(&mut w, &defs, 200, |w| seated(w, 0));
    run_until(&mut w, &defs, 20_000, |w| !w.chart_events().is_empty());
    assert_eq!(w.chart_events()[0].kind, ChartEventKind::Found { pos: only, reachable: true });
    assert_eq!(w.map[only].resource, Some(ruins(&defs)));

    // Nothing left: the seat opens again, but a settler who takes it immediately finds nothing
    // to chart and gives it straight back.
    for _ in 0..200 {
        w.tick(&defs);
    }
    assert!(w.units[0].task().is_none(), "{:?}", w.units[0].task());
    assert!(w.jobs().iter().all(|j| j.claimed_by.is_none()));
}

#[test]
fn charting_is_deterministic() {
    let run = || {
        let (mut w, defs) = setup(Nation::Water, &GRID, &[at(1, 1), at(8, 8)]);
        place_map_room(&mut w, &defs, at(1, 3));
        w.buried.insert(at(5, 5), ruins(&defs));
        w.buried.insert(at(9, 2), ruins(&defs));
        for _ in 0..20_000 {
            w.tick(&defs);
        }
        (w.chart_events().to_vec(), w.map.iter().filter(|(_, t)| t.resource.is_some()).count())
    };
    assert_eq!(run(), run());
}
