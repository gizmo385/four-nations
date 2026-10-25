# Bending: one signature ability per nation

Status: **proposed** (#6). Follow-up work: #67 to #72 (see the end of this doc). Fire's second
ability, Inner Fire, was added in #123 (see "Branch abilities").

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
starves in about two hours (#56). Thirst means walking to water, sleep happens in a bed or wherever a
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
- **#58 alignment (done):** a built campfire is the same kind of **heat source** with the same
  radius semantics, so the cook and warm-sleep code is shared. It is lit for good once built
  (no fuel yet), and its radius scales with `heating`. Hearthflame is the timed heat source that
  needs no building. `dancing_dragon` (heating ×1.25) lengthens
  the flame, and also unlocks Inner Fire, the carried version (below).

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

## Branch abilities

The four abilities above are each nation's signature: available from the start. A branch tech
can add a second ability through `unlocks.abilities`. Inner Fire (#123) is the first.

### Fire Nation: Inner Fire (unlocked by `dancing_dragon`)

*The Sun Warriors' teaching: fire is life and energy.* Where Hearthflame leaves a hearth in
one place, Inner Fire makes the bender **the hearth**. For a while, a warmth aura follows the
bender wherever they go: meals eaten near them are cooked and sleep near them is better, so a
forager or hauler eats cooked food in the field, which Hearthflame can't do.

| | Value |
|---|---|
| Chi | 40 |
| Channel | 6 s (no speed modifier) |
| Duration | 4 min × `heating` (15 min for Fire's 2.5 × smelting 1.2 × Dancing Dragon 1.25 = 3.75) |
| Radius | 2 tiles (Chebyshev), around the carrier |
| Cooked meals | A unit eating within the radius eats the food cooked, as at a flame |
| Warm sleep | A unit sleeping within the radius regains energy at `sleep_bonus = 1.25` × the base rate. Never stacks with a flame's or campfire's: the best one applies |
| Seeking | None. Nobody walks to an aura, because it moves |
| Recast | Casting it again resets the timer (same chi) |

- **Who gets it: the caster.** This is the one self-cast ability. The player designates a tile,
  like every bend, and it means "channel here": open ground with no resource or building on it.
  The bender who claims the job (nearest with enough chi, as always) walks there, channels, and
  **it is that bender who carries the aura**, not the tile. This keeps the command shape of
  `Command::Bend { ability, pos }` and indirect control (pillar 2). The cost is that the player
  can't pick which unit becomes the hearth; see "Owner decision" below.
- **Why it's a new effect and not `heat_source` with a bigger radius:** a flame is a place
  (`BTreeMap<Pos, HeatSource>`) and the aura is a unit's property. Its numbers differ too (no
  `seek_tiles`). The effect is `heat_aura` in `data/bending.toml`, and the sim keeps it as an
  `Aura { unit, radius, sleep_bonus, until }` in `heat.rs`. Every heat question
  (`is_heated`, `sleep_factor_at`) treats a lit aura as a source at its carrier's tile *this
  tick*, so cooking and warm sleep needed no new code paths. Auras are stored by carrier, so one
  unit has at most one, and one that outlives its carrier (death) is dropped.
- **Scaling:** `magnitude_modifier = "heating"`, so research that raises `heating` lengthens
  the aura, as it lengthens a flame. Nothing else scales: the radius and bonus are data.
- **Why it's Fire's:** it extends Fire's identity (cooking and rest anywhere, no fuel) to
  people on the move, and it is where the `dancing_dragon` branch pays off: heating ×1.25
  lengthens it, and Dancing Dragon is the only way to get it.
- **Counterplay:** 40 chi for 15 minutes is about 2.7 chi/min, more than the 2 chi/min a
  waking bender regains, so one bender can't hold it up forever and also cast Hearthflame; a
  camp that wants permanent warmth still builds a campfire. The radius is small (2 against a
  flame's 3), and the sleep bonus is smaller (1.25 against 1.5), so it is a field tool, not a
  home. It only helps while the carrier is close to whoever is eating or sleeping. It can't
  harm anyone, and a carrier who dies takes it with them.
- **Client:** the cast effect is a warm glow that swells around the bender (`Look::Glow` in
  `cast.rs`). While it lasts, the carrier has a soft ring of light under their feet, drawn from
  `World::glows()` (position, radius and time left, as `flames()` does for flames).
  Hovering the carrier outlines its reach, and its tooltip line gains "Warm 12m 03s".
- **Picking it:** `B` selects the Bend tool, and `B` again cycles through the abilities the
  settlement can use (`fn_ui::tools::cycle_ability`, `Session::bend_choice`), so Inner Fire
  appears once Dancing Dragon is known. This is the "small picker" the Client section promised.
- **Saves:** `auras` is part of world state with `#[serde(default)]`, so a save from before
  #123 loads with none.

#### Owner decision: who carries the aura?

Recommended (built): **the caster only**, with the designated tile as the place to channel.
Alternatives that fit the same framework, if the owner prefers:

1. **Target a unit instead** (the aura goes to whoever stands on the tile when the channel ends,
   so a bender can warm a friend). More control, but a moving target makes the command
   unreliable, and it stops being "inner".
2. **Aura on all units in a 3×3 area at completion**: not a following aura, so it fails the
   issue's intent.

Changing to option 1 touches only `validate_effect` and the `HeatAura` arm of `apply_effect`
(`world/bending.rs`), because everything downstream reads `Aura::unit`. The numbers are all in
`data/bending.toml`.

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
`fallow_minutes`), `wind_carry` (`range`, the magnitude) and `heat_aura` (`duration_minutes`, the
magnitude; `radius`; `sleep_bonus`) follow the same shape. Each effect
kind documents which field its `magnitude_modifier` scales.

**Validation at load** (`DefsError`, with a test for each): unique ids; the nation exists; chi is
greater than 0 and no more than `chi.max`, since an ability costing more than max chi could
never be cast; work is at least 0; modifier names resolve; terrain, biome and resource ids
resolve; `sprout_resource` has a harvest entry. Research's existing rules cover the rest.
`Unlocks.abilities` gets added (research.md "Staging"), an ability can only be unlocked by a
tech of its own nation, and **an ability that no tech unlocks is a signature ability, available
from the start**. That's research.md's locking rule unchanged, so no `starting` flag is needed.
Once all four exist, loading also checks that every nation has exactly one signature ability
(the abilities that no tech unlocks); a tech-unlocked one like Inner Fire doesn't count.

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
- A tint shows pending bend designations, a flame sprite (`art/fx.toml`, cycled on the world
  clock) marks a lit flame with its radius on hover, and puddle marks over the floor show wet
  tiles. Fords reuse #53's sprite.
- The selected unit's panel shows a chi bar, and bend events become toasts.

## Not in this design

- The later abilities that research.md assigns to branch techs: seismic sense, metalbend,
  healing water and air scooter. They should use this framework, and #25–#28 wire them up.
  Inner Fire is done (#123).
- Non-benders, bending skill levels and per-unit chi max. Every unit bends for now (see
  question 1).
- Auto-cast (see question 3).

## Owner decisions (2026-10-22)

The owner accepted every recommendation:

1. **Every settler bends** for now. With 4 to 10 units, a non-bender is just a weaker unit.
   Revisit when population growth exists, for example with a `bender_chance` per nation for
   newborns.
2. **Fire gets Hearthflame** (cooked meals, warm sleep), not a land-clearing Controlled Burn.
   It's peaceful and iconic, and wood is something the settlement needs.
3. **Designation only for v1.** A per-ability auto-cast toggle (for example "keep this flame lit"
   or "wind-carry stacks near this stockpile") is a later issue if the micro gets tedious.
4. **Earth's fords are permanent**, with no undo in v1. `earth_shaping` could add "Sink" later if
   regret becomes a problem.

## Follow-up issues

| # | Work | Area | Priority | Blocked by |
|---|---|---|---|---|
| #67 | Bending core: chi meter, `data/bending.toml`, ability defs and research gating, `JobKind::Bend`, `Command::Bend` / `CancelBend`, events | sim, data | p2 | #5 |
| #68 | Raise Earth: river and lake tiles to fords | sim, data | p2 | #67, #30 |
| #69 | Water the Land: wet patches that sprout wild greens | sim, data | p2 | #67 |
| #70 | Hearthflame: fuel-free cooked meals and warm sleep | sim, data | p2 | #67 |
| #71 | Wind Carry: blow a ground stack into a stockpile | sim, data | p2 | #67 |
| #72 | Client: ability tool, chi bar, flame, wet and ford display, bend toasts | client | p2 | #67 |
