use std::collections::HashMap;

use super::*;
use crate::building::{BuildingDef, BuildingId, PowerDef};
use crate::defs::{BiomeId, Terrain};
use crate::geom::Grid;
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

const GRID: [&str; 5] = [".........", ".........", ".........", ".........", "........."];

/// Puts a finished study hall at `anchor` (skipping the construction job itself, which
/// `build.rs` already covers), covering its real footprint (#214: a solid 2x2), and opens its
/// seats, the way `run_build` does the moment one completes.
fn place_study_hall(w: &mut World, defs: &Defs, anchor: Pos) {
    let hall = defs.building_id("study_hall").expect("study hall");
    let def = defs.building(hall);
    let (width, height) = def.footprint;
    w.put_building(crate::geom::Footprint::new(anchor, width, height), hall, defs);
    w.open_study_seats(anchor, def.seats);
}

/// Queues `id` so a seat is worth claiming (#284: nothing is claimed with an empty queue).
fn start_research(w: &mut World, defs: &Defs, id: &str) {
    w.apply(Command::SetResearch { tech: defs.tech_id(id).unwrap() }, defs).unwrap();
}

/// Unit `i` holds a seat and has walked in through the door (#227): studying, rather than still
/// on its way there.
fn seated(w: &World, i: usize) -> bool {
    matches!(w.units[i].task(), Some(Task::Study(_))) && w.units[i].inside().is_some()
}

#[test]
fn a_finished_study_hall_opens_one_job_per_seat() {
    let (mut w, defs) = setup(Nation::Earth, &GRID, &[at(1, 2)]);
    let seats = defs.building(defs.building_id("study_hall").unwrap()).seats;
    place_study_hall(&mut w, &defs, at(2, 2));
    assert_eq!(w.jobs().len(), seats as usize);
    start_research(&mut w, &defs, "earth_shaping");

    // The unit starts right beside the hall's solid 2x2 footprint, so it claims a seat on its
    // very first tick with no travel.
    w.tick(&defs);
    assert!(matches!(w.units[0].task(), Some(Task::Study(_))), "{:?}", w.units[0].task());
}

#[test]
fn no_seat_is_claimed_with_nothing_queued_to_research() {
    let (mut w, defs) = setup(Nation::Earth, &GRID, &[at(1, 2)]);
    place_study_hall(&mut w, &defs, at(2, 2));
    for _ in 0..200 {
        w.tick(&defs);
    }
    assert!(w.research().queue().is_empty());
    assert!(w.units[0].task().is_none(), "{:?}", w.units[0].task());
}

#[test]
fn a_scholar_leaves_its_seat_once_the_queue_empties() {
    let (mut w, defs) = setup(Nation::Earth, &GRID, &[at(1, 2)]);
    place_study_hall(&mut w, &defs, at(2, 2));
    start_research(&mut w, &defs, "fire_making");
    w.tick(&defs);
    assert!(matches!(w.units[0].task(), Some(Task::Study(_))));

    // 25 points at 3/min passive plus 9/min studying is well under a minute. `Research::tick`
    // runs before units act, so the scholar notices the now-empty queue the very tick
    // `fire_making` completes and leaves (Task::Study ends, not abandons: no retry delay).
    run_until(&mut w, &defs, 2000, |w| w.research().is_known(defs.tech_id("fire_making").unwrap()));
    assert!(w.units[0].task().is_none(), "{:?}", w.units[0].task());
    assert_eq!(w.jobs().iter().filter(|j| matches!(j.kind, JobKind::Study { .. })).count(), 2);
    assert!(w.jobs().iter().all(|j| j.claimed_by.is_none()), "the seat is open again");
}

#[test]
fn a_scholar_studying_adds_the_expected_knowledge_each_tick() {
    let (mut w, defs) = setup(Nation::Earth, &GRID, &[at(1, 2)]);
    place_study_hall(&mut w, &defs, at(2, 2));
    // `earth_shaping` needs no further prereq (Earth starts with `stoneworking`), so the pool's
    // cap while queued (#284) is its full 60 points, well above what this test banks.
    start_research(&mut w, &defs, "earth_shaping");
    // Walks to the hall's door and steps inside (#227) before it can add anything.
    run_until(&mut w, &defs, 200, |w| seated(w, 0));
    assert_eq!(w.modifiers().study, 1.0, "Earth has no study bonus yet");

    let passive = defs.research.passive_millis_per_tick;
    let study = defs.research.study_millis_per_tick;
    let ticks = 1000; // comfortably under earth_shaping's 60,000-milli cap at this rate
    let before = w.research().pool_millis();
    for _ in 0..ticks {
        w.tick(&defs);
    }
    let gained = w.research().pool_millis() - before;
    assert_eq!(gained, ticks * (passive + study));
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
        unlocked_by: None,
        seat_upgrades: Vec::new(),
        endeavor: false,
        beside: None,
    });
    id
}

