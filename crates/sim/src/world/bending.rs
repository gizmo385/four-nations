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
//! an effect lands its arm refuses every target as "not implemented yet", so a bend can't be spent
//! on nothing. (Raise Earth and Water the Land are in.) Everything else here is effect-agnostic.
//!
//! **Raise Earth** (#68) turns a river or lake tile with dry ground on one of its four sides
//! into a `Ford`. Fords are permanent. The regions cache is invalidated and idle units woken,
//! since jobs across the water may have become reachable.
//!
//! **Water the Land** (#69) leaves a `Patch` behind: a 3x3 area that stays wet for a while. When
//! `tick_irrigation` finds a patch dry (patches dry in `(tick, centre)` order) up to `sprouts` of
//! its empty, fertile, non-fallow tiles grow the sprout resource, picked with the `bending.sprout`
//! stream. A tile that sprouted is fallow for a while and can't sprout again. The patch carries
//! the effect's numbers with it, so drying never has to look at the ability again.

use rand::seq::SliceRandom;

use super::work::route_from;
use crate::bending::{AbilityDef, Effect};
use crate::defs::{BiomeId, ResourceId, Terrain};
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

/// A watered 3x3 patch waiting to dry out.
#[derive(Debug, Clone, PartialEq)]
pub(super) struct Patch {
    centre: Pos,
    /// The tick it is dry, and sprouts.
    dries_at: u64,
    sprouts: u32,
    resource: ResourceId,
    biomes: Vec<BiomeId>,
    fallow_ticks: u64,
}

/// Ticks in `minutes` in-game minutes, at least 1.
fn minutes_to_ticks(minutes: f32) -> u64 {
    ((minutes * 60.0 * TICKS_PER_SECOND as f32).round() as u64).max(1)
}

/// Seconds in `ticks`.
fn ticks_to_seconds(ticks: u64) -> f32 {
    ticks as f32 / TICKS_PER_SECOND as f32
}

