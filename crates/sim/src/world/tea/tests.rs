//! The Tea House (#710): open while it holds an herb, the burn, a caravan's extra deal and
//! longer stay, the draw term and the 1.4 cap (#706).

use super::*;
use crate::defs::{BiomeId, Terrain};
use crate::geom::{Footprint, Grid};
use crate::mapgen::Tile;
use crate::world::draw::DrawTermKind;
use crate::world::trade::OfferKind;

fn at(x: i32, y: i32) -> Pos {
    Pos::new(x, y)
}

const TRADE: &str = include_str!("../../../../../data/trade.toml");

/// The bundled data with a quick schedule: a caravan every 10 minutes, 0.2 into each, staying 8,
/// from minute 0 (`opens` dropped), so a test needn't meet a chapter.
fn defs() -> Defs {
    let src = TRADE
        .replacen("interval_minutes = 15.0", "interval_minutes = 10.0", 1)
        .replacen("offset_minutes = 4.0", "offset_minutes = 0.2", 1)
        .replacen("stay_minutes = 9.0", "stay_minutes = 8.0", 1)
        .replacen("opens = \"known_on_the_road\"", "", 1);
    Defs::bundled().with_trade(&src).expect("test trade data is valid")
}

fn minutes(m: f32) -> u64 {
    (m * 60.0 * TICKS_PER_SECOND as f32).round() as u64
}

/// A 40x40 field of land with one settler frozen at (20, 20).
fn world(seed: u64, defs: &Defs) -> World {
    let field = Grid::from_fn(40, 40, |_| Tile {
        elevation: 100,
        temperature: 128,
        moisture: 128,
        terrain: Terrain::Land,
        biome: BiomeId(0),
        resource: None,
        building: None,
    });
    let mut w = World::with_map(seed, field, Nation::Earth, &[at(20, 20)], defs);
    w.units[0].idle_until = u64::MAX;
    w
}

/// A finished Tea House on `pos` (2x2) holding `herbs`.
fn house(w: &mut World, defs: &Defs, pos: Pos, herbs: u32) {
    let id = defs.building_id("tea_house").unwrap();
    w.raise(Footprint::new(pos, 2, 2), id, defs);
    set_herbs(w, pos, herbs);
}

fn set_herbs(w: &mut World, pos: Pos, herbs: u32) {
    let b = w.anchored_mut(pos).and_then(|b| b.fuel.as_mut()).expect("a bunker");
    b.count = herbs;
    b.burned = 0;
}

fn herbs(w: &World, pos: Pos) -> u32 {
    w.anchored(pos).and_then(|b| b.fuel).expect("a bunker").count
}

fn run(w: &mut World, defs: &Defs, ticks: u64) {
    for _ in 0..ticks {
        w.tick(defs);
    }
}

/// Runs to just after the first caravan makes camp.
fn to_first_camp(w: &mut World, defs: &Defs) {
    run(w, defs, minutes(0.2) + 1);
    assert!(w.caravan().is_some(), "a caravan camped");
}

/// The deals a visit shows, as indexes into the caravan's list.
fn deals(w: &World) -> Vec<u8> {
    w.caravan()
        .expect("camped")
        .offers
        .iter()
        .filter_map(|o| match o.kind {
            OfferKind::Deal(d) => Some(d),
            OfferKind::Request(_) => None,
        })
        .collect()
}

fn request(w: &World) -> OfferKind {
    w.caravan().expect("camped").offers.last().expect("a request").kind
}

#[test]
fn the_bundled_tea_house_and_tech_load() {
    let defs = Defs::bundled();
    let id = defs.building_id("tea_house").unwrap();
    let def = defs.building(id);
    let t = def.tea_house.expect("a tea_house section");
    assert_eq!((t.extra_deals, t.extra_stay_minutes, t.draw), (1, 4.0, 0.1));
    assert_eq!(def.footprint, (2, 2));
    let fuel = def.fuel.as_ref().expect("a herb bunker");
    assert_eq!(fuel.item, defs.item_id("herbs").unwrap());
    assert_eq!(fuel.bunker, 10);
    assert_eq!(fuel.per_minute, Some(0.2));

    let tech = defs.tech(defs.tech_id("tea_ceremony").unwrap());
    assert_eq!(tech.cost, 120);
    assert!(tech.nation.is_none(), "shared");
    let prereqs: Vec<&str> = tech.prereqs.iter().map(|&p| defs.tech(p).id.as_str()).collect();
    assert_eq!(prereqs, ["herbalism", "pottery"]);
    assert_eq!(tech.unlocks.buildings, ["tea_house"]);
}

