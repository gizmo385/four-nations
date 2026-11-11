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
//! fine). Versions 2 to 4 are three lines: a `SaveInfo` header, the save's content-id tables
//! ([`SaveIds`]), and the `World`:
//!
//! ```text
//! (magic:"four-nations-save",version:4,data_hash:..,seed:..,nation:fire,tick:..)
//! (biomes:["tundra",..],resources:[..],nations:[..],techs:[..],items:[..],buildings:[..],abilities:[..])
//! (seed:..,tick:..,tiles:(..),buildings:(next:..,records:{..}),..)
//! ```
//!
//! The header is its own line so that `inspect` (and the version check) never has to parse the
//! multi-megabyte world.
//!
//! * `version` is `SAVE_VERSION`. Bump it whenever the serialized shape of `World` changes in a
//!   way old files can't be read as; `load` reads the header first and refuses a version it
//!   doesn't know with a clear error, before trying to make sense of the body. When an old
//!   version is worth keeping, add a branch there that upgrades it.
//! * `data_hash` is `Defs::id_hash`, for information only: it no longer gates loading.
//! * The header's `seed`, `nation` and `tick` let a load menu describe a save (`inspect`) without
//!   parsing the world.
//!
//! # Content changes (#602, docs/design/save-compat.md)
//!
//! Content ids such as `TechId` are indices into the `data/*.toml` tables, in memory and in the
//! body. The ids line records which id string each index meant, and `load` maps every saved index
//! to today's index by name while it parses the body (`ids::Remap`), so content may be appended,
//! inserted or reordered under an old save. A save that uses content which no longer exists fails
//! with `SaveError::MissingContent`, naming it. The one piece of state indexed by position rather
//! than by id, `Research::known`, is remapped after the parse.
//!
//! Version 1 saves have no ids line. Their tables come from a snapshot (`save/v1_ids.ron`) keyed
//! by their `data_hash`; a v1 save whose hash isn't there is `DataMismatch`.
//!
//! # Buildings as records (#604, docs/design/buildings-as-records.md)
//!
//! Version 3 keeps one record per finished building (`World::buildings`), and its tiles point at
//! them; the body's map is written as `tiles`. Versions 1 and 2 copied the building onto every
//! tile and kept its state in side tables. They are read through deserialize-only fields and
//! upgraded after the parse (`save/v2.rs`), so a v1 save loads through both steps: its id
//! tables, then the records.
//!
//! # One event log (#605)
//!
//! Version 4 keeps everything the world logged (bends, techs, deaths, endeavor events,
//! arrivals, seasons, strain, harm, charts) in one list, written as `events`. Versions 1 to 3
//! kept nine lists. They are read through deserialize-only fields and merged by tick on load
//! (`world/events.rs`, `World::upgrade_from_v3`), after the v2 step, so a v1 or v2 save loads
//! through every step.
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

mod ids;
pub(crate) mod v2;

pub use ids::SaveIds;
use ids::{Remap, Table, Unmapped};

/// The save format version this build writes. It also reads every version from 1 up.
pub const SAVE_VERSION: u32 = 4;

const MAGIC: &str = "four-nations-save";

/// What describes a save without its world: enough for a load menu.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SaveInfo {
    pub magic: String,
    pub version: u32,
    /// `Defs::id_hash` of the data the world was saved against. Information only, except in a
    /// version 1 save, where it picks the id tables the save was made against.
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
    /// A version 1 save made against game data whose id tables this build doesn't have.
    DataMismatch,
    /// The save uses content (a building, tech, item, ...) that this build's data doesn't have:
    /// removed or renamed since it was made. `kind` is "building", "tech" and so on.
    MissingContent { kind: &'static str, id: String },
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
                "this save is from an old version of the game whose content this build doesn't \
                 know (it reads old saves only from the last data before the format changed)",
            ),
            Self::MissingContent { kind, id } => write!(
                f,
                "this save uses the {kind} {id:?}, which this version of the game doesn't have \
                 (removed or renamed)"
            ),
            Self::Corrupt(e) => write!(f, "damaged save ({e})"),
            Self::Write(e) => write!(f, "could not write the save ({e})"),
        }
    }
}

impl std::error::Error for SaveError {}

