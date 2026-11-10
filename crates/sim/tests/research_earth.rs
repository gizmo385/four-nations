//! The Earth Kingdom branch of the tech tree (issue #25): what each tech unlocks, and how long a
//! settlement takes to research it at the pace docs/design/research.md targets.

use fn_sim::research::{Research, TechId, effective_modifiers};
use fn_sim::world::TICKS_PER_SECOND;
use fn_sim::{Defs, Nation};

const NATION: Nation = Nation::Earth;
const TICKS_PER_MINUTE: u64 = TICKS_PER_SECOND as u64 * 60;

fn branch(defs: &Defs) -> Vec<TechId> {
    defs.techs_for(NATION).filter(|(_, t)| t.nation.is_some()).map(|(id, _)| id).collect()
}

/// Earth techs that unlock no building, job or ability yet, and why. A tech not listed here must
/// unlock at least one concrete thing; an entry here must really unlock none, so the list can't
/// go stale. The reasons repeat the comments in data/research.toml.
const WITHOUT_CONCRETE_UNLOCKS: &[(&str, &str)] = &[];

#[test]
fn every_earth_tech_unlocks_something_or_has_a_documented_reason_not_to() {
    let defs = Defs::bundled();
    let techs = branch(&defs);
    assert_eq!(techs.len(), 6, "the Earth branch has six techs");
    for t in techs {
        let def = defs.tech(t);
        let u = &def.unlocks;
        let concrete = !(u.buildings.is_empty() && u.jobs.is_empty() && u.abilities.is_empty());
        let excused = WITHOUT_CONCRETE_UNLOCKS.iter().any(|(id, _)| *id == def.id);
        assert_ne!(
            concrete,
            excused,
            "{}: {}",
            def.id,
            if concrete {
                "unlocks something but is still listed as having no concrete unlock"
            } else {
                "unlocks nothing and has no documented reason"
            }
        );
    }
}

#[test]
fn earthen_walls_unlocks_the_earth_wall() {
    let defs = Defs::bundled();
    let wall = defs.building(defs.building_id("earth_wall").expect("earth_wall is defined"));
    assert_eq!(wall.unlocked_by, defs.tech_id("earthen_walls"));
    assert!(!wall.passable, "a wall blocks movement");
}

#[test]
fn metalbending_unlocks_metalbend() {
    let defs = Defs::bundled();
    let ability = defs.ability(defs.ability_id("metalbend").expect("metalbend is defined"));
    assert_eq!(ability.nation, Nation::Earth);
    assert_eq!(ability.unlocked_by, defs.tech_id("metalbending"));
}

/// Earth Tent (#362): a stone shelter, unlocked by the same tech that speeds up Raise Earth.
#[test]
fn earth_shaping_unlocks_earth_tent() {
    let defs = Defs::bundled();
    let ability = defs.ability(defs.ability_id("earth_tent").expect("earth_tent is defined"));
    assert_eq!(ability.nation, Nation::Earth);
    assert_eq!(ability.unlocked_by, defs.tech_id("earth_shaping"));
}

#[test]
fn earth_rail_unlocks_the_rail_and_only_earth_can_research_it() {
    let defs = Defs::bundled();
    let id = defs.building_id("rail").expect("rail is defined");
    let rail = defs.building(id);
    assert_eq!(rail.unlocked_by, defs.tech_id("earth_rail"));
    assert!(rail.passable && rail.rail.is_some());
    // Nothing in the code asks which nation: the tech belongs to Earth's branch, so only Earth's
    // tree reaches it.
    let tech = defs.tech_id("earth_rail").unwrap();
    for n in Nation::ALL {
        let reachable = defs.techs_for(n).any(|(t, _)| t == tech);
        assert_eq!(reachable, n == Nation::Earth, "{n:?}");
    }
}

/// Scholars at a study hall `minute` minutes in: the design doc's pacing assumption of one by
/// minute 5, two by 15 and three by 40.
fn scholars(minute: u64) -> u64 {
    match minute {
        0..5 => 0,
        5..15 => 1,
        15..40 => 2,
        _ => 3,
    }
}

