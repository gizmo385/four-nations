//! A read-only view of a unit's walk, so a frontend can draw it between tiles (#194).
//!
//! The sim moves a unit a whole tile at a time: `walk` adds `move_speed` to the unit's
//! `move_progress` every tick and, once that reaches what the next step needs (6 ticks at speed
//! 1.0 for a straight step, more for a diagonal or a costly terrain), puts the unit on the next
//! tile. Between those moments the unit is, in effect, part way along the step. `Stride` hands out
//! that part-way state (the tiles ahead on its route, what each step needs, the progress made and
//! the progress gained per tick) without letting anyone change it. Nothing in the sim reads it, so
//! it cannot change what happens.
//!
//! An idle wander (`World::wander`) has no stride: its next tile is drawn from the random stream
//! only on the tick the step lands, so there is nothing to look ahead to.

use super::work::step_needed;
use super::*;

/// The most steps of a route a [`Stride`] looks ahead. A unit takes at most one step a tick, and
/// a frontend looks a few ticks ahead at most, so this is generous.
pub const STRIDE_LOOKAHEAD: usize = 8;

/// How far along its route a walking unit is.
#[derive(Debug, Clone, PartialEq)]
pub struct Stride {
    /// The tile the unit stands on, then the next tiles of its route, nearest first, at most
    /// `STRIDE_LOOKAHEAD` of them. Always at least two tiles, each one step from the last.
    pub tiles: Vec<Pos>,
    /// What the step onto `tiles[i + 1]` needs, in the units of `progress`: one entry per step.
    pub needed: Vec<f32>,
    /// Progress toward the first step. It can exceed `needed[0]` by a little when the step is
    /// about to land (it lands on the next tick).
    pub progress: f32,
    /// Progress gained per tick (the `move_speed` modifier).
    pub speed: f32,
}

impl World {
    /// The walk unit `id` is on, if it is following a route. `None` for a unit that stands, works,
    /// sleeps or wanders.
    pub fn stride(&self, id: UnitId, defs: &Defs) -> Option<Stride> {
        let u = self.unit(id)?;
        let route = u.route.as_ref()?;
        let mut tiles = vec![u.pos];
        let mut needed = Vec::new();
        for &next in route.iter().rev().take(STRIDE_LOOKAHEAD) {
            let from = *tiles.last()?;
            needed.push(step_needed(self.rules_for(u), &self.map, from, next));
            tiles.push(next);
        }
        if needed.is_empty() {
            return None;
        }
        let speed = self.modifiers_for(u, defs).move_speed;
        Some(Stride { tiles, needed, progress: u.move_progress, speed })
    }

