use super::*;
use crate::defs::{BiomeId, Terrain};
use crate::geom::Grid;
use crate::jobs::JobKind;
use crate::mapgen::Tile;
use crate::nation::Nation;
use crate::world::{Command, Task};

fn at(x: i32, y: i32) -> Pos {
    Pos::new(x, y)
}

/// A wide strip of plain, walkable land with `units` on it.
fn setup(units: &[Pos]) -> (World, Defs) {
    let defs = Defs::bundled();
    let map = Grid::from_fn(32, 6, |_| Tile {
        elevation: 100,
        temperature: 128,
        moisture: 128,
        terrain: Terrain::Land,
        biome: BiomeId(0),
        resource: None,
        building: None,
    });
    (World::with_map(1, map, Nation::Earth, units, &defs), defs)
}

/// Finishes `building` with its anchor on `anchor`, the way `run_build` does (seats, craft jobs
/// and the Archive's sync included).
fn raise(w: &mut World, defs: &Defs, building: &str, anchor: Pos) {
    let id = defs.building_id(building).unwrap();
    let (fw, fh) = defs.building(id).footprint;
    w.raise(crate::geom::Footprint::new(anchor, fw, fh), id, defs);
}

fn study_jobs_at(w: &World, anchor: Pos) -> u32 {
    w.jobs().iter().filter(|j| j.kind == JobKind::Study { pos: anchor }).count() as u32
}

fn hall_seats(defs: &Defs) -> u32 {
    defs.building(defs.building_id("study_hall").unwrap()).seats
}

fn copy(defs: &Defs) -> ItemId {
    defs.item_id("scroll_copy").unwrap()
}

/// Puts `count` copies in a stockpile tile far from everything.
fn stock_copies(w: &mut World, defs: &Defs, count: u32) {
    let p = at(30, 5);
    w.zones.insert(p);
    w.stored.insert(p, Stack { item: copy(defs), count });
}

fn ground_copies(w: &World, defs: &Defs) -> u32 {
    w.ground_items().filter(|(_, s)| s.item == copy(defs)).map(|(_, s)| s.count).sum()
}

fn passive(defs: &Defs) -> u64 {
    defs.research.passive_millis_per_tick
}

#[test]
fn an_archive_adds_its_seats_to_the_halls_in_reach_and_two_do_not_stack() {
    let (mut w, defs) = setup(&[at(0, 5)]);
    let base = hall_seats(&defs);
    raise(&mut w, &defs, "study_hall", at(2, 2));
    raise(&mut w, &defs, "study_hall", at(24, 2));
    raise(&mut w, &defs, "map_room", at(14, 2));
    assert_eq!(w.seats_at(at(2, 2), &defs), base);
    raise(&mut w, &defs, "archive", at(8, 2));
    // The hall 5 tiles away is in reach (radius 6); the one 14 away is not.
    assert_eq!(w.seats_at(at(2, 2), &defs), base + 2);
    assert_eq!(study_jobs_at(&w, at(2, 2)), base + 2, "the seats opened");
    assert_eq!(w.seats_at(at(24, 2), &defs), base);
    assert_eq!(study_jobs_at(&w, at(24, 2)), base);
    // The Map Room is another building: its seat stays one.
    assert_eq!(w.seats_at(at(14, 2), &defs), 1);
    // A second Archive in reach of the same hall adds nothing.
    raise(&mut w, &defs, "archive", at(4, 4));
    assert_eq!(w.seats_at(at(2, 2), &defs), base + 2);
    assert_eq!(study_jobs_at(&w, at(2, 2)), base + 2);
    // A hall raised later opens with the bonus at once.
    raise(&mut w, &defs, "study_hall", at(10, 4));
    assert_eq!(study_jobs_at(&w, at(10, 4)), base + 2);
    // A hall's capacity follows its seats, so the extra seats can be sat in.
    w.sync_seats(&defs);
    assert_eq!(study_jobs_at(&w, at(2, 2)), base + 2, "syncing again changes nothing");
}

#[test]
fn taking_the_last_archive_down_closes_its_seats_and_steps_scholars_out() {
    let (mut w, defs) = setup(&[at(2, 4), at(3, 4), at(4, 4), at(5, 4)]);
    let base = hall_seats(&defs);
    raise(&mut w, &defs, "study_hall", at(2, 2));
    raise(&mut w, &defs, "archive", at(8, 2));
    assert_eq!(study_jobs_at(&w, at(2, 2)), base + 2);
    w.apply(Command::SetResearch { tech: defs.tech_id("shelter").unwrap() }, &defs).unwrap();
    for _ in 0..200 {
        w.tick(&defs);
    }
    assert!(
        w.jobs()
            .iter()
            .filter(|j| j.kind == JobKind::Study { pos: at(2, 2) })
            .all(|j| j.claimed_by.is_some()),
        "every seat is held"
    );
    let key = w.map[at(8, 2)].building.unwrap().key();
    w.remove_building(key, &defs).unwrap();
    assert_eq!(w.seats_at(at(2, 2), &defs), base);
    assert_eq!(study_jobs_at(&w, at(2, 2)), base, "the extra seats closed");
    let held = w.units.iter().filter(|u| matches!(u.task(), Some(Task::Study(_)))).count();
    assert_eq!(held as u32, base, "the scholars of closed seats stepped out");
    w.validate(&defs).unwrap();
    w.check_invariants(&defs).unwrap();
}

