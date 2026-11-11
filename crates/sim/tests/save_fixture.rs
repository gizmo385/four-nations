//! A save written before multi-tile buildings (#196, `docs/design/buildings.md`), kept as text.
//!
//! `tests/save.rs` only round-trips saves made by the current build, so it can't notice a change
//! that makes an *old* file unreadable. This file pins the v1 format: `Building`, `Site` and
//! `HeatSource` gain footprint fields with `serde(default)` instead of a `SAVE_VERSION` bump
//! (#207), and these tests are the proof that the promise holds. They load a checked-in save, not
//! a freshly generated world, so later changes to mapgen or balance can't change what they see.
//!
//! # The fixture
//!
//! `fixtures/v1_buildings.ron` is a 24x16 map of grass with a row of oak trees, and a Fire
//! settlement on it: a finished bed, study hall and campfire (a lit building fire), and a second
//! study hall still being built. It was generated on a hand-made map, so that not even mapgen
//! was involved, by a `regenerate_v1_fixture` test that #602 removed (see git history).
//!
//! It can't be regenerated: the build writes version 4 now (#602, #604, #605), so a regenerated file
//! would no longer prove anything about old saves. Nor does a data change need any edit to it. A
//! v1 save has no id tables of its own; `load` reads its ids against the snapshot of the tables
//! it was made with (`crates/sim/src/save/v1_ids.ron`, picked by its `data_hash`) and maps them
//! to today's data by name, as for any save, then turns its buildings into records (#604), as
//! for a v2 save, then merges its event lists into one log (#605), as for a v3 save. `tests/save_compat.rs` loads it under edited data.

use fn_sim::building::BuildingId;
use fn_sim::save::SaveIds;
use fn_sim::{Defs, Nation, Pos, SAVE_VERSION, World};

const FIXTURE: &str = include_str!("fixtures/v1_buildings.ron");

const WIDTH: i32 = 24;
const HEIGHT: i32 = 16;
const BED: Pos = Pos::new(7, 6);
const HALL: Pos = Pos::new(7, 10);
const CAMPFIRE: Pos = Pos::new(16, 6);
const SITE: Pos = Pos::new(16, 11);

fn id(defs: &Defs, building: &str) -> BuildingId {
    defs.building_id(building).unwrap_or_else(|| panic!("no building {building}"))
}

#[test]
fn the_fixture_is_a_v1_save_with_the_state_the_tests_need() {
    // Guards the other tests against a fixture that was regenerated into something empty.
    let info = fn_sim::save::inspect(FIXTURE).unwrap();
    assert_eq!(info.version, 1);
    assert_eq!(SAVE_VERSION, 4, "a new format version needs a new fixture and an upgrade path");
    assert_eq!((info.seed, info.nation), (7, Nation::Fire));
    assert!(info.tick > 0);
    assert!(FIXTURE.lines().next().is_some_and(|l| l.contains("four-nations-save")));
    assert!(FIXTURE.len() < 100_000, "keep the fixture small: {} bytes", FIXTURE.len());
}

#[test]
fn an_old_save_loads_with_its_buildings_site_and_fire_where_they_were() {
    let defs = Defs::bundled();
    let w = World::load(FIXTURE, &defs).unwrap();
    assert_eq!((w.map.width(), w.map.height()), (WIDTH, HEIGHT));

    // Finished buildings, as 1x1 buildings on their own tiles.
    assert_eq!(w.building_at(BED), Some(id(&defs, "bed")));
    assert_eq!(w.building_at(HALL), Some(id(&defs, "study_hall")));
    assert_eq!(w.building_at(CAMPFIRE), Some(id(&defs, "campfire")));
    assert_eq!(w.map.iter().filter(|(_, t)| t.building.is_some()).count(), 3);
    for pos in [BED, HALL, CAMPFIRE] {
        assert!(w.map.get(pos).unwrap().building.unwrap().passable(), "{pos:?}");
    }

    // The unfinished site, with its work done so far.
    let sites: Vec<_> = w.build_sites().collect();
    assert_eq!(sites.len(), 1);
    assert_eq!((sites[0].0.anchor, sites[0].1), (SITE, id(&defs, "study_hall")));
    assert!(sites[0].2 >= 5.0, "progress {}", sites[0].2);
    let (_, delivered, _) = w.build_site(SITE).unwrap();
    assert_eq!(delivered.iter().sum::<u32>(), 10, "the hall's wood is all delivered");
    assert!(w.building_at(SITE).is_none());

    // The campfire's fire: lit, and warming its neighbours but not the far side of the map.
    assert!(w.is_heated(CAMPFIRE));
    assert!(w.is_heated(Pos::new(CAMPFIRE.x + 1, CAMPFIRE.y)));
    assert!(!w.is_heated(Pos::new(0, 0)));
    assert!(w.flames().is_empty(), "a building's fire isn't a timed flame");

    // The rest of the settlement.
    assert_eq!(w.units.len(), 3);
    assert!(w.tick > 0);
}

