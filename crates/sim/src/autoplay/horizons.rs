//! The economy opening's use of research horizons phases 1 and 2 (docs/design/horizons-phase-1.md,
//! "Measuring it", #684; docs/design/horizons-phase-2.md, "Measuring it", #700): where the new
//! buildings go, the standing sow order, the one road, the chute's foot, the far outposts, the
//! pairs, the canal and the tunnel line. Like the rest of `Economy`, every choice reads the world
//! fresh, so a step repeated is harmless and two identical worlds get the same commands.
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
//! Phase 2 (each built once):
//!
//! - **The far deposit** (`far_deposit`): the deposit of stone or ore, among those the opening
//!   quarries, farthest from the stockpile tiles. The Waystation, the Wind Gate pair and the
//!   Balloon Dock pair are all about it.
//! - **Waystation** (`raise_waystation`): beside the far deposit, when it is `far_from_stores`
//!   tiles or more from stores (the building's own placement rule).
//! - **Pairs** (`raise_pair`): a Wind Gate (Air) or a Balloon Dock (Fire) at the stockpile's edge
//!   and a partner at the far deposit, both designated together and linked, when the two fall
//!   within the pair rule.
//! - **Stable** and **Badgermole Den**: the ordinary one-of-everything sweep. The Stable stands
//!   at the anchor. The den goes on the rock nearest the Mine Shaft (`site_anchor`).
//! - **Tunnel line** (`bore_line`): once the den's badgermole is housed, the `TUNNEL_TILES` rock
//!   tiles nearest the den's sides, outward through the range, chained so they are bored in order.
//! - **Carve Canal** (`carve_canal`): up to `CANAL_TILES` tiles from the nearest river or lake
//!   toward the farm plots, only when the cut reaches a plot's irrigation radius.
//!
//! The opening's generic bend rotation (`keep_bends_designated`) leaves Carve Canal alone, and
//! Bore Tunnel once a den stands (`own_bends`): both are placed here, and their designations
//! don't count against the bend slots.
//!
//! Buildings are named by id here, as `building_target` names the bed and the farm plot: the
//! ids are what the design doc's rules are about.

use crate::bending::AbilityId;
use crate::building::BuildingId;
use crate::defs::{Defs, Terrain};
use crate::geom::{Footprint, Pos};
use crate::nation::Nation;
use crate::research::Feature;
use crate::world::{Command, PlotStage, World};

use super::{
    DEPOSIT_RADIUS, FORAGE_RADIUS, building_count, deposit_items, have_enough, nearest_build_site,
    yields_any,
};

/// The longest road the opening paves (docs/design/horizons-phase-1.md, "Measuring it").
pub(super) const ROAD_TILES: usize = 30;

/// The longest tunnel line the opening designates from a Badgermole Den (docs/design/
/// horizons-phase-2.md, "Measuring it").
pub(super) const TUNNEL_TILES: usize = 12;

/// The longest canal the opening cuts from the water toward the plots (same section).
pub(super) const CANAL_TILES: usize = 6;

/// How far from the tile it stands beside the opening looks for a site for an outpost or one end
/// of a pair.
const OUTPOST_SEARCH: i32 = 8;

/// The shortest tunnel line worth a den's trouble: a shorter run of rock is left alone.
const MIN_TUNNEL_TILES: usize = 3;

/// Every building the opening raises for `nation`: the endeavor's own set
/// (`endeavor::reachable_buildings`, core techs only) plus those of the optional horizon techs
/// (`TechDef::core` false) open to the nation, which the endeavor doesn't count but the opening
/// still plays (module doc).
pub(super) fn buildable(nation: Nation, defs: &Defs) -> impl Iterator<Item = BuildingId> + '_ {
    defs.buildings.iter().enumerate().filter_map(move |(i, b)| {
        let open = !b.endeavor && b.unlocked_by.is_none_or(|t| defs.tech(t).open_to(nation));
        open.then_some(BuildingId(i as u16))
    })
}

