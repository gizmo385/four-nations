//! Stand-ins: items that pay for another item's costs one for one (docs/design/
//! horizons-phase-4.md, "New upgrade properties"; #726).
//!
//! Brick stands for stone from the start (`ItemDef::stands_for`, #371). A tech can add more
//! (`Defs::item_upgrades`: `northern_city`'s ice for stone), so a target may have several
//! stand-ins once that tech is known. Every cost-paying path (build sites, roads, the gate's
//! stages) asks `World::stand_ins` instead of the defs. Stores and the signs never do: the
//! stone sign keeps counting stone and brick only.

use crate::jobs::ItemId;

use super::*;

/// The stand-ins in force for one settlement: the static ones, plus each item upgrade whose tech
/// is known. Cheap to copy around while the world is borrowed mutably.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct StandIns {
    upgrades: Vec<(ItemId, ItemId)>,
}

impl StandIns {
    /// The item `item` pays for, if any.
    pub fn stands_for(&self, item: ItemId, defs: &Defs) -> Option<ItemId> {
        defs.stands_for(item)
            .or_else(|| self.upgrades.iter().find(|(i, _)| *i == item).map(|&(_, t)| t))
    }

    /// Every item that pays for `target`: the static one first, then upgrades in tech order.
    pub fn for_target(&self, target: ItemId, defs: &Defs) -> Vec<ItemId> {
        defs.stand_in_for(target)
            .into_iter()
            .chain(self.upgrades.iter().filter(|(_, t)| *t == target).map(|&(i, _)| i))
            .collect()
    }

    /// Whether `item` pays for `target`, itself or as a stand-in.
    pub fn pays(&self, item: ItemId, target: ItemId, defs: &Defs) -> bool {
        item == target || self.stands_for(item, defs) == Some(target)
    }
}

impl World {
    /// The stand-ins this settlement may use now.
    pub fn stand_ins(&self, defs: &Defs) -> StandIns {
        StandIns {
            upgrades: defs
                .item_upgrades
                .iter()
                .filter(|u| self.research.is_known(u.tech))
                .map(|u| (u.item, u.stands_for))
                .collect(),
        }
    }

    /// The item `item` pays for in this settlement, if any (`StandIns::stands_for`).
    pub fn stands_for(&self, item: ItemId, defs: &Defs) -> Option<ItemId> {
        self.stand_ins(defs).stands_for(item, defs)
    }
}

#[cfg(test)]
mod tests;
