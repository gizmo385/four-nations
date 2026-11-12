//! Spirit water (`spirit_water`, #739, docs/design/horizons-phase-5.md, "`spirit_water`: the
//! vial"). A Healing Hut whose `[building.healing]` names a `vial_item` drops `vials_per_year`
//! of them beside the first finished hut on the first tick of each spring, up to `vial_stock`
//! held. When a settler would die and a vial is in a stockpile, the vial is used up instead:
//! health and each need that was killing them go to `revive_to`, and the alert log says
//! "Spirit water saved Kya".

use super::*;
use crate::building::HealingDef;
use crate::jobs::ItemId;

impl World {
    /// How many vials a finished building `def` fills each spring: its `vials_per_year`, or the
    /// `vials_per_year` upgrade's once known. 0 for a building with no `vial_item`.
    pub fn vials_per_year(&self, def: &BuildingDef) -> u32 {
        let Some(h) = def.healing.as_ref().filter(|h| h.vial_item.is_some()) else { return 0 };
        match def.vial_upgrade {
            Some((t, n)) if self.research.is_known(t) => n,
            _ => h.vials_per_year,
        }
    }

    /// The first tick of spring: the first finished building, in anchor order, that fills
    /// vials drops its year's worth beside its door, up to `vial_stock` held.
    pub(super) fn tick_vials(&mut self, defs: &Defs) {
        let mut huts: Vec<(Pos, BuildingId)> = self
            .buildings()
            .filter(|b| self.vials_per_year(defs.building(b.id)) > 0)
            .map(|b| (b.anchor, b.id))
            .collect();
        huts.sort_by_key(|&(p, _)| (p.y, p.x));
        let Some(&(anchor, id)) = huts.first() else { return };
        let def = defs.building(id);
        let h = def.healing.as_ref().expect("filtered");
        let item = h.vial_item.expect("filtered");
        let held = self.vials_held(item);
        let n = self.vials_per_year(def).min(h.vial_stock.saturating_sub(held));
        let Some(door) = self.footprint_at(anchor).map(|f| f.entrance()) else { return };
        if n > 0 {
            self.place_ground(door, Stack { item, count: n });
        }
    }

    /// Vials of `item` held: in stockpiles, plus any on the ground waiting for a haul.
    pub fn vials_held(&self, item: ItemId) -> u32 {
        let waiting: u32 = self.ground.values().filter(|s| s.item == item).map(|s| s.count).sum();
        self.stored_total(item) + waiting
    }

    /// Saves `u`, who would have died, with a stored vial if there is one (the lowest `Pos`
    /// stack first): health, and each need at 0, go to the vial's `revive_to`. Logs
    /// `HarmEventKind::Revived`. `false`, changing nothing, with no vial in a stockpile.
    pub(super) fn use_vial(&mut self, u: &mut Unit, defs: &Defs) -> bool {
        let vials: Vec<(ItemId, &HealingDef)> = defs
            .buildings
            .iter()
            .filter_map(|b| b.healing.as_ref())
            .filter_map(|h| h.vial_item.map(|i| (i, h)))
            .collect();
        let found = self
            .stored
            .iter()
            .find_map(|(&p, s)| vials.iter().find(|(i, _)| *i == s.item).map(|&(_, h)| (p, h)));
        let Some((pos, h)) = found else { return false };
        let mut stack = self.stored.remove(&pos).expect("found");
        stack.count -= 1;
        if stack.count > 0 {
            self.stored.insert(pos, stack);
        }
        let to = h.revive_to;
        let n = &mut u.needs;
        n.health = to;
        for need in [&mut n.hunger, &mut n.thirst, &mut n.energy] {
            if *need <= 0.0 {
                *need = to;
            }
        }
        self.log_event(HarmEvent {
            tick: self.tick,
            unit: u.id,
            pos: u.pos,
            kind: HarmEventKind::Revived,
        });
        true
    }
}

#[cfg(test)]
mod tests;