#[test]
fn an_old_save_loads_with_zero_tallies_and_bends() {
    // The fixture predates the end-game counts (#233). Its settlers chopped trees before it was
    // saved, but the past isn't reconstructed: everything starts from zero.
    let defs = Defs::bundled();
    let w = World::load(FIXTURE, &defs).unwrap();
    assert!(!FIXTURE.contains("tallies") && !FIXTURE.contains("bends"));
    assert_eq!(w.tallies(), &fn_sim::world::Tallies::default());
    assert_eq!(w.tallies().harvested(defs.item_id("wood").unwrap()), 0);
    assert_eq!(w.tallies().scrolls_read(), 0);
    assert_eq!(w.tallies().crops_harvested(), 0);
    assert!(w.units.iter().all(|u| u.bends() == 0));
}

#[test]
fn an_old_save_carries_on_without_panicking() {
    let defs = Defs::bundled();
    let mut w = World::load(FIXTURE, &defs).unwrap();
    let start = w.tick;
    // The part-built hall gets finished, and the settlement keeps living around it.
    for _ in 0..600 {
        w.tick(&defs);
    }
    assert_eq!(w.tick, start + 600);
    assert_eq!(w.building_at(SITE), Some(id(&defs, "study_hall")));
    assert_eq!(w.build_sites().count(), 0);
    // And it can still be saved and loaded again.
    let again = World::load(&w.save(&defs).unwrap(), &defs).unwrap();
    assert_eq!(again.tick, w.tick);
}

/// Names of fields v1 saves carry that no save writes any more (`Unit::variant`, removed in
/// #607). Loading ignores them; the re-saved text lacks them.
const FIELDS_REMOVED_SINCE_V1: &[&str] = &["variant"];

/// Names of fields added since v1 that a save always writes. They are `serde(default)` fields, so
/// old saves load without them, but new saves write them, so the re-saved text has them and the
/// fixture doesn't. Add a name here when a feature adds a field like that. Footprint fields on
/// `Building`, `Site` and `HeatSource` (#207) should not need an entry: written only when they
/// differ from a 1x1 default (`skip_serializing_if`), a world of 1x1 buildings serializes as v1
/// did.
const FIELDS_ADDED_SINCE_V1: &[&str] = &[
    // Inner Fire's warmth auras, on `World` (#123).
    "auras",
    // Earth rail (#135): a flag on every `Building`, and the rules a hauling unit walks by.
    "rail",
    "cart_rules",
    // Air orchards (#142): the orchards, and the stream that picks where fruit trees grow.
    "orchards",
    "saplings",
    "orchard_rng",
    // End-game counts (#233): the world's lifetime tallies, and each unit's finished bends.
    "tallies",
    // Care tallies (#533): suffering and close calls, on `World`.
    "care",
    "bends",
    // The endeavor (#234): its state, and the signs met and events so far.
    "endeavor",
    "endeavor_log",
    // Units inside buildings (#226): each unit's location state.
    "inside",
    // Population growth (#232): the id counter newcomers draw from, who has joined so far, and
    // the stream their names are drawn from.
    "next_unit",
    "arrivals",
    "growth_rng",
    // The Avatar as a guest (#237): where he stands, each unit's guest tag and patience timer.
    "avatar",
    "guest",
    "unhappy_since",
    // The United City (#240): the stream a newcomer's nation is drawn from once the Harmony
    // Gate is complete.
    "newcomer_rng",
    // Farm plots (#198): which tiles are plots and what stage each is in.
    "farm_plots",
    // The irrigation channel (#126): which tiles speed up farm plots nearby.
    "irrigation_channels",
    // Air Scooter's speed bursts, on `World` (#143).
    "rides",
    // Brick standing in for stone (#371): how much of each build site's cost was paid by a
    // stand-in item rather than its own, on `Site`.
    "stand_in_delivered",
    // Shelters (#362): whether a heat source cooks, and which ability lit it (so the TUI can
    // tell Earth Tent and Air Shield apart from a flame), on `HeatSource`.
    "cooks",
    "ability",
    // The Healing Hut (#375): the healing building a unit is resting in or walking to, and its
    // own retry timer, on `Unit`.
    "healing",
    "healing_retry_at",
    // Draw Water's temporary pool (#364): `Reshape`s waiting to revert, on `World`.
    "reverts",
    // Harm and exposure (#387): the most recent harm each unit took and whether it's already
    // logged the sleeping-rough alert this sleep, on `Unit`.
    "last_harm",
    "exposed_alerted",
    // Injury (#388): the stream injury rolls draw from, on `World`.
    "injury_rng",
    // Sickness from sea water (#389): when a unit's current sickness ends, on `Unit`, and the
    // stream sickness rolls draw from, on `World`.
    "sick_until",
    "sickness_rng",
    // Power reach (#428): which tiles are finished power sources.
    "power_sources",
    // Remedies (#390): a unit's own retry timer for finding one in stores.
    "remedy_retry_at",
    // Bending ranks (#360): finished bends per ability, and the highest rank a unit has been
    // credited the toast for, on `Unit` (replaces the old `mastered` flag, #347).
    "bends_by",
    "rank_credited",
    // Buildings as records (#604, save version 3): one record per finished building.
    "buildings",
];

