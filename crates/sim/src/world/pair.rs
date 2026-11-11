//! Paired links (`[building.pair]`, docs/design/horizons-phase-2.md, "Shared sim work, 1. Paired
//! links"): the two-way case of `Building::link`, for the Wind Gate and the Balloon Dock.
//!
//! - **Linking.** `Command::SetLink` on a pair building (finished or still a site) needs a `to`
//!   that is the anchor of a building or site of the same id, within `reach` and at least
//!   `min_tiles` away (Chebyshev, anchor to anchor). It sets both ends' links and clears any
//!   partner either end had before. Taking either end down, or cancelling its site, clears the
//!   other's link (`World::unlink_partner`). A site may be linked, and hands its link to the
//!   finished building (`build.rs`).
//! - **The same-region rule.** Both ends' door tiles (`Footprint::entrance`) must lie in one
//!   `Regions` region under the settlement's move rules, or the link is refused with
//!   `LinkError::NotReachable`. A map change that later splits a pair leaves the links as they
//!   are and the status `PairStatus::CutOff`: the pair does nothing until the ground joins up
//!   again. That way links only ever change which route is cheapest, and never what is reachable.
//! - **Status.** `World::pair_status`, and `World::pair_partner` for the partner of a pair that
//!   works.

use crate::building::PairDef;
use crate::geom::Footprint;
use crate::regions::Regions;

use super::*;

/// How a pair building stands (`World::pair_status`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PairStatus {
    /// No partner is linked.
    NoPartner,
    /// A partner is linked but one end is still a build site.
    PartnerUnbuilt,
    /// Both ends stand, but their doors are no longer in one region.
    CutOff,
    /// Both ends stand and are connected: the partner's anchor and its Chebyshev distance.
    Linked { partner: Pos, distance: u32 },
}

/// One end of a pair: what it is, its rule, its footprint, and whether it is finished.
struct End {
    id: BuildingId,
    pair: PairDef,
    foot: Footprint,
    built: bool,
}

impl World {
    /// The pair building or site anchored on `pos`. The endeavor's gate is never one.
    fn pair_at(&self, pos: Pos, defs: &Defs) -> Option<End> {
        let (id, foot, built) = match self.anchored(pos) {
            Some(b) => (b.id, b.footprint(), true),
            None => {
                let site = self.sites.get(&pos).filter(|_| !self.is_gate(pos))?;
                (site.building, site.footprint(pos), false)
            }
        };
        Some(End { id, pair: defs.building(id).pair?, foot, built })
    }

    /// How far a partner of the pair building or site anchored on `pos` may be, and how near it
    /// may not (`reach`, `min_tiles`): for a picker's reach tint. `None` when no pair building
    /// is anchored there.
    pub fn pair_reach(&self, pos: Pos, defs: &Defs) -> Option<(u32, u32)> {
        self.pair_at(pos, defs).map(|e| (e.pair.reach, e.pair.min_tiles))
    }

    /// Whether the doors of `a` and `b` are in one region under the current move rules. Uses the
    /// cached `Regions` when it is current, else works a fresh one out (`ensure_regions` first to
    /// keep a read-only query cheap).
    fn doors_connected(&self, a: Footprint, b: Footprint) -> bool {
        let fresh;
        let regions = match &self.regions {
            Some(r) => r,
            None => {
                fresh = Regions::compute(&self.map, &self.rules);
                &fresh
            }
        };
        let region = regions.region(a.entrance());
        region.is_some() && region == regions.region(b.entrance())
    }

    /// Whether `to` may be the partner of the pair building or site anchored on `pos`.
    pub fn check_partner(&self, pos: Pos, to: Pos, defs: &Defs) -> Result<(), LinkError> {
        let this = self.pair_at(pos, defs).ok_or(LinkError::NotLinkable)?;
        let other = self
            .pair_at(to, defs)
            .filter(|e| e.id == this.id && to != pos)
            .ok_or(LinkError::NotPartner)?;
        let distance = pos.chebyshev(to);
        if distance > this.pair.reach as i32 {
            return Err(LinkError::TooFar);
        }
        if distance < this.pair.min_tiles as i32 {
            return Err(LinkError::TooClose);
        }
        if !self.doors_connected(this.foot, other.foot) {
            return Err(LinkError::NotReachable);
        }
        Ok(())
    }

    /// Links the pair building or site anchored on `pos` with `to`, both ways, clearing any
    /// partner either had.
    pub(super) fn set_pair(&mut self, pos: Pos, to: Pos, defs: &Defs) -> Result<(), LinkError> {
        // So the check reads the cache rather than computing a fresh labelling.
        self.ensure_regions();
        self.check_partner(pos, to, defs)?;
        for (end, other) in [(pos, to), (to, pos)] {
            if let Some(old) = self.link_at(end).filter(|&old| old != other) {
                self.unlink_partner(end, old);
            }
        }
        self.put_link(pos, Some(to));
        self.put_link(to, Some(pos));
        Ok(())
    }

    /// Sets (or clears) the link on the building or site anchored on `pos`.
    fn put_link(&mut self, pos: Pos, link: Option<Pos>) {
        match self.anchored_mut(pos) {
            Some(b) => b.link = link,
            None => {
                if let Some(site) = self.sites.get_mut(&pos) {
                    site.link = link;
                }
            }
        }
    }

    /// The end anchored on `anchor` is going (taken down, cancelled) or leaving `partner`: if
    /// `partner` still points back at it, it points at nothing.
    pub(super) fn unlink_partner(&mut self, anchor: Pos, partner: Pos) {
        if self.link_at(partner) == Some(anchor) {
            self.put_link(partner, None);
        }
    }

    /// How the pair building or site anchored on `pos` stands, or `None` if none is anchored
    /// there. A site is never working, so one that is linked reads `PartnerUnbuilt`.
    pub fn pair_status(&self, pos: Pos, defs: &Defs) -> Option<PairStatus> {
        let this = self.pair_at(pos, defs)?;
        let Some(partner) = self.link_at(pos) else { return Some(PairStatus::NoPartner) };
        let other = self
            .pair_at(partner, defs)
            .filter(|e| e.id == this.id && self.link_at(partner) == Some(pos));
        // A link that points at nothing or doesn't point back (a hand-edited save): no partner.
        let Some(other) = other else { return Some(PairStatus::NoPartner) };
        if !this.built || !other.built {
            return Some(PairStatus::PartnerUnbuilt);
        }
        if !self.doors_connected(this.foot, other.foot) {
            return Some(PairStatus::CutOff);
        }
        Some(PairStatus::Linked { partner, distance: pos.chebyshev(partner) as u32 })
    }

    /// The partner of the finished pair building anchored on `pos`, if the pair works: both ends
    /// stand and are connected (`PairStatus::Linked`).
    pub fn pair_partner(&self, pos: Pos, defs: &Defs) -> Option<Pos> {
        match self.pair_status(pos, defs)? {
            PairStatus::Linked { partner, .. } => Some(partner),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests;
