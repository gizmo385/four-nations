//! The economy opening's use of research horizons phase 1 (docs/design/horizons-phase-1.md,
//! "Measuring it", #684): where the new buildings go, the standing sow order, the one road and
//! the chute's foot. Like the rest of `Economy`, every choice reads the world fresh, so a step
//! repeated is harmless and two identical worlds get the same commands.
//!
//! - **Calendar.** Once the standing sow order is known (`Feature::StandingSow`), every plot gets
//!   one and nothing is sown by hand (`sow_plot`).
//! - **Paving.** Once, the path from the stockpile to the deposit quarried most is paved, up to
//!   `ROAD_TILES` tiles, when the stone for all of it is in hand (`pave_once`).
//! - **Placement** (`site_anchor`): the Oil Lamp beside the beds; the Lemur Roost between the
//!   orchard and the stockpile when one tile reaches both, else beside the orchard; the Hothouse
//!   beside the farm plots, and new farm plots beside the Hothouse; the chute beside the Mine
//!   Shaft, only where the ground falls far enough within reach (`chute_wanted`).
//! - **Which buildings** (`buildable`): the optional horizon buildings too, which the
//!   endeavor's own count leaves out.
//! - **The chute's foot** (`link_chutes`): the valid foot nearest the stockpile.
//!
//! Buildings are named by id here, as `building_target` names the bed and the farm plot: the
//! ids are what the design doc's rules are about.

use crate::building::BuildingId;
use crate::defs::Defs;
use crate::geom::Pos;
use crate::nation::Nation;
use crate::research::Feature;
use crate::world::{Command, PlotStage, World};

use super::{DEPOSIT_RADIUS, building_count, deposit_items, yields_any};

/// The longest road the opening paves (docs/design/horizons-phase-1.md, "Measuring it").
pub(super) const ROAD_TILES: usize = 30;

/// Every building the opening raises for `nation`: the endeavor's own set
/// (`endeavor::reachable_buildings`, core techs only) plus those of the optional horizon techs
/// (`TechDef::core` false) open to the nation, which the endeavor doesn't count but the opening
/// still plays (module doc).
pub(super) fn buildable(nation: Nation, defs: &Defs) -> impl Iterator<Item = BuildingId> + '_ {
    defs.buildings.iter().enumerate().filter_map(move |(i, b)| {
        let open = !b.endeavor
            && b.unlocked_by.is_none_or(|t| defs.tech(t).nation.is_none_or(|n| n == nation));
        open.then_some(BuildingId(i as u16))
    })
}

/// The anchor of the nearest finished building or site of `id` to `from`, if there is one.
fn nearest_of(world: &World, defs: &Defs, id: &str, from: Pos) -> Option<Pos> {
    let building = defs.building_id(id)?;
    world
        .buildings()
        .filter(|b| b.id == building)
        .map(|b| b.anchor)
        .chain(world.build_sites().filter(|&(_, b, _)| b == building).map(|(f, _, _)| f.anchor))
        .min_by_key(|&p| (p.chebyshev(from), p))
}

/// The stockpile tile nearest `from`.
fn nearest_stockpile(world: &World, from: Pos) -> Option<Pos> {
    world.stockpile_zones().min_by_key(|&p| (p.chebyshev(from), p))
}

/// Where the opening looks for a site for `id`, nearest first, instead of the settlement's own
/// anchor (module doc, "Placement"). `anchor` where the rules say nothing.
pub(super) fn site_anchor(world: &World, defs: &Defs, id: &str, anchor: Pos) -> Pos {
    let near = |of: &str| nearest_of(world, defs, of, anchor);
    match id {
        "oil_lamp" => near("bed"),
        "hothouse" => near("farm_plot"),
        "farm_plot" => near("hothouse"),
        "chute" => near("mine_shaft"),
        "lemur_roost" => near("orchard").map(|orchard| {
            let reach = defs
                .building(defs.building_id(id).expect("named by the caller"))
                .pickers
                .as_ref()
                .map_or(0, |p| p.radius as i32);
            match nearest_stockpile(world, orchard) {
                Some(s) if s.chebyshev(orchard) <= 2 * reach => {
                    Pos::new((s.x + orchard.x) / 2, (s.y + orchard.y) / 2)
                }
                _ => orchard,
            }
        }),
        _ => None,
    }
    .unwrap_or(anchor)
}

/// Whether a chute is worth cutting: a Mine Shaft stands, and some clear tile within the chute's
/// reach of the shaft lies its `drop` below the shaft (module doc, "Placement"). The head goes
/// beside the shaft, so this is the drop that decides whether a foot exists.
pub(super) fn chute_wanted(world: &World, defs: &Defs) -> bool {
    let Some(chute) = defs.building_id("chute").and_then(|c| defs.building(c).chute) else {
        return false;
    };
    let Some(shaft) = nearest_of(world, defs, "mine_shaft", world.settlement_anchor()) else {
        return false;
    };
    let Some(top) = world.map.get(shaft).map(|t| t.elevation) else { return false };
    let r = chute.reach as i32;
    (shaft.y - r..=shaft.y + r)
        .flat_map(|y| (shaft.x - r..=shaft.x + r).map(move |x| Pos::new(x, y)))
        .filter(|&p| world.build_blocker(p, defs).is_none())
        .filter_map(|p| world.map.get(p))
        .any(|t| {
            t.resource.is_none() && u16::from(t.elevation) + u16::from(chute.drop) <= u16::from(top)
        })
}

