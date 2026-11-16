# The Republic era: a factory endgame built on what people want

Doc status: current

Status: **proposed** (owner, 2026-11-15). The owner reviewed the direction in the project thread
and asked for it to be built. The six questions under "Open questions for the owner" are being
built on their recommended answers (A) until the owner says otherwise; each is listed there with
its default. Follow-up work is under "Follow-up issues".

The owner's idea: an endgame expansion where the game "becomes more factory-esque", overlapping
the open power and steel work. The first draft of this doc built machines, freight and power,
and its only products were steel and fittings that went back into more machines. The owner's
review (2026-11-15) was that this is a factory for its own sake. This draft starts from the
other end: **the factory exists to meet what the city's people want**, and machines, freight and
power are how a city meets those wants at scale.

## Decisions in brief (proposed)

1. **The era starts at the Harmony Gate.** The United City already has every nation's people
   in one place, and pacing.md left it a "lighter loop" with no new system. Canon backs it:
   Aang's United Republic grows into Republic City. Nothing before the gate changes or becomes
   required.
2. **Residents have wants, and meeting them raises their standing.** Once the gate stands, each
   resident can rise from **Settler** to **Citizen** to **Notable** when their wants are met for
   a stretch, and slips back a tier when they aren't. Nobody dies or leaves over a want. A city
   that falls short simply stays a town.
3. **Each resident wants their own nation's goods.** A Fire resident wants lanterns, then fine
   steelware. Water wants smoked fish, then parkas. Earth wants furniture, then crystal jewellery.
   Air wants tea, then incense. Every tier also wants a comfort in reach (a Tea House, Bathhouse,
   Pai Sho Table or lamp). Notables also want **goods from two other nations**: bought from
   caravans at first, then made at home once residents of that nation can teach the recipe.
   **Who you invite decides which chains you need**, and phase 6's draw steering (#761) already
   lets the player choose who comes.
4. **Standing pays back in play.** Citizens and Notables work faster and draw newcomers. Only
   Notables can crew a power plant and sit on the capstone's council. The city's goal becomes
   its people's standing, not a headcount.
5. **The factory is the means.** Goods are made in recipe buildings, as everything is today.
   Machines (recipe buildings that run themselves), freight links and power with a load are how a
   city of 40 keeps up with 40 people's wants. Steel becomes an input to several goods instead of
   the product.
6. **Nations differ in what their people want and in how they make and move it.** Air's wants are
   light goods, and Air moves light goods fastest. Earth's are heavy, and Earth has freight rail.
   Water's chains run along water. Fire packs a dense yard.

## Why

What the late game has today, from the docs and data on `main`:

