//! Borrowed techs in play (`teachable`, docs/design/horizons-phase-6.md, "Borrowed techs", #753)
//! and the From newcomers list.

use super::*;
use crate::defs::{BiomeId, Terrain};
use crate::geom::Grid;
use crate::mapgen::Tile;
use crate::research::{Condition, ResearchError};

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

/// A Water city of twelve settlers, the gate standing: units 0..3 are Fire, 3..5 Earth, 5..8 Air
/// and the rest Water's own (no nation of their own).
fn united_water(defs: &Defs) -> World {
    let spots: Vec<Pos> = (0..12).map(|x| Pos::new(x * 2, 4)).collect();
    let mut w = World::with_map(1, field(30, 8), Nation::Water, &spots, defs);
    for (i, u) in w.units.iter_mut().enumerate() {
        u.nation = match i {
            0..3 => Some(Nation::Fire),
            3..5 => Some(Nation::Earth),
            5..8 => Some(Nation::Air),
            _ => None,
        };
    }
    w.endeavor = EndeavorState::Complete { tick: 0 };
    w.refresh_roster(defs);
    w
}

fn id(defs: &Defs, tech: &str) -> TechId {
    defs.tech_id(tech).unwrap()
}

fn ids(defs: &Defs, techs: &[&str]) -> Vec<TechId> {
    techs.iter().map(|t| id(defs, t)).collect()
}

#[test]
fn before_the_gate_the_exchange_is_empty_and_nothing_can_be_borrowed() {
    let defs = Defs::bundled();
    let mut w = united_water(&defs);
    w.endeavor = EndeavorState::Dormant;
    assert!(w.exchange(&defs).is_empty());
    assert_eq!(
        w.apply(Command::SetResearch { tech: id(&defs, "brickmaking") }, &defs),
        Err(CommandError::Research(ResearchError::GateFirst))
    );
    assert!(w.research().queue().is_empty());
}

#[test]
fn a_borrowed_tech_needs_the_residents_of_its_nation_at_queueing() {
    let defs = Defs::bundled();
    let brick = id(&defs, "brickmaking");
    let mut w = united_water(&defs);
    // Three Fire residents is exactly `borrow_residents`; one leaves.
    assert_eq!(defs.research.borrow_residents, 3);
    w.units[0].nation = Some(Nation::Earth);
    w.refresh_roster(&defs);
    let err = w.apply(Command::SetResearch { tech: brick }, &defs).unwrap_err();
    let CommandError::Research(ResearchError::NotYet(c)) = err else { panic!("{err:?}") };
    assert_eq!(c, Condition::Residents { nation: Nation::Fire, need: 3, have: 2 });
    assert_eq!(c.to_string(), "3 Fire residents (2 live here)");
    assert!(w.research().queue().is_empty());
    w.units[0].nation = Some(Nation::Fire);
    w.refresh_roster(&defs);
    w.apply(Command::SetResearch { tech: brick }, &defs).unwrap();
    assert_eq!(w.research().queue().last(), Some(&brick));
}

#[test]
fn a_borrowed_tech_waives_the_bending_and_queues_teachable_prereqs_as_borrowed() {
    let defs = Defs::bundled();
    let mut w = united_water(&defs);
    w.apply(Command::SetResearch { tech: id(&defs, "steam_engines") }, &defs).unwrap();
    let queue = w.research().queue();
    // Forging (Weld) is waived; pottery is shared, so it stays.
    assert!(!queue.contains(&id(&defs, "forging")), "forging is waived");
    assert!(queue.windows(2).all(|p| p[0] < p[1]), "ascending, prereqs first");
    for kept in ["pottery", "smelting", "brickmaking", "steam_engines"] {
        assert!(queue.contains(&id(&defs, kept)), "{kept} is queued");
    }
    // Every Fire tech queued is teachable, and the target is the last.
    assert_eq!(queue.last(), Some(&id(&defs, "steam_engines")));
    for &t in queue {
        let def = defs.tech(t);
        assert!(def.nation.is_none() || def.teachable, "{}", def.id);
    }
}

