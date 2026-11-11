//! The economy opening's use of research horizons phase 3 (docs/design/horizons-phase-3.md,
//! "Measuring it", #713): the Tea House, the wares Workshop and the Archive. The caravan half
//! (the wares and scroll copy rows) is in `pacing`'s deals. Like the rest of `Economy`, every
//! choice reads the world fresh, so a step repeated is harmless and two identical worlds get the
//! same commands.
//!
//! - **Tea House** (`tea_house_target`, and `horizons::site_anchor`'s arm): one, beside the
//!   stockpile, once `tea_ceremony` is known and an Herb Garden stands: a house with no garden has
//!   nothing to burn.
//! - **Wares** (`workshop_target`, `set_workshop_recipes`): once the Workshop's wares recipe is
//!   unlocked, one more Workshop, and every Workshop but one set to wares. The one stays on tools
//!   (the Archive costs tools). A settlement whose tools are made elsewhere (Fire's forge) keeps
//!   none: every Workshop makes wares. That is read from the recipes (another finished building
//!   makes the Workshop's own first output), not from the nation. Wares are made only while wood
//!   stands over what the buildings still wanted cost: a Workshop goes back to tools when it
//!   doesn't (#713's first run found wares drain the wood the Archive and beds need).
//! - **Archive** (`archive_target`, `raise_archive`): once `archives` is known and a study hall
//!   stands, on the tile that puts the most study halls in its reach, ties to the nearest the
//!   settlement's anchor. With no free tile in reach of any hall it stands as near the first hall
//!   as it fits (#713's first run: in a crowded core that was most runs, and an Archive that
//!   never stands scribes nothing).
//! - **Far wood** (`gather_far_wood`): the wood within `FORAGE_RADIUS` is finite, and by the time
//!   these techs land it is usually gone (#713's first run: the Tea House and the Archive each cost
//!   wood the settlement no longer had, and stood in about one run in four). While one of them is
//!   wanted and the wood for it isn't in hand, the opening sends crews to the nearest trees past
//!   `FORAGE_RADIUS`, as many as the shortfall takes. Nothing before these techs changes.
//!
//! Clearing the research target is never scripted: scribing starts when the opening runs out of
//! techs, which is the case for a run whose tree is done.

use crate::defs::Defs;
use crate::geom::{Footprint, Pos};
use crate::world::{Command, World};

use super::{
    BUILD_SEARCH_RADIUS, DEPOSIT_RADIUS, FORAGE_RADIUS, HERB_GARDEN, building_count, have_enough,
    nearby, nearest_build_site, yields_any,
};

/// How far past a study hall's own edge `archive_site` looks for a tile, beyond the Archive's
/// reach: its own footprint.
const SITE_MARGIN: i32 = 2;

/// 1 once `tea_ceremony`'s house can be raised (module doc): an Herb Garden is finished, else 0.
pub(super) fn tea_house_target(world: &World, defs: &Defs) -> u32 {
    let Some(garden) = defs.building_id(HERB_GARDEN) else { return 0 };
    u32::from(world.buildings().any(|b| b.id == garden))
}

/// The Workshops wanted: 1, and one more once an alt recipe of the Workshop is unlocked by a tech
/// that is known (the wares: module doc).
pub(super) fn workshop_target(world: &World, defs: &Defs) -> u32 {
    1 + u32::from(unlocked_alt(world, defs).is_some())
}

/// The index of the Workshop's first alt recipe whose tech is known, `Command::SetRecipe`'s
/// numbering.
fn unlocked_alt(world: &World, defs: &Defs) -> Option<u8> {
    let workshop = defs.building(defs.building_id("workshop")?);
    workshop
        .alt_recipes
        .iter()
        .position(|a| a.unlocked_by.is_some_and(|t| world.research().is_known(t)))
        .map(|i| i as u8)
}

/// Wood stores must hold this much over what the buildings still wanted cost before a Workshop is
/// set to wares: a ware is two wood, and the wood the opening can gather is finite (what grows
/// within `FORAGE_RADIUS`), so a player doesn't turn the beds' and the Archive's wood into wares.
const WARES_MARGIN: u32 = 30;

