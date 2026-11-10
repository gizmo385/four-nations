//! The orchard: a building that plants and tends a renewable resource around itself (issue
//! #142). The design, and why each number is what it is, is in docs/design/research.md ("The
//! orchard"); the numbers are `[building.orchard]` in data/buildings.toml (`OrchardDef`). Water's
//! Fishing Dock (issue #374) is the same mechanism, planting `fish_spot` on the water instead of
//! `fruit_tree` on land.
//!
//! * **Planting** is a process of the building, not a job: while a finished orchard tends fewer
//!   than `max_trees` trees, every `plant_seconds` it plants one more on a free tile of the
//!   planted resource's own terrain within `radius`. The tile is drawn from the `orchard.plant`
//!   RNG stream. A new tree is an ordinary `fruit_tree` resource (nothing new on the map) that
//!   starts out *resting* for `grow_seconds` and is a **sapling** (`World::is_sapling`) until it
//!   first ripens. After that it is a plain fruit tree: picking it works exactly as for a wild
//!   one.
//! * **Tending** is twofold. The orchard counts the trees it planted that are still standing and
//!   plants no more than `max_trees`; and any resting tree of the planted kind within `radius`
//!   regrows `tend_boost` times as fast (`World::tend_rate`, read by `growth_rate`).
//! * **Regrowth**: a planted tree regrows after picking like any fruit tree (`regrow` in
//!   data/jobs.toml). With `auto_pick`, planting also leaves a standing harvest order (#81) on the
//!   tree, so it is picked each time it ripens without anyone designating it. `CancelHarvest`
//!   withdraws the order; the tree keeps growing and can still be picked by hand.
//! * Which nations can build an orchard is research's business (`orchards`, an Air tech, or
//!   `water_travel` for the Fishing Dock), and `designate_build` refuses it otherwise. Wild
//!   picking of either resource is unaffected.

use super::*;

use crate::building::OrchardDef;

/// What a finished orchard remembers: when it plants next, and which trees it planted that still
/// stand.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub(super) struct Orchard {
    /// Seconds until it plants again. Only counts down while it tends fewer than `max_trees`.
    plant_in: f32,
    /// The trees it planted, saplings included.
    trees: BTreeSet<Pos>,
}

/// The stream orchard planting draws from. A saved world that predates orchards loads with this
/// seeded by 0 instead of the world seed; still deterministic.
pub(super) fn default_rng() -> SimRng {
    rng::stream(0, "orchard.plant")
}

pub(super) fn new_rng(seed: u64) -> SimRng {
    rng::stream(seed, "orchard.plant")
}

/// Tiles within Chebyshev distance `r` of `c`, row by row (a fixed order, for determinism).
fn square(c: Pos, r: i32) -> impl Iterator<Item = Pos> {
    (-r..=r).flat_map(move |dy| (-r..=r).map(move |dx| Pos::new(c.x + dx, c.y + dy)))
}

impl World {
    /// Is the fruit tree on `pos` a sapling: planted by an orchard and not yet ripe for the first
    /// time? (An unripe one is resting too, so a client greys or shrinks it; this tells it apart
    /// from a tree that was merely picked.)
    pub fn is_sapling(&self, pos: Pos) -> bool {
        self.saplings.contains(&pos)
    }

