//! Construction: designating a building, hauling its cost to the site, and the work that
//! finishes it (`data/buildings.toml`, issue #64).
//!
//! A build is a job like a harvest (`JobKind::Build`). The player designates a building's
//! anchor, the top-left tile of its footprint (`Command::DesignateBuild`, #207), and the whole
//! footprint must be free, buildable land (`World::check_build`); one unit claims it and sees
//! the whole thing through: fetch each item the building costs, a trip at a time, from the
//! nearest stockpile or ground stack it can reach, then work the site for `BuildingDef::work`
//! seconds (scaled by the nation's `construction` modifier, data/nations.toml). Needs pre-empt it
//! and progress (both materials delivered and work done) is kept, exactly as for harvests.
//!
//! On completion every footprint tile gets a copy of the `Building` (`World::building_at`), with
//! its own `part` and the footprint's `size`. `BuildingDef::passable` is baked onto it, so
//! pathfinding (`path.rs`, `flood.rs`) never has to consult the defs to know whether it blocks
//! movement. What a finished building *does* is mostly elsewhere: beds are
//! slept in by `work.rs`, and a building that keeps a fire (`BuildingDef::heat`) lights it here,
//! in `heat.rs`' terms, the moment it is finished (#58). A study hall's seats, which `study.rs`
//! opens as jobs, are also opened right away.
//!
//! The endeavor's gate (#236) is a site here too, so its footprint blocks other buildings and its
//! construct work is a `Build` job, but its cost and work come from its stages: `run_build` hands
//! it to `gate.rs`. The queries for ordinary sites (`build_sites`, `build_site`) leave it out, and
//! `CancelBuild` refuses it.

use crate::building::{Building, BuildingId, is_one_by_one, one_by_one};
use crate::defs::Terrain;
use crate::flood::Flood;
use crate::geom::Footprint;
use crate::jobs::{ItemId, JobKind, Stack};

use super::work::route_from;
use super::*;

/// Why a build command was refused. The world is unchanged when `apply` returns one of these.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BuildError {
    /// There is no building with this id.
    UnknownBuilding,
    /// A footprint tile can't take the building: `at` is the first such tile, row-major from the
    /// anchor, and `why` says what is in the way, so a frontend can name it.
    CannotBuild { at: Pos, why: Blocker },
    /// The building needs a tech the nation doesn't know yet. The client can look up its name
    /// for a reason like "Requires Fire Making" (issue #22).
    NotResearched(TechId),
    /// There is no build designation on that tile to cancel.
    NotDesignated,
    /// The building is the endeavor's (`BuildingDef::endeavor`): the Endeavor screen places and
    /// cancels it (`Command::BeginEndeavor`, `Command::CancelEndeavor`).
    Endeavor,
}

impl std::fmt::Display for BuildError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnknownBuilding => f.write_str("no such building"),
            Self::CannotBuild { why, .. } => write!(f, "{why}"),
            Self::NotResearched(_) => f.write_str("not researched yet"),
            Self::NotDesignated => f.write_str("not designated"),
            Self::Endeavor => f.write_str("placed from the Endeavor screen"),
        }
    }
}

impl std::error::Error for BuildError {}

/// Why one tile can't be part of a new building's footprint (`World::build_blocker`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Blocker {
    OffMap,
    /// The ground isn't `Land` or `Coast`: water, mountains and fords (which mapgen placed to
    /// keep landmasses connected, so nothing may close one) can't be built on.
    Terrain,
    /// A tree, rock or other resource stands there.
    Resource,
    /// A finished building stands there.
    Building,
    /// Another building is designated there.
    Site,
    /// It is a stockpile tile, which a building would bury.
    Stockpile,
    /// The footprint's own door (`Footprint::entrance`) isn't walkable, or it would cover
    /// another building's or site's door (#226, docs/design/buildings.md, "Units inside
    /// buildings"): a building's door never moves once it's placed, so a unit that walked in
    /// could never be found again.
    Entrance,
}

impl std::fmt::Display for Blocker {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::OffMap => "off the map",
            Self::Terrain => "can't build on this ground",
            Self::Resource => "something is in the way",
            Self::Building => "a building is in the way",
            Self::Site => "already designated",
            Self::Stockpile => "a stockpile is in the way",
            Self::Entrance => "blocks a door",
        })
    }
}

/// Where a unit fetching a building's cost found it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Source {
    Stockpile(Pos),
    Ground(Pos),
}

