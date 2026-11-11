//! `data/trade.toml`'s loader (#437): strict, range-checked, and gated on systems that exist.

use crate::defs::Defs;

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
