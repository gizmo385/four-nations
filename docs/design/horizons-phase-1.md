# Research horizons, phase 1: fill the midgame

Doc status: current

Status: **proposed** (#665). This is phase 1 of [research-horizons.md](research-horizons.md)
(accepted 2026-11-10). It turns six techs from that spec, plus `writing`'s promised seats, into
buildings, bills, reaches and TOML. The one owner question is under "Open questions for the
owner", and the implementation work is under "Follow-up issues".

## Decisions in brief

1. **Six techs and one fix, no new research mechanic.** `paving` and `calendar` (shared),
   `oil_lamps` (Water), `lemur_keeping` (Air), `hothouses` (Fire) and `omashu_chutes` (Earth).
   `writing` finally adds its +2 study seats.
2. **The gate doesn't grow.** Every tech that ships today gets `core = true`. The branch sign
   counts only core nation techs, and the building-kinds sign counts only buildings that no
   tech or a core tech unlocks. Without this, phase 1 would add 2 techs and 5 buildings to the
   gate's checklist (research-horizons.md, owner decision 4).
3. **Seven small sim changes, each reusable.** A track rule shared by rail and road, an
   `upgrades` unlock kind, a `features` unlock kind, a standing sow order, fuel bunkers (shared
   with the Steam Engine's #433), a fruit-picking roost, and a one-way haul link (the `link`
   field phase 2's paired buildings reuse).
4. **Each building's decision is placement or upkeep.** Stone per road tile, with nothing back
   when one is torn up. Fish burned instead of eaten. Lemurs that eat a quarter of what they
   pick. Wood burned to keep fields growing in winter. A chute fixed between the rock face and a
   lower foot.
5. **The techs are appended with the code that makes them work, not now.** The data files
   reject fields the sim doesn't parse yet (`deny_unknown_fields`). An inert tech would be a lie
   on the Research screen, and the economy opening builds one of every reachable building, so
   inert entries would move every balance baseline twice. Each follow-up appends its own block,
   as written below.

## What ships

| Tech | Nation | Cost | Needs | Unlocks | The decision |
|---|---|---|---|---|---|
| `paving` | shared | 120 | `masonry` | **Paved Road** (1 stone a tile, 1.25x steps) | Which route gets the gate's scarcest material |
| `calendar` | shared | 250 | `writing` | **Almanac** and the **standing sow order** | 250 points to remove a chore and see winter coming, or a branch tech now |
| `oil_lamps` | Water | 120 | `fishing` | **Oil Lamp** (wide heat, burns fish) | Fish to eat or fish to burn |
| `lemur_keeping` | Air | 120 | `orchards` | **Lemur Roost** (picks fruit in reach) | Labour saved for a quarter of the fruit. Groves and stores must share a reach |
| `hothouses` | Fire | 250 | `brickmaking`, `farming` | **Hothouse** (winter growth in reach, burns wood) | Wood for winter food, and the field clusters round it |
| `omashu_chutes` | Earth | 250 | `earth_rail` | **Chute** (one-way drop from the rock to a lower foot) | Head fixed to the rock, foot fixed by the drop: where the mine goes matters twice |
| `writing` (fix) | shared | 120 | as today | **+2 seats** on every study hall | None new. It is the promise from #17, and a pacing lever |

Per nation this adds 370 shared points plus 250 (Earth, Fire) or 120 (Water, Air). File order
puts the new techs after the branch, so the economy opening reaches them from about minute 110,
which is the stretch where today's tree runs dry.

## The rule every phase-1 tech follows: `core`

```toml
[[tech]]
id = "stoneworking"
core = true        # counted by the gate's signs. Default false
```

- `core = true` goes on all 37 shipped techs in the same change. Its default is false, so that
  no later phase can lengthen the gate by forgetting a line.
- `BranchComplete` counts the nation's core techs. `EveryBuilding` counts buildings that no
  tech unlocks, or a core tech unlocks. A unit test pins every nation's two targets at today's
  values with the phase-1 data loaded.
- The Research screen doesn't mark the difference. A non-core tech is optional, and that needs
  no label.

## The techs

Numbers are starting values for the harness (see "Measuring it"). Hover text means the build
modal's effects list and the cursor panel's inspect lines (fn_ui `build.rs`, `inspect.rs`), in
the voice of text-voice.md.

### `writing`: +2 seats, through `upgrades`

```toml
# data/research.toml, writing (edited in place: its id and position don't move)
[tech.unlocks]
modifiers = { study = 1.25 }
[[tech.unlocks.upgrades]]
building = "study_hall"
seats = 2                  # added to the building's own `seats`
```

- **Schema.** An upgrade names a building and exactly one property. Phase 1 supports `seats`
  only, and an unknown key fails to load. Validation: the building exists, it already has the
  property (`seats > 0`, so an upgrade can't turn a house into a study hall), the value is 1 to
  8, a nation tech may only upgrade a building its nation can reach, and each (building,
  property) pair is upgraded by at most one tech. That is the one-tech-per-item rule that
  `buildings`, `jobs` and `abilities` already follow.
- **Sim.** `World::seats_at(pos, defs)` is the def's seats plus every known upgrade. When a tech
  completes, `world/study.rs` opens the missing Study jobs at every finished hall, and a new
  hall opens the full count. The same sync runs after `load`, so a save that already knows
  `writing` gains its seats.
- **Hover.** The Research detail reads `Unlocks: Study x1.25, Academy +2 seats` (the nation's own
  name for the hall). The build modal's existing line reads `Study hall: 4 seats` once it's
  known.
- **Why it matters now.** Study is the lowest job tier (#284), so seats fill only with idle
  hands. From minute 60 about half the settlers are idle (pacing.md), which is exactly when the
  seats arrive. The Map Room's single seat is a different building and doesn't change.

### `paving`: the Paved Road

```toml
# data/research.toml, appended
[[tech]]
id = "paving"
name = "Paving"
description = "Lay dressed stone so people and carts move faster."
cost = 120
prereqs = ["masonry"]
[tech.unlocks]
buildings = ["paved_road"]

# data/buildings.toml, appended
[[building]]
id = "paved_road"
name = "Paved Road"
verb = "Pave"
work = 6.0
passable = true
[building.track]
speed = 1.25
[[building.cost]]
item = "stone"         # brick stands in, as for any stone cost
count = 1
```

- **Track rule.** `[building.rail]` is renamed `[building.track]` (data only, since saves name
  buildings by id), and more than one building may have it. A straight step between two tiles
  of the *same* track building costs `round(10 / speed)`. Any other step, including road onto
  rail and every diagonal, is an ordinary one. That is rail's rule, generalised: `MoveRules`
  holds one step cost per track building, the step grid's `rail: Vec<bool>` becomes a
  `track: Vec<u8>` index, and the A* floor is the cheapest of the ground and every track. Old
  saves keep loading through serde defaults.
- **Why 1.25 and not 1.3.** Steps are integers on a 10-unit grid, so 1.3 can't be represented:
  10 / 1.3 is 7.7, and 8 gives 1.25 while 7 gives 1.43. The data says 1.25 so that it doesn't
  claim a number the sim can't deliver. Rail stays 2.0 and Earth's.
- **Who it speeds.** Every unit, as rail does in the code today (`rules_for` hands every unit
  the cart rules).
- **Footprint and reach.** 1x1, laid in lines with the build tool's rectangle, the same way as
  rail. It has no reach.
- **Hover.** `Road: steps along it 1.25x faster`, from the same line rail prints (`Rail: hauling
  runs 2.0x faster along it`), which becomes `Track: ...` for both.
- **The decision.** One stone a tile, and the stone sign wants 200. Tearing a road up gives
  nothing back, because `[deconstruct] refund = 0.5` rounds 1 stone down to 0. A road also takes
  the tile: no farm plot, building or regrowing tree can go there. Pave the one route that
  carries the most trips, such as stockpile to quarry or beds to the well, and leave the rest.

### `calendar`: the Almanac and the standing sow order

```toml
[[tech]]
id = "calendar"
name = "Calendar"
description = "Count the days to the frost and keep the fields sown."
cost = 250
prereqs = ["writing"]
[tech.unlocks]
features = ["almanac", "standing_sow"]
```

- **A `features` unlock kind.** It is a closed list of names that the sim knows (a `Feature`
  enum). Each feature is unlocked by at most one tech, and an unknown name fails to load.
  Systems ask `World::has_feature(Feature::StandingSow, defs)` and never a tech id (AGENTS.md
  rule 6). Later phases can add names, such as a second recipe per building in phase 3.
- **The Almanac** is a query: `World::almanac(defs) -> Option<Almanac>`, `None` until the
  feature is known. It holds `frost_in` (ticks to the next first frost, `None` with seasons off)
  and `caravan_in` (`None` until caravans, #437, add an arrival clock). The Nation screen shows
  it as an ALMANAC section (mockup below).
- **The standing sow order** is per plot, the way a standing harvest order is per tile.
  `Command::StandingSow { pos, on }` works on any `[building.farm]`: a Farm Plot or an Herb
  Garden. Without the feature it is refused with `needs Calendar`. The plot's `FarmPlot` gains
  `standing: bool` (serde default false). Whenever a standing plot is Empty with no Sow job (it
  was just built, reaped, or stripped by frost), `tick_farms` queues one. `CancelSow` on a
  standing plot also clears the order, so a cancelled sow doesn't come straight back.
- **Hover.** The plot's inspect line `Sow order: standing`. The TUI toggles the order with the
  harvest tool's existing `O` (the standing-order key) on a plot.
- **The decision.** There is no building. The price is 250 points, the same as a branch tech
  at that tier, for information and one chore removed. That is principle 6: the chore has been
  done by hand for an hour, and doing it by hand stays free.

### `oil_lamps`: the Oil Lamp (Water)

```toml
[[tech]]
id = "oil_lamps"
name = "Oil Lamps"
description = "Burn fish oil in stone lamps for heat where no trees grow."
nation = "water"
cost = 120
prereqs = ["fishing"]
[tech.unlocks]
buildings = ["oil_lamp"]

[[building]]
id = "oil_lamp"
name = "Oil Lamp"
verb = "Light"
work = 15.0
passable = true
[building.heat]
radius = 4             # x Water's heating 0.8 = 3: a 7x7 square, against a campfire's 5x5
sleep_bonus = 1.5
seek_tiles = 20
[building.fuel]
item = "fish"
bunker = 6
per_minute = 0.25      # only while someone is in reach
[[building.cost]]
item = "stone"
count = 2
[[building.cost]]
item = "fish"
count = 2
```

- **Fuel bunkers** (new, shared with #433). `[building.fuel]` names an item, a bunker size and
  a burn rate. The building's effect is on only while its bunker holds at least one item. When
  it burns is up to the effect: a heat source burns while any unit stands in its reach, a
  Hothouse while it is warming plots, and the Steam Engine (#433) while it powers a working
  consumer. Burning accrues in integer millis per tick, so it stays deterministic. At or below
  half full, a `Refuel` haul brings the shortfall from stores, the way a build site's bill
  arrives. Fuel in a bunker is out of stores, so it is never eaten. Taking the building down
  returns the bunker in full. A heat source whose bunker is empty is ignored by `is_heated`,
  `is_cooking` and `warm_sleep_route`.
- **Numbers.** The lamp burns 15 fish an hour at most, a sixth of a Fishing Dock's output. It
  warms and cooks a square nearly twice a campfire's, and its bill holds no wood.
- **Hover.** `Keeps a fire: warms 3 tiles, sleeps 1.5x faster nearby` (the existing line), then
  `Burns 1 fish every 4 minutes while in use`. Inspect: `Fuel: 4/6 Fish`, or `Out of fish` as a
  status.
- **The decision.** Fish fed to people or to lamps. Next to the igloos, a lamp burns through the
  night. Next to the dock, it burns through the working day. On a treeless polar coast it is the
  only heat that needs no wood at all. On a wooded site, a campfire is free once built and
  usually wins. That makes the site part of the decision, which research-horizons.md asks of
  this tech. Question 1 below asks whether campfires should stay free.

### `lemur_keeping`: the Lemur Roost (Air)

```toml
[[tech]]
id = "lemur_keeping"
name = "Lemur Keeping"
description = "Winged lemurs pick ripe fruit and carry it home."
nation = "air"
cost = 120
prereqs = ["orchards"]
[tech.unlocks]
buildings = ["lemur_roost"]

[[building]]
id = "lemur_roost"
name = "Lemur Roost"
verb = "Build"
work = 40.0
passable = false
[building.pickers]
resources = ["fruit_tree"]
radius = 5             # Chebyshev from the roost: an 11x11 square. Not modifier-scaled
pick_seconds = 30.0    # at most one tree per 30 s
eat_every = 4          # the lemurs eat every 4th fruit they pick
[[building.cost]]
item = "wood"
count = 6
[[building.cost]]
item = "fruit"
count = 4
```

- **Rule.** Every `pick_seconds`, the roost picks the nearest ripe tree of a listed resource in
  reach, with ties broken by tile index. It skips a tree a settler has already claimed. The pick
  is a normal harvest with no worker: the item's `count` drops, strain is added (so tiring land
  still applies), the harvest tally counts it, and an unclaimed harvest designation on the tree
  is cleared. A counter on the building eats every `eat_every`-th fruit.
- **Where the fruit goes.** The rest goes straight to the nearest stockpile tile in the roost's
  reach that has room for fruit. With none in reach, it drops as a stack beside the roost and
  is hauled like any yield.
- **Numbers.** At most 4 fruit a minute. A full orchard bears about 1.7 a minute, so one roost
  keeps up with two orchards and keeps 3 of every 4 fruit.
- **Hover.** `Lemurs pick ripe fruit within 5 tiles` and `Lemurs eat 1 fruit in 4`. Inspect:
  `Picked: 36  Eaten: 9`.
- **The decision.** Picking by hand yields more and costs hands. Lemurs cost a quarter of the
  fruit and nothing else. Their 11x11 reach rewards planting groves near the stores, against
  the orchard's habit of filling whatever open ground is free.

### `hothouses`: the Hothouse (Fire)

```toml
[[tech]]
id = "hothouses"
name = "Hothouses"
description = "Brick and glass rooms keep crops growing through winter."
nation = "fire"
cost = 250
prereqs = ["brickmaking", "farming"]
[tech.unlocks]
buildings = ["hothouse"]

[[building]]
id = "hothouse"
name = "Hothouse"
verb = "Build"
work = 90.0
passable = false
footprint = [2, 2]
[building.hothouse]
radius = 2             # from the footprint: 32 plot tiles. Not heating-scaled
winter_growth = 0.75   # replaces the climate band's winter growth in reach
[building.fuel]
item = "wood"
bunker = 20
per_minute = 2.0       # only in winter, while a plot in reach is growing or ripe
[[building.cost]]
item = "brick"
count = 10
[[building.cost]]
item = "wood"
count = 6
[[building.cost]]
item = "tool"
count = 1
```

- **Rule.** In winter, a fuelled Hothouse gives every `[building.farm]` plot in reach (Farm
  Plots and Herb Gardens) `winter_growth` in place of its climate band's, unscaled by
  `winter_severity`. The first frost spares those plots' ripe crops. It burns only in winter,
  only while some plot in reach is Growing or Ripe, and does nothing with seasons off.
  `World::crop_growth_rate` takes the best Hothouse in reach for its season factor. Two don't
  stack.
- **Why the reach is fixed.** Fire's `heating` reaches 3.75 with its branch. Scaled like a
  campfire, a Hothouse would cover 8 tiles and every field the settlement has, and the placement
  question would go away.
- **Numbers.** 30 wood a winter. In a temperate band, a plot in reach grows at 0.75 instead of
  0.3: about two harvests in a 15-minute winter instead of at most one, and frost takes
  nothing. Eight plots give about 32 cabbage a winter for 30 wood.
- **Hover.** `Plots within 2 tiles grow through winter`, then `Burns 2 wood a minute in winter`.
  Inspect: `Fuel: 14/20 Wood`, `Idle: not winter`, or `Out of wood`.
- **The decision.** Wood for winter food, sized by the fuel pile. The 2x2 takes ground from the
  field it serves, and the plots must cluster round it. The bunker's wood is hauled by settlers
  who would otherwise be idle in winter. That makes it Fire's answer to frost, with the
  Fishing Dock as Water's and the walled Agrarian Ring as Earth's later.

### `omashu_chutes`: the Chute (Earth)

```toml
[[tech]]
id = "omashu_chutes"
name = "Omashu Chutes"
description = "Earthbenders cut stone slides that carry goods down the mountain."
nation = "earth"
cost = 250
prereqs = ["earth_rail"]
[tech.unlocks]
buildings = ["chute"]

[[building]]
id = "chute"
name = "Chute"
verb = "Cut"
work = 60.0
passable = false
beside = ["mountain", "tunnel"]   # the head touches rock: the Mine Shaft's own rule
[building.chute]
reach = 16             # the foot is within 16 tiles, Chebyshev
drop = 8               # and at least 8 elevation below the head
[[building.cost]]
item = "stone"
count = 12
[[building.cost]]
item = "wood"
count = 4
[[building.cost]]
item = "tool"
count = 1
```

- **The link.** A building record gains `link: Option<Pos>`, set by `Command::SetLink { pos, to
  }`. The build tool asks for the foot right after the head is placed, and the link can be moved
  at any time. A chute's foot must be walkable ground with no impassable building on it, within
  `reach` of the head, and at least `drop` lower on the map's elevation (`Tile::elevation`, 0 to
  255). That is the downhill rule the spec's principles name. With no valid foot the chute does
  nothing, and its status reads `No foot`. Phase 2's Wind Gates and Balloon Docks reuse `link`
  to point at their partners.
- **The haul rule.** A unit picking up a stack bound for stores compares two walks: straight to
  the nearest stockpile, or to the chute's head plus the foot's own walk to stores. The foot's
  walk is cached per chute until the map changes. It uses the chute when that is cheaper. At the
  head the stack drops in and lands at the foot on the same tick, spilling to a free tile the way
  a harvest yield does. There it is an ordinary Haul job. It is one-way: build sites, refuels
  and meals never use a chute.
- **Hover.** `Drops goods up to 16 tiles downhill` and `Must touch rock` (the existing
  refusal). Inspect: `Foot: (31, 18), 9 tiles, drop 24` and `Sent: 140 Stone`.
- **The decision.** The head is fixed to the rock and the foot is fixed by the slope. A Mine
  Shaft's stone lands at the shaft, so a shaft, a chute head beside it and a foot at the
  stockpile's edge make mining nearly walk-free. Where the mine goes now matters twice. A
  settlement whose stores sit uphill of the rock gets nothing from a chute.

## Mockups

These are hand-edited from `fn_tui --scenario fire-year-two --frame --ascii --keys 'N'` and the
`earth-year-two` map. The new lines are marked `<`.

The Nation screen, with the Almanac above the stores:

```text
| STORES                                                           |
| Wood 2120  Stone 311  Fish 36  Fruit 130  Cabbage 21             |
|                                                                  |
| ALMANAC                                                          | <
| First frost in 9m 40s                                            | <
| Next caravan in 4m 10s                                           | < once #437 lands
|                                                                  |
| TRAITS                                                           |
| Heating x2.5  Water work x0.7  Power generation x1.3             |
```

The Research detail for the two shared techs:

```text
| Writing                                       Available  120 pts |
| Set knowledge down in scrolls so it outlives the teller.         |
| Unlocks: Study x1.25, Academy +2 seats                           | <
| ---------------------------------------------------------------- |
| Calendar                                         Locked  250 pts |
| Count the days to the frost and keep the fields sown.            |
| Unlocks: Almanac, Standing sow order                             | <
```

A chute on the Earth map, selected. `V` is the head against the rock, `o` the foot by the
stockpile, and `:` the link line drawn only while the head is selected:

```text
,,,,,,,,,,:::^^^^^^,,,,,,,Y,
,,,,,,,,,,,,/,,^$^^^,,,,,,,,
,,,,,,,,,,,/*,,^^^^V,,,,,,,,      HERE
,,,,,,,,,,,,,c,,^^^:,,,,,,,,      Chute
,,,,,,,,,,,,,,,,,^^$:,,,,,,,      Foot: (31, 18), 9 tiles, drop 24
,,,,,,,,,,,,,,,,,,^^,:,,,,,.      Sent: 140 Stone
,,,,,,,,,,,,,,,,,^^^,,:,,,..
,,,,,,,,,,,,,,B%^^^,,,,:,..-
,,,,,,,,,,,,,/@*^^,,,,,,o---
,,,,,,,,,,,,**,,^,,,,,,,PPPP
```

A Hothouse under the cursor in winter:

```text
| HERE ------------ (62, 40) -|
| Hothouse                    |
| Warming 6 plots             |
| Fuel: 14/20 Wood            |
```

## Measuring it

Phase 1 ends with a harness run (research-horizons.md, "Notes for the phase designs"). The
economy opening must use the new content, or the run measures nothing (follow-up L):

- **Seats** need no opening change.
- **Calendar.** Once it is known, set the standing order on every plot and stop issuing `Sow`.
- **Paving.** Once, pave the path from the stockpile to the deposit the opening quarries most,
  up to 30 tiles.
- **Oil Lamp.** One beside the beds. **Lemur Roost.** One placed to reach both the orchard and
  the stockpile where a tile allows, else beside the orchard. **Hothouse.** One beside the farm
  plots, with new plots placed in its reach. **Chute.** For Earth with a Mine Shaft, a head on
  the shaft's rock face and the foot at the stockpile edge, if the drop allows.

What the run reports, against a baseline from `main` before the first phase-1 merge, on the
panel's 32 seeds and both research orders:

| Measure | Metric | Expected |
|---|---|---|
| Decision timeline | `longest_gap` (targets `decision_gap`, `decision_gap_branch_first`), and the `timeline:` line | The 110-to-150 stretch gains 3 or 4 decisions per nation. Gaps at or under 15 on the target share |
| Food in stores at 120 and 180 | new `food_at.M`, read like `wood_at.M` | Fire and Air rise a little (Hothouse, lemurs). Water falls a little (lamps). No nation drops under the `food_winter2` band |
| Wood in stores at 120 and 180 | `wood_at.120` (existing target), `wood_at.180` | Fire falls by the Hothouse's 30 a winter. `wood_at_120` still under 300 where it passes today |
| No regressions | every existing target, through `balance verdict --against` | `stone_by_120` is the one to watch, since the road spends up to 30 stone. `ready_window` may move earlier from `writing`'s seats |

The balance follow-up records the measured numbers in a "Measured" section here, as power.md
did. If `ready_window` moves earlier than 130 because of the seats, the first lever is `+1` seat
rather than `+2`. That is a data change in this doc's `writing` block.

## Risks

- **Lemurs and tiring land.** On Hard, a tree repicked as soon as it ripens dies (strain 3.0).
  The orchard's `auto_pick` has the same exposure today. The harness's Hard run should count
  fruit-tree deaths with and without a roost. If they rise, the roost skips a tree whose strain
  is above a data threshold.
- **The chute haul rule's cost.** One cached flood per chute. Earth builds two or three, so
  this is small. The cache is keyed on the map revision the step grids already use.
- **#433 lands first.** If the Steam Engine's fuel ships before follow-up E, E becomes
  "generalise #433's bunker into `[building.fuel]`" instead of a new system. The schema above is
  the target either way.

## Open questions for the owner

1. **Should campfires stay fuel-free?** *Recommended: yes, for phase 1.* The spec pitches the
   Oil Lamp as heat that burns fish "not wood", but no heat source burns wood today. With free
   campfires, the lamp's case is reach and a wood-free bill, so it matters most on treeless
   coasts. The alternative is that campfires burn wood through the same bunker, which makes the
   lamp a true fuel swap and gives the wood glut a sink. It would also change every nation's
   winter and every baseline, and that is a pacing change rather than a phase-1 tech. Filed as
   (decision 1).

## Follow-up issues

Each one appends its own tech and building blocks from this doc, in the order the blocks appear
here, whichever lands first. `TechId` and `BuildingId` are file positions, and saves remap them
by name (save-compat.md), so the order between follow-ups doesn't matter.

| | Issue | Blocked by |
|---|---|---|
| A | Sim and data: `core` techs; the gate's signs count only core techs and their buildings (follow-up A) | none |
| B | Sim and data: the `upgrades` unlock kind; `writing` adds 2 study seats (follow-up B) | none |
| C | Sim and data: track buildings; the Paved Road and `paving` (follow-up C) | A |
| D | Sim and data: the `features` unlock kind, the Almanac query and the standing sow order; `calendar` (follow-up D) | A |
| E | Sim: fuel bunkers (`[building.fuel]`, the Refuel haul) (follow-up E) | none |
| F | Sim and data: the Oil Lamp and `oil_lamps` (follow-up F) | A, E, decision 1 |
| G | Sim and data: the Lemur Roost and `lemur_keeping` (follow-up G) | A |
| H | Sim and data: the Hothouse and `hothouses` (follow-up H) | A, E |
| I | Sim and data: the Chute, a one-way haul link, and `omashu_chutes` (follow-up I) | A |
| J | TUI: the Almanac, the sow order toggle, and fuel, roost and Hothouse readouts (follow-up J) | D, E, G, H |
| K | TUI: placing and moving a chute's foot, and its link line (follow-up K) | I |
| L | Balance: the economy opening uses phase 1, `food_at.M`, and the phase-1 run (follow-up L) | B, C, D, F, G, H, I |
