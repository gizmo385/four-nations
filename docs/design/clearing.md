# Clearing ground: uproot plants, build over what's in the way

Doc status: current

Status: **accepted** (#411, from the owner's 2026-11-06 playtest). The owner took every
recommendation; see "Owner decisions" at the end. #424, #425 and #426 have landed; #427 is open. Companion issues: #342 (deconstruct finished buildings) and #335 (rectangle cancel
keeps stockpiles and sites).

## The problem

The owner: "the inability to clear out existing structures or remove things like fruits from the
ground really hampered building space."

`World::build_blocker` refuses any footprint tile with a `tile.resource` on it ("something is in
the way"). There are three ways that hurts:

- **Renewable plants can't be removed at all.** A harvest with `regrow` (wild cabbage, fruit
  trees, every orchard tree) leaves the plant standing. Wild fruit trees spawn with
  `clustering = 0.9` on plains and forest, the same open ground a settlement builds on. An orchard
  keeps up to `max_trees` trees within `radius = 4` and replants any that go missing.
- **Non-renewables take a detour.** To build where an oak stands, the player finds the tile,
  designates a harvest, waits for it, and then comes back to place the building.
- **The refusal doesn't say how to fix it.** Today the Cursor section reads:

```
~~~~~~~=."""T,,,,,,,,,,,,Y,c,TTT,@@~~~~~~~f.~.,,,,T,,,,TTTTTFFF"FF"F| Elevation 161                |
~~~~~~~~..",TT,,,,,T,T,T,,Y,,,HH,Y@~~~~~~~~~..,,,,,,,TT,,T,,F"FFFF"F|                              |
~~~~~~~~~.",,T,TT,,,,,T,,T,,,,HH,,@~~~~~~~~~~..,TT,T,TTT,TT,,T,Y"FFF|                              |
```

with `Can't build here: an oak tr~` above it (seed 7, Earth, `--keys 'u s s <enter> <left*4>
<down*1>'`, the Scroll Library's 2x2 ghost over one oak; its door tile below the ghost holds a
second oak).

## The idea

Two features and one vocabulary rule:

1. **Clear**: a designation that removes whatever resource is on a tile for good. It uses the
   resource's own harvest when that already removes it, and a small `[harvest.clear]` table when
   the harvest would leave the plant standing.
2. **Build over**: placing a building over clearable resources is allowed. The site queues Clear
   jobs on those tiles and its door tile, and construction starts once they're gone. RimWorld and
   Dwarf Fortress both work this way.
3. **One removal vocabulary**: `X` takes back *orders* and `E` removes *things*. `E` cycles
   between ground and buildings the way `U` and `B` cycle.

## 1. The Clear designation

### Rules

- `Command::DesignateClear { pos }` and `Command::CancelClear { pos }`, with a new
  `JobKind::Clear { pos }`. It's a job like a harvest: the nearest idle unit claims it, works it
  from beside the tile, and the yield drops beside it through `place_ground`.
- **Non-renewable resource** (oak, fir, boulder, stone, gold, scroll, ruins): the Clear job *is*
  its harvest, with the same `work`, `count`/`tunnel_count`, `modifier` and `risk`. Chopping a tree
  already clears it. Clear exists for these only so one tool and one build-over rule cover every
  tile.
- **Renewable resource with `[harvest.clear]`** (cabbage, fruit tree): the job takes
  `clear.work` seconds and drops `clear.count` of `clear.item` (if any). **If the plant is ripe**
  (not resting), it also drops its ordinary harvest yield, so uprooting a laden tree doesn't throw
  the fruit away. Then the resource is removed.
- **Renewable resource without `[harvest.clear]`** (fish spots): refused with "can't be cleared".
  Fish spots are on water, where nobody builds, and clearing them would only be a way to ruin a
  Fishing Dock.
- **Research gate.** A non-renewable keeps its harvest's tech gate: a boulder can't be cleared
  before the nation can quarry. Renewables have no gate, since anyone can pull up a cabbage.
- **Resting doesn't block a clear.** A picked fruit tree can be uprooted at once. A harvest job on
  a resting tree waits for it to ripen, but a clear job is claimable straight away.
