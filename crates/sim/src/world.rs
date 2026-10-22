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
            rng: rng::stream(cfg.seed, "world.units"),
        };
        world.spawn_starting_units(starting_units, defs);
        world
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
}