/// A build designation in progress: how much of the building's cost has been delivered so far
/// (parallel to `BuildingDef::cost`), and work done on the site once it's fully supplied.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub(super) struct Site {
    pub building: BuildingId,
    delivered: Vec<u32>,
    progress: f32,
    /// The footprint it was designated with, `[w, h]`; the site is keyed by its anchor. Skipped
    /// when 1x1, like `Building::size`.
    #[serde(default = "one_by_one", skip_serializing_if = "is_one_by_one")]
    size: [u8; 2],
}

impl Site {
    /// The endeavor's site (`gate.rs`): it only holds the footprint, since the gate's deliveries
    /// and work are per stage and live in `EndeavorState::Building`.
    pub(super) fn gate(building: BuildingId, size: [u8; 2]) -> Self {
        Self { building, delivered: Vec::new(), progress: 0.0, size }
    }

    fn footprint(&self, anchor: Pos) -> Footprint {
        Footprint::new(anchor, self.size[0], self.size[1])
    }
}

/// Queries, and the designations behind `Command::DesignateBuild` and `Command::CancelBuild`.
impl World {
    /// Build designations that aren't finished, with the building and work done so far, in
    /// anchor order. The endeavor's site isn't one (`World::endeavor`).
    pub fn build_sites(&self) -> impl Iterator<Item = (Footprint, BuildingId, f32)> + '_ {
        self.sites
            .iter()
            .filter(|&(&p, _)| !self.is_gate(p))
            .map(|(&p, s)| (s.footprint(p), s.building, s.progress))
    }

    /// The unfinished build designation covering `pos` (any tile of its footprint), if any: the
    /// building, how much of each cost entry has been delivered (parallel to
    /// `BuildingDef::cost`) and the work done so far (out of `BuildingDef::work`). For a client
    /// to show a site's progress.
    pub fn build_site(&self, pos: Pos) -> Option<(BuildingId, &[u32], f32)> {
        let (anchor, s) = self.site_at(pos)?;
        (!self.is_gate(anchor)).then_some((s.building, &s.delivered, s.progress))
    }

    /// The building finished on `pos`, any tile of its footprint, if any.
    pub fn building_at(&self, pos: Pos) -> Option<BuildingId> {
        self.map.get(pos)?.building.map(|b| b.id)
    }

    /// The footprint of the finished building or the build site covering `pos`, if any.
    pub fn footprint_at(&self, pos: Pos) -> Option<Footprint> {
        match self.map.get(pos)?.building {
            Some(b) => Some(b.footprint(pos)),
            None => self.site_at(pos).map(|(anchor, s)| s.footprint(anchor)),
        }
    }

    /// Every finished building once, with its footprint, in anchor order (row-major). Scans the
    /// whole map: `World::map` is public, so a caller (or a frontend's test) may have put a
    /// building straight onto it without going through `raise`, and this has to see it too.
    /// `building_index` (#292) isn't a substitute for that; it only speeds up the one caller
    /// that is guaranteed to see only buildings `raise` put there (`finished_buildings` below).
    pub fn buildings(&self) -> impl Iterator<Item = (Footprint, BuildingId)> + '_ {
        self.map.iter().filter_map(|(p, t)| {
            let b = t.building?;
            (b.part == [0, 0]).then(|| (b.footprint(p), b.id))
        })
    }

    /// Rebuilds `building_index` from a full map scan: used once after loading a save, since the
    /// index isn't saved (`#[serde(skip)]` on `World::building_index`).
    pub(crate) fn rebuild_building_index(&mut self) {
        self.building_index = self
            .map
            .iter()
            .filter_map(|(p, t)| {
                t.building.is_some_and(|b| b.part == [0, 0]).then_some(((p.y, p.x), p))
            })
            .collect();
    }

    /// The site whose footprint covers `pos`, with its anchor. Sites are few (tens), so this
    /// scans them rather than keep a second index in sync.
    pub(super) fn site_at(&self, pos: Pos) -> Option<(Pos, &Site)> {
        self.sites.iter().find(|&(&a, s)| s.footprint(a).contains(pos)).map(|(&a, s)| (a, s))
    }

    /// The area a unit works the building or site on `pos` (any tile of it) from, and whether
    /// that is the footprint itself (a walkable building) rather than the ring around it (a
    /// solid one, whose site is worked from outside too, so no builder is left standing inside
    /// it). A tile with neither is its own 1x1 area, walkable if units can enter it.
    pub(super) fn work_area(&self, pos: Pos, defs: &Defs) -> (Footprint, bool) {
        if let Some(b) = self.map.get(pos).and_then(|t| t.building) {
            (b.footprint(pos), b.passable)
        } else if let Some((anchor, s)) = self.site_at(pos) {
            (s.footprint(anchor), defs.building(s.building).passable)
        } else {
            let walkable = self.map.get(pos).is_some_and(|t| self.rules.passable(t));
            (Footprint::single(pos), walkable)
        }
    }

    /// Puts the finished `building` on every tile of `foot`, each with its own `part`, and moves
    /// whatever a solid one would bury.
    pub(super) fn raise(&mut self, foot: Footprint, building: BuildingId, def: &BuildingDef) {
        let b = Building {
            size: [foot.w, foot.h],
            ..Building::new(building, def.passable, def.rail.is_some())
        };
        for p in foot.tiles() {
            let part = [(p.x - foot.anchor.x) as u8, (p.y - foot.anchor.y) as u8];
            self.map[p].building = Some(Building { part, ..b });
        }
        self.building_index.insert((foot.anchor.y, foot.anchor.x), foot.anchor);
        if !def.passable {
            self.clear_footprint(foot);
        }
        self.map_changed();
    }

    /// Moves what a solid building just finished on `foot` would bury: each unit on it steps to
    /// the nearest walkable tile outside it and replans, and each ground stack there is put down
    /// again nearby. The building must already be on the map, so `place_ground` avoids it.
    fn clear_footprint(&mut self, foot: Footprint) {
        for i in 0..self.units.len() {
            let at = self.units[i].pos;
            if !foot.contains(at) {
                continue;
            }
            if let Some(to) = self.nearest_outside(at, foot) {
                self.units[i].pos = to;
            }
            self.units[i].route = None;
            self.units[i].move_progress = 0.0;
        }
        let buried: Vec<Pos> = foot.tiles().filter(|p| self.ground.contains_key(p)).collect();
        for p in buried {
            let stack = self.ground.remove(&p).expect("just found");
            self.place_ground(p, stack);
        }
    }

    /// The nearest tile to `from` that units can enter and `foot` doesn't cover, searching
    /// Chebyshev rings outward, each row-major, as `place_ground` does.
    fn nearest_outside(&self, from: Pos, foot: Footprint) -> Option<Pos> {
        let reach = i32::from(foot.w.max(foot.h)) + 8;
        (1..=reach).find_map(|r| {
            (-r..=r)
                .flat_map(|dy| (-r..=r).map(move |dx| Pos::new(from.x + dx, from.y + dy)))
                .filter(|p| p.chebyshev(from) == r && !foot.contains(*p))
                .find(|&p| self.map.get(p).is_some_and(|t| self.rules.passable(t)))
        })
    }

    /// Why `pos` can't be part of a new building's footprint, if anything. Units standing there
    /// don't count: they step aside.
    pub fn build_blocker(&self, pos: Pos) -> Option<Blocker> {
        let Some(tile) = self.map.get(pos) else { return Some(Blocker::OffMap) };
        if !matches!(tile.terrain, Terrain::Land | Terrain::Coast) {
            Some(Blocker::Terrain)
        } else if tile.resource.is_some() {
            Some(Blocker::Resource)
        } else if tile.building.is_some() {
            Some(Blocker::Building)
        } else if self.site_at(pos).is_some() {
            Some(Blocker::Site)
        } else if self.zones.contains(&pos) {
            Some(Blocker::Stockpile)
        } else {
            None
        }
    }

    /// Whether `building` may be designated with its anchor (top-left tile) on `anchor`: the
    /// tech first, then every footprint tile, row-major. Changes nothing, so a frontend can call
    /// it to preview a placement.
    pub fn check_build(
        &self,
        anchor: Pos,
        building: BuildingId,
        defs: &Defs,
    ) -> Result<(), BuildError> {
        let def = defs.buildings.get(building.0 as usize).ok_or(BuildError::UnknownBuilding)?;
        if def.endeavor {
            return Err(BuildError::Endeavor);
        }
        if let Some(t) = def.unlocked_by.filter(|&t| !self.research.is_known(t)) {
            return Err(BuildError::NotResearched(t));
        }
        self.footprint_blocker(anchor, def, defs)
            .map_or(Ok(()), |(at, why)| Err(BuildError::CannotBuild { at, why }))
    }

    /// The first tile of `def`'s footprint, anchored on `anchor`, that can't take it (row-major),
    /// and why: every footprint tile first, then its door (#226).
    pub(super) fn footprint_blocker(
        &self,
        anchor: Pos,
        def: &BuildingDef,
        defs: &Defs,
    ) -> Option<(Pos, Blocker)> {
        let (w, h) = def.footprint;
        let foot = Footprint::new(anchor, w, h);
        foot.tiles()
            .find_map(|at| self.build_blocker(at).map(|why| (at, why)))
            .or_else(|| self.entrance_blocker(foot, def.capacity > 0, defs))
    }

    /// Whether `pos` could hold a door: walkable the way a unit stepping through one needs
    /// (`entrance_blocker`'s own check). The frontends' placement ghost (#230) uses this to
    /// colour a footprint's own entrance tile without duplicating `rules`, which is private.
    pub fn door_walkable(&self, pos: Pos) -> bool {
        self.map.get(pos).is_some_and(|t| self.rules.passable(t))
    }

    /// The building whose door is at `pos`, if any: a finished building or an unfinished site,
    /// so long as it can be entered (`BuildingDef::capacity` above 0; walls, beds and campfires
    /// have no door and never answer here). `entrance_blocker` uses this to refuse a footprint
    /// that would cover it; the frontends (#230) use it to name whose door a refused placement
    /// would block.
    pub fn door_owner(&self, pos: Pos, defs: &Defs) -> Option<BuildingId> {
        let door =
            |f: Footprint, b: BuildingId| (defs.building(b).capacity > 0).then(|| f.entrance());
        self.buildings().find_map(|(f, b)| (door(f, b) == Some(pos)).then_some(b)).or_else(|| {
            self.build_sites().find_map(|(f, b, _)| (door(f, b) == Some(pos)).then_some(b))
        })
    }

    /// Why `foot`'s door can't be placed, if anything (#226): it has a door (`has_door`, a
    /// capacity above 0) that isn't walkable, or the footprint would cover a door another finished
    /// building or site already has. Only buildings that can be entered have doors, so walls,
    /// beds and campfires pack freely, a vertical wall included. A unit standing on the door
    /// doesn't block it, like anywhere else in the footprint.
    fn entrance_blocker(
        &self,
        foot: Footprint,
        has_door: bool,
        defs: &Defs,
    ) -> Option<(Pos, Blocker)> {
        let entrance = foot.entrance();
        if has_door && !self.door_walkable(entrance) {
            return Some((entrance, Blocker::Entrance));
        }
        foot.tiles().find(|&p| self.door_owner(p, defs).is_some()).map(|p| (p, Blocker::Entrance))
    }

    /// Designates a building with its anchor on `pos`: the whole footprint, or nothing.
    pub(super) fn designate_build(
        &mut self,
        pos: Pos,
        building: BuildingId,
        defs: &Defs,
    ) -> Result<JobId, BuildError> {
        self.check_build(pos, building, defs)?;
        let def = defs.building(building);
        let (w, h) = def.footprint;
        let delivered = vec![0; def.cost.len()];
        self.sites.insert(pos, Site { building, delivered, progress: 0.0, size: [w, h] });
        Ok(self.add_job(JobKind::Build { pos }))
    }

    /// Withdraws the build designation covering `pos` (any tile of its footprint). Materials
    /// already delivered drop on the ground near its anchor for hauling; work done on the site
    /// is lost.
    pub(super) fn cancel_build(&mut self, pos: Pos, defs: &Defs) -> Result<(), BuildError> {
        let Some((pos, _)) = self.site_at(pos) else { return Err(BuildError::NotDesignated) };
        if self.is_gate(pos) {
            return Err(BuildError::Endeavor);
        }
        let site = self.sites.remove(&pos).expect("just found");
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
    pub(super) fn nearest_source(&self, flood: &Flood, item: ItemId) -> Option<(Source, Pos)> {
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
                consider(0, pos, Source::Stockpile(pos), self.stand_tile(flood, pos));
            }
        }
        for (&pos, s) in &self.ground {
            if s.item == item {
                consider(1, pos, Source::Ground(pos), self.stand_tile(flood, pos));
            }
        }
        best.map(|(_, source, stand)| (source, stand))
    }

    /// Takes up to `needed`, capped by carry capacity and what's actually there, from `source`.
    pub(super) fn take_for_build(
        &mut self,
        i: usize,
        source: Source,
        needed: u32,
        defs: &Defs,
    ) -> u32 {
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
        if self.is_gate(pos) {
            return self.run_gate_build(i, id, pos, defs);
        }

        if let Some(stack) = self.units[i].carrying {
            // Carrying a trip's worth of materials to the site.
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
                    match self.stand_building(&flood, pos, defs).and_then(|(_, s)| flood.path_to(s))
                    {
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
                let (building, size) = (site.building, site.size);
                let footprint = site.footprint(pos);
                let seats = def.seats;
                if let Some(orchard) = &def.orchard {
                    self.open_orchard(pos, orchard);
                }
                if def.farm.is_some() {
                    self.open_farm_plot(pos);
                }
                self.sites.remove(&pos);
                self.jobs.remove(idx);
                self.raise(footprint, building, def);
                self.open_study_seats(pos, seats);
                self.light_building_fire(pos, size, def);
                self.finish_job(i);
            }
        }
    }
}

#[cfg(test)]
mod tests;
