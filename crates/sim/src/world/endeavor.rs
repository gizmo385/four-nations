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

use crate::bending::Rank;
use crate::building::BuildingId;
use crate::endeavor::{self, ChapterDef, SignDef, SignKind, Tally};
use crate::jobs::ItemId;
use crate::research::TechDef;

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
    /// Every sign of the chapter with this id was met at once, for the first time (#439).
    ChapterMet(String),
    /// Every sign was met at once: the endeavor is Ready.
    Ready,
    /// The stage with this id was finished (`gate.rs`).
    StageDone(String),
    /// The stage task with this id (unique across every stage) was finished: its bend is done
    /// (`gate.rs`, #1009). Once per task. The score credits a stage's done tasks before the stage
    /// itself is done.
    TaskDone(String),
    /// The Avatar landed on Appa (`world/avatar.rs`, #237).
    AvatarArrived,
    /// The Avatar left, unfed or exhausted, for the reason given.
    AvatarLeft(AvatarLeaveReason),
    /// The Avatar came back.
    AvatarReturned,
    /// The Harmony Rite's progress paused: attendance fell short of its share (`world/gate.rs`,
    /// #238).
    RitePaused,
    /// A settler's death broke the Harmony Rite: its progress reset to nothing (`world/gate.rs`,
    /// #238).
    RiteBroken,
    /// The United City's population reached this milestone (`data/endeavor.toml`'s
    /// `[[milestone]]`, #442): once each, in order, after the gate stands.
    PopulationMilestone(u32),
    /// The last stage finished: the gate stands and the endeavor is `Complete` (`world/gate.rs`,
    /// #236, #238).
    Complete,
    /// The Avatar's farewell: he leaves the settlement for good, `AvatarDef::farewell_minutes`
    /// after the Harmony Rite completed (`world/avatar.rs`, #238).
    AvatarFarewell,
    /// This unit's bends and forms first reached `rank` (`World::rank_of`,
    /// `docs/design/bending.md`, "The route: novice, adept, master", #360), named here (not
    /// just by id) the way `Death` and `Arrival` carry their own name, so the toast reads
    /// correctly even if the unit later dies. Fires once per rank reached, at Adept and at
    /// Master (never at Novice, the rank everyone starts at); `Unit`'s own `rank_credited` is
    /// what makes this once-only, so a save/load, or `[mastery]` retuned later, never refires
    /// it for a rank already reached. `title` is what this unit's own nation called it at
    /// `rank` at the moment it was earned (`data/bending.toml`'s `[mastery.titles]`), resolved
    /// and stored here rather than this unit's nation: looking the title up again later would
    /// need `World::nation_of`, which needs the unit still alive, exactly the gap `name` is
    /// already carried directly to avoid (#361, docs/design/bending.md "Seeing it": "Jee is now
    /// a Firebender").
    RankUp { unit: UnitId, name: String, rank: Rank, title: String },
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
    /// Each chapter met so far (#439), by id, with the tick it was met on.
    #[serde(default)]
    chapters: BTreeMap<String, u64>,
    /// The highest United City population milestone that has fired (#442), 0 for none: the next
    /// one is the first above it, so each fires once whatever the data says later.
    #[serde(default)]
    milestone: u32,
    /// A v1 to v3 save's endeavor events (#605): merged into `World::events` on load, never
    /// written.
    #[serde(default, skip_serializing, rename = "events")]
    v3_events: Vec<EndeavorEvent>,
}

impl EndeavorLog {
    /// Ids of the chapters met so far, in id order.
    pub(super) fn chapters_met(&self) -> impl Iterator<Item = &str> {
        self.chapters.keys().map(String::as_str)
    }

    /// Whether a pre-v4 save's events are still waiting for `World::upgrade_from_v3`.
    pub(super) fn has_v3_events(&self) -> bool {
        !self.v3_events.is_empty()
    }

    /// A pre-v4 save's events, leaving none behind.
    pub(super) fn take_v3_events(&mut self) -> Vec<EndeavorEvent> {
        std::mem::take(&mut self.v3_events)
    }
}

impl World {
    pub fn endeavor(&self) -> &EndeavorState {
        &self.endeavor
    }