/// Fields whose shape changed in save version 3 (#604): each tile's `building` became a key
/// into `World::buildings` (a site's `building` is set aside with it, unchanged on both sides),
/// and a building's fire moved out of `heat` into its record. The comparison sets them aside on
/// both sides; `an_old_save_loads_with_its_buildings_site_and_fire_where_they_were` checks what
/// they hold. The map itself is written as `tiles` now, and is renamed back before comparing.
const FIELDS_RESHAPED_IN_V3: &[&str] = &["building", "heat"];

/// The event lists saves up to version 3 kept (`bend_events`, `deaths`, and the research
/// state's `log` in this fixture, which predates the others; the endeavor's were its `events`),
/// and the one log version 4 writes in their place (`events`, #605). The comparison sets them aside on both sides;
/// `saving_an_old_save_again_gives_the_same_text` checks that the log holds every old entry.
const FIELDS_MERGED_IN_V4: &[&str] = &[
    "bend_events",
    "deaths",
    "log",
    "arrivals",
    "season_events",
    "strain_events",
    "harm_events",
    "chart_events",
    "events",
];

/// How many entries the sequences in fields named `names` hold, at any depth of `value`.
fn entries_in(value: &ron::Value, names: &[&str]) -> usize {
    match value {
        ron::Value::Map(map) => map
            .iter()
            .map(|(k, v)| match (k, v) {
                (ron::Value::String(s), ron::Value::Seq(items)) if names.contains(&s.as_str()) => {
                    items.len()
                }
                _ => entries_in(v, names),
            })
            .sum(),
        ron::Value::Seq(items) => items.iter().map(|v| entries_in(v, names)).sum(),
        ron::Value::Option(Some(inner)) => entries_in(inner, names),
        _ => 0,
    }
}

/// Renames the top-level `tiles` field of `value` (a v3 `World`) to `map`, its v1/v2 name.
fn tiles_as_map(value: &mut ron::Value) {
    let ron::Value::Map(map) = value else { return };
    let mut renamed = ron::Map::new();
    for (k, v) in std::mem::take(map).into_iter() {
        let k = match k {
            ron::Value::String(s) if s == "tiles" => ron::Value::String("map".into()),
            k => k,
        };
        renamed.insert(k, v);
    }
    *map = renamed;
}

/// Wraps every `Unit::nation` in `value` (a `World`, as `ron::Value`) in `Some`, in place: #239
/// turned it from `Nation` into `Option<Nation>`, so where the v1 fixture has `fire` (every
/// unit's nation, always the settlement's own: pre-#239 units couldn't be any other), loading and
/// resaving it now writes `Some(fire)` (`World::deserialize_unit_nation` reads a legacy bare
/// value as `Some`, never `None`; `None` is only what a freshly spawned unit of today writes for
/// itself). Touches only `units`, not `World::nation` itself, which stayed a plain `Nation`.
fn wrap_legacy_unit_nations(value: &mut ron::Value) {
    let ron::Value::Map(map) = value else { return };
    let Some(units) = map
        .iter_mut()
        .find_map(|(k, v)| matches!(k, ron::Value::String(s) if s == "units").then_some(v))
    else {
        return;
    };
    let ron::Value::Seq(units) = units else { return };
    for unit in units {
        let ron::Value::Map(fields) = unit else { continue };
        for (k, v) in fields.iter_mut() {
            if matches!(k, ron::Value::String(s) if s == "nation") {
                *v = ron::Value::Option(Some(Box::new(v.clone())));
            }
        }
    }
}