#[test]
fn a_tea_house_section_is_validated() {
    let files = crate::defs::BUNDLED_FILES;
    let (head, tail) = files[7].split_once("[building.tea_house]\n").expect("the Tea House block");
    let fuel = "[building.fuel]\nitem = \"herbs\"\nbunker = 10\nper_minute = 0.2";
    assert!(tail.contains(fuel), "the Tea House's fuel moved");
    let cases: [(&[(&str, &str)], &str); 8] = [
        (&[("draw = 0.1", "draw = 1.5")], "draw"),
        (&[("draw = 0.1", "draw = -0.1")], "draw"),
        (&[("extra_stay_minutes = 4.0", "extra_stay_minutes = -1.0")], "extra_stay_minutes"),
        (
            &[
                ("extra_deals = 1", "extra_deals = 0"),
                ("extra_stay_minutes = 4.0", "extra_stay_minutes = 0.0"),
                ("draw = 0.1", "draw = 0.0"),
            ],
            "must give something",
        ),
        (&[("extra_deals = 1", "extra_deals = 1\nextra = 1")], "extra"),
        (&[(fuel, "[[building.cost]]\nitem = \"tool\"\ncount = 1")], "[building.fuel]"),
        (&[("per_minute = 0.2", "# no per_minute")], "per_minute"),
        (&[("extra_deals = 1", "extra_deals = -1")], "extra_deals"),
    ];
    for (edits, why) in cases {
        let mut tail = tail.to_owned();
        for &(from, to) in edits {
            assert!(tail.contains(from), "{from} moved");
            tail = tail.replacen(from, to, 1);
        }
        let edited = format!("{head}[building.tea_house]\n{tail}");
        let mut files = files;
        files[7] = &edited;
        let err = Defs::from_files(files).expect_err(why).to_string();
        assert!(err.contains(why), "{why}: {err}");
    }
}

#[test]
fn a_house_is_open_while_it_holds_an_herb_and_burns_whenever_stocked() {
    let defs = defs();
    let mut w = world(1, &defs);
    let pos = at(2, 2);
    assert!(!w.tea_open(&defs));
    assert_eq!(w.tea_house_open(pos, &defs), None, "no house there yet");

    house(&mut w, &defs, pos, 10);
    assert!(w.tea_open(&defs));
    assert_eq!(w.tea_house_open(pos, &defs), Some(true));
    assert_eq!(w.tea_house_open(at(20, 20), &defs), None, "not a house");

    // 0.2 herbs a minute, with no caravan camped and nobody near: 12 an hour.
    run(&mut w, &defs, minutes(4.9));
    assert_eq!(herbs(&w, pos), 10, "not yet");
    run(&mut w, &defs, minutes(0.2));
    assert_eq!(herbs(&w, pos), 9, "one herb after 5 minutes");
    run(&mut w, &defs, minutes(5.0));
    assert_eq!(herbs(&w, pos), 8);

    // The last herb closes it, and an empty bunker burns nothing.
    set_herbs(&mut w, pos, 1);
    assert!(w.tea_open(&defs));
    run(&mut w, &defs, minutes(5.1));
    assert_eq!(herbs(&w, pos), 0);
    assert!(!w.tea_open(&defs));
    assert_eq!(w.tea_house_open(pos, &defs), Some(false));
}

#[test]
fn an_empty_house_adds_nothing() {
    let defs = defs();
    let mut w = world(1, &defs);
    house(&mut w, &defs, at(2, 2), 0);
    assert_eq!(w.tea_extra_deals(&defs), 0);
    assert_eq!(w.tea_extra_stay_ticks(&defs), 0);
    assert_eq!(w.tea_draw_milli(&defs), 0);
}

