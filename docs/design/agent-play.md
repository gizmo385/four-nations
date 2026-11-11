# Agents at the controls: playing and racing with a player's capabilities

Doc status: current

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
| `observe` | Turn, clock, season, tool, notice, people (activity counts, need flags), research, stores, stockpile capacity, endeavor, cursor tile, and alerts since the last turn | `side_view`, `activity_counts`, `need_flags`, `stockpile_capacity`, `tooltip_lines`, the `Alerts` history |
| `look {x,y,w,h}` | A map region, at most 64x40 | The TUI's per-cell look (below) |
| `inspect {x,y}` | That tile's Cursor-section lines | `tooltip_lines`, as if the cursor were there |
| `screen {name}` | `nation`, `research`, `endeavor`, `log`, `help`, `build` or `bend`, as data. `bend` is the Bend tool banner's content (#586): the abilities the settlement can use now, in `B`'s order, each with the Tool row's caster count and a flag on the one the banner names; a locked ability is not on the banner, so not there. Readable with any tool active, as `build` is | `nation_view`, `research::screen_view`, `endeavor::screen_view`, `help_rows`, `build::picker_rows`, `bend::view` |
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
 "stores": [], "stockpile": {"tiles": 4, "used": 1, "cap": 20},
 "endeavor": {"met": 0, "of": 9, "next": ["10 people"]},
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
| `designate {tool, from, to, standing?, teardown?, ability?}` with tool `harvest`, `cancel`, `clear`, (#342) `deconstruct`, `stockpile`, `bend` or (#333) `sow` | Esc to Select mode if needed, the tool's letter (for `deconstruct`: `e` as many times as it takes to reach the Clear tool's buildings mode, `e e` from any other tool; for `bend` with an `ability`: the bend key as many times as moves it from whichever ability is active now to that one, wrapping through the ones the settlement can currently use, #333), `o` for a standing order (`harvest` only) or (#335) to also withdraw a build site or stockpile tile (`cancel` only; without it `cancel` withdraws only job designations and leaves those alone), a cursor walk to `from` (Shift for runs of 10), `Enter`, a walk to `to`, `Enter` |
| `build {building, from, to}` | `u`, the Build modal's navigation to `building`, `Enter`, then as `designate` |
| `select {x,y}` or `select {name}` | A cursor walk and `Enter` (again, until that unit is selected), or `N`, row navigation and `Enter` |
| `research {tech}` | `t`, arrow navigation to the tech, `Enter`, `Esc` |
| `cursor {x,y}` | A cursor walk |
| `keys {script}` | The script as written, in `--keys` syntax, without `<tick>` |

The planner reads `UiState` (the cursor, open screen and tool) to plan keys. If the plan can't
be made (an unknown tech, a building the modal doesn't offer, a tile off the map, an `ability`
that's unknown, not yet usable or named outside the `bend` tool), the step fails with the
reason and nothing is pressed. `note` is the agent's one-line reason (80 characters at most),
shown to watchers and logged, and it never affects the game. The result's `notices` carries
each `designate` or `build` step's own notice (the "N done, M skipped: reason" text a player
sees in the Tool section -- a `sow` re-designating a plot that's already sown or marked says
so, rather than "not a farm plot"), one entry per step that left one, so a mostly-skipped
rectangle is obvious right away instead of only in a later `observe` -- and never masked by a
later step in the same call overwriting the one live UI notice.

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

Watcher keys: `Space` pauses the replay, `+`/`-` change the pace (x1 up to x500), `.` steps one
action, `]` jumps to the end of the next turn and `}` ten turns, `[` goes back to the end of the
previous turn and `{` ten turns, `>` skips to the agent's next move (past the turns where it does
nothing; inside a script run, to where the script wakes it), and `q` quits. Going back restores
a copy the watcher keeps every 20 turns and replays forward from it, so it is quick even late in
a long run. These win over the game's own keys if they'd
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

