//! Connected regions of the tile map: which tiles can reach which, answered in O(1).
//!
//! `path::find` is quick when the target is reachable, but for an unreachable one it explores
//! the whole connected region before giving up (10-25 ms on an open 256x256 map). Callers that
//! may ask for unreachable spots (resources across a river, on an island) check
//! `Regions::connected` first, or call `Regions::find`, which does so itself.
//!
//! Units move 8-way without cutting corners, and that reaches exactly the tiles 4-way movement
//! does: a diagonal step between two passable tiles can always be made as two cardinal steps
//! through one of the tiles it squeezes past, and the no-corner-cutting rule demands both are
//! passable anyway. So regions are labelled by a plain 4-way flood fill.
//!
//! Passability comes from `MoveRules::passable`, so the labelling follows whatever the rules
//! say, whatever terrain that is (a ford, a swimmable sea for a nation that can cross water).
//! Movement costs don't matter here, only whether a tile can be entered at all. A region map
//! belongs to one `MoveRules`; units with different rules need their own.
//!
//! # Keeping it up to date
//!
//! A `Regions` is a snapshot. It goes stale when a tile's passability changes: terrain edits,
//! a resource appearing or being cleared (resources block by default), or different rules.
//! For now the answer is to recompute, which is one linear pass over the map. `World` keeps one,
//! marks it dirty whenever it changes the map (`World::map_changed` for outside code that edits
//! `World::map`), and recomputes lazily on the next query, so a burst of changes in one tick
//! costs one recompute. If that ever shows up in a profile, clearing a resource can only merge
//! regions, so it could be patched in place; blocking a tile can split one, which needs a
//! re-label of just the affected region.
//!
//! # What it does not do for `Flood`
//!
//! `flood::Flood` (one-to-many Dijkstra, used for job claiming) already visits only the
//! origin's region, so unlike `path::find` it has no wasted exploration to avoid for an
//! unreachable target. What the cache buys there is skipping the flood entirely: `World` asks
//! `Regions` whether any open job is in the unit's region before flooding at all.

use crate::geom::Pos;
use crate::mapgen::TileMap;
use crate::path::{self, MoveRules};

const NONE: u32 = u32::MAX;

/// Identifies a connected region of a `Regions` map. Ids are assigned in row-major order of each
/// region's first tile, so the same map and rules always label the same way.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct RegionId(pub u32);

/// A labelling of every passable tile with the region it belongs to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Regions {
    width: i32,
    height: i32,
    /// Region id per tile, `NONE` for impassable tiles.
    label: Vec<u32>,
    /// Tiles in each region, indexed by region id.
    sizes: Vec<u32>,
}

impl Regions {
    /// Labels the regions of `map` under `rules`: one linear pass over the map.
    pub fn compute(map: &TileMap, rules: &MoveRules) -> Self {
        let (width, height) = (map.width(), map.height());
        let len = (width * height) as usize;
        let passable: Vec<bool> = map.iter().map(|(_, t)| rules.passable(t)).collect();
        let mut label = vec![NONE; len];
        let mut sizes = Vec::new();
        let mut stack: Vec<u32> = Vec::new();
        for start in 0..len {
            if !passable[start] || label[start] != NONE {
                continue;
            }
            let id = sizes.len() as u32;
            let mut size = 0;
            label[start] = id;
            stack.push(start as u32);
            while let Some(i) = stack.pop() {
                size += 1;
                let i = i as usize;
                let (x, y) = (i as i32 % width, i as i32 / width);
                let mut visit = |n: usize| {
                    if passable[n] && label[n] == NONE {
                        label[n] = id;
                        stack.push(n as u32);
                    }
                };
                if x > 0 {
                    visit(i - 1);
                }
                if x + 1 < width {
                    visit(i + 1);
                }
                if y > 0 {
                    visit(i - width as usize);
                }
                if y + 1 < height {
                    visit(i + width as usize);
                }
            }
            sizes.push(size);
        }
        Self { width, height, label, sizes }
    }

    /// The region `p` is in, or `None` if it is out of bounds or impassable.
    pub fn region(&self, p: Pos) -> Option<RegionId> {
        if p.x < 0 || p.y < 0 || p.x >= self.width || p.y >= self.height {
            return None;
        }
        match self.label[(p.y * self.width + p.x) as usize] {
            NONE => None,
            id => Some(RegionId(id)),
        }
    }

    /// Whether a unit on `a` can walk to `b`: both passable and in the same region. O(1), and
    /// equal to `path::find(..).is_some()` for the rules the regions were computed with.
    pub fn connected(&self, a: Pos, b: Pos) -> bool {
        match (self.region(a), self.region(b)) {
            (Some(ra), Some(rb)) => ra == rb,
            _ => false,
        }
    }

