//! The Water Tribe branch of the tech tree (issue #27): what each tech unlocks, and a headless
//! playtest that researching the whole starter tree lands inside docs/design/research.md's
//! pacing targets.

use fn_sim::research::{MILLIS_PER_POINT, Research, effective_modifiers};
use fn_sim::world::{BuildError, Command, CommandError};
use fn_sim::{Defs, MapGenConfig, Nation, Pos, World};

const MINUTE: u64 = 600; // ticks

/// Every Water branch tech either unlocks a concrete building, job or ability, or is listed
/// here with the reason it doesn't. A tech that gains an unlock must leave this list.
const NO_CONCRETE_UNLOCK: &[(&str, &str)] = &[
    ("irrigation", "modifier-only: the channel needs farm plots (#126)"),
    ("healing", "needs a healing mechanic for the hut, healer and Healing Water (#125)"),
    ("tide_mills", "modifier-only: no power or mill system yet (#127)"),
    ("water_travel", "needs a rule for crossing water tiles (#127)"),
    ("spirit_oasis", "modifier-only: needs chi and mood regeneration (#127)"),
];

#[test]
fn every_water_branch_tech_unlocks_something_or_says_why_not() {
    let defs = Defs::bundled();
    let branch: Vec<_> = defs.techs.iter().filter(|t| t.nation == Some(Nation::Water)).collect();
    assert_eq!(branch.len(), 6, "the design doc's six-tech branch");
    for tech in branch {
        let u = &tech.unlocks;
        let concrete = !(u.buildings.is_empty() && u.jobs.is_empty() && u.abilities.is_empty());
        let excused = NO_CONCRETE_UNLOCK.iter().any(|(id, _)| *id == tech.id);
        assert_ne!(
            concrete,
            excused,
            "{}: {}",
            tech.id,
            if concrete { "unlocks something but is listed as excused" } else { "unlocks nothing" }
        );
    }
}

#[test]
fn ice_construction_unlocks_the_igloo_for_water_only() {
    let defs = Defs::bundled();
    let igloo = defs.building_id("igloo").expect("igloo is data");
    let ice = defs.tech_id("ice_construction").unwrap();
    assert_eq!(defs.building(igloo).unlocked_by, Some(ice));
    assert!(defs.building(igloo).bed.is_some(), "an igloo is somewhere to sleep");

    // Locked at the start.
    let cfg = MapGenConfig { seed: 7, ..Default::default() };
    let mut w = World::new(&cfg, Nation::Water, 4, &defs);
    let near = w.units[0].pos;
    let try_build = |w: &mut World| {
        let mut last = None;
        for dy in -3..=3 {
            for dx in -3..=3 {
                let pos = Pos::new(near.x + dx, near.y + dy);
                match w.apply(Command::DesignateBuild { pos, building: igloo }, &defs) {
                    Err(CommandError::Build(BuildError::CannotBuild | BuildError::OutOfBounds)) => {
                    }
                    other => {
                        last = Some(other);
                        break;
                    }
                }
            }
            if last.is_some() {
                break;
            }
        }
        last.expect("some tile near the start is buildable")
    };
    assert_eq!(
        try_build(&mut w),
        Err(CommandError::Build(BuildError::NotResearched(ice))),
        "locked until ice_construction"
    );

    // Research it (shelter first) on the passive trickle alone.
    w.apply(Command::SetResearch { tech: ice }, &defs).unwrap();
    for _ in 0..60 * MINUTE {
        w.tick(&defs);
        if w.research().is_known(ice) {
            break;
        }
    }
    assert!(w.research().is_known(ice), "145 points at 3 a minute fit in an hour");
    assert_eq!(try_build(&mut w), Ok(()), "unlocked once known");
}

