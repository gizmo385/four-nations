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

/// A piece of the How to tab ([`walkthrough`]).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Para {
    Heading(String),
    /// Wrapped to the page.
    Text(String),
    /// Wrapped, dimmer: a side note.
    Note(String),
    /// A line of the example layout: never wrapped, drawn as is in a fixed-width font.
    Art(String),
    Blank,
}

/// Two machines this city can build where the first makes what the second uses (a Smelter and a
/// Forge), as names and the good, and whether they are the city's own: the example for touching
/// machines. Any nation's pair if the city has none of its own yet.
fn chain(session: &Session) -> (String, String, String, bool) {
    let (world, defs) = (&session.world, &session.defs);
    let machines = |own: bool| {
        defs.buildings
            .iter()
            .filter(|b| b.uses_power && b.recipe.is_some() && !b.endeavor)
            .filter(move |b| !own || crate::build::available_to_nation(session, b))
            .collect::<Vec<_>>()
    };
    for own in [true, false] {
        let list = machines(own);
        for a in &list {
            let made = a.recipe.as_ref().map(|r| r.output.item);
            for b in &list {
                let Some(r) = b.recipe.as_ref() else { continue };
                if a.id != b.id && r.inputs.iter().any(|c| Some(c.item) == made) {
                    let good = made.map_or_else(String::new, |i| defs.item(i).name.to_lowercase());
                    let name =
                        |d: &fn_sim::building::BuildingDef| d.display_name(world.nation).to_owned();
                    return (name(a), name(b), good, own);
                }
            }
        }
    }
    ("Smelter".into(), "Forge".into(), "metal".into(), false)
}

/// The key help's How to tab (playtest, 2026-11-17: "I had a brick kiln and couldn't figure out
/// what to do"; then "phrased incredibly strangely ... include an example layout"): how to make
/// machines run on their own, in this city's own names (its yard source, the tech that raises
/// it, its machines), with a drawn layout of a yard and two touching machines.
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
        1 => format!("like the {}", machines[0]),
        _ => format!("{}...", machines.join(", ")),
    };
    let mut out = vec![
        Para::Heading("Machines that run on their own".to_owned()),
        Para::Text(format!(
            "A machine ({examples}) normally needs a settler inside. In a yard it runs by itself."
        )),
        Para::Blank,
    ];
    let Some((name, tech, known, def)) = source(session) else {
        out.push(Para::Note("Nothing your city can research raises a yard yet.".to_owned()));
        return out;
    };
    let done = if known { " (done)" } else { "" };
    out.push(Para::Text(format!("1. Research {tech} (T){done}.")));
    let mut build =
        format!("2. Build a {name}. Its yard is the tinted area around it when Y is on.");
    if def.beside.iter().any(|t| t.is_water()) {
        build.push_str(" Machines in it must touch water.");
    } else if def.beside.contains(&fn_sim::defs::Terrain::Mountain) {
        build.push_str(" Machines in it must touch rock.");
    }
    out.push(Para::Text(build));
    out.push(Para::Text(format!(
        "3. Build machines in the yard. They switch over once built, at {}% speed.",
        (def.rate * 100.0).round() as u32
    )));
    out.push(Para::Text(
        "4. Keep their inputs in stockpiles. Settlers carry goods in and out.".to_owned(),
    ));
    let (from, to, good, own) = chain(session);
    let whose = if own { "" } else { " (another nation's machines)" };
    out.push(Para::Text(format!(
        "5. Build machines side by side to skip the carrying. A {from} next to a {to}{whose} \
         hands over its {good} directly."
    )));
    out.push(Para::Text("6. Keep tools in stockpiles. Worn machines wait for one.".to_owned()));
    out.push(Para::Blank);
    // The example: the source, and the two machines touching inside its tint.
    let a = format!("[{from}]");
    let b = format!("[{to}]");
    let src = format!("[{name}]");
    let inner = (src.chars().count()).max(a.chars().count() + 1 + b.chars().count()) + 4;
    let edge = format!("  {}", ".".repeat(inner + 4));
    let row = |text: String| format!("  .  {text:<inner$}.");
    out.push(Para::Art(edge.clone()));
    out.push(Para::Art(row(src)));
    out.push(Para::Art(row(String::new())));
    out.push(Para::Art(row(format!("{a}>{b}"))));
    out.push(Para::Art(edge));
    out.push(Para::Note(format!(
        "The dots are the {name}'s yard. The {from} touches the {to}, so its {good} goes \
         straight in (the > arrow with Y on)."
    )));
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
        let how: Vec<String> = walkthrough(&s)
            .into_iter()
            .map(|p| match p {
                Para::Heading(t) | Para::Text(t) | Para::Note(t) | Para::Art(t) => t,
                Para::Blank => String::new(),
            })
            .collect();
        assert!(how.iter().any(|t| t.starts_with("1. Research Great Works (T)")), "{how:?}");
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