/// A seat powered by a source in reach adds `boost` times the knowledge an unpowered one would
/// (issue #428), deterministically: `boost` is 2.0 here, which divides evenly into the integer
/// millis `world/study.rs` scales, so the gain is exact, not just close. The settlement-wide
/// passive trickle (`Research::tick`) isn't the seat's own knowledge, so it's subtracted out
/// before comparing.
#[test]
fn a_powered_seat_adds_boost_times_the_knowledge_of_an_unpowered_one() {
    const TICKS: u64 = 1000;
    let seat_gain = |source: Option<(u32, f32)>| {
        let mut defs = Defs::bundled();
        let mut w = World::with_map(1, arena(&GRID), Nation::Earth, &[at(1, 2)], &defs);
        place_study_hall(&mut w, &defs, at(2, 2));
        if let Some((radius, boost)) = source {
            let id = test_source(&mut defs, radius, boost);
            w.put_building(crate::geom::Footprint::single(at(8, 4)), id, &defs);
        }
        start_research(&mut w, &defs, "earth_shaping");
        run_until(&mut w, &defs, 200, |w| seated(w, 0));
        let before = w.research().pool_millis();
        for _ in 0..TICKS {
            w.tick(&defs);
        }
        let gained = w.research().pool_millis() - before;
        gained - TICKS * defs.research.passive_millis_per_tick
    };

    let unpowered = seat_gain(None);
    let powered = seat_gain(Some((6, 2.0)));
    assert_eq!(unpowered, TICKS * Defs::bundled().research.study_millis_per_tick);
    assert_eq!(powered, unpowered * 2, "a boost of 2.0 doubles the seat's own knowledge exactly");
}

#[test]
fn a_hungry_scholar_leaves_to_eat_and_re_enters_once_fed() {
    let (mut w, defs) = setup(Nation::Earth, &GRID, &[at(1, 2)]);
    place_study_hall(&mut w, &defs, at(2, 2));
    start_research(&mut w, &defs, "earth_shaping");
    run_until(&mut w, &defs, 200, |w| seated(w, 0));
    let anchor = w.units[0].inside().expect("seated");

    // Food within reach, so the unit can actually act on its hunger.
    let fish = defs.item_id("fish").unwrap();
    w.ground.insert(at(0, 0), Stack { item: fish, count: 5 });
    w.units[0].needs.hunger = defs.needs.hunger.low;

    let before = w.research().pool_millis();
    w.tick(&defs);
    assert!(matches!(w.units[0].task(), Some(Task::Eat(_))), "{:?}", w.units[0].task());
    // The need pre-empts the seat (#226): it steps out before anything else, in case the door
    // is blocked by the time it's ready to come back.
    assert!(w.units[0].inside().is_none(), "stepped outside to eat");
    // Needs pre-empt the seat before it can add anything this tick: only the passive trickle
    // moves the pool.
    assert_eq!(w.research().pool_millis() - before, defs.research.passive_millis_per_tick);

    // It goes back to its seat, walking in through the door again, once fed.
    run_until(&mut w, &defs, 2000, |w| seated(w, 0));
    assert_eq!(w.units[0].inside(), Some(anchor), "re-entered the same hall");
}

