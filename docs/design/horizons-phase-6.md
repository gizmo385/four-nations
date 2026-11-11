# Research horizons, phase 6: the United City learns

Doc status: current

Status: **proposed** (#670). This is phase 6 of [research-horizons.md](research-horizons.md)
(accepted 2026-11-10). It follows the shape of phases 1 to 4
([horizons-phase-1.md](horizons-phase-1.md), [horizons-phase-2.md](horizons-phase-2.md),
[horizons-phase-3.md](horizons-phase-3.md) and horizons-phase-4.md, #665 to #668) and reuses
their patterns: the `core` rule (#673), the `upgrades` and `features` unlock kinds (#674, #677),
fuel bunkers (#675), paired links (#690), techs for some nations (#696), phase 3's `requires`
table (#707) and phase 4's item bills (#718). It turns owner decisions 3 and 8 into a wider
`requires` table, a `teachable` flag, six fusion techs and celestial days, all in TOML. Phase 5
(lost teachings) is designed in parallel, and nothing here depends on it.

**Designed ahead of the United City.** The owner chose to design this before the United City
(#442) ships and before anyone has played it. The design follows #442's current spec (endgame.md,
"The transformation: the United City", and pacing.md, section 4). Every number that depends on
how the United City plays is marked *provisional*. A playtest (follow-up N) sets those numbers
before the phase-6 balance run. The three owner questions are under "Open questions for the
owner", and the implementation work is under "Follow-up issues".

## Decisions in brief

1. **Conditions can now fall.** `requires` gains `population`, `masters` (optionally narrowed by
   `masters_of`) and `residents`. All three can drop, so a tech now completes only when three
   things hold at once: its knowledge is banked, its bill is in, and its conditions are met. If a
   condition fails at that moment, the tech **waits**. It keeps its points and its items, and the
   study seats free up, the same way phase 4 handles a short bill.
2. **Borrowed techs are another nation's craft, not its bending.** A tech marked `teachable` can
   be learned by a settlement of another nation at 2x cost, while at least 3 of the tech's nation
   live there. Teachable techs unlock no ability. A borrowed tech's bending prereqs are waived:
   a Water city borrows `brickmaking` without learning `forging`'s Weld. The tech's buildings
   work for everyone, and its modifiers help only residents of the nation that taught it.
3. **Six fusions, one per pair of nations, open to every settlement.** A fusion names its two
   nations, costs 1,000 points plus tools, and needs 3 residents of each nation and a master of
   either. The home nation is usually one of the pair. A Fire city can still learn `aqueducts`
   once Earth and Water people live there.
4. **Each fusion is one new piece on top of an existing system.** That means a flowing channel
   network (`aqueducts`), a recipe building (`steelworks`), a pair that anyone rides with a load
   (`sky_lifts`), a bank upgrade (`steam_power`), outbound voyages (`airships`) and a joint bend
   for two casters of different nations (`cloudbending`).
5. **Celestial days refill the meter.** With `star_charts` known, each nation has one day a year
   in its own season. As the day begins, that nation's benders are topped up to full chi, and
   for its 5 minutes they regain chi at 2x. The Almanac counts down to each day. After the gate,
   four nations means four days a year to plan bends around. Whether the day refills the meter
   or only doubles regen is question 2.
6. **None of this is `core`, and the home tree gains only `star_charts`.** Borrowed techs and
   fusions appear on a second Research page, **From newcomers**, which opens when the gate is
   complete. `star_charts` is an ordinary shared tech.

## What ships

| Tech | Kind | Cost | Needs | Unlocks | The decision |
|---|---|---|---|---|---|
| `star_charts` | shared | 500 | `calendar`, `cartography` | **Celestial days** on the Almanac | When to save chi and queue the big bends |
| `aqueducts` | fusion: Earth + Water | 1,000 + 6 tools | `masonry`, `farming` | **Aqueduct**: a raised stone channel that carries fresh water to drink and irrigates along its length | Stone for water anywhere. Where does the inland farm go? |
| `steelworks` | fusion: Earth + Fire | 1,000 + 8 tools | `prospecting` | **Steelworks**: one ore makes one metal, not half | Ore goes twice as far, if the yard has wood and a crafter |
| `sky_lifts` | fusion: Earth + Air | 1,000 + 6 tools | `masonry`, `carpentry` | **Sky Lift**, built in pairs against rock: people ride across a range, loads and all | Lift or tunnel? Which range is in the way? |
| `steam_power` | fusion: Fire + Water | 1,000 + 6 tools | `steam_engines` | A Steam Engine on the bank burns no fuel and reaches as far as a Tide Mill | Move the yard to the waterfront |
| `airships` | fusion: Fire + Air | 1,000 + 8 tools | `cartography`, population 24 | **Airship Dock**: send your own caravan out and get goods back a season later | Trade on your own schedule, with two people and the goods away for a season |
| `cloudbending` | fusion: Water + Air | 1,000 + 4 tools | `farming` | Joint bend **Call Rain**: plots in a wide patch grow as if irrigated for a season | Chi from two nations' benders for one big harvest |
| *borrowed* | another nation's `teachable` techs | 2x their cost | 3 residents of that nation | What the tech unlocks at home | Who do we want living here, and what can they teach us? |

The six fusions are 6,000 points and 38 tools. Borrowed techs add 1,240 (Earth's) to 2,480
(Fire's) points per teaching nation ("Which techs are teachable"). Nobody learns all of it in a
United City session, which is what owner decision 4 asks for: what to leave unlearned stays the
question.

## Designed ahead of the United City

What this design assumes about the game after the gate, and where each assumption comes from:

| Assumption | Source | Status |
|---|---|---|
| Newcomers of every nation arrive after the gate, drawn by weight: own 0.4, each other 0.2 | #240, `data/needs.toml [growth]` | Shipped |
| One newcomer every 10 minutes at 100% draw, between 60% and 140% | #353, #435 | #435 not yet shipped |
| Caravans come from all four nations, with deals scaled 1.5x and bigger requests whose reward is `draw` | #442, pacing.md section 4 | Not yet shipped |
| Population milestones at 24 and 32 | #442 | Not yet shipped |
| A newcomer works with its own nation's modifiers, casts its own signature ability, and ranks against its own nation's thresholds | #239, #240, #360 | Shipped |

**Residents arrive slowly.** At 100% draw the city gains 6 people an hour, so about 1.2 of each
other nation. Three of one nation takes about 2.5 hours, or 1.8 hours at the 140% ceiling. That
is too slow for the exchange to matter in a session of an hour or two. Question 1 proposes a fix:
let the player steer arrivals through United City caravan requests. With that fix and its
weights, three of a chosen nation take about 65 minutes. All of these numbers are *provisional*.

**What the playtest settles** (follow-up N): how long a United City session runs, how fast each
nation's residents arrive, how many research points a minute the city makes after the gate, and
whether 3 residents, 2x cost and 1,000-point fusions fit that session. Every number marked
*provisional* below goes through that issue before the balance run (follow-up O). The sim work
can start earlier, because every one of these numbers lives in data.

## The `core` rule, unchanged

`star_charts` and the six fusions leave `core` at its default, false (horizons-phase-1.md, "The
rule every phase-1 tech follows"). Borrowed techs are never core for the settlement that borrows
them, because the branch sign counts the settlement's own nation's core techs (#673). Load
refuses `core = true` on a fusion. A core tech may not carry the three new conditions either,
so the gate's branch sign never waits on who lives in the settlement. The unit test from #673
that pins each nation's two targets runs again with phase-6 data.

## Shared sim work

### 1. Conditions: the rest of `requires`

Phase 3 shipped `requires` with two keys and `deny_unknown_fields` so that phase 6 could add the
rest (horizons-phase-3.md, "Research conditions"). Phase 6 adds three keys, plus `masters_of`,
which narrows `masters`:

```toml
[[tech]]
id = "aqueducts"
requires = { residents = { earth = 3, water = 3 }, masters = 1, masters_of = ["earth", "water"] }

[[tech]]
id = "airships"
requires = { residents = { fire = 3, air = 3 }, masters = 1, masters_of = ["fire", "air"], population = 24 }
```

| Key | Value | What it counts | Can it fall? | Since |
|---|---|---|---|---|
| `scrolls_read` | 1 to 20 | The scrolls tally | No | Phase 3 |
| `chapter` | a `[[chapter]]` id | Chapter met | No | Phase 3 |
| `population` | 1 to 200 | **Residents**: units on the roster that are neither guests (the Avatar) nor beasts. Airship crew away on a voyage still count | Yes: deaths and leaving | Phase 6 |
| `masters` | 1 to 20 | Residents at `Rank::Master` by their own nation's thresholds (`World::rank_of`) | Yes: a master can die. Ranks never drop | Phase 6 |
| `masters_of` | 1 to 4 distinct nations | Narrows `masters` to residents of these nations. Needs `masters` | | Phase 6 |
| `residents` | a table, nation to 1 to 50 | Residents whose `World::nation_of` is that nation, home nation included | Yes | Phase 6 |

- **Left out: `winters_survived`.** The spec lists it, but no tech in any phase asks for it. It
  can be added as a key later, without a schema break.
- **Facts.** Phase 3's `Facts` struct gains `population: u16`, `masters: [u16; 4]` and
  `residents: [u16; 4]`, indexed by nation. The world fills it once a tick before the research
  tick, by walking the units: a few dozen rank checks, which costs nothing.
- **Queueing.** `set_target` still refuses a queue that holds a tech with an unmet condition,
  with `ResearchError::NotYet` (phase 3). The new keys read `needs 24 people`, `needs a Fire or
  Air master` and `needs 3 Earth residents (2 live here)`.
- **Completing.** `Research::tick` completes the front tech when three things hold at once: the
  pool covers its scaled cost, its bill is full (#718), and every condition still holds.
- **Waiting.** If the knowledge and the bill are in but a condition has fallen, the tech waits
  at the front of the queue. It is not dropped. It keeps its banked points and delivered items,
  and the study seats are not claimed, exactly as phase 4 does while a bill is short. The
  Research row reads `Waiting: 3 Fire residents (2 live here)`. A newcomer, or a new master,
  finishes it on the next tick. Knowledge keeps flowing into the pool until the cost is covered,
  so a condition that falls early costs nothing. A tech further back in the queue is not
  checked until it reaches the front.
- **Once learned, always known.** A tech never unlearns because a condition fell afterwards.
  For borrowed techs, whether that is right is question 3.
- **Validation.** Ranges as in the table. `residents` names nation ids, each at most once.
  `masters_of` needs `masters`. A `core` tech may not carry `population`, `masters` or
  `residents`. Everything else is as phase 3 has it.

### 2. Borrowed techs: `teachable`

```toml
# data/research.toml, edited in place on each teachable tech (ids and positions don't move)
[[tech]]
id = "brickmaking"
teachable = true

# data/research.toml [settings], two new keys
borrow_cost = 2.0          # provisional: a borrowed tech costs this many times its own cost
borrow_residents = 3       # provisional: residents of the tech's nation needed to borrow it
```

- **Who may borrow.** A settlement may target a teachable tech of another nation while at least
  `borrow_residents` people of that nation live there. That is an implicit
  `residents = { <nation> = 3 }` condition, so it uses all of section 1: refused at queueing,
  checked at completion, and waiting if it falls. Before the gate every newcomer is of the home
  nation, so nothing can be borrowed, and the pre-gate game doesn't change.
- **Cost.** `cost × borrow_cost`, then the difficulty preset's `research_cost`, rounded as
  today. `brickmaking` becomes 240 and `steam_engines` 500. Borrowed techs have no item bill:
  `cost_items_from` (#718) compares the def's own `cost`, and no teachable tech reaches 1,000.
- **Prereqs, with bending waived.** A borrowed tech needs its shared prereqs and its teachable
  prereqs. A prereq of its own nation that isn't teachable is waived, because newcomers bring the
  craft without the bending. `brickmaking` needs `pottery` (and `forging`, which unlocks Weld,
  is waived). `steam_engines` needs `smelting` and `brickmaking`, both teachable, so both are
  borrowed first. `Defs::borrow_prereqs(tech)` answers it, and `set_target` queues missing
  teachable prereqs as borrowed techs, in file order.
- **Modifiers stay with the teachers.** A borrowed tech's buildings, upgrades and features work
  for the whole settlement. Its `modifiers` apply only to residents of the tech's own nation.
  The rule in `research::effective_modifiers` becomes this: apply a known tech if it is shared,
  of the settlement's nation (today's rule, which already lets newcomers ride on the home tree,
  #239), or of the unit's own nation. The settlement's cached `World::modifiers` therefore never
  includes a borrowed tech, so reaches and power that scale by `power_generation` stay the home
  nation's. A Fire city that borrows `earth_rail` lays rail, and only its Earth residents carry
  1.25x. With no borrowed tech known, every value is bit-for-bit what it is today.
- **Validation.** `teachable` needs a single `nation`: not a shared tech, not `nations` (#696,
  so `herding` stays off Air, per owner decision 7), and not a fusion. A teachable tech unlocks
  no `abilities` and is no school's tech (#717). Phase 5's teachings are phase 5's call. This
  design marks none of them.
- **Saves.** Nothing new is saved. Whether a known tech was borrowed follows from its nation and
  the world's.

#### Which techs are teachable

The rule is "unlocks no ability". Capstones stay home, although `lightning_generation` and
`sky_bison` unlock none, because the spec protects each nation's identity (research-horizons.md,
"Keeping decisions interesting", point 5). Follow-up B flags the shipped techs and any phase 1
or 2 tech that has already landed. A later phase's follow-up sets the flag as it appends its
tech.

| Nation | Teachable (2x cost) | Kept home, and why |
|---|---|---|
| Earth | `earthen_walls` (240), `earth_rail` (500), `omashu_chutes` (500) | `earth_shaping`, `tunneling`, `seismic_sense`, `metalbending`: abilities. `badgermole_ways`: the badgermole works with earthbenders, and tunnels are Earth's |
| Fire | `brickmaking` (240), `smelting` (240), `steam_engines` (500), `hothouses` (500), `cargo_balloons` (1,000) | `forging`, `dancing_dragon`: abilities. `lightning_generation`: the capstone, fed by firebenders' lightning |
| Water | `healing` (240), `tide_mills` (500), `water_travel` (500), `oil_lamps` (240) | `irrigation`, `ice_construction`, `canals`: abilities. `spirit_oasis`: the capstone |
| Air | `orchards` (50), `wind_turbines` (500), `lemur_keeping` (240), `wind_gates` (500) | `meditation`, `gliders`, `windmills`, `air_scooter`: abilities. `sky_bison`: the capstone |

Earth teaches the least, because most of its tree is bending. It still teaches rail, which is
the one hauling answer every other nation lacks. An Earth city that borrows `wind_turbines`
builds turbines at its own `power_generation` (0.8), so they reach 2 tiles, not Air's 9. That is
the spec's "available late and at a price".

### 3. Fusions: the `fusion` field

```toml
[[tech]]
id = "steelworks"
fusion = ["earth", "fire"]     # exactly two nations; every settlement sees it after the gate
cost = 1000
prereqs = ["prospecting"]
requires = { residents = { earth = 3, fire = 3 }, masters = 1, masters_of = ["earth", "fire"] }
[[tech.cost_items]]
item = "tool"
count = 8
```

- **Who sees it.** Every settlement, once its gate is complete, on the From newcomers page
  (mockups). Fusions don't use `nation` or `nations`. Those fields say who can ever see a tech,
  and a fusion is open to anyone with the right residents.
- **The conditions are written out.** A fusion's `requires.residents` names exactly its two
  nations, and `masters_of` names one or both of them. Load checks it. The condition is plain
  data, so the playtest can lower one fusion's numbers without touching the others. The 3
  residents and 1 master are *provisional*.
- **Prereqs.** Shared techs, or teachable techs of either of its two nations. `steam_power`
  needs `steam_engines`. A Fire or Water city with Fire residents gets there: Fire knows it, and
  Water borrows it for 500.
- **Item bills.** All six have `[[tech.cost_items]]` in tools, delivered to a study hall like a
  school's (#718). The counts are *provisional*.
- **Validation.** `fusion` lists two distinct nations. It excludes `nation`, `nations`,
  `school`, `teachable` and `core`. An ability a fusion unlocks must have `nations` equal to the
  fusion's (section 4).

### 4. Joint bends: an ability for two nations

`cloudbending` needs one new bending rule: a bend that a Water bender and an Air bender channel
together.

```toml
[[ability]]
id = "call_rain"
nations = ["water", "air"]    # in place of `nation`: a joint bend, one caster of each nation
```

- **Slots.** A joint bend's job has one slot per nation. A unit claims a slot if it is of that
  nation and passes the ability's `rank` and chi checks. The two slots are claimed separately,
  each by the nearest able unit, as bend claims work today.
- **Channel.** Progress runs only while both casters are in place at the target, at the fixed
  `work` time. Joint bends have no `speed_modifier`, because two casters would have two of them.
  If one caster leaves, for a need or because it was cut off, progress holds, and the slot opens
  for another unit of that nation.
- **Chi and credit.** Each caster pays the full `chi` (a master pays `master_chi` of it) when the
  bend completes. Each gets one finished bend and one form toward rank.
- **Unlocking it.** `nations` is allowed only on an ability that a fusion with the same two
  nations unlocks. "An ability can only be unlocked by a tech of its own nation" becomes "of its
  own nation, or a fusion of its two". Out of scope stays out of scope: casters are always of the
  ability's own nations, so no bending crosses nations.
- **Without both.** A designated Call Rain with no able Air bender waits, with the status
  `Needs an Air bender`. The bend picker lists it with both counts (mockups).

### 5. Celestial days

```toml
# data/seasons.toml, a new table. Read only while `celestial_days` is known, whatever the
# difficulty preset's `seasons` flag: the calendar runs either way (world/season.rs)
[celestial]
warning_minutes = 2.0      # the "in 2 minutes" alert

[[celestial_day]]
id = "equinox_winds"
name = "The equinox winds"
nation = "air"
season = "spring"          # a [[season]] id
at_minutes = 5.0           # into the season
minutes = 5.0              # provisional
refill = 1.0               # provisional: benders are topped up to this share of the meter as it begins
chi_regen = 2.0            # multiplies the bender's chi regen while it lasts

[[celestial_day]]
id = "summer_solstice"
name = "The summer solstice"
nation = "fire"
season = "summer"
at_minutes = 5.0
minutes = 5.0
refill = 1.0
chi_regen = 2.0

[[celestial_day]]
id = "harvest_moon"
name = "The harvest moon"
nation = "water"
season = "autumn"
at_minutes = 5.0
minutes = 5.0
refill = 1.0
chi_regen = 2.0

[[celestial_day]]
id = "deep_earth_day"
name = "The deep-earth day"
nation = "earth"
season = "winter"
at_minutes = 5.0
minutes = 5.0
refill = 1.0
chi_regen = 2.0
```

- **When.** Each day falls at a fixed point in its season, from minute 5 to minute 10 of 15. A
  day is a pure function of the tick and the data, like a season (`World::celestial_day(defs)
  -> Option<&CelestialDayDef>` for the day running now, and `next_celestial(nation)`). Nothing
  is saved.
- **Who.** Residents of the day's nation (`nation_of`) who aren't guests. The Avatar's meter is
  his own and stays out of it. Beasts have no chi.
- **The rise.** On the day's first tick, if `celestial_days` is known, every such unit's chi is
  set to at least `refill × max`. A unit already above that keeps its chi, so the refill is worth
  most to benders who emptied their meters before the day.
- **The day.** While it lasts, those units regain chi at `chi_regen` times their usual rate,
  awake or asleep. This multiplies the existing `chi_regen` modifier in the needs tick, read per
  unit by its nation. Today that tick reads the settlement's cached value for everyone.
- **Learned mid-day.** No refill that day. The 2x regen applies for the rest of it.
- **Events.** `SeasonEventKind` gains `CelestialWarning { day }` (`warning_minutes` before) and
  `CelestialRise { day, units }`. Both fire only while the feature is known, and only if at
  least one resident is of the day's nation. They feed alerts: `The harvest moon in 2 minutes`
  and `The harvest moon. Waterbenders' chi floods back (6)`.
- **The Almanac.** Phase 1's `Almanac` gains `celestial: Vec<(CelestialDayId, Timing, u16)>`:
  the running day with its time left, then each other day whose nation has a resident, soonest
  first, with that nation's bender count. Before the gate that is one line. After it, it is up
  to four.
- **Why seasons, and one day each.** Each nation gets one day a year in a different season, so
  the four never overlap and none is worth more than another. Air rides the spring wind. Fire has
  midsummer. Water has the brightest full moon. Earth has midwinter, when its benders raise Earth
  Tents for crews caught in the cold.
- **Validation.** `season` names a `[[season]]`. `at_minutes + minutes` fits inside the season.
  `refill` is 0 to 1, `chi_regen` is 1 to 4, `minutes` is 1 to the season's length, and each
  nation has at most one day.
- **Numbers.** Chi is 100 at full. Regen is 2 a minute awake and 8 asleep, about 4 a minute
  over a day, so the refill is worth up to 25 minutes of regen per bender. The 2x adds about 20
  more. A Water city with 8 benders who drain their meters before the harvest moon gets about
  950 chi on the day, which is about 16 Carve Canal tiles (60 chi each). Without the refill,
  the day is worth about 160, which nobody would plan around (question 2).

### 6. Smaller pieces

- **Features.** `celestial_days` joins #677's `Feature` enum.
- **Upgrade property `bank`.** For `steam_power`, "`steam_power`" below.
- **Effect kind `rain`.** For `cloudbending`, "`cloudbending`" below.
- **Rain, canal, channel and aqueduct don't stack.** `World::crop_growth_rate` takes the best
  boost in reach across the Irrigation Channel, a canal (#695), an aqueduct and rain, as the
  channel's rule already does.
- **The From newcomers list.** `World::exchange(defs) -> Exchange` returns each other
  nation's teachable techs with their borrowed cost and state, plus the six fusions. It is empty
  until the gate is complete. The TUI and the agent surface both read it.

## The techs

Numbers are starting values for the harness. *Provisional* marks the ones the playtest sets.
Hover text means the build modal's effects list and the cursor panel's inspect lines, in the
voice of text-voice.md.

### `star_charts`: celestial days

```toml
# data/research.toml, appended
[[tech]]
id = "star_charts"
name = "Star Charts"
description = "Chart the moon, the sun and the winds, and know the days a bender's chi runs high."
cost = 500
prereqs = ["calendar", "cartography"]
[tech.unlocks]
features = ["celestial_days"]
```

- **Rule.** Shared work 5. Every nation can learn it before the gate, and then it is one day a
  year. After the gate it is up to four.
- **Hover.** The Research detail reads `Unlocks: Celestial days (on its nation's day, a bender's
  chi refills and returns 2x for 5 minutes)`.
- **The decision.** 500 points, about a branch capstone's worth, for one planned surge a year
  before the gate. It is worth the most to a nation whose benders are its labour: Earth with
  tunnels and fords, Water with canals and ice. It asks the player to plan around it: run the
  meters down before the day, and have the bends designated when it rises.

### `aqueducts`: the Aqueduct (Earth + Water)

```toml
# data/research.toml, appended
[[tech]]
id = "aqueducts"
name = "Aqueducts"
description = "Earth raises the channel and Water fills it. The river goes where the fields are."
fusion = ["earth", "water"]
cost = 1000
prereqs = ["masonry", "farming"]
requires = { residents = { earth = 3, water = 3 }, masters = 1, masters_of = ["earth", "water"] }
[[tech.cost_items]]
item = "tool"
count = 6
[tech.unlocks]
buildings = ["aqueduct"]

# data/buildings.toml, appended
[[building]]
id = "aqueduct"
name = "Aqueduct"
verb = "Raise"
work = 12.0
passable = true            # raised on arches: people walk under it
[building.aqueduct]
source = ["river", "lake"] # follow-up D adds "canal" once #695 has landed
irrigation_radius = 2      # Chebyshev, from each flowing tile: the canal's own number
growth_boost = 1.5         # the Irrigation Channel's
[[building.cost]]
item = "stone"             # brick stands in, as for any stone cost
count = 3
```

- **Flowing.** A finished aqueduct tile that is 4-adjacent to a `source` terrain is flowing, and
  so is any finished aqueduct tile 4-adjacent to a flowing one. One flood over the aqueduct
  tiles finds them. The result is a bitmap, derived and never saved, rebuilt when an aqueduct
  finishes or comes down and after load. This is the pattern of phase 4's sheltered flood, only
  far smaller. An aqueduct cut off from its source runs dry, with the status `Dry: no river
  upstream`.
- **What flowing does.** A flowing tile is a drinking place: a thirsty unit drinks standing
  beside it, as at a river. It also irrigates farm plots within `irrigation_radius`, under the
  best-boost rule. It is not a bank, so the Fishing Dock and Tide Mill still need real water.
- **Placement.** 1x1, laid in lines with the build tool's rectangle, like rail. It goes on
  buildable land only, so it never crosses rock or water. It is passable, so a line of it never
  blocks a route or a cut-off check.
- **Numbers.** A 20-tile line costs 60 stone and about 4 minutes of one builder. It irrigates a
  band 5 tiles wide and 20 long, and gives drinking water along its whole length. Water's canal
  irrigates the same band for 60 chi a tile but cuts the land in two. The Irrigation Channel
  irrigates a 7x7 for 4 wood but brings no water to drink.
- **Hover.** `Carries river water along its line`, `Irrigates plots within 2 tiles` and `People
  drink beside it`. Inspect: `Flowing: 20 tiles from (40, 61)` or `Dry: no river upstream`.
- **The decision.** Stone for water anywhere. Thirst trips set how far from a river a
  settlement can spread. An aqueduct lets the city grow inland, along a line the player
  draws, at 3 stone a tile, and stone is what the city's walls and the requests also want.

### `steelworks`: the Steelworks (Earth + Fire)

```toml
# data/research.toml, appended
[[tech]]
id = "steelworks"
name = "Steelworks"
description = "Earth reads the ore and Fire holds the heat. Nothing is lost in the slag."
fusion = ["earth", "fire"]
cost = 1000
prereqs = ["prospecting"]
requires = { residents = { earth = 3, fire = 3 }, masters = 1, masters_of = ["earth", "fire"] }
[[tech.cost_items]]
item = "tool"
count = 8
[tech.unlocks]
buildings = ["steelworks"]

# data/buildings.toml, appended
[[building]]
id = "steelworks"
name = "Steelworks"
verb = "Build"
work = 150.0
passable = false
footprint = [3, 2]
capacity = 1
uses_power = true
[[building.cost]]
item = "stone"
count = 16
[[building.cost]]
item = "metal"
count = 2
[[building.cost]]
item = "tool"
count = 2
[building.recipe]
output = "metal"
output_count = 1
work = 20.0
modifier = "heating"
input = [{ item = "gold_ore", count = 1 }, { item = "wood", count = 2 }]
stock_limit = 8
```

- **Rule.** An ordinary recipe building (#124), with no new code. The Smelter makes 1 metal from
  2 ore and 1 wood. The Steelworks makes 1 metal from 1 ore and 2 wood: half the ore, twice the
  wood.
- **Who works it.** Any crafter, at their own `heating`. A Fire resident works it at 2.5x, an
  Earth one at 1.0x. In a United City, who you have decides how fast it runs.
- **Numbers.** At `heating` 1.0, 3 metal a minute for 3 ore and 6 wood. Powered, it runs 1.5x
  to 2x. The stock limit of 8 sits above the Smelter's 3, because after the gate ore no longer
  competes with the ore sign.
- **Hover.** `Makes 1 metal from 1 ore`, `Burns 2 wood a batch`. Inspect as any recipe
  building.
- **The decision.** Ore stretches twice as far for any city with a crafter and the wood to feed
  it. Metal is what the late buildings ask for: the Lightning Station, Metal Rail, the Airship
  Dock and the Deep Mine. For Earth, which metalbends ore in place, the Steelworks trades
  benders' chi for a crafter's hours.

### `sky_lifts`: the Sky Lift (Earth + Air)

```toml
# data/research.toml, appended
[[tech]]
id = "sky_lifts"
name = "Sky Lifts"
description = "Earth anchors the towers in the rock and Air keeps the cars aloft."
fusion = ["earth", "air"]
cost = 1000
prereqs = ["masonry", "carpentry"]
requires = { residents = { earth = 3, air = 3 }, masters = 1, masters_of = ["earth", "air"] }
[[tech.cost_items]]
item = "tool"
count = 6
[tech.unlocks]
buildings = ["sky_lift"]

# data/buildings.toml, appended
[[building]]
id = "sky_lift"
name = "Sky Lift"
verb = "Raise"
work = 80.0
passable = true            # the unit steps on to board
beside = ["mountain"]      # each end stands against rock
[building.pair]
reach = 32
min_tiles = 8
[building.lift]
seconds = 4.0              # a ride takes 4 s plus 0.25 s a tile
seconds_per_tile = 0.25
[[building.cost]]
item = "wood"
count = 12
[[building.cost]]
item = "stone"
count = 6
[[building.cost]]
item = "metal"
count = 2
```

- **Rule.** A pair (#690), with both ends against rock and the same-region rule unchanged. Any
  unit rides, loaded or not. It uses #691's route comparison without the empty-hands condition:
  walk to A, plus the ride, plus walk from B, against the direct walk. Build deliveries, refuels
  and meals ride as well, because a lift is only a shorter path. Beasts don't ride.
- **Numbers.** A 32-tile ride takes 12 s. On most maps the walk around a range is two to four
  times the straight line (#53's fords), so a lift across one saves most of that trip for every
  hauler who uses it.
- **Against the others.** Air's Wind Gate carries people only, up to 48 tiles, anywhere. Fire's
  Balloon Dock carries goods only and burns wood. Earth's tunnel is permanent, costs chi a tile
  and opens the rock to mining. The lift carries both, burns nothing, and is shorter, fixed to
  rock at both ends.
- **Hover.** `Carries people and their loads to its partner up to 32 tiles away` and `Both ends
  stand against rock`. Inspect: `Partner: (52, 18), 27 tiles` and `Riders: 140`.
- **The decision.** Lift or tunnel? A tunnel takes an Earth bender's chi a tile but becomes
  ground. A lift costs a pair of buildings and works on day one. For a city without Earth's own
  bending, the lift is the only way across the range.

### `steam_power`: steam on the bank (Fire + Water)

```toml
# data/research.toml, appended
[[tech]]
id = "steam_power"
name = "Steam Power"
description = "Water feeds the boiler and Fire keeps it hot. Engines on the bank never run dry."
fusion = ["fire", "water"]
cost = 1000
prereqs = ["steam_engines"]
requires = { residents = { fire = 3, water = 3 }, masters = 1, masters_of = ["fire", "water"] }
[[tech.cost_items]]
item = "tool"
count = 6
[tech.unlocks]
[[tech.unlocks.upgrades]]
building = "steam_engine"
bank = { beside = ["water", "river", "lake"], radius = 4 }   # the Tide Mill's radius and banks
```

- **The `bank` upgrade.** It is a new upgrade property, and the target must have
  `[building.power]`. A finished building of the target whose footprint ring touches a `beside`
  terrain is **on the bank**. On the bank it burns no fuel: no Refuel haul is posted, as with
  phase 4's `fuel_free`. Its base power radius becomes the larger of its own and `radius`,
  before `power_generation` scales it. Engines away from the bank don't change. Follow-up F adds
  `"canal"` to `beside` once #695 has landed.
- **Fuel.** The fuel half needs the Steam Engine to burn fuel, which is #433. If #433 hasn't
  landed when F starts, F ships the reach half and adds the fuel half with #433.
- **Numbers.** A Fire city with its whole branch (`power_generation` 2.44) reaches 10 tiles
  from an engine on the bank, against 7 inland, the same as its Lightning Station. A Water city's
  engine on the bank reaches 5 (4 × 1.25), as its Tide Mill does, because a borrowed tech's
  `power_generation` helps only the teachers. That city must first borrow `steam_engines` (500
  points, Fire residents), so `steam_power` costs it 1,500 points in all.
- **Hover.** The build modal's Steam Engine entry gains `On the bank: burns nothing, reaches 4
  tiles`. Inspect: `On the bank: no fuel`.
- **The decision.** Move the industrial yard to the waterfront. A Fire city's yard usually sits
  by its wood. Steam power pays it to rebuild by the river, where the Tide Mill's banks already
  are, and the wood goes to the Steelworks and the airship instead.

### `airships`: the Airship Dock (Fire + Air)

```toml
# data/research.toml, appended
[[tech]]
id = "airships"
name = "Airships"
description = "Fire lifts the hull and Air steers it. Our own traders go out to the world."
fusion = ["fire", "air"]
cost = 1000
prereqs = ["cartography"]
requires = { residents = { fire = 3, air = 3 }, masters = 1, masters_of = ["fire", "air"], population = 24 }
[[tech.cost_items]]
item = "tool"
count = 8
[tech.unlocks]
buildings = ["airship_dock"]

# data/buildings.toml, appended
[[building]]
id = "airship_dock"
name = "Airship Dock"
verb = "Build"
work = 200.0
passable = false
footprint = [3, 3]
[building.airship]
crew = 2                   # residents who fly with it, away for the voyage
hold = 60                  # most items a voyage carries out
deals = 2                  # at most this many deals from one nation's caravan list
away_minutes = 15.0        # provisional: one season
fuel_per_voyage = 20
[building.fuel]
item = "wood"
bunker = 20
[[building.cost]]
item = "wood"
count = 30
[[building.cost]]
item = "metal"
count = 4
[[building.cost]]
item = "tool"
count = 2
```

- **A voyage.** `Command::PlanVoyage { dock, nation, deals: Vec<(u8, u8)> }` picks one nation,
  your own included (kin), and up to `deals` of that nation's caravan deals (#437's
  `data/trade.toml`, scaled by #442's `united_scale`), each up to its `times`. The give side
  must fit in `hold`. Haulers bring it to the dock, as they bring a deal's goods to a camp. When
  it is all in and the bunker holds `fuel_per_voyage`, a Crew job opens for `crew` residents of
  any nation. When they board, the airship sails. The crew and the goods leave the map.
- **Away.** For `away_minutes` the crew's needs are frozen. They can't die or leave, and they
  still count as residents and toward `population` (section 1). The dock reads `Away: back in 9
  min`.
- **Home.** The airship lands. The crew step off at the door, and the get side spills beside the
  dock as stacks with ordinary Haul jobs, as at a caravan camp.
- **Cancel.** `Command::CancelVoyage { dock }` works before sailing. Loaded goods drop beside
  the door. Taking the dock down is refused while the airship is away (`The airship is away`).
- **Determinism.** No RNG. The landing tick is fixed at departure. The deals are the list's own,
  in file order, not the visiting caravan's draw.
- **Why `population = 24`.** Two people away for a season is a lot for a small city. 24 is
  #442's first population milestone (*provisional*).
- **Numbers.** A voyage that takes Earth's wood-for-ore deal twice (pacing.md) carries 50 wood
  out and brings 6 gold ore back, for 20 wood of fuel and two settlers' season. Caravans come
  to the city every 15 minutes, so the airship adds about one trade a season, chosen by the
  player, from the nation the player picks.
- **Hover.** `Send two people with goods to trade, and back in a season` and `Burns 20 wood a
  voyage`. Inspect: `Loading for Earth Kingdom: 32/50`, `Away: back in 9 min`, or `Docked`.
- **The decision.** Trade on your own schedule. A caravan brings what its nation sells when it
  comes. An airship gets ore from the Earth Kingdom when the city needs it, at the cost of two
  pairs of hands and a hold of goods for a season.

### `cloudbending`: Call Rain (Water + Air)

```toml
# data/research.toml, appended
[[tech]]
id = "cloudbending"
name = "Cloudbending"
description = "Water lifts the sea into the sky and Air carries the cloud inland."
fusion = ["water", "air"]
cost = 1000
prereqs = ["farming"]
requires = { residents = { water = 3, air = 3 }, masters = 1, masters_of = ["water", "air"] }
[[tech.cost_items]]
item = "tool"
count = 4
[tech.unlocks]
abilities = ["call_rain"]

# data/bending.toml, appended
[[ability]]
id = "call_rain"
nations = ["water", "air"]
name = "Call Rain"
verb = "Call rain"
description = "A waterbender and an airbender bring a cloud over the fields. Crops under it grow as if irrigated for a season."
chi = 70.0                 # paid by each caster
work = 30.0
rank = "adept"
[ability.effect]
kind = "rain"
radius = 6                 # Chebyshev from the target: a 13x13 patch
growth_boost = 1.5         # the Irrigation Channel's; the best boost in reach applies
minutes = 15.0             # one season
```

- **The `rain` effect.** On completion, a `Rain { center, radius, boost, until_tick }` joins
  `World::rains` (serde default empty, since a save mid-rain must keep it). Farm plots in the
  patch take `boost` under the best-boost rule until `until_tick`. Rain over a patch that is
  already raining replaces the old rain and doesn't stack with it. Winter growth is still the
  climate band's, so rain on a frozen field does nothing (the bend modal warns `Nothing grows
  here in winter`).
- **Joint.** Shared work 4: one Water adept and one Air adept, each paying 70 chi.
- **Numbers.** 169 tiles at 1.5x for 15 minutes, which is about 20 plots. An Irrigation Channel
  covers 49 tiles for good, for 4 wood. Call Rain is for a city without Water's channel, or for
  a field too big for one: a harvest pushed up a third for one season, for 140 chi.
- **Hover.** The bend modal: `Plots in a 13x13 patch grow 1.5x for a season`, `Needs a
  waterbender and an airbender together`. The map draws the patch with a rain tint while the
  bend tool is open or the cursor is inside.
- **The decision.** Chi from two nations' benders for one big harvest. It is worth the most
  timed to spring's 1.25x growth and to a season when the food sign or a request needs a lot.
  With celestial days, the equinox winds refill the Air half.

## Against the specialties

Each fusion must be weaker or narrower than the nation specialty it resembles, so that identity
holds (research-horizons.md, point 5).

| Fusion | Nearest specialty | Why the specialty still wins at home |
|---|---|---|
| Aqueduct | Water's canal and Irrigation Channel | The channel is 4 wood for a 7x7, and the canal costs no stone. The aqueduct pays stone a tile |
| Steelworks | Fire's Smelter, Earth's Metalbend | It eats twice the wood, and its speed follows the crafter's `heating`, so Fire still runs it fastest |
| Sky Lift | Earth's tunnel, Air's Wind Gate | A tunnel becomes ground and opens the rock to mining. A gate reaches 48 tiles anywhere. The lift is 32 tiles, rock to rock |
| Steam power | Water's Tide Mill | It needs `steam_engines` first and a bank site. A Water city must borrow its way there for 1,500 points |
| Airship | #442's caravans, Fire's Balloon Dock | One voyage a season, with two people away. Caravans still bring a request and a new nation's goods every 15 minutes |
| Call Rain | Water's Irrigation Channel | Rain lasts a season and costs 140 chi from two adepts. The channel is permanent |

## Mockups

Hand-built in the full-screen style of #652: one hint bar, frames over the side panel, centred
confirm boxes. The bases are `fn_tui --scenario water-year-two --frame --ascii --size 110x34
--keys 'T'` (the Research screen) and `--keys 'N'` (the Nation screen), widened to 110 columns
as horizons-phase-4.md does. New lines are marked `<`, and the TUI follow-ups settle the keys
and glyphs.

The From newcomers page of the Research screen, in a Water city after the gate, with 4 Fire, 2
Earth and 3 Air residents and 21 people. `Tab` switches pages. The home tree is unchanged. The
cursor is on Cloudbending:

```text
+----------------------------------------------- * RESEARCH * -----------------------------------------------+
| Your tree   [FROM NEWCOMERS]                                                                  Tab: switch  |
|                                                                                                            |
| FIRE NATION - 4 here       EARTH KINGDOM - 2 here     AIR NOMADS - 3 here        FUSIONS                   |
| + Brickmaking              - Earthen Walls            o Orchards                 - Aqueducts  Earth+Water  |
|   Known, borrowed  240 pts   Locked          240 pts    Available        50 pts    Locked     1000+6 tools |
|   Brick Kiln                 Needs 3 Earth residents    Orchard                    Needs 3 Earth residents |
|                                                                                                            |
| * Smelting                 - Earth Rail               o Wind Turbines            - Steelworks  Earth+Fire  |
|   Queued          240 pts    Locked          500 pts    Available       500 pts    Locked     1000+8 tools |
|   Smelter                    Needs Earthen Walls, 3 ~   Wind Turbine               Needs 3 Earth residents |
|                                                                                                            |
| * Steam Engines            - Omashu Chutes            - Lemur Keeping            - Sky Lifts  Earth+Air    |
|   Queued          500 pts    Locked          500 pts    Locked          240 pts    Locked     1000+6 tools |
|   Steam Engine               Needs Earth Rail, 3 Ear~   Needs Orchards             Needs 3 Earth residents |
|                                                                                                            |
| o Hothouses                                           o Wind Gates               - Steam Power Fire+Water  |
|   Available       500 pts                               Available       500 pts    Locked     1000+6 tools |
|   Hothouse                                              Wind Gate                  Needs Steam Engines     |
|                                                                                                            |
| - Cargo Balloons                                                                 - Airships  Fire+Air      |
|   Locked         1000 pts                                                          Locked     1000+8 tools |
|   Needs Steam Engines                                                              Needs 24 people (21)    |
|                                                                                                            |
|                                                                                  >o Cloudbending Water+Air |
|                                                                                    Available  1000+4 tools |
|                                                                                    Call Rain               |
| ---------------------------------------------------------------------------------------------------------- |
| Cloudbending                                                                 Available  1000 pts + 4 tools |
| Water lifts the sea into the sky and Air carries the cloud inland.                                         |
| Unlocks: Call Rain (a waterbender and an airbender together: a 13x13 patch grows 1.5x for a season)        |
| Fusion of Water and Air. Needs 3 Air residents (3 live here) and a Water or Air master (5 here).           |
+------------------------------------------------------------------------------------------------ 6 fusions -+
 RESEARCH  Arrows choose - Enter set as target - Tab your tree - Esc or T close                         ? keys
```

The detail strip for a borrowed tech, and for one waiting on a fallen condition:

```text
| Smelting                                                                                Available  240 pts |
| Draw metal out of ore with steady firebending heat.                                                        |
| Unlocks: Smelter. Heating x1.2 for Fire residents only                                                     |
| Borrowed from the Fire Nation at 2x cost while 3 Fire residents live here (4 do). No Forging needed.       |
```

```text
| Steam Engines                                                                         Waiting  500/500 pts |
| Boil water to drive mills and pumps.                                                                       |
| Unlocks: Steam Engine. Power generation x1.25 for Fire residents only                                      |
| Waiting: needs 3 Fire residents (2 live here). It finishes as soon as another arrives.                     |
```

The side panel's Research row while waiting (left), and the alert feed on the summer solstice in
a Water city with Fire residents (right):

```text
+- SETTLEMENT ----------- [N] -+    +- ALERTS ---------------------+
| Research: Steam Engines  [T] | <  | The summer solstice.         | <
|   Waiting: 3 Fire residents  | <  | Firebenders' chi floods back | <
|   (2 live here)              | <  | (4)                          | <
| Gate complete - 21 people    |    | The summer solstice in 2     | <
+------------------------------+    | minutes                      | <
                                    +------------------------------+
```

The Nation screen's Almanac after the gate, Water city, with three nations living there. The
running day comes first:

```text
| STORES                                                                     |
| Wood 2120  Stone 311  Fish 36  Fruit 130  Cabbage 21  Metal 9              |
|                                                                            |
| ALMANAC                                                                    |
| First frost in 9m 40s                                                      |
| Next caravan in 4m 10s                                                     |
| The summer solstice: 3m 20s left. Fire benders: 4                          | <
| The harvest moon in 18m 20s. Water benders: 14                             | <
| The equinox winds in 48m 20s. Air benders: 3                               | <
|                                                                            |
| TRAITS                                                                     |
| Water work x2.3  Power generation x1.25  Heating x0.8                      |
```

The Airship screen, opened from a selected dock, full-screen like the Traders screen. The player
has picked the Earth Kingdom and its wood-for-ore deal, twice. `Enter` asks for the confirm
below it:

```text
+----------------------------------------------- * AIRSHIP * ------------------------------------------------+
| Airship Dock at (61, 40)                                     Docked   Fuel 20/20 Wood   Crew: 2 residents  |
|                                                                                                            |
| SAIL TO                    DEALS                                               Times        In stores      |
| > Earth Kingdom            > 25 wood for 3 gold ore                            2/2          1640 wood      |
|   Fire Nation                6 wares for 3 gold ore                            0/2          14 wares       |
|   Air Nomads                 1 scroll copy for 15 stone                        0/2          5 copies       |
|   Water Tribe (kin)                                                                                        |
|                                                                                                            |
| HOLD                                                                                                       |
|   Out: 50 wood                                                                 50/60                       |
|   Back in about 15 min with 6 gold ore                                                                     |
|                                                                                                            |
+------------------------------------------------------------------------------------------------------------+
 AIRSHIP  Up/Down choose - Left/Right nation or deal - +/- times - Enter load and sail - Esc close      ? keys
```

```text
                    .------------------------- SEND THE AIRSHIP -------------------------.
                    |                                                                    |
                    |  Load 50 wood and sail to the Earth Kingdom?                       |
                    |                                                                    |
                    |  Haulers bring the goods to the dock. Once it is loaded, two       |
                    |  residents board and are away for about 15 minutes. It comes       |
                    |  back with 6 gold ore. Until it sails you can cancel, and the      |
                    |  goods stay at the dock.                                           |
                    |                                                                    |
                    |           Enter  load and sail     Esc  back                       |
                    |                                                                    |
                    '--------------------------------------------------------------------'
```

An aqueduct from the lake across the plain to a farm, on the Earth map. `u` is aqueduct, `~`
farm plots, `-` lake, and `:` the irrigation tint drawn while the cursor is on an aqueduct:

```text
--------------.",,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,      HERE ------------ (36, 20) -
-------------uuuuuuuuuuuuuuuuuuuuuuu,,,,,,,,,,,      Aqueduct
-------------.,,,,,,,,,,,,,,,,,,,,,u,,,,,,,,,,,      Flowing: 31 tiles from (13, 19)  <
-------------.,,,,,,,,,,,,,,,,,,,::u::,,,,,,,,,      Irrigates plots within 2 tiles   <
------------..,,,,,,,,,,,,,,,,,,,:~u~:,,,,,,,,,      People drink beside it           <
------------.,,,,,,,,,,,,,,,,,,,,:~u~:,,,,,,,,,
------------.,,,,,,,,,,,,,,,,,,,,:~u~:,,,,,,,,,
------------.,,,,,,,,,,,,,,,,,,,,::::::,,,,,,,,
```

The bend picker (`B`) in a Water city with Air residents. The joint bend lists both nations'
counts:

```text
| BEND                                                         |
| > Water the Land        Water: 14 benders                    |
|   Carve Canal           Water: 14 benders                    |
|   Wind Carry            Air: 3 benders                       |
|   Call Rain             Water 9 adepts + Air 2 adepts        | <
|   Raise Earth           Earth: 2 benders                     |
```

## Measuring it

Phase 6 ends with a harness run (follow-up O), after the playtest (follow-up N). The harness has
never played past the gate, so it first needs a United City to play in (follow-up M):

- **Getting there.** The economy opening begins and finishes the gate. #731's `stage_minute.N`
  already asks for the endeavor's stages, so M adds whatever part of that #731 hasn't. Runs go
  to minute 420, and the new metrics count from `gate_minute`. M also adds four scenarios,
  `<nation>-united`, whose prelude runs the economy opening to the gate. The playtest starts
  from these.
- **What the opening does after the gate.** It targets each other nation's teachable techs as
  soon as they are borrowable, cheapest first. Then it targets each fusion it can, in file
  order. It plays each fusion's simplest use: one aqueduct from the nearest river or lake to the
  farm plots, one Steelworks, a Sky Lift pair across the nearest range if one splits the work from
  the stores, the Steam Engine moved to the bank, one voyage a season for the scarcest sign
  material, and Call Rain on the largest plot cluster each spring. With question 1 answered yes,
  it fills each caravan's request when stores can pay.

What the run reports, on the panel's 32 seeds and both research orders. Every expected value is
*provisional* until the playtest:

| Measure | Metric | Expected |
|---|---|---|
| Residents arrive | new `residents_at.M` per nation, from `gate_minute` | 3 of some other nation by gate + 75 on 85% of runs |
| The exchange opens | new `borrowed_minute` (the first borrowed tech) and `fusion_minute` (the first fusion) | First borrowed tech by gate + 90, first fusion by gate + 150 |
| Fusions are used | a tally per fusion: `aqueduct_tiles`, `steel_made`, `lift_rides`, `bank_engines`, `voyages`, `rains` | Above zero on 85% of the runs that learned it |
| Celestial days | new `celestial_chi` (chi refilled at rises) and the existing `chi_idle` | A rise refills at least half a meter per bender on average. `chi_idle` doesn't rise |
| Decision timeline | `longest_gap`, now also after the gate | No gap over 15 minutes from the gate to minute 420 on 7 of 8 seeds per nation |
| No regressions | every existing target, through `balance verdict --against` | Nothing before `gate_minute` changes, except `star_charts`' place in the research order |

The balance follow-up records the measured numbers in a "Measured" section here. The first
levers are all data: `borrow_residents`, `borrow_cost`, each fusion's `requires` and tool count,
the post-gate newcomer weights (question 1), and each celestial day's `minutes` and `refill`.

## Risks

- **The United City ships different from its spec.** If #442 drops caravans from all four
  nations, airships lose their deal lists and question 1 loses its lever. The playtest issue
  rechecks this doc against what shipped before any phase-6 data lands.
- **The exchange comes too late.** If the United City session is short and residents slow, the
  From newcomers page is a list of locks. The levers are data (`borrow_residents`, the weights,
  the request reward), and the playtest comes first so that they are set before the balance run.
- **Borrowed modifiers per unit.** `effective_modifiers` gains a third input (the unit's nation
  besides the settlement's), and a system that reads the settlement's cached value where it
  should read the unit's will miss the borrowed tech for the teachers. It degrades safely, since
  the cached value is today's. Follow-up B tests the three cases: home unit, teacher and third
  nation.
- **Joint bends stall.** A joint bend waits for both slots. If the one Air adept is busy for an
  hour, the designation sits. The status says who is missing, and designating more bends doesn't
  queue more waiting jobs than there are pairs of casters.
- **Airship crew and invariants.** Units leave the map and come back, so the world invariants
  (`check`, "units are out of order") must allow units held by a voyage. Follow-up G keeps them
  in `World::voyages`, in id order, and the invariant check walks both lists.
- **Celestial refill and mastery.** A refill means more bends, and so faster ranks. Before the
  gate that speeds the masters sign (#344), one of the slowest, which is welcome, but it is a
  change to pre-gate pacing from a 500-point tech. The run watches `sign_minute.masters`, and the
  lever is `refill`.

## Open questions for the owner

1. **Should the player steer which nations settle in the United City?** *Recommended: yes,
   through caravans. In the United City, a caravan's request draws a newcomer of the caravan's
   own nation, and after the gate the weights move to own 0.1 and each other nation 0.3.* Owner
   decision 3 asks "who do I want living here?", but under #240 the answer is chance. At 100%
   draw, 3 residents of one nation take about 2.5 hours. With the steer and the new weights, 3
   of a chosen nation take about 65 minutes, and the request the player fills is the choice.
   The alternatives: leave arrivals to #240's weights (the exchange arrives late and at random),
   or have the gate bring a delegation of two from each other nation at once (fast, but every
   game's exchange starts the same, and six people need six free beds at the rite). It amends
   pacing.md section 4's request reward, which is #442's to build. Section: "Designed ahead of
   the United City". (decision 1)
2. **Should a celestial day refill benders' chi, or only double its regen?** *Recommended: refill
   to full as the day begins, then 2x regen for its 5 minutes.* The spec says "regain chi twice
   as fast for a few minutes". At today's regen that is worth about 20 chi per bender a year,
   which no player would plan around. A refill is worth up to a full meter, about 25 minutes of
   regen, and it turns the day into the decision the spec names: drain the meters before it,
   and have the big bends ready when it rises. A third option, bends at half chi on the day,
   does the same as the refill but is harder to read on the meter. Section: "Celestial days".
   (decision 2)
3. **Does a borrowed tech stay known if its teachers leave?** *Recommended: yes. Residents gate
   learning only.* The spec's "while at least 3 of that nation live in the settlement" can be
   read either way. Keeping it means the Fire residents matter until the kiln is learned, and
   after that the city owns it. The other reading makes the tech work only while 3 Fire
   residents live there, so the city's mix matters for the rest of the game. But a death would
   switch off Brick Kilns, rail and turbines, which is upkeep the game doesn't ask anywhere else,
   and it would need an "unlearned" state for buildings already standing. Either way, the
   tech's modifiers help only the teachers (section 2). Section: "Borrowed techs". (decision 3)

## Follow-up issues

Each one appends its own tech, building and table blocks from this doc, and B edits the
`teachable` flags in place. `TechId`, `BuildingId` and `AbilityId` are file positions, and
saves remap them by name (save-compat.md), so the order between follow-ups doesn't matter.

| Draft | Title | Blocked by |
|---|---|---|
| A | Sim and data: research conditions that can fall (`population`, `masters`, `masters_of`, `residents`) and the Waiting state | #707 |
| B | Sim and data: borrowed techs (`teachable`, borrow cost and residents, waived bending prereqs, modifiers for the teachers, the exchange list) | A, #673, N, decision 3 |
| C | Sim and data: fusion techs (`fusion`, validation, visibility after the gate) and `steelworks` | A, #718, #673, N |
| D | Sim and data: the Aqueduct (flowing network, drinking, irrigation) and `aqueducts` | C |
| E | Sim and data: the Sky Lift (a pair anyone rides, loads and all) and `sky_lifts` | C, #690, #691 |
| F | Sim and data: the `bank` upgrade and `steam_power` | C, #674, #675 |
| G | Sim and data: the Airship Dock (voyages, crew away, `PlanVoyage`) and `airships` | C, #437, #442, #675 |
| H | Sim and data: joint bends (`nations` on an ability), the `rain` effect, and `cloudbending` | C |
| I | Sim and data: celestial days (`[[celestial_day]]`, refill and regen, rise events, the Almanac) and `star_charts` | #677, #673, decision 2 |
| J | Sim and data: steering United City newcomers (requests draw their caravan's nation, post-gate weights) | #442, decision 1 |
| K | TUI: the From newcomers page, condition lines, borrowed costs, fusion bills and the Waiting row | A, B, C, #729 |
| L | TUI: celestial days (Almanac lines, rise alerts) and fusion readouts (aqueduct tint, lift link, bank line, Airship screen, joint bends in the picker) | I, D, E, F, G, H, #682, #698 |
| M | Dev tools: the United City in the harness (the opening finishes the gate, 420-minute runs, `gate_minute`, `<nation>-united` scenarios) | #442, #731 |
| N | Playtest: an hour of the United City, and phase 6's provisional numbers | #442, M |
| O | Balance: the opening plays the exchange, post-gate metrics, and the phase-6 run | B, C, D, E, F, G, H, I, J, M, N, #731 |
| decision 1 | Decision: should the player steer which nations settle in the United City? | none |
| decision 2 | Decision: should a celestial day refill benders' chi, or only double its regen? | none |
| decision 3 | Decision: does a borrowed tech stay known if its teachers leave? | none |
