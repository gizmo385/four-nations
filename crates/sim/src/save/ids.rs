//! A save's content-id tables, and the remap that reads its ids against today's data (#602,
//! docs/design/save-compat.md).
//!
//! In memory and in a save's body, content ids (`BiomeId`, `ResourceId`, `TechId`, `ItemId`,
//! `BuildingId`, `AbilityId`) are small table indices. A save also writes, on its own line, the
//! id string each index meant: a [`SaveIds`]. While `World::load` parses the body it installs a
//! [`Remap`] built from those tables and today's `Defs`, and each id type's `Deserialize` (from
//! [`remapped_id!`] below) maps the saved index to today's index by name. Outside `load` no remap
//! is installed and the ids deserialize as the bare indices they are, exactly as the derive did.
//!
//! The remap is a thread-local because serde's derive gives a `Deserialize` impl no other way to
//! reach it. That is sound only because `ron::from_str` runs synchronously on the calling thread,
//! and because [`Remap::install`] returns a guard that clears it on every exit, a panic included.

use std::cell::RefCell;

use serde::{Deserialize, Serialize};

use crate::beast::BeastId;
use crate::bending::AbilityId;
use crate::building::BuildingId;
use crate::defs::{BiomeId, Defs, ResourceId};
use crate::jobs::ItemId;
use crate::research::TechId;

/// The id strings behind every content-id index a save uses, one table per id type, each in index
/// order. `nations` takes part only in [`SaveIds::hash`]: `Nation` is an enum written by name and
/// needs no remap.
///
/// A new content-id newtype gets a table here (with `#[serde(default)]`, so older saves read it as
/// empty) and an entry in [`Table`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SaveIds {
    pub biomes: Vec<String>,
    pub resources: Vec<String>,
    pub nations: Vec<String>,
    pub techs: Vec<String>,
    pub items: Vec<String>,
    pub buildings: Vec<String>,
    pub abilities: Vec<String>,
    /// Absent from a save written before beasts (#693), which reads as empty.
    #[serde(default)]
    pub beasts: Vec<String>,
}

impl SaveIds {
    /// The id tables of `defs`, in index order.
    pub fn of(defs: &Defs) -> Self {
        fn ids<T>(table: &[T], id: impl Fn(&T) -> &str) -> Vec<String> {
            table.iter().map(|d| id(d).to_owned()).collect()
        }
        Self {
            biomes: ids(&defs.biomes, |d| &d.id),
            resources: ids(&defs.resources, |d| &d.id),
            nations: ids(&defs.nations, |d| &d.name),
            techs: ids(&defs.techs, |d| &d.id),
            items: ids(&defs.items, |d| &d.id),
            buildings: ids(&defs.buildings, |d| &d.id),
            abilities: ids(&defs.abilities, |d| &d.id),
            beasts: ids(&defs.beasts, |d| &d.id),
        }
    }

    /// A hash of every table, in order: `Defs::id_hash`. FNV-1a, as in `rng::stream`; stable
    /// across platforms and Rust versions.
    pub fn hash(&self) -> u64 {
        let tables = [
            &self.biomes,
            &self.resources,
            &self.nations,
            &self.techs,
            &self.items,
            &self.buildings,
            &self.abilities,
        ];
        let mut h: u64 = 0xcbf2_9ce4_8422_2325;
        let mut feed = |b: u8| {
            h ^= u64::from(b);
            h = h.wrapping_mul(0x0100_0000_01b3);
        };
        for table in tables {
            for id in table {
                id.bytes().for_each(&mut feed);
                feed(0);
            }
            feed(1);
        }
        // Beasts (#693) came after the hash was first written down (a v1 save's `data_hash`,
        // `V1_IDS`), so their table is fed only when it has entries.
        if !self.beasts.is_empty() {
            for id in &self.beasts {
                id.bytes().for_each(&mut feed);
                feed(0);
            }
            feed(1);
        }
        h
    }
}

/// The id tables of v1 saves, which wrote none of their own: a snapshot of the data as it was
/// when v2 replaced v1 (#602). A v1 save loads only if its `data_hash` is one of these. Never
/// edit an entry; data whose tables an old v1 save was made against can be added as a new one.
const V1_IDS: &str = include_str!("v1_ids.ron");

/// The id tables a v1 save with header `data_hash` was made against, if this build knows them.
pub(crate) fn v1_ids(data_hash: u64) -> Option<SaveIds> {
    let known: Vec<SaveIds> = ron::from_str(V1_IDS).expect("save/v1_ids.ron must parse");
    known.into_iter().find(|ids| ids.hash() == data_hash)
}

