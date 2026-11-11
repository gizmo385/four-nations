# Research and the tech tree

Doc status: current

Status: **proposed** (#17). Follow-up work: #18 to #28 (see the end of this doc).

Research is how a settlement grows past where it starts. It unlocks buildings, jobs and bending
abilities, and raises nation modifiers. It rewards peaceful growth: people who study instead of
hauling, exploring the map, and later, learning from neighbors (vision.md: one nation, no war).
There are no weapon or military techs.

## Decisions in brief

1. **One currency, one pool.** Research produces *knowledge*, an integer pool shared by the whole
   settlement. The player picks a target, and the sim spends from the pool when the target's cost
   is covered. Switching targets costs nothing, and knowledge banks while nothing is selected.
2. **Labor is the main cost.** Most knowledge comes from units doing a low-priority **Study** job
   at a study hall, so research competes with food and building for hands. A small passive
   trickle bootstraps the very first techs.
3. **Tree shape.** A shared civilization tier (11 techs) that every nation can research, plus one
   six-tech branch per nation that expresses its bending and culture. A nation never sees other
   nations' branches.
4. **Nations start differently.** Each nation starts knowing one tech: Fire knows `fire_making`,
   Water knows `fishing`, Earth knows `stoneworking`, Air knows `orchards`. Fire is *not* a
   given for the others. A Water settlement eats raw fish until it researches fire.
5. **Everything is data.** `data/research.toml` defines techs, costs, prereqs, nation
   restrictions and unlocks. The sim validates it strictly at load, like `defs.rs` does today.
6. **Modifiers stack multiplicatively**: effective value = nation base × each known tech's
   multiplier. Products are taken in tech-definition order, so the result never depends on
   research order.

## Producing knowledge

| Source | Rate (data, `[settings]`) | When |
|---|---|---|
| Passive trickle ("oral tradition") | 3 / min for the settlement | From tick 0. Bootstraps tier 0 and keeps a starving colony from stalling completely |
| **Study job** at a study hall | 9 / min per scholar × `study` modifier | Main source. Needs #4 (jobs); one seat per scholar |
| Ancient scrolls | +40 each, one-off | Rare map resource, hauled to the study hall (rewards exploration) |
| Neighbors (later) | Discounts, not points | See "Later" below |

- **Scrolls (#24).** `ancient_scroll` is a rare map resource in tundra and desert (3 to 6 on a
  192x128 map, `data/resources.toml`). Recovering one yields an `ancient_scroll` item whose
  `knowledge` (40, `[[item]]` in `data/jobs.toml`) is the points it is worth. It is never
  stockpiled: a hauler carries it to the nearest study hall, where it is used up and adds that
  many points to the pool. With no hall yet, it stays where it lies. The data shape is open for a
  later `reveals` field on the item.
- **Who studies.** Any unit can take a Study job. It's low priority, so needs (#3) and urgent
  work pre-empt it. That trade-off is the core research decision. Research costs time and
  labor, not materials, which keeps it readable. Skills and "elder" traits can raise output
  later.
- **Study hall.** Available from the start, with no tech. It has two seats, and each nation has
  its own name in data: Meditation Pavilion (Air), Elders' Lodge (Water), Scroll Library
  (Earth), Academy (Fire). `writing` adds seats.
- **Units.** Knowledge is stored as integer milli-points (`u64`), and data rates are converted
  once at load to millis per tick. At `TICKS_PER_SECOND = 10`, 3/min is 5 millis/tick and
  9/min is 15 millis/tick. Integer math keeps it deterministic and save-friendly.

### Pacing targets (first hour, one nation)

Assumptions: the first study hall is up by about minute 5 with one scholar, there are two
scholars by minute 15 and three by minute 40, and each scholar is about 2/3 effective because of
needs.

| Time | Knowledge earned | What the player has |
|---|---|---|
| ~6 min | ~25 | First tech (one of the four 25-cost basics) |
| ~20 min | ~180 | All of tier 0, plus a first 60-cost tech (farming or carpentry) |
| ~35 min | ~400 | First nation tech; masonry or herbalism under way |
| 60 min | ~1000 | About 10 techs, including `writing` (study ×1.25) |
| 2–2.5 h | ~2200 | The whole starter tree for that nation |

Cost tiers are 25 / 60 / 120 / 250 / 500. Balance by tuning the two rates in `[settings]`
first, and individual costs second.

## The starter tree

`Unlocks` names buildings, jobs and abilities that don't exist yet. They become real data as
#4, #6 and their follow-ups land (see "Unlocks" below for how that's staged). Multipliers are
written as `field ×N`.

### Shared civilization tier (11)

| id | Cost | Prereqs | Unlocks |
|---|---|---|---|
| `fire_making` | 25 | none | campfire; cook job (cooked food fills more hunger, #3). *Fire starts with it* |
| `shelter` | 25 | none | hut (better sleep, #3) |
| `fishing` | 25 | none | fishing-spot designation, fish job. *Water starts with it* |
| `stoneworking` | 25 | none | quarry designation on stone. *Earth starts with it* |
| `farming` | 60 | none | farm plot; sow and harvest jobs |
| `pottery` | 60 | fire_making | kiln; pots (water storage for thirst); granary |
| `carpentry` | 60 | shelter | workbench; wooden buildings; construction ×1.1 |
| `masonry` | 120 | stoneworking, carpentry | stone buildings; construction ×1.15 |
| `herbalism` | 120 | farming | herb garden; herbalist job (restores health, #3) |
| `writing` | 120 | pottery | +2 study hall seats; study ×1.25 |
| `cartography` | 250 | writing | unlocks the Map Room, which charts lost ruins nearest first (cartography.md, #400) |

### Earth Kingdom: shaping the land

| id | Cost | Prereqs | Unlocks |
|---|---|---|---|
| `earth_shaping` | 60 | stoneworking | level/raise-terrain designation; earthworks ×1.2 |
| `earthen_walls` | 120 | earth_shaping, masonry | earth wall (built, #25); flood barrier and terraces (not yet: nothing for them to do); construction ×1.25 |
| `tunneling` | 120 | earth_shaping | ability: Bore Tunnel, a walkable tunnel through mountain tiles; minerals beside a tunnel yield more ("Tunneling" below, #132) |
| `earth_rail` | 250 | earthen_walls | rail building: Ba Sing Se–style track that carts run along (#135, "Rail" below); carry_capacity ×1.25 |
| `seismic_sense` | 250 | tunneling | ability: Seismic Sense, uncovers the gold seams buried in the rock within a radius ("Seismic sense" below, #133) |
| `metalbending` | 500 | seismic_sense, masonry | ability: Metalbend, ore straight into metal, no smelter trip needed ("Metalbending" below, #134) |

Earth content pass (#25): only `earthen_walls` unlocked a concrete thing then (the `earth_wall`
building). `earth_shaping` is modifier-only for now (Raise Earth already
scales with earthworks). `tunneling` unlocks Bore
Tunnel since #132, `seismic_sense` unlocks Seismic Sense since #133 (both below), and
`metalbending` unlocks Metalbend since #134 (below). Rail was added by #135 (below), and the flood barrier and terraces are #136.
`crates/sim/tests/research_earth.rs` keeps that list honest. No costs or multipliers changed:
with the pacing assumptions above (one scholar from minute 5, two from 15, three from 40, each
two-thirds effective), an Earth settlement has about 1000 points at minute 60 and the whole tree
(shared tier plus branch, 2285 points) known at about minute 107, `metalbending` at about 71.

### Tunneling (#132)

`tunneling` promised "a tunnel designation through mountain tiles; mining inside mountains", but
mountains were solid: impassable, mined only from their edge, and the gold veins and stone deep
inside a range could never be reached. The owner decided (2026-10-25) that gold stays open to every
nation, so the tech must not gate a resource. It makes Earth *better* at mining minerals instead.

**What a tunnel is.** Three shapes were weighed:

- *A building* (like rail): reuses the build designation, but a building stands on buildable land,
  and every rule about footprints, rings and doors (#207 to #230) would need an exception for one
  that is dug *into* a mountain. It would also cost materials, when boring a tunnel moves rock
  rather than bringing any.
- *A designation of its own* (a new job kind): a second "change this tile" job beside bending's,
  with its own claim, walk, work and save code.
- *A bend* (chosen). Earth already has the right tool: Raise Earth is a `reshape` effect that turns
  one terrain into another, from a tile beside walkable ground, a tile at a time. **Bore Tunnel**
  is the same effect with `from = ["mountain"]` and `to = "tunnel"`, a new `Terrain::Tunnel`. It
  needed no new effect code, only data (`data/bending.toml`), and `unlocks.abilities` gates it
  behind the tech, the way Inner Fire is gated behind `dancing_dragon`. Being a bend is also what
  makes it Earth's: earthbenders open the rock, spending chi rather than materials, so there is
  no `if nation == Earth` anywhere.

The tunnel itself:

| | Value (`data/bending.toml`, confirmed by the owner 2026-10-25) |
|---|---|
| Chi | 40 (two bores per 28-minute chi cycle, see bending.md "Chi") |
| Channel | 20 s ÷ `earthworks` (8 s for Earth's 2.5, 6.7 s with `earth_shaping`) |
| Target | A bare Mountain tile (no stone or vein on it: quarry it first), 4-adjacent to walkable ground, which includes another tunnel |
| Result | Terrain becomes `Tunnel`: walkable at cost 1 by every unit, not buildable (`Blocker::Terrain`), permanent |

Tunnels grow inward one tile at a time from the mountain's edge or from the last tunnel tile, so a
long one is many bends; like a ford, there is no undo in v1. A tunnel also joins the two sides of
a range, so it shortens hauls the way a ford does across water.

**How tunneling raises mineral yields.** Four options were weighed:

1. *A yield or speed modifier* (`mining x1.5` on the tech). The cheapest to build, but invisible:
   it changes a number and nothing on the map, and gives the tunnel nothing to do.
2. *Deep veins become reachable.* True of any walkable tunnel, for free: veins inside a range that
   no unit could stand beside are now in reach. It adds minerals to the map but makes none richer.
3. *Mining from inside a tunnel yields more* (chosen, together with 2). A vein or outcrop that a
   tunnel tile touches (any of its 8 neighbours) is mined from inside the rock and yields its
   harvest's `tunnel_count` instead of `count` (`data/jobs.toml`). It is visible (the tooltip
   says "Mine: 2 Gold Ore from the tunnel"), it makes the player decide where to dig, and it
   costs chi and channel time to set up, so it pays best on a cluster of outcrops a tunnel can
   run between.
4. *Mined-out veins regrow slowly.* Would make minerals renewable, which changes the whole
   economy for every nation rather than Earth's mining; better left for the metal and power work
   (#124, #134) if it is ever wanted.

So a tunnel both opens the inside of a mountain (2) and makes what is mined from it richer (3).
Yields (confirmed by the owner 2026-10-25): stone 3 → **5** per outcrop, gold 1 → **2** per vein. Work time doesn't change.
The rule is per harvest (`tunnel_count`), so ore and metal from #134 can carry their own.

**Gold and stone stay open to everyone.** Neither harvest is gated by `tunneling` itself: stone
needs the shared `stoneworking`, and any nation's units mine an edge vein for the plain `count`.
Nothing asks who dug the tunnel or who mines beside it; only Earth can bore one.

*Update (#383, 2026-11-05):* "gold needs no tech" above was this decision's own framing, and it
turned out to be the bug: #379 found that no density or yield retune could move the ore sign's
timing, because nothing delayed *when* mining could start at all (unlike stone, already gated
behind `stoneworking`). The owner's 2026-11-05 decision adds a shared `prospecting` tech that
gates gold mining the same way, for every nation including Earth (`data/research.toml`); see
`docs/design/materials.md`'s own follow-up there. Earth's lead from `tunneling` and
`seismic_sense` is unaffected: both still sit later in Earth's own branch, after the shared tier
`prospecting` sits in.

Tests: `crates/sim/src/world/bending/tunnel_tests.rs` (locking, targets, a deep vein reached and
mined, the same vein mined with and without a tunnel, every nation mining without the tech).
Drawing: the client draws a tunnel as a packed-earth floor (`art/floors.toml`, `tunnel0` to
`tunnel3`) with the mountain's own cliff rim facing it, since a tunnel counts as lower ground;
the TUI shows `∩` (`_` in `--ascii`, the mapgen renderer's glyph).

### Seismic sense (#133)

`seismic_sense` promised "reveal ore and gems in a radius", but nothing on the map was hidden:
every resource showed from tick 0. Fog of war for the whole map is out of scope (cartography's
reveal is separate), so the hidden state is narrow: **buried resources**.

**What is hidden.** A resource can be `buried = true` in `data/resources.toml`. Mapgen places it
like any other (its own RNG stream, listed last so it never displaces another resource), and a
world takes it off the map the moment it is made, into `World::buried`. Until it is uncovered it
is not on its tile, so no frontend draws it, no tooltip names it and no designation can reach it;
nothing else in the code has to ask whether a resource is hidden. The one buried resource today
is the **gold seam**: a rich deposit in mountain rock, worth more than a surface vein. Gems were
weighed too, but a new item with no use yet would only add art; gold already exists, and ore and
metal from #134 can be buried the same way when they do.

Other shapes considered: a per-tile "revealed" flag that every frontend checks (the leak-prone
option: one forgotten check shows the gold), and true fog of war (out of scope, and a far bigger
change).

**How the reveal shows it.** Seismic Sense is a new `Effect::Reveal { radius, finds }` (`finds`
lists which buried resources the sense can uncover, so a later sense can be narrower than "every
buried thing"; Seismic Sense's is `["gold_seam"]`, below):

| | Value (`data/bending.toml`, confirmed by the owner 2026-10-25) |
|---|---|
| Chi | 30 |
| Channel | 6 s ÷ `earthworks` (2.4 s for Earth) |
| Target | A walkable tile with nothing on it (open ground or a tunnel): where the bender stands |
| Result | Every buried resource within `radius = 8` tiles (Chebyshev) of the target is put back on its tile, to be mined or tunnelled to like any vein. Finding nothing still spends the chi |

The seam simply appears in the rock (art: `gold_seam` in `art/resources.toml`, the vein's rock
split open on a thick band of gold), and the cast draws a tremor: the ground cracks under the
bender and a ring of dust runs out (`cast.rs`, `Look::Pulse`). A Bore Tunnel aimed at a tile
that holds a buried seam strikes it instead of boring: the seam is uncovered and the rock stays,
so a tunnel can find gold by luck, but the sense is the way to look.

| Gold seam (`data/resources.toml`, `data/jobs.toml`, confirmed by the owner 2026-10-25) | Value |
|---|---|
| Chance | 0.04 per mountain tile nothing else took, `min_spacing = 5` |
| Yield | 3 gold ore (5 mined from a tunnel), against a vein's 1 (2) |
| Work | 45 s ÷ `earthworks` |

Saves: `World::buried` is left out of a save while empty, so saves from before #133 load (with
nothing buried) and re-save unchanged. Mapgen now places seams, so the large ASCII golden was
re-blessed (only `!` marks were added; nothing else moved). Tests:
`crates/sim/src/world/bending/seismic_tests.rs`.

### Gliders (#197)

`gliders` promised "scouting reveals the map", and was modifier-only (move_speed ×1.15) until a
reveal existed. #133 gave the sim one, buried resources, so gliding reuses it rather than adding
fog of war: what the glider sees from the air is a second kind of buried thing, **lost ruins**.

**What is hidden.** `lost_ruins` (`data/resources.toml`, `buried = true`) are old settlements the
grass has grown over: open land in any biome, listed last so they only take tiles nothing else
holds, and spaced far apart (`min_spacing = 20`): about 8-12 on a 192x128 map. Searching a ruin
(`data/jobs.toml`, verb "Search") turns up 2 ancient scrolls, the readable item from #24, so a
ruin is worth a trip where a lone scroll in the wastes is worth one; the ruin is gone once
searched. Ruins pay in knowledge, not goods, which fits the Air Nomads.

Other things the glider could have revealed, considered: gold seams too (but then Earth's own
sense is redundant for Air, and Air has no tunnels to mine them from), or anything buried (the
same objection). Each sense now finds only its own kind: `Effect::Reveal` carries the `finds`
list described above, and Seismic Sense's stays `["gold_seam"]`, so the rock keeps its seams
from a glider and the earth can't feel ruins.

**The glide.** `glide`, unlocked by `gliders` alongside its speed modifier:

| | Value (`data/bending.toml`) |
|---|---|
| Chi | 40 |
| Channel | 8 s (no speed modifier) |
| Target | A walkable tile with nothing on it: where the bender takes off |
| Result | Every lost ruin within `radius = 14` tiles (Chebyshev) of the target appears on its tile, to be searched. Finding nothing still spends the chi |

The reach is wider than the sense's 8 because it is seen from the air and ruins are sparse. Art
and the cast's look are frontend work for a later issue (#283 moves the client to the TUI); the
TUI glyph is `‖` (`n` in `--ascii`).

**Uncovering needs a free tile.** A buried resource can have been built or walked over since the
map was made: a ruin under open land is far more likely to have a house or a field on it by the
time someone glides over it than a gold seam is to have anything on its mountain tile. So
`World::uncover` (used by both Reveal and a Bore Tunnel striking a seam) now checks the tile is
still free -- no building, building site, stockpile zone, ground stack or bend designation on it,
and no unit standing on or routed through it -- and leaves a resource buried if it isn't: a later
reveal finds it once the tile clears. This applies to gold seams too, though a mountain tile is
rarely built on.

| Lost ruins (`data/resources.toml`, `data/jobs.toml`) | Value |
|---|---|
| Chance | 0.005 per land tile nothing else took, `min_spacing = 20` |
| Yield | 2 ancient scrolls |
| Work | 20 s |

Mapgen now places ruins, so the ASCII goldens were re-blessed (only `n` marks were added; nothing
else moved). Tests: `crates/sim/src/world/bending/glide_tests.rs`.

### Metalbending (#134)

`metalbending` promised "metal from ore without a smelter", which needed two things #124 (Fire
production) left as hooks: the `metal` item (already in `data/jobs.toml`, since the smelter makes
it too) and a way to turn ore into it outside a building.

**The ability, not a building.** The smelter (`data/buildings.toml`) is Fire's path to metal: haul
ore to the building, work there in batches, forever, through a `Craft` job (`world/forge.rs`,
#124). Metalbend is Earth's: a bend like any other (`Effect::Smelt`, `world/bending.rs`), it works
a ground stack of ore right where it sits, no building or trip needed, converting it on the spot
at the cost of chi and a channel. The two co-exist rather than one replacing the other: Fire's path
produces steadily and unattended once the ore keeps coming; Earth's is instant (one cast) but
spends chi and needs a bender free to walk there, which fits bending's own rhythm of designate,
walk, channel, spend.

| | Value (`data/bending.toml`) |
|---|---|
| Chi | 35 |
| Channel | 12 s ÷ `earthworks` |
| Target | A ground stack of `gold_ore` (the only ore today) |
| Result | The stack converts to `metal` at the smelter's own ratio, 2 ore for 1 metal. A remainder that doesn't divide evenly (an odd ore count) stays behind as ore |

Like Wind Carry, designating it over a ground stack takes over the tile's haul job, so a hauler
doesn't carry the ore off mid-channel; the job comes back if the bend is cancelled, or the stack
turns out gone or too small (less than 2 ore) when the channel completes. The cast's look borrows
Raise Earth's (`cast.rs`, `Look::Rise`): the client is being removed (#283), so no new art. The TUI
highlight is the plain target tile, same as Raise Earth and Bore Tunnel.

Tests: `crates/sim/src/world/bending/metalbend_tests.rs`.

### Rail (#135)

`earth_rail` promised "rail for haul routes", but hauling had no routes to speed up: a unit picks up
a stack and walks it to a stockpile tile along the cheapest path, and a path's cost depended only
on the terrain it enters. The decisions:

1. **Rail is a building, not terrain or a designation.** A `rail` building (`data/buildings.toml`,
   `[building.rail] speed = 2.0`) is 1x1, passable and laid one tile at a time with the ordinary
   build designation, so it reuses everything buildings already do: cost hauled to the site, work,
   tech gating (`Unlocks::buildings`, "Requires Earth Rail"), the picker, saves and the
   `Tile::building` the pathfinder already reads. The alternatives were a new terrain (which
   needs map editing and mapgen rules for something the player makes) or a "haul route"
   designation (a second kind of build order, and a route object to keep in step with the map).
   Tiles also let the player grow a line, branch it and break it, which is what makes a route.
2. **A hauling unit goes `speed` times as fast from one rail tile onto the next beside it.** The
   rule is on the *step*: a straight step from a rail tile onto a rail tile costs
   `STRAIGHT / speed` (5 instead of 10 at speed 2.0) whatever the ground under it. A diagonal is
   an ordinary diagonal (a track doesn't run that way), and a lone tile does nothing, so rail
   has to be laid as lines. `MoveRules::step_cost` is the one function that says what a step costs;
   `path::find`, `Flood`, `path_cost` and `step_needed` (the walk itself, and so the client's
   `Stride`, #194) all ask it, so a path, its cost and the time the walk takes cannot disagree.
3. **Every unit uses it, loaded or not.** (Owner, 2026-10-25. #135 first shipped rail for
   carriers only; the owner chose a road for everyone and will revisit if it proves too strong.)
   A hauler runs both legs of a trip along the line, and anyone walking to work, food or bed
   gains too. `World` keeps two `MoveRules`, `rules` for walking and `cart_rules` (walking plus
   rail), and the free function `rules_for` in `world/work.rs` is the only place that picks one;
   planning (`flood_from`), walking (`walk`) and the stride view (#194) all go through it. Today it
   always returns `cart_rules`. *To make rail a freight line again, return `cart_rules` only for a
   unit that carries a stack.* Because routes are planned with these rules, a loaded unit prefers
   a stockpile that rail makes nearer, which is what makes laying a line to a stockpile worth
   doing.
4. **Earth only, through data.** The `rail` building is unlocked by `earth_rail`, an Earth tech, so
   only Earth's tree reaches it; nothing in the code checks the nation. The tech's
   `carry_capacity` x1.25 stays: bigger carts, then rails for them. The other nations' units on a
   map with rail (none yet) would behave the same, because the speed is a property of the tile
   and the unit's load, not of who is pushing.
5. **Numbers** (placeholders for a balance pass): speed 2.0 (half the time on the loaded leg),
   2 stone and 10 s of work a tile. A ten-tile line is 20 stone and under two minutes of one
   builder's time, and saves 30 ticks (3 seconds) on every loaded trip along it.
6. **Not in this issue:** rail for non-Earth nations, trains, junction rules and
   stations. Bridges and tunnels are the rail's natural next step; tunnels exist since #132,
   and rail can't be laid in one yet (a tunnel isn't buildable ground).

The client draws a rail as flat track, not a building: sixteen pieces on the Earth sheet, one for
every set of neighbours a tile joins, chosen from the rail tiles around it (`link_frame`), so a
line is one track and a corner turns. The TUI shows it as `╪`.

### Fire Nation: heat and industry

| id | Cost | Prereqs | Unlocks |
|---|---|---|---|
| `forging` | 60 | fire_making | forge; smith job (metal into tools, #124) |
| `brickmaking` | 120 | forging, pottery | brick kiln (stone into bricks, #124); construction ×1.15 |
| `smelting` | 120 | forging | smelter (ore into metal, #124); heating ×1.2 |
| `dancing_dragon` | 250 | smelting | Sun Warrior teachings: heating ×1.25; ability: Inner Fire, a warmth aura that follows the bender (bending.md, #123) |
| `steam_engines` | 250 | smelting, brickmaking | steam engine powers mills and pumps; power_generation ×1.25 |
| `lightning_generation` | 500 | dancing_dragon, steam_engines | lightning power station (Republic City style); power_generation ×1.5 |

The buildings (`forge`, `brick_kiln`, `smelter`, `steam_engine`, `lightning_station`) exist in
`data/buildings.toml` and are gated by their techs (#26). `forge`, `brick_kiln` and `smelter`
produce now (#124, "Fire production" below); `steam_engine` and `lightning_station` stay
structures only until the power system lands (#255). No branch cost or multiplier changed in
#26 or #124; the playtest in `crates/sim/tests/research_fire.rs` has a Fire settlement with an
Academy (two scholars) finish the whole Fire tree, shared tier included, in about 107 minutes,
inside the 90 to 187 minute window (the 2 to 2.5 h target, plus or minus 25%).

Lightning is a *power source*, not an attack. Redirection can stay flavor text.

### Fire production (#124)

`forging`, `brickmaking` and `smelting` promised a forge, a brick kiln and a smelter that *do*
something, and a smith job gated behind `forging`; #26 had only raised the buildings as 2x2
structures (#214). Three decisions:

1. **One recipe per building, on its def** (`BuildingDef::recipe`, `[building.recipe]` in
   `data/buildings.toml`), not a separate recipe file or a player-authored work order: each of
   these three buildings only ever does the one thing its tech promised, so there's nothing for
   the player to choose. `inputs` (one or more `{ item, count }`) is what one batch consumes,
   `output`/`output_count` what it yields, `work` the in-game seconds once the inputs are in
   (scaled by a nation modifier if the recipe names one -- all three use `heating`, the modifier
   `smelting` already raises and Fire already leads in).
2. **A recipe building is a study hall with a different seat.** Its `capacity` (here, 1) opens
   that many `JobKind::Craft` jobs the moment it finishes construction (`world/forge.rs`,
   mirroring `world/study.rs`'s seats), and a unit holding one works *inside*, through the door
   (#227) -- the owner's cue that units-in-buildings was meant for exactly this. Fetching a
   batch's inputs reuses the same nearest-stockpile-or-ground-stack search a `Build` job fetches
   its cost from; the only new code is what happens once they're in: work, drop the output
   nearby for hauling, clear the batch, start the next one. Unlike a build site, a craft job
   never finishes.
3. **Items, minimally.** `metal`, `brick` and `tool` are new (`data/jobs.toml`); the smelter's
   ore is the `gold_ore` that mining already yields (#132, #133) -- there's no separate generic
   "ore" item, since gold is the only mineral any nation mines yet. `metal` is also the hook
   #134 (metalbending) needs: that tech is meant to produce it directly, without a smelter.
   `tool` had no use at first (nothing asked for one to work faster or build cheaper); #369
   (docs/design/nation-buildings.md, "Tools") gives all three a job: bricks and metal pay for
   Fire's own power buildings (the steam engine and the lightning station), and tools -- made by
   every nation's Workshop (`carpentry`) or, for Fire, twice as fast at the forge -- pay for the
   advanced buildings every branch raises, plus the Harmony Gate's Arch.

Costs for the three buildings got a first balance pass (placeholders from #26): `brick_kiln`
and `smelter`'s work rose a little (70 s, 90 s) to sit above `forge`'s (60 s), since both are
120-knowledge techs against `forging`'s 60, and `smelter`'s wood cost rose to 6 to match
`brick_kiln`'s. Recipe numbers (2 ore -> 1 metal in 20 s; 3 stone -> 4 bricks in 15 s; 1 metal ->
2 tools in 25 s, #369) are starting values, not a design target.

Power (the steam engine, lightning station, a grid) is out of scope: #255.

### Water Tribes: flow and healing

| id | Cost | Prereqs | Unlocks |
|---|---|---|---|
| `irrigation` | 60 | farming | irrigation channel (farm plots within reach grow faster, #126); water_work ×1.15 |
| `ice_construction` | 120 | shelter | igloo (a cheap bed with better sleep; wood until there is an ice item); ice blocks as a material later |
| `healing` | 120 | herbalism | Healing Hut (hurt settlers rest and mend, #375); healer job and ability: healing water still wait on #358 |
| `tide_mills` | 250 | irrigation | Tide Mill (a power source built on the bank, #200); power_generation ×1.25 |
| `water_travel` | 250 | fishing, ice_construction | canoes and ice skiffs; units can cross water tiles (#127) |
| `spirit_oasis` | 500 | healing, tide_mills | spirit oasis (chi and mood regeneration, #127); water_work ×1.25 |

`ice_construction` (#27), `irrigation` (#126) and `tide_mills` (#200) unlock a concrete def so far; the rest stay
modifier-only or empty until the issues above add the mechanics they need. A headless playtest
(`crates/sim/tests/research_water.rs`) following the pacing assumptions above researches the whole
Water tree (2440 points, up from 2190 since #383 added the shared `prospecting` tech) in about
118 minutes with ~950 points earned by minute 60, which is inside the targets within 25%.

No bloodbending. It's a dark art that doesn't fit peaceful progress.

### Air Nomads: lightness and spirit

| id | Cost | Prereqs | Unlocks |
|---|---|---|---|
| `orchards` | 25 | none | orchard (plants and tends fruit trees: renewable fruit, edible raw; see "The orchard" below). Wild fruit picking is open to every nation. *Air starts with it* |
| `meditation` | 60 | none | chi regeneration; study ×1.2 |
| `gliders` | 60 | carpentry | glider staffs; move_speed ×1.15; ability: glide (spot lost ruins from the air, #197) |
| `windmills` | 120 | gliders, farming | windmill (grinds grain for more food); power_generation ×1.2 |
| `air_scooter` | 250 | gliders, meditation | ability: air scooter (a temporary speed burst on the rider, docs/design/bending.md, "Air Nomads: Air Scooter", #143); move_speed ×1.2 |
| `wind_turbines` | 250 | windmills | Wind Turbine (Air's power source, built anywhere, #204); power_generation ×1.25 |
| `sky_bison` | 500 | meditation, air_scooter | bison stable; bison haul and fast travel; carry_capacity ×1.3 |

The buildings (`orchard`, `windmill`, `wind_turbine`, `bison_stable`) exist in
`data/buildings.toml` and are gated by their techs (#28). The orchard works (#142, below), and
the wind turbine is Air's power source (#204). The other two are structures only: the windmill
is decided as a power source too (decision #465) but not yet data (#488), and the bison stable
waits on its own system (#205); their costs are placeholders.
`meditation` is modifier-only by design. No branch cost or multiplier changed in #28; the
playtest in `crates/sim/tests/research_air.rs` has an Air settlement with a Meditation Pavilion
(two scholars) finish the whole Air tree, shared tier included, in about 94 minutes (meditation's
study ×1.2 puts it near the fast end), inside
the 90 to 187 minute window (the 2 to 2.5 h target, plus or minus 25%).

Air starts behind on labor (carry 0.8, earthworks 0.7), so its starting tech secures food
instead. The Air Temples' orchards fit the nomads' vegetarian life and give a steady supply of
food that needs no fire. `meditation` is an early research choice, not a starting tech. The
study hall exists from the start anyway (named Meditation Pavilion for Air), so meditation
doesn't unlock a building.

Speed check: Air's base `move_speed` is 1.6, and `gliders` (×1.15) and `air_scooter` (×1.2)
bring it to about 2.2× baseline. Revisit in a balance pass.

### The orchard (#142)

The owner's rescope (2026-10-25): **wild fruit picking stays open to every nation**, and `orchards`
unlocks only the *orchard building*, which plants and tends new `fruit_tree` resources. It is a
renewable food source that only an Air settlement can create. Every number is in
`[building.orchard]` in `data/buildings.toml` (`OrchardDef`), so the whole design below is
retunable without touching code.

**How it plants.** Planting is a process of the building, not a job. A finished orchard has a
timer: while it tends fewer than `max_trees` (6) trees, every `plant_seconds` (90) it plants one
more on a random free land tile within `radius` (4) of itself, drawn from the `orchard.plant`
RNG stream (so it is deterministic per seed). The first one comes 90 s after the building is
finished. A planted tree is the existing `fruit_tree` resource, so the map, mapgen, the picking
job, the stockpile, the eating rules and save files all see nothing new; nothing was added to
`data/resources.toml`, so mapgen's resource streams (#225) are untouched. A tile is eligible when:
it is land (the terrains `fruit_tree` spawns on; the biome is deliberately *not* checked, so a
grove can stand where the wild one wouldn't, which is what tending is for); nothing is built,
designated, stockpiled, lying or bent there and no unit stands on it or is walking over it; and
no other resource or building lies within `spacing` (2) tiles, which leaves a walkable lane
between rows, keeps a tree from walling a unit in, and keeps a canopy off the orchard's own
art. If there is no such tile the orchard waits another interval.

*Why not a planting job.* A job would make units walk out and plant, which is nicer to watch, but
it needs a new `JobKind` threaded through claiming, pre-emption, abandonment and every client
job view, and a sapling item to haul. The orchard is a grove keeper standing in for the
nomads' labour: it costs the player no worker time, which fits Air starting behind on labour
(carry 0.8, earthworks 0.7). If the owner wants planting to cost labour, the clean change is a
`JobKind::Plant` opened by the orchard each interval (the planting rules in `world/orchard.rs`
stay as they are); it is a data-light follow-up, not a redesign.

**Growth time.** A new tree is a *sapling*: a `fruit_tree` that is resting for `grow_seconds`
(480) before its first fruit, drawn small (below) and not pickable until then. It is within the
orchard's reach, so it grows at the tending rate (next paragraph): 480 / 1.5 = **320 s**, about 5
minutes. So the first fruit is ready 90 + 320 = 410 s (under 7 minutes) after the orchard is
finished, and a full grove of six is standing by 540 s and fully ripe by about 860 s (14 minutes).

**How many it tends.** `max_trees` = 6. It counts the trees it planted that are still standing,
and replants one (on the next interval) if a tree is lost. At six it stops, so it is a bounded
food source, not a spreading forest, and a settlement wanting more builds another orchard.

**Tending, and regrowth after picking.** A planted tree regrows after picking exactly like a wild
one (`regrow` = 360 s in `data/jobs.toml`; nothing special is stored on it), but the orchard
*tends* it: any resting tree of the planted kind within `radius` of an orchard, planted or wild,
regrows `tend_boost` (1.5) times as fast (`World::growth_rate`, the hook #56 left for exactly
this; the best boost among orchards in reach applies and they don't stack). So a tree that
bears 2 fruit every 360 s in the wild bears them every 240 s in an orchard.

**Compared with #81's standing harvest orders.** A standing order (#81) is the player's: "keep
picking this tile". A planted tree starts with one (`auto_pick`, on), so a settlement with an
orchard and a stockpile gets a steady flow of fruit with no designating: each tree queues a pick
job when it ripens and units haul the fruit as for any harvest. It is an ordinary standing order
otherwise: it shows as a designation tint, `CancelHarvest` withdraws it (the tree keeps growing
and can be picked by hand or by a hungry unit foraging), and Shift+Harvest sets it again. The
alternative, that the player has to order every tree, makes the orchard a chore, and the player
can still opt out per tree. Set `auto_pick = false` to remove the default.

**Balance.** A full orchard yields 6 trees x 2 fruit per 240 s = about 3 fruit a minute, nine times
a wild tree (0.33). A settler eats about 0.13 fruit a minute (4 hunger a minute, 30 per fruit), so one
orchard feeds a couple of dozen settlers' raw hunger, which is generous but is the point of
Air's starting tech: it makes the nomads the nation that never has a food problem, to pay for the
weak labour modifiers. Cost: placeholder 4 wood and 30 s of building became **8 wood and 45 s**
(about twice a bed's 5 wood and 20 s, since a bed is the first building anyone raises), which a
starting settlement can afford in the first minutes and which pays back by its first picked fruit.
Revisit when farm plots (#198) give Earth a competing food source.

**Only Air can build one.** `orchards` is an Air-branch tech, so the other nations can neither
research it nor designate the building (`BuildError::NotResearched`), and the build picker doesn't
list it for them (#137). There is no `if nation` in the orchard code: the gate is the tech. The
orchard code itself is nation-blind, and a different nation's building could carry the same
`[building.orchard]` section.

**What the player sees.** A sapling is a one-cell sprite (`sapling` in `art/resources.toml`, 16px
like the cabbage) drawn instead of the 2x2 tree until it first ripens, so a young grove reads at a
glance, and the tree replaces it in place once grown (with its canopy fade, #77, intact). The
terminal draws it `ψ` (`y`). The inspector says "Sapling" for it and "Tends 3 of 6 trees" for the
orchard. Mockups: `mockups/142/`.

**Saves.** New world state (`orchards`, `saplings`, `orchard_rng`) is `#[serde(default)]`, so
older saves load unchanged; `World::adopt_orchards` registers an orchard that is already on a
loaded map, so saves from before this issue start planting. The save version doesn't change.

### Farm plots (#198)

`farming` (shared tier, every nation) unlocks the `farm_plot` building: tilled ground, 1x1 and
walkable, raised like any other building (`Command::DesignateBuild`). Unlike the orchard, which
plants and tends itself, a farm plot needs the player's hand twice a cycle, which is the
difference the design docs asked for between the two foods: the orchard is Air's hands-off
specialty, farming is everyone's labour-for-food trade.

**The cycle.** A finished plot starts **empty**. `Command::Sow` (the TUI's `G` tool) opens a
`JobKind::Sow` job: a unit walks onto the plot and works `sow_seconds` (6 s), after which it is
**growing** for `grow_seconds` (240 s, scaled by `crop_growth` below). Once that runs out it is
**ripe**, and (like a standing harvest order, #81) a `JobKind::Reap` job queues itself with no
further order from the player: a unit walks over, works `reap_seconds` (6 s), and `harvest_count`
(3) cabbage drops on the ground to be hauled off, counting toward the end game's
`crops_harvested` tally (#233, the "farm crops" sign, #224). The plot is empty again, ready to
sow once more. `CancelSow` (the Cancel tool, `X`, over a tile with a pending Sow) withdraws an
unclaimed or claimed Sow job while the plot is still empty; there's no cancelling a Reap; that
would waste a ripe crop for nothing.

**Which crop.** Cabbage, the one food every nation already knows (wild cabbage needs no tech
either). `[building.farm]`'s `crop` names the item a plot yields, so a second farm-like
building would be a new building with its own `crop`, not a choice the player makes per plot.

**The `crop_growth` hook.** `World::crop_growth_rate(pos, defs)` is the one thing `tick_farms`
reads to know how fast a growing plot's clock counts down: the plot's own `crop_growth` (1.0),
read off the building standing there, times `World::irrigation_rate` (the irrigation channel's
boost in reach, #126, below), times the difficulty preset's season factor
(`World::season_growth_factor`, #311; identity with `seasons` off, so this is exactly
`crop_growth * irrigation_rate` for every save from before #311 landed) — the same season hook
`World::growth_rate` already uses for wild regrowth, so a plot in a cold band grows slower in
winter exactly like a wild plant does, irrigated or not. The first frost also takes an
unharvested ripe crop on a frost-band tile, the plot going back to empty (`World::frost_plot`,
called from the same `apply_frost` that sends a ripe wild plant back to resting). Farm plots are
tended, so they never take strain (#296), the way an orchard's trees don't either. Irrigation's
channel (#126) and Earth's terrace (#136, still to come) are buildings with their own reach, so
the clean way for each to speed up plots nearby is the same pattern `World::tend_rate` already
uses for an orchard's trees: scan buildings of that kind in range and take the best boost, the
same way two orchards in reach of one tree don't stack. Nothing in `tick_farms` needed to change
for that; `crop_growth_rate` was the one line to touch.

**Balance.** A plot nets 3 cabbage every 6 + 240 + 6 = 252 s, about six times a wild cabbage tile
(1 per 480 s), for two short stints of a farmer's time rather than a forage's one: the point of
researching `farming` at all. Cost and work (2 wood, 10 s) are a bed's halved, since a plot is
only marked ground, not a structure; revisit with a real balance pass once irrigation and
terraces give it a ceiling worth tuning against.

**What the player sees.** The TUI draws a plot's own glyph (`≡`/`~`) while empty, and overrides
it with `overlay.plot_growing` (`·`/`,`) or `overlay.plot_ripe` (`✶`/`*`) while growing or ripe,
the same way a sapling overrides a resource's glyph. The inspector's tooltip says "Growing, ripe
in 3m 20s" or "Ripe" for a plot, next to its "Built" line. A placeholder; real pixel art (and the
client's own rendering, since the client takes no new player-facing work per #283) is a
follow-up if the client keeps farm plots at all.

**Saves.** New world state (`farm_plots`) is `#[serde(default)]`; `World::adopt_farm_plots`
registers a farm plot that is already on a loaded map, mirroring `adopt_orchards`. `farm_plot` is
appended at the end of `data/buildings.toml` rather than filed under its shared-tier section,
since `BuildingId` is file position and an existing save's buildings are read back by it: an
earlier insertion would have silently renamed them.

### The irrigation channel (#126)

`irrigation` (Water tier 1, after `farming`) unlocks `irrigation_channel`: a 1x1, walkable
building with no job of its own (unlike a farm plot, it needs nobody's labour once built). The
permanent version of Water the Land's wet state (docs/design/bending.md, "Water the Land"), the
same way a bender's channel outlasts the one-off patch.

**The mechanic.** Every farm plot within the channel's `radius` (Chebyshev, 3 tiles) grows
`growth_boost` (1.5) times as fast, read by `World::crop_growth_rate`
(`world/farm.rs::irrigation_rate`) the same way `World::tend_rate` speeds up a resting tree an
orchard tends: the best boost among channels in reach, or 1.0 with none, so two channels near one
plot don't stack. `IrrigationDef` (`[building.irrigation]`) carries `radius` and `growth_boost`;
`World::open_irrigation_channel` registers a finished one in `World::irrigation_channels` (a
tracked set, not a full map scan, mirroring `orchards`), and `World::adopt_irrigation_channels`
registers one already on a loaded map, mirroring `adopt_orchards`.

**Composing with the season.** `crop_growth_rate` multiplies three independent factors in order:
the plot's own `crop_growth`, `irrigation_rate`, and `World::season_growth_factor` (#311). Order
doesn't matter since all three are plain multipliers, but keeping irrigation and the season
factor as separate multiplications (rather than, say, irrigation only applying outside winter)
means a channel helps exactly as much in a slow winter as it does in high summer: it halves
however long is left, whatever that is. A sim test plants the same plot in a warm band's winter
(`winter_growth` 0.6, so already slower than plain speed) with and without a channel in reach and
checks the irrigated one ripens close to `growth_boost` times faster than the bare one, not just
"faster".

**Reach and cost.** The radius matches Water the Land's own 3x3 patch (one tile out from
centre), so a channel reads as the ability's footprint made permanent; the boost reuses an
orchard's `tend_boost` scale, since both are "a building tends things nearby" at the same order
of magnitude. Cost and work (4 wood, 30 s) match the igloo, Water's other tier-1-ish building,
pending a real balance pass once terraces (#136) give irrigation a ceiling worth tuning against.

**What the player sees.** A placeholder glyph (`≈`/`=`) like every other building here; real art
is a follow-up if the client keeps buildings at all (#283).

**Saves.** New world state (`irrigation_channels`) is `#[serde(default)]`; `irrigation_channel`
is appended at the end of `data/buildings.toml`, same reasoning as `farm_plot`.

## Data schema: `data/research.toml`

```toml
[settings]
passive_knowledge_per_minute = 3.0
study_knowledge_per_minute = 9.0     # per scholar, before the `study` modifier

# File order must be topological: prereqs appear before the techs that need them.
[[tech]]
id = "fire_making"
name = "Fire Making"
description = "Keep a flame alive: cook food and stay warm."
cost = 25                            # knowledge points, > 0
prereqs = []                         # tech ids
# nation omitted = shared tier
[tech.unlocks]
buildings = ["campfire"]
jobs = ["cook"]

[[tech]]
id = "earthen_walls"
name = "Earthen Walls"
description = "Raise walls and flood barriers straight from the ground."
nation = "earth"                     # only Earth Kingdom can see or research this
cost = 120
prereqs = ["earth_shaping", "masonry"]
[tech.unlocks]
buildings = ["earth_wall", "flood_barrier", "terrace"]
abilities = ["raise_wall"]
modifiers = { construction = 1.25 }  # multiplier on the effective modifier
```

And in `data/nations.toml`, per nation:

```toml
starting_techs = ["fire_making"]     # must be shared or this nation's; prereqs must also start
[nation.modifiers]
study = 1.0                          # new modifier: research speed
```

`unlocks` has four optional lists: `buildings`, `jobs`, `abilities` and `modifiers`. Every
unlock kind is optional, and unknown keys are rejected.

## Representation in `fn_sim`

This follows `defs.rs`: TOML structs with string ids are resolved once into index newtypes and
validated at load.

```rust
pub struct TechId(pub u16);

pub struct TechDef {
    pub id: String,
    pub name: String,
    pub description: String,
    pub nation: Option<Nation>,      // None = shared
    pub cost: u32,                   // knowledge points
    pub prereqs: Vec<TechId>,        // all < self (topological order)
    pub unlocks: Unlocks,
}

pub struct Unlocks {
    pub modifiers: ModifierMultipliers,   // Option<f32> per Modifiers field
    // Added by the issue that introduces each def kind (#22, #25..#28):
    pub buildings: Vec<BuildingId>,
    pub jobs: Vec<JobKindId>,
    pub abilities: Vec<AbilityId>,
}

pub struct Research {                // lives in World
    known: Vec<bool>,                // indexed by TechId
    pool_millis: u64,
    queue: Vec<TechId>,
    effective: Modifiers,            // cached, recomputed on completion
}
```

**Validation at load** (`DefsError` on failure, with a unit test for each rule):
- unique ids; every prereq exists and appears **earlier in the file** (this makes cycles
  impossible and makes `TechId` order a valid topological order);
- a shared tech may only require shared techs; a nation tech may require shared techs or techs
  of its own nation;
- `cost > 0`; each multiplier is finite and in `(0, 4]`; `ModifierMultipliers` uses
  `#[serde(deny_unknown_fields)]`, so `contstruction = 1.2` fails to parse;
- every building, job or ability id resolves, and each is unlocked by **at most one** tech;
  abilities may only be unlocked by a tech of the same nation as the ability;
- `starting_techs` are shared or the nation's own, and closed under prereqs.

**Staging.** #18 lands the schema with `modifiers` only. Each other unlock kind is added to
`Unlocks`, with its validation, in the same change that introduces that def kind. Then
`research.toml` never contains ids that can't be checked. Until then, the tree tables above are
the record of what each tech will unlock.

**Per tick.** `pool += rate`; while the queue's front tech costs no more than the pool, subtract
the cost, mark it known, recompute `effective`, and emit a completion event. Choosing a target
replaces the queue with the target plus its unknown prereqs, sorted by `TechId`.

**Locking rule.** An item is locked for a nation if some tech unlocks it and the nation doesn't
know that tech. Items no tech mentions are available from the start, so content that hasn't
been placed in the tree just works.

## Modifier stacking

```
effective.construction = nation.construction × Π (known techs' construction multipliers, in TechId order)
```

- Multiply only, never add. That keeps stacking order-free in principle and easy to explain.
- Iterate in `TechId` order because `f32` multiplication isn't associative. Two different
  research orders that reach the same known set must give *bitwise-identical* modifiers (a test
  in #20).
- Systems read `world.modifiers()`, never `NationDef::modifiers` directly (#20 migrates
  `move_speed`).
- Readable simulation: `modifier_breakdown("construction")` returns
  `[("Earth Kingdom", 1.5), ("Carpentry", 1.1), ("Masonry", 1.15), ("Earthen Walls", 1.25)]`
  for hover tooltips.
- A new modifier field (e.g. `study` now, `food_value` with #3) is added to `Modifiers`,
  `nations.toml` and `ModifierMultipliers` together.

## How it fits the other systems

- **Jobs (#4).** Study is a job kind that adds `study` to the pool, and other jobs gain
  research-unlocked kinds (cook, smith, healer). Work speed already multiplies by a modifier, and
  techs raise those modifiers through #20.
- **Needs (#3).** Techs help needs *indirectly*, through what they unlock: huts (sleep),
  cooking (hunger), pots (thirst), herbalism and healing (health). Needs also make research
  costly: hungry scholars leave their desks. No tech directly changes need rates for now.
- **Player commands (#5).** Research is chosen with `Command::SetResearch { tech }` through
  `World::apply` (#19 added the logic, #5 the command enum), so research choices replay
  deterministically. Locked items are rejected by the sim with a reason ("Requires Pottery")
  that the client shows (#22).
- **Bending (#6, see [bending.md](bending.md)).** Each nation's signature ability from #6 is available **from the start**,
  not researched, so nations play differently from minute one. Branch techs unlock the
  *second and later* abilities (seismic sense, metalbend, healing water, air scooter, inner
  fire) and raise the modifiers those abilities scale with. All ability code stays in the one
  obvious place #6 defines. Research only toggles availability.

## Client: minimum UI (#23)

- **HUD line:** current target, a progress bar (pool / cost), and knowledge per minute.
- **Research screen** (`R`), plain `bevy_ui` like the existing panels: the shared tier plus the
  player's own branch, in columns by depth. Each tech shows name, cost, state
  (known / available / queued / needs prereqs) and its unlocks. Clicking a tech sets the target.
- **Completion toast.** A graphical node-and-edge tree, icons and modifier tooltips come later.

## Later: neighbors and cultural exchange

*The long-term plan for research beyond this tree is [research-horizons.md](research-horizons.md)
(accepted 2026-11-10). Exchange is designed in [horizons-phase-6.md](horizons-phase-6.md) (#670).*

The sketch that stood here had neighbors teach their *non-bending* branch techs through a
per-tech `teachable` flag. Phase 6 builds it with residents in place of neighbors: after the
gate, people of another nation who live in the settlement teach its `teachable` techs at double
cost, and six fusion techs need residents of two nations. Bending never crosses nations. The
neighbors themselves arrived as caravans (pacing.md), which trade goods and points, not techs.

## Owner decisions (2026-10-22)

1. **Starting without fire: accepted, on one condition.** Nobody may starve because their
   nation lacks fire. Raw food (fish, fruit, foraged plants) must fully sustain a starting
   settlement. Cooking is a bonus (more hunger filled per item), not a requirement. This is a
   hard requirement on the needs work (#3).
2. **Cross-nation learning: deferred.** Don't design or build `teachable` techs yet. The
   neighbors section above stays a sketch. *Reopened for the post-gate game (owner, 2026-11-10):
   see [research-horizons.md](research-horizons.md), "Exchange".*
3. **Era ceiling: Korra-era is in tone.** Steam engines and lightning power stations stay.
4. **Scrolls: knowledge points first.** #24 implements points only. Keep the data shape open
   (e.g. an optional `reveals` field later), so scrolls can unlock hidden techs without a
   schema break.

5. **Air's starting tech is `orchards`, not `meditation`** (owner's choice). Meditation alone gave
   only a research bonus, while every other nation's start helps it survive. Orchards give the
   nomads renewable food without fire, and meditation becomes a 60-cost Air tech.

6. **Fruit picking is open to every nation; `orchards` unlocks planting only** (2026-10-25, #142).
   Wild `fruit_tree` harvesting isn't gated. The orchard building plants and tends new fruit
   trees: a renewable food source only Air settlements can create ("The orchard", above).

## Follow-up issues

| # | Work | Area | Priority | Blocked by |
|---|---|---|---|---|
| #18 | `research.toml` schema, `study` modifier, `starting_techs`, loading and validation | sim, data | p1 | none |
| #19 | Knowledge pool, queue, passive trickle, completion, `set_research` | sim | p1 | #18 |
| #20 | Effective modifiers from known techs, plus breakdown API | sim, data | p2 | #18, #19 |
| #21 | Study job and study hall | sim, data | p2 | #4, #19 |
| #22 | Gate buildings and jobs on known techs | sim, data | p2 | #4, #19 |
| #23 | Client research screen and HUD progress | client | p2 | #19 |
| #24 | Ancient scrolls grant knowledge | sim, data | p3 | #4, #21 |
| #25–#28 | Content pass: wire Earth / Fire / Water / Air branch unlocks (adds ability unlocks) | data, design | p3 | #6, #22 |
