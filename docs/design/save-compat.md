# Saves that survive content changes

Status: **implemented** (#602). The owner decided on 2026-11-09 that saves must stay compatible
across updates. This is the implementation plan. The rule it puts in place is in
tech-stack.md, "Saves". The work is **one implementer change**.

Where the implementation differs from the plan below: `inspect` also returns `DataMismatch` for
a v1 header whose `data_hash` has no entry in `v1_ids.ron`, so `fn_ui::saves::list` still marks
such a slot unloadable without the hash arm (and `list` no longer takes `Defs`). The
fixture's `regenerate_v1_fixture` test is removed, since this build can only write v2. The
fixture's body is compared with the re-saved body only while today's id tables still match the
fixture's; after that, `save_compat.rs` covers it.

## The problem in the code

Six content ids are bare table indices with derived serde: `BiomeId(u8)` and `ResourceId(u16)`
(`defs.rs`), `TechId` (`research.rs`), `ItemId` (`jobs.rs`), `BuildingId` (`building.rs`) and
`AbilityId` (`bending.rs`). A save writes them as `(5)`. They turn up all over `World`:

- in every map tile (`Tile::biome`, `Tile::resource`, `Building::id`),
- as map keys (`buried: BTreeMap<Pos, ResourceId>`, `Tallies::harvested: BTreeMap<ItemId, _>`,
  `Unit::bends_by: BTreeMap<AbilityId, _>`),
- inside enums and structs (jobs, `Site::building`, `HeatSource::ability`, research
  `queue`/`log`, and every `Command` in `command_log`),
- and in one place **by position**, with no id stored at all: `Research::known: Vec<bool>`,
  indexed by `TechId`.

`World::save` writes `Defs::id_hash()` (FNV over the seven id tables, nations included, in
order) as `data_hash`. `World::load` refuses any mismatch, and so does `fn_ui::saves::list`,
which marks the slot unloadable. The v1 fixture's hash still matches today's data (checked on
this branch: `save_fixture` passes), but it breaks on the next content PR.

## Recommendation: the save carries its id tables, and ids are remapped while it is read

Of the issue's two options, take the **per-table id list in the save, remapped on load**. Do
the remapping inside each id newtype's `Deserialize` rather than in a pass afterwards.

1. **Format v2 is three lines.** Line 1 is the same `SaveInfo` header with `version:2`. Line 2
   is new: `SaveIds`, the save's id tables in index order. Line 3 is the `World` body, byte for
   byte what v1 writes today.

   ```text
   (magic:"four-nations-save",version:2,data_hash:..,seed:7,nation:fire,tick:1080)
   (biomes:["tundra",..],resources:[..],nations:[..],techs:[..],items:[..],buildings:[..],abilities:[..])
   (seed:7,tick:1080,map:(..),..)
   ```

   `SaveIds` lives in a new `crates/sim/src/save/ids.rs` and is built from `Defs`. Its `hash()`
   is today's `id_hash` algorithm, and `Defs::id_hash()` becomes `SaveIds::of(self).hash()`, so
   its value doesn't move. `nations` is in the struct only so that the hash matches. `Nation`
   is an enum written by name and needs no remapping.
2. **The ids stay small `Copy` indices in memory and on disk.** Each of the six newtypes keeps
   `derive(Serialize)` and drops `derive(Deserialize)` for a hand-written impl from one macro.
   The impl reads the same RON shape through a private `#[serde(rename = "ItemId")] struct
   Raw(u16)`, then maps the index through a thread-local remap table if one is installed. With
   none installed (any deserialization outside `load`), it is the identity, exactly as today.
3. **`load` installs the remap for the duration of the body parse.** It builds one
   `Vec<Option<u16>>` per table, mapping each saved index to `defs.*_id(saved_name)`. It
   installs them with an RAII guard that clears them on return, on error and on panic, then
   calls `ron::from_str` on line 3. Because the mapping happens at the type, it reaches every
   id wherever serde's derive puts it: tile fields, `Option`s, `Vec`s, enum variants, struct
   fields, and map keys. `BTreeMap`/`BTreeSet` are rebuilt by insertion, so they come out
   sorted by the *new* indices with no extra work. Nothing has to list the fields that hold
   ids, and a field added later is covered automatically.
4. **Positional data is fixed up after the parse.** `Research::known` is a `Vec<bool>` over
   the save's tech table. The new `Research::remap(&tech_map, defs)` rebuilds it at
   `defs.techs.len()` and re-sorts `queue`. The queue's invariant is "ascending `TechId`", and
   file order is topological, so sorting by the new index keeps prereqs ahead. This runs
   before `validate`, whose `research.fits` check then holds again.

### Why not "every id serializes as its string"

- Serde's derive gives `Serialize`/`Deserialize` no access to `Defs`, so strings would need the
  same thread-local trick, on the save side as well.
- Every tile would write its biome and resource names: tens of bytes per tile, on a body
  that's already megabytes.
- v1 bodies hold numbers whatever we choose, so the v1 upgrade needs an index-to-name table
  anyway. With the id-list design, v1 is just v2 whose table comes from somewhere else (below).
  With strings, it's a second decoding path.
- The id-list design leaves the body unchanged. The existing "re-saving the v1 fixture gives
  the same text" test keeps proving it, and the change is far smaller.

The one thing strings would buy is a hand-readable save, and nobody reads these by hand.

## Missing content

A saved name with no match in `defs` maps to `None`. Load fails only if the body actually uses
that entry, for example a removed tech the settlement never researched. On a `None` hit, the
`Deserialize` impl records `(table, name)` in the thread-local and returns a serde error. `load`
checks the record before it looks at ron's message and returns:

```rust
SaveError::MissingContent { kind: &'static str /* "building", "tech", .. */, id: String }
// "this save uses the building \"test_cot\", which this version of the game doesn't have
//  (removed or renamed)"
```

A tech the save knows in `known` but that has since been removed is reported the same way.
The other two cases:

- An index past the end of the save's own table is `Corrupt`.
- `DataMismatch` stays, for one case only: a v1 save whose `data_hash` this build has no table
  for. Its message changes to say so.

## Version 1 saves

The upgrade branch is a choice of where the tables come from:

```rust
let (ids, body) = match probe(text)?.version {
    1 => (v1_ids(info.data_hash)?, line 2),       // DataMismatch if the hash is unknown
    2 => (ron::from_str::<SaveIds>(line 2)?, line 3),
    v => return Err(SaveError::Version { found: v, supported: SAVE_VERSION }),
};
```

`v1_ids` looks the hash up in `crates/sim/src/save/v1_ids.ron`, a checked-in snapshot of the
seven tables as they are when this change lands. Generate it once from `Defs::bundled()`. A
unit test asserts that its `hash()` equals the fixture's `data_hash` (2265902037670627278
today). The test is what proves the snapshot is the right one. If a content PR hand-edited the
fixture's hash before this lands, regenerate the snapshot from that data. No build writes v1
any more, so the snapshot never changes afterwards.

