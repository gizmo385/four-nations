Doc status: current

# Buildings as records

Status: **draft** (#604). A technical design with no owner decisions in it. It builds on #602's
save format (save-compat.md, "For #604"). The work is **three sequential issues** (see
"Implementation split").

## What the code does today

A finished building has no entity. `raise` (`world/build.rs`) copies a `building::Building`
(`id`, `passable`, `rail`, `part`, `size`) onto every tile of the footprint. Everything else the
building owns is in a side table keyed by its anchor `Pos`, or in jobs that name that `Pos`:

| State | Where | Filled by | Iterated in |
|---|---|---|---|
| which building, footprint | `Tile::building` on every tile | `raise` | map order (row-major) |
| listing | `World::buildings()`, a whole-map scan | (none) | row-major |
| listing, fast | `building_index` (`(y,x)` keys, `serde(skip)`) + `endeavor.rs::finished_buildings` | `raise`, `rebuild_building_index` on load | row-major |
| craft batch | `crafts: BTreeMap<Pos, CraftState>` | lazily, `forge.rs` | lookups only |
| chart progress | `charts: BTreeMap<Pos, f32>` | lazily, `chart.rs` | lookups only |
| building fire | `heat: BTreeMap<Pos, HeatSource>`, shared with timed bends | `light_building_fire` | order-free (any / max) |
| orchard | `orchards: BTreeMap<Pos, Orchard>` | `open_orchard`, `adopt_orchards` on load | **`Pos` order** (x, then y) |
| farm plot | `farm_plots: BTreeMap<Pos, FarmPlot>` | `open_farm_plot`, `adopt_farm_plots` | **`Pos` order** |
| irrigation | `irrigation_channels: BTreeSet<Pos>` | `open_…`, `adopt_…` | order-free (max) |
| power | `power_sources: BTreeSet<Pos>` | `open_…`, `adopt_power_sources` | **`Pos` order** (stable sort by boost; ties report the first) |
| study halls | the job list (`work.rs::study_halls`) | `open_study_seats` | order-free (any / min) |
| seats, crafts, plots | `JobKind::{Study,Chart,Craft,Sow,Reap} { pos }` | `run_build`, `tick_farms` | job order |
| unit claims | `Unit::{bed, healing, inside}: Option<Pos>` (anchor) | `work.rs`, `inside.rs` | (none) |

`World::buildings()` is called by `bed_route`, `plan_healing`, `bed_capacity` (population
growth), `door_owner`, `autoplay`, the harness trace and `balance/record.rs`. It scans the map
because `World::map` is public and about 45 tests and frontend tests put a `Building` straight
onto a tile. `building_index` can't see those, so it serves only the endeavor check.

Two different orders are in use. The listing is row-major (`(y, x)`), and the orchard, farm and
power tables iterate in `Pos`'s derived order, which is **column-major** (`x` first). Orchard
ticks draw from `orchard_rng` and farm ticks queue Reap jobs in that order, so both orders have
to survive unchanged.

## The design

### The record and its id

A new module, `crates/sim/src/world/buildings.rs`, owns one field on `World`:

```rust
/// A finished building's stable identity. Allocated from `Buildings::next`, never reused.
pub struct BuildingKey(pub u32);                  // plain integer, not a content id: no remap

pub struct Building {                              // the record (today's `Building` is renamed, below)
    pub id: BuildingId,                            // the remapping newtype from #602
    pub anchor: Pos,
    pub size: [u8; 2],                             // baked, never re-derived from the def
    // Per-kind state. Each one is `serde(default, skip_serializing_if = "Option::is_none")`.
    craft: Option<CraftState>,                     // None = fresh batch (as absent from `crafts` today)
    chart: Option<f32>,                            // None = 0.0 (as `charts` today)
    plot: Option<FarmPlot>,
    orchard: Option<Orchard>,
    heat: Option<HeatSource>,                      // the building's own fire, copied as today
}

pub(crate) struct Buildings {
    next: u32,                                     // saved
    records: BTreeMap<BuildingKey, Building>,      // saved
    #[serde(skip)] rows: BTreeMap<(i32, i32), BuildingKey>,  // anchor (y, x): the listing order
    #[serde(skip)] kinds: BTreeMap<(Kind, Pos), BuildingKey>, // per-kind views, Pos order
}

enum Kind { Orchard, Farm, Irrigation, Power, Study }  // derived from the def on insert/load
```

**Ids.** `Buildings::next` is a counter like `next_job` and `next_unit`. Keys are given out in
the order buildings finish, and the sim is deterministic, so they are too. A removed key is
never reused. The v2→v3 upgrade allocates keys in row-major anchor order, starting at 0. A key
is a plain `u32` and needs no entry in `SaveIds`, which stays as it is. `BuildingId` stays the
#602 remapping newtype inside the record, so the remap covers it with no extra code.

Power and irrigation have no state of their own: whether a building is one follows from its
def (`PowerDef`, `IrrigationDef`), and the `kinds` view says so. "Study" means the def has
`seats > 0` and no `chart`, which is exactly the set of buildings that open Study jobs today.

**Tiles point at their building.** `Tile::building` becomes `Option<TileBuilding>`:

```rust
pub struct TileBuilding { key: BuildingKey, passable: bool, rail: bool }  // private fields, getters
```

`passable` and `rail` stay baked onto the tile, so `path.rs` and `flood.rs` still never look up
a record. `id`, `part` and `size` leave the tile. `building_at`, `footprint_at` and
`work_area` read the record through the key, which is one `BTreeMap` lookup. The private fields
mean a frontend can no longer forge a building on a tile. `map_with_pending_walls` and
`would_strand_on` write their throwaway walls onto a *cloned* map with
`TileBuilding::wall(passable)`, whose key is `BuildingKey::NONE`. That map only ever goes to
`Flood`.

### One way in, one way out

- **`World::put_building(foot, id, defs) -> BuildingKey`** (pub). It allocates the key, writes
  the record with per-kind state filled from the def (orchard timer, empty plot, the fire
  through today's `light_building_fire` maths), points every footprint tile at it, inserts the
  `rows` and `kinds` entries, and calls `map_changed`. If the anchor holds a timed flame, it
  removes it, because today a building's fire *replaces* `heat[anchor]`. It doesn't move units
  or open jobs. This is the call that tests and frontend tests use in place of
  `map[p].building = Some(..)` plus `building_index.insert` plus `adopt_*`.
- **`raise`** = `put_building`, then the unit moves it does today (`clear_footprint`,
  `free_walled_in`), then the jobs (`open_chart_seats` or `open_study_seats`, then
  `open_craft_jobs`), in today's order, so job ids don't move. `run_build` and `finish_gate` call
  only `raise`. The `open_orchard`/`farm`/`irrigation`/`power` calls and `light_building_fire`
  in `run_build` and `gate.rs` are removed.
- **`World::remove_building(key, defs) -> Option<Building>`** (`pub(crate)`, for #342). In this
  order:
  1. Every unit with `inside == Some(anchor)` leaves through the door (`leave_building`, which
     reads the footprint, so it runs first).
  2. Every unit whose task is a `Study`/`Chart`/`Craft`/`Sow`/`Reap` job at the anchor drops it
     (the `abandon_job` path).
  3. Those jobs are removed.
  4. `Unit::bed`/`Unit::healing == Some(anchor)` are cleared, and any need task on them is
     finished (`finish_need`).
  5. The footprint's tiles are cleared, the record and its `rows`/`kinds` entries are removed,
     and `map_changed` is called.

  It returns the record, so that #342 can refund a craft batch's deliveries. It doesn't decide
  refunds, and it doesn't decide whether the gate may be removed. Planted orchard trees and
  saplings are map resources, and they stay.

`adopt_orchards`, `adopt_farm_plots`, `adopt_irrigation_channels`, `adopt_power_sources` and
`rebuild_building_index` are deleted, along with their calls in `save.rs`, `fn_ui::inspect` and
the `fn_tui` tests. On load, `Buildings::reindex(defs)` rebuilds `rows` and `kinds` from the
records, and it fills any per-kind state the current def calls for but the record lacks ("fill,
never drop", today's adopt rule). Both are O(buildings), not O(map).

### Side tables: gone or views

| Today | After |
|---|---|
| `crafts`, `charts`, `orchards`, `farm_plots` | gone: the record's `craft`, `chart`, `orchard`, `plot` |
| `irrigation_channels`, `power_sources` | gone: `kinds` view (`Kind::Irrigation`, `Kind::Power`) |
| `building_index`, `finished_buildings` | gone: `rows` |
| `heat` | keeps **timed** sources only. Building fires move to `Building::heat`. `fixed_sources()` chains the two, and `refresh_heat_radii` walks records |
| study halls from the job list | `kinds` view (`Kind::Study`) |
| `sites` | unchanged (out of scope) |
| `JobKind::{..} { pos }`, `Unit::{bed, healing, inside}` | unchanged: they name the anchor, which is unique while the building stands. `remove_building` clears them eagerly, so a later building on the same anchor can't inherit them |

Every former side-table method keeps its signature (`plot_stage(pos)`, `orchards()`,
`chart_progress(pos)`, `craft_idle(pos, ..)`, `power_for(..)`, and so on). Each finds the record
through the anchor's tile key, so frontends don't change.

### The one listing

`World::buildings() -> impl Iterator<Item = &Building>` walks `rows`. Its order, row-major by
anchor, is exactly today's scan order, so `bed_route`'s shared-door `Vec`s, `door_owner`'s
`find_map` and autoplay's first-match picks don't change. Callers move from `(foot, id)` to
`b.footprint()` and `b.id`: about 10 sites in `work.rs`, `population.rs`, `build.rs`,
`endeavor.rs`, `autoplay.rs`, `balance/record.rs` and `fn_harness`. The per-kind views are
internal (`pub(super) fn of_kind(Kind)`) and iterate `kinds` in `Pos` order. That is today's
`BTreeMap<Pos, _>` order, so `tick_orchards`, `tick_farms` and `sources_reaching` see their
buildings in the same sequence.

**Test for "no whole-map scan".** `Grid` gets a `#[cfg(test)]` thread-local counter, bumped in
`Grid::iter` and `Grid::positions` (`geom::test_scans()`). In `world/rest_tests.rs`, on a 64x64
map with a bed, an igloo and a 2x2 healing hut, the test reads the counter, calls
`bed_route(0, &defs)`, `bed_capacity`, `plan_healing` and `door_owner`, and asserts the counter hasn't moved.
`Flood::nearest` doesn't touch either method today. The implementer confirms that first, so the
test can't pass for the wrong reason, and checks that it fails on the old `buildings()`.

## Saves: v2 → v3

`SAVE_VERSION` becomes 3. Line 1 and the `SaveIds` line 2 don't change. The v3 body:

- writes `buildings: (next: N, records: {0: (id: (3), anchor: (x:7, y:6), heat: Some(..)), ..})`,
- writes the map under the field name **`tiles`** (it was `map`), whose tiles carry
  `building: Some((key: 0, passable: true))`,
- drops `crafts`, `charts`, `orchards`, `farm_plots`, `irrigation_channels` and
  `power_sources`, and keeps only timed sources in `heat`.

**Upgrade with deserialize-only fields, not a second `World` type.** A full legacy copy of
`World`'s 58 fields would have to be converted field by field, and a missed field would silently
default. Instead, `World` gets these fields, all read but never written:

```rust
#[serde(default, skip_serializing, rename = "map")]   v2_map: Option<Grid<v2::Tile>>,
#[serde(default, skip_serializing, rename = "crafts")] v2_crafts: BTreeMap<Pos, CraftState>,
// likewise "charts", "orchards", "farm_plots"; irrigation and power sets are def-derived, so ignored
```

The frozen `v2::Tile` and `v2::Building` are today's `Tile` and `Building`, moved into
`crates/sim/src/save/v2.rs`. `World::map` is `#[serde(rename = "tiles", default)]`. A v1 or v2
body fills `v2_map` and leaves `tiles` empty. A v3 body does the reverse. Serde does the
dispatch, with no thread-local beyond #602's. Because the parse runs inside #602's remap guard,
legacy `BuildingId`s are already in today's indices.

`load` becomes: pick the ids by version (#602) → parse the body → research remap →
**`if version < 3 { v2::upgrade(&mut world)? }`** → `validate` → `buildings.reindex(defs)` →
`fixup_next_unit`. `upgrade` does five things:

1. It walks `v2_map` row-major and makes a record for each anchor tile (`part == [0, 0]`),
   allocating keys in that order.
2. It points every tile at its anchor's record. A part with no matching anchor, or a size that
   disagrees with the anchor's, is `Corrupt`.
3. It moves `v2_crafts` and the other shims into their records.
4. It moves each permanent entry of `heat` into the record anchored there.
5. It takes `v2_map` into `map`, then clears every shim.

v1 bodies have the v2 shape, so a v1 save loads through both steps: #602's id table, then this
upgrade.

`validate` gains these checks: no shim is left non-empty, and the map isn't 0x0. Every tile key
names a record whose footprint covers that tile. Every record's footprint tiles point back at
it. Anchors are unique, and `next` is past every key. This is a one-off O(map) pass at load
time, and it is what makes a hand-edited or half-upgraded save fail loudly.

**Fixtures.** Before the format changes, issue 1 checks in `tests/fixtures/v2_buildings.ron`,
written by *today's* build. It's a hand-made 32x24 map (no mapgen) with a bed, a 2x2 healing hut (a door), a
campfire (building fire) plus a timed Hearthflame, a study hall with a seated scholar, a
Map Room with chart progress, a forge mid-batch, an orchard tending a sapling, a farm plot
growing, an irrigation channel, a wind turbine powering the forge, a stretch of rail, and a build
site. It stays under 150 KB. The `#[ignore]`d test that writes it is deleted when the format
changes, as #602 did for v1. Next to it goes `v2_buildings.after.txt`: a digest made only from
public queries (units' ids, positions and task text; jobs; tallies; research pool; ground and
stored stacks; `plot_stage`, `orchard_trees`, `chart_progress`, `power_for`, `is_heated` and
`building_at` over the map), taken after loading the fixture and running 1,200 ticks. Issue 2
must reproduce that digest byte for byte from the v3 build. That is the cross-format proof of
"no behaviour change" for every kind of state. `save_compat.rs` adds the v2 fixture to its
saves, and its round-trip cases run on v3 output.

## Keeping behaviour identical

- **Order.** The listing walks `rows` (row-major), and the per-kind views walk `kinds` (`Pos`
  order), so each loop keeps today's sequence. Records themselves (key order = finish order) are
  never iterated for behaviour. Only `Buildings` touches `records` directly.
- **Floats.** The fire's `HeatSource` is stored as it is today, not recomputed. Craft, chart
  and plot `f32`s move as they are.
- **Job ids.** `raise` opens seats and crafts in today's order. `tick_farms` queues Reap in
  `Pos` order, as today.
- **Gates.** Existing tests pass, mapgen and frame goldens are untouched, and the `v2_buildings`
  digest matches. Before merging, the implementer runs `cargo xtask balance` with no `--bless`
  against `crates/harness/baseline.json` and the reference governor on seed 7633 (the
  `reference_governor.rs` command line). Both runs happen before and after, and the PR records
  both numbers, which must be equal. The test only checks a floor of 250.

## Implementation split

1. **Saves: v2 fixture and behaviour digest before the buildings refactor** (small, no sim
   change).
   - [ ] `tests/fixtures/v2_buildings.ron`, written by the current build, holds every state
     listed under "Fixtures". A test asserts each one is present and non-default.
   - [ ] `v2_buildings.after.txt` and the digest helper (public queries only) are checked in.
     A test loads the fixture, runs 1,200 ticks and compares the digest.
   - [ ] The issue records the balance-tier output and the seed-7633 governor score at this
     commit.
2. **Buildings as records: one table, tile keys, save v3** (large; blocked by 1).
   - [ ] `Buildings`/`Building`/`BuildingKey`/`TileBuilding` as above. `crafts`, `charts`,
     `orchards`, `farm_plots`, `irrigation_channels`, `power_sources`, `building_index`,
     `finished_buildings`, the job-list `study_halls`, every `adopt_*` and
     `rebuild_building_index` are gone.
   - [ ] `World::buildings()` walks `rows`. The `test_scans` test proves that `bed_route`,
     `bed_capacity`, `plan_healing` and `door_owner` don't scan the map.
   - [ ] `put_building` is the only way to place a finished building, and `raise` wraps it.
     Every test and frontend test that wrote `tile.building` uses it.
   - [ ] `SAVE_VERSION == 3`. The v1 fixture loads through both steps, the v2 fixture loads, and
     its digest matches issue 1's byte for byte. `save_compat` passes on v3. `validate` checks
     tile/record consistency.
   - [ ] No behaviour change: all tests pass, goldens are untouched, and the balance tier and
     seed-7633 score equal issue 1's numbers.
3. **Buildings: one remove path** (small; blocked by 2; blocks #342).
   - [ ] `World::remove_building(key, defs)` does the steps above and returns the record.
   - [ ] A test removes one building of every non-endeavor kind in `defs.buildings` while it is
     in use (a scholar seated, a crafter mid-batch, a sleeper in the bed, a Sow job queued, a
     unit inside). After each removal, `validate` passes, `buildings()` no longer lists it, no
     tile, job or unit refers to its anchor, and `put_building` at the same anchor gives fresh
     state.
   - [ ] The tests that cleared `tile.building = None` by hand (`chart`, `power`, `population`)
     call it instead.

## Risks

- **Collision with #574** (flood cost in `work.rs`). Issue 2 rewrites `bed_route` and
  `plan_healing`'s loops there. Land #574 first, or rebase onto it.
- **Hand-placed buildings in tests now get full state.** A test that put an orchard on the map
  without `adopt_orchards` and relied on it doing nothing, or a hand-placed hall that wasn't a
  "study hall" because it had no Study jobs, changes meaning. Those are test edits, not game
  changes: in play, every building goes through `raise`. The digest and the balance gates are
  what tell the two apart.
- **Data edits between save and load.** `kinds` is rebuilt from the current defs. A building
  whose def loses `orchard` keeps its dormant state instead of being dropped on the next tick.
  The building simply isn't ticked, so this isn't observable apart from that.
- **Shim fields live on.** Five read-only fields stay on `World` for as long as v1/v2 saves load,
  which under #602's policy is for good. They're empty after load, and `validate` enforces that.
- **Size of issue 2.** It is mechanical but touches about 15 sim modules and about 45 test
  sites. It can't be split further without a second format bump, because the tile shape and the
  table have to change together.

## Follow-up issues

Only the three issues in "Implementation split". No `decision` issues: nothing here is a taste
or scope call.
