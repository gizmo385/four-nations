# Materials balance: stone, ore, food and wood against the Harmony Gate

Status: **accepted** (#339, which folds in #272). The owner answered all four questions on
2026-11-05; the answers are recorded at the end under "Owner decisions". The follow-up issues are
listed under "Follow-up issues", and #355 measured the result on 2026-11-07 ("What the harness
measured", after section 7).

Two agent playtests on 2026-11-05 (seed 7 Earth and seed 42 Fire, Normal, about two in-game
years each) both stalled at 3 of 9 Harmony Gate signs. The owner's own play matches. Stone and
ore were the wall, food and wood piled up with nothing to spend them on, newcomers came about
once a season, and neither colony was ever in danger. This doc works out where those numbers come
from in `data/*.toml` and proposes data changes, two small sim changes and a way to measure the
result.

## Decisions in brief

1. **Ore is a correctness bug, not a taste call.** A Small map has about 21 surface gold veins,
   each worth 1 ore. Air, Water and Fire can't reach the 40-ore sign on any seed even by
   stripping the whole map, and the gate itself needs 32 ore in stores. The fix is denser veins
   that yield 3 ore each. Earth keeps its lead through tunnels and seams.
2. **Stone: richer outcrops plus foothill boulders.** An outcrop yields 5 stone instead of 3
   (8 instead of 5 through a tunnel). A new **boulder** resource sits on open land beside
   mountains, so every nation can quarry stone without boring into rock. The 200-stone sign stays
   as it is, because it equals the gate's own stone bill.
3. **Production stops at a stock limit.** The Brick Kiln destroys stone, and the smelter destroys
   ore, for outputs nothing uses. Each recipe gets a `stock_limit` in data, so the building idles
   once that many of its output are in stores. Whether the player also gets a pause key is an owner
   call (question 1).
4. **Food: player-built production gets leaner, and growth eats the surplus.** Farm plots and
   orchards feed about twice as many people as they should. Growth comes every 10 minutes
   instead of 15, and each newcomer takes 15 food. A food glut turns into people, and people
   turn food back into a constraint. How much risk Normal should carry is question 2.
5. **The building-kinds sign becomes "one of every building your people can raise"**, a target
   computed per nation the way the branch sign is. It tracks new content without anyone
   retuning numbers (question 3, and the #272 question).
6. **Measure it headlessly.** A supply survey adds a column to the `balance` binary, and an
   economy opening plays toward the signs, so every retune is checked by a harness instead of a
   two-hour LLM playtest.

## The numbers today

### What the gate asks for

| | Stone | Gold ore | Wood |
|---|---|---|---|
| Sign | 200 in stores at once | 40 mined (lifetime) | — |
| Buildings for the kinds sign | Fire 72, Air 18, Earth 5+, Water 0 | — | about 60–100 |
| The gate, after Ready | 180 (foundation 80, pillars 100) | 32 (Fire pillar 12, arch 20) | 110 |
| **Mined to finish** | **about 215–290** | **at least 40, about 48 for Fire** | **about 250–300** |

The gate's stone bill is paid out of the 200 in stores, so the sign and the bill are the same
stone. Ore works the same way, provided nothing eats the ore first.

### What the map holds

These counts come from a survey of seeds 1–12 and 42 on Small (192x128). Each map was generated
with `mapgen::generate`, settled at `Survey::default_site`, and flooded from the start over
walkable ground. "Reachable" means the deposit touches a tile the settlers can walk to. The
script is a throwaway; the supply-survey follow-up makes it a real tool.

| Per map (median, range) | Whole map | Reachable, ≤20 tiles | Reachable, ≤40 tiles | Seed 7 (≤20 / ≤40) | Seed 42 |
|---|---|---|---|---|---|
| Stone outcrops (3 stone each) | 232 (200–300) | 27 (5–60) | 71 (50–122) | 6 / 50 | 17 / 102 |
| Gold veins (1 ore each) | 21 (15–26) | 2 (0–6) | 8 (2–13) | 1 / 7 | 2 / 11 |
| Gold seams (buried, Earth only, 3 ore) | 16 (12–21) | — | 8 within 2 bores | 8 | 12 |

- **Ore:** a non-Earth settlement that strips every reachable vein *on the whole map* gets
  9–20 ore (median 13). The sign wants 40, so it is unreachable on every seed we surveyed. Earth
  reaches about 60 through tunnels (2 per vein) and seams (5 each), but only after Seismic Sense
  (455 knowledge into its branch) and a lot of chi.
- **Stone:** at the median, reaching 200 stone means quarrying every outcrop within 40 tiles. On
  seed 7 the 50 outcrops in that radius add up to 150 stone. The playtests' agents worked the
  rock they could see near camp and ran out.

### What a worker earns

These figures are approximate, at 0.6 s per tile for move_speed 1.0
(`BASE_TICKS_PER_STEP` 6 at 10 ticks a second), with about 30% lost to eating and sleeping.

- **One quarrier, outcrops about 25 tiles out:** about 40 s per outcrop (walk there and back,
  then 12 s ÷ earthworks of work). That's about 45 stone a season at 3 per outcrop. **Labour is
  not the constraint; supply in reach is.**
- **The Brick Kiln** runs whenever stone is in stores. At Fire's late heating (2.5 × 1.2 × 1.25)
  a batch takes about 4 s of work and eats 3 stone, so the kiln **burns stone as fast as its
  crafter can carry it over**, far faster than anyone quarries it, into bricks that nothing uses. That alone explains Fire's 0/200. **The smelter** does the same to ore
  (2 ore a batch, about 5 s), and its metal only feeds the forge's tools, which nothing uses either.
- **Food:** a settler eats about 2 raw items a season (4 hunger a minute against 30–45 per
  item). One farm plot gives 3 cabbage every 252 s, about **11 a season, enough for 5–7
  people**. A full orchard gives about 45 a season, enough for over 20. The difficulty work
  (#294–#301) tuned *wild* food and strain to its targets, and the harness's standard opening
  enters winter 2 at about 3 food a head. Farms and orchards bypass all of that, which is how the
  playtests reached 43 a head.
- **Growth:** `arrival_minutes = 15` is one season. Going from 6 people to 16 takes at least 10
  seasons (2.5 years), however much food and how many beds are spare. That matches the playtests.
- **Wood:** 4 per tree in 8 s, against total demand of about 300. The glut comes from idle
  settlers with nothing better to do, so it shrinks once stone, ore and farms need hands.

## The proposal

### 1. Ore: denser, richer veins (data)

| `data/` | Today | Proposed |
|---|---|---|
| `resources.toml` gold_vein `chance` / `min_spacing` | 0.05 / 6 | 0.08 / 4 |
| `jobs.toml` gold_vein `count` / `tunnel_count` | 1 / 2 | 3 / 5 |
| gold_seam | 3 / 5 | unchanged |

Measured with the trial data on the same 13 seeds, veins rise to about 38 a map, and the median
reachable within 40 tiles is 14 (10–17), which is **about 42 ore** (seed 7: 36, seed 42: 45).
Within 20 tiles there are still only about 4 veins, so ore stays a find that pulls people
outward, as difficulty.md intends ("Rare things far away"). It just isn't impossible any more.
Earth mines a tunnelled vein for 5 and keeps the seams, so it reaches the sign much sooner, and
that is its pillar-1 edge. One miner takes about 70 s per 3 ore at 35 tiles out, so 40 ore is
roughly one season of a miner's time once the veins have been found.

The vein density changes map generation, so the golden files need re-blessing. Resources roll
from their own streams (#225), so nothing else moves.

**Seeds 2 and 6 are acceptable hard sites, not a density bug (#396).** `balance --supply --radii
40 --resources stone,boulder,gold_vein,gold_seam` reports only 5 ore in reach on seed 2 and 17 on
seed 6, well under the 40-ore sign, against 38–72 on the other six. Tracing *why* with
`Regions::compute` (not just the box the survey counts in, the whole map): the default site on
seed 2 lands on a 401-tile pocket of land whose region borders only **6 mountain tiles in
total**, ever, on the whole map, none of them a vein or a seam. Seed 6's site borders 51 mountain
tiles, which is enough rock to eventually clear the sign, but a direct sweep of `gold_vein`
`chance` (`min_spacing` unchanged at 2) only takes seed 6 from 17 ore today to 29 at `chance =
0.30` and 53 at `chance = 0.60`, over 4x today's value; at that density seed 3, already tuned to
the sign at 69, jumps to 198. Even saturating every eligible tile (`chance = 1.0`,
`min_spacing = 1`, no spacing at all) only takes seed 2 to 18 ore: its 6 bordering mountain tiles
are a hard ceiling no density change can lift. Retuning the whole curve to rescue two outlier
seeds would overshoot the six seeds that already clear the sign at today's density. These two
sites are genuinely short on ore, not under-sampled by the survey (#445 is a different,
later-game effect and isn't the cause here): a settler on seed 2 is on a small landmass the
mountain range never touches, and seed 6's landmass touches only the edge of it.

This is the `--supply` tool's own fixed site (`Survey::default_site`, the box the autoplay
economy opening also starts from); a player founding by hand can move the New Game cursor to a
better-connected site before founding, the same way they would avoid any other hard start. What
was missing was the warning: `Survey::count_group`, which feeds the embark outlook's resource
levels, counted a group's tiles anywhere in the 21x21 report box, even across water or a sealed
border the site's own region never touches, the same kind of overcount `deposit_supply` already
guards against with its "touches a walkable tile of the region" rule (#348). Seed 2's site read
Easy (stone and gold both looked present, at counts that were never actually reachable); seed 6
read Fair. `count_group` now applies the same region-touching filter, so seed 2 reads Fair
("Stone is scarce; Food is scarce") and seed 6 reads **Hard** ("no food nearby"). Gold itself stays
`outlook_weight = 0` (`docs/design/embark.md`, "nice to have; never makes a site harder" is still
the right call: it's a find, not a baseline resource), but the stone and food groups it shares a
landmass with now correctly flag these two sites as the harder starts they are. The fix also
corrected the seed 7 golden report (food was 6/some, reachably 5/scarce) and the matching TUI
embark goldens; both re-blessed with `FN_BLESS=1 cargo test -p fn_sim` / `-p fn_tui`, no
`mapgen::generate` output changed.

### 2. Stone: richer outcrops and foothill boulders (data)

| `data/` | Today | Proposed |
|---|---|---|
| `jobs.toml` stone `count` / `tunnel_count` | 3 / 5 | 5 / 8 |
| `resources.toml` new `boulder` | — | land `beside = ["mountain"]`, chance 0.2, clustering 0.5, listed after the trees |
| `jobs.toml` boulder harvest | — | Quarry, 3 stone, 10 s, `modifier = "earthworks"`, no tunnel bonus |
| `research.toml` stoneworking `jobs` | `["stone"]` | `["stone", "boulder"]` |
| `embark.toml` stone group | `["stone"]` | `["stone", "boulder"]` |
| `tui.toml` | — | `[resource.boulder]`, ASCII `0`, in the stone colour |

The trial counted boulders at a median of 30 within 20 tiles and 72 within 40. Combined with the
richer outcrops, the stone reachable within 20 tiles goes from about 81 to **about 225 at the
median** (seed 7: 18 to 72), and within 40 tiles from about 210 to about 570. One quarrier then
makes about 70 stone a season, so the sign plus its buildings takes two people about two seasons.
That's a year-one goal for a settlement that commits to it, not a wall.

Boulders block movement like trees, which gives foothill sites some texture. They don't strain or
regrow: stone is still a finite deposit, just a less stingy one. A boulder is an ordinary harvest
of an ordinary resource, so this needs no sim code. It does change map generation, so the
re-bless goes in the same PR as section 1.

### 3. Production stops at a stock limit (small sim change)

```toml
[building.recipe]          # data/buildings.toml, brick_kiln
output = "brick"
output_count = 4
work = 15.0
modifier = "heating"
input = [{ item = "stone", count = 3 }, { item = "wood", count = 1 }]   # wood: see section 5
stock_limit = 20           # starts no new batch while 20 or more of `output` are in stores
```

- **Rule:** a crafter starts a batch only while `stored_total(output) < stock_limit`. A batch
  already started finishes. Zero means no limit. That is deterministic, adds no state, and the
  loader checks the number.
- **Starting values:** kiln 20 brick (15 stone, once), smelter 4 metal (8 ore), forge 4 tools.
  Until something consumes bricks or metal, those are the only losses.
- **Readable simulation:** the building's cursor panel gains a recipe line, a status line and,
  if the owner wants it, a pause key (question 1). Today the panel shows nothing about a recipe,
  and a crafter's task reads "Crafting at (x, y)".

```
+- CURSOR ---------- (96, 61) -+   +- CURSOR ---------- (96, 61) -+   +- CURSOR ---------- (96, 61) -+
| Brick Kiln                   |   | Brick Kiln                   |   | Brick Kiln                   |
| 3 stone, 1 wood -> 4 brick   |   | 3 stone, 1 wood -> 4 brick   |   | 3 stone, 1 wood -> 4 brick   |
| Working: 14/20 brick stored  |   | Idle: 20/20 brick stored     |   | Paused: K resumes            |
| K pause                      |   | K pause                      |   |                              |
```

*(Hand-drawn in the side panel's 30-column style; the three states are working, at the limit and
paused.)* The pause, if the owner wants it, is `Command::SetProductionPaused { anchor, paused }`
on the building's anchor, stored as a `serde(default)` flag, with no `SAVE_VERSION` bump. `K` is
free in both the game keys and the key help (and isn't an F-key, so it works on WSL).

### 4. Food and growth: leaner production, faster growth (data)

| `data/` | Today | Proposed | Effect |
|---|---|---|---|
| `buildings.toml` farm_plot `harvest_count` / `grow_seconds` | 3 / 240 | 2 / 300 | 11 → 6 a season (3–4 people in summer, about 1 in a temperate winter) |
| orchard `max_trees` / `tend_boost` | 6 / 1.5 | 4 / 1.25 | 45 → 25 a season. Still Air's easy food (pillar 1), just not a bottomless one |
| `needs.toml` growth `arrival_minutes` | 15 | 10 | 6 → 16 people in about 7 seasons instead of 10 |
| `difficulty.toml` normal `newcomer_food` | 10 | 15 | The climb to 16 spends about 150 food |

Wild food is left alone, because #301 already tuned it against the harness. A settlement of 16
then needs about five farm plots through the summer and has to store for winter (about 32 eaten
against 9 grown). The food sign (150) becomes a real winter store again instead of a formality.
The Hard preset scales these through its existing knobs. Whether Normal should go further, and
let a settlement that grows past its farms lose someone, is question 2.

### 5. Wood: a smaller yield and a fuel sink (data)

Chopping yields 3 wood instead of 4. The kiln and the smelter take 1 wood per batch as fuel
(this lands with the stock limit in #350, and it's bounded by it). The gate's foundation costs 60 wood instead of 40, and the
arch 90 instead of 60. Wood stays easy. These changes stop it piling into the thousands while
stone and ore now take hands. A real fuel economy (winter warmth burning wood) is a separate
design if the owner ever wants one. It isn't filed here.

### 6. Sign targets (#272)

**Today's reachable building kinds, with the whole tree researched.** The shared kinds are bed,
campfire, study hall and farm plot.

| | Shared | Own branch | Reachable | Shared target 8 |
|---|---|---|---|---|
| Air | 4 | orchard, windmill, wind turbine, bison stable | **8** | just |
| Water | 4 | irrigation channel, igloo | **6** | never |
| Earth | 4 | earth wall, rail | **6** | never |
| Fire | 4 | forge, kiln, smelter, steam engine, lightning station | **9** | yes, with one spare |

(The #272 triage comment's "Water 4, Earth 5" predates farm plots, irrigation and the campfire
being shared. Numbers drift like this every time content lands.) #136 (Earth terraces and flood
barrier), #200 (tide mill) and #125 (healing hut) will each add kinds, so any hand-set number
goes stale again.

**Proposed:** a new sign kind, computed per nation the way `branch_complete` is, which keeps the
sign's id so a save's met state carries over.

```toml
[[sign]]
id = "building_kinds"             # unchanged: saves refer to signs by id
group = "buildings"
name = "One of every building"
kind = "every_building"           # target: every non-endeavor building the nation can unlock
spare = 1                         # optional, default 0: kinds the player may skip
hint = "Raise something new"
```

The target is the number of non-endeavor buildings with no unlocking tech, or one unlocked by a
shared tech or the nation's own, minus `spare`. With `spare = 1` that is Air 7, Water 5, Earth 5
and Fire 8 today, and it rises by itself as #136, #200 and #125 land. The UI's hint names what's
missing ("Not yet raised: Earth Wall, Rail"), which is more useful than "Raise something new".
The fallback, if the owner prefers numbers, is `per_nation` on the existing kind, which works
today with no code.

**Stone and ore targets stay shared at 200 and 40.** They match the gate's own bill (180 stone and
32 ore), which every nation pays. Per-nation values would only move the stall from the sign to
the gate. Population (16), food (150) and scrolls (3) are unchanged. Masters belong to #340 (see
#343 and #344).

```
+-------------------------- * ENDEAVOR * --------------------------++----- * EARTH KINGDOM * ------+
| The Harmony Gate                                    3 of 9 signs || > Running x1           00:00 |
| A great work with the Avatar. When every sign is met, word of    |+- PEOPLE ---------------- 10 -+
| the settlement reaches Aang.                                     || Quarry 2  Mine 1  Farm 2 [N] |
|                                                                  |+- RESEARCH -------------------+
| Stage: not begun                                                 || No target set        3.0/min |
|                                                                  || 0 pts banked                 |
| PEOPLE                                                           |+- ENDEAVOR ------------- 3/9 -+
|   Population                      ######....     10/16           || Next: 6 more people      [G] |
|   Spare beds and food draw newcomers                             |+- CURSOR ---------- (94, 64) -+
|   Everyone has a bed              ##########     10/10           || Tyro                         |
|                                                                  || Earth Kingdom - Idle         |
|                                                                  || Carrying: nothing - Bends: 0 |
| BUILDINGS                                                        || Press Enter to select        |
|   One of every building           ######....       4/6           ||                              |
|   Not yet raised: Earth Wall, Rail                               ||                              |
|                                                                  ||                              |
| KNOWLEDGE                                                        ||                              |
|   Earth Kingdom branch            #####.....       3/6           ||                              |
|   Research your branch                                           ||                              |
|                                                                  ||                              |
| STORES AND HARVESTS                                              ||                              |
|   Food in stores                  ####......    64/150           ||                              |
|   Stockpile food                                                 ||                              |
|   Stone in stores                 #####.....    96/200           ||                              |
|   Quarry outcrops and foothill boulders                          ||                              |
|   Ore mined                       ###.......     12/40           ||                              |
|   Mine gold veins ($); tunnels yield more                        ||                              |
|                                                                  ||                              |
| BENDING                                                          ||                              |
|   Bending masters (25 bends each) ..........       0/3           ||                              |
|   Bend often                                                     ||                              |
+- Arrows W S scroll  PgUp PgDn page  Esc/G close -----------------++- seed 7 ---------------------+
```

*(A mockup built from `fn_tui 7 earth --frame --ascii --keys G --size 100x34`, with the sign rows
edited to show the proposal mid-game. The hints are the new data strings.)*

### 7. Measuring it

- **Supply survey** (`balance --supply`): for each seed, the reachable outcrops, boulders, veins
  and seams within 20 and 40 tiles of the default site, and the stone and ore they're worth,
  computed from the bundled data. This is the trial script above, turned into a tool. It's cheap,
  and it's the check for sections 1 and 2.
- **Economy opening** (`autoplay::Economy`): extends the standard opening. It researches the
  nation's branch, keeps a study hall staffed, tills one farm plot per three people, builds a bed
  ahead of each arrival, raises one of every building as it unlocks, and keeps standing quarry and
  mine orders on the nearest reachable deposits. It reports, per season, stone stored, ore mined,
  food per head, wood, population and signs met. A `--no-farms` variant tills nothing, to test
  question 2.

**Targets** (seeds 1–8, all four nations, Small, Normal, economy opening):

| | Target |
|---|---|
| Stone sign met | by minute 120 (end of year 2) on at least 7 of 8 seeds, every nation |
| Ore sign met | by minute 120 on at least 7 of 8 seeds, every nation, Earth first |
| Food per head entering winter 2 | 3 to 8 (difficulty.md's band, now with farms) |
| Wood at minute 120 | under 300 |
| Population at minute 120 | 14 or more |
| Deaths by minute 180 | 0 (and, if question 2 goes as recommended, at least one by minute 120 with `--no-farms`) |

**Pacing (#272's second check).** The branch takes about 73 minutes of research with two
scholars (1530 points at 21 a minute for Earth), and population is the long pole at about
7 seasons. With these targets every sign lands at about year 2, which is 120 minutes at 1x. Add
the project's hour and the total is about 3 to 3.5 hours: just under endgame.md's 3.5 to 4 hours,
and closer to it once masters (#344) and scrolls are counted. The final follow-up re-runs both
agent playtests and records what they found in endgame.md.

### What the harness measured (#355, 2026-11-07)

`cargo run --release -p fn_sim --bin balance -- --economy --seeds 1-8 --minutes 180 --nations N
--preset normal`, plus the same with `--no-farms`, on `main` at 3127de9 (every follow-up above
merged). Counts are out of 8 seeds. "Ready" here is the latest sign's first-met minute: a lower
bound, since a stores sign can dip again before the rest are met.

| | Target | Earth | Fire | Water | Air |
|---|---|---|---|---|---|
| Stone sign by minute 120 | 7 of 8 | **8** | 6 | 6 | **7** |
| Ore sign by minute 120 | 7 of 8 | 6 | 6 | 5 | 6 |
| Food per head entering winter 2 (minute 105), in 3-8 | 3 to 8 | 3 (median 3.5) | 4 (3.3) | 5 (5.0) | 2 (**11.0**) |
| Wood at minute 120 under 300 | under 300 | 4 (median 253) | 6 (100) | 6 (130) | 6 (92) |
| Population at minute 120, 14 or more | 14+ | 7 (median 16) | 6 (17) | 6 (18) | **8** (18) |
| Deaths by minute 180, with farms | 0 | 0 | 0 | 4 (seed 3) | 0 |
| Deaths by minute 120, `--no-farms` | at least one | 0 | 0 | 0 | 0 |
| Every sign met by minute 180 (Ready) | | 1 (145) | 1 (129) | 1 (159) | 4 (108-123) |

Where the misses come from, and where each is tracked:

- **Ore on seeds 2 and 6, every nation** (and Water on seed 7, #397). Seed 2 has no stone
  outcrop or gold vein within 40 tiles of the default site, and seed 6 has 9 ore in veins.
  #396 closed, but the sign is still missed there. #440's caravans are the planned answer
  (its targets ask for ore by minute 150 on seeds 2 and 6).
- **Stone on seeds 3 and 4, Fire and Water** (Air too on seed 4): stores stop at 40 stone and
  wood falls to 0 from about minute 45, with plenty of trees and boulders in reach. Likely the
  stranding in #490; checked by #512.
- **Wood over 300** on seeds 2, 4 and 6: the stone- and ore-poor sites, where idle hands have
  nothing else to do (the same finding as #353's). #489 decides how the harness treats hard
  sites.
- **Air's food per head** runs 4 to 20 entering winter 2, more than twice the band's top on most
  seeds: #514.
- **Water on seed 3** loses 4 settlers with farms (#487, now #490). The `--no-farms` check never
  loses anyone, so the "close calls" half of owner decision 2 isn't met yet (#486).
- **The food sign** is met by minute 180 on only 2 (Earth), 1 (Fire), 3 (Water) and 6 (Air)
  of 8 seeds, because the economy opening builds a bed ahead of every arrival with no ceiling and
  growth spends each surplus on a newcomer. That's a harness gap before it's a balance one: #513.
- **Masters** is unmet by minute 180 on 9 of 32 runs, and scrolls on 5 (#340, #344, #443).
  Across all 32 runs the unmet signs at minute 180 are food 19, ore 9, masters 9, then stone,
  population, building kinds and scrolls 5 each, and branch 1.

Stone, population, wood and food for the three non-Air nations now sit at or near their targets
on every seed that isn't one of the known hard sites. Section 1's correctness problem is fixed:
every nation mines 40 ore wherever 40 ore is in reach, except Water on seed 7 (#397).

## Fit with other work

- **#340, #343, #344 (bending mastery):** these are separate, and this doc doesn't touch chi or the
  masters sign. Bore Tunnel's chi cost limits Earth's *bonus* (tunnel yields, seams). The base
  stone and ore supply no longer depends on it.
- **#336 (harvest tool filter):** "stone and ore" designation over a rectangle is how a player, or
  an agent, actually works a foothill of boulders and veins. It's a natural companion.
- **#337 (explain idle settlers):** a crafter idling at its stock limit should show up there as
  "Kiln idle: 20/20 brick stored".
- **#134 (metal ore):** when a second ore lands, the ore sign's `items` grows. Veins of that ore
  need the same density check, done by the supply survey.
- **#255, #203–#205 (power, mills, bison):** they don't change the kinds count, since those
  buildings already exist as data. When something finally consumes bricks or metal, revisit the
  stock limits.
- **difficulty.md:** sections 4 and 5 extend its tuning to the player-built producers it didn't
  cover. Wild food, strain and seasons stay as #301 left them.

## Follow-up issues

Filed without `ready`. Issues that wait on an owner answer are labelled `needs-owner`.

| # | Issue | Blocked by | Owner question |
|---|---|---|---|
| #348 | Balance harness: supply survey (stone and ore in reach, per seed) | none | |
| #349 | Data: stone and ore supply (richer outcrops and veins, foothill boulders, re-bless) | #348 | 4 |
| #350 | Sim and data: production stops at a recipe's `stock_limit` (the Brick Kiln and smelter drain) | none | |
| #351 | Sim and TUI: pause a production building, with a recipe line in the cursor panel | #350 | 1 |
| #352 | Balance harness: an economy opening that plays toward the gate signs | none | |
| #353 | Data: food, growth and wood retune for Normal (farms, orchard, arrivals, chop) | #352 | 2 |
| #354 | Sim and data: the "one of every building" sign (`every_building`, `spare`) | none | 3 |
| #355 | Balance check: re-run the playtests and update the endgame pacing (closes out #339 and #272) | #349, #350, #353, #354 | |
| #356 | Fire: give bricks and metal a use. Designed with #357 in [nation-buildings.md](nation-buildings.md) | none | |

## Owner decisions

The owner answered all four questions on 2026-11-05 (#339's comments). The recommendations as
written are kept below each answer.

1. **Kiln control: a stock limit plus a designed use for bricks, metal and tools** (#350, and
   #356 at p1). The pause key (#351) is parked.
2. **Normal gives close calls (b):** a colony that grows without farming loses someone by year 2,
   and one that farms never does. Real risk belongs to Hard. (#353; the `--no-farms` half isn't
   met yet, #486.)
3. **The building-kinds sign is computed per nation, with one spare kind** (#354). A designer
   task was filed to add buildings so the nations' counts are closer together (#357).
4. **Raise supply and keep the shared 200 stone and 40 ore targets** (#349).

### The questions as asked

1. **Brick Kiln control: a stock limit, a pause key, or both?** A stock limit in data
   (section 3) fixes the drain with no player action, while a pause key gives direct control and
   needs a key and a panel line. *Recommended: both.* The limit is the default that makes the
   kiln safe, and the pause covers a player saving stone for the gate. Making bricks worth
   something (#356) is the longer-term fix, and it's left for later.
2. **How much should Normal hurt?** (a) As difficulty.md set it: only neglect kills, and leaner
   food just removes the glut. (b) Close calls: a settlement that grows without farming loses
   someone by year 2, while one that farms never does. (c) Real risk even for attentive play,
   from harsher winters or events. *Recommended: (b).* It answers "never came close to dying"
   without a new system. Section 4's numbers get there, and the `--no-farms` run checks it. (c)
   is what Hard is for.
3. **Building-kinds target (#272): shared 8, per-nation numbers, or computed?** A shared 8 is
   out of reach for Earth and Water. Per-nation numbers (Air 7, Water 5, Earth 5, Fire 8) work
   today in data but go stale whenever content lands. *Recommended: computed, "one of every
   building your people can raise", with `spare = 1`.* It's fair to every nation by construction,
   and it needs one small sim change.
4. **Stone and ore: raise supply to meet 200 and 40, or lower the targets?** *Recommended: raise
   supply (sections 1 and 2) and keep the targets shared,* because they equal the gate's bill. The
   alternative is a lower stone target and a cheaper gate (say 120 stone and 20 ore), which makes
   the endgame thinner without making it any less of a grind for non-Earth nations. Per-nation
   ore (Air and Water lower) would only move the stall from the sign to the arch.
