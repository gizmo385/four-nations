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
//! are #68 Raise Earth, #69 Water the Land, #70 Hearthflame and #71 Wind Carry. Everything else
//! here is effect-agnostic.
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
//!
//! **Hearthflame** (#70) lights a `HeatSource` on an empty Land or Coast tile for
//! `duration_minutes` x the `heating` modifier; bending a lit tile again resets the timer. What
//! a lit source does (cooked meals, faster and sought-out sleep) lives in `heat.rs`, which a
//! campfire (#58) can reuse.
//!
//! **Inner Fire** (#123) is the one self-cast ability: the designated tile is where the bender
//! stands to channel (open ground, like a flame), and the aura goes to *whoever channelled it*,
//! not to the tile. It is an `Aura` (`heat.rs`) that follows its carrier for `duration_minutes` x
//! the `heating` modifier; casting again resets the timer.
//!
//! **Wind Carry** (#71) sends the whole stack on the ground tile, ignoring carry capacity, to
//! the nearest stockpile tile with room (holding that item, or empty) within `range` tiles of
//! Chebyshev distance x the `power_generation` modifier, straight over rivers and mountains.
//! Whatever doesn't fit stays on the ground with a fresh haul job. Designating it over a ground
//! stack takes over the tile's existing haul job: a hauler who claimed it but hasn't walked
//! there yet is released back to idle, and the haul job comes back if the bend is cancelled or
//! the target is no longer valid when the channel finishes.

use rand::seq::SliceRandom;

use super::work::route_from;
use crate::bending::{AbilityDef, Effect};
use crate::defs::{BiomeId, ResourceId, Terrain};
use crate::jobs::{ItemId, JobKind};

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
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct BendEvent {
    pub tick: u64,
    pub unit: UnitId,
    pub ability: AbilityId,
    pub pos: Pos,
    pub outcome: BendOutcome,
    /// Wind Carry: the stack that was blown away and where it landed (#166), for the client to
    /// fly it there. `None` for every other effect, and for a bend that failed.
    #[serde(default)]
    pub moved: Option<Moved>,
}

/// A stack Wind Carry moved: `count` of `item` from the bent tile to the stockpile tile `to`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Moved {
    pub to: Pos,
    pub item: ItemId,
    pub count: u32,
}

/// A bend being channelled right now (the unit stands at its spot and the channel is running),
/// for the client's cast animation (#166). Read-only: a view of the job, not a second clock.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BendChannel {
    pub unit: UnitId,
    pub ability: AbilityId,
    /// The tile being bent.
    pub pos: Pos,
    /// In-game seconds of channelling left at the unit's current rate: until the bend completes
    /// and its effect lands.
    pub remaining_secs: f32,
}

/// A watered 3x3 patch waiting to dry out.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
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

/// Chebyshev distance between two tiles, in tiles (Wind Carry's range, straight over anything).
fn chebyshev(a: Pos, b: Pos) -> u32 {
    (a.x - b.x).unsigned_abs().max((a.y - b.y).unsigned_abs())
}

/// `range` scaled by the ability's magnitude (Wind Carry's `power_generation`), rounded to the
/// nearest tile.
fn scaled_range(range: u32, magnitude: f32) -> u32 {
    ((range as f32) * magnitude).max(0.0).round() as u32
}

/// Queries and the designations behind `Command::Bend` and `Command::CancelBend`.
impl World {
    /// Every bend that has finished or failed, oldest first.
    pub fn bend_events(&self) -> &[BendEvent] {
        &self.bend_events
    }

