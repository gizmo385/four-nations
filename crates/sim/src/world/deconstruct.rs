//! Taking finished buildings down (issue #342, docs/design/buildings.md, "Cancel and
//! deconstruct"; docs/design/clearing.md, section 5).
//!
//! `Command::Deconstruct` on any tile of a finished building marks the whole building: one
//! `JobKind::Deconstruct` on its anchor, whichever tile was named, so a rectangle over a 2x2 hall
//! marks it once. A unit walks over and works it from beside the building (or on it, if it can be
//! walked on, like a bed), for `[deconstruct] work` times the building's own build `work`, at the
//! nation's `construction` rate. When it's done, `World::remove_building` takes the whole
//! footprint off the map in that tick, with everything the building owned (seats, craft jobs, a
//! farm plot's Sow and Reap jobs, its fire, the units inside, anyone bound for its bed), and
//! part of its cost drops by the anchor: `[deconstruct] refund` of each cost item, rounded down,
//! plus a production building's batch inputs already delivered, in full (they were never used).
//! `Command::CancelDeconstruct` takes the mark back, and the building stands untouched.
//!
//! The endeavor's gate can't be taken down. A stockpile is not a building: it goes through the
//! Cancel tool's explicit teardown mode (#335), since a stockpile tile is an order ("store here")
//! rather than a thing on the land.

use crate::jobs::{JobKind, Stack};

use super::*;

impl World {
    /// Anchors of finished buildings marked for taking down (`Command::Deconstruct`), claimed or
    /// not, oldest mark first.
    pub fn deconstruct_designations(&self) -> impl Iterator<Item = Pos> + '_ {
        self.jobs.iter().filter_map(|j| match j.kind {
            JobKind::Deconstruct { pos } => Some(pos),
            _ => None,
        })
    }

    /// Whether the finished building covering `pos` (any tile of its footprint) is marked for
    /// taking down.
    pub fn is_marked_for_deconstruct(&self, pos: Pos) -> bool {
        self.building_record(pos).is_some_and(|b| {
            let anchor = b.anchor;
            self.jobs.iter().any(|j| j.kind == JobKind::Deconstruct { pos: anchor })
        })
    }

    /// Marks the finished building covering `pos` for taking down. Refused off the map, on a
    /// tile with no finished building (a build site is `CancelBuild`'s), on the endeavor's gate,
    /// and on a building already marked.
    pub(super) fn designate_deconstruct(
        &mut self,
        pos: Pos,
        defs: &Defs,
    ) -> Result<JobId, DesignateError> {
        self.map.get(pos).ok_or(DesignateError::OutOfBounds)?;
        let b = self.building_record(pos).ok_or(DesignateError::NoBuilding)?;
        if defs.building(b.id).endeavor {
            return Err(DesignateError::CannotDeconstruct);
        }
        let anchor = b.anchor;
        if self.jobs.iter().any(|j| j.kind == JobKind::Deconstruct { pos: anchor }) {
            return Err(DesignateError::AlreadyDesignated);
        }
        Ok(self.add_job(JobKind::Deconstruct { pos: anchor }))
    }

    /// Takes back the mark on the finished building covering `pos`. Work already done on it is
    /// lost; the building itself was never touched.
    pub(super) fn cancel_deconstruct(&mut self, pos: Pos) -> Result<(), DesignateError> {
        self.map.get(pos).ok_or(DesignateError::OutOfBounds)?;
        let anchor = self.building_record(pos).ok_or(DesignateError::NotDesignated)?.anchor;
        let before = self.jobs.len();
        self.jobs.retain(|j| j.kind != JobKind::Deconstruct { pos: anchor });
        if self.jobs.len() == before {
            return Err(DesignateError::NotDesignated);
        }
        Ok(())
    }

    /// One tick of a unit taking a building down: walk to it, work `[deconstruct] work` times
    /// its build work at the `construction` rate, then remove it and drop the refund by the
    /// anchor.
    pub(super) fn run_deconstruct(&mut self, i: usize, id: JobId, defs: &Defs) {
        let Some(idx) = self.job_index(id) else { return self.finish_job(i) };
        let JobKind::Deconstruct { pos } = self.jobs[idx].kind else { return self.finish_job(i) };
        let Some(building) = self.anchored(pos).map(|b| b.id) else {
            // The building is gone some other way: the job is moot.
            self.jobs.remove(idx);
            return self.finish_job(i);
        };
        if !self.go_or_abandon(i, id, StandRule::Building(pos), defs) {
            return;
        }
        let def = defs.building(building);
        let rate = self.modifiers().construction;
        let job = &mut self.jobs[idx];
        job.progress += rate / TICKS_PER_SECOND as f32;
        if job.progress < def.work * defs.deconstruct.work {
            return;
        }
        self.jobs.remove(idx);
        let key = self.map[pos].building.expect("anchored above").key();
        let record = self.remove_building(key, defs).expect("anchored above");
        for cost in &def.cost {
            let count = defs.deconstruct.refund_of(cost.count);
            if count > 0 {
                self.place_ground(pos, Stack { item: cost.item, count });
            }
        }
        // A fuel bunker comes back in full (`fuel.rs`).
        if let (Some(bunker), Some(fuel)) = (record.fuel, def.fuel)
            && bunker.count > 0
        {
            self.place_ground(pos, Stack { item: fuel.item, count: bunker.count });
        }
        // So does a pantry's food (`outpost.rs`).
        for stack in record.pantry.iter().flat_map(|p| &p.stacks) {
            self.place_ground(pos, *stack);
        }
        if let (Some(craft), Some(recipe)) = (&record.craft, &def.recipe) {
            for (input, &count) in recipe.inputs.iter().zip(&craft.delivered) {
                if count > 0 {
                    self.place_ground(pos, Stack { item: input.item, count });
                }
            }
        }
        self.finish_job(i);
    }
}

#[cfg(test)]
mod tests;
