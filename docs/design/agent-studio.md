# Agent studio: the watcher as a demo of an agent writing code

Status: **draft** (#496). The owner settled the layout, the tab fallback, the sparkline's data
and the code pane's contents on 2026-11-06 (quoted in the issue). Two smaller calls are still
open: see "Open questions for the owner".

`fn_tui --watch` replays a run log in the normal TUI with a one-line agent strip
([agent-play.md](agent-play.md), "Watching"; #482 added the script and wake text). For a scripted
run ([agent-scripting.md](agent-scripting.md)) that hides the interesting part: the script the
agent keeps rewriting, what changed between revisions, and the calls that led to each run. This
doc turns the watcher into a **studio**: the running script with its changed lines marked, beside
the live game, with an activity sparkline and a feed of the agent's calls. A terminal recording
of it should work as a demo video.

## Decisions in brief

1. **The studio is how `--watch` looks at 160x45 and up.** Code pane on the left. On the right,
   the existing watch view, and an agent feed under it. There's no flag and no toggle: a log
   with no scripts just shows an empty code pane with a one-line notice.
2. **Below 160x45 the same three panes become tabs** (`1` code, `2` game, `3` feed). Above the
   threshold the same digits zoom a pane to full screen. So there is one model, not two.
3. **The code pane shows the revision that is running at that point in the replay.** Rhai
   syntax highlighting and a line diff against the previous revision of the same script both
   come from plain functions in a new `fn_ui::code` module, with no new crates.
4. **A new `call {tool, summary}` log line** is written by `fn_agent serve` before each MCP tool
   call, so the feed can show `observe`, `briefing` and the rest. Like `note`, replay, verify,
   `keys` and `--resume` ignore it.
5. **The sparkline plots acts (logged `keys` lines) per turn** over the last 24 turns, on the
   agent strip.
6. **Recording is `fn_tui --watch LOG --record out.cast`**, a deterministic asciicast rendered
   headless and faster than real time. That's the recommendation; whether it's in scope is an
   owner question (Q1).

## Layout

```text
 ≥ 160 x 45                                       below 160 x 45 (≥ 80 x 24)
┌ code ───────────┐┌ game (today's watch view) ┐  ┌ the zoomed tab, full screen ──────┐
│                 ││ map             │ panel   │  │ 1 code │ 2 game (default) │ 3 feed │
│                 ││                 │         │  │                                   │
│                 │├ strip + sparkline ─────────┤  │                                   │
│                 │├ AGENT feed ───────────────┤  │                                   │
│                 ││                           │  ├ strip + sparkline + 1[2]3 ────────┤
└─────────────────┘└───────────────────────────┘  └───────────────────────────────────┘
```

- **Game column width** = `clamp(round(0.6 × W), 96, 120)`. 96 columns is a 64-column map plus
  the 32-column panel. Past 120 a wider map only shows more terrain, so the code pane gets
  the rest of the width. At 160 the split is 64 + 96, and at 200 it's 80 + 120.
- **The feed** is the bottom `max(12, H / 4)` rows of the game column. The game gets what's
  left (33 rows at 45 high), and its bottom row is the agent strip, as it is today.
- **The game pane is the existing watcher render** (`Watcher::render` at the pane's size, drawn
  into the frame at the pane's position). Follow and free look, ghosts and the message log all
  work unchanged inside it.
- All of these numbers are presentation, so they go in `data/tui.toml` `[studio]` (below), not
  code.

Here is the studio at 160x45, turn 20 of a real `fn_agent play` run of
`crates/agent/scripts/settle.rhai`. The game column is a real
`fn_tui --watch run.jsonl --frame --size 96x33 --at 20`, and the code pane, strip and feed are
composed to match this design. Colour doesn't show in text: keywords, strings, comments and
calls are coloured, and the `▌` lines also get a tinted background.

```text
┌ keep-fed.rhai ── rev 3/9 ── run 45t ── +9 −2 ────────────────┐━ HARVEST · REPEATING ━ Enter: set a corner  ·  O: one-shot  · …┏───── ◆ EARTH KINGDOM ◆ ──────┓
│  1  // keep-fed v3: standing food orders, a bed one ahead,   │♠♠.≈.♠""""""",♣,,τ,,,,,,,,···▲▲•▲▲▲,,♣♣♣♣,¥,,,.≈≈≈≈≈..%♣,,,,..≈≈│ ▶ Running x1           06:40 │
│  2▌ // and a wake of my own before thirst kills anyone.      │♠♠...""""♠♠""",,♣,,,,,,,,,♣,,♣▲$▲▲▲♣♣,,,,,%,,..≈≈≈≈..,♣τ,♣♣,,.≈≈├─ SETTLEMENT ─────────── [N] ─┤
│  3  let camp = memory.camp ?? observe().cursor;              │♠♠♠♠♠""""♠""♠"",,♣,,,,♣,,,,,♣♣▲▲▲•♣,,♣,♣,♣,τ,.≈≈≈≈≈≈..,,♣♣,,,.≈≈│ People 6              Idle 1 │
│  4  memory.camp = camp;                                      │♠♠♠♠""""♠♠"♠♠""",,,,,,,,,,,,τ,,▲▲▲,♣,,♣,,,,,..≈≈≈≈≈≈≈..,,,♣,,.≈.│ Food 0 · 0/head              │
│  5                                                           │♠♠♠♠♠♠"♠"♠♠""♠♠"♠,,,,,,,,,,,,,,▲▲$●♣,~♣,,,,,.≈≈≈≈≈≈≈ƒ≈..,,,♣,...│ Research: none set       [T] │
│  6  loop {                                                   │♠""♠♠"♠♠♠""""¥""♠¥""¥♣,♣,,,,,,●,•▲,,,,~,,,,..≈≈≈≈≈≈≈≈≈≈.,♣♣,,♣,,│ Gate 0/9 · 10 people     [G] │
│  7      designate_near("harvest", "Wild Cabbage", camp, 15, …│♠""♠♠♠"♠♠♠♠♠♠♠""♠""♠"♠,,,,,,,,,•▲▲●♣♣,,~,,..≈≈≈≈≈≈≈≈≈≈≈..♣,,,,,.├─ HARVEST · REPEATING ── [H] ─┤
│  8      designate_near("harvest", "Fruit Tree", camp, 15, tr…│▲""♠♠"♠"♠♠♠♠¥♠♠"♠"""""",,,,,,,●▲▲▲,,,...~..≈≈≈≈≈≈≈≈≈≈≈≈≈..♣♣%...│ designate harvest (chop,     │
│  9                                                           │♠♠♠♠"♠♠♠♠♠""♠♠"""""¥"♠♠♠",,,,,,▲▲●●,,.≈≈≈≈≈≈≈≈≈≈≈≈≈≈≈≈≈≈≈.,τ,.≈≈│ mine, gather): 1 done, 0     │
│ 10▌     let g = observe();                                   │"""♠♠♠♠♠""♠♠"""¥¥""""""ƒ,,,,,,,•,●♣♣♣.≈≈≈≈≈≈≈≈≈≈≈≈≈≈≈≈≈≈≈.,,,.ƒ≈│ skipped                      │
│ 11▌     let people = g.people.total;                         │"♠♠♠♠♠♠♠♠♠♠♠♠"""""♠"""~♠,,¥,,,●,●,♣,♣...≈≈≈≈≈≈≈≈≈≈≈≈≈≈≈≈≈.♣,,.≈≈├─ HERE ──────────── (79, 75) ─┤
│ 12▌     let thirsty = g.people.needs.Thirsty ?? 0;           │"♠"♠♠♠♠♠♠♠♠♠"♠"""""""~"",,,,,,,%,,,,,,,....≈≈≈≈≈≈≈≈≈≈≈≈≈..♣,,...│ Fruit Tree                   │
│ 13▌     if thirsty > 0 {                                     │"♠♠""♠♠♠...~..♠"♠♠""~"",,¥,,,,,♣¥,,,♣,,♣,♣..≈≈≈≈≈≈≈≈≈≈≈≈.,,,¥♣,.│ Plains - Land (79, 75)       │
│ 14▌         log(`thirsty: ${thirsty}`);                      │"♠√♠♠♠♠..≈≈≈≈...""""~",,¥,,,,,,,,,,,,,,,,♣♣.≈≈≈≈≈≈≈≈≈≈.≈.,,♣♣♣♣♣│ Pick: 2 Fruit - 4s           │
│ 15▌         wake(`${thirsty} thirsty: water plan needed`);   │"♣♣~"♠♠.≈≈≈≈≈≈≈..""~τ",♣,,τ,%,,,,,,,,,♣,♣♣,..≈≈≈≈≈≈≈≈....,,,♣,♣♣│ (repeating)                  │
│ 16▌     }                                                    │..~."...≈≈≈≈≈≈≈≈..~""♠,♣,,,,,¥,,,,%,%♣,♣,♣,,.≈≈≈≈≈≈≈≈≈..♣,♣♣♣,,,│                              │
│ 17                                                           │.≈≈...≈≈ƒ≈≈≈≈≈≈≈≈~."""♣,,,,,,,,,,,,¥%τ,♣♣♣%,.≈≈≈≈≈≈≈ƒ.≈.,,,,♣,,,│                              │
│ 18      let beds = find("Bed", camp, 15).len();              │≈≈≈ƒ≈≈≈≈≈≈≈≈≈≈≈≈≈≈..",♣♣,,,,,♣,♣,♣,,¥,,,,♣,¥.≈≈≈≈≈≈≈≈≈..,,,,,,,♣│                              │
│ 19      if beds < people + 1 && store("Wood") >= 5 {         │≈≈≈≈≈≈≈≈≈≈≈≈≈≈≈≈≈≈≈.",,♣,♣♣,,,,,♣,,♣,,,,,,,,.≈≈≈≈≈≈≈≈≈≈..,♣♣,♣,♣│                              │
│ 20          let r = build_near("Bed", camp);                 │≈≈≈≈≈≈≈≈≈≈≈≈≈≈≈≈≈≈≈."♣,♣,,,♣,,♣,,,♣,♣♣♣,♣♣,,..≈≈≈≈≈≈≈≈≈≈.%,,,♣,,│                              │
│ 21          if !r.ok { log(`bed: ${r.msg}`); }               │≈≈≈≈≈≈≈≈≈≈≈≈≈≈≈≈≈≈.."♣,,,,,,,,,☺,♣,,,,,♣,♣,♣.≈≈≈≈≈≈≈≈≈≈≈≈.τ,,,,¥│                              │
│ 22▁     }                                                    │≈≈≈≈≈≈≈≈≈≈≈≈≈≈≈≈≈≈.♠♠♣♣♣,,,,,,%%☺,,,♣,♣♣,♣,..≈≈≈≈≈≈≈≈≈≈≈≈.,♣,♣,♣│                              │
│ 23▌     stalled("food", store("Fruit"), 30);                 │≈≈≈≈≈≈≈≈≈≈≈≈≈≈≈≈≈≈.♠,,,♣♣♣♣♣♣♣,¥¥,,♣,,,,,,♣.≈≈≈≈≈≈≈≈≈≈≈≈≈.,,,,,.│                              │
│ 24      wait(5);                                             │≈≈≈≈≈≈≈≈≈≈≈≈≈≈≈≈≈≈.♠,♣♣,♣,♣,,♣♣,,,,,,,♣♣♣,♣.≈≈≈≈≈≈≈≈≈≈≈≈..,,,♣..│                              │
│ 25  }                                                        │≈≈≈≈≈≈≈≈≈≈≈≈≈≈≈≈≈..♠,,,♣,♣♣,♣,%,,,,♣♣♣,♣♣...≈≈≈≈≈≈≈≈≈≈≈≈.,,,...≈│                              │
│                                                              │≈≈≈≈≈≈≈≈≈≈≈≈≈≈≈≈≈.♠♠,,,,♣,,,,♣,τ...♣,,,...≈≈≈≈≈≈≈≈≈≈≈ƒ...,,..≈..│                              │
│                                                              │≈≈≈≈≈≈≈≈≈≈≈≈≈≈≈≈≈.¥♠♠"♠,♣,♣,%,,..≈..♣,..≈≈≈≈≈≈≈≈≈≈≈≈≈..☺,,..ƒ..,│                              │
│                                                              │≈≈≈≈≈≈≈≈≈≈≈≈≈≈≈≈≈......,,,,,,¥,.≈≈≈..,.≈≈≈≈≈≈≈≈≈≈≈≈≈..,,,..≈≈.,,│                              │
│                                                              │≈≈≈≈≈≈≈≈≈≈≈≈≈≈≈≈≈≈≈≈≈≈....,,,,,.≈≈≈≈.,.≈≈≈≈≈≈≈≈≈≈≈≈≈.,,,,.≈...,,│                              │
│                                                              │≈≈≈≈≈≈≈≈≈≈≈≈≈≈≈≈≈≈≈≈≈≈≈≈≈...,,,☺≈≈≈≈....≈≈≈≈≈≈≈≈≈ƒ≈.☺,,,,...,,♣♣│                              │
│                                                              │≈≈≈≈≈≈≈≈≈≈≈≈≈≈≈≈≈≈≈≈≈≈≈≈≈≈≈..,,☺≈≈≈≈≈ƒ≈.≈≈≈≈≈≈≈≈≈...,,,,,,,,,,,♣┗─ seed 7 ─────────────────────┛
│                                                              │ AGENT earth  FOLLOW  turn 20  06:40  x1                         acts ▁▁▃▁▁▁█▁▁▂▁▁▁▁▄▁▁▁▁▂ 7
│                                                              │┌ AGENT ── 11 calls · 3 runs ──────────────────────────────────────────────────────────────────┐
│                                                              ││ › briefing                                                                                   │
│                                                              ││ › sdk_reference                                                                              │
│                                                              ││ › save_script keep-fed v1   new, 22 lines                                                    │
│                                                              ││ › run_script keep-fed v1 · 5t → budget, 3 acts                                               │
│                                                              ││ › save_script keep-fed v2   +4 −1                                                            │
│                                                              ││ › run_script keep-fed v2 · 15t → woke: need: Thirsty first shown                             │
│                                                              ││ › notes + "thirst wakes me too late; check needs each loop"                                  │
│                                                              ││ › save_script keep-fed v3   +9 −2                                                            │
│                                                              ││ › run_script keep-fed v3 · 45t   turn 20/45 · 12 acts                                        │
│                                                              ││     · designate harvest 1x1 at (93,65) "standing food"                                       │
└ { } revision ─ j k scroll ─ 1 2 3 zoom ──────────────────────┘└──────────────────────────────────────────────────────────────────────────────────────────────┘
```

## The code pane

### Which revision is shown, and when

- **A revision** is one distinct `code` for a script name, in log order. For a saved script it's
  the `version` (each `save_script` bumps it). For an ad hoc `run_script {code}` (version 0,
  tagged by its note, or `play` for `fn_agent play`) it's the count of distinct codes seen
  under that tag. Re-running the same code doesn't add a revision.
- **The pane follows the latest replayed `script` line with `turns > 0`**, which is a run
  starting. This is the owner's "version running at that point in the replay". Until the first
  run, the pane shows the latest saved script, marked `saved` instead of `run Nt`. A
  `save_script` that comes after a run doesn't take the pane over. It shows in the feed, and the
  next run picks it up.
- **The watcher scans `script` lines as they are fed, not as they are replayed**, so `M` in
  `rev N/M` counts every revision in the log read so far. Live, that's usually `N/N`. In a
  replay, it shows how far into the agent's iteration you are.

### The header

`keep-fed.rhai ── rev 3/9 ── run 45t ── +9 −2`, where:

- `run 45t` is the run's turn budget while it runs. After the run, it becomes `woke 23t` (turns
  actually run, the same number #482's message-log line uses), or `saved` before any run.
- `+A −D` counts lines added and deleted against the previous revision of the same name. For
  rev 1 the header shows `new` instead.

### Highlighting: `fn_ui::code::highlight`

A hand-written Rhai lexer in **`fn_ui::code`**. It's a plain function with no I/O, testable
on its own, and fn_ui already holds the frontend-neutral view logic. It classifies, it doesn't
style:

```rust
pub enum Class { Plain, Keyword, Literal, Number, String, Comment, Call, Op }
pub struct Span { pub start: usize, pub end: usize, pub class: Class } // byte range in the line
pub struct LexState { /* open block comment depth, open backtick string */ }
pub fn highlight_line(line: &str, state: &mut LexState) -> Vec<Span>;
pub fn highlight(src: &str) -> Vec<Vec<Span>>; // one Vec per line
```

- **Keywords:** Rhai's (`let const fn if else switch while loop do until for in break continue
  return throw try catch private this global import export as`), with `true false ()` as
  `Literal`.
- **Strings:** `"..."`, `'c'` and backtick strings. A backtick string is a `String` that spans
  lines (hence the carried `LexState`), and its `${...}` is lexed as code.
- **Comments:** `//`, `///` and nested `/* */`, also spanning lines.
- **`Call`:** an identifier directly followed by `(`. That covers SDK and porcelain calls
  without the TUI having to know their names. `fn_tui` can't depend on `fn_agent`, which
  depends on it.
- **Rejected: pulling in `rhai`'s own tokenizer.** It sits behind the `internals` feature, it
  would put a script engine into the TUI's dependency tree, and a display lexer can be wrong in
  harmless ways. A test runs the lexer over `settle.rhai` and `prelude.rhai` to check that it
  never panics and that every keyword in them comes back as `Keyword`.

`fn_tui` maps each `Class` to a colour from `data/tui.toml` `[code]`. With `--ascii`, colour
still applies (`--ascii` only changes glyphs). With no colour, the code is plain text, and the
diff marker column still carries the diff.

### Diff: `fn_ui::code::line_diff`

```rust
pub enum Mark { Same, Added, RemovedAfter } // per line of the new revision
pub fn line_diff(old: &str, new: &str) -> (Vec<Mark>, usize /*added*/, usize /*deleted*/);
```

- **The algorithm is a plain LCS over lines.** Scripts are tens to a few hundred lines, so
  `n × m` is at most around 10⁵ cells. Above 10⁶ cells it gives up and marks every line `Added`.
  It's deterministic, needs no new dependency, and is about 40 lines plus tests. The `similar`
  crate (Myers or patience) is the fallback if the diffs come out ugly in practice.
- **A changed line counts as one deleted and one added**, the way `diff` counts it. This is what
  the `+A −D` header shows.
- **Deleted lines aren't drawn.** The pane shows the code that runs. A deletion leaves a `▁`
  (`_` with `--ascii`) in the marker column of the line just above the gap, in the
  `removed` colour.
- **Added lines** get `▌` (`+` with `--ascii`) in the marker column, plus the `added_bg` tint
  across the line.

### Gutter, scrolling, long lines, two columns

- **The gutter** is the line number, right-aligned to the width of the largest number, then the
  marker column, then a space.
- **Long lines are cut with `…`** (`>` with `--ascii`). They aren't wrapped, because wrapping
  breaks the one-row-per-line reading of a diff.
- **Auto-scroll:** when the shown revision changes, the pane scrolls so that the first marked
  line sits a third of the way down. `j`/`k` scroll one line, and `J`/`K` scroll half a pane.
  Your scroll position holds until the revision changes.
- **Two columns, newspaper order:** when the script doesn't fit in the pane's height and each
  column would still get at least `two_column_min` (56) code characters. At the game column's
  120-column cap, that happens from about 250 columns wide.

## The agent feed

### A new log line: `call`

```json
{"type":"call","tool":"observe","summary":""}
{"type":"call","tool":"run_script","summary":"keep-fed 45t"}
{"type":"call","tool":"act","summary":"2 steps \"wood for two beds\""}
```

- **`LogLine::Call { tool: String, summary: String }`** goes in `fn_tui::surface`, next to
  `Note`.
- **`fn_agent::tools::Tools::call` writes it before `dispatch`.** That way it comes before the
  call's own `keys`, `look`, `script` and `wake` lines, and a live watcher sees "› run_script
  keep-fed 45t" the moment the script starts. That matters because a script run's own lines
  only land when the run ends (`run_script` flushes them before its `wake`).
- **`summary` is built from the arguments only**, capped at 60 characters: `look` gives
  `(x,y) wxh`, `act` gives `N steps` plus the note, `run_script` gives `name Nt` (or `ad hoc Nt`;
  never the code), `end_turn` gives `N`, `screen` gives its name, `frame` gives `WxH`, and `notes`
  gives `append` or nothing. Results come from the lines that follow, so there's no wall-clock
  time and no result text. That keeps the log deterministic for a given call sequence. A
  refused call (bad arguments) shows as a call with nothing after it. That's acceptable for now.
- **Ignored like `note`:** `to_keys_script`, `Host::resume`, `Tools::resume` (`rebuild_session`)
  and `verify` skip `call`. A test strips every `call` line from a served log and checks that
  `fn_agent keys` and a `--resume` give the same keys and final digest. `fn_agent play` makes no
  MCP calls and writes no `call` lines, so its byte-for-byte CI check doesn't change.
- **The message log (`L`) doesn't get `call` lines.** It already has an Agent line for each act,
  run and wake, so call lines would only be noise there.

### What the feed shows

Each `call` line opens one entry: `› tool summary`. The lines after it, up to the next `call`,
fill that entry in:

| Lines after the call | The entry reads |
|---|---|
| `script` with `turns: 0` | `› save_script keep-fed v3   +9 −2` (`new, 22 lines` for rev 1) |
| `script` with `turns > 0` … `wake` | `› run_script keep-fed v3 · 45t`, then `turn 20/45 · 12 acts` while it runs, and a dimmed sub-line with the script's latest act label. On `wake`: `→ woke: why`, or `→ budget, 3 acts` / `→ done` / `→ error: …` |
| `keys` (outside a run) | `› act designate harvest 6x4 "wood"`, with every label it pressed joined by `;` |
| `turn` ×N (outside a run) | `› end_turn 3` |
| `look` / `inspect` | `› look (80,60) 6x4` |
| `note` | `› notes + "thirst wakes me too late"` |

- **Logs with no `call` lines** still work: `fn_agent play` logs, and logs written before this
  change. The same lines then open their own synthetic entries, so a run reads
  `› run_script play · 40t → woke: need: Thirsty first shown`.
- **The feed only shows what's been replayed**, so a replay never shows the future. Newest is at
  the bottom and it tails automatically. To read further back, zoom it (`3`).
- **Its title** counts what's been replayed so far: `AGENT ── 11 calls · 3 runs`.

## The sparkline and the strip

- **Data:** `acts: Vec<u16>` indexed by the watcher's turn. Each replayed `keys` line adds one to
  the current turn when its first key is pressed. Script acts and MCP acts count the same, and
  reads don't count. The rightmost bar is the turn in progress. `--frame --at N` fills it in the
  same way, so it's golden-testable.
- **Window:** the last `spark_turns` turns (24 in the studio, which is 8 game minutes at
  20-second turns; 10 in tabs).
- **Scale:** each bar is `n / max(window peak, spark_floor)`. With `spark_floor = 4`, a quiet
  turn with one act doesn't fill the bar. Zero draws `▁` dimmed, and 1 and up draw `▂`…`█`.
  With `--ascii` the ramp is `_.:-=+*#`. The window's peak follows as a number (`… 7`).
- **The studio strip** is the game pane's bottom row. It drops `SCRIPT name vN` and the action
  text, because the code header and the feed now carry them. It reads
  `AGENT earth  FOLLOW  turn 20  06:40  x1 … acts ▁▁▃▁▁▁█▁▁▂▁▁ 7`.
- **The tab strip** keeps today's text and adds the sparkline and the tab indicator on the
  right. The action text is cut first when space runs short.

## The tab fallback

Below 160x45 (and at or above the existing 80x24 minimum) the three panes become tabs. The game
tab is the default, so a small terminal shows today's watcher plus a few characters of strip.
Here's the bottom of a real 110x34 `--ascii` frame with the new strip:

```text
~~~~~~~~~~~~~~~~~~~~~~....,,,,,.~~~~.,.~~~~~~~~~~~~~.,,,,.~...,,,,..="""F,,F"F|                              |
~~~~~~~~~~~~~~~~~~~~~~~~~...,,,@~~~~....~~~~~~~~~f~.@,,,,...,,TT,,,,"="""T,T,F|                              |
~~~~~~~~~~~~~~~~~~~~~~~~~~~..,,@~~~~~f~.~~~~~~~~~...,,,,,,,,,,,TT,,T""="FT,,,,+- seed 7 ---------------------+
 AGENT earth  FOLLOW  turn 20  06:40  SCRIPT keep-fed v3  designate harvest 1x1 at (93> acts __-___#__= 1[2]3
```

The code tab at the same size (`--keys 1`):

```text
+ keep-fed.rhai -- rev 3/9 -- run 45t -- +9 -2 -- { } revision  j k scroll ----------------------------------+
|  1  // keep-fed v3: standing food orders, a bed one ahead,                                                 |
|  2+ // and a wake of my own before thirst kills anyone.                                                    |
|  3  let camp = memory.camp ?? observe().cursor;                                                            |
|  4  memory.camp = camp;                                                                                    |
|  5                                                                                                         |
|  6  loop {                                                                                                 |
|  7      designate_near("harvest", "Wild Cabbage", camp, 15, true);                                         |
|  8      designate_near("harvest", "Fruit Tree", camp, 15, true);                                           |
|  9                                                                                                         |
| 10+     let g = observe();                                                                                 |
| 11+     let people = g.people.total;                                                                       |
| 12+     let thirsty = g.people.needs.Thirsty ?? 0;                                                         |
| 13+     if thirsty > 0 {                                                                                   |
| 14+         log(`thirsty: ${thirsty}`);                                                                    |
| 15+         wake(`${thirsty} thirsty: water plan needed`);                                                 |
| 16+     }                                                                                                  |
| 17                                                                                                         |
| 18      let beds = find("Bed", camp, 15).len();                                                            |
| 19      if beds < people + 1 && store("Wood") >= 5 {                                                       |
| 20          let r = build_near("Bed", camp);                                                               |
| 21          if !r.ok { log(`bed: ${r.msg}`); }                                                             |
| 22_     }                                                                                                  |
| 23+     stalled("food", store("Fruit"), 30);                                                               |
| 24      wait(5);                                                                                           |
| 25  }                                                                                                      |
|                                                                                                            |
|                                                                                                            |
|                                                                                                            |
|                                                                                                            |
|                                                                                                            |
|                                                                                                            |
+------------------------------------------------------------------------------------------------------------+
 AGENT earth  FOLLOW  turn 20  06:40  SCRIPT keep-fed v3  designate harvest 1x1 at (9> acts __-___#__= [1]2 3
```

- The feed tab is the feed, full screen, under the same strip.
- **Resizing switches** between the studio and tabs live, and keeps the zoom you chose.
- In the studio, `1`/`2`/`3` zoom a pane, and the same digit again (or `0`) goes back to the
  split. Below the threshold, `0` does nothing, and the strip briefly says
  `studio needs 160x45`.

## Keys

Watcher keys win over free look (#330). Free look passes every other key to the spectator.

| Key | Does | Why this key |
|---|---|---|
| `1` `2` `3` | Zoom or tab: code, game, feed | Digits are the game's nation switch, which free look already refuses (`input::spectator_allowed`), so they're free |
| `0` | Back to the studio split | Same reason |
| `j` `k` / `J` `K` | Scroll the code pane a line / half a pane | Not bound anywhere in the game (`input::build_table`) |
| `{` `}` | Previous / next revision of the shown script (#502) | The owner suggested `[`/`]`, but `]` already jumps to the end of the next turn. See Q2 |

No F-keys (WSL never delivers them). Every studio key goes through one new
`Watcher::key(KeyEvent) -> bool`, called by both `watch::run`'s loop and the
`--watch --frame --keys` path, so all of them can be golden-tested without a terminal.

**Revision stepping** pins the pane to the chosen revision. The header shows
`rev 2/9 · pinned`, and the diff is still against that revision's predecessor. While a pane is
pinned, a new run only changes the header's `running v4` hint. Stepping back to the running
revision unpins it.

## Data: `data/tui.toml`

```toml
# The watcher's studio layout (docs/design/agent-studio.md): presentation only.
[studio]
min_width = 160          # below this (or min_height) the panes become tabs
min_height = 45
game_share = 0.6         # the game column's share of the width,
game_min_width = 96      #   clamped to these
game_max_width = 120
feed_min_rows = 12       # the feed is max(feed_min_rows, height / 4) rows
two_column_min = 56      # code characters per column before the pane splits in two
spark_turns = 24         # sparkline window in the studio
spark_turns_tabs = 10    # and in the tab strip
spark_floor = 4          # acts per turn that fill a bar, at least

# Rhai syntax colours for the studio's code pane, by fn_ui::code::Class.
[code]
keyword = [198, 120, 221]
literal = [209, 154, 102]
number = [209, 154, 102]
string = [152, 195, 121]
comment = [110, 118, 129]
call = [97, 175, 239]
op = [171, 178, 191]
gutter = [92, 99, 112]
added_bg = [30, 60, 34]    # mixed into the line's background
added = [120, 200, 110]    # the ▌ marker
removed = [224, 108, 117]  # the ▁ marker
```

A `tui.toml` test checks that every `Class` has a colour.

## Recording

**Recommended (Q1): `fn_tui --watch LOG --record OUT.cast [--size WxH] [--pace N] [--fps N]`.**

- **It needs no terminal.** It drives the `Watcher` on a virtual clock (`advance(1 / fps)` per
  frame) and draws each frame through a ratatui `Terminal` over a `CrosstermBackend<Vec<u8>>`.
  Ratatui already sends only the cells that changed, so the frames stay small. Each frame's bytes
  become one asciicast v2 event at its virtual timestamp. It stops at the log's `end` line, or
  when a finished log is fully replayed.
- **The same log gives a byte-identical `.cast`.** It runs faster than real time, works in CI,
  and agents can make one (they must never launch `fn_tui` interactively). A year at pace x10
  is about six minutes of video, which takes a few seconds to render.
- **Turning it into video is the user's step.** `agg OUT.cast OUT.gif`, or `asciinema play`.
  The doc and `--help` say so, and nothing here shells out.
- **Alternatives:** an `asciinema rec -c "fn_tui --watch …"` wrapper script needs no Rust, but
  it records in real time, only a human with a terminal can run it, and it needs a way to quit
  when the replay finishes. VHS tapes need Chromium and ttyd. Both remain possible for the owner
  whatever is chosen.

## Testing

- `fn_ui::code`: unit tests for the lexer (classes, multi-line comments and strings, `${}`
  inside backticks, the fixture scripts) and for `line_diff` (identical, pure add, pure delete,
  change, empty, the cell cap).
- **Golden frames** (`crates/tui/tests/watch.rs`, logs generated in the test so a data change
  doesn't break a checked-in log): `--watch --frame --size 160x45 --at N` for the studio, two
  revisions apart; `--size 110x34` with `--keys 1`, nothing, and `--keys 3` for the tabs; one
  `--ansi` frame to pin the highlight and diff colours.
- A replay test checks that the code pane at turn N shows the revision whose `script` line was
  the last one replayed.
- `fn_agent`: a served session's log has exactly one `call` line per `tools/call`, written ahead
  of that call's own lines, and a log with its `call` lines stripped replays to the same digest.
- `--record`: two recordings of one log are byte-identical, and the header's width and height
  match `--size`.

## Not chosen

- **A `--studio` flag or toggle.** The owner placed the studio in `--watch`, and the tab
  fallback already covers small terminals.
- **Drawing deleted lines inline** (red, struck through). Then the pane would no longer show the
  code that runs, and the length of every revision would jump around. The marker and the
  `−D` count are enough.
- **Streaming a script's lines while it runs** (flushing on every `wait`). It would make live
  watching smoother, but replay already paces the lines once they land, and it changes
  `run_script`'s flush contract. It can be its own issue if live watching feels jumpy.
- **A "current line" marker** (which line pressed this act). Porcelain calls run inside
  `prelude.rhai`, so the position Rhai reports isn't a line in the agent's script. Getting the
  real line would need Rhai's debugger interface.
- **Result summaries in `call`** (observe's numbers, act errors). Results are only known after
  dispatch, which would put `call` after the lines it should head. The wake report and the act
  labels carry what matters.

## Follow-up issues

In dependency order:

- #497 fn_agent: log a `call {tool, summary}` line for every MCP tool call (independent, but it
  touches the same files as #334, so start it after #334 merges)
- #498 fn_ui::code: a Rhai line highlighter and an LCS line diff (independent)
- #499 Watch studio: the 160x45 layout and the code pane with highlighting and diff markers
  (blocked by #498)
- #500 Watch studio: the agent feed and the acts-per-turn sparkline (blocked by #497, #499)
- #501 Watch: the tab fallback below the studio size, and `1`/`2`/`3` zoom (blocked by #500)
- #502 Watch studio: step through script revisions (blocked by #499 and decision #505)
- #503 `fn_tui --watch --record`: a deterministic asciicast of a replay (blocked by #501 and
  decision #504)

## Open questions for the owner

1. **Recording: build `--record`, or leave recording to asciinema?** (#504) **Recommended:**
   build `--record`. It's deterministic, headless, faster than real time, and agents can use it.
   The alternative is a documented `asciinema rec` wrapper and no Rust, at the cost of real-time
   recording that only a human can run.
2. **Revision stepping keys.** (#505) `]` already means "jump to the end of the next turn".
   **Recommended:** `{`/`}` for revisions (the same physical keys with Shift), with `]` left
   alone. The alternative is to move the next-turn jump elsewhere (to `>`, say) and give `[`/`]`
   to revisions as first suggested.
