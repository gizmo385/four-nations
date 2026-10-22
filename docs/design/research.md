# Research and the tech tree

Status: **proposed** (#17). Follow-up work: #18 to #28 (see the end of this doc).

Research is how a settlement grows past where it starts. It unlocks buildings, jobs and bending
abilities, and raises nation modifiers. It rewards peaceful growth: people who study instead of
hauling, exploring the map, and later, learning from neighbors (vision.md: one nation, no war).
There are no weapon or military techs.

## Decisions in brief

1. **One currency, one pool.** Research produces *knowledge*, an integer pool shared by the whole
   settlement. The player picks a target, and the sim spends from the pool when the target's cost
   is covered. Switching targets costs nothing, and knowledge banks while nothing is selected.
2. **Labor is the main cost.** Most knowledge comes from units doing a low-priority **Study** job
   at a study hall, so research competes with food and building for hands. A small passive
   trickle bootstraps the very first techs.
3. **Tree shape.** A shared civilization tier (11 techs) that every nation can research, plus one
   six-tech branch per nation that expresses its bending and culture. A nation never sees other
   nations' branches.
4. **Nations start differently.** Each nation starts knowing one tech: Fire knows `fire_making`,
   Water knows `fishing`, Earth knows `stoneworking`, Air knows `orchards`. Fire is *not* a
   given for the others. A Water settlement eats raw fish until it researches fire.
5. **Everything is data.** `data/research.toml` defines techs, costs, prereqs, nation
   restrictions and unlocks. The sim validates it strictly at load, like `defs.rs` does today.
6. **Modifiers stack multiplicatively**: effective value = nation base × each known tech's
   multiplier. Products are taken in tech-definition order, so the result never depends on
   research order.

## Producing knowledge

| Source | Rate (data, `[settings]`) | When |
|---|---|---|
| Passive trickle ("oral tradition") | 3 / min for the settlement | From tick 0. Bootstraps tier 0 and keeps a starving colony from stalling completely |
| **Study job** at a study hall | 9 / min per scholar × `study` modifier | Main source. Needs #4 (jobs); one seat per scholar |
| Ancient scrolls | +40 each, one-off | Rare map resource, hauled to the study hall (rewards exploration) |
| Neighbors (later) | Discounts, not points | See "Later" below |

- **Who studies.** Any unit can take a Study job. It's low priority, so needs (#3) and urgent
  work pre-empt it. That trade-off is the core research decision. Research costs time and
  labor, not materials, which keeps it readable. Skills and "elder" traits can raise output
  later.
- **Study hall.** Available from the start, with no tech. It has two seats, and each nation has
  its own name in data: Meditation Pavilion (Air), Elders' Lodge (Water), Scroll Library
  (Earth), Academy (Fire). `writing` adds seats.
- **Units.** Knowledge is stored as integer milli-points (`u64`), and data rates are converted
  once at load to millis per tick. At `TICKS_PER_SECOND = 10`, 3/min is 5 millis/tick and
  9/min is 15 millis/tick. Integer math keeps it deterministic and save-friendly.

### Pacing targets (first hour, one nation)

Assumptions: the first study hall is up by about minute 5 with one scholar, there are two
scholars by minute 15 and three by minute 40, and each scholar is about 2/3 effective because of
needs.

| Time | Knowledge earned | What the player has |
|---|---|---|
| ~6 min | ~25 | First tech (one of the four 25-cost basics) |
| ~20 min | ~180 | All of tier 0, plus a first 60-cost tech (farming or carpentry) |
| ~35 min | ~400 | First nation tech; masonry or herbalism under way |
| 60 min | ~1000 | About 10 techs, including `writing` (study ×1.25) |
| 2–2.5 h | ~2200 | The whole starter tree for that nation |

Cost tiers are 25 / 60 / 120 / 250 / 500. Balance by tuning the two rates in `[settings]`
first, and individual costs second.

## The starter tree

`Unlocks` names buildings, jobs and abilities that don't exist yet. They become real data as
#4, #6 and their follow-ups land (see "Unlocks" below for how that's staged). Multipliers are
written as `field ×N`.

### Shared civilization tier (11)

| id | Cost | Prereqs | Unlocks |
|---|---|---|---|
| `fire_making` | 25 | none | campfire; cook job (cooked food fills more hunger, #3). *Fire starts with it* |
| `shelter` | 25 | none | hut (better sleep, #3) |
| `fishing` | 25 | none | fishing-spot designation, fish job. *Water starts with it* |
| `stoneworking` | 25 | none | quarry designation on stone. *Earth starts with it* |
| `farming` | 60 | none | farm plot; sow and harvest jobs |
| `pottery` | 60 | fire_making | kiln; pots (water storage for thirst); granary |
| `carpentry` | 60 | shelter | workbench; wooden buildings; construction ×1.1 |
| `masonry` | 120 | stoneworking, carpentry | stone buildings; construction ×1.15 |
| `herbalism` | 120 | farming | herb garden; herbalist job (restores health, #3) |
| `writing` | 120 | pottery | +2 study hall seats; study ×1.25 |
| `cartography` | 250 | writing | reveals the region around the settlement; scrolls shown on the map |

### Earth Kingdom: shaping the land

| id | Cost | Prereqs | Unlocks |
|---|---|---|---|
| `earth_shaping` | 60 | stoneworking | level/raise-terrain designation; earthworks ×1.2 |
| `earthen_walls` | 120 | earth_shaping, masonry | earth wall, flood barrier, terraces; construction ×1.25 |
| `tunneling` | 120 | earth_shaping | tunnel designation through mountain tiles; mining inside mountains |
| `earth_rail` | 250 | earthen_walls | Ba Sing Se–style rail for haul routes; carry_capacity ×1.25 |
| `seismic_sense` | 250 | tunneling | ability: reveal ore and gems in a radius |
| `metalbending` | 500 | seismic_sense, masonry | metal from ore without a smelter; ability: metalbend |

### Fire Nation: heat and industry

| id | Cost | Prereqs | Unlocks |
|---|---|---|---|
| `forging` | 60 | fire_making | forge; smith job (tools) |
| `brickmaking` | 120 | forging, pottery | brick kiln; construction ×1.15 |
| `smelting` | 120 | forging | smelter (ore to metal); heating ×1.2 |
| `dancing_dragon` | 250 | smelting | Sun Warrior teachings: heating ×1.25; ability: inner fire (warmth aura) |
| `steam_engines` | 250 | smelting, brickmaking | steam engine powers mills and pumps; power_generation ×1.25 |
| `lightning_generation` | 500 | dancing_dragon, steam_engines | lightning power station (Republic City style); power_generation ×1.5 |

Lightning is a *power source*, not an attack. Redirection can stay flavor text.

### Water Tribes: flow and healing

| id | Cost | Prereqs | Unlocks |
|---|---|---|---|
| `irrigation` | 60 | farming | irrigation channel (crops near it grow faster); water_work ×1.15 |
| `ice_construction` | 120 | shelter | ice blocks as a cheap building material; igloo |
| `healing` | 120 | herbalism | healing hut; healer job; ability: healing water |
| `tide_mills` | 250 | irrigation | push-and-pull tide mill on the coast; power_generation ×1.25 |
| `water_travel` | 250 | fishing, ice_construction | canoes and ice skiffs; units can cross water tiles |
| `spirit_oasis` | 500 | healing, tide_mills | spirit oasis (chi and mood regeneration); water_work ×1.25 |

No bloodbending. It's a dark art that doesn't fit peaceful progress.

### Air Nomads: lightness and spirit

| id | Cost | Prereqs | Unlocks |
|---|---|---|---|
| `orchards` | 25 | none | orchard (plant and tend fruit trees: renewable fruit, edible raw); fruit-picking job. *Air starts with it* |
| `meditation` | 60 | none | chi regeneration; study ×1.2 |
| `gliders` | 60 | carpentry | glider staffs; move_speed ×1.15; scouting reveals the map |
| `windmills` | 120 | gliders, farming | windmill (grinds grain for more food); power_generation ×1.2 |
| `air_scooter` | 250 | gliders, meditation | ability: air scooter; move_speed ×1.2 |
| `wind_turbines` | 250 | windmills | wind turbine powers workshops; power_generation ×1.25 |
| `sky_bison` | 500 | meditation, air_scooter | bison stable; bison haul and fast travel; carry_capacity ×1.3 |

Air starts behind on labor (carry 0.8, earthworks 0.7), so its starting tech secures food
instead. The Air Temples' orchards fit the nomads' vegetarian life and give a steady supply of
food that needs no fire. `meditation` is an early research choice, not a starting tech. The
study hall exists from the start anyway (named Meditation Pavilion for Air), so meditation
doesn't unlock a building.

Speed check: Air's base `move_speed` is 1.6, and `gliders` (×1.15) and `air_scooter` (×1.2)
bring it to about 2.2× baseline. Revisit in a balance pass.

## Data schema: `data/research.toml`

```toml
[settings]
passive_knowledge_per_minute = 3.0
study_knowledge_per_minute = 9.0     # per scholar, before the `study` modifier

# File order must be topological: prereqs appear before the techs that need them.
[[tech]]
id = "fire_making"
name = "Fire Making"
description = "Keep a flame alive: cook food and stay warm."
cost = 25                            # knowledge points, > 0
prereqs = []                         # tech ids
# nation omitted = shared tier
[tech.unlocks]
buildings = ["campfire"]
jobs = ["cook"]

[[tech]]
id = "earthen_walls"
name = "Earthen Walls"
description = "Raise walls and flood barriers straight from the ground."
nation = "earth"                     # only Earth Kingdom can see or research this
cost = 120
prereqs = ["earth_shaping", "masonry"]
[tech.unlocks]
buildings = ["earth_wall", "flood_barrier", "terrace"]
abilities = ["raise_wall"]
modifiers = { construction = 1.25 }  # multiplier on the effective modifier
```

And in `data/nations.toml`, per nation:

```toml
starting_techs = ["fire_making"]     # must be shared or this nation's; prereqs must also start
[nation.modifiers]
study = 1.0                          # new modifier: research speed
```

`unlocks` has four optional lists: `buildings`, `jobs`, `abilities` and `modifiers`. Every
unlock kind is optional, and unknown keys are rejected.

## Representation in `fn_sim`

This follows `defs.rs`: TOML structs with string ids are resolved once into index newtypes and
validated at load.

```rust
pub struct TechId(pub u16);

pub struct TechDef {
    pub id: String,
    pub name: String,
    pub description: String,
    pub nation: Option<Nation>,      // None = shared
    pub cost: u32,                   // knowledge points
    pub prereqs: Vec<TechId>,        // all < self (topological order)
    pub unlocks: Unlocks,
}

pub struct Unlocks {
    pub modifiers: ModifierMultipliers,   // Option<f32> per Modifiers field
    // Added by the issue that introduces each def kind (#22, #25..#28):
    pub buildings: Vec<BuildingId>,
    pub jobs: Vec<JobKindId>,
    pub abilities: Vec<AbilityId>,
}

pub struct Research {                // lives in World
    known: Vec<bool>,                // indexed by TechId
    pool_millis: u64,
    queue: Vec<TechId>,
    effective: Modifiers,            // cached, recomputed on completion
}
```

**Validation at load** (`DefsError` on failure, with a unit test for each rule):
- unique ids; every prereq exists and appears **earlier in the file** (this makes cycles
  impossible and makes `TechId` order a valid topological order);
- a shared tech may only require shared techs; a nation tech may require shared techs or techs
  of its own nation;
- `cost > 0`; each multiplier is finite and in `(0, 4]`; `ModifierMultipliers` uses
  `#[serde(deny_unknown_fields)]`, so `contstruction = 1.2` fails to parse;
- every building, job or ability id resolves, and each is unlocked by **at most one** tech;
  abilities may only be unlocked by a tech of the same nation as the ability;
- `starting_techs` are shared or the nation's own, and closed under prereqs.

**Staging.** #18 lands the schema with `modifiers` only. Each other unlock kind is added to
`Unlocks`, with its validation, in the same change that introduces that def kind. Then
`research.toml` never contains ids that can't be checked. Until then, the tree tables above are
the record of what each tech will unlock.

**Per tick.** `pool += rate`; while the queue's front tech costs no more than the pool, subtract
the cost, mark it known, recompute `effective`, and emit a completion event. Choosing a target
replaces the queue with the target plus its unknown prereqs, sorted by `TechId`.

**Locking rule.** An item is locked for a nation if some tech unlocks it and the nation doesn't
know that tech. Items no tech mentions are available from the start, so content that hasn't
been placed in the tree just works.

## Modifier stacking

```
effective.construction = nation.construction × Π (known techs' construction multipliers, in TechId order)
```

- Multiply only, never add. That keeps stacking order-free in principle and easy to explain.
- Iterate in `TechId` order because `f32` multiplication isn't associative. Two different
  research orders that reach the same known set must give *bitwise-identical* modifiers (a test
  in #20).
- Systems read `world.modifiers()`, never `NationDef::modifiers` directly (#20 migrates
  `move_speed`).
- Readable simulation: `modifier_breakdown("construction")` returns
  `[("Earth Kingdom", 1.5), ("Carpentry", 1.1), ("Masonry", 1.15), ("Earthen Walls", 1.25)]`
  for hover tooltips.
- A new modifier field (e.g. `study` now, `food_value` with #3) is added to `Modifiers`,
  `nations.toml` and `ModifierMultipliers` together.

## How it fits the other systems

- **Jobs (#4).** Study is a job kind that adds `study` to the pool, and other jobs gain
  research-unlocked kinds (cook, smith, healer). Work speed already multiplies by a modifier, and
  techs raise those modifiers through #20.
- **Needs (#3).** Techs help needs *indirectly*, through what they unlock: huts (sleep),
  cooking (hunger), pots (thirst), herbalism and healing (health). Needs also make research
  costly: hungry scholars leave their desks. No tech directly changes need rates for now.
- **Player commands (#5).** `World::set_research(tech)` is added by #19 and becomes
  `Command::SetResearch { tech }` when #5's `Command` enum lands, so research choices replay
  deterministically. Locked items are rejected by the sim with a reason ("Requires Pottery")
  that the client shows (#22).
- **Bending (#6).** Each nation's signature ability from #6 is available **from the start**,
  not researched, so nations play differently from minute one. Branch techs unlock the
  *second and later* abilities (seismic sense, metalbend, healing water, air scooter, inner
  fire) and raise the modifiers those abilities scale with. All ability code stays in the one
  obvious place #6 defines. Research only toggles availability.

## Client: minimum UI (#23)

- **HUD line:** current target, a progress bar (pool / cost), and knowledge per minute.
- **Research screen** (`R`), plain `bevy_ui` like the existing panels: the shared tier plus the
  player's own branch, in columns by depth. Each tech shows name, cost, state
  (known / available / queued / needs prereqs) and its unlocks. Clicking a tech sets the target.
- **Completion toast.** A graphical node-and-edge tree, icons and modifier tooltips come later.

## Later: neighbors and cultural exchange

When peaceful neighbor civilizations exist, knowledge flows through **exchange**, not points.
For example, a shared tech a friendly neighbor already knows costs 50% less, and a neighbor can
teach its *non-bending* branch techs (e.g. Fire's `brickmaking`, Air's `windmills`). This needs
a per-tech `teachable` flag, which isn't added until neighbors exist. It's the main way the
tree grows beyond one nation's identity.

## Owner decisions (2026-10-22)

1. **Starting without fire: accepted, on one condition.** Nobody may starve because their
   nation lacks fire. Raw food (fish, fruit, foraged plants) must fully sustain a starting
   settlement. Cooking is a bonus (more hunger filled per item), not a requirement. This is a
   hard requirement on the needs work (#3).
2. **Cross-nation learning: deferred.** Don't design or build `teachable` techs yet. The
   neighbors section above stays a sketch.
3. **Era ceiling: Korra-era is in tone.** Steam engines and lightning power stations stay.
4. **Scrolls: knowledge points first.** #24 implements points only. Keep the data shape open
   (e.g. an optional `reveals` field later), so scrolls can unlock hidden techs without a
   schema break.

5. **Air's starting tech is `orchards`, not `meditation`** (owner's choice). Meditation alone gave
   only a research bonus, while every other nation's start helps it survive. Orchards give the
   nomads renewable food without fire, and meditation becomes a 60-cost Air tech.

## Follow-up issues

| # | Work | Area | Priority | Blocked by |
|---|---|---|---|---|
| #18 | `research.toml` schema, `study` modifier, `starting_techs`, loading and validation | sim, data | p1 | none |
| #19 | Knowledge pool, queue, passive trickle, completion, `set_research` | sim | p1 | #18 |
| #20 | Effective modifiers from known techs, plus breakdown API | sim, data | p2 | #18, #19 |
| #21 | Study job and study hall | sim, data | p2 | #4, #19 |
| #22 | Gate buildings and jobs on known techs | sim, data | p2 | #4, #19 |
| #23 | Client research screen and HUD progress | client | p2 | #19 |
| #24 | Ancient scrolls grant knowledge | sim, data | p3 | #4, #21 |
| #25–#28 | Content pass: wire Earth / Fire / Water / Air branch unlocks (adds ability unlocks) | data, design | p3 | #6, #22 |
