# Midgame pacing: the long wait from minute 60 to the Harmony Gate

Doc status: current

Status: **accepted** (#412). The owner answered on 2026-11-06 and 2026-11-08, recorded under
"Owner decisions". The follow-up work is under "Follow-up issues". The harness numbers below
predate #353, #490 and #513, and need a fresh measure before anyone tunes against them. "After
#440" (below them) is that measure, with draw, caravans and the chapter in.

The owner's playtest (2026-11-06): "the midgame and end game felt like they really lagged." This
is about pacing, not frame rate. The middle of the game has too few new decisions, and the total
length isn't the problem: endgame.md's target of 3.5 to 4 hours to the gate stays. This doc
measures where the decisions stop, then proposes four levers that don't touch research content.
Research content is #413's job (docs/design/late-tree.md once it lands): its empty techs are one of
the gaps measured below, and this doc only points at them.

## Decisions in brief

1. **Measure decisions, not stores.** The economy harness gains a decision timeline: every
   visible unlock, first-met sign and caravan, the longest gap between them, and how much of the
   time settlers sit idle. The pacing target is **no gap longer than 15 minutes between new
   decisions from minute 30 to Ready, on 7 of 8 seeds per nation**.
2. **Growth follows what the player builds.** The fixed arrival clock becomes a **draw** meter.
   It fills faster with spare beds, warm beds and food variety, and a bare settlement draws at 60%
   of #353's pace. A settlement with all three bonuses draws at 140%. #353 keeps its
   `arrival_minutes`, which now means "minutes per newcomer at 100% draw".
3. **Caravans spend the surplus.** Traders from the other three nations camp near the settlement
   once a season and offer fixed barter deals: your food and wood for their stone, ore, tools,
   bricks or a chart to a ruin. Each caravan also brings one **request**, a gift whose reward is
   draw or knowledge. Settlers haul both sides, which puts the idle half of the colony to work.
   It is vision.md's "neighbors to trade with", with no war and no diplomacy screen.
4. **One chapter, and it opens a door.** "Known on the road" (population 10, four kinds of
   building, 60 food) sits at the top of the Endeavor screen. Meeting it brings the first caravan.
   There is no second chapter, because a reward that changes play is worth a milestone and a
   badge isn't.
5. **After the gate, the same loop, wider.** United City caravans come from all four nations, and
   their requests grow. The population milestones carry on. The Spirit Portal stays the sequel,
   with its own design issue when the owner wants it.
6. **Time controls stay as they are** (owner, #461). Content carries the whole pacing load, with
   no fast-forward safety valve.

## What the harness shows

`balance --economy`, seeds 1-8, all four nations, Small, Normal, 240 minutes, on main at
`dee4911`. That's before #353, so a newcomer still arrives every 15 minutes. A throwaway patch also
printed each tech's completion minute and the share of settlers with no task, sampled every
in-game minute (#434 makes it permanent). On seeds 3 and 4 the opening raises no bed for
Fire, Water or Air (#395), and Water never researches on seed 7 (#397), so those runs are left out
of the medians. Earth's are kept. Seeds 2 and 6 never meet the ore sign for any nation (#396).

### When each sign is first met (median minute)

| | Beds | Stone | Food | Ore | Kinds | Branch | Population | Masters | Scrolls |
|---|---|---|---|---|---|---|---|---|---|
| Air | 6 | 18 | 39 | 69 (4/6) | 114 | 130 | **150** | 121 | never |
| Water | 6 | 19 | 74 | 76 (3/5) | 107 | 148 | **150** | 236 (4/5) | never |
| Earth | 5 | 11 | 86 | 77 (6/8) | 113 | 147 | **150** | never | never |
| Fire | 6 | 20 | 84 | 75 (4/6) | 124 | 143 | **150** | 203 | never |

Leaving masters (#344) and scrolls (#380, #402) aside, the gate would be Ready at **minute 150 to 165 on
every seed where ore is met**. Population is the long pole on all but one of them: 6 to 16 is ten
arrivals times 15 minutes, whatever the player does.

### The decision timeline

A decision is anything that gives the player something new to do: a tech that unlocks a building,
ability or job, or a sign met for the first time. A typical run (Fire, seed 7):

| Minutes | New decisions |
|---|---|
| 0-15 | Beds met (5), Fishing (11), Stoneworking (13) |
| 15-30 | Stone met (21), Farming (26) |
| 30-45 | Carpentry (42) |
| **45-60** | **none**: Pottery, Masonry, Herbalism and Writing finish, and none unlocks anything |
| 60-75 | Food met (66), Prospecting (73) |
| 75-90 | Ore met (76), Forging (88) |
| 90-120 | Brickmaking (95), Smelting (100), Dancing Dragon (110) |
| 120-150 | Steam Engines (121), kinds met (123), Lightning Generation and branch met (142), population met (150): Ready but for masters and scrolls |
| **150-200** | **none** |
| 200+ | Masters met (204). Scrolls never |

| | Longest gap, minute 30 to Ready | Where | What fills it today |
|---|---|---|---|
| Air | 25-31 min | 35-67 | Shared techs that unlock nothing (#413) |
| Water | 29-38 min | 36-74 | The same |
| Earth | 18-35 min | 36-77 | The same |
| Fire | 25-35 min | 36-74 | The same |
| Every nation, seeds 2 and 6 | 75-90 min | 150-240 | Nothing: ore is never met, so the gate is never Ready |

Two notes on reading this:

- **The harness researches the shared tier first**, then the branch. A player who goes branch
  first moves the branch unlocks to minutes 30-90 and the dead block to 90-150. The block is the
  same size either way, about 700 points of techs with no visible unlock. That block is #413's to
  fill.
- **Wherever the dead block sits, the end is a wait for the clock.** The branch is done by
  minute 130-150, and the last ten minutes or more wait for the fifteenth newcomer.

### Stores and idle hands (medians)

| Minute | 30 | 60 | 90 | 120 | 150 | 180 |
|---|---|---|---|---|---|---|
| Settlers with no task, Air | 6% | 50% | 60% | 58% | 72% | 77% |
| Water | 0% | 44% | 52% | 56% | 60% | 76% |
| Earth | 1% | 45% | 46% | 44% | 54% | 66% |
| Fire | 1% | 43% | 50% | 38% | 50% | 61% |
| Stone stored (all nations) | 400-540 | 580-630 | flat | flat | flat | flat |
| Wood stored (all nations) | 430-490 | 390-450 | slowly down | | 290-410 | |
| Food per head, Air / others | 14 / 5-7 | 20 / 6-10 | 32 / 13-15 | 33 / 13-15 | 41 / 18-20 | 42 / 17-22 |

**From minute 60, about half the settlers have nothing to do**, and stone and wood stop changing.
The harness's opening gives fewer orders than a player would, so the real idle share is lower.
But the shape is the owner's: full stores, spare hands and nothing to spend them on.
#353 makes food and wood leaner, but it doesn't give either a use.

### After #440 (2026-11-11)

Draw (#435), caravans (#437), the chapter (#439) and the Traders screen (#438) have landed, and
the economy opening uses them (`crates/sim/src/autoplay/pacing.rs`). Seeds 1 to 32, all four
nations, Small, Normal, 180 minutes, on main at `c2e3cdf`, through `balance verdict` with the
`steady` and `branch_first` styles (the 32-seed panel; the first tables below are its seeds 1 to
8, to compare with the "before" tables above). "Before" is the same main without #440's opening.

**What the opening does now.**

- **Warm beds.** Beds go on a free tile in reach of the campfire with the fewest beds. A bed that
  ends up outside every campfire's reach (the first bed comes before fire-making for most
  nations) gets another campfire on a tile within reach of it, one at a time, up to 8. A finished
  bed with no such tile is taken down. The passive variant lights its one campfire 8 tiles east
  of the anchor instead.
- **Food variety.** While fewer than two food kinds have 10 in stores, the standing harvest
  orders reach past 15 tiles, two far sources of the shortest kind at a time. (Draw counts a
  third kind too, for 120%; the opening stops at two, which with warm beds is the 100% the
  targets assume.)
- **Caravans.** With a caravan camped, it takes first the deals that feed a sign not met yet
  (stone and bricks for the stone sign, ore for the ore sign), then the request, then the deals
  that spend whichever of food or wood is the more over its threshold, as long as the goods can be
  spared. The thresholds are constants in code, not data: 8 food a settler (at least a newcomer's
  30, and the food sign's 150 once the population sign is met) and 150 wood. A request or sign deal
  may dip to half of that while the settlement is still growing. Stone and everything else is never
  paid, except the phase-3 trade goods (wares and scroll copies, #713): those have no threshold,
  so any deal that takes them goes ahead of the rest whenever stores hold them.
- **Data nudge.** `arrival_minutes` goes from 10 to 12. An opening that keeps all three bonuses
  and takes every request draws at 120% to 140%, and with 10 it reached 16 people at minute 85 to
  100, before the window. At 12 the economy opening is near the window's middle (median 108 for
  the shared-first order on seeds 1 to 8), and a bare settlement (60% to 100%, below) is 25 minutes
  behind it.

**Signs first met** (`steady`, seeds 1 to 8, median minute, runs that met it of 8):

| | Beds | Stone | Food | Ore | Kinds | Branch | Population | Masters | Scrolls |
|---|---|---|---|---|---|---|---|---|---|
| Air | 6 (8) | 18 (6) | 80 (6) | 66 (6) | 96 (7) | 106 (8) | 100 (5) | 108 (6) | 78 (6) |
| Water | 6 (8) | 19 (6) | 140 (5) | 73 (6) | 106 (5) | 124 (7) | 109 (5) | 104 (5) | 84 (6) |
| Earth | 5 (8) | 11 (8) | 142 (4) | 71 (7) | 108 (7) | 122 (8) | 114 (7) | 105 (7) | 83 (7) |
| Fire | 6 (8) | 17 (5) | 142 (4) | 69 (6) | 110 (5) | 126 (8) | 105 (5) | 146 (3) | 82 (6) |

Before, on the same seeds: Population 111, 117, 129 and 108 (Air, Water, Earth, Fire) and Food met
on 5, 4, 3 and 2 runs of 8. The runs that miss a sign are mostly the ones the harness already
knows: seeds 2 and 6 have too little stone and ore in reach, and seeds 3, 4 and 6 stall in the
opening (#395, #397).

**Gaps, tail and Ready** (`steady`, then `branch_first`, seeds 1 to 8; "tail" is the median
minutes from the last decision before Ready to Ready, the new `tail:` line):

| | Gaps of 15 or less | Longest gap, min-median-max | Tail | Ready, median (runs that reach it) | Ready in 130-170 | Population 16 in 100-130 | Ore on seeds 2, 6 |
|---|---|---|---|---|---|---|---|
| Air | 1 of 8 | 14-21-59 | 0 | 108 (4) | 0 | 3 | none, none |
| Water | 0 of 8 | 16-17-80 | 9 | 138 (4) | 3 | 5 | none, none |
| Earth | 0 of 8 | 16-19-22 | 15 | 142 (4) | 3 | 7 | none, minute 180 |
| Fire | 0 of 8 | 16-19-26 | 6 | 142 (4) | 4 | 5 | none, none |
| Air, branch first | 0 of 8 | 17-24-40 | 7 | 140 (4) | 3 | 3 | none, none |
| Water, branch first | 0 of 8 | 21-27-43 | 3 | 157 (2) | 2 | 3 | none, none |
| Earth, branch first | 0 of 8 | 28-29-30 | | never | 0 | 0 | none, minute 174 |
| Fire, branch first | 0 of 8 | 26-27-40 | | never | 0 | 0 | none, none |

On the whole panel (`steady`, 32 seeds per nation) Ready is reached on 58 of 128 runs, up from 51,
at a median of minute 134 (minute 128 before), with the middle 80% from 108 to 150 (109 to 163
before). The signs that keep the other 70 from it are ore (16 runs miss only it), food (13 miss only it)
and the stalled openings.

**The targets, `balance verdict`** (32 seeds, `steady` unless named; pass counts of the runs that
count, before then after):

| Target | Air | Water | Earth | Fire |
|---|---|---|---|---|
| `wood_at_120` | 10/24 FAIL, 23/27 pass | 16/30 FAIL, 26/30 pass | 12/32 FAIL, 28/32 pass | 17/30 FAIL, 21/28 FAIL |
| `food_winter2` | 1/24, 8/27 | 16/31, 25/30 | 15/32, 27/32 | 19/31, 28/30 pass |
| `pop_at_120` | 20/24, 23/27 pass | 22/30, 24/30 | 29/32, 30/32 pass | 23/30, 24/28 pass |
| `stone_by_120` | pass, pass | 16/19 FAIL, 16/19 FAIL | pass, pass | 16/20 FAIL, 16/17 pass |
| `ore_by_120`, hard sites at 150 | 6/15, 6/16 FAIL | 6/16, 6/16 FAIL | 8/16, 8/16 pass | 6/14, 6/15 FAIL |
| `pop16_window` | 18/23, 17/27 | 21/30, 19/30 | 20/32, 27/32 | 18/29, 19/28 |
| `ready_window` | 2/21, 1/23 | 6/29, 9/26 | 8/32, 11/32 | 4/27, 9/27 |
| `decision_gap` | 1/21, 2/22 | 1/27, 0/25 | 1/32, 1/32 | 0/27, 0/27 |
| `decision_gap_branch_first` | 1/30, 1/29 | 0/26, 0/24 | 0/31, 0/31 | 0/27, 0/28 |

No death on any of the 256 runs (`no_deaths_by_180` holds).

**What meets the targets and what doesn't.**

- **Met or better:** wood in stores at minute 120 (three nations now pass; Fire is at 21 of 28
  against 85%), food per head into winter 2 (Fire passes; Water and Earth are at 83% and 84%),
  population 14 at minute 120 (Air, Earth and Fire pass), and population 16 for Earth (27 of 32 in
  100 to 130, from 20, one run short of 85%).
- **Population 16 in 100-130** is still below 85% for the other three nations. In every case the
  misses are runs that never reach 16 (seeds 3, 4 and 6; #395, #397), not runs that come early:
  the median is 102, 110 and 109 (Air, Water, Fire).
- **Ready in 130-170** moved from 2-8 of 32 to 1-11 of 32 and stays well below 85%, because most
  runs are held up by a sign that isn't the clock: ore on the hard sites (fixed by #834, next section), the food sign
  (150 in stores against 3 to 8 a head into winter 2), and the stalled openings. Where Ready is
  reached, it is mostly at minute 108 to 150.
- **Longest gap of 15 or less** is met on 0 to 2 seeds of 32 per nation, both orders. The gap is
  research with nothing to unlock, at minute 54 to 80 between Herbalism and Prospecting (Earth,
  122 to 142, between Paving and Omashu Chutes): #835. Caravans don't count in the timeline, so
  they don't hide it.
- **Ore on seeds 2 and 6 by minute 150** was not met for any nation here: a caravan's gold ore
  was hauled home but didn't count toward the ore sign, and the lots were small. #834 fixed both
  (next section).
- **Population 16 for the passive player** (`balance economy --beds-only`: beds, one campfire
  lit apart from them, no far foraging, no deals): minute 112 to 162, median 134, 150 or later on 4
  of 22 runs. The economy opening reaches it at 94 to 127 (median 109). The passive player isn't
  passive enough because wild foraging fills two food kinds for free: #836.

**Tail.** The `tail:` line shows the wait from the last decision before Ready to Ready. On the
whole panel its median is 0 (Air), 5 (Water), 11 (Earth) and 5 (Fire) minutes. It is 0 for Air
because the last sign is the branch, met a tick after its last tech. For the others the last sign is
usually the food sign, filling toward 150 after the last tech (Earth: 8 of 13 runs, after Paving),
so the wait is a few minutes of stores filling, not of the clock.

### After #834 (2026-11-11)

A caravan's ore is now a second route to the ore sign, as this doc claimed. Two changes:

- **Bought ore counts.** `settle_offer` adds a deal's `get` to `Tallies::harvested` when the deal
  completes (the sign's "Ore mined" tally keys on the item, so only gold ore moves it; no other
  sign tallies a harvested item). The owner's call (#834, option a): ore bought at a camp is ore
  for the sign. A test (`a_taken_ore_deal_moves_the_ore_sign`) takes the Fire caravan's ore deal
  and reads the tally.
- **Bigger lots.** Earth's and Fire's gold ore deals sell 8 ore a take (from 3) at 50 wood (from
  25) or 12 wares (from 6), twice a visit. Per ore that is a little cheaper in wood (6.25 against
  8.3) and the wares stay about twice as good as raw wood (12 wares cost about 24 wood). At 3 an
  ore, 40 ore took ten visits; at 8 it is five takes, which two visits that offer both ore deals
  cover. Lots of 6 were tried first and left Fire at 7 of 15 hard runs (8 are needed).

Measured as in the section above (seeds 1 to 32, Small, Normal, 180 minutes, `steady`, through
`balance verdict`, on `main` at `47db3bb` (with #713's opening); "before" is that main, "counted" is
only the tally change, "lots" is both):

| `ore_by_120`, hard sites, by minute 150 | Air | Water | Earth | Fire |
|---|---|---|---|---|
| Before | 6/13 FAIL | 6/16 FAIL | 8/16 pass | 6/15 FAIL |
| Bought ore counted, lots of 3 | 7/13 pass | 7/16 FAIL | 11/16 pass | 7/15 FAIL |
| Lots of 6 (Earth, Fire only run) | | | 14/16 pass | 7/15 FAIL |
| Lots of 8 (this change) | 10/14 pass | 10/16 pass | 14/16 pass | 10/15 pass |

The hard bar is 50% of the hard runs. Earth and Fire now meet it, and Air and Water do too, since
all four nations buy from an Earth or Fire caravan. The standard bar (85% by minute 120) was met
by Earth and Fire before and still is (16/16 and 13/13), and Air and Water still pass it
(10/11 and 12/14). On seed 2 the sign is met at minute 124 (Earth), 125 (Fire), 80 (Air) and 95
(Water), from never. Seed 6 is better but not met for three nations: Earth buys its way to 35 ore
by minute 150 and meets the sign at 165, and Air, Water and Fire stall in the opening (#395,
#397: no wood to pay with), so no lot size can help them there.

Side effects on the other targets are small (pass counts before then after): `wood_at_120` Air
21/27 to 23/27 (now passes), Water 21/30 to 25/30, Fire 19/28 to 21/28, Earth 25/32 unchanged;
`ready_window` Air 1/21 to 2/21, Water 11/27 to 12/27, Earth 14/32 to 13/32, Fire 9/26 to 11/26;
`pop_at_120` and `no_deaths_by_180` unchanged.

## 1. Growth the player can drive: draw

**Today:** every `arrival_minutes`, on the absolute clock, a newcomer comes if a bed is free and
30 food is stored. Once the first spare bed stands, nothing else the player builds matters.

**Proposed:** a `draw` meter (world state, `serde(default)`) fills every tick at

```
rate = (1 / arrival_minutes) x multiplier, per minute
multiplier = base + food variety + warm beds + spare beds, capped at max
```

When draw reaches 1.0 and growth's existing conditions hold (bed capacity above the living settlers, #552, and `food_per_newcomer`
in stores), one newcomer arrives through the same spawn, and 1.0 is spent. While the conditions
fail, accrual stops at 1.0, so a player who builds a bed late gets one newcomer, not a burst.

| Term | Rule | Value | Why it's a decision |
|---|---|---|---|
| base | Bed capacity above the headcount and the food floor | **0.6** | The passive pace: 16.7 min per newcomer with #353's 10, close to today's 15 |
| food variety | Each food kind beyond the first with at least 10 in stores (fish, fruit, cabbage today) | **+0.2** each, up to 2 | Fish, forage and farm, not one big pile. Air's orchard alone doesn't max it |
| warm beds | Every bed's tile is heated (`World::is_heated`: a campfire's radius, or Hearthflame) | **+0.2** | Where to put beds, and campfires to keep. Fire's Hearthflame counts, which suits the nation |
| spare beds | Each free bed beyond the first | **+0.1** each, up to 2 | Building ahead of arrivals |
| max | | **1.4** | 7.1 min per newcomer |

**Numbers.** 6 to 16 takes about 167 minutes for a passive player (today's pace) and about 71 for
one who keeps all three bonuses. The harness's economy opening should land near 100% (#353's own
target, population 14 or more at minute 120) once it keeps a campfire by its beds and two foods in
store. #440 teaches it that and checks the result.

**Lumps on top.** A festival (#365) still brings a newcomer at once and doesn't touch draw. A
caravan request (section 2) can grant draw directly, and only these grants can lift draw past
1.0, up to 2.0. Arrivals are at least 2 minutes apart, so lumps can't produce a crowd at the edge
of the map.

**Readable.** The Endeavor screen's population hint and the side panel say when the next newcomer
is due and what is raising or missing from draw:

```
| PEOPLE                                                           |
|   Population                      #######...     11/16           |
|   Next newcomer in about 6 min                                   |
|   Draw 120%: 2 foods, warm beds, 1 spare bed                     |
|   Not yet: a third food                                          |
```

*(Edited from `fn_tui 7 earth --frame --ascii --size 100x34 --ticks 3000 --keys G`. The strings
follow text-voice.md: one idea per line, with no key glued on.)*

**Alternatives.** Comfort scores per bed (housing quality) would be richer, but there's only one
bed kind besides the igloo, so it would score nothing yet. Births are a family system, out of
scope. A "draw from renown" (sum of everything built) would reward what the building-kinds sign
already rewards.

```toml
# data/needs.toml
[growth]
arrival_minutes = 10.0     # #353: minutes per newcomer at 100% draw
food_per_newcomer = 30

[growth.draw]
base = 0.6
food_kind = 0.2
food_kind_min = 10         # stored items of one food for it to count
food_kinds_max = 2         # kinds beyond the first that count
warm_beds = 0.2
spare_bed = 0.1
spare_beds_max = 2
max = 1.4
bank = 2.0                 # the most draw can hold, lumps included
min_gap_minutes = 2.0
```

## 2. Something to spend surpluses on: caravans

**Recommended: caravans with deals and one request each.** They turn food and wood, the two
surpluses, into what's scarce (stone, ore, tools, metal, bricks, a chart to a ruin) and into
draw. Hauling to and from the camp uses the idle half of the colony. It also introduces the
other nations as friendly neighbors before the United City makes them settlers.

**How it works.**

- **Arrival.** Once the chapter is met (section 3), a caravan comes every `interval_minutes`
  (15, one a season), `offset_minutes` (4) into the season so it doesn't land on the season's own
  alert. It stays `stay_minutes` (9). It walks in from the nearest map edge like a newcomer and
  makes camp on the first free 2x2 spot 8 to 14 path tiles from the settlement anchor, searched in
  row-major order so it's deterministic.
- **Who.** The three other nations take turns in a fixed order, so there's no RNG. Each nation's
  caravan has its own deal list in data, which keeps nations distinct: Earth sells stone, ore and
  tools, Fire sells metal and bricks, Water sells fish and healing herbs once #390 lands, and Air
  sells knowledge and charts. Each visit shows `deals_per_visit` (3) of the list, drawn from
  `rng::stream(seed, "trade.deals")`.
- **Deals are fixed barters** ("30 food for 2 metal", up to 2 times). There are no prices, no
  currency and no haggling screen. "Food" means any food items, as the food sign counts them.
- **Indirect control.** Taking a deal queues haul jobs from your stockpiles to the camp, which
  acts as a stockpile that accepts only what deals still need. When one side is complete, their
  side appears as a stack at the camp, and ordinary hauling brings it home. When the caravan
  leaves, anything delivered to an unfinished deal is left as a stack on the camp tile, so nothing
  is lost.
- **The request** is a one-off gift with no goods back: "Food for the families at Fire Fountain
  City: 40 food." Rewards are `draw` (0.5 of a newcomer), `knowledge` (research points, mostly
  Air's monks) or `chart`, which reuses #402's Map Room effect and uncovers the nearest lost ruin.

**What it fixes, in harness terms:**

- On seeds 2 and 6, ore is out of reach on the map for every nation (#396). An Earth or Fire
  caravan offering ore is a second route to the sign (#834: a deal's `get` counts toward the
  harvested tally, and the lots are sized for it). It doesn't replace #396's data fix.
- Fire's bricks and metal (#356) and Earth's tools get a market without adding a new building.
- Food and wood have a use for the rest of the game, including after the gate.

**Screen.** `M` opens the camp's deals, as a centred screen like Endeavor. Mockup, edited from the
Endeavor frame above:

```
+--------------------------- * TRADERS * --------------------------++----- * EARTH KINGDOM * ------+
| Fire Nation traders                              Leave in 7 min  || > Running x1           52:40 |
| Camp at (88, 60)                                                 |+- PEOPLE ---------------- 11 -+
|                                                                  || Haul 4  Quarry 2  Farm 2 [N] |
| DEALS                                       Taken     In stores  |+- RESEARCH -------------------+
| > 30 food for 2 metal                        0/2       212 food  || Tunneling          62/120    |
|   40 wood for 8 bricks                       1/3       847 wood  || 4.0/min                      |
|   50 wood for 8 gold ore                     0/2       847 wood  |+- ENDEAVOR ------------- 4/9 -+
|                                                                  || Next: 4 more people      [G] |
| REQUEST                                                          |+- ALERTS ---------------------+
|   Food for the families at Fire Fountain City                    || Fire Nation traders made     |
|   40 food                                    Draws a newcomer    || camp                         |
|                                                                  ||                              |
| UNDER WAY                                                        ||                              |
|   40 wood for 8 bricks                      24/40 delivered      ||                              |
|                                                                  ||                              |
+- Up Down choose  Enter take  X cancel  Esc/M close --------------++- seed 7 ---------------------+
```

The camp draws as a 2x2 glyph block, like any building (`data/tui.toml`), and the trader as a
unit glyph in their nation's colour.

**Considered, and why not now:**

| Option | Verdict |
|---|---|
| **Upkeep: winter fuel for warmth** | No. A recurring cost gives the player a chore, not a choice. #353 left fuel out of scope, and the kiln and smelter already burn wood (#350). Revisit if Normal still feels safe after #353 and #394 |
| **Upkeep: tools wear out** | Not now. It's a good sink for Fire's metal, but it's invisible until a job slows. Keep it as a candidate for #356's follow-ups |
| **Standalone requests** (a quest log between start and gate) | Folded into caravans. A request needs someone to ask it, and a quest log is the checklist question 4 of #412 warns about |
| **A market with prices** | No. A price UI and a currency are a lot of screen for a single-player colony sim, and fixed barters say the same thing in data |

```toml
# data/trade.toml
[caravans]
opens = "known_on_the_road"   # a chapter id; omit to start at minute 0
interval_minutes = 15.0
offset_minutes = 4.0
stay_minutes = 9.0
camp_distance = [8, 14]       # path tiles from the settlement anchor
deals_per_visit = 3
order = ["earth", "fire", "water", "air"]   # the settlement's own nation is skipped

[[caravan]]
nation = "fire"
name = "Fire Nation traders"

[[caravan.deal]]
give = [{ item = "food", count = 30 }]    # "food" = any food items
get = [{ item = "metal", count = 2 }]
times = 2

[[caravan.deal]]
give = [{ item = "wood", count = 50 }]
get = [{ item = "gold_ore", count = 8 }]
times = 2

[[caravan.request]]
name = "Food for the families at Fire Fountain City"
give = [{ item = "food", count = 40 }]
reward = { draw = 0.5 }                   # or { knowledge = 60 }, or { chart = 1 }
```

Loaded strictly like `endeavor.rs`: unknown keys are rejected, items resolve at load, and every
count has a range check. A deal whose items the visited nation can't use is still allowed,
because selling Fire metal to Water is the point.

## 3. Intermediate goals: one chapter

**Decision: one chapter, "Known on the road", whose reward is the caravans.** It's not a second
list to tick off. It's the moment the outside world notices the settlement, and it changes what
the player can do. A second chapter between it and the gate would be a checklist: the gate's
signs already show progress from minute one.

- **Signs:** population 10, four kinds of building, 60 food in stores. These are ordinary sign
  kinds (`SignKind`) with their own ids and smaller targets, so there's no new counting code.
- **When:** about minute 40 to 60 with #353 and draw. That's the start of the measured dead block.
- **Shown:** a short section at the top of the Endeavor screen until it's met, then one line
  ("Known on the road"). It gets a toast when met, and the side panel's "Next:" names its missing
  sign first.

```
| CHAPTER                                                          |
|   Known on the road                                  2 of 3      |
|   Population                      #########.     9/10            |
|   Kinds of building               ##########      4/4            |
|   Food in stores                  ##########    60/60            |
|   Traders come once the road knows you                           |
```

```toml
# data/endeavor.toml
[[chapter]]
id = "known_on_the_road"
name = "Known on the road"
hint = "Traders come once the road knows you"
signs = ["chapter_population", "chapter_kinds", "chapter_food"]   # [[sign]] ids with chapter = true
```

Chapter signs carry `chapter = true` and are left out of the gate's Ready check and its "N of 9"
count.

## 4. Post-gate: a lighter loop

**Recommended:** the United City keeps the same loop, wider. No new system is needed:

- Caravans come from all four nations, including your own (kin visiting). Deals scale by
  `united_scale` (1.5) in data.
- Requests get bigger and their rewards change: "Stone for Omashu's walls: 150 stone" rewards
  `draw`, which now brings newcomers of any nation (#240's weights).
- The population milestones already planned by endgame.md (the next milestone shown on the
  Endeavor screen) carry on, at 24 and 32 by default, each with a toast.

The **Spirit Portal** (endgame.md, concept B) is the real sequel. It needs spirit AI and a new
resource, so it's a separate design issue when the owner wants it (question 4). It isn't filed
here.

## 5. Time controls: unchanged

**Decided (owner, #461): neither.** The ladder stays x1, x2, x3 and x5, and x10 to x50 stay a
host setting (`--fast`, #321). The options weighed were a "skip to news" key (`]`, run fast until
an alert, sign, newcomer, caravan or tech) and x10 on the normal ladder. #441, the follow-up that
would have built skip to news, is closed as not planned.

What this means for the rest of the doc:

- **The gaps have to close by content.** The 15-minute target (see "Measuring it") is the only
  pacing check. Nothing papers over a gap the levers leave.
- **The tail after the last decision matters more.** With no skip, the stretch between the last
  sign and Ready is felt at x5. `balance economy` prints it on a `tail:` line beside the longest
  gap (#440).
- **A gap #440 can't close becomes a content issue**, for #413's follow-ups if it's research, or
  a new caravan or request kind if it's the economy. It doesn't reopen time controls. Revisiting
  that is a new `decision` issue, after a playtest with draw, caravans and the chapter in.

## How this fits work in flight

| Work | Interaction | Tuned where |
|---|---|---|
| #353 food, growth and wood retune | Keeps `arrival_minutes` (10) and `newcomer_food`. Draw multiplies it, so draw lands **after** #353 and re-checks #353's population row | #353 owns the base pace, this doc the multiplier |
| #365 festivals | Unchanged: a festival is a newcomer now, separate from draw | #365 |
| #344, #360 mastery | Untouched. Masters stay a long sign, not a pacing lever here | #344 |
| #380, #402 scrolls | A caravan `chart` reward reuses #402's reveal effect, so Air caravans become a second route to a ruin. The Air chart deal waits on #402 | #402 owns the effect |
| #413 late research tier | Its empty techs are the measured 30-minute dead block. This doc adds no research content, and the timeline harness (A) is how #413 shows its effect | #413 |
| #355 close-out | Runs before this. Its pacing line in endgame.md is re-measured by #440 once draw and caravans land | #355, then #440 |
| #396 ore in reach | Caravans are a second route on seeds 2 and 6 (#834). #396's data fix still stands | #396 |
| #356 Fire bricks and metal | Fire caravans buy and sell them: a use for #356's outputs, not a replacement | #356 |
| #395, #397 opening bugs | Excluded from the medians. The timeline target counts 7 of 8 seeds so one bad seed doesn't block it | those issues |

## Measuring it

#434 adds to `balance --economy`:

- a `techs:` line (each completion's minute) and a `timeline:` line (every decision event: a tech
  that unlocks a building, ability or job, a first-met sign, a chapter, a caravan arrival);
- the longest gap from minute 30 to Ready (or to the end of the run), and the number of gaps
  over 15 minutes;
- the idle share per 15-minute window;
- `--branch-first`, which researches the nation's branch before the shared tier, closer to how a
  person plays;
- (#440) a `tail:` line, the stretch from the last decision before Ready to Ready, and
  `--beds-only`, the passive player (beds, one campfire away from them, no far foraging, no
  caravan deals).

As built, the timeline holds techs that unlock something and first-met signs, not the chapter or
a caravan's arrival: a caravan every 15 minutes would meet the 15-minute target by itself, so the
check is about new content, not about the cadence.

**Targets** (all nations, Small, Normal, economy opening, once draw, caravans and the chapter
have landed): the numbers live in `data/harness.toml` (`[[target]]`, #537) and `balance verdict`
judges them on a 32-seed panel, so this table names the targets instead of repeating them.

| | Target |
|---|---|
| Longest gap between decisions, minute 30 to Ready, on nearly every seed per nation, in both research orders | `decision_gap`, `decision_gap_branch_first` |
| Population 16, economy opening (draw near 100%) | `pop16_window` |
| Population 16, beds-only opening (no far foraging, a campfire away from the beds, no deals) | minute 150 or later (not a harness target yet: no play style does this). Measured at 112 to 162 (#836) |
| Ore sign on the sites where it is hard (seeds 2 and 6), with caravan deals | the hard bar of `ore_by_120` (applied to every nation; Earth and Fire were the ones named) |
| Ready, ignoring masters and scrolls (unchanged length; the middle is fuller, not shorter) | `ready_window` |

## Follow-up issues

Filed without `ready`; triage decides. The `needs-owner` labels on #435, #437, #439 and #442 are
still on, because work-tracking.md leaves removing them to the owner.

| # | Issue | Blocked by | Owner decision |
|---|---|---|---|
| #434 | Balance harness: decision timeline, gaps, idle share, `--branch-first` | none | |
| #435 | Growth: a draw meter that spare beds, warm beds and food variety fill | #353 | #462 |
| #436 | TUI: when the next newcomer is due, and what draw is missing | #435 | |
| #437 | Caravans: `data/trade.toml`, the camp, deals, requests and events | none | #463, #551 |
| #438 | TUI: the Traders screen (`M`), the camp glyph and caravan alerts | #437 | #463 |
| #439 | Endeavor: the "Known on the road" chapter, which opens the caravans | #437 | #463 |
| #440 | Balance: the economy opening takes caravan deals, and the pacing targets are checked (done: "After #440") | #434, #435, #437, #439 | |
| #441 | TUI: skip to news (`]`): closed, not planned | none | #461 |
| #442 | United City: caravans from all four nations, bigger requests, population milestones | #437 | #464 |

## Owner decisions

Each answer is recorded on its `decision` issue.

| # | Question | Recommended | Owner's answer | Effect |
|---|---|---|---|---|
| #461 | Time controls: skip to news, x10 on the ladder, or neither? | Skip to news (`]`) | **Neither** | Section 5. #441 closed as not planned |
| #462 | Should a passive player grow slower than #353's clock? | Yes, draw 60% to 140% | **Yes, 60% to 140%** | Section 1 as written |
| #463 | Caravans before the gate, opened by one chapter? | Yes, "Known on the road" | **Yes, opened by "Known on the road"** | Sections 2 and 3 as written |
| #464 | After the gate: the midgame loop now, the Spirit Portal later? | Yes | **Yes** | Section 4 as written. The Spirit Portal waits for a playtest of the United City loop |
| #551 | May caravans sell food, given Normal's lean-food decision? | Small lots | **Small lots**: at most about 10 food a visit, paid in stone | Adds to section 2, whose schema doesn't show it yet. #437 carries it, and #440 sets the exact size |
