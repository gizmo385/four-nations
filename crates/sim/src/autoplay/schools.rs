//! The economy opening's use of the phase-4 schools (docs/design/horizons-phase-4.md, "Measuring
//! it", #731): what it does with each school once it is learned. Like the rest of `Economy`,
//! every choice reads the world fresh, so a step repeated is harmless.
//!
//! - **Agrarian Ring** (`ring_farms`): one ring of Earth Wall round the farm plots, the smallest
//!   rectangle with a one-tile margin, at most `RING_TILES` wall tiles, with a Ring Gate in the
//!   middle of the side facing the stockpile. Plots scattered among stockpiles and water rarely
//!   fit one ring, so it rings the group of plots that shelters most; failing any, an open field
//!   near the anchor (`open_field`), sown with `FIELD_PLOTS` plots. The rectangle grows a tile at a time, up to
//!   `RING_PADS`, until every tile round it can take a wall; each step fills any gap left.
//! - **Metal City** (`metal_line`): one metal rail line from the Hot Spring Mill to the nearest
//!   powered building out of the mill's own reach (a Scroll Library or Workshop first), the
//!   shortest run of free tiles, at most `LINE_TILES` long.
//! - **Sun Warriors** (`sun_fire`): one more campfire beside the deposit quarried most.
//! - **Great Works** (`yard_anchor`): new machines go in a Lightning Station's yard.
//! - **Northern City** (`wanted`): ice is wanted while fewer than `ICE_KEPT` are in stores, so
//!   the bend rotation keeps cutting it.
//! - **Southern Ways** (`southern_camp`): a second Waystation at the second-farthest deposit, and
//!   `SHORE_ORDERS` standing shore Fish orders beside each Waystation.
//! - **High Temple**: placed against the rock nearest the study halls (`horizons::site_anchor`).
//! - **Nomad Way** (`nomad_move`, owner decision #892, option A): farm plots never tire in the
//!   sim, so the doc's farm move never triggers. Instead, each spring, the building farthest
//!   from the settlement's anchor, where the first stockpile went down (`MOVE_FROM` tiles or
//!   more), is packed up, and the usual building sweep raises it again near the stores with
//!   what came back. Panel settlements are compact (their multi-tile buildings sit 6 tiles or
//!   less from the anchor), so the distance is short. That exercises what Nomad Way
//!   buys: Pack Up's full refund and quick work.

use crate::building::BuildingId;
use crate::defs::Defs;
use crate::geom::{Footprint, Pos};
use crate::jobs::ItemId;
use crate::research::Feature;
use crate::world::{Command, World};

use super::horizons::{far_deposits, nearest_of};
use super::{
    DEPOSIT_RADIUS, building_count, deposit_items, have_enough, nearest_build_site, yields_any,
};

/// The longest ring the opening raises, in wall tiles.
pub(super) const RING_TILES: usize = 120;
/// The longest metal rail line it lays.
pub(super) const LINE_TILES: usize = 30;
/// The ice Northern City keeps in stores.
pub(super) const ICE_KEPT: u32 = 40;
/// Standing shore Fish orders beside each Waystation.
pub(super) const SHORE_ORDERS: usize = 4;
/// How far from a Waystation those orders go.
const SHORE_REACH: i32 = 6;
/// How far out from the plots the ring may go, in tiles (2 is the one-tile margin).
const RING_PADS: i32 = 5;
/// The outer size of a ring round an open field (`open_field`): 18 wall tiles round 12 inside.
const FIELD: (i32, i32) = (6, 5);
/// How far from the anchor an open field is looked for.
const FIELD_SEARCH: i32 = 20;
/// Farm plots sown inside a ring that has none.
const FIELD_PLOTS: usize = 4;
/// Walls whose bill is in hand before the ring is begun.
const RING_START: u32 = 10;
/// How far from the study hall the High Temple's rock is looked for.
const TEMPLE_SEARCH: i32 = 40;
/// How far from its anchor a school building's site is looked for.
const SEARCH: i32 = 8;
/// Nomad Way packs up a building only this far or farther from the settlement's anchor.
pub(super) const MOVE_FROM: i32 = 5;

