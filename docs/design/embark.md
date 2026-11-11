# Embark: choosing where to settle

Doc status: current

Status: **accepted** (#286, owner answers 2026-11-03, recorded at the end). Follow-up work: #288,
#289 and #290.

Today mapgen makes the map and `World::new` drops six settlers on the walkable tiles nearest the
map centre, wherever that is. On seed 7 the centre, (96, 64), is open sea, so the settlers land on
a strip of coast with the sea filling half the area around them and no stone within 10 tiles. The
owner's playtest asked for Dwarf Fortress's embark instead: see the whole map, zoom in, and choose
the site. This doc designs that step for the TUI, the only frontend from now on (#283).

## Decisions in brief

1. **One more step in #280's New game flow.** Title → New game → nation → seed, map size and
   survey → **embark** → game. Embark opens with the site box on a default site, so `E` `y`
   starts the game straight away. Passing `SEED NATION` on the command line still skips every
   menu, embark included.
2. **Map size is a choice.** Small (192x128, today's map), Medium (288x192) and Large (384x256),
   as presets in `data/embark.toml`. Small is the default, so nothing changes unless the player
   picks a bigger map.
3. **Zoom levels, computed from the terminal.** A deliberately coarse *overview* that fits the
   whole map, then 1:4, 1:2 and the *site* view at 1:1, which is the game's own map drawing. The
   overview shows the shape of the land and not much else. Detail arrives as you zoom in: water
   at 1:4 and 1:2, resources at 1:1.
4. **A site is a 21x21 box around the cursor.** The side panel reports on it live: the land, room
   to roam, the walk to water, counts of wood, stone, food, gold and scrolls, and a one-word
   outlook. The sim computes all of it in a pure `Survey::report(pos)`.
5. **An optional rough survey.** A New game toggle, *Survey: Exact / Rough* (Exact by default).
   A rough survey stops zooming at 1:2, shows resources as estimated levels that can be one step
   wrong, and hides gold and scrolls. Uncertainty applies only to choosing; once settled, the game
   shows everything as usual.
6. **The sim surveys first, then founds.** A new `fn_sim::embark::Survey` holds a generated map
   (with buried resources already hidden) and answers reports. `World::found(survey, nation,
   start, ..)` builds the world from it. Mapgen never sees the start, so a map is still a
   function of its seed and size alone.
7. **Deterministic, with no churn.** A founded world is a function of (seed, size, nation, start,
   data). The start goes into the world as `start: Option<Pos>`, so saves and replays have it.
   Today's `World::new` path is unchanged byte for byte, so no golden needs reblessing.
8. **Only impossible sites are refused.** A site must be open land, away from the edge, and on a
   landmass big enough to live on. A poor site (no food, water far away) is allowed with a warning,
   because a hard start is a legitimate choice (#287).

## Flow and keys

```
Title ─ New game ─▶ Nation (1-4) ─▶ Seed, size, survey ─▶ EMBARK ─E,y─▶ game
                                        ▲              │
                                        └──── Esc ─────┘  (at the overview; the seed is kept)
```

#280 ends the seed step with "then starts". Embark replaces that "starts": Enter on the seed step
opens embark for that seed and nation instead of building a `Session`. The seed step gains two
rows under the seed, moved between with up/down and changed with left/right: **Size** (Small,
Medium, Large, from `data/embark.toml`, default Small) and **Survey** (Exact, Rough, default
Exact). Each row shows a one-line hint, e.g. "Rough: you won't know exactly what's there until
you arrive." Nothing else in #280 changes. Whatever #280 names its pre-game states, embark is one more state after the seed one,
holding a `Survey`, the zoom level and the cursor.

| Key | In embark |
|---|---|
| Arrows, `W` `A` `S` `D` | Move the cursor one cell, which is N tiles at 1:N. Shift moves 10 cells |
| `Enter`, `+` | Zoom in on the cursor. At the closest zoom (1:1, or 1:2 in a rough survey), settle here (asks y/n) |
| `-`, `Esc` | Zoom out. `Esc` at the overview goes back to the seed step |
| `E` | Settle at the cursor from any zoom (asks y/n: "Settle at (88, 44)?") |
| `R` | A new random seed: a new map, back to the overview, cursor on its default site. No prompt, because nothing is lost |
| `?`, `Q` | Key help, and quit (asks for the title screen or out, with its keys in the hint bar's `EXIT` mode) |

The y/n prompt is asked on the hint bar, in its `SETTLE?` mode (`Y settle here · Any other key stay`, #686);
settling on a refused site shows the reason as a notice instead of the prompt. The embark and title
screens name their keys there and nowhere else: `EMBARK` (by zoom), `TITLE`, `NATION`, `SEED` and `LOAD`
are modes of the `fn_ui::keys` table, and no border or box prints them.

`+` and `-` change speed in the game. Embark has no clock, so they're free to zoom.

## The zoom levels

All mockups are 80x24, the smallest supported terminal. Map glyphs are shown in the `--ascii`
set, because sea and lake differ only by colour in the Unicode set. `~` is sea, `-` lake, `=`
river, `^` mountain, `,` plains, `"` forest, `:` desert, `*` tundra, `&` taiga and `%` swamp.
The cursor (reverse video) and the tinted site box don't show in plain text. The four corner
marks do.

### Overview (fits the map)

The mockup below predates the owner's call for a less detailed overview: it still singles out
rivers and lakes (`=`, `-`), and its panel shows the full report. Under the rules that follow,
fresh water folds into the land around it, and the panel stops after the Land section. The
#290 goldens are the reference.

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
 EMBARK  Arrows move · Enter zoom in · E settle · R new seed · Esc back          ? keys
```

**Scale.** The overview scale N is the smallest whole number, and at least `min_overview_scale`
(6, in the `[embark]` table of `data/tui.toml`), for which the map fits the map region (the
terminal minus the 32-column panel and the status line): `ceil(W/N) <= cols` and
`ceil(H/N) <= rows`. The floor keeps the overview coarse on big terminals, so it is always a
sketch of the land and never a near-1:1 map.

| Map | 80x24 | 120x40 | 200x60 |
|---|---|---|---|
| Small 192x128 | 1:6 | 1:6 | 1:6 |
| Medium 288x192 | 1:9 | 1:6 | 1:6 |
| Large 384x256 | 1:12 | 1:7 | 1:6 |

The overview is centred in the region. One cell covers NxN tiles, so it has the same proportions
as the game map (both are stretched tall by the cell shape). Any map size works with no special
cases.

**What a cell shows.** The overview is deliberately low on detail: it answers "where is the land,
and what kind" and leaves "where is the water, and what's on it" to closer zooms. Each block of
tiles is classified in this order, and the first rule that matches wins:

1. Sea is at least half the block → sea.
2. *(1:4 and 1:2 only, not the overview)* Fresh water (river, lake or ford) covers at least
   `max(1, N/2)` tiles → river or lake, whichever is more common. Rivers are one tile wide and
   would vanish in a plain majority vote, but at these zooms water is the thing a player is
   looking for.
3. Mountain is at least a third of the block → mountain.
4. Otherwise → the most common biome among the land, coast and fresh-water tiles (fresh water
   counts as the biome it sits in), with ties going to file order.

The cell takes that class's glyph and colours from `data/tui.toml`, so `--ascii` and
`--color never` work unchanged. The thresholds live in the same `[embark]` table, because they're
presentation, not rules. Resources aren't drawn above 1:1, because single glyphs would be too
sparse to read. Each view is computed once per map and scale, and cached.

**What the panel shows at each zoom.** At the overview, only the header and the Land section
(biome mix, mainland, room to roam): the site box there covers a few cells and a resource count
would be false precision. At 1:4 and closer, the full report: water, the resource section and
the outlook. The cursor's site is the same at every zoom, so zooming in on a promising patch of
land is how a player reads its report.

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
 EMBARK  Arrows move · Enter zoom in · - zoom out · E settle · R new seed       ? keys
```

The same classification at N=2, with a view that scrolls to follow the cursor the way the game's
camera does. This is the level that matters on small terminals: at 80x24 it shows 96x46 tiles,
enough to compare neighbouring sites. A 1:4 level with the same rules sits between it and the
overview, so the step down from a 1:12 Large overview isn't a jump. The zoom ladder is
`[fit, 4, 2, 1]`, dropping any fixed level at or above `fit`, and dropping 1 in a rough survey.

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
 EMBARK  Arrows move · Enter or E settle · - zoom out · R new seed               ? keys
```

This is the game's own map renderer (`map.rs`) over the survey's map, with no units and with the
site box drawn on top. Buried gold seams (`!` in the mapgen ASCII) aren't on it: `Survey` hides
them exactly as `World::bury` does today. At 1:1 the 21x21 box plus its corner marks is 23 rows,
which is exactly the map region at 80x24. That is why the report radius defaults to 10.

**The site box.** At every zoom the box's cells take the accent tint that dragged rectangles use
(reverse and dim without colour), and a corner mark `┌ ┐ └ ┘` sits just outside each corner. The
marks hide four cells and make the box visible in text goldens and in mono. On a refused site
the marks and tint turn to the `bad` tone.

### Rough survey

With *Survey: Rough*, the player chooses from a scout's report instead of a map:

- **No 1:1 zoom.** The ladder ends at 1:2, so individual trees, stones and fish aren't visible.
  Terrain (land, water, mountains, biomes) stays accurate at every zoom: the uncertainty is about
  what's on the land, not where the land is.
- **Estimated levels, not counts.** The resource section shows each group's level ("Wood:
  plenty") without a number. Each level is the true level moved one step up or down with
  probability `rough.error_chance` (data), clamped at none and plenty. The jitter is seeded from
  the map seed, the site's position and the group id (`rng::stream` under `embark.rough`, keyed
  by those), so a site always reads the same, however the cursor got there.
- **Rumours only for gold and scrolls.** Groups marked `rough_hidden = true` (gold and scrolls)
  read "?" in a rough survey.
- **Water is near or far.** The walk to water reads "near" (within `water_far`) or "far", with no
  step count.
- **A rough outlook.** The outlook is computed from the estimates, so it can be wrong too, and it
  reads "Fair (rough guess)".

Once settled, the game shows everything as it always does. There's no fog of war; the surprise
is in what the scouts got wrong. A rough survey changes nothing about the world that gets
founded, only what the embark screen tells the player.

A refused site replaces the outlook section with the reason (refusals are always exact, rough or
not):

```
│ ◇ OUTLOOK                    │
│ ✗ Can't settle: open water.  │
```

## The site report (sim)

`Survey::report(pos, mode, defs) -> SiteReport`, where `mode` is `SurveyMode::Exact` or
`SurveyMode::Rough`, is pure, cheap (at most 441 tiles plus a walk capped at 40 steps) and runs
on every cursor move. Exact mode draws no randomness. Rough mode draws only from the keyed
`embark.rough` stream described above, so it is deterministic too.

| Field | What it is |
|---|---|
| `problem: Option<SiteProblem>` | Why you can't settle here: `NotLand`, `NearEdge`, `TooSmall`, `NoRoom` (fewer open tiles than settlers near the start) |
| `biomes: Vec<(BiomeId, u8)>` | The share of the box per biome, plus mountain and water, largest first. The panel shows the top four |
| `room: u32`, `mainland: bool` | Open tiles reachable from the start (`Regions` under the default `MoveRules`), and whether that is the largest landmass |
| `water: Option<(Terrain, u32)>` | The nearest drinkable tile by walking steps, and its kind. Units drink from any water (`Terrain::is_water`), the sea included |
| `groups: Vec<GroupCount>` | Per resource group from data: the count in the box *that touches the start's own region* (8 neighbours; the same "reachable" rule the supply survey uses, #396) and its level (none, scarce, some, plenty). A deposit across water or behind a sealed-off pocket of the box doesn't count; it was never truly in reach. Rough: no count, the estimated level, or unknown for `rough_hidden` groups |
| `outlook: Outlook`, `reasons`, `rough: bool` | Easy, Fair or Hard, and the one or two facts that decided it. Rough: computed from the estimates |

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

# Map size presets for #280's New game flow. The first is the default and matches
# MapGenConfig::default(). continent_frequency, if set, overrides mapgen's so a bigger map gets
# more landmasses instead of the same ones scaled up; #288 tunes it.
[[map_size]]
id = "small"
name = "Small"
width = 192
height = 128

[[map_size]]
id = "medium"
name = "Medium"
width = 288
height = 192

[[map_size]]
id = "large"
name = "Large"
width = 384
height = 256

[rough]
error_chance = 0.35       # chance each group's estimated level is one step off

# Each group is counted inside the box, among the tiles that touch the start's own region
# (#396: a deposit across water or a sealed border isn't actually reachable, so it doesn't
# count just because it's in the box). levels = [scarce_from, some_from, plenty_from].
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
rough_hidden = true       # reads "?" in a rough survey

[[group]]
id = "scrolls"
name = "Scrolls"
resources = ["ancient_scroll"]
levels = [1, 1, 3]
outlook_weight = 0
rough_hidden = true

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
pub enum SurveyMode { Exact, Rough }
pub struct Survey { cfg, surface: TileMap, buried: BTreeMap<Pos, ResourceId>, regions: Regions }
impl Survey {
    pub fn new(cfg: &MapGenConfig, defs: &Defs) -> Self;   // mapgen::generate, then hide buried
    pub fn config(seed: u64, size: &MapSizeDef) -> MapGenConfig; // a preset over the defaults
    pub fn map(&self) -> &TileMap;                          // what frontends draw
    pub fn default_site(&self, defs: &Defs) -> Pos;         // the valid site nearest the centre
    pub fn report(&self, at: Pos, mode: SurveyMode, defs: &Defs) -> SiteReport;
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
  mapgen goldens untouched. `R` costs one generation: measured with our crates at opt-level 1,
  0.12 s for Small, 0.27 s for Medium and 0.48 s for Large.
- **Bigger maps and tick cost.** Mapgen scales fine, but some systems may scan the whole map each
  tick. #289 measures ticks per second on Large against Small headless and files a follow-up if
  Large is more than 2x slower per tick. The TUI only draws the viewport, so it doesn't care.
- **Spawning.** `found` puts the settlers on open tiles (walkable, no resource) in the start's
  region, nearest first by squared distance and then by position, within `spawn_radius`. It draws
  no randomness, and the name and variant streams (`world.names`, `world.units`) are untouched, so
  the same seed gives the same people wherever they settle. #287's starting stores, if it adds
  them, go down at the start here too.
- **No churn for the old path.** `World::new` keeps today's centre spiral and leaves
  `start` as `None`, which isn't serialized (`skip_serializing_if`). The one change (#582): the
  spiral keeps only tiles of one connected region, the first landmass of `min_room` tiles it
  meets, because on a forested map the nearest open tiles can be sealed pockets (trees block
  walking) and a settler born in one starved beside full stores. Maps with no such pocket spawn
  exactly as before. Every mapgen, world, save and TUI golden stays as it is. Moving the CLI path onto `found(default_site)` would improve
  seed 7's start but rebless many goldens while #212, #239 and #284 are in flight. It's a later
  cleanup and is not filed.
- **Saves.** The map's size is already in the saved `TileMap`. `start` is an additive `#[serde(default)]` field, so there's no `SAVE_VERSION`
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
  scripts and tests. `--size small|medium|large` and `--rough` apply to both. Plain
  `SEED NATION` is unchanged.
- In the game, `R` (new map, after its y/n) opens embark for a new random seed. The `1`-`4`
  nation switch rebuilds at the world's `start` when it has one, so it doesn't snap back to the
  centre.
- Goldens: `earth_seed7_embark` (120x40 overview), `earth_seed7_embark_small` (80x24),
  `earth_seed7_embark_site` (zoomed to 1:1), `earth_seed7_embark_refused` (cursor on water),
  `earth_seed7_embark_large` (Large map, 80x24 overview) and `earth_seed7_embark_rough` (rough
  survey at 1:2), plus the seed step with its Size and Survey rows.

## Hooks into other work

- **#287 difficulty and pressure.** If difficulty is picked before embark (on #280's seed step,
  say), the panel header shows it, and a preset can scale the `[outlook]` thresholds in its own
  data. Starting stores spawn in `World::found`. If #287 adds hidden information to make
  exploring pay, the rough survey is the natural place for it, and a difficulty preset could
  force it on. Embark decides none of this.
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

## Owner decisions (2026-11-03)

1. **Map size is a choice.** Small, Medium and Large presets, on the seed step. Small, today's
   192x128, is the default.
2. **The report gives a verdict.** A one-word outlook plus the fact that decided it. The
   playtest found the game too easy, and a visible "Hard" invites choosing a harder start.
3. **Embark reveals what a new game would show, with two changes the owner asked for.** The
   overview is less detailed (no rivers or lakes, a minimum scale, and only the Land section in
   the panel). And a *rough survey* toggle adds uncertainty for players who want it. Exact is
   the default.
4. **A poor site can be settled.** The outlook says Hard and why. Only impossible sites are
   refused (water, the edge, a tiny island, no room for the settlers).