#[test]
fn the_study_modifier_scales_how_fast_a_scholar_adds_knowledge() {
    let (mut w, defs) = setup(Nation::Air, &GRID, &[at(1, 2)]);
    place_study_hall(&mut w, &defs, at(2, 2));
    // `meditation` has no prereqs and raises Air's `study` modifier to 1.2 (data/research.toml).
    let meditation = defs.tech_id("meditation").unwrap();
    start_research(&mut w, &defs, "meditation");
    // Walks to the hall's door and steps inside (#227) before it can add anything.
    run_until(&mut w, &defs, 200, |w| seated(w, 0));

    let passive = defs.research.passive_millis_per_tick;
    let base_study = defs.research.study_millis_per_tick;
    let before = w.research().pool_millis();
    w.tick(&defs);
    assert_eq!(w.research().pool_millis() - before, passive + base_study);

    // The scholar's own output, plus the passive trickle, pays for it.
    run_until(&mut w, &defs, 10_000, |w| w.research().is_known(meditation));
    assert_eq!(w.modifiers().study, 1.2);

    // Something still queued (#284 would otherwise send the scholar back to idle): Air's next
    // branch tech. Meditation's completion left the queue briefly empty, so the scholar already
    // stepped out (#226) by the time `is_known` above turned true; it walks back in once there's
    // something to study toward again.
    start_research(&mut w, &defs, "gliders");
    run_until(&mut w, &defs, 200, |w| seated(w, 0));

    let before = w.research().pool_millis();
    w.tick(&defs);
    let boosted_study = (base_study as f32 * 1.2).round() as u64;
    assert_ne!(boosted_study, base_study);
    assert_eq!(w.research().pool_millis() - before, passive + boosted_study);
}

// ---- ancient scrolls (#24) ----

use crate::jobs::ItemId;
use crate::research::MILLIS_PER_POINT;

fn scroll_of(defs: &Defs) -> ItemId {
    defs.item_id("ancient_scroll").expect("ancient_scroll item")
}

/// Ticks until knowledge arrives in a lump (one tick adds more than the passive trickle and a
/// seated scholar can), and returns what that tick added beyond the passive trickle.
fn lump_sum(w: &mut World, defs: &Defs, limit: u64) -> u64 {
    let passive = defs.research.passive_millis_per_tick;
    for _ in 0..limit {
        let before = w.research().pool_millis();
        w.tick(defs);
        let gained = w.research().pool_millis() - before - passive;
        if gained > defs.research.study_millis_per_tick * 2 {
            return gained;
        }
    }
    panic!("no lump of knowledge in {limit} ticks");
}

#[test]
fn the_scroll_is_a_readable_item_worth_forty_points_in_data() {
    let defs = Defs::bundled();
    assert_eq!(defs.item(scroll_of(&defs)).knowledge, Some(40));
    assert_eq!(defs.item(defs.item_id("wood").unwrap()).knowledge, None);
    assert_eq!(defs.item(defs.item_id("fish").unwrap()).knowledge, None);
    let res = defs.resource_id("ancient_scroll").expect("ancient_scroll resource");
    let harvest = defs.harvest(res).expect("a scroll can be recovered");
    assert_eq!(harvest.item, scroll_of(&defs));
    assert_eq!(harvest.regrow, None, "a scroll is read once");
}

#[test]
fn a_scroll_hauled_to_a_study_hall_is_consumed_for_its_knowledge() {
    let (mut w, defs) = setup(Nation::Earth, &GRID, &[at(0, 0)]);
    place_study_hall(&mut w, &defs, at(7, 3));
    let scroll = scroll_of(&defs);
    w.place_ground(at(1, 1), Stack { item: scroll, count: 1 });
    assert_eq!(w.ground_items().count(), 1);

    assert_eq!(lump_sum(&mut w, &defs, 2000), 40 * MILLIS_PER_POINT);
    // Used up: not on the ground, in a stockpile or in anyone's hands.
    assert_eq!(w.ground_items().count(), 0);
    assert_eq!(w.stored_total(scroll), 0);
    assert!(w.units[0].carrying.is_none());
    // Delivered at the hall, from beside it: hauling never enters (#227).
    assert!(w.units[0].inside().is_none());
}

#[test]
fn a_whole_stack_of_scrolls_is_read_in_one_delivery() {
    let (mut w, defs) = setup(Nation::Fire, &GRID, &[at(0, 0)]);
    place_study_hall(&mut w, &defs, at(7, 3));
    w.place_ground(at(1, 1), Stack { item: scroll_of(&defs), count: 3 });
    assert_eq!(lump_sum(&mut w, &defs, 2000), 3 * 40 * MILLIS_PER_POINT);
    assert_eq!(w.tallies().scrolls_read(), 3, "each scroll in the stack counts (#233)");
}

