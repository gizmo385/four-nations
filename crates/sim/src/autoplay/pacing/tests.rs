use super::*;
use crate::autoplay::{Economy, Opening};
use crate::embark::Survey;
use crate::mapgen::MapGenConfig;
use crate::nation::Nation;
use crate::world::{DrawTermKind, OfferKind};

/// The settlement `balance::run_economy` would found on `seed`, under `defs`.
fn founded(nation: Nation, seed: u64, defs: &Defs) -> World {
    let size = defs.embark.map_size("small").unwrap();
    let cfg =
        MapGenConfig { seed, width: size.width, height: size.height, ..MapGenConfig::default() };
    let survey = Survey::new(&cfg, defs);
    let start = survey.default_site(defs);
    World::found(survey, nation, start, 6, Some("normal".to_owned()), defs)
        .expect("Survey::default_site always names a site World::found accepts")
}

/// Caravans from minute 0 (no chapter to meet first), so a test needn't play to minute 40.
fn defs_with_open_road() -> Defs {
    let src = include_str!("../../../../../data/trade.toml").replacen(
        "opens = \"known_on_the_road\"",
        "",
        1,
    );
    Defs::bundled().with_trade(&src).expect("the bundled trade data minus `opens` is valid")
}

fn play(w: &mut World, defs: &Defs, economy: &Economy, minutes: u64, mut each: impl FnMut(&World)) {
    for tick in 0..minutes * 600 {
        if Opening::due(tick) {
            economy.step(w, defs);
        }
        w.tick(defs);
        if tick % 600 == 0 {
            each(w);
        }
    }
}

fn warm_term_met(w: &World, defs: &Defs) -> bool {
    w.draw_multiplier(defs).terms.iter().any(|t| t.kind == DrawTermKind::WarmBeds && t.is_met())
}

/// The warm-beds term of draw is met within the first hour for every nation, and the passive
/// player (`beds_only`) leaves it to chance on at least one of them.
#[test]
fn the_opening_keeps_its_beds_warm() {
    let defs = Defs::bundled();
    let mut passive_missed = false;
    for nation in Nation::ALL {
        let mut w = founded(nation, 7, &defs);
        let mut warm = false;
        play(&mut w, &defs, &Economy::new(6), 60, |w| warm |= warm_term_met(w, &defs));
        assert!(warm, "{nation:?}: no minute of the first hour had every bed warm");
        let mut w = founded(nation, 7, &defs);
        let mut warm = false;
        play(&mut w, &defs, &Economy::new(6).with_beds_only(true), 60, |w| {
            warm |= warm_term_met(w, &defs)
        });
        passive_missed |= !warm;
    }
    assert!(passive_missed, "the passive player was as warm as the one who plans for it");
}

/// A bed outside every campfire's reach gets a campfire within reach of it.
#[test]
fn a_cold_bed_gets_a_campfire_within_reach() {
    let defs = Defs::bundled();
    let mut w = founded(Nation::Fire, 7, &defs);
    let anchor = w.settlement_anchor();
    play(&mut w, &defs, &Economy::new(6), 15, |_| {});
    // A bed 12 tiles from the anchor, past every campfire's reach.
    let bed = defs.building_id("bed").unwrap();
    let far = nearest_build_site(&w, &defs, Pos::new(anchor.x + 12, anchor.y), bed, 6)
        .expect("room for a bed");
    w.apply(Command::DesignateBuild { pos: far, building: bed }, &defs).unwrap();
    assert!(cold_beds(&w, &defs).contains(&far));
    play(&mut w, &defs, &Economy::new(6), 10, |_| {});
    assert!(!cold_beds(&w, &defs).contains(&far), "no campfire came to reach the bed at {far:?}");
}

/// With a caravan camped, the opening takes its request and the deals it can pay for; with
/// `beds_only` it takes nothing.
#[test]
fn the_opening_takes_deals_and_the_passive_player_does_not() {
    let defs = defs_with_open_road();
    let taken = |economy: Economy| {
        let mut w = founded(Nation::Earth, 7, &defs);
        let mut taken = 0;
        let mut camped = false;
        play(&mut w, &defs, &economy, 40, |w| {
            if let Some(v) = w.caravan() {
                camped = true;
                taken = taken.max(v.offers.iter().map(|o| o.taken).sum::<u32>());
            }
        });
        assert!(camped, "no caravan came in 40 minutes with the road open");
        taken
    };
    assert!(taken(Economy::new(6)) > 0, "the opening took no deal in 40 minutes");
    assert_eq!(taken(Economy::new(6).with_beds_only(true)), 0);
}

/// A request is taken at most once, and the goods owed to the camp are never counted twice.
#[test]
fn deals_are_never_promised_twice() {
    let defs = defs_with_open_road();
    let mut w = founded(Nation::Water, 3, &defs);
    play(&mut w, &defs, &Economy::new(6), 45, |w| {
        let Some(v) = w.caravan() else { return };
        for o in &v.offers {
            if let OfferKind::Request(_) = o.kind {
                assert!(o.taken <= 1);
            }
        }
        // Owed goods are already taken out of `stock`, so it can only be below stores.
        assert!(Stock::read(w, &defs).food <= w.stored_food(&defs));
    });
}

/// The opening is deterministic with caravans and draw in play.
#[test]
fn the_opening_is_deterministic() {
    let defs = defs_with_open_road();
    let run = || {
        let mut w = founded(Nation::Fire, 5, &defs);
        play(&mut w, &defs, &Economy::new(6), 30, |_| {});
        (w.population(), w.draw(), w.stored_food(&defs), w.tick)
    };
    assert_eq!(run(), run());
}