/// Plays the design doc's pacing assumptions for one settlement without the world around it: the
/// first study hall is up at minute 5 with one scholar, two scholars from minute 15, three from
/// minute 40, each about 2/3 effective because of needs. The player always researches the
/// cheapest tech available, so `writing` (study x1.25) lands early.
struct Playtest<'a> {
    defs: &'a Defs,
    research: Research,
    tick: u64,
    /// Everything ever added to the pool, in points.
    earned_millis: u64,
    /// Fractional millis a scholar earned that haven't made a whole milli yet.
    carry: f64,
}

impl<'a> Playtest<'a> {
    fn new(nation: Nation, defs: &'a Defs) -> Self {
        Self { defs, research: Research::new(nation, defs), tick: 0, earned_millis: 0, carry: 0.0 }
    }

    fn scholars(&self) -> f64 {
        match self.tick / MINUTE {
            0..5 => 0.0,
            5..15 => 1.0,
            15..40 => 2.0,
            _ => 3.0,
        }
    }

    /// The cheapest tech this nation can research whose prereqs are known (ties: file order).
    fn next_target(&self, nation: Nation) -> Option<fn_sim::research::TechId> {
        self.defs
            .techs_for(nation)
            .filter(|(id, t)| {
                !self.research.is_known(*id) && t.prereqs.iter().all(|p| self.research.is_known(*p))
            })
            .min_by_key(|(id, t)| (t.cost, id.0))
            .map(|(id, _)| id)
    }

    /// Runs until nothing is left to research. Returns the minutes it took, and the points
    /// earned by minute 60.
    fn run(&mut self, nation: Nation) -> (f64, f64) {
        let mut at_hour = 0.0;
        loop {
            if self.research.queue().is_empty() {
                match self.next_target(nation) {
                    Some(t) => self.research.set_target(t, nation, self.defs).unwrap(),
                    None => return (self.tick as f64 / MINUTE as f64, at_hour),
                }
            }
            assert!(self.tick < 600 * MINUTE, "research never finishes");
            if self.tick == 60 * MINUTE {
                at_hour = self.earned_millis as f64 / MILLIS_PER_POINT as f64;
            }
            // Modifiers change as techs complete, so read them fresh every tick.
            let mods = effective_modifiers(nation, self.research.known_techs(), self.defs);
            let per_scholar =
                self.defs.research.study_millis_per_tick as f64 * f64::from(mods.study) * 2.0 / 3.0;
            self.carry += self.scholars() * per_scholar;
            let whole = self.carry.floor();
            self.carry -= whole;
            self.research.add_knowledge(whole as u64);
            self.earned_millis += whole as u64 + self.defs.research.passive_millis_per_tick;
            self.tick += 1;
            self.research.tick(self.tick, self.defs);
        }
    }
}

#[test]
fn a_water_settlement_researches_its_whole_tree_on_the_design_docs_pacing() {
    let defs = Defs::bundled();
    let mut play = Playtest::new(Nation::Water, &defs);
    let (minutes, at_hour) = play.run(Nation::Water);

    // Every tech Water can see is known: 11 shared (fishing from the start) plus the branch.
    for (id, t) in defs.techs_for(Nation::Water) {
        assert!(play.research.is_known(id), "{} was never researched", t.id);
    }
    let total: u32 = defs.techs_for(Nation::Water).map(|(_, t)| t.cost).sum();
    assert_eq!(total, 2190, "the doc's ~2200 for the whole tree; update the doc if costs change");

    // "2-2.5 h: the whole starter tree for that nation", within 25%.
    let (low, high) = (120.0 * 0.75, 150.0 * 1.25);
    assert!((low..=high).contains(&minutes), "took {minutes:.0} min, want {low}..={high}");
    // "60 min: ~1000 knowledge earned", within 25%.
    assert!((750.0..=1250.0).contains(&at_hour), "{at_hour:.0} points by minute 60");
}

#[test]
fn the_playtest_is_deterministic() {
    let defs = Defs::bundled();
    let run = || {
        let mut play = Playtest::new(Nation::Water, &defs);
        play.run(Nation::Water);
        play.research.completions().to_vec()
    };
    assert_eq!(run(), run());
}
