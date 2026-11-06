//! The Avatar as a guest: arrival, leaving and returning (`docs/design/endgame.md`, "The
//! Avatar"; issue #237). What he is allowed to do once he is here (needs, Avatar tasks, pillar
//! tasks, bend jobs) is spread across `work.rs`, `gate.rs` and `bending.rs`, each at the one spot
//! that already decides who may claim what; this file only owns *whether* he is here.
//!
//! **Arrival.** Two in-game minutes (`AvatarDef::arrive_after_minutes`) after the gate's
//! foundation stage is done (`GateSite`, `gate.rs`, #236), he appears on the nearest free tile
//! west of the site, searched in row-major order (`Grid::positions`) so it is deterministic. A
//! world with no gate, or whose data has no `[avatar]`, never schedules him.
//!
//! **Leaving and returning.** While he is here, `check_avatar_patience` watches his hunger and
//! energy: if either has stayed at or below `low` with no gap for `AvatarDef::patience_minutes`,
//! he leaves (an event names the reason) and comes back `AvatarDef::return_minutes` later, by
//! the same arrival search. He never dies (`World::tick_needs`); thirst isn't a leaving reason,
//! by design. This is suspended for as long as he holds the Harmony Rite (`Task::Rite`,
//! `world/gate.rs`, #238): his needs never pre-empt it once it has begun.
//!
//! **The farewell** (#238): once the rite completes, `gate.rs` calls `begin_avatar_farewell`,
//! which moves him to `Farewell`. `AvatarDef::farewell_minutes` later he departs for good
//! (`Departed`): v1 has no later visit.
//!
//! **One guest, one id.** The Avatar is never more than one unit at a time, so he keeps a single
//! reserved id (`AVATAR_UNIT_ID`, the top of the `u32` range, never used by an ordinary settler)
//! across every arrival: nothing outside this file keeps a `UnitId` of his around once he leaves
//! (a claimed job is released like a death's, `World::release`), so reusing it is safe.

use super::*;

/// Reserved for the Avatar: higher than any id a settler or a newcomer (#232) could ever reach.
const AVATAR_UNIT_ID: UnitId = UnitId(u32::MAX);

/// Ticks in `minutes` in-game minutes, rounded to the nearest tick.
fn minutes_to_ticks(minutes: f32) -> u64 {
    (minutes * 60.0 * TICKS_PER_SECOND as f32).round() as u64
}

/// Where the Avatar stands. `#[serde(default)]` on `World::avatar`, so a save from before #237
/// reads as `NotYetArrived`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
pub enum AvatarState {
    /// The gate's foundation stage isn't done, or there is no gate or no `[avatar]` data.
    #[default]
    NotYetArrived,
    /// The foundation stage is done; he lands on or after this tick, as soon as a tile is free.
    Arriving { at: u64 },
    /// He is in `World::units`, with `Unit::guest == Some(Guest::Avatar)`.
    Present,
    /// He left; he is due back on or after this tick, as soon as a tile is free.
    Away { returns_at: u64 },
    /// The Harmony Rite is complete: he stays for the celebration until this tick, then departs
    /// for good (#238). He is still `Unit::is_avatar` in `World::units` throughout.
    Farewell { at: u64 },
    /// He has flown off for good once his farewell ended. V1 has no later visit.
    Departed,
}

impl World {
    /// Where the Avatar stands (`AvatarState`). The Endeavor screen (#242) reads this for a
    /// countdown; the unit itself, while `Present`, is the one `Unit::is_avatar` picks out of
    /// `World::units`.
    pub fn avatar(&self) -> &AvatarState {
        &self.avatar
    }

    /// The chi cap unit `u`'s meter never exceeds: the Avatar's own (`[avatar].chi_max`, which
    /// may be more than `data/bending.toml`'s `chi.max`), or that for anyone else.
    pub(super) fn chi_max_for(&self, u: &Unit, defs: &Defs) -> f32 {
        if u.is_avatar()
            && let Some(avatar) = &defs.endeavor.avatar
        {
            return avatar.chi_max;
        }
        defs.chi.max
    }

    /// Whether `pos` could take a unit stepping onto the map right now: walkable, and nothing
    /// (building, site, or another unit standing where the map shows it) already there.
    fn free_for_guest(&self, pos: Pos) -> bool {
        self.map.get(pos).is_some_and(|t| self.rules.passable(t))
            && self.footprint_at(pos).is_none()
            && !self.sites.contains_key(&pos)
            && self.units_at(pos).next().is_none()
    }

    /// The nearest free tile west of the gate's site, searched row-major (top to bottom, left to
    /// right) so it is deterministic (`docs/design/endgame.md`, "Arrival"). `None` while no tile
    /// west of the site is free yet, or there is no site to arrive beside.
    fn avatar_arrival_tile(&self) -> Option<Pos> {
        let anchor = self.gate_anchor()?;
        self.map.positions().filter(|p| p.x < anchor.x).find(|&p| self.free_for_guest(p))
    }

