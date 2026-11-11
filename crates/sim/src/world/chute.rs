//! Chutes (`[building.chute]`, docs/design/horizons-phase-1.md, "`omashu_chutes`").
//!
//! - **The link.** A chute's record points at its foot (`Building::link`), set by
//!   `Command::SetLink`. The foot must be on the map, on ground a unit can stand on with no
//!   impassable building, within `reach` of the head (Chebyshev) and at least `drop` lower in
//!   elevation (`check_foot`). The rule is checked again at every use, so a foot that a wall later
//!   covers stops the chute: with no valid foot it does nothing.
//! - **The haul rule.** A unit taking a stack to stores (`Task::Deliver`) compares walking
//!   straight to the nearest stockpile with room against walking to a chute's head plus the
//!   foot's own walk to stores (`World::chute_for`). It takes the chute when that is cheaper. The
//!   foot's walks to stores are cached per chute until the map (`map_rev`) or the stockpile tiles
//!   change, so the comparison costs one flood per chute per change.
//! - **One way.** At the head the stack drops in and lands at the foot on the same tick, spilling
//!   to a free tile the way a yield does, where it is an ordinary Haul job (`run_chute`). Build
//!   sites, refuels, meals and scrolls never use a chute: only `run_deliver` asks.

use crate::flood::Flood;
use crate::geom::Footprint;
use crate::jobs::{ItemId, Stack};

use super::buildings::Kind;
use super::work::Go;
use super::*;

/// Why a chute's foot was refused (`Command::SetLink`). Fragments, like every refusal.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LinkError {
    /// No chute's or pair building's anchor is on that tile.
    NotLinkable,
    /// The foot is off the map.
    OffMap,
    /// The foot is ground nobody can stand on: water, rock, a resource, an impassable building.
    NotOpen,
    /// The foot (or partner) is farther than the building's `reach`.
    TooFar,
    /// The foot isn't at least the chute's `drop` lower than the head.
    NotDownhill,
    /// A pair's `to` isn't the anchor of another building or site of the same id.
    NotPartner,
    /// The partner is nearer than the pair's `min_tiles`.
    TooClose,
    /// The two doors aren't in one region: not reachable on foot.
    NotReachable,
}

impl std::fmt::Display for LinkError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::NotLinkable => "nothing to link",
            Self::OffMap => "off the map",
            Self::NotOpen => "foot needs open ground",
            Self::TooFar => "out of reach",
            Self::NotDownhill => "foot not far enough downhill",
            Self::NotPartner => "not a partner",
            Self::TooClose => "too close",
            Self::NotReachable => "not reachable on foot",
        })
    }
}

impl std::error::Error for LinkError {}

/// A chute's cached walks from its foot to every stockpile tile it reaches, cheapest first, and
/// what they were worked out for (`World::foot_walks`).
#[derive(Debug, Clone)]
pub(super) struct ChuteWalks {
    rev: u64,
    foot: Pos,
    zones: Vec<Pos>,
    walks: Vec<(u32, Pos)>,
}

impl World {
    /// The chute anchored on `head`, finished or still a site: its def and footprint.
    fn chute_at(&self, head: Pos, defs: &Defs) -> Option<(crate::building::ChuteDef, Footprint)> {
        let (id, foot) = match self.anchored(head) {
            Some(b) => (b.id, b.footprint()),
            None => {
                let site = self.sites.get(&head).filter(|_| !self.is_gate(head))?;
                (site.building, site.footprint(head))
            }
        };
        Some((defs.building(id).chute?, foot))
    }

    /// How far the foot of the chute anchored on `head` (finished or still a site) may be, or
    /// `None` when no chute is anchored there: for the foot picker's reach tint.
    pub fn chute_reach(&self, head: Pos, defs: &Defs) -> Option<u32> {
        self.chute_at(head, defs).map(|(chute, _)| chute.reach)
    }

    /// Whether `to` may be the foot of the chute anchored on `head`, finished or still a site.
    pub fn check_foot(&self, head: Pos, to: Pos, defs: &Defs) -> Result<(), LinkError> {
        let (chute, footprint) = self.chute_at(head, defs).ok_or(LinkError::NotLinkable)?;
        let foot = self.map.get(to).ok_or(LinkError::OffMap)?;
        if footprint.contains(to) || !self.rules.passable(foot) {
            return Err(LinkError::NotOpen);
        }
        if footprint.distance(to) > chute.reach as i32 {
            return Err(LinkError::TooFar);
        }
        let top = self.map.get(head).expect("the chute is on the map").elevation;
        if i32::from(top) - i32::from(foot.elevation) < i32::from(chute.drop) {
            return Err(LinkError::NotDownhill);
        }
        Ok(())
    }

    /// Points the chute anchored on `pos` at the foot `to`. A site keeps it until it is raised.
    pub(super) fn set_link(&mut self, pos: Pos, to: Pos, defs: &Defs) -> Result<(), LinkError> {
        // A pair building (or site) takes a partner instead (`pair.rs`).
        if self.pair_reach(pos, defs).is_some() {
            return self.set_pair(pos, to, defs);
        }
        self.check_foot(pos, to, defs)?;
        match self.anchored_mut(pos) {
            Some(b) => b.link = Some(to),
            None => self.sites.get_mut(&pos).expect("checked above").link = Some(to),
        }
        // A new foot is a new walk to stores.
        self.chute_walks.remove(&pos);
        Ok(())
    }

