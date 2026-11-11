//! The Traders screen's view (#438).

use fn_sim::Nation;

use super::*;

/// A fresh Earth game whose caravans come from minute 0, the first 6 seconds in.
fn session() -> Session {
    let mut s = Session::new(7, Nation::Earth, 64, 64);
    let src = include_str!("../../../../data/trade.toml")
        .replacen("opens = \"known_on_the_road\"", "", 1)
        .replacen("offset_minutes = 4.0", "offset_minutes = 0.1", 1);
    s.defs = s.defs.clone().with_trade(&src).unwrap();
    s
}

fn camp(s: &Session) -> CampView {
    match view(s) {
        TradersView::Camp(c) => c,
        TradersView::Away(text) => panic!("no caravan: {text}"),
    }
}

/// Ticks until a caravan camps.
fn arrive(s: &mut Session) {
    for _ in 0..200 {
        s.tick();
    }
    assert!(s.world.caravan().is_some(), "Fire's traders made camp");
}

#[test]
fn before_the_first_caravan_the_screen_says_who_is_due_and_when() {
    let s = session();
    assert_eq!(view(&s), TradersView::Away("Fire Nation traders are due in 0:06.".to_owned()));
}

#[test]
fn with_the_road_closed_the_screen_says_so() {
    let s = Session::new(7, Nation::Earth, 64, 64);
    assert_eq!(view(&s), TradersView::Away("The road isn't open yet.".to_owned()));
}

#[test]
fn a_camped_caravan_lists_its_deals_then_its_request() {
    let mut s = session();
    arrive(&mut s);
    let c = camp(&s);
    assert_eq!(c.name, "Fire Nation traders");
    assert!(c.left.ends_with(" left"), "{}", c.left);
    assert_eq!(c.offers.len(), 4, "three deals and the request");
    for (i, o) in c.offers.iter().enumerate() {
        assert_eq!(usize::from(o.slot), i);
        assert_eq!(o.request, i == 3);
        assert!(o.can_take && !o.can_cancel && o.under_way.is_none(), "{o:?}");
        assert!(o.stores.ends_with(" in stores"), "{o:?}");
    }
    let request = &c.offers[3];
    assert_eq!(request.name, "Food for the families at Fire Fountain City");
    assert_eq!(request.give, "40 food");
    assert_eq!(request.get, "half a newcomer's draw");
    assert_eq!(request.taken, "not taken");
    assert!(c.offers[0].taken.starts_with("0 of "), "{:?}", c.offers[0]);
}

#[test]
fn taking_a_deal_shows_it_under_way_and_cancelling_drops_it() {
    let mut s = session();
    arrive(&mut s);
    let first = camp(&s).offers[0].clone();
    let said = take(&mut s, 0).unwrap();
    assert_eq!(said, format!("Taken: {} for {}", first.give, first.get));
    let o = &camp(&s).offers[0];
    assert!(o.taken.starts_with("1 of "), "{o:?}");
    let under_way = o.under_way.as_deref().expect("under way");
    assert!(under_way.starts_with("0 of ") && under_way.ends_with(" delivered"), "{under_way}");
    assert!(o.can_cancel);

    assert!(cancel(&mut s, 0).unwrap().starts_with("Cancelled: "));
    let o = &camp(&s).offers[0];
    assert!(o.taken.starts_with("0 of ") && o.under_way.is_none(), "{o:?}");
    assert_eq!(cancel(&mut s, 0), Err("Nothing to cancel: nothing is owed on it".to_owned()));
}

#[test]
fn a_request_is_taken_once() {
    let mut s = session();
    arrive(&mut s);
    take(&mut s, 3).unwrap();
    assert_eq!(camp(&s).offers[3].taken, "taken");
    assert!(!camp(&s).offers[3].can_take);
    assert_eq!(take(&mut s, 3), Err("Can't take that deal: already taken in full".to_owned()));
}

#[test]
fn with_no_caravan_taking_is_refused() {
    let mut s = session();
    assert_eq!(take(&mut s, 0), Err("Can't take that deal: no traders are camped".to_owned()));
}
