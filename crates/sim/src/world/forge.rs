//! Fire production: the `Craft` job that turns a production building's recipe into its output
//! (the forge's smith job, a brick kiln, a smelter; `data/buildings.toml`, `BuildingDef::recipe`,
//! issue #124).
//!
//! A production building's `capacity` becomes that many `JobKind::Craft` jobs the moment it
//! finishes construction (`world/build.rs`), mirroring a study hall's seats (`world/study.rs`).
//! Unlike a study seat, a craft job cycles forever in batches: a unit holding it fetches each
//! input the recipe names, a trip at a time, from the nearest stockpile or ground stack it can
//! reach -- exactly the sources `world/build.rs` fetches a building's cost from -- then steps
//! inside through the door (`World::enter_building`, #227: a smith works *in* the forge) and
//! spends the recipe's `work` seconds there, scaled by the nation's modifier if it names one.
//! Finishing a batch drops the output on the ground near the building (`World::place_ground`,
//! queuing a haul like any other yield) and the unit steps back out to start the next one.
//! Progress toward a batch (both the inputs delivered and the work done) is kept per building, in
//! `World::crafts`, the way a build site's is (`world/build.rs`'s `Site`): pulling a crafter off
//! for a need, or losing the claim, loses nothing of the batch under way.
//!
//! Needs pre-empt a craft job exactly like a study seat: `consider_needs` steps a unit standing
//! inside back out before planning anything, and the next tick picks the cycle up again from
//! wherever `World::crafts` says it left off.

use crate::building::{BuildingDef, RecipeDef};
use crate::jobs::{ItemId, JobKind, Stack};

use super::work::route_from;
use super::*;

/// One production building's progress toward its next batch: how much of each of its recipe's
/// inputs has been delivered so far (parallel to `RecipeDef::inputs`), and work done once they
/// all have. Missing from `World::crafts` is the same as a fresh, empty batch.
#[derive(Debug, Clone, PartialEq, Default, serde::Serialize, serde::Deserialize)]
pub(super) struct CraftState {
    delivered: Vec<u32>,
    progress: f32,
}

impl World {
    /// Opens `def.capacity` new `Craft` jobs at `pos`, if `def` has a recipe: called once, when a
    /// production building finishes construction, mirroring `open_study_seats`. A no-op for an
    /// ordinary building (`recipe` is `None`).
    pub(super) fn open_craft_jobs(&mut self, pos: Pos, def: &BuildingDef) {
        if def.recipe.is_some() {
            for _ in 0..def.capacity {
                self.add_job(JobKind::Craft { pos });
            }
        }
    }

    /// The recipe's next shortfall for the batch under way at `pos`: index into
    /// `RecipeDef::inputs` (and the state's `delivered`), the item, and how much more is needed.
    /// `None` once every input for this batch is in.
    fn next_craft_shortfall(&self, pos: Pos, recipe: &RecipeDef) -> Option<(ItemId, u32)> {
        let delivered = self.crafts.get(&pos).map(|s| s.delivered.as_slice()).unwrap_or(&[]);
        recipe.inputs.iter().enumerate().find_map(|(idx, c)| {
            let have = delivered.get(idx).copied().unwrap_or(0);
            (have < c.count).then_some((c.item, c.count - have))
        })
    }

    /// Adds a delivered stack to the building's batch under way. A no-op if the item isn't one
    /// of the recipe's inputs (shouldn't happen: a crafter only ever fetches what
    /// `next_craft_shortfall` named).
    fn deliver_to_craft(&mut self, pos: Pos, stack: Stack, recipe: &RecipeDef) {
        let Some(idx) = recipe.inputs.iter().position(|c| c.item == stack.item) else { return };
        let state = self.crafts.entry(pos).or_insert_with(|| CraftState {
            delivered: vec![0; recipe.inputs.len()],
            progress: 0.0,
        });
        if state.delivered.len() <= idx {
            state.delivered.resize(recipe.inputs.len(), 0);
        }
        state.delivered[idx] += stack.count;
    }

    /// One tick of a unit holding a craft job: fetch the recipe's inputs for the batch under way
    /// (as `world/build.rs` fetches a building's cost), then step inside and work; once the
    /// batch is done, drop the output nearby, clear the batch and start the next one. The job
    /// never finishes on its own, like a study seat.
    pub(super) fn run_craft(&mut self, i: usize, id: JobId, defs: &Defs) {
        let Some(idx) = self.job_index(id) else { return self.finish_job(i) };
        let JobKind::Craft { pos } = self.jobs[idx].kind else { return self.finish_job(i) };
        let Some(recipe) =
            self.building_at(pos).map(|b| defs.building(b)).and_then(|d| d.recipe.as_ref())
        else {
            // The building is gone, or its data lost its recipe: moot (no demolish command
            // exists yet, so unreachable in practice today).
            self.jobs.remove(idx);
            return self.finish_job(i);
        };

        if let Some(stack) = self.units[i].carrying {
            // Carrying a trip's worth of an input to the building.
            if self.units[i].route.is_none() {
                let flood = self.flood_from(i);
                match self.stand_building(&flood, pos, defs).and_then(|(_, s)| flood.path_to(s)) {
                    Some(path) => self.units[i].route = Some(route_from(path)),
                    None => return self.abandon_job(i, id, defs),
                }
            }
            if !self.walk(i, defs) {
                return;
            }
            self.deliver_to_craft(pos, stack, recipe);
            self.units[i].carrying = None;
            self.units[i].route = None;
            return;
        }

        match self.next_craft_shortfall(pos, recipe) {
            Some((item, needed)) => {
                let flood = self.flood_from(i);
                let Some((source, stand)) = self.nearest_source(&flood, item) else {
                    return self.abandon_job(i, id, defs);
                };
                if self.units[i].route.is_none() {
                    match flood.path_to(stand) {
                        Some(path) => self.units[i].route = Some(route_from(path)),
                        None => return self.abandon_job(i, id, defs),
                    }
                }
                if !self.walk(i, defs) {
                    return;
                }
                let take = self.take_for_build(i, source, needed, defs);
                if take == 0 {
                    // Someone else got there first: look again next tick.
                    return self.abandon_job(i, id, defs);
                }
                self.units[i].carrying = Some(Stack { item, count: take });
                self.units[i].route = None;
            }
            None => {
                // Fully supplied: work inside the building.
                if self.units[i].inside.is_none() {
                    if self.units[i].route.is_none() {
                        let flood = self.flood_from(i);
                        match self.stand_entrance(&flood, pos).and_then(|(_, s)| flood.path_to(s)) {
                            Some(path) => self.units[i].route = Some(route_from(path)),
                            None => return self.abandon_job(i, id, defs),
                        }
                    }
                    if !self.walk(i, defs) {
                        return;
                    }
                    if !self.enter_building(i, pos, defs) {
                        return;
                    }
                }
                let mods = self.modifiers_for(&self.units[i], defs);
                let rate = recipe.modifier.as_deref().and_then(|m| mods.get(m)).unwrap_or(1.0);
                let len = recipe.inputs.len();
                let state = self
                    .crafts
                    .entry(pos)
                    .or_insert_with(|| CraftState { delivered: vec![0; len], progress: 0.0 });
                state.progress += rate / TICKS_PER_SECOND as f32;
                if state.progress < recipe.work {
                    return;
                }
                let output = recipe.output;
                self.crafts.remove(&pos);
                self.leave_building(i);
                self.units[i].route = None;
                self.place_ground(pos, Stack { item: output.item, count: output.count });
            }
        }
    }
}

#[cfg(test)]
mod tests;
