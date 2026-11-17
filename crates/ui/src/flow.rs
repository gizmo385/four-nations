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

/// The yard source this city would raise ([`source`]).
struct Source<'a> {
    id: fn_sim::building::BuildingId,
    name: String,
    tech: String,
    known: bool,
    def: &'a fn_sim::building::AutomateDef,
}

/// The yard source this city would raise: the first it can build whose tech is known, else the
/// first it could build at all, with that tech's name and its yard.
fn source(session: &Session) -> Option<Source<'_>> {
    let (world, defs) = (&session.world, &session.defs);
    let mut all = defs
        .buildings
        .iter()
        .enumerate()
        .filter(|(_, b)| crate::build::available_to_nation(session, b))
        .filter_map(|(i, b)| {
            let (tech, def) = b.automate_upgrade.as_ref()?;
            Some(Source {
                id: fn_sim::building::BuildingId(i as u16),
                name: b.display_name(world.nation).to_owned(),
                tech: defs.tech(*tech).name.clone(),
                known: world.research().is_known(*tech),
                def,
            })
        });
    let first = all.next()?;
    if first.known {
        return Some(first);
    }
    Some(all.find(|s| s.known).unwrap_or(first))
}

/// A piece of the Machines tab ([`walkthrough`]).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Para {
    Heading(String),
    /// Wrapped to the page.
    Text(String),
    /// Wrapped, dimmer: a side note.
    Note(String),
    /// The example layout, drawn by the frontend as a patch of map.
    Demo(Demo),
    Blank,
}

/// The Machines tab's example (playtest, 2026-11-17: "use the actual game icons"): a patch of map
/// `w` by `h` tiles with the yard source, its tint, and two touching machines, the first passing
/// its batches into the second (the arrow). A frontend draws each tile as its map would.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Demo {
    pub w: i32,
    pub h: i32,
    /// The source, then the machine that makes the good, then the one that uses it.
    pub buildings: Vec<(fn_sim::building::BuildingId, Footprint)>,
    /// The source's footprint and its yard's radius: every tile within it is tinted.
    pub reach: (Footprint, u32),
    /// Where the flow arrow draws, and which way it points.
    pub arrow: (Pos, Dir),
    /// The bottom row's terrain, when the yard's machines must touch it (water, rock).
    pub beside: Option<fn_sim::Terrain>,
}

impl Demo {
    /// The building on `p`, if any.
    pub fn building_at(&self, p: Pos) -> Option<fn_sim::building::BuildingId> {
        self.buildings.iter().find(|(_, f)| f.distance(p) == 0).map(|(b, _)| *b)
    }

    /// Whether `p` is in the yard's tint.
    pub fn tinted(&self, p: Pos) -> bool {
        let (foot, radius) = self.reach;
        foot.distance(p) <= radius as i32
    }
}

/// Two machines this city can build where the first makes what the second uses (a Smelter and a
/// Forge), and the good, and whether they are the city's own: the example for touching
/// machines. Any nation's pair if the city has none of its own yet.
fn chain(
    session: &Session,
) -> Option<(fn_sim::building::BuildingId, fn_sim::building::BuildingId, String, bool)> {
    use fn_sim::building::BuildingId;
    let defs = &session.defs;
    let machines = |own: bool| {
        defs.buildings
            .iter()
            .enumerate()
            .filter(|(_, b)| b.uses_power && b.recipe.is_some() && !b.endeavor)
            .filter(move |(_, b)| !own || crate::build::available_to_nation(session, b))
            .collect::<Vec<_>>()
    };
    for own in [true, false] {
        let list = machines(own);
        for (ia, a) in &list {
            let Some(made) = a.recipe.as_ref().map(|r| r.output.item) else { continue };
            for (ib, b) in &list {
                let Some(r) = b.recipe.as_ref() else { continue };
                if ia != ib && r.inputs.iter().any(|c| c.item == made) {
                    let good = defs.item(made).name.to_lowercase();
                    return Some((BuildingId(*ia as u16), BuildingId(*ib as u16), good, own));
                }
            }
        }
    }
    None
}

/// The example layout: the source in the top left, the two machines touching below it, inside
/// its reach, and the tint running a tile past the source's reach on the right and below so its
/// edge shows. With a `beside` terrain, the machines sit on a row of it.
fn demo(
    source: &Source<'_>,
    from: fn_sim::building::BuildingId,
    to: fn_sim::building::BuildingId,
    session: &Session,
) -> Demo {
    let defs = &session.defs;
    let size = |b: fn_sim::building::BuildingId| defs.building(b).footprint;
    let (sw, sh) = size(source.id);
    let (aw, ah) = size(from);
    let (bw, bh) = size(to);
    let src = Footprint { anchor: Pos::new(1, 1), w: sw, h: sh };
    let row = 1 + i32::from(sh) + 1;
    let a = Footprint { anchor: Pos::new(1, row), w: aw, h: ah };
    let b = Footprint { anchor: Pos::new(1 + i32::from(aw), row), w: bw, h: bh };
    let radius = source.def.radius;
    let beside = if source.def.beside.iter().any(|t| t.is_water()) {
        Some(fn_sim::Terrain::Water)
    } else if source.def.beside.contains(&fn_sim::Terrain::Mountain) {
        Some(fn_sim::Terrain::Mountain)
    } else {
        None
    };
    let machines_bottom = row + i32::from(ah.max(bh));
    // Two untinted columns past the reach, so its edge shows; the machines sit on the bottom
    // row's terrain when they must touch it.
    let w = (1 + i32::from(sw) + radius as i32 + 2).max(1 + i32::from(aw) + i32::from(bw) + 2);
    let h = match beside {
        Some(_) => machines_bottom + 1,
        None => (1 + i32::from(sh) + radius as i32 + 2).max(machines_bottom + 1),
    };
    let arrow = arrow_between(a, b).unwrap_or((a.anchor, Dir::East));
    Demo {
        w,
        h,
        buildings: vec![(source.id, src), (from, a), (to, b)],
        reach: (src, radius),
        arrow,
        beside,
    }
}