/// Points every finished chute with no foot at the valid foot nearest the stockpile (module doc).
pub(super) fn link_chutes(world: &mut World, defs: &Defs) {
    let heads: Vec<Pos> = world
        .buildings()
        .filter(|b| b.link.is_none() && defs.building(b.id).chute.is_some())
        .map(|b| b.anchor)
        .collect();
    for head in heads {
        let Some(reach) = defs.building(world.building_at(head).expect("listed above")).chute
        else {
            continue;
        };
        let r = reach.reach as i32;
        let zones: Vec<Pos> = world.stockpile_zones().collect();
        let foot = (head.y - r..=head.y + r)
            .flat_map(|y| (head.x - r..=head.x + r).map(move |x| Pos::new(x, y)))
            .filter(|&p| world.check_foot(head, p, defs).is_ok())
            .min_by_key(|&p| (zones.iter().map(|&z| z.chebyshev(p)).min().unwrap_or(i32::MAX), p));
        if let Some(to) = foot {
            let _ = world.apply(Command::SetLink { pos: head, to }, defs);
        }
    }
}

/// Sows the empty plot at `pos`, or once the standing sow order is known gives it one instead
/// (module doc, "Calendar"). A plot that already has one is left alone.
pub(super) fn sow_plot(world: &mut World, defs: &Defs, pos: Pos) {
    if world.is_standing_sow(pos) {
        return;
    }
    if world.has_feature(Feature::StandingSow, defs)
        && world.apply(Command::StandingSow { pos, on: true }, defs).is_ok()
    {
        return;
    }
    if world.plot_stage(pos) == Some(PlotStage::Empty) {
        let _ = world.apply(Command::Sow { pos }, defs);
    }
}

/// Paves the path from the stockpile to the deposit quarried most, once (module doc, "Paving"):
/// only when `paving` is known, no road stands or is under way, and the stone for every tile is
/// in stores, so the road is never left half paid for.
pub(super) fn pave_once(world: &mut World, defs: &Defs, anchor: Pos) {
    let Some(road) = defs.building_id("paved_road") else { return };
    let def = defs.building(road);
    if def.unlocked_by.is_some_and(|t| !world.research().is_known(t))
        || building_count(world, road) > 0
    {
        return;
    }
    let Some(path) = road_path(world, defs, anchor) else { return };
    let tiles: Vec<Pos> =
        path.into_iter().filter(|&p| world.check_build(p, road, defs).is_ok()).collect();
    if tiles.is_empty()
        || !def.cost.iter().all(|c| world.stored_total(c.item) >= c.count * tiles.len() as u32)
    {
        return;
    }
    for pos in tiles {
        let _ = world.apply(Command::DesignateBuild { pos, building: road }, defs);
    }
}

/// The first `ROAD_TILES` tiles of the cheapest walk from the stockpile tile nearest `anchor` to
/// a tile beside the nearest reachable deposit or a building that makes the item from nothing (within `DEPOSIT_RADIUS` of the anchor, where the
/// opening quarries) of the item quarried most so far.
fn road_path(world: &World, defs: &Defs, anchor: Pos) -> Option<Vec<Pos>> {
    let item = deposit_items(defs)
        .into_iter()
        .max_by_key(|&i| (world.tallies().harvested(i), std::cmp::Reverse(i)))?;
    let start = nearest_stockpile(world, anchor)?;
    let flood = world.flood_from_anchor(start, defs);
    // A building that makes the item from nothing (Earth's Mine Shaft) counts as a deposit: by
    // the time `paving` is known, the surface deposits in reach are often quarried out.
    let sources: Vec<Pos> = world
        .buildings()
        .filter(|b| {
            defs.building(b.id)
                .recipe
                .as_ref()
                .is_some_and(|r| r.output.item == item && r.inputs.is_empty())
        })
        .filter_map(|b| world.footprint_at(b.anchor))
        .flat_map(|f| f.tiles())
        .collect();
    let r = DEPOSIT_RADIUS;
    let goal = (anchor.y - r..=anchor.y + r)
        .flat_map(|y| (anchor.x - r..=anchor.x + r).map(move |x| Pos::new(x, y)))
        .filter(|&p| world.map.get(p).is_some_and(|t| yields_any(t, defs, &[item])))
        .chain(sources)
        .flat_map(|deposit| {
            (-1..=1).flat_map(move |dy| {
                (-1..=1).map(move |dx| Pos::new(deposit.x + dx, deposit.y + dy))
            })
        })
        .filter_map(|p| flood.cost(p).map(|c| (c, p)))
        .min()?
        .1;
    let mut path = flood.path_to(goal)?;
    path.truncate(ROAD_TILES);
    Some(path)
}

#[cfg(test)]
mod tests;
