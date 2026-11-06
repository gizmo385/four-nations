# Agents at the controls: playing and racing with a player's capabilities

Status: **draft** (#313). Owner decisions are pending: see "Open questions for the owner".

The owner wants AI agents to play The Four Nations, race each other, and be watched doing it.
The rule that makes this worth doing is **capability parity**: an agent sees nothing a human
player can't see and does nothing a human player can't do. The interface doesn't have to be the
literal screen and keyboard (owner, 2026-11-04). A structured API is fine as long as parity
holds, and this doc's main job is to show how parity holds by construction and how it is tested.

## Decisions in brief

1. **A structured API over the TUI's own state, with keys underneath.** Agents get JSON
   observations built from the same `fn_ui` view models and map cells the TUI draws, and named
   actions that are **lowered to key presses** and fed to the same `Shell::key` a human's
   keyboard feeds. An agent action can't do anything a key can't, because it *is* keys.
2. **One host, two front doors.** `fn_tui::surface::Host` (pure, with no I/O and no wall clock)
   wraps a `Shell`. A new `fn_agent` binary serves it to agents as an **MCP server over stdio**
   and writes a run log. `fn_sim` is untouched, so purity rule 4 holds trivially.
3. **Lockstep time in fixed turns.** The sim stands still while an agent thinks. `end_turn`
   advances a fixed slice of game time (20 game seconds by default, set in the race data), so
   every racer gets the same sim time per decision. The host owns the clock: agents can't pause
   or change speed, and those keys are refused.
4. **Speed is a host setting and changes no outcome.** Headless, a turn runs as fast as the CPU
   allows. When someone is watching, the watcher picks its own pace. Because time is lockstep,
   pace never changes what happens in the game.
5. **Watching means replaying the log in the normal TUI.** `fn_tui --watch run.jsonl` follows a
   run live, or replays it afterwards, by feeding the logged keys into a real `Shell`. Every
   tool banner, cursor walk, drag rectangle, designation, selection and alert appears as if a
   player made it, and an **agent strip** names each action and the agent's one-line reason.
6. **Races are data.** `data/races.toml` presets fix the seed, nation, map size, difficulty,
   site, turn length, limit and goal, and goals reuse the endeavor's sign kinds. The first
   preset is **"First to ten"**: reach ten people on Normal.
7. **Results are verifiable.** A run log is a seed plus a key script, so `fn_agent verify`
   replays it and checks the claimed goal tick and final world digest. A privileged
   **referee** reads `World` to judge the goal, but none of what it reads goes into an
   observation.
8. **Parity is tested three ways:** action round trips (named action = its key script),
   observation containment (every value an agent sees also appears in some rendered screen),
   and map-cell equality (the agent's map equals the frame's cells). The racers themselves run
   sandboxed, with the game's MCP tools only.

## The parity rule

**An agent may see** what some screen of the TUI shows, reachable without changing game state:
the main view (map, panel, banner, bar), the Nation (`N`), Research (`T`), Endeavor (`G`), log
(`L`) and help (`?`) screens, and the Cursor section for any tile (in a terminal, "hover" means
moving the cursor there). Scrolling is free for a human, so the agent may look at any part of
the map. There is no in-game fog today.

**An agent may not see:**

- `World` internals: RNG streams, job queues, paths, exact timers, strain, or anything else the
  tooltips and screens don't print;
- in embark (later), anything the survey report hides, such as a rough survey's true values;
- the future: no lookahead copies of the world, which is why racers are sandboxed;
- other racers' runs or logs.

**An agent may do** what keys do, through `Shell::key`. **It may not** run ticks, change the
clock, or issue a `Command` itself.

**Parity goes both ways.** If agents need a convenience that changes what a player can learn or
do, it ships in the TUI first. Input efficiency doesn't count: "designate this rectangle" spares
the agent the cursor walk, but the walk still happens as keys in the log.

## Frames and keys, or a structured API

The owner's first idea was the literal screen. Both options were weighed:

| | Frames + keys | Structured API, lowered to keys (chosen) |
|---|---|---|
| Parity | Trivial, since it *is* the UI | By construction: view models in, keys out. Tested below |
| What a text frame loses | The cursor, the drag rectangle and designation tints are style-only, so `--frame` text drops them (see below). ASCII glyphs collide: `F`, `*`, `$`, `%`, `=` and `S` each mean two or more things, which only colour tells apart | Each map cell carries its thing, its ground and its marks: the three channels a human reads from glyph, background and tint |
| Cost per look | A 120x40 frame is about 5k characters, and most of it is unchanged map | A status read is small, and map regions are fetched on demand |
| Acting | Count cursor steps, track modal state | "designate harvest from A to B", and the host plans the keys |
| Churn | The side panel is being restyled (#309, #310), so agents' prompts break | Reads view models, not panel layout |

A frame of seed 7 after `<tick 1200> h <left*8> <up*4> <enter> <right*5> <down*3>` (the Harvest tool
mid-drag) shows the banner, but the 6x4 rectangle and the cursor aren't in the text at all:

```text
= HARVEST = Enter: designate 6x4 - Esc: drop the corner ============+----- * EARTH KINGDOM * ------+
"FF""",,,,,,,,,,,,c,,^^^,T,,T,,,,,..~~~~~~~..,,,T,,.~..~~~.",""""""F| Seed 7                 02:00 |
...
~~~~~~~~.F,,,TTTTTTT,YY,,T,,,,,,T.~~~~~~~~~~~~~.,,,,,....===F"F"FFFF| Next: 10 more people, 6 mor~ |
~~~~~~~~.F,TT,T,T,,TT,,,,,,,TTT,T.~~~~~~~~~~~~..,,,T..~~=,""=F"==FFF|                              |
```

The rendered frame stays available as an observation (`frame`), as a parity escape hatch and
for UI-readability experiments. A race preset may restrict a run to frames and keys only.

## Architecture

```text
 racer (LLM, sandboxed)          fn_agent serve (stdio MCP)           run.jsonl
   tools/call  ───────────────▶  fn_tui::surface::Host  ──────────▶  header, acts, turns,
   ◀─────────── JSON results       ├─ Shell (title/embark/game)        result
                                   ├─ referee (reads World: goal,          │
                                   │  digest; never in a result)           ▼
                                   └─ lowering: action → KeyEvents    fn_tui --watch run.jsonl
                                                                      (normal TUI, replays keys)
```

- **`fn_tui::surface`** (a new module in the `fn_tui` lib). It lives in `fn_tui` because it needs
  `Shell`, `UiState`, the map's cell looks and `frame::render_shell`, and because lowering an
  action to keys has to change whenever a screen's navigation changes. It does no I/O and reads
  no clock, and tests drive it the way they drive `--frame`.
- **`crates/agent` (`fn_agent`)**, a binary crate depending on `fn_tui`, `fn_ui` and
  `serde_json`. It holds the MCP transport (JSON-RPC 2.0 over stdio: `initialize`, `tools/list`,
  `tools/call`, hand-rolled and roughly 200 lines with no SDK), the race runner, the log writer,
  `verify` and `keys`. A scripted, non-LLM bot speaks the same protocol, which is how CI tests
  it.
- **Saves are off** in agent sessions (the `Shell` gets no save directory), and so are `R`
  (new map), `1`-`4` (switch nation) and `q`.

## Observation

MCP tools that read. They're free, change nothing, and take no game time:

| Tool | Returns | Built from |
|---|---|---|
| `briefing` | The race's rules and goal text, turn length, limit, calls left, and the key help | `races.toml`, `input::help_rows` |
| `observe` | Turn, clock, season, tool, notice, people (activity counts, need flags), research, stores, endeavor, cursor tile, and alerts since the last turn | `side_view`, `activity_counts`, `need_flags`, `tooltip_lines`, the `Alerts` history |
| `look {x,y,w,h}` | A map region, at most 64x40 | The TUI's per-cell look (below) |
| `inspect {x,y}` | That tile's Cursor-section lines | `tooltip_lines`, as if the cursor were there |
| `screen {name}` | `nation`, `research`, `endeavor`, `log`, `help` or `build`, as data | `nation_view`, `research::screen_view`, `endeavor::screen_view`, `help_rows`, `build::picker_rows` |
| `frame {w,h}` | The rendered frame as ASCII text | `frame::render_shell` |

**The map.** `look` returns three layers, which are what a human reads from glyph, background
colour and tint:

```json
{"x": 84, "y": 58, "w": 10, "h": 4,
 "things": ["TT,T,,TT,,", ",,,,T,,,,T", "T,a,TTT,,,", ",,,,T,,c,,"],
 "ground": ["pppppppppp", "pppppppppp", "pppppppppp", "ppppppppff"],
 "legend": {"T": "oak tree", ",": "-", "a": "settler (Tyro)", "c": "berry bush",
            "p": "plains", "f": "forest floor"},
 "marks": [{"kind": "harvest", "x": 86, "y": 60, "w": 6, "h": 4},
           {"kind": "cursor", "x": 91, "y": 63}]}
```

A response's legend only lists what's actually in it, so there are no collisions within one, but
the character behind a description is fixed for the whole session (#332): once something has a
character it keeps it, whatever else later shares a region with it, so the legend reads the same
call after call instead of depending on what else that particular region happened to contain.
The meanings come from the same look the painter uses, so the agent's map can't show something
the screen hides. Today's painter (`map.rs`) works out glyph, colours and modifiers in one pass.
Extracting a pure `cell_look(...) -> CellLook { thing, ground, marks }` that the painter then
styles is a prerequisite.

**`observe`** (abridged):

```json
{"turn": 7, "turns_left": 353, "clock": "02:20", "season": "Spring",
 "tool": "Harvest", "notice": "Designated 9 tiles to harvest",
 "people": {"total": 6, "activity": {"Harvest": 4, "Idle": 2}, "needs": {}},
 "research": {"label": "Masonry", "pool": "", "rate": ""},
 "stores": [], "endeavor": {"met": 0, "of": 9, "next": ["10 people"]},
 "cursor": {"x": 91, "y": 63, "lines": ["Oak Tree", "Plains - Land (91, 63)", "Chop: 4 Wood - 8s"]},
 "alerts": []}
```

Text fields are the view models' own strings, the ones the TUI prints. That is what makes the
containment test below possible.

## Action

`act {steps: [...], note?}` runs its steps in order and returns the keys it pressed and the
resulting `observe`. No game time passes. A step is one of:

| Step | Lowered to |
|---|---|
| `designate {tool, from, to, standing?}` with tool `harvest`, `cancel`, `stockpile` or `bend` | Esc to Select mode if needed, the tool's letter, `o` for a standing order, a cursor walk to `from` (Shift for runs of 10), `Enter`, a walk to `to`, `Enter` |
| `build {building, from, to}` | `u`, the Build modal's navigation to `building`, `Enter`, then as `designate` |
| `select {x,y}` or `select {name}` | A cursor walk and `Enter` (again, until that unit is selected), or `N`, row navigation and `Enter` |
| `research {tech}` | `t`, arrow navigation to the tech, `Enter`, `Esc` |
| `cursor {x,y}` | A cursor walk |
| `keys {script}` | The script as written, in `--keys` syntax, without `<tick>` |

The planner reads `UiState` (the cursor, open screen and tool) to plan keys. If the plan can't
be made (an unknown tech, a building the modal doesn't offer, a tile off the map), the step
fails with the reason and nothing is pressed. `note` is the agent's one-line reason (80
characters at most), shown to watchers and logged, and it never affects the game. The result's
`notices` carries each `designate` or `build` step's own notice (the "N done, M skipped:
reason" text a player sees in the Tool section), one entry per step that left one, so a
mostly-skipped rectangle is obvious right away instead of only in a later `observe` -- and
never masked by a later step in the same call overwriting the one live UI notice.

**Refused keys:** `Space`, `+`, `-`, `.` (the host owns time), and `R`, `1`-`4`, `q`, `F5`,
`F6` and `F9`, plus the Esc menu's Save, Load and Quit (no saves, no new maps, no quitting
mid-race). Refusing is always parity-safe. Named actions never need an F-key, so the WSL gap
doesn't matter, and raw `keys` may still name one. A bare `Esc` with nothing open to close is
refused too: for a player it opens the game menu, but every row there except Resume and Key
help is already refused above, so for an agent the menu is only a trap, never a tool.

## Time: lockstep turns

- The game clock isn't driven by real time in an agent session. `end_turn {turns = 1}` runs
  `turns × turn_seconds × TICKS_PER_SECOND` ticks through the same path `<tick N>` takes
  (`App::run_ticks`), then returns `observe`.
- **It stops early** at the end of the first turn in which a new alert lands in the log (a
  death, an arrival, a finished tech, a met sign) or a need flag first shows. Stops stay on the
  turn grid, so racers' decision points line up. A human sees those same alerts as toasts the
  moment they land. This doesn't tell the agent anything new; it only stands in for a human's
  continuous attention.
- **Fairness:** every racer gets the same game time per decision, and nobody gains by thinking
  faster. Pausing to think is something a human can always do, so free thinking time is within
  parity.
- **Speed** is the host's (and the watcher's) wall-clock pace for running turns: as fast as
  possible headless, or a chosen multiple of real time when watched. It's never an agent
  capability, and since turns are lockstep it can't change an outcome. For two-way parity,
  humans get the same fast steps as a host setting (`fn_tui --fast` adds x10, x20 and x50 to
  the `+`/`-` ladder, #321).
- **Determinism:** a run is a seed plus keys and `<tick N>`s. `fn_ui::Clock` and the wall clock
  are never consulted. Toasts don't age in the host, and observations read the alert history,
  not toasts.

## Watching

**`fn_tui --watch run.jsonl [--pace x40]`** is the normal TUI that a human runs in their own
terminal. It reads the log's header, builds the same `Shell`, and replays events. If the log is
still growing, it follows it live, and if the run is finished, it replays it. Two racers side by
side means two panes (tmux, or Windows Terminal's split), each watching its own log. Racers run
headless at full speed, and watching never slows them.

**"Visible" means:**

- **The real keys, animated.** Each act's keys play at a readable rate (about 20 keys a second;
  a walk longer than 2 seconds is compressed into Shift-steps). The watcher sees the tool banner
  (#307) appear, the cursor walk, the drag rectangle grow in the tool's hue, the designation
  tint land, the Build ghost, the research screen open and its selection move, the selection
  bar open.
- **An agent strip** replaces the bottom key-hint bar (a watcher's keys are watcher controls).
  It shows the racer, turn, game clock, the current action's label, the agent's note and the
  watch pace.
- **The message log** (`L`) gains an Agent entry for every act, so a replay can be read back
  afterwards.
- **Attention ghosts:** `look` and `inspect` are logged too. The watcher briefly outlines the
  looked-at region or the inspected tile, so a human sees what the agent was reading.
- **Turns** play at the watch pace too, like the normal game clock: x1 is real time, x5 is five
  game seconds a real second, and so on (#328), with the render throttled to 30 fps. A turn no
  longer lands the instant it's reached -- that froze the screen while the agent thought, then
  flashed the whole turn by at once. When the watcher catches up with a live log, the strip
  reads "waiting for agent (12 s)"; when the agent writes turns faster than the chosen pace
  plays them back, it reads "3 turns behind" instead, so a human watching knows to bump the
  pace (`+`).
- **The end marker** (#327): `fn_agent serve` writes an `end` line (the world's own tick, and
  the reason -- stdin closed, or an error) when its session ends. Once the watcher replays it,
  the strip reads "run finished" instead of ever counting up a wait that will never end. A log
  with no `end` line (one from before #327, or still live) follows exactly as before.

Watcher keys: `Space` pauses the replay, `+`/`-` change the pace, `.` steps one action, `]`
jumps to the end of the next turn, and `q` quits. These win over the game's own keys if they'd
otherwise clash (#330): the watcher's meaning always comes first.

**Free-look spectating** (#330, `crate::spectate`): the rest of the game's own keys move a
second, separate `UiState` -- its own camera, cursor, selection and open screen -- over the same
read-only `Session`, never the one the replay drives. `V` (not an F-key: the owner's WSL
terminal never sees those) switches between **Follow** (the agent's own view, as above) and
**Free look** (the spectator's own); moving the spectator's camera or cursor, selecting a unit,
or opening a screen switches to Free look on its own, since otherwise its effect would never
show. In Free look the agent's own cursor and the attention ghosts still outline on the map, so
a spectator looking elsewhere can still see what the agent is doing. Every read-only screen and
modal opens for browsing (Nation, Research, Endeavor, the log, key help, and the Build list,
browse-only); every mutating action -- a designation tool, `Enter` that would place a building
or choose a tech, cancel, saves and loads, a new map, a nation switch, and the game's own
pause/speed/quit -- is refused outright (`input::spectator_allowed`, exhaustively judged by
`Action`, so a new one fails to compile unclassified). The strip says which view is on
(`FOLLOW`/`FREE`). The hard constraint: nothing a spectator does ever reaches the replay's own
`Session` -- proved by replaying the same log twice, once untouched and once with a spectator
doing everything it can in between every step, and comparing the final command log
(`fn_sim::world::command_log`), which comes out identical either way.

Mockup (a real `--frame`, with the bottom row replaced by the strip):

```text
= HARVEST = Enter: designate 6x4 - Esc: drop the corner ============+----- * EARTH KINGDOM * ------+
"FF""",,,,,,,,,,,,c,,^^^,T,,T,,,,,..~~~~~~~..,,,T,,.~..~~~.",""""""F| Seed 7                 02:00 |
F""FF"F,,,,,,,,,,,,,,^^^,T,=T,,,,,.~~~~~~~f~..,,,T,.....~..",c""F"""| > Running x1      Space  + - |
"""Y""FY""YT,T,,,,,,,,o^,,,,=,,,,..~~~~~~~~~~.,TT,,T,,,.~.,F,,,"""""|                              |
FFFF""F""F"F,,,,,,,,,o^^,TT,,=,,..~~~~~~~~~~~..T,,,,,...~.,,F,"YF"FF| o PEOPLE                   6 |
FFYFF"F"""""",,,,,,,,^^^,,,...=..~~~~~~~~~~~~~..TT,...~~~.YF"""F"F"F|  Tyro Idle                   |
""FF"""""Y"FFF",,,,,,^^,,,,.~~~~~~~~~~~~~~~~~~~.,c,.~~~~~.FFF"F"""""|  Ping Idle                   |
FF"""YY""""""f,,,,,,,o,,TTT.~~~~~~~~~~~~~~~~~~~.,,,.f~~~~=FFF""F"FF"|  Ming Idle                   |
FFF"""""F"""=F,,Y,,,,,,,T,T...~~~~~~~~~~~~~~~~~.T,,.~~.~~.=FF"F"FFFF|  Oma  Idle                   |
FF"F"""""""="",,,,,,,,,,,,,,,....~~~~~~~~~~~~~..T,,...~~~.F=F""FF"F"|  Song Idle                   |
.=..F"FF""="",,Y,,,,,TY,,,T,,T,T@.~~~~~~~~~~~~.,,,YT,.....FF=FFFF...|  Lin  Idle                   |
~~~...""""=",,Y,,,,,,,,,,,,,,,,TT@~~~~~~~~~~.~.,,TTTTT.~.TFFF=="F.-.|                              |
~~~~~..""=c",T,,c,,,,,,,,,,,T,TT,..~~~~~~~~....,,,T,TT...TFFFFFf==-.| o RESEARCH                   |
~~~~~~..=""F,T,,,,,Y,,,,,,,T,T,T,,.~~~~~~~~~..T,TTT,,,,T,T,FFFFF"..=| No target set        3.0/min |
~~~~~~~=."""T,,,,,,,,,,,,Y,c,TTT@,.~~~~~~~f.~.,,,,T,,,,TTTTTFFF"FF"F| 6 pts banked                 |
~~~~~~~~..",TT,,,,,T,T,T,,Y,,,,T,Y.~~~~~~~~~..,,,,,,,TT,,T,,F"FFFF"F|                              |
~~~~~~~~~.",,T,TT,,,,,T,,T,,,,,,@,.~~~~~~~~~~..,TT,T,TTT,TT,,T,Y"FFF| o ENDEAVOR                   |
~~~~~~~~~."T,T,,,T,,T,,,T,TTT,TT,,..~~~~~~~~~~..,,,T,,T,TTT,TTFTT,FF| The Harmony Gate         0/9 |
~~~~~~~~.."T,,,,,,,,,,,T,,,,,T,T,T@~~~~~~~~~~~~.c,,,,Y,,cTFFFFTFF"FF| ..........                   |
~~~~~~~~.FFTTT,,,,,,,,,,,,T,TT,T,..~~~~~~~~~~~~.,T,T,TTT,FF"F"FFFF"F| Signs met - G: open          |
~~~~~~~~.F,,,TTTTTTT,YY,,T,,,,,,T.~~~~~~~~~~~~~.,,,,,....===F"F"FFFF| Next: 10 more people, 6 mor~ |
~~~~~~~~.F,TT,T,T,,TT,,,,,,,TTT,T.~~~~~~~~~~~~..,,,T..~~=,""=F"==FFF|                              |
~~~~~~~..F,,,T,TT,T,,,,,,TTT,TT...~~~~~~~~~~~~.,,,...~~~.T,""==FF="F| o TOOL           Harvest [H] |
~~~~~~~.FF,,,,T,,,,T,c...T,,,...~~~~~~~~~~~f...,,..~...~..,TcFFF""fF|                              |
~~~~~~~.YFF"F,T,T,,,,..~..T,..~~~~~~~~~~~~~..,,,..f..,..~..,,FFFFF"=| o CURSOR            (91, 63) |
~~~~~~~......,,,,,,Y,.~~~..,.~~~~~~~~~~~~~..,,,..~~.,,T..~.","TTFFFF| Oak Tree                     |
~~~~~~~~~~~~....,,,,,.~~~~.,.~~~~~~~~~~~~~.,,,,.~...,,,,..="""F,,F"F| Plains - Land (91, 63)       |
~~~~~~~~~~~~~~~...,,,.~~~~....~~~~~~~~~f~..,,,,...,,TT,,,,"="""T,T,F| Chop: 4 Wood - 8s            |
~~~~~~~~~~~~~~~~~..,,.~~~~~f~.~~~~~~~~~...,,,,,,,,,,,TT,,T""="FT,,,,+------------------------------+
 AGENT earth-a  turn 6  02:00 > designate harvest 6x4 at (86,60)  "wood for two beds"      >> x40
```

The panel's time row in a watched session should read as a turn counter rather than "Running
x1". Its exact shape follows the panel restyle (#309) and isn't specified here.

**Not chosen: attaching an agent to the owner's open game** over a socket. It ties the agent's
pace to a terminal, needs a socket and a thread inside `run`, makes races depend on someone
watching, and shows nothing the follower doesn't. It's question 4.

## Racing

**`data/races.toml`**, read by `fn_agent` (the sim never reads it, as with `tui.toml`):

```toml
# Races for agents (docs/design/agent-play.md). Every racer in a race plays the same preset.
# - seed, nation, map_size, difficulty: as fn_tui's --start path; site: "default" (the
#   embark default site, Survey::default_site) or [x, y].
# - turn_seconds: game seconds per end_turn. limit_minutes: game minutes before a DNF.
# - max_calls: tool calls before a DNF (bounds cost; reads count too).
# - goal: an endeavor sign (data/endeavor.toml's kind/target/items schema), judged by the
#   referee on every tick of a turn.
[[race]]
id = "first-ten"
name = "First to ten"
brief = "Grow the settlement to ten people. Spare beds and food draw newcomers."
seed = 7
nation = "earth"
map_size = "small"
difficulty = "normal"
site = "default"
turn_seconds = 20
limit_minutes = 120
max_calls = 3000
goal = { kind = "population", target = 10 }
```

- **Why "First to ten" first.** It's the vision's first-playable win and a number the panel
  shows, so the agent can track the goal from its observations. It needs beds, food and
  stockpiles (every early system) through the first winter at minute 45. The Harmony Gate is
  hours long and is for later.
- **Scoring:** the game tick the goal was first met, lowest first. A DNF (limit, call budget,
  or everyone dead) ranks below every finisher, by goal progress at the end and then by fewer
  deaths. Remaining ties go to fewer tool calls.
- **Runs** are separate processes on the same preset. They can run at the same time or one
  after another, since lockstep makes the two the same. `fn_agent race first-ten --log
  runs/a.jsonl` serves one racer.
- **Run log** (JSON Lines): a header (race id, preset copy, data digest = a stable FNV-1a hash
  of the bundled TOML, crate version), then `act` (label, note, keys), `look`/`inspect` (for
  ghosts), `turn` (index, tick), and a `result` (goal tick or DNF, deaths, calls, final world
  digest).
- **`fn_agent verify run.jsonl`** rebuilds the session, replays it, and checks the result and
  digest. It refuses a log made against other data, because replays only hold for the same code
  and data (as `save.rs` explains). **`fn_agent keys run.jsonl`** prints the run as a `--keys`
  script, so `fn_tui 7 earth --start ... --frame --keys "..."` shows any moment of it.
- **Sandboxing racers.** An agent with a shell could run its own copy of the seed and look
  ahead, or read the other racer's log. A racer's harness grants **only the game's MCP tools**:
  no shell, files or web. The race doc gives the recipe, and a racer's prompt is the briefing
  plus a short standard preamble that's the same for everyone.

## Enforcing and testing parity

Rust visibility can't enforce parity on its own, because `Shell` and `Session` are public
within the workspace. So parity rests on a narrow design plus tests:

1. **One way in for actions.** Everything that changes state goes through `Host::press(KeyEvent)`,
   which calls `Shell::key` and logs the key. A **source lint** (xtask, part of `ci --fast`)
   rejects `.apply(`, `run_ticks`, `tick_n`, `.world` and `Command` in `crates/tui/src/surface/`,
   apart from the one `end_turn` function and the `referee` module. In `crates/agent/src/`, it
   rejects any use of the referee in the MCP tool handlers.
2. **Action round trips.** For every named action, a test runs it through the Host and runs its
   expected key script through `Shell::play`, then asserts the same command log and the same
   frame. A test also asserts that refused keys change nothing.
3. **Observation containment.** For golden scenarios (each nation, mid-game, a selected unit,
   the Build tool), a test collects every leaf string and number in `observe` and `screen`, and
   asserts each one appears in the rendered text of the reachable screens (main, `N`, `T`, `G`,
   `L`, `?`, and the Cursor section with the cursor on each referenced tile). A field the TUI
   never prints fails the test. This follows the panel restyle automatically.
4. **Map-cell equality.** For the same scenarios, `look` over the visible view must equal
   the frame's map cells: each cell's thing has that glyph in `--ascii`, and each mark matches
   that cell's mono-palette modifier.
5. **Sandboxed racers**, as above. This is the one parity guarantee a test can't give.

## Slices

1. **Playable by an agent:** the cell-look refactor, the Host with `observe`, `look`, `inspect`,
   `screen`, `frame`, raw `keys` and `end_turn`, the MCP server, and the run log. A Claude Code
   session can play a casual game (`fn_agent serve --seed 7 --nation earth`).
2. **Named actions** and the parity tests.
3. **Races and verify**, with "First to ten" and a scripted bot that runs part of a race in CI.
4. **Watching:** `fn_tui --watch` with the strip, animation and ghosts, and `fn_tui --fast` for
   humans.
5. **Later, not filed yet:** embark as part of a race (the survey report as an observation, the
   rough survey's hidden truth kept hidden), owner-vs-agent races (a TUI key log in real time;
   fairness differs), and taking over a watched run to play on from it.

## Not chosen

- **Frames and keys only.** Parity is trivial, but the text frame drops the cursor, drags and
  designations, costs about 5k characters a look, and churns with the panel. It's still
  available per preset.
- **A privileged command API** (agents issue `Command`s). It skips the UI's gating (the Build
  modal's offer, Research's availability, prompts) and would test nothing about the UI.
- **Real-time races.** These measure model latency rather than play, and they make runs depend
  on the wall clock.
- **Agent-chosen waits instead of fixed turns.** Racers would get different decision grids, and
  "equal sim time per decision" would only hold on average.
- **The host inside `fn_ui`.** It needs `Shell`, `UiState` and the renderer, all in `fn_tui`.

## Follow-up issues

- #314 TUI: extract each map cell's look (thing, ground, marks) from the painter
- #315 Agent surface: `fn_tui::surface::Host` with lockstep turns, observations, raw keys and a
  run log
- #316 Agent surface: named actions (designate, build, select, research) lowered to keys
- #317 `fn_agent`: an MCP server over stdio for the agent surface
- #318 Races: `data/races.toml`, referee, scoring, `verify`, and "First to ten"
- #319 Watch: `fn_tui --watch LOG` follows or replays an agent run with an agent strip
- #320 Parity tests: source lint, observation containment, map-cell equality
- #321 TUI: `--fast` host setting adds x10, x20 and x50 speeds

## Open questions for the owner

1. **Time model.** *Recommended:* lockstep with fixed turns of 20 game seconds (the sim stands
   still while an agent thinks, and every racer gets the same decisions per game minute).
   Alternatives are agent-chosen waits, or real time, where thinking speed matters.
2. **The first race.** *Recommended:* "First to ten" on Normal, Small, seed 7, Earth, the
   default site, a 120-minute limit, every racer the same nation. Alternatives are "survive two
   winters with the most people" (a fixed horizon, scored rather than raced), or one racer per
   nation (more fun, less comparable).
3. **What racers may know.** *Recommended:* in-game knowledge only (the briefing, key help and
   what the screens say), with no reading of `docs/` or `data/`, and harnesses that grant only
   the game's tools. Rules knowledge from the data files would make it a test of the agent
   rather than of the UI.
4. **How you watch.** *Recommended:* a headless host plus `fn_tui --watch run.jsonl`, the normal
   TUI replaying the run live, any number of panes, and replay afterwards for free. The
   alternative is an agent attaching to a TUI you have open over a socket, which is more
   literal but couples the race to your terminal.
