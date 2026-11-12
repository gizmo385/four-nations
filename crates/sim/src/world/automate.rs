//! The Factory (docs/design/horizons-phase-4.md, `great_works`, #725).
//!
//! A finished building with a recipe and `uses_power`, whose footprint is within `radius` of a
//! power source whose `automate` upgrade is known, is **automated** unless the player took it
//! off (`Command::SetAutomated { on: false }`, `Building::manual`). Its Craft jobs can't be
//! claimed. Its batch runs on its own at `rate` times the recipe's plain speed (the recipe's
//! `modifier` applies, the power boost doesn't), and its inputs, one batch at a time, come by a
//! **Supply** haul: a `JobKind::Refuel` on the building that fetches the next input short
//! (`run_supply`), as a fuel bunker's does. The output drops beside it as a crafter's would, and
//! `stock_limit` still idles it.

use crate::jobs::{ItemId, JobKind, Stack};

use super::work::route_from;
use super::*;

/// A batch's inputs as `(item, delivered, needed)`, and the work done as a fraction
/// (`World::craft_batch`).
pub type CraftBatch = (Vec<(ItemId, u32, u32)>, f32);

impl World {
    /// The rate the building anchored on `pos` runs at with no crafter, if it is automated now.
    pub fn automated(&self, pos: Pos, defs: &Defs) -> Option<f32> {
        let b = self.anchored(pos)?;
        if b.manual {
            return None;
        }
        self.yard_rate(pos, defs)
    }

    /// Whether the building anchored on `pos` stands in an automating yard, on automation or
    /// taken off it: what `Command::SetAutomated` accepts.
    pub fn automatable(&self, pos: Pos, defs: &Defs) -> bool {
        self.yard_rate(pos, defs).is_some()
    }

    /// The batch under way at the recipe building anchored on `pos`, for a client's readout:
    /// each input as `(item, delivered, needed)`, and the work done as a fraction of the batch.
    /// `None` off a recipe building; all zeros before a batch begins.
    pub fn craft_batch(&self, pos: Pos, defs: &Defs) -> Option<CraftBatch> {
        let recipe = self.recipe_at(pos, defs)?;
        let state = self.anchored(pos)?.craft.as_ref();
        let inputs = recipe
            .inputs
            .iter()
            .enumerate()
            .map(|(k, c)| {
                let got = state.and_then(|s| s.delivered.get(k)).copied().unwrap_or(0);
                (c.item, got.min(c.count), c.count)
            })
            .collect();
        let done = state.map_or(0.0, |s| (s.progress / recipe.work.max(f32::EPSILON)).min(1.0));
        Some((inputs, done))
    }

    /// The rate of the best automating yard the building anchored on `pos` stands in, whether
    /// or not the player took it off automation: `None` for a building automation can't touch.
    fn yard_rate(&self, pos: Pos, defs: &Defs) -> Option<f32> {
        if !self.has_kind(Kind::Automates) {
            return None;
        }
        let b = self.anchored(pos)?;
        let def = defs.building(b.id);
        if def.recipe.is_none() || !def.uses_power {
            return None;
        }
        let foot = b.footprint();
        self.of_kind(Kind::Automates)
            .filter_map(|s| {
                let (tech, a) = defs.building(s.id).automate_upgrade?;
                (self.research.is_known(tech) && s.footprint().gap(foot) <= a.radius as i32)
                    .then_some(a.rate)
            })
            .reduce(f32::max)
    }

    /// `Command::SetAutomated`: puts the building anchored on `pos` on automation, or gives it
    /// back to a crafter. Refused off a finished building, while no automating tech is known,
    /// and outside every automating yard.
    pub(super) fn set_automated(
        &mut self,
        pos: Pos,
        on: bool,
        defs: &Defs,
    ) -> Result<(), RecipeError> {
        let Some(b) = self.anchored(pos) else {
            return Err(if self.sites.contains_key(&pos) {
                RecipeError::Site
            } else {
                RecipeError::NoBuilding
            });
        };
        if defs.building(b.id).recipe.is_none() {
            return Err(RecipeError::NoSuchRecipe);
        }
        let sources: Vec<_> =
            defs.buildings.iter().filter_map(|d| Some((d, d.automate_upgrade?.0))).collect();
        if !sources.iter().any(|&(_, t)| self.research.is_known(t)) {
            let tech = sources.first().map_or("automation", |&(_, t)| defs.tech(t).name.as_str());
            return Err(RecipeError::Locked(tech.to_owned()));
        }
        if self.yard_rate(pos, defs).is_none() {
            let name = sources.first().map_or("", |(d, _)| d.display_name(self.nation));
            return Err(RecipeError::NotInYard(name.to_owned()));
        }
        self.anchored_mut(pos).expect("just found").manual = !on;
        Ok(())
    }

