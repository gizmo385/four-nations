//! Bending at work: designating an ability, a unit channeling it, and what it does.
//!
//! A bend is a job like a harvest (`JobKind::Bend`). The player designates a tile
//! (`Command::Bend`), which the settlement's own nation must be able to use, or failing that
//! someone living here must (`check_ability`: a foreign-nation unit, or the Avatar, #237); a unit
//! whose chi covers the cost and who is allowed to use the ability claims it (`unit_can_bend`,
//! #239: a signature ability needs only that its nation matches the unit's own, home settler or
//! not; a branch ability also needs the settlement's research, so only a home-nation unit can
//! take it; #237: the Avatar may claim any of the four signature abilities, whatever his own
//! "nation" -- he has none -- and whatever the settlement's), walks to the tile and channels for
//! `AbilityDef::work` seconds (divided by the `speed_modifier` named in `data/bending.toml`).
//! Needs pre-empt it and its progress is kept, exactly as for harvests. **Chi is spent only when
//! the bend completes**; since a unit holds one job at a time it can't be spent twice. If the
//! target is no longer valid when the channel finishes, the job is dropped, no chi is spent and
//! the event log says why.
//!
//! **This file holds every `match` on `Effect`** (AGENTS.md rule 7): `validate_effect` says
//! whether a target is acceptable and `apply_effect` changes the world. The four real effects
//! are #68 Raise Earth, #69 Water the Land, #70 Hearthflame and #71 Wind Carry (Metalbend, #134,
//! joined them later). Everything else here is effect-agnostic.
//!
//! **Raise Earth** (#68) turns a river or lake tile with dry ground on one of its four sides
//! into a `Ford`. Fords are permanent. The regions cache is invalidated and idle units woken,
//! since jobs across the water may have become reachable. **Bore Tunnel** (#132, below) shares
//! this same `Reshape` effect on mountains instead of water, and so shares everything below about
//! chained designation and claim order.
//!
//! A `Reshape` tile with no dry ground beside it may still be designated, chained off an
//! orthogonal neighbour's own pending `Reshape` designation of the same effect (#304): since a
//! pending designation only ever exists because it validated too, by the bank or a chain of its
//! own, a whole dragged line or rectangle can be designated at once, out from dry ground, without
//! waiting on each tile in turn. A tile that chains to nothing -- no bank, no designated neighbour
//! -- is still refused ("needs dry ground beside it"). The chain can't be *worked* out of that
//! order, though: `bend_ready` keeps a job unclaimable until its tile actually has walkable
//! ground beside it, which for an inner tile means the neighbour it chained off has actually
//! been reshaped, so the chain is bored or raised bank outward, one real tile at a time, however
//! many units are free to work it. Cancelling a designation mid-chain only withdraws that one
//! tile: whatever is designated beyond it stays designated, simply unclaimable, until some other
//! bank reaches it (or the player cancels those too).
//!
//! **Water the Land** (#69) leaves a `Patch` behind: a 3x3 area that stays wet for a while. When
//! `tick_irrigation` finds a patch dry (patches dry in `(tick, centre)` order) up to `sprouts` of
//! its empty, fertile, non-fallow tiles grow the sprout resource, picked with the `bending.sprout`
//! stream. A tile that sprouted is fallow for a while and can't sprout again. A tile that holds a
//! building or a site, or whose sprout would seal ground off for good (`World::would_cut_off`,
//! #490: a cabbage blocks movement for as long as it stands), is skipped. The patch carries the
//! effect's numbers with it, so drying never has to look at the ability again.
//!
//! **Hearthflame** (#70) lights a `HeatSource` on an empty Land or Coast tile for
//! `duration_minutes` x the `heating` modifier; bending a lit tile again restokes it, resetting
//! the timer, but only once it has burned below `restoke_below`'s share of a fresh burn (#359):
//! restoking a flame with more than that share left is refused ("flame still burning, restoke
//! below half"), both when designated and when the channel finishes (the flame may have been
//! restoked by someone else, or burned down, in between). What a lit source does (cooked meals,
//! faster and sought-out sleep) lives in `heat.rs`, which a campfire (#58) can reuse. Earth's
//! Earth Tent and Air's Air Shield (#362) are the same `HeatSource` with `cooks = false`: a
//! shelter, not a flame, so meals eaten within reach stay raw (`heat.rs`'s `is_cooking`), but the
//! cold and sleep stay exactly as above. Both share Hearthflame's restoke rule above (at its own
//! default of "always", unless the ability's own data says otherwise).
//!
//! **Inner Fire** (#123) is the one self-cast ability: the designated tile is where the bender
//! stands to channel (open ground, like a flame), and the aura goes to *whoever channelled it*,
//! not to the tile. It is an `Aura` (`heat.rs`) that follows its carrier for `duration_minutes` x
//! the `heating` modifier; casting again recasts it, resetting the timer, under the same
//! `restoke_below` rule (#359) applied to the carrier's own aura instead of a tile: since the
//! aura belongs to a unit, not a position, `validate_effect`'s check is a settlement-wide
//! feasibility check (is anyone's aura cool enough to recast right now), and `unit_can_bend`'s
//! own check on the specific unit is what actually gates whether it can claim or keep the job.
//!
//! **Wind Carry** (#71) sends the whole stack on the ground tile, ignoring carry capacity, to
//! the nearest stockpile tile with room (holding that item, or empty) within `range` tiles of
//! Chebyshev distance x the `power_generation` modifier, straight over rivers and mountains.
//! Whatever doesn't fit stays on the ground with a fresh haul job. Designating it over a ground
//! stack takes over the tile's existing haul job: a hauler who claimed it but hasn't walked
//! there yet is released back to idle, and the haul job comes back if the bend is cancelled or
//! the target is no longer valid when the channel finishes.
//!
//! **Metalbend** (#134, from `metalbending`) turns a ground stack of ore into metal, in place:
//! the same conversion the Fire Nation's smelter does (`data/buildings.toml`'s `smelter`), but
//! on the spot, with no building, no haul to one and no steady batches, at the cost of chi and a
//! channel instead. Any ore left over once the stack divides by the recipe's `ratio` stays
//! behind, as ore. It shares Wind Carry's tile-ownership rule: designating it over a ground stack
//! takes over the tile's haul job, given back if the bend is cancelled or the stack is gone or
//! too small when the channel finishes.
//!
//! The target may also be a stockpile tile holding at least a batch's worth of ore (#398, from
//! #376: by the time Earth researches `metalbending`, every vein in reach is usually mined out
//! and the ore is already stockpiled, so a bare ground stack is rarely there to bend). The
//! conversion is the same either way (`validate_effect`/`apply_effect` read whichever of
//! `ground` or `stored` holds the target), but the metal's destination differs: a ground
//! stack's metal lands back on the same tile when nothing's left of the ore (`place_ground`'s
//! own search tries the bent tile first); a stockpile tile is never offered that way
//! (`place_ground` always skips zone tiles), so the metal instead starts its own haul job
//! nearby and finds a stockpile the ordinary way, never overwriting the ore entry a build or
//! craft job may already be mid-trip to fetch from (`take_for_build` trusts whatever is at the
//! position it was sent to, so the ore at that key must stay ore, never metal, until it's
//! actually gone). There is no haul job to take over on a stockpile tile -- the ore isn't
//! waiting to be hauled -- so `owns_ground_stack`'s take-over is simply a no-op there.
//!
//! **Air Scooter** (#143, from `air_scooter`) is the second self-cast ability, like Inner Fire:
//! the designated tile is only where the bender stands to summon it (open ground, like Inner
//! Fire's aura). It leaves a `Ride` on the caster: for `duration_minutes` x the `move_speed`
//! modifier, every place that reads a unit's effective move speed (`walk`, `wander` and
//! `Stride`) multiplies it by `multiplier` on top of everything else. It ends with its timer, not
//! with distance covered; casting it again resets the timer. Unlike Inner Fire it does nothing
//! to anyone nearby, so there is no interaction with heat or sleep.
//!
//! **Weld** (Fire, from `forging`) and **Lift** (Air, from `windmills`, #363) both add
//! `work_seconds` x the ability's own magnitude to an ordinary build site's banked work
//! (`World::add_build_work`), capped at the building's total. Neither the Harmony Gate (its
//! stages are `gate.rs`'s own) nor a site whose work is already banked in full is a valid
//! target (`World::build_target`). Work banked before the site's materials arrive is kept: there
//! is no special case for an unsupplied site, since `run_build`'s own completion check already
//! finishes a fully-worked site the moment its materials are too.
//!
//! **Draw Water** (Water, from `irrigation`, #364) shares Raise Earth's `Reshape` effect, but on
//! dry land instead of water: an empty `Land` tile becomes a `Ford` too, except this one is
//! temporary, through `Reshape`'s new `revert_minutes`. The old terrain and the tick it reverts
//! at are kept in `reverts` (a `Vec<Revert>`, sorted the way `patches` is), and `tick_reverts`
//! restores it, oldest-then-position first, marking the regions cache dirty and waking idle
//! units exactly as the forward change does: nothing about reverting is special-cased, so a
//! future `Reshape` whose `to` isn't walkable would strand nobody only if its own `from` wasn't
//! either, which is Earth's call, not Water's. Since the target is already walkable `Land`,
//! `validate_effect`'s `Reshape` arm skips the bank/chain rule entirely for it: the bender just
//! stands where it's working, the way Water the Land or Hearthflame do, and a tile with a
//! resource, a building, a ground stack or a stockpile on it is refused ("something is in the
//! way") the same as an occupied target anywhere else. Raise Earth and Bore Tunnel are
//! unaffected: their own targets are never walkable before the bend, so they still need the bank.
//!
//! **Carve Canal** (Water, from `canals`, #695) is a `Reshape` with a `source`, cutting `Land` or
//! `Coast` into a permanent `Canal`: fresh water that is not walkable. The `source` terrains
//! (river, lake, canal; never the sea) take the place of the bank: the target must touch one
//! orthogonally, or chain off a pending designation of the same bend, and `bend_ready` holds a
//! chained tile until the water really reaches it. The target is walkable, so the bender stands
//! *beside* it (`StandRule::Beside`), and a finished channel waits until no unit is on the tile.
//! A cut is refused ("would cut the land in two") when it would leave its walkable neighbours in
//! two regions: `World::would_cut_off`'s flood, at designation (with the other pending canal
//! tiles counted as carved) and again when the channel finishes. `[canal]` in `bending.toml` sets
//! the irrigation a canal tile gives (`World::irrigation_rate`). A canal blocks walking exactly as
//! a river does; #201 decides what Water's people may do about that.
//!
//! **Cut Ice** (Water, from `ice_construction`, #364) is the new `DrawItem` effect: the target is
//! a river, lake or sea tile beside passable ground, like Raise Earth's bank, but nothing about
//! the target tile changes. Instead `count` of `item` (ice) appears on the tile the bender
//! actually ended up standing on to reach it (`place_ground`, the same way a dead unit's load
//! falls where it stood) -- the water tile itself can never hold a stack, since nobody can stand
//! there to receive one.