fn known(world: &World, defs: &Defs, tech: &str) -> bool {
    defs.tech_id(tech).is_some_and(|t| world.research().is_known(t))
}

/// Every finished building and site of `building`, by its footprint.
fn standing(world: &World, building: BuildingId) -> Vec<Footprint> {
    world
        .buildings()
        .filter(|b| b.id == building)
        .map(|b| b.footprint())
        .chain(world.build_sites().filter(|&(_, b, _)| b == building).map(|(f, _, _)| f))
        .collect()
}

/// The ring's two buildings: the wall and the gate (`[enclosure] walls`, the passable one).
fn ring_buildings(defs: &Defs) -> Option<(BuildingId, BuildingId)> {
    let walls = &defs.seasons.enclosure.as_ref()?.walls;
    let wall = walls.iter().copied().find(|&w| !defs.building(w).passable)?;
    let gate = walls.iter().copied().find(|&w| defs.building(w).passable)?;
    Some((wall, gate))
}

/// The tiles round the rectangle from `lo` to `hi`, both corners included, each once.
fn perimeter(lo: Pos, hi: Pos) -> Vec<Pos> {
    let mut out = Vec::new();
    for x in lo.x..=hi.x {
        out.push(Pos::new(x, lo.y));
        if hi.y != lo.y {
            out.push(Pos::new(x, hi.y));
        }
    }
    for y in lo.y + 1..hi.y {
        out.push(Pos::new(lo.x, y));
        if hi.x != lo.x {
            out.push(Pos::new(hi.x, y));
        }
    }
    out
}

/// Whether a building or a site covers `pos`.
fn placed(world: &World, pos: Pos) -> bool {
    world.building_at(pos).is_some() || world.build_sites().any(|(f, _, _)| f.contains(pos))
}

/// The ring's rectangle: from the walls already raised or sited when there are some (a lone wall
/// the sweep raised before the school is left out: it has no wall beside it), else the smallest
/// rectangle round the farm plots, from a one-tile margin out, whose every tile can take a wall.
fn ring_rect(world: &World, defs: &Defs, wall: BuildingId, gate: BuildingId) -> Option<(Pos, Pos)> {
    let tiles: Vec<Pos> =
        standing(world, wall).into_iter().chain(standing(world, gate)).map(|f| f.anchor).collect();
    let linked: Vec<Pos> = tiles
        .iter()
        .copied()
        .filter(|p| {
            [(1, 0), (-1, 0), (0, 1), (0, -1)]
                .iter()
                .any(|&(dx, dy)| tiles.contains(&Pos::new(p.x + dx, p.y + dy)))
        })
        .collect();
    let bound = |ps: &[Pos], pad: i32| -> Option<(Pos, Pos)> {
        let lo = Pos::new(ps.iter().map(|p| p.x).min()? - pad, ps.iter().map(|p| p.y).min()? - pad);
        let hi = Pos::new(ps.iter().map(|p| p.x).max()? + pad, ps.iter().map(|p| p.y).max()? + pad);
        Some((lo, hi))
    };
    if !linked.is_empty() {
        return bound(&linked, 0);
    }
    let plot = defs.building_id("farm_plot")?;
    let plots = standing(world, plot);
    // Plots scattered among stockpiles and water rarely fit one ring, so each plot in turn seeds
    // a group (the plots within a reach of it); the ring that shelters most plots wins, the
    // shorter on a tie.
    let mut best: Option<(usize, usize, (Pos, Pos))> = None;
    for seed in &plots {
        for reach in [0, 6, 12, 24, i32::MAX] {
            let group: Vec<Pos> = plots
                .iter()
                .filter(|f| f.anchor.chebyshev(seed.anchor) <= reach)
                .flat_map(|f| f.tiles())
                .collect();
            let count = plots.iter().filter(|f| f.anchor.chebyshev(seed.anchor) <= reach).count();
            if best.is_some_and(|(c, _, _)| c > count) {
                continue;
            }
            let fits = (2..=RING_PADS).filter_map(|pad| bound(&group, pad)).find(|&(lo, hi)| {
                let ring = perimeter(lo, hi);
                ring.len() <= RING_TILES
                    && ring.iter().all(|&p| world.check_build(p, wall, defs).is_ok())
            });
            if let Some(rect) = fits {
                let len = perimeter(rect.0, rect.1).len();
                if best.is_none_or(|(c, l, _)| count > c || (count == c && len < l)) {
                    best = Some((count, len, rect));
                }
            }
        }
    }
    best.map(|(_, _, rect)| rect).or_else(|| open_field(world, defs, wall, plot))
}

