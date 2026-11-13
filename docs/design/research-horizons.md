# Research horizons: a long-term spec for the tech tree

Doc status: current

Status: **accepted** (owner, 2026-11-10; all eight answers are under "Owner decisions"). This is
the map for research content beyond the starter tree in [research.md](research.md) and the late
tree in [late-tree.md](late-tree.md). Each phase under "Build order" gets its own design doc and
issues; footprints, bills and reaches are left to those docs.

The tree today is a checklist: every game researches the same 18 or 19 techs (about 2,400
points) in about two hours, and the only choice is the order. This spec proposes about 40 more
techs and five new kinds of research decision, so that research keeps asking questions through
the Harmony Gate and into the United City.

## Decisions in brief

The five decisions the tree should ask, and the mechanic behind each:

| The question | Mechanic | Today |
|---|---|---|
| What first? | A tree bigger than one game finishes before the gate | The whole tree is done by minute 110 to 150 |
| Which one, for good? | **Schools**: one either/or fork at the top of each nation's branch | Nothing is ever given up |
| Where does it go? | Every new tech puts something on the map with a placement rule | True of power, tunnels and rail; not of the modifier-only techs |
| What is it worth to you? | **Lost teachings** drafted from ruins (pick 1 of 2), and top-tier techs that cost scrolls or tools | Knowledge is the only cost |
| What now that the tree is done? | **Exchange** techs taught by United City newcomers, and scribing as a sink | Study halls go idle after the branch |

Nothing here changes a shipped tech's cost or position. Every new tech is appended to
`data/research.toml`, so `TechId` order and existing saves hold. The spec keeps the rules already
decided: no war or weapon techs, nations differ through data, power stays a speed-up, no bend
makes stone, ore or food from chi, and no spoilage.

Two owner decisions here amend earlier ones: cross-nation learning (deferred 2026-10-22,
research.md) reopens for the post-gate game, and top-tier techs may cost items, where research.md
said "time and labor, not materials".

## Where research stands today

Research runs out about halfway through a game. A nation's whole tree is known by minute 107 to
150 (research.md, pacing.md), the gate targets 3.5 to 4 hours (endgame.md), and the United City
after it has no research at all.

| | Shared tier | Earth | Fire | Water | Air |
|---|---|---|---|---|---|
| Techs | 12 | 6 | 6 | 6 | 7 |
| Points | 1,140 | 1,300 | 1,300 | 1,300 | 1,265 |
| Starts knowing | | `stoneworking` | `fire_making` | `fishing` | `orchards` |
| Capstone (500) | | `metalbending` | `lightning_generation` | `spirit_oasis` | `sky_bison` |
| Leads in (`nations.toml`) | | earthworks 2.5, construction 1.5 | heating 2.5 | water_work 2.0 | move 1.6, power 2.0 |
| Weak in | | move 0.9, power 0.8 | water_work 0.7 | heating 0.8 | carry 0.8, earthworks 0.7 |

Four things limit how interesting it is:

1. **Nothing is exclusive.** No tech locks another out, so two Fire games end with the same
   known set. Only the order differs, and the harness plays the file order.
2. **Knowledge is the only cost.** Study competes with hauling for hands, which is a real trade
   early. From minute 60 about half the settlers are idle (pacing.md), so the trade disappears
   just as techs get expensive.
3. **The tree is fully visible and fully reachable.** Every game sees the same techs from tick
   0. Scrolls pay 40 points each. Their `reveals` hook was kept open on 2026-10-22 and is unused.
4. **There is no late research.** Cost tiers stop at 500. The pool caps at 10 points with no
   target (`idle_knowledge_cap`), so a finished tree turns scholars into idle hands.