    /// Puts the Avatar on the map at `pos`, with a full meter of everything (`[avatar]`).
    fn spawn_avatar(&mut self, pos: Pos, defs: &Defs) {
        let avatar = defs.endeavor.avatar.as_ref().expect("checked by the caller");
        self.units.push(Unit {
            id: AVATAR_UNIT_ID,
            name: avatar.name.clone(),
            nation: None,
            pos,
            variant: 0,
            needs: Needs::full(&defs.needs),
            chi: avatar.chi_max,
            carrying: None,
            need_task: None,
            job_task: None,
            route: None,
            work: 0.0,
            bed: None,
            healing: None,
            need_retry_at: [0; 3],
            healing_retry_at: 0,
            claim_retry_at: 0,
            move_progress: 0.0,
            idle_until: 0,
            wander_left: 0,
            bends: 0,
            mastered: false,
            inside: None,
            guest: Some(Guest::Avatar),
            unhappy_since: None,
            last_harm: None,
            exposed_alerted: false,
        });
    }

    /// Unit `i` is the Avatar and has gone unfed or exhausted for too long: he leaves.
    fn avatar_leaves(&mut self, i: usize, reason: AvatarLeaveReason, defs: &Defs) {
        let avatar = defs.endeavor.avatar.as_ref().expect("checked by the caller");
        let returns_at = self.tick + minutes_to_ticks(avatar.return_minutes);
        let u = self.units.remove(i);
        self.release(&u);
        self.avatar = AvatarState::Away { returns_at };
        self.endeavor_event(EndeavorEventKind::AvatarLeft(reason));
    }

    /// While the Avatar is here: tracks how long his hunger or energy has stayed at or below
    /// `low` with no gap, and sends him home once that reaches `patience_minutes`. Suspended for
    /// as long as he holds the Harmony Rite (`docs/design/endgame.md`, "What can go wrong";
    /// #238): his needs never pre-empt it once it has begun.
    fn check_avatar_patience(&mut self, defs: &Defs) {
        let Some(avatar) = &defs.endeavor.avatar else { return };
        let Some(i) = self.units.iter().position(Unit::is_avatar) else { return };
        if self.units[i].job_task == Some(Task::Rite) {
            return;
        }
        let (hunger, energy) = (self.units[i].needs.hunger, self.units[i].needs.energy);
        let hungry = hunger <= defs.needs.hunger.low;
        let tired = energy <= defs.needs.energy.low;
        if !hungry && !tired {
            self.units[i].unhappy_since = None;
            return;
        }
        let since = *self.units[i].unhappy_since.get_or_insert(self.tick);
        if self.tick - since < minutes_to_ticks(avatar.patience_minutes) {
            return;
        }
        let reason = if hungry { AvatarLeaveReason::Hungry } else { AvatarLeaveReason::Tired };
        self.avatar_leaves(i, reason, defs);
    }

    /// Once a tick: arrival, patience, and return, in that order (`AvatarState`).
    pub(super) fn tick_avatar(&mut self, defs: &Defs) {
        if defs.endeavor.avatar.is_none() {
            return;
        }
        match self.avatar {
            AvatarState::NotYetArrived => {
                if self.gate_past_first_stage(defs) {
                    let avatar = defs.endeavor.avatar.as_ref().expect("checked above");
                    let at = self.tick + minutes_to_ticks(avatar.arrive_after_minutes);
                    self.avatar = AvatarState::Arriving { at };
                }
            }
            AvatarState::Arriving { at } if self.tick >= at => {
                if let Some(pos) = self.avatar_arrival_tile() {
                    self.spawn_avatar(pos, defs);
                    self.avatar = AvatarState::Present;
                    self.endeavor_event(EndeavorEventKind::AvatarArrived);
                }
            }
            AvatarState::Arriving { .. } => {}
            AvatarState::Present => self.check_avatar_patience(defs),
            AvatarState::Away { returns_at } if self.tick >= returns_at => {
                if let Some(pos) = self.avatar_arrival_tile() {
                    self.spawn_avatar(pos, defs);
                    self.avatar = AvatarState::Present;
                    self.endeavor_event(EndeavorEventKind::AvatarReturned);
                }
            }
            AvatarState::Away { .. } => {}
            AvatarState::Farewell { at } if self.tick >= at => self.avatar_departs(),
            AvatarState::Farewell { .. } => {}
            AvatarState::Departed => {}
        }
    }

    /// The Harmony Rite just completed (`gate.rs`, #238): he stays for the celebration until
    /// `AvatarDef::farewell_minutes` from now, then departs for good.
    pub(super) fn begin_avatar_farewell(&mut self, defs: &Defs) {
        let Some(avatar) = &defs.endeavor.avatar else { return };
        let at = self.tick + minutes_to_ticks(avatar.farewell_minutes);
        self.avatar = AvatarState::Farewell { at };
    }

    /// His farewell is over: he leaves the settlement for good.
    fn avatar_departs(&mut self) {
        if let Some(i) = self.units.iter().position(Unit::is_avatar) {
            let u = self.units.remove(i);
            self.release(&u);
        }
        self.avatar = AvatarState::Departed;
        self.endeavor_event(EndeavorEventKind::AvatarFarewell);
    }
}

#[cfg(test)]
mod tests;
