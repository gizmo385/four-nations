//! Units inside buildings: capacity, entering and leaving through the door
//! (`docs/design/buildings.md`, "Units inside buildings", #226).
//!
//! A unit that is inside (`Unit::inside`, the building's anchor) stays on the map at its
//! `pos` -- the entrance tile (`Footprint::entrance`) -- so everything that floods or paths from
//! a unit keeps working unchanged. Only code that asks "who is standing on the map here" has to
//! skip it: `World::units_at` is that question, and the idle wander, bending and occupancy code
//! that used to scan `self.units` by `pos` alone now goes through it (or checks `inside`
//! directly). This issue adds the state, the door and these primitives; #227 makes the jobs
//! (study first) actually call `enter_building`/`leave_building`.

use crate::geom::Footprint;

use super::*;

impl World {
    /// Units standing on `pos` that the map actually shows there: not one that is `inside` a
    /// building, even though its `pos` still names the entrance tile it went in by. The frontends
    /// use this (instead of scanning `units` directly) for "is anyone standing here" -- hover,
    /// cursor selection and the like.
    pub fn units_at(&self, pos: Pos) -> impl Iterator<Item = &Unit> + '_ {
        self.units.iter().filter(move |u| u.pos == pos && u.inside.is_none())
    }

    /// The units inside the building anchored on `anchor`, in id order (`self.units` stays sorted
    /// by id).
    pub fn occupants(&self, anchor: Pos) -> impl Iterator<Item = UnitId> + '_ {
        self.units.iter().filter(move |u| u.inside == Some(anchor)).map(|u| u.id)
    }

    /// Whether the building anchored on `anchor` has room for one more, per
    /// `BuildingDef::capacity`: it exists, its capacity is above 0, and fewer than that many are
    /// in it already.
    fn has_room(&self, anchor: Pos, defs: &Defs) -> bool {
        let Some(id) = self.building_at(anchor) else { return false };
        let capacity = defs.building(id).capacity;
        capacity > 0 && (self.occupants(anchor).count() as u32) < capacity
    }

    /// Steps unit `i` inside the building anchored on `anchor`: it must be standing on the door
    /// (`Footprint::entrance`) and not already inside, and the building must have room. True if
    /// it went in; false (and nothing changed) otherwise, so a caller can keep waiting outside.
    /// No job calls this yet (#227 is the first), so only tests do for now.
    #[allow(dead_code, reason = "used by #227's jobs; tests exercise it meanwhile")]
    pub(super) fn enter_building(&mut self, i: usize, anchor: Pos, defs: &Defs) -> bool {
        if self.units[i].inside.is_some() {
            return false;
        }
        let Some(foot) = self.footprint_at(anchor) else { return false };
        if self.units[i].pos != foot.entrance() || !self.has_room(anchor, defs) {
            return false;
        }
        self.units[i].inside = Some(anchor);
        self.units[i].route = None;
        self.units[i].move_progress = 0.0;
        true
    }

    /// Steps unit `i` back out, a no-op if it isn't inside. It comes out on the door if that is
    /// still walkable, else the nearest walkable ring tile (Chebyshev distance to the door, ties
    /// on `Pos`); `pos` doesn't otherwise move, since it was the door all along.
    pub(super) fn leave_building(&mut self, i: usize) {
        let Some(anchor) = self.units[i].inside.take() else { return };
        let Some(foot) = self.footprint_at(anchor) else { return };
        let entrance = foot.entrance();
        if !self.map.get(entrance).is_some_and(|t| self.rules.passable(t))
            && let Some(out) = self.nearest_walkable_ring(foot)
        {
            self.units[i].pos = out;
        }
    }

    /// The nearest tile of `foot`'s ring that units can walk onto, ties on `Pos`, distance
    /// measured from the door (so leaving by the next best spot is as close to it as possible).
    fn nearest_walkable_ring(&self, foot: Footprint) -> Option<Pos> {
        let entrance = foot.entrance();
        foot.ring()
            .filter(|&p| self.map.get(p).is_some_and(|t| self.rules.passable(t)))
            .map(|p| (entrance.chebyshev(p), p))
            .min()
            .map(|(_, p)| p)
    }
}