Work already designed covers the content gaps in the existing tree, and this spec builds on it
without repeating it: power as a reach (late-tree.md, power.md), the Cistern and `shelter` (#430,
#431), herbs and harm (harm.md), caravans (#437), and abilities by rank (bending.md, #358).
late-tree.md set the test this spec keeps: a tech must show the player something new and ask for
a new decision.

## Design principles

A tech earns its place when the player would sometimes choose not to research it yet. Seven
rules follow from that, and every tech in the catalogue below was checked against them.

1. **A tech answers a pressure the player has already felt.** The pressures in the game are
   winter and exposure, tiring land, injury, sea water, long hauls, scarce stone and ore, idle
   hands and slow growth. A tech with no pressure behind it is a badge.
2. **It puts something on the map.** A building, a bend, a route or a creature. A modifier may
   ride along, never stand alone.
3. **It asks where, or at what price.** A placement rule (the bank, the rock face, downhill, in
   pairs) or a running cost (food, wood, stone, a scholar's time). Power's reach and the Tide
   Mill's bank rule are the model.
4. **Nations get different answers to the same pressure.** Winter food is a Hothouse for Fire,
   the Fishing Dock for Water, a walled Agrarian Ring for Earth, and moving camp for Air.
5. **The strongest techs spend what is scarce and soak up what is in glut.** Stone, ore and tools
   are scarce. Food, wood and labour pile up from minute 60. A late tech that eats food or wood
   is doing two jobs.
6. **Automation is a reward, and it is honest about it.** A tech that removes a chore
   (re-sowing, picking fruit) is welcome late, once the chore has been learned. It should cost
   enough that keeping the chore is a real option.
7. **Peaceful, readable, data-first.** No weapons and nothing that targets a unit. Every effect
   has a hover line, and a reach tint where it has a reach. Every number lives in TOML, and no
   system asks which nation it is.

## New research mechanics

Five additions to how research works carry the five decisions. Each is a small schema change on
top of `research.toml`, and each can ship without the others.

### 1. Schools: one fork per nation

A school is a set of two techs where learning one closes the other for the rest of the game.
Each nation gets exactly one, at a new 1,000-point tier above its capstone.

```toml
[[tech]]
id = "nomad_way"
nation = "air"
cost = 1000
prereqs = ["sky_bison"]
school = "air_path"     # techs sharing a school exclude each other
```

- The Research screen draws both options side by side from tick 0, with what each gives and
  "Choosing one closes the other". Setting the target asks for a confirm.
- The sim rejects a closed tech with a reason, as it does for a locked building.
- One fork per nation, not many. A single clear fork gets remembered as "my Nomad game". A tree
  full of them reads as a puzzle to solve once.

### 2. Lost teachings: a draft from the ruins

Some techs are hidden until found. A **great ruin** (two or three per map, the largest of the
existing lost ruins) offers two teachings when searched. The player keeps one, and the other is
gone for that game.

- This is the `reveals` hook kept open on the scroll item. The offer is drawn from
  `rng::stream(seed, "research.teachings")`, and the pick is a `Command`, so it replays.
- A kept teaching appears in the tree as an ordinary tech. Researching it costs knowledge and
  uses up the teaching scroll.
- The pool is each nation's own two teachings plus two shared ones. Every game sees a different
  handful, which is the replay value the fixed tree lacks.
- Glide, the Map Room and a caravan's chart already find ruins, so three existing systems gain a
  bigger prize.

### 3. Costs beyond knowledge

Top-tier techs may also ask for items, hauled to a study hall like a build site's bill.

```toml
[[tech.cost_items]]
item = "tool"
count = 4
```

Knowledge alone stops being a trade once hands are idle. Scrolls, tools, metal and bricks are
things the gate and the caravans also want, so a 1,000-point school competes with the endeavor
for them. Only schools, teachings and exchange techs use this. The first 500 points of the tree
stay labour-only.

### 4. Conditions: research tied to what the settlement has done

A tech may require a fact about the settlement as well as other techs.

```toml
requires = { masters = 1 }    # or population, scrolls_read, winters_survived,
                              # chapter = "known_on_the_road", residents = ["earth"]
```

The endeavor's signs already count all of these, so the sim has the tallies. Conditions let the
tree respond to how the game was played, and they are what gates exchange techs behind the gate.

### 5. Exchange: the United City learns from its newcomers

After the gate, a settlement with enough residents of another nation can learn from them. There
are two kinds:

- **Borrowed techs.** Another nation's non-bending techs marked `teachable`, at double cost,
  while at least 3 of that nation live in the settlement. Brickmaking is teachable. Metalbending
  is not.
- **Fusion techs.** Six techs, one per pair of nations, that need residents of both. They are
  the Korra-era payoff the owner's era ceiling allows.

This is the "Later: neighbors and cultural exchange" section of research.md, moved from
neighbours to residents.

### Smaller changes that ride along

- **A new unlock kind, `upgrades`.** A tech can change a property of an existing building, with
  the same one-tech-per-item validation. Its first use is `writing`'s promised +2 study seats,
  which has been a comment since #17.
- **A second recipe per building.** `trade_wares` needs the Workshop to make tools or wares. The
  player sets it per building, which research.md avoided while every building had one job.
- **Scribing, the sink.** With the Archive built, scholars with no target copy scrolls: 150
  points become one scroll copy, a trade good and a caravan gift. The pool cap stays for
  settlements without one.
- **The branch sign stays as it is.** The gate's "branch complete" sign counts today's branch
  techs only, through a `core = true` flag. Schools and teachings never block the gate.

## Proposed technologies

The catalogue holds 42 techs: 8 shared, 8 nation techs, 8 school options, 10 lost teachings, 6
fusions and 2 examples of borrowed techs. One game sees about 14 of them before the gate. Costs
and numbers are starting values for the balance harness, in the existing tiers plus a new 1,000
tier.

### Shared tier: the settlement grows up (8)

These fill the stretch from minute 60 to the gate, where pacing.md measured the longest gaps.
Each one uses a system that exists or is already designed.

| Tech | Cost | Needs | What appears | The decision it asks |
|---|---|---|---|---|
| `paving` | 120 | `masonry` | **Paved Road**: 1 stone a tile, steps along it 1.3 times as fast. Rail stays 2.0 and Earth's | Stone is the gate's scarcest material. Which one route is worth 30 of it? |
| `tea_ceremony` | 120 | `herbalism`, `pottery` | **Tea House**: caravans camp longer and show 4 deals, not 3. A full house adds to draw | Herbs go to tea or to the sick. Built before or after the first caravan? |
| `calendar` | 250 | `writing` | **Almanac** on the Nation screen: minutes to first frost and to the next caravan. Farm plots gain a standing sow order | 250 points to remove a chore and see winter coming, or a branch tech now |
| `trade_wares` | 250 | `carpentry`, the first chapter met | The Workshop can make **wares** from wood. Caravans pay about twice as much for wares as for raw wood | Each Workshop makes tools or wares. It turns the wood glut into stone and ore |
| `herding` | 250 | `farming` | **Stable** with the nation's beast: ostrich horse (Earth), komodo rhino (Fire), buffalo yak (Water). It hauls a double load and eats from stores | Food for hands. Air skips it: the sky bison is theirs and better |
| `waystations` | 250 | `cartography` | **Waystation**: a small far stockpile with two beds and a water barrel. Crews near it eat, drink and sleep there | Where does the second site go: the far ore, the mild southern land, or the ruin? |
| `archives` | 500 | `writing`, 3 scrolls read | **Archive**: +2 study seats in reach, and scribing (idle knowledge becomes scroll copies to trade or gift) | Scholars keep researching or scribe for trade. It is the tree's own sink |
| `star_charts` | 500 | `calendar`, `cartography` | **Celestial days** on the Almanac: each nation's benders regain chi twice as fast for a few minutes on their day (full moon, solstice, equinox winds, the deep-earth day) | When to queue the big bends. After the gate, four nations mean four days to plan around |

### Earth Kingdom: Omashu's ingenuity, then a city

Earth is slow and weak at power. Its new techs move goods without walking and decide what kind
of city the walls are for.

| Tech | Cost | Needs | What appears | The decision it asks |
|---|---|---|---|---|
| `omashu_chutes` | 250 | `earth_rail` | **Chute**: a one-way stone slide. A stack dropped at the head arrives at the foot at once. The head must touch a mountain, mine or tunnel | One-way and fixed. It pairs with the Mine Shaft, so where the mine goes now matters twice |
| `badgermole_ways` | 500 | `tunneling`, `seismic_sense` | **Badgermole Den**: a badgermole bores a designated line of tunnel with no chi, slowly, and eats from stores | Chi now or food and time. Long tunnels through a whole range become a plan |
| School: `agrarian_ring` | 1,000 + 6 tools | `metalbending` | Land fully enclosed by Earth Wall is **sheltered**: no exposure inside, frost takes no crop, plots grow through winter | How much land to wall in, at stone per tile. Ba Sing Se: a broad farming city |
| School: `metal_city` | 1,000 + 8 metal | `metalbending` | **Metal rail**: rail runs at 3.0, may be laid in tunnels, and carries a power source's reach along its line | Ore goes to the gate or to the line. Zaofu: a deep mining city with power at last |

### Fire Nation: what is fire for?

Fire has the longest production chain and no food identity. Its fork is the one the tree already
hints at, where `dancing_dragon` and `steam_engines` both lead to the capstone.

| Tech | Cost | Needs | What appears | The decision it asks |
|---|---|---|---|---|
| `hothouses` | 250 | `brickmaking`, `farming` | **Hothouse**: plots in its reach grow through winter. It burns wood while they do | Wood for winter food. Fire's answer to frost, sized by the fuel pile |
| `cargo_balloons` | 500 | `steam_engines` | **Balloon Dock**, built in pairs: goods fly between two docks over any terrain. Each flight burns wood | Which two points on the map deserve a line. Fuel per trip, so not every haul |
| School: `sun_warriors` | 1,000 + 3 scrolls | `lightning_generation` | **Eternal Sun**: every flame is fuel-free, and people in a flame's reach mend and regain chi faster | Warmth as life: a settlement that never chops for fuel and heals at its hearths |
| School: `great_works` | 1,000 + 8 metal | `lightning_generation` | **Factory**: recipe buildings in a Lightning Station's reach run a slow batch with no crafter inside | Industry: hands freed for the gate, at the cost of metal and a tight yard |

### Water Tribes: shape the water, then choose a pole

Water is weak at heat and stone. Its new techs make a treeless coast liveable and let the tribe
redraw the shoreline.

| Tech | Cost | Needs | What appears | The decision it asks |
|---|---|---|---|---|
| `oil_lamps` | 120 | `fishing` | **Oil Lamp**: a small heat source that burns fish, not wood | Fish as food or as fuel. Polar sites with no trees become a real embark choice |
| `canals` | 250 | `irrigation`, `water_travel` | Ability **Carve Canal**: a land tile beside water becomes fresh shallow water. It irrigates, gives a drinking place and takes a Fishing Dock or Tide Mill | Permanent, and it cuts land routes. Bring the water to the fields, or the mill to the workshop? |
| School: `northern_city` | 1,000 + 6 tools | `spirit_oasis` | **Ice masonry**: cut ice pays any stone cost one for one, as brick does for Fire | A great ice city that needs no quarry, tied to the water's edge and its benders' chi |
| School: `southern_ways` | 1,000 + 3 scrolls | `spirit_oasis` | **Ranging camps**: Waystations cost nothing but work, crews based at one take no cold, and any shore can be fished | A spread-out tribe that lives off the whole coast and builds little |

### Air Nomads: lightness, then temple or sky

Air is weak at carrying and digging. Its new techs move people and fruit without labour, and its
fork is the question in its name.

| Tech | Cost | Needs | What appears | The decision it asks |
|---|---|---|---|---|
| `lemur_keeping` | 120 | `orchards` | **Lemur Roost**: lemurs pick ripe fruit in reach and carry it to stores | Fruit picked for free, but only fruit and only in reach. Where the groves go |
| `wind_gates` | 250 | `gliders`, `windmills` | **Wind Gate**, built in pairs: people glide between two gates over any terrain. Loads stay behind, so it pairs with Wind Carry | Which far place gets a gate: the quarry, the ruin or the second grove |
| School: `high_temple` | 1,000 + 40 stone | `sky_bison` | **High Temple**: a landmark that can't be moved. In its reach chi returns twice as fast and study runs half again faster | Settle. Stone is Air's hardest material, so the temple is the whole late game |
| School: `nomad_way` | 1,000 + 3 scrolls | `sky_bison` | **Pack up**: every building returns its full cost when taken down and comes down twice as fast. Bison carry a whole building's bill | Move. Follow fresh land and mild winters, and never let the ground tire |

### Lost teachings (10)

A great ruin offers two of these, and the player keeps one. Each costs 250 points and the
teaching scroll. Nation teachings appear only for that nation.

| Teaching | For | What appears | Why it is a hard pick |
|---|---|---|---|
| `pai_sho` | All | **Pai Sho Table**: the White Lotus sends one extra caravan a year, and request rewards double | Trade for the rest of the game, but nothing today |
| `spirit_library` | All | **Knowledge Seeker**: one study seat fills itself with a fox that needs no food or sleep | A free scholar, most valuable the earlier it is found |
| `sandbending` | Earth | **Sand Sailer**: desert tiles cost half to cross, and sand becomes a second source of stone | Only worth it if the map has desert nearby |
| `crystal_growing` | Earth | **Glow crystals** grow in tunnels: the richest ware a caravan takes | Rewards a settlement that tunnelled; nothing for one that didn't |
| `sun_stone` | Fire | **Sun Stone**: firebenders in its reach regain chi faster through spring and summer | Seasonal power, so winter plans can't lean on it |
| `hot_springs` | Fire | **Bathhouse**: must touch rock. Bathers shed cold and tiredness in one visit | The answer to exposure, tied to a placement rule |
| `plantbending` | Water | Ability **Regrow**: lifts the strain from a 3x3 patch of tired land | Makes the land near camp last, against anything that looks further out |
| `spirit_water` | Water | The Spirit Oasis yields one **vial** a year. A vial saves a settler who would have died | Insurance, which is worth most on Hard |
| `guru_teachings` | Air | **Chakras**: a master's bends cost 60% chi, not 80% | Pays only once masters exist |
| `spirit_projection` | Air | Ability **Project**: a meditating monk reveals every buried thing in a wide radius, whatever its kind | The one sense that crosses kinds, for a nation that can't dig what it finds |

### Exchange techs: after the gate

Fusions need at least 3 residents of each named nation (no masters condition, #955). Each costs
1,000 points plus tools. The home nation counts as one of the pair, so every game has three open to it
at first and all six once every nation has settled.

| Fusion | Nations | What appears | The decision it asks |
|---|---|---|---|
| `aqueducts` | Earth + Water | **Aqueduct**: a raised stone channel that carries fresh water and irrigation across any land | Stone for water anywhere: the inland farm city |
| `steelworks` | Earth + Fire | **Steelworks**: one ore makes one metal, not half | Ore stretches twice as far, if the city can feed the yard |
| `sky_lifts` | Earth + Air | **Sky Lift**: a cable lift up a mountain face. Goods and people cross a range without a tunnel | A range stops being a wall. Lift or tunnel? |
| `steam_power` | Fire + Water | A Steam Engine built on the bank burns no fuel and gains the Tide Mill's reach | Moves the industrial yard to the waterfront |
| `airships` | Fire + Air | **Airship Dock**: send your own caravan out. It leaves with goods and returns a season later with what you asked for | Trade on your schedule, with crew and goods away for a season |
| `cloudbending` | Water + Air | Ability **Call Rain**: a wide patch grows as if irrigated for a season | Chi from two nations' benders for one big harvest |

Borrowed techs reuse what exists. Two examples: a Water city with Fire residents learns
`brickmaking` for 240 points and stretches its stone. An Earth city with Air residents learns
`wind_turbines` and finally has real power, turned by the Air residents themselves.

## Keeping decisions interesting over a long game

The tree should be bigger than the road to the gate, so that what to leave unlearned is the
standing question. A nation's visible tree grows from about 2,400 points to about 7,000, and a
steady player should reach the gate knowing roughly two thirds of it.

That share is an estimate, not a measurement. It assumes three scholars at about 25 to 37 points
a minute from minute 130 (`writing`, two-thirds effective, with and without power), which gives
2,000 to 3,000 more points by a gate at 3.5 hours. The decision timeline in the balance harness
(#434) is the tool to check it.

| Stretch | Minutes | Research on offer | The question in the player's head |
|---|---|---|---|
| Survival | 0 to 30 | Tier 0 and the first branch tech, as today | What keeps us alive on this site? |
| Settling | 30 to 90 | The branch, plus `paving`, `tea_ceremony` and the 120-point nation techs | Branch first for the nation's tools, or shared techs for trade and roads? |
| Midgame | 90 to 150 | The 250 and 500 tier, the first great ruin, caravans | Which pressure do I buy off: winter, hauling, growth or stone? |
| The road | 150 to the gate | The school, more teachings, `archives`, `star_charts` | Do tools, metal and scrolls go to my school or to the gate? |
| United City | After the gate | Borrowed techs, fusions, scribing | Who do I want living here, and what can they teach us? |

Five things keep it from going stale or going wrong:

1. **No dead tier.** pacing.md's target is no gap longer than 15 minutes between new decisions.
   Every tech above unlocks something visible, so each completion counts on that timeline.
2. **Variety between games comes from three sources.** The school (2 per nation), the teachings
   (a different draft each map) and the site: `oil_lamps`, `sandbending`, `hot_springs` and
   `canals` are each worth a lot or nothing depending on where the settlement stands.
3. **Nothing new is required.** The gate's signs don't grow, power stays a speed-up, and a
   player who ignores every new tech can still finish. New techs make the road shorter or
   different, never possible.
4. **Automation arrives after the chore is learned.** The standing sow order, lemurs, the
   badgermole and the Factory all remove work the player has done by hand for an hour. Each has
   a running cost, so doing it by hand stays a choice.
5. **Nation identity is protected.** Roads are slower than rail, beasts are weaker than bison,
   and a borrowed tech costs double and needs residents. Another nation's answer is available
   late and at a price, which is the point of the United City.

Three risks to watch in the harness: schools that one option always wins (measure both per
nation), teachings that are dead on some maps (the draft should skip a teaching the site can't
use), and a food glut that the new food-eating techs don't dent.

## Build order

Six phases, ordered so that each ships something playable and the cheapest content comes first.
Phase 1 needs no new mechanic. Each phase has a design issue (see "Follow-up issues").

| Phase | What ships | New sim work | Waits on |
|---|---|---|---|
| 1. Fill the midgame | `paving`, `calendar`, `oil_lamps`, `lemur_keeping`, `hothouses`, `omashu_chutes`, and `writing`'s +2 seats | Small: road step cost (rail's rule at 1.3), the `upgrades` unlock kind, standing sow order, a heat source with a fuel item, a one-way haul link | Nothing. Hothouse reuses the Steam Engine's fuel bunker if #433 lands first |
| 2. Routes and outposts | `waystations`, `herding`, `wind_gates`, `cargo_balloons`, `canals`, `badgermole_ways` | Medium: paired-building links, a beast unit kind, a land-to-water reshape | Bison haul (#205) for the beast unit. Water travel (#201) for canals |
| 3. Trade techs | `tea_ceremony`, `trade_wares`, `archives` with scribing | Medium: a second recipe per building, scroll copies as an item | Caravans (#437) |
| 4. Schools | The `school` field, the 1,000 tier, item costs, the fork on the Research screen, 8 school techs | Medium: exclusion and validation, item delivery to a study hall, enclosure detection for the Agrarian Ring | Power (#428 family) for `metal_city` and `great_works` |
| 5. Lost teachings | Great ruins, the draft, hidden techs, 10 teachings | Medium: the `reveals` hook, a pick command, a draft modal | Mastery ranks (#358) for `guru_teachings` |
| 6. The United City learns | `requires` conditions, `teachable` flags, 6 fusions, `star_charts` | Large: residents as a condition, celestial days, outbound caravans for `airships` | The United City (#442), and a playtest of it |

Notes for the phase designs:

- Every new tech and building is appended to its TOML file, since ids are file positions and
  saves are checked against them (save-compat.md).
- Each phase ends with a harness run: the decision timeline, both schools per nation, and the
  food and wood stores at minutes 120 and 180.
- The Spirit Portal stays out of scope. `spirit_water`, `spirit_projection` and `star_charts` are
  the natural hooks into it when its own design opens.

## Owner decisions (2026-11-10)

The owner took every recommendation.

| # | Question | Owner's answer | Effect |
|---|---|---|---|
| 1 | May a school lock the other option out for the whole game? | **Yes, one permanent fork per nation** | Schools as written, shown from tick 0 |
| 2 | May top-tier research cost items as well as knowledge? | **Yes, top tier only** | Schools, teachings and fusions cost items. The first 500 points stay labour-only. Amends research.md's "time and labor, not materials" |
| 3 | Reopen cross-nation learning for the game after the gate? | **Yes, borrowed techs and fusions** | Phase 6 as written. Supersedes research.md's owner decision 2 (2026-10-22) for the post-gate game |
| 4 | Make the tree bigger than one game finishes before the gate? | **Yes, about two thirds learned** | The branch sign stays fixed at today's techs (`core = true`) |
| 5 | Offer lost teachings as a draft? | **Yes, pick 1 of 2** | Great ruins offer two teachings, and the other is lost |
| 6 | What do scholars do once the tree is done? | **Scribing** | The Archive turns idle knowledge into scroll copies. No repeatable studies |
| 7 | Beasts of burden for Earth, Fire and Water? | **Yes, weaker than bison** | `herding` as written. Air skips it |
| 8 | Celestial days through `star_charts`? | **Yes, but built last** | Ships in phase 6 |

## Follow-up issues

One design issue per phase, filed without `ready`. Each produces a design doc and its own
implementation issues.

| Phase | Issue |
|---|---|
| 1. Fill the midgame | #665 ([horizons-phase-1.md](horizons-phase-1.md)) |
| 2. Routes and outposts | #666 ([horizons-phase-2.md](horizons-phase-2.md)) |
| 3. Trade techs | #667 ([horizons-phase-3.md](horizons-phase-3.md)) |
| 4. Schools | #668 ([horizons-phase-4.md](horizons-phase-4.md)) |
| 5. Lost teachings | #669 ([horizons-phase-5.md](horizons-phase-5.md)) |
| 6. The United City learns | #670 ([horizons-phase-6.md](horizons-phase-6.md)) |
