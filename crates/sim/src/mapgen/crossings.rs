//! River crossings (issue #30): fords that keep land connected.
//!
//! Rivers are one tile wide and not walkable, and units step 4-connected, so a river that
//! runs from the highlands to the sea (or to a lake) splits its landmass in two. This pass
//! turns the *smallest* river tiles that would join separate regions into `Terrain::Ford`,
//! which is walkable.
//!
//! It is Kruskal's algorithm: candidates are river tiles touching walkable tiles of two or more
//! different regions, taken in order of upstream flow (headwaters, where a river is a stream,
//! first; ties by position), and a candidate becomes a ford only if it still joins regions that
//! aren't already joined. So every divided landmass gets the minimum number of fords, spread
//! over the narrowest places. Rivers that never divide anything stay unbroken.
//!
//! Wide river blobs (confluences) may have no single tile touching land on both banks; those
//! are handled by `bridge_blobs`, which fords the shortest chain of river tiles between regions.

//! Decision record: fords placed by mapgen, rather than bridges as a build job or a walkable
//! but slow river. Fords guarantee connectivity from tick 0 with no player action, keep rivers
//! a real barrier everywhere else (defence, borders, trade routes later), and cost a handful of
//! tiles. Bridges can still be added later for the fords' "wide river" cousins.
//!
//! Water Tribe "work and travel on water" (vision.md) needs no mapgen support: pathfinding
//! takes per-nation `MoveRules`, so a water-faring nation just gives `Terrain::River` (and
//! `Lake`/`Water`) a movement cost instead of `None`. Fords are then simply a shortcut for
//! everyone else.

use std::collections::{BTreeSet, VecDeque};

use crate::defs::Terrain;
use crate::geom::{Grid, Pos};

/// Union-find over region labels.
struct Regions(Vec<usize>);

impl Regions {
    fn find(&mut self, mut a: usize) -> usize {
        while self.0[a] != a {
            self.0[a] = self.0[self.0[a]];
            a = self.0[a];
        }
        a
    }

    fn union(&mut self, a: usize, b: usize) {
        let (a, b) = (self.find(a), self.find(b));
        // Keep the smaller label as root so results don't depend on call order.
        self.0[a.max(b)] = a.min(b);
    }
}

/// Labels each walkable tile with its 4-connected region.
fn label_regions(terrain: &Grid<Terrain>) -> (Grid<Option<usize>>, usize) {
    let mut label = terrain.map_with(|_, _| None);
    let mut count = 0;
    for start in terrain.positions() {
        if label[start].is_some() || !terrain[start].is_walkable() {
            continue;
        }
        label[start] = Some(count);
        let mut queue = VecDeque::from([start]);
        while let Some(p) = queue.pop_front() {
            for q in terrain.neighbors(p, false) {
                if label[q].is_none() && terrain[q].is_walkable() {
                    label[q] = Some(count);
                    queue.push_back(q);
                }
            }
        }
        count += 1;
    }
    (label, count)
}

/// Fords enough river tiles that all land connected through rivers is walkable-connected.
///
/// `flow` is the upstream rain at each tile (bigger is a bigger river).
pub fn add_fords(terrain: &mut Grid<Terrain>, flow: &Grid<f64>) {
    let (mut label, count) = label_regions(terrain);
    let mut regions = Regions((0..count).collect());

    let mut candidates: Vec<Pos> =
        terrain.positions().filter(|&p| terrain[p] == Terrain::River).collect();
    candidates.sort_by(|&a, &b| flow[a].total_cmp(&flow[b]).then(a.cmp(&b)));
    for p in candidates {
        let banks: Vec<usize> =
            terrain.neighbors(p, false).filter_map(|q| label[q]).map(|l| regions.find(l)).collect();
        let Some(&first) = banks.first() else { continue };
        if banks.iter().all(|&b| b == first) {
            continue;
        }
        for &b in &banks {
            regions.union(first, b);
        }
        terrain[p] = Terrain::Ford;
        label[p] = Some(regions.find(first));
    }

    bridge_blobs(terrain, &mut label, &mut regions);
}

/// Handles river areas wider than one tile: while two regions of one landmass remain apart,
/// ford the shortest chain of river tiles between them.
fn bridge_blobs(
    terrain: &mut Grid<Terrain>,
    label: &mut Grid<Option<usize>>,
    regions: &mut Regions,
) {
    while let Some(chain) = shortest_chain(terrain, label, regions) {
        let mut joined = Vec::new();
        for &p in &chain {
            joined.extend(terrain.neighbors(p, false).filter_map(|q| label[q]));
            terrain[p] = Terrain::Ford;
        }
        let first = joined[0];
        for &l in &joined {
            regions.union(first, l);
        }
        let root = regions.find(first);
        for p in chain {
            label[p] = Some(root);
        }
    }
}

