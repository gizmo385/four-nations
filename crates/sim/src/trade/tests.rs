//! `data/trade.toml`'s loader (#437): strict, range-checked, and gated on systems that exist.

use crate::defs::Defs;
use crate::nation::Nation;
use crate::trade::{Goods, Reward};

const TRADE: &str = include_str!("../../../../data/trade.toml");

fn error(from: &str, to: &str) -> String {
    assert!(TRADE.contains(from), "{from}");
    Defs::bundled().with_trade(&TRADE.replacen(from, to, 1)).expect_err("should be rejected").0
}

#[test]
fn food_means_any_food_and_items_resolve() {
    let defs = Defs::bundled();
    let fire =
        defs.trade.caravans.iter().find(|c| c.nation == crate::nation::Nation::Fire).unwrap();
    let food = &fire.deals[0].give[0];
    assert!(food.any_food);
    assert_eq!(food.items.len(), defs.foods.len());
    assert_eq!(fire.deals[0].get, [(defs.item_id("metal").unwrap(), 2)]);
}

#[test]
fn the_loader_is_strict() {
    assert!(error("deals_per_visit = 3", "deals_per_visit = 3\nprices = 1").contains("prices"));
    assert!(error("opens = \"known_on_the_road\"", "opens = \"nope\"").contains("unknown chapter"));
    assert!(error("item = \"metal\"", "item = \"mithril\"").contains("mithril"));
    assert!(error("stay_minutes = 9.0", "stay_minutes = 12.0").contains("fit in interval"));
    assert!(error("times = 2", "times = 0").contains("times"));
    assert!(
        error(
            "count = 30 }]\nget = [{ item = \"metal\"",
            "count = 0 }]\nget = [{ item = \"metal\""
        )
        .contains("count must be")
    );
    assert!(error("camp_distance = [8, 14]", "camp_distance = [14, 8]").contains("camp_distance"));
    assert!(
        error(
            "order = [\"earth\", \"fire\", \"water\", \"air\"]",
            "order = [\"earth\", \"earth\"]"
        )
        .contains("twice")
    );
}

#[test]
fn a_caravan_sells_at_most_ten_food_a_visit() {
    let e = error(
        "get = [{ item = \"cabbage\", count = 5 }]\ntimes = 2",
        "get = [{ item = \"cabbage\", count = 6 }]\ntimes = 2",
    );
    assert!(e.contains("at most 10"), "{e}");
    assert!(
        error("get = [{ item = \"metal\", count = 2 }]", "get = [{ item = \"food\", count = 2 }]")
            .contains("concrete")
    );
}

#[test]
fn rewards_are_checked_and_chart_waits_for_its_system() {
    assert!(error("reward = { draw = 0.5 }", "reward = { chart = 1 }").contains("chart"));
    assert!(error("reward = { draw = 0.5 }", "reward = { draw = 3.0 }").contains("bank"));
    assert!(
        error("reward = { draw = 0.5 }", "reward = { draw = 0.5, knowledge = 5 }")
            .contains("one reward")
    );
    assert!(error("reward = { draw = 0.5 }", "reward = {}").contains("needs a reward"));
}

#[test]
fn copies_never_buy_knowledge() {
    // A deal that takes scroll copies and pays knowledge, and the same for a request.
    let deal = error(
        "give = [{ item = \"brick\", count = 8 }]\nknowledge = 30",
        "give = [{ item = \"scroll_copy\", count = 1 }]\nknowledge = 30",
    );
    assert!(deal.contains("scroll copies") && deal.contains("knowledge"), "{deal}");
    let request = error(
        "give = [{ item = \"food\", count = 40 }]\nreward = { knowledge = 60 }",
        "give = [{ item = \"scroll_copy\", count = 2 }]\nreward = { knowledge = 60 }",
    );
    assert!(request.contains("scroll copies") && request.contains("knowledge"), "{request}");
    // Copies for goods, or for draw, are fine: the bundled rows say so.
    let defs = Defs::bundled();
    let copy = defs.item_id("scroll_copy").unwrap();
    let takes_copies = |g: &[Goods]| g.iter().any(|g| g.accepts(copy));
    let deals = || defs.trade.caravans.iter().flat_map(|c| &c.deals);
    let requests = || defs.trade.caravans.iter().flat_map(|c| &c.requests);
    assert_eq!(deals().filter(|d| takes_copies(&d.give)).count(), 3, "Earth, Fire and Water");
    assert!(deals().filter(|d| takes_copies(&d.give)).all(|d| d.knowledge == 0));
    assert_eq!(requests().filter(|r| takes_copies(&r.give)).count(), 2, "Earth's and Air's");
    assert!(
        requests()
            .filter(|r| takes_copies(&r.give))
            .all(|r| !matches!(r.reward, Reward::Knowledge(_)))
    );
}

#[test]
fn every_caravan_lists_at_least_four_deals() {
    let defs = Defs::bundled();
    assert_eq!(defs.trade.caravans.len(), 4);
    for c in &defs.trade.caravans {
        assert!(c.deals.len() >= 4, "{:?} has {} deals", c.nation, c.deals.len());
    }
}

#[test]
fn caravans_take_wares_and_copies() {
    let defs = Defs::bundled();
    let wares = defs.item_id("wares").unwrap();
    let copy = defs.item_id("scroll_copy").unwrap();
    let count = |item, give: &[Goods]| give.iter().any(|g| g.accepts(item));
    for c in &defs.trade.caravans {
        let got = (
            c.deals.iter().filter(|d| count(wares, &d.give)).count(),
            c.deals.iter().filter(|d| count(copy, &d.give)).count(),
            c.requests.iter().filter(|r| count(wares, &r.give)).count(),
            c.requests.iter().filter(|r| count(copy, &r.give)).count(),
        );
        // (wares deals, copy deals, wares requests, copy requests)
        let want = match c.nation {
            Nation::Earth => (1, 1, 0, 1),
            Nation::Fire | Nation::Water => (1, 1, 0, 0),
            Nation::Air => (0, 0, 1, 1),
        };
        assert_eq!(got, want, "{:?}", c.nation);
    }
}