/// The anchor of the nearest finished building or site of `id` to `from`, if there is one.
pub(super) fn nearest_of(world: &World, defs: &Defs, id: &str, from: Pos) -> Option<Pos> {
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
    // Great Works (#731): a new machine goes in a yard that automates it.
    if let Some(yard) = super::schools::yard_anchor(world, defs, id) {
        return yard;
    }
    match id {
        "high_temple" => super::schools::temple_anchor(world, defs, anchor),
        "oil_lamp" => near("bed"),
        "hothouse" => near("farm_plot"),
        "farm_plot" => near("hothouse"),
        "tea_house" => nearest_stockpile(world, anchor),
        "chute" | "badgermole_den" => near("mine_shaft"),
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

/// Whether `horizons` raises `id` itself, so the opening's one-of-everything sweep leaves it
/// alone (`Economy::raise_buildings`): the Waystation stands far from stores and a pair's ends
/// far from each other, neither near the anchor; and the Archive, which goes where the most study
/// halls are in its reach (`trade_techs::raise_archive`, #713).
pub(super) fn raises_itself(id: &str) -> bool {
    matches!(
        id,
        "waystation" | "wind_gate" | "balloon_dock" | "archive" | "ring_gate" | "metal_rail"
    )
}

/// How many of `id` the opening wants standing, for the buildings `raises_itself`: the
/// Waystation when a far deposit is far enough, a pair when one is far enough for the pair rule.
/// Zero otherwise, so the items they cost aren't wanted for nothing (`Economy::wanted_items`).
pub(super) fn outpost_target(world: &World, defs: &Defs, id: &str) -> u32 {
    let Some(def) = defs.building_id(id).map(|b| defs.building(b)) else { return 0 };
    let Some(far) = far_deposit(world, defs, world.settlement_anchor()) else { return 0 };
    if let Some(o) = &def.outpost {
        return u32::from(far.from_stores >= o.far_from_stores as i32);
    }
    match def.pair {
        Some(pair) if far.from_stores >= pair.min_tiles as i32 => 2,
        _ => 0,
    }
}

/// The deposit of stone or ore the opening quarries that lies farthest from the stockpile
/// (module doc, "The far deposit").
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct FarDeposit {
    /// The deposit's tile.
    pub deposit: Pos,
    /// The tile beside it a crew can walk to from the stockpile.
    pub stand: Pos,
    /// Chebyshev distance from the deposit to the nearest stockpile tile.
    pub from_stores: i32,
}

/// The deposit of stone or ore that lies farthest from every stockpile tile (ties to the lower
/// position), among those the opening quarries within `DEPOSIT_RADIUS` of `anchor` and can walk
/// to from the stockpile: the ones still standing, and the ones it has already quarried, which
/// it remembers the way a player would, from its own `DesignateHarvest` orders (`command_log`).
/// By the time these buildings' techs land (minute 125 on), every deposit in reach has usually
/// been quarried out, so the standing ones alone would leave nothing to build beside. A logged
/// order counts when its tile now holds no resource and lies beyond `FORAGE_RADIUS`, where the
/// opening's orders on trees stop. `None` with no stockpile or no such deposit.
pub(super) fn far_deposit(world: &World, defs: &Defs, anchor: Pos) -> Option<FarDeposit> {
    far_deposits(world, defs, anchor).into_iter().next()
}

/// Every deposit `far_deposit` weighs, farthest from stores first (Southern Ways' second
/// Waystation goes at the second, #731).
pub(super) fn far_deposits(world: &World, defs: &Defs, anchor: Pos) -> Vec<FarDeposit> {
    let items = deposit_items(defs);
    let Some(start) = nearest_stockpile(world, anchor) else { return Vec::new() };
    let flood = world.flood_from_anchor(start, defs);
    let zones: Vec<Pos> = world.stockpile_zones().collect();
    let r = DEPOSIT_RADIUS;
    let quarried = world.command_log().iter().filter_map(|rec| match rec.command {
        Command::DesignateHarvest { pos }
            if pos.chebyshev(anchor) > FORAGE_RADIUS
                && world.map.get(pos).is_some_and(|t| t.resource.is_none()) =>
        {
            Some(pos)
        }
        _ => None,
    });
    let standing = (anchor.y - r..=anchor.y + r)
        .flat_map(|y| (anchor.x - r..=anchor.x + r).map(move |x| Pos::new(x, y)))
        .filter(|&p| world.map.get(p).is_some_and(|t| yields_any(t, defs, &items)));
    let deposits: std::collections::BTreeSet<Pos> =
        standing.chain(quarried.filter(|p| p.chebyshev(anchor) <= r)).collect();
    let mut far = deposits
        .into_iter()
        .filter_map(|deposit| {
            let stand = (-1..=1)
                .flat_map(|dy| (-1..=1).map(move |dx| Pos::new(deposit.x + dx, deposit.y + dy)))
                .filter_map(|p| flood.cost(p).map(|c| (c, p)))
                .min()?
                .1;
            let from_stores = zones.iter().map(|z| z.chebyshev(deposit)).min()?;
            Some(FarDeposit { deposit, stand, from_stores })
        })
        .collect::<Vec<_>>();
    far.sort_by_key(|f| (std::cmp::Reverse(f.from_stores), f.deposit));
    far
}

/// Designates one Waystation beside the far deposit, once, when the deposit is as far from
/// stores as the building's own rule asks and the cost is in hand (module doc, "Waystation").
pub(super) fn raise_waystation(world: &mut World, defs: &Defs, anchor: Pos) {
    let Some(id) = defs.building_id("waystation") else { return };
    let def = defs.building(id);
    if def.unlocked_by.is_some_and(|t| !world.research().is_known(t))
        || !buildable(world.nation, defs).any(|b| b == id)
        || building_count(world, id) > 0
        || !have_enough(world, &def.cost)
    {
        return;
    }
    let Some(far) = far_deposit(world, defs, anchor) else { return };
    if def.outpost.as_ref().is_none_or(|o| far.from_stores < o.far_from_stores as i32) {
        return;
    }
    if let Some(pos) = nearest_build_site(world, defs, far.stand, id, OUTPOST_SEARCH) {
        let _ = world.apply(Command::DesignateBuild { pos, building: id }, defs);
    }
}

/// Designates one pair of `id` (a Wind Gate or a Balloon Dock), once: an end at the stockpile's
/// edge nearest the far deposit and its partner beside that deposit, both at once and linked,
/// when the cost of both is in hand and the two ends fall within the pair rule (module doc,
/// "Pairs").
pub(super) fn raise_pair(world: &mut World, defs: &Defs, anchor: Pos, id: &str) {
    let Some(building) = defs.building_id(id) else { return };
    let def = defs.building(building);
    let Some(pair) = def.pair else { return };
    if def.unlocked_by.is_some_and(|t| !world.research().is_known(t))
        || !buildable(world.nation, defs).any(|b| b == building)
        || building_count(world, building) > 0
    {
        return;
    }
    let both: Vec<_> = def
        .cost
        .iter()
        .map(|c| crate::building::CostItem { item: c.item, count: c.count * 2 })
        .collect();
    if !have_enough(world, &both) {
        return;
    }
    let Some(far) = far_deposit(world, defs, anchor) else { return };
    let Some(edge) = world.stockpile_zones().min_by_key(|&z| (z.chebyshev(far.deposit), z)) else {
        return;
    };
    let Some(near_end) = nearest_build_site(world, defs, edge, building, OUTPOST_SEARCH) else {
        return;
    };
    let Some(far_end) = nearest_build_site(world, defs, far.stand, building, OUTPOST_SEARCH) else {
        return;
    };
    let apart = near_end.chebyshev(far_end);
    if apart < pair.min_tiles as i32 || apart > pair.reach as i32 {
        return;
    }
    for pos in [near_end, far_end] {
        let _ = world.apply(Command::DesignateBuild { pos, building }, defs);
    }
    let _ = world.apply(Command::SetLink { pos: near_end, to: far_end }, defs);
}

/// The bends the opening's generic rotation leaves to this module (module doc): Carve Canal
/// always, and Bore Tunnel while a Badgermole Den stands. Their designations don't count against
/// the bend slots either.
pub(super) fn own_bends(world: &World, defs: &Defs) -> Vec<AbilityId> {
    let mut own: Vec<AbilityId> = defs.ability_id("carve_canal").into_iter().collect();
    let den = defs.building_id("badgermole_den");
    if let (Some(bore), Some(den)) = (defs.ability_id("bore_tunnel"), den)
        && world.buildings().any(|b| b.id == den)
    {
        own.push(bore);
    }
    own
}

/// The four ways a line runs, as (dx, dy).
const DIRS: [(i32, i32); 4] = [(0, -1), (1, 0), (0, 1), (-1, 0)];

/// Designates the tunnel line from the Badgermole Den, once its badgermole is housed and has
/// bored nothing: the `TUNNEL_TILES` tiles of clear mountain nearest the den's sides, outward, a
/// layer at a time (module doc, "Tunnel line"). A run shorter than `MIN_TUNNEL_TILES` is left alone.
pub(super) fn bore_line(world: &mut World, defs: &Defs) {
    let (Some(ability), Some(den)) =
        (defs.ability_id("bore_tunnel"), defs.building_id("badgermole_den"))
    else {
        return;
    };
    if world.check_ability(ability, defs).is_err() {
        return;
    }
    let Some(home) = world.buildings().filter(|b| b.id == den).map(|b| b.anchor).min() else {
        return;
    };
    if world.boring_left(home, defs) != Some(0) || world.bored_by(home, defs) != Some(0) {
        return;
    }
    let Some(foot) = world.footprint_at(home) else { return };
    let rock = |p: Pos| {
        world
            .map
            .get(p)
            .filter(|t| t.resource.is_none() && t.building.is_none())
            .map(|t| t.terrain)
            .filter(|t| matches!(t, Terrain::Mountain | Terrain::Tunnel))
    };
    // Outward from the den's sides through rock and the tunnel already bored beside it (the
    // settlers' own rotation leaves some), a layer at a time and in position order, taking the
    // rock still to bore as it comes: each tile touches the den, a tunnel or an earlier tile,
    // so every designation is accepted and worked in order.
    let sides: Vec<Pos> = foot
        .ring()
        .filter(|&p| DIRS.into_iter().any(|(dx, dy)| foot.contains(Pos::new(p.x - dx, p.y - dy))))
        .collect();
    let mut seen: std::collections::BTreeSet<Pos> = sides.iter().copied().collect();
    let mut layer: Vec<Pos> = sides.into_iter().filter(|&p| rock(p).is_some()).collect();
    let mut line: Vec<Pos> = Vec::new();
    while !layer.is_empty() && line.len() < TUNNEL_TILES {
        layer.sort();
        let mut next: Vec<Pos> = Vec::new();
        for &p in &layer {
            if rock(p) == Some(Terrain::Mountain) && line.len() < TUNNEL_TILES {
                line.push(p);
            }
            for (dx, dy) in DIRS {
                let q = Pos::new(p.x + dx, p.y + dy);
                if rock(q).is_some() && seen.insert(q) {
                    next.push(q);
                }
            }
        }
        layer = next;
    }
    if line.len() < MIN_TUNNEL_TILES {
        return;
    }
    for pos in line {
        if world.apply(Command::Bend { ability, pos }, defs).is_err() {
            break;
        }
    }
}

/// Cuts one canal, once: the shortest run of free land, at most `CANAL_TILES` tiles, that starts
/// beside a river or lake and ends within the canal's irrigation radius of a farm plot, so none
/// is cut that waters nothing (module doc, "Carve Canal"). Ties go to the lower position. Does
/// nothing while a canal stands or is designated, or with no plot yet.
pub(super) fn carve_canal(world: &mut World, defs: &Defs) {
    let Some(ability) = defs.ability_id("carve_canal") else { return };
    let Some(plot) = defs.building_id("farm_plot") else { return };
    if world.check_ability(ability, defs).is_err()
        || world.bend_designations().any(|(a, _)| a == ability)
        || world.map.iter().any(|(_, t)| t.terrain == Terrain::Canal)
    {
        return;
    }
    let plots: Vec<Footprint> = world
        .buildings()
        .filter(|b| b.id == plot)
        .filter_map(|b| world.footprint_at(b.anchor))
        .collect();
    if plots.is_empty() {
        return;
    }
    let radius = defs.canal.irrigation_radius as i32;
    let zones: Vec<Pos> = world.stockpile_zones().collect();
    let ground: Vec<Pos> = world.ground_items().map(|(p, _)| p).collect();
    let free = |p: Pos| {
        world.map.get(p).is_some_and(|t| {
            matches!(t.terrain, Terrain::Land | Terrain::Coast)
                && t.resource.is_none()
                && t.building.is_none()
        }) && !zones.contains(&p)
            && !ground.contains(&p)
    };
    let wet = |p: Pos| {
        world.map.get(p).is_some_and(|t| matches!(t.terrain, Terrain::River | Terrain::Lake))
    };
    let steps = |p: Pos| DIRS.map(|(dx, dy)| Pos::new(p.x + dx, p.y + dy));
    let waters = |p: Pos| plots.iter().any(|f| f.distance(p) <= radius);
    // Breadth first from every free tile beside the water, a layer a tile, in position order.
    let mut parent: std::collections::BTreeMap<Pos, Option<Pos>> = Default::default();
    let mut layer: Vec<Pos> = world
        .map
        .iter()
        .filter(|&(p, _)| free(p) && steps(p).into_iter().any(wet))
        .map(|(p, _)| p)
        .collect();
    layer.sort();
    for &p in &layer {
        parent.insert(p, None);
    }
    let mut goal = None;
    for _ in 0..CANAL_TILES {
        goal = layer.iter().copied().find(|&p| waters(p));
        if goal.is_some() {
            break;
        }
        let mut next: Vec<Pos> = Vec::new();
        for &p in &layer {
            for q in steps(p) {
                if free(q) && !parent.contains_key(&q) {
                    parent.insert(q, Some(p));
                    next.push(q);
                }
            }
        }
        next.sort();
        layer = next;
    }
    let Some(mut at) = goal else { return };
    let mut line = vec![at];
    while let Some(Some(from)) = parent.get(&at) {
        line.push(*from);
        at = *from;
    }
    line.reverse();
    for pos in line {
        if world.apply(Command::Bend { ability, pos }, defs).is_err() {
            break;
        }
    }
}

#[cfg(test)]
mod tests;
