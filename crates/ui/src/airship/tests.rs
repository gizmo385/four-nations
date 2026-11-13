//! The Airship screen's picks, view and actions (#763).

use fn_sim::geom::Footprint;
use fn_sim::{Nation, Terrain};

use super::*;

const DOCK: Pos = Pos::new(30, 30);

/// A Fire game with a finished Airship Dock on cleared ground at `DOCK`.
fn session() -> Session {
    let mut s = Session::new(7, Nation::Fire, 64, 64);
    for y in 28..36 {
        for x in 28..36 {
            let t = &mut s.world.map[Pos::new(x, y)];
            t.terrain = Terrain::Land;
            t.resource = None;
        }
    }
    s.world.units.retain(|u| !(27..37).contains(&u.pos.x) || !(27..37).contains(&u.pos.y));
    s.world.map_changed();
    let dock = s.defs.building_id("airship_dock").unwrap();
    s.world.put_building(Footprint::new(DOCK, 3, 3), dock, &s.defs);
    s
}

/// Earth's caravan, by deal: 0 is 30 food for 20 stone, 1 is 50 wood for 8 gold ore.
const FOOD_FOR_STONE: usize = 0;
const WOOD_FOR_ORE: usize = 1;

#[test]
fn a_dock_opens_the_screen_on_the_first_other_nation() {
    let s = session();
    // Any tile of the footprint, and nothing off it.
    assert_eq!(dock_at(&s, Pos::new(31, 31)), Some(DOCK));
    assert_eq!(dock_at(&s, Pos::new(40, 40)), None);
    assert_eq!(open(&s, Pos::new(40, 40)), None);
    let picks = open(&s, Pos::new(32, 32)).unwrap();
    assert_eq!(picks.dock, DOCK);
    let v = view(&s, &picks);
    assert_eq!(v.head, "Airship Dock at (30, 30)");
    assert_eq!(v.nations.len(), 4, "{:?}", v.nations);
    assert!(v.nations[v.nation_at].starts_with("Earth Kingdom"), "{:?}", v.nations);
    assert!(v.nations.iter().any(|n| n.ends_with("(kin)")), "{:?}", v.nations);
    assert_eq!(v.status, "Docked");
    assert_eq!(v.crew, "Crew: 2 residents");
    assert!(v.editable);
}

#[test]
fn the_deals_read_with_their_times_and_what_stores_hold() {
    let s = session();
    let picks = open(&s, DOCK).unwrap();
    let v = view(&s, &picks);
    assert_eq!(v.deals[FOOD_FOR_STONE].text, "30 food for 20 stone");
    assert_eq!(v.deals[WOOD_FOR_ORE].text, "50 wood for 8 gold ore");
    assert_eq!(v.deals[WOOD_FOR_ORE].times, "0/2");
    assert!(v.deals[WOOD_FOR_ORE].stores.ends_with(" wood"), "{:?}", v.deals[WOOD_FOR_ORE]);
    assert_eq!(v.out, "Out: nothing");
    assert_eq!(v.load, "0/60");
    assert_eq!(v.back, "Pick a deal to sail with");
}

#[test]
fn plus_and_minus_change_the_times_within_the_deal_the_dock_and_the_hold() {
    let s = session();
    let mut picks = open(&s, DOCK).unwrap();
    picks.deal = WOOD_FOR_ORE;
    adjust(&s, &mut picks, 1).unwrap();
    let v = view(&s, &picks);
    assert_eq!(v.deals[WOOD_FOR_ORE].times, "1/2");
    assert_eq!(v.out, "Out: 50 wood");
    assert_eq!(v.load, "50/60");
    assert_eq!(v.back, "Back in about 15 min with 8 gold ore");
    // A second time of it would carry 100 items to a hold of 60.
    let why = adjust(&s, &mut picks, 1).unwrap_err();
    assert_eq!(why, "100 items won't fit a hold of 60");
    assert_eq!(picks.times[WOOD_FOR_ORE], 1);
    // Never below none.
    picks.deal = FOOD_FOR_STONE;
    adjust(&s, &mut picks, -1).unwrap();
    assert_eq!(picks.times[FOOD_FOR_STONE], 0);
    picks.deal = WOOD_FOR_ORE;
    adjust(&s, &mut picks, -1).unwrap();
    assert!(picks.times.iter().all(|&t| t == 0), "{:?}", picks.times);
}

