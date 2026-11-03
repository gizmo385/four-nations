# Embark: choosing where to settle

Status: **proposed** (#286). Owner questions are at the end. Follow-up work: #288, #289 and #290.

Today mapgen makes the map and `World::new` drops six settlers on the walkable tiles nearest the
map centre, wherever that is. On seed 7 the centre, (96, 64), is open sea, so the settlers land on
a strip of coast with the sea filling half the area around them and no stone within 10 tiles. The
owner's playtest asked for Dwarf Fortress's embark instead: see the whole map, zoom in, and choose
the site. This doc designs that step for the TUI, the only frontend from now on (#283).

## Decisions in brief

1. **One more step in #280's New game flow.** Title → New game → nation → seed → **embark** →
   game. Embark opens with the site box on a default site, so `E` `y` starts the game straight
   away. Passing `SEED NATION` on the command line still skips every menu, embark included.
2. **Three zoom levels, computed from the terminal.** An *overview* that fits the whole map (1:6
   at 80x24, 1:4 at 120x40), a *region* view at 1:2 and the *site* view at 1:1, which is the
   game's own map drawing. Each cell of the overview and region views shows the block's dominant
   land class as a glyph from `data/tui.toml`.
3. **A site is a 21x21 box around the cursor.** The side panel reports on it live: the land, room
   to roam, the walk to water, counts of wood, stone, food, gold and scrolls, and a one-word
   outlook. The sim computes all of it in a pure `Survey::report(pos)`.
4. **The sim surveys first, then founds.** A new `fn_sim::embark::Survey` holds a generated map
   (with buried resources already hidden) and answers reports. `World::found(survey, nation,
   start, ..)` builds the world from it. Mapgen never sees the start, so a map is still a
   function of its seed and size alone.
5. **Deterministic, with no churn.** A founded world is a function of (seed, size, nation, start,
   data). The start goes into the world as `start: Option<Pos>`, so saves and replays have it.
   Today's `World::new` path is unchanged byte for byte, so no golden needs reblessing.
6. **Only impossible sites are refused.** A site must be open land, away from the edge, and on a
   landmass big enough to live on. A poor site (no food, water far away) is allowed with a warning,
   because a hard start is a legitimate choice (#287).

## Flow and keys

```
Title ─ New game ─▶ Nation (1-4) ─▶ Seed (digits) ─▶ EMBARK ─E,y─▶ game
                                        ▲              │
                                        └──── Esc ─────┘  (at the overview; the seed is kept)
```

#280 ends the seed step with "then starts". Embark replaces that "starts": Enter on the seed step
opens embark for that seed and nation instead of building a `Session`. Nothing else in #280
changes. Whatever #280 names its pre-game states, embark is one more state after the seed one,
holding a `Survey`, the zoom level and the cursor.

| Key | In embark |
|---|---|
| Arrows, `W` `A` `S` `D` | Move the cursor one cell, which is N tiles at 1:N. Shift moves 10 cells |
| `Enter`, `+` | Zoom in on the cursor. At 1:1, settle here (asks y/n) |
| `-`, `Esc` | Zoom out. `Esc` at the overview goes back to the seed step |
| `E` | Settle at the cursor from any zoom (asks y/n: "Settle at (88, 44)?") |
| `R` | A new random seed: a new map, back to the overview, cursor on its default site. No prompt, because nothing is lost |
| `?`, `Q` | Key help, and quit (as in the game, via #280's back-to-title prompt) |

The y/n prompt is the one the game already uses for `R` and quickload. Settling on a refused site
shows the reason as a notice instead of the prompt.

`+` and `-` change speed in the game. Embark has no clock, so they're free to zoom.

## The three zoom levels

All mockups are 80x24, the smallest supported terminal. Map glyphs are shown in the `--ascii`
set, because sea and lake differ only by colour in the Unicode set. `~` is sea, `-` lake, `=`
river, `^` mountain, `,` plains, `"` forest, `:` desert, `*` tundra, `&` taiga and `%` swamp.
The cursor (reverse video) and the tinted site box don't show in plain text. The four corner
marks do.

### Overview (fits the map)

```
        ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~        ┏───── ◆ CHOOSE A SITE ◆ ──────┓
        ~~~~~~~~~~~*"~~~~~~~~~~**~~~~~~~        │ Seed 7         Earth Kingdom │
        ~~~~~~~~***="*~**~~~~**"~~~~~~~~        │ Zoom 1:6 overview   (88, 44) │
        ~~~~~~~~~-****,***~~~**""~"~~~~~        │                              │
        ~~~~~~~~~,,,┌^,,,┐~~~=-&&&&""&&~        │ ◇ LAND              mainland │
        ~~~~~~~~~,,,,^,,,~~~,---^^&-"&&~        │ Plains 58%  Desert 15%       │
        ~~~~~~~~",,,^^,,,~~~"-""^^=="~~~        │ Mountain 16%  Sea 6%         │
        ~~~~""=""",,,,,,,,~~""""^""%~~~~        │ Room to roam     4,563 tiles │
        ~~~~""="""",,^,,~,,"""=--"--~~~~        │ Water          lake, 8 steps │
        ~~~~"""""""",,,~~~,=""="^^"-~~~~        │                              │
        ~~~~""="""~=└,,,~┘,,===^^"==~~~~        │ ◇ WITHIN 10 TILES      21x21 │
        ~~~~~""~~~~,,,,,~~,=="""^"""~~~~        │ Wood     52           plenty │
        ~~~~~%%"~~~",,,~~,,,===--"""~~~~        │ Stone    15           plenty │
        ~~~~~~"""~~~~,,~,,,=","""""""~~~        │ Food     14             some │
        ~~~~~~""""~~~~,,,^,,,,-"="""~~~~        │ Gold      2             some │
        ~~~~~~""",,~~,,,^-,,,-===,"~~~~~        │ Scrolls   0             none │
        ~~~~~""^^,,,,^^^^^,,=&&,,,~~~~~~        │                              │
        ~"""~&*^"^,,,^^^^^^^*&&"~~~~~~~~        │ ◇ OUTLOOK               Easy │
        ~""-&~"""^**-"^"*-*-**~~~~~~~~~~        │ Wood and stone plentiful;    │
        ~~~*~~~*"***""**~-^*~~~~~~~~~~~~        │ water 8 steps away.          │
        ~~~~~~~~~**~~~~~~~~~~~~~~~~~~~~~        │                              │
        ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~        │                              │
                                                ┗──────────────────────────────┛
 EMBARK  arrows move  Enter zoom in  E settle  R new seed  Esc back  ? keys
```

**Scale.** The overview scale N is the smallest whole number for which the map fits the map region
(the terminal minus the 32-column panel and the status line): `ceil(W/N) <= cols` and
`ceil(H/N) <= rows`. For the 192x128 map that is N=6 at 80x24 (32x22 cells), N=4 at 120x40
(48x32) and N=3 at 200x60. The overview is centred in the region. One cell covers NxN tiles,
so it has the same proportions as the game map (both are stretched tall by the cell shape). Any
map size works with no special cases, which keeps map-size presets cheap if the owner wants them
(question 1).

**What a cell shows.** Each block of tiles is classified in this order, and the first rule that
matches wins:

1. Sea is at least half the block → sea.
2. Fresh water (river, lake or ford) covers at least `max(1, N/2)` tiles → river or lake,
   whichever is more common. Rivers are one tile wide and would vanish in a plain majority vote,
   but water is the thing a player is looking for.
3. Mountain is at least a third of the block → mountain.
4. Otherwise → the most common biome among the land and coast tiles, with ties going to file
   order.

The cell takes that class's glyph and colours from `data/tui.toml`, so `--ascii` and
`--color never` work unchanged. The two thresholds live in a new `[embark]` table there, because
they're presentation, not rules. Resources aren't drawn at the overview or the region scale,
because single glyphs would be too sparse to read. The panel counts them instead. The overview is
computed once per map and scale, and cached.

*Considered:* a colour-only biome map with half-block cells (`▀`, two tiles per cell, square
pixels). It's prettier in truecolor, but it's unreadable with `--color never` and in the text
goldens, and it reads differently from the game map. The glyph overview keeps one visual
language at every zoom.

### Region (1:2)

```
~~~~~~~",************,,,,,****,.,.~~~~~~~~~"****┏───── ◆ CHOOSE A SITE ◆ ──────┓
~~~~~~.,,,****,*****",,,,,-**,,~~~~~~~~~~~~&&&*&│ Seed 7         Earth Kingdom │
~~~~~~",,,,,,,,,**"^",,,,,-,,,,~~~~~~~~~~~~=&&&&│ Zoom 1:2 region     (88, 44) │
~~~~~~~,,,,,,,--",,^^,,,,,,,,,,.~~~~~~~~~~"=="--│                              │
~~~~~~~,,,,,,,,,,,,,^,,,,,,,,,,.~~~~~~~~~~""=---│ ◇ LAND              mainland │
~~~~~~~.,,,,,,,,,,,^^^,,,,,,,,,~~~~~~~~~~."""---│ Plains 58%  Desert 15%       │
~~~~~~~~,,,,,,,,,,┌^^^,,,,,,,,┐~~~~~~~~~.","----│ Mountain 16%  Sea 6%         │
~~~~~.~~,,,,,,,,,,,^^^,,,,,,,,,.~~~~~~~~,,,"---"│ Room to roam     4,563 tiles │
~~~~~~~~,,,,,,,,,^^^^^,,,,,,,,,.~~~~~~~.,,,""-""│ Water          lake, 8 steps │
.~~~~.~.,,,,,,,^^^^^-^,,,,,,::,~~~~~~~~~"""""=="│                              │
~~~."".~.,,,,,,,,,,^^^,,,,,::,,,~~~~~~~~~"""""""│ ◇ WITHIN 10 TILES      21x21 │
~~""""".~===,,,,,,,,^^,:::::,,,,,,~~~~~~~~""""""│ Wood     52           plenty │
%,"""""""",,,,,,,,,,,,,,::,,,,,,,.~~~~~~~~""""""│ Stone    15           plenty │
%"""""""""",,,,,,,:,,,,,,,,,,~~.,,.~~~~~~"""""""│ Food     14             some │
="""""-""""",,,,,,:,^^^,,,,,~~~,,,,,~~~.""""""""│ Gold      2             some │
=,""""""""""",,,,,,,^^,,,,,,~~~.,,,,~~~""""""""-│ Scrolls   0             none │
=""""""""""""",,,,,,,^,==,,~~~~~.,,,,,~",""",^,=│                              │
%""""""""""""""",,,,^^,,==.~~~~~~,,,,~~,""""""%=│ ◇ OUTLOOK               Easy │
""""""""""""""""",└,^,,,~~~~~~┘~~~,,~~~""""""",=│ Wood and stone plentiful;    │
"""""""""""""""==,,,,,,,,~~~~~~~~.,,~~~=""""""%=│ water 8 steps away.          │
""""""""".~."""=,,,,,,,,,,,~~~~~~,,,,.,"=="-""==│                              │
="""""=""~~~~"=",,,,,,,,,,,~~~~~.,,,,,,"""=-==="│                              │
=="""~~~~~~~~~"",,,,,,,,,,,~~~~~.,,,,,,,""""="""┗──────────────────────────────┛
 EMBARK  arrows move  Enter zoom in  - zoom out  E settle  R new seed  ? keys
```

The same classification at N=2, with a view that scrolls to follow the cursor the way the game's
camera does. This is the level that matters on small terminals: at 80x24 it shows 96x46 tiles,
enough to compare neighbouring sites. The zoom ladder is `[fit, 2, 1]` with duplicates removed,
so a terminal big enough to fit the map at 1:2 gets two levels.

### Site (1:1)

```
,,c,,T,,T,,,,┌,^^^^^,,,,,,,,,,,,,,,┐,,.~~~~~~~~~┏───── ◆ CHOOSE A SITE ◆ ──────┓
,,,,T,T,,,,,T,,^^^^^,,,,,,,,,,,,Tc,,T,.~~~~~~~~~│ Seed 7         Earth Kingdom │
,,,T,T,T,,,,TT,o^^^^,T,,TT,,,,,,,,,TTT..~~~~~~~~│ Zoom 1:1 site       (88, 44) │
,,,,,,T,T,,,,TT^^^^^,c,,,,,,,,,T,,,T,T..~~~~~~~~│                              │
,,,,,TT,,,^oo^,$oo^o^,Y,,,Tc,,T,Y:::,..~~~~~~~~~│ ◇ LAND              mainland │
TTT,,T,^^o^^^^^^-^^^^,,,,,,,,,,T:::T,.~~~~~~~~~~│ Plains 58%  Desert 15%       │
TT,,TT^o$^^^^o^o^^,^,,,,,,,,T,c::::,T.~~~~~~~~~~│ Mountain 16%  Sea 6%         │
,TTT,,^T,o^,T,^^o^o^,,,,,,,,,,:::::TT...~~~~~~~~│ Room to roam     4,563 tiles │
,,TT,T,TTTT,,,,,^o^^,,,::::,c,::::TTT,,....~~~~~│ Water          lake, 8 steps │
TTT,,,TTYTTT,,T,o^^^,,::::::::::TT,TTT,,,,..~~~~│                              │
T,,T,T,,,,,T,,T,o^o,,,::::::::,,,Tc,T,,T,Y,.~~~~│ ◇ WITHIN 10 TILES      21x21 │
TT,,,,TTTT,,,,,,,,T,,,,::::::,,,,,YT,,,,,,..~~~~│ Wood     52           plenty │
T,,,,,,,,,,,::,:,TT,,,,::::,,,Y,,,,...,,,,.~~~~~│ Stone    15           plenty │
,,T,,,,,,,,,::TTT,TT,TT,,,:,,,,,,...~...,,...~~.│ Food     14             some │
,T,,,,T,,,T::::^TTT,,TTT,,cT,,,,..~~~f~.,,,,...~│ Gold      2             some │
T,,c,,,,,,,,:::^^o^^^,,TTTT,Y,,,.~~~~~..,T,,,,..│ Scrolls   0             none │
,,T,,,,,,,,,T,,T^$^^^TT,,,,,,,,..~~~~..,Tc,TT,,.│                              │
",,T,,,,T,,,,,TT^^^oT,,T,T,T,c,.~~~~~~..,,TT,,,.│ ◇ OUTLOOK               Easy │
"",,,,,,,,,,,,c,,^^^,T,,T,,,,,..~~~~~~~..,,,T,,.│ Wood and stone plentiful;    │
F"F,,,,,,,,,,,,,,^^^,T,=T,,,,,.~~~~~~~f~..,,,T,.│ water 8 steps away.          │
""FY""YT,T,,,,,,,,o^,,,,=,,,,..~~~~~~~~~~.,TT,,T│                              │
""F""F"F,,,,,,,,,o^^,TT,,=,,..~~~~~~~~~~~..T,,,,│                              │
F"F"""""",,,,└,,,^^^,,,...=..~~~~~~┘~~~~~~..TT,.┗──────────────────────────────┛
 EMBARK  arrows move  - zoom out  Enter/E settle here  R new seed  ? keys
```

This is the game's own map renderer (`map.rs`) over the survey's map, with no units and with the
site box drawn on top. Buried gold seams (`!` in the mapgen ASCII) aren't on it: `Survey` hides
them exactly as `World::bury` does today. At 1:1 the 21x21 box plus its corner marks is 23 rows,
which is exactly the map region at 80x24. That is why the report radius defaults to 10.

**The site box.** At every zoom the box's cells take the accent tint that dragged rectangles use
(reverse and dim without colour), and a corner mark `┌ ┐ └ ┘` sits just outside each corner. The
marks hide four cells and make the box visible in text goldens and in mono. On a refused site
the marks and tint turn to the `bad` tone.

A refused site replaces the outlook section with the reason:

```
│ ◇ OUTLOOK                    │
│ ✗ Can't settle: open water.  │
```

## The site report (sim)

`Survey::report(pos, defs) -> SiteReport` is pure, cheap (at most 441 tiles plus a walk capped
at 40 steps) and runs on every cursor move.

| Field | What it is |
|---|---|
| `problem: Option<SiteProblem>` | Why you can't settle here: `NotLand`, `NearEdge`, `TooSmall`, `NoRoom` (fewer open tiles than settlers near the start) |
| `biomes: Vec<(BiomeId, u8)>` | The share of the box per biome, plus mountain and water, largest first. The panel shows the top four |
| `room: u32`, `mainland: bool` | Open tiles reachable from the start (`Regions` under the default `MoveRules`), and whether that is the largest landmass |
| `water: Option<(Terrain, u32)>` | The nearest drinkable tile by walking steps, and its kind. Units drink from any water (`Terrain::is_water`), the sea included |
| `groups: Vec<GroupCount>` | Per resource group from data: the count in the box and its level (none, scarce, some, plenty) |
| `outlook: Outlook`, `reasons` | Easy, Fair or Hard, and the one or two facts that decided it |

Buried resources never count, and neither does anything else the player couldn't see on the map.

### `data/embark.toml`

Everything that's balance lives in data, read into `Defs` the way `endeavor.toml` is
(`Defs::with_embark`). Validation checks that every resource id exists and that each group's
levels ascend.

```toml
report_radius = 10        # the site box is (2r+1)^2 tiles
min_edge_distance = 8     # tiles from the map edge
min_room = 300            # open tiles reachable from the start
spawn_radius = 4          # settlers start within this many tiles of it

# Each group is counted inside the box. levels = [scarce_from, some_from, plenty_from].
[[group]]
id = "wood"
name = "Wood"
resources = ["oak_tree", "fir_tree"]
levels = [1, 10, 30]
outlook_weight = 1

[[group]]
id = "stone"
name = "Stone"
resources = ["stone"]
levels = [1, 4, 12]
outlook_weight = 1

[[group]]
id = "food"
name = "Food"
resources = ["cabbage", "fruit_tree", "fish_spot"]
levels = [1, 6, 15]
outlook_weight = 2

[[group]]
id = "gold"
name = "Gold"
resources = ["gold_vein"]
levels = [1, 1, 4]
outlook_weight = 0        # nice to have; never makes a site harder

[[group]]
id = "scrolls"
name = "Scrolls"
resources = ["ancient_scroll"]
levels = [1, 1, 3]
outlook_weight = 0

[outlook]
# Points: a group at "none" adds 2x its weight, at "scarce" 1x. Water adds water_weight when the
# walk is longer than water_far, and twice that when there is none within the cap.
water_far = 15
water_weight = 2
fair_from = 1
hard_from = 4
```

With these numbers the mockup's site at (88, 44) rates Easy. Seed 7's default site, by the
centre, rates Fair: there's no stone in its box.

The outlook is a *site* hint, a fact about the land. It is not the difficulty mode that #287 is
designing (see "Hooks" below).

## Sim changes

```rust
// crates/sim/src/embark.rs
pub struct Survey { cfg, surface: TileMap, buried: BTreeMap<Pos, ResourceId>, regions: Regions }
impl Survey {
    pub fn new(cfg: &MapGenConfig, defs: &Defs) -> Self;   // mapgen::generate, then hide buried
    pub fn map(&self) -> &TileMap;                          // what frontends draw
    pub fn default_site(&self, defs: &Defs) -> Pos;         // the valid site nearest the centre
    pub fn report(&self, at: Pos, defs: &Defs) -> SiteReport;
}
impl World {
    pub fn found(survey: Survey, nation: Nation, start: Pos, units: u32, defs: &Defs)
        -> Result<World, SiteProblem>;
    pub fn start(&self) -> Option<Pos>;
}
// crates/ui/src/session.rs
impl Session { pub fn found(survey: Survey, nation: Nation, start: Pos) -> Result<Self, SiteProblem>; }
```

- **Mapgen doesn't take the start.** The issue suggested `Session::new` and mapgen take a start.
  Only world construction does: the embark screen needs the map before there's a world, and
  keeping mapgen start-free means one generation serves every site the player looks at, with the
  mapgen goldens untouched. `R` costs one generation, about 0.13 s for 192x128 in a debug build.
- **Spawning.** `found` puts the settlers on open tiles (walkable, no resource) in the start's
  region, nearest first by squared distance and then by position, within `spawn_radius`. It draws
  no randomness, and the name and variant streams (`world.names`, `world.units`) are untouched, so
  the same seed gives the same people wherever they settle. #287's starting stores, if it adds
  them, go down at the start here too.
- **No churn for the old path.** `World::new` keeps today's centre spiral exactly and leaves
  `start` as `None`, which isn't serialized (`skip_serializing_if`). Every mapgen, world, save
  and TUI golden stays as it is. Moving the CLI path onto `found(default_site)` would improve
  seed 7's start but rebless many goldens while #212, #239 and #284 are in flight. It's a later
  cleanup and is not filed.
- **Saves.** `start` is an additive `#[serde(default)]` field, so there's no `SAVE_VERSION`
  bump. `SaveInfo` doesn't need it.
- **Replay.** The initial world of a replay is `World::found(Survey::new(cfg), nation, start, ..)`
  with the recorded start, or `World::new` when it's `None`. The doc comment on `World::replay`
  says so.
- **Tests.** Founding twice at the same (seed, start) serializes identically. Settlers land in the
  start's region within the radius. Each `SiteProblem` is refused. A text golden of seed 7's
  report at three sites (`crates/sim/tests/golden/embark_seed7.txt`) includes one with a buried
  seam in the box, which must not be counted.

## TUI changes

- `crates/tui/src/embark.rs`: the state (survey, zoom ladder, cursor, cached overview and report),
  the block classifier, and a pure `draw` like the other screens. The panel is a "CHOOSE A SITE"
  Seal panel with the same section style as the game's.
- `--frame` support: `fn_tui SEED NATION --embark --frame [--keys ..]` opens embark for that seed
  instead of the game. `fn_tui SEED NATION --start X,Y` starts a founded game at a site, for
  scripts and tests. Plain `SEED NATION` is unchanged.
- In the game, `R` (new map, after its y/n) opens embark for a new random seed. The `1`-`4`
  nation switch rebuilds at the world's `start` when it has one, so it doesn't snap back to the
  centre.
- Goldens: `earth_seed7_embark` (120x40 overview), `earth_seed7_embark_small` (80x24),
  `earth_seed7_embark_site` (after `<enter*2>`) and `earth_seed7_embark_refused` (cursor on
  water).

## Hooks into other work

- **#287 difficulty and pressure.** If difficulty is picked before embark (on #280's seed step,
  say), the panel header shows it, and a preset can scale the `[outlook]` thresholds in its own
  data. Starting stores spawn in `World::found`. If #287 adds hidden information to make
  exploring pay, embark must honour it (question 3). Embark decides none of this.
- **#232 population growth.** Newcomers can arrive at `World::start()`.
- **#199 floods.** The report doesn't flag flood-prone land, in keeping with the owner's "no
  advance warning" decision.
- **#280 title screen.** Only its "then starts" step changes, as above. Its acceptance criteria
  and goldens stand.

## Follow-up issues

| Issue | What | Blocked by |
|---|---|---|
| #288 | Sim: `Survey`, the site report and `data/embark.toml` | |
| #289 | Sim: `World::found`, `start` in saves, `Session::found` | #288 |
| #290 | TUI: the embark screen, `--embark` and `--start`, in-game `R` and nation switch | #280, #288, #289 |

## Open questions for the owner

1. **Should map size be a choice?** *Recommended: not now.* Keep 192x128. The overview already
   scales to any size, so Small/Medium/Large later is a row on #280's seed step (size and seed
   together define the map) plus a `MapGenConfig` preset in data.
2. **Should the report give a verdict (Easy/Fair/Hard) or only the facts?** *Recommended: a
   one-word outlook plus the fact that decided it.* The playtest found the game too easy, and a
   visible "Hard" invites choosing a harder start. Facts alone are more neutral but slower to
   read.
3. **How much should embark reveal?** *Recommended: everything a new game would show anyway.*
   The game has no fog of war, so the 1:1 view shows resources and the report counts them.
   Revisit if #287 adds hidden information to reward exploring. Embark would then show terrain
   and biomes only, beyond what the settlers could know.
4. **Can the player settle a poor site (no food or wood within 10 tiles, water far away)?**
   *Recommended: yes, with the outlook saying Hard and why.* Only impossible sites are refused
   (water, the edge, a tiny island, no room for the settlers).