use rand::seq::SliceRandom;

use crate::bending::{AbilityDef, Effect, Rank};
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
    /// The ability belongs to another nation, and nobody living here (the settlement's own
    /// nation, a guest, or #240's newcomers) can cast it.
    WrongNation,
    /// The ability needs a rank (`AbilityDef::rank`, #360) nobody living here holds yet (the
    /// Avatar aside, if it's one of his own): unlike `WrongNation`, the right nation is here,
    /// just not anyone ranked for it. Without this check a rank-gated ability would still
    /// designate (nation and tech are all `check_ability` looks at), just sit forever
    /// unclaimable, so #361's bend picker ("Causeway needs an Earthbending Master") needs the
    /// settlement to know up front. `ability` and `rank` are carried (not just the rank alone,
    /// `Locked`'s own pattern) so a frontend with `Defs` can name both the ability and this
    /// nation's own title for the rank.
    NeedsRank { ability: AbilityId, rank: Rank },
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
            Self::WrongNation => f.write_str("nobody here can cast this"),
            Self::NeedsRank { .. } => f.write_str("nobody here holds the rank for this"),
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

/// A `Reshape` with `revert_minutes` set (Draw Water, #364), waiting to turn back: the terrain
/// `pos` had before the bend, and the tick it reverts at. Kept sorted by `(at, pos)`, like
/// `patches`, so `tick_reverts` restores them in that order.
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
pub(super) struct Revert {
    pos: Pos,
    terrain: Terrain,
    at: u64,
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

/// A speed burst a unit is riding (Air Scooter, #143). While it lasts, the rider's `move_speed`
/// is multiplied by `multiplier` on top of every other modifier.
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
pub(super) struct Ride {
    pub multiplier: f32,
    /// The first tick it is over. The rider is on it while `tick < until`.
    pub until: u64,
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

/// `pos`'s four orthogonal neighbours (Raise Earth's and Bore Tunnel's bank, #304: a diagonal
/// doesn't count, as `targets_are_river_or_lake_tiles_beside_dry_ground` pins down).
fn orthogonal(pos: Pos) -> [Pos; 4] {
    [
        Pos::new(pos.x, pos.y - 1),
        Pos::new(pos.x + 1, pos.y),
        Pos::new(pos.x, pos.y + 1),
        Pos::new(pos.x - 1, pos.y),
    ]
}

/// `range` scaled by the ability's magnitude (Wind Carry's `power_generation`), rounded to the
/// nearest tile.
fn scaled_range(range: u32, magnitude: f32) -> u32 {
    ((range as f32) * magnitude).max(0.0).round() as u32
}

/// Whether `effect` works a ground stack at its target tile the way Wind Carry does (#71) and
/// Metalbend does (#134): designating it takes over the tile's existing haul job, and the job
/// comes back if the bend is cancelled or fails. Metalbend's own target may be a stockpile tile
/// instead (#398), which never has a haul job of its own to take over, so this is simply a
/// no-op there.
fn owns_ground_stack(effect: &Effect) -> bool {
    matches!(effect, Effect::WindCarry { .. } | Effect::Smelt { .. })
}

/// Why a `Reshape`'s target isn't one of `from`'s terrains, worded for whichever ability this
/// is: Raise Earth's river or lake, Bore Tunnel's mountain (#132), or Draw Water's dry land
/// (#364). `terrain` is the tile's actual terrain.
fn reshape_mismatch(from: &[Terrain], terrain: Terrain) -> &'static str {
    if terrain == Terrain::Ford {
        return "already a ford";
    }
    if terrain == Terrain::Tunnel {
        return "already a tunnel";
    }
    if terrain == Terrain::Canal {
        return "already a canal";
    }
    if from.contains(&Terrain::Mountain) {
        return "needs a mountain tile";
    }
    if from.contains(&Terrain::Land) {
        return "needs dry land";
    }
    match terrain {
        Terrain::Water | Terrain::Coast => "the sea is out of reach",
        _ => "needs a river or lake tile",
    }
}

/// Whether `effect` cuts into ground a unit can walk on and leaves something it can't (Carve
/// Canal, #695), when its target has `terrain` now. The bender can't stand on such a tile: it
/// stands beside it, and the cut waits until no unit is on the tile.
fn carves(effect: &Effect, terrain: Terrain) -> bool {
    matches!(effect, Effect::Reshape { to, .. } if !to.is_walkable()) && terrain.is_walkable()
}

/// Why a `Reshape` with a `source` has none beside its target: Carve Canal's fresh water.
fn source_missing(source: &[Terrain]) -> &'static str {
    if source.iter().all(|t| t.is_fresh()) {
        "needs fresh water beside it"
    } else {
        "needs its source beside it"
    }
}

