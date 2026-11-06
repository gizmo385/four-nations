# Nation buildings: deeper build lists, and a use for bricks, metal and tools

Status: **decided** (#357 and #356, designed together). All five owner questions below are
answered. The follow-ups that give bricks, metal and tools their use are closed (#369, #371,
#373, #374, #375, #376); #398 (Metalbend on a stockpile, so the Deep Mine can be paid for) is
still in progress. See "Open questions for the owner" and "Follow-up issues" for details.

Two problems share one fix. With the whole tree researched, Fire can raise 9 kinds of building,
Air 8, and Earth and Water only 6. (#357's "8, 7, 5, 4-5" count the same lists with one spare
kind taken off.) Fire's production chain ends in nothing: no cost, recipe or sign consumes
bricks, metal or tools (#356, materials.md section 3). So this design gives Earth and Water new
buildings that do real jobs, and makes bricks, metal and tools the costs of the buildings that
need them. Each of the three items gets a job the player can see.

## Decisions in brief

1. **Brick stands in for stone.** Any building or Harmony Gate cost that asks for stone can be
   paid in brick, one for one, and the stone sign counts brick. The kiln turns 3 stone into 4
   brick, so it becomes Fire's way to stretch stone, the gate's scarcest material. The owner
   asked for a use *in addition to* a limit, so the kiln's stock limit stays, raised from 20 to
   about 60.
2. **Tools are the fitting-out good every nation needs.** A shared **Workshop** (`carpentry`)
   makes tools from wood. Fire's forge makes them faster, from metal. Every branch's
   advanced buildings, and the gate's Arch, cost a few tools.
3. **Metal is Fire's and Earth's machine material.** Fire's two power buildings cost metal and
   brick, and Earth's capstone costs metal from Metalbend. Water and Air never need metal: they
   can't make it, and their buildings stay in their own idiom.
4. **Earth gets three buildings and Water three**, each with a job: Earth's Terrace (food), Mine
   Shaft (stone) and Deep Mine (ore); Water's Healing Hut (recovery), Fishing Dock (winter food)
   and Spirit Oasis (chi). Air gets none, because it is already within one of the others.
   Fire gets none.
5. **Every nation lands at 9 or 10 kinds**, and a new load-time check guarantees each nation
   can pay for every building it can unlock, which the "one of every building" sign (#354)
   depends on.

## Today's build lists

These are all the non-endeavor buildings each nation can raise with its whole tree researched.
The shared ones are the Bed (no tech), Campfire (`fire_making`), Study Hall (no tech) and Farm
Plot (`farming`).

| Nation | Own buildings (tech) | What they do | Kinds |
|---|---|---|---|
| Fire | Forge (`forging`), Brick Kiln (`brickmaking`), Smelter (`smelting`), Steam Engine (`steam_engines`), Lightning Station (`lightning_generation`) | metal into tools; stone into brick; ore into metal; the last two do nothing until power (#255) | 4 + 5 = **9** |
| Air | Orchard (`orchards`), Windmill (`windmills`), Wind Turbine (`wind_turbines`), Bison Stable (`sky_bison`) | renewable fruit; the other three do nothing yet (#203, #204, #205) | 4 + 4 = **8** |
| Earth | Earth Wall (`earthen_walls`), Rail (`earth_rail`) | blocks movement; hauling runs twice as fast | 4 + 2 = **6** |
| Water | Irrigation Channel (`irrigation`), Igloo (`ice_construction`) | farm plots nearby grow 1.5x faster; a warmer bed | 4 + 2 = **6** |

Earth's last two techs (`seismic_sense`, `metalbending`) unlock only abilities, so Earth runs out
of things to build halfway up its tree, which the 2026-11-05 playtest hit. Water's branch is the
emptiest: four of its six techs (`healing`, `tide_mills`, `water_travel`, `spirit_oasis`) unlock
nothing concrete.

## The materials

### Brick: Fire's stone

`data/jobs.toml` gains one optional field on an item:

```toml
[[item]]
id = "brick"
icon = "brick"
name = "Brick"
stands_for = "stone"   # may pay, one for one, any building or endeavor cost that asks for stone
```

- **Rule.** When a site still needs N stone, a hauler may bring stone or brick, whichever source
  is nearer (ties go to brick, so a Fire settlement keeps its raw stone). The site's tally counts
  both. A cancelled site refunds exactly the items that were delivered, brick as brick, which
  needs one `serde(default)` vector on `Site` and no `SAVE_VERSION` bump. Stand-ins never pay a
  **recipe input**, so the kiln can't fire its own bricks back into bricks.
- **The sign.** `data/endeavor.toml`'s stone sign lists `items = ["stone", "brick"]`. That works
  today, because `stored` already sums its items. Its hint becomes "Quarry stone (bricks count)".
- **What it's worth.** The gate's 180 stone costs Fire 135 quarried stone, and the 200 sign
  costs 150. Earth's lead in stone is the tunnel bonus (5 to 8 an outcrop after #349), and
  Fire's is now the kiln. Air and Water have neither, which is their trade for other strengths.
- **Stock limit.** Brick now does everything stone does for Fire, so stretching stone alone
  would let the kiln run forever. **Decided (owner, 2026-11-05): the kiln's `stock_limit`
  (20, #350) stays, raised to about 60** rather than removed, so brick is worth stretching
  stone with but not hoarding without end. The pause key (#351) remains the player's control.

### Tools: everyone's fitting-out good

| | Recipe | Work | Modifier | `stock_limit` |
|---|---|---|---|---|
| **Workshop** (new, shared, `carpentry`) | 3 wood → 1 tool | 40 s | construction | 6 |
| Forge (Fire) | 1 metal → **2** tools (was 1) | 25 s | heating | 6 (was 4) |

The Workshop is research.md's long-planned "workbench" for `carpentry`. Its input is wood on
purpose: wood is the glut (materials.md section 5), and stone is what the gate is short of. The
forge is Fire's faster route: at Fire's heating a tool takes about 5 s of work against the
Workshop's 36. It also turns ore the gate doesn't need into something useful.

**Where tools go:** the advanced buildings below (1 or 2 each), the Wind Turbine and Bison
Stable (+2 each, data only), the Lightning Station (+2), and the Harmony Gate's **Arch (+6)**.
That is about 4 to 6 tools for one of every building, plus the Arch's 6. It's a steady, modest
draw, and the stock limit keeps workshops from running forever.

### Metal: the machine material

| Building | Cost today | Proposed |
|---|---|---|
| Steam Engine (Fire) | 15 stone, 10 wood | 10 **brick**, 2 **metal**, 10 wood |
| Lightning Station (Fire) | 25 stone, 15 wood | 16 **brick**, 3 **metal**, 2 tools, 15 wood |
| Deep Mine (Earth, new) | — | 20 stone, 4 **metal**, 2 tools |

These costs name brick itself, not stone, so the kiln is a real step on the way to Fire's top
tier. Metal stays scarce: Fire's two machines take 5 metal (10 ore), so Fire's ore bill is about
42 (the gate's 32 plus 10). That is materials.md's "about 48 for Fire" estimate, now spent on
something. The smelter's stock limit rises from 4 to 6, enough for the Lightning Station with
some spare. (#376 measured it and cut the limit to 3: see "Measured".)

## The new buildings

Every new entry is **appended** to `data/buildings.toml`, because a `BuildingId` is the file
position baked into saves. Each one gets a `data/tui.toml` glyph, and nations keep their own
buildings through their techs' unlocks (no `if nation` anywhere).

| Building | Nation, tech | Footprint | Cost, work | Job | Sim work |
|---|---|---|---|---|---|
| **Workshop** | shared, `carpentry` (60) | 2x2, solid, capacity 1 | 8 wood, 4 stone; 50 s | tools from wood (above) | none: a recipe |
| **Terrace** | Earth, `earth_shaping` (60) | 1x1, walkable | 4 stone; 15 s | farm plots within 1 tile grow 1.5x faster | none: the channel's `[building.irrigation]`, radius 1 |
| **Mine Shaft** | Earth, `tunneling` (120) | 2x2, solid, capacity 1; must touch mountain or tunnel | 10 stone, 6 wood, 2 tools; 80 s | a miner inside cuts 1 stone per 50 s (earthworks): about 40 a season at Earth's rate, and never runs out | a recipe with no input; the `beside` placement rule |
| **Deep Mine** | Earth, `metalbending` (500) | 2x2, solid, capacity 1; must touch mountain or tunnel | 20 stone, 4 metal, 2 tools; 150 s | 1 gold ore per 150 s (earthworks): about 12 a season at Earth's rate | data only, once the Mine Shaft lands |
| **Healing Hut** | Water, `healing` (120) | 2x2, solid, capacity 2 | 10 wood, 4 stone, 1 tool; 60 s | a hurt settler rests inside and heals 8 health a minute (times water_work) instead of 2 | a `[building.healing]` section and a seek rule |
| **Fishing Dock** | Water, `water_travel` (250) | 2x2, solid; must touch water | 12 wood, 2 tools; 70 s | plants up to 3 fishing spots in the water within 4 tiles, tends them 1.25x faster, and picks them as they ripen | the orchard, planting on water |
| **Spirit Oasis** | Water, `spirit_oasis` (500) | 2x2, solid (a pool) | 20 stone, 2 tools; 120 s | settlers within 4 tiles regain chi 2x faster, awake or asleep | a chi aura, like heat |

### Earth: work the land, then dig in

- **Terrace.** Ba Sing Se's farm ring. It gives Earth a 60-knowledge building, and Earth's first
  food lever, the counterpart to Water's channel: tighter reach, built in stone. It moves from
  `earthen_walls` (#136's plan) to `earth_shaping` ("level and raise the ground"), so Earth has
  something to raise before tier 120. It's data only: an `[building.irrigation]` with radius 1,
  and the channel's no-stacking rule carries over.
- **Mine Shaft.** Earth is the mining nation, so it digs in. The miner works inside, so nobody
  walks out to deposits. It yields stone only, at about half a quarrier's rate after #349 (70 a
  season), so it supplements quarrying and doesn't replace it. It never runs out. Whether Earth
  should have a renewable mineral at all is question 3.
- **Deep Mine.** Earth's capstone, and what Metalbend's metal is for: metal braces a shaft deep
  enough to reach gold. It arrives at the end of the tree, when the ore sign is usually met
  already, so it shortens the gate's 32-ore bill without trivialising the hunt. Recipe output
  isn't a harvest, so it doesn't add to the "Ore mined" tally, the same way Metalbend's metal
  doesn't.
- **Not now:** the flood barrier stays with #136 and floods (#199), and Earth reaches 11 kinds
  when it lands.

### Water: heal, fish through winter, restore chi

- **Healing Hut** (`healing`, the issue #125 asked for, simplified). A settler whose health is
  below `seek_below` (60) and who has no need below its `low` walks in and rests. It heals
  `regen_per_minute` (8, times water_work: 16 for Water) until it reaches 90 or a need pre-empts
  it. Health only falls when a need runs out, so the hut is the safety net that makes
  materials.md question 2's "close calls" survivable. The healer job and the Healing Water
  ability are dropped from #125's scope: abilities belong to #358.
- **Fishing Dock** (`water_travel`: canoes go out from it). Water's orchard, on the water.
  Fishing is frost-hardy, so this is the one producer that works through winter, which suits a
  polar nation. Three spots give about 22 fish a season (2 per pick, regrow 300 s / 1.25), close
  to the retuned orchard's 25 (#353), and fish carry strain on Hard as they do today. The
  orchard's planting check (`can_plant`) already uses the resource's own terrains, but it
  requires walkable ground, and it must also honour `fish_spot`'s `beside = ["coast", "land"]`
  so every spot can be fished from shore. That is the whole sim change.
- **Spirit Oasis** (`spirit_oasis`, #202). Chi ran dry in both playtests. The oasis is the place
  benders go back to. The numbers here (radius 4, 2x) are placeholders for **#358**, which
  designs how chi and mastery pace together. Like `heat`, it's a radius around the footprint,
  read where chi regenerates.
- **Not now:** the tide mill (#200) waits on power (#255), and Water reaches 11 when it lands.
  Ice as a building material (research.md: "ice blocks as a material later") would be Water's
  own brick. It's a good idea, but it's another item, and it's out of scope here.

### Air and Fire

- **Air** keeps its four buildings, plus the Workshop, which makes 9. Three of its four do nothing
  yet (#203, #204, #205), and making those work is worth more than a fifth kind (question 4).
  The Turbine and Stable gain 2 tools each.
- **Fire** gains no kinds. Its chain now closes: the kiln stretches stone, the smelter feeds the
  machines and the forge, and the forge's tools go to every advanced building.

### Bending

- The Spirit Oasis and the Healing Hut give Water's bending something to restore and protect.
  Healing Water, chi numbers, and whether time at a building counts toward mastery all belong to
  **#358**.
- The Deep Mine is Metalbend's economic use. Neither Earth mine touches Bore Tunnel's yields:
  a shaft placed beside a tunnel works the same as one beside rock.

## The result

| Nation | Today | New | Total | `every_building` target (`spare = 1`, #354) |
|---|---|---|---|---|
| Fire | 9 | Workshop | **10** | 9 |
| Air | 8 | Workshop | **9** | 8 |
| Earth | 6 | Workshop, Terrace, Mine Shaft, Deep Mine | **10** | 9 |
| Water | 6 | Workshop, Healing Hut, Fishing Dock, Spirit Oasis | **10** | 9 |

All four nations are within one kind of each other. The planned flood barrier (#136) and tide
mill (#200) take Earth and Water to 11, which is still within two.

### Bills against materials.md's supply targets

This is what one of every building costs, summed from `data/buildings.toml` as it stands after
#369 to #375 (#376). The Terrace (#372) and Spirit Oasis (#202) haven't landed, so Earth's and
Water's bills leave them out; they would add 4 and 20 stone. The figures in brackets are
materials.md's "Buildings for the kinds sign" figures before this design.

| | Stone | Ore | Tools | Wood | Other |
|---|---|---|---|---|---|
| Fire (10 kinds) | 36 stone plus 26 brick (about 20 stone through the kiln) [72] | 10, as 5 metal | 2 | 69 | |
| Air (9) | 22 [18] | 0 | 4 | 78 | |
| Earth (9) | 39 [5+], repaid by the Mine Shaft | 8, as 4 metal from Metalbend | 4 | 34 | |
| Water (9) | 8 [0] | 0 | 3 | 54 | 4 ice |
| The gate | 180 (Fire: 135 quarried) | 32 | 6 | 150 | |

### Measured (#376)

`balance --economy --seeds 1-8 --minutes 180`, Small, Normal, all four nations, with the
supply survey (`balance --supply --radii 40 --resources stone,boulder,gold_vein,gold_seam`)
alongside. The harness now also prints the minute each building first stood and what is in
stores at the end, so the opening's coverage of the new buildings can be read off directly.

**Coverage.** On every seed where the opening isn't stalled (below), it raises one of every
building, the new ones included: the Workshop by minute 33 to 57, Fire's forge, kiln and smelter
by minute 105, both machines by 152, the Mine Shaft by 106, and the Healing Hut and Fishing Dock
by 109 and 132. Tools sit at their stock limit (6 to 11 in stores) and the kiln at its 60 brick
for the whole second half of every Fire run, so both crafters are kept running. Two gaps remain:

- **The Deep Mine is never built**, on any seed, even in 300-minute runs. Its 4 metal can only
  come from Metalbend, and Metalbend only works a loose ground stack of ore. By the time Earth
  researches `metalbending` (minute 147 or so), every vein in reach has been mined and every bit
  of ore hauled into stockpiles, so there is never a stack to bend. That is a mechanic, not a
  number. Decided (question 5): Metalbend may work ore in a stockpile, which #398 implements.
- **The igloo was never built** either, because the opening filled every bend slot with cheaper
  bends and never cut ice. The harness now designates a bend that makes an item an unbuilt
  building is short of (Cut Ice, Metalbend) ahead of cheaper ones. With that, Water raises its
  igloo on 4 of the 6 seeds that grow; the rest have no river, lake or sea in reach of the anchor.

**Signs by minute 120.** Stone and ore are met on these seeds (target: 7 of 8 for each):

| | Stone | Ore |
|---|---|---|
| Fire | 6 of 8 (16 to 36 min) | 5 of 8 |
| Air | 7 of 8 (15 to 30 min) | 5 of 8 |
| Earth | 8 of 8 (6 to 32 min) | 6 of 8 |
| Water | 6 of 8 (17 to 37 min) | 4 of 8 |

None of the misses trace to a building cost. Every one has one of three causes, all filed:

- **Seeds 3 and 4: the opening stalls.** Fire, Water and Air raise no bed on either seed (and on
  seed 4 next to nothing at all), with 40 to 290 wood in stores, so the population stays at 6.
  Only Earth, which tunnels, grows there. #384 noted these seeds as slow growth; this run shows
  the cause is placement, not food (#395).
- **Seeds 2 and 6: ore isn't there.** The supply survey finds 5 and 17 ore within 40 tiles, and
  every nation mines 27 to 30 and stops. No cost change helps (#396).
- **Water on seed 7** never researches its branch and mines no ore (#397).

**Fire's ore bill.** The gate needs 32 ore in stores after the machines are built. With the
smelter's `stock_limit` at 6, Fire ended with 28 and 29 ore on seeds 5 and 7, below the gate's
32, while Air on the same seeds kept 54 and 57: the smelter's standing stock and the forge's
tools ate about 14 metal, 28 ore. **Retuned:** the smelter's `stock_limit` falls from 6 to 3,
which is still enough for the Lightning Station's 3 metal. Fire now ends with 34 to 69 ore on
every seed that meets the ore sign and doesn't stall (35, 34 and 39 on seeds 5, 7 and 8), so its
whole bill fits the reachable ore wherever the ore sign itself can be met. (On stalled seed 3 the
forge turns all of Fire's ore into tools, because its tools never reach a stockpile and so never
count toward its stock limit. That's part of the stall.)

**Food per head entering winter 2** (minute 105) is 10 to 21 for Fire, 5 to 20 for Earth, 6 to
20 for Water and 30 to 55 for Air, outside the 3 to 8 band almost everywhere. The new producers
aren't the cause: the Terrace isn't in data yet (#372), and the Fishing Dock first stands at
minute 124 to 127, after winter 2 has begun. Fire, with no new food building at all, overshoots
as much as Water does. Bringing food into the band is #353's retune (farms, orchard, arrivals),
which is still open, so nothing is changed here. Wood at minute 120 is over 300 on seeds 2, 5 and
6 for the same reason.

## UI

The build picker (`U`) needs no new layout: new rows appear for the nation's own techs, and the
detail pane describes each building from its sections, as the Bed's does today. A mockup for
Water late in the game follows. It was built from
`fn_tui 7 water --frame --ascii --keys 'U <down*5>' --size 100x24`, cropped to the modal, with
the rows and detail edited to show the proposal:

```
+------------------------- * BUILD * --------------------------+
|   Bed             5 Wood  Fishing Dock                       |
|   Campfire        Locked  Footprint: 2x2                     |
|   Elders' ~ 10 Wood, 2x2  12 Wood, 2 Tool (0 in stores)      |
|   Igloo           Locked                                     |
|   Farm Plot       Locked  Plants up to 3 fishing spots       |
|   Irrigation Cha~ Locked  in water within 4 tiles and        |
|   Workshop        Locked  tends them: they regrow 1.25x      |
|   Healing Hut     Locked  faster. Must touch water.          |
| > Fishing Dock    Locked                                     |
|   Spirit Oasis    Locked  Requires Water Travel              |
|                                                              |
|                                                              |
+- Arrows/W S choose  Enter build  Esc close ------------------+
```

- A stone cost shows the stand-in for a nation that can make brick: "10 Stone (or brick), 12 in
  stores", where the stores figure counts both.
- A building that must touch a terrain says so, and the placement ghost's red reason is "must
  touch water" or "must touch rock".
- The kiln's cursor panel (#351) reads "Brick pays for stone" under its recipe line, so the
  player learns why it never idles.

## Sim changes, in one place

All of these are small. Each is data-driven, deterministic, and uses `serde(default)` for any new
state:

1. **`stands_for` on an item:** build and endeavor cost fetching, delivery and refunds. It never
   applies to recipe inputs.
2. **A `beside` placement rule:** `beside = ["water", "river", "lake"]` on a building means its
   footprint's ring must touch one of those terrains. It is a new `Blocker`, so the ghost explains
   itself. The tide mill (#200) can reuse it.
3. **Source recipes:** `input = []` is allowed. A crafter works with nothing to fetch, and
   `stock_limit` still applies.
4. **Orchards that plant on water:** `can_plant` skips the walkable check for a resource that
   spawns on water, and honours the resource's `beside`.
5. **Healing:** `[building.healing] { regen_per_minute, seek_below, modifier }` and a seek rule
   in the needs and work code, next to sleep.
6. **A chi aura:** `[building.chi] { radius, regen_boost }`, read where chi regenerates (#202).
7. **A load-time check (or a test):** for each nation, the items it can obtain (harvests its
   techs allow, recipe outputs of its buildings, and its bends' outputs) cover the cost of every
   building it can unlock. Today's data passes. Without the check, a nation could be handed a
   building it can never pay for, and the `every_building` sign would become unreachable for it.

## Follow-up issues

Filed without `ready`. The ones that wait on an owner answer are labelled `needs-owner`.

| # | Issue | Blocked by | Owner question |
|---|---|---|---|
| #368 | Defs: every building a nation can unlock is payable from what it can make | none | |
| #369 | Data: Workshop and tools as costs (forge 2 tools, machines in brick and metal, Arch 6 tools) | #350 | 1 |
| #371 | Sim: brick stands in for stone (`stands_for`; costs, refunds, stone sign, kiln limit off) | #350 | 2 |
| #372 | Data: Earth terrace under `earth_shaping` | none | |
| #373 | Sim and data: Earth Mine Shaft and Deep Mine (source recipes, `beside` rule) | #350, #369 | 3 |
| #374 | Sim and data: Water Fishing Dock (orchard planting on water) | #369, #373 (`beside`) | |
| #375 | Sim and data: Water Healing Hut (narrows #125) | #369 | |
| #202 | Water: Spirit Oasis (existing issue; spec commented there, numbers from #358) | #358 | |
| #376 | Balance check: these costs against the supply survey and the economy opening | #348, #352, #369, #371, #373, #374 | |
| #395 | Balance harness: the economy opening raises no bed on seeds 3 and 4 (from #376) | none | |
| #396 | Data: ore in reach on seeds 2 and 6 is far below the ore sign (from #376) | none | |
| #397 | Balance harness: Water on seed 7 never researches its branch (from #376) | none | |
| #398 | Earth: Metalbend works ore in a stockpile, so the Deep Mine can be paid for (from #376) | none | 5 (decided) |

## Open questions for the owner

1. **Who makes tools: every nation (a shared Workshop, from wood), or only Fire (the forge)?**
   If only Fire makes them, tools can only be costs for Fire's own buildings, and Earth's and
   Water's new buildings use nothing from #356. *Recommended: a shared Workshop.* Tools become
   the one refined good everyone needs, the forge stays Fire's faster route, and the input is
   the wood glut, not scarce stone. **Decided (owner, 2026-11-05): a shared Workshop; every
   nation can make tools.**
2. **Should brick pay for stone everywhere, the gate and the stone sign included?**
   *Recommended: yes, one for one, except as a recipe input.* It is the kiln's purpose and
   Fire's answer to the stone wall, and it cuts Fire's gate stone by a quarter. The alternative
   is bricks only in Fire's own building costs, which is a smaller sink that leaves the kiln a
   side job. **Decided (owner, 2026-11-05): yes, one for one, everywhere, never as a recipe
   input.**
3. **Should Earth have renewable minerals (the Mine Shaft's stone, the Deep Mine's ore)?**
   materials.md keeps stone and ore finite, so that finding them is the game. *Recommended:
   yes, slow and labour-bound:* stone at half a quarrier's rate from tier 120, and ore only at
   the capstone, when the ore sign is usually met. The alternative is the Mine Shaft alone, or
   neither (Earth gets 1 or 2 new kinds, not 3). **Decided (owner, 2026-11-05): yes, Earth gets
   the slow, worked Mine Shaft and Deep Mine.**
4. **Air: a new building, or make its idle three work first?** *Recommended: no new Air building
   now.* Air sits at 9, within one of the others, and #203 (the windmill grinds grain) is worth
   more than a fifth kind that does something. Consider raising #203 from p3. **Decided (owner,
   2026-11-05): no new Air building; #203 raised to p2 instead.**
5. **Earth's Deep Mine can never be paid for (#376, #398). Let Metalbend work ore in a
   stockpile, or drop metal from the Deep Mine's cost?** Metalbend only bends a loose stack, and
   by the time Earth knows it all ore is stockpiled. **Decided (owner, 2026-11-06): Metalbend may
   target a stockpile tile holding ore.** It keeps the design's point (Metalbend's metal is what
   the capstone is for), and a player would expect to bend the ore they've gathered. #398
   implements it.
