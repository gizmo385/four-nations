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

use super::*;

/// Something the player asks the settlement to do.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Command {
    /// Mark the resource on a tile for harvesting (chop, quarry, gather, mine).
    DesignateHarvest { pos: Pos },
    /// Withdraw a harvest designation. Work already done on it is lost.
    CancelHarvest { pos: Pos },
    /// Mark a tile as stockpile.
    DesignateStockpile { pos: Pos },
    /// Set the research target (see `Research::set_target`).
    SetResearch { tech: TechId },
}

/// Why a command was refused. The world is unchanged when `apply` returns one of these.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommandError {
    Designate(DesignateError),
    Research(ResearchError),
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

impl std::fmt::Display for CommandError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Designate(DesignateError::OutOfBounds) => f.write_str("off the map"),
            Self::Designate(DesignateError::NothingToHarvest) => f.write_str("nothing to harvest"),
            Self::Designate(DesignateError::NotResearched) => f.write_str("not researched yet"),
            Self::Designate(DesignateError::AlreadyDesignated) => f.write_str("already designated"),
            Self::Designate(DesignateError::NotDesignated) => f.write_str("not designated"),
            Self::Designate(DesignateError::CannotStockpile) => f.write_str("can't stockpile here"),
            Self::Research(e) => e.fmt(f),
        }
    }
}

impl std::error::Error for CommandError {}

/// A command that was applied, and when.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Recorded {
    /// `World::tick` at the moment it was applied, i.e. before the next tick ran.
    pub tick: u64,
    pub command: Command,
}

impl World {
    /// Applies a player command. On success it is added to the command log; on failure the world
    /// is untouched.
    pub fn apply(&mut self, command: Command, defs: &Defs) -> Result<(), CommandError> {
        match command {
            Command::DesignateHarvest { pos } => self.designate_harvest(pos, defs).map(drop)?,
            Command::CancelHarvest { pos } => self.cancel_harvest(pos)?,
            Command::DesignateStockpile { pos } => self.designate_stockpile(pos)?,
            Command::SetResearch { tech } => self.set_research(tech, defs)?,
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
