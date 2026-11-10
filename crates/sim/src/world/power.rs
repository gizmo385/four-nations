//! Power reach: a finished building with `[building.power]` (`BuildingDef::power`, issue #428,
//! docs/design/late-tree.md) speeds up work in any `uses_power` building within its reach.
//! There's no grid and no load: a consumer just asks "is anything in reach, and at what boost",
//! the same question heat and irrigation already answer for their own readers
//! (`world/heat.rs::is_heated`, `world/farm.rs::irrigation_rate`).
//!
//! `PowerDef::radius` is for a `power_generation` modifier of 1.0; the reach at the settlement's
//! current modifier is `PowerDef::radius_for`, re-derived from the def every time (never
//! cached), the same way `irrigation_rate` re-reads `IrrigationDef` rather than storing a copy.
//! Reach is Chebyshev, measured from every tile of the source's own footprint (`Footprint::
//! distance`), exactly `world/heat.rs::warms`'s shape. A consumer is powered when any tile of
//! *its* footprint falls within that reach; two sources in reach of one consumer don't stack,
//! so `sources_reaching` picks the best `boost` among them.
//!
//! `World::power_at` is the per-tile query a client reads (#429); `World::power_boost` and
//! `World::power_boost_millis` are what `world/forge.rs`'s recipe batches and `world/study.rs`'s
//! seats scale their rate by. `boost_millis` (`boost * 1000`, rounded at load) keeps the study
//! seat's integer millis-per-tick maths free of floating point, so the sim stays deterministic
//! across platforms.

use super::*;
use crate::building::PowerDef;
use crate::geom::Footprint;

/// Which source powers a tile, and at what boost: for a client's cursor panel (#429).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PowerAt {
    /// The source's anchor tile.
    pub source: Pos,
    pub boost: f32,
}

impl World {
    /// The `PowerDef` of the building on `pos`, if it is a power source.
    fn power_def(&self, pos: Pos, defs: &Defs) -> Option<PowerDef> {
        defs.building(self.map.get(pos)?.building?.id).power
    }

    /// A power source building has just finished.
    pub(super) fn open_power_source(&mut self, pos: Pos) {
        self.power_sources.insert(pos);
    }

    /// Registers power sources that stand on the map but not in `power_sources` (a save from
    /// before this issue, or a map a test or tool built by hand), the way
    /// `adopt_irrigation_channels` does for irrigation channels. Sources already known are left
    /// as they are.
    ///
    /// Only a building's anchor tile (`part == [0, 0]`) counts: a 2x2 power building is one
    /// source. A save written by a build that registered every tile has an entry on each
    /// non-anchor tile too; those are dropped here (a source holds no state beyond its position,
    /// so nothing else is lost) and the anchor's entry stays.
    pub fn adopt_power_sources(&mut self, defs: &Defs) {
        let map = &self.map;
        self.power_sources
            .retain(|&p| map.get(p).and_then(|t| t.building).is_none_or(|b| b.part == [0, 0]));
        let found: Vec<Pos> = self
            .map
            .iter()
            .filter(|(p, t)| {
                t.building.is_some_and(|b| b.part == [0, 0] && defs.building(b.id).power.is_some())
                    && !self.power_sources.contains(p)
            })
            .map(|(p, _)| p)
            .collect();
        for p in found {
            self.open_power_source(p);
        }
    }

    /// Every finished power source that reaches any tile of `footprint`, with its def, best
    /// `boost` first. Two sources in reach of one consumer don't stack; this is what both
    /// `power_at` and `power_boost`/`power_boost_millis` pick the best of.
    fn sources_reaching(&self, footprint: Footprint, defs: &Defs) -> Vec<(Pos, PowerDef)> {
        let power_generation = self.modifiers.power_generation;
        let mut reaching: Vec<(Pos, PowerDef)> = self
            .power_sources
            .iter()
            .filter_map(|&source| {
                let def = self.power_def(source, defs)?;
                let reach = def.radius_for(power_generation);
                let source_footprint = self.footprint_at(source)?;
                footprint
                    .tiles()
                    .any(|p| source_footprint.distance(p) <= reach as i32)
                    .then_some((source, def))
            })
            .collect();
        reaching.sort_by(|a, b| b.1.boost.partial_cmp(&a.1.boost).unwrap());
        reaching
    }

    /// Which source powers a consumer whose footprint is `footprint`, and at what boost: the best
    /// among sources whose reach covers any of its tiles, exactly what `power_boost` scales work
    /// by. For a client's readouts (#429, #452): ask with the building's `footprint_at`, so a
    /// multi-tile consumer reached only through a far tile reads as powered. `None` if nothing
    /// reaches it.
    pub fn power_for(&self, footprint: Footprint, defs: &Defs) -> Option<PowerAt> {
        let (source, def) = self.sources_reaching(footprint, defs).into_iter().next()?;
        Some(PowerAt { source, boost: def.boost })
    }

    /// Which source powers the single tile `pos` (`power_for` over `Footprint::single`): for the
    /// reach tint and the cursor panel on an empty tile (#429).
    pub fn power_at(&self, pos: Pos, defs: &Defs) -> Option<PowerAt> {
        self.power_for(Footprint::single(pos), defs)
    }

    /// The best boost a consumer whose footprint is `footprint` runs at: 1.0 with no source in
    /// reach of any of its tiles. Read by `world/forge.rs`'s recipe batches.
    pub(super) fn power_boost(&self, footprint: Footprint, defs: &Defs) -> f32 {
        self.sources_reaching(footprint, defs).first().map_or(1.0, |(_, d)| d.boost)
    }

    /// `power_boost` in `boost * 1000` integer form, for `world/study.rs`'s seats to scale their
    /// integer millis-per-tick rate without floating point: 1000 (1.0x) with no source in reach.
    pub(super) fn power_boost_millis(&self, footprint: Footprint, defs: &Defs) -> u32 {
        self.sources_reaching(footprint, defs).first().map_or(1000, |(_, d)| d.boost_millis)
    }
}

#[cfg(test)]
mod tests;
