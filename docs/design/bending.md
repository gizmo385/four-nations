# Bending: one signature ability per nation

Doc status: current

Status: **proposed** (#6). Follow-up work: #67 to #72 (see the end of this doc). Fire's second
ability, Inner Fire, was added in #123 (see "Branch abilities"). More abilities per nation and a
route from novice to master are proposed at the end, in "More abilities and the route to mastery"
(#358).

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
| `sleep_regen_per_minute` | 8 | While asleep, times the same sleep-rate multiplier as energy (bed `sleep_bonus`, a heat source, the cold penalty, #345). Rest restores chi, which ties it to the energy need |

- **Effective regen** is the setting × a new modifier `chi_regen`: 1.0 for every nation, and
  Air's `meditation` tech gives ×1.25 (it already promises "chi regeneration"). The modifier
  is added to `Modifiers`, `nations.toml` and `ModifierMultipliers` together (research.md rule).
- **Budget.** With the needs rates in `data/needs.toml`, a unit is awake about 23 minutes and
  asleep about 5 minutes per cycle. That's roughly **85 chi per 28-minute cycle**, or one to
  three bends depending on the ability. The cost table below is tuned against that budget. A
  bed shortens sleep, but earns chi faster by the same factor it shortens sleep (#345), so it
  restores about the same 85 instead of less: the same budget caps how fast a settler can
  become a bending master: see endgame.md, "Mastery pacing" (#340, #345).
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
  unlocks **Bore Tunnel** (#132), the same `reshape` effect aimed at mountains: it turns a bare
  mountain tile beside walkable ground into a walkable `Tunnel`, and minerals beside a tunnel
  yield more (research.md, "Tunneling").

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
- **Later (#56):** wet plots grow faster (`crop_growth` × a data value), the same ability with a
  second reader of the wet state. The `irrigation` tech's channel is the permanent version of
  that boost, already built (docs/design/research.md, "The irrigation channel", #126): a farm
  plot within a channel's radius grows faster for good, with no bending or re-watering needed.

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
| Restoke | Designating a lit flame resets its timer (same chi cost), but only once it has burned below half its time (`restoke_below = 0.5`, #359). Restoking too soon is refused ("flame still burning, restoke below half") and spends no chi |

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
| Recast | Casting it again resets the timer (same chi), but only once the carrier's aura has burned below half its time (`restoke_below = 0.5`, #359). Recasting too soon on a bender whose aura is still warmer than that is refused ("aura still warm, recast below half") and spends no chi |

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
- **Client (historical, removed with it in #281):** the cast effect was a warm glow that swelled
  around the bender (`Look::Glow` in `cast.rs`). While it lasted, the carrier had a soft ring of
  light under their feet, drawn from `World::glows()` (position, radius and time left, as
  `flames()` does for flames). Hovering the carrier outlined its reach, and its tooltip line
  gained "Warm 12m 03s".
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

### Air Nomads: Air Scooter (unlocked by `air_scooter`)

*Aang's spinning ball of air.* The bender steps onto it and rides, much faster than on foot, for
a little while. Where Wind Carry moves goods, Air Scooter moves the bender: a quick way to reach
a far harvest, a stranded hauler's stockpile, or a unit that needs tending, without waiting on
foot the whole way.

| | Value |
|---|---|
| Chi | 20 |
| Channel | 3 s (no speed modifier) |
| Duration | 2 min × `move_speed` (3.84 min for Air's 1.6 × Gliders 1.15 × Air Scooter's own 1.2) |
| Multiplier | 2.5 × the rider's own effective `move_speed`, stacking on top of it |
| Recast | Casting it again resets the timer (same chi) |

- **Who gets it: the caster.** Like Inner Fire, this is a self-cast ability: the designated
  tile is only where the bender stands to summon the scooter (open ground, no resource or
  building on it), not a place the scooter goes. The bender who claims the job rides it, not
  whoever stands on the tile afterward.
- **Why it's a new effect and not a bigger `move_speed` modifier:** a nation modifier is
  permanent and settlement-wide; a ride is temporary and personal to the unit that cast it, and
  it has to end on its own timer without the rest of the sim noticing anything but a faster
  walk. The effect is `scooter` in `data/bending.toml`, kept as a `Ride { multiplier, until }` in
  `world/bending.rs`, by rider. Every place that reads a unit's effective `move_speed` for
  movement (`walk`, `wander` and the client-facing `Stride`) multiplies by the rider's current
  `multiplier`, so no other system needs to know a ride exists. A ride that outlives its rider
  (death) is dropped, the same way an aura is.
- **Scaling:** `magnitude_modifier = "move_speed"` scales `duration_minutes`, so research that
  raises `move_speed` (Gliders, Air Scooter's own modifier, later Sky Bison) also makes the ride
  last a little longer: a nation already fast on its feet keeps the burst going longer, the same
  way `heating` lengthens Hearthflame and Inner Fire. The multiplier itself (2.5) is flat, not
  scaled by anything, and `move_speed` is read once, at the moment the bend completes.
- **Why it's Air's:** Air already leans on speed (base `move_speed` 1.6, Gliders ×1.15), and
  the ride compounds on top of every modifier the nation has earned, so an Air settlement deep
  into its tree rides both longer and faster than one that just unlocked it. It fits the "quick
  response" niche Wind Carry doesn't cover: Wind Carry moves what's already stacked on the
  ground, Air Scooter moves the bender who still has to get somewhere.
- **Counterplay:** the burst ends with its timer, not with distance covered, so a long haul
  can outrun it partway there and finish on foot. It costs chi (20, against Wind Carry's
  signature 15) and a few seconds to summon, so it isn't free to spam. It affects only the
  rider's own movement: it does nothing for anyone else, and unlike Inner Fire it has no
  radius, so there is no interaction with heat, sleep or anyone standing nearby.
- **Terminal frontend (`crates/tui`):** the Bend tool already lists every ability the
  settlement can use generically (`fn_ui::tools::usable_abilities`), so Air Scooter needed no
  tool code once it was in the data. The one entry that is per-ability is the post-bend flash
  (`crates/tui/src/map.rs`'s `HIGHLIGHTS`, the TUI's live equivalent of the old client's
  `cast::LOOKS`): Air Scooter's is `Shape::Target`, the same shape Inner Fire uses, since both
  land on the caster's own tile. A visible "riding" indicator on the map (so another player
  watching can tell who is on a scooter right now, the way a glow marks an Inner Fire carrier)
  is a nice-to-have, not required for the ability to work, and is left to a follow-up issue.
- **Saves:** `rides` is part of world state with `#[serde(default)]`, so a save from before
  #143 loads with none.

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
restoke_below = 0.5             # optional, default 1.0 (today's "always restoke"): a share of
                                 # the duration in (0, 1]; a lit flame with more than that share
                                 # left refuses a restoke (#359)
```

`irrigate` (`source_range`, `wet_minutes`, `sprouts`, `sprout_resource`, `biomes`,
`fallow_minutes`), `wind_carry` (`range`, the magnitude) and `heat_aura` (`duration_minutes`, the
magnitude; `radius`; `sleep_bonus`; `restoke_below`, the same rule applied to the carrier's own
aura) follow the same shape. Each effect kind documents which field its `magnitude_modifier`
scales.

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

## Client (minimum; historical, shipped in #72 and removed with the Bevy client in #281)

- `B` opens the nation's ability tool, and drag-designation works like #62's tools. There's
  one ability now; a small picker appears once research unlocks more.
- A tint shows pending bend designations, a flame sprite (`art/fx.toml`, cycled on the world
  clock) marks a lit flame with its radius on hover, and puddle marks over the floor show wet
  tiles. Fords reuse #53's sprite.
- The selected unit's panel shows a chi bar, and bend events become toasts.
- The terminal frontend's equivalent is docs/design/tui.md's glyph table (lit flame, wet
  background, bend designation tints) and its selection bar's chi meter; that's the live
  reference now, not this section.

## Not in this design

- Healing water, the one remaining branch ability research.md assigns but this doc doesn't
  design. It should use this framework, and #25 wires it up. Inner Fire (#123), Bore Tunnel
  (#132), Seismic Sense (#133), Glide (#197), Metalbend (#134) and Air Scooter (#143) are done.
- Non-benders and per-unit chi max. Every unit bends for now (see question 1). Bending ranks
  (novice, adept, master) are proposed in "More abilities and the route to mastery" (#358).
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

## More abilities and the route to mastery (#358, proposed 2026-11-05)

The owner calls mastery part of the core loop: a settler should grow from novice to master by
bending, and getting there should be fun. The 2026-11-05 agent playtests showed the opposite.
Bending mostly meant one cheap ability over and over (game 2 placed 108 Hearthflames) while chi
ran dry, and nobody could see bend counts or how bends are credited. endgame.md, "Mastery
pacing" (#340), fixed the *pace*, and the owner accepted it: per-nation thresholds (Fire 10,
Earth 8, Water 8, Air 16 bends, implemented by #344), chi regen unchanged, and no practice route
for now. This section keeps those numbers. It adds abilities so that each nation has something
worth bending in every role, and it layers ranks on top of the count, so the route is something
the player sees and plans for.

### Decisions in brief

1. **Fifteen new abilities from seven effect changes.** Each nation gets one in each role it
   lacks (economy, building and access, survival, expression), plus a master technique. Three
   existing effect kinds gain fields and four are new, so the code cost is seven effects, not
   fifteen, and the rest is data.
2. **Three ranks: novice, adept and master.** A master is #344's bend count *plus* bends in at
   least **3 different abilities** ("forms"). An adept is at half the count with 2 forms. The
   count, the threshold numbers and chi regen don't change.
3. **Ranks unlock bending.** Adepts can lead their nation's **festival** (the expression role,
   which brings a newcomer sooner). Masters get a **master technique** (the grand version of
   what novices do: Eternal Flame, Causeway, Ice Bridge, Whirlwind) and pay **80% chi** for every
   bend.
4. **Hearthflame spam gets a direct fix.** A flame can only be restoked once it has burned below
   half its time. The forms rule means one ability can never make a master on its own, and
   Eternal Flame ends the restoking chore for good.
5. **Progress is on screen** through #346 and #347's surfaces: a title in the selection bar,
   bends and forms on the Nation screen, rank tags in the bend picker, and a toast at each rank.
   Optionally, **"ask a bender"** reserves a bend for the selected settler, so the player can
   train someone on purpose (owner question 3).
6. **#339's economy stays put.** No new ability makes stone, ore or food from chi. The new
   levers are access, labour, warmth, thirst and growth. The sections below go through each
   lever.

### Audit: what each nation bends today

"Reason to use" is how often a real colony has cause to designate it, judged from the
playtests and the systems on `main`.

| Nation | Ability | Unlock (cost) | Chi | Channel | Role | Reason to use |
|---|---|---|---|---|---|---|
| Earth | Raise Earth | signature | 50 | 6 s | access | Early and situational: where rivers split the land. A handful to a few dozen per map |
| Earth | Bore Tunnel | tunneling (120) | 40 | 8 s | economy | Steady mid-game while mining: tunnel yields and seams |
| Earth | Seismic Sense | seismic_sense (250) | 30 | 2.4 s | economy | A few per map (radius 8, about 16 seams a map) |
| Earth | Metalbend | metalbending (500) | 35 | 5 s | economy | Rare: metal has no use until #356 |
| Water | Water the Land | signature | 40 | 5 s | economy (food) | Steady until farms take over. **Water's only ability** |
| Fire | Hearthflame | signature | 30 | 4 s | survival | Constant: every flame needs restoking every 20 min, and a restoke resets the timer even at 18 min left. The 108 |
| Fire | Inner Fire | dancing_dragon (250) | 40 | 6 s | survival | Occasional: field workers. Costs more than a bender regains |
| Air | Wind Carry | signature | 15 | 2 s | economy (hauling) | Constant, whenever stacks lie in range |
| Air | Glide | gliders (60) | 40 | 8 s | exploration | A few per map (ruins) |
| Air | Air Scooter | air_scooter (250) | 20 | 3 s | mobility | Rarely worth a designation: a 4-minute burst for whoever happens to claim it |

Channel times are at the nation's own modifiers.

What the audit shows:

- **Water has one ability, and Fire has two that both make heat.** No nation has an expression
  ability. Earth and Air have no survival ability, and Fire and Air have no building one.
- **Fire has nothing but Hearthflame to bend** from minute 0 until Dancing Dragon (about minute
  60 or later). It's cheap, it's always valid, and restoking early wastes nothing the player can
  see. That's exactly the playtests' spam.
- **A third of the abilities are rarely useful** (Metalbend, Seismic Sense, Glide, Air Scooter).
  That's fine for niche tools, but it leaves only one everyday ability per nation.
- **Nothing tells a 10-bend settler from a 0-bend one** except the endeavor sign.

### New abilities

| Nation | Economy | Building and access | Survival | Expression (adept) | Master technique |
|---|---|---|---|---|---|
| Earth | Bore Tunnel, Seismic Sense, Metalbend | Raise Earth | **Earth Tent** | **Stone Lanterns** | **Causeway** |
| Water | Water the Land | **Cut Ice** | **Draw Water**, Healing Water (#125) | **Moon Festival** | **Ice Bridge** |
| Fire | (its kiln, smelter and forge) | **Weld** | Hearthflame, Inner Fire | **Fire Days** | **Eternal Flame** |
| Air | Wind Carry, Glide | **Lift** | **Air Shield**, Air Scooter | **Airball** | **Whirlwind** |

New abilities are in bold. Fire's economy stays its buildings, which is its pillar-1 identity
(industry), and Weld and Eternal Flame free its benders' time for them.
"Defence" means against the environment (cold, thirst, injury), since there are no enemies
(vision.md). As before, no ability targets a unit, so none can harm one.

| Ability | Unlock | Chi | Channel | Effect | What it does | Why it's fun, and its limit |
|---|---|---|---|---|---|---|
| **Earth Tent** | earth_shaping (60) | 30 | 8 s ÷ earthworks | `heat_source`, `cooks = false` | Toph's tent: a stone shelter. Sleepers within 1 tile get warm sleep (×1.4) and no cold, for 12 min × earthworks (30 min for Earth) | Earth sleeps warm at a far quarry in winter. It isn't a bed: it doesn't count for growth or the bed sign |
| **Cut Ice** | ice_construction (120) | 30 | 10 s ÷ water_work | `draw_item` (new) | Freezes and cuts a fresh water or sea tile beside the bender into 4 `ice`, dropped on the bender's tile | Water's own building material. The igloo costs 4 ice instead of 4 wood. Other ice uses are #357's call |
| **Draw Water** | irrigation (60) | 30 | 8 s ÷ water_work | `reshape`, `revert_minutes` | Pulls groundwater up into a pool: an empty Land tile becomes a Ford for 10 min × water_work (23 min for Water), then dries | A drink beside the stockpile, and a fresh-water source for Water the Land away from rivers. Water's kit starts to combine |
| **Healing Water** | healing (120) | 40 | 8 s ÷ water_work | `heal` (new, #125) | Every hurt unit within 2 tiles regains 30 × water_work health | Rescues a starving camp after the food arrives. Only valid while someone in range is hurt, so it can't be spammed. #125 keeps the hut and the healer job |
| **Weld** | forging (60) | 30 | 6 s | `build` (new) | Adds 30 s × heating (75 s for Fire) of work to an ordinary build site. Work done before the materials arrive is kept | Fire's early second form. A bender speeds the kiln up, and later anything #357 adds |
| **Lift** | windmills (120) | 25 | 4 s | `build` | The same, at 30 s × power_generation (60 s, growing with windmills and turbines) | Air's weak backs build with the wind, as Wind Carry hauls with it |
| **Air Shield** | meditation (60) | 20 | 3 s | `heat_source`, `cooks = false`, `sleep_bonus = 1.0` | A dome of still air: no cold within 3 tiles for 6 min × power_generation | Winter work on cold land: a farm or quarry crew keeps its energy. Air's mountain temples are cold |
| **Festival** (Fire Days, Moon Festival, Stone Lanterns, Airball) | adept | 50 | 20 s | `festival` (new) | If growth's own conditions hold (a free bed and `food_per_newcomer` in stores), a newcomer arrives now and eats `newcomer_food` as usual. Once per 30 min, settlement-wide | Expression with a real payoff: the player prepares beds and food, and the festival draws people. A failed festival (too soon, no bed, no food) spends nothing and counts nothing |
| **Eternal Flame** | master | 90 | 30 s | `heat_source`, `eternal` | The Sun Warriors' flame: a Hearthflame that never goes out while its keeper lives. One per keeper, and kindling another moves it | Ends the restoking chore. The master's flame is personal: if they die, it goes out |
| **Causeway** | master | 80 | 30 s ÷ earthworks | `reshape`, `line = 5` | Raises up to 5 river or lake tiles in a straight line from the bank, in one bend | Many bends of fords become one. It's still Earth's lever: rivers and lakes, not sea |
| **Ice Bridge** | master | 80 | 20 s ÷ water_work | `reshape`, `line = 6`, to a new `Ice` terrain | The same over sea, river or lake: walkable ice, permanent like fords | Water's "travel on water" (vision.md) at last, and the sea that Raise Earth leaves to Water. It reaches other islands |
| **Whirlwind** | master | 50 | 6 s | `wind_carry`, `radius = 3` | Wind Carry for every ground stack within 3 tiles of the target | The grand version of Air's everyday bend: a whole felled grove in one gust |

Festival names are data, one ability per nation with the same effect. Water's master technique
was the closest call: plantbending (ripening wild plants) is more canon for a Foggy Swamp
master, but it's another food source, and #339 says food is already too easy.

**Hearthflame's restoke rule.** `heat_source` gains `restoke_below = 0.5`: designating a lit
flame that still has more than half its time left fails ("still burning strong"), so it spends
no chi and counts nothing. Inner Fire's recast gets the same rule. That removes the waste
without removing restoking.

### Effect changes

Seven changes cover all fifteen: three kinds gain fields and four are new. Each is a variant or
a field in `world/bending.rs`, the one obvious place (AGENTS.md rule 6).

| Change | Abilities | Notes |
|---|---|---|
| `heat_source`: `cooks` (default true), `eternal`, `restoke_below` | Earth Tent, Air Shield, Eternal Flame, Hearthflame | `is_heated` already answers cold, so a shelter only needs the cooking check to skip a source with `cooks = false`. An eternal flame stores its keeper and is dropped when they die, the way an aura is |
| `reshape`: `line`, `revert_minutes`; `Terrain::Ice` | Causeway, Ice Bridge, Draw Water | A line runs from the target away from its first passable 4-neighbour (N, E, S, W order), stopping at the first tile not in `from`. A revert stores the old terrain in a `BTreeMap<Pos, (Terrain, u64)>`. A Ford is walkable, so nobody is stranded when it dries |
| `wind_carry`: `radius` (default 0) | Whirlwind | Stacks in `(y, x)` order, each to its own nearest stockpile |
| New `build`, `draw_item`, `heal` | Weld, Lift, Cut Ice, Healing Water | `build` excludes the Harmony Gate in v1, since gate.rs owns its stages |
| New `festival` | Four festivals | Calls growth's own spawn (`World::spawn_newcomer`), and stores `last_festival` in world state |

### The route: novice, adept, master

Every settler has a rank, computed from two numbers the sim already has or can cheaply keep:
finished bends (`Unit::bends`) and **forms**, which are the distinct abilities the settler has
finished at least one bend of.

| Rank | Needs | Fire | Earth | Water | Air | Can do |
|---|---|---|---|---|---|---|
| Novice | nothing | | | | | The signature, and whatever research has unlocked |
| Adept | half the master bends, rounded up, in 2 forms | 5 | 4 | 4 | 8 | Lead the nation's festival |
| Master | #344's bends, in 3 forms | 10 | 8 | 8 | 16 | The master technique. Every bend costs 80% chi. Counts for the masters sign |

- **Titles are data, per nation:** Student, Firebender, Firebending Master. Student, Earthbender,
  Earthbending Master. Student, Waterbender, Waterbending Master. Initiate, Airbender, Airbending
  Master (whose toast says they receive the arrow tattoos).
- **Ranks never go down,** and a newcomer starts as a novice. United City newcomers rank against
  their own nation's numbers and can cast their own nation's rank abilities, as #239's `can_cast`
  already does for signatures. The Avatar has no rank: he casts his own list.
- **Rank gates join research gates.** An ability may name a `rank`, and `can_cast` checks it next
  to `unlocked_by`. A signature ability is still one that no tech unlocks and no rank gates, so
  load validation's "one signature per nation" is unchanged.

**Why forms, weighed against #344's plain count.** The count stays the spine: it's what the
player watches, and #344's numbers already equalise the nations by chi (about 300 chi of
finished bends each).

| Rule | Reads as | Stops one-ability spam | Verdict |
|---|---|---|---|
| Plain count (#344) | "7 of 10 bends" | No: 10 Hearthflames make a master | Kept as the base |
| **Count plus forms** | "7 of 10 bends, 2 of 3 forms" | Mostly: it needs three different abilities | **Recommended** |
| At most half the count from any one ability | "Hearthflame 5 of 5 (full)" | Yes | Stronger, but frustrating while the player can't steer who claims which bend |
| Harder abilities count more | "7.5 of 10" | Partly | No. It's hard to read, and per-nation thresholds already weight by chi |

**Forms don't slow the pace #344 measured.** They change which bends a settler makes, not how
many chi allows. Every nation has a third form well before its third master is due (minute
150): Fire has Weld at forging (60 points) and Inner Fire at Dancing Dragon, Earth has Earth Tent
at earth_shaping (60), Water has Draw Water at irrigation (60) and Cut Ice, and Air has Air
Shield at meditation (60) and Glide. A festival is a form too, but at one per 30 minutes it
can't be a nation's only third form. #343's harness re-measures with a mixed opening to confirm.

**Ask a bender** (owner question 3). With a settler selected, `B` designates a bend reserved for
them: only they claim it (when their chi allows), and the side panel says "for Jee". A
reservation drops if they die. This is a named order, not direct control: the player still
designates work, and needs still pre-empt it. Without it, the forms rule works but the player
can't choose who becomes the master, so the route is less personal. `Command::Bend` gains
`caster: Option<UnitId>` with `serde(default)`.

### Seeing it

These build on #346 and #347, which already plan bends in the selection bar and the Nation
screen, a master toast, and the credit rule on the Endeavor screen.

**Selection bar.** The title is in the border, and the fourth line shows what's left to the next
rank. Edited from `cargo run -p fn_tui -- 42 fire --frame --ascii --ticks 3000 --keys '<tab> <enter>'`:

```
+- Jee - Fire Nation - Firebender -----------------------------------------------------+
| Kindle (Hearthflame)                                         Health ########## 100%  |
| Carrying: nothing                                            Hunger ########..  80%  |
| At (98, 62)                                                  Thirst #######...  70%  |
| Firebender: 9 of 10 bends, 2 of 3 forms to master            Energy #########.  85%  |
| [Esc] close  [Tab] next  [F] follow  [B] ask Jee to bend     Chi    ###.......  30%  |
+--------------------------------------------------------------------------------------+

+- Lu Ten - Fire Nation - Firebending Master ------------------------------------------+
| Kindle (Eternal Flame)                                       Health ########## 100%  |
| Carrying: nothing                                            Hunger ########..  80%  |
| At (98, 62)                                                  Thirst #######...  70%  |
| Master: 12 bends in 4 forms. Bends cost 80% chi              Energy #########.  85%  |
| [Esc] close  [Tab] next  [F] follow  [B] ask him to bend     Chi    ######....  60%  |
+--------------------------------------------------------------------------------------+
```

**Nation screen.** #346's `Bends` column gains `Forms`, and a master shows a star. Edited from
`--keys 'N'`:

```
|                                 Hunger  Thirst  Energy  Health  Chi    Bends  Forms  |
| > Lu Ten Kindle                 ####-   ###--   ####-   #####   #----  Master *      |
|   Jee    Weld                   ###--   ####-   ###--   #####   ##---  9/10   2/3    |
|   Shyu   Haul                   ####-   ###--   ####-   #####   ###--  8/10   3/3    |
|   Ikem   Chop                   ####-   ####-   ##---   #####   #####  4/10   1/3    |
```

**Bend picker** (the side panel's BEND section). A rank ability counts only the settlers who
hold the rank, and it stays listed while nobody does, so the player can see what's coming.
Edited from `cargo run -p fn_tui -- 7 earth --frame --ascii --ticks 3000 --keys 'B'`, with
the Earth branch researched:

```
+- BEND ----------------- [B] -+
| >Raise Earth       6 benders |
|  Earth Tent        6 benders |
|  Bore Tunnel       6 benders |
|  Stone Lanterns     2 adepts |
|  Causeway         no master  |
+- CURSOR ---------- (94, 64) -+
```

**Events.** "Jee is now a Firebender. She can lead Fire Days." "Lu Ten has become a Firebending
Master: Eternal Flame." A finished bend's log line carries its progress ("Jee welded the kiln:
bend 9 of 10, a new form"). A refused designation names the rank: "Causeway needs an
Earthbending Master". On the Endeavor screen, the masters sign reads "Bending masters (10 bends
in 3 forms)", and its "Closest" line shows forms too. Masters are drawn bold on the map
(`data/tui.toml`), so the player can spot them at a glance.

### The economy (materials.md, #339)

| Lever | What changes | Effect on #339's balance |
|---|---|---|
| Stone (Bore Tunnel) | Masters bore at 80% chi. Causeway reaches across rivers | Only Earth's *bonus* (tunnel yields, seams) moves. After #349, base stone supply doesn't need tunnels. Causeway adds reach, not stone |
| Ore and metal (Metalbend) | Masters pay 80% chi | Metal's uses are #356's. The 40-ore sign counts ore mined, and Metalbend doesn't mine |
| Reach | Ice Bridge reaches other islands | Water gains reachable ore, which is where it's short. The supply survey (#348) ignores bridges, so its numbers stay conservative |
| Food (Water the Land) | Draw Water lets it work away from rivers | Same chi per cabbage, so no more food per chi, just more places to grow it. No new food ability: plantbending and fishing were left out on purpose |
| Food into people (festivals) | One extra arrival per 30 min | At #353's 10-minute arrivals, that's about 33% faster growth: 6 to 16 people in about 5 seasons instead of 7. It turns food into people, which is #353's aim. It also pulls the gate's pacing earlier, so #355 re-measures with festivals, and `cooldown_minutes` is the knob |
| Wood | The igloo costs ice, not wood | A small cut in wood demand while wood is in glut. Negligible |
| Labour | Weld, Lift, Whirlwind | materials.md: labour isn't the constraint. These buy time, not materials |

None of the new abilities turns chi into stone, ore, food or wood. That rules out two tempting
master techniques: "move the mountain", which would quarry mountains into unlimited stone, and
plantbending.

### Data

```toml
# data/bending.toml
[mastery]
master_bends = 10
master_bends_per_nation = { earth = 8, water = 8, air = 16 }   # #344's numbers, moved here unchanged
master_forms = 3          # distinct abilities with at least one finished bend
adept_share = 0.5         # adept at this share of the master bends, rounded up
adept_forms = 2
master_chi = 0.8          # a master pays this share of every bend's chi

[mastery.titles]          # novice, adept, master
fire  = ["Student", "Firebender", "Firebending Master"]
earth = ["Student", "Earthbender", "Earthbending Master"]
water = ["Student", "Waterbender", "Waterbending Master"]
air   = ["Initiate", "Airbender", "Airbending Master"]

[[ability]]
id = "eternal_flame"
nation = "fire"
rank = "master"           # optional: "adept" or "master". An ability can have a rank, a tech, both or neither
name = "Eternal Flame"
verb = "Kindle forever"
chi = 90.0
work = 30.0
[ability.effect]
kind = "heat_source"
eternal = true            # burns while its keeper lives, one per keeper
radius = 3
sleep_bonus = 1.5
seek_tiles = 20

[[ability]]
id = "fire_days"
nation = "fire"
rank = "adept"
name = "Fire Days"
verb = "Celebrate"
description = "A festival of firebending. Word spreads, and a newcomer comes sooner."
chi = 50.0
work = 20.0
[ability.effect]
kind = "festival"
cooldown_minutes = 30.0   # shared by every festival in the settlement
```

- The masters sign stays `kind = "masters"`, but it counts units at master rank, so its
  `bends` and `bends_per_nation` move to `[mastery]`. #344 can land first exactly as filed, and
  the ranks issue moves the numbers.
- Validation: `rank` resolves, titles cover every nation, the shares and multipliers are in
  (0, 1], and `master_forms` is no more than the abilities each nation has (or loading fails).
- **Sim state:** `Unit::bends_by: BTreeMap<AbilityId, u32>` (`serde(default)`) next to `bends`,
  plus `last_festival`, `reverts`, and the eternal flame's keeper. There's no RNG and no
  `SAVE_VERSION` bump. A save from before this loads with `bends_by` empty, so its settlers have
  0 forms. Fine for dev saves, and #344's count is unchanged.
- **One helper** (`World::rank_of(unit) -> RankProgress { rank, bends, forms, next }`) feeds
  every frontend, the sign and the hints.

### Open questions for the owner

1. **Should mastery need variety?** *Recommended: yes, #344's count plus 3 forms (2 for adept).*
   It's the smallest rule that stops one-ability spam from making a master, and it reads plainly.
   The alternatives are the plain count alone (keeps the spam), or a cap per ability (stronger,
   but frustrating unless question 3 is yes).
2. **What should ranks unlock?** *Recommended: adepts lead festivals, and masters get a master
   technique and 80% chi.* The alternative is techniques only, with no discount. That's simpler,
   but a master then plays like a novice except for one ability.
3. **Ask a bender: reserve a bend for the selected settler?** *Recommended: yes.* It makes the
   route personal ("I'm training Jee") at the cost of a little indirect control. It's a named
   order, and needs still win. Without it, who becomes a master is down to who stands nearest.
4. **Should expression bends draw newcomers (festivals)?** *Recommended: yes, one per 30 minutes,
   settlement-wide.* It's the one expression effect that uses a real pressure (growth) without a
   mood system. The alternatives are knowledge for research (dead once the tree is done) or
   leaving expression out until mood exists. Ability names and titles are data, so rename
   freely.

### Follow-up issues

Filed without `ready`. The issues waiting on an answer above are labelled `needs-owner`.

| # | Issue | Blocked by | Question |
|---|---|---|---|
| #359 | Hearthflame and Inner Fire: restoke or recast only below half time | none | |
| #360 | Bending ranks: forms, adept and master, rank-gated abilities, master chi | #344 | 1, 2 |
| #361 | TUI: ranks, forms and titles (selection bar, Nation screen, bend picker, toasts, bold masters) | #360, #346, #347 | |
| #362 | Shelters: `heat_source` `cooks = false`; Earth Tent and Air Shield | none | |
| #363 | `build` effect: Weld (Fire) and Lift (Air) | none | |
| #364 | Water: Draw Water (`reshape` revert) and Cut Ice (`draw_item`, an `ice` item, the igloo in ice) | none | |
| #365 | Festivals: the `festival` effect and one adept festival per nation | #360 | 4 |
| #366 | Master techniques: Eternal Flame, Whirlwind, Causeway and Ice Bridge | #360 | 2 |
| #367 | Ask a bender: a bend reserved for the selected settler | none | 3 |
| #370 | Balance check: mastery with forms, and festivals' effect on growth pacing | #360, #365, #343, #352 | |

Healing Water's ability design above goes to #125 (the healing hut, healer job and Healing
Water), as a comment, rather than as a new issue.