#[test]
fn a_scrolls_knowledge_comes_from_data_not_code() {
    let mut defs = Defs::bundled();
    let scroll = scroll_of(&defs);
    defs.items[scroll.0 as usize].knowledge = Some(75);
    let mut w = World::with_map(1, arena(&GRID), Nation::Earth, &[at(0, 0)], &defs);
    place_study_hall(&mut w, &defs, at(7, 3));
    w.place_ground(at(1, 1), Stack { item: scroll, count: 1 });
    assert_eq!(lump_sum(&mut w, &defs, 2000), 75 * MILLIS_PER_POINT);
}

#[test]
fn a_scroll_is_never_stockpiled_and_waits_for_a_study_hall() {
    let (mut w, defs) = setup(Nation::Earth, &GRID, &[at(0, 0)]);
    let scroll = scroll_of(&defs);
    w.apply(Command::DesignateStockpile { pos: at(0, 4) }, &defs).unwrap();
    w.place_ground(at(1, 1), Stack { item: scroll, count: 1 });

    let before = w.research().pool_millis();
    for _ in 0..600 {
        w.tick(&defs);
    }
    assert_eq!(w.stored_total(scroll), 0, "scrolls are read, not stored");
    assert_eq!(w.ground_items().count(), 1, "the scroll stays where it lies");
    assert!(w.units[0].carrying.is_none());
    assert_eq!(w.research().pool_millis() - before, 600 * defs.research.passive_millis_per_tick);
    assert_eq!(w.tallies().scrolls_read(), 0, "a scroll counts once it is read, not before");

    // Once a hall stands, the scroll is hauled over and read.
    place_study_hall(&mut w, &defs, at(7, 3));
    assert_eq!(lump_sum(&mut w, &defs, 2000), 40 * MILLIS_PER_POINT);
    assert_eq!(w.ground_items().count(), 0);
}

#[test]
fn a_scroll_on_the_map_is_recovered_hauled_and_read_end_to_end() {
    let defs = Defs::bundled();
    let mut map = arena(&GRID);
    let scroll_tile = at(2, 1);
    map[scroll_tile].resource = defs.resource_id("ancient_scroll");
    let mut w = World::with_map(1, map, Nation::Water, &[at(0, 0)], &defs);
    place_study_hall(&mut w, &defs, at(7, 3));

    w.apply(Command::DesignateHarvest { pos: scroll_tile }, &defs).unwrap();
    assert_eq!(lump_sum(&mut w, &defs, 3000), 40 * MILLIS_PER_POINT);
    assert!(w.map[scroll_tile].resource.is_none(), "the scroll is taken off the map");
    // Both tallies saw it: recovered off the map, then read (#233).
    assert_eq!(w.tallies().harvested(scroll_of(&defs)), 1);
    assert_eq!(w.tallies().scrolls_read(), 1);
}

#[test]
fn reading_scrolls_is_deterministic() {
    let run = || {
        let (mut w, defs) = setup(Nation::Air, &GRID, &[at(0, 0), at(8, 4)]);
        place_study_hall(&mut w, &defs, at(7, 3));
        w.place_ground(at(1, 1), Stack { item: scroll_of(&defs), count: 1 });
        for _ in 0..1500 {
            w.tick(&defs);
        }
        w.research().pool_millis()
    };
    assert_eq!(run(), run());
}

#[test]
fn two_scholars_fill_a_solid_two_by_two_hall_while_a_third_waits() {
    let (mut w, mut defs) = setup(Nation::Earth, &GRID, &[at(0, 0), at(8, 4), at(1, 4)]);
    let hall = defs.building_id("study_hall").unwrap();
    let def = &mut defs.buildings[hall.0 as usize];
    def.footprint = (2, 2);
    def.passable = false;
    let seats = def.seats;
    assert_eq!(seats, 2, "exactly two seats to fill, with one scholar left over");
    let foot = crate::geom::Footprint::new(at(4, 1), 2, 2);
    w.put_building(foot, hall, &defs);
    w.open_study_seats(foot.anchor, seats);
    start_research(&mut w, &defs, "earth_shaping");

    run_until(&mut w, &defs, 2000, |w| seated(w, 0) && seated(w, 1));
    assert_eq!(
        w.occupants(foot.anchor).collect::<Vec<_>>(),
        [w.units[0].id, w.units[1].id],
        "both scholars are inside, in id order"
    );
    for u in &w.units[..2] {
        assert_eq!(foot.distance(u.pos), 1, "{} studies from the door, {:?}", u.name, u.pos);
        assert!(w.units_at(u.pos).next().is_none(), "hidden from the map while inside");
    }
    // No third seat, and nothing else for the third scholar to do either.
    assert!(w.units[2].task().is_none(), "{:?}", w.units[2].task());
    assert!(w.units[2].inside().is_none());

    let before = w.research().pool_millis();
    for _ in 0..100 {
        w.tick(&defs);
    }
    let passive = defs.research.passive_millis_per_tick * 100;
    assert!(w.research().pool_millis() - before > passive, "both scholars add knowledge");
}