/// The 3x3 block around `centre`, row by row.
fn block(centre: Pos) -> impl Iterator<Item = Pos> {
    (-1..=1).flat_map(move |dy| (-1..=1).map(move |dx| Pos::new(centre.x + dx, centre.y + dy)))
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

    /// Every wet tile with the seconds until it dries, in position order. A tile in two patches
    /// stays wet for the longer. Water tiles and rock don't get wet, so a patch at the shore
    /// shows only its dry-land part.
    pub fn wet_tiles(&self) -> Vec<(Pos, f32)> {
        let mut dries: BTreeMap<Pos, u64> = BTreeMap::new();
        for patch in &self.patches {
            for p in block(patch.centre) {
                if self
                    .map
                    .get(p)
                    .is_some_and(|t| matches!(t.terrain, Terrain::Land | Terrain::Coast))
                {
                    let at = dries.entry(p).or_insert(0);
                    *at = (*at).max(patch.dries_at);
                }
            }
        }
        dries
            .into_iter()
            .map(|(p, at)| (p, ticks_to_seconds(at.saturating_sub(self.tick))))
            .collect()
    }

    /// Every fallow tile (one that sprouted lately and can't sprout again) with the seconds
    /// until its fallow time ends, in position order.
    pub fn fallow_tiles(&self) -> Vec<(Pos, f32)> {
        self.fallow
            .iter()
            .filter(|&(_, &until)| until > self.tick)
            .map(|(&p, &until)| (p, ticks_to_seconds(until - self.tick)))
            .collect()
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

    /// The nation modifier the ability scales its effect's magnitude by, or 1.0.
    fn bend_magnitude(&self, def: &AbilityDef) -> f32 {
        def.magnitude_modifier.as_deref().and_then(|m| self.modifiers().get(m)).unwrap_or(1.0)
    }

    /// Spends the chi and takes effect. The target was just validated.
    fn apply_bend(&mut self, i: usize, def: &AbilityDef, pos: Pos) {
        let u = &mut self.units[i];
        u.chi = (u.chi - def.chi).max(0.0);
        let magnitude = self.bend_magnitude(def);
        self.apply_effect(&def.effect, pos, magnitude);
    }

    // ---- the effects: the only matches on `Effect` ----

    // `pos` is unused outside tests until the real effects land.
    #[allow(unused_variables)]
    fn validate_effect(&self, effect: &Effect, pos: Pos) -> Result<(), &'static str> {
        match effect {
            Effect::Reshape { from, .. } => {
                let tile = &self.map[pos];
                if !from.contains(&tile.terrain) {
                    return Err(match tile.terrain {
                        Terrain::Water | Terrain::Coast => "the sea is out of reach",
                        Terrain::Ford => "already a ford",
                        _ => "needs a river or lake tile",
                    });
                }
                // A fishing spot (or anything else) on the tile would be left on the new terrain.
                if tile.resource.is_some() {
                    return Err("something is in the way");
                }
                let bank = [(0, -1), (1, 0), (0, 1), (-1, 0)]
                    .iter()
                    .any(|&(dx, dy)| self.walkable(Pos::new(pos.x + dx, pos.y + dy)));
                if bank { Ok(()) } else { Err("needs dry ground beside it") }
            }
            Effect::Irrigate { source_range, .. } => {
                let range = *source_range as i32;
                let open = |t: Terrain| matches!(t, Terrain::Land | Terrain::Coast);
                if !open(self.map[pos].terrain) {
                    return Err("needs open ground to water");
                }
                let fresh = (-range..=range)
                    .flat_map(|dy| (-range..=range).map(move |dx| Pos::new(pos.x + dx, pos.y + dy)))
                    .filter_map(|q| self.map.get(q))
                    .any(|t| matches!(t.terrain, Terrain::River | Terrain::Lake | Terrain::Ford));
                if fresh { Ok(()) } else { Err("needs fresh water nearby") }
            }
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

    // `magnitude` is unused until the effects that scale by it land.
    #[allow(unused_variables)]
    fn apply_effect(&mut self, effect: &Effect, pos: Pos, magnitude: f32) {
        match effect {
            // The new terrain is permanent (no undo in v1). It changes who can walk where, so
            // the cached regions are stale and idle units should look for jobs that opened up.
            Effect::Reshape { to, .. } => {
                self.map[pos].terrain = *to;
                self.map_changed();
                self.wake_idle();
            }
            // TODO(#70), TODO(#71): unreachable until `validate_effect` accepts a target.
            Effect::HeatSource { .. } | Effect::WindCarry { .. } => {}
            // The magnitude scales how long the patch stays wet.
            Effect::Irrigate {
                wet_minutes,
                sprouts,
                sprout_resource,
                biomes,
                fallow_minutes,
                ..
            } => {
                let patch = Patch {
                    centre: pos,
                    dries_at: self.tick + minutes_to_ticks(wet_minutes * magnitude),
                    sprouts: *sprouts,
                    resource: *sprout_resource,
                    biomes: biomes.clone(),
                    fallow_ticks: minutes_to_ticks(*fallow_minutes),
                };
                // Keep the list in drying order; equal times stay in the order they were made.
                let key = (patch.dries_at, patch.centre);
                let at = self.patches.partition_point(|p| (p.dries_at, p.centre) <= key);
                self.patches.insert(at, patch);
            }
            #[cfg(test)]
            Effect::Probe => {}
        }
    }
}

/// Patches drying and sprouting.
impl World {
    /// Dries every patch whose time is up, oldest first, and ends fallow times that have run out.
    pub(super) fn tick_irrigation(&mut self, defs: &Defs) {
        if !self.fallow.is_empty() {
            let now = self.tick;
            self.fallow.retain(|_, until| *until > now);
        }
        let due = self.patches.partition_point(|p| p.dries_at <= self.tick);
        if due == 0 {
            return;
        }
        let dried: Vec<Patch> = self.patches.drain(..due).collect();
        let mut changed = false;
        for patch in &dried {
            changed |= self.sprout(patch, defs);
        }
        if changed {
            // New resources block movement, so connectivity may have changed.
            self.map_changed();
        }
    }

    /// Grows up to `patch.sprouts` of its eligible tiles. True if the map changed.
    fn sprout(&mut self, patch: &Patch, defs: &Defs) -> bool {
        let mut candidates: Vec<Pos> =
            block(patch.centre).filter(|&p| self.can_sprout(p, patch, defs)).collect();
        if candidates.is_empty() {
            return false;
        }
        candidates.shuffle(&mut self.sprout_rng);
        candidates.truncate(patch.sprouts as usize);
        for &p in &candidates {
            self.map[p].resource = Some(patch.resource);
            self.resting.remove(&p);
            self.fallow.insert(p, self.tick + patch.fallow_ticks);
        }
        true
    }

    /// Can `p` take a sprout of this patch's resource: empty, fertile, not fallow, and not in
    /// use (a stockpile, a stack on the ground, a bend target, or where a unit is or is headed,
    /// since resources block movement).
    fn can_sprout(&self, p: Pos, patch: &Patch, defs: &Defs) -> bool {
        let Some(tile) = self.map.get(p) else { return false };
        // The terrain must be one the resource grows on in mapgen too (cabbages: land only).
        let terrain_ok = matches!(tile.terrain, Terrain::Land | Terrain::Coast)
            && defs
                .resource(patch.resource)
                .terrains
                .as_ref()
                .is_none_or(|t| t.contains(&tile.terrain));
        terrain_ok
            && tile.resource.is_none()
            && patch.biomes.contains(&tile.biome)
            && !self.fallow.contains_key(&p)
            && !self.zones.contains(&p)
            && !self.ground.contains_key(&p)
            && !self.bend_designations().any(|(_, q)| q == p)
            && !self
                .units
                .iter()
                .any(|u| u.pos == p || u.route.as_ref().is_some_and(|r| r.contains(&p)))
    }
}

#[cfg(test)]
mod earth_tests;
#[cfg(test)]
mod tests;
#[cfg(test)]
mod water_tests;