/// The open field nearest the settlement anchor that a ring of `FIELD` outer size fits round,
/// every tile of it free to build on: the ring when no ring fits the plots standing (they sit
/// among stockpiles, water and buildings on most maps). `ring_farms` sows its inside.
fn open_field(
    world: &World,
    defs: &Defs,
    wall: BuildingId,
    plot: BuildingId,
) -> Option<(Pos, Pos)> {
    let anchor = world.settlement_anchor();
    let (w, h) = FIELD;
    let mut corners: Vec<Pos> = (anchor.y - FIELD_SEARCH..=anchor.y + FIELD_SEARCH)
        .flat_map(|y| {
            (anchor.x - FIELD_SEARCH..=anchor.x + FIELD_SEARCH).map(move |x| Pos::new(x, y))
        })
        .collect();
    let centre = |lo: Pos| Pos::new(lo.x + w / 2, lo.y + h / 2);
    corners.sort_by_key(|&lo| (centre(lo).chebyshev(anchor), lo));
    corners.into_iter().map(|lo| (lo, Pos::new(lo.x + w - 1, lo.y + h - 1))).find(|&(lo, hi)| {
        perimeter(lo, hi).iter().all(|&p| world.check_build(p, wall, defs).is_ok())
            && inside(lo, hi).all(|p| world.check_build(p, plot, defs).is_ok())
    })
}

/// The tiles strictly inside the rectangle from `lo` to `hi`.
fn inside(lo: Pos, hi: Pos) -> impl Iterator<Item = Pos> {
    (lo.y + 1..hi.y).flat_map(move |y| (lo.x + 1..hi.x).map(move |x| Pos::new(x, y)))
}

/// Agrarian Ring's ring round the farm plots (module doc).
pub(super) fn ring_farms(world: &mut World, defs: &Defs) {
    if !world.has_feature(Feature::ShelteredRings, defs) {
        return;
    }
    let Some((wall, gate)) = ring_buildings(defs) else { return };
    let begun = building_count(world, gate) > 0;
    if !begun {
        let bill: Vec<_> = defs
            .building(wall)
            .cost
            .iter()
            .map(|c| crate::building::CostItem { item: c.item, count: c.count * RING_START })
            .collect();
        if !have_enough(world, &bill) {
            return;
        }
    }
    let Some((lo, hi)) = ring_rect(world, defs, wall, gate) else { return };
    let ring = perimeter(lo, hi);
    if ring.len() > RING_TILES {
        return;
    }
    // The gate: the middle of the side facing the stockpile.
    let centre = Pos::new((lo.x + hi.x) / 2, (lo.y + hi.y) / 2);
    let toward = world.stockpile_zones().min_by_key(|&z| (z.chebyshev(centre), z));
    let gate_at = toward.map(|s| {
        let (dx, dy) = (s.x - centre.x, s.y - centre.y);
        if dx.abs() >= dy.abs() {
            Pos::new(if dx >= 0 { hi.x } else { lo.x }, centre.y)
        } else {
            Pos::new(centre.x, if dy >= 0 { hi.y } else { lo.y })
        }
    });
    for pos in ring {
        let building = if Some(pos) == gate_at && !begun { gate } else { wall };
        if !placed(world, pos) && world.check_build(pos, building, defs).is_ok() {
            let _ = world.apply(Command::DesignateBuild { pos, building }, defs);
        }
    }
    // A ring round an open field gets plots sown inside it.
    let Some(plot) = defs.building_id("farm_plot") else { return };
    if inside(lo, hi).any(|p| placed(world, p)) {
        return;
    }
    let spots: Vec<Pos> = inside(lo, hi)
        .filter(|&p| world.check_build(p, plot, defs).is_ok())
        .take(FIELD_PLOTS)
        .collect();
    for pos in spots {
        let _ = world.apply(Command::DesignateBuild { pos, building: plot }, defs);
    }
}

