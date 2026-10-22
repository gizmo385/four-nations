//! Bending at work: designating an ability, a unit channeling it, and what it does.
//!
//! A bend is a job like a harvest (`JobKind::Bend`). The player designates a tile
//! (`Command::Bend`); a unit whose nation can use the ability and whose chi covers the cost
//! claims it, walks to the tile and channels for `AbilityDef::work` seconds (divided by the
//! `speed_modifier` named in `data/bending.toml`). Needs pre-empt it and its progress is kept,
//! exactly as for harvests. **Chi is spent only when the bend completes**; since a unit holds
//! one job at a time it can't be spent twice. If the target is no longer valid when the channel
//! finishes, the job is dropped, no chi is spent and the event log says why.
//!
//! **This file holds every `match` on `Effect`** (AGENTS.md rule 7): `validate_effect` says
//! whether a target is acceptable and `apply_effect` changes the world. The four real effects
//! are separate work (#68 Raise Earth, #69 Water the Land, #70 Hearthflame, #71 Wind Carry); until
//! each lands its arm refuses every target as "not implemented yet", so a bend can't be spent on
//! nothing. Everything else here is effect-agnostic.

use super::work::route_from;
use crate::bending::{AbilityDef, Effect};
use crate::jobs::JobKind;

use super::*;

/// Why a bend command was refused. The world is unchanged when `apply` returns one of these.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BendError {
    /// There is no ability with this id.
    UnknownAbility,
    /// The ability needs a tech the settlement hasn't learned.
    Locked { tech: TechId },
    /// The ability belongs to another nation.
    WrongNation,
    /// The tile can't take this ability, and why ("needs fresh water within 4 tiles").
    InvalidTarget(&'static str),
    /// A bend is already designated on that tile.
    AlreadyDesignated,
    /// There is no bend designation on that tile to cancel.
    NotDesignated,
}

impl std::fmt::Display for BendError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnknownAbility => f.write_str("no such ability"),
            Self::Locked { .. } => f.write_str("not researched yet"),
            Self::WrongNation => f.write_str("that ability belongs to another nation"),
            Self::InvalidTarget(why) => f.write_str(why),
            Self::AlreadyDesignated => f.write_str("already designated"),
            Self::NotDesignated => f.write_str("not designated"),
        }
    }
}

impl std::error::Error for BendError {}

/// How a bend ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BendOutcome {
    /// The ability took effect and the chi was spent.
    Done,
    /// The target was no longer valid when the channel finished (why, as `InvalidTarget` would
    /// say it). No chi was spent.
    Failed(&'static str),
}

/// A bend that finished or failed, for the client's toasts and hover text.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BendEvent {
    pub tick: u64,
    pub unit: UnitId,
    pub ability: AbilityId,
    pub pos: Pos,
    pub outcome: BendOutcome,
}

/// Queries and the designations behind `Command::Bend` and `Command::CancelBend`.
impl World {
    /// Every bend that has finished or failed, oldest first.
    pub fn bend_events(&self) -> &[BendEvent] {
        &self.bend_events
    }

    /// Tiles with a bend designation that isn't finished, claimed or not, with the ability.
    pub fn bend_designations(&self) -> impl Iterator<Item = (AbilityId, Pos)> + '_ {
        self.jobs.iter().filter_map(|j| match j.kind {
            JobKind::Bend { ability, pos } => Some((ability, pos)),
            _ => None,
        })
    }

    /// Can this settlement use `ability` right now: it is the nation's and its tech (if any) is
    /// known. For the client to decide which abilities to offer.
    pub fn check_ability(&self, ability: AbilityId, defs: &Defs) -> Result<(), BendError> {
        let def = defs.abilities.get(ability.0 as usize).ok_or(BendError::UnknownAbility)?;
        if def.nation != self.nation {
            return Err(BendError::WrongNation);
        }
        match def.unlocked_by {
            Some(tech) if !self.research.is_known(tech) => Err(BendError::Locked { tech }),
            _ => Ok(()),
        }
    }

    /// Designates `ability` on `pos`.
    pub(super) fn designate_bend(
        &mut self,
        ability: AbilityId,
        pos: Pos,
        defs: &Defs,
    ) -> Result<JobId, BendError> {
        self.check_ability(ability, defs)?;
        if self.bend_designations().any(|(_, p)| p == pos) {
            return Err(BendError::AlreadyDesignated);
        }
        self.validate_bend(defs.ability(ability), pos).map_err(BendError::InvalidTarget)?;
        Ok(self.add_job(JobKind::Bend { ability, pos }))
    }

    /// Withdraws the bend designation on `pos`. Channeling done on it is lost.
    pub(super) fn cancel_bend(&mut self, pos: Pos) -> Result<(), BendError> {
        let before = self.jobs.len();
        self.jobs.retain(|j| !matches!(j.kind, JobKind::Bend { pos: p, .. } if p == pos));
        if self.jobs.len() == before {
            return Err(BendError::NotDesignated);
        }
        Ok(())
    }
}