    /// Every bend being channelled right now, in unit order: the unit has reached its spot, no
    /// need has pre-empted the job, and the channel has not yet completed. A unit still walking
    /// to the tile, or set aside to eat, is not channelling.
    pub fn bend_channels(&self, defs: &Defs) -> Vec<BendChannel> {
        let mut out = Vec::new();
        for u in &self.units {
            let (None, Some(Task::Bend(id))) = (u.need_task, u.job_task) else { continue };
            if !u.route.as_ref().is_some_and(|r| r.is_empty()) {
                continue;
            }
            let Some(job) = self.job(id) else { continue };
            let JobKind::Bend { ability, pos } = job.kind else { continue };
            let def = defs.ability(ability);
            let remaining_secs = (def.work - job.progress).max(0.0) / self.bend_rate(def).max(1e-3);
            out.push(BendChannel { unit: u.id, ability, pos, remaining_secs });
        }
        out
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
        let def = defs.ability(ability);
        self.validate_bend(def, pos, defs).map_err(BendError::InvalidTarget)?;
        let id = self.add_job(JobKind::Bend { ability, pos });
        // Wind Carry takes over the tile's haul job (validation just confirmed a stack is there).
        if matches!(def.effect, Effect::WindCarry { .. }) {
            self.take_haul_job(pos);
        }
        Ok(id)
    }

    /// Withdraws the bend designation on `pos`. Channeling done on it is lost. A Wind Carry
    /// designation gives the tile's haul job back, if the stack is still there.
    pub(super) fn cancel_bend(&mut self, pos: Pos, defs: &Defs) -> Result<(), BendError> {
        let Some(idx) = self
            .jobs
            .iter()
            .position(|j| matches!(j.kind, JobKind::Bend { pos: p, .. } if p == pos))
        else {
            return Err(BendError::NotDesignated);
        };
        let JobKind::Bend { ability, .. } = self.jobs[idx].kind else {
            unreachable!("just matched above")
        };
        self.jobs.remove(idx);
        if matches!(defs.ability(ability).effect, Effect::WindCarry { .. })
            && self.ground.contains_key(&pos)
        {
            self.ensure_haul_job(pos);
        }
        Ok(())
    }

    /// Removes the haul job on `pos`, if any, releasing a unit that claimed it but hasn't
    /// walked there yet. Per `run_haul`, a claimed haul job's unit never has picked anything up:
    /// the pickup and the claim's release happen in the same step.
    fn take_haul_job(&mut self, pos: Pos) {
        let Some(idx) = self.jobs.iter().position(|j| j.kind == JobKind::Haul { pos }) else {
            return;
        };
        let job = self.jobs.remove(idx);
        if let Some(unit_id) = job.claimed_by {
            if let Some(u) = self.units.iter_mut().find(|u| u.id == unit_id) {
                u.job_task = None;
                u.route = None;
            }
            self.wake_idle();
        }
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
            match self.stand_tile(&flood, pos).and_then(|(_, s)| flood.path_to(s)) {
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
        let mut moved = None;
        let outcome = match self.validate_bend(def, pos, defs) {
            Ok(()) => {
                moved = self.apply_bend(i, def, pos, defs);
                self.units[i].bends += 1;
                BendOutcome::Done
            }
            Err(why) => {
                // Wind Carry's target was a ground stack that had taken over the haul job: give
                // it back, if the stack is still there to haul.
                if matches!(def.effect, Effect::WindCarry { .. }) && self.ground.contains_key(&pos)
                {
                    self.ensure_haul_job(pos);
                }
                BendOutcome::Failed(why)
            }
        };
        let unit = self.units[i].id;
        self.bend_events.push(BendEvent { tick: self.tick, unit, ability, pos, outcome, moved });
    }

    /// Is `pos` an acceptable target for `def` right now? Run when the command is given and
    /// again when the channel finishes. The error is the reason, shown to the player.
    fn validate_bend(&self, def: &AbilityDef, pos: Pos, defs: &Defs) -> Result<(), &'static str> {
        if self.map.get(pos).is_none() {
            return Err("off the map");
        }
        let magnitude = self.bend_magnitude(def);
        self.validate_effect(&def.effect, pos, magnitude, defs)
    }

    /// The nation modifier the ability scales its effect's magnitude by, or 1.0.
    fn bend_magnitude(&self, def: &AbilityDef) -> f32 {
        def.magnitude_modifier.as_deref().and_then(|m| self.modifiers().get(m)).unwrap_or(1.0)
    }