#[test]
fn a_borrowed_tech_costs_borrow_cost_times_its_own_and_the_difficulty_scales_it() {
    let defs = Defs::bundled();
    let brick = id(&defs, "brickmaking");
    assert_eq!(defs.tech(brick).cost, 120);
    assert_eq!(defs.research.borrow_cost, 2.0);
    let mut w = united_water(&defs);
    // Shared prereqs first: make them known so the front is brickmaking itself.
    w.research.grant(id(&defs, "pottery"));
    w.apply(Command::SetResearch { tech: brick }, &defs).unwrap();
    assert_eq!(w.research().queue(), [brick]);
    // 239.9 points aren't enough; 240 are.
    let before = w.research().pool_millis();
    w.research.bank_for_test(239_900 - before);
    w.tick(&defs);
    assert!(!w.research().is_known(brick));
    w.research.bank_for_test(1_000);
    w.tick(&defs);
    assert!(w.research().is_known(brick), "240 points paid for a 120-point tech");
    // Its own nation pays the listed cost, and the preset's knob multiplies the borrowed one.
    let cost = |home, rc| crate::research::cost_millis(&defs, brick, home, rc);
    assert_eq!(cost(Nation::Fire, 1.0), 120_000);
    assert_eq!(cost(Nation::Water, 1.0), 240_000);
    assert_eq!(cost(Nation::Water, 1.5), 360_000);
    assert_eq!(defs.tech(brick).cost_for(Nation::Water, 2.0), 240);
    assert_eq!(defs.tech(brick).cost_for(Nation::Fire, 2.0), 120);
    // No item bill: a borrowed tech never has one.
    assert!(defs.tech(brick).cost_items.is_empty());
}

#[test]
fn a_borrowed_tech_waits_if_its_teachers_leave_and_stays_known_once_learned() {
    let defs = Defs::bundled();
    let brick = id(&defs, "brickmaking");
    let mut w = united_water(&defs);
    w.research.grant(id(&defs, "pottery"));
    w.apply(Command::SetResearch { tech: brick }, &defs).unwrap();
    w.research.bank_for_test(240_000);
    // A Fire resident leaves before it completes: it waits with its points.
    w.units[0].nation = Some(Nation::Earth);
    w.tick(&defs);
    assert!(!w.research().is_known(brick));
    assert_eq!(w.research().queue(), [brick]);
    assert!(w.research().pool_millis() >= 240_000);
    let cost = w.knobs(&defs).research_cost;
    assert_eq!(
        w.research().waiting_on(&w.facts(), &defs, cost, w.nation),
        Some(Condition::Residents { nation: Nation::Fire, need: 3, have: 2 })
    );
    assert!(!w.study_wanted(&defs), "scholars are not seated for a tech that is only waiting");
    w.units[0].nation = Some(Nation::Fire);
    w.tick(&defs);
    assert!(w.research().is_known(brick));
    // The teachers all leave: the tech stays (owner decision #749).
    for u in &mut w.units {
        u.nation = Some(Nation::Water);
    }
    for _ in 0..3 {
        w.tick(&defs);
    }
    assert_eq!(w.facts().residents[Nation::Fire.index()], 0);
    assert!(w.research().is_known(brick));
}

#[test]
fn a_borrowed_techs_modifiers_help_only_its_teachers_nation() {
    let defs = Defs::bundled();
    let brick = id(&defs, "brickmaking");
    let mut w = united_water(&defs);
    let before: Vec<Modifiers> =
        (0..w.units.len()).map(|i| w.modifiers_for(&w.units[i], &defs)).collect();
    let settlement = *w.modifiers();
    w.research.grant(brick);
    w.refresh_modifiers(&defs);
    // The settlement's cached value and a home (Water) unit's don't change.
    assert_eq!(*w.modifiers(), settlement);
    assert_eq!(w.modifiers_for(&w.units[11], &defs), before[11]);
    // A Fire resident (the teacher) gets the 1.15x on top of Fire's own base.
    let fire = w.modifiers_for(&w.units[0], &defs);
    assert_eq!(fire.construction, before[0].construction * 1.15);
    // An Earth resident (a third nation) and an Air one don't.
    assert_eq!(w.modifiers_for(&w.units[3], &defs), before[3]);
    assert_eq!(w.modifiers_for(&w.units[5], &defs), before[5]);
    // The breakdown agrees.
    let parts = w.modifier_breakdown(&w.units[0], "construction", &defs);
    assert!(parts.iter().any(|&(n, f)| n == "Brickmaking" && f == 1.15), "{parts:?}");
    let parts = w.modifier_breakdown(&w.units[3], "construction", &defs);
    assert!(parts.iter().all(|&(n, _)| n != "Brickmaking"), "{parts:?}");
}

