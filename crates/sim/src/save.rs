//! Saving and loading a game (#7).
//!
//! # Snapshot, not replay
//!
//! A save is a snapshot of the whole `World`, not its seed plus command log. The world already
//! keeps its command log (`World::command_log`) and `World::replay` can rebuild a session from
//! it, but replay costs a full re-simulation of every tick played (a long session is hundreds of
//! thousands of ticks, each with pathfinding), and it only works while the sim code and the
//! balance data stay exactly as they were when the log was recorded: any rule or number change
//! silently turns an old save into a different game. A snapshot loads in time proportional to its
//! size and survives balance changes. The command log is saved too (it is part of the world), so
//! a loaded game can still be replayed or inspected.
//!
//! # Format
//!
//! RON text, human-readable and diffable (map keys that aren't strings, which JSON can't do, are
//! fine). The first line is a `SaveInfo` header and the rest is the `World`:
//!
//! ```text
//! (magic:"four-nations-save",version:1,data_hash:..,seed:..,nation:Fire,tick:..)
//! (seed:..,tick:..,map:(..),..)
//! ```
//!
//! The header is its own line so that `inspect` (and the version check) never has to parse the
//! multi-megabyte world.
//!
//! * `version` is `SAVE_VERSION`. Bump it whenever the serialized shape of `World` changes in a
//!   way old files can't be read as; `load` reads the header first and refuses a version it
//!   doesn't know with a clear error, before trying to make sense of the body. When an old
//!   version is worth keeping, add a branch there that upgrades it.
//! * `data_hash` is `Defs::id_hash`: ids such as `TechId` are indices into the data tables, so a
//!   save made against different tables would mean something else. Retuned numbers don't matter.
//! * The header's `seed`, `nation` and `tick` let a load menu describe a save (`inspect`) without
//!   parsing the world.
//!
//! `World` derives `Serialize`/`Deserialize` over all of its state (only the connectivity cache is
//! skipped; it is rebuilt on demand). A new field on `World` therefore either serializes or fails
//! to compile, and the round-trip tests compare complete serializations, so state can't be
//! silently left out of a save.
//!
//! This module only turns a world into text and back; it does no I/O. The client chooses where
//! the text goes.

use std::collections::BTreeSet;
use std::sync::Mutex;

use serde::{Deserialize, Serialize};

use crate::defs::Defs;
use crate::nation::Nation;
use crate::world::{BendOutcome, World};

/// The save format version this build writes and reads.
pub const SAVE_VERSION: u32 = 1;

const MAGIC: &str = "four-nations-save";

/// What describes a save without its world: enough for a load menu.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SaveInfo {
    pub magic: String,
    pub version: u32,
    /// `Defs::id_hash` of the data the world was saved against.
    pub data_hash: u64,
    pub seed: u64,
    pub nation: Nation,
    /// The world's tick when saved.
    pub tick: u64,
}

/// Why a save couldn't be written or read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SaveError {
    /// The text isn't a Four Nations save at all (or is too damaged to tell).
    NotASave(String),
    /// A save from a format this build doesn't know.
    Version { found: u32, supported: u32 },
    /// Made against different game data (ids would point at the wrong things).
    DataMismatch,
    /// Looked like a save but couldn't be read, or described an impossible world.
    Corrupt(String),
    /// Couldn't be serialized (not expected to happen).
    Write(String),
}

impl std::fmt::Display for SaveError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotASave(e) => write!(f, "not a Four Nations save ({e})"),
            Self::Version { found, supported } => write!(
                f,
                "save format version {found} is not supported by this build (it reads version \
                 {supported})"
            ),
            Self::DataMismatch => f.write_str(
                "this save was made with different game data (biomes, techs, items, buildings or \
                 abilities were added, removed or reordered)",
            ),
            Self::Corrupt(e) => write!(f, "damaged save ({e})"),
            Self::Write(e) => write!(f, "could not write the save ({e})"),
        }
    }
}

impl std::error::Error for SaveError {}

/// Splits save text into its header line and the world text after it.
fn split(text: &str) -> (&str, &str) {
    text.split_once('\n').unwrap_or((text, ""))
}