    /// Brings the resource buried on `pos`, if any, onto the map (#133). Returns whether it did.
    /// Ruins under open ground (#197) can have been built or walked over since the map was made:
    /// one under a building, a site, a stockpile, a stack or a unit stays buried for now, since a
    /// resource there would block what is already using the tile.
    fn uncover(&mut self, pos: Pos) -> bool {
        if !self.buried.contains_key(&pos) || !self.free_to_uncover(pos) {
            return false;
        }
        let r = self.buried.remove(&pos).expect("checked");
        self.map[pos].resource = Some(r);
        self.map_changed();
        true
    }

    fn free_to_uncover(&self, p: Pos) -> bool {
        let tile = &self.map[p];
        tile.resource.is_none()
            && tile.building.is_none()
            && self.site_at(p).is_none()
            && !self.zones.contains(&p)
            && !self.ground.contains_key(&p)
            && !self.bend_designations().any(|(_, q)| q == p)
            && !self
                .units
                .iter()
                .any(|u| u.pos == p || u.route.as_ref().is_some_and(|r| r.contains(&p)))
    }

    /// The nearest stockpile tile within `reach` tiles of `pos` (Chebyshev) that has room for
    /// `item`: holds some of that item already, or is empty. Ties by position.
    fn stockpile_in_reach(&self, pos: Pos, item: ItemId, reach: u32, defs: &Defs) -> Option<Pos> {
        self.zones
            .iter()
            .filter(|&&z| chebyshev(pos, z) <= reach && self.room_at(z, item, defs) > 0)
            .min_by_key(|&&z| (chebyshev(pos, z), z))
            .copied()
    }

    /// Spends the chi and takes effect. The target was just validated. Returns the stack Wind
    /// Carry moved, if that is what it was.
    fn apply_bend(&mut self, i: usize, def: &AbilityDef, pos: Pos, defs: &Defs) -> Option<Moved> {
        let u = &mut self.units[i];
        u.chi = (u.chi - def.chi).max(0.0);
        let magnitude = self.bend_magnitude(def);
        let caster = self.units[i].id;
        self.apply_effect(&def.effect, caster, pos, magnitude, defs)
    }

    // ---- the effects: the only matches on `Effect` ----