#[cfg(test)]
mod tests {
    use crate::building::BuildingId;
    use crate::defs::{BiomeId, Terrain};
    use crate::geom::Grid;
    use crate::mapgen::Tile;

    use super::*;

    /// A flat land map, `w` by `h`, so tests can place a building and units freely.
    fn field(w: i32, h: i32) -> TileMap {
        Grid::from_fn(w, h, |_| Tile {
            elevation: 100,
            temperature: 128,
            moisture: 128,
            terrain: Terrain::Land,
            biome: BiomeId(0),
            resource: None,
            building: None,
        })
    }

    /// A world with one finished 2x2 hall of `capacity`, anchored on `anchor`, and `units` at
    /// the positions given (the door, usually).
    fn world_with_hall(anchor: Pos, capacity: u32, units: &[Pos]) -> (World, Defs, BuildingId) {
        let mut defs = Defs::bundled();
        let hall = defs.building_id("bed").unwrap();
        {
            let def = &mut defs.buildings[hall.0 as usize];
            def.footprint = (2, 2);
            def.bed = None;
            def.passable = false;
            def.capacity = capacity;
        }
        let mut w = World::with_map(1, field(12, 12), Nation::Earth, units, &defs);
        let foot = Footprint::new(anchor, 2, 2);
        w.raise(foot, hall, defs.building(hall));
        (w, defs, hall)
    }

    #[test]
    fn a_unit_on_the_door_enters_if_there_is_room() {
        let anchor = at(4, 4);
        let door = Footprint::new(anchor, 2, 2).entrance();
        let (mut w, defs, _) = world_with_hall(anchor, 1, &[door]);
        assert!(w.units[0].inside().is_none());
        assert!(w.enter_building(0, anchor, &defs));
        assert_eq!(w.units[0].inside(), Some(anchor));
        assert_eq!(w.units[0].pos, door, "pos stays on the door");
        assert_eq!(w.occupants(anchor).collect::<Vec<_>>(), [w.units[0].id]);
        // Visible-unit queries no longer see it there.
        assert!(w.units_at(door).next().is_none());
    }

    #[test]
    fn a_unit_off_the_door_or_without_room_does_not_enter() {
        let anchor = at(4, 4);
        let door = Footprint::new(anchor, 2, 2).entrance();
        let elsewhere = at(0, 0);
        let (mut w, defs, _) = world_with_hall(anchor, 1, &[elsewhere]);
        assert!(!w.enter_building(0, anchor, &defs), "not standing on the door");
        assert!(w.units[0].inside().is_none());

        let (mut w, defs, _) = world_with_hall(anchor, 0, &[door]);
        assert!(!w.enter_building(0, anchor, &defs), "no capacity");
        assert!(w.units[0].inside().is_none());
    }

    #[test]
    fn capacity_fills_in_unit_id_order_and_a_full_building_turns_the_rest_away() {
        let anchor = at(4, 4);
        let door = Footprint::new(anchor, 2, 2).entrance();
        let (mut w, defs, _) = world_with_hall(anchor, 2, &[door, door, door]);
        let ids: Vec<_> = w.units.iter().map(|u| u.id).collect();
        let entered: Vec<_> =
            (0..w.units.len()).map(|i| w.enter_building(i, anchor, &defs)).collect();
        assert_eq!(entered, [true, true, false], "the first two by id claim the two spots");
        assert_eq!(w.occupants(anchor).collect::<Vec<_>>(), [ids[0], ids[1]]);
        assert!(w.units_at(door).map(|u| u.id).eq([ids[2]]), "the third is still visible");
    }

