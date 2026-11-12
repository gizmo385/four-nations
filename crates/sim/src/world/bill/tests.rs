use super::*;
use crate::defs::{BiomeId, Terrain};
use crate::geom::{Footprint, Grid};
use crate::mapgen::Tile;
use crate::research::Facts;

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

/// The bundled research file plus `extra`.
fn defs_with(extra: &str) -> Result<Defs, crate::defs::DefsError> {
    let mut files = crate::defs::BUNDLED_FILES;
    let research = format!("{}\n{extra}\n", files[3]);
    files[3] = &research;
    Defs::from_files(files)
}

/// A shared 1000-point tech, `billed`, whose bill is 5 wood and 1 ancient scroll.
const BILLED: &str = "[[tech]]\nid = \"billed\"\nname = \"Billed\"\ndescription = \"\"\n\
     cost = 1000\n[[tech.cost_items]]\nitem = \"wood\"\ncount = 5\n\
     [[tech.cost_items]]\nitem = \"ancient_scroll\"\ncount = 1\n";

fn billed_defs() -> Defs {
    defs_with(BILLED).expect("test data is valid")
}

fn item(defs: &Defs, id: &str) -> ItemId {
    defs.item_id(id).unwrap()
}

/// A 30x16 field with units at `units`, a stockpile of 20 wood at (2, 2), and a finished study
/// hall at (20, 8) unless `hall` is false.
fn setup(units: &[Pos], hall: bool) -> (World, Defs, Pos) {
    let defs = billed_defs();
    let mut w = World::with_map(1, field(30, 16), Nation::Earth, units, &defs);
    w.designate_stockpile(at(2, 2)).unwrap();
    w.stored.insert(at(2, 2), Stack { item: item(&defs, "wood"), count: 20 });
    let anchor = at(20, 8);
    if hall {
        raise_hall(&mut w, &defs, anchor);
    }
    (w, defs, anchor)
}

fn raise_hall(w: &mut World, defs: &Defs, anchor: Pos) {
    let id = defs.building_id("study_hall").unwrap();
    w.raise(Footprint::new(anchor, 2, 2), id, defs);
}

fn target(w: &mut World, defs: &Defs) -> TechId {
    let t = defs.tech_id("billed").unwrap();
    w.apply(Command::SetResearch { tech: t }, defs).unwrap();
    t
}

#[test]
fn cost_items_are_validated() {
    let tech = |cost: u32, extra: &str, lines: &str| {
        format!(
            "[[tech]]\nid = \"t\"\nname = \"T\"\ndescription = \"\"\ncost = {cost}\n{extra}\n{lines}"
        )
    };
    let line = |id: &str, n: u32| format!("[[tech.cost_items]]\nitem = \"{id}\"\ncount = {n}\n");
    for (data, why) in [
        (tech(999, "", &line("wood", 1)), "needs a cost of at least 1000"),
        (tech(1000, "core = true", &line("wood", 1)), "a core tech can't have `cost_items`"),
        (tech(1000, "", &format!("{}{}", line("wood", 1), line("wood", 2))), "listed twice"),
        (tech(1000, "", &line("wood", 0)), "must be 1 to 99"),
        (tech(1000, "", &line("wood", 100)), "must be 1 to 99"),
        (tech(1000, "", &line("nope", 1)), "unknown item \"nope\""),
        (format!("{}extra = 1\n", tech(1000, "", &line("wood", 1))), "extra"),
    ] {
        let e = defs_with(&data).expect_err(&data).0;
        assert!(e.contains(why), "{data}: {e}");
    }
    let defs = billed_defs();
    let t = defs.tech(defs.tech_id("billed").unwrap());
    assert_eq!(
        t.cost_items,
        [
            Stack { item: item(&defs, "wood"), count: 5 },
            Stack { item: item(&defs, "ancient_scroll"), count: 1 }
        ]
    );
    assert_eq!(defs.research.bill_hall_id, defs.building_id("study_hall"));
    // The hall must have seats, and must be named once any tech has a bill.
    let mut files = crate::defs::BUNDLED_FILES;
    let research = format!("{}\n{BILLED}", files[3])
        .replace("bill_hall = \"study_hall\"", "bill_hall = \"campfire\"");
    files[3] = &research;
    let e = Defs::from_files(files).unwrap_err().0;
    assert!(e.contains("has no seats"), "{e}");
    let research = format!("{}\n{BILLED}", crate::defs::BUNDLED_FILES[3])
        .replace("bill_hall = \"study_hall\"", "");
    files[3] = &research;
    let e = Defs::from_files(files).unwrap_err().0;
    assert!(e.contains("needs research settings `bill_hall`"), "{e}");
}

#[test]
fn a_tech_completes_only_with_its_knowledge_and_its_bill_and_items_are_never_scaled() {
    let defs = billed_defs();
    let t = defs.tech_id("billed").unwrap();
    let (wood, scroll) = (item(&defs, "wood"), item(&defs, "ancient_scroll"));
    let mut r = Research::new(Nation::Earth, &defs);
    r.set_target(t, Nation::Earth, &Facts::default(), &defs).unwrap();
    r.bank_for_test(2_000_000);
    assert!(r.tick(1, &defs, 2.0, Nation::Earth).is_empty(), "the bill is short");
    assert!(r.waiting_on_bill(&defs, 2.0, Nation::Earth));
    assert_eq!(r.pay_bill(wood, 9, &defs), 5, "never more than owed");
    assert_eq!(r.pay_bill(wood, 1, &defs), 0);
    assert_eq!(r.bill(t, &defs), [(wood, 5, 5), (scroll, 0, 1)]);
    assert!(r.tick(2, &defs, 2.0, Nation::Earth).is_empty());
    assert_eq!(r.pay_bill(scroll, 1, &defs), 1);
    // Hard's research_cost doubles the knowledge, never the items.
    let done = r.tick(3, &defs, 2.0, Nation::Earth);
    assert_eq!(done.iter().map(|c| c.tech).collect::<Vec<_>>(), [t]);
    assert!(r.is_known(t));
    assert_eq!(r.bills_owed(&defs).count(), 0);
}