#[test]
fn an_open_house_adds_a_deal_and_four_minutes_and_keeps_the_first_three() {
    let defs = defs();
    // Every seed: with and without a house the first three deals and the request match, and the
    // extra deal is one of the ones not shown.
    for seed in 1..=40 {
        let mut plain = world(seed, &defs);
        to_first_camp(&mut plain, &defs);
        let mut tea = world(seed, &defs);
        house(&mut tea, &defs, at(2, 2), 10);
        to_first_camp(&mut tea, &defs);

        let (without, with) = (deals(&plain), deals(&tea));
        assert_eq!(without.len(), 3, "seed {seed}");
        assert_eq!(with.len(), 4, "seed {seed}");
        assert!(without.iter().all(|d| with.contains(d)), "seed {seed}: {without:?} in {with:?}");
        let mut distinct = with.clone();
        distinct.dedup();
        assert_eq!(distinct, with, "seed {seed}: listed in data order, no repeats");
        assert_eq!(request(&plain), request(&tea), "seed {seed}: the request is drawn the same");
        assert_eq!(plain.caravan().unwrap().camp, tea.caravan().unwrap().camp);

        let (a, b) = (plain.caravan().unwrap(), tea.caravan().unwrap());
        assert_eq!(a.leaves - a.arrived, minutes(8.0));
        assert_eq!(b.leaves - b.arrived, minutes(12.0), "four minutes longer");
    }
}

#[test]
fn the_extra_deals_differ_by_seed_and_repeat_for_one_seed() {
    let defs = defs();
    let extra = |seed: u64| {
        let mut plain = world(seed, &defs);
        to_first_camp(&mut plain, &defs);
        let mut tea = world(seed, &defs);
        house(&mut tea, &defs, at(2, 2), 10);
        to_first_camp(&mut tea, &defs);
        let shown = deals(&plain);
        deals(&tea).into_iter().find(|d| !shown.contains(d)).expect("one extra")
    };
    let picks: Vec<u8> = (1..=40).map(extra).collect();
    assert_eq!(picks, (1..=40).map(extra).collect::<Vec<u8>>(), "deterministic");
    assert!(picks.iter().any(|&p| p != picks[0]), "drawn, not fixed: {picks:?}");
}

#[test]
fn the_stay_is_fixed_at_arrival_and_a_dry_house_keeps_what_it_gave() {
    let defs = defs();
    let mut w = world(3, &defs);
    house(&mut w, &defs, at(2, 2), 10);
    to_first_camp(&mut w, &defs);
    let before = w.caravan().unwrap().clone();
    assert_eq!(before.offers.len(), 5, "four deals and the request");

    set_herbs(&mut w, at(2, 2), 0);
    run(&mut w, &defs, minutes(1.0));
    assert!(!w.tea_open(&defs));
    assert_eq!(w.caravan(), Some(&before), "the visit keeps its deals and its leave time");
    let wait = before.leaves - w.tick;
    run(&mut w, &defs, wait);
    assert!(w.caravan().is_none(), "left at the extended time");

    // A house that opens after the caravan arrived changes nothing for that visit.
    let mut w = world(3, &defs);
    house(&mut w, &defs, at(2, 2), 0);
    to_first_camp(&mut w, &defs);
    let v = w.caravan().unwrap().clone();
    assert_eq!(v.offers.len(), 4);
    set_herbs(&mut w, at(2, 2), 10);
    run(&mut w, &defs, minutes(1.0));
    assert_eq!(w.caravan(), Some(&v));
    assert_eq!(v.leaves - v.arrived, minutes(8.0));
}

#[test]
fn two_houses_do_not_stack() {
    let defs = defs();
    let mut w = world(3, &defs);
    house(&mut w, &defs, at(2, 2), 10);
    house(&mut w, &defs, at(6, 2), 10);
    assert_eq!(w.tea_extra_deals(&defs), 1);
    assert_eq!(w.tea_extra_stay_ticks(&defs), minutes(4.0));
    assert_eq!(w.tea_draw_milli(&defs), 100);
    to_first_camp(&mut w, &defs);
    assert_eq!(deals(&w).len(), 4);

    // One going dry leaves the other open.
    set_herbs(&mut w, at(2, 2), 0);
    assert!(w.tea_open(&defs));
}