    #[test]
    fn leaving_clears_inside_and_stays_on_the_door_when_it_is_still_walkable() {
        let anchor = at(4, 4);
        let door = Footprint::new(anchor, 2, 2).entrance();
        let (mut w, defs, _) = world_with_hall(anchor, 1, &[door]);
        assert!(w.enter_building(0, anchor, &defs));
        w.leave_building(0);
        assert!(w.units[0].inside().is_none());
        assert_eq!(w.units[0].pos, door);
        assert!(w.units_at(door).next().is_some(), "visible again");
    }

    #[test]
    fn a_blocked_door_sends_a_leaver_to_the_nearest_walkable_ring_tile() {
        let anchor = at(4, 4);
        let door = Footprint::new(anchor, 2, 2).entrance();
        let (mut w, defs, _) = world_with_hall(anchor, 1, &[door]);
        assert!(w.enter_building(0, anchor, &defs));
        // Something grows on the door while the unit is hidden inside (harmless: `can_sprout`
        // and `can_plant` skip units that are inside).
        w.map[door].resource = defs.resource_id("oak_tree");
        w.leave_building(0);
        assert!(w.units[0].inside().is_none());
        assert_ne!(w.units[0].pos, door, "the door is blocked");
        assert_eq!(Footprint::new(anchor, 2, 2).distance(w.units[0].pos), 1, "still on the ring");
        assert!(w.rules.passable(&w.map[w.units[0].pos]));
    }

    #[test]
    fn leaving_a_unit_that_is_not_inside_does_nothing() {
        let anchor = at(4, 4);
        let door = Footprint::new(anchor, 2, 2).entrance();
        let (mut w, _, _) = world_with_hall(anchor, 1, &[door]);
        let before = w.units[0].clone();
        w.leave_building(0);
        assert_eq!(w.units[0], before);
    }

    #[test]
    fn a_unit_that_dies_inside_is_no_longer_an_occupant() {
        let anchor = at(4, 4);
        let door = Footprint::new(anchor, 2, 2).entrance();
        let (mut w, defs, _) = world_with_hall(anchor, 1, &[door]);
        assert!(w.enter_building(0, anchor, &defs));
        w.units[0].needs.hunger = 0.0;
        w.units[0].needs.health = 0.01;
        w.tick(&defs);
        assert!(w.units.is_empty(), "it died");
        assert_eq!(w.deaths.len(), 1);
        assert_eq!(w.occupants(anchor).next(), None);
    }

    #[test]
    fn two_runs_of_entering_and_leaving_match_and_so_does_a_save_and_load_mid_way() {
        let anchor = at(4, 4);
        let door = Footprint::new(anchor, 2, 2).entrance();
        let run = || {
            let (mut w, defs, _) = world_with_hall(anchor, 1, &[door]);
            for t in 0..40u64 {
                match t {
                    0 => assert!(w.enter_building(0, anchor, &defs)),
                    // A hunger need, planned every tick `consider_needs` runs, steps the unit
                    // back out before it starts walking off to eat.
                    10 => w.units[0].needs.hunger = 0.0,
                    20 => {
                        // Back on the door (foraging failed with nothing to eat nearby): fed and
                        // free to go back in.
                        w.units[0].needs.hunger = defs.needs.max();
                        w.units[0].pos = door;
                        w.units[0].route = None;
                        assert!(w.enter_building(0, anchor, &defs));
                    }
                    _ => {}
                }
                w.tick(&defs);
            }
            w
        };
        let (a, b) = (run(), run());
        assert_eq!(a.units, b.units);
        assert_eq!(a.units[0].inside(), Some(anchor));

        // A save round-trip only needs the ids to line up, which `Defs::bundled` always gives
        // the same way, even though it doesn't carry `world_with_hall`'s edits to the bed.
        let defs = Defs::bundled();
        let text = a.save(&defs).unwrap();
        let loaded = World::load(&text, &defs).unwrap();
        assert_eq!(loaded.units, a.units);
        assert_eq!(
            loaded.occupants(anchor).collect::<Vec<_>>(),
            a.occupants(anchor).collect::<Vec<_>>()
        );
    }

    fn at(x: i32, y: i32) -> Pos {
        Pos::new(x, y)
    }
}