/// Queries and the designations behind `Command::Bend` and `Command::CancelBend`.
impl World {
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
            let rate = self.bend_rate(u, def, defs);
            let remaining_secs = (def.work - job.progress).max(0.0) / rate.max(1e-3);
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

    /// Can `ability` be designated right now: the settlement's own nation can use it, or failing
    /// that, someone living here can (a foreign-nation unit, or the Avatar, #237 -- "nobody here
    /// can cast this" otherwise), and its tech (if any) is known. For the client to decide which
    /// abilities to offer.
    pub fn check_ability(&self, ability: AbilityId, defs: &Defs) -> Result<(), BendError> {
        let def = defs.abilities.get(ability.0 as usize).ok_or(BendError::UnknownAbility)?;
        let avatar_knows = defs.endeavor.avatar.as_ref().is_some_and(|a| {
            a.abilities.contains(&ability) && self.units.iter().any(Unit::is_avatar)
        });
        if def.nation != self.nation
            && !avatar_knows
            && !self.units.iter().any(|u| self.nation_of(u) == def.nation)
        {
            return Err(BendError::WrongNation);
        }
        match def.unlocked_by {
            Some(tech) if !self.research.is_known(tech) => Err(BendError::Locked { tech }),
            _ => Ok(()),
        }
    }

    /// Can `ability` actually be *designated* right now, given its rank gate (#360, #361),
    /// beyond `check_ability`'s nation and tech checks: most abilities have no `rank` at all,
    /// which this always lets through. Deliberately not folded into `check_ability`, which
    /// `usable_abilities` (#248) also reads to decide what the Bend tool cycles through and the
    /// picker lists: a rank-gated ability stays listed and cyclable even while nobody holds the
    /// rank (docs/design/bending.md "Seeing it", "it stays listed while nobody does"), it's only
    /// *designating* it that this refuses, so the job never sits forever unclaimable
    /// (`unit_can_bend`'s own `meets_rank` is what a claim re-checks, and would always fail the
    /// same way). The Avatar counts only through his own `[avatar]` list, matching `can_cast`.
    fn check_rank(
        &self,
        ability: AbilityId,
        def: &AbilityDef,
        defs: &Defs,
    ) -> Result<(), BendError> {
        let Some(rank) = def.rank else { return Ok(()) };
        let avatar_can =
            defs.endeavor.avatar.as_ref().is_some_and(|a| a.abilities.contains(&ability))
                && self.units.iter().any(Unit::is_avatar);
        if avatar_can
            || self.units.iter().any(|u| !u.is_avatar() && self.rank_of(u, defs).rank >= rank)
        {
            Ok(())
        } else {
            Err(BendError::NeedsRank { ability, rank })
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
        let def = defs.ability(ability);
        self.check_rank(ability, def, defs)?;
        if self.bend_designations().any(|(_, p)| p == pos) {
            return Err(BendError::AlreadyDesignated);
        }
        self.validate_bend(def, pos, defs).map_err(BendError::InvalidTarget)?;
        let id = self.add_job(JobKind::Bend { ability, pos });
        // Wind Carry and Metalbend take over the tile's haul job (validation just confirmed a
        // stack is there).
        if owns_ground_stack(&def.effect) {
            self.take_haul_job(pos);
        }
        Ok(id)
    }

    /// Withdraws the bend designation on `pos`. Channeling done on it is lost. A Wind Carry or
    /// Metalbend designation gives the tile's haul job back, if the stack is still there.
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
        if owns_ground_stack(&defs.ability(ability).effect) && self.ground.contains_key(&pos) {
            self.ensure_haul_job(pos);
        }
        Ok(())
    }

    /// Whether `pos` has real, walkable ground on one of its four orthogonal sides right now:
    /// the bank Raise Earth and Bore Tunnel need, with no regard to what's merely designated
    /// (#304). `validate_effect`'s `Reshape` arm also accepts a tile chained to one of these
    /// through pending designations; this is the stricter, unchained check a job's claim needs,
    /// so a chain is worked bank outward, one real tile at a time.
    fn has_bank(&self, pos: Pos) -> bool {
        orthogonal(pos).into_iter().any(|p| self.walkable(p))
    }