/// The key help's Machines tab (playtest, 2026-11-17: "I had a brick kiln and couldn't figure out
/// what to do"; then "phrased incredibly strangely ... include an example layout", "use the
/// actual game icons"): how to make machines run on their own, in this city's own names (its
/// yard source, the tech that raises it, its machines), with a patch of map showing a yard and
/// two touching machines.
pub fn walkthrough(session: &Session) -> Vec<Para> {
    let (world, defs) = (&session.world, &session.defs);
    let machines: Vec<String> = defs
        .buildings
        .iter()
        .filter(|b| b.uses_power && b.recipe.is_some())
        .filter(|b| crate::build::available_to_nation(session, b))
        .take(3)
        .map(|b| b.display_name(world.nation).to_owned())
        .collect();
    let examples = match machines.len() {
        0 => String::new(),
        1 => format!(" (like the {})", machines[0]),
        _ => format!(" ({}...)", machines.join(", ")),
    };
    let mut out = vec![
        Para::Heading("Machines that run on their own".to_owned()),
        Para::Text(format!(
            "A machine{examples} normally needs a settler inside. In a yard it runs by itself."
        )),
        Para::Blank,
    ];
    let Some(source) = source(session) else {
        out.push(Para::Note("Nothing your city can research raises a yard yet.".to_owned()));
        return out;
    };
    let (name, def) = (source.name.clone(), source.def);
    let done = if source.known { " (done)" } else { "" };
    out.push(Para::Text(format!("1. Research {} (T){done}.", source.tech)));
    let mut build =
        format!("2. Build a {name}. Its yard is the tinted area around it when Y is on.");
    match def.beside.iter().any(|t| t.is_water()) {
        true => build.push_str(" Machines in it must touch water."),
        false if def.beside.contains(&fn_sim::Terrain::Mountain) => {
            build.push_str(" Machines in it must touch rock.");
        }
        false => {}
    }
    out.push(Para::Text(build));
    out.push(Para::Text(format!(
        "3. Build machines in the yard. They switch over once built, at {}% speed.",
        (def.rate * 100.0).round() as u32
    )));
    out.push(Para::Text(
        "4. Keep their inputs in stockpiles. Settlers carry goods in and out.".to_owned(),
    ));
    let chain = chain(session);
    if let Some((a, b, good, own)) = &chain {
        let (from, to) = (
            defs.building(*a).display_name(world.nation),
            defs.building(*b).display_name(world.nation),
        );
        let whose = if *own { "" } else { " (another nation's machines)" };
        out.push(Para::Text(format!(
            "5. Build machines side by side to skip the carrying. A {from} next to a {to}{whose} \
             hands over its {good} directly."
        )));
    }
    out.push(Para::Text(format!(
        "{}. Keep tools in stockpiles. Worn machines wait for one.",
        if chain.is_some() { 6 } else { 5 }
    )));
    if let Some((a, b, good, _)) = chain {
        let (from, to) = (
            defs.building(a).display_name(world.nation),
            defs.building(b).display_name(world.nation),
        );
        out.push(Para::Blank);
        out.push(Para::Demo(demo(&source, a, b, session)));
        out.push(Para::Note(format!(
            "The tint is the {name}'s yard. The {from} touches the {to}, so its {good} goes \
             straight in (the arrow)."
        )));
    }
    out.push(Para::Blank);
    if def.capacity > 0 {
        out.push(Para::Note(format!(
            "A {name}'s yard runs about {} machines before they slow down.",
            def.capacity
        )));
    }
    out.push(Para::Note("O on a machine in a yard turns its automation off or on.".to_owned()));
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
        return "Flow view on: no yard yet. ? then Tab to Machines shows how to set one up".into();
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
        let how: Vec<String> = walkthrough(&s)
            .into_iter()
            .map(|p| match p {
                Para::Heading(t) | Para::Text(t) | Para::Note(t) => t,
                Para::Demo(_) | Para::Blank => String::new(),
            })
            .collect();
        assert!(how.iter().any(|t| t.starts_with("1. Research Great Works (T)")), "{how:?}");
        assert!(how.iter().any(|t| t.contains("Brick Kiln")), "{how:?}");
        assert!(how.iter().any(|t| t.starts_with("2. Build a Lightning Station")), "{how:?}");

        let mut app = crate::app::App::new(s, (120, 40));
        app.act(crate::keys::Action::ToggleFlow);
        assert!(app.ui.notice.contains("Machines"), "{}", app.ui.notice);
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