- **The United City is the same loop, wider** (pacing.md, section 4, as built in #442 and #761):
  caravans from all four nations, requests that reward draw, and population milestones every 8
  people forever. Its goal is a number going up.
- **Newcomers of other nations change who can do what** (their modifiers and bends), but they
  want exactly what everyone wants: food, water and a bed. Nothing about a Water resident's life
  differs from an Earth resident's.
- **Production is one step deep and has no consumer at the end.** Ore makes metal, metal makes
  tools, and tools go into buildings and tech bills. Once the city is built, nothing is ever
  used up, so a bigger economy has nothing to do.
- **Steel has no purpose** (#968), and power has a handful of consumers (power.md).
- **Draw already rewards comfort in embryo.** Food variety and warm beds raise it (pacing.md,
  section 1). That's a want system with two wants and no tiers. This doc grows it.

## Principles

The seven rules in research-horizons.md still hold. Five more for this era:

1. **Every chain ends in a person.** A good exists because some resident wants it. No item is
   made only to build the machines that make it.
2. **Unmet wants slow a city, never break it.** Falling a tier is the cost. Nobody starves, sickens,
   leaves or dies over a lantern. Survival pressure stays where harm.md put it.
3. **People stay the point.** Machines wear and need tending, plants need crews, and freight is
   loaded by hand at one end. A city of 40 should be busier, not idle.
4. **Rates are readable.** Every want, good, machine and link says what it uses or makes per
   minute and what it's short of.
5. **Reuse before invent.** Consumption reuses the remedy fetch (a hurt settler fetching herbs,
   harm.md). Comfort reach reuses heat reach. Machines reuse `automate` (#725). Freight reuses
   the chute and the paired buildings. Power reuses `[building.power]`.

## Layer 1: wants and standing (the foundation)

### Tiers

| Tier | Becomes it when | What it gives (provisional) |
|---|---|---|
| **Settler** | Every resident starts here, as today | Nothing new |
| **Citizen** | Their Citizen wants have been met for 15 minutes | Works 15% faster at everything. Adds +0.05 to draw, up to +0.4 for the city |
| **Notable** | A Citizen whose Notable wants have been met for 30 minutes | Works 30% faster. Can crew a power plant (layer 4) and sit on the council (the goal) |

A want unmet for 20 minutes drops the resident one tier, and they climb back the same way. The
Avatar and other guests have no tier. Tiers are per resident, saved with `serde(default)`, and
need no RNG.

### Wants

A want is one of two kinds, both reusing a shipped mechanic:

- **A good**, consumed. Every `every_minutes` the resident fetches one from stores, the way a
  hurt settler fetches herbs (`World::plan_remedy`). Missing from stores means the want is unmet.
- **A comfort**, in reach. Their bed stands within the reach of a qualifying building, the way
  warm beds check a campfire's radius (`World::is_heated`).

| Nation | Citizen wants | Notable wants (adds) |
|---|---|---|
| **Fire** | **Lanterns** (metal and wood, Forge), one every 15 min | **Fine steelware** (steel and glow crystal, Steelworks second recipe), one every 20 min |
| **Water** | **Smoked fish** (fish and wood, a new Smokehouse), one every 10 min | **Parkas** (hide and fittings, Workshop second recipe), one every 20 min. Hide is a new by-product of the Fishing Dock |
| **Earth** | **Furniture** (wood and fittings, Workshop), one every 15 min | **Crystal jewellery** (glow crystal and metal, Forge second recipe), one every 20 min |
| **Air** | **Tea** (herbs, a new Tea Works), one every 10 min | **Incense** (herbs and fruit, Tea Works second recipe), one every 15 min |
| **Everyone** | One comfort in reach: Tea House, Bathhouse, Pai Sho Table or an Oil Lamp | Two different comforts in reach, and **Citizen goods of two other nations** (any two) |

All names and numbers are placeholders. What matters is the shape:

- **Every nation's Citizen good is one step from what it already makes**, and its Notable good is
  two or three steps, reaching into steel, crystal or a by-product.
- **Each nation's goods suit its own strengths.** Air's tea and incense are light goods, which
  Air's wind tubes carry (layer 3). Earth's furniture is heavy, and Earth has freight rail.
  Water's smoked fish comes off the water. Fire's lanterns and steelware come out of the yard.
- **A United City has to serve all four.** A Fire city that invites Air people needs herbs and a
  Tea Works. Inviting Earth people means fittings, which means steel, which means ore. Who you
  steer in (#761) is now an economic choice, not only a choice of who can bend what.
- **Comforts reuse shipped buildings** that today only help caravans or a tired settler. The Tea
  House, Bathhouse and Pai Sho Table get a second job.

At about one good per resident every 15 minutes, a city of 40 uses roughly 3 goods a minute
across eight kinds, with Notables doubling their share. That's the factory's load.

### Foreign goods

The owner's review (2026-11-15) added that residents should want other nations' goods too. A
Notable wants the Citizen goods of any two nations besides their own, one of each every 30
minutes, counted the way draw counts food kinds. That makes the Notable tier a taste for the
wider world, which suits a United City, and it gives a want three sources over a game:

1. **Caravans.** Each nation's caravan sells its own Citizen good in small lots (`data/trade.toml`,
   as food is sold today, #551). Before anyone of that nation lives in the city, buying is the
   only way. Caravans stop being only a sink for surplus and become a supply line.
2. **Borrowed recipes.** With 3 residents of a nation, the city can borrow that good's recipe at
   2x cost (phase 6's `teachable`), and make it at home in the ordinary building.
3. **Fusion machines** (layer 2, tier 5) make two nations' goods well at once.

So the arc of each foreign good is buy, then learn from the neighbours, then build. A Fire city's
Notables who want tea and furniture send the player first to the Air and Earth caravans, then to
steering Air and Earth newcomers in, then to a Tea Works and fittings.

### What it replaces

Draw's food variety and warm-bed terms stay for the road to the gate. In the United City, the
Citizen draw term sits on top of them. The population milestones (#929) stay, and the Endeavor
screen's United City view adds residents by tier and nation.

## Layer 2: machines and how they connect

The Factory (`great_works`, #725) already runs recipe buildings in a Lightning Station's yard
with no crafter, fed by Supply hauls (`crates/sim/src/world/automate.rs`). This layer gives every
nation machines, a way to connect them, and a short tree to climb.

### How machines connect

No conveyor belts. Machines connect three ways, and only the first is new:

1. **Touching.** When a machine's output is an input of a machine whose footprint touches it, a
   finished batch passes straight across with no haul and no stockpile. Smelter, Steelworks, Forge
   and Workshop in a row turn ore and wood into furniture with haulers only at the two ends. This
   is the chain rule from the Fire yard, opened to every nation by `mechanisms`.
2. **Freight links** (layer 3). A machine touching a link's far station takes its inputs from the
   link directly, so a mine at the rock face can feed a Steelworks across the map.
3. **Stockpiles**, as today. Anything that doesn't pass across or ride a link spills by the door
   and is hauled to stores, which is the buffer between chains.

**One routing control per machine:** `Send to: stores` or `Send to: next machine`, defaulting to
next machine where one touches. With it off, the machine stocks the stores as today, which is how
a player splits one Steelworks' steel between Fire's steelware and Earth's fittings. That's
`Command::SetRoute { pos, to }`, a `serde(default)` flag like `Building::manual`.

**Readable.** A machine's Here panel shows its chain: `From: Smelter (touching)`, `To: Forge
(touching)`, `Waiting: no wood`. A production overlay draws arrows between touching machines and
along links, the way power has a reach tint.

### The machine tree

Five tiers. Each one lets a city serve a higher tier of resident, or more nations at once.

| Tier | Unlocked by | Adds | What it serves |
|---|---|---|---|
| 1. **Hand work** (ships) | The existing techs | Forge, Kiln, Smelter, Workshop, plus F1's new recipe buildings (Smokehouse, Tea Works). A crafter works every batch | Citizens of your own nation |
| 2. **Mechanisms** | `mechanisms`, post-gate shared, 1,000 points plus steel | Recipe buildings in your nation's power reach run themselves (shapes below). Touching machines pass batches across. Machines wear, and a settler tends them | More Citizens than hands allow |
| 3. **Freight** | One tech per nation, 1,000 points | Earth's freight line, Water's flume, Air's wind tube. Fire has the Balloon Dock already | Chains that span the map: rock face to yard, river to city |
| 4. **Power and big works** | One tech per nation, after tier 2 | Plants with a capacity, crewed by Notables (layer 4). **Big works**: 3x3 machines that do two steps in one (steel to fittings to furniture, or herbs to tea and incense), with a heavy draw | Notables' fine goods at scale |
| 5. **Fusion machines** | Six fusions in phase 6's pattern: two nations, 3 residents of each, 1,000 points plus tools | One machine per pair that makes both nations' goods well. Examples: a **Smoke Kiln** (Fire and Water: lanterns and smoked fish from one fire), a **Mountain Works** (Earth and Air: furniture and incense) | Notables' foreign goods without buying them |

The Steelworks is already the Earth and Fire fusion (phase 6), so tier 5 grows the pattern it
started rather than inventing one.

### The pipeline planner (owner's idea, 2026-11-15)

**How a player wires a factory.** Indirect control already means "you plan it, they build it",
so the factory's main tool is a planner, not a wiring tool. Two screens and a stamp:

1. **The planner** shows one chain as a flow from left to right: raw goods on the left, the
   goal on the right. Each step is a box with its machine, how many are needed and how busy they
   would be. Each arrow between two steps is labelled with what moves along it and how much a
   minute, and its line shows how it travels: a plain line where the two machines touch, rail
   ties for a freight line, dots for haulers. Inputs that feed many steps (wood) run along a bus
   underneath, with the amount each step takes.
2. **Every connection is a choice.** Selecting an arrow shows why it is what it is ("the mines
   must stand at the rock face, 38 tiles from the yard, so these two can't touch") and the ways
   the good can travel there: a freight line, haulers, or moving the machines so they touch.
   That's where rails, flumes and wind tubes come in: they are what an arrow is built from when
   two steps can't touch, and the stamp builds them with the machines.
3. **A good has its own screen.** Opening a good (from the planner, the Goods screen or a
   building) shows what makes it now, what uses it, the gap, and every way to get more, each
   with how much it adds a minute and what it costs: build another machine, buy from a caravan,
   or send settlers to the deposits. Choosing one adds it to the plan the screen was opened from.
4. **Stamp it on the map.** The plan becomes a ghost block of touching machines in the right
   order, with any freight stations and the rail between them, moved and turned like the Build
   tool's ghost, which already shows tiles to clear (#427). Enter turns it into ordinary build
   orders, and settlers haul and build it.

The Goods screen then shows whether the real factory meets the plan, and a saved plan can be
stamped again. The planner is arithmetic over recipes and rates, so it lives in `fn_ui` beside
the other view models and needs nothing new in the sim beyond F1's per-machine rates. The
pictures are `planner.png`, `good.png` and `stamp.png` in the project's `factory-endgame` folder.

### Each nation's machines

`mechanisms` lands on the city's own nation's source, with its own shape:

| Nation | Style | Source and shape | Rate |
|---|---|---|---|
| Fire | **The yard** (ships pre-gate as the `great_works` school) | Lightning Station, 4-tile yard | 1.0 |
| Earth | **The face**. Mines and the Crystal Grotto along the rock run themselves | Hot Spring Mill, beside the rock | 0.6 |
| Water | **The race**. Machines run only along water: bank, canal or aqueduct | Tide Mill | 0.75 |
| Air | **The loft**. The slowest machines, with the widest reach | Wind Turbine, anywhere | 0.5 |

Fire keeps its edge: it reaches tier 2 before the gate through its school, at the best rate.

**Wear.** Every few batches an automated machine stops and posts a **Tend** job that uses a tool
or fittings. That keeps people in the loop and gives tools a standing sink.

## Layer 3: freight

Today every item moves on someone's back. Wants spread goods across the whole city, to every
resident's door, so hauling becomes the bottleneck. Freight is what makes layout matter.

**Rule (proposed).** A freight link is a pair of stations, like a Wind Gate pair. Haulers drop
goods into the near station's small hold, and the link moves a stack to the far station every
`interval` seconds. The hauling AI treats a link the way it treats a chute: one more edge in a
route, taken when cheaper.

| Nation | Link | Placement | Carries |
|---|---|---|---|
| Earth | **Freight line**, a cart on rail pushed by an earthbender | Both ends on one rail line | Heaviest loads. A third speed with no bender aboard |
| Water | **Flume** | Downhill or downstream along river, canal or aqueduct | Anything, free running |
| Air | **Wind tube** | Straight, up to 30 tiles, any terrain | Light goods only (no stone, ore or metal), fastest |
| Fire | None long-range. The yard's hand-across rule, and Balloon Docks for distance | | |

The alternative is tile-by-tile conveyor belts: the purest factory feel, but a new spatial system
for the glyph map, and haulers stop mattering. That's question 4.

## Layer 4: power with a load

**Rule (proposed).** Machine-tier buildings (anything automated, plus the era's new buildings)
draw on a source's `capacity`. Over capacity, every machine on that source slows in proportion (a
brownout). Nothing stops. Every existing building keeps power.md's speed-up rule.

The era's sources are **plants**, crewed by **Notables**. A lightning plant crewed by Fire
Notables is Republic City's power station. Wind Farms (Air) need no crew but vary by season.
Hydro Mills (Water) sit on the bank. Geothermal Vents (Earth) are small and stack along the rock.
So your best-served residents power the city, which ties standing back into the machines that
serve everyone else.

## Steel and other goods

New items, all provisional:

| Item | Made at | From | Wanted by, or spent on |
|---|---|---|---|
| **Steel** | Steelworks | 2 metal and 2 wood | Fine steelware (Fire Notables), fittings, plants and freight stations, and some of the phase-6 big builds' metal (#968's favourite) |
| **Fittings** | Forge, second recipe | 1 steel and 1 wood, makes 2 | Furniture (Earth Citizens), parkas (Water Notables), tending machines |
| **Lanterns, smoked fish, furniture, tea** | See the wants table | | Citizens |
| **Fine steelware, parkas, crystal jewellery, incense** | See the wants table | | Notables |
| **Hide** | Fishing Dock by-product | | Parkas |

**#968, steel.** The Steelworks turns metal into steel, so Earth's metalbent glut (#958) becomes
feedstock and its stock limit counts steel, not metal. Steel's uses are now mostly people's
goods, with building costs second. The scripted opening's `united_steel_made` target measures
real production.

**#890, scroll-billed schools.** Option A (bill scroll copies from the Archive) is a production
chain instead of a ruin search, which is this doc's direction too. It's also right on its own.

## The goal: the Republic Council

The United City view counts residents by tier and nation. The capstone is a second great work,
the **Council Hall**: it needs Notables of every nation living in the city (2 each, provisional)
plus a build in stages paid in steel, brick and goods. Finishing it founds the city's council,
canon's United Republic Council with a member from each nation. Its payoff could be a council
seat per nation that grants that nation's strongest modifier city-wide, or the Spirit Portal's
opening. That part is open.

Standing caravan contracts (rates of a good, for rewards) from the first draft become a side
outlet for surplus, not the goal.

## Mockups

Hand-drawn in the terminal's style, at 100x34 with the ASCII glyph set, edited from real frames
(`fn_tui 7 fire --frame --ascii --size 100x34 --ticks 30000 --keys N`, and `G` for the Endeavor
screen). Every number is invented for a Fire city of 34 in its third year. The strings aim at
text-voice.md: one idea per line, keys in the footer.

### The United City screen (`G`, after the gate)

The Endeavor screen already becomes the United City view once the gate stands (#442). It gains
standing, a row per nation, the wants that are short, and the Council Hall's progress.

```
+-------------------------------------- * THE UNITED CITY * ---------------------------------------+
| The United City                                                                        34 people |
| Every nation's people live here. Meet their wants and they rise.                                 |
|                                                                                                  |
| STANDING                                                                                         |
|    Notables      #.........       5                                 Lightning Plant: 3 of 5 crew |
|    Citizens      #####.....      17                                                              |
|    Settlers      ####......      12                                                              |
|                                                                                                  |
| BY NATION           PEOPLE  SETTLER  CITIZEN  NOTABLE   SHORT OF                                 |
|    Fire Nation         14        3        8        3   Tea, for 3 Notables                       |
|    Earth Kingdom        9        4        4        1   Furniture: none in stores                 |
|    Water Tribes         7        3        3        1                                             |
|    Air Nomads           4        2        2        0   Tea: the Tea Works has no herbs           |
|                                                                                                  |
| WANTS THIS HOUR            MADE   USED   STORES                                                  |
|    Lanterns                  14     11       9                                                   |
|    Furniture                  4      9       0   Short                                           |
|    Smoked fish               10      8      12                                                   |
|    Tea                        6      8       1   Short                                           |
|    Fine steelware             3      3       2                                                   |
|                                                                                                  |
| THE COUNCIL HALL                                                                                 |
|    Notables of every nation   ######....   5/8                     Earth 1/2, Water 1/2, Air 0/2 |
|                                                                                                  |
| Next milestone: 40 people                                                                        |
+--------------------------------------------------------------------------------------------------+
 CITY  Up/Down choose - Enter show on map - Esc or G close                                    ? keys
```

### The Nation screen, with standing and a resident's wants

Two new columns (nation and standing) and a WANTS meter. The detail pane under the list shows
what the selected resident wants, how often, and when they slip a tier. Foreign goods say whose
they are.

```
+------------------------------------------- * NATION * -------------------------------------------+
| Fire Nation city - 34 people - seed 7                                             Year 3, Autumn |
|                                                                                                  |
|   NAME      NATION  STANDING  DOING                     WANTS              BENDING               |
| > Kuzon     Fire    Notable   Crewing Lightning Plant   ####. met          Master                |
|   Pema      Air     Citizen   Fetching tea              ###.. tea short    Adept                 |
|   Haru      Earth   Settler   Hauling ore               #.... furniture    Student               |
|   Siku      Water   Citizen   Tending Smokehouse        ##### met          Adept                 |
|   Ran       Fire    Citizen   Smelting                  ####. met          Adept                 |
|                                                                                                  |
+--------------------------------------------------------------------------------------------------+
| Kuzon - Fire Nation, Notable                                                          Works x1.3 |
| Crewing the Lightning Plant. Carrying nothing.                                                   |
|                                                                                                  |
| WANTS                        EVERY    NEXT                                                       |
|    Lantern                   15 min   in 11 min     Met                                          |
|    Fine steelware            20 min   in 11 min     Met                                          |
|    Tea (Air)                 30 min   17 min late   Unmet: 1 in stores, 3 people waiting         |
|    Smoked fish (Water)       30 min   in 24 min     Met                                          |
|    Two comforts in reach                            Met: Tea House, Bathhouse                    |
|                                                                                                  |
| Slips to Citizen in 3 min unless tea reaches them.                                               |
+--------------------------------------------------------------------------------------------------+
 NATION  Up/Down choose - Enter select - Esc or N close - PgUp/PgDn page                      ? keys
```

### The Goods screen (new; its key is still to be chosen, since `p` is the Stockpile tool)

Phase F1's readout and the factory's main tool: every good's rate in and out, where it's made,
and who wants it. The selected row explains the shortfall and names the ways to close it, which
are exactly the three tools this doc adds: automate, carry, or buy.

```
+------------------------------------------- * GOODS * --------------------------------------------+
| Goods                                                                per minute, last 10 minutes |
|                                                                                                  |
|   GOOD             MADE   USED   NET    STORES   MADE AT                  WANTED BY              |
|   Gold ore          4.2    4.0   +0.2      18   2 mines (face), line     Smelter                 |
|   Metal             2.0    2.1   -0.1       3   Smelter                  Steelworks, Forge       |
|   Steel             1.0    0.9   +0.1       6   Steelworks               Forge, Fire Notables    |
| > Fittings          0.6    1.1   -0.5       0   Forge (manual)           Furniture, tending      |
|   Furniture         0.3    0.6   -0.3       0   Furniture Works          9 Earth residents       |
|   Lanterns          1.4    1.1   +0.3       9   Forge, 2nd recipe        14 Fire residents       |
|   Tea               0.6    0.8   -0.2       1   Tea Works, caravan       4 Air, 3 Notables       |
|                                                                                                  |
+--------------------------------------------------------------------------------------------------+
| Fittings: short 0.5 a minute                                                                     |
| Made by 1 Forge, worked by hand, 40% of the time (the crafter also hauls).                       |
| Automating it in the Lightning Station's yard would make about 1.0 a minute.                     |
| Earth caravans sell 6 fittings a visit.                                                          |
+--------------------------------------------------------------------------------------------------+
 GOODS  Up/Down choose - Enter show where it's made - Esc close                               ? keys
```

### The inspector on machines and links

The side panel's last section, which describes whatever is under the cursor (side-panel-v2.md,
#653). Its heading names the thing. One idea per line, five lines at most. Where a good comes from and goes to is dim, under what
the building is doing. Three examples: a Steelworks in a Fire yard, the near end of an Earth
freight line, and a Workshop nothing feeds.

```
+- STEELWORKS ---- (24, 11) -+   +- FREIGHT STATION - (26, 2) +   +- WORKSHOP ------ (30, 18) -+
| Making steel - 60%         |   | Sending gold ore           |   | Making furniture           |
| Runs itself in the yard    |   | Next cart in 12 s          |   | Waiting for fittings       |
| Metal from the Smelter     |   | To the yard, 38 tiles      |   | Nothing feeds it           |
| Steel to the Forge         |   |                            |   |                            |
```

### The map, with the production overlay

A toggle like the reach tint. Arrows show where goods flow: from the mines (`HH`, a Deep Mine at
the rock face) onto a freight line (`#` stations, `=` rail, `>` the cart's direction), then down
into a row of touching machines (`SS` Smelter, `WWW` Steelworks, `FF` Forge, `UU` Furniture
Works) and out to stores (`[]`). `LLLL` is the Lightning Station whose yard runs them. These
letters are placeholders: the picture version uses the game's real glyphs (Smelter `Ψ`,
Steelworks `Ξ`, Forge `Ω`, Workshop `⚒`, Lightning Station `Ϟ`, Deep Mine `◈`, rail `╪`), and
proposes `⊟` for a freight station.

```
^^^^^^^^^^,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,
^^MMMM^^^^,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,
^^MMMM^#>>=================================>#,,,,,,,,,,,,,,,
^^^^^^^^^^,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,v,,,,,,,,,,,,,,,
^^HHMM^^^^,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,SS>WWW>FF>UU,,,,,
^^HHMM^^^^,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,SS WWW FF UU,,,,,
^^^^^^^^^^,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,v,,,,,,,
,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,LLLL,,,,,,,[],,,,,,
,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,LLLL,,,,,,,,,,,,,,,
```

### Research: the machine tree

A page on the Research screen, beside phase 6's From newcomers page. The five tiers read left to
right, and fusion machines name the residents they still need.

```
+------------------------------------------ * RESEARCH * ------------------------------------------+
| Machines                                                                  Research: Machine tree |
|                                                                                                  |
| 1 HAND WORK            2 MECHANISMS          3 FREIGHT             4 POWER, BIG WORKS            |
|   Forge          ok      Mechanisms     ok      Freight lines  50%    Lightning plants           |
|   Kiln           ok      1,000 + 4 steel        1,000 points          1,000 + 6 steel            |
|   Smelter        ok      Yard runs itself,      Carts on rail,        Notables crew it;          |
|   Workshop       ok      touching machines      40 a minute           machines draw load         |
|   Smokehouse     ok      pass goods along                              Big works (3x3)           |
|   Tea Works      new                                                                             |
|                                                                                                  |
| 5 FUSION MACHINES  (from newcomers: 3 residents of each nation)                                  |
|   Steelworks    Earth + Fire     known                                                           |
|   Smoke Kiln    Fire + Water     needs 2 more Water residents                                    |
|   Mountain Works Earth + Air     needs 3 Air residents                                           |
|                                                                                                  |
+--------------------------------------------------------------------------------------------------+
| Freight lines - 500 of 1,000                                                                     |
| A cart on rail, pushed by an earthbender, moves 20 at a time between two stations.               |
| Unlocks: Freight Station (pair).                                                                 |
+--------------------------------------------------------------------------------------------------+
 RESEARCH  Arrows choose - Enter study - Tab page - Esc or T close                            ? keys
```

### A caravan selling another nation's goods

Caravans gain each nation's Citizen goods in small lots, which is the first way a city gets a good
it can't make.

```
+------------------------------------------ * CARAVAN * -------------------------------------------+
| Air Nomad caravan                                                                Leaves in 6 min |
|                                                                                                  |
| THEY SELL                            FOR                                                         |
| > 6 Tea                              8 wood                                                      |
|   4 Incense                          2 lanterns                                                  |
|   10 Fruit                           6 stone                                                     |
|                                                                                                  |
| THEY WANT                                                                                        |
|   4 Lanterns            Draws an Air newcomer                                                    |
|                                                                                                  |
| Tea is short: 3 Fire Notables and 1 Air resident are waiting.                                    |
+--------------------------------------------------------------------------------------------------+
 CARAVAN  Up/Down choose - Enter trade - Esc close                                            ? keys
```

## How the four nations stay different

| | Fire | Earth | Water | Air |
|---|---|---|---|---|
| **Its people want** | Lanterns, then fine steelware | Furniture, then crystal jewellery | Smoked fish, then parkas | Tea, then incense |
| **Makes** | Densest yard, hand-across chains | Automated extraction along the rock | Machines strung along water | Slow machines, but the most of them |
| **Moves** | Short. Balloons for distance | Freight rail, the heaviest loads | Flumes, downhill | Wind tubes, light goods only |
| **Powers** | Lightning plants crewed by Notables | Many small vents | Steady Hydro Mills | Free, seasonal Wind Farms |
| **Weak at** | Moving goods far | Power | Anything away from water | Heavy goods |

## How this fits the open power lane

Nothing in the lane has to wait on this doc.

| Issue | Effect |
|---|---|
| #447, #446, #448, #449, #450, #451, #429, #432, #488 | Unchanged |
| #433 (Steam Engine burns wood) | **Leans yes.** Running costs are this era's language |
| #413 (late tier pays off) | Unchanged. This doc picks up power.md's "revisit after the gate" |
| #835 (minute 50 to 80 gap) | **Not addressed.** That gap is before the gate |
| #968 (steel) | **Answered here, provisionally:** steel from metal at the Steelworks, wanted by residents first |
| #890 (scroll-billed schools) | **A** fits this direction, and is right on its own |

## Phases

Each phase ships something playable. Wants come first, because they make every later phase
matter.

| Phase | Ships | New sim work | Waits on |
|---|---|---|---|
| F0. The lane | The open power issues, #968 steel, #890 | As filed | Nothing new |
| F1. Wants and standing | Tiers, wants, the new goods made by hand in recipe buildings, a **People** view by tier and nation, rates on the Here panel | Medium: a per-resident tier, a want fetch (the remedy's), comfort reach (heat's), a few recipes and buildings | F0's steel. A United City playtest (#752) for numbers |
| F2. Machines | `mechanisms`: the per-nation `automate` siblings, touching hand-across, the route toggle, wear and Tend | Small to medium: `automate` exists | F1 |
| F3. Freight | Freight line, Flume, Wind tube | Medium: a link edge in hauling (the chute's), station holds | F1 |
| F4. Power and big works | `draw`, `capacity`, brownout, plants crewed by Notables, 3x3 big works | Medium | Question 5 |
| F4b. Fusion machines | Six two-nation machines | Small: phase 6's fusion pattern | F2, residents of both nations |
| F5. The Council Hall | The capstone great work | Medium: reuses the Harmony Gate's stages | F1 |
| F6. Spirits | Industry unsettles the spirits, vines creep, balance pushes back, then the Spirit Portal ([spirits.md](spirits.md)) | Large | A playtest of F1 to F4 |

**F1 alone is a small factory game.** Four nations' wants, eight goods, chains two or three steps
deep, all made by hand. If that's fun, F2 to F4 make it bigger. If it isn't, F2 to F4 won't save
it, and we've learned that cheaply.

## Playtesting it

The era starts after the Harmony Gate, hours into a game, so each phase must be reachable at
turn 0 (#1025):

- **Republic starts.** `fire-republic`, `water-republic`, `earth-republic` and `air-republic` in
  `data/scenarios.toml`: four hours of the economy opening, the gate stood, then
  `World::start_republic` learns the techs and fills the stores the newest phase needs, on
  sandbox rules so nobody starves while a feature is tried. `fn_tui --scenario fire-republic`
  starts one; so does **Playtest starts** on the title screen, in the terminal and the window.
- **Each phase keeps them current.** A phase's PR adds what its feature needs to
  `REPUBLIC_TECHS` and `REPUBLIC_ITEMS` (`crates/sim/src/world/playtest.rs`) and says in its body
  what to try and what should happen.
- **Saves.** A good setup can be saved and loaded as usual, so a playtest can pick up where it
  left off.

## Risks

- **Busywork.** Eight goods fetched by 40 people could feel like upkeep. The rates and the
  "what's short" readout are the defence, and the harness can measure how much of the city's time
  goes to hauling goods.
- **One nation's wants dominate.** If Earth's furniture chain is much harder than Air's tea, cities
  steer away from Earth residents. Measure chain depth and labour per want, and tune per nation.
- **Session length is unknown.** No one has played the United City for long (#752).
- **The glyph map gets dense.** A production overlay (rates and links) will be needed, like power's
  reach tint.
- **Map room.** Small maps (192x128) may cramp Water's lines and Earth's freight.

## Open questions for the owner

**Defaults in use (2026-11-15):** 1A, 2A, 3A, 4A, 5A, 6A. Any of them can change before its phase
lands.

The era starting at the gate is assumed. Say so if it shouldn't.

1. **What do unmet wants cost?** *Recommended: A.*
   - **A. Residents slip a tier.** Nobody leaves or dies.
   - B. Residents who stay unhappy long enough leave the city.
   - C. Nothing. Tiers only go up.
2. **Whose wants?** *Recommended: A.*
   - **A. Citizens want their own nation's goods, and Notables also want two other nations'.**
     Foreign goods come from caravans, then borrowed recipes, then fusion machines.
   - B. Own nation's goods only.
   - C. One list for the whole city, whatever nation people are from.
3. **What does standing give?** *Recommended: A.*
   - **A. Work speed and draw, and Notables crew plants and sit on the council.**
   - B. Only draw and the city's score.
4. **How do goods move?** *Recommended: A.*
   - **A. Touching machines pass goods across, and paired freight links cover distance**, one
     kind per nation. Haulers still load the near end.
   - B. Conveyor belts laid tile by tile.
   - C. Settlers only, with better roads.
   - D. A as well as explicit supply routes drawn in the planner, a hauler assigned to each.
5. **Does power get a load?** *Recommended: A.*
   - **A. Only for machines and the era's new buildings**, with crewed plants. Overload slows,
     never stops.
   - B. Power stays a speed-up forever.
6. **Do machines wear?** *Recommended: A.*
   - **A. Yes, and a settler tends them** with a tool or fittings.
   - B. No. Machines run until their inputs run out.

Spirits (F6) are a later question, best asked after F1 to F4 have been played.

## Follow-up issues

None filed yet. Once the questions are answered: one design issue per phase (F1 to F6; F0 is
already filed), a comment on #968 with this doc's steel proposal, and a comment on #433 noting it
leans yes.
