//! The production overlay (docs/design/factory-endgame.md, "The map, with the production
//! overlay", #1040): a map toggle (`Y`) that shows where goods flow. An arrow on a machine's edge
//! points into each touching machine its batches pass straight to, and every automating yard
//! running now tints its reach, the way a power source's does under the cursor.

use std::collections::BTreeMap;

use fn_sim::geom::{Footprint, Pos};

use crate::session::Session;

/// Which way a flow arrow points.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Dir {
    North,
    East,
    South,
    West,
}

impl Dir {
    const ALL: [(Self, i32, i32); 4] =
        [(Self::North, 0, -1), (Self::East, 1, 0), (Self::South, 0, 1), (Self::West, -1, 0)];
}

/// Every flow arrow, by the tile it draws on: one per pass-across link (`World::flow_links`), on
/// the middle tile of the sending machine's edge that faces the taker. Where two links would draw
/// on the same tile, the later one in the sim's order wins.
pub fn arrows(session: &Session) -> BTreeMap<Pos, Dir> {
    let world = &session.world;
    let mut out = BTreeMap::new();
    for (from, to) in world.flow_links(&session.defs) {
        let (Some(a), Some(b)) = (world.footprint_at(from), world.footprint_at(to)) else {
            continue;
        };
        if let Some((tile, dir)) = arrow_between(a, b) {
            out.insert(tile, dir);
        }
    }
    out
}

/// Where the arrow from `a` into `b` draws: the middle of `a`'s tiles that have a side on `b`,
/// pointing at it. Two footprints that touch only at a corner get the corner tile, pointing
/// across (east or west) toward `b`.
pub fn arrow_between(a: Footprint, b: Footprint) -> Option<(Pos, Dir)> {
    let inside = |f: Footprint, p: Pos| f.distance(p) == 0;
    let edge: Vec<(Pos, Dir)> = a
        .tiles()
        .flat_map(|t| {
            Dir::ALL.into_iter().map(move |(d, dx, dy)| (t, d, Pos::new(t.x + dx, t.y + dy)))
        })
        .filter(|&(_, _, next)| inside(b, next))
        .map(|(t, d, _)| (t, d))
        .collect();
    if let Some(&mid) = edge.get(edge.len() / 2) {
        return Some(mid);
    }
    if a.gap(b) != 1 {
        return None;
    }
    let corner = a.tiles().min_by_key(|&t| b.distance(t))?;
    let dir = if b.anchor.x > corner.x { Dir::East } else { Dir::West };
    Some((corner, dir))
}

/// Every running yard's reach, as footprint and radius, while the overlay is on
/// (`World::yards`).
pub fn yard_reaches(session: &Session) -> Vec<(Footprint, u32)> {
    session.world.yards(&session.defs)
}

/// The notice the toggle leaves, on or off.
pub fn toggled_notice(on: bool, session: &Session) -> String {
    if !on {
        return "Flow view off".into();
    }
    let links = session.world.flow_links(&session.defs).len();
    match links {
        0 => "Flow view on: no machine passes goods across yet. Tinted: each yard's reach".into(),
        1 => {
            "Flow view on: 1 machine passes goods across (arrow). Tinted: each yard's reach".into()
        }
        n => {
            format!("Flow view on: {n} links pass goods across (arrows). Tinted: each yard's reach")
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn foot(x: i32, y: i32, w: u8, h: u8) -> Footprint {
        Footprint { anchor: Pos::new(x, y), w, h }
    }

    #[test]
    fn the_arrow_sits_mid_edge_and_points_into_the_taker() {
        // A 2x2 at (0,0) beside a 3x2 at (2,0): the edge is x = 1, rows 0 and 1.
        let (tile, dir) = arrow_between(foot(0, 0, 2, 2), foot(2, 0, 3, 2)).unwrap();
        assert_eq!(dir, Dir::East);
        assert_eq!(tile.x, 1);
        // Below, pointing south; and back the other way, west.
        assert_eq!(arrow_between(foot(0, 0, 2, 2), foot(0, 2, 2, 2)).unwrap().1, Dir::South);
        assert_eq!(arrow_between(foot(2, 0, 2, 2), foot(0, 0, 2, 2)).unwrap().1, Dir::West);
    }

    /// The automated case is the sim's (`World::flow_links`, `World::yards`); without a yard
    /// running, nothing passes across and the notice says so.
    #[test]
    fn with_no_yard_running_the_overlay_shows_nothing_and_says_so() {
        let s = Session::new(7, fn_sim::Nation::Fire, 64, 64);
        assert!(arrows(&s).is_empty());
        assert!(yard_reaches(&s).is_empty());
        assert!(toggled_notice(true, &s).contains("no machine passes goods across yet"));
        assert_eq!(toggled_notice(false, &s), "Flow view off");
    }

    #[test]
    fn a_corner_touch_points_across_and_apart_gets_none() {
        let (tile, dir) = arrow_between(foot(0, 0, 2, 2), foot(2, 2, 2, 2)).unwrap();
        assert_eq!((tile, dir), (Pos::new(1, 1), Dir::East));
        assert!(arrow_between(foot(0, 0, 2, 2), foot(4, 0, 2, 2)).is_none());
    }
}
