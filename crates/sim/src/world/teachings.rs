//! Lost teachings (docs/design/horizons-phase-5.md): great ruins and the pick-1-of-2 draft.
//!
//! - **Great ruins** (#736): `placed` tells a pre-phase-5 save to get its great ruins on load
//!   (`World::place_great_ruins`).
//! - **The draft** (#737): reading a teaching scroll at a study hall (`run_deliver`) draws
//!   `min(reveals, eligible)` teachings from `teachings_rng` and pushes an `TeachingOffer`.
//!   `Command::KeepTeaching` keeps one and loses the rest of its offer. A teaching that is
//!   neither kept nor lost is hidden: `Research::set_target` refuses it (`Facts::kept`,
//!   `Facts::lost`). A pending offer never blocks play.
//!
//! The draw comes only from `teachings_rng`, which nothing else draws from, so harvest order and
//! hauling can't change an offer, only the number of scrolls read before it. The pick is a
//! logged command, so a run log replays the same draft.

use std::collections::{BTreeMap, BTreeSet};

use rand::RngExt;
use serde::{Deserialize, Serialize};

use super::*;
use crate::research::{MILLIS_PER_POINT, SiteRule};

/// The settlement's phase-5 state. Keyed by id, never by position (tech-stack.md, "Saves").
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Teachings {
    /// Whether this world's great ruins have been placed: `false` only in a save from before
    /// phase 5, which `World::load` upgrades.
    #[serde(default)]
    pub(super) placed: bool,
    /// Teachings kept from a draft: ordinary techs from then on.
    #[serde(default)]
    kept: BTreeSet<TechId>,
    /// Teachings lost to a draft, for the rest of the game.
    #[serde(default)]
    lost: BTreeSet<TechId>,
    /// Drafts waiting for a pick, oldest first.
    #[serde(default)]
    offers: Vec<TeachingOffer>,
    /// The study hall each kept teaching's scroll was read at, where it stays shelved until the
    /// teaching is learned ("250 points plus the teaching scroll").
    #[serde(default)]
    shelved: BTreeMap<TechId, Pos>,
}

impl Teachings {
    /// A new world's: mapgen already placed its great ruins.
    pub(super) fn new() -> Self {
        Self { placed: true, ..Self::default() }
    }
}

/// One pending draft: the teachings on offer (in `TechId` order), the study hall the scroll was
/// read at, and the tick it was read.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TeachingOffer {
    pub techs: Vec<TechId>,
    pub hall: Pos,
    pub tick: u64,
}

/// A draft drawn, a teaching kept, or a scroll that had nothing left to offer, as logged in the
/// world's event log (`World::teaching_events`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TeachingEvent {
    pub tick: u64,
    pub kind: TeachingEventKind,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum TeachingEventKind {
    /// A teaching scroll was read at `hall` and offers `techs`.
    Offered { techs: Vec<TechId>, hall: Pos },
    /// `kept` was kept; the rest of its offer, `lost`, is gone.
    Kept { kept: TechId, lost: Vec<TechId> },
    /// A teaching scroll read with nothing left to offer gave `points` knowledge instead
    /// (owner #735).
    Empty { points: u32 },
}

/// `teachings_rng` for a new world (`research.teachings`).
pub(super) fn new_rng(seed: u64) -> SimRng {
    rng::stream(seed, "research.teachings")
}

/// `teachings_rng` for a save from before phase 5: seeded by 0, as `sickness_rng` is for #389.
pub(super) fn default_rng() -> SimRng {
    rng::stream(0, "research.teachings")
}

impl World {
    /// Gives a pre-phase-5 save its great ruins (docs/design/horizons-phase-5.md, "Old saves"):
    /// runs `mapgen::pick_promoted` over the still-buried tiles of each promoted resource's
    /// `from` (in scan order, with the saved map for sizes and the save's own seed) and rewrites
    /// those buried entries. Ruins already uncovered are left alone, since the player has seen
    /// them as they are. Then sets `placed`, so this runs once. Deterministic: the same save
    /// always gets the same ruins.
    pub(crate) fn place_great_ruins(&mut self, defs: &Defs) {
        if self.teachings.placed {
            return;
        }
        for (i, r) in defs.resources.iter().enumerate() {
            let Some(promote) = &r.promote else { continue };
            let Some(from) = defs.resource_id(&promote.from) else { continue };
            let mut candidates: Vec<Pos> =
                self.buried.iter().filter(|&(_, &b)| b == from).map(|(&p, _)| p).collect();
            candidates.sort_by_key(|p| (p.y, p.x));
            for p in mapgen::pick_promoted(&self.map, &candidates, promote, self.seed, &r.id) {
                self.buried.insert(p, ResourceId(i as u16));
            }
        }
        self.teachings.placed = true;
    }

    /// Whether this world's great ruins have been placed (`Teachings::placed`).
    pub fn great_ruins_placed(&self) -> bool {
        self.teachings.placed
    }

