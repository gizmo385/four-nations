# Spirits: industry unsettles the spirits, and the Spirit Portal

Doc status: current

Status: **proposed** (owner, 2026-11-16). This is phase F6 of
[factory-endgame.md](factory-endgame.md). The owner asked for the questions to be drafted and the
work started on their recommended answers. The six questions are under "Open questions for the
owner", each with the default being built. Any of them can change before its slice lands.

## The idea

The Republic era's machines change the city, and canon says the spirits notice. Republic City's
industry drew the spirits' anger, and spirit vines crept over it until Korra opened the portals
and the two worlds met again. F6 gives the factory a cost that isn't a stockpile number. The more
a city runs on machines, the more the spirits stir. Left alone, vines creep in and choke the
machines nearest them. A city that keeps its balance gets something back: the Spirit Portal,
and spirits that walk its streets.

It adds one pressure and one goal, and no second map:

- **Pressure.** A city-wide **balance** meter that industry pulls down and green ground holds up.
  Low balance grows **spirit vines** that slow the machines they touch.
- **Goal.** The **Spirit Portal**, a great work that needs balance held high and vines put to
  use. Opening it brings friendly spirits who bless the city while it stays in balance.

## Principles

1. **Industry has a cost you can see on the map.** Vines grow where the machines are thickest,
   so the player sees the problem and where to fix it.
2. **Slow, never stop.** It's the same rule as the power brownout. Vines halve a machine's speed.
   They never wreck it or hurt anyone.
3. **Balance is a choice, not a tax.** A city can run hot and keep cutting vines, or plant
   gardens and run cooler. Both work, and they play differently.
4. **People stay the point.** Cutting vines is a job for settlers, and gardens need tending.

## Layer 1: balance

**Rule (proposed).** A United City has a **balance** from 0 to 100. It starts at 100 when the gate
stands. Once a minute it moves by **calm** minus **unrest**, then is clamped to 0 to 100.

| Unrest, a minute | Calm, a minute |
|---|---|
| 0.25 for each point of draw on every running yard (`World::yard_loads`, F4) | 1.0 always: the spirits settle on their own |
| 1.0 for each tree felled that minute | 1.5 for each Spirit Garden (layer 3) |
| | 0.5 for each 25 living trees on the map, at most 3.0 |

All numbers are provisional, live in `[spirits]` in `data/jobs.toml`, and are for a playtest to
set. With these numbers a yard of six machines at draw 1 costs 1.5 a minute, so a city with one
yard and some forest holds steady. Two big works on a plant (draw 8) cost 2.0 a minute, which
needs a garden to hold.