#[test]
fn a_caravan_with_three_deals_shows_them_all_and_still_stays_longer() {
    // Fire's list has 4 deals, Water's only 3: its visit already shows them all, so the house has
    // none left to add. Its stay is longer all the same.
    let src = TRADE
        .replacen("interval_minutes = 15.0", "interval_minutes = 10.0", 1)
        .replacen("offset_minutes = 4.0", "offset_minutes = 0.2", 1)
        .replacen("opens = \"known_on_the_road\"", "", 1)
        .replacen("order = [\"earth\", \"fire\"", "order = [\"water\", \"earth\", \"fire\"", 1)
        .replacen("\"water\", \"air\"]", "\"air\"]", 1);
    let defs = Defs::bundled().with_trade(&src).expect("valid");
    let mut w = world(1, &defs);
    house(&mut w, &defs, at(2, 2), 10);
    to_first_camp(&mut w, &defs);
    let v = w.caravan().expect("camped");
    assert_eq!(v.nation, Nation::Water);
    assert_eq!(w.caravan_def(&defs).unwrap().deals.len(), 3);
    assert_eq!(deals(&w), [0, 1, 2], "every deal, and none twice");
    assert_eq!(v.leaves - v.arrived, minutes(13.0), "the bundled 9 minutes and 4");
}

#[test]
fn draw_gains_a_tenth_while_a_house_is_open_and_the_cap_holds() {
    let defs = Defs::bundled();
    let mut w = world(1, &defs);
    assert_eq!(w.draw_multiplier(&defs).milli, 600);

    house(&mut w, &defs, at(2, 2), 10);
    let r = w.draw_multiplier(&defs);
    assert_eq!(r.milli, 700);
    assert!(!r.capped);
    let term = r.terms.iter().find(|t| t.kind == DrawTermKind::TeaHouse).expect("a reason");
    assert!(term.is_met());
    assert_eq!(term.milli, 100);

    set_herbs(&mut w, at(2, 2), 0);
    let r = w.draw_multiplier(&defs);
    assert_eq!(r.milli, 600, "closed: no term");
    assert!(r.terms.iter().all(|t| t.kind != DrawTermKind::TeaHouse));

    // Two houses add 0.1, not 0.2.
    set_herbs(&mut w, at(2, 2), 10);
    house(&mut w, &defs, at(6, 2), 10);
    assert_eq!(w.draw_multiplier(&defs).milli, 700);

    // A settlement with every other bonus is at the 1.4 cap already: the house adds nothing,
    // and the cap is not lifted (#706).
    for (x, item) in [(0, "fish"), (1, "fruit"), (2, "cabbage")] {
        let item = defs.item_id(item).unwrap();
        w.stored.insert(at(x, 0), crate::jobs::Stack { item, count: 40 });
    }
    for x in 10..15 {
        let id = defs.building_id("bed").unwrap();
        w.put_building(Footprint::single(at(x, 10)), id, &defs);
    }
    let id = defs.building_id("campfire").unwrap();
    w.put_building(Footprint::single(at(12, 11)), id, &defs);
    let r = w.draw_multiplier(&defs);
    assert_eq!(r.milli, 1400, "capped at 1.4, not 1.5");
    assert!(r.capped);
    set_herbs(&mut w, at(2, 2), 0);
    set_herbs(&mut w, at(6, 2), 0);
    let r = w.draw_multiplier(&defs);
    assert_eq!(r.milli, 1400, "the same without a house");
    assert!(!r.capped, "1.4 exactly");
}

#[test]
fn an_open_house_speeds_draw() {
    let defs = defs();
    let fill = |open: bool| {
        let mut w = world(1, &defs);
        house(&mut w, &defs, at(2, 2), if open { 10 } else { 0 });
        run(&mut w, &defs, minutes(1.0));
        w.draw()
    };
    // 0.6 and 0.7 of a newcomer per 12 minutes: 50 and 58 milli-units in one minute.
    assert_eq!(fill(false), 50);
    assert_eq!(fill(true), 58);
}