/// Sets every finished Workshop but `keep` of them to the unlocked alt recipe while wood allows
/// it, and back to tools when it doesn't (module doc, "Wares"). `wood_wanted` is the wood the
/// buildings still wanted cost (`Economy::wood_wanted`): a Workshop goes to wares only with
/// `WARES_MARGIN` over it in stores, and comes back to tools once stores fall under it, so the
/// wares never starve a building. The `keep` first Workshops in position order stay on tools.
pub(super) fn set_workshop_recipes(world: &mut World, defs: &Defs, wood_wanted: u32) {
    let Some(workshop) = defs.building_id("workshop") else { return };
    let Some(alt) = unlocked_alt(world, defs) else { return };
    let def = defs.building(workshop);
    let Some(tools) = def.recipe.as_ref().map(|r| r.output.item) else { return };
    // Tools made elsewhere (a forge) leave no Workshop on tools.
    let elsewhere = world.buildings().any(|b| {
        b.id != workshop
            && defs.building(b.id).recipe.as_ref().is_some_and(|r| r.output.item == tools)
    });
    let keep = usize::from(!elsewhere);
    let wood = defs.item_id("wood").map_or(0, |w| world.stored_total(w));
    let mut shops: Vec<Pos> =
        world.buildings().filter(|b| b.id == workshop).map(|b| b.anchor).collect();
    shops.sort();
    for (i, &pos) in shops.iter().enumerate() {
        let on_wares = world.recipe_choice_at(pos, defs) == Some(alt);
        let want = if i < keep {
            false
        } else if on_wares {
            wood >= wood_wanted
        } else {
            wood >= wood_wanted + WARES_MARGIN
        };
        if want != on_wares {
            let recipe = want.then_some(alt);
            let _ = world.apply(Command::SetRecipe { pos, recipe }, defs);
        }
    }
}

/// The footprints of the study halls (standing or being built) the Archive adds seats to.
fn halls(world: &World, defs: &Defs) -> Vec<Footprint> {
    let Some(hall) =
        defs.building_id("archive").and_then(|a| defs.building(a).archive).map(|a| a.hall)
    else {
        return Vec::new();
    };
    let mut all: Vec<Footprint> = world
        .buildings()
        .filter(|b| b.id == hall)
        .filter_map(|b| world.footprint_at(b.anchor))
        .chain(world.build_sites().filter(|&(_, id, _)| id == hall).map(|(f, _, _)| f))
        .collect();
    all.sort_by_key(|f| f.anchor);
    all
}

/// The Archives wanted: 1 once a study hall stands (or is on its way), else 0, so the items it
/// costs aren't wanted for nothing (`Economy::wanted_items`).
pub(super) fn archive_target(world: &World, defs: &Defs) -> u32 {
    u32::from(!halls(world, defs).is_empty())
}

/// The tile for the Archive: the buildable one that has the most study halls within its reach,
/// ties to the nearest `anchor`, then the lower position. `None` if no buildable tile reaches one.
fn archive_site(world: &World, defs: &Defs, anchor: Pos) -> Option<Pos> {
    let id = defs.building_id("archive")?;
    let reach = defs.building(id).archive?.radius as i32;
    let (w, h) = defs.building(id).footprint;
    let halls = halls(world, defs);
    let mut tiles: std::collections::BTreeSet<Pos> = Default::default();
    for hall in &halls {
        let m = reach + SITE_MARGIN;
        for y in hall.anchor.y - m..hall.anchor.y + i32::from(hall.h) + m {
            for x in hall.anchor.x - m..hall.anchor.x + i32::from(hall.w) + m {
                tiles.insert(Pos::new(x, y));
            }
        }
    }
    let mut scored: Vec<(usize, Pos)> = tiles
        .into_iter()
        .filter_map(|p| {
            let foot = Footprint::new(p, w, h);
            let n = halls.iter().filter(|hall| hall.gap(foot) <= reach).count();
            (n > 0).then_some((n, p))
        })
        .collect();
    scored.sort_by_key(|&(n, p)| {
        (std::cmp::Reverse(n), (p.x - anchor.x).pow(2) + (p.y - anchor.y).pow(2), p)
    });
    scored.into_iter().map(|(_, p)| p).find(|&p| {
        world.check_build(p, id, defs).is_ok()
            && world.to_clear(p, id, defs).is_empty()
            && nearest_build_site(world, defs, p, id, 0) == Some(p)
    })
}

