# Research horizons, phase 4: schools

Doc status: current

Status: **proposed** (#668). This is phase 4 of [research-horizons.md](research-horizons.md)
(accepted 2026-11-10). It follows the shape of phases 1 to 3
([horizons-phase-1.md](horizons-phase-1.md), [horizons-phase-2.md](horizons-phase-2.md) and
horizons-phase-3.md, #665 to #667) and reuses their patterns: the `core` rule (#673), the
`upgrades` and `features` unlock kinds (#674, #677), fuel bunkers (#675), track buildings (#676)
and beasts (#693). It turns owner decisions 1 and 2 into a `school` field, a 1,000-point tier
that costs items as well as knowledge, a fork on the Research screen, and eight techs in TOML.
`metal_city` and `great_works` are designed against power reach (late-tree.md, power.md, the
#428 family). The two owner questions are under "Open questions for the owner", and the
implementation work is under "Follow-up issues".

## Decisions in brief

1. **One fork per nation, closed on learning.** Two techs that name the same `[[school]]` exclude
   each other. Learning one closes the other for the rest of the game, and the sim refuses a
   closed tech with `ResearchError::Closed`. Until then the player may switch, and any items
   already delivered come back. Choosing asks for a confirm, because the bill starts moving at
   once.
2. **The top tier costs items, hauled like a build site's bill.** `[[tech.cost_items]]` lists
   items that haulers bring to any study hall while the tech is queued. A tech completes when its
   knowledge is banked *and* its bill is in. Only techs of 1,000 points or more may have one, so
   the first 500 points of the tree stay labour-only (owner decision 2).
3. **The gate never needs a school.** School techs can't be `core`, so the branch sign and the
   building-kinds sign ignore them and their buildings (owner decision 4). No tech may list a
   school tech as a prereq, so a closed option never strands anything.
4. **Eight school techs, each a different answer to one pressure.** Earth walls in its winter or
   rails its ore. Fire makes its flames free or its machines run alone. Water builds in ice or
   lives off the whole coast. Air settles at a temple or packs up and moves. Each uses an existing
   system plus one new piece: enclosure detection, power carried along rail, a heat aura,
   automated recipe buildings, a stand-in item, shore fishing, a landmark aura, and a full refund.
5. **Most effects are upgrades, so the data says what changes.** Phase 4 adds six upgrade
   properties (`boost`, `automate`, `fuel_free`, `aura`, `free`, `shelter`) and lets an upgrade
   name an item (`stands_for`). Three new buildings carry the rest: the Ring Gate, Metal Rail and
   the High Temple.
6. **The harness plays both options of every fork.** Two styles, `school_first` and
   `school_second`, and a new `[[fork]]` check: neither option may finish the gate's material
   stages sooner on more than 75% of paired runs, per nation.

## What ships

| School | Tech | Nation | Cost | Unlocks | The decision |
|---|---|---|---|---|---|
| Which city? | `agrarian_ring` | Earth | 1,000 + 6 tools | **Ring Gate**; **sheltered rings**: inside a closed ring of Earth Wall, no cold, no frost, crops grow in winter | How much land to wall in, at 3 stone a tile, and where the gates go |
| | `metal_city` | Earth | 1,000 + 8 metal | **Metal Rail** (2.5x, lays in tunnels, carries power); Hot Spring Mill boost 1.5x | Ore goes to the gate or to the line |
| What is fire for? | `sun_warriors` | Fire | 1,000 + 3 scrolls | Campfires heal (2x) and restore chi (1.5x) in their reach; Hothouse and Steam Engine burn nothing | Warmth as life: hearths placed where people work and rest |
| | `great_works` | Fire | 1,000 + 8 metal | **Factory**: recipe buildings within 4 tiles of a Lightning Station run with no crafter | Hands freed for the gate, at the cost of metal and a tight yard |
| North or South? | `northern_city` | Water | 1,000 + 6 tools | **Ice masonry**: ice pays any stone cost one for one | A city at the water's edge, paid in chi instead of quarrying |
| | `southern_ways` | Water | 1,000 + 3 scrolls | Waystations cost no materials and keep crews within 6 tiles out of the cold; **shore fishing** from any water's edge | A tribe spread along the coast, building little |
| Temple or sky? | `high_temple` | Air | 1,000 + 40 stone | **High Temple**: one per game, touches rock, never moves. Within 8 tiles chi returns 2x and study runs 1.5x | Settle, and spend Air's scarcest material on it |
| | `nomad_way` | Air | 1,000 + 3 scrolls | **Pack up**: taking a building down returns its full bill, twice as fast. Bison haul to build sites | Move to fresh land and mild winters instead of fighting tired ground |

Every nation sees exactly one school, from tick 0, and learns at most 1,000 more points through
it. Each school tech's only prereq is the nation's capstone, so the fork opens at minute 110 to
150 on the economy opening, which is the start of "the road" in the spec's timeline.

## The `core` rule, and what a school may not be

School techs leave `core` at its default, false (horizons-phase-1.md, "The rule every phase-1
tech follows"), and load refuses a school tech marked `core = true`. So the branch sign never
asks for a closed tech, and the building-kinds sign never asks for a Ring Gate in a Metal City
game. The unit test from #673 that pins each nation's two targets runs again with phase-4 data.

## Shared sim work

### 1. Schools

```toml
# data/research.toml, a new top-level table, one row per school
[[school]]
id = "earth_city"
nation = "earth"
question = "Which city?"     # the fork's heading on the Research screen and in the confirm

# on each of its two techs
[[tech]]
id = "agrarian_ring"
school = "earth_city"
```

- **Validation.** `school` names a `[[school]]` row. A school has exactly two techs, both of its
  own nation. A nation has at most one school. A school tech is not `core`, is no nation's
  starting tech, and is no tech's prereq. A `[[school]]` row that no tech names fails to load.
  Two is a rule of the data, not of the code: the exclusion is "any other tech of the same
  school", so a three-way fork later is a one-line validation change and a wider fork box.
- **Defs.** `TechDef` gains `school: Option<SchoolId>`. `Defs::school_mates(tech)` lists the
  other techs of its school.
- **When it closes.** A tech is **closed** while any school mate is known. Nothing else closes it.
  Queuing a school tech closes nothing, so the player can switch targets until one is learned.
  That is the spec's "learning one closes the other".
- **The refusal.** `Research::set_target` returns `ResearchError::Closed { chosen: TechId }` for
  a closed tech. Its `Display` reads `the other path of this school was chosen`, and clients name
  it: `Closed: you chose Metal City`. This is the same shape as `BuildError::NotResearched(TechId)`.
  `Research::closed_by(tech, defs) -> Option<TechId>` is the query clients and the agent surface
  read.
- **Saves.** Nothing new is saved: closed is derived from `known`. A save whose data later puts
  two known techs in one school keeps both. Load doesn't refuse it, and both effects apply.
- **Autoplay.** The economy opening's research order skips closed techs. Which option it picks
  is the harness style's `school` field ("Measuring it").

### 2. Item bills: `[[tech.cost_items]]`

```toml
[[tech]]
id = "metal_city"
cost = 1000
[[tech.cost_items]]
item = "metal"
count = 8

# data/research.toml [settings], two new keys
cost_items_from = 1000        # only a tech costing at least this may have cost_items
bill_hall = "study_hall"      # the building that takes deliveries; must have seats > 0
```

- **Validation.** `cost_items` is allowed only on a non-core tech whose `cost` is at least
  `cost_items_from`. Each item exists and is listed once, with a count from 1 to 99. Phase 5
  decides whether a teaching's scroll goes through this list or its own rule, and may lower
  `cost_items_from` if so.
- **State.** `Research` gains `bills: Vec<Bill>` (serde default empty), where `Bill { tech,
  delivered: Vec<u16> }` follows the def's `cost_items` order. A queued tech with `cost_items`
  has a bill. The front tech's bill is what clients show.
- **Delivery.** While a queued tech's bill is short, the world posts research deliveries: hauls
  from stores to the nearest finished `bill_hall`, on the same tier as a build site's deliveries
  and with the same "at most what's still owed" rule. An item delivered at the door leaves the
  world and is counted on the bill, as a scroll is used up today. Fuel in a bunker is the model:
  it is out of stores, so nothing eats or spends it twice.
- **Completion.** `Research::tick` completes the front tech when the pool covers its scaled cost
  *and* its bill is full. The difficulty preset's `research_cost` scales knowledge only, never
  items, as it never scales a building's bill.
- **Waiting on the bill.** If the pool already holds the front tech's whole cost and the bill is
  short, study seats are not claimed, exactly as with an empty queue (#284). Scholars become hands
  for the hauling, and nothing studied is clamped away. The Research row reads `Waiting for 3
  metal`.
- **Switching.** When `set_target` drops a tech with a bill, its delivered items drop beside the
  door of the first finished `bill_hall` in row-major anchor order, as ordinary stacks with Haul
  jobs. That is how phase 3's `SetRecipe` returns a batch. With no hall standing, the bill is kept
  and refunded the next time one stands. Nothing is lost but the hauling.
- **Scrolls.** Ancient scrolls are never stockpiled. A hauler carries each one straight to a hall,
  where it is read (`run_deliver`). With a bill short of `ancient_scroll`, an arriving scroll goes
  on the bill instead. It gives no knowledge and isn't counted as read, so it doesn't advance the
  scrolls sign (#715). The confirm says so, and a scroll on the way when the target changes
  is read as usual.

### 3. New upgrade properties

#674's rules hold: an upgrade names one target and exactly one property, and each (target,
property) pair is upgraded by at most one tech. Each property states what its target must
already have, which is #674's "an upgrade can't turn a house into a study hall".

| Property | Target must have | Value | Effect | Used by |
|---|---|---|---|---|
| `boost` | `[building.power]` | 1.0 to 4.0 | Replaces the source's `boost` | `metal_city` |
| `automate` | `[building.power]` | `{ rate, radius }`: 0.1 to 1.0, 1 to 8 | Recipe buildings in `radius` (unscaled) run with no crafter at `rate` | `great_works` |
| `fuel_free` | `[building.fuel]` | `true` | The bunker never burns, and no Refuel haul is posted. The effect stays on | `sun_warriors` |
| `aura` | `[building.heat]` | `{ heal, chi_regen, study }`, each 1.0 to 4.0 | A `[building.aura]` over the building's own heat reach | `sun_warriors` |
| `free` | a `cost` | `true` | Sites placed afterwards have no bill. Sites already waiting keep theirs | `southern_ways` |
| `shelter` | nothing | 1 to 12 | No tile within this radius (unscaled) is cold | `southern_ways` |
| `stands_for` (on an `item`) | no `stands_for` of its own | an item id | The item pays that item's costs one for one, as brick pays stone | `northern_city` |

An upgrade on an item uses `item = "ice"` in place of `building = ...`. Exactly one of the two
is given. Stand-ins become a world query, `World::stands_for(item, defs)`: the static `stands_for`
or the known upgrade. Every cost-paying path asks it in place of the def. The stone sign keeps
its own `items` list, so ice never counts as stone in stores.

### 4. Small pieces

- **Features.** `sheltered_rings`, `shore_fishing` and `pack_up` join #677's `Feature` enum.
- **`[building.aura]`** (new, generic). It has `radius` (Chebyshev from the footprint, unscaled)
  and optional `heal`, `chi_regen` and `study` multipliers, each defaulting to 1.0. At least one
  must be above 1.0. Units in reach regenerate health and chi that many times as fast. Study seats
  in a hall whose footprint is in reach run `study` times as fast, multiplied with the power
  boost, in integer millis as `boost_millis` is. The best value per stat applies, and two auras
  don't stack. The `aura` upgrade gives a heat source the same section, with the heat reach as
  its radius. It is the shape #202's Spirit Oasis (chi 2x within 4 tiles) can reuse.
- **`landmark = true`** (new). At most one per settlement, counting sites, and it can't be taken
  down. Placing a second is refused with `only one`. Deconstruct (#342) refuses with `A landmark
  stays`.
- **`also_on = [terrain, ...]`** (new). The footprint may stand on these terrains as well as
  buildable land. It is only allowed on a passable 1x1 building, so it can never seal a tunnel.

## The techs

Numbers are starting values for the harness. Hover text means the build modal's effects list
and the cursor panel's inspect lines, in the voice of text-voice.md.

### Earth: which city?

#### `agrarian_ring`: sheltered rings

```toml
# data/research.toml, appended
[[school]]
id = "earth_city"
nation = "earth"
question = "Which city?"

[[tech]]
id = "agrarian_ring"
name = "Agrarian Ring"
description = "Ring the fields in stone, and winter stays outside the walls."
nation = "earth"
cost = 1000
prereqs = ["metalbending"]
school = "earth_city"
[[tech.cost_items]]
item = "tool"
count = 6
[tech.unlocks]
buildings = ["ring_gate"]
features = ["sheltered_rings"]

# data/buildings.toml, appended
[[building]]
id = "ring_gate"
name = "Ring Gate"
verb = "Raise"
work = 30.0
passable = true           # people walk through it; it still closes a ring
[[building.cost]]
item = "stone"
count = 4
[[building.cost]]
item = "wood"
count = 2

# data/seasons.toml, a new table, read only while `sheltered_rings` is known
[enclosure]
walls = ["earth_wall", "ring_gate"]   # finished buildings that close a ring
winter_growth = 0.75                  # replaces the climate band's, unscaled: the Hothouse's number
frost = false                         # the first frost spares ripe crops inside
cold = false                          # no unit inside is cold, so there is no exposure
```

**Enclosure detection.**

- A tile is a **bound** if a finished building listed in `walls` stands on it.
- One flood starts from every map-edge tile that isn't a bound. It steps in eight directions
  through every tile that isn't a bound, whatever its terrain, so mountains and water don't
  close a ring (#716). Every tile that isn't a bound and the flood never reaches is
  **sheltered**.
- Eight directions means a diagonal step in a wall is a gap, the same gap a walker could slip
  through. Rings drawn with the build tool's rectangle are orthogonal lines, so they close.
- The result is a bitmap, `World::sheltered`, derived and never saved. It is rebuilt when a bound
  finishes or comes down, when the feature is learned, and after load. One flood over a
  192x128 map is about 25,000 tiles, and walls change rarely.
- `World::is_sheltered(pos)` feeds three readers. `is_cold` is false on a sheltered tile.
  `season_growth_factor` uses `max(band, winter_growth)` there in winter. The first frost skips
  sheltered tiles. The flood runs the same with seasons off, but nothing reads it then.
- A ring needs a way in. The cut-off rule (#384) already keeps the last wall from sealing anyone
  in, so in practice the Ring Gate goes in before the ring closes. Gates are bounds, so a ring
  with three gates is as sheltered as a ring with one.

- **Numbers.** A ring round a 16x16 field is 68 wall tiles: 204 stone and about 10 minutes of
  one builder at Earth's `construction` with its branch (2.4). It shelters 256 tiles, room for
  about 20 plots, the beds and a hall. The Hothouse warms 32 plot tiles for 30 wood a winter.
  The ring costs stone once, the land inside grows with the square of its side, and it also
  ends exposure for everyone inside.
- **Hover.** `Ring Gate: people pass, and it closes a ring`. Inspect on any tile inside: `Sheltered:
  inside a ring of 256 tiles`. The Earth Wall's build entry gains `A closed ring shelters what's
  inside`.
- **The decision.** How much land to wall in, at 3 stone a tile, when the stone sign wants 200.
  The ring shapes every route too: walls are solid, so the gates are the only way in, and
  haulers queue at them. On a warm site winter barely bites, and the ring is worth much less
  there. That is the point of a site-dependent fork.

#### `metal_city`: Metal Rail

```toml
# data/research.toml, appended
[[tech]]
id = "metal_city"
name = "Metal City"
description = "Lay rails of bent metal that carry goods and power through the mountain."
nation = "earth"
cost = 1000
prereqs = ["metalbending"]
school = "earth_city"
[[tech.cost_items]]
item = "metal"
count = 8
[tech.unlocks]
buildings = ["metal_rail"]
[[tech.unlocks.upgrades]]
building = "hot_spring_mill"     # power.md's Earth source (#448)
boost = 1.5                       # from 1.2: the other nations' first sources

# data/buildings.toml, appended
[[building]]
id = "metal_rail"
name = "Metal Rail"
verb = "Lay"
work = 12.0
passable = true
also_on = ["tunnel"]
[building.track]
speed = 2.5               # a straight step costs 4 (phase 1's track rule); stone rail costs 5
power_reach = 1           # new: consumers within 1 tile of a live run are powered
[[building.cost]]
item = "metal"
count = 1
[[building.cost]]
item = "stone"
count = 1
```

- **Power along the line.** A **run** is a 4-connected set of finished tiles of one track
  building with `power_reach`. A run is **live** when any of its tiles is in a source's reach,
  and it takes the best `boost` among the sources reaching it. A consumer whose footprint comes
  within `power_reach` of a live run's tile is powered at that boost, as if a source reached it.
  `World::sources_reaching` (power.rs) adds those runs to its answer. Runs are cached per map
  revision. The reach tint (#429) covers the run and its one-tile margin.
- **Why 2.5 and not 3.0.** Steps are integers on a 10-unit grid (horizons-phase-1.md): 10 / 3.0
  can't be represented, and 3 gives 3.33. A 67% jump over stone rail on top of carrying power is
  too much. 2.5 is a quarter faster than stone rail, and the power is the prize.
- **Why a new building, and not an upgrade to rail.** Metal per tile is the decision: a 20-tile
  line costs 20 metal, which is 40 ore through Metalbend. An upgrade would make all rail metal
  for the price of the tech.
- **Numbers.** A Hot Spring Mill reaches 2 tiles at Earth's 0.8. With metal rail, a Scroll
  Library 30 tiles from the rock runs at 1.5x, which is what every other nation's first source
  gives. A Deep Mine down a tunnel takes the same line.
- **Hover.** `Rail: hauling runs 2.5x faster along it`, `Carries power to buildings beside it`
  and `Can be laid in tunnels`. Inspect on a consumer: `Powered x1.5 by rail from Hot Spring Mill
  (8, 4)`.
- **The decision.** Ore goes to the gate (32 ore over the Pillars and the Arch) or to the line.
  The line also decides where the city goes: along the rail, from the rock face out to the
  fields.

### Fire: what is fire for?

#### `sun_warriors`: the Eternal Sun

```toml
# data/research.toml, appended
[[school]]
id = "fire_purpose"
nation = "fire"
question = "What is fire for?"

[[tech]]
id = "sun_warriors"
name = "Sun Warriors"
description = "Learn fire from the dragons: flames that need no fuel, and warmth that heals."
nation = "fire"
cost = 1000
prereqs = ["lightning_generation"]
school = "fire_purpose"
[[tech.cost_items]]
item = "ancient_scroll"
count = 3
[tech.unlocks]
[[tech.unlocks.upgrades]]
building = "campfire"
aura = { heal = 2.0, chi_regen = 1.5 }     # over the campfire's own heat reach
[[tech.unlocks.upgrades]]
building = "hothouse"                      # #680
fuel_free = true
[[tech.unlocks.upgrades]]
building = "steam_engine"                  # only once #433 gives it a [building.fuel]
fuel_free = true
```

- **Rule.** Every campfire heals and restores chi in its heat reach, and the Hothouse and Steam
  Engine keep working with empty bunkers that are never refilled. Bunkers already full are
  returned on deconstruct as today. If #672 makes campfires burn wood, a fourth upgrade line
  makes them free too.
- **Numbers.** Fire's `heating` is 3.75 with its branch, so a campfire reaches 8 tiles: a 17x17
  square. Health regen goes from 2 to 4 a minute and chi from 2 to 3 a minute awake (before
  `chi_regen`). That saves the Hothouse's 30 wood a winter and the Steam Engine's burn (about 60
  an hour under #433).
- **Hover.** `Flames heal 2x and restore chi 1.5x nearby` on the campfire, `Needs no fuel` on the
  two machines. Inspect on a unit in reach: `By a sun-warmed fire: healing 2x`.
- **The decision.** Where the hearths go. A campfire by the quarry heals the injured miners
  (harm.md), and one by the bending ground refills the firebenders. On Hard, with injuries and
  exposure on, this is the safer school.

#### `great_works`: the Factory

```toml
# data/research.toml, appended
[[tech]]
id = "great_works"
name = "Great Works"
description = "Lightning drives the machines, and the yard runs with no hands at the bench."
nation = "fire"
cost = 1000
prereqs = ["lightning_generation"]
school = "fire_purpose"
[[tech.cost_items]]
item = "metal"
count = 8
[tech.unlocks]
[[tech.unlocks.upgrades]]
building = "lightning_station"
automate = { rate = 1.0, radius = 4 }    # radius from the footprint, unscaled
```

- **Rule.** A finished building with a `[building.recipe]` and `uses_power`, whose footprint is
  within `radius` of an automating source, is **automated**. It posts no craft job. Its batch
  runs at `rate` times the recipe's plain speed (its `modifier` applies, the power boost
  doesn't). It holds at most one batch of inputs, brought by a **Supply** haul: #675's Refuel
  haul with a recipe's input list in place of one fuel item. Output drops beside the door as
  today, and `stock_limit` still idles it.
- **The toggle.** `Command::SetAutomated { pos, on }`, default on. Off gives the building back to
  a crafter at the full 2.0x boost. It is refused with `needs Great Works`, or `not in a Lightning
  Station's yard`.
- **Why a fixed radius.** The Station's power reach is 10 tiles at Fire's `power_generation`,
  which covers the whole settlement. A 4-tile yard round a 2x2 is a 10x10 square: room for four
  2x2 recipe buildings and a lane. That is the spec's "tight yard".
- **Numbers.** An automated batch runs at half a powered crafter's speed with no hands. Fire's
  forge, kiln, smelter and Workshop in the yard make about two crafters' worth of goods, and free
  four settlers for the gate.
- **Hover.** `Machines within 4 tiles run with no crafter, at half speed`. Inspect on a recipe
  building: `Automated: Batch 2/3 Metal, 40%`, or `Waiting for inputs`.
- **The decision.** Which four buildings get the yard. Placing the Station was a power question.
  It is now also a layout question, and buildings already standing may be worth taking down
  (#342) and raising again inside the yard.

### Water: North or South?

#### `northern_city`: ice masonry

```toml
# data/research.toml, appended
[[school]]
id = "water_pole"
nation = "water"
question = "North or South?"

[[tech]]
id = "northern_city"
name = "Northern City"
description = "Cut ice like stone, and raise a city of it at the water's edge."
nation = "water"
cost = 1000
prereqs = ["spirit_oasis"]
school = "water_pole"
[[tech.cost_items]]
item = "tool"
count = 6
[tech.unlocks]
[[tech.unlocks.upgrades]]
item = "ice"
stands_for = "stone"
```

- **Rule.** Ice pays any stone cost one for one: buildings, the Paved Road, and the gate's stages.
  A site takes stone, brick or ice, whichever is nearest, as it takes stone or brick today.
- **Numbers.** Cut Ice makes 4 ice for 30 chi. The gate's stone (about 180 over the Foundation
  and the Pillars) is 45 cuts, so a few benders' chi for a season. The stone sign still counts
  only stone and brick in stores.
- **Hover.** The Research detail reads `Unlocks: Ice pays for stone`. Build costs read `12 Stone
  (or ice)`.
- **The decision.** Chi and the water's edge in place of a quarry. Cutting ice ties benders to
  the shore, and a Water site far from good stone becomes a strong one.

#### `southern_ways`: ranging camps

```toml
# data/research.toml, appended
[[tech]]
id = "southern_ways"
name = "Southern Ways"
description = "Range the whole coast in small camps that keep the crews warm and fed."
nation = "water"
cost = 1000
prereqs = ["spirit_oasis"]
school = "water_pole"
[[tech.cost_items]]
item = "ancient_scroll"
count = 3
[tech.unlocks]
features = ["shore_fishing"]
[[tech.unlocks.upgrades]]
building = "waystation"          # #694
free = true
[[tech.unlocks.upgrades]]
building = "waystation"
shelter = 6

# data/jobs.toml, a new table, read only while `shore_fishing` is known
[shore_fishing]
item = "fish"
count = 1
work = 30.0                       # at water_work 1.0; Water's branch makes it about 10 s
modifier = "water_work"
terrains = ["water", "lake", "river", "canal"]
frost_hardy = true
```

- **Shore fishing.** The harvest tool may designate a Fish order on any listed water tile with a
  walkable neighbour, not only on a fish spot. It never depletes and has no regrow. The fisher
  stands on the neighbour. A standing order (#81) keeps it going.
- **Ranging camps.** A Waystation placed after the tech has no bill, only its work. Nobody
  within 6 tiles of a finished one is cold. Its pantry, beds and far-from-stores rule are as
  #694 builds them.
- **Numbers.** A shore fisher makes about 6 fish a minute with Water's branch (`water_work`
  2.9). That is a fraction of a fish spot's burst, but it never runs out. Food overflows from
  minute 60 anyway, so the value is where the food is: at a far camp, the crew fishes beside its
  Waystation and the pantry lasts.
- **Hover.** `Waystations cost only work, and keep crews within 6 tiles warm`. The harvest
  tool's hint on a water tile reads `Fish from the shore`.
- **The decision.** Spread out or build up. A Southern game places two or three free camps by the
  far ore, the ruins and the mild land, and its crews never come home in winter.

### Air: temple or sky?

#### `high_temple`: the High Temple

```toml
# data/research.toml, appended
[[school]]
id = "air_path"
nation = "air"
question = "Temple or sky?"

[[tech]]
id = "high_temple"
name = "High Temple"
description = "Raise a temple on the peak, where chi and study come easily."
nation = "air"
cost = 1000
prereqs = ["sky_bison"]
school = "air_path"
[[tech.cost_items]]
item = "stone"
count = 40
[tech.unlocks]
buildings = ["high_temple"]

# data/buildings.toml, appended
[[building]]
id = "high_temple"
name = "High Temple"
verb = "Raise"
work = 300.0
passable = false
footprint = [3, 3]
beside = ["mountain"]     # it touches the peak
landmark = true
[building.aura]
radius = 8                # Chebyshev from the footprint, not modifier-scaled
chi_regen = 2.0
study = 1.5
[[building.cost]]
item = "stone"
count = 40
[[building.cost]]
item = "wood"
count = 30
[[building.cost]]
item = "tool"
count = 4
```

- **Rule.** One temple, against the rock, never taken down. Units within 8 tiles regain chi twice
  as fast, and study halls within 8 tiles run half again faster, multiplied with their power.
- **Numbers.** 80 stone in all, the tech's 40 and the building's 40. That is close to half of what
  the gate asks, from the nation with `earthworks` 0.7. A Meditation Pavilion under a Wind Turbine
  and the temple runs at 1.5 x 1.5 = 2.25x. The chi aura speeds the masters sign for monks who
  bend near it.
- **Hover.** `Chi returns 2x and study runs 1.5x within 8 tiles`, `Must touch rock` and `One per
  game; it can't be moved`. Inspect: `Halls in reach: 2`.
- **The decision.** Where to settle for good, against the rock, with the halls round it. It is
  the only building Air can never move, which is the point.

#### `nomad_way`: pack up

```toml
# data/research.toml, appended
[[tech]]
id = "nomad_way"
name = "Nomad Way"
description = "Build light, pack up whole, and follow fresh land."
nation = "air"
cost = 1000
prereqs = ["sky_bison"]
school = "air_path"
[[tech.cost_items]]
item = "ancient_scroll"
count = 3
[tech.unlocks]
features = ["pack_up"]

# data/buildings.toml, [deconstruct] (edited in place), two new keys
[deconstruct]
refund = 0.5
work = 0.5
packed_refund = 1.0      # with `pack_up` known: the whole bill comes back
packed_work = 0.25       # and a quarter of the building's work, not half
```

- **Rule.** With `pack_up`, taking a building down returns its whole bill in half the usual time.
  Haul beasts (#693) also take build-site deliveries of at least `min_trip`, carrying up to their
  `carry` of one bill line. A bison brings a whole line of most bills in one trip.
- **Why not "a whole bill" in one trip.** A unit carries one stack today. Mixed loads would touch
  every haul path for one school. One line per trip, at 20, covers every Air bill line but the
  High Temple's, which Nomad games can't build.
- **Numbers.** Moving an orchard and four plots 30 tiles costs their work and some walking, and
  nothing else. On Hard, where a repicked tree dies at strain 3.0, moving to fresh ground beats
  waiting for it to rest.
- **Hover.** `Buildings come down twice as fast and return everything` on the Clear tool. Bison
  inspect: `Hauling to a build site`.
- **The decision.** Move or stay. A Nomad game follows mild land into winter and fresh land when
  the ground tires, with the gate as the one thing that stays put.

## Against each other

A fork is good when each side wins on some maps. Each pair answers the nation's main pressure
two ways, and the site tips it.

| Nation | Option A is stronger when | Option B is stronger when | What both spend |
|---|---|---|---|
| Earth | Winters are cold (cool and polar bands) and stone is plentiful | Ore is near the rock and the settlement is long or tunnelled | Tools or metal the Arch also wants |
| Fire | Harm is on (Hard), and wood is short | The gate's labour is the bottleneck, and the yard has room | Scrolls or metal |
| Water | Stone is far and the coast is close | The map is large or polar, and the ore or ruins are far | Tools or scrolls |
| Air | Research and the masters sign lag, and the rock is near the core | The land tires (Hard) or winters are harsh | Stone or scrolls |

## Mockups

Hand-built in the full-screen style of #652, where the Research screen covers the side panel and
has a detail strip at the bottom. The base is `fn_tui --scenario earth-year-two --frame --ascii
--size 110x34 --keys 'T <right*5>'`, which today draws the screen 78 columns wide beside the side
panel. Phase 1 to 3 techs are drawn where their prereqs put them. The fork is a box in the
deepest tier, headed with the school's `question`. The TUI follow-up settles the glyphs (`x`
for closed, `*` for queued) and the keys.

At tick 0 the fork is already visible, and both options are locked behind Metalbending:

```text
+----------------------------------------------- * RESEARCH * -----------------------------------------------+
| <TIER 3                   TIER 4                    TIER 5                    TIER 6                       |
| - Masonry                 - Cartography             - Earth Rail              - Omashu Chutes              |
|   Locked          120 pts   Locked          250 pts   Locked          250 pts   Locked          250 pts    |
|   Needs: Carpentry          Needs: Writing            Needs: Earthen Walls      Needs: Earth Rail          |
|                                                                                                            |
| o Writing                 - Earthen Walls           - Metalbending            .- WHICH CITY? ---------.    |
|   Available       120 pts   Locked          120 pts   Locked          500 pts |>- Agrarian Ring       |    |
|   Study x1.25, Scroll Li~   Needs: Earth Shaping, ~   Needs: Seismic Sense, ~ |  Locked      1000 pts |    |
|                                                                               |  + 6 tools            |    |
| - Tunneling               - Seismic Sense           - Badgermole Ways         |---------- or ---------|    |
|   Locked          120 pts   Locked          250 pts   Locked          500 pts | - Metal City          |    |
|   Needs: Earth Shaping      Needs: Tunneling          Needs: Tunneling, Seis~ |  Locked      1000 pts |    |
|                                                                               |  + 8 metal            |    |
| - Paving                  - Calendar                - Archives                '-----------------------'    |
|   Locked          120 pts   Locked          250 pts   Locked          500 pts                              |
|   Needs: Masonry            Needs: Writing            Needs: Writing, 3 scro~                              |
|                                                                                                            |
|                                                                                                            |
|                                                                                                            |
|                                                                                                            |
|                                                                                                            |
|                                                                                                            |
|                                                                                                            |
|                                                                                                            |
|                                                                                                            |
|                                                                                                            |
| ---------------------------------------------------------------------------------------------------------- |
| Agrarian Ring                                                            Needs prereqs  1000 pts + 6 tools |
| Ring the fields in stone, and winter stays outside the walls.                                              |
| Unlocks: Ring Gate, Sheltered rings (inside a closed Earth Wall: no cold, no frost, crops grow in winter)  |
| School: Which city? Choosing one closes the other for the rest of the game.   Needs: Metalbending          |
+------------------------------------------------------------------------------------------- tiers 3-6 of 6 -+
 RESEARCH  Arrows choose - Enter set as target - Esc or T close                                         ? keys
```

Later, with Metalbending known, `Enter` on Metal City asks first. The box names what is given
up, what the bill is, and that it comes back on a change of mind:

```text
+----------------------------------------------- * RESEARCH * -----------------------------------------------+
| <TIER 3                   TIER 4                    TIER 5                    TIER 6                       |
| + Masonry                 + Cartography             + Earth Rail              + Omashu Chutes              |
|   Known           120 pts   Known           250 pts   Known           250 pts   Known           250 pts    |
|   Construction x1.15        Map Room                  Rail, Carry x1.25         Chute                      |
|                                                                                                            |
| + Writing                 + Earthen Walls           + Metalbending            .- WHICH CITY? ---------.    |
|   Known           120 pts   Known           120 pts   Known           500 pts | o Agrarian Ring       |    |
|   Study x1.25, Scro.------------------------ CHOOSE A SCHOOL -------------------------.ble   1000 pts |    |
|                    |                                                                  |ols            |    |
| + Tunneling        |  Which city? Metal City or Agrarian Ring.                        |-- or ---------|    |
|   Known           1|                                                                  | City          |    |
|   Bore Tunnel, Mine|  Set Metal City as the target? Once it is learned, Agrarian Ring |ble   1000 pts |    |
|                    |  is closed for the rest of this game.                            |tal            |    |
| + Paving           |                                                                  |---------------'    |
|   Known           1|  It costs 1000 pts and 8 metal. Haulers take the metal to a      |                    |
|   Paved Road       |  Scroll Library as soon as you choose. Change your mind before   |                    |
|                    |  it is learned and the metal comes back.                         |                    |
|                    |                                                                  |                    |
|                    |  In stores: 11 metal                                             |                    |
|                    |                                                                  |                    |
|                    |              Enter  choose Metal City     Esc  back              |                    |
|                    '------------------------------------------------------------------'                    |
|                                                                                                            |
|                                                                                                            |
|                                                                                                            |
|                                                                                                            |
| ---------------------------------------------------------------------------------------------------------- |
| Metal City                                                                   Available  1000 pts + 8 metal |
| Lay rails of bent metal that carry goods and power through the mountain.                                   |
| Unlocks: Metal Rail (2.5x hauling, lays in tunnels, carries power), Hot Spring Mill boost 1.5x             |
| School: Which city? Choosing one closes the other for the rest of the game.                                |
+------------------------------------------------------------------------------------------- tiers 3-6 of 6 -+
 CHOOSE A SCHOOL  Enter choose - Esc back                                                               ? keys
```

Chosen and on the way (left), then learned (right). Only the fork column and the detail strip
change:

```text
    .- WHICH CITY? ---------. .- WHICH CITY? ---------.
    | o Agrarian Ring       | |>x Agrarian Ring       |
    |  Available   1000 pts | |  Closed      1000 pts |
    |  + 6 tools            | |  You chose Metal City |
    |---------- or ---------| |---------- or ---------|
    |>* Metal City          | | + Metal City          |
    |  Queued      1000 pts | |  Known       1000 pts |
    |  640 pts, metal 5/8   | |  Metal Rail, Mill 1.~ |
    '-----------------------' '-----------------------'
```

```text
| Metal City                                                                 Queued  640/1000 pts  Metal 5/8 |
| Lay rails of bent metal that carry goods and power through the mountain.                                   |
| Unlocks: Metal Rail (2.5x hauling, lays in tunnels, carries power), Hot Spring Mill boost 1.5x             |
| Bill: Metal 5/8, hauled to a Scroll Library. Agrarian Ring stays open until this is learned.               |
```

```text
| Agrarian Ring                                                                   Closed  1000 pts + 6 tools |
| Ring the fields in stone, and winter stays outside the walls.                                              |
| Unlocks: Ring Gate, Sheltered rings (inside a closed Earth Wall: no cold, no frost, crops grow in winter)  |
| Closed: this settlement chose Metal City. A school is one choice for the whole game.                       |
```

The side panel's Research row while the bill is short, and a hall under the cursor:

```text
+- SETTLEMENT ----------- [N] -+    +- HERE ------------ (31, 22) -+
| Research: Metal City 64% [T] | <  | Scroll Library               |
|   Metal 5/8 on the way       | <  | Seats: 4/4                   |
| Gate 6/9 - 2 scrolls     [G] |    | Toward Metal City: Metal 5/8 | <
+------------------------------+    +------------------------------+
```

An Agrarian Ring on the Earth map, with the wall tool selected. `#` is Earth Wall, `O` a Ring
Gate, `~` farm plots, `HH` the Scroll Library, and `:` the sheltered tint drawn only while a wall
tool is selected:

```text
,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,      HERE ------------ (40, 22) -
,,,,##############,,,,,,,,,,,,      Plains (40, 22)
,,,,#~~~~:~~~~:~~#,,,,,,,,,,,,      Sheltered: inside a ring of 84 tiles  <
,,,,#~~~~:~~~~:~~#,,,,,,,,,,,,      Cool climate: crops grow in winter here  <
,,,,#::::::::::::#,,,,,,,,,,,,
,,,,#HH::::::::A:#,,,,,,,,,,,,
,,,,#HH::::::::::O,,,,,,,PPPP,
,,,,#::::::::::::#,,,,,,,PPPP,
,,,,##############,,,,,,,,,,,,
```

Metal rail from a Hot Spring Mill (`SS`) on the rock face, up through a tunnel past a Deep Mine
(`GG`), and out to a Workshop (`WW`) and the Scroll Library (`HH`). `&` is metal rail, and the
`&` tiles inside the rock lie on tunnel:

```text
^^^^^^^^^^^^^^^^^^^,,,,,,,,,,,,,,,,      HERE ------------ (32, 4) -
^^^GG^^^^^^^^^^^^^^,,,,,,,,,,,,,,,,      Scroll Library
^^^GG&&&&&&&&&&&&&&&&&&&&&&&&&&&,,,      Powered x1.5 by rail     <
^^^^^&^^^^^^^^^^^^^,,,,,,,,,,WW&,,,        from Hot Spring Mill   <
^^SS,&,,,,,,,,,,,,,,,,,,,,,,,WW&HH,        (2, 4)                 <
^^SS&&,,,,,,,,,,,,,,,,,,,,,,,,,,HH,
```

## Measuring it

Phase 4 ends with a harness run (#713 is the phase-3 run before it), and the economy opening
must play both options of every fork.

**Two styles.** `data/harness.toml` gains a style field, `school = "first" | "second"`: which of
the nation's school techs, in file order, the opening targets as soon as its capstone is known,
ahead of the phase 1 to 3 techs. `steady` leaves it unset and reaches the school last, in file
order, as the control.

```toml
[[style]]
id = "school_first"
inherits = "steady"
school = "first"

[[style]]
id = "school_second"
inherits = "steady"
school = "second"

# A new check: two runs that must NOT separate too far, per nation
[[fork]]
id = "schools_both_viable"
doc = "horizons-phase-4.md#measuring-it"
metric = "stage_minute.3"     # new: the minute the endeavor's third stage (the Arch) finished
a = { style = "school_first", preset = "normal" }
b = { style = "school_second", preset = "normal" }
tie = 5.0                     # paired runs within 5 minutes are ties
max_share = 0.75              # neither side may win more than 75% of paired runs
minutes = 300
```

**What the opening does with each school**, once it is learned:

| School | Scripted use |
|---|---|
| `agrarian_ring` | One ring round the farm plots and the nearest beds: the smallest rectangle with a 1-tile margin, at most 120 wall tiles, a Ring Gate on the side facing the stockpile |
| `metal_city` | A metal rail line from the Hot Spring Mill to the nearest Scroll Library or Workshop, at most 30 tiles |
| `sun_warriors` | One more campfire by the quarry the opening uses most |
| `great_works` | Its next recipe buildings go inside the yard. Nothing standing is moved |
| `northern_city` | Keeps 40 ice in stores with Cut Ice whenever a bender has the chi |
| `southern_ways` | One more Waystation at the second-farthest deposit, and 4 shore Fish orders beside each |
| `high_temple` | The temple against the rock nearest the study halls |
| `nomad_way` | Each spring, if most farm plots sit on strained ground, takes the farm cluster down and rebuilds it on the nearest fresh land of the same size |

**What the run reports**, on the panel's 32 seeds, against `steady` from `main` before the first
phase-4 merge:

| Measure | Metric | Expected |
|---|---|---|
| Both options viable | the `[[fork]]` check, per nation, on Normal; Air and Earth again on Hard | Each side wins some seeds. A side that wins more than 75% fails the check |
| The school arrives in time | a new `school_minute` | At or before minute 200 on 85% of runs with either school style |
| Each school does something | a tally per school: `sheltered_tiles`, `rail_powered`, `aura_heal`, `automated_batches`, `ice_paid`, `shore_fish`, `temple_study`, `packed_refund` | Above zero on 85% of that school's runs |
| Items compete | `sign_minute.ore`, `stage_minute.3` under `steady` | The control moves by no more than its noise floor. Schools change only the styles that pick them |
| Decision timeline | `longest_gap`, the `timeline:` line | The fork and the bill add one decision in the 150-to-gate stretch |
| No regressions | every existing target, through `balance verdict --against` | `steady` is unchanged except where a school is reached by file order |

The balance follow-up records the measured numbers in a "Measured" section here. The first
levers per pair, all data: Earth, `[enclosure] winter_growth` and the Metal Rail's metal per
tile. Fire, the campfire `aura` and `automate.rate`. Water, the shore fishing `work` and the
Waystation `shelter`. Air, the temple's `study` and `packed_work`.

## Risks

- **Nomad Way is hard to script.** The opening's move is crude, and a person who plans moves
  around winters will do much better. If the `[[fork]]` check fails for Air on both presets,
  the first step is a playtest, not a nerf to the temple. If the check fails there too, the
  lever is `packed_work` and the bison's site deliveries.
- **The school lands after the gate.** If `school_minute` is past 200, schools decorate the
  United City rather than the road to it. The lever is `cost_items_from` and the school costs,
  not the techs below.
- **Scroll starvation.** Three scrolls for a school, three for the gate and three read for the
  Archive make nine. Maps hold 16 to 24 in 8 to 12 ruins, all buried, so it's a race, not a lock.
  The confirm says scrolls go to the school, and #715 is the owner's call.
- **#448 or #433 lands late.** `metal_city`'s mill upgrade needs #448's Hot Spring Mill. Until it
  lands, metal rail has nothing to carry, so that follow-up waits on it. `sun_warriors` ships
  without the Steam Engine line if #433 hasn't landed, and gains it later.
- **Enclosure flood cost.** One full-map flood per wall change. Earth raises walls a tile at a
  time, so dragging a 68-tile ring runs 68 floods as the walls finish one by one, spread over
  minutes of build time. If it shows in a profile, rebuild once a tick at most.
- **Automation versus the stock limits.** An automated yard idles at the same `stock_limit` as a
  crafted one, so Fire's tools and bricks don't run away. The harness's wood and stone stores at
  180 watch it.

## Open questions for the owner

1. **Do scrolls given to a school count as read?** *Recommended: no. They go on the bill, give no
   knowledge, and don't count toward the scrolls sign or phase 3's `scrolls_read` condition.* The
   spec's question for the road is "do scrolls go to my school or to the gate?", and this makes it
   a real one. Maps hold enough scrolls that it delays and never locks. The alternative is that a
   school scroll counts as read but gives no knowledge, which makes the scroll cost almost free:
   just a reason to search three more ruins. A third option, scroll copies from phase 3's Archive,
   would tie three schools to a non-core building that itself needs 3 scrolls read. Section:
   "Shared sim work, 2. Item bills". (#715)
2. **May mountains and water close an Agrarian Ring?** *Recommended: no, walls and Ring Gates
   only.* The spec says "fully enclosed by Earth Wall", and the decision it names is stone per
   tile. Letting rock close a ring would make a valley site shelter its fields for a dozen wall
   tiles, which is very Earth Kingdom, but the school's strength would then swing with the map far
   more than its counterpart's. It is one line of data either way (`[enclosure]` would gain
   `terrains = ["mountain"]`). Section: "`agrarian_ring`: sheltered rings". (#716)

## Follow-up issues

Each one appends its own tech, school, building and table blocks from this doc. `TechId` and
`BuildingId` are file positions, and saves remap them by name (save-compat.md), so the order
between follow-ups doesn't matter.

| Issue | Title | Blocked by |
|---|---|---|
| #717 | Sim and data: schools (`[[school]]`, the `school` field, validation, `ResearchError::Closed`) | #673 |
| #718 | Sim and data: research item bills (`[[tech.cost_items]]`, deliveries to a study hall, refunds, scrolls on the bill) | #717, #715 |
| #721 | Sim and data: enclosure detection, the Ring Gate and `agrarian_ring` | #717, #718, #677, #716 |
| #722 | Sim and data: Metal Rail (power along a run, `also_on`, the `boost` upgrade) and `metal_city` | #717, #718, #674, #676, #448 |
| #723 | Sim and data: auras and the High Temple (`[building.aura]`, `landmark`) and `high_temple` | #717, #718 |
| #724 | Sim and data: the `aura` and `fuel_free` upgrades and `sun_warriors` | #717, #718, #723, #674, #675, #680 |
| #725 | Sim and data: the Factory (`automate` upgrade, Supply hauls, `SetAutomated`) and `great_works` | #717, #718, #674, #675, #428 |
| #726 | Sim and data: item upgrades (`stands_for`) and `northern_city` | #717, #718, #674 |
| #727 | Sim and data: shore fishing, the `free` and `shelter` upgrades, and `southern_ways` | #717, #718, #674, #677, #694 |
| #728 | Sim and data: pack up (packed refunds, beasts haul to sites) and `nomad_way` | #717, #718, #677, #693 |
| #729 | TUI: the school fork on the Research screen, the confirm, the Closed state and bill progress | #717, #718, #652 |
| #730 | TUI: school readouts (sheltered tint, rail power, Factory toggle, auras, shore fishing, pack up) | #721, #722, #723, #724, #725, #726, #727, #728 |
| #731 | Balance: school styles, the `[[fork]]` check, `stage_minute.N`, `school_minute`, and the phase-4 run | #721, #722, #723, #724, #725, #726, #727, #728, #713 |
| #715 | Decision: do scrolls given to a school count as read? | none |
| #716 | Decision: may mountains and water close an Agrarian Ring? | none |