/// Doing it.
impl World {
    /// Whether unit `i` may take a bend job for `def`: it is a nation that can use it and its
    /// chi covers the cost. (Research gating is settlement-wide and checked on designation.)
    pub(super) fn unit_can_bend(&self, i: usize, def: &AbilityDef) -> bool {
        let u = &self.units[i];
        u.nation == def.nation && u.chi >= def.chi
    }

    /// Channel seconds per second: the nation modifier the ability names, or 1.0.
    fn bend_rate(&self, def: &AbilityDef) -> f32 {
        def.speed_modifier.as_deref().and_then(|m| self.modifiers().get(m)).unwrap_or(1.0)
    }

    pub(super) fn run_bend(&mut self, i: usize, id: JobId, defs: &Defs) {
        let Some(idx) = self.job_index(id) else { return self.finish_job(i) };
        let JobKind::Bend { ability, pos } = self.jobs[idx].kind else {
            return self.finish_job(i);
        };
        let def = defs.ability(ability);
        if !self.unit_can_bend(i, def) {
            // Chi can only have gone up since the claim, so this is a safety net.
            return self.abandon_job(i, id, defs);
        }
        if self.units[i].route.is_none() {
            // Resuming after a need, or the claim's route was lost: plan from here.
            let flood = self.flood_from(i);
            match self.stand(&flood, pos).and_then(|(_, s)| flood.path_to(s)) {
                Some(path) => self.units[i].route = Some(route_from(path)),
                None => return self.abandon_job(i, id, defs),
            }
        }
        if !self.walk(i, defs) {
            return;
        }
        let rate = self.bend_rate(def);
        let job = &mut self.jobs[idx];
        job.progress += rate / TICKS_PER_SECOND as f32;
        if job.progress < def.work {
            return;
        }
        self.jobs.remove(idx);
        self.finish_job(i);
        let outcome = match self.validate_bend(def, pos) {
            Ok(()) => {
                self.apply_bend(i, def, pos);
                BendOutcome::Done
            }
            Err(why) => BendOutcome::Failed(why),
        };
        let unit = self.units[i].id;
        self.bend_events.push(BendEvent { tick: self.tick, unit, ability, pos, outcome });
    }

    /// Is `pos` an acceptable target for `def` right now? Run when the command is given and
    /// again when the channel finishes. The error is the reason, shown to the player.
    fn validate_bend(&self, def: &AbilityDef, pos: Pos) -> Result<(), &'static str> {
        if self.map.get(pos).is_none() {
            return Err("off the map");
        }
        self.validate_effect(&def.effect, pos)
    }

    /// Spends the chi and takes effect. The target was just validated.
    fn apply_bend(&mut self, i: usize, def: &AbilityDef, pos: Pos) {
        let u = &mut self.units[i];
        u.chi = (u.chi - def.chi).max(0.0);
        self.apply_effect(&def.effect, pos);
    }

    // ---- the effects: the only matches on `Effect` ----

    // `pos` is unused outside tests until the real effects land.
    #[allow(unused_variables)]
    fn validate_effect(&self, effect: &Effect, pos: Pos) -> Result<(), &'static str> {
        match effect {
            // TODO(#68): the tile is a `from` terrain, 4-adjacent to a passable tile.
            Effect::Reshape { .. } => Err("not implemented yet"),
            // TODO(#69): a Land or Coast centre within `source_range` of fresh water.
            Effect::Irrigate { .. } => Err("not implemented yet"),
            // TODO(#70): an empty Land or Coast tile.
            Effect::HeatSource { .. } => Err("not implemented yet"),
            // TODO(#71): a ground stack, and a stockpile in range.
            Effect::WindCarry { .. } => Err("not implemented yet"),
            #[cfg(test)]
            Effect::Probe => {
                if self.map[pos].terrain.is_walkable() {
                    Ok(())
                } else {
                    Err("probe needs walkable ground")
                }
            }
        }
    }

    // `pos` is unused until the real effects land.
    #[allow(unused_variables)]
    fn apply_effect(&mut self, effect: &Effect, pos: Pos) {
        match effect {
            // TODO(#68), TODO(#69), TODO(#70), TODO(#71): unreachable until `validate_effect`
            // accepts a target.
            Effect::Reshape { .. }
            | Effect::Irrigate { .. }
            | Effect::HeatSource { .. }
            | Effect::WindCarry { .. } => {}
            #[cfg(test)]
            Effect::Probe => {}
        }
    }
}

#[cfg(test)]
mod tests;