/// Designates one Archive, once, when `archives` is known, a study hall stands and the cost is in
/// hand (module doc, "Archive").
pub(super) fn raise_archive(world: &mut World, defs: &Defs, anchor: Pos) {
    let Some(id) = defs.building_id("archive") else { return };
    let def = defs.building(id);
    if def.unlocked_by.is_some_and(|t| !world.research().is_known(t))
        || building_count(world, id) > 0
        || !have_enough(world, &def.cost)
    {
        return;
    }
    // With no free tile in reach of any hall (the core is crowded by minute 160 or so: stockpile
    // tiles, buildings, rock) it still stands as near the first hall as it fits: scribing needs an
    // Archive, not seats.
    let site = archive_site(world, defs, anchor).or_else(|| {
        let hall = halls(world, defs).first().map(|f| f.anchor)?;
        nearest_build_site(world, defs, hall, id, BUILD_SEARCH_RADIUS)
    });
    if let Some(pos) = site {
        let _ = world.apply(Command::DesignateBuild { pos, building: id }, defs);
    }
}

/// Wood a tree yields, the divisor for how many trees a shortfall takes (`data/jobs.toml`: 3).
/// Read from the data, not named here, so a change there needs none.
fn wood_per_tree(defs: &Defs, wood: crate::jobs::ItemId) -> u32 {
    defs.harvests.iter().filter(|h| h.item == wood).map(|h| h.count).min().unwrap_or(1).max(1)
}

/// Designates the nearest trees past `FORAGE_RADIUS` (within `DEPOSIT_RADIUS`), one-shot, as many
/// as the wood the Tea House and the Archive still lack takes (module doc, "Far wood"), until each
/// is finished. The same
/// nearest trees are designated again each step until they are cut, which is harmless.
pub(super) fn gather_far_wood(world: &mut World, defs: &Defs, anchor: Pos) {
    let Some(wood) = defs.item_id("wood") else { return };
    let have = world.stored_total(wood)
        + world.ground_items().filter(|(_, s)| s.item == wood).map(|(_, s)| s.count).sum::<u32>();
    let mut lacking = 0;
    for (id, target) in
        [("tea_house", tea_house_target(world, defs)), ("archive", archive_target(world, defs))]
    {
        let Some(b) = defs.building_id(id) else { continue };
        let def = defs.building(b);
        if def.unlocked_by.is_some_and(|t| !world.research().is_known(t))
            || world.buildings().filter(|f| f.id == b).count() as u32 >= target
        {
            continue;
        }
        // A site that is designated but unfinished still lacks its wood when stores are empty: the
        // first run found Archive sites waiting for wood the beds had taken.
        let need: u32 = def.cost.iter().filter(|c| c.item == wood).map(|c| c.count).sum();
        lacking += need;
    }
    let lacking = lacking.saturating_sub(have);
    if lacking == 0 {
        return;
    }
    let trees = lacking.div_ceil(wood_per_tree(defs, wood)) as usize;
    let mut far: Vec<Pos> =
        nearby(anchor, DEPOSIT_RADIUS, world, defs, |t, defs| yields_any(t, defs, &[wood]))
            .into_iter()
            .filter(|p| p.chebyshev(anchor) > FORAGE_RADIUS)
            .collect();
    // Only trees a crew can walk up to from the stockpile: an unreachable one would sit among
    // the nearest, designated and never cut.
    let Some(start) = world.stockpile_zones().min_by_key(|&p| (p.chebyshev(anchor), p)) else {
        return;
    };
    let flood = world.flood_from_anchor(start, defs);
    far.retain(|p| {
        (-1..=1).any(|dy| (-1..=1).any(|dx| flood.cost(Pos::new(p.x + dx, p.y + dy)).is_some()))
    });
    far.sort_by_key(|&p| (p.chebyshev(anchor), p));
    for pos in far.into_iter().take(trees) {
        let _ = world.apply(Command::DesignateHarvest { pos }, defs);
    }
}
