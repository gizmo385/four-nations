//! Headless playtest of the Fire Nation research branch (issue #26): a settlement raises an
//! Academy, studies its way through the whole Fire tree, and each branch tech unlocks what
//! `data/research.toml` says it does. Pacing targets: docs/design/research.md, "Pacing targets".

use fn_sim::flood::Flood;
use fn_sim::research::TechId;
use fn_sim::world::{BuildError, Command, CommandError};
use fn_sim::{Defs, MapGenConfig, MoveRules, Nation, Pos, World};

const MINUTE: u64 = 600; // ticks

/// The Fire branch in tree order, with the buildings each tech must unlock. `dancing_dragon`
/// unlocks an ability (Inner Fire, #123) instead of a building.
const BRANCH: [(&str, &[&str]); 6] = [
    ("forging", &["forge"]),
    ("brickmaking", &["brick_kiln"]),
    ("smelting", &["smelter"]),
    ("dancing_dragon", &[]),
    ("steam_engines", &["steam_engine"]),
    ("lightning_generation", &["lightning_station"]),
];

fn fresh(defs: &Defs) -> World {
    let cfg = MapGenConfig { seed: 13, ..Default::default() };
    World::new(&cfg, Nation::Fire, 4, defs)
}

/// A reachable, empty tile 3 to 6 tiles from `near` (outside the stockpile around it) that isn't
/// in `avoid`.
fn free_tile_near(w: &World, near: Pos, avoid: &[Pos]) -> Pos {
    let flood = Flood::new(&w.map, near, &MoveRules::default());
    w.map
        .iter()
        .filter(|(p, t)| {
            let d = (p.x - near.x).abs().max((p.y - near.y).abs());
            t.terrain.is_walkable()
                && t.resource.is_none()
                && (3..=6).contains(&d)
                && flood.cost(*p).is_some()
        })
        .map(|(p, _)| p)
        .find(|p| !avoid.contains(p))
        .expect("a free tile near home")
}

/// Every Fire branch building is refused with the right tech until that tech is known.
fn assert_unlock_gates(w: &mut World, defs: &Defs, pos: Pos, expect_open: &dyn Fn(&str) -> bool) {
    for (tech, buildings) in BRANCH {
        let tech_id = defs.tech_id(tech).unwrap();
        for b in buildings {
            let building = defs.building_id(b).unwrap_or_else(|| panic!("{tech} unlocks {b}"));
            assert_eq!(defs.building(building).unlocked_by, Some(tech_id), "{b}");
            let result = w.apply(Command::DesignateBuild { pos, building }, defs);
            if expect_open(tech) {
                result.unwrap_or_else(|e| panic!("{b} should be open: {e}"));
                w.apply(Command::CancelBuild { pos }, defs).unwrap();
            } else {
                assert_eq!(
                    result,
                    Err(CommandError::Build(BuildError::NotResearched(tech_id))),
                    "{b} should need {tech}"
                );
            }
        }
    }
}

#[test]
fn every_fire_branch_tech_unlocks_something_or_says_why_not() {
    let defs = Defs::bundled();
    for (tech, buildings) in BRANCH {
        let def = defs.tech(defs.tech_id(tech).unwrap());
        assert_eq!(def.nation, Some(Nation::Fire), "{tech}");
        let unlocked =
            def.unlocks.buildings.len() + def.unlocks.jobs.len() + def.unlocks.abilities.len();
        assert_eq!(def.unlocks.buildings, buildings, "{tech}");
        if buildings.is_empty() {
            // The one tech with no building: it unlocks Inner Fire (#123).
            assert_eq!(tech, "dancing_dragon");
            assert_eq!(def.unlocks.abilities, ["inner_fire"]);
            assert!(
                def.unlocks.modifiers.heating.is_some(),
                "{tech} must at least raise a modifier"
            );
        } else {
            assert!(unlocked > 0, "{tech}");
        }
    }
}

/// A Fire settlement with an Academy (two seats, so two scholars beside the passive trickle)
/// researches its whole tree: every shared tech and the six-tech branch, about 2200 knowledge.
/// The design doc targets 2 to 2.5 hours for that, and the test allows 25% either side.
#[test]
fn a_fire_settlement_finishes_its_tree_within_the_pacing_targets() {
    let defs = Defs::bundled();
    let mut w = fresh(&defs);
    let home = w.units[0].pos;

    // Wood for the Academy, a stockpile to put it in, and the Academy itself.
    let flood = Flood::new(&w.map, home, &MoveRules::default());
    let reach = |p: Pos| w.map.neighbors(p, false).filter_map(|n| flood.cost(n)).min();
    let mut trees: Vec<(u32, Pos)> = w
        .map
        .iter()
        .filter(|(_, t)| {
            t.resource
                .is_some_and(|r| defs.harvest(r).is_some_and(|h| defs.item(h.item).id == "wood"))
        })
        .filter_map(|(p, _)| reach(p).map(|c| (c, p)))
        .collect();
    trees.sort();
    let trees: Vec<Pos> = trees.into_iter().take(4).map(|(_, p)| p).collect();
    assert_eq!(trees.len(), 4, "seed has too little reachable wood");
    for p in &trees {
        w.apply(Command::DesignateHarvest { pos: *p }, &defs).unwrap();
    }
    for dy in -2..=2 {
        for dx in -2..=2 {
            let _ = w.apply(
                Command::DesignateStockpile { pos: Pos::new(home.x + dx, home.y + dy) },
                &defs,
            );
        }
    }
    let spot = free_tile_near(&w, home, &trees);
    let hall = defs.building_id("study_hall").unwrap();
    w.apply(Command::DesignateBuild { pos: spot, building: hall }, &defs).unwrap();

    // Nothing in the branch is open yet.
    let gate = free_tile_near(&w, home, &[trees.as_slice(), &[spot]].concat());
    assert_unlock_gates(&mut w, &defs, gate, &|_| false);

    // Study whatever is cheapest-first by file order until the tree is done.
    let nation_techs: Vec<TechId> = (0..defs.techs.len() as u16)
        .map(TechId)
        .filter(|&t| defs.tech(t).nation.is_none_or(|n| n == Nation::Fire))
        .collect();
    let mut done_at = None;
    let limit = 4 * 60 * MINUTE;
    for _ in 0..limit {
        if w.research().queue().is_empty()
            && let Some(&next) = nation_techs.iter().find(|&&t| !w.research().is_known(t))
        {
            w.apply(Command::SetResearch { tech: next }, &defs).unwrap();
        }
        w.tick(&defs);
        if nation_techs.iter().all(|&t| w.research().is_known(t)) {
            done_at = Some(w.tick);
            break;
        }
    }
    let minutes = done_at.expect("the tree should finish within 4 hours") as f64 / MINUTE as f64;
    // 2 to 2.5 hours, plus or minus 25%.
    assert!((90.0..=187.5).contains(&minutes), "finished after {minutes:.1} min");
    assert!(w.deaths.is_empty(), "{:?}", w.deaths);

    // The branch finished in prerequisite order, and everything it unlocks is now open.
    let tick_of = |name: &str| {
        let id = defs.tech_id(name).unwrap();
        w.research().completions().iter().find(|c| c.tech == id).map(|c| c.tick).unwrap()
    };
    for t in nation_techs.iter().map(|&t| defs.tech(t)) {
        for p in &t.prereqs {
            if let Some(c) = w.research().completions().iter().find(|c| c.tech == *p) {
                assert!(c.tick <= tick_of(&t.id), "{} before its prereq", t.id);
            }
        }
    }
    assert_unlock_gates(&mut w, &defs, gate, &|_| true);
}
