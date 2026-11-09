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
//!
//! A recipe's `stock_limit` (issue #350) stops a crafter from starting a *new* batch once that
//! many of its output are already in stores (`World::craft_idle`); a batch already under way
//! still finishes. This is the only thing keeping the Brick Kiln and the smelter from burning
//! every stone or ore a settlement can carry into an output nothing uses.

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

    /// Whether the craft job at `pos` is idle at `recipe`'s `stock_limit` (issue #350): no batch
    /// is under way (nothing of this one delivered yet) and the output is already at or past the
    /// limit. A batch already started (`World::crafts` holds an entry for `pos`) always finishes,
    /// however much is in stores. `stock_limit` 0 means no limit, so this is always false. Used
    /// both to gate `run_craft` and to describe the crafter's task to the player.
    pub fn craft_idle(&self, pos: Pos, recipe: &RecipeDef) -> bool {
        recipe.stock_limit > 0
            && !self.crafts.contains_key(&pos)
            && self.stored_total(recipe.output.item) >= recipe.stock_limit
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
        let Some(def) = self.building_at(pos).map(|b| defs.building(b)) else {
            // The building is gone: moot (no demolish command exists yet, so unreachable in
            // practice today).
            self.jobs.remove(idx);
            return self.finish_job(i);
        };
        let Some(recipe) = def.recipe.as_ref() else {
            // Its data lost its recipe: moot, same as above.
            self.jobs.remove(idx);
            return self.finish_job(i);
        };

        if let Some(stack) = self.units[i].carrying {
            // Carrying a trip's worth of an input to the building.
            if !self.go_or_abandon(i, id, StandRule::Building(pos), defs) {
                return;
            }
            self.deliver_to_craft(pos, stack, recipe);
            self.units[i].carrying = None;
            self.units[i].route = None;
            return;
        }

        if self.craft_idle(pos, recipe) {
            // At the stock limit with no batch under way: wait rather than fetch inputs for one
            // that would just pile up more output nothing uses (issue #350).
            return;
        }

        match self.next_craft_shortfall(pos, recipe) {
            Some((item, needed)) => {
                if self.keep_walking(i, defs) {
                    return;
                }
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
                    if !self.go_or_abandon(i, id, StandRule::Entrance(pos), defs) {
                        return;
                    }
                    if !self.enter_building(i, pos, defs) {
                        return;
                    }
                }
                let mods = self.modifiers_for(&self.units[i], defs);
                let rate = recipe.modifier.as_deref().and_then(|m| mods.get(m)).unwrap_or(1.0);
                // A powered building's batch runs `boost` times as fast (issue #428): the best
                // boost in reach of any tile of its footprint, 1.0 with nothing in reach.
                let boost = if def.uses_power {
                    self.footprint_at(pos).map_or(1.0, |f| self.power_boost(f, defs))
                } else {
                    1.0
                };
                let len = recipe.inputs.len();
                let state = self
                    .crafts
                    .entry(pos)
                    .or_insert_with(|| CraftState { delivered: vec![0; len], progress: 0.0 });
                state.progress += rate * boost / TICKS_PER_SECOND as f32;
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