#[test]
fn a_home_tech_still_reaches_every_unit_and_nothing_borrowed_changes_a_bit() {
    use crate::research::effective_modifiers;
    let defs = Defs::bundled();
    for home in Nation::ALL {
        // Everything the home nation can research itself, fusions and all.
        let known: Vec<TechId> = defs
            .techs
            .iter()
            .enumerate()
            .filter(|(_, t)| t.open_to(home))
            .map(|(i, _)| TechId(i as u16))
            .collect();
        for unit in Nation::ALL {
            let mut naive = defs.nation(unit).modifiers;
            for &t in &known {
                naive.apply(&defs.tech(t).unlocks.modifiers);
            }
            assert_eq!(
                effective_modifiers(unit, home, known.iter().copied(), &defs),
                naive,
                "{home:?} home, {unit:?} unit"
            );
        }
    }
}

#[test]
fn the_exchange_lists_the_other_nations_teachable_techs_then_the_fusions() {
    let defs = Defs::bundled();
    let mut w = united_water(&defs);
    let ex = w.exchange(&defs);
    // 4 Earth + 5 Fire + 4 Air teachable techs, Water's own are not listed, and the two fusions so far.
    let count = |n| ex.borrowed_from(n).count();
    assert_eq!(
        (count(Nation::Earth), count(Nation::Fire), count(Nation::Water), count(Nation::Air)),
        (3, 5, 0, 4)
    );
    assert_eq!(ex.fusions().count(), 2);
    assert_eq!(ex.entries.len(), 3 + 5 + 4 + 2);
    // Nations in `Nation::ALL` order, each in file order, fusions last.
    let kinds: Vec<_> = ex.entries.iter().map(|e| e.kind).collect();
    assert!(matches!(kinds.last(), Some(ExchangeKind::Fusion { .. })));
    let fire: Vec<_> = ex.borrowed_from(Nation::Fire).map(|e| e.tech).collect();
    assert_eq!(
        fire,
        ids(&defs, &["brickmaking", "smelting", "steam_engines", "hothouses", "cargo_balloons"])
    );
    let get = |ex: &Exchange, t: &str| {
        ex.entries.iter().find(|e| e.tech == id(&defs, t)).unwrap().clone()
    };
    // Costs are the borrowed ones.
    assert_eq!(get(&ex, "brickmaking").cost, 240);
    assert_eq!(get(&ex, "steam_engines").cost, 500);
    assert_eq!(get(&ex, "orchards").cost, 50);
    assert_eq!(get(&ex, "cargo_balloons").cost, 1000);
    assert_eq!(get(&ex, "steelworks").cost, 1000);
    // Orchards: 3 Air residents, no prereq. Available.
    assert_eq!(get(&ex, "orchards").standing, ExchangeStanding::Available);
    // Earthen Walls: only 2 Earth residents live here, and masonry is not known.
    assert_eq!(
        get(&ex, "earthen_walls").standing,
        ExchangeStanding::Locked(vec![
            ExchangeNeed::Tech(id(&defs, "masonry")),
            ExchangeNeed::Condition(Condition::Residents {
                nation: Nation::Earth,
                need: 3,
                have: 2
            }),
        ])
    );
    // Smelting waives forging, so it only waits on its nation's people.
    assert_eq!(get(&ex, "smelting").standing, ExchangeStanding::Available);
    // Queued, then known.
    w.apply(Command::SetResearch { tech: id(&defs, "smelting") }, &defs).unwrap();
    assert_eq!(get(&w.exchange(&defs), "smelting").standing, ExchangeStanding::Queued);
    w.research.grant(id(&defs, "smelting"));
    w.research.clear_target(&defs);
    assert_eq!(get(&w.exchange(&defs), "smelting").standing, ExchangeStanding::Known);
    // Steam engines now wait only for brickmaking, a teachable prereq.
    assert_eq!(
        get(&w.exchange(&defs), "steam_engines").standing,
        ExchangeStanding::Locked(vec![ExchangeNeed::Tech(id(&defs, "brickmaking"))])
    );
}

#[test]
fn a_nation_s_own_teachable_techs_are_not_borrowed() {
    let defs = Defs::bundled();
    let brick = id(&defs, "brickmaking");
    let spots: Vec<Pos> = (0..4).map(|x| Pos::new(x * 2, 4)).collect();
    let mut w = World::with_map(1, field(20, 8), Nation::Fire, &spots, &defs);
    w.endeavor = EndeavorState::Complete { tick: 0 };
    assert!(defs.tech(brick).borrowed_from(Nation::Fire).is_none());
    // No residents asked of Fire's own city, and its prereqs are all needed.
    w.apply(Command::SetResearch { tech: brick }, &defs).unwrap();
    assert!(w.research().queue().contains(&id(&defs, "forging")));
    assert!(w.exchange(&defs).borrowed_from(Nation::Fire).next().is_none());
}