/// BFS through river tiles from one region at a time until it reaches a different region;
/// returns the river tiles of the shortest such chain (the first one found, in label order).
fn shortest_chain(
    terrain: &Grid<Terrain>,
    label: &Grid<Option<usize>>,
    regions: &mut Regions,
) -> Option<Vec<Pos>> {
    // Every river tile touching a region, as (region label, tile), in a deterministic order.
    let mut starts: Vec<(usize, Pos)> = Vec::new();
    for p in terrain.positions().filter(|&p| terrain[p] == Terrain::River) {
        starts.extend(terrain.neighbors(p, false).filter_map(|q| label[q]).map(|l| (l, p)));
    }
    starts.sort();
    let mut tried = BTreeSet::new();
    for &(l, _) in &starts {
        let root = regions.find(l);
        if !tried.insert(root) {
            continue;
        }
        let mut parent: Grid<Option<Pos>> = terrain.map_with(|_, _| None);
        let mut seen = terrain.map_with(|_, _| false);
        let mut queue = VecDeque::new();
        for &(l, p) in &starts {
            if regions.find(l) == root && !seen[p] {
                seen[p] = true;
                queue.push_back(p);
            }
        }
        while let Some(p) = queue.pop_front() {
            for q in terrain.neighbors(p, false) {
                if let Some(other) = label[q] {
                    if regions.find(other) != root {
                        let mut chain = vec![p];
                        let mut cur = p;
                        while let Some(prev) = parent[cur] {
                            chain.push(prev);
                            cur = prev;
                        }
                        return Some(chain);
                    }
                } else if terrain[q] == Terrain::River && !seen[q] {
                    seen[q] = true;
                    parent[q] = Some(p);
                    queue.push_back(q);
                }
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(rows: &[&str]) -> Grid<Terrain> {
        let (w, h) = (rows[0].len() as i32, rows.len() as i32);
        Grid::from_fn(w, h, |p| match rows[p.y as usize].as_bytes()[p.x as usize] {
            b'.' => Terrain::Land,
            b'=' => Terrain::River,
            b'~' => Terrain::Water,
            b'-' => Terrain::Lake,
            b'#' => Terrain::Mountain,
            c => panic!("bad glyph {}", c as char),
        })
    }

    fn render(g: &Grid<Terrain>) -> Vec<String> {
        (0..g.height())
            .map(|y| {
                (0..g.width())
                    .map(|x| match g[Pos::new(x, y)] {
                        Terrain::Land => '.',
                        Terrain::River => '=',
                        Terrain::Water => '~',
                        Terrain::Lake => '-',
                        Terrain::Mountain => '#',
                        Terrain::Ford => ':',
                        Terrain::Coast => ',',
                        Terrain::Tunnel => '_',
                    })
                    .collect()
            })
            .collect()
    }

    /// Flow grows toward the bottom, like a river running south to the sea.
    fn flow_by_row(g: &Grid<Terrain>) -> Grid<f64> {
        Grid::from_fn(g.width(), g.height(), |p| f64::from(p.y))
    }

    fn fords(g: &Grid<Terrain>) -> usize {
        g.iter().filter(|(_, t)| **t == Terrain::Ford).count()
    }

    #[test]
    fn splitting_river_gets_one_ford_at_the_headwaters() {
        let mut t = parse(&["..=..", "..=..", "..=..", "..=.."]);
        let flow = flow_by_row(&t);
        add_fords(&mut t, &flow);
        assert_eq!(render(&t), ["..:..", "..=..", "..=..", "..=.."]);
    }

    #[test]
    fn river_that_divides_nothing_is_left_alone() {
        // The river ends in the sea at the top and the land is all one piece.
        let mut t = parse(&["~~=~~", "~...~", "~...~"]);
        let flow = flow_by_row(&t);
        add_fords(&mut t, &flow);
        assert_eq!(fords(&t), 0);
    }

    #[test]
    fn diagonal_river_is_forded_too() {
        let mut t = parse(&["~~~~~", "...=.", "..=..", ".=...", "~~~~~"]);
        let flow = flow_by_row(&t);
        add_fords(&mut t, &flow);
        assert_eq!(fords(&t), 1);
        assert_eq!(label_regions(&t).1, 1);
    }

    #[test]
    fn wide_river_gets_a_chain_of_fords() {
        let mut t = parse(&["..==..", "..==..", "..==.."]);
        let flow = flow_by_row(&t);
        add_fords(&mut t, &flow);
        assert_eq!(fords(&t), 2, "{:?}", render(&t));
        assert_eq!(label_regions(&t).1, 1);
    }

    #[test]
    fn lakes_and_sea_are_never_forded() {
        let mut t = parse(&["..-..", "..-..", "..~..", "..#.."]);
        let flow = flow_by_row(&t);
        add_fords(&mut t, &flow);
        assert_eq!(fords(&t), 0);
    }
}
