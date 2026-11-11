//! The Archive (`[building.archive]`, docs/design/horizons-phase-3.md, "`archives`", #709):
//! scribing, and the seats it adds to the study halls in its reach (`study.rs`'s `seats_for`).
//!
//! **Scribing is on** when the research queue is empty, an Archive is finished and fewer than its
//! `stock_limit` scroll copies are in stores. While it is on, whatever `Research::
//! add_toward_target` trims above `idle_knowledge_cap` goes to `Research::scribe_millis` instead
//! of being lost, and study seats stay claimable with nothing queued. `tick_scribing` turns every
//! `copy_points` x 1000 of it into one copy, dropped at the door of the first finished Archive in
//! row-major anchor order and hauled to stores like any ground stack. A copy has no `knowledge`,
//! so it is never read and the scrolls sign never counts it. With no Archive, none of this runs.

use crate::building::ArchiveDef;
use crate::geom::Footprint;
use crate::jobs::{ItemId, Stack};
use crate::research::MILLIS_PER_POINT;

use super::*;

impl World {
    /// The first finished Archive in row-major anchor order, with its def.
    fn first_archive(&self, defs: &Defs) -> Option<(Footprint, ArchiveDef)> {
        if !self.has_kind(Kind::Archive) {
            return None;
        }
        self.of_kind(Kind::Archive)
            .min_by_key(|b| (b.anchor.y, b.anchor.x))
            .and_then(|b| defs.building(b.id).archive.map(|a| (b.footprint(), a)))
    }

    /// The seats the best Archive in reach of a building `id` on `foot` adds, 0 if none is
    /// (two don't stack).
    pub(super) fn archive_bonus(&self, id: BuildingId, foot: Footprint, defs: &Defs) -> u32 {
        if !self.has_kind(Kind::Archive) {
            return 0;
        }
        self.of_kind(Kind::Archive)
            .filter_map(|b| {
                let a = defs.building(b.id).archive?;
                (a.hall == id && b.footprint().gap(foot) <= a.radius as i32).then_some(a.seats)
            })
            .max()
            .unwrap_or(0)
    }

    /// How many copies of `item` exist: in stores, plus those dropped and waiting for a haul.
    pub fn copies_held(&self, item: ItemId) -> u32 {
        let waiting: u32 = self.ground.values().filter(|s| s.item == item).map(|s| s.count).sum();
        self.stored_total(item) + waiting
    }

    /// Whether the settlement has a finished Archive.
    pub fn has_archive(&self) -> bool {
        self.has_kind(Kind::Archive)
    }

    /// Whether scribing is on: nothing queued, an Archive finished, and room for more copies.
    pub fn scribing_on(&self, defs: &Defs) -> bool {
        self.research.queue().is_empty()
            && self
                .first_archive(defs)
                .is_some_and(|(_, a)| self.copies_held(a.copy_item) < a.stock_limit)
    }

    /// Whether a study seat is worth claiming or holding: something is queued, or scribing is on
    /// (#284 left an empty queue with no use for a seat; an Archive gives it one). Not while the
    /// front tech waits only on its item bill (#718): its knowledge is banked, so scholars haul.
    pub(super) fn study_wanted(&self, defs: &Defs) -> bool {
        let waiting = self.research.waiting_on_bill(defs, self.knobs(defs).research_cost);
        (!self.research.queue().is_empty() && !waiting) || self.scribing_on(defs)
    }

    /// Turns the scribing counter into copies, once per tick after the research tick. Stops at
    /// the Archive's `stock_limit`, counting copies still waiting for a haul, and keeps what it
    /// can't spend: nothing is lost while it waits.
    pub(super) fn tick_scribing(&mut self, defs: &Defs) {
        if self.research.scribe_millis() == 0 {
            return;
        }
        let Some((foot, archive)) = self.first_archive(defs) else { return };
        let per = u64::from(archive.copy_points) * MILLIS_PER_POINT;
        let mut held = self.copies_held(archive.copy_item);
        while self.research.scribe_millis() >= per && held < archive.stock_limit {
            self.place_ground(foot.entrance(), Stack { item: archive.copy_item, count: 1 });
            self.research.spend_scribe(per);
            self.tallies.count_copy();
            held += 1;
        }
    }

    /// Scroll copies made so far (`scrolls_copied`), for readouts and the harness.
    pub fn scrolls_copied(&self) -> u32 {
        self.tallies.scrolls_copied()
    }

    /// `Command::ClearResearch`: empties the queue so the scholars scribe. Refused without a
    /// finished Archive, since scribing is its only use. Knowledge banked above the idle cap moves
    /// to the scribing counter.
    pub(super) fn clear_research(&mut self, defs: &Defs) -> Result<(), ResearchError> {
        if !self.has_kind(Kind::Archive) {
            return Err(ResearchError::NeedsArchive);
        }
        self.research.clear_target(defs);
        Ok(())
    }
}

#[cfg(test)]
mod tests;
