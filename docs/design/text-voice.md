# Text voice: how the game talks

Status: **proposed** (#401, from the owner's 2026-11-06 playtest notes). Applies to every string a
player reads: the TUI, `fn_ui` view text, and the descriptions, names and hints in `data/*.toml`.
It does not cover developer text (`fn_tui --help`, data validation errors, test messages, doc
comments).

## The problem

The owner's note: across the game's text there are "LLM-esque" instructions and asides alongside
simple descriptions, and the mix feels odd. The audit below bears that out. Most labels are fine
("Idle: out of chi", "Food in stores", "Learned Pottery"). The trouble is a handful of habits that
show up all over:

- **Instructions glued onto status.** A line reports a state, then a dash, then tells the player
  what to press: "Chi to spare, no bend designated yet - press B on a tile a bender can work."
  "No saves yet: press N to write one." The key belongs in the footer or key help, where it
  already is.
- **Asides that explain the rules.** Parentheses and trailing clauses add a second idea to a
  line: "Quarry outcrops and foothill boulders (bricks count)", "Mine gold veins ($); tunnels
  yield more", "Harvest tool: toggle a standing order (re-designates when the resource ripens
  again)", "Each finished bend counts for whoever channels it".
- **Developer notes that leaked.** The key help for Esc ends "(Resume, Save as…, Quicksave,
  Load…, Quickload, Key help, Quit, #306, #312) — every save and load action, for a terminal the
  function keys don't reach". Issue numbers and the reason a feature exists are for the repo, not
  the player.
- **Writerly tics in flavour text.** The "X, not Y" turn ("fire is life and energy, not rage"),
  name-dropping asides ("as in Ba Sing Se", "Republic City style"), "no forge needed", "but it
  kindles nothing", and intensifiers ("dramatically cheaper").
- **Narration mixed into readouts.** "Spring: the land wakes." sits in the same log as "42 wood
  can't be stored: every stockpile is full." "Welcome to the Four Nations." heads a plain menu.
  "Nothing has happened yet: no bends, no discoveries, no deaths."
- **Inconsistent mechanics.** Alerts end with a period about half the time. Joiners vary between
  " - ", "—", ";" and ":" for the same job. Footers say "Arrows" in one place and "arrows" in
  the next, and the screen frame and the bottom bar repeat the same keys with different words
  ("Enter target" vs "Enter set target").

## The voice

Two registers, kept apart. Every string is one or the other, never both.

**Interface text** is labels, readouts, alerts, prompts, key help and refusals. It's how a
well-made instrument panel talks: short, factual, the same words every time. It never narrates
and never explains why.

**Flavour text** is the one-line description of a tech, nation, ability, building or difficulty,
and the Endeavor's opening line. It's a short in-world sentence that says what the thing *does*
in plain terms. It can have colour from the setting, but it earns it with concrete nouns, not
with style.

### Rules for all text

1. **One idea per line.** If a line needs a dash, semicolon or parenthesis to add a second idea,
   cut the second idea or give it its own line.
2. **No glued-on instructions.** State the fact. Keys live in the footer, the bottom bar and the
   key help. If a line must point at a key (a panel hint with nothing else to show), the whole
   line is the hint, in the form `B: bend` or `Press B to bend`, never appended to a status.
3. **No explanations of the rules in passing.** "Bricks count" or "tunnels yield more" is a rule
   worth knowing, so it goes where rules live: the build modal's effects list, the inspect panel,
   or the item's own description. Not a parenthesis on a progress hint.
4. **Nothing from the repo.** No issue numbers, file names, platform workarounds, or the reason a
   feature was added.
5. **Name things the way the game names them.** Use the building, tech, ability and item names
   from data, in the same case, everywhere. "Study hall" is "Academy" if that's its name.
6. **Plain words over jargon.** "Research points" or a number with its unit, not "pts banked".
   "Needs Pottery", not "Needs prereqs". Avoid "designate", "channel", "credit" and "tick" in
   player text when a plainer word works ("mark", "bend", "counts", "moment").
7. **No "you" in interface text,** and no "your" except in the few lines that are about the
   player's choice (difficulty hints, the embark survey).
8. **Settlers are they;** canon characters keep their canon pronouns.

### Interface text

- **Labels and headings**: sentence case, no trailing period. `Food in stores`, `Can't build
  here: on a stockpile`.
- **Readouts**: `Label: value`, with the colon as the only joiner. `Idle: out of chi`,
  `Growing: ripe in 3m 20s`.
- **Alerts and log lines**: one plain sentence, past or present tense, **no trailing period**,
  subject first. `Haru raised earth at (5, 3)`, `Winter in 4 minutes: 120 food for 8 people`,
  `First frost: 12 wild plants died back`. A season change is a fact (`Winter has come`), not a
  mood.
- **Refusals** (why a bend failed, why a site is blocked): a lower-case fragment that completes
  the label before it. `Can't build here: a tree is in the way`, `Haru's bend failed: needs dry
  ground`.
- **Prompts**: the question, then the keys. `Load the quicksave? y/n`.
- **Footers and key help**: `Key action` pairs separated by two spaces, keys capitalised as on
  the keyboard (`Arrows`, `Enter`, `Esc`, `PgUp`), actions as short verbs. The bottom bar and the
  screen frame show the same keys in the same words, or only one of them shows them. Key help
  rows are one short clause; a key that does different things in different modes gets one row
  per mode.
- **Empty states**: say what's empty, nothing else. `No saves`, `No messages`, `Nothing to
  build`.

### Flavour text

- **One sentence, at most about 70 characters**, ending with a period.
- **Say what it does** in game terms first; setting colour is optional and comes from concrete
  things (sky bison, glider staffs, the Sun Warriors), never from comparisons ("as in X",
  "X style").
- **No "X, not Y"**, no "no X needed", no "but it does nothing for Y". If a limit matters, it's a
  fact in the effects list.
- **No intensifiers** ("dramatically", "truly", "incredibly"), no hedges ("a little while"), no
  exclamation marks.
- **Numbers live in readouts**, not in flavour text, so balance changes don't make it lie.

## Before and after

| Where | Today | Proposed |
|---|---|---|
| Alert | Chi to spare, no bend designated yet - press B on a tile a bender can work. | Benders have chi to spare |
| Endeavor hint | Designate a bend (B) on a tile a bender can work - credit to whoever channels it | Bends count for the bender who makes them |
| Endeavor hint | Quarry outcrops and foothill boulders (bricks count) | Quarry outcrops and boulders |
| Endeavor hint | Mine gold veins ($); tunnels yield more | Mine gold veins |
| Key help, Esc | Close the screen, a modal or the bar. In a tool: drop the corner, then leave the tool. With nothing to close: the game menu (Resume, …, #306, #312) — every save and load action, for a terminal the function keys don't reach | Three rows: `Esc  Close a screen or the bar` / `Esc  In a tool: drop the corner, then leave` / `Esc  Open the game menu` |
| Key help, O | Harvest tool: toggle a standing order (re-designates when the resource ripens again) | Harvest tool: repeat the order when it regrows |
| Saves screen | No saves yet: press N to write one. | No saves |
| Event log empty | Nothing has happened yet: no bends, no discoveries, no deaths. | No messages |
| Title | Welcome to the Four Nations. | (removed) |
| Season | Spring: the land wakes. | Spring has come |
| Season | Winter. Wild plants rest, and the cold tires people away from fires. | Winter has come: wild plants stop growing |
| Research | Firebenders feed lightning into a power station, Republic City style. | Firebenders feed lightning into a power station. |
| Research | Stone carts pushed along rails by earthbenders, as in Ba Sing Se. | Earthbenders push stone carts along rails. |
| Research | The Sun Warriors' teaching: fire is life and energy, not rage. | The Sun Warriors' teaching: fire as the source of life. |
| Research panel | Unlocks: Prerequisite for Carpentry | Leads to: Carpentry |
| Nation | Stubborn builders. Earthworks, quarrying and walls are dramatically cheaper. | Stubborn builders. Earthworks, quarrying and walls cost less. |
| Ability | Raise a dome of still air. No cold within reach, but it kindles nothing. | Raise a dome of still air that keeps out the cold. |
| Ability | Lend a building site extra work. Banked work waits for the materials to arrive. | Speed up work on a building site. |

The proposed column is a direction, not the final copy: each follow-up issue writes its own.

## Audit and follow-up work

The rewrite splits by file so agents can run in parallel without conflicts. Each issue is
blocked by #401, and each must leave golden frames re-blessed with the diff reviewed.

| Area | Files | Main problems | Issue |
|---|---|---|---|
| Alerts and Endeavor text | `crates/ui/src/alerts.rs`, `crates/ui/src/endeavor.rs`, `data/endeavor.toml` | Glued instructions, rule asides in hints, mixed trailing periods, season narration | #405 |
| Key help, footers and prompts | `crates/tui/src/input.rs`, `draw.rs`, `screens.rs`, `title.rs`, `embark.rs`, `menu.rs`, `saves.rs`, `panel.rs` hints, `state.rs`, `crates/ui/src/tools.rs` | Issue numbers in help, multi-clause rows, duplicate and inconsistent footers, "press N" empty states, welcome line | #406 |
| Inspect, build and status readouts | `crates/ui/src/inspect.rs`, `build.rs`, `text.rs`, `research.rs`, `view.rs`, `crates/tui/src/panel.rs` (rows), `crates/tui/src/research.rs` | Parenthesised asides, " - " joiners, jargon ("pts banked", "Needs prereqs", "Unlocks: Prerequisite for") | #408 |
| Flavour text in data | `data/research.toml`, `data/bending.toml`, `data/nations.toml`, `data/difficulty.toml` | "X, not Y", "as in"/"style" asides, intensifiers, limits phrased as asides | #409 |

Known overlaps: #381 rewords the standing harvest notice and #393 adds harm alerts. Whichever
lands second follows this guide and rebases over the other.

## Open questions for the owner

1. **Alerts without a trailing period?** Recommended: yes, every log line and toast is a fragment
   with no period, matching most of today's lines. Flavour text keeps its period.
2. **Should season changes keep any mood?** Recommended: no. "Winter has come" plus the effect.
   The setting's colour lives in flavour text and art.
3. **Canon name-drops in tech blurbs** (Ba Sing Se, Republic City)? Recommended: drop the
   comparison form, keep canon nouns where they describe the thing itself (sky bison, the Sun
   Warriors).
4. **One set of keys per screen?** Recommended: the screen frame's footer is the one place a
   screen's keys show, and the bottom bar on a screen shows only global keys. #406 does this.