/// The shortest 4-connected run of `free` tiles, at most `LINE_TILES` long, from beside `from`
/// to a tile within `reach` of `to`.
fn rail_path(
    from: Footprint,
    to: Footprint,
    reach: i32,
    free: &impl Fn(Pos) -> bool,
) -> Option<Vec<Pos>> {
    use std::collections::{BTreeMap, VecDeque};
    let tiles: Vec<Pos> = from.tiles().collect();
    let step = [(1, 0), (-1, 0), (0, 1), (0, -1)];
    let mut came: BTreeMap<Pos, Option<Pos>> = BTreeMap::new();
    let mut queue = VecDeque::new();
    for t in &tiles {
        for (dx, dy) in step {
            let p = Pos::new(t.x + dx, t.y + dy);
            if !tiles.contains(&p) && free(p) && !came.contains_key(&p) {
                came.insert(p, None);
                queue.push_back((p, 1usize));
            }
        }
    }
    while let Some((p, len)) = queue.pop_front() {
        if to.gap(Footprint::single(p)) <= reach {
            let mut line = vec![p];
            let mut at = p;
            while let Some(&Some(prev)) = came.get(&at) {
                line.push(prev);
                at = prev;
            }
            return Some(line);
        }
        if len >= LINE_TILES {
            continue;
        }
        for (dx, dy) in step {
            let q = Pos::new(p.x + dx, p.y + dy);
            if !came.contains_key(&q) && !to.contains(q) && free(q) {
                came.insert(q, Some(p));
                queue.push_back((q, len + 1));
            }
        }
    }
    None
}

/// Metal City's line from the Hot Spring Mill (module doc).
pub(super) fn metal_line(world: &mut World, defs: &Defs) {
    if !known(world, defs, "metal_city") {
        return;
    }
    let (Some(rail), Some(mill)) =
        (defs.building_id("metal_rail"), defs.building_id("hot_spring_mill"))
    else {
        return;
    };
    if building_count(world, rail) > 0 {
        return;
    }
    let Some(source) = world.buildings().find(|b| b.id == mill).map(|b| b.footprint()) else {
        return;
    };
    // The nearest finished building that runs on power and sits out of the mill's own reach,
    // a Scroll Library or Workshop first; the line is the shortest run of free tiles from beside
    // the mill to within the rail's power reach of it.
    let reach =
        defs.building(rail).track.as_ref().and_then(|t| t.power_reach).map_or(1, |r| r as i32);
    let own = defs.building(mill).power.as_ref().map_or(0, |p| p.radius as i32);
    let ends: Vec<BuildingId> =
        ["study_hall", "workshop"].iter().filter_map(|id| defs.building_id(id)).collect();
    let mut targets: Vec<(bool, i32, Pos, Footprint)> = world
        .buildings()
        .filter(|b| defs.building(b.id).uses_power && b.footprint().gap(source) > own)
        .map(|b| (!ends.contains(&b.id), b.footprint().gap(source), b.anchor, b.footprint()))
        .collect();
    targets.sort_by_key(|&(other, gap, anchor, _)| (other, gap, anchor));
    let free = |p: Pos| world.check_build(p, rail, defs).is_ok();
    let Some(line) = targets.iter().find_map(|&(_, _, _, t)| rail_path(source, t, reach, &free))
    else {
        return;
    };
    let bill: Vec<_> = defs
        .building(rail)
        .cost
        .iter()
        .map(|c| crate::building::CostItem { item: c.item, count: c.count * line.len() as u32 })
        .collect();
    if !have_enough(world, &bill) {
        return;
    }
    for pos in line {
        let _ = world.apply(Command::DesignateBuild { pos, building: rail }, defs);
    }
}