    /// Straight steps a unit takes per tick over plain ground (the `move_speed` modifier over the
    /// ticks a baseline step takes): how fast an idle wanderer, which has no [`Stride`], goes.
    pub fn steps_per_tick(&self) -> f32 {
        self.modifiers.move_speed / BASE_TICKS_PER_STEP
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::defs::{BiomeId, Terrain};
    use crate::geom::Grid;
    use crate::mapgen::Tile;

    fn field() -> TileMap {
        Grid::from_fn(31, 21, |_| Tile {
            elevation: 100,
            temperature: 128,
            moisture: 128,
            terrain: Terrain::Land,
            biome: BiomeId(0),
            resource: None,
            building: None,
        })
    }

    fn world(defs: &Defs) -> World {
        World::with_map(1, field(), Nation::Earth, &[Pos::new(5, 10)], defs)
    }

    #[test]
    fn a_unit_that_is_not_walking_has_no_stride() {
        let defs = Defs::bundled();
        let w = world(&defs);
        assert!(w.stride(UnitId(0), &defs).is_none());
        assert!(w.stride(UnitId(99), &defs).is_none(), "no such unit");
    }

    #[test]
    fn a_stride_says_what_the_next_ticks_of_the_walk_will_do() {
        let defs = Defs::bundled();
        let mut w = world(&defs);
        let tree = Pos::new(15, 10);
        w.map[tree].resource = defs.resource_id("oak_tree");
        w.units[0].idle_until = u64::MAX;
        w.designate_harvest(tree, &defs).unwrap();
        w.tick(&defs);
        let id = UnitId(0);

        // Every tick of the walk, the stride's prediction of the next one holds: the unit stays
        // on its tile while progress is short of the step, and moves on to the stride's next tile
        // once it isn't.
        let mut landed = 0;
        for _ in 0..80 {
            let Some(before) = w.stride(id, &defs) else { break };
            assert_eq!(before.tiles[0], w.units[0].pos);
            assert_eq!(before.speed, w.modifiers().move_speed);
            w.tick(&defs);
            let at = w.units[0].pos;
            if before.progress + before.speed >= before.needed[0] {
                assert_eq!(at, before.tiles[1], "the step lands");
                landed += 1;
            } else {
                assert_eq!(at, before.tiles[0], "still short of the step");
            }
        }
        assert!(landed >= 8, "it walked a long way: {landed} steps");
    }

    #[test]
    fn the_route_ahead_is_listed_nearest_first_and_cut_off_after_the_lookahead() {
        let defs = Defs::bundled();
        let mut w = world(&defs);
        w.units[0].route = Some((6..=20).rev().map(|x| Pos::new(x, 10)).collect());
        let s = w.stride(UnitId(0), &defs).unwrap();
        assert_eq!(s.tiles.len(), STRIDE_LOOKAHEAD + 1);
        assert_eq!(s.needed, vec![BASE_TICKS_PER_STEP; STRIDE_LOOKAHEAD]);
        assert_eq!(&s.tiles[..3], &[Pos::new(5, 10), Pos::new(6, 10), Pos::new(7, 10)]);
    }

    #[test]
    fn diagonals_and_costly_terrain_need_more_progress() {
        let defs = Defs::bundled();
        let mut w = world(&defs);
        w.units[0].route = Some(vec![Pos::new(7, 11), Pos::new(6, 10)]);
        let s = w.stride(UnitId(0), &defs).unwrap();
        assert_eq!(s.needed[0], BASE_TICKS_PER_STEP);
        assert!(s.needed[1] > BASE_TICKS_PER_STEP, "a diagonal takes longer: {:?}", s.needed);

        // A terrain that costs double takes double.
        w.rules = w.rules.clone().with_cost(Terrain::Coast, Some(2));
        w.cart_rules = w.cart_rules.clone().with_cost(Terrain::Coast, Some(2));
        w.map[Pos::new(6, 10)].terrain = Terrain::Coast;
        w.units[0].route = Some(vec![Pos::new(6, 10)]);
        assert_eq!(w.stride(UnitId(0), &defs).unwrap().needed, [2.0 * BASE_TICKS_PER_STEP]);
    }

    #[test]
    fn a_stride_over_rail_matches_the_walk_it_predicts() {
        let defs = Defs::bundled();
        let mut w = world(&defs);
        let rail = defs.building_id("rail").unwrap();
        for x in 5..=20 {
            w.map[Pos::new(x, 10)].building =
                Some(crate::building::Building::new(rail, true, true));
        }
        let wood = defs.item_id("wood").unwrap();
        w.units[0].carrying = Some(crate::jobs::Stack { item: wood, count: 2 });
        w.units[0].route = Some((6..=20).rev().map(|x| Pos::new(x, 10)).collect());
        let first = w.stride(UnitId(0), &defs).unwrap();
        assert_eq!(first.needed[0], BASE_TICKS_PER_STEP / 2.0, "the cart runs the rail");
        let mut landed = 0;
        for _ in 0..40 {
            let Some(before) = w.stride(UnitId(0), &defs) else { break };
            w.walk(0, &defs);
            let at = w.units[0].pos;
            if before.progress + before.speed >= before.needed[0] {
                assert_eq!(at, before.tiles[1]);
                landed += 1;
            } else {
                assert_eq!(at, before.tiles[0]);
            }
        }
        // Three ticks a step instead of six: 40 ticks cover well over the 7 steps of plain ground.
        assert!(landed >= 9, "{landed} steps in 40 ticks");
    }

    #[test]
    fn wander_speed_follows_the_move_speed_modifier() {
        let defs = Defs::bundled();
        let w = world(&defs);
        assert_eq!(w.steps_per_tick(), w.modifiers().move_speed / BASE_TICKS_PER_STEP);
    }
}
