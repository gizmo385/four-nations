//! Farm plots: a building that is empty until sown, grows a crop, then is harvested (issue
//! #198). The design is in docs/design/research.md ("Farm plots"); the numbers are
//! `[building.farm]` in data/buildings.toml (`FarmDef`).
//!
//! * A finished farm plot starts **empty**. `Command::Sow` opens a `JobKind::Sow` job: a unit
//!   walks to it and works `sow_seconds`, after which the plot starts **growing**, with
//!   `grow_seconds` to ripen (`World::crop_growth_rate` scales how fast that counts down; see
//!   below). `CancelSow` withdraws an unclaimed or claimed Sow job while the plot is still
//!   empty.
//! * Once a growing plot's time runs out it is **ripe**, and a `JobKind::Reap` job is queued for
//!   it automatically (`tick_farms`), the same way a standing harvest order re-queues itself
//!   (#81): the player never has to notice the moment a plot ripens. A unit works `reap_seconds`
//!   there, which drops `harvest_count` of the crop on the ground (hauled off like any other
//!   yield), counts toward the end game's `crops_harvested` tally (#233), and returns the plot to
//!   empty, ready to sow again.
//! * `World::crop_growth_rate` composes three multipliers, in this order: the plot's own
//!   `crop_growth` (1.0 for the plain plot), `World::irrigation_rate` (the best `growth_boost`
//!   among irrigation channels in reach, else 1.0; #126, below), and the difficulty preset's
//!   season factor (`World::season_growth_factor`, `world/season.rs`, #295/#311): identity with
//!   `seasons` off, so this is exactly `crop_growth * irrigation_rate` for every save from before
//!   #311. Multiplying keeps the three independent: irrigation halves a plot's remaining time in
//!   any season, including a slow winter one, the same way it halves it in high summer.
//! * `World::irrigation_rate` is read the same way `World::tend_rate` speeds up a tree an
//!   orchard tends: the best `growth_boost` among irrigation channels within their own `radius`
//!   of the plot, or 1.0 with none in reach; two channels in reach of one plot don't stack.
//!   Terraces (#136) will add a second reader of the same hook.
//! * The first frost of winter (`World::tick_season`) also takes an unharvested **ripe** crop on
//!   a frost-band tile (#311): `frost_plot` sends it back to **empty**, the same way frost sends
//!   a ripe wild plant back to resting, and the frost event's count includes it.
//! * Farm plots are shared-tier (`farming`, every nation), unlike the orchard's Air-only tech, so
//!   there's no nation gate in this module at all: `designate_build` already refuses a farm plot
//!   to a nation that hasn't researched `farming`.

use super::*;

use crate::building::{FarmDef, IrrigationDef};
use crate::jobs::JobKind;

/// What a farm plot (`[building.farm]`) is doing right now. Read through `World::plot_stage`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum PlotStage {
    /// Nothing planted; a `Sow` job may be designated.
    Empty,
    /// Growing toward ripe; `World::plot_growth_left` says how much longer.
    Growing,
    /// Ready to gather; a `Reap` job is already queued (`tick_farms`).
    Ripe,
}

/// A farm plot's state: which stage it's in, and (while `Growing`) the seconds left to ripen.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub(super) struct FarmPlot {
    pub(super) stage: PlotStage,
    /// Seconds left to ripen while `Growing`; meaningless (and left at 0) otherwise.
    growing_left: f32,
}

impl FarmPlot {
    /// Nothing planted.
    pub(super) fn empty() -> Self {
        Self { stage: PlotStage::Empty, growing_left: 0.0 }
    }
}

impl World {
    /// What stage the farm plot on `pos` is in, if there is one.
    pub fn plot_stage(&self, pos: Pos) -> Option<PlotStage> {
        self.plot(pos).map(|p| p.stage)
    }

    /// Seconds left until the growing plot on `pos` ripens, if it is growing.
    pub fn plot_growth_left(&self, pos: Pos) -> Option<f32> {
        let p = self.plot(pos)?;
        (p.stage == PlotStage::Growing).then_some(p.growing_left)
    }