/// Knowledge earned so far, in points: what finished techs cost plus what is banked. The
/// finished techs are the known ones the nation didn't start with.
fn earned(r: &Research, defs: &Defs) -> f64 {
    let starting = &defs.nation(NATION).starting_techs;
    let spent: u64 = r
        .known_techs()
        .filter(|t| !starting.contains(t))
        .map(|t| u64::from(defs.tech(t).cost))
        .sum();
    spent as f64 + r.pool_millis() as f64 / 1000.0
}

/// Runs one tick of the settlement: the scholars' knowledge for this minute, each two-thirds
/// effective because needs and urgent work pull them from their desks, times the `study`
/// modifier the known techs give, then the research tick (which adds the passive trickle).
fn step(r: &mut Research, tick: u64, defs: &Defs) {
    let study = effective_modifiers(NATION, r.known_techs(), defs).study;
    let per_scholar = defs.research.study_millis_per_tick as f32 * study * 2.0 / 3.0;
    r.add_knowledge(per_scholar as u64 * scholars(tick / TICKS_PER_MINUTE));
    let _ = r.tick(tick, defs, 1.0);
}

const LIMIT: u64 = 4 * 60 * TICKS_PER_MINUTE;

#[test]
fn an_earth_settlement_finishes_its_whole_tree_at_the_design_docs_pace() {
    let defs = Defs::bundled();
    let mut research = Research::new(NATION, &defs);
    let mut at_hour = 0.0;
    let mut finished = None;
    for tick in 0..LIMIT {
        // Writing first (its study bonus pays for everything after), then the cheapest tech
        // left, one target at a time like a player working down a list.
        if research.queue().is_empty() {
            let next = defs
                .techs_for(NATION)
                .filter(|(id, _)| !research.is_known(*id))
                .min_by_key(|(id, t)| (t.id != "writing", t.cost, *id));
            match next {
                Some((id, _)) => research.set_target(id, NATION, &defs).unwrap(),
                None => {
                    finished = Some(tick);
                    break;
                }
            }
        }
        step(&mut research, tick, &defs);
        if tick == 60 * TICKS_PER_MINUTE {
            at_hour = earned(&research, &defs);
        }
    }
    let minutes = finished.expect("the whole tree is researched within four hours") as f64
        / TICKS_PER_MINUTE as f64;

    for t in branch(&defs) {
        assert!(research.is_known(t), "{} not researched", defs.tech(t).id);
    }
    println!("whole tree at {minutes:.1} min, {at_hour:.0} points at minute 60");
    // research.md, "Pacing targets": about 1000 points by minute 60, and the whole starter tree
    // for the nation in 2 to 2.5 hours. Both within 25%.
    assert!(
        (0.75 * 120.0..=1.25 * 150.0).contains(&minutes),
        "whole tree took {minutes:.0} min; the doc's target is 120-150 min (+/-25%)"
    );
    assert!(
        (750.0..=1250.0).contains(&at_hour),
        "{at_hour:.0} points by minute 60; the doc's target is ~1000 (+/-25%)"
    );
}

#[test]
fn going_straight_for_metalbending_finishes_inside_the_whole_tree_window() {
    // The branch plus the shared techs it needs (masonry, carpentry, shelter) costs 1505 points,
    // about two thirds of the whole tree, so it must land before the tree's 187 minute limit.
    let defs = Defs::bundled();
    let metal = defs.tech_id("metalbending").unwrap();
    let mut research = Research::new(NATION, &defs);
    research.set_target(metal, NATION, &defs).unwrap();
    let done = (0..LIMIT).find(|&tick| {
        step(&mut research, tick, &defs);
        research.is_known(metal)
    });
    let minutes = done.expect("metalbending is reachable") as f64 / TICKS_PER_MINUTE as f64;
    assert!(minutes < 1.25 * 150.0, "the branch took {minutes:.0} min");
    assert!(minutes > 0.75 * 60.0, "the branch took only {minutes:.0} min; too fast");
    println!("metalbending at {minutes:.1} min");
}
