//! The site profile (`docs/design/balance-harness.md`, section 1; #534): what a (seed, nation)
//! start offers, measured from the supply survey before anyone plays. It is how a miss on a poor
//! site is told from a miss on a good one: a sign whose material isn't in reach was never going to
//! be met, whoever plays.

use std::collections::BTreeMap;

use crate::autoplay::{DEPOSIT_RADIUS, FORAGE_RADIUS, sign_items};
use crate::bending::Effect;
use crate::defs::{Defs, ResourceId, Terrain};
use crate::embark::Survey;
use crate::geom::Pos;
use crate::nation::Nation;

/// The signs whose material the map has to supply, by `data/endeavor.toml` id: what the stores
/// and harvest detectors (`probe`) watch, and what `SiteProfile::supply` counts.
pub(super) const MATERIAL_SIGNS: [&str; 2] = ["stone", "ore"];

/// How far from the anchor "cramped" looks: the room a settler has to move and build in.
pub const OPEN_RADIUS: i32 = 8;

/// Bore Tunnel casts deep a nation that can tunnel is assumed to reach for buried material: the
/// supply survey's own default (`balance --supply --bore-limit`).
pub const BORE_LIMIT: u32 = 2;

/// What a start offers a nation. Computed from the survey, so it's the same for every style and
/// costs nothing per run.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct SiteProfile {
    /// The start tile.
    pub anchor: (i32, i32),
    /// Yield of each material sign's deposits (`MATERIAL_SIGNS`) in reach within `DEPOSIT_RADIUS`
    /// tiles, by sign id, counting what the nation's tunnelling reaches (`tunnels`). Stone and
    /// ore: whether the sign's target (`data/endeavor.toml`) is within what the map holds.
    pub supply: BTreeMap<String, u32>,
    /// Whether the nation can bore tunnels, so `supply` counted buried material.
    pub tunnels: bool,
    /// Walking steps to the nearest fresh water (a river, lake or ford), `None` beyond 40.
    pub fresh_water_steps: Option<u32>,
    /// Wild food resources in reach within `FORAGE_RADIUS` tiles.
    pub food_sources: u32,
    /// Open ground a settler can walk to within `OPEN_RADIUS` tiles of the anchor: a low number
    /// means a cramped start, where something built can seal people off (#490).
    pub open_tiles: u32,
}

impl SiteProfile {
    /// The profile of `nation` starting at `start` on `survey`.
    pub fn compute(survey: &Survey, start: Pos, nation: Nation, defs: &Defs) -> Self {
        let tunnels = can_tunnel(nation, defs);
        let bore_limit = if tunnels { BORE_LIMIT } else { 0 };
        let mut supply = BTreeMap::new();
        for sign in MATERIAL_SIGNS {
            let resources = sign_resources(defs, sign);
            let worth = survey
                .supply(start, &resources, &[DEPOSIT_RADIUS], bore_limit, defs)
                .first()
                .map_or(0, |r| r.deposits.iter().map(|d| d.worth).sum());
            supply.insert(sign.to_owned(), worth);
        }
        let food: Vec<ResourceId> = defs
            .harvests
            .iter()
            .filter(|h| defs.food_of(h.item).is_some())
            .map(|h| h.resource)
            .collect();
        Self {
            anchor: (start.x, start.y),
            supply,
            tunnels,
            fresh_water_steps: survey.fresh_water_steps(start),
            food_sources: survey.count_resources(start, FORAGE_RADIUS, &food),
            open_tiles: survey.open_tiles_near(start, OPEN_RADIUS),
        }
    }

    /// The stone in reach (`supply["stone"]`).
    pub fn stone(&self) -> u32 {
        self.supply.get("stone").copied().unwrap_or(0)
    }

    /// The ore in reach (`supply["ore"]`).
    pub fn ore(&self) -> u32 {
        self.supply.get("ore").copied().unwrap_or(0)
    }
}

/// Whether `nation` has a bend that turns mountain into tunnel (Earth's Bore Tunnel) among the
/// techs it can research, so buried material counts for it.
fn can_tunnel(nation: Nation, defs: &Defs) -> bool {
    defs.abilities.iter().any(|a| {
        a.of_nation(nation)
            && matches!(&a.effect, Effect::Reshape { to: Terrain::Tunnel, .. })
            && a.unlocked_by.is_none_or(|t| defs.techs_for(nation).any(|(id, _)| id == t))
    })
}

/// The resources whose harvest yields one of sign `id`'s items: the deposits that feed it.
pub(super) fn sign_resources(defs: &Defs, id: &str) -> Vec<ResourceId> {
    let items = sign_items(defs, id);
    defs.harvests.iter().filter(|h| items.contains(&h.item)).map(|h| h.resource).collect()
}
