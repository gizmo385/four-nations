//! Player commands: the only way a client changes the world.
//!
//! A client never reaches into `World`; it builds a `Command` and hands it to `World::apply`.
//! Every accepted command is stamped with the tick it was applied on and kept in the world's
//! command log, so a session is its seed, nation, map config and that log: `World::replay` feeds
//! the log to a fresh world and reproduces the session exactly (the sim is deterministic).
//!
//! Commands are small and per tile on purpose: a drag over fifty tiles is fifty commands, which
//! keeps the enum (and any save format built on it) trivial. Commands that fail validation change
//! nothing and aren't logged, so replaying a log never depends on what was rejected.

use crate::building::BuildingId;

use super::*;

/// Something the player asks the settlement to do.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum Command {
    /// Mark the resource on a tile for harvesting (chop, quarry, gather, mine).
    DesignateHarvest { pos: Pos },
    /// Mark the resource on a tile for harvesting, and keep re-queuing the job every time it
    /// ripens again, so a renewable resource (#56) doesn't need re-designating by hand (#81).
    DesignateRepeatingHarvest { pos: Pos },
    /// Withdraw a harvest designation and any standing order on it. Work already done is lost.
    CancelHarvest { pos: Pos },
    /// Mark a tile as stockpile.
    DesignateStockpile { pos: Pos },
    /// Withdraw a stockpile designation. Items stored on the tile are dropped on the ground
    /// there (not lost), to be hauled elsewhere.
    CancelStockpile { pos: Pos },
    /// Set the research target (see `Research::set_target`).
    SetResearch { tech: TechId },
    /// Designate a bend: a unit with enough chi walks to the tile, channels the ability and
    /// spends the chi when it completes.
    Bend { ability: AbilityId, pos: Pos },
    /// Withdraw a bend designation. Channeling already done is lost.
    CancelBend { pos: Pos },
    /// Designate a building: a unit hauls its cost to the tile and constructs it
    /// (`data/buildings.toml`).
    DesignateBuild { pos: Pos, building: BuildingId },
    /// Withdraw a build designation. Materials delivered drop on the ground; work done is lost.
    CancelBuild { pos: Pos },
    /// Place the endeavor's gate with its anchor (top-left tile) on `anchor`, once the endeavor
    /// is Ready, and start building it (`world/gate.rs`, #236).
    BeginEndeavor { anchor: Pos },
    /// Withdraw the gate's site before work on it starts. Materials delivered drop on the ground
    /// and the endeavor is Ready again.
    CancelEndeavor,
    /// Plant the empty farm plot on a tile (`data/buildings.toml`, `[building.farm]`, #198).
    Sow { pos: Pos },
    /// Withdraw a Sow designation. Work already done on it is lost.
    CancelSow { pos: Pos },
    /// Mark the resource on a tile to be removed for good (`docs/design/clearing.md`, issue
    /// #411/#424): a unit uproots it (`[harvest.clear]`) or works its ordinary harvest, which
    /// already clears it. Withdraws any harvest designation or standing order on the tile; a
    /// unit mid-harvest drops it, as on `CancelHarvest`.
    DesignateClear { pos: Pos },
    /// Withdraw a Clear designation. Work already done on it is lost.
    CancelClear { pos: Pos },
    /// Mark the finished building covering this tile (any tile of its footprint) to be taken
    /// down (`world/deconstruct.rs`, issue #342): a unit works it, the whole footprint clears at
    /// once, and part of its cost drops by the anchor.
    Deconstruct { pos: Pos },
    /// Take back the mark on the building covering this tile. Work done on it is lost; the
    /// building stands untouched.
    CancelDeconstruct { pos: Pos },
}

/// Why a command was refused. The world is unchanged when `apply` returns one of these.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommandError {
    Designate(DesignateError),
    Research(ResearchError),
    Bend(BendError),
    Build(BuildError),
    Endeavor(EndeavorError),
}

impl CommandError {
    /// Whether a stateless player (`autoplay`) gets this refusal as a matter of course: it asked
    /// for what is already so (`AlreadyDesignated`, `AlreadyKnown`, `PlotNotEmpty`), for something
    /// not unlocked yet, or tried a bend on a tile to let validation say whether it fits. Such a
    /// player re-issues its whole plan every step, so these number in the thousands and say
    /// nothing; any other refusal means a command that was wrong (`World::refusals`).
    pub fn is_routine(&self) -> bool {
        match self {
            Self::Designate(e) => matches!(
                e,
                DesignateError::AlreadyDesignated
                    | DesignateError::NotDesignated
                    | DesignateError::NotResearched(_)
                    | DesignateError::PlotNotEmpty
            ),
            Self::Research(e) => matches!(e, ResearchError::AlreadyKnown),
            Self::Bend(e) => matches!(
                e,
                BendError::AlreadyDesignated
                    | BendError::NotDesignated
                    | BendError::Locked { .. }
                    | BendError::InvalidTarget(_)
            ),
            Self::Build(e) => matches!(e, BuildError::NotResearched(_) | BuildError::NotDesignated),
            Self::Endeavor(e) => matches!(e, EndeavorError::NotReady),
        }
    }
}

impl From<DesignateError> for CommandError {
    fn from(e: DesignateError) -> Self {
        Self::Designate(e)
    }
}

impl From<ResearchError> for CommandError {
    fn from(e: ResearchError) -> Self {
        Self::Research(e)
    }
}