#[test]
fn a_hauler_brings_the_bill_from_stores_and_it_leaves_the_world() {
    let (mut w, defs, _hall) = setup(&[at(4, 4)], true);
    let t = target(&mut w, &defs);
    let wood = item(&defs, "wood");
    for _ in 0..3_000 {
        w.tick(&defs);
        if w.research().bill(t, &defs)[0].1 == 5 {
            break;
        }
    }
    assert_eq!(w.research().bill(t, &defs)[0], (wood, 5, 5));
    assert_eq!(w.stored_total(wood), 15, "five left stores and went on the bill");
    let carried: u32 =
        w.units.iter().filter_map(|u| u.carrying).filter(|s| s.item == wood).map(|s| s.count).sum();
    let ground: u32 = w.ground.values().filter(|s| s.item == wood).map(|s| s.count).sum();
    assert_eq!(carried + ground, 0, "nothing extra was fetched");
    for _ in 0..50 {
        w.tick(&defs);
    }
    assert!(
        !w.jobs.iter().any(|j| matches!(j.kind, JobKind::Refuel { .. })),
        "no delivery is kept open once the haulable lines are in"
    );
}

#[test]
fn seats_are_not_wanted_while_the_knowledge_waits_on_the_bill() {
    let (mut w, defs, _hall) = setup(&[at(4, 4)], true);
    target(&mut w, &defs);
    assert!(w.study_wanted(&defs));
    w.research.bank_for_test(1_000_000);
    assert!(!w.study_wanted(&defs), "banked and short: scholars haul instead");
}

#[test]
fn retargeting_drops_the_bill_beside_the_first_hall_or_keeps_it_until_one_stands() {
    let (mut w, defs, hall) = setup(&[at(4, 4)], true);
    let t = target(&mut w, &defs);
    let wood = item(&defs, "wood");
    assert_eq!(w.research.pay_bill(wood, 3, &defs), 3);
    let other = defs.tech_id("pottery").unwrap();
    w.apply(Command::SetResearch { tech: other }, &defs).unwrap();
    assert_eq!(w.research().bill(t, &defs)[0].1, 0, "the bill went with the target");
    w.tick_bills(&defs);
    let near: u32 = w
        .ground
        .iter()
        .filter(|(p, s)| s.item == wood && Footprint::new(hall, 2, 2).distance(**p) <= 2)
        .map(|(_, s)| s.count)
        .sum();
    assert_eq!(near, 3, "dropped beside the hall, as ordinary stacks");
    assert!(w.jobs.iter().any(|j| matches!(j.kind, JobKind::Haul { .. })));

    // With no hall standing, the bill keeps until one does.
    let (mut w, defs, hall) = setup(&[at(4, 4)], false);
    target(&mut w, &defs);
    w.research.pay_bill(wood, 2, &defs);
    w.apply(Command::SetResearch { tech: other }, &defs).unwrap();
    w.tick_bills(&defs);
    assert!(w.ground.values().all(|s| s.item != wood));
    assert!(w.research.has_refunds());
    raise_hall(&mut w, &defs, hall);
    w.tick_bills(&defs);
    assert!(!w.research.has_refunds());
    assert_eq!(w.ground.values().filter(|s| s.item == wood).map(|s| s.count).sum::<u32>(), 2);
}

#[test]
fn a_scroll_goes_on_a_short_bill_unread() {
    let (mut w, defs, hall) = setup(&[at(18, 8)], true);
    let t = target(&mut w, &defs);
    let scroll = item(&defs, "ancient_scroll");
    w.stored.clear();
    w.place_ground(at(17, 8), Stack { item: scroll, count: 2 });
    let pool = w.research().pool_millis();
    for _ in 0..1_000 {
        w.tick(&defs);
        if w.ground.values().all(|s| s.item != scroll)
            && w.units.iter().all(|u| u.carrying.is_none())
        {
            break;
        }
    }
    let _ = hall;
    assert_eq!(w.research().bill(t, &defs)[1], (scroll, 1, 1), "one went on the bill");
    assert_eq!(w.tallies().scrolls_read(), 1, "only the other was read");
    let points = u64::from(defs.item(scroll).knowledge.unwrap()) * 1000;
    assert!(w.research().pool_millis() >= pool + points, "the read one gave its knowledge");
    assert!(w.research().pool_millis() < pool + 2 * points, "the billed one gave none");
}

#[test]
fn a_bill_survives_a_save_round_trip() {
    let (mut w, defs, _hall) = setup(&[at(4, 4)], true);
    let t = target(&mut w, &defs);
    w.research.pay_bill(item(&defs, "wood"), 4, &defs);
    let saved = w.save(&defs).expect("saves");
    let w2 = World::load(&saved, &defs).expect("loads");
    assert_eq!(w2.research().bill(t, &defs), w.research().bill(t, &defs));
    assert_eq!(w2.research(), w.research());
}
