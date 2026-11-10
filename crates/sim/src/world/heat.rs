//! Heat sources: a place that keeps the cold away, and -- unless it is a shelter (#362) -- cooks
//! meals eaten near it and makes sleep near it better.
//!
//! There are two kinds, and everything that *uses* heat asks only the questions below and never
//! what lit it:
//!
//! - Fire's Hearthflame (#70) burns without fuel until its end tick.
//! - A built campfire (`BuildingDef::heat`, #58) is lit for good the moment it is finished
//!   (`until` is `HeatSource::FOREVER`). Its radius follows the nation's `heating` modifier, so
//!   `refresh_heat_radii` re-derives it whenever research changes that.
//!
//! Earth's Earth Tent and Air's Air Shield (#362) are timed sources exactly like Hearthflame,
//! with one difference: `cooks = false`. A shelter still keeps the cold away and gives its own
//! `sleep_bonus`, but nothing kindles there, so a meal eaten within reach stays raw.
//!
//! - `is_heated(pos)`: is `pos` within the radius of a lit source, of any kind? This is what
//!   keeps the cold away (`World::is_cold`); a shelter counts exactly like a flame.
//! - `is_cooking(pos)`: is `pos` within the radius of a lit source that actually cooks (`cooks`
//!   is true)? A meal eaten there is cooked (`work.rs` calls this), so `FoodDef::cooked_hunger`
//!   applies. A shelter never counts, whatever else is in reach.
//! - `sleep_factor_at(pos)`: the sleep-rate multiplier there, the best of the sources in range
//!   (they don't stack). The needs pass in `world.rs` applies it.
//! - `warm_sleep_route`: where a unit about to sleep should walk first, if a source is near.
//!
//! - Fire's Inner Fire (#123) is a third kind, an `Aura`: a source that rides on a unit, and
//!   always cooks (#362 only split `heat_source`, not `heat_aura`). Every question above treats
//!   it as a source at the carrier's tile this tick, so cooked meals and warm sleep need no
//!   special case. It differs in one way: nobody walks to it (`warm_sleep_route`), because it
//!   moves. It ends with its time, or with its carrier.
//!
//! A source is lit while `tick < until`. Radius is Chebyshev (a square), like the rest of the
//! bending ranges, measured from every tile the source covers: a building's whole footprint
//! (#208). Nothing here blocks movement.

use super::*;
use crate::bending::AbilityId;
use crate::building::{ONE_BY_ONE, is_one_by_one, one_by_one};
use crate::geom::Footprint;
use crate::path::STRAIGHT;

/// The default `cooks` for a save from before #362, which predates Earth Tent and Air Shield and
/// so can only have meant a source that cooks.
fn always_cooks() -> bool {
    true
}

/// A lit place of heat.
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct HeatSource {
    /// Where it is: the tile of a flame, or the anchor of a building's fire.
    pub pos: Pos,
    /// Tiles around it, Chebyshev from the nearest tile it covers, that count as near it.
    pub radius: u32,
    /// Energy regained asleep nearby, times the base rate (at least 1.0).
    pub sleep_bonus: f32,
    /// A unit about to sleep walks to this source if it is within this many path tiles.
    pub seek_tiles: u32,
    /// The first tick it is out. It is lit while `tick < until`.
    pub until: u64,
    /// The tiles it covers, `[w, h]` from `pos`: a building's footprint (#208). A flame or an
    /// aura is 1x1, and 1x1 isn't written to a save.
    #[serde(default = "one_by_one", skip_serializing_if = "is_one_by_one")]
    pub size: [u8; 2],
    /// Whether a meal eaten within reach counts as cooked (#362): true for Fire's Hearthflame
    /// and every building fire, false for Earth's Earth Tent and Air's Air Shield, which keep
    /// the cold away and give their own `sleep_bonus` but cook nothing. A save from before the
    /// split can only have meant true.
    #[serde(default = "always_cooks")]
    pub cooks: bool,
    /// Which ability lit it, if any (#362): `None` for a building's fire, or a save from before
    /// the split. Presentation only (the TUI's glyph and cursor line for a shelter apart from a
    /// flame); nothing in `fn_sim` reads it.
    #[serde(default)]
    pub ability: Option<AbilityId>,
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

    fn footprint(&self) -> Footprint {
        Footprint::new(self.pos, self.size[0], self.size[1])
    }

    fn warms(&self, p: Pos) -> bool {
        self.footprint().distance(p) as u32 <= self.radius
    }
}

/// A warmth aura carried by a unit (Inner Fire, #123). It is a `HeatSource` whose position is
/// wherever the carrier stands.
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Aura {
    pub unit: UnitId,
    /// Tiles around the carrier, Chebyshev, that count as near it.
    pub radius: u32,
    /// Energy regained asleep nearby, times the base rate (at least 1.0).
    pub sleep_bonus: f32,
    /// The first tick it is out. It is lit while `tick < until`.
    pub until: u64,
}

/// An aura as the client sees it: where it is now, and for how long.
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Glow {
    pub unit: UnitId,
    /// The carrier's tile.
    pub pos: Pos,
    pub radius: u32,
    /// In-game seconds until it fades.
    pub seconds_left: f32,
}

/// A lit flame or shelter as the client sees it.
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Flame {
    pub pos: Pos,
    pub radius: u32,
    /// In-game seconds until it goes out.
    pub seconds_left: f32,
    /// Which ability lit it (#362): `None` for a save from before the split, which predates
    /// Earth Tent and Air Shield and so can only have meant Hearthflame.
    pub ability: Option<AbilityId>,
}