- **A clear replaces other orders on its tile.** Designating Clear withdraws the tile's Harvest
  job and standing order (#81). A unit halfway through the harvest drops it, as on `CancelHarvest`.
- **Removal cleans up after itself.** When the job finishes, the tile's `resting`, `saplings` and
  strain entries go, along with any standing order. `map_changed()` runs. The orchard needs no new
  code: `tick_orchards` already drops a tree from `Orchard::trees` once its tile no longer holds
  the planted resource (see section 3).
- Hungry units never forage a Clear job, and the harvest tally counts everything a clear drops.
- Determinism: the only roll is `risk`, drawn from the existing `harm.injury` stream exactly as a
  harvest's is. `JobKind::Clear` and the two commands are new serde variants, so old saves and
  command logs load without a `SAVE_VERSION` bump.

### Data

The table goes under the harvest it belongs to, so validation stays local: `clear` is allowed only
on a harvest with `regrow`, `work` must be above 0, `item` must be a known item and `count` needs
an `item`.

```toml
[[harvest]]
resource = "fruit_tree"
verb = "Pick"
item = "fruit"
count = 2
work = 4.0
regrow = 360.0
strain = 1.0

# Uprooting (#411): a little wood, plus the fruit if it's ripe.
[harvest.clear]
work = 6.0
item = "wood"
count = 2
risk = 0.01

[[harvest]]
resource = "cabbage"
# ...
[harvest.clear]
work = 1.0
```

The numbers: a fruit tree takes 6 s to uproot, between picking (4 s) and felling an oak (8 s). It
yields half an oak's wood because it's a smaller tree, and the risk matches chopping. A cabbage
takes 1 s and yields nothing but its crop when ripe. The orchard can't become a wood farm: one
sapling every 90 s (`plant_seconds`) for 2 wood is far worse than chopping wild oak, and #339
already finds wood too plentiful.

## 2. Building over obstacles

### Rules

- `check_build` stops refusing **clearable** resources. A footprint tile, or the door tile of a
  building that has one, whose resource can be cleared is now fine. `Blocker::Resource` remains,
  but it only means "can't be cleared" (a fish spot, or a boulder before quarrying is known), and
  its text names the resource and the reason.
- A new query, `World::to_clear(anchor, building, defs) -> Vec<Pos>`, lists those tiles
  row-major with the door last. The TUI's ghost and the autoplay both read it.
- `designate_build` stays all-or-nothing. On success it also issues a Clear on each `to_clear`
  tile that doesn't already have one. Those jobs go through the same path as
  `Command::DesignateClear`, but the command log records only the `DesignateBuild`. The clears
  follow from it deterministically, so replay reproduces them.
- **Construction waits.** A Build job can't be claimed while any of its `to_clear` tiles still
  holds a resource (`site_ready`, the same pattern as `bend_ready`). Anyone may claim the Clear
  jobs. Once the last one finishes, the next idle unit claims the Build job as usual. Hauling
  waits too: deliveries starting before the ground is clear would add a second wait state for
  little gain.
- **Cancelling the site** also withdraws the Clear jobs on its `to_clear` tiles that no unit has
  started (progress 0). One the player marked by hand counts too: the player can mark it again,
  and the rule stays simple.
- **Ground stacks** need nothing new. They don't block placement today, and `raise` already moves
  them off a solid footprint when it finishes.
- **Orchards** already skip a site's footprint. They also need to treat a site like a finished
  building in `can_plant`'s spacing check. Today an orchard can plant directly beside a pending
  site, including on its door tile, which would block the door the moment it was cleared.
- **The autoplay doesn't change.** `nearest_build_site` keeps choosing footprints where
  `to_clear` is empty, so the balance harness's openings and their numbers stay the same. Letting
  it build over trees is a separate tuning question.

### TUI (proposed)

The ghost draws footprint tiles that will be cleared in a new `clear` colour (data/tui.toml), so
the player sees what goes before pressing Enter. The Cursor section stops refusing and says what
will happen (mockup edited from the frame above, panel rows only):

```
~~~...""""=",,Y,,,,,,,,,,,,,,,,TT.~~~~~~~~~~.~.,,TTTTT.~.TFFF=="F.-.| Clears 2 first: oak trees    |
~~~~~..""=c",T,,c,,,,,,,,,,,T,TT,.@~~~~~~~~....,,,T,TT...TFFFFFf==-.| Land                         |
~~~~~~..=""F,T,,,,,Y,,,,,,,T,T,T,,@~~~~~~~~~..T,TTT,,,,T,T,FFFFF"..=| Plains (90, 65)              |
~~~~~~~=."""T,,,,,,,,,,,,Y,c,TTT,@@~~~~~~~f.~.,,,,T,,,,TTTTTFFF"FF"F| Elevation 161                |
~~~~~~~~..",TT,,,,,T,T,T,,Y,,,HH,Y@~~~~~~~~~..,,,,,,,TT,,T,,F"FFFF"F|                              |
~~~~~~~~~.",,T,TT,,,,,T,,T,,,,HH,,@~~~~~~~~~~..,TT,T,TTT,TT,,T,Y"FFF|                              |
```

Placing it gives the notice `Scroll Library placed. 2 to clear first.` A site that is waiting
shows `Waiting: 2 to clear` in inspect, beside the delivery line.

## 3. Orchards and planned space

- **A tree cleared on purpose leaves the orchard's count.** `tick_orchards` already runs
  `o.trees.retain(|&t| map[t].resource == Some(def.plants))`, so this works today and only needs a
  test. The orchard then replants on some other free tile within its radius, after
  `plant_seconds`.
- **No "keep clear" zone.** Planned space is a building site, and orchards already avoid sites
  (plus the spacing fix above). To keep land for a building, place the building: build-over makes
  that one step. To shrink a grove, deconstruct the orchard (#342). A zone would be a fourth kind
  of rectangle to explain, for a problem build-over already solves. The owner agreed (#459).
- A pending Clear mark needs no special case. The tile still holds a resource, so `can_plant`
  already skips it, and once it's cleared the mark is gone.

## 4. Raised and bent terrain: out of scope

Fords and tunnels can't be cleared.

- **Mapgen fords and Raise Earth fords look identical on the map** (`Terrain::Ford`), and mapgen
  fords must stay because they keep landmasses connected. Telling them apart would take new saved
  state per tile, and removing one would need a strand check like `would_strand`. That's real cost
  for a problem nobody has hit.
- **Draw Water's fords already revert by themselves** (`revert_minutes`).
- **Tunnels are inside mountains**, which were never building land, and a tunnel only exists
  because someone bored it.

Reopen this if a playtest wants a ford gone.

## 5. One removal vocabulary

| Key | Tool | What it does | Never touches |
|---|---|---|---|
| `H` | Harvest | Take the yield. Renewables stay standing. `O` sets a standing order. | |
| `X` | Remove | Take away everything the player made: harvest, standing, sow, bend and clear marks, build sites, stockpile tiles and finished buildings (#953, docs/design/tool-pickers.md; it replaced Cancel and this table's buildings mode). | Landmarks, units, ground items, terrain |
| `E` | Clear | Remove a **thing of the land**: plants, trees, rocks (this doc). Ground only; `E` pressed again changes nothing. | Orders and buildings |

- `E` is free and sits beside WASD. Superseded by #953: it no longer has a buildings mode (a
  press-again toggle was the thing the owner could not find), so a Remove drag over the
  settlement never fells its trees and a Clear drag never takes down a house.
- Stockpile removal and buildings are the Remove tool's (`X`), whose preview lists what goes.
- The notice follows the other tools: `clear: 6 marked, 2 skipped (can't be cleared)`.
- The agent's `act` gets `designate {tool: clear}` (`e`) and `designate {tool: remove}` (`x`,
  #953; it replaced `cancel` and `deconstruct`).
- Marks draw in the new `clear` colour, the same way harvest marks use `harvest`.

The owner chose this one-key layout (#458), so #342's TUI work is `E`'s second mode, not a new
key.

## Not chosen

- **Refuse and point at the tiles.** It's safer, but it keeps the loop the owner complained about:
  find, designate, wait, come back.
- **A new top-level `[[clear]]` table.** It would split one resource's numbers across two tables
  and need its own "every renewable on land has an entry" check. A sub-table keeps them together.
- **Clear as a Harvest flag** (`JobKind::Harvest { pos, uproot: true }`). Every harvest query
  (`harvest_designations`, forage, standing orders, #336's filter) would have to learn to skip
  it. A separate kind keeps those queries untouched, and its run code can still share
  `run_harvest`'s walk-and-work.
- **Letting fords and tunnels be cleared.** See section 4.

## Follow-up issues

- **#424** (done) Sim and data: the Clear designation (`DesignateClear`/`CancelClear`,
  `JobKind::Clear`, `[harvest.clear]`).
- **#425** (done) Sim: build over clearable resources (`to_clear`, `site_ready`, cancel withdraws
  unstarted clears, orchard spacing around sites, autoplay unchanged). Blocked by #424.
- **#426** (done) TUI and agent: the Clear tool on `E` (ground mode), the `clear` colour, notices
  and `designate {tool: clear}`. Blocked by #424.
- **#427** (open) TUI: the build ghost shows tiles to clear, plus the Cursor and inspect lines.
  Blocked by #425.

None of them is blocked by #342 or #335. Clearing touches neither buildings nor stockpiles.
Following #458, #342's TUI work was `E`'s second mode (buildings); #953 moved it to the Remove
tool (`X`), the agent step `designate {tool: remove}`.

## Owner decisions

All four were answered on 2026-11-06, each with the recommended option.

1. **Build-over is automatic** (#457). Placing a building over trees or plants queues their
   clearing, and the ghost shows which tiles go.
2. **One removal key** (#458). `E` Clear has two modes, ground then buildings on a second press,
   and `X` takes back orders only.
3. **No "keep clear" zone** (#459). Placing the building (build-over) or deconstructing the
   orchard covers it.
4. **Uprooting a food plant gives its ripe crop, plus 2 wood for a fruit tree** (#460). The
   numbers live in data/jobs.toml.
