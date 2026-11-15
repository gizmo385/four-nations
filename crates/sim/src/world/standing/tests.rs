use super::*;
use crate::jobs::Stack;
use crate::mapgen::MapGenConfig;
use crate::nation::Nation;

/// A small Fire world with its gate stood: a United City, where standing runs.
fn united() -> (World, Defs) {
    let defs = Defs::bundled();
    let cfg = MapGenConfig { width: 48, height: 32, seed: 7, ..Default::default() };
    let mut w = World::new(&cfg, Nation::Fire, 4, &defs);
    w.complete_endeavor(&defs);
    (w, defs)
}

/// Puts `count` of `id` on stockpile tiles laid out near the first resident.
fn stock(w: &mut World, defs: &Defs, id: &str, count: u32) {
    let at = w.units[0].pos;
    let mut laid = 0;
    'out: for dy in -4..=4 {
        for dx in -4..=4 {
            if w.designate_stockpile(crate::geom::Pos::new(at.x + dx, at.y + dy)).is_ok() {
                laid += 1;
                if laid == 4 {
                    break 'out;
                }
            }
        }
    }
    w.add_to_stores(Stack { item: defs.item_id(id).unwrap(), count }, defs);
}

fn run_minutes(w: &mut World, defs: &Defs, minutes: u64) {
    for _ in 0..minutes * MINUTE {
        w.tick(defs);
    }
}

fn tiers(w: &World) -> Vec<Tier> {
    w.residents().map(|u| u.standing.tier).collect()
}

#[test]
fn lanterns_in_stores_make_fire_settlers_citizens_and_their_lack_drops_them_again() {
    let (mut w, defs) = united();
    let lantern = defs.item_id("lantern").unwrap();
    stock(&mut w, &defs, "lantern", 40);
    let before = w.stored_total(lantern);
    run_minutes(&mut w, &defs, 16);
    assert!(tiers(&w).iter().all(|&t| t == Tier::Citizen), "{:?}", tiers(&w));
    assert!(w.stored_total(lantern) < before, "the lanterns were used");
    assert_eq!(w.tier_counts()[1] as usize, w.residents().count());
    // The draw counts them.
    let report = w.draw_multiplier(&defs);
    assert!(report.terms.iter().any(|t| t.kind == crate::world::DrawTermKind::Citizens));
    // A Citizen works faster.
    let u = w.residents().next().unwrap().clone();
    let base = crate::research::effective_modifiers(
        Nation::Fire,
        Nation::Fire,
        w.research().known_techs(),
        &defs,
    );
    assert!(w.modifiers_for(&u, &defs).construction > base.construction);
    // With no lanterns left, 20 minutes unmet drops them back.
    let left = w.stored_total(lantern);
    for _ in 0..left {
        assert!(w.take_one_stored(lantern));
    }
    run_minutes(&mut w, &defs, 36);
    assert!(tiers(&w).iter().all(|&t| t == Tier::Settler), "{:?}", tiers(&w));
}

#[test]
fn nothing_happens_before_the_gate() {
    let defs = Defs::bundled();
    let cfg = MapGenConfig { width: 48, height: 32, seed: 7, ..Default::default() };
    let mut w = World::new(&cfg, Nation::Fire, 4, &defs);
    let lantern = defs.item_id("lantern").unwrap();
    stock(&mut w, &defs, "lantern", 40);
    let held = |w: &World| {
        let ground: u32 =
            w.ground_items().filter(|(_, s)| s.item == lantern).map(|(_, s)| s.count).sum();
        w.stored_total(lantern)
            + ground
            + w.units
                .iter()
                .filter_map(|u| u.carrying)
                .filter(|s| s.item == lantern)
                .map(|s| s.count)
                .sum::<u32>()
    };
    run_minutes(&mut w, &defs, 16);
    assert!(tiers(&w).iter().all(|&t| t == Tier::Settler));
    assert_eq!(held(&w), 40, "no lantern used");
}

#[test]
fn a_citizens_standing_survives_a_save() {
    let (mut w, defs) = united();
    stock(&mut w, &defs, "lantern", 40);
    run_minutes(&mut w, &defs, 16);
    let text = w.save(&defs).unwrap();
    let back = World::load(&text, &defs).unwrap();
    assert_eq!(tiers(&back), tiers(&w));
    assert_eq!(
        back.residents().map(|u| u.standing.clone()).collect::<Vec<_>>(),
        w.residents().map(|u| u.standing.clone()).collect::<Vec<_>>()
    );
}