impl From<BendError> for CommandError {
    fn from(e: BendError) -> Self {
        Self::Bend(e)
    }
}

impl From<BuildError> for CommandError {
    fn from(e: BuildError) -> Self {
        Self::Build(e)
    }
}

impl From<EndeavorError> for CommandError {
    fn from(e: EndeavorError) -> Self {
        Self::Endeavor(e)
    }
}

impl std::fmt::Display for CommandError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Designate(DesignateError::OutOfBounds) => f.write_str("off the map"),
            Self::Designate(DesignateError::NothingToHarvest) => f.write_str("nothing to harvest"),
            Self::Designate(DesignateError::NotResearched(_)) => f.write_str("not researched yet"),
            Self::Designate(DesignateError::AlreadyDesignated) => f.write_str("already designated"),
            Self::Designate(DesignateError::NotDesignated) => f.write_str("not designated"),
            Self::Designate(DesignateError::CannotStockpile) => f.write_str("can't stockpile here"),
            Self::Designate(DesignateError::NoFarmPlot) => f.write_str("not a farm plot"),
            Self::Designate(DesignateError::PlotNotEmpty) => f.write_str("already sown"),
            Self::Designate(DesignateError::CannotClear) => f.write_str("can't be cleared"),
            Self::Designate(DesignateError::NoBuilding) => f.write_str("no building here"),
            Self::Designate(DesignateError::CannotDeconstruct) => {
                f.write_str("can't be taken down")
            }
            Self::Research(e) => e.fmt(f),
            Self::Bend(e) => e.fmt(f),
            Self::Build(e) => e.fmt(f),
            Self::Endeavor(e) => e.fmt(f),
        }
    }
}

impl std::error::Error for CommandError {}

/// A command that was applied, and when.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Recorded {
    /// `World::tick` at the moment it was applied, i.e. before the next tick ran.
    pub tick: u64,
    pub command: Command,
}

impl World {
    /// Applies a player command. On success it is added to the command log; on failure the world
    /// is untouched.
    pub fn apply(&mut self, command: Command, defs: &Defs) -> Result<(), CommandError> {
        let result = self.apply_command(command, defs);
        self.refusals.record(result.as_ref().err());
        result
    }

    fn apply_command(&mut self, command: Command, defs: &Defs) -> Result<(), CommandError> {
        match command {
            Command::DesignateHarvest { pos } => self.designate_harvest(pos, defs).map(drop)?,
            Command::DesignateRepeatingHarvest { pos } => {
                self.designate_repeating_harvest(pos, defs)?
            }
            Command::CancelHarvest { pos } => self.cancel_harvest(pos)?,
            Command::DesignateStockpile { pos } => self.designate_stockpile(pos)?,
            Command::CancelStockpile { pos } => self.cancel_stockpile(pos)?,
            Command::SetResearch { tech } => self.set_research(tech, defs)?,
            Command::Bend { ability, pos } => self.designate_bend(ability, pos, defs).map(drop)?,
            Command::CancelBend { pos } => self.cancel_bend(pos, defs)?,
            Command::DesignateBuild { pos, building } => {
                self.designate_build(pos, building, defs).map(drop)?
            }
            Command::CancelBuild { pos } => self.cancel_build(pos, defs)?,
            Command::BeginEndeavor { anchor } => self.begin_endeavor(anchor, defs)?,
            Command::CancelEndeavor => self.cancel_endeavor(defs)?,
            Command::Sow { pos } => self.designate_sow(pos, defs).map(drop)?,
            Command::CancelSow { pos } => self.cancel_sow(pos)?,
            Command::DesignateClear { pos } => self.designate_clear(pos, defs).map(drop)?,
            Command::CancelClear { pos } => self.cancel_clear(pos)?,
            Command::Deconstruct { pos } => self.designate_deconstruct(pos, defs).map(drop)?,
            Command::CancelDeconstruct { pos } => self.cancel_deconstruct(pos)?,
        }
        self.command_log.push(Recorded { tick: self.tick, command });
        Ok(())
    }

    /// Every command applied so far, in order.
    pub fn command_log(&self) -> &[Recorded] {
        &self.command_log
    }

    /// Plays a command log forward on this world until `self.tick` reaches `until`: each command
    /// is applied when the world's tick equals its recorded tick, in log order. Run it on a world
    /// built from the same seed, nation and map config as the one that made the log (and still at
    /// tick 0) to reproduce that session. Commands recorded after `until` are left unapplied.
    ///
    /// Rebuild that initial world from (seed, size, nation, `start`, `difficulty`) first:
    /// `World::new(cfg, nation, units, defs)` when `start` is `None` (a save or log from before
    /// #289, or `new`'s own path), or otherwise `World::found(Survey::new(cfg, defs), nation,
    /// start, units, difficulty, defs).expect(..)`, with `cfg` built from the saved map's size.
    /// The founded site was valid when it was recorded, so `found` refusing it again would mean
    /// the data or the map changed underneath the log.
    pub fn replay(&mut self, log: &[Recorded], until: u64, defs: &Defs) {
        let mut next = 0;
        loop {
            while next < log.len() && log[next].tick <= self.tick {
                // A logged command was valid when it was recorded, so it is again.
                let applied = self.apply(log[next].command, defs);
                debug_assert!(applied.is_ok(), "replayed {:?} failed: {applied:?}", log[next]);
                next += 1;
            }
            if self.tick >= until {
                return;
            }
            self.tick(defs);
        }
    }
}