**Picking a run to watch (#556).** `cargo xtask agent-run watch` with no run named, in a
terminal, opens the run browser, `fn_tui --runs DIR` (`crates/tui/src/browser.rs`): one row a
folder under `$FN_AGENT_RUNS`, newest first, with the start time, nation, seed or scenario,
difficulty, mode, model, length, score, signs, deaths, cost and state. Enter opens the run in the
watcher above, and quitting the watcher comes back to the same row. The score is the
`fn_agent score` scorecard at the 540-turn horizon (`referee::score`, the same call): a log's own
`score` line if it has one, else a background replay cached in `score-cache.json` in the run
folder (never in `run.jsonl`), so the list opens at once and the cells fill in. `fn_tui --runs
DIR --frame --keys SCRIPT` draws it headless, which is how the tests drive it.

**Campaigns in the browser (#585).** A hill-climb campaign folder is a row of its own, and `l`
or Right opens the attempts under it (`crates/tui/src/runs/campaign.rs` reads the folder). An
attempt's row has its live game (turns, cost, the score it played to) and its clean evaluation
(mean, kept or not); Enter on one with live turns opens it in the watcher. Under an attempt,
opened, are the clean runs of its chosen governor, one per seed with the seed's tier, and Enter
opens one as it would a run: `fn_tui --watch` reads any log file, so `watch PATH/seed-N.jsonl`
works too. An attempt with no live turns (its calls went to `test_governor`, which writes no
log) says so and offers its clean runs. A running campaign shows which attempt is under way and
whether it is playing, testing or evaluating, read from the folder (`stop.json` and the clean
logs mean evaluating; a last `test_governor` call in `run.jsonl` means testing). Both ledger
shapes read: the first one's `clean`/`train_mean`/`held_out` and the seed tiers'
`screen`/`gate`/`owner` with the gate verdict. Lists sort on the date in the folder name, never
on the folder's modification time, which a cache write changes. `watch CAMPAIGN_DIR` opens the
browser on that campaign (`--focus`), and `watch --list` prints campaigns with their attempts.

### Watching on the web (#595)

The same watcher runs in a browser. `crates/web` (`fn_web`) compiles `fn_tui`'s `Watcher` to
`wasm32-unknown-unknown` behind a small C ABI (no wasm-bindgen), and `web/` is a static page
that loads a log, draws each frame onto a canvas and passes keys and real time across. A
replay re-simulates the log exactly as the terminal does, so the page needs only the
`run.jsonl` (a 3 h attempt's is 1-4 MB, its asciicast 40-120 MB), and it shows the same
screen: the studio panes at 160x45 and up, the agent strip, free look.

The terminal watcher's keys work there too (Space, `+`/`-`, `.`, `]`, `0`-`3`, free look),
with buttons for them and a scrubber over turns. Seeking forwards replays a turn at a time
between frames; seeking backwards restores the nearest snapshot (one is kept every 20 turns,
as the replay first reaches it) and replays from there, so it costs at most 20 turns, well
under a second. A log made against other data is refused with the build it was made on.

The scrubber marks the turns worth finding again: deaths, endeavor milestones (a sign met, the
endeavor Ready, a stage done, the gate complete), the Avatar coming and going (and the rite
pausing or breaking), techs learned and bending ranks reached, newcomers, and the agent's script changes (each new
saved revision, or new ad hoc code, with its `+A −D` against the last one). Each kind has its
own colour and shape; hovering one lists what happened that turn, clicking it seeks there, and
the legend's chips hide or show a kind. The log holds only keys and turns, so the page finds
them by replaying the whole log a second time (`fn_web::scan`) on another instance of the wasm
in a Web Worker; markers fill in as it goes (a 540-turn run takes a few seconds) without
slowing the replay being watched.

`cargo xtask web` builds the page and the wasm into `target/replayer/`; `--serve` serves it on
localhost, and each `LOG` argument is copied in and listed in the page's run picker
(`runs/index.json`). A page link takes `?run=PATH` (a `.jsonl` or `.jsonl.gz`); a file can
also be opened or dropped on it.

**The hosted replayer (#596).** `.github/workflows/pages.yml` builds `main`'s replayer on every
push and deploys it to GitHub Pages (`https://gizmo385.github.io/four-nations/`; public, as the
owner chose). The site holds no runs: a viewer opens a run's `run.jsonl` (or `.jsonl.gz`) from
their own disk, or drops it on the page, and it is replayed in their browser and never
uploaded. A log replays only against the game data it was made with, so the hosted page plays
logs made on recent builds of `main`; an older one is refused with the build it came from, and
`cargo xtask web --serve` from a checkout of that build plays it.

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

## The player's sandbox

Owner decision #476 (2026-11-06, option A, recorded in agent-scripting.md): a scripted playtest
agent knows what the game's tools show, plus its own notes and scripts carried from run to
run, and not `docs/` or `data/`. `cargo xtask agent-run` implements that by how it starts
`claude` (#567, `xtask/src/agent_run/sandbox.rs`). Each run gets one empty directory under the
system temp dir, and every `claude` call for the run (every chunk, and `--interactive`) starts
there with:

| Flag | Takes away |
|---|---|
| (the working directory) | the repo: no `CLAUDE.md`, `.claude/`, `docs/` or `data/` to find, and no project memory directory, which Claude Code keys on the working directory |
| `--tools ""` | every built-in tool: no shell, file, search, web, agent or tool-search tools |
| `--setting-sources local` | user and project settings: hooks, permission rules, plugins, the user's `CLAUDE.md` |
| `--disable-slash-commands` | skills and slash commands |
| `--system-prompt` | Claude Code's own instructions, replaced by a few lines saying the game's tools are the only way to act |
| `--strict-mcp-config --mcp-config` | every MCP server except the run's own `fn_agent` |

The MCP config, the `fn_agent` path and the log path are absolute, since the child's working
directory is not the one `agent-run` started in. A carried library, plan and notes still
reach the agent the way #476 allows: in its first prompt and through `fn_agent serve
--carry-in`, never as files it reads.

Each session is started with `--session-id`, and its transcript is copied from Claude Code's
own store into the run folder's `transcripts/` (`chunk-N.jsonl`, `mcp.jsonl` or
`interactive.jsonl`), so the model's reasoning stays with the run. A transcript that can't be
found is a warning, not a failed run. Transcripts hold the whole session, including any
account context Claude Code adds, so they stay under `~/fn-agent-runs`, outside the repo, and
are never committed or pasted into issues.

**What it guarantees:** the child is *given* only the game's tools and none of the repo. Checked
on the arguments and working directory it is started with (`xtask/tests/agent_run.rs`) and on
one real Sonnet session (#567, 2026-11-08, $0.02): the tool list sent to the API was the eight
`mcp__four-nations__*` tools and nothing else, loaded up front (no tool-search round trip); the
system prompt was ours; the working directory was the empty temp directory, not a git repo; and
no `CLAUDE.md`, memory, skill, slash-command or other MCP server context appeared. The first
request was about 3.3k tokens against roughly 22k for a full Claude Code session in the repo.
The model could not read a repo file because it had no tool that reads files; that session did
not force an attempt, so "cannot" rests on the tool list.

**What it does not:** the flags configure a child process; they are not a container. The child
runs as the owner, with the owner's credentials and home directory, so a harness bug or a
future tool outside `--tools` would not be stopped by the OS. Organisation-managed settings, if
any, still apply (`--setting-sources` does not turn them off). Claude Code still adds a fixed
first system line ("You are a Claude agent, ..."), the date, and account context to every
session: the signed-in user's email address and organisation id both appeared in the real
session. Those can't be removed without `--bare`, which refuses subscription login and needs
an API key; `--bare` was therefore not used. `--restricted` and `--safe-mode` were not used
either: `--tools ""` already removes the tools they would. The child inherits the caller's
environment, so a run started from inside another Claude Code session carries that session's
`CLAUDE_CODE_*` variables. Last, the sandbox hides files, not the game: it does not limit what
the player does with the game's own tools.

## What bounds a run's spend (#568)

`cargo xtask agent-run` refuses to start any run that calls the model (the default mode,
`--chunk-turns`, `--scripts`, `--track`, `--resume`) without `--max-usd N`. `--interactive` is the
exception: the owner is in that session and can end it, and `claude --max-budget-usd` only works
with `-p`, so a cap there would be a promise the harness couldn't keep; it is refused rather than
ignored. What limits the spend, from the inside out:

- **`claude`'s own `--max-budget-usd`**, given to every `claude -p` call as what is left of the cap
  (the whole cap for the default mode's one call; the cap minus `cost.json` for a chunk; for a
  campaign attempt, the invocation's cap minus the attempts so far, minus the attempt's chunks).
  This is the only thing that stops a chunk while it runs. `--chunk-calls` is a request in the
  prompt and nothing enforces it, so a chunk that ignores it runs until it hits this budget. By
  `claude`'s own account it stops once a response takes it past the amount, so a runaway chunk can
  overshoot by what was in flight, not by a chunk's worth; that overshoot has not been measured
  with a real session (nothing here runs the model), so treat it as "one response, a few cents",
  not as a guarantee.
- **The loop between chunks** reads `cost.json` and stops once the cap is spent. A chunk that did
  overshoot is therefore charged and is the last one.
- **A chunk whose cost can't be read** (no JSON, a result with no `total_cost_usd`, `claude`
  killed) is unknown, not $0. The run stops with an error rather than start another chunk it can't
  account for, and `cost.json` lists the chunk under `unknown_chunks` (its spend is missing from
  `usd`, so that total is a floor). That chunk was itself started with the remaining budget, so
  the unknown amount is bounded by the cap too. A nonzero exit whose result still has a cost is
  reported (stderr, `agent.log`) and charged, and the run goes on: that may be how `claude` ends a
  chunk that hit its budget, and the next check sees the cap spent.
- **No progress** (two chunks in a row that add no turn, script save, plan edit, note or
  `test_governor` call to `run.jsonl`) ends a plain run; a campaign attempt ends on the first, as
  #569 made it. A chunk that only edits and tests counts as work, so this does not bound a model
  that edits forever: the cap does.

Every chunked loop writes `stop.json` in its run folder (or the campaign's `attempt-K/`): `done`,
`cap`, `no_progress`, `unknown_cost` or `gave_up`, the chunk, the turns and a sentence. The
backstop on chunk count (`gave_up`) is a last resort, not a bound anyone should rely on.

## Playtests: the model says what it thinks (#576)

`cargo xtask agent-run --playtest --max-usd N` plays a scripted run to learn about the game, not
to score. It is `--scripts` with its own prompt and one more channel; campaigns (`--track`,
`--resume`) and plain `--scripts` runs keep their prompts byte for byte (a test pins them),
because narration costs tokens and could change how a model plays. A playtest can't be combined
with `--track`, `--resume`, `--interactive` or `--chunk-turns`.

- **The prompt** is the script prompt's governor-and-plan framing (no example governor is given,
  #638) with the aim changed: help the people who make the game learn how it plays, don't play
  for a high score. It asks for a remark of at most 30 words before significant actions and when
  something surprises the model (what it expects, what surprised it, what is confusing), and for
  a line starting `game problem:` for anything that seems wrong with the game itself. `SDK gap:` stays for the scripting
  tools.
- **What is captured** is the model's visible text (`say` lines, "says:" in the feed) and, in
  every run, summaries of its thinking (`think` lines, "thinks:"; see below). No signature is
  stored.
- **How it gets into the log**: `claude -p` is run with `--output-format stream-json --verbose`
  and its stdout read as it arrives. Each text block is appended to `run.jsonl` as a
  `{"type":"say","text":...}` line at once, so a watcher sees it live and a crash keeps what was
  already written. The final `result` event is written to `chunk-N.json` in the same shape
  `--output-format json` gives, so cost reading, `--max-usd` and the unknown-cost stop are the
  code they were; a stream that ends with no result event is an unknown cost. Reading the
  session transcript after each chunk would have been simpler, but the transcript only exists
  once the session is over, and the feed is meant to be live. One consequence: the remark and
  the `call` line that follows it are written by two processes, so on a loaded machine a remark
  can land a line after its call.
- **`say` is a commentary line** like `note` and `plan`: replay, scoring, `keys` and `--resume`
  ignore it, and a test pins that a log with and without remarks gives the same world and score.
  A reader without a `say` variant (a build from before #576) refuses the whole log with an
  "unknown variant" error rather than skipping the line; that is the behaviour for every new line
  kind, and it is why such a log needs a current build to watch or score.
- **Thinking summaries, in every run** (2026-11-10). `claude -p` defaults to an empty thinking
  display, which is why thinking first came through as an empty block. Claude Code has a
  `--thinking-display summarized` flag (not in `--help`); with it, each thinking block carries
  Anthropic's summary of the model's reasoning. That is the closest thing to an internal
  monologue that is available (the raw chain of thought is never returned), so every `-p` chunk,
  campaigns and plain `--scripts` included, now streams with it, and each non-empty summary is
  appended as `{"type":"think","text":...}`, a commentary line like `say`. Streaming and the
  display change only what is recorded, not what the model is asked, so scored prompts are
  unchanged.
- **The header** carries `"playtest":true` (only then), set by `fn_agent serve --playtest`. The
  watcher's strip reads `AGENT earth  PLAYTEST ...`, and the run browser and `watch --list` show
  `playtest` where the score would go and rank it with the unscored runs.
- **`game-problems.md`** is written after every chunk and at the end: the `game problem:` lines
  from the notes and the remarks, each distinct problem once with the turn it was first said
  (repeats, such as a carried note that reappears in a later session, are folded into it with the
  turns they came back at). They are the model's claims, not checked bugs. Run-health flags
  (#534's detectors) are not attached: they describe a `RunSpec` balance run, and a played log is
  not one.

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
