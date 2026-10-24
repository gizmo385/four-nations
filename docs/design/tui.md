# Terminal UI (Dwarf Fortress-style)

Status: **proposed** (#93). Follow-up work: #96 to #100 (see the end of this doc). The
owner decisions are under "Open questions for the owner".

The owner wants to play the whole game from a terminal: a glyph map, keyboard menus, no window.
`fn_sim` is already headless and the Bevy client changes the world only through
`World::apply` and `World::tick`, so a second frontend needs no new game rules. It also gives
agents a way to play and check the game without opening the GUI.

## Decisions in brief

1. **Two new crates, no Bevy.** `crates/ui` (`fn_ui`) holds the frontend logic that both
   frontends need today: tools and their commands, the text that describes units and tiles, view
   models, theme tokens and the session clock. `crates/tui` (`fn_tui`) is the terminal binary,
   on **ratatui** with the **crossterm** backend. Both depend on `fn_sim`, and neither on Bevy.
2. **Same screen as the Bevy client, in cells.** The map fills the left side, the Seal side
   panel sits on the right, the selection bar runs along the bottom, and a status line sits below
   everything. The hover tooltip becomes a fixed **Cursor** section in the side panel, because the
   keyboard cursor *is* the hover.
3. **One cell per tile, glyphs and colours from data.** A new `data/tui.toml` gives every
   terrain, biome, resource, item, building and overlay a Unicode glyph, an ASCII fallback and
   sRGB colours. Units take their nation's `color`. Colour is quantised automatically for
   256- and 16-colour terminals.
4. **Keyboard first, Dwarf Fortress-style.** Arrow keys move a cursor, the view scrolls to
   follow it, and a rectangle is designated with **tool key, Enter, move, Enter**. Tool hotkeys
   are the client's: `H` `X` `P` `B`, `T` for research, `Space` to pause.
5. **A deterministic clock.** `fn_ui::Clock` turns real elapsed time and a speed into a whole
   number of ticks. Commands apply only between ticks, and `World` logs the tick of each one, so
   a session replays exactly, whatever the frame rate was.
6. **Headless by construction.** Rendering is a pure function into a ratatui `Buffer`.
   `fn_tui --frame` prints one frame to stdout, after an optional scripted key sequence, and
   exits. Golden-frame tests use the same `FN_BLESS=1` convention as the mapgen goldens.

## Crates

```
crates/sim   fn_sim   game rules (unchanged)
crates/ui    fn_ui    frontend logic shared by every UI; depends on fn_sim only
crates/tui   fn_tui   terminal binary: ratatui + crossterm; depends on fn_sim and fn_ui
crates/client fn_client  Bevy; depends on fn_sim and fn_ui
```

**ratatui 0.30 + crossterm 0.29**, pinned in the PR that adds the crate.

- ratatui is the standard Rust TUI library. It renders immediate-mode into a cell `Buffer` and
  diffs frames, which matches how the HUD already rebuilds from plain view structs.
- Its `TestBackend` renders without a terminal, which is what makes golden-frame tests and
  `--frame` cheap (see "Verifying headlessly").
- crossterm is pure Rust. It works on Linux, WSL and Windows, and needs no ncurses or other
  C library, which suits the Nix devShell. It also handles mouse events for later.
- Not chosen: **cursive** (retained widgets driven by callbacks, harder to snapshot);
  **termion** (Unix only); **bracket-lib** (opens its own window, which defeats the point);
  raw crossterm (we would rebuild layout and diffing).

**CI.** `fn_ui` and `fn_tui` join the `sim` scope in `xtask` (`-p fn_sim -p fn_ui -p fn_tui -p
xtask`). Neither pulls in Bevy, so `cargo xtask ci sim --fast` gates the TUI in seconds. A test
in `fn_tui` checks `cargo metadata` and fails if `bevy` ever enters its dependency tree.

## Screen layout

Colour mockups: [Unicode glyphs](https://github.com/gizmo385/four-nations/blob/8372ae68fd21126bfb343ba0af50c7a4cd99c418/mockups/93/tui-unicode.svg)
and [ASCII fallback](https://github.com/gizmo385/four-nations/blob/8372ae68fd21126bfb343ba0af50c7a4cd99c418/mockups/93/tui-ascii.svg),
hand-built over seed 7's real map (`mockups/93/mock.py` on the reference branch). As text, at
100×32:

```
♠"",♣,♣,,,♣,,,,,,,•▲▲,,,,τ,,,,,..≈≈≈≈≈≈≈..,,,,,,.≈..≈≈≈.",♠♠♠♠♠♠"♠♠♠┏───── ◆ EARTH KINGDOM ◆ ──────┓
♠""♠,,,♣♣,♣,,τ,τ,,•▲•,,,~,,,,,,.≈≈≈≈≈≈≈≈ƒ..,,,,,.....≈.."♣,"♠♠♠♠♠♠▲▲│ Seed 7                 03:12 │
♠♠♠♠♠♠",♣♣,,,,,♣,,,•▲,,,,~,τ,♣..≈≈≈≈≈≈≈≈≈≈.,,,,,τ,,,.≈.,"♣,,"♠♠"♠"♠•│ ▶ Running x2   Space  + -    │
♠♠♠♠♠"""τ,,,,,,,,,•▲▲τ,,,,~,,..≈≈≈≈≈≈≈≈≈≈≈..,,,,,¥...≈.,,"τ"♠♠♠♠♠""▲│                              │
♠♠♠"♠♠♠♠"♠,,,,,,,,▲▲•,,,...~..≈≈≈≈≈≈≈≈≈≈≈≈≈..,,¥...≈≈≈.,♠""""""♠"""♠│ ◇ PEOPLE                   6 │
♠♠♠♠♠♠♠♠♠♠"♠,,¥,,,▲•,,,♣.≈ƒ≈≈≈≈≈≈≈≈≈≈≈≈≈≈≈≈≈.,,,.≈≈≈≈≈."♠"♠♠"""¥""""│ Haru   Chopping Oak Tree     │
τ♠♠♠♠♠♠♠♠♠~,,,,,,,•,,,,,.≈≈≈≈≈≈≈≈≈≈≈≈≈≈≈≈≈≈≈.,,,.≈≈≈≈≈~""""""""""""♠│ Tyro   Hauling 5 Wood        │
♠♠♠♠♠♠"♠"~",,,,,,,♣,,,,,...≈≈≈≈≈≈≈≈≈≈≈≈≈≈≈≈≈.,,,.≈≈.≈≈.~""""""""""♠"│ Oma    Sleeping              │
♠♠♠♠♠♠♠♠~♠τ,,,,,,,,,♣,♣,,,....≈≈≈≈≈≈≈≈≈≈≈≈≈..,,,...≈≈≈.♠~"""♠"""""""│ Shu    Raising stones        │
.♠♠♠♠♠♠~♠♠,,,,,¥,,,,,♣τ,,,,,,..≈≈≈≈≈≈≈≈≈≈≈≈.♣,♣,,,.....""~""""...""♠│ +2 more               [Tab]  │
...♠♠♠"~"♣,,,,,,♣,,♣,,,,,,,,,,.≈≈≈≈≈≈≈≈≈≈.≈.,,,,,,,.≈.,♠""~~"♠.≈.τ¥"│ ◇ STORES                     │
≈≈..♠♠~♠",,,,,,♣,,,♣☺♣,,,,,,,,..≈≈≈≈≈≈≈≈....,,,,,,,...,"""""~ƒ~≈."""│ Wood 12  Stone 4  Cabbage 3  │
≈≈≈..~♠"♠♣♣♣≡♣,,,,,,,,,,,,,♣,☺,.≈≈≈≈≈≈≈≈≈..♣♣,,,,,,,,,♣,""♠"""..~~"~│ ◇ RESEARCH                   │
≈≈≈≈ƒ.♠♠♠,♣♣♣♣,,,,♣♣,,♣☺,,,,,,,.ƒ≈≈≈≈≈≈≈.≈.♣,,,,,τ,,,,,,,"♠""""♠♠♠~♠│ Masonry                      │
≈≈≈≈≈.."♣,♣♣♣,♣♣,♣,♣,,,,,☺,,,♣,.≈≈≈≈≈≈≈≈≈..,,,♣,,,,τ,,,,,""♠"♠"♠♠"♠"│ ██████████░░░░ 68%  7.2/min  │
≈≈≈≈≈≈."♣,,,♣,♣,♣¥,,,☺,♣,,≡≡%♣,.≈≈≈≈≈≈≈≈ƒ≈..,¥,,,,,,,,,,¥,,,,"""♠♠♠"│ ◇ TOOL  [H] HARVEST          │
≈≈≈≈≈≈."♣♣,,♣♣,,,τ,,♣,♣,♣,■,,♣,..≈≈≈≈≈≈≈≈≈≈..,,,,,,,,,,♣,,♣",,,♠"♠♠"│ Chop, quarry, gather, mine   │
≈≈≈≈≈.."♣♣,,,,♣,,,♣,♣,,,☺,,♣♣♣,.≈≈≈≈≈≈≈≈≈≈≈≈.,,,,τ,,,,,""♠","♠"♠♠"♠"│ Corner set at (77, 67).      │
≈≈≈≈≈."♠♣,♣♣,,♣,♣♣♣,,,,,,♣,♣♣,..≈≈≈≈≈≈≈≈≈≈≈≈.,,♣,,,,,,"""""""♠♠♠♠"""│ ◇ ALERTS                     │
≈≈≈≈≈."♣♣,♣,,,♣,,,,,,,,,,,,,♣♣.≈≈≈≈≈≈≈≈≈≈≈≈≈.,,,,,....~~░♠♠♠♠"♠♠"♠♠"│ Shu raised stones (119, 70)  │
≈≈≈≈≈.♠♣,♣,,,,♣♣,♣,,,♣,,,,,,,,.≈≈≈≈≈≈≈≈≈≈≈≈..,,,,..≈≈~,""~"♠~~♠♠♠♠♠♠│ Learned Stoneworking         │
≈≈≈≈..",♣♣,τ♣¥♣♣♣,♣,♣,,,,,,,...≈≈≈≈≈≈≈≈≈≈≈≈.,,,...≈≈≈.,,♠"~~♠♠~♠""♠♠│ ◇ CURSOR  (81, 69)           │
≈≈≈≈.♠"♣,,,♣♣¥¥,♣,♣...,,,,...≈≈≈≈≈≈≈≈≈≈≈≈...,,..≈...ƒ..,,♠♠♠"♠♠~"♠♠♠│ Oak Tree                     │
≈≈≈≈."""""♣,,♣,,♣,..≈..,,..ƒ≈≈≈≈≈≈≈≈≈≈≈≈..,,,..≈..,..≈..,♣♠♠♠"♠♠~ƒ♠♠│ Plains - Land                │
┏─ Haru · Earth Kingdom ───────────────────────────────────────────┓│ Chop: 3 Wood - 8s            │
│ Chopping Oak Tree at (76, 64), 40% done  Health ██████████ 100%  ││ Enter: designate 5x3         │
│ Carrying: 2 Wood                         Hunger ███████░░░  72%  ││                              │
│ At (86, 64)                              Thirst █████░░░░░  51%  ││                              │
│                                          Energy ██░░░░░░░░  23%! ││                              │
│ [Esc] close  [Tab] next  [F] follow      Chi    ██████░░░░  60%  ││                              │
┗──────────────────────────────────────────────────────────────────┛┗──────────────────────────────┛
 HARVEST  arrows move  Shift x10  Enter corner  Esc cancel  T research  ? keys  q quit
```

In the colour mockup the harvest rectangle being dragged and the queued harvests are amber
backgrounds, the stockpile is blue, the cursor is yellow and the selected unit is reversed.

| Zone (ui.md) | TUI | Size | Contents |
|---|---|---|---|
| **Side panel** | Right edge, full height above the status line | 32 columns | Seal frame: `accent_soft` box lines, `accent` corners `┏┓┗┛`, title `◆ NATION ◆` centred on the top edge, `◇` section bullets. Sections in ui.md order: header (seed, clock, speed); **People** (count, then one line per unit while they fit); **Stores**; **Research**; **Tool** (hint and notice); **Alerts**; **Cursor**. Empty sections hide, as in ui.md |
| **Selection bar** | Bottom of the map area, left of the side panel | 7 rows | Name and nation in the top edge; task, carrying, tile; meters as 10-cell bars in `ok`/`warn`/`bad`, with a `!` for anything not ok (readable without colour); chi in `accent`; key hints. No portrait. Present only while a unit is selected |
| **Hover tooltip** | The **Cursor** section | up to 4 lines + title | The client's tooltip lines for the tile under the cursor, same order (name, context, yield or reason, hint). Fixed, so it never covers map glyphs |
| **Screens** | Over the map area; the side panel stays | map area | Research (`T`), the message log (`L`), key help (`?`) |
| *(new)* **Status line** | Last row | 1 row | Current mode or tool, its keys, and a "Paused" badge. Vim and Dwarf Fortress both teach this: in a modal keyboard UI the mode must always be visible |

- **Sizes.** Designed for 120×40, which shows 88×32 tiles; it works down to 80×24. Below that,
  the frame shows "Terminal too small (80×24 needed)" and nothing else.
- **One cell per tile.** Terminal cells are about twice as tall as wide, so the map looks
  stretched vertically. Two cells per tile would fix that but halve how much map you see, and
  Dwarf Fortress-style play wants density. A `--square` option can come later if the stretch
  bothers the owner.
- **The view follows the cursor.** It scrolls when the cursor comes within 3 tiles of an edge,
  and it never scrolls past the map. When the selection bar opens, the view shifts so the
  selected unit stays visible (the client's `center_on_selection`).

## Glyphs and colour

### What draws on top

Each cell has one glyph and one background, chosen independently, top to bottom:

- **Glyph:** unit > building or build site > item stack (ground or stored) > lit flame >
  resource > terrain or biome.
- **Background:** cursor > rectangle being dragged > designation tint (harvest, bend, build,
  stockpile) > wet patch > terrain or biome.
- A renewable resource that's growing back is drawn in `dim` grey, as the client greys its
  sprite. A build site is its building glyph, dimmed. A selected unit is drawn in reverse video.
  A unit on top of a stack hides it; the Cursor section lists everything on the tile.
- While the cursor is on a flame, the tiles it warms get a warm background (the client outlines
  its reach on hover).

### The glyph set

The proposed default is a **CP437-flavoured Unicode** set: the characters Dwarf Fortress made
familiar, all one cell wide in any modern monospace font. `--ascii` switches to the fallback,
which for terrain, biomes and resources is exactly the glyphs the mapgen ASCII renderer and its
goldens already use.

| Thing | Glyph | ASCII | Colour (fg on bg) |
|---|---|---|---|
| Sea / lake / river | `≈` / `≈` / `~` | `~` `-` `=` | blue on navy; the lighter blues are fresh water |
| Ford | `░` | `+` | tan stones on river blue |
| Coast | `.` | `.` | sand |
| Mountain | `▲` | `^` | grey on dark grey |
| Plains, forest, taiga | `,` `"` `'` | `,` `"` `&` | greens on dark greens |
| Tundra, swamp, desert | `°` `√` `·` | `*` `%` `:` | white on slate, olive on mud, gold on tan |
| Oak, fir, fruit tree | `♣` `♠` `¥` | `T` `F` `Y` | green, dark green, orange |
| Wild cabbage | `τ` | `c` | light green |
| Stone outcrop, gold vein | `•` `$` | `o` `$` | pale grey, gold |
| Fishing spot | `ƒ` | `f` | white on water |
| Wood, stone, gold ore | `≡` `■` `£` | `/` `*` `$` | brown, grey, gold |
| Food items | `%` | `%` | the food's colour |
| Bed (buildings) | `Θ` | `B` | warm tan; dimmed while a site |
| Lit flame | `☼` | `!` | orange |
| Unit | `☺` | `@` | the nation's `color` lifted toward white, bold |
| Cursor | the tile's glyph | | black on yellow, and the terminal's own cursor sits there too |

Designations are background tints with the client's hues: harvest amber, stockpile blue, bend
the nation's accent, build violet-grey. A wet patch is a dark mud background.

### `data/tui.toml`

Presentation only: `fn_sim` never reads it. `fn_tui` embeds it with `include_str!`, like
`Defs::bundled`, and a test fails if any terrain, biome, resource, item or building in `Defs`
lacks an entry, if a `glyph` is not exactly one character of display width 1, or if an `ascii` is
not printable ASCII.

```toml
# Glyphs and colours for the terminal frontend (docs/design/tui.md). fn_sim never reads this.
# glyph: one Unicode character, one cell wide. ascii: the --ascii fallback.
# fg / bg: sRGB [r, g, b], quantised automatically on 256- and 16-colour terminals.
# A resource, item or building has no bg: it draws on its tile's terrain or biome colour.

[terrain.water]
glyph = "≈"
ascii = "~"
fg = [70, 130, 210]
bg = [12, 32, 72]

[biome.plains]           # bare land in this biome
glyph = ","
ascii = ","
fg = [150, 192, 82]
bg = [30, 52, 22]

[resource.oak_tree]
glyph = "♣"
ascii = "T"
fg = [72, 182, 66]

# [item.*] and [building.*] look like [resource.*].

[unit]                   # fg is the nation's `color` (data/nations.toml), lifted toward white
glyph = "☺"
ascii = "@"

[overlay]                # background tints, mixed into the tile's bg
harvest = [255, 166, 26]
stockpile = [51, 140, 255]
build = [150, 130, 200]
wet = [64, 46, 20]
cursor = [250, 220, 60]
flame = { glyph = "☼", ascii = "!", fg = [255, 150, 40] }
# bend: the nation's accent, as in the client
```

Panel colours are **not** in this file. They are the ui.md tokens (`panel`, `text`, `dim`,
`accent`, `accent_text`, `accent_soft`, `ok`, `warn`, `bad`), computed from the nation's colour
by `fn_ui::theme`, so both frontends match.

A test checks that the unit glyph's colour has at least 3:1 contrast on every biome and terrain
background for every nation, in the same spirit as ui.md's WCAG test. Earth's green units on
green plains are the case it guards.

### Terminals without truecolor

`fn_tui` picks a colour depth once at start:

1. `--color truecolor|256|16|never` if given.
2. `NO_COLOR` set: `never`.
3. `COLORTERM` is `truecolor` or `24bit`, or `WT_SESSION` is set (Windows Terminal, which WSL
   passes through but which doesn't set `COLORTERM`): truecolor.
4. `TERM` ends in `256color`: 256.
5. Otherwise: 16.

256 colours use the nearest xterm-256 entry (the 6×6×6 cube or the grey ramp), and 16 colours
the nearest ANSI colour. Both are computed from the same RGB, so no extra data is needed. A test
checks that the harvest, stockpile and bend tints stay distinct after quantising for every
nation. In `never`, the cursor and the selected unit are reverse video, a designated tile is
underlined, and meters rely on their `!` marks.

## Input

The TUI is modal like Dwarf Fortress: **Select** mode by default, a **tool** mode after a tool
key, and **screens** that take over the keys while open. Letter keys ignore case, so `h` and `H`
are the same key, matching the client, which ignores Shift.

| Key | Select mode | Tool mode | Client today |
|---|---|---|---|
| Arrows, numpad (with diagonals) | Move the cursor 1 tile | same | Arrows pan |
| Shift + arrows | Move 10 tiles | same | none |
| `W` `A` `S` `D` | Scroll the view a quarter screen; the cursor moves with it | same | Pan |
| `Enter` | Select the unit under the cursor (again: the next unit on that tile) | 1st: set a corner. 2nd: apply to the rectangle | Click / drag |
| `Esc` | Close the selection bar | Drop the corner, or (with none) back to Select | Same |
| `Tab` / Shift+`Tab` | Select the next / previous unit; the cursor jumps to it | none | none |
| `F` | Follow the selected unit (the cursor tracks it) | none | Centres once |
| `H` `X` `P` `B` | Harvest, cancel harvest, stockpile, bend tool | Switch tool (drops the corner) | Same |
| `T` | Research screen | same | Same |
| `Space` | Pause / run | same | Same |
| `+` / `-` | Faster / slower | same | none (new) |
| `.` | One tick, while paused | same | none (new) |
| `L` / `?` | Message log / key help | same | none |
| `R`, `1`-`4` | New map / switch nation, after a y/n prompt | same | Same, without a prompt |
| `Q` | Quit, after a y/n prompt | same | Close the window |

- **Rectangles, as in Dwarf Fortress.** In a tool mode, `Enter` anchors a corner at the cursor.
  Moving the cursor stretches a tinted rectangle, and the Cursor section shows its size
  ("Enter: designate 5x3"). A second `Enter` applies the tool to every tile in it. One tile is
  `Enter` twice. The tool stays active for the next rectangle; `Esc` leaves it.
- **Applying** is the client's logic, moved to `fn_ui`: one `Command` per tile through
  `World::apply`, then a notice such as "designate harvest: 12 done, 3 skipped (nothing to
  harvest)" in the Tool section.
- **Why no vi-keys.** `hjkl` would collide with `H` (harvest) and with the client's other
  letters, and keeping the client's hotkeys matters more. See owner question 4.
- **Mouse** is later: crossterm reports clicks and drags, which can map onto the same
  cursor-and-corner actions.
- **Build tool.** The sim has build designations (#64), but neither frontend has a tool yet.
  When one lands, it goes into `fn_ui`'s tool table with one key and a building picker, so both
  frontends get it together.

## Time and the frame loop

`fn_ui::Clock` holds `paused`, a speed from `SPEEDS = [1, 2, 3, 5]` (times
`TICKS_PER_SECOND`), and the fraction of a tick it owes. Speeds are pacing, not balance, so they
are constants in `fn_ui` rather than data.

```rust
impl Clock {
    /// Ticks to run for `real` seconds of wall time: 0 while paused, else
    /// owed + real * speed * TICKS_PER_SECOND, capped at MAX_TICKS_PER_FRAME (any debt
    /// beyond that is dropped, so a slow machine runs slower instead of spiralling).
    pub fn advance(&mut self, real: f64) -> u32;
    /// While paused: exactly one tick.
    pub fn step(&mut self) -> u32;
}
```

The loop in `fn_tui`:

1. Wait for input with `crossterm::event::poll`, with a timeout until the next tick is due.
   Paused, with no input, it sleeps and redraws nothing.
2. Handle every pending key. A key that issues commands calls `World::apply` now, between
   ticks.
3. Run `clock.advance(elapsed)` ticks of `World::tick`.
4. Redraw if anything changed (a key, a tick or a resize), at most 30 times a second.
   ratatui only writes the cells that changed.

**Why it stays deterministic.** `fn_sim` never sees real time. Real time only decides *how many*
ticks run between inputs. Each applied command is logged with the tick it landed on, so
`World::replay` reproduces the session exactly. A test runs a key script, replays its command log
into a fresh world, and checks that the two frames match. Only `fn_tui`'s `main` reads
`Instant`. `fn_ui` takes elapsed time as an argument, so `fn_ui` and the render tests never read
the clock either.

The Bevy client switches to the same `Clock` (its `FixedUpdate` asks for one tick at a time).
Giving it `+`, `-` and `.` keys is a small optional follow-up, not part of this work.

## Moving shared logic out of `fn_client`

The client holds no game rules, but it does hold *frontend* logic that the TUI would otherwise
copy: which command each tool issues, how a task reads in English, what the tooltip says.
That moves into `fn_ui`, with its tests. The client keeps calling it, and its behaviour doesn't
change.

| Today (`crates/client/src/`) | Moves to | Notes |
|---|---|---|
| `designate`: `Tool`, `label`, `command`, `ability_for_nation`, `tiles_between`, the apply-a-rectangle loop and its notice text | `fn_ui::tools` | The tool table gains each tool's key, so both frontends' key lists come from one place. `drag_color` and `Layer` tints stay in the client |
| `jobview`: `describe_task`, `describe_stack` | `fn_ui::text` | |
| `hud`: `tooltip_lines`, `bending_status_line` | `fn_ui::inspect` | Hint lines become an enum (`SelectUnit`, `StartTool(Tool)`, `ApplyTool`), and each frontend words them ("Click to select" versus "Enter to select") |
| `hud`: `side_view` data, `format_clock`, `modifier_rows`, `multiplier`, `humanize`, `unit_view` (without the portrait), `need_status`, `HEALTH_LOW` | `fn_ui::view` | Pure functions over `World`, already tested as such |
| `bending`: `describe_bend_event`, `Alerts`; `research`: `CompletionLog` | `fn_ui::alerts` | One log fed by bend events, tech completions and (new) deaths. Aged by a `dt` the caller passes, so the client's timing doesn't change. The TUI keeps the full history for `L` |
| `research`: `research_status`, `tech_state`, `tech_depths`, `unlock_summary`, `screen_view` | `fn_ui::research` | |
| `research`: which techs a nation can see (shared plus its own branch) | **`fn_sim`**, e.g. `Defs::techs_for(nation)` | This one *is* a rule, so it belongs in the sim |
| `ui_style::Theme` token maths and its contrast test | `fn_ui::theme`, as sRGB `[f32; 3]` | The client converts to `Color`. Layout pixels, fonts and builders stay in the client |
| `main`: `Sim`, `STARTING_UNITS`, `paused`, `tick_sim` | `fn_ui::Session` (`World`, `Defs`, `Clock`) | One way to start a game and drive ticks |

**What keeps `fn_ui` honest.** It depends on `fn_sim` only. It has no Bevy, no I/O and no
wall-clock reads. It changes the world only through `World::apply` and `World::tick`. It never
decides something the sim decides: it doesn't pre-filter tiles a command would accept or
reject; it formats and maps. Sprites, entities, the camera and interpolation stay in
`fn_client`.

## Feature parity

| Bevy client | TUI | Issue |
|---|---|---|
| Map: terrain, biomes, resources, regrowing resources greyed | Glyph map | MVP (#97) |
| Units in nation colours; items on the ground and in stockpiles; flames, wet patches, fords | Glyphs and backgrounds | MVP (#97) |
| Camera pan; zoom | Cursor, scrolling view. No zoom: an overview map (one cell per 2×2 tiles) is later | MVP (#97) |
| Pause (`Space`) | Pause, speeds, single step | MVP (#97) |
| Side panel header: nation, seed, clock, speed, people count | Same | MVP (#97) |
| Hover tooltip | Cursor section | MVP (#97) |
| Tools `H` `X` `P` `B`, drag rectangle, designation tints, notice | Dwarf Fortress-style rectangles | #98 |
| Selection: click a unit, selection bar with needs and chi | `Enter`, `Tab`, `F`; text bars | #99 |
| Side panel: stores, research line, modifiers, tool hint, alerts | Same, plus a message log (`L`) and key help (`?`) | #99 |
| `R` and `1`-`4` regenerate | Same, with a prompt | #99 |
| Research screen (`T`) | Tier columns, cursor, `Enter` sets the target | #100 |
| Build tool (sim only so far, #64) | Through `fn_ui`'s tool table, in both frontends at once | Later |
| Save and load (#7) | Once the sim has it | Later |
| Mouse | Click moves the cursor; a drag is a rectangle | Later |
| Portraits, sprite smoothing, canopy fade (#77) | Not applicable | none |

The MVP is: look at the map, move the cursor, read what's under it, and pause or run time.
Designation comes next because it's the core loop. Panels come after it, because the side
panel's header and Cursor section already make the MVP playable to watch.

## Verifying headlessly

- **Pure render.** `fn_tui::draw(&Session, &UiState, &Palette, &mut Buffer)` draws the whole
  frame. `UiState` is the cursor, view, mode, selection and open screen. Nothing in it touches
  the terminal.
- **Golden frames.** `crates/tui/tests/golden/*.txt` hold frames as plain text, one row per
  line with trailing spaces trimmed. `FN_BLESS=1 cargo test -p fn_tui` rewrites them, as with
  `fn_sim`'s mapgen goldens. They cover a fixed seed per nation, a selected unit, a rectangle
  being dragged, `--ascii`, and the "too small" frame. Colours are checked by unit tests on
  particular cells ("this cell has the harvest tint"), not by colour goldens, which would churn
  on every palette tweak.
- **`--frame` for agents.** `cargo run -p fn_tui -- SEED NATION --frame [--size 120x40]
  [--keys SCRIPT] [--ticks N] [--ascii] [--ansi]` builds the session, plays the script, renders
  one frame to stdout and exits. It never enters raw mode or the alternate screen, and it needs
  no TTY. `--ansi` keeps the colour escapes for a human to `cat`. This is how an agent "plays"
  and checks a change, in place of the GUI or a screenshot.
- **Key scripts** are whitespace-separated tokens: a character is that key; `<enter>`,
  `<esc>`, `<space>`, `<tab>`, `<up>`, `<s-up>` and so on name keys; `<right*5>` repeats; and
  `<tick 600>` runs 600 ticks. A script never uses real time, so seed plus script is
  deterministic. For example, `h <right*5> <enter> <down*3> <enter> <tick 600>` designates a
  rectangle and lets the settlers work.
- **No hanging.** Interactive mode refuses to start when stdout isn't a TTY, and points to
  `--frame`. An agent that runs the binary by mistake gets an error, not a stuck shell. The TUI
  never opens a window, so it can't steal focus either.

## Follow-up issues

Chained in order, each blocked by the one before it:

1. **#96**: create `fn_ui` and move the shared logic out of `fn_client` (no behaviour
   change).
2. **#97**: TUI MVP, with the `fn_tui` crate, `data/tui.toml`, map view, cursor, time
   control, `--frame` and golden frames.
3. **#98**: designation tools with Dwarf Fortress-style rectangles, and key scripts for
   `--frame`.
4. **#99**: panels: selection bar, side panel sections, alerts, message log, key help.
5. **#100**: research screen.

## Open questions for the owner

1. **Default glyph set.** CP437-style Unicode (`♣ ≈ ▲ ☺`), with `--ascii` falling back to the
   mapgen ASCII glyphs, or plain ASCII by default?
   *Recommendation:* Unicode by default. It reads like Dwarf Fortress, modern terminal fonts
   cover CP437, and Windows Terminal falls back to another font for any glyph its font lacks.
   Compare the two linked mockups.
2. **Parity policy.** Must every new player-facing feature land in both frontends, or does the
   TUI trail behind?
   *Recommendation:* trailing parity. Bevy stays the main frontend. Any issue that adds
   player-facing UI gets a sibling `feat:tui` issue at triage, and `fn_ui` keeps that sibling
   small. Revisit if the TUI becomes the way you actually play.
3. **Priority.** #93 is p3, and the follow-ups are filed at p3 to match.
   *Recommendation:* raise #96 and #97 to p2. `fn_ui` tidies the client as well, and
   `--frame` gives agents a way to play-test every later feature without the GUI. Keep #98 to
   #100 at p3.
4. **Movement keys.** Arrows and numpad only (Dwarf Fortress), keeping the client's letter
   hotkeys, or add vi-keys (`hjkl`), which would mean moving `H` (harvest) and friends?
   *Recommendation:* arrows and numpad only, with letters matching the client. Mouse support
   later.
