# Bending: one signature ability per nation

Status: **proposed** (#6). Follow-up work: #67 to #72 (see the end of this doc).

Modifiers (`data/nations.toml`) make a nation *better* at things. A signature ability makes it
play *differently*: each one opens a strategy the other three nations don't have. Each nation
has its ability from tick 0, with no research needed (research.md, "Bending"). Abilities serve
peaceful growth and survival (vision.md: no warfare). None of them targets a unit, so none can
harm one.

## Decisions in brief

1. **Four abilities, four different survival levers.** Each one works with systems on `main`
   today, so none waits on buildings or farming:

   | Nation | Ability | What it does | Lever |
   |---|---|---|---|
   | Earth Kingdom | **Raise Earth** | Turns a river or lake tile into a walkable stepping-stone ford | Map access: cross water, shorten hauls |
   | Water Tribes | **Water the Land** | Wets a 3×3 patch near fresh water. Wild greens sprout there a few minutes later | Food supply |
   | Fire Nation | **Hearthflame** | A fuel-free flame. Meals eaten near it count as cooked, and sleep near it is better | Food value and rest |
   | Air Nomads | **Wind Carry** | Blows a whole ground stack into a stockpile up to 16 tiles away, over anything | Hauling, Air's weak spot |

2. **Bending is work, not a spell.** You designate a target, and a bender with enough chi
   claims it, walks there, channels and spends the chi. Needs pre-empt it like any job. This
   keeps indirect control (pillar 2), and the code reuses the job queue from `world/work.rs`.
3. **Chi is a per-unit meter that never kills.** It regenerates slowly while awake and faster
   while asleep. A bender spends it on completion. Low chi just means the unit won't claim
   bend jobs.
4. **Every number is data.** `data/bending.toml` holds chi rates, costs, ranges, durations and
   which modifier scales what. Each ability's *effect* is one variant of a Rust enum in one
   module, `world/bending.rs`, which is the "one obvious place" AGENTS.md rule 7 asks for.
   There's no `if nation == ...` anywhere: an ability belongs to a nation in data, and
   research's locking rule decides who can use it.
5. **Abilities are commands.** `Command::Bend { ability, pos }` and `Command::CancelBend { pos }`
   join #5's `Command` enum (PR #62), so they're logged and replay deterministically.

## Where the pressure comes from today

On `main`, food is the binding constraint. Wild cabbage is finite, and a settlement of four
starves in about two hours (#56). Thirst means walking to water, sleep happens wherever a
unit drops (#58), and rivers and lakes cut the land, with one ford per divided region once #53
lands. Each ability answers that pressure in its own way:

- **Water** grows more food.
- **Fire** gets more out of the same food: cooked cabbage restores 45 hunger instead of 30, and
  fish restore 70 instead of 40.
- **Earth** reaches more food, across rivers and lakes.
- **Air** starts with orchards (renewable food, #56) and uses its ability to make up for weak
  backs (carry 0.8).

## Chi

Each unit has `chi` from 0 to `max`, stored on `Unit` next to `needs`.

| Setting (`[chi]`) | Value | Note |
|---|---|---|
| `max` | 100 | Units start full |
| `regen_per_minute` | 2 | While awake |
| `sleep_regen_per_minute` | 8 | While asleep. Rest restores chi, which ties it to the energy need |

- **Effective regen** is the setting × a new modifier `chi_regen`: 1.0 for every nation, and
  Air's `meditation` tech gives ×1.25 (it already promises "chi regeneration"). The modifier
  is added to `Modifiers`, `nations.toml` and `ModifierMultipliers` together (research.md rule).
- **Budget.** With the needs rates in `data/needs.toml`, a unit is awake about 23 minutes and
  asleep about 5 minutes per cycle. That's roughly **85 chi per 28-minute cycle**, or one to
  three bends depending on the ability. The cost table below is tuned against that budget.
- **Spent on completion, checked on claim.** A unit only claims a bend job if `chi >= cost`.
  Since a unit holds one job at a time, chi can't be double-spent. If the target is no longer
  valid at completion (the stack is gone, the tile has changed), the job is dropped, no chi is
  spent, and an event explains why.
- **Not a need.** Chi doesn't decay, doesn't hurt health and doesn't pre-empt work. Vision.md
  lists chi as a future survival pressure. That can come later, through mood or spirit, without
  changing this meter.
- **Determinism.** Chi uses `f32` like `Needs`: the same operations in the same order every
  tick, regenerated in the needs pass.

## The four abilities

Each ability states its cost and its counterplay. With no enemies, counterplay means the limits
the player has to plan around and the ways the world pushes back.

### Earth Kingdom: Raise Earth

*Toph's bridge of stones.* The bender stands next to a River or Lake tile and raises it into a
**Ford**, the walkable shallow-water terrain from #53.

| | Value |
|---|---|
| Chi | 50 |
| Channel | 15 s ÷ `earthworks` (6 s for Earth's 2.5) |
| Target | River or Lake, 4-adjacent to a passable tile. Not sea or coast |
| Result | Terrain becomes `Ford`. Still water, so units can drink beside it. Marks `regions` dirty and wakes idle units, because new jobs may now be reachable |

- **Why it's Earth's:** #53 guarantees one ford per divided region pair, which still means
  long detours. Earth builds its own crossings where its haul routes need them, and walks
  across a lake to reach cabbage on the far shore. Earth's slow walking (move 0.9) makes short
  routes matter more.
- **Counterplay:** about half a cycle's chi per tile. Fords are built outward one tile at a
  time from dry ground, so a wide lake takes many bends. Sea tiles are off limits (that's
  Water's `water_travel`). Fords are permanent, and there's no undo in v1, so a misplaced
  causeway stays.
- **Research hooks:** `earth_shaping` (earthworks ×1.2) makes the channel faster. `tunneling`
  stays a separate designation for mountain tiles.

### Water Tribes: Water the Land

*Pull water from the river and pour it into the soil.* The target is a 3×3 patch, centered on
a tile within 4 tiles of fresh water (River, Lake or Ford). The patch stays **wet** for a while.
When it dries, up to `sprouts` of its empty, fertile land tiles grow a wild food resource.

| | Value |
|---|---|
| Chi | 40 |
| Channel | 10 s ÷ `water_work` (5 s for Water's 2.0) |
| Target | Center on Land or Coast, within `source_range = 4` of fresh water |
| Wet | 6 min, then up to `sprouts = 2` tiles in the patch sprout `cabbage` |
| Fertile biomes | `plains`, `forest`, `swamp` (a data list) |
| Fallow | A tile that sprouted can't sprout again for 30 min |

- **Why it's Water's:** it's the only renewable food at minute one that needs no tech. Two
  cabbages are 60 hunger for 40 chi, so one bender spending all its chi on watering roughly
  feeds one more mouth. That's strong, but it doesn't snowball, and it costs labor because
  someone still has to gather and haul.
- **Counterplay:** It needs a river or lake nearby, so deserts, tundra and taiga don't respond.
  Food arrives late, 6 minutes after the bend. Fallow tiles stop players from farming one
  patch forever. Sprouts are resources, and **resources block movement and stockpiles**, so
  careless watering clutters the camp.
- **Randomness:** which tiles sprout comes from `rng::stream(seed, "bending.sprout")`, a
  stream held by `World`. Patches dry in `(tick, pos)` order.
- **Later (#56):** when farm plots exist, wet plots grow faster (`crop_growth` × a data value).
  That's the same ability with a second reader of the wet state, and the `irrigation` tech's
  channel becomes the permanent version.

### Fire Nation: Hearthflame

*Iroh heating his tea anywhere.* The bender kindles a flame on an empty Land or Coast tile. The
flame burns without fuel for a while, and it doesn't block movement.

| | Value |
|---|---|
| Chi | 30 |
| Channel | 4 s |
| Duration | 8 min × `heating` (20 min for Fire's 2.5) |
| Radius | 3 tiles (Chebyshev) |
| Cooked meals | A unit eating within the radius eats the food cooked (`cooked_hunger`) |
| Warm sleep | A unit sleeping within the radius regains energy at `sleep_bonus = 1.5` × the base rate. Multiple flames don't stack |
| Seeking | A unit about to sleep walks to a lit flame's radius if one is within 20 path tiles. Otherwise it sleeps where it is, as today |
| Restoke | Designating a lit flame resets its timer (same chi cost) |

- **Why it's Fire's:** others cook only once they have a campfire, which takes a building, wood
  and labor (#58). Fire cooks from minute one, anywhere, without fuel, and its people rest
  better. That's the early-game version of the issue's "fuel-free smelting": Fire's heat never
  needs a wood economy. One Fire bender can keep a flame lit indefinitely (30 chi per 20 min
  is less than its regen), so a Fire camp always has a hearth. The cost is that bender's time.
- **Counterplay:** the radius is small, so the camp has to be compact: the stockpile where people
  eat goes next to the flame. Foragers eating in the field eat raw. The flame dies if no one
  restokes it. It can't be lit on water or on a resource tile.
- **#58 alignment:** campfires and hearths there should be the same kind of **heat source**
  with the same radius semantics, so the cook and warm-sleep code is shared. Hearthflame is
  then a timed heat source that needs no building. `dancing_dragon` (heating ×1.25) lengthens
  the flame. Its "inner fire" ability stays a separate later design.

### Air Nomads: Wind Carry

*A gust that lifts the load.* The bender walks to a ground item stack and sends **the whole
stack**, ignoring carry capacity, to the nearest stockpile tile with room within range. It
flies over rivers, mountains and trees.

| | Value |
|---|---|
| Chi | 15 |
| Channel | 2 s |
| Range | 8 tiles × `power_generation` (16 for Air's 2.0), straight-line Chebyshev distance from the stack |
| Destination | The nearest stockpile tile that holds that item or is empty, ties by position. What doesn't fit stays on the ground with its haul job |
| Target | A tile with a ground stack, and a stockpile in range when designated |

- **Why it's Air's:** Air carries 4 items per trip (base 5 × 0.8). A chopped tree drops 4 wood,
  and a quarried outcrop drops 3 stone. Wind Carry moves up to 20 items in one bend with no
  return trip, and it reaches stockpiles across a river with no ford. Air's fast walkers
  (move 1.6) reach the stacks, and the wind does the lifting.
- **Counterplay:** the range is limited, so stockpiles have to sit near the work. It's one stack
  per bend. The bender still walks to the stack. A full stockpile wastes nothing but also moves
  nothing. It's cheap on purpose, so chi is spent on volume.
- **Interaction with hauling:** designating Wind Carry on a stack takes over that tile's haul
  job. A hauler that has claimed it but hasn't picked anything up is released. If the bend is
  cancelled or fails, the haul job comes back.
- **Research hooks:** `windmills` and `wind_turbines` raise `power_generation`, so the range
  grows (19, then 24 tiles). `air_scooter` and `sky_bison` stay separate.

## Data schema: `data/bending.toml`

```toml
[chi]
max = 100.0
regen_per_minute = 2.0        # awake, times the `chi_regen` modifier
sleep_regen_per_minute = 8.0  # asleep, times the `chi_regen` modifier

[[ability]]
id = "raise_earth"
nation = "earth"              # only this nation can ever use it
name = "Raise Earth"
verb = "Raise stones"         # designation tool label
description = "Raise the riverbed into stepping stones."
chi = 50.0
work = 15.0                   # channel seconds at modifier 1.0
speed_modifier = "earthworks" # optional: divides channel time (like a harvest's `modifier`)
[ability.effect]
kind = "reshape"              # one Rust enum variant per kind; unknown keys rejected
from = ["river", "lake"]
to = "ford"

[[ability]]
id = "hearthflame"
nation = "fire"
name = "Hearthflame"
verb = "Kindle"
chi = 30.0
work = 4.0
magnitude_modifier = "heating"  # optional: scales the effect's one documented magnitude
[ability.effect]
kind = "heat_source"
duration_minutes = 8.0          # the magnitude for this kind
radius = 3
sleep_bonus = 1.5
seek_tiles = 20
```

`irrigate` (`source_range`, `wet_minutes`, `sprouts`, `sprout_resource`, `biomes`,
`fallow_minutes`) and `wind_carry` (`range`, the magnitude) follow the same shape. Each effect
kind documents which field its `magnitude_modifier` scales.

**Validation at load** (`DefsError`, with a test for each): unique ids; the nation exists; chi is
greater than 0 and no more than `chi.max`, since an ability costing more than max chi could
never be cast; work is at least 0; modifier names resolve; terrain, biome and resource ids
resolve; `sprout_resource` has a harvest entry. Research's existing rules cover the rest.
`Unlocks.abilities` gets added (research.md "Staging"), an ability can only be unlocked by a
tech of its own nation, and **an ability that no tech unlocks is a signature ability, available
from the start**. That's research.md's locking rule unchanged, so no `starting` flag is needed.
Once all four exist, loading also checks that every nation has exactly one signature ability.

## Representation in `fn_sim`

```rust
// bending.rs: defs, loading and validation (like research.rs)
pub struct AbilityId(pub u16);
pub struct AbilityDef { id, nation: Nation, name, verb, description, chi: f32, work: f32,
                        speed_modifier: Option<ModifierName>, magnitude_modifier: Option<ModifierName>,
                        effect: Effect }
pub enum Effect { Reshape { .. }, Irrigate { .. }, HeatSource { .. }, WindCarry { .. } }

// jobs.rs
pub enum JobKind { Harvest { pos }, Haul { pos }, Bend { ability: AbilityId, pos: Pos } }

// world/bending.rs: the one place that matches on Effect
impl World {
    fn validate_bend(&self, ability, pos, defs) -> Result<(), BendError>;  // on command and on completion
    fn apply_bend(&mut self, unit: usize, ability, pos, defs);             // spends chi, applies effect
    fn tick_bending(&mut self, defs);   // expire flames, dry wet patches and sprout, in (tick, pos) order
}
// World state: flames: BTreeMap<Pos, u64 /*until tick*/>, wet: BTreeMap<Pos, u64>,
//              fallow: BTreeMap<Pos, u64>, bend_rng: SimRng, bend_events: Vec<BendEvent>
```

- **Tick order:** research → needs and chi → `tick_bending` → units act in id order. A bend that
  completes this tick takes effect immediately, so later units in the same tick see it.
- **Claiming:** bend jobs sit in the same queue and use the same nearest-by-path rule. Units
  without enough chi skip them, and so do units whose nation can't use the ability, which only
  matters once mixed-nation settlements exist.
- **`BendError`:** `Locked { tech }`, `WrongNation`, `InvalidTarget(&'static str)` (for
  example "needs fresh water within 4 tiles"), `AlreadyDesignated`, and `NotDesignated` for
  cancel. It's wrapped as `CommandError::Bend`, so the client can show the reason.
- **Events** (`BendEvent { tick, unit, ability, pos, outcome }`) work like research
  completions and feed toasts and hover text. Examples: "Haru raised stones at (40, 12)",
  "Watering failed: no fresh water".
- **Readable sim:** hover text for a wet tile reads "Wet: sprouts in 3:20", for a flame
  "Hearthflame: 12 min left", and for a unit "Chi 62/100".
- **Saves (#7):** chi, flames, wet and fallow maps and `bend_rng` are part of world state.

## Client (minimum)

- `B` opens the nation's ability tool, and drag-designation works like #62's tools. There's
  one ability now; a small picker appears once research unlocks more.
- A tint shows pending bend designations, a glow tile marks a lit flame with its radius on
  hover, and a darker soil overlay marks wet tiles. Fords reuse #53's sprite.
- The selected unit's panel shows a chi bar, and bend events become toasts.

## Not in this design

- The later abilities that research.md assigns to branch techs: seismic sense, metalbend,
  healing water, air scooter and inner fire. They should use this framework, and #25–#28 wire
  them up.
- Non-benders, bending skill levels and per-unit chi max. Every unit bends for now (see
  question 1).
- Auto-cast (see question 3).

## Open questions for the owner

1. **Does every settler bend?** In canon, benders are a minority. **Recommendation:** every
   unit bends for now, because with 4 to 10 units a non-bender is just a weaker unit. Revisit
   when population growth exists, for example with a `bender_chance` per nation for newborns.
2. **Fire's ability: Hearthflame (cooked meals, warm sleep) or a land-clearing Controlled
   Burn?** **Recommendation:** Hearthflame. It's peaceful, iconic (Iroh's tea) and helps
   survival now. Burning forests echoes the Fire Nation's worst moments in the show, and wood
   is something the settlement needs.
3. **Designate each use, or allow standing orders?** Flames need restoking and stacks keep
   appearing. **Recommendation:** designation only for v1, which keeps indirect control
   explicit and the code small. Add a per-ability auto-cast toggle (for example "keep this
   flame lit" or "wind-carry stacks near this stockpile") as a later issue if the micro gets
   tedious in play.
4. **Should Earth's fords be permanent?** **Recommendation:** permanent, with no undo in v1.
   Shaping the land for good is the Earth Kingdom fantasy, and the chi cost already makes
   players place fords carefully. `earth_shaping` could add "Sink" later if regret becomes a
   problem.

## Follow-up issues

| # | Work | Area | Priority | Blocked by |
|---|---|---|---|---|
| #67 | Bending core: chi meter, `data/bending.toml`, ability defs and research gating, `JobKind::Bend`, `Command::Bend` / `CancelBend`, events | sim, data | p2 | #5 |
| #68 | Raise Earth: river and lake tiles to fords | sim, data | p2 | #67, #30 |
| #69 | Water the Land: wet patches that sprout wild greens | sim, data | p2 | #67 |
| #70 | Hearthflame: fuel-free cooked meals and warm sleep | sim, data | p2 | #67 |
| #71 | Wind Carry: blow a ground stack into a stockpile | sim, data | p2 | #67 |
| #72 | Client: ability tool, chi bar, flame, wet and ford display, bend toasts | client | p2 | #67 |
