//! The Chart job: the Map Room's labor (`data/buildings.toml`, `BuildingDef::chart`,
//! `data/research.toml`'s `cartography`, issue #402, docs/design/cartography.md).
//!
//! A finished Map Room's `seats` become that many `JobKind::Chart` jobs (`open_chart_seats`,
//! called from `world/build.rs` in place of `open_study_seats` whenever the building has a
//! `chart` def), mirroring a study hall's seat (`world/study.rs`) in almost every way: it is the
//! same low priority (`work::job_priority`), a unit holds it by walking to the room's door and
//! stepping inside (`World::enter_building`), needs pre-empt it the same way, and it never
//! completes on its own. It isn't claimed at all while nothing within the room's `range` is left
//! to chart (`chart_has_target`), the same way a study seat isn't claimed with an empty research
//! queue.
//!
//! Where it differs: progress lives on the building itself (`World::charts`, by anchor, the way
//! a production building's batch does, `world/forge.rs`), not the research pool, and a finished
//! chart (`minutes` of seated work, scaled by the nation's `study` modifier) doesn't add to
//! anything -- it uncovers one buried resource (`World::uncover`) instead. Which one
//! (`pick_chart_target`) is deterministic: reachable candidates first, by walking cost from the
//! room's door under the settlement's current move rules; then unreachable ones, by Chebyshev
//! distance; ties by `Pos`. Every finished chart, found or not, is logged (`ChartEvent`) for a
//! frontend's alert log.

use crate::building::ChartDef;
use crate::flood::Flood;
use crate::jobs::JobKind;

use super::work::route_from;
use super::*;

/// One finished Chart job, oldest first, for a frontend's alert log (mirrors `StrainEvent`).
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ChartEvent {
    pub tick: u64,
    /// The Map Room that made this chart.
    pub room: Pos,
    pub kind: ChartEventKind,
}

/// What a finished chart found (`World::chart_events`).
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum ChartEventKind {
    /// It uncovered `pos`: `reachable` says whether a settler could walk there under the
    /// settlement's current move rules when the chart finished (`pick_chart_target`).
    Found { pos: Pos, reachable: bool },
    /// Nothing was left to chart within the room's range; its seat stops being claimed until
    /// something buried turns up there again (it never will, on today's static map, but the
    /// rule costs nothing to leave general).
    NoneLeft,
}

/// Ticks in `minutes` in-game minutes, at least 1 (mirrors `world/study.rs`'s own copy; #402).
fn minutes_to_ticks(minutes: f32) -> u64 {
    ((minutes * 60.0 * TICKS_PER_SECOND as f32).round() as u64).max(1)
}

impl World {
    /// Opens `seats` new `Chart` jobs at `pos`. Called once, when a Map Room finishes
    /// construction, in place of `open_study_seats` (`world/build.rs`).
    pub(super) fn open_chart_seats(&mut self, pos: Pos, seats: u32) {
        for _ in 0..seats {
            self.add_job(JobKind::Chart { pos });
        }
    }

    /// Every finished chart so far, oldest first.
    pub fn chart_events(&self) -> &[ChartEvent] {
        &self.chart_events
    }

    /// The Map Room anchored on `pos`'s progress toward its next chart, in ticks (scaled by the
    /// `study` modifier as they're added), for a frontend's inspect panel. 0 off a Map Room or
    /// one with nothing banked yet.
    pub fn chart_progress(&self, pos: Pos) -> f32 {
        self.charts.get(&pos).copied().unwrap_or(0.0)
    }