/// Splits save text into its first line and the text after it.
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
/// seed and tick. Fails if the text isn't a save, or is one this build can't read on its header
/// alone: a format version it doesn't know, or a version 1 save made against data it has no id
/// tables for (`DataMismatch`).
pub fn inspect(text: &str) -> Result<SaveInfo, SaveError> {
    let found = probe(text)?.version;
    if !(1..=SAVE_VERSION).contains(&found) {
        return Err(SaveError::Version { found, supported: SAVE_VERSION });
    }
    let info: SaveInfo =
        ron::from_str(split(text).0).map_err(|e| SaveError::NotASave(e.to_string()))?;
    if info.version == 1 && ids::v1_ids(info.data_hash).is_none() {
        return Err(SaveError::DataMismatch);
    }
    Ok(info)
}

/// The id tables a save was made against, and its body.
fn ids_and_body<'t>(info: &SaveInfo, text: &'t str) -> Result<(SaveIds, &'t str), SaveError> {
    let after_header = split(text).1;
    match info.version {
        1 => Ok((ids::v1_ids(info.data_hash).ok_or(SaveError::DataMismatch)?, after_header)),
        _ => {
            let (ids, body) = split(after_header);
            let ids = ron::from_str(ids).map_err(|e| SaveError::Corrupt(format!("ids: {e}")))?;
            Ok((ids, body))
        }
    }
}

impl From<Unmapped> for SaveError {
    fn from(e: Unmapped) -> Self {
        match e {
            Unmapped::Missing { table, id } => Self::MissingContent { kind: table.kind(), id },
            Unmapped::OutOfRange { table, index } => {
                Self::Corrupt(format!("{} index {index} is past the save's table", table.kind()))
            }
        }
    }
}

impl World {
    /// Serializes the whole world to save-file text. Deterministic: the same world always gives
    /// the same text.
    pub fn save(&self, defs: &Defs) -> Result<String, SaveError> {
        let ids = SaveIds::of(defs);
        let info = SaveInfo {
            magic: MAGIC.into(),
            version: SAVE_VERSION,
            data_hash: ids.hash(),
            seed: self.seed,
            nation: self.nation,
            tick: self.tick,
        };
        let write = |e: ron::Error| SaveError::Write(e.to_string());
        let mut text = ron::ser::to_string(&info).map_err(write)?;
        text.push('\n');
        text.push_str(&ron::ser::to_string(&ids).map_err(write)?);
        text.push('\n');
        text.push_str(&ron::ser::to_string(self).map_err(write)?);
        Ok(text)
    }

    /// Rebuilds a world from `save` text, checked against `defs`. The loaded world continues
    /// exactly as the saved one would have. Content ids are mapped to `defs` by name, so the save
    /// may come from data with content added or reordered since (#602).
    pub fn load(text: &str, defs: &Defs) -> Result<World, SaveError> {
        let info = inspect(text)?;
        let (ids, body) = ids_and_body(&info, text)?;
        let guard = Remap::new(&ids, defs).install();
        let parsed: Result<World, _> = ron::from_str(body);
        let remap = guard.finish();
        // A missing id is the real reason, whatever ron made of the error it caused.
        if let Some(failed) = remap.failure() {
            return Err(failed.clone().into());
        }
        let mut world = parsed.map_err(|e| SaveError::Corrupt(e.to_string()))?;
        world.research_mut().remap(remap.table(Table::Tech), defs).map_err(|e| match e {
            Some(saved) => SaveError::MissingContent {
                kind: Table::Tech.kind(),
                id: remap.saved_name(Table::Tech, saved).to_owned(),
            },
            None => SaveError::Corrupt("the research state doesn't match the save's techs".into()),
        })?;
        if info.version < 3 {
            // Buildings became records (#604): the body has the v2 shape (`save/v2.rs`).
            world.upgrade_from_v2().map_err(SaveError::Corrupt)?;
        }
        if info.version < 4 {
            // One event log (#605): the body has nine event lists (`world/events.rs`).
            world.upgrade_from_v3();
        }
        world.validate(defs).map_err(SaveError::Corrupt)?;
        // The building indexes aren't saved; rebuild them from the records, from today's defs.
        world.reindex_buildings(defs);
        // next_unit (#232) defaults to 0 for a save from before it existed; restore its
        // invariant (past every unit id ever handed out) from the units and deaths on file.
        world.fixup_next_unit();
        // Seats a known tech adds (`writing`'s, docs/design/horizons-phase-1.md) that a hall
        // saved before the upgrade existed doesn't have yet.
        world.sync_seats(defs);
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
