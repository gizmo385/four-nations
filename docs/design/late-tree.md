# The late tree: make every tier-3 and tier-4 unlock change play

Status: **proposed** (#413, from the owner's 2026-11-06 playtest). Owner decisions are listed at
the end under "Open questions for the owner", and the follow-up issues under "Follow-up issues".

The owner's note was that "the midgame and end game felt like they really lagged". #412 deals
with pacing: when new decisions turn up, growth and surpluses. This doc covers the content
side. Much of the back half of the research tree unlocks something the player can't see. Five
buildings stand inert, six techs raise a modifier that nothing reads, and Water's and Air's
500-point capstones pay the least. Finishing the branch is a sign on the way to the gate, so
the slowest stretch of research ends in the least reward.

## Decisions in brief

1. **Power is a reach, not a grid.** A power source has a `radius` (scaled by the nation's
   `power_generation`, the way `heating` scales a campfire's) and a `boost`. Work done inside
   any building that can use power and stands in reach runs `boost` times faster. Two sources
   in reach of one building don't stack, as with irrigation. There's no wiring, load or
   capacity.
2. **Power drives work inside buildings**: recipe batches (Workshop, Forge, Brick Kiln, Smelter,
   both Earth mines), study seats, and the Map Room's charting (#402). Those are the colony's
   labour sinks. A crafter or scholar who finishes sooner goes back to hauling and building, so
   power buys back hands while population is still a fixed clock (#412).
3. **Five sources, three nations**: Windmill and Wind Turbine (Air), Tide Mill (Water, on the
   water's edge), Steam Engine and Lightning Station (Fire). Earth gets none. It has rail and
   two mines instead, and its `power_generation` of 0.8 is the vision's "weak at this".
4. **Empty shared techs get a job.** `shelter` becomes a starting tech for every nation and
   unlocks the Bed, which is what it always described. `pottery` unlocks a **Cistern**, a fresh
   drinking place away from the river.
5. **The capstones come first.** Fire's two machines and Air's two wind buildings are data once
   power exists, and they go first. The Spirit Oasis (#202) is unblocked from #358 so that it
   can land with placeholder numbers, and the Bison Stable (#205) moves up to p2.

## The audit: every tech costing 120 or more

"Changes play" means whether the player sees something new and makes a new decision. A
modifier alone does neither. The **Proposal** column is empty where the answer is already yes.

| Tech | Nation, cost | Unlocks today | Changes play today? | Proposal |
|---|---|---|---|---|
| `masonry` | shared, 120 | construction ×1.15 | No: builds go a bit faster | Leave as a modifier. It's a gate to `earthen_walls` and `metalbending`, and a gate tech is fine at 120 |
| `herbalism` | shared, 120 | nothing | No | Herb Garden and herbs as a remedy (#390, filed) |
| `writing` | shared, 120 | study ×1.25 | No | Leave as a modifier. Its "+2 seats" note is a pacing lever for #412, not content |
| `prospecting` | shared, 250 | gold vein and gold seam mining | **Yes**: ore becomes minable | |
| `cartography` | shared, 250 | nothing | No | Map Room charts lost ruins (cartography.md, #402). Power speeds up charting too |
| `earthen_walls` | Earth, 120 | Earth Wall, construction ×1.25 | Yes | |
| `tunneling` | Earth, 120 | Bore Tunnel, Mine Shaft | Yes | |
| `earth_rail` | Earth, 250 | Rail, carry ×1.25 | Yes | |
| `seismic_sense` | Earth, 250 | Seismic Sense (finds buried gold) | Yes | |
| `metalbending` | Earth, 500 | Metalbend, Deep Mine | Partly: the Deep Mine can't be paid for yet | Metalbend works stockpiled ore (#398, filed) |
| `brickmaking` | Fire, 120 | Brick Kiln, construction ×1.15 | Yes | |
| `smelting` | Fire, 120 | Smelter, heating ×1.2 | Yes | |
| `dancing_dragon` | Fire, 250 | Inner Fire, heating ×1.25 | Yes | |
| `steam_engines` | Fire, 250 | Steam Engine (**inert**), power ×1.25 | **No** | The Steam Engine becomes a power source (#255) |
| `lightning_generation` | Fire, 500 | Lightning Station (**inert**), power ×1.5 | **No** | The Lightning Station becomes the strongest source (#255) |
| `ice_construction` | Water, 120 | Igloo, Cut Ice | Yes | |
| `healing` | Water, 120 | Healing Hut | Yes | |
| `tide_mills` | Water, 250 | power ×1.25 **only** | **No** | Unlocks the Tide Mill, a power source on the water's edge (#200) |
| `water_travel` | Water, 250 | Fishing Dock | Yes | |
| `spirit_oasis` | Water, 500 | water_work ×1.25 **only** | **No** | Unlocks the Spirit Oasis, a chi aura (#202, unblocked) |
| `windmills` | Air, 120 | Windmill (**inert**), Lift, power ×1.2 | Partly: Lift only | The Windmill becomes a small power source (#204, question 1) |
| `air_scooter` | Air, 250 | Air Scooter, move ×1.2 | Yes | |
| `wind_turbines` | Air, 250 | Wind Turbine (**inert**), power ×1.25 | **No** | The Wind Turbine becomes a power source (#204) |
| `sky_bison` | Air, 500 | Bison Stable (**inert**), carry ×1.3 | **No** | Bison haul (#205, moved up to p2). Flight later (#254) |

All seven tech rows marked **No** at 250 or 500 have a follow-up. With power in, every
`power_generation` multiplier gets a visible reader: each one widens the reach of the nation's
sources. (The issue says no system reads `power_generation`. One does: Wind Carry's range, in
world/bending.rs. That's Air's bend, though, and the other nations' power techs still do
nothing.)

## Power

### The rule

A finished building with a `[building.power]` section is a **source**. A finished building with
`uses_power = true` is a **consumer**. A consumer is **powered** when any tile of its footprint
is within a source's reach. Reach is a Chebyshev radius (a square) measured from every tile of
the source's footprint, the same shape as heat and irrigation. The reach is

    reach = max(1, round(radius × power_generation))

and it's re-derived whenever research changes the modifier, the way `refresh_heat_radii` works
for campfires. A powered consumer's work runs at `boost` times its rate: a recipe batch's
progress, or a study seat's knowledge. If several sources reach it, the best `boost` applies,
and they don't stack.

```toml
# data/buildings.toml, on a source
[building.power]
radius = 3      # tiles at power_generation 1.0
boost = 1.5     # work inside powered buildings runs this many times as fast

# on a consumer
uses_power = true
```

Consumers: `workshop`, `forge`, `brick_kiln`, `smelter`, `mine_shaft`, `deep_mine`,
`study_hall`, and the Map Room when #402 lands. A source doesn't need a worker and burns no fuel
in v1 (question 4 is about fuel for the Steam Engine).

**Why a reach.** It's the smallest rule that makes placement matter. A source covers a patch,
so the player builds the Workshop and the study hall together under a turbine, or moves the
study hall to the coast for a tide mill. Every tech that raises `power_generation` visibly grows
that patch. The heat and irrigation code already answers "best source in reach" and draws a
reach tint, so this is the third reader of a pattern the code already has. **Not a grid**
(#255's original framing): wires or pylons would need a new tile layer, connectivity and a
load-balancing rule, all to answer a question the reach answers well enough. **Not a pool**:
settlement-wide power would make placement meaningless, and the sources would be another
invisible modifier.

**Why no load or capacity.** Load would make the player count consumers per source. With at
most about eight consumers in a whole colony, that arithmetic isn't worth it. A second source
earns its keep by covering a second cluster, not by adding watts.

### The sources

Radius is at `power_generation` 1.0. The reach columns use each nation's modifier
(data/nations.toml) times its techs (data/research.toml) at the moment the building unlocks,
and once the branch is done.

| Source | Nation, tech | Footprint | Radius | Boost | Reach on unlock | Reach, branch done | Placement |
|---|---|---|---|---|---|---|---|
| **Windmill** | Air, `windmills` (120) | 2x2 | 2 | 1.25 | 5 (2.0 × 1.2) | 6 | anywhere |
| **Wind Turbine** | Air, `wind_turbines` (250) | 1x1 | 3 | 1.5 | 9 (2.0 × 1.2 × 1.25) | 9 | anywhere |
| **Tide Mill** | Water, `tide_mills` (250) | 2x2 | 4 | 1.5 | 5 (1.0 × 1.25) | 5 | must touch water (`beside`, #373) |
| **Steam Engine** | Fire, `steam_engines` (250) | 2x2 | 3 | 1.5 | 5 (1.3 × 1.25) | 7 | anywhere |
| **Lightning Station** | Fire, `lightning_generation` (500) | 2x2 | 4 | 2.0 | 10 (1.3 × 1.25 × 1.5) | 10 | anywhere |

How each one plays:

- **Air.** The Windmill is early, cheap and small. It covers the pavilion and the Workshop if
  they stand together. The Turbine is a 1x1 that reaches 9 tiles, so one of them covers a whole
  core: Air is the nation that's good at power, as the vision says. Air has the fewest recipe
  buildings, so its power mostly goes into study, which suits the monks.
- **Water.** The Tide Mill is the one source with a placement rule, so Water lays out a
  waterfront quarter where the Elders' Lodge and the Workshop share the bank with the Fishing
  Dock. The mill's own reach is modest because Water's `power_generation` is 1.0.
- **Fire.** Fire has the most consumers (forge, kiln, smelter, Workshop, Academy), so its power
  matters most. The Steam Engine covers the industrial yard. The Lightning Station is the one
  source with a `boost` of 2.0: it doubles everything around it, and the 500-point capstone is
  the strongest building Fire owns.
- **Earth** builds no source. Its shared consumers just run unpowered, and nothing in the UI
  suggests otherwise, because no Earth tech names power.

The radius and boost values are starting points for the balance follow-up (#432) to check. The
four existing buildings keep the costs #369 and #376 gave them. The Tide Mill is new: 14 wood,
6 stone and 2 tools, 100 s of work. That puts it between the Fishing Dock and the Healing Hut,
and like every new building it is appended to buildings.toml.

### What it's worth

- **Study.** A powered seat makes 9 × 1.5 = 13.5 knowledge a minute before `study`. With two
  seats powered by a Turbine, Air's 500-point `sky_bison` takes about a third less scholar time.
  That's the main way power shortens the long research wait between 250 and 500, and it comes
  from content, not from a pacing knob (#412 owns those).
- **Recipes.** A Workshop tool takes 40 s, or 27 s when powered. Fire's kiln and smelter under
  the Lightning Station run at double speed. Recipes idle at their `stock_limit` (#350), so the
  gain is crafter time handed back to the colony, not a bigger pile.
- **Study code** counts knowledge in integer millis per tick (research.rs). The boost must stay
  in integer maths there, as `boost × 1000` rounded at load, so the sim stays deterministic
  across platforms.

### The Windmill (question 1)

#203 has the Windmill grinding grain into more food. That needs a grain crop, a grain item, a
milling recipe and a food entry, and it delivers food, which #412 measured as a glut (37 per
head on seed 42 Fire). **Recommended: the Windmill is Air's first, small power source**, and
#203's grain is parked. The tech's description changes to say so, which #409's voice pass can
pick up. A powered windmill still reads as a mill, just one that turns the Workshop's lathe and
the pavilion's prayer wheels rather than a millstone.

## Empty shared techs

### `shelter`: a starting tech that unlocks the Bed

`shelter` (25) describes the Bed ("Raise simple huts so people sleep dry and warm"), but the Bed
needs no tech. So `shelter` gives nothing except the prereq for `carpentry` and
`ice_construction`.

**Recommended (question 2):** add `shelter` to every nation's `starting_techs` and give it
`unlocks.buildings = ["bed"]`. Play doesn't change, since the Bed is available from tick 0 as
before. What changes is that the Research screen shows Shelter as known and naming the Bed, so
it never asks for research that yields nothing. Removing the tech outright would change every
later `TechId` (file position), which `Defs::id_hash` would reject for every existing save.

Two alternatives, both worse:

- Gating the Bed behind 25 points would delay the first beds, and arrivals need a free bed.
  That slows the opening #412 is trying to make busier.
- A new "Hut" sleeping building would duplicate the Bed and the Igloo, and a bed must be 1x1.

### `pottery`: the Cistern

`pottery` (60) says "pots for storing water and grain". Since harm landed, water is the one
thing a settlement can't always store or reach well. A coastal site without a river drinks sea
water and falls sick (harm.md, "Sickness: sea water"), and a quarry far from any water costs a
long walk every 8 minutes or so.

**Recommended (question 3):** `pottery` unlocks the **Cistern**: 1x1, solid, 6 stone, 30 s of
work. It's a fresh drinking place: `plan_drink` treats a tile beside a Cistern exactly like a
tile beside a river. It never runs dry in v1. That's a real choice for a coast or desert site,
and a reason to put one by a far quarry or the gate's site.

```toml
[[building]]
id = "cistern"
name = "Cistern"
verb = "Build"
work = 30.0
passable = false
[building.drink]
fresh = true    # counts as fresh water for harm's sickness rule
[[building.cost]]
item = "stone"
count = 6
```

A granary (more food per stockpile tile) is the alternative. It's only worth it once food gets
scarcer or spoils, which is #353's and #412's territory. If #412 adds spoilage, the granary can
go under `pottery` too, next to the Cistern.

## The capstones

- **Spirit Oasis (#202, Water 500).** The spec is in nation-buildings.md: 2x2, 20 stone and 2
  tools, chi regenerates 2x as fast within 4 tiles. It's held on #358 only for its numbers.
  **Recommended: unblock it** and land it with those placeholder numbers in data, and #358
  retunes a line of TOML later. Water's capstone currently has the least effect of any tech in
  the tree, and the chi aura is the same shape as heat.
- **Bison Stable (#205, Air 500).** The owner has already scoped v1 to hauling only (2026-10-25).
  **Raise #205 to p2.** One bison per stable, a hauling-only unit with no needs in v1. It carries
  `base_carry` × 4 at the nation's `move_speed`, and only takes haul jobs whose stack is at
  least 10 tiles from a stockpile. Those are the late-game trips to far stone and ore, the gate's
  bottleneck. Flight (#254) stays p3 and comes after.
- **Lightning Station** is covered above: the only source with a boost of 2.0.
- **Metalbending** is covered by #398, already decided and filed.

## UI

Power has to be readable (pillar 4). That takes three surfaces. All of them are text and tint,
and none of them is a new screen.

**Build modal.** A source's detail lists its reach, already scaled for the nation's current
`power_generation`. A consumer gains one line, "Uses power". Today the Wind Turbine reads
"Nothing beyond shelter" (`fn_tui 7 air --frame --ascii --size 100x26 --keys 'U <down*5>'`,
cropped to the modal):

```
+------------------------- * BUILD * --------------------------+
|   Bed             5 Wood  Wind Turbine                       |
|   Campfire        Locked  Footprint: 1x1                     |
|   Meditati~ 10 Wood, 2x2                                     |
|   Orchard    8 Wood, 2x2  12 Wood (0 in stores)              |
|   Windmill        Locked  8 Stone (0 in stores)              |
| > Wind Turbine    Locked  2 Tool (0 in stores)               |
|   Bison Stable    Locked                                     |
|   Farm Plot       Locked  Nothing beyond shelter             |
|   Workshop        Locked                                     |
|                           Requires Wind Turbines             |
+- Arrows/W S choose  Enter build  Esc close ------------------+
```

Proposed (same frame, with the detail pane edited):

```
+------------------------- * BUILD * --------------------------+
|   Bed             5 Wood  Wind Turbine                       |
|   Campfire        Locked  Footprint: 1x1                     |
|   Meditati~ 10 Wood, 2x2                                     |
|   Orchard    8 Wood, 2x2  12 Wood (0 in stores)              |
|   Windmill        Locked  8 Stone (0 in stores)              |
| > Wind Turbine    Locked  2 Tool (0 in stores)               |
|   Bison Stable    Locked                                     |
|   Farm Plot       Locked  Powers buildings within 9 tiles    |
|   Workshop        Locked  Powered work: 1.5x faster          |
|                                                              |
|                           Requires Wind Turbines             |
+- Arrows/W S choose  Enter build  Esc close ------------------+
```

And a consumer, Fire's Forge (from `fn_tui 7 fire --frame --ascii --size 100x26 --keys
'U <down*3>'`, with "Uses power" added):

```
+------------------------- * BUILD * --------------------------+
|   Bed             5 Wood  Forge                              |
|   Campfire        3 Wood  Footprint: 2x2                     |
|   Academy   10 Wood, 2x2                                     |
| > Forge           Locked  8 Stone (or brick) (0 in stores)   |
|   Brick Kiln      Locked  4 Wood (0 in stores)               |
|   Smelter         Locked                                     |
|   Steam Engine    Locked  Room for 1 inside, through the     |
|   Lightning Stat~ Locked  door                               |
|   Farm Plot       Locked  Uses power                         |
|   Workshop        Locked                                     |
|                           Requires Forging                   |
+- Arrows/W S choose  Enter build  Esc close ------------------+
```

**Cursor panel.** On a consumer, one readout: `Powered: Wind Turbine, 1.5x` or `Unpowered`.
On a source: `Reach: 9 tiles` and `Powering: 2 buildings`. A mock of the side panel's Cursor
section (the frame's right column, with the section's lines edited):

```
+- CURSOR ---------- (41, 30) -+     +- CURSOR ---------- (44, 28) -+
| Workshop                     |     | Wind Turbine                 |
| Making: Tool, 60%            |     | Reach: 9 tiles               |
| Powered: Wind Turbine, 1.5x  |     | Powering: 2 buildings        |
|                              |     |                              |
```

**Map tint.** With the cursor on a source, or while placing one, its reach is tinted the way the
flame's reach is today (`Tint::Warm` in crates/tui/src/color.rs), with a new `Tint::Powered`
hue. In `--ascii` mode the tint is colour only, as Warm is. The cursor readout carries the
information without it.

## Order of work

This re-ranks the existing issues against the new ones. The first four together fill the
biggest dead spot: five inert buildings and six techs with no reader of their modifier.

| Rank | Issue | What | Priority (now → proposed) | Blocked by |
|---|---|---|---|---|
| 1 | #428 | Sim and data: power reach core | new, p2 | none |
| 2 | #255 | Fire: Steam Engine and Lightning Station as sources (narrowed: no grid) | p3 → p2 | #428 |
| 3 | #204 | Air: Wind Turbine and Windmill as sources | p3 → p2 | #428 |
| 4 | #429 | TUI: power in the build modal, the cursor panel and the reach tint | new, p2 | #428 |
| 5 | #202 | Water: Spirit Oasis with placeholder numbers (unblocked from #358) | p3 → p2 | none |
| 6 | #200 | Water: Tide Mill | p3 → p2 | #428 |
| 7 | #205 | Air: bison haul (v1) | p3 → p2 | none |
| 8 | #430 | Data: `shelter` known from the start, unlocks the Bed | new, p3 | none |
| 9 | #431 | Sim and data: `pottery` unlocks the Cistern | new, p3 | none |
| 10 | #432 | Balance: power in the economy opening, measured | new, p2 | #428, #255, #204, #200 |
| 11 | #433 | Fire: the Steam Engine burns wood (question 4) | new, p3 | #255 |
| 12 | #203 | Air: windmill grain (parked if question 1 goes as recommended) | p2 → p3 | owner |
| 13 | #254 | Air: bison flight (v2) | p3 | #205 |

#390 (Herb Garden) and #402 (Map Room) are already filed and unchanged. #402's Map Room adds
`uses_power = true` when both have landed, which #432 checks.

## How this fits with work in flight

- **#412 (pacing).** This doc adds content and leaves the clocks alone: no arrival timer, no
  knowledge rates, no sign thresholds. Powered study does shorten the 250-to-500 research
  stretch, so #412's measured timeline should be re-run after #432, not tuned against the
  numbers from before power. If #412 picks upkeep as a surplus sink, the Steam Engine's wood
  (#433) is a ready-made one.
- **#356 / #357 (nation buildings).** Their costs stand: the machines' brick and metal, and the
  tools on the Turbine and Stable. Nothing here changes a cost or a stock limit.
- **#411 (clearing ground).** No interaction, except that a cluster of 2x2 buildings under one
  source needs open ground, which is one more reason clearing matters.
- **#409 (voice pass).** The Windmill's and `tide_mills`' descriptions change with #204 and #200,
  so whichever lands second rewrites them to the voice guide.
- **#358 (chi and mastery)** owns the Spirit Oasis's final numbers. Landing #202 first leaves it
  one line of data to retune.
- **The main checkout** has an uncommitted edit to data/nations.toml. #430 touches
  `starting_techs` there, so whoever picks it up should check that edit has landed first.

## Follow-up issues

Filed without `ready`. The three that wait on an owner answer (#430, #431, #433) are labelled
`needs-owner`. Each existing issue got a comment pointing at this doc, and #255, #204 and #200
are now blocked by #428. Priority changes are recommendations in those comments, for triage to
make.

| # | Issue | Blocked by | Owner question |
|---|---|---|---|
| #428 | Sim and data: power reach (`[building.power]`, `uses_power`, recipe and study boost) | none | |
| #429 | TUI: power readouts and reach tint | #428 | |
| #430 | Data: `shelter` is a starting tech for every nation and unlocks the Bed | none | 2 |
| #431 | Sim and data: `pottery` unlocks the Cistern, a fresh drinking place | none | 3 |
| #432 | Balance: the economy opening places consumers in reach; measure power's effect | #428, #255, #204, #200 | |
| #433 | Fire: the Steam Engine burns wood while it powers something | #255 | 4 |
| #255 | (comment) Narrowed to Fire's two sources as data on #428; grid dropped | #428 | |
| #204 | (comment) Wind Turbine, plus the Windmill as Air's small source | #428 | 1 |
| #200 | (comment) Tide Mill as a source with the `beside` water rule | #428 | |
| #203 | (comment) Parked to p3 pending question 1 | | 1 |
| #202 | (comment) Unblock from #358 and land with placeholder numbers | | |
| #205 | (comment) Raise to p2, Air's capstone | | |

## Open questions for the owner

1. **Is the Windmill a power source, or does it grind grain into food (#203)?**
   *Recommended: a power source.* Food is already a glut by the midgame (#412). Grain needs a
   crop, an item, a recipe and a food entry, all to add more of what overflows. A small,
   cheap source at tier 120 gives Air power a tier before the Turbine, and the "mill" reads fine
   as one that turns lathes and prayer wheels. Grain can come back later as its own building if
   food ever gets tight.
2. **`shelter`: known from the start and unlocking the Bed, gating the Bed behind 25 points, or a
   new hut?** *Recommended: known from the start.* Play doesn't change, the Research screen stops
   offering an empty tech, and saves stay compatible. Gating the Bed would slow the opening's
   arrivals.
3. **`pottery`: a Cistern (fresh water anywhere) or a granary (more food per stockpile)?**
   *Recommended: the Cistern.* Thirst and sea-water sickness are real pressures today. Storage
   isn't, because food overflows. A granary can join it later if #412 adds spoilage.
4. **Should the Steam Engine burn wood while it powers something?** *Recommended: yes, as a
   second step (#433).* Haulers keep a small bunker topped up, and it burns about 1 wood a
   minute while a consumer in reach is working. That makes the 1,300-wood surplus #412 found
   worth something, and gives the fuel-free Lightning Station a reason to be the upgrade. The
   alternative is that no source ever needs fuel, which is simpler but leaves Fire's two
   machines differing only by numbers.