    /// Each sign in `data/endeavor.toml` with its progress right now, in file order.
    pub fn endeavor_signs<'d>(&self, defs: &'d Defs) -> Vec<SignProgress<'d>> {
        defs.endeavor.signs.iter().map(|s| self.sign_progress(s, defs)).collect()
    }

    fn sign_progress<'d>(&self, sign: &'d SignDef, defs: &Defs) -> SignProgress<'d> {
        let population = self.population() as u32;
        let stored = |items: &[ItemId]| items.iter().map(|&i| self.stored_total(i)).sum::<u32>();
        let (value, target) = match &sign.kind {
            SignKind::Population => (population, None),
            SignKind::BedsForAll => {
                let beds = self.finished_buildings().filter(|&b| defs.building(b).bed.is_some());
                (beds.count() as u32, Some(population))
            }
            SignKind::EveryBuilding { spare } => {
                let reachable: Vec<BuildingId> =
                    endeavor::reachable_buildings(self.nation, &defs.techs, &defs.buildings)
                        .collect();
                let finished: BTreeSet<BuildingId> = self.finished_buildings().collect();
                let value = reachable.iter().filter(|b| finished.contains(b)).count() as u32;
                // `spare` is validated at load time to be below every nation's own count, so
                // this never underflows. A sign with a target in the data (a chapter's "four
                // kinds", #439) uses that instead.
                let computed = (sign.target_for(self.nation).is_none())
                    .then(|| reachable.len() as u32 - spare);
                (value, computed)
            }
            SignKind::BranchComplete => {
                // Only core techs count, so optional techs never lengthen the gate
                // (docs/design/horizons-phase-1.md, `core`).
                let counted = |t: &TechDef| t.core && t.nation == Some(self.nation);
                let known = self.research.known_techs().filter(|&t| counted(defs.tech(t))).count();
                let branch = defs.techs.iter().filter(|t| counted(t)).count();
                (known as u32, Some(branch as u32))
            }
            SignKind::Stored { items } => (stored(items), None),
            SignKind::Tally(Tally::Harvested(items)) => {
                (items.iter().map(|&i| self.tallies.harvested(i)).sum(), None)
            }
            SignKind::Tally(Tally::ScrollsRead) => (self.tallies.scrolls_read(), None),
            SignKind::Masters => {
                let count = self
                    .units
                    .iter()
                    .filter(|u| self.rank_of(u, defs).rank == Rank::Master)
                    .count() as u32;
                (count, None)
            }
        };
        let target = target.or(sign.target_for(self.nation)).unwrap_or(0);
        // A computed target of 0 (no people, an empty branch) never counts as met.
        SignProgress { id: &sign.id, value, target, met: target > 0 && value >= target }
    }

    /// Whether the Harmony Gate stands: the settlement is the United City (#240, #442).
    pub fn united(&self) -> bool {
        matches!(self.endeavor, EndeavorState::Complete { .. })
    }

    /// The settlement's own people: every living unit but the Avatar and the beasts. What the
    /// United City's milestones count (#442).
    pub fn settlers(&self) -> usize {
        self.residents().count()
    }

    /// The next population milestone of the United City (#442): the first in the data above the
    /// last one that fired. `None` when the data has no more. Meaningful once [`united`](Self::united).
    pub fn next_milestone(&self, defs: &Defs) -> Option<u32> {
        defs.endeavor.milestones.next_after(self.endeavor_log.milestone)
    }

    /// The id of every finished building, once per building however many tiles it covers. Walks
    /// the building records (`World::buildings`, #604), never the map: this runs once a second
    /// for as long as the `BedsForAll` and `EveryBuilding` signs below are unmet
    /// (`tick_endeavor`), essentially the whole game, so a bigger map mustn't cost more to check.
    fn finished_buildings(&self) -> impl Iterator<Item = BuildingId> + '_ {
        self.buildings().map(|b| b.id)
    }

    /// Each sign of `chapter` with its progress right now, in its own order (#439).
    pub fn chapter_signs<'d>(&self, chapter: &'d ChapterDef, defs: &Defs) -> Vec<SignProgress<'d>> {
        chapter.signs.iter().map(|s| self.sign_progress(s, defs)).collect()
    }

    /// The tick the chapter with this id was met on, or `None` while it isn't (#439). A chapter
    /// latches: once met it stays met.
    pub fn chapter_met(&self, id: &str) -> Option<u64> {
        self.endeavor_log.chapters.get(id).copied()
    }

    /// The first chapter in the data not met yet, the one the Endeavor screen and the side
    /// panel show (#439).
    pub fn current_chapter<'d>(&self, defs: &'d Defs) -> Option<&'d ChapterDef> {
        defs.endeavor.chapters.iter().find(|c| self.chapter_met(&c.id).is_none())
    }

    /// The reachable kinds (`endeavor::reachable_buildings`) this settlement hasn't finished
    /// yet, in data order: what the `EveryBuilding` sign's hint names (`fn_ui::endeavor`, #354).
    pub fn missing_building_kinds(&self, defs: &Defs) -> Vec<BuildingId> {
        let finished: BTreeSet<BuildingId> = self.finished_buildings().collect();
        endeavor::reachable_buildings(self.nation, &defs.techs, &defs.buildings)
            .filter(|b| !finished.contains(b))
            .collect()
    }

    /// The once-a-second check: log signs met for the first time, and latch Ready when all are
    /// met at once. Does nothing without signs or once Ready.
    pub(super) fn tick_endeavor(&mut self, defs: &Defs) {
        if !self.tick.is_multiple_of(u64::from(TICKS_PER_SECOND)) {
            return;
        }
        self.tick_chapters(defs);
        self.tick_milestones(defs);
        if defs.endeavor.signs.is_empty() || self.endeavor != EndeavorState::Dormant {
            return;
        }
        let tick = self.tick;
        let progress = self.endeavor_signs(defs);
        for p in progress.iter().filter(|p| p.met) {
            if self.endeavor_log.met.insert(p.id.to_owned()) {
                let kind = EndeavorEventKind::SignMet(p.id.to_owned());
                self.log_event(EndeavorEvent { tick, kind });
            }
        }
        if progress.iter().all(|p| p.met) {
            self.endeavor = EndeavorState::Ready { tick };
            self.log_event(EndeavorEvent { tick, kind: EndeavorEventKind::Ready });
        }
    }

    /// Fires each population milestone the United City has reached and not fired yet (#442): an
    /// event each, in order, so a settlement already past one when the gate stands still hears
    /// of it. Nothing else happens (owner decision #930).
    fn tick_milestones(&mut self, defs: &Defs) {
        if !self.united() {
            return;
        }
        let population = self.settlers() as u32;
        while let Some(next) = self.next_milestone(defs)
            && population >= next
        {
            self.endeavor_log.milestone = next;
            self.endeavor_event(EndeavorEventKind::PopulationMilestone(next));
        }
    }

    /// Latches each chapter whose signs are all met at once (#439), whatever the gate's state.
    /// Its signs log no `SignMet` of their own: the chapter's one event is the news.
    fn tick_chapters(&mut self, defs: &Defs) {
        for chapter in &defs.endeavor.chapters {
            if self.chapter_met(&chapter.id).is_some() {
                continue;
            }
            if self.chapter_signs(chapter, defs).iter().all(|p| p.met) {
                self.endeavor_log.chapters.insert(chapter.id.clone(), self.tick);
                self.endeavor_event(EndeavorEventKind::ChapterMet(chapter.id.clone()));
            }
        }
    }

    /// Logs an endeavor event at the current tick.
    pub(super) fn endeavor_event(&mut self, kind: EndeavorEventKind) {
        self.log_event(EndeavorEvent { tick: self.tick, kind });
    }

    /// Checks whether unit `i`'s bend just finished has raised its rank (`World::rank_of`) past
    /// `rank_credited`, and if so logs `EndeavorEventKind::RankUp` for the new rank
    /// (`world/bending.rs::run_bend`, `world/gate.rs::run_gate_task`, #360). Independent of
    /// whether the data even has a `masters` sign: `[mastery]` is foundational, not tied to any
    /// one endeavor. `Unit::rank_credited`, not a recomputed comparison, is what makes this a
    /// once-per-rank event, so a save/load or `[mastery]` retuned later never refires it for a
    /// rank already reached (though it also means a save from before `bends_by` existed, once
    /// its forms catch back up, fires again for whichever rank it re-earns).
    pub(super) fn note_rank_up(&mut self, i: usize, defs: &Defs) {
        let progress = self.rank_of(&self.units[i], defs);
        if progress.rank <= self.units[i].rank_credited {
            return;
        }
        self.units[i].rank_credited = progress.rank;
        let unit = self.units[i].id;
        let name = self.units[i].name.clone();
        let nation = self.nation_of(&self.units[i]);
        let title = defs.mastery.title(nation, progress.rank).to_owned();
        self.endeavor_event(EndeavorEventKind::RankUp { unit, name, rank: progress.rank, title });
    }
}

#[cfg(test)]
mod tests;
