# Script-driven agent play: an SDK and a trampoline

Status: **accepted** (#472). The owner took every recommendation on 2026-11-06: see "Owner decisions".

Agent playtests today go through `fn_agent serve` ([agent-play.md](agent-play.md)). The model
makes every decision itself, one MCP round trip at a time. A ~2 game-year Opus run (seed 7,
Earth) cost $9–10.50. Most of that was cache reads of a context that grew to ~200k tokens over
~258 round trips (#341). Chunking (`scripts/agent-run.sh --chunk-turns`) caps the context, but the
number of round trips still scales with game time. That makes midgame and late-game testing, and
re-running playtests after a balance change, too expensive.

This doc proposes letting the agent **write scripts** that play the routine work, and only
calling the model when something surprising happens. With this, cost scales with decisions and
surprises rather than with game time.

## Decisions in brief

1. **Scripts run inside `fn_agent`, in embedded Rhai**, against an SDK that is a thin layer over
   the existing `fn_tui::surface::Host`. Reads go through the same view models. Actions go
   through the same `act` and `keys`, lowered to the same keys. Rhai is pure Rust and has no I/O,
   clock or RNG unless we register one, so a script can't read `docs/`, can't look ahead, and
   always does the same thing from the same state.
2. **The trampoline:** the model calls `run_script`, the script plays until it wakes the
   model, and the model gets back a compact **wake report**. Then the model revises the script
   and runs it again, or switches to another one. The model and the script never run at the same
   time.
3. **No coroutines are needed.** The host is in-process and lockstep, so the script's
   `wait(turns)` runs those turns inline. When a wake fires, the script ends. "Resuming" means
   running the script again: scripts re-read the world fresh each loop, like
   `fn_sim::autoplay::Opening`, and keep anything they need to remember in a small `memory` map.
4. **The SDK has two layers, borrowed from rs-sdk.** The *plumbing* layer is 1:1 with `Host` and
   changes slowly. The *porcelain* layer (`prelude.rhai`) is written in Rhai on top of the
   plumbing. It carries the domain knowledge ("build a bed near the camp", "wait until wood
   ≥ 20"), returns once the effect is done or has clearly failed, and grows from the gaps agents
   report.
5. **Parity holds by construction.** Scripts get the same reads, the same keys and the same
   20-second turn grid, with no waits shorter than a turn. Unlimited actions per turn are already
   allowed under lockstep, for MCP agents and for a paused human alike. Whether races may use
   scripts is an owner question (Q1).
6. **The run log is unchanged at its core:** it is still a seed plus keys plus turns. `verify`,
   `--watch`, `keys` and `--resume` keep working. New `script`, `wake` and `note` lines are
   commentary that replay ignores, and the agent strip shows which script is acting.
7. **Scenario starts are data.** `data/scenarios.toml` presets start a run from the
   deterministic `autoplay::Economy` opening fast-forwarded N game minutes (Q3). The header
   records the prelude, so every tool can rebuild it.
8. **A proven script library runs with no model at all.** `fn_agent play --script` re-runs a
   library headless after a balance change. It stops at the first wake and leaves a report.
   That is the cheap regression loop the owner wants.

## What we borrow from runebench, and what we don't

Sources: the [benchmark page](https://maxbittker.github.io/runebench/), the
[rs-sdk repo](https://github.com/loopwork/rs-sdk) (its `SDK_DESIGN.md`, `agents.md`,
`scripts/CLAUDE.md` and best-practices notes), and a
[talk summary](https://youtubesummary.com/summary/cNm4zyUtWj0).

In runebench, coding agents write TypeScript bots against a local emulated RuneScape server and
are scored on XP rate over 30-minute tasks. Results are plotted against cost.

**Worth borrowing:**

- **Porcelain over plumbing.** Plumbing calls (`send*`, `getState`) resolve when the server
  acknowledges them. Porcelain calls (`chopTree`, `walkTo`) resolve when the effect is done, and
  return `{success, message}` with readable failure text. The client below holds only
  primitives, and anything smart lives in porcelain.
- **One execute tool.** The MCP server exposes `execute_code(bot, code)`, which returns the
  result, the logs and the final state. Its API docs are served from the server. Our
  `run_script` follows this shape.
- **Confidence-based durations.** Run a new script for 10–30 s, a validated one for 2–5 min,
  and a proven one for 10+ min. "Fail fast."
- **Scripts detect their own stalls**, and a **lab log** per script records each run's outcome,
  root cause and fix, plus a section for "SDK issues and gaps". Their SDK grew by analysing
  these reports.
- **Pitfalls they found:** blocking dialogs stall scripts, old messages linger in buffers
  (filter them by tick), and each action needs its own success signal.

**What doesn't transfer:**

- **Real time.** Their scripts are `async` and poll a live server ticking every 600 ms. Our sim
  is deterministic and lockstep, and the host owns the clock. A script here is synchronous code
  in-process, and `wait` is "run N turns now". We need no async runtime, no polling races and no
  wall clock, and replay survives.
- **Their score.** We're not building a benchmark (that's #318's races). We want cheap
  playtests that find balance and UI problems.

## Architecture

```text
 model (Claude Code, MCP tools only)
   │ run_script {name, turns, wake_on}          ▲ wake report (why, diff, log tail, observe)
   ▼                                            │
 fn_agent serve --scripts ─────────────────────────────────────────────────▶ run.jsonl
   ├─ script runner (Rhai Engine: no I/O, no clock, no RNG, op budget)      header (+prelude),
   │    ├─ prelude.rhai (porcelain: find, build_near, wait_until, ...)      keys, turns,
   │    └─ plumbing bindings ──▶ fn_tui::surface::Host  (unchanged rules)   script, wake, note
   └─ the existing 8 tools (observe, look, act, end_turn, ...)                   │
                                                                                 ▼
 fn_agent play --script lib.rhai --scenario ID    (no model: stops at first wake)  fn_tui --watch
```

The runner lives in `crates/agent/src/script/`. The parity lint (#320) already scans
`crates/agent/src/`, so the runner can't reach `World` or `Command`. `fn_sim` is untouched.

## The SDK

### Language: Rhai, embedded (Q2)

| | Rhai in `fn_agent` (recommended) | TypeScript in Deno (`--deny-all`) | Python subprocess |
|---|---|---|---|
| Sandbox | By construction: no I/O, imports, clock or RNG unless registered | Deno's permissions: good, but outside our code | None. It needs an OS sandbox, or the racer rule breaks |
| Harness | A text argument to one MCP tool, so the agent still needs no shell | The same, plus a `deno` binary and a JSON pipe | The same, plus `python3` and a JSON pipe |
| Deterministic re-run | Yes (Rhai maps are ordered) | Only if `Date`/`Math.random` are stubbed | Only with care |
| Model fluency | Fair. The syntax is close to JS and Rust, errors give line:col, and we ship a reference | Best | Best |
| Build cost | One pure-Rust crate, in `fn_agent` only | An external tool to install | An external tool to install |

Rhai wins on sandboxing and on deterministic re-runs. The cost is fluency, so the SDK reference
is generated from the registered functions and served by a tool (below). The runner configures
the engine with no module resolver (no `import`, so no file reads) and no time functions, and
it routes `print` and `debug` to the script log. That routing matters because stdout is the MCP
channel. An operation budget (`Engine::on_progress`) stops runaway scripts deterministically.
It counts the work done since the script's last `wait`, so a loop that waits can run all game
(see "The operation budget" under `run_script`).

### Plumbing: 1:1 with `Host`

| Rhai | Host | Notes |
|---|---|---|
| `observe()` | `Host::observe` | A map, the same JSON as the tool |
| `look(x, y, w, h)` / `inspect(x, y)` / `screen(name)` | the same | Script reads don't draw attention ghosts (they would outline the whole map every turn) |
| `cells(x, y, w, h)` | **new** `Host::cells` over `cell_look` | `[#{x, y, thing, ground, marks}]`, so scripts don't parse `look`'s strings. Added to the map-cell equality test |
| `can_build(building, x, y)` | **new** `Host::can_build` over `fn_ui::build::placement_at` | `#{ok, reason}`: whether `building` could be designated with its anchor at `(x, y)`, the same check a `build` step's own confirm uses (#509). Read-only: presses no keys, changes nothing |
| `act(steps, note)` | `Host::act` | Returns `#{ok, keys, label, notices, error, observe}`. `ok` is added by the binding (the `act` tool's result has no such field): false when a step couldn't be planned (`error` is then `#{step, reason}`, else `()`) or a `designate`/`build` step got "0 done", the test the run report's `acts.failed` counts. It says the keys were pressed, not what they did: a `research` step for a locked or finished tech is `ok` too, so check `observe.research.label` or `screen("research")`. It already closes open screens first, the job rs-sdk's `dismissBlockingUI` does |
| `keys(script, note)` | `Host::press_keys` | The same refused keys apply |
| `wait(turns)` | `Host::end_turn(1)`, repeated | Returns the last turn's observation. At least 1 turn, so nothing shorter than the turn grid. Checks wake rules after every turn |
| `events()` | **new**: alerts with their `Kind` | `#{turn, kind, text}` since the script started. The kind is the toast's own category, which a player sees |
| `log(value)`, `wake(msg)`, `memory` | runner | `log` takes any value (a string as written, anything else as its printed form, #565). `memory` is a map that persists across runs in a session (8 KB cap) |

### Porcelain: `prelude.rhai`

The porcelain is Rhai written on the plumbing, so it can't do anything the plumbing can't. It is
embedded in `fn_agent` because it is code, not balance data. Its calls return
`#{ok, msg, ...}` with a player's words for failures, using the act's own notice text where
there is one. It starts small and grows from the agents' "SDK gaps" notes:

- `find(desc, near, radius)` returns tiles whose thing matches, nearest first.
- `designate_near(tool, desc, near, radius, standing)` splits matches into rectangles and
  designates them.
- `build_near(building, near)` finds a free footprint (size from `screen("build")`), builds it,
  and reports the notice.
- `wait_until(|| cond, max_turns)` returns true once the condition holds, or false at the limit.
- `stalled(name, value, turns)` is stuck detection: it calls `wake` if a tracked number hasn't
  moved in N turns.
- `store(name)`, `idle()` and `need(name)` read single values from `observe`.

An example, as an agent might write it:

```rust
// keep-fed v3: berries on standing orders, a bed one ahead of the people.
let camp = memory.camp ?? #{x: 58, y: 42};
loop {
    let g = observe();
    designate_near("harvest", "berry bush", camp, 15, true);
    if store("Wood") >= 4 && g.people.activity.Idle > 0 {
        let r = build_near("Bed", camp);
        if !r.ok { log(`bed: ${r.msg}`); }
    }
    stalled("food", store("Berries"), 30);   // wakes the model if food stops moving
    wait(3);                                 // three turns, one game minute
}
```

`sdk_reference` is a new tool. It returns the plumbing signatures (from Rhai's
`gen_fn_signatures`) and the prelude's doc comments, so the reference can't drift from the
code, plus (#508) every `act` step shape (`designate`, `build`, `select`, `research`, `cursor`,
`keys`, docs/design/agent-play.md's own "Action" table) as a `#{step, fields, example}`,
including `sow` (#333) and `bend`'s own `ability` field.

## The trampoline

### `run_script`

`run_script {name? | code?, turns, wake_on?, note?}` runs a saved script by name, or inline code
for a one-off probe. The script runs until one of these happens:

| Stop | Wake reason |
|---|---|
| The script returns | `done` (with the return value) |
| A runtime or parse error | `error` (message, line:col, and the line's text) |
| `turns` used up | `budget` |
| A `wake_on` event lands during a `wait` | `event` (which one) |
| The script calls `wake(msg)` | `asked` (the message) |
| More work than the budget since the last `wait` | `runaway` (how much, and the line it stopped on) |

Event kinds are the alert kinds plus needs: `death`, `arrival`, `tech`, `endeavor`, `season`,
`stockpile`, `bend_failed`, and `need` (a need flag first shown). These are the same events that
stop `end_turn` today. The default `wake_on` is `death`, `need` and `endeavor`. The others are
listed in `events()` for the script to handle. `turns` defaults to 15 and is capped at 180 (one
game year). A headless year takes seconds: a release build ran 69 early-game turns in 1.2 s.

`save_script {name, code}` stores a script and bumps its version. `scripts {}` lists the saved
scripts with their versions, line counts and last outcomes; `scripts {name}` adds that one script's
source (#560: the list leaves sources out, since a governor is 3-5 KB, so only a named read pays
for one). `notes {append?}` reads or appends the agent's own
lab notes. The briefing teaches confidence-based budgets in game terms: 3–5 turns for a new
script, a season (45 turns) for one that worked once, and a year for a proven one.

All limits go in `data/agent.toml` `[scripts]`: `default_turns`, `max_turns`, `max_operations`,
`log_tail`, `memory_bytes`, `plan_bytes` (the `plan` tool's cap, see agent-hillclimb.md) and
`default_wake`. Like `tui.toml`, the sim never reads this file.

### The operation budget

A script may do `max_operations` of work between two `wait`s (#564): Rhai operations, plus a
fixed cost for each native call, because Rhai counts `cells()` or `act()` as one operation
however long the game takes over it. Every `wait` starts the count again, so a loop that waits
runs all game, and a loop that never reaches a `wait` stops as `runaway`. The `why` says how
much work it had done and quotes the line it stopped on (for a call into one of the script's own
functions, the line of the call). Nothing about the stop suggests a `wait` after each scan: one
pass of a governor may be as long as the budget allows.

The costs are in `bindings::cost`: `observe`, `act` and `keys` 2000 each, `cells` and `look`
20 plus half the cells in the window, `can_build`, `inspect`, `screen`, `events` and `plan` 40.
They are flat or a function of the arguments, never of a clock, so a run stops at the same call
on every machine. They are about a microsecond to the operation, set from release-build timings
on the reference governor (`crates/agent/tests/reference/`): a late-game `observe` or `act` takes
3.5 to 4.5 ms, a 39x39 `cells` 0.7 ms and a `can_build` 10 to 15 us. That governor's heaviest
pass, 540 turns on three seeds, is 48,000 operations plus 40 `act`s and 9 `observe`s, about
160,000 in all. The budget is 1,000,000: six times that, so a governor twice its size still fits
a pass, and a stuck loop stops in a second or two: 0.15 s for a bare loop of arithmetic, 2 s for
one of `cells(0, 0, 39, 39)` (measured in the `ci` profile).

`fn_agent play` and `test_governor` use the same budget as `run_script` (they run the same
runner), and replaying a log runs no script at all: keys drive a replay, so no budget can change
what one does. A harness that runs `fn_agent play` as a child (`cargo xtask agent-eval`) also
kills a child that outlives a wall-clock limit, and reports that as an error, never as a score.

### The wake report

The wake report is what the model reads instead of a turn-by-turn transcript. It should be about
1–2k tokens, and it never includes a frame unless the model asks for one.

```json
{"wake": "event", "why": "death: Ping died of thirst", "script": "keep-fed", "version": 3,
 "line": 9, "turns_run": 23, "clock": "15:40 -> 23:20", "season": "Summer",
 "since_start": {"people": "6 -> 7", "stores": {"Wood": "+38", "Berries": "-12"},
                 "events": {"arrival": 1, "tech": 1, "stockpile": 2, "death": 1},
                 "texts": ["Masonry is finished", "Ping died of thirst"]},
 "acts": {"ok": 9, "failed": 1, "last_failure": "build Bed: 0 done, 4 skipped (blocked)"},
 "log_tail": ["turn 41: 2 idle", "turn 52: bed blocked at (60,41), trying (63,44)"],
 "memory": {"camp": {"x": 58, "y": 42}},
 "observe": {"turn": 70, "clock": "23:20", "people": {"total": 7, "needs": {"Thirsty": 2}}}}
```

The `since_start` diff is computed from two observations taken at the start and at the wake,
so it contains nothing an observation doesn't. Event texts are deduplicated with counts.
`log_tail` keeps the last 30 `log` lines.

### Memory across sessions

Every `save_script`, `run_script` and `notes` call writes a log line (below). A chunked session
started with `--resume` therefore gets the script library, the versions, `memory` and the notes
back from the log. The handoff between chunks becomes "here are your scripts and notes" instead
of a prose report, which is both smaller and more exact.

## Parity

Agent-play's rule is unchanged: an agent sees what some screen shows and does what keys do.
Scripts are a third way in, next to the MCP tools, and they reach the same `Host` methods:

- **Reads.** Every binding calls a `Host` read built from `fn_ui` view models or `cell_look`. The
  observation-containment and map-cell tests (#320) already cover them, plus `cells`.
- **Actions.** All of them go through `act` and `keys`, so through `Shell::key`. The same keys
  are refused.
- **Time.** `wait` can't go below one turn. A script never sees the middle of a turn and can't
  change the clock.
- **Rate.** Within a turn, a script may read and act without limit. So can an MCP agent, and so
  can a human who pauses, since lockstep already gives everyone free thinking between turns.
  What scripts add is tirelessness (checking 30 things every turn, forever), not a new
  capability. So this doc proposes **no actions-per-minute cap** for playtests. A
  `max_keys_per_turn` knob for race presets is part of Q1.
- **Knowledge.** The sandbox is stronger than the MCP path. A script can't read files, fork the
  world or reach the network, whatever the harness allows. What the *model* may read between
  runs is Q4.

## Determinism, replay and watching

- **The log stays a seed plus keys plus turns** (plus the prelude, below). Replay never re-runs
  a script: it re-plays the keys the script pressed. `verify` (#318), `--watch`, `fn_agent keys`
  and `--resume` work unchanged.
- **New log lines:** `script {name, version, code, turns, wake_on}` when a run starts,
  `wake {reason, why, turn, memory}` when it stops, and `note {text}`. Each act a script makes is
  a normal `keys` line. Its label is the step and its note is `[keep-fed v3]` plus the script's
  own note.
- **Re-runs are reproducible.** The same script on the same state presses the same keys
  (Rhai with no clock or RNG, ordered maps). So a headless `fn_agent play` run from the same
  scenario, library, data and code gives the same final digest, and a difference after a balance
  change comes from the balance change.
- **The strip** shows the script between the clock and the action. When the script wakes, the
  strip says why until the next act. Here is a real `fn_tui --watch run.jsonl --frame --ascii
  --size 110x34` of a served run, with the strip line replaced by the proposal:

```text
= STOCKPILE = Enter: set a corner - Esc: leave ===============================+----- * EARTH KINGDOM * ------+
.~~~~.,,,,TTTT,,,,,TT,,,^oo^,$oo^o^,Y,,,Tc,,T,Y:::,..~~~~~~~~~~~~~~~..F,,,,,F"| > Running x1           02:00 |
.~~~~.T,,,,,Y,TTT,,T,^$o^^^^^^-^^^$0,,,,,,,,,T:::T,.~~~~~~~~~~~~~~~~~.F"F"F,""+- SETTLEMENT ----------- [N] -+
..~~..,,TT,,,,TT,,TT^o^^^^^o^o^^^^,,,,,,,,T,c::::,T.~~~~~~~~~~~~~~~~~..FFF"F"F| People 6              Idle 6 |
F..~~...,TT,,,,TTT,,^T,o$,T,^$o$o$,,,,,,,,,,:::::TT...~~~~~~~~~~~~~~~~...""""F| Food 0 - 0/head              |
""..~.~..,,c,T,,TT,T0TTTT,,0,,^o^^0,,::::,c,::::TTT,,....~~~~~~~~~~~~f~~.FF""F| Research: none set       [T] |
"Y"..~~~=/=f=,TTT,,,TTYTTT,,T,o^^$,,::::::::::TT,TTT,,,,..~~~~~~~~~~~~~~...F""| Gate 0/9 - 10 people     [G] |
"""F..~..=,TTTT,,T,T,,,,,T,,T,$^o,,,::::::::,,,Tc,T,,T,Y,.~~~~~~~~~~~~~~~~.FFF+- STOCKPILE ------------ [P] -+
FFF""..@"//,,,TT,,,,TTTT,,,,,,0,T,,,,::::::,,,,,YT,,,,,,..~~~~~~~~~~~~~~~~."FF| designate stockpile: 6 done, |
FF""""""@",,T,T,,,,,,,,,,,::,:,TT,,,,::::,,,Y,,,,...,,,,.~~~~~~~~~~~~~~~~~.F""| 3 skipped (can't stockpile   |
"FFFFFF""@"TTT,,T,,,,,,,,,::TTT,TT,TT,,,:,,,,,,...~...,,...~~.~~~~~~~~~~..."FF| here), 6 unreachable         |
YF...F"FF""",,,T,,,,T,,,T:::0^TTT,,TTT,,cT,,,,..~~~f~.,,,,...~~~~~~~~~..."""F"+- HERE ------------ (64, 42) -+
...
 AGENT earth  FOLLOW  turn 6  02:00  SCRIPT keep-fed v3  designate stockpile 3x3 at (62,40) "store by camp"  x20
 AGENT earth  FOLLOW  turn 70  23:20  SCRIPT keep-fed v3  woke: death: Ping died of thirst  x20
```

The message log (`L`) gets an Agent entry for each script start and wake, so a replay reads
back as "ran keep-fed v3 for 23 turns, woke on a death".

## Scenario starts (Q3)

Testing the late game shouldn't require playing the early game first. **`data/scenarios.toml`**
is read by `fn_agent` and `fn_tui`, never by the sim:

```toml
# Scenario starts (docs/design/agent-scripting.md). A scenario is a normal start plus a
# prelude: a deterministic stock opening run for `minutes` game minutes before anyone plays.
# - seed, nation, map_size, difficulty, site: as races.toml.
# - prelude.opening: "economy" (fn_sim::autoplay::Economy) or "opening" (autoplay::Opening).
[[scenario]]
id = "earth-year-two"
name = "Earth, year two"
brief = "A settlement a year in, with its first buildings standing."
seed = 7
nation = "earth"
map_size = "small"
difficulty = "normal"
site = "default"
prelude = { opening = "economy", minutes = 60 }
```

- **How it runs.** `Host::new` runs the prelude before the first read. A new `mod prelude` in
  `surface.rs` joins `end_turn` on the lint's allow-list: it's the one other place that may
  apply `Command`s and run ticks, and it runs before the agent exists. The header records the
  prelude, so `--watch`, `--resume`, `verify` and `keys` rebuild the same state. Turn 0 starts
  after the prelude, and the clock shows real game time.
- **Why autoplay.** `autoplay::Economy` already plays toward the Harmony Gate, is deterministic,
  and its pacing is measured by the balance harness (#352, #440). Its output follows the current
  balance, so the late game you test is the one today's numbers produce. No save files need
  updating.
- **Humans get it too.** `fn_tui --scenario earth-year-two` starts the same game for the owner,
  which keeps parity in both directions. `fn_agent keys` prints `--scenario ID` in the command it
  suggests.
- **Not chosen now:** save-file starts (the run would need the file beside it, and a save made
  under old balance hides what the new numbers would have built), and a stock *Rhai* prelude
  (visible in the watcher and uses the SDK, but it needs the SDK first and is worse than
  Economy until someone writes it well). The prelude field allows a script prelude later.

## Headless play: `fn_agent play`

`fn_agent play --script lib.rhai [--scenario ID | --seed N --nation X] --turns N --log PATH`
runs a script library with no model. It stops at the first wake and writes the run log plus a
JSON wake report. This is how:

- **regression playtests** run after a balance change: replay the proven library on each
  scenario, diff the reports, and call a model only for the runs that woke early;
- **CI** gets the scripted bot agent-play.md wanted. A stock library under
  `crates/agent/scripts/` must survive a short slice of seed 7, Earth, every build;
- **porcelain** gets tested on real worlds.

## Cost model

Calibration: an early, uncapped single-session Opus run (the one the opening paragraph cites)
read ~30M context tokens for $9–10.50, about $0.33 per million including output. Cost is roughly
the sum, over round trips, of the context size at that moment.

**Measured (#485), 2026-11-07.** One game year (180 turns, `data/agent.toml`'s own cap) on seed
7, Earth, Normal, `--chunk-turns 30` or `--chunk-calls 30`, each on Sonnet and Opus, plus one
`--scripts` run on the `earth-year-two` scenario (Sonnet). `scripts/agent-run.sh --max-usd 5`
capped every run; none hit it. Figures are from each run's own `cost.json`.

| Per game year | MCP, chunked (Sonnet) | MCP, chunked (Opus) | Scripted (Sonnet) | Scripted (Opus) | Scripted, `earth-year-two` (Sonnet) |
|---|---|---|---|---|---|
| Round trips | 132 | 162 | 53 | 87 | 39 |
| Turns played | 195 | 180 | 317 | 180 | 180 |
| Context tokens read | 3.1M | 4.8M | 1.2M | 2.7M | 1.1M |
| Average context | ~23k | ~30k | ~22k | ~31k | ~29k |
| USD | $0.97 | $3.00 | $0.48 | $2.12 | $0.39 |
| Population | 6 → 11 | 6 → 11 | 6 → 16 | 6 → 12 | 12 → 18 |
| Deaths | 0 | 0 | 0 | 0 | 0 |

- **During play, scripted runs cost about half of chunked MCP runs on the same model**: $0.48 vs
  $0.97 on Sonnet, $2.12 vs $3.00 on Opus. Both `--scripts` runs also overshot their 180-turn
  target in a single `run_script` call once their script was going well (195 and 317 turns) --
  cost tracks round trips and decisions, not the turn count asked for, so the scripted Sonnet run
  actually bought 1.76 game-years for $0.48, not one.
- **The scenario run was the cheapest of all** ($0.39, 39 round trips), despite being `--scripts`
  on the same model as the next-cheapest row: it starts a year in, with a settlement already
  past the opening's many small early decisions, confirming "scenario starts skip the opening in
  both modes" without needing a from-scratch run to compare against.
- **Opus costs 3–4x Sonnet for the same mode**: 3.1x for MCP-chunked, 4.4x for scripted. Neither
  Opus run played obviously better: same or lower population growth, same zero deaths, and
  neither cleared more of the Harmony Gate's signs than its Sonnet counterpart.
- **The big change is in re-runs.** Once a library is proven, re-running it after a balance
  change costs CPU seconds, and tokens are spent only on surprises. One `fn_agent play --script
  crates/agent/scripts/settle.rhai` re-run of the full year after a throwaway balance change
  (`cabbage` harvest `regrow`, 480s → 4800s, reverted after) still finished with `reason:
  "budget"` and no deaths -- it didn't wake, so this re-run's own triage cost was $0. That's the
  regression-playtest story working as designed, not evidence the balance change itself was
  harmless (the change was never meant to go anywhere; `settle.rhai`'s own standing-harvest and
  bed orders have enough slack that a single harvest's regrow time wasn't the thing to break it).
- **Model tiers.** Scripted play needs a model that writes and debugs code from error messages.
  Measured: Sonnet did that well in every run above, at roughly a quarter to a third of Opus's
  own cost for no worse an outcome, so Sonnet stays the default tier for scripted playtests.
  Opus is for design-critique runs, where judgment matters more than cost, not for routine
  playtesting. Haiku-class triaging of a proven library's wakes wasn't measured this round
  (#485's budget went to the five playtest runs and the `fn_agent play` re-run instead); that
  stays an open question for a follow-up.

## Relation to existing work

- **Builds on `fn_agent serve`.** `--scripts` adds `run_script`, `save_script`, `scripts`,
  `notes` and `sdk_reference` to the same server, the same `Host` and the same log. MCP-turn
  play stays. It remains the right tool for UI-readability tests (can an agent follow the
  screens turn by turn?) and for races as #318 specifies them.
- **`agent-run.sh`** gains `--scripts`: a script-mode prompt, chunking by round trips instead of
  turns, the library and notes as the handoff, and a cost line per run from `claude -p
  --output-format json`. `--chunk-turns` stays for MCP mode.
- **`fn_sim::autoplay` and the `balance` binary** stay the privileged tools for measuring what
  the rules allow. Scenario preludes reuse them. Scripts measure what a player can do through
  the UI.
- **#318 races** are unaffected unless Q1 allows scripted racers. **#320** gains `cells`.

## Follow-up issues

In dependency order. Decisions gate the first item of each chain.

- #477 fn_agent: embedded Rhai runner with the plumbing bindings and `run_script` (blocked
  by #473, #474)
- #478 fn_agent: wake rules, `events()` with alert kinds, and the wake report (#477)
- #479 fn_agent: script library, `memory` and lab notes, logged and restored by `--resume`
  (#477)
- #480 fn_agent: the porcelain prelude, `cells()`, and a generated `sdk_reference` (#477)
- #481 Scenario starts: `data/scenarios.toml`, an autoplay prelude, and `--scenario` for both
  binaries (#475; independent of the runner, so it helps MCP-only runs too)
- #482 Watch: script and wake lines in the agent strip and the message log (#478, #479)
- #483 fn_agent play: headless script runs with no model, a stock library, and a CI slice
  (#478, #480)
- #484 agent-run.sh: `--scripts` mode with library handoff, carried notes and per-run cost
  (#478, #479, #480, #476)
- #485 Measure: scripted vs MCP playtest cost per game year, and which model tier (#481, #483,
  #484)

## Not chosen

- **Suspendable coroutines** (Lua threads, async TS). A script could continue after the model
  looks at a wake, but on a wake the model usually wants to change the script anyway.
  Restartable scripts plus `memory` give the same result without machinery that would have to
  survive `--resume`.
- **Scripts issuing `Command`s, or reading `World`.** This would be faster, but it's the
  privileged API that agent-play.md rejected, and the balance harness already provides it.
- **Sub-turn waits** (`wait_ticks`). They would break the shared decision grid and add little:
  a turn is 20 game seconds.
- **The model writing scripts with its own shell and files.** That's how rs-sdk works, but it
  hands the agent `docs/`, `data/` and a way to run the seed ahead.

## Owner decisions

All four answered on 2026-11-06, each as recommended:

1. **Parity and races** (#474): parity by construction everywhere, with no action cap.
   Playtests may script freely. Race presets get `scripts = true/false` (default false), and
   scripted runs rank in their own bracket.
2. **Script language** (#473): Rhai, embedded in `fn_agent`.
3. **Scenario starts** (#475): presets in `data/scenarios.toml` whose prelude runs
   `autoplay::Economy` for N game minutes.
4. **What a scripted playtest agent may read** (#476): in-game knowledge plus its own notes and
   scripts, carried from run to run, but not `docs/` or `data/`.