#[test]
fn study_seats_filled_counts_only_scholars_already_inside() {
    let (mut w, defs) = setup(Nation::Earth, &GRID, &[at(1, 2)]);
    place_study_hall(&mut w, &defs, at(2, 2));
    assert_eq!(w.study_seats_filled(), 0, "nothing queued, nobody seated");
    start_research(&mut w, &defs, "earth_shaping");
    // One unit starts right beside the door and claims a seat before it has entered.
    w.tick(&defs);
    assert!(!seated(&w, 0), "still walking in");
    assert_eq!(w.study_seats_filled(), 0, "claimed, but not inside yet");
    run_until(&mut w, &defs, 200, |w| seated(w, 0));
    assert_eq!(w.study_seats_filled(), 1);
}

fn study_jobs_at(w: &World, anchor: Pos) -> usize {
    w.jobs().iter().filter(|j| j.kind == JobKind::Study { pos: anchor }).count()
}

/// `writing` adds 2 seats to every study hall (docs/design/horizons-phase-1.md): `seats_at`
/// counts them once it is known, and learning it opens them at a hall that already stands.
#[test]
fn learning_writing_opens_two_more_seats_at_every_hall() {
    let (mut w, defs) = setup(Nation::Earth, &GRID, &[at(0, 0)]);
    let base = defs.building(defs.building_id("study_hall").unwrap()).seats;
    place_study_hall(&mut w, &defs, at(2, 2));
    assert_eq!(w.seats_at(at(2, 2), &defs), base);
    assert_eq!(w.seats_at(at(0, 4), &defs), 0, "no building there");
    let writing = defs.tech_id("writing").unwrap();
    for &p in &defs.tech(writing).prereqs.clone() {
        for &q in &defs.tech(p).prereqs.clone() {
            w.research.grant(q);
        }
        w.research.grant(p);
    }
    w.apply(Command::SetResearch { tech: writing }, &defs).unwrap();
    w.research.bank_for_test(10_000_000);
    w.tick(&defs);
    assert!(w.research().is_known(writing));
    assert_eq!(w.seats_at(at(2, 2), &defs), base + 2);
    assert_eq!(study_jobs_at(&w, at(2, 2)) as u32, base + 2);
    // A hall raised afterwards opens the full count at once.
    let hall = defs.building_id("study_hall").unwrap();
    let (fw, fh) = defs.building(hall).footprint;
    w.raise(crate::geom::Footprint::new(at(5, 2), fw, fh), hall, &defs);
    assert_eq!(study_jobs_at(&w, at(5, 2)) as u32, base + 2);
    // Syncing again changes nothing.
    w.sync_seats(&defs);
    assert_eq!(study_jobs_at(&w, at(2, 2)) as u32, base + 2);
}

/// A save whose halls predate the upgrade gains the seats on load.
#[test]
fn a_save_that_knows_writing_loads_with_the_new_seats() {
    let (mut w, defs) = setup(Nation::Water, &GRID, &[at(0, 0)]);
    let base = defs.building(defs.building_id("study_hall").unwrap()).seats;
    place_study_hall(&mut w, &defs, at(2, 2));
    // Known, but the seats weren't opened: what an older save looks like.
    w.research.grant(defs.tech_id("writing").unwrap());
    assert_eq!(study_jobs_at(&w, at(2, 2)) as u32, base);
    let saved = w.save(&defs).expect("world saves");
    let w2 = World::load(&saved, &defs).expect("world loads");
    assert_eq!(study_jobs_at(&w2, at(2, 2)) as u32, base + 2);
}

/// The Map Room keeps its single seat whatever is known.
#[test]
fn writing_leaves_the_map_room_alone() {
    let (mut w, defs) = setup(Nation::Fire, &GRID, &[at(0, 0)]);
    let room = defs.building_id("map_room").unwrap();
    w.research.grant(defs.tech_id("writing").unwrap());
    assert_eq!(w.seats_of(defs.building(room)), defs.building(room).seats);
}