    /// How many regions there are.
    pub fn count(&self) -> usize {
        self.sizes.len()
    }

    /// How many tiles `region` has.
    pub fn size(&self, region: RegionId) -> usize {
        self.sizes[region.0 as usize] as usize
    }

    /// `path::find`, but answering `None` at once when `from` and `to` aren't connected instead
    /// of searching the whole region first. `self` must be current for `map` and `rules`.
    pub fn find(&self, map: &TileMap, from: Pos, to: Pos, rules: &MoveRules) -> Option<Vec<Pos>> {
        if !self.connected(from, to) {
            return None;
        }
        path::find(map, from, to, rules)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::defs::{BiomeId, ResourceId, Terrain};
    use crate::geom::Grid;
    use crate::mapgen::Tile;

    fn map(rows: &[&str]) -> TileMap {
        Grid::from_fn(rows[0].len() as i32, rows.len() as i32, |p| Tile {
            elevation: 0,
            temperature: 0,
            moisture: 0,
            terrain: match rows[p.y as usize].as_bytes()[p.x as usize] {
                b'#' => Terrain::Mountain,
                b'~' => Terrain::Water,
                _ => Terrain::Land,
            },
            biome: BiomeId(0),
            resource: None,
        })
    }

    #[test]
    fn labels_split_by_walls_in_row_major_order() {
        let m = map(&["..#..", "..#..", "..#.."]);
        let r = Regions::compute(&m, &MoveRules::default());
        assert_eq!(r.count(), 2);
        assert_eq!(r.region(Pos::new(0, 0)), Some(RegionId(0)));
        assert_eq!(r.region(Pos::new(4, 2)), Some(RegionId(1)));
        assert_eq!(r.size(RegionId(0)), 6);
        assert!(r.connected(Pos::new(0, 0), Pos::new(1, 2)));
        assert!(!r.connected(Pos::new(0, 0), Pos::new(3, 0)));
    }

    #[test]
    fn impassable_and_out_of_bounds_tiles_connect_to_nothing() {
        let m = map(&["..#"]);
        let r = Regions::compute(&m, &MoveRules::default());
        let wall = Pos::new(2, 0);
        assert_eq!(r.region(wall), None);
        assert!(!r.connected(wall, wall));
        assert!(!r.connected(Pos::new(0, 0), wall));
        assert!(!r.connected(Pos::new(0, 0), Pos::new(-1, 0)));
        assert!(!r.connected(Pos::new(9, 9), Pos::new(9, 9)));
        assert!(r.connected(Pos::new(0, 0), Pos::new(0, 0)));
    }

    #[test]
    fn diagonal_gaps_between_touching_walls_do_not_connect() {
        // Walls touching at a corner: no corner cutting, so 4-way and 8-way agree.
        let m = map(&[".#...", "#....", "....."]);
        let rules = MoveRules::default();
        let r = Regions::compute(&m, &rules);
        let (a, b) = (Pos::new(0, 0), Pos::new(1, 1));
        assert!(!r.connected(a, b));
        assert_eq!(path::find(&m, a, b, &rules), None);
    }

    #[test]
    fn follows_the_rules_not_the_terrain() {
        let m = map(&["..~~..", "..~~.."]);
        let (a, b) = (Pos::new(0, 0), Pos::new(5, 1));
        assert!(!Regions::compute(&m, &MoveRules::default()).connected(a, b));
        let swim = MoveRules::default().with_cost(Terrain::Water, Some(2));
        let r = Regions::compute(&m, &swim);
        assert!(r.connected(a, b));
        assert_eq!(r.count(), 1);
    }

    #[test]
    fn resources_split_regions_only_when_they_block() {
        let mut m = map(&["...", "...", "..."]);
        for y in 0..3 {
            m[Pos::new(1, y)].resource = Some(ResourceId(0));
        }
        let (a, b) = (Pos::new(0, 1), Pos::new(2, 1));
        let mut rules = MoveRules::default();
        assert!(!Regions::compute(&m, &rules).connected(a, b));
        rules.resources_block = false;
        assert!(Regions::compute(&m, &rules).connected(a, b));
    }

    #[test]
    fn find_agrees_with_path_find_on_every_pair() {
        let m = map(&["..#..", "..#..", "..#.."]);
        let rules = MoveRules::default();
        let r = Regions::compute(&m, &rules);
        for a in m.positions() {
            for b in m.positions() {
                let direct = path::find(&m, a, b, &rules);
                assert_eq!(r.connected(a, b), direct.is_some(), "{a:?} {b:?}");
                assert_eq!(r.find(&m, a, b, &rules), direct);
            }
        }
    }
}