/// Sun Warriors' extra campfire beside the deposit quarried most (module doc).
pub(super) fn sun_fire(world: &mut World, defs: &Defs, anchor: Pos, target: u32) {
    if !known(world, defs, "sun_warriors") {
        return;
    }
    let Some(fire) = defs.building_id("campfire") else { return };
    if building_count(world, fire) > target || !have_enough(world, &defs.building(fire).cost) {
        return;
    }
    let Some(item) = deposit_items(defs)
        .into_iter()
        .max_by_key(|&i| (world.tallies().harvested(i), std::cmp::Reverse(i)))
    else {
        return;
    };
    let r = DEPOSIT_RADIUS;
    let Some(deposit) = (anchor.y - r..=anchor.y + r)
        .flat_map(|y| (anchor.x - r..=anchor.x + r).map(move |x| Pos::new(x, y)))
        .filter(|&p| world.map.get(p).is_some_and(|t| yields_any(t, defs, &[item])))
        .min_by_key(|&p| (p.chebyshev(anchor), p))
    else {
        return;
    };
    if let Some(pos) = nearest_build_site(world, defs, deposit, fire, SEARCH) {
        let _ = world.apply(Command::DesignateBuild { pos, building: fire }, defs);
    }
}

/// Where Great Works puts a new machine (module doc): beside the Lightning Station whose yard
/// automates it, once the tech is known. `None` for a building no yard would automate.
pub(super) fn yard_anchor(world: &World, defs: &Defs, id: &str) -> Option<Pos> {
    let def = defs.building(defs.building_id(id)?);
    if def.recipe.is_none() || !def.uses_power {
        return None;
    }
    world
        .buildings()
        .filter(|b| {
            defs.building(b.id).automate_upgrade.is_some_and(|(t, _)| world.research().is_known(t))
        })
        .map(|b| b.anchor)
        .min_by_key(|&p| (p.chebyshev(world.settlement_anchor()), p))
}

/// What the schools want made (module doc): ice while Northern City is known and fewer than
/// `ICE_KEPT` are in stores, and the metal rail's bill while Metal City's line isn't laid.
pub(super) fn wanted(world: &World, defs: &Defs) -> Vec<ItemId> {
    let mut out = Vec::new();
    if let Some(ice) = defs.item_id("ice")
        && known(world, defs, "northern_city")
        && world.stored_total(ice) < ICE_KEPT
    {
        out.push(ice);
    }
    if let Some(rail) = defs.building_id("metal_rail")
        && known(world, defs, "metal_city")
        && building_count(world, rail) == 0
    {
        out.extend(defs.building(rail).cost.iter().map(|c| c.item));
    }
    out
}

/// Southern Ways' second Waystation and the shore orders beside each (module doc).
pub(super) fn southern_camp(world: &mut World, defs: &Defs, anchor: Pos) {
    if !known(world, defs, "southern_ways") {
        return;
    }
    let Some(camp) = defs.building_id("waystation") else { return };
    let camps = standing(world, camp);
    if camps.len() == 1 && have_enough(world, &defs.building(camp).cost) {
        let far = far_deposits(world, defs, anchor);
        let rule = defs.building(camp).outpost.as_ref().map_or(0, |o| o.far_from_stores as i32);
        if let Some(second) = far.get(1).filter(|d| d.from_stores >= rule)
            && let Some(pos) = nearest_build_site(world, defs, second.stand, camp, SEARCH)
        {
            let _ = world.apply(Command::DesignateBuild { pos, building: camp }, defs);
        }
    }
    let finished: Vec<Pos> = world.buildings().filter(|b| b.id == camp).map(|b| b.anchor).collect();
    for at in finished {
        let r = SHORE_REACH;
        let mut shore: Vec<Pos> = (at.y - r..=at.y + r)
            .flat_map(|y| (at.x - r..=at.x + r).map(move |x| Pos::new(x, y)))
            .filter(|&p| {
                world.map.get(p).is_some_and(|t| t.resource.is_none())
                    && world.harvest_at(p, defs).is_some()
            })
            .collect();
        shore.sort_by_key(|&p| (p.chebyshev(at), p));
        let (ordered, open): (Vec<Pos>, Vec<Pos>) =
            shore.into_iter().partition(|&p| world.is_standing_harvest(p));
        let mut ordered = ordered.len();
        for pos in open {
            if ordered >= SHORE_ORDERS {
                break;
            }
            if world.apply(Command::DesignateRepeatingHarvest { pos }, defs).is_ok() {
                ordered += 1;
            }
        }
    }
}