    /// The finished orchards with how many trees each tends, in tile order.
    pub fn orchards(&self) -> impl Iterator<Item = (Pos, usize)> + '_ {
        self.orchards.iter().map(|(&p, o)| (p, o.trees.len()))
    }

    /// How many trees the orchard on `pos` tends right now, if there is one.
    pub fn orchard_trees(&self, pos: Pos) -> Option<usize> {
        self.orchards.get(&pos).map(|o| o.trees.len())
    }

    /// Seconds until the orchard on `pos` plants its next tree, if it is due to (it tends fewer
    /// than its maximum).
    pub fn orchard_plants_in(&self, pos: Pos, defs: &Defs) -> Option<f32> {
        let o = self.orchards.get(&pos)?;
        let def = self.orchard_def(pos, defs)?;
        ((o.trees.len() as u32) < def.max_trees).then_some(o.plant_in)
    }

    fn orchard_def(&self, pos: Pos, defs: &Defs) -> Option<OrchardDef> {
        defs.building(self.map.get(pos)?.building?.id).orchard
    }

    /// A building has just been finished on `pos`: if it is an orchard, it starts its first
    /// planting timer.
    pub(super) fn open_orchard(&mut self, pos: Pos, def: &OrchardDef) {
        self.orchards.insert(pos, Orchard { plant_in: def.plant_seconds, trees: BTreeSet::new() });
    }

    /// Registers orchards that stand on the map but not in `orchards`: ones in a save from before
    /// orchards did anything, and ones a test or tool put on the map by hand (like
    /// `map_changed`, for code that writes to `map` directly). Orchards already known are left
    /// as they are.
    ///
    /// Only a building's anchor tile (`part == [0, 0]`) counts: a 2x2 orchard is one orchard. A
    /// save written by a build that registered every tile has an entry on each non-anchor tile
    /// too; those are dropped here, and with them their trees and planting timer. The anchor's
    /// entry (its own trees and timer) is the one that stays.
    pub fn adopt_orchards(&mut self, defs: &Defs) {
        let map = &self.map;
        self.orchards
            .retain(|&p, _| map.get(p).and_then(|t| t.building).is_none_or(|b| b.part == [0, 0]));
        let found: Vec<(Pos, OrchardDef)> = self
            .map
            .iter()
            .filter_map(|(p, t)| {
                let b = t.building?;
                (b.part == [0, 0]).then_some(())?;
                Some((p, defs.building(b.id).orchard?))
            })
            .filter(|(p, _)| !self.orchards.contains_key(p))
            .collect();
        for (p, def) in found {
            self.open_orchard(p, &def);
        }
    }

    /// How many times faster than normal a resting resource on `pos` grows because an orchard
    /// tends it: the best `tend_boost` among orchards in reach that plant its kind, else 1.0.
    pub(super) fn tend_rate(&self, pos: Pos, defs: &Defs) -> f32 {
        let Some(kind) = self.map.get(pos).and_then(|t| t.resource) else { return 1.0 };
        self.orchards
            .keys()
            .filter_map(|&o| self.orchard_def(o, defs).map(|d| (o, d)))
            .filter(|(o, d)| d.plants == kind && o.chebyshev(pos) <= d.radius as i32)
            .map(|(_, d)| d.tend_boost)
            .fold(1.0, f32::max)
    }

    /// One tick of every orchard's planting.
    pub(super) fn tick_orchards(&mut self, defs: &Defs) {
        if self.orchards.is_empty() {
            return;
        }
        let step = 1.0 / TICKS_PER_SECOND as f32;
        let spots: Vec<Pos> = self.orchards.keys().copied().collect();
        for pos in spots {
            let Some(def) = self.orchard_def(pos, defs) else {
                // Nothing stands here any more (no demolish yet, so only a hand-edited map).
                self.orchards.remove(&pos);
                continue;
            };
            let map = &self.map;
            let o = self.orchards.get_mut(&pos).expect("just listed");
            o.trees.retain(|&t| map[t].resource == Some(def.plants));
            if o.trees.len() as u32 >= def.max_trees {
                o.plant_in = def.plant_seconds;
                continue;
            }
            o.plant_in -= step;
            if o.plant_in > 0.0 {
                continue;
            }
            o.plant_in = def.plant_seconds;
            if let Some(tree) = self.pick_planting_site(pos, &def, defs) {
                self.plant(pos, tree, &def);
            }
        }
    }

    /// Plants one tree of the orchard on `pos` at `tree`.
    fn plant(&mut self, orchard: Pos, tree: Pos, def: &OrchardDef) {
        self.map[tree].resource = Some(def.plants);
        self.map_changed();
        self.resting.insert(tree, def.grow_seconds);
        self.saplings.insert(tree);
        if def.auto_pick {
            self.standing_harvest.insert(tree);
        }
        self.orchards.get_mut(&orchard).expect("planting orchard exists").trees.insert(tree);
    }

    /// Where the orchard on `pos` plants next: a random one of the tiles in reach that
    /// `can_plant` allows, or `None` if there is none.
    fn pick_planting_site(&mut self, pos: Pos, def: &OrchardDef, defs: &Defs) -> Option<Pos> {
        let mut sites: Vec<Pos> =
            square(pos, def.radius as i32).filter(|&p| self.can_plant(p, def, defs)).collect();
        // A tree blocks movement for good, so one that would seal ground off (`would_cut_off`,
        // #490) is passed over for another. The first draw is the same one it always was.
        while !sites.is_empty() {
            let site = sites.swap_remove(self.orchard_rng.random_range(0..sites.len()));
            if !self.would_cut_off(site) {
                return Some(site);
            }
        }
        None
    }

    /// Can `p` take a new tree: terrain the tree spawns on in mapgen (fruit trees: land; biome is
    /// deliberately not checked, since tending is what lets a grove stand where the wild one
    /// wouldn't), with nothing built, designated, stored,
    /// lying or bent there, no unit on it or walking over it (a resource blocks movement), and no
    /// other resource or building closer than `spacing` tiles. A unit inside a building (#226)
    /// doesn't hold its entrance tile this way. A resource that spawns on water (Water's fishing
    /// spots, issue #374) skips the walkable check entirely -- the water is the point -- but
    /// still honours the resource's own mapgen `beside` rule (fish spots: coast or land), so
    /// every one it plants stays within reach of the shore the same way a wild one would. A
    /// pending build site is treated like a finished building here too
    /// (`docs/design/clearing.md`, issue #425), door tile included: a sapling beside one, or on
    /// its door, would block the door the moment the site was cleared.
    fn can_plant(&self, p: Pos, def: &OrchardDef, defs: &Defs) -> bool {
        let Some(tile) = self.map.get(p) else { return false };
        let plant = defs.resource(def.plants);
        let aquatic = plant.terrains.as_ref().is_some_and(|t| t.iter().any(|t| t.is_water()));
        if tile.resource.is_some()
            || tile.building.is_some()
            || (!aquatic && !self.rules.passable(tile))
            || !plant.terrains.as_ref().is_none_or(|t| t.contains(&tile.terrain))
            || !plant.beside.as_ref().is_none_or(|ts| {
                self.map.neighbors(p, true).any(|q| ts.contains(&self.map[q].terrain))
            })
            || self.site_at(p).is_some()
            || self.site_door(p, defs)
            || self.zones.contains(&p)
            || self.ground.contains_key(&p)
            || self.bend_designations().any(|(_, q)| q == p)
            || self.units_at(p).next().is_some()
            || self.units.iter().any(|u| u.route.as_ref().is_some_and(|r| r.contains(&p)))
        {
            return false;
        }
        // A building counts too, the orchard's own included: a tree's canopy would hide its art.
        // A pending site does too, footprint and door both.
        !square(p, def.spacing as i32 - 1).any(|q| {
            q != p
                && (self.map.get(q).is_some_and(|t| t.resource.is_some() || t.building.is_some())
                    || self.site_at(q).is_some()
                    || self.site_door(q, defs))
        })
    }
}

#[cfg(test)]
mod tests;
