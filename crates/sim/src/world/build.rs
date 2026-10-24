//! Construction: designating a building, hauling its cost to the site, and the work that
//! finishes it (`data/buildings.toml`, issue #64).
//!
//! A build is a job like a harvest (`JobKind::Build`). The player designates a free, buildable
//! tile (`Command::DesignateBuild`); one unit claims it and sees the whole thing through: fetch
//! each item the building costs, a trip at a time, from the nearest stockpile or ground stack it
//! can reach, then work the site for `BuildingDef::work` seconds (scaled by the nation's
//! `construction` modifier, data/nations.toml). Needs pre-empt it and progress (both materials
//! delivered and work done) is kept, exactly as for harvests.
//!
//! On completion the tile gets a `Building` (`World::building_at`): `BuildingDef::passable` is
//! baked onto it, so pathfinding (`path.rs`, `flood.rs`) never has to consult the defs to know
//! whether it blocks movement. What a finished building *does* is mostly out of scope here (#58
//! beds/hearth), except a study hall's seats, which `study.rs` opens as jobs right away.

use crate::building::{Building, BuildingId};
use crate::flood::Flood;
use crate::jobs::{ItemId, JobKind, Stack};

use super::work::route_from;
use super::*;

/// Why a build command was refused. The world is unchanged when `apply` returns one of these.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BuildError {
    /// There is no building with this id.
    UnknownBuilding,
    OutOfBounds,
    /// The tile isn't free: it's impassable, or a building already stands (or is being built)
    /// there.
    CannotBuild,
    /// The building needs a tech the nation doesn't know yet. The client can look up its name
    /// for a reason like "Requires Fire Making" (issue #22).
    NotResearched(TechId),
    /// There is no build designation on that tile to cancel.
    NotDesignated,
}

impl std::fmt::Display for BuildError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnknownBuilding => f.write_str("no such building"),
            Self::OutOfBounds => f.write_str("off the map"),
            Self::CannotBuild => f.write_str("can't build here"),
            Self::NotResearched(_) => f.write_str("not researched yet"),
            Self::NotDesignated => f.write_str("not designated"),
        }
    }
}

impl std::error::Error for BuildError {}

/// Where a unit fetching a building's cost found it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Source {
    Stockpile(Pos),
    Ground(Pos),
}

/// A build designation in progress: how much of the building's cost has been delivered so far
/// (parallel to `BuildingDef::cost`), and work done on the site once it's fully supplied.
#[derive(Debug, Clone, PartialEq)]
pub(super) struct Site {
    pub building: BuildingId,
    delivered: Vec<u32>,
    progress: f32,
}

/// Queries, and the designations behind `Command::DesignateBuild` and `Command::CancelBuild`.
impl World {
    /// Build designations that aren't finished, with the building and work done so far, in tile
    /// order.
    pub fn build_sites(&self) -> impl Iterator<Item = (Pos, BuildingId, f32)> + '_ {
        self.sites.iter().map(|(&p, s)| (p, s.building, s.progress))
    }

    /// The building finished on `pos`, if any.
    pub fn building_at(&self, pos: Pos) -> Option<BuildingId> {
        self.map.get(pos)?.building.map(|b| b.id)
    }

    /// Designates a building on `pos`.
    pub(super) fn designate_build(
        &mut self,
        pos: Pos,
        building: BuildingId,
        defs: &Defs,
    ) -> Result<JobId, BuildError> {
        let def = defs.buildings.get(building.0 as usize).ok_or(BuildError::UnknownBuilding)?;
        if let Some(t) = def.unlocked_by.filter(|&t| !self.research.is_known(t)) {
            return Err(BuildError::NotResearched(t));
        }
        let tile = self.map.get(pos).ok_or(BuildError::OutOfBounds)?;
        if !self.rules.passable(tile) || tile.building.is_some() || self.sites.contains_key(&pos) {
            return Err(BuildError::CannotBuild);
        }
        self.sites
            .insert(pos, Site { building, delivered: vec![0; def.cost.len()], progress: 0.0 });
        Ok(self.add_job(JobKind::Build { pos }))
    }

    /// Withdraws the build designation on `pos`. Materials already delivered drop on the ground
    /// for hauling; work done on the site is lost.
    pub(super) fn cancel_build(&mut self, pos: Pos, defs: &Defs) -> Result<(), BuildError> {
        let Some(site) = self.sites.remove(&pos) else { return Err(BuildError::NotDesignated) };
        self.jobs.retain(|j| j.kind != JobKind::Build { pos });
        let def = defs.building(site.building);
        for (cost, &count) in def.cost.iter().zip(&site.delivered) {
            if count > 0 {
                self.place_ground(pos, Stack { item: cost.item, count });
            }
        }
        Ok(())
    }
}

