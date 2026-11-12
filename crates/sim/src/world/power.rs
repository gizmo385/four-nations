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

/// A run of power-carrying track (`World::rail_runs`, #722).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RailRun {
    /// Its tiles, first found first.
    pub tiles: Vec<Pos>,
    /// Consumers within this many tiles of any of them are powered while it's live.
    pub reach: u32,
}

/// Which source powers a tile, and at what boost: for a client's cursor panel (#429).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PowerAt {
    /// The source's anchor tile.
    pub source: Pos,
    pub boost: f32,
}

impl World {
    /// The `PowerDef` of the building on `pos`, if it is a power source.
    /// A known `boost` upgrade (#722) replaces the def's boost.
    fn power_def(&self, pos: Pos, defs: &Defs) -> Option<PowerDef> {
        let def = defs.building(self.building_at(pos)?);
        let mut power = def.power?;
        if let Some((tech, boost, millis)) = def.boost_upgrade
            && self.research.is_known(tech)
        {
            power.boost = boost;
            power.boost_millis = millis;
        }
        Some(power)
    }

    /// Every finished power source that reaches any tile of `footprint`, with its def, best
    /// `boost` first. Two sources in reach of one consumer don't stack; this is what both
    /// `power_at` and `power_boost`/`power_boost_millis` pick the best of.
    fn sources_reaching(&self, footprint: Footprint, defs: &Defs) -> Vec<(Pos, PowerDef)> {
        let mut reaching: Vec<(Pos, PowerDef)> = self
            .of_kind(Kind::Power)
            .filter_map(|b| {
                let source = b.anchor;
                let def = self.power_def(source, defs)?;
                self.source_reaches(source, &def, &footprint).then_some((source, def))
            })
            .collect();
        // A live rail run beside the consumer powers it as its best source would (#722).
        for run in self.rail_runs(defs) {
            let near = run
                .tiles
                .iter()
                .any(|&t| footprint.tiles().any(|p| p.chebyshev(t) <= run.reach as i32));
            if near && let Some(best) = self.run_source(&run.tiles, defs) {
                reaching.push(best);
            }
        }
        reaching.sort_by(|a, b| b.1.boost.partial_cmp(&a.1.boost).unwrap());
        reaching
    }

    /// Whether the source anchored on `source` reaches any tile of `footprint`.
    fn source_reaches(&self, source: Pos, def: &PowerDef, footprint: &Footprint) -> bool {
        let reach = def.radius_for(self.modifiers.power_generation);
        self.footprint_at(source)
            .is_some_and(|f| footprint.tiles().any(|p| f.distance(p) <= reach as i32))
    }

    /// The best source reaching any tile of a run, if the run is live.
    fn run_source(&self, tiles: &[Pos], defs: &Defs) -> Option<(Pos, PowerDef)> {
        self.of_kind(Kind::Power)
            .filter_map(|b| {
                let def = self.power_def(b.anchor, defs)?;
                tiles
                    .iter()
                    .any(|&t| self.source_reaches(b.anchor, &def, &Footprint::single(t)))
                    .then_some((b.anchor, def))
            })
            .max_by(|a, b| a.1.boost.partial_cmp(&b.1.boost).unwrap().then(b.0.cmp(&a.0)))
    }

    /// Every run of a power-carrying track (#722): the 4-connected finished tiles of one track
    /// building with `power_reach`, in the order of their first tile's anchor.
    pub fn rail_runs(&self, defs: &Defs) -> Vec<RailRun> {
        if !self.has_kind(Kind::PowerRail) {
            return Vec::new();
        }
        let carries = |p: Pos| -> Option<(crate::building::BuildingId, u32)> {
            let id = self.building_at(p)?;
            Some((id, defs.building(id).track?.power_reach?))
        };
        let mut seen = std::collections::BTreeSet::new();
        let mut runs = Vec::new();
        let starts: Vec<Pos> = self.of_kind(Kind::PowerRail).map(|b| b.anchor).collect();
        for p in starts {
            if seen.contains(&p) {
                continue;
            }
            let Some((id, reach)) = carries(p) else { continue };
            let mut tiles = vec![p];
            seen.insert(p);
            let mut i = 0;
            while i < tiles.len() {
                let q = tiles[i];
                i += 1;
                for (dx, dy) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
                    let n = Pos::new(q.x + dx, q.y + dy);
                    if !seen.contains(&n) && carries(n).is_some_and(|(o, _)| o == id) {
                        seen.insert(n);
                        tiles.push(n);
                    }
                }
            }
            runs.push(RailRun { tiles, reach });
        }
        runs
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

    /// Whether the power `power_for` names reaches `footprint` only along a rail run (#722):
    /// for the cursor panel's "Powered x1.5 by rail from ..." (#730).
    pub fn powered_by_rail(&self, footprint: Footprint, defs: &Defs) -> bool {
        let Some(p) = self.power_for(footprint, defs) else { return false };
        self.power_def(p.source, defs)
            .is_some_and(|def| !self.source_reaches(p.source, &def, &footprint))
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
