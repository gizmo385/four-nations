//! The Tea House (`[building.tea_house]`, docs/design/horizons-phase-3.md, "`tea_ceremony`").
//!
//! A Tea House is a herb bunker (`fuel.rs`) with three effects. It is **open** while its bunker
//! holds at least one herb, which it burns at `per_minute` whenever it is finished and stocked,
//! not only during a visit, because draw runs all the time. Two houses don't stack: each effect
//! asks whether any house is open and takes the best open house's value.
//!
//! - **Caravans** (`trade.rs`): a caravan that makes camp while a house is open stays
//!   `extra_stay_minutes` longer, fixed at arrival, and shows `extra_deals` more deals, drawn
//!   from `rng::stream(seed, "trade.tea")` among the deals not already shown. The first
//!   `deals_per_visit` deals are drawn exactly as without a house. A house that runs dry
//!   mid-visit changes nothing: the visit keeps what it got.
//! - **Draw** (`draw.rs`): `draw` joins the draw multiplier while any house is open, under the
//!   same `[growth.draw].max` cap (#706, option A: the existing 1.4 stays).

use super::buildings::Kind;
use super::*;
use crate::building::TeaHouseDef;

impl World {
    /// The effects of every Tea House that is open now: finished, with at least one herb in its
    /// bunker.
    fn open_tea_houses<'a>(&'a self, defs: &'a Defs) -> impl Iterator<Item = TeaHouseDef> + 'a {
        self.of_kind(Kind::Tea)
            .filter(|b| self.fuel_lit(b, defs))
            .filter_map(|b| defs.building(b.id).tea_house)
    }

    /// Whether any Tea House is open.
    pub fn tea_open(&self, defs: &Defs) -> bool {
        self.open_tea_houses(defs).next().is_some()
    }

    /// Whether the Tea House anchored on `pos` is open (`Open` or `Closed: out of herbs`), or
    /// `None` off a Tea House.
    pub fn tea_house_open(&self, pos: Pos, defs: &Defs) -> Option<bool> {
        let b = self.building_record(pos)?;
        defs.building(b.id).tea_house?;
        Some(self.fuel_lit(b, defs))
    }

    /// Deals a caravan camping now shows beyond `deals_per_visit`: the best open house's
    /// `extra_deals`, or 0 with none open.
    pub(super) fn tea_extra_deals(&self, defs: &Defs) -> usize {
        self.open_tea_houses(defs).map(|t| t.extra_deals).max().unwrap_or(0) as usize
    }

    /// Ticks a caravan camping now stays beyond `stay_minutes`: the best open house's
    /// `extra_stay_minutes`, or 0 with none open.
    pub(super) fn tea_extra_stay_ticks(&self, defs: &Defs) -> u64 {
        let minutes = self.open_tea_houses(defs).map(|t| t.extra_stay_minutes).fold(0.0, f32::max);
        (minutes * 60.0 * TICKS_PER_SECOND as f32).round() as u64
    }

    /// What the draw multiplier gains from a Tea House now, in milli-units: the best open
    /// house's `draw`, or 0 with none open. The cap is applied by `draw_multiplier`.
    pub(super) fn tea_draw_milli(&self, defs: &Defs) -> u32 {
        let draw = self.open_tea_houses(defs).map(|t| t.draw).fold(0.0, f32::max);
        (draw * super::draw::DRAW_ONE as f32).round().max(0.0) as u32
    }
}

#[cfg(test)]
mod tests;