`inspect` accepts `1..=SAVE_VERSION` (both headers parse as the same `SaveInfo`).
`SAVE_VERSION` becomes 2.

## Tests

**Data copies.** Add `Defs::from_files(files: [&str; 13])` and have `bundled()` call it, so
that a test builds `Defs` from edited text through the same `from_toml` + `with_*` chain. A
helper in the new `crates/sim/tests/save_compat.rs` loads the 13 `data/*.toml` files as
`String`s and edits them by `[[table]]` block. A building block runs up to the next
`[[building]]` line and keeps its `[[building.cost]]` sub-tables with it. Each case applies
one edit to all six tables at once:

| Case    | Edit (each table)                                                                    |
|---------|--------------------------------------------------------------------------------------|
| append  | a copy of an existing entry under a new id (`test_<table>`), at the end              |
| insert  | the same copy, first in the table, so that every saved index shifts                  |
| reorder | the first entry moved to the end. For techs, a leaf tech (in no one's `prereqs`) moved to the end. For items, reorder the `[[item]]` materials in jobs.toml |

For techs, insert copies a root tech, one with no prereqs. Each case first asserts that the
edit happened, e.g. `building_id("bed")` differs between the copy and `bundled()` for insert and
reorder.

**Saves under test.** Use the v1 fixture, plus a v2 save of a busier world built in the test
from bundled data: a fixed seed per nation, `autoplay` for a few in-game days, then queue some
research. Assert that it has a completed non-starting tech, a non-empty research queue, a
build site, non-zero `tallies` and `bends_by`, and jobs, so that every id-bearing structure is
really exercised.

**Per case, for each save:**

- **Round trip.** Save S0 under bundled data, load it under the copy, save it as S1, load S1
  under bundled data and save it as S2. Assert `S2 == S0`, byte for byte. This is the generic
  check: an id that wasn't remapped, or new positional state, comes back different.
- **By-name spot checks** under the copy: `building_at(BED)` is `bed`, the site's building, a
  known tech, and `harvested(item_id("wood"))`.
- **Play on.** Run 600 ticks under the copy without panicking, then save and load again.

**Missing content.** Under the append copy, designate the appended building, save, and load
under bundled data. The result is `MissingContent { kind: "building", id: "test_building" }`.
A v1 header with an unknown hash gives `DataMismatch`.

**Fixture test edits** (`save_fixture.rs`):

- Delete `the_fixture_matches_the_current_data` and the module doc's "edit the hash" advice.
- `the_fixture_is_a_v1_save…` asserts `SAVE_VERSION == 2`.
- `saving_an_old_save_again_gives_the_same_text` compares the re-saved body (line 3) with the
  fixture's body (line 2), and checks the header and ids lines on their own.

## Other places that change

- `fn_ui::saves::list` drops its `data_hash != id_hash` → `Unloadable` arm. Its `modded.ron`
  test now expects `Ready`. Missing content shows up when the player loads the slot, as any
  load error does.
- Doc comments that say saves are gated by the hash: `save.rs` module docs, `Defs::id_hash`,
  and `RunLogHeader::data_digest` in `fn_tui` ("the same rule `fn_sim::save` applies" is no
  longer true).

**Save size.** One extra line of about 110 ids, roughly 2 KB. The body is unchanged.

**Run logs** are unaffected. A run log is replayed from seed and keystrokes, and that only
reproduces against the same id order (mapgen, menus and key scripts all walk the tables). So
`data_digest` keeps being `Defs::id_hash()`, whose value this change doesn't move, and
`--watch`, `--resume` and `fn_agent verify` keep refusing a mismatch. That is now a run-log rule
only.

## Risks in the current code

- **Positional state.** `Research::known` is the only `Vec` indexed by a content id inside
  `World` today (a grep of `Vec<bool|u32|…>` fields). The round-trip test catches a new one
  only if the test world populates it. The tech-stack.md rule forbids them: key by id in a
  `BTreeMap`/`BTreeSet`.
- **State parallel to a def's own lists is not covered.** These lists depend on a definition's
  *contents*, not on the id tables:
  - `Site::delivered` and `stand_in_delivered` (parallel to `BuildingDef::cost`),
  - `forge.rs`'s `delivered` (parallel to `RecipeDef::inputs`),
  - `SeasonEventKind::Changed { season: usize }`,
  - endeavor stage indices.

  Reordering a building's cost entries while a save has a site of it misattributes deliveries.
  This is outside #602; the rule names it, and an optional follow-up is below.
- **Iteration order after a reorder.** A `BTreeMap<ItemId, _>` iterates in the new index order,
  so a loaded game can make different (still deterministic) choices than it would have made
  under the old data. That is expected. Only "same save + same data → same game" is promised.
- **The thread-local.** It is sound because `ron::from_str` is synchronous on the calling
  thread. The guard must clear it on every exit, and it must never be read outside a
  `Deserialize` impl.

## For #604 (buildings as records, SAVE_VERSION 3)

- **Keep line 2 as it is.** `SaveIds` is version-independent: add a table only if a new id
  newtype appears, with `#[serde(default)]` so that older saves read it as empty.
- **Keep `BuildingId` the same remapping newtype inside the new records.** The id remap then
  covers them for free.
- **Upgrade the body inside the remap scope.** Parse a v1/v2 body into a legacy shape (the old
  `Tile { building: Option<Building> }`), with the guard installed, so its `BuildingId`s are
  already in today's indices. Then convert. `load` becomes: pick the id tables by version, then
  parse the body into the version's shape, then upgrade it step by step to the current shape.
- **Write a v2 fixture first.** Before changing the format, check in one written by this
  build (`fixtures/v2_….ron`, including a multi-tile building and a site). The v1 fixture must
  then load through both steps, and the save_compat cases should run against the v3 format too.

## Open question for the owner

1. **Should v1 saves made with older data also load?** Today's design loads only v1 saves
   whose `data_hash` matches the data in the snapshot. Older tables can be recovered from git
   history and added to `v1_ids.ron`, one entry per hash. **Recommended: no**, unless you have
   v1 saves you want to keep. Those saves are already unloadable today, and the snapshot file
   can take more entries later without a format change.

## Follow-up issues

- Optional, not needed for #602's criteria: *"Saves: in-progress state tied to a def's lists
  survives content edits"*. It covers site and forge `delivered`, the season index and endeavor
  stages, so that editing a cost or recipe list doesn't misattribute what an old save had
  delivered.
