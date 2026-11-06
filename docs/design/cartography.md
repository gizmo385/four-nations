# Cartography: charting where the scrolls lie

Status: **accepted** (#400). Follow-up work: #402 to #404.

The "Ancient scrolls read" sign (3 scrolls, `data/endeavor.toml`) is the one sign players make
no progress on. #380 fixes how scrolls *read* on screen: the legend, `inspect`, and why a marked
scroll isn't fetched. This doc fixes the other half, which is that there usually aren't three
scrolls a settlement can reach, and nothing but luck finds the ones there are. `cartography`
(250 knowledge, after `writing`) has been in the tree since #17 with a "Later: reveals the region
around the settlement; scrolls shown on the map" note and no effect. This gives it one.

## What the map holds today

Scrolls come from two places (`data/resources.toml`, `data/jobs.toml`):

- **Surface scrolls**: tundra and desert only, `min_spacing = 20`, 1 scroll each. Always visible.
- **Lost ruins** (#197): any open land, `buried = true`, 2 scrolls each when searched. Only Air's
  Glide uncovers them.

Measured with the supply survey (`cargo run --release -p fn_sim --bin balance -- --supply
--seeds 1-12 --radii 40,90 --resources ancient_scroll,lost_ruins`), Small map, default site,
reachable on foot. Radius 90 covers the whole landmass the site is on:

| Seed | Scrolls on the map | Surface scrolls in reach | Ruins in reach (scrolls) |
|---|---|---|---|
| 1 | 3 | 0 | 0 (0) |
| 2 | 5 | 0 | 0 (0) |
| 3 | 6 | 4 | 10 (20) |
| 4 | 4 | 0 | 1 (2) |
| 5 | 3 | 2 | 9 (18) |
| 6 | 1 | 0 | 2 (4) |
| 7 | 3 | 1 | 6 (12) |
| 8 | 4 | 4 | 7 (14) |
| 9 | 4 | 2 | 10 (20) |
| 10 | 6 | 0 | 4 (8) |
| 11 | 5 | 1 | 2 (4) |
| 12 | 5 | 3 | 6 (12) |

So for Fire, Water and Earth the sign is out of reach on 9 of 12 seeds (fewer than 3 surface
scrolls on the settlement's landmass), and no amount of better UI changes that. Counting ruins,
10 of 12 seeds have 3 or more scrolls in reach. The supply is already in the ground. What's
missing is a way for every nation to find it.

Seeds 1 and 2 have nothing in reach at all: their default sites sit on landmasses with no
tundra, no desert and no ruins. That's the map's water and landmass shape, which a separate
mapgen thread is looking at, and it's out of scope here.

## Decision: a Map Room that charts lost ruins, nearest first

`cartography` unlocks a new shared building, the **Map Room**. A settler seated there does
**Chart** work, and each finished chart uncovers the nearest lost ruin the settlement hasn't
found yet. It's the research loop applied to the map: labor at a building, slow and steady,
competing for hands with everything else.

### The building

| Map Room (`data/buildings.toml`) | Value |
|---|---|
| Unlocked by | `cartography` (shared tier: every nation) |
| Cost | 12 wood |
| Work | 40 s (the study hall's) |
| Footprint | 2x2, not passable |
| Seats | 1 |

One seat on purpose: charting is a trickle, and a second scholar doubling it would make the
building's number of seats the real balance knob.

### The Chart job

The Chart job is a copy of Study (`world/study.rs`) with a different payoff:

- **Opening.** A finished Map Room opens one `JobKind::Chart` seat. It's the lowest priority, the
  same as Study, so needs and urgent work pre-empt it. In a midgame where #337 found 6 to 10 of
  14 settlers idle, that's exactly the slack it should soak up.
- **Only while there's something to chart.** Just as Study closes when the research queue is
  empty, a chart seat isn't claimed when no unfound ruin is left in range (below).
- **Progress.** Every tick a settler is inside, the room's chart progress grows by one tick times
  the nation's `study` modifier. `writing` (Chart's prerequisite through `cartography`) already
  raises `study` to 1.25, so no new modifier is needed. Progress lives on the building, not the
  settler, so a scholar who leaves to eat doesn't lose it.
- **A finished chart** (progress reaches `minutes`) picks one target, uncovers it with the same
  `World::uncover` that Seismic Sense and Glide use, resets progress, and raises a notice.

```toml
# data/buildings.toml, on map_room
[building.chart]
finds = ["lost_ruins"]   # buried resources a chart can uncover
minutes = 6.0            # seated work per chart, before the study modifier
range = 60               # tiles, Chebyshev, from the Map Room's anchor
```

`finds` reuses the buried-resource ids that `Effect::Reveal` uses, so a later chart could find
something else without new code. It stays `["lost_ruins"]`: gold seams are Earth's to sense.

### Which ruin a chart finds

Deterministic, with no RNG:

1. Candidates are buried resources in `finds` within `range` of the room whose tile is free to
   uncover (`World::free_to_uncover`: a ruin under a building or stockpile waits).
2. **Reachable ones first**, by walking cost from the room's door (one flood under the
   settlement's current `MoveRules`, so a ford that's been built counts).
3. Then unreachable ones, by Chebyshev distance.
4. Ties break on `Pos` order.

Charting reachable ruins first means the first few charts always pay. Charting the unreachable
ones afterwards, rather than never, tells the player there's more across the water, and #380's
"no settler can reach it" flag explains why nobody goes. That gives fords, Water's canoes (#201)
and air travel something to aim at.

### Notices

A finished chart raises one notice through the existing notice path:

- `Charted lost ruins 34 tiles north-west.`
- `Charted lost ruins 52 tiles east, across water. No settler can reach them.`
- When nothing is left in range: `Every ruin within 60 tiles is charted.` (once), and the seat
  closes.

Keep these plain, with no advice or asides (the owner's 2026-11-06 note on text across the game).

## What the player sees

- **Research screen.** `cartography`'s unlock summary reads "Map Room" automatically once its
  `[tech.unlocks]` lists the building. `crates/ui/src/research.rs` has a test asserting "-" for
  cartography today; that test changes.
- **Endeavor screen, scrolls sign.** #380 picks one of three approaches. This design wants its
  third one, "a found-but-unreached scroll is listed on the Endeavor screen", and extends that list
  with uncovered ruins. Each row: what it is, how many scrolls, distance and direction from the
  settlement, and its state (unmarked, marked, no settler can reach). Mockup (text, not yet a real
  frame):

  ```
  Ancient scrolls read                 1 / 3
    Lost ruins (2)     34 NW   marked
    Ancient scroll     58 E    no settler can reach
    Lost ruins (2)     52 E    no settler can reach
  ```

  Before `cartography` is known, the sign's hint (`data/endeavor.toml`) becomes
  `Bring scrolls to a study hall. Cartography charts ruins.` That's in-game knowledge (#313): the
  player can read the tech in the research screen.
- **Inspecting a Map Room** shows its chart progress (`Charting: 40%`), or `Nothing left to
  chart` once the job closes.

## Nations

- **Air** still finds ruins best. Glide uncovers every ruin within 14 tiles at once, from
  `gliders`, which costs 60 after `carpentry` against `cartography`'s 250 after `writing`. A Map Room
  finds one ruin every few minutes. Air can build one too, to chart past where its gliders reach.
- **Earth, Fire, Water** get their first reliable source of scrolls, through the same labor
  trade-off as research.
- Nothing here asks which nation is playing. Everything that varies is in data.

## Alternatives considered

- **Cartography reveals everything in a radius the moment it's researched.** The cheapest to build,
  but it's a one-off pop with no labor cost and nothing to do in the midgame, which the owner
  flagged as dragging. The Map Room's slow drip also spaces the scroll trips out.
- **A non-bending "Survey" designation** (a settler walks to a tile and reveals ruins around it,
  like a Glide without chi). It gives the player more control, but asks them to guess where to
  survey, which is the original problem.
- **A new buried find just for charts** (a "forgotten cache" with 1 scroll), so ruins stay Air's
  alone. It keeps Air's niche cleaner but needs a new resource, art and harvest. It's offered as
  question 1 below.
- **Fog of war** with cartography lifting it. Far bigger, and #133 already ruled it out.

## Saves and determinism

- Chart progress is a per-building field, skipped in a save while zero, so older saves load and
  re-save unchanged.
- Uncovering reuses `World::buried`, which saves already carry.
- No randomness: target choice is the ordered rule above. Mapgen is unchanged, so no golden files
  move.

## Measuring it

The economy opening (`fn_sim::autoplay::Economy`, #352) researches the whole tree in file order,
so it reaches `cartography`. It should also raise a Map Room and recover what's charted, and the
balance harness should report the minute the scrolls sign is met. Target (decision 3): on Normal,
Small, seeds 1-8, the sign lands in year 2 to 3 for every nation wherever the survey above shows
3 or more scrolls in reach. Tune `minutes` and `range`, not the ruins' density.

## Owner decisions (2026-11-06)

1. **Should every nation be able to find lost ruins, or only Air?** *Decided by the owner
   2026-11-06:* every nation, through the Map Room. Air keeps a large head start (a Glide finds
   every ruin nearby at once, from a much cheaper tech).
2. **One shared name, or a name per nation like the study hall?** *Decided:* one shared name
   ("Map Room") for now, with nation names and art later alongside #357's building pass.
3. **When should the scrolls sign land?** *Decided:* year 2 to 3 on Normal, which is after
   `cartography` arrives (around year 2) plus a couple of charts and the walks out to them.
4. **Should charts also mark surface scrolls on the map?** *Decided:* no. They're already
   visible, and #380's list on the Endeavor screen covers "where are they". Charts only uncover
   what's hidden.

## Follow-up issues

Filed with #400 (none marked `ready`; triage decides):

- #402 Sim and data: Map Room and the Chart job.
- #403 TUI: charted ruins on the Endeavor scrolls sign, Map Room inspect, and the new hint
  (blocked by #402 and #380).
- #404 Balance: the economy opening charts and recovers scrolls, the harness reports the scrolls
  sign, then tune `minutes` and `range` (blocked by #402).
