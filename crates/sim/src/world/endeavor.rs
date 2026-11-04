//! The endeavor's live state: how far each sign has got, and the Ready latch
//! (`docs/design/endgame.md`, "The road: ten signs"; issue #234). The signs themselves are data
//! (`endeavor.rs`, `data/endeavor.toml`).
//!
//! `endeavor_signs` is a pure query over the world, so a frontend can show progress at any time.
//! Once a second (`TICKS_PER_SECOND` ticks) the world checks the signs itself: each sign met for
//! the first time logs an event, and the first check that finds every sign met at once latches
//! the endeavor to `Ready`. Nothing undoes the latch: stores eaten afterwards don't matter. A
//! second is far finer than anything the signs measure, and it keeps the map-wide building scan
//! off every tick. After Ready there is nothing left to check (every sign has been met).
//!
//! Signs are remembered by string id, never by index, so a reordered `data/endeavor.toml` still
//! reads an old save correctly.
//!
//! After Ready, placing the site and building the gate in stages is `gate.rs` (#236).

use std::collections::BTreeSet;

use crate::building::BuildingId;
use crate::endeavor::{SignDef, SignKind, Tally};
use crate::jobs::ItemId;

use super::gate::GateSite;
use super::*;

/// Where the end game stands. The Avatar and the rite add more later.
#[derive(Debug, Clone, PartialEq, Default, serde::Serialize, serde::Deserialize)]
pub enum EndeavorState {
    /// Not every sign has been met at once yet.
    #[default]
    Dormant,
    /// Every sign was met at once on `tick`; the player may choose a site.
    Ready { tick: u64 },
    /// The site is placed and the gate is going up, a stage at a time (`gate.rs`).
    Building(GateSite),
    /// The last stage was done on `tick`: the gate stands, a finished building on its site.
    Complete { tick: u64 },
}

/// A sign's progress: `value` of `target`, and whether it is met now.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SignProgress<'d> {
    pub id: &'d str,
    pub value: u32,
    pub target: u32,
    pub met: bool,
}

/// Something that happened to the endeavor, for the client's toasts.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct EndeavorEvent {
    pub tick: u64,
    pub kind: EndeavorEventKind,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum EndeavorEventKind {
    /// The sign with this id was met for the first time.
    SignMet(String),
    /// Every sign was met at once: the endeavor is Ready.
    Ready,
    /// The stage with this id was finished (`gate.rs`).
    StageDone(String),
    /// The Avatar landed on Appa (`world/avatar.rs`, #237).
    AvatarArrived,
    /// The Avatar left, unfed or exhausted, for the reason given.
    AvatarLeft(AvatarLeaveReason),
    /// The Avatar came back.
    AvatarReturned,
}

/// Why the Avatar left (`world/avatar.rs`, #237): named for the client's toast ("Aang has gone
/// to find food. He'll be back.").
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum AvatarLeaveReason {
    Hungry,
    Tired,
}

/// What the world remembers of the endeavor between checks. Its own struct so a save carries it
/// as one field.
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub(super) struct EndeavorLog {
    /// Ids of the signs that have ever been met.
    met: BTreeSet<String>,
    /// Every event so far, oldest first.
    events: Vec<EndeavorEvent>,
}

impl World {
    pub fn endeavor(&self) -> &EndeavorState {
        &self.endeavor
    }

    /// Every event so far, oldest first.
    pub fn endeavor_events(&self) -> &[EndeavorEvent] {
        &self.endeavor_log.events
    }

    /// Each sign in `data/endeavor.toml` with its progress right now, in file order.
    pub fn endeavor_signs<'d>(&self, defs: &'d Defs) -> Vec<SignProgress<'d>> {
        defs.endeavor.signs.iter().map(|s| self.sign_progress(s, defs)).collect()
    }

    fn sign_progress<'d>(&self, sign: &'d SignDef, defs: &Defs) -> SignProgress<'d> {
        let population = self.units.len() as u32;
        let stored = |items: &[ItemId]| items.iter().map(|&i| self.stored_total(i)).sum::<u32>();
        let (value, target) = match &sign.kind {
            SignKind::Population => (population, None),
            SignKind::BedsForAll => {
                let beds = self.finished_buildings().filter(|&b| defs.building(b).bed.is_some());
                (beds.count() as u32, Some(population))
            }
            SignKind::BuildingKinds => {
                let kinds: BTreeSet<BuildingId> = self.finished_buildings().collect();
                (kinds.len() as u32, None)
            }
            SignKind::BranchComplete => {
                let own = |t: &TechId| defs.tech(*t).nation == Some(self.nation);
                let known = self.research.known_techs().filter(own).count() as u32;
                let branch = defs.techs.iter().filter(|t| t.nation == Some(self.nation)).count();
                (known, Some(branch as u32))
            }
            SignKind::Stored { items } => (stored(items), None),
            SignKind::Tally(Tally::Harvested(items)) => {
                (items.iter().map(|&i| self.tallies.harvested(i)).sum(), None)
            }
            SignKind::Tally(Tally::ScrollsRead) => (self.tallies.scrolls_read(), None),
            SignKind::Masters { bends } => {
                (self.units.iter().filter(|u| u.bends >= *bends).count() as u32, None)
            }
        };
        let target = target.or(sign.target_for(self.nation)).unwrap_or(0);
        // A computed target of 0 (no people, an empty branch) never counts as met.
        SignProgress { id: &sign.id, value, target, met: target > 0 && value >= target }
    }

    /// The id of every finished building, once per building however many tiles it covers. Reads
    /// `building_index` (#292), maintained wherever a building finishes (`World::raise`,
    /// `build.rs`), rather than `buildings()`'s full map scan: this runs once a second for as
    /// long as the `BedsForAll` and `BuildingKinds` signs below are unmet (`tick_endeavor`),
    /// essentially the whole game, so a bigger map mustn't cost more to check.
    fn finished_buildings(&self) -> impl Iterator<Item = BuildingId> + '_ {
        self.building_index.values().filter_map(|&p| self.map[p].building.map(|b| b.id))
    }

    /// The once-a-second check: log signs met for the first time, and latch Ready when all are
    /// met at once. Does nothing without signs or once Ready.
    pub(super) fn tick_endeavor(&mut self, defs: &Defs) {
        if !self.tick.is_multiple_of(u64::from(TICKS_PER_SECOND))
            || defs.endeavor.signs.is_empty()
            || self.endeavor != EndeavorState::Dormant
        {
            return;
        }
        let tick = self.tick;
        let progress = self.endeavor_signs(defs);
        for p in progress.iter().filter(|p| p.met) {
            if self.endeavor_log.met.insert(p.id.to_owned()) {
                let kind = EndeavorEventKind::SignMet(p.id.to_owned());
                self.endeavor_log.events.push(EndeavorEvent { tick, kind });
            }
        }
        if progress.iter().all(|p| p.met) {
            self.endeavor = EndeavorState::Ready { tick };
            self.endeavor_log.events.push(EndeavorEvent { tick, kind: EndeavorEventKind::Ready });
        }
    }

    /// Logs an endeavor event at the current tick.
    pub(super) fn endeavor_event(&mut self, kind: EndeavorEventKind) {
        self.endeavor_log.events.push(EndeavorEvent { tick: self.tick, kind });
    }
}

#[cfg(test)]
mod tests;