    /// Teachings kept from a draft, in `TechId` order.
    pub fn kept_teachings(&self) -> impl Iterator<Item = TechId> + '_ {
        self.teachings.kept.iter().copied()
    }

    /// Teachings lost to a draft, in `TechId` order.
    pub fn lost_teachings(&self) -> impl Iterator<Item = TechId> + '_ {
        self.teachings.lost.iter().copied()
    }

    /// Drafts waiting for a pick, oldest first.
    pub fn teaching_offers(&self) -> &[TeachingOffer] {
        &self.teachings.offers
    }

    /// The study hall where a kept teaching's scroll is shelved, while the teaching isn't known
    /// yet. `None` for any other tech.
    pub fn shelved_at(&self, tech: TechId) -> Option<Pos> {
        if self.research.is_known(tech) {
            return None;
        }
        self.teachings.shelved.get(&tech).copied()
    }

    /// Whether `tech` is a teaching no draft has kept or lost (or one still on offer): the
    /// tree doesn't show it.
    pub fn is_hidden_teaching(&self, tech: TechId, defs: &Defs) -> bool {
        defs.tech(tech).teaching.is_some()
            && !self.teachings.kept.contains(&tech)
            && !self.teachings.lost.contains(&tech)
    }

    /// Teachings a new draft may offer (docs/design/horizons-phase-5.md, "Who is eligible"), in
    /// `TechId` order: open to this nation, not kept, not lost, not on another pending offer,
    /// and their site rule (if any) holds on today's map.
    pub fn eligible_teachings(&self, defs: &Defs) -> Vec<TechId> {
        let offered: BTreeSet<TechId> =
            self.teachings.offers.iter().flat_map(|o| o.techs.iter().copied()).collect();
        defs.techs
            .iter()
            .enumerate()
            .map(|(i, t)| (TechId(i as u16), t))
            .filter(|(id, t)| {
                t.teaching.is_some()
                    && t.open_to(self.nation)
                    && !self.teachings.kept.contains(id)
                    && !self.teachings.lost.contains(id)
                    && !offered.contains(id)
                    && t.teaching
                        .as_ref()
                        .and_then(|x| x.site.as_ref())
                        .is_none_or(|s| self.site_holds(s, defs))
            })
            .map(|(id, _)| id)
            .collect()
    }

    /// Whether a teaching's site rule holds now: at least `min_tiles` tiles within `within`
    /// (Chebyshev) of `settlement_anchor` match its terrains and biomes.
    pub fn site_holds(&self, rule: &SiteRule, defs: &Defs) -> bool {
        self.site_tiles(rule, defs) >= rule.min_tiles
    }

    /// How many tiles match a site rule around the settlement's anchor, counting up to
    /// `min_tiles` (it stops there). For the draft and for a reason line ("12 of 40 desert
    /// tiles").
    pub fn site_tiles(&self, rule: &SiteRule, defs: &Defs) -> u32 {
        let biomes: Option<Vec<crate::defs::BiomeId>> =
            rule.biomes.as_ref().map(|bs| bs.iter().filter_map(|b| defs.biome_id(b)).collect());
        let anchor = self.settlement_anchor();
        let r = rule.within as i32;
        let mut n = 0;
        for dy in -r..=r {
            for dx in -r..=r {
                let Some(t) = self.map.get(Pos::new(anchor.x + dx, anchor.y + dy)) else {
                    continue;
                };
                let terrain_ok = rule.terrains.as_ref().is_none_or(|ts| ts.contains(&t.terrain));
                let biome_ok = biomes.as_ref().is_none_or(|bs| bs.contains(&t.biome));
                if terrain_ok && biome_ok {
                    n += 1;
                    if n >= rule.min_tiles {
                        return n;
                    }
                }
            }
        }
        n
    }

    /// A teaching scroll (`reveals`) was read at the study hall anchored on `hall`
    /// (docs/design/horizons-phase-5.md, "The draw"): draws `min(reveals, eligible)` teachings
    /// without replacement from `teachings_rng` and pushes an offer, or, with nothing eligible,
    /// reads for `empty_scroll_knowledge` points instead. Adds no knowledge otherwise and never
    /// counts toward `scrolls_read`.
    pub(super) fn read_teaching_scroll(&mut self, hall: Pos, reveals: u32, defs: &Defs) {
        let mut pool = self.eligible_teachings(defs);
        let n = (reveals as usize).min(pool.len());
        let tick = self.tick;
        if n == 0 {
            let points = defs.research.empty_scroll_knowledge;
            self.research.add_knowledge(u64::from(points) * MILLIS_PER_POINT);
            self.log_event(TeachingEvent { tick, kind: TeachingEventKind::Empty { points } });
            return;
        }
        let mut techs = Vec::with_capacity(n);
        for _ in 0..n {
            let k = self.teachings_rng.random_range(0..pool.len());
            techs.push(pool.remove(k));
        }
        techs.sort();
        self.teachings.offers.push(TeachingOffer { techs: techs.clone(), hall, tick });
        self.log_event(TeachingEvent { tick, kind: TeachingEventKind::Offered { techs, hall } });
    }

    /// `Command::KeepTeaching`: keeps `tech` from the pending offer it is in, loses the rest of
    /// that offer and removes it. Refused with `NotOffered` if no pending offer holds it.
    pub(super) fn keep_teaching(&mut self, tech: TechId) -> Result<(), ResearchError> {
        let i = self
            .teachings
            .offers
            .iter()
            .position(|o| o.techs.contains(&tech))
            .ok_or(ResearchError::NotOffered)?;
        let offer = self.teachings.offers.remove(i);
        let lost: Vec<TechId> = offer.techs.iter().copied().filter(|&t| t != tech).collect();
        self.teachings.kept.insert(tech);
        self.teachings.shelved.insert(tech, offer.hall);
        self.teachings.lost.extend(lost.iter().copied());
        let tick = self.tick;
        self.log_event(TeachingEvent { tick, kind: TeachingEventKind::Kept { kept: tech, lost } });
        Ok(())
    }

    /// `Facts::kept` and `Facts::lost`.
    pub(super) fn teaching_facts(&self) -> (Vec<TechId>, Vec<TechId>) {
        (
            self.teachings.kept.iter().copied().collect(),
            self.teachings.lost.iter().copied().collect(),
        )
    }
}

#[cfg(test)]
mod tests;
