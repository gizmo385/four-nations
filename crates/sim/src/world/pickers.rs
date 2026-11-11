//! Pickers: a building that harvests ripe trees in reach with no worker (`[building.pickers]`,
//! the Lemur Roost, docs/design/horizons-phase-1.md, "`lemur_keeping`").
//!
//! Every `pick_ticks`, a roost picks the nearest ripe tree of a listed resource within `radius`
//! of its footprint (Chebyshev), ties broken by tile index. It skips a tree a settler has
//! already claimed. The pick is a normal harvest with no worker (`World::work_done`): the
//! harvest's count drops, strain is added (tiring land still applies), the harvest tally counts
//! it, and an unclaimed harvest designation on the tree is cleared (a standing order stays, and
//! re-queues when it ripens). Every `eat_every`-th item picked is eaten by the lemurs.
//!
//! The rest goes straight into the nearest stockpile tiles in reach with room for it. Whatever
//! doesn't fit drops as a stack beside the roost and is hauled like any yield. With nothing to
//! pick, the roost looks again a second later.

use crate::building::PickersDef;
use crate::geom::Footprint;
use crate::jobs::{JobKind, Stack};

use super::buildings::Kind;
use super::*;

/// What a roost remembers: when it picks next, and what it has picked and eaten.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Pickers {
    /// Ticks until it may pick again.
    pub next_in: u32,
    /// Items picked, eaten ones included.
    pub picked: u32,
    /// Items eaten.
    pub eaten: u32,
}

impl Pickers {
    /// A just-finished roost: it waits one full interval before its first pick.
    pub(super) fn new(def: &PickersDef) -> Self {
        Self { next_in: def.pick_ticks, picked: 0, eaten: 0 }
    }
}

impl World {
    /// The pickers' counts on the building covering `pos`, if it has pickers: the cursor
    /// panel's `Picked: 36  Eaten: 9`.
    pub fn pickers_at(&self, pos: Pos) -> Option<Pickers> {
        self.building_record(pos)?.pickers
    }

    /// Counts every roost down and lets each one that is due pick.
    pub(super) fn tick_pickers(&mut self, defs: &Defs) {
        if !self.has_kind(Kind::Pickers) {
            return;
        }
        let roosts: Vec<Pos> = self.of_kind(Kind::Pickers).map(|b| b.anchor).collect();
        for anchor in roosts {
            let Some(p) = self.anchored_mut(anchor).and_then(|b| b.pickers.as_mut()) else {
                continue;
            };
            p.next_in = p.next_in.saturating_sub(1);
            if p.next_in == 0 {
                self.pick(anchor, defs);
            }
        }
    }

    /// The roost anchored on `anchor` picks once, if it has a tree to pick.
    fn pick(&mut self, anchor: Pos, defs: &Defs) {
        let Some(b) = self.anchored(anchor) else { return };
        let def = defs.building(b.id).pickers.clone().expect("a pickers building");
        let foot = b.footprint();
        let r = def.radius as i32;
        let claimed: BTreeSet<Pos> = self
            .jobs
            .iter()
            .filter(|j| j.claimed_by.is_some())
            .filter_map(|j| match j.kind {
                JobKind::Harvest { pos } | JobKind::Clear { pos } => Some(pos),
                _ => None,
            })
            .collect();
        let width = self.map.width();
        let mut best: Option<((i32, i32), Pos)> = None;
        for dy in -r..i32::from(foot.h) + r {
            for dx in -r..i32::from(foot.w) + r {
                let p = Pos::new(anchor.x + dx, anchor.y + dy);
                let Some(tile) = self.map.get(p) else { continue };
                let Some(res) = tile.resource else { continue };
                if !def.resources.contains(&res) || claimed.contains(&p) {
                    continue;
                }
                if self.ripe_harvest(p, defs).is_none() {
                    continue;
                }
                let key = (foot.distance(p), p.y * width + p.x);
                if best.is_none_or(|(k, _)| key < k) {
                    best = Some((key, p));
                }
            }
        }
        let Some((_, tree)) = best else {
            // Nothing ripe: look again in a second.
            if let Some(p) = self.anchored_mut(anchor).and_then(|b| b.pickers.as_mut()) {
                p.next_in = TICKS_PER_SECOND;
            }
            return;
        };
        let h = defs.harvest(self.map[tree].resource.expect("listed above")).expect("ripe");
        let count = self.work_done(tree, h, defs);
        // An unclaimed designation on the tree is done with.
        self.jobs.retain(|j| j.kind != JobKind::Harvest { pos: tree } || j.claimed_by.is_some());
        let p = self.anchored_mut(anchor).and_then(|b| b.pickers.as_mut()).expect("a roost");
        let before = p.picked;
        p.picked += count;
        let eaten = p.picked / def.eat_every - before / def.eat_every;
        p.eaten += eaten;
        p.next_in = def.pick_ticks;
        let kept = count - eaten;
        if kept > 0 {
            self.store_near(anchor, foot, r, Stack { item: h.item, count: kept }, defs);
        }
    }

    /// Puts `stack` into the nearest stockpile tiles within `r` of `foot` that have room
    /// (nearest first, ties by tile index), and drops what doesn't fit beside `anchor`.
    fn store_near(&mut self, anchor: Pos, foot: Footprint, r: i32, stack: Stack, defs: &Defs) {
        let width = self.map.width();
        let mut zones: Vec<((i32, i32), Pos)> = self
            .zones
            .iter()
            .filter(|&&z| foot.distance(z) <= r)
            .map(|&z| ((foot.distance(z), z.y * width + z.x), z))
            .collect();
        zones.sort();
        let mut left = stack.count;
        for (_, z) in zones {
            if left == 0 {
                break;
            }
            let put = self.room_at(z, stack.item, defs).min(left);
            if put == 0 {
                continue;
            }
            self.stored.entry(z).or_insert(Stack { item: stack.item, count: 0 }).count += put;
            left -= put;
        }
        if left > 0 {
            self.place_ground(anchor, Stack { item: stack.item, count: left });
        }
    }
}

#[cfg(test)]
mod tests;