/// One content-id table.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Table {
    Biome,
    Resource,
    Tech,
    Item,
    Building,
    Ability,
    Beast,
}

impl Table {
    const ALL: [Table; 7] = [
        Table::Biome,
        Table::Resource,
        Table::Tech,
        Table::Item,
        Table::Building,
        Table::Ability,
        Table::Beast,
    ];

    /// What a player would call an entry of this table, for `SaveError::MissingContent`.
    pub(crate) fn kind(self) -> &'static str {
        match self {
            Table::Biome => "biome",
            Table::Resource => "resource",
            Table::Tech => "tech",
            Table::Item => "item",
            Table::Building => "building",
            Table::Ability => "ability",
            Table::Beast => "beast",
        }
    }

    fn saved(self, ids: &SaveIds) -> &[String] {
        match self {
            Table::Biome => &ids.biomes,
            Table::Resource => &ids.resources,
            Table::Tech => &ids.techs,
            Table::Item => &ids.items,
            Table::Building => &ids.buildings,
            Table::Ability => &ids.abilities,
            Table::Beast => &ids.beasts,
        }
    }

    fn today(self, defs: &Defs, id: &str) -> Option<usize> {
        match self {
            Table::Biome => defs.biome_id(id).map(|i| usize::from(i.0)),
            Table::Resource => defs.resource_id(id).map(|i| usize::from(i.0)),
            Table::Tech => defs.tech_id(id).map(|i| usize::from(i.0)),
            Table::Item => defs.item_id(id).map(|i| usize::from(i.0)),
            Table::Building => defs.building_id(id).map(|i| usize::from(i.0)),
            Table::Ability => defs.ability_id(id).map(|i| usize::from(i.0)),
            Table::Beast => defs.beast_id(id).map(|i| usize::from(i.0)),
        }
    }
}

/// A saved index that couldn't be mapped to today's data.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Unmapped {
    /// The saved table names `id`, which today's data doesn't have.
    Missing { table: Table, id: String },
    /// The index is past the end of the save's own table: the save is damaged.
    OutOfRange { table: Table, index: usize },
}

/// For each table, today's index of every saved index (`None` where the saved id is gone), and
/// the saved names for the error.
pub(crate) struct Remap {
    maps: Vec<Vec<Option<usize>>>,
    names: Vec<Vec<String>>,
    /// The first saved index that failed to map during the parse, if any.
    failed: Option<Unmapped>,
}

thread_local! {
    static REMAP: RefCell<Option<Remap>> = const { RefCell::new(None) };
}

/// Clears the installed remap when dropped.
pub(crate) struct RemapGuard(());

impl RemapGuard {
    /// Uninstalls the remap and hands it back, with the first failure recorded while it was
    /// installed.
    pub(crate) fn finish(self) -> Remap {
        REMAP.with(|r| r.borrow_mut().take()).expect("installed until the guard drops")
    }
}

impl Drop for RemapGuard {
    fn drop(&mut self) {
        REMAP.with(|r| *r.borrow_mut() = None);
    }
}

impl Remap {
    /// Maps the save's tables `saved` onto `defs` by name.
    pub(crate) fn new(saved: &SaveIds, defs: &Defs) -> Self {
        let maps = Table::ALL
            .iter()
            .map(|&t| t.saved(saved).iter().map(|id| t.today(defs, id)).collect())
            .collect();
        let names = Table::ALL.iter().map(|&t| t.saved(saved).to_vec()).collect();
        Self { maps, names, failed: None }
    }

    /// Today's index of every saved index of `table`, in saved order.
    pub(crate) fn table(&self, table: Table) -> &[Option<usize>] {
        &self.maps[table as usize]
    }

    /// The id the save's `table` has at `index`.
    pub(crate) fn saved_name(&self, table: Table, index: usize) -> &str {
        &self.names[table as usize][index]
    }

    /// Installs this remap for the ids deserialized on this thread until the guard drops.
    pub(crate) fn install(self) -> RemapGuard {
        REMAP.with(|r| *r.borrow_mut() = Some(self));
        RemapGuard(())
    }

    /// The first index that failed to map while this remap was installed.
    pub(crate) fn failure(&self) -> Option<&Unmapped> {
        self.failed.as_ref()
    }