    /// The next input an automated building's batch is short, or `None` when it needs nothing
    /// (not automated, idle at its stock limit, or fully supplied).
    pub(super) fn supply_shortfall(&self, pos: Pos, defs: &Defs) -> Option<(ItemId, u32)> {
        self.automated(pos, defs)?;
        if self.craft_idle(pos, defs) {
            return None;
        }
        self.next_craft_shortfall(pos, self.recipe_at(pos, defs)?)
    }

    /// Once a tick: every automated building with its batch supplied works it; one short posts
    /// a Supply haul if it has none.
    pub(super) fn tick_automation(&mut self, defs: &Defs) {
        if !self.has_kind(Kind::Automates) {
            return;
        }
        let anchors: Vec<Pos> = self.buildings().map(|b| b.anchor).collect();
        for pos in anchors {
            let Some(rate) = self.automated(pos, defs) else { continue };
            let Some(recipe) = self.recipe_at(pos, defs) else { continue };
            if self.craft_idle(pos, defs) {
                continue;
            }
            if self.next_craft_shortfall(pos, recipe).is_some() {
                if !self.jobs.iter().any(|j| j.kind == JobKind::Refuel { pos }) {
                    self.add_job(JobKind::Refuel { pos });
                }
                continue;
            }
            let speed = recipe.modifier.as_deref().and_then(|m| self.modifiers().get(m));
            let step = rate * speed.unwrap_or(1.0) / TICKS_PER_SECOND as f32;
            let (len, work, output) = (recipe.inputs.len(), recipe.work, recipe.output);
            let b = self.anchored_mut(pos).expect("listed");
            let state = b.craft.get_or_insert_with(|| super::forge::CraftState {
                delivered: vec![0; len],
                progress: 0.0,
            });
            state.progress += step;
            if state.progress >= work {
                b.craft = None;
                self.place_ground(pos, Stack { item: output.item, count: output.count });
            }
        }
    }

    /// One tick of a unit on a Supply haul to the automated building anchored on `pos`: fetch
    /// the next input short from the nearest store, carry it over and put it in the batch. One
    /// trip finishes the job; `tick_automation` posts another while the batch is still short.
    pub(super) fn run_supply(&mut self, i: usize, id: JobId, pos: Pos, defs: &Defs) {
        let Some(idx) = self.job_index(id) else { return self.finish_job(i) };
        let Some(recipe) = self.recipe_at(pos, defs) else {
            self.jobs.remove(idx);
            return self.finish_job(i);
        };
        let wanted = |s: &Stack| recipe.inputs.iter().any(|c| c.item == s.item);
        if let Some(stack) = self.units[i].carrying.filter(wanted) {
            if !self.go_or_abandon(i, id, StandRule::Building(pos), defs) {
                return;
            }
            self.deliver_to_craft(pos, stack, recipe);
            self.units[i].carrying = None;
            self.units[i].route = None;
            let idx = self.job_index(id).expect("still queued");
            self.jobs.remove(idx);
            return self.finish_job(i);
        }
        let Some((item, short)) = self.supply_shortfall(pos, defs) else {
            self.jobs.remove(idx);
            return self.finish_job(i);
        };
        if self.units[i].carrying.is_some() {
            self.jobs.remove(idx);
            return self.finish_job(i);
        }
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
        let take = self.take_for_build(i, source, short, defs);
        if take == 0 {
            return self.abandon_job(i, id, defs);
        }
        self.units[i].carrying = Some(Stack { item, count: take });
        self.units[i].route = None;
    }

    /// Whether some store holds what the automated building on `pos` is short of.
    pub(super) fn supply_ready(&self, pos: Pos, defs: &Defs) -> bool {
        self.supply_shortfall(pos, defs).is_some_and(|(item, _)| self.any_source_for_fuel(item))
    }
}

#[cfg(test)]
mod tests;