#[test]
fn closing_seats_takes_unclaimed_ones_first() {
    let (mut w, defs) = setup(&[at(2, 4)]);
    let base = hall_seats(&defs);
    raise(&mut w, &defs, "study_hall", at(2, 2));
    raise(&mut w, &defs, "archive", at(8, 2));
    let first = w.jobs().iter().find(|j| matches!(j.kind, JobKind::Study { .. })).unwrap().id;
    // The unit holds the lowest-id seat; the Archive's two new seats are the highest ids anyway,
    // so hold the highest one instead to show claimed seats outlast unclaimed ones.
    let last = w.jobs().iter().rfind(|j| matches!(j.kind, JobKind::Study { .. })).unwrap().id;
    assert_ne!(first, last);
    let j = w.job_index(last).unwrap();
    w.jobs[j].claimed_by = Some(w.units[0].id);
    w.units[0].job_task = Some(Task::Study(last));
    let key = w.map[at(8, 2)].building.unwrap().key();
    w.remove_building(key, &defs).unwrap();
    assert_eq!(study_jobs_at(&w, at(2, 2)), base);
    assert!(w.job_index(last).is_some(), "the held seat stays, an unclaimed one went");
    assert_eq!(w.units[0].job_task, Some(Task::Study(last)));
}

#[test]
fn idle_knowledge_above_the_cap_becomes_copies_with_an_archive() {
    let (mut w, defs) = setup(&[at(0, 5)]);
    raise(&mut w, &defs, "archive", at(8, 2));
    assert!(w.research().queue().is_empty());
    w.research.bank_for_test(400_000);
    w.tick(&defs);
    let cap = defs.research.idle_knowledge_cap_millis;
    assert_eq!(w.research().pool_millis(), cap, "the pool is still capped");
    let trimmed = 400_000 + passive(&defs) - cap;
    // 150 points make a copy: two of them, and the rest is counted toward the next.
    assert_eq!(w.research().scribe_millis(), trimmed - 300_000);
    assert_eq!(w.scrolls_copied(), 2);
    assert_eq!(ground_copies(&w, &defs), 2, "dropped beside the Archive");
    for (pos, s) in w.ground_items() {
        assert!(pos.chebyshev(at(8, 2)) <= 3, "{pos:?}: {s:?}");
    }
    assert!(w.jobs().iter().any(|j| matches!(j.kind, JobKind::Haul { .. })), "a haul job each");
    // Copies are not scrolls read.
    assert_eq!(w.facts().scrolls_read, 0);
}

#[test]
fn without_an_archive_the_cap_trims_and_nothing_changes() {
    let (mut w, defs) = setup(&[at(0, 5)]);
    w.research.bank_for_test(400_000);
    w.tick(&defs);
    assert_eq!(w.research().pool_millis(), defs.research.idle_knowledge_cap_millis);
    assert_eq!(w.research().scribe_millis(), 0);
    assert_eq!(w.scrolls_copied(), 0);
    assert_eq!(ground_copies(&w, &defs), 0);
    assert!(!w.scribing_on(&defs));
}

#[test]
fn a_target_keeps_the_overflow_from_scribing() {
    let (mut w, defs) = setup(&[at(0, 5)]);
    raise(&mut w, &defs, "archive", at(8, 2));
    w.apply(Command::SetResearch { tech: defs.tech_id("shelter").unwrap() }, &defs).unwrap();
    assert!(!w.scribing_on(&defs));
    w.research.bank_for_test(400_000);
    w.tick(&defs);
    assert_eq!(w.research().scribe_millis(), 0, "trimmed toward the target, as today");
}

#[test]
fn scribing_pauses_at_the_stock_limit_and_the_seats_free_up() {
    let (mut w, defs) = setup(&[at(1, 2)]);
    raise(&mut w, &defs, "study_hall", at(2, 2));
    raise(&mut w, &defs, "archive", at(8, 2));
    w.tick(&defs);
    assert!(
        matches!(w.units[0].task(), Some(Task::Study(_))),
        "with an Archive, a seat is claimed with an empty queue: {:?}",
        w.units[0].task()
    );
    // The stores fill to the limit: scribing is off, the scholar leaves and the overflow is lost.
    stock_copies(&mut w, &defs, 12);
    assert!(!w.scribing_on(&defs));
    w.tick(&defs);
    assert!(w.units[0].task().is_none() || !matches!(w.units[0].task(), Some(Task::Study(_))));
    w.research.bank_for_test(400_000);
    w.tick(&defs);
    assert_eq!(w.research().scribe_millis(), 0);
    assert_eq!(w.scrolls_copied(), 0);
    // One copy leaves stores: scribing is on again.
    stock_copies(&mut w, &defs, 11);
    assert!(w.scribing_on(&defs));
    w.research.bank_for_test(400_000);
    w.tick(&defs);
    assert!(w.scrolls_copied() >= 1);
}

