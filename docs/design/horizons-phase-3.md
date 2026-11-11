# Research horizons, phase 3: trade techs

Doc status: current

Status: **proposed** (#667). This is phase 3 of [research-horizons.md](research-horizons.md)
(accepted 2026-11-10). It follows the shape of phase 1's design
([horizons-phase-1.md](horizons-phase-1.md), #665) and phase 2's (horizons-phase-2.md, #666),
and reuses their patterns: the `core` rule (#673), the `upgrades` unlock kind (#674) and fuel
bunkers (#675). It turns three shared techs into two buildings, a second Workshop recipe, two new
items and TOML. It is designed against the caravan spec in [pacing.md](pacing.md), section 2,
which #437 implements. The one owner question is under "Open questions for the owner", and the
implementation work is under "Follow-up issues".

## Decisions in brief

1. **Three shared techs that tie research to the caravans.** `tea_ceremony` (the Tea House),
   `trade_wares` (the Workshop's second recipe, wares) and `archives` (the Archive: more study
   seats and scribing). None of them is `core`, so the gate doesn't grow.
2. **A second recipe is an `upgrades` property, set per building.** A building may list
   `[[building.alt_recipe]]` entries, and a tech unlocks one with `[[tech.unlocks.upgrades]]
   recipe = "..."`. The player switches a building with `Command::SetRecipe`. Switching
   mid-batch drops the delivered inputs beside the building, so nothing is lost but the work.
3. **Scribing is what the pool used to throw away.** With an Archive standing and no research
   target, knowledge above `idle_knowledge_cap` flows into a scribing counter instead of being
   trimmed. Every 150 points becomes one **scroll copy** at the Archive. A copy is a plain
   stored item. It is never read and never counts as a scroll read, and caravans take it in deals
   and requests. Without an Archive, the cap behaves exactly as today.
4. **The Tea House is a herb bunker with three effects.** While it holds herbs: a caravan that
   makes camp shows one more deal and stays 4 minutes longer, and draw gains a small term. It
   burns 12 herbs an hour, which is the decision: herbs go to tea or to the sick.
5. **Two research conditions arrive early.** `trade_wares` needs the "Known on the road" chapter
   and `archives` needs 3 scrolls read (research-horizons.md's own table). Phase 3 ships
   `requires` with just those two keys. Phase 6 extends it with population, masters and
   residents.
6. **The only caravan changes are the Tea House's.** That means the extra deal and the longer
   stay, plus new deal and request rows in `data/trade.toml` that take wares and scroll copies.
   Arrival, camp placement, the deal shape and the request rewards stay as #437 builds them.

## What ships

| Tech | Nation | Cost | Needs | Unlocks | The decision |
|---|---|---|---|---|---|
| `tea_ceremony` | shared | 120 | `herbalism`, `pottery` | **Tea House** (herb bunker: a 4th deal, a longer stay, a little draw) | Herbs go to tea or to the sick. Before or after the first caravan? |
| `trade_wares` | shared | 250 | `carpentry`, chapter "Known on the road" | The Workshop's **wares** recipe | Each Workshop makes tools or wares. Wood becomes stone and ore at about twice the raw rate |
| `archives` | shared | 500 | `writing`, 3 scrolls read | **Archive** (+2 seats for study halls in reach, scribing) | Scholars research or scribe. Which halls the Archive stands among |

Every nation gets the same 870 points. File order puts the three after phase 2's techs, so the
economy opening reaches them from about minute 150, though `tea_ceremony` is cheap enough to
pull forward by hand. That suits the spec's timeline: tea in the settling stretch, wares in the
midgame once caravans come, and the Archive on the road to the gate.

## The `core` rule, unchanged

All three techs leave `core` at its default, false (horizons-phase-1.md, "The rule every phase-1
tech follows"). The branch sign doesn't count them, and the building-kinds sign doesn't count
the Tea House or the Archive. A second Workshop recipe changes nothing about the Workshop's own
`core` status. The unit test from #673 that pins each nation's two targets runs again with
phase-3 data loaded.

## Shared sim work

### 1. Research conditions: `requires`, two keys

```toml
[[tech]]
id = "archives"
requires = { scrolls_read = 3 }                 # the "scrolls_read" tally the scrolls sign counts

[[tech]]
id = "trade_wares"
requires = { chapter = "known_on_the_road" }    # a [[chapter]] id in data/endeavor.toml (#439)
```

- **Schema.** `requires` is a table with `deny_unknown_fields`, so phase 6 can add keys
  (`population`, `masters`, `winters_survived`, `residents`) and an unknown one fails to load
  today. `scrolls_read` is 1 to 20. `chapter` must name a chapter that exists. Both may be set
  at once.
- **Sim.** `Research::set_target` takes the facts it needs (a small `Facts` struct the world
  fills: scrolls read, chapters met). A target whose queue would hold a tech with an unmet
  condition is refused with `ResearchError::NotYet`, which reads `needs 3 scrolls read` or `needs
  Known on the road`. Both phase-3 conditions only ever become true: a tally never falls and a
  chapter stays met. So the check at queue time is enough. Phase 6 must recheck on every
  completion once it adds a condition that can fall, such as population.
- **Research screen.** The Locked line lists conditions after prereqs: `Needs: Writing, 3
  scrolls read`. A condition that is already met drops off the list, the way a known prereq
  does. The autoplay research order skips a tech whose condition is unmet and comes back to it.
- **Why now and not in phase 6.** Without the chapter, `trade_wares` could be researched before
  any caravan can buy wares, which would be a trap on the Research screen. Without the scroll
  condition, the Archive would be one more 500-point tech, and the spec ties it to the scrolls
  that the scrolls sign already asks for.

### 2. A second recipe per building

```toml
# data/buildings.toml, workshop (edited in place: its id and position don't move)
[building.recipe]          # unchanged: tools from wood, the building's first recipe
output = "tool"
output_count = 1
work = 40.0
modifier = "construction"
input = [{ item = "wood", count = 3 }]
stock_limit = 6

[[building.alt_recipe]]    # new: any number of entries, each with the [building.recipe] fields
id = "wares"               # unique within the building; what saves and upgrades name
output = "wares"
output_count = 2
work = 30.0
modifier = "construction"
input = [{ item = "wood", count = 4 }]
stock_limit = 30
```

- **Schema.** `[[building.alt_recipe]]` needs a `[building.recipe]` on the same building, and an
  `id` that is unique within it. It is resolved with the same `resolve_recipe` checks. The
  first recipe has no id and shows under its output's name (`Tools`). An alt recipe shows under
  its own output's name (`Wares`).
- **Unlock.** #674's upgrade gains a second property:

  ```toml
  [[tech.unlocks.upgrades]]
  building = "workshop"
  recipe = "wares"         # names an alt_recipe on that building
  ```

  #674's rules carry over. An upgrade names exactly one property. The building must already
  have it, which here means an alt recipe of that id. Each (building, recipe) pair is upgraded
  by at most one tech. An alt recipe that no tech names is available from the start, as an
  unlocked building is.
- **State.** The building record gains `recipe: Option<String>`, the chosen alt recipe's id,
  with serde default `None` meaning the first recipe. Saves name it by id, so reordering a
  building's alt recipes doesn't break them (save-compat.md). A save whose id no longer exists
  falls back to the first recipe.
- **Command.** `Command::SetRecipe { pos, recipe: Option<u8> }`: `None` is the first recipe and
  `Some(i)` is the `i`-th alt recipe. It is refused if the building has no such recipe, with
  `needs Trade Wares` while the unlocking tech is unknown, and on a building still under
  construction. A site keeps its default. Setting the recipe it already has is a no-op.
- **Switching mid-batch.** The batch's delivered inputs drop beside the door as ordinary stacks,
  as a deconstructed building returns them (#342), and its work progress is lost. The crafter
  keeps the job and starts fetching for the new recipe. `craft_idle` reads the chosen recipe's
  `stock_limit`, so a Workshop on wares idles at 30 wares whatever the tools count.
- **Every recipe system reads the chosen recipe.** That means `run_craft`, `craft_idle`, the
  inspect line and the power boost. A `World::recipe_at(pos, defs) -> Option<&RecipeDef>`
  helper is the single place that answers. The forge, kiln, smelter and Mine Shaft have no alt
  recipe, so nothing changes for them.

### 3. Scribing: what `idle_knowledge_cap` does with an Archive

Today, `Research::add_toward_target` clamps the pool at `cap_millis`: the front tech's cost, or
`idle_knowledge_cap` (10 points) with nothing queued. Anything above is lost, and study seats
are not claimed at all while the queue is empty (#284). Phase 3 keeps every part of that and
adds one branch.

**Scribing is on** when all three hold:

- the queue is empty;
- at least one Archive is finished;
- fewer than the Archive's `stock_limit` scroll copies are in stores.

| | No Archive (or scribing off) | Archive standing, scribing on |
|---|---|---|
| Pool with a target | Capped at the front tech's cost, as today | The same. Nothing scribes while a target is set |
| Pool with no target | Capped at `idle_knowledge_cap` (10). Anything above is lost | Still capped at 10, kept as the bootstrap for the next pick. Anything above goes to `scribe_millis` |
| Study seats with no target | Not claimed, so scholars become idle hands | Claimed as usual (still the lowest job tier) |
| Passive trickle with no target | Stalls at the cap | Its overflow scribes too: about one copy every 50 minutes with no scholars |
| Clearing the target | Not offered (`ClearResearch` needs an Archive) | Banked knowledge above 10 moves to `scribe_millis` |

- **State.** `Research` gains `scribe_millis: u64` (serde default 0). `add_toward_target` takes a
  `scribing: bool`. When it is true, whatever the clamp trims is added to `scribe_millis`
  instead of being dropped. The pool's own arithmetic doesn't change, so every existing #284 test
  holds.
- **Copies.** After the research tick, while `scribe_millis` is at least `copy_points` × 1000,
  one `copy_item` drops beside the door of the first finished Archive in row-major anchor order,
  with an ordinary Haul job, and `copy_points` × 1000 is spent. A `scrolls_copied` tally counts
  them for readouts and the harness. `copy_points` is not scaled by the difficulty preset's
  `research_cost`, for the same reason the idle cap isn't: it is a buffer size, not a tech cost.
- **Taking the last Archive down.** `scribe_millis` keeps what it holds, and scribing resumes
  when an Archive stands again. Nothing is lost.
- **`Command::ClearResearch`** empties the queue. It is refused without a finished Archive
  (`needs an Archive`), because its only use is to start scribing. This is how the player picks
  "scribe for trade" over "keep researching" while techs are still left. Once the tree is done
  the queue is empty anyway, and scribing starts on its own.
- **Why the overflow, and not a scribe job.** The issue's rule is "idle knowledge becomes scroll
  copies". The overflow is exactly the idle knowledge, so the seats, the power boost, the `study`
  modifier and the passive trickle all feed scribing through the code that feeds research. A
  separate Scribe job would duplicate all of it and add a third seat kind beside Study and
  Chart.
- **Why a stock limit.** At four scholars a copy comes every 3 minutes, and caravans come every
  15. Without a limit copies pile up for the rest of the game. At 12 copies scribing pauses, the
  seats free up, and the readout says why. That is the same rule as a recipe's `stock_limit`
  (#350).
- **No loop.** A scroll copy has no `knowledge` field, so it is stockpiled, never read, and the
  scrolls sign never counts it. `data/trade.toml` refuses a deal or request that takes scroll
  copies and pays `knowledge` (#711), so Air's monks can't turn copies back into points.
  Owner decision 6 holds: there are no repeatable studies, and knowledge leaves the tree only as
  a trade good.

### 4. Small pieces

- **New items** in `data/jobs.toml`: `wares` and `scroll_copy`. Both are plain stored items, with
  no `knowledge` and no `stands_for`.
- **The Tea House's effect section** `[building.tea_house]` (below) is read in two places: the
  caravan's arrival (#437) and the draw multiplier (#435).
- **Seats in reach.** #674's `World::seats_at(pos, defs)` adds the best Archive bonus in reach to
  the def's seats and the known upgrades. #674 opens missing seats when a tech completes. Phase 3
  turns that into one `sync_seats` that also closes extra seats: unclaimed ones first, then the
  highest `JobId`, whose scholar steps out. It runs when a tech completes, when a study hall or
  Archive finishes or comes down, and after load. A hall's `capacity` follows `seats_at`, as
  #674 has it.

## The techs

Numbers are starting values for the harness (see "Measuring it"). Hover text means the build
modal's effects list and the cursor panel's inspect lines, in the voice of text-voice.md.

### `tea_ceremony`: the Tea House

```toml
# data/research.toml, appended
[[tech]]
id = "tea_ceremony"
name = "Tea Ceremony"
description = "Brew herbs into tea for guests, and the traders stay a while."
cost = 120
prereqs = ["herbalism", "pottery"]
[tech.unlocks]
buildings = ["tea_house"]

# data/buildings.toml, appended
[[building]]
id = "tea_house"
name = "Tea House"
verb = "Build"
work = 60.0
passable = false
footprint = [2, 2]
[building.tea_house]
extra_deals = 1           # a caravan that makes camp while a house is open shows 4 deals, not 3
extra_stay_minutes = 4.0  # and stays 13 minutes, not 9
draw = 0.1                # a term in #435's draw multiplier while a house is open
[building.fuel]
item = "herbs"
bunker = 10
per_minute = 0.2          # 12 herbs an hour, burned whenever the house is finished and stocked
[[building.cost]]
item = "wood"
count = 14
[[building.cost]]
item = "stone"
count = 6
[[building.cost]]
item = "herbs"
count = 4
```

- **Rule.** A Tea House is **open** while its bunker holds at least one herb (#675). It burns
  all the time, not only during a visit, because draw runs all the time. Two houses don't stack:
  each effect asks whether any house is open.
- **The caravan.** When a caravan makes camp (pacing.md section 2, #437), the sim checks once
  for an open house. If one is open, the visit's stay is `stay_minutes + extra_stay_minutes`, and
  `extra_deals` more deals are drawn from `rng::stream(seed, "trade.tea")` among the caravan's
  deals not already shown. The first three are drawn exactly as without a house, so building one
  never changes which three a visit offers. A caravan with fewer deals in its list shows them
  all. If the house runs dry mid-visit, the visit keeps what it got.
- **Draw.** #435's multiplier gains a term: `+ draw` while any house is open, under the same
  `max` (1.4). For a player who already keeps two foods, warm beds and two spare beds, it adds
  nothing. For everyone else it is worth half a spare bed. Whether it should lift the ceiling is
  question 1 below (#706).
- **Numbers.** 12 herbs an hour is about half of what one tended Herb Garden grows (2 herbs per
  5-minute crop). A hurt settler takes one herb per remedy (harm.md). So a settlement with one
  garden and a Tea House has few herbs to spare for injuries, which is the trade the spec asks
  for. A 4th deal is a third more choice per visit, and 4 more minutes is a 44% longer stay, which
  is time for haulers to finish the deals they took.
- **Hover.** `Traders stay 4 minutes longer`, `Traders offer 1 more deal` and `Burns 12 herbs an
  hour`. Inspect: `Open`, `Herbs: 7/10`, or `Closed: out of herbs` as a status. The draw line on
  the Endeavor screen gains `Tea House` among its reasons (#436).
- **The decision.** Herbs go to tea or to the sick. Built before the chapter, a Tea House is a
  small draw term that eats herbs. Built in time for the first caravan, it gives the visit a 4th
  deal and the haulers time to finish. The fourth deal matters most when the caravan sells what
  the site lacks, such as ore on seeds 2 and 6.

### `trade_wares`: the Workshop's wares

```toml
# data/research.toml, appended
[[tech]]
id = "trade_wares"
name = "Trade Wares"
description = "Carve bowls, boxes and toys that traders pay well for."
cost = 250
prereqs = ["carpentry"]
requires = { chapter = "known_on_the_road" }
[tech.unlocks]
[[tech.unlocks.upgrades]]
building = "workshop"
recipe = "wares"

# data/jobs.toml, appended
[[item]]
id = "wares"
name = "Wares"
```

The Workshop's `[[building.alt_recipe]]` is in "Shared sim work, 2" above.

- **Rule.** Once `trade_wares` is known, each Workshop can be set to Tools or Wares. A new
  Workshop starts on Tools, and the player switches the ones they want.
- **Numbers.** Two wares from 4 wood in 30 s, at `construction` 1.0. That is 4 wares and 8 wood a
  minute per Workshop, scaled by `construction`: Earth's 1.5 makes 6 a minute and Air's 0.9
  about 3.6, before carpentry and masonry raise it. At the stock limit of 30 it idles, which
  takes about 8 minutes at 1.0, half a caravan interval. The caravan deals (#711) price a
  ware at about 4 wood where raw wood buys goods. A ware costs 2 wood, so wood in wares buys
  about twice what raw wood does:

  | Caravan | Raw wood deal (pacing.md) | Wares deal (#711) |
  |---|---|---|
  | Earth | 25 wood for 3 gold ore | 6 wares for 3 gold ore |
  | Fire | 40 wood for 8 bricks | 10 wares for 8 bricks |
  | Water | (new) | 8 wares for 4 herbs |
  | Air | (new) | A request: 12 wares for half a newcomer's draw |

- **Fire.** Fire's forge makes tools from metal, so a Fire settlement can set every Workshop to
  wares. That gives Fire's long chain a second outlet for its large wood stock, and it costs no
  data line.
- **Hover.** The Research detail reads `Unlocks: Workshop makes Wares`. The build modal's
  Workshop entry gains `Makes Tools or Wares`. Inspect: `Making: Wares`, then the batch line as
  today (`Batch: 4/4 Wood, 60%`), or `Idle: 30 Wares in stores`.
- **The decision.** Each Workshop makes tools or wares, and a crafter's hands go with it. It
  turns the wood glut into stone and ore at about twice the raw rate. But wares are worth
  something only while a caravan is camped. A settlement that builds a wares Workshop and skips
  the deals has turned wood into a pile.

### `archives`: the Archive

```toml
# data/research.toml, appended
[[tech]]
id = "archives"
name = "Archives"
description = "Keep a hall of records, and copy what the scholars know for the traders."
cost = 500
prereqs = ["writing"]
requires = { scrolls_read = 3 }
[tech.unlocks]
buildings = ["archive"]

# data/buildings.toml, appended
[[building]]
id = "archive"
name = "Archive"
verb = "Build"
work = 120.0
passable = false
footprint = [2, 2]
[building.archive]
hall = "study_hall"       # the building whose seats it adds to; must have seats > 0
seats = 2                 # added to each such building in reach. Two Archives don't stack
radius = 6                # Chebyshev, footprint to footprint. Not modifier-scaled
copy_item = "scroll_copy"
copy_points = 150         # idle knowledge per copy
stock_limit = 12          # scribing pauses while this many copies are in stores
[[building.cost]]
item = "wood"
count = 20
[[building.cost]]
item = "stone"
count = 12
[[building.cost]]
item = "tool"
count = 2

[building.names]
earth = "Hall of Records"
fire = "Royal Archive"
water = "Story Lodge"
air = "Temple Archive"

# data/jobs.toml, appended
[[item]]
id = "scroll_copy"
name = "Scroll Copy"
```

- **Seats in reach.** Every study hall whose footprint is within `radius` of an Archive's
  footprint gains `seats` places, through `seats_at` and `sync_seats` above. With `writing`'s +2,
  a hall in reach seats 6. The Map Room is a different building (`hall` names `study_hall`), so it
  keeps its one seat. The Archive holds no seats itself, so it is a placement question and not
  another hall.
- **Scribing.** "Shared sim work, 3". Copies drop at the Archive and are hauled to stores.
- **Numbers.** Four scholars with `writing` make about 45 points a minute, plus the passive 3.
  That is a copy every 3 minutes and the 12-copy limit in about 40 minutes. A caravan comes every
  15 minutes, and its deals and request take 2 to 5 copies (#711). So a steady scribing
  settlement keeps about half its scholars busy over a season, and the rest go idle at the limit.
  Before the tree is done, a scholar's hour is about 4 copies or about 700 points toward a tech.
- **Hover.** `Study halls within 6 tiles gain 2 seats`, `Copies scrolls when nothing is being
  researched` and `1 copy for every 150 research points`. Inspect: `Halls in reach: 2`,
  `Copying: 90/150`, `Copies made: 9`, or `Copying paused: 12 in stores` as a status.
- **The settlement panel.** With an Archive and no target, the Research row reads `Copying
  scrolls: 90/150` instead of `Research: none set`.
- **The decision.** Scholars keep researching or scribe for trade. Before the gate, clearing the
  target trades the next tech for copies, which buy metal, tools and stone from the caravans: the
  gate's own materials. After the tree is done, the Archive turns idle halls into trade. It also
  asks where: an Archive among two halls adds four seats, and one beside a single hall adds two.

## Caravans: what they take

Follow-up E appends these to #437's `data/trade.toml`. Deals stay fixed barters and requests stay
one-off gifts. The rows are starting values for #440's balance pass.

```toml
# data/trade.toml, appended to each nation's [[caravan]]

# Earth traders
[[caravan.deal]]
give = [{ item = "wares", count = 6 }]
get = [{ item = "gold_ore", count = 3 }]
times = 2
[[caravan.deal]]
give = [{ item = "scroll_copy", count = 1 }]
get = [{ item = "stone", count = 15 }]
times = 2
[[caravan.request]]
name = "Copies for the library at Ba Sing Se"
give = [{ item = "scroll_copy", count = 3 }]
reward = { draw = 0.5 }

# Fire traders
[[caravan.deal]]
give = [{ item = "wares", count = 10 }]
get = [{ item = "brick", count = 8 }]
times = 2
[[caravan.deal]]
give = [{ item = "scroll_copy", count = 2 }]
get = [{ item = "metal", count = 3 }]
times = 1

# Water traders
[[caravan.deal]]
give = [{ item = "wares", count = 8 }]
get = [{ item = "herbs", count = 4 }]
times = 2
[[caravan.deal]]
give = [{ item = "scroll_copy", count = 1 }]
get = [{ item = "tool", count = 2 }]
times = 2

# Air traders: monks buy with what they sell, charts and newcomers, through requests
[[caravan.request]]
name = "Gifts for the festival at the Western Air Temple"
give = [{ item = "wares", count = 12 }]
reward = { draw = 0.5 }
[[caravan.request]]
name = "Copies for the Eastern Air Temple"
give = [{ item = "scroll_copy", count = 2 }]
reward = { chart = 1 }                   # never knowledge: see the validation below
```

- **Validation.** A deal or request that takes `scroll_copy` may not give or reward `knowledge`.
  The file fails to load if one does.
- **Shown only when it can be paid?** No. #437 shows any deal, and pacing.md accepts deals whose
  items the visited nation can't use yet. A wares deal before `trade_wares` is a hint, as Fire's
  metal is to Water.
- **Water's herbs.** Herbs are the Tea House's fuel, so a Water caravan closes the loop: wares
  for herbs, and herbs for a longer stay.
- **At least 4 deals a list.** Earth, Fire and Water each gain two deals here. Follow-up E checks
  that every caravan's list has at least 4, so the Tea House's 4th deal always has something to
  show, and adds one to Air's list if #437 ships it with 3.
- **Air through requests.** Air's traders sell knowledge and charts. Copies for knowledge would be
  a loop, so Air takes wares and copies through requests whose rewards are a chart or draw. Both
  reward kinds are #437's own.

## Mockups

Hand-edited from `fn_tui --scenario water-year-two --frame --ascii --size 110x34 --keys 'T'`
and the default frame of `earth-year-two`. The new lines are marked `<`, and the TUI follow-up
settles the keys.

The keys (#712): `R` in Select mode sets the building under the cursor to its next recipe, and
`X` on the Research screen clears the target. The Traders line reads `Tea House: 1 more deal`
whenever the visit shows more deals than `deals_per_visit`, and the mockups below differ from the
frames in layout (the Archive's lines follow `Built (x, y)`, and no batch line is drawn yet).

The Research detail for the three techs, Water, before the chapter and with 2 scrolls read:

```text
| -------------------------------------------------------------------------- |
| Tea Ceremony                                            Available  120 pts | <
| Brew herbs into tea for guests, and the traders stay a while.              | <
| Unlocks: Tea House                                                         | <
| -------------------------------------------------------------------------- |
| Trade Wares                                                Locked  250 pts | <
| Carve bowls, boxes and toys that traders pay well for.                     | <
| Needs: Known on the road                                                   | <
| Unlocks: Workshop makes Wares                                              | <
| -------------------------------------------------------------------------- |
| Archives                                                   Locked  500 pts | <
| Keep a hall of records, and copy what the scholars know for the traders.   | <
| Needs: 3 scrolls read                                                      | <
| Unlocks: Story Lodge                                                       | <
```

The side panel with an Archive and no target, and a Workshop set to wares under the cursor:

```text
+- SETTLEMENT ----------- [N] -+
| People 16             Idle 2 |
| Food 228 - 14/head  mild     |
| Wood 1640  Stone 392         |
| Copying scrolls: 90/150      | <
| Gate 6/9 - 2 scrolls     [G] |
+- HERE ------------ (61, 40) -+
| Workshop                     |
| Making: Wares                | <
| Batch: 4/4 Wood, 60%         |
| Powered x1.5                 |
```

An Archive and a Tea House under the cursor:

```text
+- HERE ------------ (58, 37) -+    +- HERE ------------ (66, 42) -+
| Hall of Records              | <  | Tea House                    | <
| Halls in reach: 2            | <  | Open                         | <
| Copying: 90/150              | <  | Herbs: 7/10                  | <
| Copies made: 9               | <  |                              |
+------------------------------+    +------------------------------+
```

The Traders screen with a Tea House open, edited from pacing.md's mockup. The 4th deal is the
house's, and the stay reads 13 minutes on arrival:

```text
+--------------------------- * TRADERS * --------------------------+
| Fire Nation traders                             Leave in 11 min  | <
| Camp at (88, 60)                                                 |
|                                                                  |
| DEALS                                       Taken     In stores  |
| > 30 food for 2 metal                        0/2       212 food  |
|   40 wood for 8 bricks                       1/3       847 wood  |
|   25 wood for 3 gold ore                     0/2       847 wood  |
|   2 scroll copies for 3 metal                0/1     5 copies    | <
|   Tea House: 1 more deal                                         | <
|                                                                  |
| REQUEST                                                          |
|   Food for the families at Fire Fountain City                    |
|   40 food                                    Draws a newcomer    |
+- Up Down choose  Enter take  X cancel  Esc/M close --------------+
```

## Measuring it

Phase 3 ends with a harness run, and the economy opening must use the new content (#713):

- **Tea House.** One, once `tea_ceremony` is known and an Herb Garden stands, beside the
  stockpile.
- **Wares.** Once `trade_wares` is known, build one more Workshop and set it to wares. Every
  nation keeps one Workshop on tools. Fire, whose forge makes tools, sets all of its Workshops to
  wares.
- **Archive.** Once `archives` is known, place it within reach of the most study halls.
  Clearing the target is never scripted. Scribing starts when the opening runs out of techs.
- **Deals.** #440 teaches the opening to take caravan deals. It gains the wares and scroll copy
  rows, taken whenever stores can pay.

The run reports these, on the panel's 32 seeds and both research orders, against a baseline from
`main` before the first phase-3 merge:

| Measure | Metric | Expected |
|---|---|---|
| Decision timeline | `longest_gap`, and the `timeline:` line | The 150-to-gate stretch gains 2 or 3 decisions per nation |
| Wood at 180 | `wood_at.180` | Falls by a wares Workshop's draw. Fire falls most |
| Stone, ore and tools | `sign_minute.stone`, `sign_minute.ore` | Earlier or unchanged for every nation, from wares and copies bought as ore, stone and metal |
| Herbs and harm | a new `herbs_at.M`, and `no_deaths_by_180` | Herbs fall by up to 12 an hour. No rise in deaths: tea mustn't starve the sick |
| Scribing | a new `copies_by.M` (the `scrolls_copied` tally) at 240 | At least 6 copies in a run whose tree is done, and idle share after the tree falls |
| No regressions | every existing target, through `balance verdict --against` | `ready_window` may move earlier from the Archive's seats. If it moves past the target, the first lever is the Archive's `seats` |

The balance follow-up records the measured numbers in a "Measured" section here. The first
levers are the Tea House's `per_minute`, the wares deals' counts, `copy_points` and the Archive's
`stock_limit`, all data.

### Measured

The phase-3 run (#713): seeds 1 to 32, all four nations, Small, Normal, both research orders
(`steady` and `branch_first`), 256 runs a set, through `balance verdict`. The runs are **240
minutes** long, not 180, because the three techs land at 140 to 240 and a 180-minute run would
end before most of what they do. Three sets, so phase 3's effect can be told from #440's:

- **base**: `main` at fec8e9d, before phase 3's first merge (and before #440);
- **main**: `main` at 398d6cf, after #440's opening and all of phase 3's sim, TUI and data (#711),
  but with the opening ignoring phase 3 except that its one-of-everything sweep builds a Tea House
  and an Archive at the anchor when it can pay;
- **now**: this change.

`herbs_at.M` and `copies_by.M` were back-ported to the two baselines so all three sets read them
(copies are 0 at base: no Archive exists). `first_built` now lists every building the opening
raises, the optional ones too, and a run record's `paid_in` counts completed deals by the good they
paid in; those two only exist for "now". Medians per nation, `steady` unless said; "base / main /
now". Ore numbers predate #834 (a bought ore doesn't count as mined, so the caravan's ore deals
don't help the ore sign yet).

**What the opening does with it** (`crates/sim/src/autoplay/trade_techs.rs`, `pacing.rs`):

- **Tea House**: one, beside the stockpile, once `tea_ceremony` is known and an Herb Garden is
  finished.
- **Wares**: once `trade_wares` is known, a second Workshop. Every Workshop but one is set to wares
  (all of them where another building makes tools, which is Fire's forge: read from the recipes,
  not the nation). A wares Workshop goes back to tools when stores hold less wood than the
  buildings still wanted cost, and goes to wares only with 30 more than that.
- **Archive**: once `archives` is known and a study hall stands, on the tile with the most study
  halls in its reach (ties to the nearest the settlement's anchor). With no free tile in reach of
  any hall it stands as near the first hall as it fits.
- **Far wood**: while the Tea House or the Archive is wanted and its wood isn't in hand (the
  stores and the ground, an unfinished site included), crews cut the nearest reachable trees past
  `FORAGE_RADIUS` (within `DEPOSIT_RADIUS`), as many as the shortfall takes.
- **Deals**: wares and scroll copies are spared in full (no threshold), less what taken deals
  already owe the camp, and a deal that pays in them goes first. Clearing the target is never
  scripted.

**What the first runs found**, all three about wood or crowding and none in the design:

- The opening has no wood left by the time these techs land. `wood_at.180` is 10, 42, 106 and 1
  on `main` (Air, Water, Earth, Fire) against 192, 200, 349 and 82 at base, because #440's deals
  and every phase-2 building spend the forest within 15 tiles and nothing refills it. With only
  the plain rules and the reserve, the Tea House stood in about 16 to 24 runs of 32 and the Archive
  in 5 to 7 (14 and 20 wood), and a wares Workshop that ran unchecked took the last of what there was. Hence the wares
  rule's reserve and the far-wood rule. The far-wood rule has to count a designated but unfinished
  site as still lacking: an Archive site waiting for wood the beds had taken stood unfinished
  for the rest of the run on Air seed 1.
- By minute 160 the core is full of stockpile tiles (83 on a trace of Fire seed 12 at minute 200),
  buildings and rock. On Air seed 1 no tile within 6 of the study hall was free from minute 170 on,
  so the Archive's reach rule found no site and none was built. The fallback to the nearest fit costs the seats (`archive stood` counts it either way)
  and keeps the scribing.
- The research tree is done in most runs: each nation's last tech is `archives`, at minute 160,
  204, 219 and 187 (median, Air, Water, Earth, Fire, `steady`), which is the Archive's own tech,
  so scribing starts as soon as the Archive stands.

| Measure | Air | Water | Earth | Fire |
|---|---|---|---|---|
| `tea_ceremony` / `trade_wares` / `archives`, median minute | 139 / 148 / 160 | 179 / 189 / 205 | 192 / 201 / 220 | 169 / 173 / 187 |
| Tea House stands by 240 (runs of 32), first minute | 28, 144 | 28, 187 | 31, 194 | 27, 176 |
| Archive stands by 240 (runs of 32), first minute | 22, 158 | 19, 212 | 27, 224 | 18, 188 |
| `food_at.180` | 337 / 260 / 250 | 136 / 158 / 161 | 130 / 146 / 145 | 125 / 154 / 160 |
| `wood_at.180` | 192 / 10 / 1 | 200 / 42 / 34 | 349 / 106 / 106 | 82 / 1 / 0 |
| `wood_at.180`, `branch_first` | 192 / 19 / 4 | 152 / 44 / 44 | 426 / 382 / 382 | 146 / 104 / 104 |
| `wood_at.240` | 192 / 10 / 0 | 200 / 7 / 6 | 346 / 53 / 12 | 52 / 0 / 0 |
| `herbs_at.120` | 39 / 40 / 40 | 34 / 36 / 36 | 36 / 40 / 40 | 35 / 38 / 38 |
| `herbs_at.180` | 92 / 84 / 82 | 87 / 87 / 84 | 84 / 94 / 94 | 88 / 84 / 84 |
| `herbs_at.240` | 153 / 130 / 131 | 144 / 128 / 128 | 141 / 134 / 136 | 146 / 128 / 130 |
| `copies_by.240`, median | 0 / 0 / 19 | 0 / 0 / 4 | 0 / 0 / 4 | 0 / 0 / 12 |
| `copies_by.240` of 6 or more (runs of 32) | 0 / 11 / 22 | 0 / 8 / 12 | 0 / 5 / 8 | 0 / 7 / 18 |
| `copies_by.240` of 6 or more, `branch_first` | 0 / 10 / 21 | 0 / 5 / 9 | 0 / 0 / 0 | 0 / 5 / 7 |
| Idle share after the tree is done (median) | 0.73 / 0.63 | 0.62 / 0.47 | 0.45 / 0.31 | 0.55 / 0.39 |
| `sign_minute.stone` | 22 / 21 / 21 | 28 / 30 / 30 | 19 / 19 / 19 | 28 / 27 / 27 |
| `sign_minute.ore` | 74 / 73 / 73 | 86 / 83 / 83 | 75 / 75 / 75 | 80 / 78 / 78 |
| `longest_gap` | 36 / 30 / 21 | 41 / 29 / 27 | 25 / 20 / 20 | 37 / 25 / 21 |
| Decisions 130 to 180 | 1 / 3 / 3 | 5 / 5 / 5 | 5 / 4 / 4 | 5 / 6 / 6 |
| Decisions 150 to 240 | 0 / 1 / 1 | 3 / 4.5 / 5 | 4 / 6 / 6 | 3 / 5 / 5 |
| Decisions 150 to 240, `branch_first` | 0 / 1 / 1 | 6 / 7 / 8 | 12 / 13 / 13 | 9 / 11 / 11 |
| Ready reached (runs of 32) | 16 / 16 / 17 | 15 / 15 / 16 | 15 / 23 / 23 | 15 / 16 / 17 |
| Ready reached, `branch_first` | 15 / 11 / 17 | 15 / 10 / 15 | 1 / 6 / 6 | 6 / 6 / 10 |
| Deaths in 240 minutes (64 runs) | 0 / 0 / 1 | 0 / 0 / 0 | 0 / 0 / 0 | 0 / 0 / 1 |

Idle share is the share of settlers idle in the 15-minute windows from the minute the last tech
lands, in the runs that finish the tree.

How the deals went (completed deals and requests that paid in the good, `steady`, runs of 32 with
at least one): wares 13, 15, 19 and 12 (Air, Water, Earth, Fire), median 6, 3, 2 and 4 a run;
scroll copies 21, 11, 11 and 14, median 7, 2, 1 and 5.5. `main` pays in neither, by construction.
In a 360-minute probe of seeds 1 to 8 (`steady`), an Archive stood in 22 of the 24 runs that know
`archives`, made 12 to 39 copies, and the deals took 0 to 20 of them (a run that makes only 12 sits
at the stock limit with nothing taken: the caravans' random draw never showed a copy deal).

Against "Expected" in the table above:

- **Decision timeline: met at the low end.** From 150 on, the shared stretch gains 1 to 2
  decisions against base (Air 0 to 1, Water 3 to 5, Earth 4 to 6, Fire 3 to 5), where 2 or 3 were
  expected, and the same with `branch_first` (0 to 1, 6 to 8, 12 to 13, 9 to 11). The gain is the
  techs, not the opening: it adds no decision against `main`, since the timeline counts techs that
  unlock a building, and `trade_wares` unlocks none. The one timeline reading the opening moves is
  `longest_gap`: Air 30 to 21 and Fire 25 to 21 against `main` (Air `branch_first` 55 to 24),
  from the Tea House's draw and fourth deal, which bring Ready in on more runs (Air 11 to 17 of 32
  with `branch_first`, Fire 6 to 10).
- **Wood: Air falls, Fire has none to lose, Earth doesn't.** `wood_at.180` drops 10 to 1 and 42 to
  34 (Air, Water) against `main` and 1 to 0 (Fire), and `wood_at.240` 53 to 12 on Earth. The wares
  Workshop's draw is small because the reserve rule stops it where there is no wood to spare: the
  wood was spent before the wares tech landed. The bigger fall is #440's (base to `main`: 192 to 10,
  200 to 42, 349 to 106, 82 to 1).
- **Stone and ore: unchanged.** Both signs fall at minute 19 to 86, before any phase-3 tech, so
  `sign_minute.stone` and `.ore` do not move in any nation. The ore deals pay in wares and copies,
  and a bought ore isn't mined (#834), so they could not help the ore sign yet either.
- **Herbs and harm: no regression.** `herbs_at.240` is within 4 of `main` in every nation: the
  house burns 12 an hour from minute 144 (Air) to 194 (Earth) and the gardens fill it back. Herbs
  stand at 30 to 140 at 240 (the low ones are `branch_first` Earth and Fire, as in `main`), so
  tea does not compete with the sick, and `no_deaths_by_180` holds on all 256 runs.
- **Scribing: the 6-copy bar is met wherever an Archive stands and the tree is done.** `copies_by.240`
  is 6 or more in 22, 12, 8 and 18 of 32 `steady` runs (Air, Water, Earth, Fire) against 11, 8, 5 and
  7 on `main`. Of the 27, 21, 28 and 22 runs that finish the tree, 22, 19, 27 and 18 have an
  Archive by 240; the rest finish it too late to build one or still lack the wood. Earth's median is
  4 because its tree is done at minute 219, 21 minutes before the end. The idle share after the tree falls from 0.62 to 0.47 (Water),
  0.45 to 0.31 (Earth) and 0.55 to 0.39 (Fire).
- **No regressions: 0 of 12.** `balance verdict --against` `main` calls no reading moved, and
  `no_deaths_by_180` holds. Against base it calls four moved, all #440's: `wood_at.120` (median
  -172), `pop_at.120`, `longest_gap` and `sign_minute.population`. Two runs in 256 end with a
  starvation death after minute 180 (Air seed 30 `branch_first` at 182 and Fire seed 12 `steady`
  at 207), both with the sim's `stranded` flag: a settler in a pocket with no way to the stockpile.
  Switching off any one of the opening's phase-3 moves removes the death on Fire 12, and switching
  off all of them leaves Air 30's, so it is a latent trap the run's timeline finds, not a phase-3
  building. Filed as #848. `ready_window` and the other pacing targets pass and fail where they did.

```
against      main at 398d6cf: 256 runs paired
         sign_minute.stone            steady/normal        n 128  earlier   2  later   0  same 126  median       0  noise 0.58333
         sign_minute.ore              steady/normal        n 128  earlier   5  later   0  same 123  median       0  noise 0.958332
         wood_at.120                  steady/normal        n 128  earlier   0  later   0  same 128  median       0  noise 2
         pop_at.120                   steady/normal        n 128  earlier   0  later   0  same 128  median       0  noise 0
         food_per_head.winter.2       steady/normal        n 128  earlier   0  later   0  same 128  median       0  noise 0.4375
         deaths_by.180                steady/normal        n 128  earlier   0  later   0  same 128  median       0  noise 0
         longest_gap                  steady/normal        n 128  earlier  23  later  13  same  92  median       0  noise 2
         longest_gap                  branch_first/normal  n 128  earlier  21  later   9  same  98  median       0  noise 2
         sign_minute.population       steady/normal        n 128  earlier   5  later   0  same 123  median       0  noise 0
         ready_minute                 steady/normal        n 128  earlier   5  later   3  same 120  median       0  noise 0
         care.suffering               steady/normal        n 128  earlier  16  later  13  same  99  median       0  noise 0
         deaths                       steady/normal        n 128  earlier   0  later   1  same 127  median       0  noise 0
  0 of 12 readings moved; flags: 4 runs newly flagged, 9 no longer
```

**Levers.** None of the data levers needed touching. The Tea House's `per_minute` (12 herbs an
hour) leaves herbs where `main` had them, the wares deals' counts and `copy_points` move the
stock they were written for (copies traded in 11 to 21 of 32 runs), and `stock_limit` pauses
scribing only once the deals have had their share. What was off was wood and the core's crowding,
and the fixes are in the opening (the reserve, the far-wood rule, the Archive's fallback site),
not in the data. If the Archive and the Tea House should not depend on the forest, their wood
costs (20 and 14) are the lever.

## Risks

- **#437 lands in a shape this doesn't expect.** This design assumes pacing.md's schema: a deal
  list per nation, `deals_per_visit`, `stay_minutes`, and deals drawn per visit. If #437 draws
  its three deals differently, #710 still draws the extra one from its own stream among
  the deals not shown. The rule that a house never changes the first three holds either way.
- **Draw (#435) or the chapter (#439) isn't in yet.** The Tea House's draw term waits on #435,
  and `trade_wares`'s condition waits on #439. Follow-up D could ship the caravan half without
  draw, but then the Tea House does nothing before the chapter. It is simpler to wait.
- **Scribing as a research leak.** A player who forgets to set a target loses nothing any more,
  since that knowledge now becomes copies. That is a gain, not a leak. The only knowledge still
  lost is overflow with no Archive or at the stock limit, which is the #284 rule today.
- **Seat churn.** `sync_seats` closes seats when an Archive comes down. A scholar mid-study steps
  out, which needs-preemption already does every few minutes, so no new state is involved.
- **The Archive's 6-seat halls and power.** A powered hall's seats each run at the boost, so an
  Archive among powered halls is a big research jump late in the tree. The harness's
  `ready_window` and the decision timeline catch it, and `seats` is the lever.

## Open questions for the owner

1. **May the Tea House lift draw past its 140% ceiling?** *Recommended: no. It adds 0.1 under the
   same 1.4 `max`.* The owner set draw at 60% to 140% (#462), and the spec says only that a full
   house "adds to draw". Under the cap, the house helps a player who is missing a food kind or a
   spare bed, and does nothing for one who has them all. Raising `max` to 1.5 while a house is
   open would make it a real growth building for every player, at the cost of amending #462 and
   letting tea outpace the beds and foods that draw was designed to reward. A third option is no
   draw term at all, which would leave the house with nothing to do before the chapter. Section:
   "`tea_ceremony`: the Tea House". (#706)

## Follow-up issues

Each one appends its own tech, building, item and deal blocks from this doc. `TechId` and
`BuildingId` are file positions, and saves remap them by name (save-compat.md), so the order
between follow-ups doesn't matter.

| Draft | Title | Blocked by |
|---|---|---|
| #707 | Sim and data: research conditions (`requires` with `scrolls_read` and `chapter`) | #439 |
| #708 | Sim and data: alternate recipes (`[[building.alt_recipe]]`, the `recipe` upgrade, `SetRecipe`), wares, and `trade_wares` | #674, #673, #707 |
| #709 | Sim and data: the Archive (seats in reach, scribing, scroll copies, `ClearResearch`) and `archives` | #674, #673, #707 |
| #710 | Sim and data: the Tea House (herb bunker, extra deal and stay, draw term) and `tea_ceremony` | #437, #435, #675, #673, #706 |
| #711 | Data: caravans take wares and scroll copies (deal and request rows, the no-knowledge rule) | #437, #708, #709 |
| #712 | TUI: recipe switching, Archive and Tea House readouts, condition lines, clearing the target, the 4th deal | #708, #709, #710, #438 |
| #713 | Balance: the economy opening uses phase 3, `herbs_at.M`, `copies_by.M`, and the phase-3 run | #708, #709, #710, #711, #440, #700 |
| #706 | Decision: may the Tea House lift draw past its 140% ceiling? | none |