    fn validate_effect(
        &self,
        effect: &Effect,
        pos: Pos,
        magnitude: f32,
        defs: &Defs,
    ) -> Result<(), &'static str> {
        match effect {
            Effect::Reshape { from, .. } => {
                let tile = &self.map[pos];
                if !from.contains(&tile.terrain) {
                    // Raise Earth works water, Bore Tunnel (#132) works mountains.
                    let rock = from.contains(&Terrain::Mountain);
                    return Err(match tile.terrain {
                        Terrain::Ford => "already a ford",
                        Terrain::Tunnel => "already a tunnel",
                        _ if rock => "needs a mountain tile",
                        Terrain::Water | Terrain::Coast => "the sea is out of reach",
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
            // An empty open tile. A lit flame is a fine target too: that restokes it.
            Effect::HeatSource { .. } => {
                let tile = &self.map[pos];
                if !matches!(tile.terrain, Terrain::Land | Terrain::Coast) {
                    Err("needs open ground to kindle")
                } else if tile.resource.is_some() || tile.building.is_some() {
                    Err("something is in the way")
                } else {
                    Ok(())
                }
            }
            // Somewhere to stand and channel: open ground with nothing on it. The aura lands on
            // the caster, so there is nothing else about the tile to check.
            Effect::HeatAura { .. } => {
                let tile = &self.map[pos];
                if !tile.terrain.is_walkable() {
                    Err("needs open ground to stand on")
                } else if tile.resource.is_some() || tile.building.is_some() {
                    Err("something is in the way")
                } else {
                    Ok(())
                }
            }
            // Somewhere to stand and feel the ground: open ground or a tunnel with nothing on it.
            // Whether anything is buried nearby is what the bend finds out, so it isn't checked.
            Effect::Reveal { .. } => {
                let tile = &self.map[pos];
                if !tile.terrain.is_walkable() {
                    Err("needs open ground to stand on")
                } else if tile.resource.is_some() || tile.building.is_some() {
                    Err("something is in the way")
                } else {
                    Ok(())
                }
            }
            // A ground stack with a stockpile tile in reach that could take at least some of it.
            Effect::WindCarry { range } => {
                let Some(stack) = self.ground.get(&pos) else {
                    return Err("needs a stack to blow");
                };
                let reach = scaled_range(*range, magnitude);
                if self.stockpile_in_reach(pos, stack.item, reach, defs).is_some() {
                    Ok(())
                } else {
                    Err("no stockpile with room in reach")
                }
            }
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

    fn apply_effect(
        &mut self,
        effect: &Effect,
        caster: UnitId,
        pos: Pos,
        magnitude: f32,
        defs: &Defs,
    ) -> Option<Moved> {
        match effect {
            // The new terrain is permanent (no undo in v1). It changes who can walk where, so
            // the cached regions are stale and idle units should look for jobs that opened up.
            // Boring into a buried seam (#133) strikes it instead: the seam is uncovered and the
            // rock stays, so the miners have something to work from beside it.
            Effect::Reshape { to, .. } => {
                if self.uncover(pos) {
                    self.wake_idle();
                    return None;
                }
                self.map[pos].terrain = *to;
                self.map_changed();
                self.wake_idle();
                None
            }
            // The whole stack, ignoring carry capacity, to the nearest stockpile tile with room
            // within reach, re-checked now (a destination `validate_effect` saw may have filled
            // up since). What doesn't fit stays on the ground with a fresh haul job.
            Effect::WindCarry { range } => {
                let stack = self.ground.get(&pos).copied()?;
                let reach = scaled_range(*range, magnitude);
                let dest = self.stockpile_in_reach(pos, stack.item, reach, defs)?;
                let put = stack.count.min(self.room_at(dest, stack.item, defs));
                self.stored
                    .entry(dest)
                    .and_modify(|s| s.count += put)
                    .or_insert(Stack { item: stack.item, count: put });
                let rest = stack.count - put;
                if rest > 0 {
                    self.ground.insert(pos, Stack { item: stack.item, count: rest });
                    self.ensure_haul_job(pos);
                } else {
                    self.ground.remove(&pos);
                }
                Some(Moved { to: dest, item: stack.item, count: put })
            }
            // The magnitude scales how long the flame burns. Lighting a lit tile restokes it.
            Effect::HeatSource { duration_minutes, radius, sleep_bonus, seek_tiles } => {
                self.light_heat_source(HeatSource {
                    pos,
                    radius: *radius,
                    sleep_bonus: *sleep_bonus,
                    seek_tiles: *seek_tiles,
                    until: self.tick + minutes_to_ticks(duration_minutes * magnitude),
                    size: crate::building::ONE_BY_ONE,
                });
                None
            }
            // The aura goes to the caster and follows them. The magnitude scales how long it
            // lasts; casting it again resets the timer.
            Effect::HeatAura { duration_minutes, radius, sleep_bonus } => {
                self.light_aura(Aura {
                    unit: caster,
                    radius: *radius,
                    sleep_bonus: *sleep_bonus,
                    until: self.tick + minutes_to_ticks(duration_minutes * magnitude),
                });
                None
            }
            // What it finds within the (scaled) radius comes onto the map.
            Effect::Reveal { radius, finds } => {
                let reach = scaled_range(*radius, magnitude);
                let near: Vec<Pos> = self
                    .buried
                    .iter()
                    .filter(|&(&b, r)| chebyshev(pos, b) <= reach && finds.contains(r))
                    .map(|(&b, _)| b)
                    .collect();
                let mut found = false;
                for b in near {
                    found |= self.uncover(b);
                }
                if found {
                    self.wake_idle();
                }
                None
            }
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
                None
            }
            #[cfg(test)]
            Effect::Probe => None,
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
mod air_tests;
#[cfg(test)]
mod earth_tests;
#[cfg(test)]
mod fire_tests;
#[cfg(test)]
mod glide_tests;
#[cfg(test)]
mod seismic_tests;
#[cfg(test)]
mod tests;
#[cfg(test)]
mod tunnel_tests;
#[cfg(test)]
mod water_tests;
