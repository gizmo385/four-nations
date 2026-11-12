//! Research item bills (`[[tech.cost_items]]`, docs/design/horizons-phase-4.md, "Shared sim
//! work, 2. Item bills"; #718).
//!
//! - **Deliveries.** While a queued tech's bill is short of an item stores hold, one
//!   `JobKind::Refuel` is kept open at the bill hall (`ResearchSettings::bill_hall`) nearest the
//!   settlement's anchor. It is the Refuel haul with the bill's lines in place of a bunker's fuel:
//!   on the same tier as a build site's deliveries, fetching at most what is still owed. An item
//!   delivered at the door leaves the world and is counted on the bill (`Research::pay_bill`).
//! - **Scrolls.** A readable item is never stockpiled, so it never comes through this haul. A
//!   scroll carried to any study hall while a bill is short of it goes on the bill instead of
//!   being read: no knowledge, and it isn't counted as read (#715; `World::run_deliver`).
//! - **Refunds.** Retargeting drops a bill (`Research::set_target`), and its items wait in
//!   `Research::refunds` until `tick_bills` drops them beside the first finished bill hall in
//!   row-major anchor order, as ordinary stacks with Haul jobs. With no hall standing they keep.

use crate::jobs::{ItemId, JobKind, Stack};

use super::work::route_from;
use super::*;

impl World {
    /// Finished bill halls' anchors, in row-major order.
    fn bill_halls(&self, defs: &Defs) -> Vec<Pos> {
        let Some(hall) = defs.research.bill_hall_id else { return Vec::new() };
        let mut out: Vec<Pos> =
            self.buildings().filter(|b| b.id == hall).map(|b| b.anchor).collect();
        out.sort_by_key(|p| (p.y, p.x));
        out
    }

    /// The bill hall that takes deliveries: the finished one nearest the settlement's anchor,
    /// ties to the first in row-major order. `None` with none standing.
    pub fn bill_hall(&self, defs: &Defs) -> Option<Pos> {
        let anchor = self.settlement_anchor();
        self.bill_halls(defs).into_iter().min_by_key(|p| p.chebyshev(anchor))
    }

    /// Items queued bills still owe that a haul can bring: not readable ones, which go to a
    /// hall on their own (`run_deliver`). Each item once, front tech first.
    fn bill_items_owed(&self, defs: &Defs) -> Vec<ItemId> {
        let mut out: Vec<ItemId> = Vec::new();
        for (_, item, _) in self.research.bills_owed(defs) {
            if !defs.item(item).is_readable() && !out.contains(&item) {
                out.push(item);
            }
        }
        out
    }

    /// How many of `item` queued bills still owe, in all.
    fn bill_owed(&self, item: ItemId, defs: &Defs) -> u32 {
        self.research.bills_owed(defs).filter(|&(_, i, _)| i == item).map(|(_, _, n)| n).sum()
    }

    /// Drops waiting refunds beside the first bill hall, and keeps one delivery job open at the
    /// delivery hall while a bill is short of something stores hold.
    pub(super) fn tick_bills(&mut self, defs: &Defs) {
        if self.research.has_refunds()
            && let Some(&hall) = self.bill_halls(defs).first()
        {
            for stack in self.research.take_refunds() {
                self.place_ground(hall, stack);
            }
        }
        let Some(hall) = self.bill_hall(defs) else { return };
        if self.bill_items_owed(defs).is_empty() {
            return;
        }
        if !self.jobs.iter().any(|j| j.kind == JobKind::Refuel { pos: hall }) {
            self.add_job(JobKind::Refuel { pos: hall });
        }
    }

    /// Whether `pos` is the delivery hall and some bill owes an item a haul could bring: the
    /// arm of `refuel_live` for a research delivery.
    pub(super) fn bill_live(&self, pos: Pos, defs: &Defs) -> bool {
        self.bill_hall(defs) == Some(pos) && !self.bill_items_owed(defs).is_empty()
    }

    /// `bill_live`, and some store holds an owed item.
    pub(super) fn bill_ready(&self, pos: Pos, defs: &Defs) -> bool {
        self.bill_live(pos, defs)
            && self.bill_items_owed(defs).into_iter().any(|item| self.any_source_for_fuel(item))
    }

    /// One tick of a unit on a research delivery to the hall anchored on `pos`: fetch up to what
    /// is owed of the nearest owed item, carry it to the hall and put it on the bill. One trip
    /// finishes the job; `tick_bills` opens another while something is still owed.
    pub(super) fn run_bill(&mut self, i: usize, id: JobId, pos: Pos, defs: &Defs) {
        let items = self.bill_items_owed(defs);
        if let Some(stack) = self.units[i].carrying.filter(|s| items.contains(&s.item)) {
            if !self.go_or_abandon(i, id, StandRule::Building(pos), defs) {
                return;
            }
            let put = self.research.pay_bill(stack.item, stack.count, defs);
            let left = stack.count - put;
            self.units[i].carrying = (left > 0).then_some(Stack { item: stack.item, count: left });
            self.units[i].route = None;
            let idx = self.job_index(id).expect("still queued");
            self.jobs.remove(idx);
            return self.finish_job(i);
        }
        if items.is_empty() || self.units[i].carrying.is_some() {
            // Paid some other way, or hands full of something else: done for now.
            let idx = self.job_index(id).expect("still queued");
            self.jobs.remove(idx);
            return self.finish_job(i);
        }
        if !items.iter().any(|&item| self.any_source_for_fuel(item)) {
            return self.abandon_job(i, id, defs);
        }
        if self.keep_walking(i, defs) {
            return;
        }
        let flood = self.flood_from(i);
        let Some((source, stand, item)) = self.nearest_source_of(&flood, &items) else {
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
        let owed = self.bill_owed(item, defs);
        let take = self.take_for_build(i, source, owed, defs);
        if take == 0 {
            return self.abandon_job(i, id, defs);
        }
        self.units[i].carrying = Some(Stack { item, count: take });
        self.units[i].route = None;
    }
}

#[cfg(test)]
mod tests;