impl World {
    /// Every lit timed flame or shelter (Hearthflame, Earth Tent, Air Shield) with the time it
    /// has left, in position order. A building's fire isn't listed: it is a building, with the
    /// tile it stands on.
    pub fn flames(&self) -> Vec<Flame> {
        self.fixed_sources()
            .filter(|s| !s.is_permanent())
            .map(|s| Flame {
                pos: s.pos,
                radius: s.radius,
                seconds_left: (s.until - self.tick) as f32 / TICKS_PER_SECOND as f32,
                ability: s.ability,
            })
            .collect()
    }

    /// Is a timed flame (Hearthflame) lit on exactly this tile?
    pub fn is_lit(&self, pos: Pos) -> bool {
        self.fixed_sources().any(|s| s.pos == pos && !s.is_permanent())
    }

    /// Every lit aura (Inner Fire) with where it is and the time it has left, in unit order.
    pub fn glows(&self) -> Vec<Glow> {
        self.lit_auras()
            .map(|(a, pos)| Glow {
                unit: a.unit,
                pos,
                radius: a.radius,
                seconds_left: (a.until - self.tick) as f32 / TICKS_PER_SECOND as f32,
            })
            .collect()
    }

    /// Is `pos` within the radius of a lit source, flame or shelter: does it keep the cold away
    /// there (`World::is_cold`)? A shelter (`cooks = false`) counts exactly like a flame.
    pub fn is_heated(&self, pos: Pos) -> bool {
        self.lit_sources().any(|s| s.warms(pos))
    }

    /// Is `pos` within the radius of a lit source that actually cooks (#362)? Unlike
    /// `is_heated`, a shelter with `cooks = false` (Earth's Earth Tent, Air's Air Shield) never
    /// counts, whatever else is in reach: a meal eaten there stays raw.
    pub fn is_cooking(&self, pos: Pos) -> bool {
        self.lit_sources().any(|s| s.cooks && s.warms(pos))
    }

    /// Every lit source this tick: timed flames, building fires and auras (as a source at the
    /// carrier's tile). An aura always cooks (#362 only split `heat_source`, not `heat_aura`).
    fn lit_sources(&self) -> impl Iterator<Item = HeatSource> + '_ {
        let fixed = self.fixed_sources().copied();
        let carried = self.lit_auras().map(|(a, pos)| HeatSource {
            pos,
            radius: a.radius,
            sleep_bonus: a.sleep_bonus,
            seek_tiles: 0,
            until: a.until,
            size: ONE_BY_ONE,
            cooks: true,
            ability: None,
        });
        fixed.chain(carried)
    }

    /// The lit sources that stay put: timed flames and shelters, then building fires. Every
    /// reader asks "any" or "best of", so the order doesn't matter.
    fn fixed_sources(&self) -> impl Iterator<Item = &HeatSource> + '_ {
        self.heat.values().chain(self.buildings.fires()).filter(|s| s.lit_at(self.tick))
    }

    /// Every lit aura with its carrier's tile, in unit order. One whose carrier is gone is
    /// skipped (and dropped at the end of the tick).
    fn lit_auras(&self) -> impl Iterator<Item = (&Aura, Pos)> + '_ {
        self.auras.values().filter(|a| a.until > self.tick).filter_map(|a| {
            let i = self.units.binary_search_by_key(&a.unit, |u| u.id).ok()?;
            Some((a, self.units[i].pos))
        })
    }

    /// Gives `aura.unit` the aura, replacing the one it had (a recast resets the timer).
    pub(super) fn light_aura(&mut self, aura: Aura) {
        self.auras.insert(aura.unit, aura);
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

    /// Re-derives the radius of every building's fire from the current `heating` modifier, for
    /// when research changes it. A finished building's fire is lit by `put_building`
    /// (`world/buildings.rs`) and kept on its record.
    pub(super) fn refresh_heat_radii(&mut self, defs: &Defs) {
        let heating = self.modifiers.heating;
        for (id, source) in self.buildings.fires_mut() {
            if let Some(heat) = defs.building(id).heat {
                source.radius = heat.radius_for(heating);
            }
        }
    }

    /// The source anchored on `pos`: a finished building's fire, else a timed flame there.
    #[cfg(test)]
    pub(super) fn heat_at(&self, pos: Pos) -> Option<HeatSource> {
        self.anchored(pos).and_then(|b| b.heat).or_else(|| self.heat.get(&pos).copied())
    }

    /// Drops the sources whose time is up, and the auras whose time is up or whose carrier died.
    pub(super) fn tick_heat(&mut self) {
        if !self.heat.is_empty() {
            let now = self.tick;
            self.heat.retain(|_, s| s.lit_at(now));
        }
        if !self.auras.is_empty() {
            let (now, units) = (self.tick, &self.units);
            self.auras
                .retain(|id, a| a.until > now && units.binary_search_by_key(id, |u| u.id).is_ok());
        }
    }

    /// For unit `i` about to sleep: the route to the nearest tile warmed by a lit source within
    /// that source's `seek_tiles` path tiles (empty if it is already warm), or `None` if there
    /// is no such source and it should sleep where it is.
    pub(super) fn warm_sleep_route(&self, i: usize) -> Option<Vec<Pos>> {
        // Only sources that stay put are worth walking to; an aura moves.
        if self.heat.is_empty() && self.buildings.fires().next().is_none() {
            return None;
        }
        let flood = self.flood_from(i);
        let mut best: Option<(u32, Pos)> = None;
        for s in self.fixed_sources() {
            let limit = s.seek_tiles * STRAIGHT;
            // The footprint grown by the radius on every side.
            let r = s.radius as i32;
            let (w, h) = (i32::from(s.size[0]), i32::from(s.size[1]));
            for dy in -r..h + r {
                for dx in -r..w + r {
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
