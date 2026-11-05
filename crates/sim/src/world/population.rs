//! Population growth (issue #232, `docs/design/endgame.md` "Population growth"): a settlement
//! that still has room and food to spare draws newcomers, so the end game's population sign
//! isn't stuck at the starting six forever.
//!
//! Every `[growth].arrival_minutes` (`data/needs.toml`), if a bed nobody owns is free and at
//! least `food_per_newcomer` food items sit in stores, one newcomer joins: an ordinary unit
//! (`Unit`, no `guest`) added to `World::units`, on the outskirts: the walkable tile in the
//! settlement's own region nearest the map edge (owner decision, 2026-11-03). Mapgen rings every
//! map with sea (`MapGenConfig::water_border`), so the literal map edge is never walkable; the
//! settlement's coast farthest out is the nearest thing to "walking in from off the map", and
//! staying in the settlement's region means a newcomer never lands on another island. It is named from the nation's pool, drawn from its own RNG
//! stream (`rng::stream(seed, "population.names")`, kept separate from the starting settlers'
//! `world.names` shuffle, since growth draws one name at a time for as long as the game runs);
//! its character variant is picked the same way `World::spawn_units` picks the starting
//! settlers', from the shared `world.units` stream. From there it is a unit like any other: the
//! normal need and idle-wander code (`World::wander`) is what actually walks it inward, and
//! nobody assigns it a bed or a job.
//!
//! **The United City** (`docs/design/endgame.md`, "The transformation"; issue #240). Before the
//! Harmony Gate is complete, every newcomer is the settlement's own nation, exactly as #232
//! always worked. Once it stands, each newcomer's nation is instead drawn from `[growth]`'s
//! `own_weight` (this settlement's own nation) against `other_weight` (each of the other three),
//! from its own RNG stream (`rng::stream(seed, "endeavor.newcomers")`, kept apart from the name
//! draw above so retuning the weights doesn't reshuffle names). A newcomer of another nation
//! gets `Unit::nation` set, works with that nation's own modifiers (`World::modifiers_for`) and
//! can bend that nation's signature ability (`World::unit_can_bend`), both already per-unit from
//! #239; this module only has to decide which nation to draw and hand it to `Unit::nation`.
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
use crate::flood::Flood;
use crate::nation::NationDef;

/// The stream newcomers' names are drawn from. A saved world that predates population growth
/// loads with this seeded by 0 instead of the world seed, as `orchard_rng` does for #142; still
/// deterministic.
pub(super) fn default_rng() -> SimRng {
    rng::stream(0, "population.names")
}

pub(super) fn new_rng(seed: u64) -> SimRng {
    rng::stream(seed, "population.names")
}

/// The stream a newcomer's nation is drawn from, once the Harmony Gate is complete (#240). A
/// saved world that predates it loads with this seeded by 0 instead of the world seed, as
/// `default_rng` does for #232; still deterministic.
pub(super) fn default_newcomer_rng() -> SimRng {
    rng::stream(0, "endeavor.newcomers")
}