/// Doing it.
impl World {
    /// The next cost entry this site still needs, with how much: index into `BuildingDef::cost`
    /// (and `Site::delivered`), the item, and the amount still short.
    fn next_shortfall(&self, pos: Pos, defs: &Defs) -> Option<(usize, ItemId, u32)> {
        let site = self.sites.get(&pos)?;
        let cost = &defs.building(site.building).cost;
        cost.iter()
            .zip(&site.delivered)
            .enumerate()
            .find_map(|(idx, (c, &d))| (d < c.count).then_some((idx, c.item, c.count - d)))
    }

    /// The nearest stockpile tile or ground stack holding `item`, stockpiles preferred on a tie,
    /// with the tile a unit should stand on to reach it.
    fn nearest_source(&self, flood: &Flood, item: ItemId) -> Option<(Source, Pos)> {
        let mut best: Option<((u32, u8, Pos), Source, Pos)> = None;
        let mut consider = |rank: u8, pos: Pos, source: Source, stand: Option<(u32, Pos)>| {
            let Some((cost, stand)) = stand else { return };
            let key = (cost, rank, pos);
            if best.as_ref().is_none_or(|b| key < b.0) {
                best = Some((key, source, stand));
            }
        };
        for (&pos, s) in &self.stored {
            if s.item == item {
                consider(0, pos, Source::Stockpile(pos), self.stand(flood, pos));
            }
        }
        for (&pos, s) in &self.ground {
            if s.item == item {
                consider(1, pos, Source::Ground(pos), self.stand(flood, pos));
            }
        }
        best.map(|(_, source, stand)| (source, stand))
    }

    /// Takes up to `needed`, capped by carry capacity and what's actually there, from `source`.
    fn take_for_build(&mut self, i: usize, source: Source, needed: u32, defs: &Defs) -> u32 {
        let cap = self.carry_capacity(i, defs);
        let pos = match source {
            Source::Stockpile(p) | Source::Ground(p) => p,
        };
        let stock = match source {
            Source::Stockpile(_) => &mut self.stored,
            Source::Ground(_) => &mut self.ground,
        };
        let Some(s) = stock.get_mut(&pos) else { return 0 };
        let take = s.count.min(needed).min(cap);
        s.count -= take;
        if s.count == 0 {
            stock.remove(&pos);
        }
        take
    }

    /// Adds a delivered stack to the site's tally for the cost entry it matches. A no-op if the
    /// site is gone (cancelled mid-trip) or the item isn't part of its cost.
    fn deliver_to_site(&mut self, pos: Pos, stack: Stack, defs: &Defs) {
        let Some(site) = self.sites.get(&pos) else { return };
        let cost = &defs.building(site.building).cost;
        if let Some(idx) = cost.iter().position(|c| c.item == stack.item) {
            self.sites.get_mut(&pos).expect("just read").delivered[idx] += stack.count;
        }
    }

    pub(super) fn run_build(&mut self, i: usize, id: JobId, defs: &Defs) {
        let Some(idx) = self.job_index(id) else { return self.finish_job(i) };
        let JobKind::Build { pos } = self.jobs[idx].kind else { return self.finish_job(i) };
        if !self.sites.contains_key(&pos) {
            // The site vanished (cancelled, which also drops the job): moot.
            self.jobs.remove(idx);
            return self.finish_job(i);
        }

        if let Some(stack) = self.units[i].carrying {
            // Carrying a trip's worth of materials to the site.
            if self.units[i].route.is_none() {
                let flood = self.flood_from(i);
                match self.stand(&flood, pos).and_then(|(_, s)| flood.path_to(s)) {
                    Some(path) => self.units[i].route = Some(route_from(path)),
                    None => return self.abandon_job(i, id, defs),
                }
            }
            if !self.walk(i, defs) {
                return;
            }
            self.deliver_to_site(pos, stack, defs);
            self.units[i].carrying = None;
            self.units[i].route = None;
            return;
        }

        match self.next_shortfall(pos, defs) {
            Some((_, item, needed)) => {
                // Fetch a trip's worth of the next missing item.
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
                // Fully supplied: work the site.
                if self.units[i].route.is_none() {
                    let flood = self.flood_from(i);
                    match self.stand(&flood, pos).and_then(|(_, s)| flood.path_to(s)) {
                        Some(path) => self.units[i].route = Some(route_from(path)),
                        None => return self.abandon_job(i, id, defs),
                    }
                }
                if !self.walk(i, defs) {
                    return;
                }
                let rate = self.modifiers().construction;
                let def = defs.building(self.sites[&pos].building);
                let site = self.sites.get_mut(&pos).expect("checked above");
                site.progress += rate / TICKS_PER_SECOND as f32;
                if site.progress < def.work {
                    return;
                }
                let building = Building { id: site.building, passable: def.passable };
                let seats = def.seats;
                self.sites.remove(&pos);
                self.jobs.remove(idx);
                self.map[pos].building = Some(building);
                self.map_changed();
                self.open_study_seats(pos, seats);
                self.finish_job(i);
            }
        }
    }
}

#[cfg(test)]
mod tests;
