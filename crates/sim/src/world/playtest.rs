//! Playtest starts for the Republic era (docs/design/factory-endgame.md, #1025). The era opens
//! after the Harmony Gate, hours into a game, so a scenario's `republic` prelude plays the
//! economy opening, stands the gate (`World::complete_endeavor`) and then calls
//! [`World::start_republic`]: the techs and stores the era's newest phase needs, so the owner can
//! try it at turn 0. A dev hook like `complete_endeavor`: no `Command` reaches it, and only the
//! scenario prelude (`fn_tui::surface::prelude`) and the balance par (`balance::par`) call it.
//!
//! The lists below are scaffolding, not balance: each factory-endgame phase adds what its
//! feature needs, so the newest one is always a command away.

use crate::defs::Defs;
use crate::geom::Pos;
use crate::jobs::Stack;

use super::World;

/// Techs a Republic start knows: the Steelworks (#1023), so steel can be made from the start,
/// and every nation's goods techs (#1027), so each good can be made by hand.
pub const REPUBLIC_TECHS: &[&str] =
    &["steelworks", "lanterncraft", "smokehouses", "joinery", "tea_works"];

/// Items a Republic start adds to stores, by id and count: steel to spend, metal and wood for the
/// Steelworks to make more, fittings, fish and herbs for the goods (#1027), and some of each
/// nation's Citizen good, so residents start climbing at once (#1026).
pub const REPUBLIC_ITEMS: &[(&str, u32)] = &[
    ("steel", 20),
    ("metal", 30),
    ("wood", 80),
    ("tool", 10),
    ("fittings", 10),
    ("fish", 20),
    ("herbs", 20),
    ("lantern", 10),
    ("smoked_fish", 10),
    ("furniture", 10),
    ("tea", 10),
];

impl World {
    /// Stands the gate if it isn't already, learns [`REPUBLIC_TECHS`] and puts [`REPUBLIC_ITEMS`]
    /// in stores (module doc). What doesn't fit on a stockpile tile is dropped beside the first
    /// one for the haulers. An id the data doesn't have is skipped.
    pub fn start_republic(&mut self, defs: &Defs) {
        self.complete_endeavor(defs);
        for id in REPUBLIC_TECHS {
            if let Some(tech) = defs.tech_id(id) {
                self.research.grant(tech);
            }
        }
        for &(id, count) in REPUBLIC_ITEMS {
            if let Some(item) = defs.item_id(id) {
                self.add_to_stores(Stack { item, count }, defs);
            }
        }
    }

    /// Puts `stack` on stockpile tiles with room, in tile order. What's left goes on new
    /// stockpile tiles, the free open ground nearest the first stockpile tile (or the map's
    /// centre with none), so a start whose stores are full still holds every item (#1028:
    /// otherwise the goods lay on the ground beside full stores, out of residents' reach).
    pub(super) fn add_to_stores(&mut self, stack: Stack, defs: &Defs) {
        let zones: Vec<_> = self.zones.iter().copied().collect();
        let mut left = stack.count;
        for z in &zones {
            if left == 0 {
                break;
            }
            let put = self.room_at(*z, stack.item, defs).min(left);
            if put == 0 {
                continue;
            }
            self.stored.entry(*z).or_insert(Stack { item: stack.item, count: 0 }).count += put;
            left -= put;
        }
        if left == 0 {
            return;
        }
        let near = zones
            .first()
            .copied()
            .unwrap_or_else(|| Pos::new(self.map.width() / 2, self.map.height() / 2));
        let mut open: Vec<Pos> = self
            .map
            .positions()
            .filter(|&p| {
                self.map.get(p).is_some_and(|t| t.terrain.is_walkable() && t.resource.is_none())
                    && !self.zones.contains(&p)
                    && !self.ground.contains_key(&p)
                    && self.building_at(p).is_none()
            })
            .collect();
        open.sort_by_key(|p| ((p.x - near.x).pow(2) + (p.y - near.y).pow(2), *p));
        let per_tile = defs.jobs.stockpile_stack.max(1);
        for pos in open {
            if left == 0 {
                return;
            }
            let put = left.min(per_tile);
            self.zones.insert(pos);
            self.stored.insert(pos, Stack { item: stack.item, count: put });
            left -= put;
        }
        if left > 0 {
            self.place_ground(near, Stack { item: stack.item, count: left });
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::defs::Defs;
    use crate::mapgen::MapGenConfig;
    use crate::nation::Nation;
    use crate::world::World;

    #[test]
    fn a_republic_start_is_united_knows_the_steelworks_and_holds_steel() {
        let defs = Defs::bundled();
        let cfg = MapGenConfig { width: 48, height: 32, seed: 7, ..Default::default() };
        let mut w = World::new(&cfg, Nation::Fire, 6, &defs);
        w.start_republic(&defs);
        assert!(w.united());
        assert!(w.research().is_known(defs.tech_id("steelworks").unwrap()));
        let steel = defs.item_id("steel").unwrap();
        assert_eq!(w.stored_total(steel), 20);
    }

    #[test]
    fn a_full_store_grows_new_stockpile_tiles_for_the_rest() {
        let defs = Defs::bundled();
        let cfg = MapGenConfig { width: 48, height: 32, seed: 7, ..Default::default() };
        let mut w = World::new(&cfg, Nation::Fire, 6, &defs);
        let zones = w.zones.len();
        let lantern = defs.item_id("lantern").unwrap();
        w.add_to_stores(crate::jobs::Stack { item: lantern, count: 500 }, &defs);
        assert_eq!(w.stored_total(lantern), 500);
        assert!(w.zones.len() > zones);
    }

    #[test]
    fn every_listed_tech_and_item_is_in_the_bundled_data() {
        let defs = Defs::bundled();
        for id in super::REPUBLIC_TECHS {
            assert!(defs.tech_id(id).is_some(), "{id}");
        }
        for (id, _) in super::REPUBLIC_ITEMS {
            assert!(defs.item_id(id).is_some(), "{id}");
        }
    }
}
