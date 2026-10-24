//! Heat sources: a place that cooks meals eaten near it and makes sleep near it better.
//!
//! There are two kinds, and everything that *uses* heat asks only the questions below and never
//! what lit it:
//!
//! - Fire's Hearthflame (#70) burns without fuel until its end tick.
//! - A built campfire (`BuildingDef::heat`, #58) is lit for good the moment it is finished
//!   (`until` is `HeatSource::FOREVER`). Its radius follows the nation's `heating` modifier, so
//!   `refresh_heat_radii` re-derives it whenever research changes that.
//!
//!
//! - `is_heated(pos)`: is `pos` within the radius of a lit source? A meal eaten there is cooked
//!   (`work.rs` calls this), so `FoodDef::cooked_hunger` applies.
//! - `sleep_factor_at(pos)`: the sleep-rate multiplier there, the best of the sources in range
//!   (they don't stack). The needs pass in `world.rs` applies it.
//! - `warm_sleep_route`: where a unit about to sleep should walk first, if a source is near.
//!
//! A source is lit while `tick < until`. Radius is Chebyshev (a square), like the rest of the
//! bending ranges. Nothing here blocks movement.

use super::*;
use crate::path::STRAIGHT;

/// A lit place of heat.
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct HeatSource {
    pub pos: Pos,
    /// Tiles around `pos`, Chebyshev, that count as near it.
    pub radius: u32,
    /// Energy regained asleep nearby, times the base rate (at least 1.0).
    pub sleep_bonus: f32,
    /// A unit about to sleep walks to this source if it is within this many path tiles.
    pub seek_tiles: u32,
    /// The first tick it is out. It is lit while `tick < until`.
    pub until: u64,
}

impl HeatSource {
    /// The `until` of a source that never goes out: a building's fire.
    pub const FOREVER: u64 = u64::MAX;

    /// Never goes out: it belongs to a building, rather than being a timed flame.
    pub fn is_permanent(&self) -> bool {
        self.until == Self::FOREVER
    }

    fn lit_at(&self, tick: u64) -> bool {
        tick < self.until
    }

    fn warms(&self, p: Pos) -> bool {
        p.x.abs_diff(self.pos.x).max(p.y.abs_diff(self.pos.y)) <= self.radius
    }
}

/// A lit flame as the client sees it.
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Flame {
    pub pos: Pos,
    pub radius: u32,
    /// In-game seconds until it goes out.
    pub seconds_left: f32,
}

impl World {
    /// Every lit timed flame (Hearthflame) with the time it has left, in position order. A
    /// building's fire isn't listed: it is a building, with the tile it stands on.
    pub fn flames(&self) -> Vec<Flame> {
        self.lit_sources()
            .filter(|s| !s.is_permanent())
            .map(|s| Flame {
                pos: s.pos,
                radius: s.radius,
                seconds_left: (s.until - self.tick) as f32 / TICKS_PER_SECOND as f32,
            })
            .collect()
    }

    /// Is a timed flame (Hearthflame) lit on exactly this tile?
    pub fn is_lit(&self, pos: Pos) -> bool {
        self.lit_sources().any(|s| s.pos == pos && !s.is_permanent())
    }

    /// Is `pos` within the radius of a lit heat source: do meals eaten there count as cooked?
    pub fn is_heated(&self, pos: Pos) -> bool {
        self.lit_sources().any(|s| s.warms(pos))
    }

    fn lit_sources(&self) -> impl Iterator<Item = &HeatSource> + '_ {
        self.heat.values().filter(|s| s.lit_at(self.tick))
    }

    /// How much faster than the base rate energy is regained asleep at `pos`: the best bonus
    /// among the sources that reach it (they don't stack), or 1.0.
    pub(super) fn sleep_factor_at(&self, pos: Pos) -> f32 {
        self.lit_sources().filter(|s| s.warms(pos)).map(|s| s.sleep_bonus).fold(1.0, f32::max)
    }

    /// Lights `source`, replacing whatever was lit on its tile (a restoke resets the timer).
    pub(super) fn light_heat_source(&mut self, source: HeatSource) {
        self.heat.insert(source.pos, source);
    }

    /// A finished building keeps a fire if its def says so (`BuildingDef::heat`).
    pub(super) fn light_building_fire(&mut self, pos: Pos, def: &BuildingDef) {
        if let Some(heat) = def.heat {
            self.light_heat_source(HeatSource {
                pos,
                radius: heat.radius_for(self.modifiers.heating),
                sleep_bonus: heat.sleep_bonus,
                seek_tiles: heat.seek_tiles,
                until: HeatSource::FOREVER,
            });
        }
    }

    /// Re-derives the radius of every building's fire from the current `heating` modifier, for
    /// when research changes it.
    pub(super) fn refresh_heat_radii(&mut self, defs: &Defs) {
        let heating = self.modifiers.heating;
        for (pos, source) in self.heat.iter_mut().filter(|(_, s)| s.is_permanent()) {
            let heat =
                self.map.get(*pos).and_then(|t| t.building).and_then(|b| defs.building(b.id).heat);
            if let Some(heat) = heat {
                source.radius = heat.radius_for(heating);
            }
        }
    }

    /// Drops the sources whose time is up.
    pub(super) fn tick_heat(&mut self) {
        if !self.heat.is_empty() {
            let now = self.tick;
            self.heat.retain(|_, s| s.lit_at(now));
        }
    }

    /// For unit `i` about to sleep: the route to the nearest tile warmed by a lit source within
    /// that source's `seek_tiles` path tiles (empty if it is already warm), or `None` if there
    /// is no such source and it should sleep where it is.
    pub(super) fn warm_sleep_route(&self, i: usize) -> Option<Vec<Pos>> {
        if self.heat.is_empty() {
            return None;
        }
        let flood = self.flood_from(i);
        let mut best: Option<(u32, Pos)> = None;
        for s in self.lit_sources() {
            let limit = s.seek_tiles * STRAIGHT;
            let r = s.radius as i32;
            for dy in -r..=r {
                for dx in -r..=r {
                    let p = Pos::new(s.pos.x + dx, s.pos.y + dy);
                    let Some(cost) = flood.cost(p).filter(|&c| c <= limit) else { continue };
                    if best.is_none_or(|b| (cost, p) < b) {
                        best = Some((cost, p));
                    }
                }
            }
        }
        let (_, stand) = best?;
        Some(work::route_from(flood.path_to(stand)?))
    }
}