/// The two header fields every format version is expected to keep, so a file from a newer format
/// (whose header may have grown) can still be told apart from a file that isn't a save.
#[derive(Deserialize)]
struct Probe {
    magic: String,
    version: u32,
}

fn probe(text: &str) -> Result<Probe, SaveError> {
    let probe: Probe =
        ron::from_str(split(text).0).map_err(|e| SaveError::NotASave(e.to_string()))?;
    if probe.magic != MAGIC {
        return Err(SaveError::NotASave("wrong magic".into()));
    }
    Ok(probe)
}

/// Reads just the header of a save, without parsing or checking the world: its version, nation,
/// seed and tick. Fails if the text isn't a save, or is one from a format this build can't read.
pub fn inspect(text: &str) -> Result<SaveInfo, SaveError> {
    let found = probe(text)?.version;
    if found != SAVE_VERSION {
        return Err(SaveError::Version { found, supported: SAVE_VERSION });
    }
    ron::from_str(split(text).0).map_err(|e| SaveError::NotASave(e.to_string()))
}

impl World {
    /// Serializes the whole world to save-file text. Deterministic: the same world always gives
    /// the same text.
    pub fn save(&self, defs: &Defs) -> Result<String, SaveError> {
        let info = SaveInfo {
            magic: MAGIC.into(),
            version: SAVE_VERSION,
            data_hash: defs.id_hash(),
            seed: self.seed,
            nation: self.nation,
            tick: self.tick,
        };
        let write = |e: ron::Error| SaveError::Write(e.to_string());
        let mut text = ron::ser::to_string(&info).map_err(write)?;
        text.push('\n');
        text.push_str(&ron::ser::to_string(self).map_err(write)?);
        Ok(text)
    }

    /// Rebuilds a world from `save` text, checked against `defs`. The loaded world continues
    /// exactly as the saved one would have.
    pub fn load(text: &str, defs: &Defs) -> Result<World, SaveError> {
        let info = inspect(text)?;
        if info.data_hash != defs.id_hash() {
            return Err(SaveError::DataMismatch);
        }
        let world: World =
            ron::from_str(split(text).1).map_err(|e| SaveError::Corrupt(e.to_string()))?;
        world.validate(defs).map_err(SaveError::Corrupt)?;
        Ok(world)
    }
}

/// How `BendOutcome` is written: the same shape, but owning its reason, because a string read
/// from a save can't borrow from the file.
#[derive(Serialize, Deserialize)]
enum OutcomeRepr {
    Done,
    Failed(String),
}

impl From<BendOutcome> for OutcomeRepr {
    fn from(o: BendOutcome) -> Self {
        match o {
            BendOutcome::Done => Self::Done,
            BendOutcome::Failed(why) => Self::Failed(why.to_owned()),
        }
    }
}

// Written by hand: the derive would demand `'de: 'static` for the `&'static str`, which no real
// deserializer can meet.
impl Serialize for BendOutcome {
    fn serialize<S: serde::Serializer>(&self, ser: S) -> Result<S::Ok, S::Error> {
        OutcomeRepr::from(*self).serialize(ser)
    }
}

impl<'de> Deserialize<'de> for BendOutcome {
    fn deserialize<D: serde::Deserializer<'de>>(de: D) -> Result<Self, D::Error> {
        OutcomeRepr::deserialize(de).map(Self::from)
    }
}

impl From<OutcomeRepr> for BendOutcome {
    fn from(o: OutcomeRepr) -> Self {
        match o {
            OutcomeRepr::Done => Self::Done,
            OutcomeRepr::Failed(why) => Self::Failed(intern(why)),
        }
    }
}

/// A `'static` copy of `s`. The sim keeps its failure reasons as `&'static str`; each distinct
/// one read from a save is leaked once and shared afterwards. They are a handful of short
/// sentences, so the cost is negligible.
fn intern(s: String) -> &'static str {
    static INTERNED: Mutex<BTreeSet<&'static str>> = Mutex::new(BTreeSet::new());
    let mut interned = INTERNED.lock().unwrap_or_else(|e| e.into_inner());
    if let Some(&known) = interned.get(s.as_str()) {
        return known;
    }
    let leaked: &'static str = Box::leak(s.into_boxed_str());
    interned.insert(leaked);
    leaked
}