    /// Whether one of `pos`'s four orthogonal neighbours is a terrain in `source` right now:
    /// Carve Canal's fresh water (#695), the stricter, unchained check a claim needs.
    fn has_source(&self, pos: Pos, source: &[Terrain]) -> bool {
        orthogonal(pos)
            .into_iter()
            .any(|p| self.map.get(p).is_some_and(|t| source.contains(&t.terrain)))
    }

    /// Whether an orthogonal neighbour of `pos` has a pending designation of `effect` (#304): the
    /// chain that lets a dragged line or rectangle be designated all at once. Each pending tile
    /// only ever got there by validating itself, by the bank or source or a chain of its own.
    fn chained_to_pending(&self, pos: Pos, effect: &Effect, defs: &Defs) -> bool {
        orthogonal(pos).into_iter().any(|q| {
            self.jobs.iter().any(|j| {
                matches!(j.kind, JobKind::Bend { ability, pos: p } if p == q
                    && defs.ability(ability).effect == *effect)
            })
        })
    }

    /// Whether cutting `pos` into `to` would leave its walkable neighbours in two regions under
    /// the settlement's current move rules (Carve Canal, #695). Every other tile with a pending
    /// designation of `effect` counts as already cut, so a long dragged line is checked tile by
    /// tile against the earlier ones; at completion the job is gone from the queue and the live
    /// map has whatever was really carved. A flood bounded to the land the tile touches
    /// (`would_cut_off`), cached while the map doesn't change.
    fn carve_splits(&self, pos: Pos, to: &Terrain, effect: &Effect, defs: &Defs) -> bool {
        let pending: Vec<Pos> = self
            .jobs
            .iter()
            .filter_map(|j| match j.kind {
                JobKind::Bend { ability, pos: p }
                    if p != pos
                        && self.map[p].terrain.is_walkable()
                        && defs.ability(ability).effect == *effect =>
                {
                    Some(p)
                }
                _ => None,
            })
            .collect();
        if pending.is_empty() {
            return self.would_cut_off(pos);
        }
        let mut map = self.map.clone();
        for p in pending {
            map[p].terrain = *to;
        }
        self.would_cut_off_on(&map, pos)
    }

    /// Where the bender stands for a bend on `pos`: on the tile, or beside it when the bend cuts
    /// a tile it couldn't then stand on (Carve Canal, #695).
    pub(super) fn bend_stand(&self, ability: AbilityId, pos: Pos, defs: &Defs) -> StandRule {
        match self.map.get(pos) {
            Some(t) if carves(&defs.ability(ability).effect, t.terrain) => StandRule::Beside(pos),
            _ => StandRule::Tile(pos),
        }
    }

    /// Is a `Bend` job on `pos` for `ability` reachable right now, beyond `unit_can_bend`'s chi
    /// and nation check (#304)? Only a `Reshape` whose target isn't already walkable cares
    /// (Raise Earth, Bore Tunnel): a tile designated by chaining off a still-pending neighbour
    /// (`validate_effect`) can't be worked until that neighbour has actually become walkable
    /// ground, so a chained line or rectangle is worked in order, bank outward, whoever claims
    /// it. Draw Water's target (#364) is already walkable `Land`, so it needs no bank at all and
    /// is always ready. Every other effect's target is ready as soon as it validates, which
    /// claiming re-checks itself by trying to stand on it.
    pub(super) fn bend_ready(&self, ability: AbilityId, pos: Pos, defs: &Defs) -> bool {
        match &defs.ability(ability).effect {
            // Carve Canal's target is walkable land, but it still waits on its water: the
            // source terrain really beside it, so a chained line is cut outward from the river.
            Effect::Reshape { source, .. } if !source.is_empty() => self.has_source(pos, source),
            Effect::Reshape { .. } if !self.walkable(pos) => self.has_bank(pos),
            _ => true,
        }
    }

    /// Removes the haul job on `pos`, if any, releasing a unit that claimed it but hasn't
    /// walked there yet. Per `run_haul`, a claimed haul job's unit never has picked anything up:
    /// the pickup and the claim's release happen in the same step.
    ///
    /// A need (hunger, thirst, energy) can have pre-empted the haul before this ever runs
    /// (`work.rs`'s "needs pre-empt jobs"): the claimant's `job_task` is set aside, not dropped,
    /// and its `route` is doing double duty for the need task instead (#386). Clearing `route`
    /// unconditionally here, as if the claimant were still walking the haul, stranded such a
    /// unit mid-need with nothing left to walk toward: `need_task` stayed set, but `route` went
    /// to `None`, so `walk` never returned true and the need was never met. Only `job_task` is
    /// safe to clear unconditionally; `route` only if the unit isn't mid-need (`finish_need`
    /// clears it once the need is actually done, same as any other interruption).
    pub(super) fn take_haul_job(&mut self, pos: Pos) {
        let Some(idx) = self.jobs.iter().position(|j| j.kind == JobKind::Haul { pos }) else {
            return;
        };
        let job = self.jobs.remove(idx);
        if let Some(unit_id) = job.claimed_by {
            if let Some(u) = self.units.iter_mut().find(|u| u.id == unit_id) {
                u.job_task = None;
                if u.need_task.is_none() {
                    u.route = None;
                }
            }
            self.wake_idle();
        }
    }
}

/// `World::rank_of`'s answer (#360): a unit's rank right now, the bends and forms behind it, and
/// what the next rank needs. `next` is `None` once at Master, since there's no rank above it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MasteryProgress {
    pub rank: Rank,
    pub bends: u32,
    pub forms: u32,
    /// The rank, bends and forms the next rank needs, or `None` already at Master.
    pub next: Option<(Rank, u32, u32)>,
}

/// Doing it.
impl World {
    /// `unit`'s bending rank right now (`docs/design/bending.md`, "The route: novice, adept,
    /// master", #360): computed live from its own finished bends and forms against its own
    /// nation's thresholds (`nation_of`, so a United City newcomer or the Avatar's own unit --
    /// though the Avatar never casts anything rank-gated -- is judged by its own nation's
    /// numbers, never the settlement's). Never cached: ranks only rise as bends and forms do, so
    /// recomputing it is always correct, even after `[mastery]` is retuned or a save from before
    /// `bends_by` existed loads with none.
    pub fn rank_of(&self, unit: &Unit, defs: &Defs) -> MasteryProgress {
        let nation = self.nation_of(unit);
        let bends = unit.bends();
        let forms = unit.forms();
        let m = &defs.mastery;
        let master_bends = m.master_bends_for(nation);
        let adept_bends = m.adept_bends_for(nation);
        let rank = if bends >= master_bends && forms >= m.master_forms {
            Rank::Master
        } else if bends >= adept_bends && forms >= m.adept_forms {
            Rank::Adept
        } else {
            Rank::Novice
        };
        let next = match rank {
            Rank::Novice => Some((Rank::Adept, adept_bends, m.adept_forms)),
            Rank::Adept => Some((Rank::Master, master_bends, m.master_forms)),
            Rank::Master => None,
        };
        MasteryProgress { rank, bends, forms, next }
    }

