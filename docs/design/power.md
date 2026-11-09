# Power as a progression track, and Earth's weak source

Doc status: current

Status: **proposed** (#447, #446). The owner's direction (2026-11-06): Earth gets a weak power
source, and power should be "another way for a nation to progress". The owner's calls are under
"Open questions for the owner", and the follow-up work is under "Follow-up issues".

This builds on late-tree.md (#413): power is a reach, not a grid. A source has a `radius`,
scaled by the nation's `power_generation`, and a `boost`. Work inside a `uses_power` building
in reach runs `boost` times as fast. #428 built that rule, and #429 added the readouts. #413's
four open questions are still with the owner. This doc doesn't answer them, but it says below
where it depends on them.

**Earth's source (#446) is self-contained.** It's the first section and can be approved on its
own.

## Decisions in brief

1. **Earth: the Hot Spring Mill** (working name, question 1). It's a cheap 2x2 that must touch
   rock, unlocked by `seismic_sense`. It has `radius = 2` and `boost = 1.2`, so its reach is 2
   tiles at Earth's 0.8, against 5 for every other nation's first source. It powers the mines
   it stands beside.
2. **The track is a ladder of visible rungs.** Each nation has a first source, then techs that
   widen its reach, then (for the strong nations) a stronger source. How deep the ladder goes
   follows `power_generation`: Air and Fire get two sources, Water gets one source and a later
   reach step, and Earth gets one weak source.
3. **Each nation's power has its own placement rule**, which is the decision it adds. Air builds
   anywhere and reaches furthest. Water builds on the bank. Earth builds on the rock face. Fire
   needs its industrial chain, and fuel too if #413's question 4 is a yes.
4. **Power stays a speed-up** (question 2). Nothing requires power to work at all. Unpowered
   means slower, never broken. That matters most for Earth.
5. **#255, #204 and #200 land as they are.** This doc layers two small data changes on top:
   `spirit_oasis` widens Water's reach (Tui and La move the tides, #450), and the Steam Engine's
   2 metal becomes bricks, so Fire's first rung doesn't wait for gold ore (#449).
6. **One new readout:** a POWER section on the Nation screen that shows where the nation stands
   on its track. There's no power sign: the gate's signs stay as endgame.md has them.

## Earth's source (#446)

Today #429 shows "Uses power" and "Unpowered" on five Earth buildings, and an Earth player has no
way to change that. The owner's call is a **weak** source, in keeping with Earth's 0.8. The
implementation is #448, which waits only on question 1 (the name and theme).

### The proposal

| | Hot Spring Mill |
|---|---|
| Theme | Earthbenders open a seam to the warm rock under the mountain. The steam turns a wheel |
| Tech | `seismic_sense` (Earth, 250): Seismic Sense already finds what lies beneath, and here it finds hot ground |
| Footprint | 2x2, solid |
| Placement | Must touch rock: `beside = ["mountain", "tunnel"]`, the Mine Shaft's rule from #373. No new code |
| Cost | 12 stone and 4 wood, 50 s of work (33 s at Earth's `construction` of 1.5) |
| `radius` / `boost` | **2 / 1.2** |
| Reach | 2 tiles at 0.8. No Earth tech raises `power_generation`, so it stays 2 |

```toml
# data/buildings.toml, appended (a new building goes at the end, so existing ids keep their place)
[[building]]
id = "hot_spring_mill"
name = "Hot Spring Mill"
verb = "Build"
work = 50.0
passable = false
footprint = [2, 2]
beside = ["mountain", "tunnel"]
[building.power]
radius = 2
boost = 1.2
[[building.cost]]
item = "stone"
count = 12
[[building.cost]]
item = "wood"
count = 4
```

**Weaker than every other first source**, before and after Earth's 0.8:

| Source | `radius` | `boost` | Reach on unlock | Tiles covered by a 2x2 |
|---|---|---|---|---|
| **Hot Spring Mill** (Earth) | 2 | **1.2** | **2** | 36 |
| Windmill (Air, if #413's question 1 is a yes) | 2 | 1.25 | 5 | 144 |
| Steam Engine (Fire) | 3 | 1.5 | 5 | 144 |
| Tide Mill (Water) | 4 | 1.5 | 5 | 144 |

Its boost is the lowest of any source, and it covers a quarter of the ground the others do. Its
raw radius ties the Windmill's. I chose that over `radius = 1`, which gives a reach of 1 at
0.8: that's only buildings touching the mill, there'd be no tint worth drawing, and no later
modifier could ever grow it. If the Windmill turns out to grind grain (#413's question 1), the
smallest other radius is 3, and the mill is below every source on both numbers.

**Why touching rock.** It puts Earth's power where Earth's own consumers already have to stand.
The Mine Shaft and the Deep Mine must touch rock too. So the Earth decision is a rock-face
quarter: the mill, a shaft on either side, and the Scroll Library or Workshop beside them if the
player wants study or tools sped up. The Deep Mine is the consumer that matters most, because a
powered Deep Mine cuts gold ore 20% faster, and ore is the gate's slowest sign on many seeds
(#396). A mill costs only stone, which Earth has hundreds of, so the way Earth grows its power
is breadth: one small mill per cluster, never one big one. That's the vision's "weak at this"
played as a trait rather than a gap.

**Why `seismic_sense`.** It's Earth's only 250 tech that unlocks no building, and it follows
`tunneling` (the Mine Shaft), so the shafts the mill is meant to serve already exist. `tunneling`
at 120 would be earlier than Air's Windmill, which is too early for the weakest nation, and that
tech already unlocks two things.

### How it reads

Build modal (from `fn_tui 7 earth --frame --ascii --size 100x26 --keys 'U <down*8>'`, the Mine
Shaft's frame, with the new row and its detail edited in):

```
FF+------------------------- * BUILD * --------------------------+"F
""|   Bed                  5 Wood  Hot Spring Mill               |""
FF|   Campfire             Locked  Footprint: 2x2                |F"
FF|   Scroll Library 10 Wood, 2x2                                |FF
FF|   Map Room             Locked  12 Stone (0 in stores)        |F"
.=|   Earth Wall           Locked  4 Wood (0 in stores)          |..
~~|   Rail                 Locked                                |-.
~~|   Farm Plot            Locked  Powers buildings within 2     |-.
~~|   Workshop             Locked  tiles                         |.=
~~|   Mine Shaft           Locked  Powered work: 1.2x faster     |"F
~~|   Deep Mine            Locked                                |"F
~~| > Hot Spring Mill      Locked  Requires Seismic Sense        |FF
~~+- Arrows/W S choose  Enter build  Esc close ------------------+"F
```

Here panel, on the mill and on a Mine Shaft beside it (the side panel's section, with the lines
edited):

```
+- HERE ------------ (23, 6) --+     +- HERE ------------ (25, 4) --+
| Hot Spring Mill              |     | Mine Shaft                   |
| Reach: 2 tiles               |     | Making: Stone, 40%           |
| Powering: 2 buildings        |     | Powered: Hot Spring Mill,    |
|                              |     | 1.2x                         |
```

### Measured

I made a throwaway patch with the five late-tree sources and this mill (seeds 1, 5, 7 and 8,
Small, Normal, 180 minutes, with an instrumented harness that counted powered consumer-minutes).
In it, the mill stands at minute 111 to 120 when the shared tier is researched first, and at
minute 76 to 84 with `--branch-first`. That's the earliest any nation's first source stands in
branch-first order, because its cost needs nothing Earth doesn't start with. The economy opening
puts every building at the nearest free site to the settlement's anchor, so it powered a mine on
only 2 of 8 runs, and the Scroll Library on none. That's the harness, not the mill: #432 teaches
the opening to place sources and consumers together. Adding the mill also moves "one of every
building" 6 to 10 minutes later for Earth, since there's one more kind to build. That sign is
never the long pole.

## What the harness shows about power today

The same patch, compared with main (shared tier first / branch first):

| Nation | First source stands (min) | Second source stands | Study hall powered, of ~179 min | Effect |
|---|---|---|---|---|
| Air | Windmill 87-98 / 75-81 | Turbine 102-112 / 79-87 | 70-86 / 93-105 | Branch sign 2-4 min sooner. Branch first: ore 6-9 min and scrolls 7-11 min sooner |
| Fire | Steam Engine 108-120 / **140-159 or never** | Lightning Station 122-135 / 144-163 or never | 46-72 / 0-40 | Branch sign 0-3 min sooner |
| Water | Tide Mill 102-110 / 110-111 | none | 0 / 0 | None (the harness doesn't build on the bank) |
| Earth | Hot Spring Mill 111-120 / 76-84 | none | 0 / 0 | None (the harness doesn't build on the rock face) |

Three findings shape the track:

1. **Power pays only while research is still ahead.** When the shared tier comes first, every
   source stands after minute 87, and the branch is done by 109 to 136, so powered study saves a
   few minutes at most. When the branch comes first, Air's turbine stands with the whole shared
   tier still to research: prospecting and cartography finish sooner, and the ore and scrolls
   signs follow. A human player interleaves the two orders, so the truth is in between.
2. **Cost gates a source more than its tech does.** Fire knows `steam_engines` by minute 48 to
   54 in branch-first order, but the engine needs metal, and metal needs gold ore and
   `prospecting`. So it stands 90 minutes later or not at all. Water's Tide Mill waits about 60
   minutes for its 2 tools.
3. **Population is the long pole today** (minute 150 on every seed), so faster research doesn't
   move Ready yet. Once draw lands (#435, population 16 at minute 100 to 130), the branch and ore
   signs become the poles, and power's research speed-up moves Ready directly. Power gets more
   valuable after #435, not less.

## The track, per nation

| Rung | Air (2.0) | Fire (1.3) | Water (1.0) | Earth (0.8) |
|---|---|---|---|---|
| **First source** | Windmill, `windmills` 120: r2, b1.25, reach 5 (#204, #413's question 1) | Steam Engine, `steam_engines` 250: r3, b1.5, reach 5 (#255) | Tide Mill, `tide_mills` 250: r4, b1.5, reach 5 (#200) | Hot Spring Mill, `seismic_sense` 250: r2, b1.2, reach 2 (#446) |
| **Wider reach** | `wind_turbines` ×1.25: Windmill to 6 | `lightning_generation` ×1.5: Engine to 7 | **`spirit_oasis` ×1.25 (new): Mill to 6** | none |
| **Strong source** | Wind Turbine, 250: r3, b1.5, reach 9 | Lightning Station, 500: r4, **b2.0**, reach 10 | none | none: breadth instead (one cheap mill per cluster) |
| **Where it goes** | Anywhere. One turbine covers a whole core | Fire's yard, and it needs the whole chain. With fuel (#433), it also needs a hauler's wood | On the bank, so the study hall and Workshop move to the waterfront | On the rock face, beside the mines |
| **What it speeds most** | Study (Air has the fewest recipe buildings) | Forge, kiln, smelter, Workshop, study: the most consumers | Study and the Workshop | The Deep Mine's gold, then the Mine Shaft |

Every rung except Water's reach step is either an existing tech's unlock or late-tree.md's
numbers. The two additions:

- **Water's reach rung: `spirit_oasis` gains `power_generation = 1.25`.** In canon, the Spirit
  Oasis is home to Tui and La, the Moon and Ocean spirits, who are the push and pull of the tide.
  Honouring them makes the tides run stronger, which takes the Tide Mill's reach from 5 to 6. It's
  one line of data next to the existing `water_work = 1.25`, and #202's chi aura is unaffected.
  Without it, Water's track has a single rung and the tech that unlocks it, with no growth after.
- **The Steam Engine costs 16 brick and 10 wood, not 10 brick, 2 metal and 10 wood.** Fire's
  first rung then needs only Fire's own chain (kiln and brick), not ore from a shared 250 tech.
  The Lightning Station keeps its 3 metal, so the capstone still needs the full chain, and #369's
  intent stands: Fire's industry pays for its power. This should stand the engine within about 20
  minutes of its tech in both research orders. #432 checks that.

**Not proposed:** techs that raise `boost` (a second modifier for a number that already differs
per source), a third Water or Earth source (more buildings on the weak nations would flatten the
differences `power_generation` is meant to make), and power for the Healing Hut or the Fishing
Dock (they don't do work inside, which is the rule).

## Speed-up, not a requirement (question 2)

**Recommended: power stays a pure speed-up.** No recipe or building needs power to work at all.

- The decisions power adds are placement decisions: where the source goes, which consumers
  move to it, and, with each new rung, whether to re-lay the core. Each nation's rule makes those
  decisions different: open ground, the bank, the rock face, the yard.
- A `requires_power` building would make "Unpowered" a broken state rather than a slow one. For
  Earth, with a 2-tile reach on the rock face, that's a lockout, not a weakness. With at most about
  eight consumers in a colony, a dead building costs more than the decision it adds.
- Power already reaches three gate signs through what it speeds up: the branch through study,
  ore through the Deep Mine and through prospecting researched sooner, and scrolls through the
  Map Room. It doesn't need a sign of its own.

**Revisit after the gate.** The United City (#442) is where a power-hungry building might make
sense, once every nation's settlers are in one place. It isn't filed here.

## UI: the Nation screen's POWER section

#429's readouts are per building. The progression needs one place that shows where the nation
stands. The Nation screen gains a POWER section above TRAITS. Mockup, edited from
`fn_tui 7 earth --frame --ascii --size 100x34 --keys N`, late in an Earth game:

```
| STORES                                                           |
| Stone 812  Wood 216  Gold Ore 46  Tool 7                         |
|                                                                  |
| POWER                                                            |
| Reach x0.8 - 2 sources powering 3 of 5 buildings                 |
| Unpowered: Scroll Library, Workshop                              |
|                                                                  |
| TRAITS                                                           |
| Move speed x0.9  Carry capacity x1.3  Earthworks x2.5            |
| Water work x0.9  Power generation x0.8  Construction x1.5        |
```

Before any source stands, it names the next rung: `No power yet - Seismic Sense unlocks the Hot
Spring Mill`. A nation with no consumers built yet omits the section (#451). The strings follow
text-voice.md: one idea per line, with no key glued on.

#429 also has two readout bugs that matter more once there are four kinds of source. Both go in
one follow-up (#452):

- The Here panel asks `World::power_at` about the consumer's **anchor tile** only, but the sim
  powers a building if **any** footprint tile is in reach. A 2x2 consumer reached only through
  its far tiles reads "Unpowered" while it's running at full boost, and its source's "Powering"
  count misses it.
- `{:.1}` prints the Windmill's 1.25 as "1.2x", the same as the Hot Spring Mill's 1.2. A boost
  should print to two places with trailing zeros dropped: "1.25x", "1.2x", "2x".

## How this fits with work in flight

- **#255, #204, #200 (now p2): land them as they are.** Nothing here changes their acceptance
  criteria or numbers. An implementer can start #255 now. The Steam Engine's cost change and
  Water's reach rung are separate small data issues (#449, #450) that land after them, so none of
  the three has to wait on this doc's open questions.
- **#413's question 1 (Windmill: source or grain).** If it's grain (#203), Air's first rung is the
  Turbine at 250. Air is still the strongest by reach (9), but it loses its early rung, and all
  four first sources land on 250 techs. The Hot Spring Mill's radius is then strictly below every
  other source. Nothing else here changes.
- **#413's question 4 (Steam Engine fuel, #433).** If yes, Fire's rung identity is fuel: a
  bunker its haulers keep full, and the fuel-free Lightning Station as the upgrade. Dropping the
  engine's metal matters more then, because the engine's running cost is wood, and its build cost
  shouldn't also wait on ore. If no, Fire's identity is the most consumers and the only 2.0
  boost, and the cost change still stands on finding 2.
- **#432 (power balance)** gains the Hot Spring Mill and two targets: each nation's first source
  stands within 20 minutes of its tech in both research orders on 7 of 8 seeds, and the opening
  places the Tide Mill and the Hot Spring Mill with at least one consumer in reach. A comment on
  #432 records this.
- **#448 vs #255/#204/#200.** The Hot Spring Mill is independent of the other three and can land
  first, as soon as the owner picks a name.
- **#435 (draw)** makes power matter more (finding 3). #432's measured targets should be re-run
  once it lands.
- **#409 (voice pass).** `seismic_sense`'s and `spirit_oasis`'s descriptions change with this.
  Whichever lands second rewrites them to the voice guide.

## Follow-up issues

Filed without `ready`. The issues that wait on an owner answer are labelled `needs-owner`.

| # | Issue | Blocked by | Owner question |
|---|---|---|---|
| #448 | Earth: the Hot Spring Mill, a weak power source on the rock face (`seismic_sense`) | #428 | 1 |
| #449 | Data: the Steam Engine costs brick, not metal, so Fire's first rung doesn't wait on ore | #255 | |
| #450 | Data: `spirit_oasis` widens Water's power reach | #200 | 3 |
| #451 | TUI: the Nation screen's POWER section | #428, #429 | |
| #452 | TUI: power readouts check the whole footprint, and print a boost like 1.25x (bug) | #429 | |
| #432 | (comment) Adds the Hot Spring Mill, placing beside-rule sources, the 20-minute target, and a re-run after #435 | | |

Question 2 has no issue: a yes needs no work, and a no would need a design of its own.

## Open questions for the owner

1. **Earth's source: what is it, and where does it go?** *Recommended: A, the Hot Spring Mill.*
   - **A. Hot Spring Mill** (geothermal, must touch rock). Earthbenders open a seam to the warm
     rock, and its steam turns a wheel. It sits with the mines, and it reuses #373's rule.
   - **B. Stone Mill** (earthbenders roll a great millstone, the way Omashu's chutes and Ba Sing
     Se's rail cars run on earthbending). It goes anywhere, so Earth's power has no placement
     identity, and it reads as labour, which a source isn't (no worker in v1).
   - **C. Magma Vent** (lavabending, an Earth sub-skill from Korra's era, must touch rock). It
     plays the same as A, but the fiction is far stronger than a 1.2 boost.
   Other names for A, if the theme is right but the name isn't: Steam Vent (too close to Fire's
   Steam Engine), Fumarole, Earth Kiln.
2. **Does power stay a pure speed-up?** *Recommended: yes.* Unpowered means slower, never
   broken. Placement and the per-nation rules are the decisions. The alternative, buildings that
   need power to run, turns Earth's weak power into a lockout and #429's "Unpowered" into an error.
   Revisit after the gate.
3. **Should the ladder's depth follow `power_generation`, with the Spirit Oasis as Water's
   reach rung?** *Recommended: yes.* Air and Fire get two sources. Water gets one source plus
   `spirit_oasis` widening its reach (Tui and La are the tide, so it's the most canon-true lever
   Water has, and it's one line of data). Earth gets one weak, cheap source it builds many of. The
   alternative is the same depth for everyone, with a second source each for Water and Earth.
   That's more content, and it flattens the 2.0-to-0.8 spread that makes power a nation trait. If
   the oasis rung is a no, Water's track is a single rung, which is acceptable for the middling
   nation.
