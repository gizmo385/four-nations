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

/// The yard source this city would raise: the first it can build whose tech is known, else the
/// first it could build at all, with that tech's name and its yard.
fn source(session: &Session) -> Option<(String, String, bool, &fn_sim::building::AutomateDef)> {
    let (world, defs) = (&session.world, &session.defs);
    let mut all =
        defs.buildings.iter().filter(|b| crate::build::available_to_nation(session, b)).filter_map(
            |b| {
                let (tech, def) = b.automate_upgrade.as_ref()?;
                let known = world.research().is_known(*tech);
                Some((
                    b.display_name(world.nation).to_owned(),
                    defs.tech(*tech).name.clone(),
                    known,
                    def,
                ))
            },
        );
    let first = all.next()?;
    if first.2 {
        return Some(first);
    }
    Some(all.find(|s| s.2).unwrap_or(first))
}

/// The key help's How to tab (playtest, 2026-11-17: "I had a brick kiln and couldn't figure out
/// what to do"): how to make machines run on their own and pass goods along, step by step, in
/// this city's own names (its yard source, the tech that raises it, its machines). Each entry is
/// a paragraph; a `Tone::Title` one is a heading and an empty one a blank row.
pub fn walkthrough(session: &Session) -> Vec<(String, crate::rows::Tone)> {
    use crate::rows::Tone;
    let (world, defs) = (&session.world, &session.defs);
    let machines: Vec<String> = defs
        .buildings
        .iter()
        .filter(|b| b.uses_power && b.recipe.is_some())
        .filter(|b| crate::build::available_to_nation(session, b))
        .take(4)
        .map(|b| b.display_name(world.nation).to_owned())
        .collect();
    let mut out = vec![("Machines that run on their own".to_owned(), Tone::Title)];
    out.push((
        format!(
            "A machine is a building with a recipe, such as {}. Normally a settler has to work \
             inside it. Inside a yard it runs with nobody there, and settlers only haul its \
             goods in and out.",
            machines.join(", ")
        ),
        Tone::Text,
    ));
    out.push((String::new(), Tone::Text));
    let Some((name, tech, known, def)) = source(session) else {
        out.push(("No tech this city can reach raises a yard yet.".to_owned(), Tone::Dim));
        return out;
    };
    let done = if known { " (done)" } else { "" };
    out.push((format!("1. Research {tech} on the Research screen (T){done}."), Tone::Text));
    let mut build = format!(
        "2. Build a {name}. Its yard is the tinted area up to {} tiles around it, shown while \
         the flow view (Y) is on.",
        def.radius
    );
    if def.beside.iter().any(|t| t.is_water()) {
        build.push_str(" A machine in it runs only if it touches water.");
    } else if def.beside.contains(&fn_sim::defs::Terrain::Mountain) {
        build.push_str(" A machine in it runs only if it touches rock (a mountain or tunnel).");
    }
    out.push((build, Tone::Text));
    out.push((
        format!(
            "3. Build machines inside the tint, or keep ones already there. A finished machine \
             in a yard switches over by itself, with nothing to press, and runs at {}% of a \
             settler's speed. Its panel reads \"Automated\".",
            (def.rate * 100.0).round() as u32
        ),
        Tone::Text,
    ));
    out.push((
        "4. Keep its inputs in your stores. Haulers bring each batch in, and the goods it makes \
         are carried to stores. A machine waiting on a missing input says which one."
            .to_owned(),
        Tone::Text,
    ));
    out.push((
        "5. To skip the hauling, make machines touch. When one makes what a touching machine \
         uses, each batch passes straight across, shown as an arrow in the flow view. For \
         example, a Smelter touching a Forge hands its metal straight to the Forge."
            .to_owned(),
        Tone::Text,
    ));
    out.push((
        "6. Keep a few tools in stores. Every few batches a machine wears and waits for a \
         settler to bring one."
            .to_owned(),
        Tone::Text,
    ));
    out.push((String::new(), Tone::Text));
    if def.capacity > 0 {
        out.push((
            format!(
                "A {name}'s yard has room for about {} small machines (big works take more). \
                 Past that, every machine in it slows down.",
                def.capacity
            ),
            Tone::Dim,
        ));
    }
    out.push((
        "O on a machine in a yard hands it back to a settler, or puts it back on its own."
            .to_owned(),
        Tone::Dim,
    ));
    out
}

/// The notice the toggle leaves, on or off (playtest, 2026-11-17: the side panel stays out of
/// it): how many yards run and links pass goods, or where the walkthrough is while no yard runs.
pub fn toggled_notice(on: bool, session: &Session) -> String {
    if !on {
        return "Flow view off".into();
    }
    let (world, defs) = (&session.world, &session.defs);
    let yards = world.yards(defs).len();
    if yards == 0 {
        return "Flow view on: no yard yet. How to set one up: ? then Tab to How to".into();
    }
    let links = world.flow_links(defs).len();
    let count = |n: usize, one: &str| {
        if n == 1 { format!("1 {one}") } else { format!("{n} {one}s") }
    };
    format!(
        "Flow view on: {} (tinted), {} passing goods (arrows)",
        count(yards, "yard"),
        count(links, "link")
    )
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
        assert!(toggled_notice(true, &s).contains("no yard yet"));
        assert_eq!(toggled_notice(false, &s), "Flow view off");
    }

    /// The walkthrough names the city's own tech, yard and machines; the side panel stays out
    /// of the flow view, and the toggle's notice points at the walkthrough.
    #[test]
    fn the_walkthrough_names_the_city_s_own_things_and_the_panel_stays_out() {
        let s = Session::new(7, fn_sim::Nation::Fire, 64, 64);
        let how: Vec<String> = walkthrough(&s).into_iter().map(|(t, _)| t).collect();
        assert!(how.iter().any(|t| t.starts_with("1. Research Great Works")), "{how:?}");
        assert!(how.iter().any(|t| t.contains("Brick Kiln")), "{how:?}");
        assert!(how.iter().any(|t| t.starts_with("2. Build a Lightning Station")), "{how:?}");

        let mut app = crate::app::App::new(s, (120, 40));
        app.act(crate::keys::Action::ToggleFlow);
        assert!(app.ui.notice.contains("How to"), "{}", app.ui.notice);
        let panel = crate::panel::SidePanel::build(
            &app.session,
            &app.ui,
            &crate::rows::Marks::new(true),
            0,
        );
        assert!(panel.tool.is_none());
    }

    #[test]
    fn a_corner_touch_points_across_and_apart_gets_none() {
        let (tile, dir) = arrow_between(foot(0, 0, 2, 2), foot(2, 2, 2, 2)).unwrap();
        assert_eq!((tile, dir), (Pos::new(1, 1), Dir::East));
        assert!(arrow_between(foot(0, 0, 2, 2), foot(4, 0, 2, 2)).is_none());
    }
}
