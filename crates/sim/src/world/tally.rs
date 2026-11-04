//! Lifetime tallies: counts of things the settlement has done, kept for the end-game signs
//! (`docs/design/endgame.md`, "The road: ten signs"; issue #233).
//!
//! Stores can be eaten and buildings torn down, but some signs count output over the whole game:
//! ore mined, crops harvested, scrolls read. Nothing else in the world remembers those, so the
//! world keeps a running count as it happens. The counts only ever go up. A save from before they
//! existed loads with zeros (`serde(default)`): the past isn't reconstructed.
//!
//! Bending mastery is counted on the unit instead (`Unit::bends`), since a master is a person.

use std::collections::BTreeMap;

use crate::jobs::ItemId;

#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Tallies {
    /// Items that came off a worked resource, by item: every finished harvest job and every
    /// forage (`World::work_done`), counted in full at the moment the work ends, whether or not
    /// the items are later hauled, stored or eaten.
    harvested: BTreeMap<ItemId, u32>,
    /// Readable items used up at a study hall (ancient scrolls, #24), one per item.
    scrolls_read: u32,
    /// Crops harvested from farm plots (`world/farm.rs`, #198): `harvest_count` each time a Reap
    /// job finishes.
    crops_harvested: u32,
}

impl Tallies {
    /// How many of `item` have been harvested so far.
    pub fn harvested(&self, item: ItemId) -> u32 {
        self.harvested.get(&item).copied().unwrap_or(0)
    }

    /// Every item harvested at least once, with its count, in item order.
    pub fn harvested_items(&self) -> impl Iterator<Item = (ItemId, u32)> + '_ {
        self.harvested.iter().map(|(&item, &n)| (item, n))
    }

    pub fn scrolls_read(&self) -> u32 {
        self.scrolls_read
    }

    pub fn crops_harvested(&self) -> u32 {
        self.crops_harvested
    }

    pub(super) fn count_harvest(&mut self, item: ItemId, count: u32) {
        let n = self.harvested.entry(item).or_insert(0);
        *n = n.saturating_add(count);
    }

    pub(super) fn count_scrolls(&mut self, count: u32) {
        self.scrolls_read = self.scrolls_read.saturating_add(count);
    }

    /// Called for each crop a farm plot yields (#198).
    pub(super) fn count_crops(&mut self, count: u32) {
        self.crops_harvested = self.crops_harvested.saturating_add(count);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn counts_start_at_zero_and_add_up() {
        let mut t = Tallies::default();
        assert_eq!((t.harvested(ItemId(0)), t.scrolls_read(), t.crops_harvested()), (0, 0, 0));
        t.count_harvest(ItemId(3), 4);
        t.count_harvest(ItemId(3), 2);
        t.count_harvest(ItemId(1), 1);
        t.count_scrolls(2);
        t.count_crops(5);
        assert_eq!(t.harvested(ItemId(3)), 6);
        assert_eq!(t.harvested_items().collect::<Vec<_>>(), [(ItemId(1), 1), (ItemId(3), 6)]);
        assert_eq!((t.scrolls_read(), t.crops_harvested()), (2, 5));
    }
}