    /// Where the chute or pair building (or site) anchored on `head` points, valid or not: for drawing its
    /// link line.
    pub fn link_at(&self, head: Pos) -> Option<Pos> {
        match self.anchored(head) {
            Some(b) => b.link,
            None => self.sites.get(&head)?.link,
        }
    }

    /// The foot of the chute anchored on `head`, if it has one and it is still valid.
    pub fn chute_foot(&self, head: Pos, defs: &Defs) -> Option<Pos> {
        let to = self.anchored(head)?.link?;
        self.check_foot(head, to, defs).is_ok().then_some(to)
    }

    /// The walk from a chute's foot to stores, cached until the map or the stockpile tiles
    /// change: every stockpile tile it reaches, cheapest first. A Balloon Dock's door is the
    /// foot of the same walk (`balloon.rs`), cached under the dock's anchor.
    pub(super) fn foot_walks(&mut self, head: Pos, foot: Pos, i: usize) -> &[(u32, Pos)] {
        let fresh = self.chute_walks.get(&head).is_some_and(|c| {
            c.rev == self.map_rev && c.foot == foot && c.zones.iter().eq(self.zones.iter())
        });
        if !fresh {
            let rules = self.rules_for(&self.units[i]);
            let flood = Flood::over(self.step_grids.get(&self.map, self.map_rev, rules), foot);
            let mut walks: Vec<(u32, Pos)> =
                self.zones.iter().filter_map(|&z| flood.cost(z).map(|c| (c, z))).collect();
            walks.sort_unstable();
            let zones = self.zones.iter().copied().collect();
            self.chute_walks.insert(head, ChuteWalks { rev: self.map_rev, foot, zones, walks });
        }
        &self.chute_walks[&head].walks
    }

    /// The chute unit `i` should take `item` down instead of walking `straight` (its cost to
    /// the nearest stockpile with room, if any), and the path to stand at its head: the cheapest
    /// head walk plus foot-to-stores walk that beats `straight`. Ties go to walking.
    pub(super) fn chute_for(
        &mut self,
        i: usize,
        flood: &Flood,
        item: ItemId,
        straight: Option<u32>,
        defs: &Defs,
    ) -> Option<(Pos, Vec<Pos>)> {
        if !self.has_kind(Kind::Chute) || Self::is_readable(item, defs) {
            return None;
        }
        let heads: Vec<Pos> = self.of_kind(Kind::Chute).map(|b| b.anchor).collect();
        let mut best: Option<(u32, Pos, Pos)> = None;
        for head in heads {
            let Some(foot) = self.chute_foot(head, defs) else { continue };
            let Some((walk, stand)) = self.stand_building(flood, head, defs) else { continue };
            if straight.is_some_and(|s| walk >= s) {
                continue;
            }
            let walks = self.foot_walks(head, foot, i).to_vec();
            let down = walks.into_iter().find(|&(_, z)| self.room_at(z, item, defs) > 0);
            let Some((down, _)) = down else { continue };
            let total = walk + down;
            if straight.is_none_or(|s| total < s) && best.is_none_or(|(c, _, _)| total < c) {
                best = Some((total, head, stand));
            }
        }
        let (_, head, stand) = best?;
        Some((head, flood.path_to(stand)?))
    }

    /// One tick of carrying a stack to the chute anchored on `head` (`Task::Chute`). At the head
    /// it drops in and lands at the foot. If the chute is gone, its foot no longer valid, or its
    /// head out of reach, the unit carries the stack to stores on foot instead.
    pub(super) fn run_chute(&mut self, i: usize, head: Pos, defs: &Defs) {
        let Some(stack) = self.units[i].carrying else { return self.finish_job(i) };
        let Some(foot) = self.chute_foot(head, defs) else { return self.walk_instead(i) };
        match self.go_to(i, StandRule::Building(head), defs) {
            Go::Walking => {}
            Go::Unreachable => self.walk_instead(i),
            Go::Arrived => {
                self.units[i].carrying = None;
                self.units[i].route = None;
                self.drop_down_chute(head, foot, stack);
                self.finish_job(i);
            }
        }
    }

    /// The stack lands at the foot, spilling to a free tile the way a yield does, and waits there
    /// for an ordinary haul. The chute counts it.
    fn drop_down_chute(&mut self, head: Pos, foot: Pos, stack: Stack) {
        if let Some(b) = self.anchored_mut(head) {
            match b.sent.iter_mut().find(|s| s.item == stack.item) {
                Some(s) => s.count += stack.count,
                None => b.sent.push(stack),
            }
        }
        self.place_ground(foot, stack);
    }

    /// What the chute anchored on `head` has sent down so far, one stack per item.
    pub fn chute_sent(&self, head: Pos) -> &[Stack] {
        self.anchored(head).map_or(&[], |b| b.sent.as_slice())
    }

    /// Gives up on a chute and walks the stack to stores.
    pub(super) fn walk_instead(&mut self, i: usize) {
        let u = &mut self.units[i];
        u.job_task = Some(Task::Deliver);
        u.route = None;
    }
}

#[cfg(test)]
mod tests;
