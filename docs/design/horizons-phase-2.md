# Research horizons, phase 2: routes and outposts

Doc status: current

Status: **proposed** (#666). This is phase 2 of [research-horizons.md](research-horizons.md)
(accepted 2026-11-10). It follows the shape of phase 1's design (docs/design/horizons-phase-1.md,
#665) and reuses its patterns: the `core` rule, the building `link` field and the one-way haul
link (#681), and fuel bunkers (#675). It turns six techs into buildings, a bend, beasts and
TOML. The two owner questions are under "Open questions for the
owner", and the implementation work is under "Follow-up issues".

## Decisions in brief

1. **Six techs that change how goods and people cross the map.** `waystations` and `herding`
   (shared), `wind_gates` (Air), `cargo_balloons` (Fire), `canals` (Water) and
   `badgermole_ways` (Earth). None of them is `core`, so the gate doesn't grow.
2. **Links shorten routes, but they never connect new ground.** A Wind Gate or Balloon Dock pair
   must stand in one walkable region. With that rule, links are only a cheaper route between two
   places that already connect. Reachability, regions and job choice stay as they are, which
   keeps the change small. A gate to an island is later work.
3. **One beast system, built on bison haul (#205).** A new `data/beasts.toml` makes the sky bison
   its first row. The ostrich horse, komodo rhino, buffalo yak and badgermole are four more rows.
   A beast lives in a home building and eats from that building's food bunker (#675's bunker,
   holding food). It either hauls long trips or bores tunnel. Air has no herd beast (owner
   decision 7).
4. **Canal is a new terrain, not a river.** Carve Canal turns land beside fresh water into
   `Canal`. To every water rule it counts as a river: drinking, fresh water, a Fishing Dock or
   Tide Mill bank, fish spots, and #201's crossing rule. It also irrigates nearby plots. It is
   permanent, and it refuses a cut that would split a walkable region in two.
5. **The Waystation is an outpost, not a second base.** It has a pantry kept stocked from stores,
   two beds and a water barrel, and it must stand at least 20 tiles from any stockpile. It holds
   no general store. That keeps far yields flowing home, where beasts and balloons can carry
   them (#688).
6. **Everything stays weaker than the specialty it resembles.** Beasts carry 10 to 12 to the
   bison's 20, at a fixed speed, and they eat. Balloons burn wood every flight and fly a fixed
   line. Gates carry people only, and each pair is a fixed line. The table under "Against the
   specialties" checks each one.

## What ships

| Tech | Nation | Cost | Needs | Unlocks | The decision |
|---|---|---|---|---|---|
| `waystations` | shared | 250 | `cartography` | **Waystation** (pantry, 2 beds, a barrel, at least 20 tiles from stores) | Where the second site goes: the far ore, the mild land or the ruin |
| `herding` | Earth, Fire, Water | 250 | `farming` | **Stable** with the nation's beast | Food for hands, about 9 food an hour per beast |
| `wind_gates` | Air | 250 | `gliders`, `windmills` | **Wind Gate**, in pairs: people glide between them | Which far place gets the gate, knowing loads still walk |
| `cargo_balloons` | Fire | 500 | `steam_engines` | **Balloon Dock**, in pairs: goods fly between them | Which line deserves a balloon, at 2 wood a flight |
| `canals` | Water | 250 | `irrigation`, `water_travel` | Bend **Carve Canal** | Bring water to the fields or the mill to the workshop, and accept a permanent cut |
| `badgermole_ways` | Earth | 500 | `tunneling`, `seismic_sense` | **Badgermole Den**: bores designated tunnel with no chi | Chi now, or food and time for a long tunnel |

Per nation this adds 500 shared points (250 for Air) plus a nation tech: Earth and Fire 1,000,
Water 750, Air 500. Air's share is the smallest because the sky bison already covers its hauling.
File order puts the new techs after phase 1's, so the economy opening reaches them from about
minute 130.

## The `core` rule, unchanged

All six techs leave `core` at its default, false (horizons-phase-1.md, "The rule every phase-1
tech follows"). The branch sign doesn't count them, and the building-kinds sign doesn't count the
five buildings they unlock. The unit test from #673 that pins each nation's two targets runs again
with phase-2 data loaded.

## Shared sim work

Five pieces. The first three each serve two techs, the canal is Water's alone, and the last
holds the small schema changes.

### 1. Paired links

Phase 1 gives a building record `link: Option<Pos>` and `Command::SetLink { pos, to }` for the
chute's one-way foot (#681). A pair is the two-way case.

```toml
[building.pair]
reach = 48          # the partner's anchor is within 48 tiles (Chebyshev)
min_tiles = 12      # and at least 12 away: a short pair is a waste of a building
```

- **Linking.** `SetLink` on a `[building.pair]` building needs a `to` that is a building or a
  build site of the same id, within `reach` and at least `min_tiles` away. It sets both links
  and clears any partner either one had before. A site may be linked, so the build tool can
  place both ends in one go (the second placement is the partner, as a chute's foot is). The
  pair works once both ends are finished. Taking either end down clears the other's link.
- **The same-region rule.** Both anchors' door tiles must be in the same walkable region
  (`Regions`, under the settlement's current move rules). Otherwise the link is refused with
  `Not reachable on foot`, and a pair that a later map change splits stops working, with the
  status `Cut off`. #53 guarantees a ford between regions that mapgen divided, so on almost every
  map a pair across a lake or a range is still allowed. It saves the long way round. It never
  opens ground nobody could reach.
- **Why the rule.** Without it, reachability would need a region union through links, and a
  loaded hauler could be stranded on the far side of a people-only gate. Every job query and
  every "no settler can reach it" flag would have to learn about links. With the rule, links
  only change which route is cheapest.
- **Status.** `No partner`, `Partner unbuilt`, `Cut off`, or the partner's position and
  distance.

### 2. Routes through links

Two route rules, one for people and one for goods. Both compare a direct walk with a walk that
uses the link, and they cache one distance field per link end, keyed on the map revision that
the step grids already use.

- **People (Wind Gate).** A unit carrying nothing plans its route as today, then compares it
  with walk to gate A, plus the hop, plus walk from gate B to the destination. The two walks come
  from the cached fields of A and B. If the gate route is cheaper, the unit walks to A, stands
  for the hop, and lands on B. A hop isn't interrupted by a need, and beasts never hop. Job choice still ranks jobs by
  plain walking cost. A gate makes the trip faster once a job is chosen, and it doesn't pull
  people to far jobs. That is a simplification to revisit only if the harness shows idle gates.
- **Goods (Balloon Dock).** This is #681's chute rule with a flight in the middle. A unit
  picking up a stack bound for stores compares the walk straight to stores with the walk to dock
  A, plus the flight time at its own walking speed, plus dock B's cached walk to stores. Both
  directions work. Build-site deliveries, refuels and meals never use a dock, as with the chute.

### 3. Beasts

```toml
# data/beasts.toml (new). Read by fn_sim like the other data files; one row per beast.
[[beast]]
id = "sky_bison"
name = "Sky Bison"
nation = "air"
home = "bison_stable"   # each finished home building houses one beast of the settlement's nation
# speed omitted: the nation's own move_speed, as #205 specifies
[beast.haul]
carry = 20              # base_carry x 4 (#205)
min_trip = 10           # takes only trips of at least 10 tiles, pickup to drop (Chebyshev)
```

- **The system.** A beast is a unit with a `beast: Option<BeastId>` (or whatever form #205 gives
  the bison; this design only needs one place to say "this unit is a beast"). Beasts have no
  hunger, thirst, energy, chi, health or exposure. They aren't counted as people, they don't
  study and they never take a job that their row doesn't name. One row section is allowed per
  beast: `[beast.haul]` or `[beast.bend]`.
- **Home.** When a home building is finished, one beast of the settlement nation's row for that
  building appears at its door. Each `home` may be named by at most one row per nation. A beast
  with nothing to do walks back to its home and stands there. When the home comes down, its
  beast leaves the map, and the log reads `The ostrich horse returns to the wild.`
- **Eating.** A home building may have `[building.fuel]`. Its bunker burns while the home has a
  beast, and #675's Refuel haul keeps it stocked. While the bunker is empty, the beast goes
  home and works nothing, with the status `Hungry` (#689). The bison stable has no fuel
  section, so the bison never eats, as the owner scoped for v1. Food in a bunker is out of
  stores, as fuel is.
- **Hauling.** A `[beast.haul]` beast takes Haul jobs and Refuel hauls whose trip is at least
  `min_trip` tiles. It carries `carry` items, not scaled by `carry_capacity`, at `speed` (or the
  nation's `move_speed` when that is absent). It uses rail, roads and docks as a settler does.
  Settlers keep the short trips. Bison haul (#205) has the same rule, so #205's "at least 10
  tiles from a stockpile" becomes this row's `min_trip`.
- **Fuel takes a list.** `[building.fuel]` gains `items = [...]` as an alternative to `item`
  (exactly one of the two). A Refuel haul brings whichever listed item stores hold the most of,
  with ties broken by list order. A beast eats any food the settlement grows.
- **As built (#693).** `data/beasts.toml` is read after the other files, as `trade.toml` is
  (`Defs::with_beasts`), and `BeastId` is a saved content id like `AbilityId`. A beast is a `Unit`
  with `beast: Some(Beast { id, home })`, and `Unit::is_settler` is the test for "one of the
  people" that population, growth, beds and the signs use. Each tick starts with `tick_beasts`: a
  finished home building with no beast gets one at its door, and a beast whose home is gone
  returns to the wild (a `BeastEvent`, the line `The sky bison returns to the wild.`). That also
  gives a save made before beasts the beast for its stable. A trip is the Chebyshev distance
  from the pickup to the nearest place that would take the stack (a stockpile tile with room, a
  study hall for a scroll), or, for a Refuel, from the nearest stack of the fuel to the
  building. A hungry beast finishes the job it holds, then rests at its door and claims nothing;
  settlers still refuel its bunker (a beast refuels it too while it has some left). Beasts act
  before settlers each tick, so the long trips reach them first. A bunker that takes several
  fuels may hold a mix; it remembers the item last tipped in, which is what teardown returns.

### 4. Canal: a land-to-water reshape

- **A terrain.** `Terrain::Canal` is appended to the enum. Saves name terrain by its snake-case
  name, so old saves load, and mapgen never makes it, so no golden file changes. `is_water` is
  true, `is_walkable` is false, and it has no shore. Every place that asks for fresh water
  (drinking under harm, Water the Land's `source_range`, the river checks in `work.rs` and
  `bending.rs`) accepts it.
- **Data that names it.** The Fishing Dock's and Tide Mill's `beside` lists and the
  `fish_spot` resource's `terrains` gain `"canal"`. Mapgen never sees a canal, so the
  resource's spawn chance is unaffected.
- **A source rule for `reshape`.** The effect gains `source = [terrain, ...]`. The target must be
  4-adjacent to one of those terrains, or chained off a pending designation of the same bend
  (#304's chain, which Raise Earth and Bore Tunnel already use). A canal grows outward from the
  river a tile at a time, and a dragged line can be designated at once.
- **Standing beside it.** When `to` isn't walkable and the target is, the bender works from a
  walkable neighbour, and the cut waits until no unit stands on the tile. Draw Water keeps its
  old rule (it stands on the target), because its `to` is a walkable ford.
- **No split.** A canal tile is refused if carving it would leave its walkable neighbours in
  two regions, under the settlement's current move rules (`would cut the land in two`). The check
  is a flood bounded to the region, run at designation and again when the bend completes. A canal
  makes routes longer, and it never strands a building, a stockpile or a person. Once #201 lets
  Water's people cross water, that same check passes more often, since it uses the same rules.
- **Irrigation.** A canal tile irrigates farm plots within a radius, read by
  `World::crop_growth_rate` with the Irrigation Channel's rule: the best boost in reach applies,
  and two don't stack.

```toml
# data/bending.toml, a new top-level table: the terrain that Carve Canal makes
[canal]
irrigation_radius = 2   # Chebyshev, from each canal tile
growth_boost = 1.5      # the Irrigation Channel's own boost
```

### 5. Smaller pieces

- **A tech for some nations.** `[[tech]]` gains `nations = [...]`, the alternative to `nation`
  (exactly one of the two, or neither for a shared tech). It needs at least two nations, and its
  prereqs must be shared techs. Only those nations see it. `herding` is the first use, and
  phase 6's fusions (two nations each) are the next. The alternative, three nation techs named
  `herding_earth` and so on, would triple the entries and lose the single id the spec uses.
  **As built (#696).** `TechDef` keeps `nation` (`None` for a `nations` tech) and gains
  `nations: Vec<Nation>`. Ask `TechDef::open_to(nation)` and `is_shared()` instead of reading
  the two fields; `techs_for`, `set_research` (its refusal is `WrongNation`, "that tech belongs
  to another nation"), the build picker, the endeavor's reach and the autoplay all go through
  `open_to`. The loader refuses fewer than two nations, a nation named twice, `nations` beside
  `nation`, and a `nations` tech with a prereq that isn't shared. It also refuses any tech that
  requires a `nations` tech, a rule for phase 6 to relax when it needs one. A `nations` tech
  can't unlock an ability, since an ability belongs to one nation's own tech.
- **Per-flight fuel.** `[building.fuel]`'s `per_minute` becomes optional. When it's absent, the
  building's own effect section says when it burns: the balloon's `fuel_per_flight`.
- **A pantry and a barrel.** These are two new need sources, used only by the Waystation for now.
  A pantry is a bunker of food that hungry units eat from at the door, counted with the
  stockpiles in the need flood. A barrel is a drinking place at the door that counts as fresh
  water.

## The techs

Numbers are starting values for the harness (see "Measuring it"). Hover text means the build
modal's effects list and the cursor panel's inspect lines, in the voice of text-voice.md.

### `waystations`: the Waystation

```toml
# data/research.toml, appended
[[tech]]
id = "waystations"
name = "Waystations"
description = "Raise a shelter with food, water and beds where the crews work far from home."
cost = 250
prereqs = ["cartography"]
[tech.unlocks]
buildings = ["waystation"]

# data/buildings.toml, appended
[[building]]
id = "waystation"
name = "Waystation"
verb = "Build"
work = 60.0
passable = false
footprint = [2, 2]
capacity = 2           # two sleeping places inside (#227's rule for a bed bigger than 1x1)
[building.bed]
sleep_bonus = 1.2      # a bed is 1.4: a cot, not a home
seek_tiles = 30
[building.outpost]
far_from_stores = 20   # every footprint tile at least 20 tiles (Chebyshev) from any stockpile tile
pantry = 12            # food kept here, refilled at half by #675's Refuel haul
pantry_items = ["cabbage", "fruit", "fish"]
barrel = true          # a drinking place at the door
[[building.cost]]
item = "wood"
count = 10
[[building.cost]]
item = "stone"
count = 4
```

- **Rule.** The pantry, the beds and the barrel join the need floods. A hungry unit eats at the
  nearest stockpile or pantry with food, a thirsty one drinks at the nearest water's edge or
  barrel, and a tired one sleeps in the nearest free bed. Nothing else changes: crews working
  near a Waystation use it because it is nearest. The `far_from_stores` rule is checked only at
  placement. A stockpile drawn next to it later is the player's choice.
- **Numbers.** A crew 40 tiles from home walks about 80 tiles for each meal, drink and sleep,
  about 15 trips an hour, or roughly 12 minutes an hour at speed 1.0. A Waystation turns those
  into a few steps. Its pantry of 12 feeds two settlers for about 45 minutes, and the refill is
  a long trip that a beast takes when there is one.
- **Hover.** `Food, water and two beds for crews nearby` and `Must be 20 tiles from any
  stockpile`. Inspect: `Pantry: 9/12`, `Beds: 1/2`, or `Pantry empty` as a status.
- **The decision.** Where the second site goes. The far ore wants one by the quarry, the mild
  southern land wants one by new fields, and an unreachable-looking ruin wants one as a base for
  the search. Two Waystations need two pantries kept stocked, so a third is rarely worth it.

### `herding`: the Stable (Earth, Fire, Water)

```toml
[[tech]]
id = "herding"
name = "Herding"
description = "Raise beasts of burden to carry loads on the long trips."
nations = ["earth", "fire", "water"]
cost = 250
prereqs = ["farming"]
[tech.unlocks]
buildings = ["stable"]

[[building]]
id = "stable"
name = "Stable"
verb = "Build"
work = 80.0
passable = false
footprint = [2, 2]
[building.fuel]
items = ["cabbage", "fruit", "fish"]
bunker = 8
per_minute = 0.15      # about 9 food an hour, a little more than a settler eats
[[building.cost]]
item = "wood"
count = 12
[[building.cost]]
item = "stone"
count = 4
[[building.cost]]
item = "tool"
count = 1

# data/beasts.toml
[[beast]]
id = "ostrich_horse"
name = "Ostrich Horse"
nation = "earth"
home = "stable"
speed = 1.4            # fast: Earth's settlers walk at 0.9
[beast.haul]
carry = 10
min_trip = 10

[[beast]]
id = "komodo_rhino"
name = "Komodo Rhino"
nation = "fire"
home = "stable"
speed = 0.8            # slow and strong
[beast.haul]
carry = 12
min_trip = 10

[[beast]]
id = "buffalo_yak"
name = "Buffalo Yak"
nation = "water"
home = "stable"
speed = 1.0
[beast.haul]
carry = 10
min_trip = 10
```

- **Rule.** Each Stable houses one beast, as the Bison Stable houses one bison. The nation picks
  the row, so one building serves three nations with no `if nation` in the code.
- **Numbers.** On a 40-tile trip an ostrich horse moves about 18 items a minute, and an Earth
  settler, who carries 8 once `earth_rail` is known, about 9. The rhino and the yak each move
  about 12 a minute to a settler's 6. Every beast moves about twice what a settler does on a
  long haul, for about 9 food an hour. A sky bison moves about 55 a minute at Air's speed and
  eats nothing.
- **Hover.** `Houses an ostrich horse: carries 10 on trips of 10 tiles or more` and `Eats 9 food
  an hour`. Inspect: `Food: 5/8`, `Hauling 10 Stone`, or `Hungry`.
- **The decision.** Food for hands. From minute 60 food piles up and half the settlers are idle
  (pacing.md), so a beast is worth it only where the trips are long: the far quarry, the
  Waystation's pantry, the gate's ore. That is principle 5 at work, since a late tech eats from
  the glut.

### `wind_gates`: the Wind Gate (Air)

```toml
[[tech]]
id = "wind_gates"
name = "Wind Gates"
description = "Paired towers where airbenders ride the wind from one to the other."
nation = "air"
cost = 250
prereqs = ["gliders", "windmills"]
[tech.unlocks]
buildings = ["wind_gate"]

[[building]]
id = "wind_gate"
name = "Wind Gate"
verb = "Raise"
work = 50.0
passable = true        # the unit steps onto it to launch
[building.pair]
reach = 48
min_tiles = 12
[building.glide]
seconds = 2.0          # the hop takes 2 s plus 0.1 s a tile
seconds_per_tile = 0.1
[[building.cost]]
item = "wood"
count = 10
[[building.cost]]
item = "stone"
count = 2
[[building.cost]]
item = "tool"
count = 1
```

- **Rule.** A pair (shared work 1) that people cross by the route rule (shared work 2). Only a
  unit carrying nothing hops. A unit with a load walks.
- **Numbers.** A 48-tile hop takes 6.8 s. An Air settler with its branch walks 48 open tiles in
  about 13 s, so over open ground a gate saves half the trip. Its real value is over a range or a
  lake, where the walk is two or three times as long.
- **Hover.** `Carries people to its partner up to 48 tiles away` and `Loads stay behind`.
  Inspect: `Partner: (88, 31), 41 tiles` and `Crossed: 57`.
- **The decision.** Which far place gets the gate: the quarry, the ruin or the second grove. A
  crew can reach the far quarry in seconds, but its stone still walks home, or goes by Wind Carry
  or bison. Air's own tools for loads finish what the gate starts.

### `cargo_balloons`: the Balloon Dock (Fire)

```toml
[[tech]]
id = "cargo_balloons"
name = "Cargo Balloons"
description = "Hot-air balloons carry goods between two docks over any ground."
nation = "fire"
cost = 500
prereqs = ["steam_engines"]
[tech.unlocks]
buildings = ["balloon_dock"]

[[building]]
id = "balloon_dock"
name = "Balloon Dock"
verb = "Build"
work = 100.0
passable = false
footprint = [2, 2]
[building.pair]
reach = 64
min_tiles = 16
[building.balloon]
load = 20              # items a flight carries
hold = 40              # items waiting at a dock; a full hold sends haulers walking
wait_seconds = 60.0    # flies part loaded once the first item has waited this long
tiles_per_second = 1.5
fuel_per_flight = 2    # from the launching dock's bunker; empty flights burn too
[building.fuel]
item = "wood"
bunker = 10
[[building.cost]]
item = "brick"
count = 8
[[building.cost]]
item = "wood"
count = 10
[[building.cost]]
item = "metal"
count = 1
[[building.cost]]
item = "tool"
count = 1
```

- **Rule.** A pair has one balloon, which starts at the first dock finished. A stack dropped at
  a dock by the goods route rule waits in its hold. The balloon launches from its dock when the
  hold has `load` items, or when the oldest has waited `wait_seconds`. If the goods are at the
  other dock, the balloon flies there empty first. Every launch burns `fuel_per_flight` from the
  launching dock's bunker, and a dock with too little fuel doesn't launch (`Out of wood`). On
  landing, the goods spill beside the dock as stacks with ordinary Haul jobs, as at a chute's
  foot. Taking a dock down returns its hold and bunker.
- **Numbers.** A 48-tile line flies in 32 s. With an empty return that is about 20 items a minute
  for 4 wood, and about 37 a minute when both docks send goods. The wood comes from Fire's glut:
  the `fire-year-two` scenario holds about 2,100.
- **Hover.** `Flies goods to its partner up to 64 tiles away` and `Burns 2 wood a flight`.
  Inspect: `Partner: (61, 40), 38 tiles`, `Hold: 12/40`, `Fuel: 6/10 Wood`, `Balloon: landing in
  9s`.
- **The decision.** Which two points deserve a line. Haulers still walk to the near dock and from
  the far one, so the docks belong at the far quarry's edge and the stockpile's edge. A second
  line means a second pair.

### `canals`: Carve Canal (Water)

```toml
[[tech]]
id = "canals"
name = "Canals"
description = "Waterbenders cut channels that bring the river to the fields."
nation = "water"
cost = 250
prereqs = ["irrigation", "water_travel"]
[tech.unlocks]
abilities = ["carve_canal"]

# data/bending.toml, appended
[[ability]]
id = "carve_canal"
nation = "water"
name = "Carve Canal"
verb = "Carve"
description = "Cut a channel from the river. The water stays, and fields beside it grow faster."
chi = 60.0
work = 20.0
speed_modifier = "water_work"
[ability.effect]
kind = "reshape"
from = ["land", "coast"]
to = "canal"
source = ["river", "lake", "canal"]
```

- **Rule.** Shared work 4: a permanent `Canal` tile, 4-adjacent to fresh water or to a
  designated canal tile, refused if it would split a region. Sea doesn't count as a source,
  since a canal is fresh.
- **Why it waits on #201.** A canal is a river to the movement rules, and #201 decides those for
  Water. If #201 lets Water's people cross rivers, a canal slows them instead of stopping them.
  If #201 makes river travel cheaper than walking, a canal becomes a waterway. Either way its
  step cost must not beat the Paved Road's 1.25, so that roads stay below rail (see "Against the
  specialties").
- **Numbers.** 60 chi a tile, against Raise Earth's 50. Water's benders channel it in 10 s. A
  6-tile canal from the river to a field costs 360 chi, nearly four full meters. It irrigates a band 5
  tiles wide along its length at 1.5x. A Tide Mill on its bank brings 5 tiles of power reach to
  the workshops it reaches.
- **Hover.** The bend modal: `Turns land beside fresh water into a canal`, `Irrigates plots
  within 2 tiles` and `Permanent`. Refusals: `Needs fresh water beside it` and `Would cut the
  land in two`.
- **The decision.** It is permanent, and every tile of it is a tile nobody walks on or builds
  on. Water can bring the river to the fields, or bring the bank to the workshops for a Tide
  Mill and a Fishing Dock. A long canal does both and puts a wall through the settlement.

### `badgermole_ways`: the Badgermole Den (Earth)

```toml
[[tech]]
id = "badgermole_ways"
name = "Badgermole Ways"
description = "Badgermoles dig the tunnels that taught the first earthbenders."
nation = "earth"
cost = 500
prereqs = ["tunneling", "seismic_sense"]
[tech.unlocks]
buildings = ["badgermole_den"]

[[building]]
id = "badgermole_den"
name = "Badgermole Den"
verb = "Dig"
work = 90.0
passable = false
footprint = [2, 2]
beside = ["mountain", "tunnel"]
[building.fuel]
items = ["cabbage", "fruit", "fish"]
bunker = 10
per_minute = 0.2       # 12 food an hour
[[building.cost]]
item = "stone"
count = 10
[[building.cost]]
item = "wood"
count = 6
[[building.cost]]
item = "tool"
count = 1

# data/beasts.toml
[[beast]]
id = "badgermole"
name = "Badgermole"
nation = "earth"
home = "badgermole_den"
speed = 0.8
[beast.bend]
ability = "bore_tunnel"   # works this bend's designations with no chi
seconds = 120.0           # per tile, not modifier-scaled
```

- **Rule.** A `[beast.bend]` beast claims ready designations of its ability (`bend_ready`: a
  bank or a bored neighbour). It picks the one nearest its home by walking cost, so a dragged
  line is bored from the den outward. It works there for `seconds`, then the effect applies as if
  a bender had finished: the tile becomes tunnel, or a buried seam is struck. No chi is spent and
  no one's mastery counts it. A settler with the chi may still claim the same designations, so
  the player can rush a line by hand.
- **Numbers.** About 30 tiles an hour. An Earth bender channels a tile in about 7 s but pays 40
  chi, and chi comes back at about 2 to 8 a minute, so a bender bores about 5 tiles an hour
  alongside its other bends. The badgermole is chi-free and hands-free, slow per tile, and eats
  12 food an hour, idle or not.
- **Hover.** `A badgermole bores designated tunnel` and `Eats 12 food an hour`. Inspect:
  `Boring: 4 tiles left`, `Bored: 23`, `Food: 6/10`.
- **The decision.** Chi now, or food and time. A tunnel through a whole range to the far ore, or
  a mining gallery along a seam, becomes a plan you designate once. When the line is done, the
  den is worth taking down, because the badgermole eats while it waits.
- **As built (#697).** The beast takes only `Bend` jobs of its row's ability (`beast_may_take`),
  and `unit_can_bend` lets it claim with no chi and no rank. `pick_job` ranks its open jobs by the
  walk from the den's door (`beast_home_flood`), not from where it stands, then claims by the
  route from where it stands. `bend_rate` gives it `def.work / seconds` of channel a second, so
  a tile takes the row's `seconds` whatever any modifier says, and a job it leaves half done is
  half done for a settler too. A finished bore spends no chi and bumps `Beast::bored` (a saved
  field, default 0) in place of the unit's `bends`, `bends_by` and rank-up, so no one's mastery,
  the masters sign or the harness's top-bends count sees it. A struck seam and a bored tile go
  through the same `apply_effect` as a bender's. `World::boring_left(den)` (designations of the
  ability not yet finished, chained ones included) and `World::bored_by(den)` are the inspect
  numbers; `Food: 6/10` is the bunker's own readout. A hungry badgermole finishes the tile it
  holds, then rests at the door (the rule for every beast). Taking the den down leaves the
  designations standing for a bender.

## Against the specialties

The issue's test: roads, beasts and links stay weaker than the nation specialties they resemble.

| New thing | Resembles | Kept weaker by |
|---|---|---|
| Beasts | Sky bison (#205): 20 a trip, Air's speed, no upkeep | 10 to 12 a trip, a fixed speed of 1.4 at most, about 9 food an hour, one per Stable |
| Balloon Dock | Bison haul, and later bison flight (#254) | A fixed line, 2 wood a flight (empty flights too), and hands at both ends |
| Wind Gate | Bison flight (#254), people over terrain | A fixed line, people only, and the same-region rule |
| Canal as a waterway (with #201) | Earth's rail at 2.0 | #201's step on canal no better than the Paved Road's 1.25 |
| Badgermole | Earth's own benders | Slower per tile than a channel, and it eats. Its edge is only that it's chi-free |
| Waystation | A home base | Two 1.2 beds, a 12-food pantry, and placement at least 20 tiles out |

Air keeps the strongest hauler. Earth keeps the fastest track. Each nation's new thing answers a
weakness: Earth's slow walking gets the fast horse, Fire's long chain gets a supply line, Water's
weak stone and heat get water brought inland, and Air's weak carrying gets fast crews whose
loads are left to Wind Carry and bison.

## Mockups

These are hand-edited from `fn_tui --scenario earth-year-two --frame --ascii` and the
`fire-year-two` and `water-year-two` maps. The new lines are marked `<`, and the glyphs are the
TUI follow-ups' to settle.

The Research detail for `herding`, Earth:

```text
| ---------------------------------------------------------------------- |
| Herding                                            Available  250 pts  |
| Raise beasts of burden to carry loads on the long trips.               |
| Unlocks: Stable (Ostrich Horse)                                        | <
```

A Badgermole Den on the Earth map, selected. `D` is the den against the rock, `_` the bored
tunnel, `x` the designated line still to bore, and `m` the badgermole at the face:

```text
,,,,,,,,,,:::^^^^^^^^^^,,,,Y,,,.-----    HERE ------------ (21, 4) -
,,,,,,,,,,,,/,,^$^^^^^^^^,,,,,,..----    Badgermole Den               <
,,,,,,,,,,,/*,,^^^^^^^^^^^,,,,c,.----    Boring: 4 tiles left         <
,,,,,,,,,,,,,cDD____mxxxx^^,,,,,..---    Bored: 23                    <
,,,,,,,,,,,,,,DD^^^^^^^^^^^^,,=,,..--    Food: 6/10                   <
,,,,,,,,,,,,,,,,,^^^^^^^^^,,,,=,,,..-
```

A Balloon Dock pair on the Fire map, the near dock selected. `K` is a dock, `:` the link line
drawn only while a dock is selected, and `*` the balloon in flight:

```text
""""""//"""""""*,,,,*%,,,^^^,,,,,=,,..---    HERE ------------ (19, 2) -
"""""""Y""""KK"",,,/,,,,^^^,,,...=..-----    Balloon Dock                 <
""""""""""""KK:"""/%,,,,^^,,,,.----------    Partner: (37, 13), 18 tiles  <
""""""""""YY"""::B""f,,,^,,,,,.----------    Hold: 12/40                  <
"""""""""""""""""*:/Y,,,,,,,,,,...-------    Fuel: 6/10 Wood              <
""""""""""""""""=""//:,,,,,,,B/,,....----    Balloon: landing in 9s       <
"""...=..""""""="",,Y,,:,,,,Y,,%/,,,,,..-
""..----...""""=",,Y,,,,,:,,,,,,,,,,,,.--
"".-------..""=c",,,,c,B/,:,,,,,,,,,,,,..
...--------..=""",,,,,,/Y,,:,,,,,,,,,,,,/
.--f--------=.""",,,,,,,%,,,,:,,c,,,,/%/-
-------------..",,,,,,,,,,,,,,KK,,,,,,Y.-
--------------.",,,,,,,,,,,,,,KK,,,B/,,.-
```

A canal on the Water map, carved east from the river toward the plots. `=` is river and canal
alike in ASCII, `T` is a Tide Mill on the canal's bank, and the Unicode glyph set draws the
canal in a lighter blue:

```text
/"""""""""""""""="",,,,,,,,,c,,,,,....---    HERE ------------ (21, 7) -
/""...=..""""""==========Yc,c,,,,,,..----    Canal                        <
"/..----...""""=",,Y,,,TT,c,,,,,,,%,.----    Irrigates plots within 2     <
//.-------..""=c",,,,c,TT,,,,,,,,,%cc..--    Fresh water                  <
```

## Measuring it

Phase 2 ends with a harness run, and the economy opening must use the new content (#700):

- **Waystation.** Once, beside the farthest deposit the opening quarries, if it is 20 tiles or
  more from stores.
- **Stable.** One, for Earth, Fire and Water, once `herding` is known.
- **Wind Gate.** For Air, one pair from the stockpile's edge to that same far deposit, when the
  pair rule allows.
- **Balloon Dock.** For Fire, one pair from the stockpile's edge to the far deposit, when it is
  16 tiles or more.
- **Carve Canal.** For Water, up to 6 tiles from the nearest river toward the farm plots.
- **Badgermole Den.** For Earth, one den on the rock nearest the Mine Shaft, and a 12-tile
  tunnel line designated from it into the range.

The run reports these, on the panel's 32 seeds and both research orders, against a baseline from
`main` before the first phase-2 merge:

| Measure | Metric | Expected |
|---|---|---|
| Decision timeline | `longest_gap`, and the `timeline:` line | The 130-to-180 stretch gains 2 or 3 decisions per nation |
| Ore and stone | `sign_minute.ore`, `sign_minute.stone` (`ore_by_120`, `stone_by_120`) | Earlier or unchanged for every nation. If none moves, the routes aren't carrying the gate's materials |
| Food in stores at 120 and 180 | `food_at.M` (#684) | Earth, Fire and Water fall by about a Stable's 9 an hour. Still inside the `food_winter2` band |
| Wood in stores at 180 | `wood_at.180` | Fire falls by its balloon flights |
| No regressions | every existing target, through `balance verdict --against` | `no_deaths_by_180` is the one to watch. A Waystation's empty pantry mustn't strand a hungry crew |

The balance follow-up records the measured numbers in a "Measured" section here. The first
levers are each beast's `carry`, the Stable's `per_minute`, and the balloon's
`fuel_per_flight`, all data.

## Risks

- **#205 lands in a shape this doesn't expect.** The beast follow-up (#693) is written against #205's
  spec in late-tree.md: one bison per stable, hauling only, no needs, at least 10 tiles. If #205
  lands differently, D generalises what #205 built rather than adding a second system.
  `data/beasts.toml` is the target either way.
- **Route caches.** Each link end keeps one distance field, rebuilt lazily after a map change.
  Fire and Air build one or two pairs, so that is four floods at most. Canals change the map one
  tile at a time, which would rebuild them often while a canal is being carved. The cache is
  lazy, so a field is rebuilt only when a route asks for it.
- **The no-split flood.** It runs at designation and again on completion. A long dragged canal is
  checked tile by tile, each against the map with the earlier designations treated as carved.
- **A crew with an empty pantry.** Hungry units fall back to the nearest stockpile, which is
  home, so the worst case is today's walk. The harness's `deaths_by.180` watches it.

## Open questions for the owner

1. **What is a Waystation's store?** *Recommended: a pantry, beds and a barrel, with no general
   store.* The spec calls it "a small far stockpile". A real far stockpile would keep far yields
   in stores where they land, so a Mine Shaft beside it could be built from them. But it would
   also hide those yields from beasts and balloons, whose rules carry long trips to stores, and
   the gate's materials would then sit 40 tiles out. A player can already draw a far stockpile
   zone if they want one. Section: "`waystations`: the Waystation". (#688)
2. **What happens to a beast nobody feeds?** *Recommended: it rests at its home until the
   bunker is refilled, and it never dies or leaves.* The alternative is that it returns to the
   wild after 10 unfed minutes and the Stable must be rebuilt. That gives the upkeep real teeth,
   but it adds a loss to a peaceful game for a slip in hauling. Section: "Shared sim work, 3.
   Beasts". (#689)

## Follow-up issues

Each one appends its own tech, building, ability and beast blocks from this doc. `TechId`,
`BuildingId` and `AbilityId` are file positions, and saves remap them by name (save-compat.md),
so the order between follow-ups doesn't matter.

| Issue | Title | Blocked by |
|---|---|---|
| #690 | Sim: paired links (`[building.pair]`, two-way `SetLink`, the same-region rule) | #681 |
| #691 | Sim and data: the Wind Gate, people's routes through a pair, and `wind_gates` | #690, #673 |
| #692 | Sim and data: the Balloon Dock, goods' routes through a pair, per-flight fuel, and `cargo_balloons` | #690, #675, #673 |
| #693 | Sim and data: beasts (`data/beasts.toml`, bison as its first row, fuel `items`, the long-trip haul rule) | #205, #675, #689 |
| #696 | Sim and data: techs for some nations (`nations`), the Stable and `herding` | #693, #673 |
| #697 | Sim and data: the Badgermole Den, a beast that bores, and `badgermole_ways` | #693, #673 |
| #694 | Sim and data: the Waystation (pantry, barrel, far placement) and `waystations` | #673, #675, #688 |
| #695 | Sim and data: the canal terrain, Carve Canal, and `canals` | #201, #673 |
| #698 | TUI: placing pairs, their link lines, and dock, gate, Stable, den and Waystation readouts | #690, #691, #692, #696, #697, #694, #683 |
| #699 | TUI: beasts on the map and in the Settlement panel; the canal glyph and Carve Canal | #693, #695 |
| #700 | Balance: the economy opening uses phase 2, and the phase-2 run | #691, #692, #696, #697, #694, #695, #684 |