#[test]
fn at_most_the_docks_deals_and_each_deals_own_times() {
    let s = session();
    let mut picks = open(&s, DOCK).unwrap();
    let caravan = &caravans(&s)[picks.nation];
    // The deals small enough to carry together, whatever else the hold allows.
    let small: Vec<usize> = caravan
        .deals
        .iter()
        .enumerate()
        .filter(|(_, d)| d.give.iter().map(|g| g.count).sum::<u32>() <= 20)
        .map(|(i, _)| i)
        .collect();
    assert!(small.len() >= 3, "the data has three small deals: {small:?}");
    for &i in &small[..2] {
        picks.deal = i;
        adjust(&s, &mut picks, 1).unwrap();
    }
    picks.deal = small[2];
    assert_eq!(adjust(&s, &mut picks, 1).unwrap_err(), "at most 2 deals a voyage");
    // Each deal's own `times` caps it.
    picks.deal = small[0];
    for _ in 0..20 {
        let _ = adjust(&s, &mut picks, 1);
    }
    assert!(u32::from(picks.times[small[0]]) <= caravan.deals[small[0]].times);
}

#[test]
fn the_arrows_move_the_pointer_in_a_column_and_between_them() {
    let s = session();
    let mut picks = open(&s, DOCK).unwrap();
    let nation = picks.nation;
    step(&s, &mut picks, 0, 1);
    assert_eq!(picks.nation, nation + 1);
    step(&s, &mut picks, 1, 0);
    assert_eq!(picks.column, Column::Deals);
    adjust(&s, &mut picks, 1).unwrap();
    step(&s, &mut picks, 0, 1);
    assert_eq!(picks.deal, 1);
    // A deal picked for one nation is not carried to another.
    step(&s, &mut picks, -1, 0);
    step(&s, &mut picks, 0, -1);
    assert_eq!((picks.nation, picks.deal), (nation, 0));
    assert!(picks.times.iter().all(|&t| t == 0), "{:?}", picks.times);
    // The ends hold.
    for _ in 0..10 {
        step(&s, &mut picks, 0, -1);
    }
    assert_eq!(picks.nation, 0);
}

#[test]
fn enter_asks_only_with_something_picked_and_then_sends() {
    let mut s = session();
    let mut picks = open(&s, DOCK).unwrap();
    let why = ask(&s, &mut picks).unwrap_err();
    assert_eq!(why, "Can't send it: pick a deal to sail with");
    assert!(!picks.confirm);
    picks.deal = WOOD_FOR_ORE;
    adjust(&s, &mut picks, 1).unwrap();
    ask(&s, &mut picks).unwrap();
    assert!(picks.confirm);
    let lines = confirm_lines(&s, &picks);
    assert_eq!(lines[0], "Load 50 wood and sail to the Earth Kingdom?");
    assert!(
        lines[2].contains("two residents board and are away for about 15 minutes"),
        "{lines:?}"
    );
    assert!(lines[2].contains("It comes back with 8 gold ore."), "{lines:?}");

    let said = send(&mut s, &mut picks).unwrap();
    assert_eq!(said, "Loading the airship for the Earth Kingdom");
    assert!(!picks.confirm);
    let v = view(&s, &picks);
    assert!(!v.editable);
    assert_eq!(v.status, "Loading for Earth Kingdom: 0/50");
    assert_eq!(v.deals.len(), 1, "a planned voyage reads back its own deals: {:?}", v.deals);
    assert_eq!(v.deals[0].times, "1/2");
    assert_eq!(v.out, "Out: 50 wood");
    // It can't be planned or edited again until it is cancelled.
    assert_eq!(adjust(&s, &mut picks, 1).unwrap_err(), "a voyage is already planned");
    assert_eq!(ask(&s, &mut picks).unwrap_err(), "a voyage is already planned");

    let said = cancel(&mut s, DOCK).unwrap();
    assert_eq!(said, "Voyage cancelled: the goods wait at the dock");
    assert_eq!(view(&s, &picks).status, "Docked");
    assert!(cancel(&mut s, DOCK).unwrap_err().contains("no voyage is planned"));
}

#[test]
fn the_status_names_what_the_dock_is_doing() {
    let mut s = session();
    let mut picks = open(&s, DOCK).unwrap();
    assert_eq!(status_text(&s.world, &s.defs, DOCK).as_deref(), Some("Docked"));
    picks.deal = WOOD_FOR_ORE;
    adjust(&s, &mut picks, 1).unwrap();
    send(&mut s, &mut picks).unwrap();
    assert_eq!(
        status_text(&s.world, &s.defs, DOCK).as_deref(),
        Some("Loading for Earth Kingdom: 0/50")
    );
    // Off a dock there is nothing to say.
    assert_eq!(status_text(&s.world, &s.defs, Pos::new(1, 1)), None);
}