    /// Whether `unit` is allowed to use `ability` at all, whatever its chi (`unit_can_bend` adds
    /// that check). The Avatar (#237) may claim any ability his own `[avatar]` list names,
    /// whatever his "nation" (he has none) and whatever the settlement's. Otherwise (#239): a
    /// signature ability (no tech unlocks it) needs only that the unit's own nation matches: any
    /// unit of that nation can use it, home settler or not, since it needs no research to begin
    /// with. A branch ability needs the settlement's research too, so only a home-nation unit
    /// (one with no nation of its own, or explicitly the settlement's) can take it: a unit of
    /// another nation doesn't benefit from techs its own nation hasn't researched (teaching
    /// across nations is deferred). Public so a frontend can count who could cast an ability
    /// (`caster_count`, #248: the bend picker's "Raise Earth: 14 benders").
    pub fn can_cast(&self, unit: &Unit, ability: AbilityId, defs: &Defs) -> bool {
        let def = defs.ability(ability);
        if unit.is_avatar() {
            return defs.endeavor.avatar.as_ref().is_some_and(|a| a.abilities.contains(&ability));
        }
        let unit_nation = self.nation_of(unit);
        if unit_nation != def.nation {
            return false;
        }
        let tech_known = match def.unlocked_by {
            None => true,
            Some(tech) => unit_nation == self.nation && self.research.is_known(tech),
        };
        tech_known && self.meets_rank(unit, def, defs)
    }

    /// Whether `unit`'s own rank (`rank_of`) is at least `def.rank`, if it names one (#360):
    /// always true for a signature or any other ability with no rank gate. Like a signature
    /// ability, this needs no home-settlement check (`can_cast`'s `unlocked_by` match does for a
    /// branch ability): rank is purely the caster's own, so a United City newcomer is judged
    /// against their own nation's thresholds, never the settlement's.
    fn meets_rank(&self, unit: &Unit, def: &AbilityDef, defs: &Defs) -> bool {
        match def.rank {
            None => true,
            Some(required) => self.rank_of(unit, defs).rank >= required,
        }
    }

    /// What `unit` pays for `def`'s bend right now: `master_chi` of the ability's own `chi` once
    /// the unit's rank is Master (#360), or the full cost otherwise. Shared by `unit_can_bend`'s
    /// claim check and `apply_bend`'s actual spend, so a unit can never claim at one rate and pay
    /// another.
    fn chi_cost(&self, unit: &Unit, def: &AbilityDef, defs: &Defs) -> f32 {
        if self.rank_of(unit, defs).rank == Rank::Master {
            def.chi * defs.mastery.master_chi
        } else {
            def.chi
        }
    }

    /// Whether unit `i` may take a bend job for `ability` right now: its chi covers the cost
    /// (a master's own discount, #360), `can_cast` allows it, and (Inner Fire's restoke rule,
    /// #359) its own aura, if it has one from a prior cast, isn't still too fresh to recast. The
    /// aura check is per-unit because the aura belongs to a unit, not a tile (`validate_effect`'s
    /// `HeatAura` arm is the settlement-wide feasibility half of the same rule).
    pub(super) fn unit_can_bend(&self, i: usize, ability: AbilityId, defs: &Defs) -> bool {
        let def = defs.ability(ability);
        let u = &self.units[i];
        u.chi >= self.chi_cost(u, def, defs)
            && self.can_cast(u, ability, defs)
            && self.unit_can_restoke(u.id, def)
    }

    /// Whether `unit` is clear to carry a fresh cast of `def`'s effect right now: always true
    /// unless it is a `HeatAura` and `unit` already carries one with more than `restoke_below`'s
    /// share of a fresh cast left (#359).
    fn unit_can_restoke(&self, unit: UnitId, def: &AbilityDef) -> bool {
        let Effect::HeatAura { duration_minutes, restoke_below, .. } = &def.effect else {
            return true;
        };
        let magnitude = self.bend_magnitude(def);
        let until = self.auras.get(&unit).map(|a| a.until);
        !self.too_fresh_to_restoke(until, *duration_minutes, magnitude, *restoke_below)
    }

    /// How many living units could cast `ability` at all, `can_cast`'s chi-free eligibility
    /// (#248): for the bend picker's count of casters, which shouldn't flicker as chi regens.
    pub fn caster_count(&self, ability: AbilityId, defs: &Defs) -> usize {
        self.units.iter().filter(|u| self.can_cast(u, ability, defs)).count()
    }

    /// Channel seconds per second: the caster's own effective modifiers (`modifiers_for`, #239;
    /// the Avatar's are his own, #237) read by the nation modifier the ability names, or 1.0.
    fn bend_rate(&self, u: &Unit, def: &AbilityDef, defs: &Defs) -> f32 {
        def.speed_modifier
            .as_deref()
            .and_then(|m| self.modifiers_for(u, defs).get(m))
            .unwrap_or(1.0)
    }

