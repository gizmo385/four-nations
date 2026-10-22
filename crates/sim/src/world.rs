//! The live game state and its fixed-step update.
//!
//! The sim advances in discrete ticks (`World::tick`). Clients render whatever state the world is
//! in and may interpolate between ticks, but must never mutate it except through commands.

use rand::RngExt;
use rand::seq::SliceRandom;

use crate::defs::Defs;
use crate::geom::Pos;
use crate::mapgen::{self, MapGenConfig, TileMap};
use crate::nation::Nation;
use crate::research::{Research, ResearchError, TechId};
use crate::rng::{self, SimRng};

/// Ticks per in-game second. Movement and needs are expressed per tick.
pub const TICKS_PER_SECOND: u32 = 10;

/// Ticks a baseline (move_speed 1.0) unit waits between steps.
const BASE_TICKS_PER_STEP: f32 = 6.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct UnitId(pub u32);

#[derive(Debug, Clone, PartialEq)]
pub struct Unit {
    pub id: UnitId,
    pub name: String,
    pub nation: Nation,
    pub pos: Pos,
    /// Which of the nation's sprite variants to draw. Purely cosmetic.
    pub variant: u8,
    /// Progress toward the next step; a step happens when this reaches `BASE_TICKS_PER_STEP`.
    move_progress: f32,
}

#[derive(Debug, Clone)]
pub struct World {
    pub seed: u64,
    pub tick: u64,
    pub map: TileMap,
    pub nation: Nation,
    pub units: Vec<Unit>,
    research: Research,
    rng: SimRng,
}

impl World {
    pub fn new(cfg: &MapGenConfig, nation: Nation, starting_units: u32, defs: &Defs) -> Self {
        let map = mapgen::generate(cfg, defs);
        let mut world = Self {
            seed: cfg.seed,
            tick: 0,
            map,
            nation,
            units: Vec::new(),
            research: Research::new(nation, defs),
            rng: rng::stream(cfg.seed, "world.units"),
        };
        world.spawn_starting_units(starting_units, defs);
        world
    }

    pub fn research(&self) -> &Research {
        &self.research
    }

    /// Sets the research target: the queue becomes `tech` plus its unknown prereqs. Banked
    /// knowledge is kept. Becomes `Command::SetResearch` when the command enum lands (#5).
    pub fn set_research(&mut self, tech: TechId, defs: &Defs) -> Result<(), ResearchError> {
        self.research.set_target(tech, self.nation, defs)
    }

    /// Places units on walkable tiles, spiralling out from the map center.
    fn spawn_starting_units(&mut self, count: u32, defs: &Defs) {
        let mut names = defs.nation(self.nation).names.clone();
        names.shuffle(&mut rng::stream(self.seed, "world.names"));
        let center = Pos::new(self.map.width() / 2, self.map.height() / 2);
        let mut candidates: Vec<Pos> = self
            .map
            .iter()
            .filter(|(_, t)| t.terrain.is_walkable() && t.resource.is_none())
            .map(|(p, _)| p)
            .collect();
        candidates.sort_by_key(|p| ((p.x - center.x).pow(2) + (p.y - center.y).pow(2), *p));
        for (i, pos) in candidates.into_iter().take(count as usize).enumerate() {
            let variant = self.rng.random_range(0..3);
            // Reuse the pool with a numeral suffix if there are more units than names.
            let name = match i / names.len() {
                0 => names[i % names.len()].clone(),
                n => format!("{} {}", names[i % names.len()], n + 1),
            };
            self.units.push(Unit {
                id: UnitId(i as u32),
                name,
                nation: self.nation,
                pos,
                variant,
                move_progress: 0.0,
            });
        }
    }

    fn walkable(&self, p: Pos) -> bool {
        self.map.get(p).is_some_and(|t| t.terrain.is_walkable() && t.resource.is_none())
    }