pub(super) fn new_newcomer_rng(seed: u64) -> SimRng {
    rng::stream(seed, "endeavor.newcomers")
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
        // The Avatar (#237) has a fixed id at u32::MAX, outside the counter's range.
        let settlers = self.units.iter().filter(|u| !u.is_avatar());
        let used = settlers.map(|u| u.id.0).chain(self.deaths.iter().map(|d| d.id.0)).max();
        if let Some(m) = used {
            self.next_unit = self.next_unit.max(m + 1);
        }
    }

    /// Checks population growth once every `arrival_minutes` and joins one newcomer if a bed
    /// and enough food are both there for it. Named `tick_population` (not `tick_growth`,
    /// already resource regrowth's name in `work.rs`). A newcomer who actually joins also takes
    /// the difficulty preset's `newcomer_food` from stores (`take_newcomer_food`, #294), on top
    /// of this threshold; identity's `newcomer_food` is 0, so nothing is taken by default.
    pub(super) fn tick_population(&mut self, defs: &Defs) {
        let growth = defs.needs.growth;
        let interval = (growth.arrival_minutes * 60.0 * TICKS_PER_SECOND as f32).round() as u64;
        if interval == 0 || !self.tick.is_multiple_of(interval) {
            return;
        }
        if !self.has_free_bed(defs) || self.stored_food(defs) < growth.food_per_newcomer {
            return;
        }
        if self.spawn_newcomer(defs) {
            let newcomer_food = self.knobs(defs).newcomer_food;
            self.take_newcomer_food(newcomer_food, defs);
        }
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
    /// Public so a headless tool (`fn_sim::autoplay`, the `balance` binary, #300) can read food
    /// per head without duplicating this filter.
    pub fn stored_food(&self, defs: &Defs) -> u32 {
        self.stored.values().filter(|s| defs.food_of(s.item).is_some()).map(|s| s.count).sum()
    }

    /// `World::start()` if the world was founded at a chosen site, else the mean position of
    /// its living units (see the module doc). Public so a headless tool (`fn_sim::autoplay`,
    /// the `balance` binary, #300) can anchor the opening's placements the same way growth
    /// anchors a newcomer's entry point.
    pub fn settlement_anchor(&self) -> Pos {
        if let Some(start) = self.start {
            return start;
        }
        let (n, sx, sy) = self
            .units
            .iter()
            .filter(|u| !u.is_avatar())
            .fold((0i64, 0i64, 0i64), |(n, sx, sy), u| {
                (n + 1, sx + i64::from(u.pos.x), sy + i64::from(u.pos.y))
            });
        if n == 0 {
            return Pos::new(self.map.width() / 2, self.map.height() / 2);
        }
        Pos::new((sx / n) as i32, (sy / n) as i32)
    }

    /// The walkable tile reachable from the settlement that is nearest the map's rectangular
    /// edge, ties by squared distance to `anchor` and then by `Pos`. The region is flooded from
    /// `start`, or from the first living unit when there is none. This runs once per arrival
    /// check (every `arrival_minutes`), so one flood and one map pass are cheap enough.
    fn nearest_edge_tile(&self, anchor: Pos) -> Option<Pos> {
        let first = self.units.iter().find(|u| !u.is_avatar());
        let origin = self.start.or_else(|| first.map(|u| u.pos))?;
        let flood = Flood::new(&self.map, origin, &self.rules);
        let (w, h) = (self.map.width(), self.map.height());
        self.map
            .positions()
            .filter(|&p| {
                flood.cost(p).is_some() && self.map.get(p).is_some_and(|t| self.rules.passable(t))
            })
            .min_by_key(|&p| {
                let edge = p.x.min(p.y).min(w - 1 - p.x).min(h - 1 - p.y);
                let d2 = (p.x - anchor.x).pow(2) + (p.y - anchor.y).pow(2);
                (edge, d2, p)
            })
    }

    /// Which nation a newcomer joining right now is. Before the Harmony Gate is complete, always
    /// the settlement's own (there is nobody else to draw from yet); after, a weighted draw
    /// between the settlement's own nation (`[growth].own_weight`) and each of the other three
    /// (`other_weight`), from `newcomer_rng` (see the module doc, #240).
    fn newcomer_nation(&mut self, defs: &Defs) -> Nation {
        if !matches!(self.endeavor, EndeavorState::Complete { .. }) {
            return self.nation;
        }
        let growth = defs.needs.growth;
        let total = growth.own_weight + 3.0 * growth.other_weight;
        let draw = self.newcomer_rng.random_range(0.0..total);
        let mut reached = 0.0;
        for n in Nation::ALL {
            reached += if n == self.nation { growth.own_weight } else { growth.other_weight };
            if draw < reached {
                return n;
            }
        }
        // Floating-point rounding could in principle leave `draw` just past the last edge; the
        // last nation in `Nation::ALL` is the only share left to give it to.
        *Nation::ALL.last().expect("Nation::ALL is never empty")
    }

    /// A name for a newcomer of `nation`: one no living unit (of any nation; #239 lets several
    /// share a settlement) already has, while the pool still has one unused, drawn from
    /// `growth_rng` so it stays deterministic per seed (issue #303 — population growth used to
    /// draw with replacement and hand out duplicates once a pool of ~20 names outran the
    /// starting roster). Once every name in the pool belongs to somebody already, draws a base
    /// name the same way and appends the lowest numeral that makes it unique, as `spawn_units`
    /// already does for a starting roster bigger than its pool.
    fn draw_newcomer_name(&mut self, nation: &NationDef) -> String {
        let taken: BTreeSet<&str> =
            self.units.iter().filter(|u| !u.is_avatar()).map(|u| u.name.as_str()).collect();
        let available: Vec<&String> =
            nation.names.iter().filter(|n| !taken.contains(n.as_str())).collect();
        if !available.is_empty() {
            let idx = self.growth_rng.random_range(0..available.len());
            return available[idx].clone();
        }
        let base = &nation.names[self.growth_rng.random_range(0..nation.names.len())];
        (2..)
            .map(|n| format!("{base} {n}"))
            .find(|c| !taken.contains(c.as_str()))
            .expect("an unbounded sequence of numeral suffixes always finds one no living unit has")
    }

    /// Adds one newcomer on the settlement's outskirts (`nearest_edge_tile`), of a nation drawn
    /// by `newcomer_nation` and named from that nation's pool (`draw_newcomer_name`). Does
    /// nothing, and returns `false`, if the settlement has no walkable tile at all; `true`
    /// otherwise, so `tick_population` only spends `newcomer_food` (#294) when someone actually
    /// joined.
    fn spawn_newcomer(&mut self, defs: &Defs) -> bool {
        let Some(pos) = self.nearest_edge_tile(self.settlement_anchor()) else { return false };
        let drawn = self.newcomer_nation(defs);
        let nation = defs.nation(drawn);
        let name = self.draw_newcomer_name(nation);
        let variant = self.rng.random_range(0..nation.characters.len() as u8);
        let id = UnitId(self.next_unit);
        self.next_unit += 1;
        self.units.push(Unit {
            id,
            name: name.clone(),
            // `None` means the settlement's own nation (`World::nation_of`); only a unit of
            // another nation needs the field set.
            nation: (drawn != self.nation).then_some(drawn),
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
            guest: None,
            unhappy_since: None,
        });
        self.arrivals.push(Arrival { tick: self.tick, unit: id, name });
        true
    }

    /// Takes up to `amount` food items from stores for a newcomer who just joined (the
    /// difficulty preset's `newcomer_food` knob, #294): the nearest stack to the settlement
    /// first, then by position, the same rule `found` uses for starting stores. Takes whatever
    /// is there if stores hold less than `amount`; a no-op for 0 (identity's default).
    fn take_newcomer_food(&mut self, amount: u32, defs: &Defs) {
        if amount == 0 {
            return;
        }
        let anchor = self.settlement_anchor();
        let mut positions: Vec<Pos> = self
            .stored
            .iter()
            .filter(|(_, s)| defs.food_of(s.item).is_some())
            .map(|(&p, _)| p)
            .collect();
        positions.sort_by_key(|p| ((p.x - anchor.x).pow(2) + (p.y - anchor.y).pow(2), *p));
        let mut remaining = amount;
        for pos in positions {
            if remaining == 0 {
                break;
            }
            let Some(mut stack) = self.stored.remove(&pos) else { continue };
            let take = remaining.min(stack.count);
            stack.count -= take;
            remaining -= take;
            if stack.count > 0 {
                self.stored.insert(pos, stack);
            }
        }
    }
}

#[cfg(test)]
mod tests;