    pub(super) fn run_bend(&mut self, i: usize, id: JobId, defs: &Defs) {
        let Some(idx) = self.job_index(id) else { return self.finish_job(i) };
        let JobKind::Bend { ability, pos } = self.jobs[idx].kind else {
            return self.finish_job(i);
        };
        let def = defs.ability(ability);
        if !self.unit_can_bend(i, ability, defs) {
            // Chi can only have gone up since the claim, so this is a safety net.
            return self.abandon_job(i, id, defs);
        }
        let stand = self.bend_stand(ability, pos, defs);
        if !self.go_or_abandon(i, id, stand, defs) {
            return;
        }
        let rate = self.bend_rate(&self.units[i], def, defs);
        let job = &mut self.jobs[idx];
        job.progress += rate / TICKS_PER_SECOND as f32;
        if job.progress < def.work {
            return;
        }
        // A cut into walkable ground waits until nobody stands on the tile (#695): the channel is
        // done, and it finishes the moment the tile is empty.
        if stand == StandRule::Beside(pos) && self.units.iter().any(|u| u.pos == pos) {
            return;
        }
        self.jobs.remove(idx);
        self.finish_job(i);
        let mut moved = None;
        let outcome = match self.validate_bend(def, pos, defs) {
            Ok(()) => {
                moved = self.apply_bend(i, ability, def, pos, defs);
                self.units[i].bends += 1;
                *self.units[i].bends_by.entry(ability).or_insert(0) += 1;
                self.note_rank_up(i, defs);
                BendOutcome::Done
            }
            Err(why) => {
                // Wind Carry's and Metalbend's target was a ground stack that had taken over the
                // haul job: give it back, if the stack is still there to haul.
                if owns_ground_stack(&def.effect) && self.ground.contains_key(&pos) {
                    self.ensure_haul_job(pos);
                }
                BendOutcome::Failed(why)
            }
        };
        let unit = self.units[i].id;
        self.log_event(BendEvent { tick: self.tick, unit, ability, pos, outcome, moved });
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

    /// The restoke rule (#359): whether something still lit until `until` (a flame's tile, or an
    /// aura's carrier) has more than `restoke_below`'s share of a fresh `duration_minutes` x
    /// `magnitude` cast left to run. `None`, or anything already out, is never too fresh.
    fn too_fresh_to_restoke(
        &self,
        until: Option<u64>,
        duration_minutes: f32,
        magnitude: f32,
        restoke_below: f32,
    ) -> bool {
        let Some(until) = until.filter(|&u| u > self.tick) else { return false };
        let full = minutes_to_ticks_min1(duration_minutes * magnitude) as f32;
        (until - self.tick) as f32 > restoke_below * full
    }

    /// Whether every living unit's own aura (if it has one) is still too fresh to recast, so
    /// nobody in the settlement could restoke Inner Fire right now: the settlement-wide half of
    /// the restoke rule (#359), checked by `validate_effect` at designation and again at
    /// completion. A unit with no aura at all is always free to cast one, so this is only true
    /// once every living unit (if any) carries a fresh one. `unit_can_bend`'s `unit_can_restoke`
    /// is what actually decides which unit may claim or finish the job.
    fn nobody_can_restoke_aura(
        &self,
        duration_minutes: f32,
        magnitude: f32,
        restoke_below: f32,
    ) -> bool {
        !self.units.is_empty()
            && self.units.iter().all(|u| {
                let until = self.auras.get(&u.id).map(|a| a.until);
                self.too_fresh_to_restoke(until, duration_minutes, magnitude, restoke_below)
            })
    }

    /// Brings the resource buried on `pos`, if any, onto the map (#133). Returns whether it did.
    /// A buried resource under open land (#197's lost ruins; gold seams in the rock, far more
    /// rarely) can have been built or walked over since the map was made: one under a building,
    /// a building site, a stockpile zone, a ground stack or a bend designation stays buried, and
    /// so does one a unit is standing on or routed through, since a resource there would clash
    /// with whatever is already using the tile, and so does one that would seal ground off for
    /// good (`would_cut_off`, #490). A later reveal finds it once the tile clears.
    /// `pub(super)` so a Map Room's Chart job (`world/chart.rs`, #402) can uncover too.
    pub(super) fn uncover(&mut self, pos: Pos) -> bool {
        if !self.buried.contains_key(&pos) || !self.free_to_uncover(pos) {
            return false;
        }
        let r = self.buried.remove(&pos).expect("just checked it's there");
        self.map[pos].resource = Some(r);
        self.map_changed();
        true
    }

    /// `pub(super)` so `world/chart.rs` can filter candidates the same way `Effect::Reveal` and
    /// Seismic Sense do.
    pub(super) fn free_to_uncover(&self, p: Pos) -> bool {
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
            // A ruin in the one way through a narrow place stays buried rather than wall off
            // what is beyond it (#490); rock a seam is bored out of isn't walkable, so it passes.
            && !self.would_cut_off(p)
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

    /// Spends the chi (a master's own discount, #360) and takes effect. The target was just
    /// validated. Returns the stack Wind Carry moved, if that is what it was.
    fn apply_bend(
        &mut self,
        i: usize,
        ability: AbilityId,
        def: &AbilityDef,
        pos: Pos,
        defs: &Defs,
    ) -> Option<Moved> {
        let cost = self.chi_cost(&self.units[i], def, defs);
        let u = &mut self.units[i];
        u.chi = (u.chi - cost).max(0.0);
        let magnitude = self.bend_magnitude(def);
        let caster = self.units[i].id;
        self.apply_effect(&def.effect, ability, caster, pos, magnitude, defs)
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
            Effect::Reshape { from, to, source, .. } => {
                let tile = &self.map[pos];
                if !from.contains(&tile.terrain) {
                    return Err(reshape_mismatch(from, tile.terrain));
                }
                // A fishing spot, a building, a ground stack or a stockpile (or anything else) on
                // the tile would be left on the new terrain, or lost under it.
                if tile.resource.is_some()
                    || tile.building.is_some()
                    || self.ground.contains_key(&pos)
                    || self.zones.contains(&pos)
                {
                    return Err("something is in the way");
                }
                // Carve Canal (#695) names what the target must touch, in place of the bank: that
                // terrain beside it, or a pending designation of this same bend beside it (the
                // chain, below). And it must leave the land in one piece.
                if !source.is_empty() {
                    if !self.has_source(pos, source) && !self.chained_to_pending(pos, effect, defs)
                    {
                        return Err(source_missing(source));
                    }
                    if carves(effect, tile.terrain) && self.carve_splits(pos, to, effect, defs) {
                        return Err("would cut the land in two");
                    }
                    return Ok(());
                }
                // Draw Water's land is already walkable (#364): the bender stands right on it,
                // the way Water the Land or Hearthflame do, so there's no bank or chain to check.
                if tile.terrain.is_walkable() {
                    return Ok(());
                }
                if self.has_bank(pos) {
                    return Ok(());
                }
                // Chained off a neighbour's pending designation of this same effect (#304): it
                // was itself only ever added here once it validated, by the bank or a chain of
                // its own, so a dragged line or rectangle can be designated all at once, out
                // from dry ground, each tile accepted before any of it is actually bored or
                // raised. A tile that chains to nothing -- no bank, no designated neighbour -- is
                // still refused. The chain can't be *worked* out of this order: `bend_ready`.
                if self.chained_to_pending(pos, effect, defs) {
                    Ok(())
                } else {
                    Err("needs dry ground beside it")
                }
            }
            // A river, lake or sea tile beside passable ground, like Raise Earth's bank: nothing
            // about the tile itself needs to be empty, since the bend doesn't touch it.
            Effect::DrawItem { from, .. } => {
                let tile = &self.map[pos];
                if !from.contains(&tile.terrain) {
                    Err("needs river, lake or sea")
                } else if self.has_bank(pos) {
                    Ok(())
                } else {
                    Err("needs dry ground beside it")
                }
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
                    .any(|t| t.terrain.is_fresh());
                if fresh { Ok(()) } else { Err("needs fresh water nearby") }
            }
            // An empty open tile. A lit flame is a fine target too: that restokes it, unless it
            // still has more than `restoke_below`'s share of a fresh burn left (#359).
            Effect::HeatSource { duration_minutes, restoke_below, .. } => {
                let tile = &self.map[pos];
                let until = self.heat.get(&pos).map(|s| s.until);
                if !matches!(tile.terrain, Terrain::Land | Terrain::Coast) {
                    Err("needs open ground to kindle")
                } else if tile.resource.is_some() || tile.building.is_some() {
                    Err("something is in the way")
                } else if self.too_fresh_to_restoke(
                    until,
                    *duration_minutes,
                    magnitude,
                    *restoke_below,
                ) {
                    Err("flame still burning, restoke below half")
                } else {
                    Ok(())
                }
            }
            // Somewhere to stand and channel: open ground with nothing on it. The aura lands on
            // the caster, so there is nothing else about the tile to check -- except that nobody
            // in the settlement can recast one while every living aura-carrier's is still too
            // fresh (#359): the per-unit half of the same rule is `unit_can_bend`'s own check,
            // since this tile check can't yet know which unit will claim the job.
            Effect::HeatAura { duration_minutes, restoke_below, .. } => {
                let tile = &self.map[pos];
                if !tile.terrain.is_walkable() {
                    Err("needs open ground to stand on")
                } else if tile.resource.is_some() || tile.building.is_some() {
                    Err("something is in the way")
                } else if self.nobody_can_restoke_aura(*duration_minutes, magnitude, *restoke_below)
                {
                    Err("aura still warm, recast below half")
                } else {
                    Ok(())
                }
            }
            // Somewhere to stand and summon the scooter: open ground with nothing on it. The
            // ride lands on the caster, so there is nothing else about the tile to check.
            Effect::Scooter { .. } => {
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
            // A ground stack or a stockpile tile holding the right ore, with enough of it for
            // at least one batch (#398: a stockpile tile is accepted the same as a ground
            // stack, since Earth's ore is usually all stockpiled by the time it knows
            // Metalbend).
            Effect::Smelt { ore, ratio, .. } => {
                let Some(stack) = self.ground.get(&pos).or_else(|| self.stored.get(&pos)) else {
                    return Err("needs ore to bend");
                };
                if stack.item != *ore {
                    Err("that isn't ore")
                } else if stack.count < *ratio {
                    Err("not enough ore for a batch")
                } else {
                    Ok(())
                }
            }
            // An ordinary build site, not the gate, whose work isn't already done (#363).
            Effect::Build { .. } => self.build_target(pos, defs),
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
        ability: AbilityId,
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
            Effect::Reshape { to, revert_minutes, .. } => {
                // A cut into walkable ground (Carve Canal) buries whatever lay hidden under it
                // instead of striking it: the water covers a lost ruin for good.
                if carves(effect, self.map[pos].terrain) {
                    self.buried.remove(&pos);
                } else if self.uncover(pos) {
                    self.wake_idle();
                    return None;
                }
                if let Some(minutes) = revert_minutes {
                    let was = self.map[pos].terrain;
                    let at = self.tick + minutes_to_ticks_min1(minutes * magnitude);
                    let key = (at, pos);
                    let i = self.reverts.partition_point(|r| (r.at, r.pos) <= key);
                    self.reverts.insert(i, Revert { pos, terrain: was, at });
                }
                self.map[pos].terrain = *to;
                self.map_changed();
                self.wake_idle();
                None
            }
            // The ice lands where the bender actually stood to reach the water (`place_ground`,
            // the same spot a dead unit's carried load falls on): the target tile itself is never
            // walkable, so nothing could land there to be hauled away.
            Effect::DrawItem { item, count, .. } => {
                if let Some(stand) = self.unit(caster).map(|u| u.pos) {
                    self.place_ground(stand, Stack { item: *item, count: *count });
                }
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
            // The magnitude scales how long the source burns. Lighting a lit tile restokes it
            // (`validate_effect` already refused one still more than half full, #359). `cooks`
            // (#362) rides along unchanged: a shelter (Earth Tent, Air Shield) stays a shelter
            // when restoked, a flame stays a flame.
            Effect::HeatSource {
                duration_minutes, radius, sleep_bonus, seek_tiles, cooks, ..
            } => {
                self.light_heat_source(HeatSource {
                    pos,
                    radius: *radius,
                    sleep_bonus: *sleep_bonus,
                    seek_tiles: *seek_tiles,
                    until: self.tick + minutes_to_ticks_min1(duration_minutes * magnitude),
                    size: crate::building::ONE_BY_ONE,
                    cooks: *cooks,
                    ability: Some(ability),
                });
                None
            }
            // The aura goes to the caster and follows them. The magnitude scales how long it
            // lasts; casting it again resets the timer (`unit_can_bend` already refused a caster
            // whose own aura is still more than half full, #359).
            Effect::HeatAura { duration_minutes, radius, sleep_bonus, .. } => {
                self.light_aura(Aura {
                    unit: caster,
                    radius: *radius,
                    sleep_bonus: *sleep_bonus,
                    until: self.tick + minutes_to_ticks_min1(duration_minutes * magnitude),
                });
                None
            }
            // The ride goes to the caster. The magnitude scales how long it lasts; casting it
            // again resets the timer.
            Effect::Scooter { duration_minutes, multiplier } => {
                self.mount_scooter(
                    caster,
                    Ride {
                        multiplier: *multiplier,
                        until: self.tick + minutes_to_ticks_min1(duration_minutes * magnitude),
                    },
                );
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
                    dries_at: self.tick + minutes_to_ticks_min1(wet_minutes * magnitude),
                    sprouts: *sprouts,
                    resource: *sprout_resource,
                    biomes: biomes.clone(),
                    fallow_ticks: minutes_to_ticks_min1(*fallow_minutes),
                };
                // Keep the list in drying order; equal times stay in the order they were made.
                let key = (patch.dries_at, patch.centre);
                let at = self.patches.partition_point(|p| (p.dries_at, p.centre) <= key);
                self.patches.insert(at, patch);
                None
            }
            // The stack converts batch by batch, right where it sits; what doesn't divide evenly
            // by `ratio` stays behind as ore, with a fresh haul job if any is left. The target
            // may be a ground stack or a stockpile tile (#398): whichever it was, the leftover
            // ore goes right back where it came from, keyed on the same tile a build or craft
            // job may already be mid-trip to fetch it from -- it must stay ore under that key,
            // never metal, so `take_for_build` (which trusts whatever it finds there) never
            // hands someone a stack of metal labelled as the ore they went to fetch. The metal
            // itself always goes through `place_ground`: for a ground stack that tries the bent
            // tile first and lands there once the ore's gone, exactly as it always has; for a
            // stockpile tile `place_ground` never offers the bent tile at all (it skips every
            // zone tile), so the metal starts its own haul job on nearby open ground instead,
            // never retaking the stockpile entry a pending fetch is aimed at.
            Effect::Smelt { ore, metal, ratio } => {
                let from_ground = self.ground.get(&pos).copied();
                let stack = from_ground.or_else(|| self.stored.get(&pos).copied())?;
                if stack.item != *ore {
                    return None;
                }
                let batches = stack.count / ratio;
                let leftover = stack.count - batches * ratio;
                if from_ground.is_some() {
                    if leftover > 0 {
                        self.ground.insert(pos, Stack { item: *ore, count: leftover });
                        self.ensure_haul_job(pos);
                    } else {
                        self.ground.remove(&pos);
                    }
                } else if leftover > 0 {
                    self.stored.insert(pos, Stack { item: *ore, count: leftover });
                } else {
                    self.stored.remove(&pos);
                }
                self.place_ground(pos, Stack { item: *metal, count: batches });
                None
            }
            // The magnitude scales how much work lands: banked straight onto the site's own
            // progress, capped at the building's total (#363). Work banked before the
            // materials arrive is kept; `run_build`'s own check finishes the building once both
            // are done, so there is nothing more to do here even for an unsupplied site.
            Effect::Build { work_seconds } => {
                self.add_build_work(pos, work_seconds * magnitude, defs);
                None
            }
            #[cfg(test)]
            Effect::Probe => None,
        }
    }
}

/// Riding a scooter (Air Scooter, #143).
impl World {
    /// Gives `unit` the ride, replacing the one it had (a recast resets the timer).
    pub(super) fn mount_scooter(&mut self, unit: UnitId, ride: Ride) {
        self.rides.insert(unit, ride);
    }

    /// `unit`'s speed burst, if it is riding one right now: `multiplier` while `tick < until`,
    /// else 1.0. Every place that moves a unit at its effective `move_speed` (`walk`, `wander`
    /// and `Stride`) multiplies by this on top of everything else.
    pub(super) fn scooter_multiplier(&self, unit: UnitId) -> f32 {
        self.rides.get(&unit).filter(|r| r.until > self.tick).map_or(1.0, |r| r.multiplier)
    }

    /// Drops the rides whose time is up, or whose rider died.
    pub(super) fn tick_scooters(&mut self) {
        if self.rides.is_empty() {
            return;
        }
        let (now, units) = (self.tick, &self.units);
        self.rides
            .retain(|id, r| r.until > now && units.binary_search_by_key(id, |u| u.id).is_ok());
    }
}

/// Timed `Reshape`s reverting (Draw Water, #364).
impl World {
    /// Restores every `Reshape` whose revert is due, oldest-then-position first (`reverts` is
    /// kept sorted that way), marking the regions cache dirty and waking idle units exactly as
    /// the forward change does: a tile's connectivity can change either way, so both directions
    /// get the same treatment. A unit standing on the tile when it reverts is never stranded: the
    /// only effect that reverts today (Draw Water) only ever moves between two walkable terrains
    /// (`Land` and `Ford`), and nothing here assumes otherwise for one that didn't.
    pub(super) fn tick_reverts(&mut self) {
        let due = self.reverts.partition_point(|r| r.at <= self.tick);
        if due == 0 {
            return;
        }
        let ready: Vec<Revert> = self.reverts.drain(..due).collect();
        for r in ready {
            self.map[r.pos].terrain = r.terrain;
        }
        self.map_changed();
        self.wake_idle();
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
        // A resource blocks movement for good, so each sprout is checked against the map as the
        // ones before it left it: a tile that would seal ground off (`would_cut_off`, #490) is
        // skipped (and not left fallow), and the next candidate takes its place.
        let mut grown = 0;
        for p in candidates {
            if grown == patch.sprouts as usize {
                break;
            }
            if self.would_cut_off(p) {
                continue;
            }
            self.map[p].resource = Some(patch.resource);
            // The next candidate's `would_cut_off` must see this sprout.
            self.map_changed();
            self.resting.remove(&p);
            self.fallow.insert(p, self.tick + patch.fallow_ticks);
            grown += 1;
        }
        grown > 0
    }

    /// Can `p` take a sprout of this patch's resource: empty, fertile, not fallow, and not in
    /// use (a stockpile, a stack on the ground, a bend target, or where a unit is or is headed,
    /// since resources block movement). A unit that is inside a building (#226) doesn't hold its
    /// entrance tile this way: it isn't standing there as far as the map is concerned.
    fn can_sprout(&self, p: Pos, patch: &Patch, defs: &Defs) -> bool {
        let Some(tile) = self.map.get(p) else { return false };
        // The terrain must be one the resource grows on in mapgen too (cabbages: land only).
        let terrain_ok = matches!(tile.terrain, Terrain::Land | Terrain::Coast)
            && defs
                .resource(patch.resource)
                .terrains
                .as_ref()
                .is_none_or(|t| t.contains(&tile.terrain));
        // Nothing built or about to be: a sprout on a bed's tile would make it solid (#490, the
        // Water seed 3 settlement of #487), and one on a site's door would block the door.
        terrain_ok
            && tile.resource.is_none()
            && tile.building.is_none()
            && self.site_at(p).is_none()
            && !self.site_door(p, defs)
            && patch.biomes.contains(&tile.biome)
            && !self.fallow.contains_key(&p)
            && !self.zones.contains(&p)
            && !self.ground.contains_key(&p)
            && !self.bend_designations().any(|(_, q)| q == p)
            && self.units_at(p).next().is_none()
            && !self.units.iter().any(|u| u.route.as_ref().is_some_and(|r| r.contains(&p)))
    }
}

#[cfg(test)]
mod air_tests;
#[cfg(test)]
mod build_tests;
#[cfg(test)]
mod canal_tests;
#[cfg(test)]
mod cut_ice_tests;
#[cfg(test)]
mod draw_water_tests;
#[cfg(test)]
mod earth_tests;
#[cfg(test)]
mod fire_tests;
#[cfg(test)]
mod glide_tests;
#[cfg(test)]
mod metalbend_tests;
#[cfg(test)]
mod rank_tests;
#[cfg(test)]
mod scooter_tests;
#[cfg(test)]
mod seismic_tests;
#[cfg(test)]
mod shelter_tests;
#[cfg(test)]
mod strand_tests;
#[cfg(test)]
mod tests;
#[cfg(test)]
mod tunnel_tests;
#[cfg(test)]
mod water_tests;