#[test]
fn the_drop_stops_at_the_limit_and_keeps_what_it_cannot_spend() {
    let (mut w, defs) = setup(&[at(0, 5)]);
    raise(&mut w, &defs, "archive", at(8, 2));
    stock_copies(&mut w, &defs, 11);
    w.research.bank_for_test(1_000_000);
    w.tick(&defs);
    assert_eq!(w.scrolls_copied(), 1, "one more copy fills the limit");
    assert!(w.research().scribe_millis() > 150_000, "the rest waits");
    // Taking a copy away lets it go on, with nothing lost.
    let before = w.research().scribe_millis();
    stock_copies(&mut w, &defs, 10);
    w.tick(&defs);
    assert!(w.scrolls_copied() >= 2);
    assert!(w.research().scribe_millis() < before);
}

#[test]
fn clearing_the_target_needs_an_archive_and_keeps_the_banked_knowledge() {
    let (mut w, defs) = setup(&[at(0, 5)]);
    let target = defs.tech_id("shelter").unwrap();
    w.apply(Command::SetResearch { tech: target }, &defs).unwrap();
    let refused = w.apply(Command::ClearResearch, &defs);
    assert_eq!(
        refused,
        Err(crate::world::CommandError::Research(crate::research::ResearchError::NeedsArchive))
    );
    assert_eq!(crate::research::ResearchError::NeedsArchive.to_string(), "needs an Archive");
    assert_eq!(w.research().queue(), [target], "refused: nothing changed");
    assert!(w.command_log().iter().all(|r| r.command != Command::ClearResearch));

    raise(&mut w, &defs, "archive", at(8, 2));
    w.research.bank_for_test(20_000);
    w.apply(Command::ClearResearch, &defs).unwrap();
    let cap = defs.research.idle_knowledge_cap_millis;
    assert!(w.research().queue().is_empty());
    assert_eq!(w.research().pool_millis(), cap);
    assert_eq!(w.research().scribe_millis(), 20_000 - cap);
    assert!(w.scribing_on(&defs));
    assert_eq!(w.command_log().last().unwrap().command, Command::ClearResearch);
}

#[test]
fn a_copy_is_never_read_as_a_scroll() {
    let defs = Defs::bundled();
    let item = defs.item(copy(&defs));
    assert_eq!(item.knowledge, None);
    assert_eq!(item.stands_for, None);
    let (mut w, defs) = setup(&[at(0, 5)]);
    raise(&mut w, &defs, "archive", at(8, 2));
    raise(&mut w, &defs, "study_hall", at(2, 2));
    w.research.bank_for_test(2_000_000);
    for _ in 0..600 {
        w.tick(&defs);
    }
    assert!(w.scrolls_copied() > 0);
    assert_eq!(w.facts().scrolls_read, 0, "copies never count toward the scrolls sign");
}

#[test]
fn scribing_is_deterministic_and_survives_a_save() {
    let run = || {
        let (mut w, defs) = setup(&[at(1, 2), at(3, 4)]);
        raise(&mut w, &defs, "study_hall", at(2, 2));
        raise(&mut w, &defs, "archive", at(8, 2));
        w.research.bank_for_test(300_000);
        for _ in 0..2000 {
            w.tick(&defs);
        }
        (w, defs)
    };
    let (a, defs) = run();
    let (b, _) = run();
    assert_eq!(a.research().scribe_millis(), b.research().scribe_millis());
    assert_eq!(a.scrolls_copied(), b.scrolls_copied());
    assert!(a.scrolls_copied() >= 2);
    assert_eq!(a.save(&defs).unwrap(), b.save(&defs).unwrap());
    // The counter and tally survive a save and load.
    let loaded = World::load(&a.save(&defs).unwrap(), &defs).expect("loads");
    assert_eq!(loaded.research().scribe_millis(), a.research().scribe_millis());
    assert_eq!(loaded.scrolls_copied(), a.scrolls_copied());
}

#[test]
fn the_seats_an_archive_adds_can_be_sat_in() {
    let door = at(2, 4);
    let (mut w, defs) = setup(&[door, door, door, door]);
    let base = hall_seats(&defs);
    assert!(base < 4);
    raise(&mut w, &defs, "study_hall", at(2, 2));
    let enter = |w: &mut World| (0..4).filter(|&i| w.enter_building(i, at(2, 2), &defs)).count();
    assert_eq!(enter(&mut w) as u32, base, "a hall seats its own seats");
    for u in &mut w.units {
        u.inside = None;
    }
    raise(&mut w, &defs, "archive", at(8, 2));
    assert_eq!(enter(&mut w) as u32, base + 2, "and the Archive's");
}