/// Where the High Temple's site is looked for (module doc): the anchor nearest the study hall it
/// can stand on, against the rock.
pub(super) fn temple_anchor(world: &World, defs: &Defs, anchor: Pos) -> Option<Pos> {
    let hall = nearest_of(world, defs, "study_hall", anchor).unwrap_or(anchor);
    let Some(temple) = defs.building_id("high_temple") else { return Some(hall) };
    let unlocked = defs.building(temple).unlocked_by.is_none_or(|t| world.research().is_known(t));
    if !unlocked
        || building_count(world, temple) > 0
        || !have_enough(world, &defs.building(temple).cost)
    {
        return Some(hall);
    }
    // The rock is rarely within the opening's usual search of the hall, so the spot is found
    // here: the nearest anchor the temple can stand on.
    let r = TEMPLE_SEARCH;
    let mut spots: Vec<Pos> = (hall.y - r..=hall.y + r)
        .flat_map(|y| (hall.x - r..=hall.x + r).map(move |x| Pos::new(x, y)))
        .collect();
    spots.sort_by_key(|&p| (p.chebyshev(hall), p));
    spots.into_iter().find(|&p| world.check_build(p, temple, defs).is_ok()).or(Some(hall))
}

/// Nomad Way's spring move (module doc; owner decision #892): once Pack Up is known, in spring,
/// and only if nothing has been packed up since spring began, takes down the multi-tile
/// building farthest from the settlement's anchor, if it is at least `MOVE_FROM` away. Beds,
/// plots, outposts and pairs (which `horizons` places where they are for a reason), landmarks
/// and the endeavor's buildings stay. `Economy::raise_buildings` rebuilds it from the anchor,
/// which is where the stockpile is.
pub(super) fn nomad_move(world: &mut World, defs: &Defs) {
    use crate::world::SeasonEventKind;
    if !world.has_feature(Feature::PackUp, defs) || world.season_index(defs) != 0 {
        return;
    }
    let Some(spring) = world
        .season_events()
        .filter(|e| matches!(e.kind, SeasonEventKind::Changed { season: 0 }))
        .map(|e| e.tick)
        .next_back()
    else {
        return;
    };
    let packed = |c: &Command| matches!(c, Command::Deconstruct { .. });
    if world.command_log().iter().any(|r| r.tick >= spring && packed(&r.command)) {
        return;
    }
    // The stores grow into a wide field (dozens of zone tiles round the buildings), so the
    // distance is to the settlement's anchor, where the first stockpile went down.
    let anchor = world.settlement_anchor();
    let far = world
        .buildings()
        .filter(|b| {
            let def = defs.building(b.id);
            let (w, h) = def.footprint;
            u32::from(w) * u32::from(h) > 1
                && !def.landmark
                && !def.endeavor
                && !def.cost.is_empty()
                && !super::horizons::raises_itself(&def.id)
                && def.id != "farm_plot"
        })
        .map(|b| (anchor.chebyshev(b.anchor), b.anchor))
        .filter(|&(d, _)| d >= MOVE_FROM)
        .max_by_key(|&(d, p)| (d, std::cmp::Reverse(p)));
    if let Some((_, pos)) = far {
        let _ = world.apply(Command::Deconstruct { pos }, defs);
    }
}