    /// Advances the world by one tick. Placeholder behaviour: units wander randomly, at a speed
    /// set by their nation's `move_speed` modifier.
    pub fn tick(&mut self, defs: &Defs) {
        self.tick += 1;
        self.research.tick(self.tick, defs);
        for i in 0..self.units.len() {
            let speed = defs.nation(self.units[i].nation).modifiers.move_speed;
            self.units[i].move_progress += speed;
            if self.units[i].move_progress < BASE_TICKS_PER_STEP {
                continue;
            }
            self.units[i].move_progress -= BASE_TICKS_PER_STEP;
            let pos = self.units[i].pos;
            let options: Vec<Pos> =
                self.map.neighbors(pos, false).filter(|&q| self.walkable(q)).collect();
            if !options.is_empty() {
                self.units[i].pos = options[self.rng.random_range(0..options.len())];
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::research::TechCompleted;

    fn world(nation: Nation) -> (World, Defs) {
        let defs = Defs::bundled();
        let cfg = MapGenConfig { seed: 7, ..Default::default() };
        (World::new(&cfg, nation, 4, &defs), defs)
    }

    #[test]
    fn units_spawn_on_walkable_tiles() {
        let (w, _) = world(Nation::Earth);
        assert_eq!(w.units.len(), 4);
        assert!(w.units.iter().all(|u| w.walkable(u.pos)));
    }

    #[test]
    fn units_get_unique_deterministic_names() {
        let (a, _) = world(Nation::Fire);
        let (b, _) = world(Nation::Fire);
        let names: Vec<_> = a.units.iter().map(|u| u.name.clone()).collect();
        assert_eq!(names, b.units.iter().map(|u| u.name.clone()).collect::<Vec<_>>());
        let unique: std::collections::BTreeSet<_> = names.iter().collect();
        assert_eq!(unique.len(), names.len());
    }

    #[test]
    fn units_stay_on_walkable_tiles() {
        let (mut w, defs) = world(Nation::Water);
        for _ in 0..500 {
            w.tick(&defs);
        }
        assert!(w.units.iter().all(|u| w.walkable(u.pos)));
    }

    #[test]
    fn airbenders_cover_more_ground_than_earthbenders() {
        // Count steps taken: air's move_speed modifier is higher than earth's.
        let steps = |nation| {
            let (mut w, defs) = world(nation);
            let mut moved = 0;
            for _ in 0..600 {
                let before: Vec<Pos> = w.units.iter().map(|u| u.pos).collect();
                w.tick(&defs);
                moved += w.units.iter().zip(&before).filter(|(u, b)| u.pos != **b).count();
            }
            moved
        };
        assert!(steps(Nation::Air) > steps(Nation::Earth));
    }

    fn tech(defs: &Defs, id: &str) -> TechId {
        defs.tech_id(id).unwrap()
    }

    fn spent_millis(w: &World, defs: &Defs) -> u64 {
        w.research().completions().iter().map(|c| u64::from(defs.tech(c.tech).cost) * 1000).sum()
    }

    #[test]
    fn research_starts_with_the_nations_starting_techs() {
        let (w, defs) = world(Nation::Fire);
        let known: Vec<_> = w.research().known_techs().collect();
        assert_eq!(known, [tech(&defs, "fire_making")]);
        assert!(w.research().queue().is_empty());
        assert_eq!(w.research().pool_millis(), 0);
    }

    #[test]
    fn passive_rate_is_converted_to_millis_per_tick() {
        // 3 and 9 points a minute at 10 ticks a second.
        assert_eq!(Defs::bundled().research.passive_millis_per_tick, 5);
        assert_eq!(Defs::bundled().research.study_millis_per_tick, 15);
    }

    #[test]
    fn fire_making_completes_when_the_pacing_math_says() {
        let (mut w, defs) = world(Nation::Earth);
        let fm = tech(&defs, "fire_making");
        w.set_research(fm, &defs).unwrap();
        // 25 points at 3 a minute is 500 seconds, so tick 5000.
        for _ in 0..4999 {
            w.tick(&defs);
        }
        assert!(!w.research().is_known(fm));
        w.tick(&defs);
        assert!(w.research().is_known(fm));
        assert_eq!(w.research().completions(), [TechCompleted { tech: fm, tick: 5000 }]);
        assert!(w.research().queue().is_empty());
        assert_eq!(w.research().pool_millis(), 0);
    }

    #[test]
    fn deep_tech_queues_its_unknown_prereq_chain() {
        let (mut w, defs) = world(Nation::Earth);
        let target = tech(&defs, "earthen_walls");
        w.set_research(target, &defs).unwrap();
        let mut expected: Vec<TechId> =
            ["shelter", "carpentry", "masonry", "earth_shaping", "earthen_walls"]
                .iter()
                .map(|n| tech(&defs, n))
                .collect();
        expected.sort();
        // stoneworking is a starting tech, so it isn't queued.
        assert_eq!(w.research().queue(), expected);
        assert_eq!(w.research().queue().last(), Some(&target));
    }

    #[test]
    fn set_research_rejects_other_nations_and_known_techs() {
        let (mut w, defs) = world(Nation::Earth);
        assert_eq!(
            w.set_research(tech(&defs, "lightning_generation"), &defs),
            Err(ResearchError::WrongNation)
        );
        assert_eq!(
            w.set_research(tech(&defs, "stoneworking"), &defs),
            Err(ResearchError::AlreadyKnown)
        );
        assert_eq!(w.set_research(TechId(u16::MAX), &defs), Err(ResearchError::UnknownTech));
        assert!(w.research().queue().is_empty());
    }

    #[test]
    fn knowledge_banks_while_idle_and_carries_over() {
        let (mut w, defs) = world(Nation::Earth);
        for _ in 0..6000 {
            w.tick(&defs);
        }
        assert_eq!(w.research().pool_millis(), 30_000);
        w.set_research(tech(&defs, "shelter"), &defs).unwrap();
        w.tick(&defs);
        // 30_005 in the pool, minus shelter's 25_000.
        assert_eq!(w.research().pool_millis(), 5_005);
        assert_eq!(w.research().completions().len(), 1);
    }

    #[test]
    fn several_techs_can_complete_in_one_tick() {
        let (mut w, defs) = world(Nation::Earth);
        // 100 points banked pays for shelter (25) and carpentry (60) together.
        for _ in 0..20_000 {
            w.tick(&defs);
        }
        w.set_research(tech(&defs, "carpentry"), &defs).unwrap();
        w.tick(&defs);
        let ticks: Vec<_> = w.research().completions().iter().map(|c| c.tick).collect();
        assert_eq!(ticks, [20_001, 20_001]);
        assert!(w.research().is_known(tech(&defs, "carpentry")));
    }

    #[test]
    fn changing_target_loses_no_knowledge() {
        let (mut w, defs) = world(Nation::Earth);
        w.set_research(tech(&defs, "fire_making"), &defs).unwrap();
        for _ in 0..3000 {
            w.tick(&defs);
        }
        let before = w.research().pool_millis();
        w.set_research(tech(&defs, "masonry"), &defs).unwrap();
        assert_eq!(w.research().pool_millis(), before);
        for _ in 0..9000 {
            w.tick(&defs);
        }
        // Everything earned is either still banked or was spent on a completed tech.
        assert_eq!(w.research().pool_millis() + spent_millis(&w, &defs), 5 * 12_000);
        assert!(!w.research().completions().is_empty());
    }

    #[test]
    fn research_is_deterministic() {
        let run = || {
            let (mut w, defs) = world(Nation::Water);
            w.set_research(tech(&defs, "healing"), &defs).unwrap();
            for t in 0..20_000 {
                if t == 7_000 {
                    w.set_research(tech(&defs, "water_travel"), &defs).unwrap();
                }
                w.tick(&defs);
            }
            w.research().clone()
        };
        assert_eq!(run(), run());
    }
}
