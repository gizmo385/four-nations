# Research horizons, phase 5: lost teachings

Doc status: current

Status: **proposed** (#669). This is phase 5 of [research-horizons.md](research-horizons.md)
(accepted 2026-11-10), "Lost teachings: a draft from the ruins". It follows the shape of the
phase 1 to 3 designs ([horizons-phase-1.md](horizons-phase-1.md),
[horizons-phase-2.md](horizons-phase-2.md), [horizons-phase-3.md](horizons-phase-3.md)) and
reuses their patterns: the `core` rule (#673), the `upgrades` unlock kind (#674) and `requires`
conditions (#707). It doesn't depend on phase 4 (schools and item costs, #668). The Spirit
Portal is out of scope: `spirit_water` and `spirit_projection` are hooks into it, not its
design. The owner questions are under "Open questions for the owner", and the implementation
work is under "Follow-up issues".

## Decisions in brief

1. **Great ruins are promoted lost ruins.** After mapgen places resources, a new pass turns the
   2 or 3 "largest" `lost_ruins` (the most land around them) into `great_ruins`, spaced far
   apart. No other tile changes, so every other resource stays where it was. Glide, the Map Room
   and a caravan's chart find them through their existing `finds` lists.
2. **Searching one gives a teaching scroll, and reading it opens the draft.** A great ruin
   yields the lost ruin's 2 ancient scrolls plus one **teaching scroll**. That scroll is the
   `reveals` hook (research.md, owner decision 4 of 2026-10-22). It goes to a study hall like any
   scroll. When it is read there, the sim draws two teachings from
   `rng::stream(seed, "research.teachings")` and waits for the player's pick.
3. **The pick is `Command::KeepTeaching`.** The kept teaching appears in the tree as an ordinary
   tech: 250 points, plus the teaching scroll, which stays shelved at the hall until the tech is
   learned. The other teaching is lost for the rest of the game. A pending offer never blocks
   play, and "Decide later" is the safe answer.
4. **Hidden techs are `[tech.teaching]` entries in `research.toml`.** They are never `core`,
   never a starting tech and never anyone's prereq. A teaching may carry a `site` rule, and the
   draft skips one whose site rule fails ("a teaching the site can't use").
5. **Ten teachings, each built on a system that exists.** Two are shared (`pai_sho`,
   `spirit_library`) and two belong to each nation. Every one is a building, an ability, an
   upgrade or a modifier, with numbers in TOML below. None makes stone, ore or food from chi.
6. **Saves and replays stay deterministic.** The pick is a logged command, the draw comes from a
   saved RNG, and a pre-phase-5 save gets its great ruins on load, chosen from its still-buried
   lost ruins by the same picker.

## What ships

| Teaching | For | Cost | Needs | Unlocks | Site rule |
|---|---|---|---|---|---|
| `pai_sho` | all | 250 + scroll | chapter "Known on the road" | **Pai Sho Table**: White Lotus traders join each winter caravan, and request rewards double | none |
| `spirit_library` | all | 250 + scroll | `writing` | **Knowledge seekers**: one seat in every study hall is filled by a fox spirit that never eats, sleeps or leaves | none |
| `sandbending` | Earth | 250 + scroll | `earth_shaping` | **Sand sailers** (desert crossed twice as fast) and the **Sandstone Works** (stone from desert sand, by labour) | 40 desert tiles within 40 |
| `crystal_growing` | Earth | 250 + scroll | `tunneling` | **Crystal Grotto** beside a tunnel: grows glow crystals, the richest ware a caravan takes | 20 mountain tiles within 30 |
| `sun_stone` | Fire | 250 + scroll | `dancing_dragon` | **Sun Stone**: chi returns twice as fast in its reach in spring and summer | none |
| `hot_springs` | Fire | 250 + scroll | `brickmaking` | **Bathhouse** against rock: one bath restores energy and keeps the cold off for 20 minutes | 4 mountain tiles within 30 |
| `plantbending` | Water | 250 + scroll | `irrigation` | Ability **Regrow**: clears the strain from a 3x3 patch | none |
| `spirit_water` | Water | 250 + scroll | `healing` | **Spirit water**: a Healing Hut fills one vial each spring. A vial saves a settler who would have died | none |
| `guru_teachings` | Air | 250 + scroll | `meditation` | **Chakras**: a master's bends cost 60% chi, not 80% | none |
| `spirit_projection` | Air | 250 + scroll | `meditation` | Ability **Project**: a monk meditates and reveals every buried thing within 20 tiles | none |

Each nation's pool holds four teachings: the two shared ones and its own two. A game keeps at
most two of them, since each draft loses one. Which two depends on the map's draws and on the
player, which is the variety the fixed tree lacks.

## The `core` rule, unchanged

Every teaching leaves `core` at its default, false, and validation refuses `core = true` on one.
The branch sign doesn't count the four nation teachings, and the building-kinds sign doesn't
count the five teaching buildings (horizons-phase-1.md, "The rule every phase-1 tech follows").
The unit test from #673 that pins each nation's two gate targets runs again with phase-5 data.

## Great ruins on the map

### What mapgen does

```toml
# data/resources.toml, appended (last, after lost_ruins)
[[resource]]
id = "great_ruins"
name = "Great Ruins"
glyph = "N"
chance = 0.0              # never placed by the scan: only promoted from lost_ruins
terrains = ["land"]
buried = true
[resource.promote]
from = "lost_ruins"       # the resource whose tiles may become this one
tiles_per = 12000         # one per this many map tiles, clamped to [min, max]
min = 2
max = 3
spacing = 40              # Chebyshev, between great ruins
size_radius = 3           # a ruin's "size" is the Land tiles in the 7x7 box around it
```

- **Count.** `clamp(round(width * height / tiles_per), min, max)`: 2 on Small (192x128), 3 on
  Medium and Large.
- **Picker** (`mapgen::pick_promoted`, one function used by mapgen and by the save upgrade):
  1. Candidates are the `from` tiles, in scan order.
  2. Shuffle them with `rng::stream(seed, "mapgen.great_ruins")`, then stable-sort by size,
     largest first. The shuffle only breaks ties.
  3. Take candidates in that order, skipping one closer than `spacing` to a ruin already
     taken, until `count` are taken.
  4. If fewer than `min` were taken, fill from the skipped ones in the same order, ignoring
     spacing.
- **Then** each picked tile's resource changes from `lost_ruins` to `great_ruins`. That is the
  pass's only write.
- **Why it moves nothing else.** Resources are placed in file order, each from its own stream
  (`mapgen.resources.<id>`), and a tile keeps the first resource that claims it. `great_ruins`
  is appended last with `chance = 0.0`, so its scan claims nothing, and the promote pass runs
  after `add_resources` and only rewrites tiles that were already ruins. A test generates seeds
  1 to 32 with and without the pass and asserts that the maps differ only on the promoted tiles,
  and only from `lost_ruins` to `great_ruins`.
- **Why "most land around it".** The spec says great ruins are "the largest of the existing lost
  ruins", and a ruin is one tile. The land around it is the honest stand-in for size: a great
  city stood on a broad plain, not on a spit between sea and mountain. It also leans great ruins
  toward reachable ground.
- **Validation.** `promote` needs `chance = 0.0`, names an existing resource that is itself
  `buried`, and has `1 <= min <= max <= 5`, `spacing <= 100` and `size_radius` from 1 to 6.
- **Goldens.** Every mapgen golden changes in 2 or 3 glyphs (`n` to `N`), and the follow-up
  re-blesses them. Nothing else in a golden may move.

On seed 7, Small, the eight lost ruins score from 5 to 38, and the picker takes (105, 44) with
38 and (114, 86) with 33, 42 tiles apart. The `air-year-two` settlement is at about (94, 64), so
both are about 20 tiles out. Cropped from
`cargo run --profile ci -p fn_sim --bin mapgen -- 7 192 128`, x 86 to 135, with the two promoted
tiles hand-marked `N`:

```text
 42 ::::::::::TT,TTT,,,,..~~~~~~~~~~~~~~...F"""""F"FFF
 43 ::::::::,,,Tc,T,,T,Y,.~~~~~~~~~~~~~~~~.FFF""""YFFF
 44 :::::::,,,,,YT,,,,,N..~~~~~~~~~~~~~~~~."FF""""""FF     (105, 44): desert edge, by the sea
 45 ,::::,,,Y,,,,...,,,,.~~~~~~~~~~~~~~~~~.F"""F""FFFF
 46 T,,,:,,,,,,...-...,,...~~.~~~~~~~~~~..."FFFFF""FFF
    ...
 84 ,,,TT,T,,T,T,,,,^o$,TT,,TTT,TT,""FYF",,,,,T,YTT,,,
 85 ,,,YT,,,,,,T,T,,^^^^,,T,,,T,,T,"F"FF,,,,,T,,T,,T,,
 86 ,T,T,,,,TT,,T,,,oo^o^,,T,T,,N,T,T,,,,,c,,T,T,,TT,F     (114, 86): plains under the gold hills
 87 T,::,,,,T,,,T,,^^^oo^^,,T,TT,,,,,T,,,,,,,,,TTTT,F"
 88 cT::,,,T,,T,,,^^^^$^o^,c,,,,,TT,,,,,,,,,,,,,,T,TF"
```

### How they are found

Nothing new finds them. Three existing systems gain a bigger prize by adding one id to a list:

```toml
# data/bending.toml, glide (edited in place)
finds = ["lost_ruins", "great_ruins"]

# data/buildings.toml, map_room's [building.chart] (edited in place)
finds = ["lost_ruins", "great_ruins"]
```

- **Glide** (Air) spots both kinds within 14 tiles of where it soars, as today.
- **The Map Room** charts them in its existing order: reachable first by walking cost, then the
  rest by distance, ties by `Pos` (cartography.md, "Which ruin a chart finds"). It doesn't
  prefer great ruins. On seed 7, judging by distance, the first great ruin should be the second
  or third chart, about 10 to 15 minutes after the room opens. A preference would make the Map
  Room a teaching machine and take the edge from Air's Glide, the nation that ought to be the
  seeker.
- **A caravan's chart** reward reuses the Map Room's pick (pacing.md, section 2). #437 should
  read the `finds` list from the Map Room's def rather than naming `lost_ruins`. If it ships
  with the id hard-coded, #736 changes that one line.
- **Spirit Projection** (below) also reveals them, since it finds every buried kind.
- **Notices.** `Spotted great ruins 12 tiles east.` and `Charted great ruins 48 tiles north-east.`
  replace the lost-ruin wording when the find is a great ruin. The existing "no settler can reach
  them" suffix still applies.
- **The scrolls sign's list** (cartography.md, owner decision of 2026-11-09) lists an uncovered
  great ruin like a lost one, with "2 scrolls and a teaching" for its count.

### Searching one

```toml
# data/jobs.toml, appended
[[harvest]]
resource = "great_ruins"
verb = "Search"
item = "ancient_scroll"
count = 2                                       # the lost ruin's 2, so the scrolls sign loses nothing
work = 40.0                                     # twice a lost ruin's 20
also = [{ item = "teaching_scroll", count = 1 }]

[[item]]
id = "teaching_scroll"
name = "Teaching Scroll"
reveals = 2               # read at a study hall: offers this many teachings (the draft)
```

- **`also`** is a new optional harvest field: stacks that drop beside the main yield when the
  harvest completes, one ground stack per entry, each hauled as usual. Only this harvest uses it.
  It isn't scaled by `tunnel_count` or anything else.
- **`reveals`** is the hook research.md kept open. An item with `reveals` is **readable**, as an
  item with `knowledge` is (`World::is_readable`): it is never stockpiled, and a hauler carries it
  to the nearest study hall. Validation: an item has at most one of `knowledge` and `reveals`,
  and `reveals` is 1 to 4.
- **Reading one** doesn't add knowledge and doesn't count toward `scrolls_read`, because the
  scrolls sign is about ancient scrolls and the great ruin's two still count. It runs the draft
  (next section) and logs a `TeachingEvent::Offered`.
- **Autoplay** keeps a one-shot Search designated on uncovered great ruins within its reach, as
  it does for lost ruins.

## The draft

### Who is eligible

A teaching is **eligible** for a new offer when all of these hold:

- its `nation` is unset or is the settlement's nation;
- it is not kept, not lost, and not in another pending offer;
- its `site` rule, if it has one, holds (below).

### The draw

When a teaching scroll is read:

1. Eligible teachings are listed in `TechId` order.
2. `n = min(reveals, eligible)`. Draw `n` without replacement from the world's `teachings_rng`
   (`random_range(0..len)`, then remove). The offer lists them in `TechId` order.
3. With `n >= 1` a pending **offer** is pushed: `{ techs, hall, tick }`, where `hall` is the
   study hall it was read at.
4. With `n = 0` (every teaching kept, lost or excluded) the scroll reads for
   `empty_scroll_knowledge` points instead, through `add_knowledge`, and the notice says so.
   Whether that is the right answer for a third great ruin is question 3 (#735).

`teachings_rng` is a new `World` field seeded with `rng::stream(seed, "research.teachings")`, so
each read draws the next numbers rather than the same ones. A save from before phase 5 loads it
seeded by 0, as `sickness_rng` does for #389. Nothing else draws from it, so harvest order,
hauling and other streams can't change an offer, only the number of scrolls read before it.

```toml
# data/research.toml, a new top-level table
[teachings]
empty_scroll_knowledge = 100   # what a teaching scroll reads for when nothing is left to offer
```

### The site rule

```toml
[tech.teaching]
site = { biomes = ["desert"], within = 40, min_tiles = 40 }
```

- **Meaning.** At least `min_tiles` tiles within `within` tiles (Chebyshev) of
  `World::settlement_anchor()` match. A tile matches when its terrain is in `terrains` and its
  biome is in `biomes`. An omitted list matches anything, and at least one of the two must be
  given.
- **Checked at draw time**, on today's map. Terrain can change (a tunnel, a canal), so a
  teaching excluded early could be eligible for a later scroll.
- **Validation.** `within` is 1 to 100, `min_tiles` is at least 1, and every terrain and biome
  name exists.
- **Why at draw time and not at mapgen.** The site is chosen at embark, after mapgen, and a
  great ruin's offer belongs to the settlement that reads it.
- Three teachings carry one: `sandbending`, `crystal_growing` and `hot_springs`, the three the
  spec calls "worth a lot or nothing depending on where the settlement stands". The others are
  useful on any site, sooner or later.

### The pick

```rust
Command::KeepTeaching { tech: TechId }
```

- **Valid** when `tech` is in a pending offer. A teaching is in at most one pending offer (the
  eligibility rule), so the tech names its offer. Otherwise it is refused with
  `ResearchError::NotOffered`.
- **Effect.** `tech` joins `kept`. Every other tech in that offer joins `lost`. The offer is
  removed and a `TeachingEvent::Kept { tick, kept, lost }` is logged in `World::events`, for the
  alert log and the harness.
- **No "keep neither".** Owner decision 5 is "pick 1 of 2", and losing both helps nobody.
  "Decide later" is a UI answer that sends no command (below).
- **Pending offers pile up in read order.** The modal shows the oldest first.

### Hidden, kept, lost: what the tree does

The new state is one `World` field, keyed by id and never by position (tech-stack.md,
"Saves"):

```rust
#[derive(Default, Serialize, Deserialize)]
pub struct Teachings {
    /// Whether this world's great ruins have been placed (`false` only in a pre-phase-5 save).
    placed: bool,
    kept: BTreeSet<TechId>,
    lost: BTreeSet<TechId>,
    offers: Vec<Offer>,       // oldest first
}
```

| State | Research screen | `SetResearch` |
|---|---|---|
| Hidden (never offered, or offered and still pending) | Not drawn | Refused: `ResearchError::Hidden` |
| Kept | Drawn in a TEACHINGS column after the last tier, like any tech: Locked, Available, Queued or Known | As any tech: prereqs queue first, `requires` applies (#707) |
| Lost | Drawn in the TEACHINGS column, marked `x`, "Lost to the ruins" | Refused: `ResearchError::Lost` |

- **Phase 3's `Facts`** (the struct `set_target` takes for `requires`) gains `kept`, so
  `set_target` can tell hidden from kept. Prereqs are checked as usual. No tech may name a
  teaching as a prereq, so a queue can't pull a hidden tech in by the back door.
- **Autoplay's research order** skips hidden and lost teachings and treats a kept one like any
  tech.
- **Validation** for a `[tech.teaching]` tech: not `core`, not in any nation's
  `starting_techs`, not in any tech's `prereqs`, and `cost` greater than 0 as usual. Its prereqs
  may be ordinary techs.

### "250 points plus the teaching scroll"

A kept teaching costs its `cost` in knowledge (250, scaled by the difficulty preset's
`research_cost` like any tech) and the teaching scroll it came from. The scroll is already at the
study hall where it was read, so it is **shelved** there: the hall holds it until the teaching is
learned, and completion uses it up. In state this needs nothing beyond `kept`. A kept teaching
that isn't known yet is a shelved scroll.

- **The Research detail** reads `Available  250 pts + scroll` and `Uses up the teaching scroll
  read at the Meditation Pavilion`.
- **Why not phase 4's item delivery** (`[[tech.cost_items]]`, research-horizons.md, "Costs beyond
  knowledge"). The scroll would go to stores and then back to a hall: two hauls of an item that is
  already where it is needed, and a dependency on phase 4 for no decision. Owner #734 still
  holds, because the scroll is the teaching's item cost. If phase 4 lands first and the owner
  wants one mechanism, a teaching can swap to `cost_items` with `item = "teaching_scroll"`. The
  draft doesn't change.
- **Taking the hall down** doesn't lose the scroll. Shelving is the settlement's state, not the
  building's. The detail line then names the nearest standing hall.

### Old saves

- A save from before phase 5 has no `teachings` field, so it loads with `placed = false`.
  `load` then runs `pick_promoted` over the **still-buried** `lost_ruins` in `World::buried`, with
  the saved map for sizes and the save's own seed, and rewrites those entries to `great_ruins`.
  Then it sets `placed`. Ruins already uncovered are left alone, because the player has seen them
  as lost ruins.
- If fewer than `min` are still buried (an Air game that glided over most of the map), the save
  gets fewer great ruins. That is honest, and only old saves can hit it.
- New ids are all appended: one resource, three items (`teaching_scroll`, `spirit_vial`,
  `glow_crystal`), ten techs, five buildings and two abilities. Nothing is renamed. Saves remap
  them by name (save-compat.md). `Unit::warm_until` (the Bathhouse) has a serde default.
- **Run logs** recorded before phase 5 are refused, as for any id-table change (save-compat.md,
  "Run logs"). A run log recorded after it replays exactly: the draw is from a saved stream, and
  the pick is a logged command.

## The teachings

Numbers are starting values for the harness (see "Measuring it"). Every block is appended by
the follow-up that makes it work, as phase 1 decided. Hover text means the build modal's effects
list and the cursor panel's inspect lines, in the voice of text-voice.md.

### `pai_sho`: the Pai Sho Table (all)

```toml
[[tech]]
id = "pai_sho"
name = "Pai Sho"
description = "Learn the old game, and the White Lotus learns of you."
cost = 250
requires = { chapter = "known_on_the_road" }
[tech.teaching]
[tech.unlocks]
buildings = ["pai_sho_table"]

[[building]]
id = "pai_sho_table"
name = "Pai Sho Table"
verb = "Build"
work = 30.0
passable = false
[building.pai_sho]
lotus_deals = 3           # extra deals on each winter visit, from every other nation's lists
request_reward = 2.0      # every caravan request's reward is multiplied by this
[[building.cost]]
item = "wood"
count = 8
[[building.cost]]
item = "stone"
count = 4
```

- **Rule.** While a table stands, the caravan that makes camp in winter brings White Lotus
  traders along. The visit shows `lotus_deals` more deals, drawn from
  `rng::stream(seed, "trade.lotus")` among every caravan list except the visiting nation's and
  the settlement's own. Request rewards are doubled on every visit: draw 0.5 becomes a whole
  newcomer, 60 knowledge becomes 120, and one chart becomes two.
- **Why not a fifth visit.** Caravans come once a season and stay 9 of its 15 minutes (pacing.md).
  A fifth visit would put two camps on the map at once. The spec's "one extra caravan a year"
  arrives as companions to the winter one: the same extra trade, in the season that needs it.
- **Needs the chapter**, as `trade_wares` does (#707). Before caravans come, the table would do
  nothing, and the Research screen shouldn't offer a trap.
- **Hover.** `White Lotus traders join the winter caravan` and `Caravan requests pay double`.
- **The decision.** Trade for the rest of the game, but nothing until the next winter. Against
  `spirit_library`, it is the long bet against the early one.

### `spirit_library`: knowledge seekers (all)

```toml
[[tech]]
id = "spirit_library"
name = "Spirit Library"
description = "Wan Shi Tong's foxes come to study beside your scholars."
cost = 250
prereqs = ["writing"]
[tech.teaching]
[tech.unlocks]
[[tech.unlocks.upgrades]]
building = "study_hall"
spirit_seats = 1          # a new property on study_hall, 0 by default

# data/buildings.toml, study_hall (edited in place)
spirit_seats = 0
```

- **Rule.** Each finished study hall fills `spirit_seats` of its seats with a fox spirit. A fox
  studies exactly as a seated scholar does: `study_knowledge_per_minute` times the `study`
  modifier and the hall's power boost, through `add_toward_target`, so it feeds scribing when the
  queue is empty and an Archive stands (horizons-phase-3.md). It never eats, sleeps, leaves or is
  pre-empted, and it isn't a unit: nothing paths, nothing counts it as a person.
- **It takes a seat, it doesn't add one.** A 4-seat hall (with `writing`) seats 3 settlers and a
  fox. Early, when hands are short, that is a free scholar. Late, with half the settlement idle,
  it is worth a third more than the settler it displaces, because it never stops. That is
  "most valuable the earlier it is found".
- **With no target** a fox rests, as a scholar does (#284), unless scribing is on.
- **Hover.** `A fox spirit studies in each study hall`. Inspect: `Seats: 2 of 3, and a fox`.

### `sandbending`: sand sailers and the Sandstone Works (Earth)

```toml
[[tech]]
id = "sandbending"
name = "Sandbending"
description = "Shape the desert's sand into sails, roads and blocks of stone."
nation = "earth"
cost = 250
prereqs = ["earth_shaping"]
[tech.teaching]
site = { biomes = ["desert"], within = 40, min_tiles = 40 }
[tech.unlocks]
buildings = ["sandstone_works"]
biome_speed = { desert = 2.0 }   # a new unlock key: steps onto this biome are 2x as fast

[[building]]
id = "sandstone_works"
name = "Sandstone Works"
verb = "Build"
work = 40.0
passable = false
footprint = [2, 2]
capacity = 1
on_biomes = ["desert"]    # a new placement rule: every footprint tile must be this biome
[[building.cost]]
item = "wood"
count = 8
[[building.cost]]
item = "tool"
count = 1
[building.recipe]
output = "stone"
output_count = 1
work = 40.0
modifier = "earthworks"
input = []
```

- **Sand sailers.** `biome_speed` multiplies the step speed onto a tile of that biome, for every
  settler, through `MoveRules` so routes plan with it. It is the same kind of rule as rail and the
  Paved Road's track speed (horizons-phase-1.md), keyed by biome. Multipliers are in (0, 4] and
  biomes must exist.
- **Stone by labour, not chi.** The Works is a source recipe like the Mine Shaft: a crafter
  packs sand into stone, 40 s a block, 16 s at Earth's `earthworks` 2.5. That keeps the spec's
  rule that no bend makes stone from chi. `on_biomes` sits beside `beside` (#373), with the
  refusal `must stand on desert`.
- **The decision.** Only worth it with desert nearby, which is what the site rule guarantees.
  With it, the desert stops being a wall and becomes a quarry.

### `crystal_growing`: the Crystal Grotto (Earth)

```toml
[[tech]]
id = "crystal_growing"
name = "Crystal Growing"
description = "Coax glowing crystals from the walls of the tunnels."
nation = "earth"
cost = 250
prereqs = ["tunneling"]
[tech.teaching]
site = { terrains = ["mountain"], within = 30, min_tiles = 20 }
[tech.unlocks]
buildings = ["crystal_grotto"]

[[building]]
id = "crystal_grotto"
name = "Crystal Grotto"
verb = "Build"
work = 60.0
passable = false
footprint = [2, 2]
capacity = 1
beside = ["tunnel"]       # tunnel only: a bored passage, not bare rock
[[building.cost]]
item = "stone"
count = 8
[[building.cost]]
item = "tool"
count = 2
[building.recipe]
output = "glow_crystal"
output_count = 1
work = 120.0
modifier = "earthworks"
input = []
stock_limit = 12

[[item]]
id = "glow_crystal"
name = "Glow Crystal"
```

- **Caravans take them** (#738 appends these to #437's `data/trade.toml`): Fire traders
  give 5 metal for 2 crystals, Water traders give 4 tools for 2, and Air traders ask for 3 in a
  request whose reward is a whole newcomer's draw (1.0). Earth's own caravan never visits an Earth
  settlement, so there is no Earth row.
- **Numbers.** One crystal every 48 s at `earthworks` 2.5, so the stock limit of 12 takes about
  10 minutes, under a caravan interval. Per caravan that is about 12 metal or tools from one
  crafter, which is what Earth lacks most at the gate.
- **The decision.** It rewards a settlement that tunnelled, and is nothing for one that didn't.
  The site rule only checks that there is rock to tunnel. Boring the tunnel is the player's part.

### `sun_stone`: the Sun Stone (Fire)

```toml
[[tech]]
id = "sun_stone"
name = "Sun Stone"
description = "The Sun Warriors' stone gathers the summer sun for firebenders."
nation = "fire"
cost = 250
prereqs = ["dancing_dragon"]
[tech.teaching]
[tech.unlocks]
buildings = ["sun_stone"]

[[building]]
id = "sun_stone"
name = "Sun Stone"
verb = "Raise"
work = 60.0
passable = false
[building.chi_regen]
radius = 8                          # Chebyshev, from the footprint. Not modifier-scaled
factor = 2.0                        # chi regained per minute, awake or asleep, is multiplied
seasons = ["spring", "summer"]      # season ids from data/seasons.toml
[[building.cost]]
item = "stone"
count = 12
[[building.cost]]
item = "metal"
count = 2
```

- **Rule.** A unit standing within `radius` of a finished Sun Stone during a listed season
  regains chi `factor` times as fast. Two stones don't stack. `[building.chi_regen]` is general:
  the Spirit Oasis (#127) and phase 4's High Temple can use it with no new code.
- **The decision.** Seasonal power, so winter plans can't lean on it. It asks where the stone
  goes: among the homes, so sleepers wake full, or at the work site.

### `hot_springs`: the Bathhouse (Fire)

```toml
[[tech]]
id = "hot_springs"
name = "Hot Springs"
description = "Build baths over the springs, where the mountain's heat rises."
nation = "fire"
cost = 250
prereqs = ["brickmaking"]
[tech.teaching]
site = { terrains = ["mountain"], within = 30, min_tiles = 4 }
[tech.unlocks]
buildings = ["bathhouse"]

[[building]]
id = "bathhouse"
name = "Bathhouse"
verb = "Build"
work = 60.0
passable = false
footprint = [2, 2]
capacity = 2
beside = ["mountain", "tunnel"]     # must touch rock, like the Mine Shaft
[[building.cost]]
item = "stone"                      # brick stands in, as everywhere
count = 10
[[building.cost]]
item = "wood"
count = 8
[building.bath]
seek_energy_below = 30.0   # a unit this tired walks to a free bath within seek_tiles
seek_tiles = 30
minutes = 0.5              # a visit
warm_minutes = 20.0        # afterwards, no exposure damage for this long
cooldown_minutes = 20.0    # one bath per unit per this long
```

- **Rule.** A tired unit with a free bath in reach bathes instead of lying down: half a minute
  inside, then energy is full and `Unit::warm_until` is set `warm_minutes` ahead. While it holds,
  `apply_exposure` skips the unit. The cooldown means settlers still sleep, just less.
- **The decision.** It answers exposure, Fire's winter pressure, and it is tied to the rock face.
  A settlement that built away from the mountains pays in walking.

### `plantbending`: Regrow (Water)

```toml
[[tech]]
id = "plantbending"
name = "Plantbending"
description = "Draw the water through the roots and wake the tired soil."
nation = "water"
cost = 250
prereqs = ["irrigation"]
[tech.teaching]
[tech.unlocks]
abilities = ["regrow"]

# data/bending.toml, appended
[[ability]]
id = "regrow"
nation = "water"
name = "Regrow"
verb = "Regrow"
description = "Move the water in the roots, and the land forgets its weariness."
chi = 40.0
work = 10.0
speed_modifier = "water_work"
[ability.effect]
kind = "unstrain"         # a new effect kind
radius = 1                # a 3x3 patch
```

- **Rule.** Strain on every tile of the patch drops to 0 (difficulty.md, "Strain: tiring land").
  The target must have strain on at least one tile, or validation refuses it with `nothing here
  is tired`, as Seismic Sense refuses a bend with nothing to find.
- **The decision.** It makes the land near camp last, against anything that looks further out.
  On Normal, with gentle strain, it is small. On Hard, where repicking kills plants, it is a farm.

### `spirit_water`: the vial (Water)

```toml
[[tech]]
id = "spirit_water"
name = "Spirit Water"
description = "Water from the spirit oasis, kept for the one who can't be saved any other way."
nation = "water"
cost = 250
prereqs = ["healing"]
[tech.teaching]
[tech.unlocks]
[[tech.unlocks.upgrades]]
building = "healing_hut"
vials_per_year = 1

# data/buildings.toml, healing_hut's [building.healing] (edited in place)
vials_per_year = 0
vial_item = "spirit_vial"
vial_stock = 3             # no new vial while this many are in stores
revive_to = 30.0           # the need that was killing the settler is set to this

[[item]]
id = "spirit_vial"
name = "Spirit Water"
```

- **Rule.** On the first tick of each spring, if a Healing Hut is finished, `vials_per_year`
  vials drop beside the first hut in anchor order, up to `vial_stock` in stores. When a settler
  would die, and a vial is in a stockpile, the vial is used up (the lowest `Pos` stack first).
  The settler lives on with the need that was killing it (hunger, thirst or health) set to
  `revive_to`. Notice: `Spirit water saved Kya.`
- **Why the Healing Hut and not the Spirit Oasis.** The spec says the oasis yields the vial, but
  `spirit_oasis` is still modifier-only (#127). When the oasis gets a building, the vial moves
  to it in data. This is the hook the Spirit Portal design can pick up.
- **The decision.** Insurance, worth most on Hard, where harm kills. On Normal, it saves a
  starving or freezing settler once a year.

### `guru_teachings`: chakras (Air)

```toml
[[tech]]
id = "guru_teachings"
name = "Guru Teachings"
description = "Open the seven chakras, and let chi flow freely."
nation = "air"
cost = 250
prereqs = ["meditation"]
[tech.teaching]
[tech.unlocks]
modifiers = { master_chi = 0.75 }   # 0.8 x 0.75 = 0.6: a master's bends cost 60% chi
```

- **Rule.** `master_chi` is a new modifier, 1.0 for every nation by serde default (so
  `nations.toml` doesn't change). It multiplies `[mastery] master_chi` wherever a master's chi
  cost is computed. The modifier breakdown names the source: `Master chi x0.75: Guru Teachings`.
- **Waits on mastery ranks (#358).** The 80% master discount is that spec's. This follow-up is
  blocked on it and designed against its numbers.
- **The decision.** It pays only once masters exist: 16 bends in 3 forms for an Air settler.
  Kept early, it is 250 points for a promise.

### `spirit_projection`: Project (Air)

```toml
[[tech]]
id = "spirit_projection"
name = "Spirit Projection"
description = "Leave the body behind in meditation, and see what the land hides."
nation = "air"
cost = 250
prereqs = ["meditation"]
[tech.teaching]
[tech.unlocks]
abilities = ["project"]

# data/bending.toml, appended
[[ability]]
id = "project"
nation = "air"
name = "Project"
verb = "Project"
description = "Meditate, and send the spirit out to see what lies buried."
chi = 60.0
work = 30.0               # a long meditation, standing still
[ability.effect]
kind = "reveal"
radius = 20
finds = ["lost_ruins", "great_ruins", "gold_seam"]
```

- **Rule.** Today's `reveal` effect, with a wider radius and every buried kind. A test asserts
  that `finds` lists every `buried = true` resource, so a buried resource added later can't be
  forgotten here.
- **The decision.** It is the one sense that crosses kinds, for a nation that can't dig what it
  finds: gold seams are revealed for caravans and for the United City, not for Air's own picks.
  Against Glide, it reaches further but costs 30 seconds of a monk standing still.

## Mockups

All are hand-edited from `fn_tui` output. The TUI is being reworked (#651: one hint line; #655:
confirm questions as centred boxes with answer rows; #652: full-screen frames, in flight), so the
draft is mocked as a #655-style centred box and the TUI follow-up redraws it in whatever frame
#652 settles.

**The draft**, from `fn_tui --scenario air-year-two --frame --ascii --size 110x34`, with the box
drawn in the style of the Quit box (`--keys '<esc> <down> <down> <down> <down> <down> <down>
<enter>'`). It opens by itself and pauses the clock (question 1, #733). The cursor starts on
the safe answer, which `Esc` also picks, as #655 requires:

```text
"""""""",,,,c*/*,//,*::/^^^^^^,,/,,,,Y,,,.-----..,T,,,,..~~~~~~~..FF"""""""""F+------- * AIR NOMADS * -------+
/"""""""",,,,,/,,,,,,,,//^$^^^,,,,,,,,,,..----..,,c,,/,,.~~~~~...FFF"F"""""F"F| | Paus~ Spring Y2 15:00 left |
/""""""""",,,,,,,,//,,,,,^^^^,,,,,,,,,c,.------..,,,,,,,.~~~~~.."""F"""F""""""+- SETTLEMENT ----------- [N] -+
""""""""""",,,,,,,/%/,,c,,^^^,,,,,,,,,,..-------..,,,,,,.~..~~~.",""""""F""F""| People 12             Idle 0 |
""/"""""""/",,,,,,,/,,,,,,^^$,,,=,,,,,,.-------f-..,,,T,.....~..",c""F""""^^$^| Hungry 3  Thirsty 7          |
"//"""""Y//"Y""Y,,,,,B,,,,,^^,,,,=,,,,..----------.,,/,,T,,,.~.,F,,,"""""0co^T| Food 311 - 25/head  mild     |
"""""""""""""*"/",,/,,,,,,^^^,,,,,=,,..-----------..,,,,,,...~.,,F,"YF"FF""^F0| Wood 2000  Stone 394  Tool 7 |
"""""""Y""""*/%*""/%,,,,,,^^^,,,...=..-------------../T,...~~~.YF"""F"F"F"""F%| Herbalism ..........   3%    |
""""""""""/"""Y""""",/,,,,^^,,,,.-------------------.,c,.~~~~~.FFF"F""""""Y""T| Gate 3/9 - 4 people      [G] |
TT"""""""/YY""""B"fB/,*,,/^%,,,,.-------------------.,,,.f~~~~=FFF""F"FF"""FF%+- HERE ------------ (94, 64) -+
TT+-------------------------- * LOST TEACHING * ---------------------------+"%| Coast                        |
@"| A teaching scroll was read at the Meditation Pavilion.                 |"F| Plains (94, 64)              |
/"| Keep one teaching. The other is lost for this game.                    |F%| Elevation 152                |
/"|                                                                        |Fc| Warm climate: mild in winter |
""| > Decide later                                                         |F=| Stockpile: 20/20 Wood        |
B.|   Keep Guru Teachings                                 250 pts + scroll |="|                              |
.-|     Chakras: a master's bends cost 60% chi, not 80%.                   |"F|                              |
--|     No master yet.                                                     |"F|                              |
--|   Keep Pai Sho                                        250 pts + scroll |Y"|                              |
--|     Pai Sho Table: the White Lotus comes with each winter caravan,     |""|                              |
--|     and request rewards double.                                        |""|                              |
--|     Needs: Known on the road                                           |F"|                              |
--+------------------------------------------------------------------------+""|                              |
-------------.",,,,,,,,,,,,,,,,,,,,,,,.------------..=,,==T,,,,""=F"==FFFFc"F"|                              |
------------..",,,,,,,,,,,,,,,,,,,,,...------------.,,==,,,,,TT,""==FF="FF"F""|                              |
------------."",,,,,,,,,,,c...,,,,...-----------f...,,,=====,,T,TcFFF""fFFFFF"|                              |
------------.Y"""",,,,,,,,..-..,,..-------------..,,,,,,,,,,fTT,,,FFFFF"==F"FF|                              |
------------......,,,,,,Y,.---..,.-------------..,,,,,,,,,,T,=TYT,"TTFFFFFf==F|                              |
-----------------....,,,,,.----.,.-------------.,,,,T,,,,,,,,T=""""F,,F"FFFFF=|                              |
--------------------...,,,.----....---------f-..,,,,,,,,,TT,,,,=="""T,T,F"FFF"|                              |
----------------------..,,.-----f-.---------...,,,,,,,,,,,TT,,Y""="FT,,,,,,"FF|                              |
---------------------f-..,.....------------../,,,,,,,,T,T,,,,,Y"="""T,TT,,,,,,|                              |
.-----------------------..,,,,...-------....,/,,,,,,,,,,,,TT,T"="""""TTT,,,,T,+- seed 7 ---------------------+
 TEACHING  Up/Down choose - Enter answer - Esc decide later                                     PAUSED ? keys
```

- Each teaching row is `Keep <name>`, then its unlock line, then one line on why it might not pay
  yet: an unmet condition (`Needs: Known on the road`) or a fact (`No master yet.`, `No tunnel
  yet.`). That line is the reason the pick is hard, and it comes from the sim, not the UI.
- `Keep …` asks no second confirm. The box already says the other is lost, and a second box
  would be a confirm of a confirm.
- With one teaching left to offer the box shows one `Keep` row. With several offers pending, the
  title reads `LOST TEACHING 1 OF 2`.
- "Decide later" closes the box. The offer waits, an alert reads `A teaching waits for your
  choice [T]`, and the Research screen shows a line at the top of the TEACHINGS column, `!
  Choose a teaching`, where Enter reopens the box.

**The Research screen** with Guru Teachings kept and Pai Sho lost, from
`fn_tui --scenario air-year-two --frame --ascii --size 110x34 --keys 'T <right> <right> <right>
<right> <right> <right> <right>'`, right panel cut. The TEACHINGS column follows the last tier.
The `? 2 more to find` line is question 2 (#734):

```text
+------------------------------- * RESEARCH * -------------------------------+
|<TIER 4                   TIER 5                   TEACHINGS                |
|  - Cartography            - Wind Turbines         >o Guru Teachings        |
|   Locked         250 pts   Locked         250 pts   Available      250 pts |
|   Needs: Writing           Needs: Windmills         Masters' chi x0.75     |
|                                                                            |
|  - Windmills              - Sky Bison              x Pai Sho               |
|   Locked         120 pts   Locked         500 pts   Lost                   |
|   Needs: Gliders           Needs: Meditation, Ai~   Lost to the ruins      |
|                                                                            |
|  - Air Scooter                                     ? 2 more to find        |
|   Locked         250 pts                                                   |
|   Needs: Gliders, Medit~                                                   |
|                                                                            |
| -------------------------------------------------------------------------- |
| Guru Teachings                                 Available  250 pts + scroll |
| Open the seven chakras, and let chi flow freely.                           |
| Unlocks: Masters' bends cost 60% chi, not 80%                              |
| Uses up the teaching scroll read at the Meditation Pavilion                |
+------------------------------------------------ tiers 4-5 of 5, teachings -+
 RESEARCH  Arrows choose - Enter set as target - Esc or T close
```

**A great ruin under the cursor**, and the alert feed as it is found, searched and read. The
glyph is `Π` (ascii `N`) in `data/tui.toml`, beside the lost ruin's `‖`:

```text
+- HERE ----------- (105, 44) -+    +- ALERTS ---------------------+
| Great Ruins                  | <  | Spotted great ruins 12 tiles | <
| Search: 2 scrolls and a      | <  | north                        |
| teaching scroll              | <  | Searched the great ruins: 2  | <
| Plains (105, 44)             |    | scrolls and a teaching       |
+------------------------------+    | A teaching scroll was read.  | <
                                    | Choose a teaching [T]        |
                                    | Kept Guru Teachings. Pai Sho | <
                                    | is lost                      |
                                    +------------------------------+
```

## Measuring it

Phase 5 ends with a harness run, and the economy opening must use the new content (#745):

- **Search.** The opening already designates uncovered ruins (`autoplay.rs`). Great ruins join
  that list.
- **Pick.** The opening keeps the lower `TechId` in file order and the higher in reverse order,
  so the panel's two research orders measure both sides of every draft.
- **Build.** A kept teaching joins the research order after the branch. Its building is placed
  by the existing "one of every reachable building" rule, with the placement rules above.

The run reports these, on the panel's 32 seeds and both research orders, against a baseline from
`main` before the first phase-5 merge:

| Measure | Metric | Expected |
|---|---|---|
| Found | a new `great_ruins_found_by.M` (count uncovered) at 60, 120 and 180 | Air finds its first by 60 on most seeds. Others by 120 once the Map Room opens |
| Drafted | a new `teachings_kept` at 240, and `first_offer_minute` | At least one teaching kept by 240 on 24 of 32 seeds per nation |
| Dead drafts | a unit test over seeds 1 to 32 | No offer holds a teaching whose site rule fails |
| Empty scrolls | `empty_scrolls` (scrolls read with nothing to offer) | Under 1 per run on Small. Medium's third ruin is the expected case |
| Reach | `balance supply --resources great_ruins --radii 40,90` (#744) | At least one great ruin reachable on foot within 90 on 28 of 32 seeds |
| No regressions | every existing target, through `balance verdict --against` | `scrolls_by.M` unchanged or earlier (a great ruin still gives 2), and no gate target moves |

The balance follow-up records the measured numbers in a "Measured" section here. The first levers
are `[resource.promote]` (count, spacing), the Map Room's `finds` order, and each teaching's own
numbers, all data.

## Risks

- **The third great ruin is often empty.** A pool of four with one lost per draft runs dry after
  two drafts. Medium and Large maps have three great ruins, so the third reads for 100 points.
  That follows from owner decision 5 (the other is lost), and question 3 asks whether that is the
  wanted answer.
- **Great ruins on another landmass.** The picker favours broad land but doesn't know the site.
  The reach measure catches it. If it fails, the lever is to score size by the largest connected
  land region as well, still in mapgen and still deterministic.
- **Air finds them first.** Glide plus Project make Air the seeker nation. That is in character
  (research-horizons.md gives Air the two spirit teachings), and the Map Room keeps the other
  three in reach. The `great_ruins_found_by` spread per nation shows whether it is too wide.
- **#437 lands in a shape this doesn't expect.** Pai Sho assumes pacing.md's deal lists and
  requests. If #437 draws deals differently, the White Lotus deals still come from their own
  stream among deals not shown, and the visit's first three never change because of a table.
- **Mastery ranks (#358) slip.** Air's pool is three until `guru_teachings` lands. The draft works
  with any pool size, so nothing else waits on it.

## Open questions for the owner

1. **Does the draft open by itself and pause the game?** *Recommended: yes, once per offer, with
   "Decide later" as the first row.* The read happens at a hall, often while the player is busy
   elsewhere, and a choice that loses a teaching deserves the player's full attention. The
   alternative is an alert only (`A teaching waits [T]`), which never interrupts but is easy to
   miss for an hour. Either way the offer waits, so nothing is lost by closing it. Section: "The
   draft", mockup. (#733)
2. **Does the Research screen show how many teachings are still hidden?** *Recommended: nothing
   until the first teaching scroll is read, then a count (`? 2 more to find`), never names.* The
   softer-sign decision (cartography.md, 2026-11-09) prefers discovery to answers handed out, and
   adding a hint later is easier than taking one away. A count after the first draft tells the
   player there is more out there without spoiling which. The options either side are no count
   ever, or greyed names from tick 0 (a spoiler, and a checklist). Section: "Hidden, kept, lost".
   (#734)
3. **What does a teaching scroll hold when nothing is left to offer?** *Recommended: 100
   knowledge points (`empty_scroll_knowledge`).* It keeps owner decision 5 exact: a lost
   teaching stays lost. The alternatives are to offer a lost teaching back, which softens "pick 1
   of 2" into "pick 1 now", or to offer another nation's teaching, which brings cross-nation
   learning before the gate, against owner decision 3's post-gate scope. Section: "The draw".
   (#735)

## Follow-up issues

Each one appends its own tech, building, ability, item and trade blocks from this doc. `TechId`,
`BuildingId`, `AbilityId`, `ItemId` and `ResourceId` are file positions, and saves remap them by
name (save-compat.md), so the order between follow-ups doesn't matter.

| Issue | Title | Blocked by |
|---|---|---|
| #736 | Sim, data and mapgen: great ruins (the promote pass, `also` yields, `finds`, notices, the old-save upgrade) | none |
| #737 | Sim and data: teachings and the draft (`[tech.teaching]`, `reveals`, `teachings_rng`, `KeepTeaching`, site rules) | #736, #673, #707, #735 |
| #738 | Sim and data: `pai_sho` and `crystal_growing` (Pai Sho Table, White Lotus deals, Crystal Grotto, glow crystal rows) | #737, #437 |
| #739 | Sim and data: `spirit_library` and `spirit_water` (spirit seats, vials) | #737, #674 |
| #740 | Sim and data: `sandbending`, `plantbending` and `spirit_projection` (`biome_speed`, `on_biomes`, the Sandstone Works, Regrow, Project) | #737 |
| #741 | Sim and data: `sun_stone` and `hot_springs` (`[building.chi_regen]`, the Bathhouse) | #737 |
| #742 | Sim and data: `guru_teachings` and the `master_chi` modifier | #737, #358 |
| #743 | TUI: the draft box, the TEACHINGS column, the great ruin glyph and readouts | #736, #737, #733, #734 |
| #744 | Dev tools: great ruins in `balance supply`; offers and a `keep_teaching` step for agents | #736, #737 |
| #745 | Balance: the economy opening uses phase 5, and the phase-5 run | #736, #737, #738, #739, #740, #741, #713 |
| #733 | Decision: does the teaching draft open by itself and pause the game? | none |
| #734 | Decision: does the Research screen show how many teachings are still hidden? | none |
| #735 | Decision: what does a teaching scroll hold when nothing is left to offer? | none |