    fn map(&mut self, table: Table, index: usize) -> Result<usize, Unmapped> {
        let found = match self.maps[table as usize].get(index) {
            None => Err(Unmapped::OutOfRange { table, index }),
            Some(&Some(today)) => Ok(today),
            Some(None) => {
                Err(Unmapped::Missing { table, id: self.names[table as usize][index].clone() })
            }
        };
        if let Err(e) = &found {
            self.failed.get_or_insert_with(|| e.clone());
        }
        found
    }
}

/// `index` of `table` as today's index: through the installed remap, or unchanged if there is
/// none. Only for the `Deserialize` impls below.
fn remap(table: Table, index: usize) -> Result<usize, String> {
    REMAP.with(|r| match r.borrow_mut().as_mut() {
        None => Ok(index),
        Some(remap) => remap.map(table, index).map_err(|e| match e {
            Unmapped::Missing { id, .. } => format!("unknown {} {id:?}", table.kind()),
            Unmapped::OutOfRange { index, .. } => {
                format!("{} index {index} is past the save's table", table.kind())
            }
        }),
    })
}

/// Implements `Deserialize` for a content-id newtype: the same shape the derive reads (a newtype
/// struct named `$name` around `$int`), with the index passed through the installed [`Remap`].
/// `Serialize` stays derived. Every content-id type uses this, with its own [`Table`].
macro_rules! remapped_id {
    ($ty:ident, $int:ty, $name:literal, $table:expr) => {
        impl<'de> Deserialize<'de> for $ty {
            fn deserialize<D: serde::Deserializer<'de>>(de: D) -> Result<Self, D::Error> {
                #[derive(Deserialize)]
                #[serde(rename = $name)]
                struct Raw($int);
                let Raw(index) = Raw::deserialize(de)?;
                let today = remap($table, usize::from(index)).map_err(serde::de::Error::custom)?;
                <$int>::try_from(today).map(Self).map_err(serde::de::Error::custom)
            }
        }
    };
}

remapped_id!(BiomeId, u8, "BiomeId", Table::Biome);
remapped_id!(ResourceId, u16, "ResourceId", Table::Resource);
remapped_id!(TechId, u16, "TechId", Table::Tech);
remapped_id!(ItemId, u16, "ItemId", Table::Item);
remapped_id!(BuildingId, u16, "BuildingId", Table::Building);
remapped_id!(AbilityId, u16, "AbilityId", Table::Ability);
remapped_id!(BeastId, u16, "BeastId", Table::Beast);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn without_a_remap_ids_read_as_their_index() {
        let id: ItemId = ron::from_str("(7)").unwrap();
        assert_eq!(id, ItemId(7));
        let id: BiomeId = ron::from_str("BiomeId(3)").unwrap();
        assert_eq!(id, BiomeId(3));
    }

    #[test]
    fn a_remap_maps_by_name_and_records_what_is_missing() {
        let defs = Defs::bundled();
        let mut saved = SaveIds::of(&defs);
        let bed = defs.building_id("bed").unwrap();
        // The save's table lists a building first that today's data doesn't have.
        saved.buildings.insert(0, "gone".into());
        let guard = Remap::new(&saved, &defs).install();
        let read: BuildingId = ron::from_str(&format!("({})", bed.0 + 1)).unwrap();
        assert_eq!(read, bed);
        assert!(ron::from_str::<BuildingId>("(0)").is_err());
        assert!(ron::from_str::<BuildingId>("(9999)").is_err());
        let remap = guard.finish();
        assert_eq!(
            remap.failure(),
            Some(&Unmapped::Missing { table: Table::Building, id: "gone".into() })
        );
        assert_eq!(ron::from_str::<BuildingId>("(0)").unwrap(), BuildingId(0));
    }

    #[test]
    fn the_guard_clears_the_remap_on_drop() {
        let defs = Defs::bundled();
        let mut saved = SaveIds::of(&defs);
        saved.items.clear();
        {
            let _guard = Remap::new(&saved, &defs).install();
            assert!(ron::from_str::<ItemId>("(0)").is_err());
        }
        assert_eq!(ron::from_str::<ItemId>("(0)").unwrap(), ItemId(0));
    }

    #[test]
    fn the_v1_snapshot_is_the_data_the_v1_fixture_was_made_against() {
        // The fixture's `data_hash` (tests/fixtures/v1_buildings.ron). If this fails, the
        // snapshot was edited: it must never change, only gain entries.
        let fixture_hash = 2_265_902_037_670_627_278;
        let ids = v1_ids(fixture_hash).expect("the fixture's tables are in the snapshot");
        assert_eq!(ids.hash(), fixture_hash);
        assert_eq!(v1_ids(1), None);
    }
}
