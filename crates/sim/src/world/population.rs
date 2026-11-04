//! Population growth (issue #232, `docs/design/endgame.md` "Population growth"): a settlement
//! that still has room and food to spare draws newcomers, so the end game's population sign
//! isn't stuck at the starting six forever.
//!
//! Every `[growth].arrival_minutes` (`data/needs.toml`), if a bed nobody owns is free and at
//! least `food_per_newcomer` food items sit in stores, one newcomer joins: an ordinary unit
//! (`Unit`, no `guest`) added to `World::units`, on the passable map-edge tile nearest the
//! settlement (ties by `Pos`). It is named from the nation's pool, drawn from its own RNG
//! stream (`rng::stream(seed, "population.names")`, kept separate from the starting settlers'
//! `world.names` shuffle, since growth draws one name at a time for as long as the game runs);
//! its character variant is picked the same way `World::spawn_units` picks the starting
//! settlers', from the shared `world.units` stream. From there it is a unit like any other: the
//! normal need and idle-wander code (`World::wander`) is what actually walks it inward, and
//! nobody assigns it a bed or a job.
//!
//! **"Nearest the settlement"** is `World::start()` (where the player chose to embark, #289)
//! when the world has one; a world made by `World::new` or a save from before #289 has none, so
//! this falls back to the mean position of its living units instead (there is always at least
//! one, or there would be nobody left to grow a population for).
//!
//! This never runs out of unit ids to hand out: `World::next_unit` is a monotonic counter, never
//! reused even by a unit that later dies (`World::load` restores its invariant for a save from
//! before this counter existed).

use super::*;

/// The stream newcomers' names are drawn from. A saved world that predates population growth
/// loads with this seeded by 0 instead of the world seed, as `orchard_rng` does for #142; still
/// deterministic.
pub(super) fn default_rng() -> SimRng {
    rng::stream(0, "population.names")
}

pub(super) fn new_rng(seed: u64) -> SimRng {
    rng::stream(seed, "population.names")
}

/// A newcomer who joined the settlement (`World::tick_population`).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Arrival {
    pub tick: u64,
    pub unit: UnitId,
    pub name: String,
}

impl World {
    /// Every newcomer who has joined so far, oldest first.
    pub fn arrivals(&self) -> &[Arrival] {
        &self.arrivals
    }

    /// Restores `next_unit`'s invariant (past every id a living or dead unit has): a no-op for
    /// a save written since #232, since it is already past every one of those; a save from
    /// before it existed deserializes the field as 0, which this fixes from the units and
    /// deaths already on file. Called once after `World::load`.
    pub(crate) fn fixup_next_unit(&mut self) {
        let used =
            self.units.iter().map(|u| u.id.0).chain(self.deaths.iter().map(|d| d.id.0)).max();
        if let Some(m) = used {
            self.next_unit = self.next_unit.max(m + 1);
        }
    }

    /// Checks population growth once every `arrival_minutes` and joins one newcomer if a bed
    /// and enough food are both there for it. Named `tick_population` (not `tick_growth`,
    /// already resource regrowth's name in `work.rs`).
    pub(super) fn tick_population(&mut self, defs: &Defs) {
        let growth = defs.needs.growth;
        let interval = (growth.arrival_minutes * 60.0 * TICKS_PER_SECOND as f32).round() as u64;
        if interval == 0 || !self.tick.is_multiple_of(interval) {
            return;
        }
        if !self.has_free_bed(defs) || self.stored_food(defs) < growth.food_per_newcomer {
            return;
        }
        self.spawn_newcomer(defs);
    }

    /// Whether some bed somewhere has room for one more sleeper nobody has claimed yet. Mirrors
    /// `bed_route`'s own capacity rule (a 1x1 bed holds one sleeper keyed by its own tile; a
    /// bigger one holds `capacity`, keyed by its anchor), but existence is all growth cares
    /// about: no particular unit is looking for it, and reachability doesn't matter yet either.
    fn has_free_bed(&self, defs: &Defs) -> bool {
        self.buildings().any(|(foot, id)| {
            let Some(_bed) = defs.building(id).bed else { return false };
            let big = foot.w > 1 || foot.h > 1;
            let limit = if big { defs.building(id).capacity } else { 1 };
            if limit == 0 {
                return false;
            }
            let claimed = self.units.iter().filter(|u| u.bed == Some(foot.anchor)).count() as u32;
            claimed < limit
        })
    }

    /// How many food items (anything `Defs::food_of` recognizes) are sitting in stockpiles.
    fn stored_food(&self, defs: &Defs) -> u32 {
        self.stored.values().filter(|s| defs.food_of(s.item).is_some()).map(|s| s.count).sum()
    }

    /// `World::start()` if the world was founded at a chosen site, else the mean position of
    /// its living units (see the module doc).
    fn settlement_anchor(&self) -> Pos {
        if let Some(start) = self.start {
            return start;
        }
        let (n, sx, sy) = self.units.iter().fold((0i64, 0i64, 0i64), |(n, sx, sy), u| {
            (n + 1, sx + i64::from(u.pos.x), sy + i64::from(u.pos.y))
        });
        if n == 0 {
            return Pos::new(self.map.width() / 2, self.map.height() / 2);
        }
        Pos::new((sx / n) as i32, (sy / n) as i32)
    }

    /// The passable tile on the map's rectangular edge nearest `anchor` (squared distance, ties
    /// by `Pos`), if there is one.
    fn nearest_edge_tile(&self, anchor: Pos) -> Option<Pos> {
        let (w, h) = (self.map.width(), self.map.height());
        let perimeter = (0..w)
            .flat_map(move |x| [Pos::new(x, 0), Pos::new(x, h - 1)])
            .chain((0..h).flat_map(move |y| [Pos::new(0, y), Pos::new(w - 1, y)]));
        perimeter.filter(|&p| self.map.get(p).is_some_and(|t| self.rules.passable(t))).min_by_key(
            |&p| {
                let d2 = (p.x - anchor.x).pow(2) + (p.y - anchor.y).pow(2);
                (d2, p)
            },
        )
    }

    /// Adds one newcomer on the passable map-edge tile nearest the settlement, named from the
    /// nation's pool. Does nothing if there is no such tile (every real map's forced sea border,
    /// `MapGenConfig::water_border`, can make that so; it isn't this check's business to pick
    /// a different rule).
    fn spawn_newcomer(&mut self, defs: &Defs) {
        let Some(pos) = self.nearest_edge_tile(self.settlement_anchor()) else { return };
        let nation = defs.nation(self.nation);
        let name = nation.names[self.growth_rng.random_range(0..nation.names.len())].clone();
        let variant = self.rng.random_range(0..nation.characters.len() as u8);
        let id = UnitId(self.next_unit);
        self.next_unit += 1;
        self.units.push(Unit {
            id,
            name: name.clone(),
            nation: None,
            pos,
            variant,
            needs: Needs::full(&defs.needs),
            chi: defs.chi.max,
            carrying: None,
            need_task: None,
            job_task: None,
            route: None,
            work: 0.0,
            bed: None,
            need_retry_at: [0; 3],
            claim_retry_at: 0,
            move_progress: 0.0,
            idle_until: 0,
            wander_left: 0,
            bends: 0,
            inside: None,
        });
        self.arrivals.push(Arrival { tick: self.tick, unit: id, name });
    }
}

#[cfg(test)]
mod tests;