    /// Tiles with a `Sow` job designated on them (claimed or not): a plot about to be planted.
    pub fn sow_designations(&self) -> impl Iterator<Item = Pos> + '_ {
        self.jobs.iter().filter_map(|j| match j.kind {
            JobKind::Sow { pos } => Some(pos),
            _ => None,
        })
    }

    /// The farm plot anchored on `pos`, if there is one.
    fn plot(&self, pos: Pos) -> Option<&FarmPlot> {
        self.anchored(pos)?.plot.as_ref()
    }

    /// Puts the farm plot anchored on `pos` into `plot`'s state. A no-op with no building there.
    fn set_plot(&mut self, pos: Pos, plot: FarmPlot) {
        if let Some(b) = self.anchored_mut(pos) {
            b.plot = Some(plot);
        }
    }

    /// The `FarmDef` of the building on `pos`, if it is a farm plot.
    fn farm_def(&self, pos: Pos, defs: &Defs) -> Option<FarmDef> {
        defs.building(self.building_at(pos)?).farm
    }

    /// The `IrrigationDef` of the building on `pos`, if it is an irrigation channel.
    fn irrigation_def(&self, pos: Pos, defs: &Defs) -> Option<IrrigationDef> {
        defs.building(self.building_at(pos)?).irrigation
    }

    /// How many times faster than normal a farm plot on `pos` grows because an irrigation
    /// channel tends it: the best `growth_boost` among channels within their own `radius` of
    /// `pos`, else 1.0, the same way `tend_rate` layers onto an orchard's resting trees. Two
    /// channels in reach of one plot don't stack.
    pub(super) fn irrigation_rate(&self, pos: Pos, defs: &Defs) -> f32 {
        self.of_kind(Kind::Irrigation)
            .filter_map(|b| self.irrigation_def(b.anchor, defs).map(|d| (b.anchor, d)))
            .filter(|(c, d)| c.chebyshev(pos) <= d.radius as i32)
            .map(|(_, d)| d.growth_boost)
            .fold(1.0, f32::max)
    }

    /// How many times faster than its plain `grow_seconds` the crop on `pos` grows: the plot's
    /// own `crop_growth` (`FarmDef`), times `World::irrigation_rate` (the irrigation channel's
    /// boost, #126), times the season factor `World::growth_rate` already uses for wild harvests
    /// (`World::season_growth_factor`, #311; identity with `seasons` off). A farm plot is tended,
    /// so it is never `frost_hardy`-exempt the way fishing is. Terraces (#136) will scale
    /// further, the same way `tend_rate` layers onto an orchard's trees; nothing else in
    /// `tick_farms` would need to change.
    fn crop_growth_rate(&self, pos: Pos, defs: &Defs) -> f32 {
        let base = self.farm_def(pos, defs).map(|d| d.crop_growth).unwrap_or(1.0);
        base * self.irrigation_rate(pos, defs) * self.season_growth_factor(pos, defs, false)
    }

    /// The first frost of winter also takes an unharvested ripe crop (#311, called from
    /// `World::tick_season`'s `apply_frost`): the plot goes back to **empty**, as if nothing had
    /// grown, the same way frost sends a ripe wild plant back to resting.
    pub(super) fn frost_plot(&mut self, pos: Pos) {
        self.set_plot(pos, FarmPlot::empty());
    }

    /// Checks that `pos` has an empty farm plot, for `designate_sow`.
    fn sowable(&self, pos: Pos, defs: &Defs) -> Result<(), DesignateError> {
        self.map.get(pos).ok_or(DesignateError::OutOfBounds)?;
        match self.plot_stage(pos) {
            Some(PlotStage::Empty) if self.farm_def(pos, defs).is_some() => Ok(()),
            Some(PlotStage::Empty) => Err(DesignateError::NoFarmPlot),
            Some(_) => Err(DesignateError::PlotNotEmpty),
            None => Err(DesignateError::NoFarmPlot),
        }
    }

    /// Designates the empty farm plot on `pos` to be sown.
    pub(super) fn designate_sow(&mut self, pos: Pos, defs: &Defs) -> Result<JobId, DesignateError> {
        self.sowable(pos, defs)?;
        if self.jobs.iter().any(|j| j.kind == JobKind::Sow { pos }) {
            return Err(DesignateError::AlreadyDesignated);
        }
        Ok(self.add_job(JobKind::Sow { pos }))
    }

    /// Withdraws a Sow designation on `pos`. Work already done on it is lost.
    pub(super) fn cancel_sow(&mut self, pos: Pos) -> Result<(), DesignateError> {
        let before = self.jobs.len();
        self.jobs.retain(|j| j.kind != JobKind::Sow { pos });
        if self.jobs.len() == before {
            return Err(DesignateError::NotDesignated);
        }
        Ok(())
    }

    /// One tick of a unit planting an empty plot: walk to it, work `sow_seconds`, then start it
    /// growing.
    pub(super) fn run_sow(&mut self, i: usize, id: JobId, defs: &Defs) {
        let Some(idx) = self.job_index(id) else { return self.finish_job(i) };
        let JobKind::Sow { pos } = self.jobs[idx].kind else { return self.finish_job(i) };
        let Some(def) = self.farm_def(pos, defs) else {
            // The plot is gone (no demolish yet, so only a hand-edited map): moot.
            self.jobs.remove(idx);
            return self.finish_job(i);
        };
        if self.plot_stage(pos) != Some(PlotStage::Empty) {
            // Someone else sowed it, or it moved on some other way: moot.
            self.jobs.remove(idx);
            return self.finish_job(i);
        }
        if !self.go_or_abandon(i, id, StandRule::Building(pos), defs) {
            return;
        }
        let job = &mut self.jobs[idx];
        job.progress += 1.0 / TICKS_PER_SECOND as f32;
        if job.progress < def.sow_seconds {
            return;
        }
        self.jobs.remove(idx);
        self.set_plot(pos, FarmPlot { stage: PlotStage::Growing, growing_left: def.grow_seconds });
        self.finish_job(i);
    }

    /// One tick of a unit gathering a ripe plot: walk to it, work `reap_seconds`, then drop its
    /// crop and empty it again.
    pub(super) fn run_reap(&mut self, i: usize, id: JobId, defs: &Defs) {
        let Some(idx) = self.job_index(id) else { return self.finish_job(i) };
        let JobKind::Reap { pos } = self.jobs[idx].kind else { return self.finish_job(i) };
        let Some(def) = self.farm_def(pos, defs) else {
            self.jobs.remove(idx);
            return self.finish_job(i);
        };
        if self.plot_stage(pos) != Some(PlotStage::Ripe) {
            self.jobs.remove(idx);
            return self.finish_job(i);
        }
        if !self.go_or_abandon(i, id, StandRule::Building(pos), defs) {
            return;
        }
        let job = &mut self.jobs[idx];
        job.progress += 1.0 / TICKS_PER_SECOND as f32;
        if job.progress < def.reap_seconds {
            return;
        }
        self.jobs.remove(idx);
        self.tallies.count_crops(def.harvest_count);
        self.place_ground(pos, Stack { item: def.crop, count: def.harvest_count });
        self.set_plot(pos, FarmPlot::empty());
        self.finish_job(i);
    }

    /// Advances every growing plot, ripening those that are done and queueing their Reap job.
    pub(super) fn tick_farms(&mut self, defs: &Defs) {
        if !self.has_kind(Kind::Farm) {
            return;
        }
        let step = 1.0 / TICKS_PER_SECOND as f32;
        let growing: Vec<Pos> = self
            .of_kind(Kind::Farm)
            .filter(|b| b.plot.as_ref().is_some_and(|p| p.stage == PlotStage::Growing))
            .map(|b| b.anchor)
            .collect();
        let mut ripened = Vec::new();
        for pos in growing {
            let rate = self.crop_growth_rate(pos, defs);
            let plot = self.anchored_mut(pos).and_then(|b| b.plot.as_mut()).expect("just listed");
            plot.growing_left -= rate * step;
            if plot.growing_left <= 0.0 {
                plot.stage = PlotStage::Ripe;
                plot.growing_left = 0.0;
                ripened.push(pos);
            }
        }
        for pos in ripened {
            if !self.jobs.iter().any(|j| j.kind == JobKind::Reap { pos }) {
                self.add_job(JobKind::Reap { pos });
            }
        }
    }
}

#[cfg(test)]
mod tests;