**Readout.** The side panel's glance line shows `Balance 72` once the gate stands. The United City
screen shows the meter, its trend a minute, and the two biggest terms ("Machines -2.5, Trees
+1.5").

## Layer 2: spirit vines

**Rule (proposed).** While balance is below **40**, every 30 seconds one vine tile grows. It
appears on a free land tile beside the most-loaded yard's machines: the yard with the highest draw,
the free tile nearest its anchor, first in tile order. An existing vine tile next to a free land
tile spreads into it instead, half the time, decided with the seeded RNG. Below **15** the
interval halves.

- A vine is a **resource on the tile** (`spirit_vines` in `data/resources.toml`). The Harvest
  tool cuts it. Cutting takes 6 seconds and yields one **spirit vine** item.
- A vine tile can't be built on, and settlers walk through it at half speed.
- An automated machine whose footprint ring touches a vine runs at **half speed**: "Choked by
  vines". This stacks with a brownout.
- At balance 40 or above, vines stop growing. They don't wither, so they still need cutting.

## Layer 3: calming the spirits

**Spirit Garden** (tech `spirit_gardens`, shared, after the gate, 1,000 points): a 3x3 passable
garden of stone and fruit trees. It adds **calm** while tended. A settler visits it like a Tea
House (the existing visit pattern) once a day. An untended garden still counts, at half calm.
Vines never grow within 4 tiles of a garden.

Trees count too (layer 1). A city that felled its forest for wood has less to stand on.

## Layer 4: the Spirit Portal

The era's second great work, after the Council Hall (F5). It reuses the Council Hall's one-site
shape. The design's stages wait on the endeavor system taking more than one endeavor.

| | Spirit Portal |
|---|---|
| Tech | `spirit_portal`, shared, 2,500 points. Needs the Council Hall standing and balance at 80 or above (a new `requires.balance`) |
| Footprint | 3x3, passable |
| Cost | 30 spirit vines, 20 stone, 10 steel, and a Notable of each nation present when it finishes (the council's) |
| On finishing | The portal opens. Notice: `The Spirit Portal opens. The spirits come back to the city.` |

**After it opens.** Friendly spirits wander in, one per 10 points of balance above 50, at most 5.
They're simple wanderers, the way beasts idle: no jobs, no needs. Each one **blesses** what it
passes. A farm plot or orchard it stands beside grows 25% faster for a minute. While balance
stays above 50 the portal also adds 2.0 calm, so an open portal helps hold the city's balance.
If balance falls below 30, the spirits leave until it recovers.

The portal is the era's finish line. Reaching it is a score event, as the gate is, and the
United City screen names it.

## How the four nations stay different

| Nation | Unsettles the spirits | Calms them |
|---|---|---|
| Fire | Hottest yards, biggest draw | Lightning plants run hot, so gardens matter most |
| Earth | Felled forest for timber | Vents are small, and the rock face isn't green, so few trees count |
| Water | Steady | Hydro mills on the bank: canals and farms nearby help |
| Air | Least: low draw, wide reach | Air Nomads' gardens give 2.0 calm, not 1.5 (their way with spirits) |

## Phases

| Slice | Ships | Sim work |
|---|---|---|
| F6a. Balance | The meter, its terms, the readout, `[spirits]` data | Small: a once-a-minute fold over loads and tallies |
| F6b. Vines | Vines grow at low balance, slow machines, and are cut for spirit vines | Medium: a resource that spreads, a choke check in `tick_automation` |
| F6c. Gardens | Spirit Garden, its calm and vine-free ring | Small: a building and a calm term |
| F6d. Portal | `requires.balance`, the portal, its notice | Small: F5's shape |
| F6e. Spirits | Wanderers, blessings, leaving at low balance | Medium: a beast-like wanderer with no jobs |

## Open questions for the owner

**Defaults in use (2026-11-16):** 1A, 2A, 3A, 4A, 5A, 6A.

1. **What does low balance cost?** *Recommended: A.*
   - **A. Vines creep and slow the machines they touch.** Nobody is hurt.
   - B. As A, and residents near vines slip a tier faster.
   - C. As A, and angry spirits appear that damage buildings.
2. **What unsettles the spirits?** *Recommended: A.*
   - **A. Machine draw and felled trees.**
   - B. Machine draw only.
   - C. Every building over a size.
3. **How does a city calm them?** *Recommended: A.*
   - **A. Spirit Gardens, standing trees, and time.**
   - B. Gardens only.
4. **What are cut vines for?** *Recommended: A.*
   - **A. A spirit vine item: the portal's main cost, and later a Notable good.**
   - B. Nothing. Cutting only clears them.
5. **What is the Spirit Portal?** *Recommended: A.*
   - **A. The era's finish line after the Council: built at high balance, it brings friendly
     spirits that bless the city.**
   - B. A win screen.
   - C. A door to a second map, the spirit world. Much larger.
6. **Do spirits walk the city?** *Recommended: A.*
   - **A. Yes, as simple wanderers after the portal opens (slice F6e).**
   - B. No. The portal's bonus is a modifier.

## Risks

- **Balance as a nag.** If the meter falls too fast, F6 becomes chores. The numbers start lenient,
  so one yard and a forest hold steady.
- **Vines on paths.** Half-speed walking through vines could jam haulers. Vines grow beside
  machines, not on roads, and a road tile never takes one.
- **Hard to see.** The readout names the biggest terms, so the player knows what to change.