/// `value` without any struct field named in `names`, at any depth. Returns how many it removed.
fn without_fields(value: &mut ron::Value, names: &[&str]) -> usize {
    match value {
        ron::Value::Map(map) => {
            let mut removed = 0;
            let mut kept = ron::Map::new();
            for (mut k, mut v) in std::mem::take(map).into_iter() {
                if matches!(&k, ron::Value::String(s) if names.contains(&s.as_str())) {
                    removed += 1;
                    continue;
                }
                removed += without_fields(&mut k, names) + without_fields(&mut v, names);
                kept.insert(k, v);
            }
            *map = kept;
            removed
        }
        ron::Value::Seq(items) => items.iter_mut().map(|v| without_fields(v, names)).sum(),
        ron::Value::Option(Some(inner)) => without_fields(inner, names),
        _ => 0,
    }
}

#[test]
fn saving_an_old_save_again_gives_the_same_text() {
    let defs = Defs::bundled();
    let w = World::load(FIXTURE, &defs).unwrap();
    let text = w.save(&defs).unwrap();
    // A v1 save is a header and a body; it comes back as v4: a header, the id tables and a body.
    let (old_header, old_body) = FIXTURE.split_once('\n').unwrap();
    let mut lines = text.splitn(3, '\n');
    let (header, ids, body) = (lines.next().unwrap(), lines.next().unwrap(), lines.next().unwrap());
    let info = fn_sim::save::inspect(&text).unwrap();
    let old = fn_sim::save::inspect(FIXTURE).unwrap();
    assert_eq!(info.version, SAVE_VERSION);
    assert_eq!((info.seed, info.nation, info.tick), (old.seed, old.nation, old.tick));
    assert_eq!(info.data_hash, defs.id_hash());
    assert_eq!(ron::from_str::<SaveIds>(ids).unwrap(), SaveIds::of(&defs));
    if info.data_hash != old.data_hash {
        // The data's id tables have changed since the fixture was made, so the two bodies number
        // content differently. `tests/save_compat.rs` covers loading under changed tables; the
        // body's shape can only be compared here while the tables match.
        return;
    }
    // The header also records the seasons knob (#766), which the old one had no field for.
    let old_header = old_header.replacen("version:1", &format!("version:{SAVE_VERSION}"), 1);
    assert_eq!(header, old_header.replacen(')', ",seasons:Some(false))", 1));
    // Not byte for byte (the buildings are records now, #604): compare the two as data, setting
    // aside the fields listed above.
    let (mut old, mut new) = (
        ron::from_str::<ron::Value>(old_body).unwrap(),
        ron::from_str::<ron::Value>(body).unwrap(),
    );
    tiles_as_map(&mut new);
    wrap_legacy_unit_nations(&mut old);
    // One event log (#605): every entry of the old lists is in it, and nothing else. (This
    // fixture's lists are empty; `tests/save_v3_fixture.rs` has one with entries in each.)
    let old_entries = entries_in(&old, FIELDS_MERGED_IN_V4);
    assert_eq!(entries_in(&new, &["events"]), old_entries);
    assert_eq!(w.events().len(), old_entries);
    assert!(without_fields(&mut old, FIELDS_MERGED_IN_V4) > 0);
    assert!(without_fields(&mut new, FIELDS_MERGED_IN_V4) > 0);
    without_fields(&mut old, FIELDS_ADDED_SINCE_V1);
    without_fields(&mut old, FIELDS_REMOVED_SINCE_V1);
    assert!(without_fields(&mut old, FIELDS_RESHAPED_IN_V3) > 0);
    without_fields(&mut new, FIELDS_RESHAPED_IN_V3);
    let removed = without_fields(&mut new, FIELDS_ADDED_SINCE_V1);
    assert!(removed > 0, "the re-saved text differs, but not in the fields listed");
    if old != new {
        let (old, new) = (ron::to_string(&old).unwrap(), ron::to_string(&new).unwrap());
        let at = new.bytes().zip(old.bytes()).position(|(a, b)| a != b);
        let around = |s: &str| {
            let from = at.unwrap_or(0).saturating_sub(60);
            s.get(from..(from + 160).min(s.len())).unwrap_or("").to_owned()
        };
        panic!(
            "re-saving the v1 fixture changed more than the fields added since v1 (first \
             difference at byte {at:?} of the data without them); a world of 1x1 buildings must \
             serialize as v1 did. If this is a new always-written `serde(default)` field (a new \
             `World` field, say), add its name to FIELDS_ADDED_SINCE_V1; a footprint field should \
             be skipped when it is the 1x1 default instead.\nfixture: {}\nresaved: {}",
            around(&old),
            around(&new)
        );
    }
}