    /// The Map Room's own def at `pos`, if one stands there (`BuildingDef::chart`).
    pub(super) fn chart_def_at<'a>(&self, pos: Pos, defs: &'a Defs) -> Option<&'a ChartDef> {
        self.building_at(pos).and_then(|id| defs.building(id).chart.as_ref())
    }

    /// Whether `def` (the Map Room anchored on `pos`) still has something to chart: any resource
    /// among `finds`, buried within `range` tiles (Chebyshev) of `pos`, free to uncover
    /// (`World::free_to_uncover`). Ignores reachability on purpose: an unreachable ruin is still
    /// charted eventually (`pick_chart_target`), just after every reachable one, so it still
    /// counts as something left to do.
    pub(super) fn chart_has_target(&self, pos: Pos, def: &ChartDef) -> bool {
        let range = def.range as i32;
        self.buried.iter().any(|(&b, r)| {
            def.finds.contains(r) && pos.chebyshev(b) <= range && self.free_to_uncover(b)
        })
    }

    /// Picks what a finished chart at `pos` uncovers (docs/design/cartography.md, "Which ruin a
    /// chart finds"): reachable candidates first, by walking cost from the room's door under the
    /// settlement's current `rules` (not a hauler's cart rules: nobody is carrying anything);
    /// then unreachable ones, by Chebyshev distance from `pos`. Ties break on `Pos` either way.
    /// `None` once `chart_has_target` would already say there's nothing left.
    fn pick_chart_target(&self, pos: Pos, def: &ChartDef) -> Option<(Pos, bool)> {
        let range = def.range as i32;
        let candidates: Vec<Pos> = self
            .buried
            .iter()
            .filter(|&(&b, r)| {
                def.finds.contains(r) && pos.chebyshev(b) <= range && self.free_to_uncover(b)
            })
            .map(|(&b, _)| b)
            .collect();
        if candidates.is_empty() {
            return None;
        }
        let door = self.footprint_at(pos).map(|f| f.entrance())?;
        let flood = Flood::new(&self.map, door, &self.rules);
        let reachable =
            candidates.iter().filter_map(|&c| flood.cost(c).map(|cost| (cost, c))).min();
        if let Some((_, best)) = reachable {
            return Some((best, true));
        }
        candidates.into_iter().map(|c| (pos.chebyshev(c), c)).min().map(|(_, c)| (c, false))
    }

    /// One tick of a unit holding a Map Room's seat: leave it if nothing is left to chart within
    /// range since it was claimed, else walk to the room's door, step inside, and add progress
    /// every tick it's there. A finished chart (`minutes` of it, scaled by the nation's `study`
    /// modifier) uncovers one target (`pick_chart_target`) and logs a `ChartEvent`; finding
    /// nothing left (should only happen the instant the last one in range was just claimed by
    /// this very chart) logs the "none left" event instead and frees the seat.
    pub(super) fn run_chart(&mut self, i: usize, id: JobId, defs: &Defs) {
        let Some(idx) = self.job_index(id) else { return self.finish_job(i) };
        let JobKind::Chart { pos: anchor } = self.jobs[idx].kind else { return self.finish_job(i) };
        let Some(def) = self.chart_def_at(anchor, defs) else { return self.finish_job(i) };
        if !self.chart_has_target(anchor, def) {
            self.jobs[idx].claimed_by = None;
            return self.finish_job(i);
        }
        if self.units[i].inside.is_none() {
            if self.units[i].route.is_none() {
                let flood = self.flood_from(i);
                match self.stand_entrance(&flood, anchor).and_then(|(_, s)| flood.path_to(s)) {
                    Some(path) => self.units[i].route = Some(route_from(path)),
                    None => return self.abandon_job(i, id, defs),
                }
            }
            if !self.walk(i, defs) {
                return;
            }
            if !self.enter_building(i, anchor, defs) {
                return;
            }
        }
        let target = minutes_to_ticks(def.minutes) as f32;
        let study = self.modifiers().study;
        let progress = self.charts.entry(anchor).or_insert(0.0);
        *progress += study;
        if *progress < target {
            return;
        }
        self.charts.remove(&anchor);
        let event_kind = match self.pick_chart_target(anchor, def) {
            Some((target, reachable)) => {
                self.uncover(target);
                self.wake_idle();
                ChartEventKind::Found { pos: target, reachable }
            }
            None => ChartEventKind::NoneLeft,
        };
        self.chart_events.push(ChartEvent { tick: self.tick, room: anchor, kind: event_kind });
    }
}

#[cfg(test)]
mod tests;
