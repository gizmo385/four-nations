# Difficulty and pressure: seasons, tiring land and presets

Status: **proposed** (#287). The owner's decisions are pending; see "Open questions for the
owner" at the end. Follow-up work: #294 to #301.

The owner's 2026-11-03 playtest found the game "pretty light": "I never had to venture outside of
where I started and never felt pressure for food or resources." This doc proposes the pressure
that's missing, three difficulty presets that scale it, and a headless harness that measures it.
It designs for the TUI only (#283), and it stays inside the vision's rule that pressure comes
from survival, the environment and growth, not from enemies (vision.md, "One nation, no war").

## Why it's easy today

Wild food is unlimited and close. A unit loses 4 hunger a minute, so it needs 240 hunger an hour:
about 8 fruit. The numbers in `data/jobs.toml` give these yields:

| Wild source | Yield | Hunger per hour | People it feeds |
|---|---|---|---|
| Fruit tree | 2 fruit every 6 min | 600 | 2.5 |
| Fishing spot | 2 fish every 5 min | 960 | 4 |
| Wild cabbage | 1 cabbage every 8 min | 225 | 0.9 |

Seed 7's default site has 14 food tiles in its 21x21 box, enough to feed about 30 people without
anyone walking more than 10 tiles. It never runs out, and nothing about the calendar changes it.
Population growth (#232) raises demand, but sixteen people still fit inside the starting box.
Every other system (research, buildings, the Harmony Gate) sits on top of a food supply that
needs no thought.

So the fix starts with food. The land near camp must give less the harder it's worked, and the
year must have a lean season that rewards planning ahead. Everything else in this doc follows
from those two.

## Decisions in brief

1. **Seasons.** A 60-minute year of four 15-minute seasons, starting in spring. Plants grow
   faster in spring and slower in autumn. Winter depends on the land's **climate band**, read
   from the tile's mapgen temperature: polar land stops growing, warm land barely slows. The first
   frost strips the ripe crop from wild plants in frost bands, so stores carry the settlement
   through winter. Cold land in winter tires people faster unless they're near a fire.
2. **Tiring land (strain).** Each harvest of a wild renewable plant adds strain to its tile.
   Strain lengthens the regrow time and fades slowly while the tile is left alone. A tree picked
   whenever it ripens settles at about a third of its fresh yield, so the land near camp gets
   tired and the fresh land farther out is worth the walk. Plants an orchard tends don't tire.
   On Hard, a plant worked to exhaustion dies.
3. **Growth eats.** A newcomer eats `newcomer_food` items from stores on arrival, on top of
   #232's threshold. Each new person costs something.
4. **Three presets in `data/difficulty.toml`: Sandbox, Normal and Hard.** Each scales the
   pressure (seasons on or off, winter severity, strain, need decay, regrow, research cost,
   newcomer food) and sets the starting stores that `World::found` lays down. Normal is the
   default. The player picks a preset on the New game seed step. The world stores the preset id,
   so saves and replays keep it, and retuned presets reach old saves, as other retuned numbers
   do.
5. **Exploration is driven by the land, not by quests.** Tired land near camp, mild southern
   land that keeps growing in winter, and the far-off scrolls, gold and seams that the endgame
   signs already need all pull people outward. Air's speed and Earth's carrying make that pull
   play differently by nation, with no new modifiers.
6. **No spoilage, no threats in v1.** Spoilage is invisible loss and micromanagement, and frost
   plus strain already cap how much a settlement can store. Hostile threats are an owner call
   (question 1).
7. **Measured headlessly.** A `balance` binary runs seeds, nations and presets with no input and
   with a scripted standard opening. It reports the minute of the first death, deaths, food per
   head going into each winter, and how far people walk for food. A CI soak test asserts that the
   presets rank in the right order. The targets in "Measuring it" are the bar for the tuning pass.

## Pressure sources, ranked

| Rank | Source | Verdict | Why |
|---|---|---|---|
| 1 | Seasons and winter | **v1** | Environmental, readable, and gives the year a rhythm: grow, store, endure. Fits Avatar (the poles, the Water Tribes). Climate makes embark choices matter |
| 2 | Strain on wild food | **v1** | The only source that creates distance pressure: near land tires and far land stays fresh. Makes farms (#198) and orchards the long-term answer |
| 3 | Growth eating more | **v1, small** | #232 already raises demand. A food cost on arrival makes growth a choice the player feels |
| 4 | Rare things far away | **Exists** | Scrolls (tundra and desert), gold (mountains) and seams (buried) are already far and already needed by the endgame signs |
| 5 | Floods (#199) | **Later knob** | An event pressure that is already planned. When it lands, it adds a `flood_chance` knob to the presets |
| 6 | Food spoilage | **Not now** | Invisible loss that needs granaries or rot timers to read well. Frost and strain already cap hoarding. Revisit if the harness shows stores snowballing |
| — | Hostile threats | **Owner** | No combat system exists, and the vision puts pressure on survival. See question 1 |

## Seasons

### The calendar

Season and year are pure functions of the tick: `season = (tick / season_ticks) % 4`. No new
state, no RNG. A world starts on the first tick of spring, so the first winter begins at minute 45
of play. The endgame takes about 3.5 to 4 hours (endgame.md), which is four winters: enough to
learn the rhythm, and too few for it to become a chore.

| Season | Plant growth | Notes |
|---|---|---|
| Spring | x1.25 | Recovery: tired land and bare plants come back fastest |
| Summer | x1.0 | |
| Autumn | x0.75 | The harvest season: stock up. A warning comes 3 minutes before winter |
| Winter | by climate band | First frost on its first tick. Cold on cold land |

"Growth" is the factor `World::growth_rate` already returns (the hook that the orchard's
`tend_rate` uses today), so anything that grows through it obeys the seasons: wild regrowth,
orchard saplings, and #198's farm plots once they use the same hook.

### Climate bands

Each tile's band comes from its mapgen `temperature` (0 is polar), the first band whose
`max_temperature` reaches it. A band is a fact about the land and shows on hover and in the site
report.

| Band | Temperature up to | Winter growth | Frost | Cold |
|---|---|---|---|---|
| Polar | 0.28 (the tundra line) | x0 | yes | yes |
| Cool | 0.45 | x0.1 | yes | yes |
| Temperate | 0.70 | x0.3 | yes | no |
| Warm | 1.0 | x0.6 | no | no |

**Frost.** On winter's first tick, every ripe wild plant whose harvest isn't `frost_hardy`, on a
frost band tile, goes to resting with its full regrow time. Its crop is gone, so whatever wasn't
picked and stored in autumn is lost. Fishing spots are `frost_hardy`: they skip the frost and
grow at a flat `hardy_winter_growth` (x0.5) in every band. Ice fishing keeps the Water Tribes'
signature trade alive in winter.

**Cold.** In winter, a unit on a cold band tile that isn't within a heat source's radius (a
campfire, a Hearthflame or an Inner Fire aura: anything `is_heated` already answers for) is
*cold*. A cold unit loses energy x1.5 and hunger x1.25. Asleep, it sleeps at x0.5 unless it's
heated or in a bed with a `sleep_bonus` above 1 (the igloo), so Water's ice construction is
winter shelter. Cold never damages health directly: it kills only through the needs it drains, which
the player can see coming. Fire's `heating` modifier already widens its campfires, and Inner
Fire already moves with its carrier, so Fire endures the cold best through data that already
exists.

### `data/seasons.toml`

```toml
[calendar]
season_minutes = 15.0
winter_warning_minutes = 3.0
hardy_winter_growth = 0.5        # frost_hardy harvests (fish) in winter, every band

[[season]]
id = "spring"
name = "Spring"
growth = 1.25

[[season]]
id = "summer"
name = "Summer"
growth = 1.0

[[season]]
id = "autumn"
name = "Autumn"
growth = 0.75

[[season]]
id = "winter"
name = "Winter"
winter = true                    # growth from the band; frost at its start; cold applies

[[climate]]
id = "polar"
name = "Polar"
max_temperature = 0.28
winter_growth = 0.0
frost = true
cold = true
outlook_points = 2               # added to a site's outlook when seasons are on

# ... cool, temperate, warm as in the table above

[cold]
energy_decay = 1.5
hunger_decay = 1.25
sleep = 0.5
```

`data/jobs.toml` gains `frost_hardy = true` on the fish harvest. Validation requires exactly one
`winter` season, four seasons in order, bands that ascend to 1.0, and growth factors in `[0, 4]`.

## Strain: tiring land

A wild renewable harvest gains a `strain` amount in `data/jobs.toml`:

```toml
[[harvest]]
resource = "fruit_tree"
# ...
regrow = 360.0
strain = 1.0          # added to the tile's strain per harvest (times the preset's strain)

[settings]
strain_recovery_per_minute = 0.05   # strain fades this fast, ripe or resting
strain_kill_at = 4.0                # with the preset's strain_kills: a harvest that leaves
                                    # the tile at or above this kills the plant
```

- **Regrow.** A harvest sets the resting time to `regrow x preset.regrow x (1 + strain)`, using
  the strain after this harvest.
- **Recovery.** Strain falls by `strain_recovery_per_minute` every tick, and the entry is dropped
  at 0. It's stored as `World::strain: BTreeMap<Pos, f32>` (`#[serde(default)]`, skipped when
  empty, so old saves and the `World::new` path don't change).
- **Tended plants don't tire.** No strain builds within an orchard's radius (`tend_rate > 1`).
  Strain models wild land being over-picked; a tended orchard or a farm plot is the sustainable
  answer.
- **Death (Hard only).** With `strain_kills`, a harvest that leaves strain at or above
  `strain_kill_at` clears the resource: the plant is gone, as a felled tree is.

With these numbers, a fruit tree picked whenever it ripens settles at strain 2.3 and a 20-minute
cycle: 6 fruit an hour instead of 20. A fishing spot (strain 0.5) settles at 12 fish an hour
instead of 24. Under Hard's x1.5 strain, a tree picked whenever it ripens reaches 4 within a few
cycles and dies, so picking has to rotate. A 21x21 start box then feeds roughly its first
dozen people in summer and fewer in winter. That's the point where the player farms, plants
orchards or walks farther. The harness tunes the exact numbers.

Strain doesn't apply to one-shot harvests (trees, stone, gold). Those are already used up when
they're worked.

## Growth and starting stores

- **`newcomer_food`.** When #232 admits a newcomer (a free bed, and `food_per_newcomer` food in
  stores), the arrival also takes `newcomer_food` food items from stores, the nearest stack to
  the start first, then by position. 0 in Sandbox, 10 in Normal, 20 in Hard.
- **Starting stores.** `World::found` designates a stockpile of open tiles nearest the start (in
  the start's region, by squared distance and then by position, the same rule as for spawns) and
  fills them up to `stockpile_stack` per tile from the preset's `starting_stores`. It draws no
  RNG. The settlers arrive with something to eat and something to build with, which matters now
  that the first winter comes at minute 45.

## Exploration

The issue asks what should pull the player away from the start. This design answers with the land
itself, with no new quest systems:

- **Tired land.** Strain makes the near tiles slow and leaves the far ones fresh. A forager's walk
  grows as the settlement grows, and a second stockpile and beds out by a fresh grove become a
  real choice.
- **Climate.** Warm land keeps growing at x0.6 in winter. A warm site to the south, or a winter
  fishing camp, changes how winter plays. The embark report shows the band (see below), so where
  to settle becomes a choice about winter too.
- **Far finds that already exist.** Scrolls, gold veins and buried seams are rare and far away,
  and the endgame signs ask for 3 scrolls read and 40 ore. Nothing new is needed here.
- **Nations through existing modifiers.** Air (move speed 1.6) forages far cheaply, Earth (carry
  1.3) hauls a distant harvest home in fewer trips, Water's fish keep coming through winter and its
  igloos sleep warm, and Fire stays warm. Pillar 1 holds with no `if nation ==`.
- **Later hooks.** #197 (glider scouting) and any future ruins sit naturally on top of this. The
  rough survey stays the place for hidden information at embark.

## Difficulty presets

### What each preset turns

| Knob | Sandbox | **Normal** (default) | Hard | Scales |
|---|---|---|---|---|
| `seasons` | off | on | on | Calendar, frost, cold, winter growth |
| `winter_severity` | — | 1.0 | 1.3 | Winter growth loss `(1-g)` and cold's extra drain `(m-1)`. Frost needs > 0 |
| `strain` | 0 | 1.0 | 1.5 | Strain per harvest. 0 means plants never tire |
| `strain_kills` | no | no | yes | Exhausted wild plants die |
| `need_decay` | 1.0 | 1.0 | 1.1 | Hunger, thirst and energy decay |
| `regrow` | 1.0 | 1.0 | 1.25 | Every regrow time |
| `research_cost` | 1.0 | 1.0 | 1.25 | Every tech's cost |
| `newcomer_food` | 0 | 10 | 20 | Food eaten from stores on arrival |
| `starting_stores` | 30 food, 20 wood, 10 stone | 15 food, 10 wood | none | Laid down by `World::found` |

Sandbox is today's game with a head start: no winter, no tiring land. Normal adds the year and
the tiring land, and nothing that punishes beyond them. Hard turns everything up and makes
over-picking fatal to the plant, never directly to a person.

The rough survey stays a separate choice on the same screen (question 4). Floods (#199) add a
`flood_chance` knob when they land; until then the schema has none, because validation rejects
unknown keys.

### `data/difficulty.toml`

```toml
default = "normal"

[[preset]]
id = "normal"
name = "Normal"
hint = "Winters to store food for, and land that tires when it's over-picked."
seasons = true
winter_severity = 1.0
strain = 1.0
strain_kills = false
need_decay = 1.0
regrow = 1.0
research_cost = 1.0
newcomer_food = 10
starting_stores = [{ item = "fruit", count = 15 }, { item = "wood", count = 10 }]

# [[preset]] sandbox and hard as in the table. File order is the picker's order.
```

`Defs::with_difficulty` loads it strictly, as `with_embark` does: unknown keys are rejected, item
ids resolve, multipliers are in `(0, 4]` (strain and winter severity also allow 0), and
`default` names a preset.

### How the sim reads it

- **`World::difficulty: Option<String>`**, the preset id (`#[serde(default)]`, skipped when
  `None`). `None` is the identity: today's rules exactly, with no seasons, strain, stores or
  newcomer food. It's what `World::new` and every save from before this change get, so no golden
  is reblessed. The command-line `SEED NATION` path stays on `None`, as embark did with its
  start; moving it onto Normal is a later cleanup with a golden rebless, not filed.
- **`World::knobs(&defs) -> &Knobs`** resolves the id (an index cached in a `#[serde(skip)]`
  field). Each system multiplies where it already reads its number: `tick_needs` (need decay and
  cold), harvest completion (regrow and strain), research (cost, including #284's pool cap, which
  is "the cost of the target"), population (newcomer food), and `growth_rate` (season times
  band). Defs themselves are never mutated, so a loaded save needs no special Session plumbing:
  `Outcome::Loaded` keeps working as it is.
- **Store the id, not the numbers.** A retuned preset reaches existing saves, the same way a
  retuned need rate does (save.rs: a snapshot "survives balance changes"). A save whose preset id
  no longer exists fails to load with a clear `SaveError`, the same as a data mismatch.
- **Construction.** `World::found(survey, nation, start, units, difficulty, defs)` and
  `Session::found(.., difficulty)` take the id. Replays rebuild the initial world from the
  recorded id. No `SAVE_VERSION` bump: every field is additive.

### Embark

- **Climate in the report.** `SiteReport` gains `climate`, the start tile's band. The Land section
  shows it when the preset has seasons: `Winter     cool: cold, frost`. It's exact in a rough
  survey too, because terrain and climate are never what the scouts get wrong (embark.md).
- **A winter term in the outlook.** With seasons on, the outlook adds the band's
  `outlook_points x winter_severity` (rounded), and the reason reads "cold winters". This is how
  difficulty reaches the outlook: a polar site rates harder under Hard than under Normal, and
  Sandbox ignores climate. Embark.md suggested scaling the thresholds instead. A term tied to a
  fact about the land is more honest than shifting every verdict.
- **The panel header** gains a line under the seed: `Normal     Small · Exact`.

## TUI

**Seed step.** A fourth row, *Difficulty*, after Survey, changed with left and right, showing the
preset's `hint`:

```
│ Air Nomads — new game:                                       │
│                                                              │
│   Seed: 7                                                    │
│   Size: Small                                                │
│   Survey: Exact                                              │
│ ▸ Difficulty: Normal                                         │
│                                                              │
│ Winters to store food for, and land that tires when it's     │
│ over-picked.                                                 │
```

**In game.** When the preset has seasons, the side panel gets a Season section under People:

```
│ ◇ AUTUMN · YEAR 1  4:10 left │
│ Food in stores   38 · 4/head │
│ Winter here: cold, frost     │
```

Food per head turns to the warn tone in autumn and winter when it's below `winter_food_per_head`
in `data/tui.toml` (4, about two winter meals each), because that's presentation, not a rule.
The "here" line describes the band at the start (or at the map centre, for `World::new`).

**Alerts** (a new `Kind::Season`): "Winter in 3 minutes: 38 food for 9 people." "Winter. Wild
plants rest, and the cold tires people away from fires." "First frost: 23 wild plants died
back." "Spring: the land wakes." On Hard: "A fruit tree at (90, 41) was picked to death."

**Readable simulation (hover).** A plant reads `Fruit Tree · regrows in 9:10 · tired (strain
2.1)`, plus on Hard `one more picking may kill it` at or above `strain_kill_at - strain`. A tile
reads `Cool climate: cold and frost in winter`. A cold unit's status line reads `Cold: tiring
faster (no fire near)`.

**Winter look.** Optional and cheap: a `[winter]` palette override in `data/tui.toml` paints land
glyphs paler in winter. Mono and `--ascii` are unaffected.

**Command line.** `--difficulty ID` applies to `--embark`, `--start` and the plain path (where it
swaps `None` for that preset), so goldens can show seasons.

## Measuring it

A `balance` binary (`cargo run -p fn_sim --bin balance -- --seeds 1-8 --minutes 180`) runs every
(preset, nation, seed) on Small, headless and deterministic, in two modes:

- **No input.** The settlers forage, drink and sleep on their own, as they do today.
- **Standard opening.** `fn_sim::autoplay::opening` issues commands from world state at fixed
  minutes: a stockpile by the start, standing harvest orders on wild food within 15 tiles, a bed
  per person, a campfire once known, and a fixed research queue. It's plain code over `Command`,
  so it's deterministic and doubles as a smoke test for the command API.

It prints one row per run: the minute of the first death, deaths and population at 180 minutes,
food per head on the first tick of each winter, and the median path length walked per meal in
the 15-minute windows at minute 15 and minute 90.

**Targets for the tuning pass** (seeds 1-8, all four nations, Small):

| | Sandbox | Normal | Hard |
|---|---|---|---|
| No input: first death | none in 3 h | first or second winter (45-120 min) | first winter (45-60 min) |
| Opening: deaths by 3 h | 0 | 0 | at most 2 |
| Food per head entering winter 2 (opening) | — | 3 to 8 | 1 to 4 |
| Walk per meal, minute 90 vs minute 15 | about 1x | at least 1.5x | at least 2x |

**CI** (`crates/sim/tests/balance.rs`, a soak test under #291's split, seed 7, Earth, 90
minutes): two runs give identical output. With no input, the first death comes no later under
Hard than under Normal, and never under Sandbox. With the standard opening, Normal loses nobody.
Absolute targets stay out of CI, because they shift with every retune.

## Fit with other work

- **#198 farm plots** (in progress): growing through `growth_rate` and resting means plots obey
  the seasons for free. Plots are tended, so they take no strain. Frost should take unharvested
  ripe crops in frost bands. The seasons issue covers this once both have landed.
- **#232 population**: unchanged except `newcomer_food`. **#272** (retune endeavor targets after
  growth) should wait for the tuning pass.
- **#237 the Avatar**: he's a guest. Cold and need decay apply to him as to anyone, and his
  patience rule already sends him home before his needs could kill him.
- **#199 floods**: adds `flood_chance` to the presets when it lands.
- **#280 and #290**: only the seed step's rows and the embark header change.

## Follow-up issues

| Issue | What | Blocked by |
|---|---|---|
| #294 | Sim and data: `data/difficulty.toml`, `World::difficulty`, scalar knobs, newcomer food, starting stores | |
| #295 | Sim and data: seasons, climate bands, frost and cold (`data/seasons.toml`) | #294 |
| #296 | Sim and data: strain on wild renewable harvests | #294 |
| #297 | Embark: climate in the site report and a winter term in the outlook | #295 |
| #298 | TUI: Difficulty row on the seed step, embark header and `--difficulty` | #294 |
| #299 | TUI: seasons and strain made readable (Season section, alerts, hover, cold status) | #295, #296, #298 |
| #300 | Tooling: `balance` harness, standard opening and CI soak test | #294, #295, #296 |
| #301 | Data: tune the presets, seasons and strain to the targets | #300 |

## Open questions for the owner

1. **Hostile threats: none?** *Recommended: none in this design.* The vision puts pressure on
   survival, the environment and growth, and there's no combat system to build on. If the game
   later wants a non-combat antagonist, an angry spirit that blights land the settlement has
   over-picked would fit both Avatar and strain, with nothing to fight. *Alternative:* wildlife
   or spirits that harm people, which needs a combat or flee design of its own.
2. **Should the default get harder than today?** *Recommended: yes.* Normal, the default, has
   seasons (a 60-minute year with winter from minute 45) and tiring land. Today's feel lives on as
   Sandbox, with starting stores added. *Alternative:* default to Sandbox and make the pressure
   opt-in, which leaves the playtest's complaint as the first-run experience.
3. **Can over-picked wild plants die?** *Recommended: on Hard only.* On Normal, strain only slows
   regrowth, so a mistake costs time, not the land. *Alternative:* on Normal too, which makes the
   map visibly scarred by careless picking.
4. **Preset names and the rough survey.** *Recommended:* three presets with plain names (Sandbox,
   Normal, Hard), with Avatar flavour in the hints. The rough survey stays a separate toggle that
   no preset forces. *Alternatives:* themed names (for example "Spirit Oasis, Village, Northern
   Water Tribe"), a fourth "Harsh" tier that forces the rough survey, or Hard forcing it.
