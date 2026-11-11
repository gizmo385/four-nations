//! Hothouses (`[building.hothouse]`, docs/design/horizons-phase-1.md, "`hothouses`").
//!
//! - **Winter growth in reach.** In winter, a fuelled Hothouse gives every farm plot
//!   (`[building.farm]`: Farm Plots and Herb Gardens) within `radius` of its footprint its
//!   `winter_growth` in place of the climate band's, not scaled by `winter_severity`
//!   (`World::crop_growth_rate`). The best one in reach applies; two don't stack. A band whose
//!   own winter is kinder keeps it.
//! - **Frost spares it.** The first frost doesn't take a ripe crop in reach of a fuelled
//!   Hothouse (`World::apply_frost`).
//! - **Burns only while warming.** Its bunker burns only in winter, and only while some plot in
//!   reach is Growing or Ripe (`World::fuel_in_use`). With seasons off it does nothing.
//! - **Reach is fixed.** Not scaled by `heating`, so where it stands matters.

use super::buildings::{Building, Kind};
use super::farm::PlotStage;
use super::*;

impl World {
    /// Whether it is winter in a world with seasons on: the only time a Hothouse does anything.
    fn hothouse_season(&self, defs: &Defs) -> bool {
        self.knobs(defs).seasons && self.season(defs).winter
    }

    /// Whether the farm plot on `pos` is within `b`'s reach, `b` being a Hothouse.
    fn in_hothouse_reach(b: &Building, pos: Pos, defs: &Defs) -> bool {
        defs.building(b.id).hothouse.is_some_and(|h| b.footprint().distance(pos) <= h.radius as i32)
    }

    /// The best `winter_growth` of a fuelled Hothouse reaching `pos`, or `None` when none does.
    /// Seasons aside: the caller asks only in winter.
    fn hothouse_reaching(&self, pos: Pos, defs: &Defs) -> Option<f32> {
        if !self.has_kind(Kind::Hothouse) {
            return None;
        }
        self.of_kind(Kind::Hothouse)
            .filter(|b| b.fuel_lit() && Self::in_hothouse_reach(b, pos, defs))
            .filter_map(|b| defs.building(b.id).hothouse.map(|h| h.winter_growth))
            .reduce(f32::max)
    }

    /// A Hothouse's winter growth for the farm plot on `pos`, if one warms it now: winter, seasons
    /// on, and a fuelled Hothouse in reach. The cursor panel reads it too.
    pub fn hothouse_growth(&self, pos: Pos, defs: &Defs) -> Option<f32> {
        if !self.hothouse_season(defs) {
            return None;
        }
        self.hothouse_reaching(pos, defs)
    }

    /// Whether the first frost spares the ripe plot on `pos`: a fuelled Hothouse reaches it.
    pub(super) fn frost_spares(&self, pos: Pos, defs: &Defs) -> bool {
        self.hothouse_reaching(pos, defs).is_some()
    }

    /// The farm plots the Hothouse `b` is warming now: Growing or Ripe ones in reach, in winter.
    /// Empty outside winter or with seasons off. Its bunker burns while this isn't empty.
    pub(super) fn plots_warmed_by<'a>(
        &'a self,
        b: &'a Building,
        defs: &'a Defs,
    ) -> impl Iterator<Item = Pos> + 'a {
        let on = self.hothouse_season(defs) && b.fuel_lit();
        self.of_kind(Kind::Farm)
            .filter(move |_| on)
            .filter(|p| {
                p.plot
                    .as_ref()
                    .is_some_and(|p| matches!(p.stage, PlotStage::Growing | PlotStage::Ripe))
            })
            .map(|p| p.anchor)
            .filter(move |&pos| Self::in_hothouse_reach(b, pos, defs))
    }

    /// How many plots the Hothouse anchored on `pos` is warming now (`Warming 6 plots`), or
    /// `None` off a Hothouse.
    pub fn hothouse_warming(&self, pos: Pos, defs: &Defs) -> Option<usize> {
        let b = self.building_record(pos)?;
        defs.building(b.id).hothouse?;
        Some(self.plots_warmed_by(b, defs).count())
    }
}

#[cfg(test)]
mod tests;
