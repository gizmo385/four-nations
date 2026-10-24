# Terminal UI (Dwarf Fortress-style)

Status: **proposed** (#93), with the owner's answers recorded under "Owner decisions
(2026-10-24)". Follow-up work: #96 to #100 (see the end of this doc).

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
4. **Keyboard first, Dwarf Fortress-style.** The arrows *and* `W` `A` `S` `D` move a cursor
   (the owner's choice), the view scrolls to follow it, and a rectangle is designated with
   **tool key, Enter, move, Enter**. Tool hotkeys are the client's: `H` `X` `P` `B`, `T` for
   research, `Space` to pause.
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
  and it never scrolls past the map. Ctrl+arrows and PageUp/PageDown jump it a quarter screen
  (see "Input"). When the selection bar opens, the view shifts so the
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
are the same key, matching the client, which ignores Shift. The exception is `W` `A` `S` `D`:
they move the cursor exactly like the arrows, so Shift (an upper-case letter) moves 10 tiles,
like Shift+arrows. With Caps Lock on, they move 10 tiles too.

| Key | Select mode | Tool mode | Client today |
|---|---|---|---|
| Arrows, `W` `A` `S` `D`, numpad (with diagonals) | Move the cursor 1 tile | same | Pan |
| Shift + arrows, Shift + `W` `A` `S` `D` | Move 10 tiles | same | none |
| Ctrl + arrows; PageUp / PageDown | Jump the view a quarter screen; the cursor moves with it | same | Pan |
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
- **Why WASD and not vi-keys.** `W` `A` `S` `D` are free: no tool, screen or prompt in this
  table or in #97 to #100 uses them, and they are the client's pan keys, so the hands already
  know them. `hjkl` would collide with `H` (harvest).
- **Where scrolling went.** The view follows the cursor at the edges, which covers ordinary
  movement. For quarter-screen jumps, Ctrl+arrows (all four directions) and PageUp/PageDown
  (vertical) were chosen because they collide with nothing. Ctrl+`W` `A` `S` `D` was rejected:
  Ctrl+S is the terminal's flow-control stop outside raw mode, and Ctrl+D is end-of-input,
  which makes them risky in the shells and multiplexers the TUI runs in.
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
- **Key scripts** are whitespace-separated tokens: a character is that key (so `w` and `W`
  move the cursor 1 and 10 tiles); `<enter>`, `<esc>`, `<space>`, `<tab>`, `<up>`, `<s-up>`,
  `<c-up>`, `<pgdn>` and so on name keys; `<right*5>` repeats; and
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
5. **#100**: research screen (done).

## What #97 built

The MVP, in `crates/tui`. Where it differs from the proposal above:

- **`draw(&Session, &UiState, &Palette, &mut Buffer)`.** `Palette` holds the glyph table
  (`data/tui.toml`), the nation's `fn_ui::theme::Theme`, the colour depth and `--ascii`. It
  hands out every colour already quantised, so the renderer never branches on depth.
- **Tints on 256 and 16 colours.** A tint mixed into a dark ground collapses onto a neighbour
  (an Air Nomad's yellow bend and the amber harvest are one palette entry). On 256 colours each
  designation takes the nearest entry that is clearly apart from the ground, the cursor and the
  tints before it. On 16 colours each gets a solid colour of its own. A test checks this for
  every nation and every ground.
- **Numpad.** A terminal sends the numpad's digits as the number row's unless it speaks the
  kitty keyboard protocol, which `fn_tui` asks for when it is offered. Without it, numpad
  `7` and `1` work as Home and End (NumLock off), and `8` `4` `6` `2` as the arrows, but `9` and
  `3` arrive as PageUp and PageDown, which jump the view. Digits on the number row do nothing yet:
  `1`-`4` are for switching nation (#99).
- **Status line.** It shows `SELECT` and the keys until a tool gives it a mode to show (#98).
- **Hints.** The Cursor section drops the `fn_ui::inspect::Hint` lines (`panel::hint_text`) until
  `Enter` selects (#99), rather than promise a key that does nothing. The tool hints came with
  #98.

Seams for the next issues: `UiState::tool` and `Prompt` (modal state), `input::Action` and
`input::map_key` (keys), `panel::draw_panel` (sections) and `map::tile_cell` (the rectangle being
dragged goes between the cursor and the designation tints).

## What #98 built

The designation tools and key scripts. Where it differs from the proposal above:

- **Keys.** `H` `X` `P` `B` in either case pick `Tool::Harvest`, `CancelHarvest`, `Stockpile` and
  `Bend` through `fn_ui::tools::switch_tool`; `U` (build) is not bound until the build tool has a
  picker. `Enter` and `Esc` are `Action::Confirm` and `Action::Cancel`; in Select mode both do
  nothing yet (#99 gives them selecting and closing the bar). Picking a tool, even the active one,
  drops the corner and the last result.
- **Applying** is `fn_ui::tools::apply_rect` as the client calls it. The Shift state (a repeating
  harvest, #81) and the build choice are not wired, so the Harvest hint still says "hold Shift to
  repeat", which the TUI can't do yet.
- **State.** `UiState` gains `anchor` (the first corner) and `notice` (the last result), with
  `rect()`, `rect_size()` and `in_rect()`.
- **Screen.** The status line shows the tool's badge, what `Enter` and `Esc` do now, and the
  tool's `hint` (for `B`, the nation's ability). The side panel's **Tool** section (hidden in
  Select mode) shows the tool with its key, the hint and the result notice, wrapped. The Cursor
  section's first line says what `Enter` does next ("Enter: set a corner", then "Enter:
  designate 5x3"), and its tooltip's "start this tool" hint reads "Press H for the Harvest tool".
- **The rectangle** is drawn by `map::tile_cell` in the tool's hue (`Tint::for_tool`: the
  client's harvest, stockpile and bend hues; cancel wears harvest), mixed in more strongly than a
  designation (`Palette::dragged`) so the two stay apart where they overlap. On 256 colours it
  avoids the ground and the cursor; on 16 it is the designation's solid colour; with no colour it
  is reversed and dim (the cursor alone is reversed). The cursor tile keeps its own look.
- **`--keys SCRIPT`** is parsed up front (`keys::parse`, so a typo is an exit-2 usage error before
  anything runs) and played by `App::play` after `--ticks`. Tokens are the doc's, plus `<tick N>`;
  a plain upper-case letter carries Shift, as a terminal sends it. `<tick N>` runs the world
  directly, so the clock (and real time) never matters.
- **Tests.** A script designates a rectangle and `harvest_designations()` matches what the sim
  accepts tile by tile; the script's command log replayed into a fresh world (`World::replay`)
  draws the same frame; two golden frames show a rectangle mid-drag and after it is applied.

## What #99 built

Selection, the rest of the side panel, the alert log, the message log and key help. Where it
differs from the proposal above:

- **One key table.** `input::bindings()` lists every key with its action, its help group and its
  wording. `map_key` reads it, and the key help screen is generated from it (`input::help_rows`:
  bindings with the same wording share a row, the four arrows read "Arrows"), so the help can't
  drift from the handlers. The tool rows come from `fn_ui`'s `Tool::ALL`. The research screen
  (#100) adds its action and its row to this table. Shift is not part of a key: it makes a move
  ten tiles and does nothing else.
- **Selecting.** `Enter` in Select mode selects the unit under the cursor, and again the next unit
  there. `Tab` and `Shift+Tab` step through the people list (the first and the last unit when
  nothing is selected) and put the cursor on the unit. `F` toggles following: the cursor tracks the
  unit each tick, and moving the cursor by hand stops it. `Tab` and `F` do nothing in a tool. `Esc`
  closes the bar in Select mode (in a tool it still drops the corner, then leaves the tool). A unit
  that dies closes the bar. Selecting issues no command, so a replay needs nothing for it.
- **The bar.** `bar.rs` draws the 7 rows on the bottom of the map region: name, `·` and nation on
  the top edge; the task (two lines at most), what the unit carries and its tile on the left; the
  key hints on the last row, as many as fit; and the five meters on the right (10 cells, a `!` after
  any that is not `ok`; chi in the accent, never flagged). While it is open `UiState::visible()` is
  7 rows shorter than the map region, and the view scrolls by that, so the cursor and the selected
  unit stay above the bar (the client's `center_on_selection`). The selected unit is drawn in
  reverse video. A screen hides the bar, as ui.md says.
- **The side panel.** Sections in ui.md's order, then Tool and Cursor: header, **People** (the
  count, then a line per unit: name and what it is doing without the place, `▸` marking the selected
  one), **Stores** (`fn_ui::view::stores`, new: the stockpiles' totals, "Wood 12  Stone 4"),
  **Research** (target and rate, then a bar, the percentage and the points; "No target set" and the
  points banked without one), **Modifiers** (those not at 1.0, as the client's `modifier_rows`),
  Tool, **Alerts** (the toasts from `fn_ui::alerts`) and Cursor. A section with nothing to show
  hides. When the panel is too short (80x24 with a tool active, say) it gives up lines in this
  order: the tool's hint (the status line says it too), the modifiers, the unit lines (the rest are
  counted as "+N more [Tab]"), the older alerts, all but one line of stores. The header, Research
  and Cursor stay. `panel.rs` builds each section as rows and the fitting loop picks how many.
- **Alerts and the log.** `UiState::alerts` is a `fn_ui::Alerts`, fed by `App` after every tick.
  The toasts age by real seconds (the interactive loop passes the elapsed time, including while
  paused, and keeps waking while one shows). In `--frame` there is no real time: `<tick N>` and `.`
  add the new lines without ageing, so a frame after the ticks shows what they just logged, and the
  oldest toasts leave only when newer ones push them out. The log keeps the whole history (1000
  lines), newest last, each line stamped with the game time and wrapped under it; deaths are `bad`
  and tech completions `ok`.
- **Screens.** `L` and `?` open the log and the key help over the map region (`screens.rs`; the
  side panel stays). Each toggles itself and switches to the other. While one is open the arrows,
  `W` `S` and `PgUp` `PgDn` scroll it (Shift: 10 lines), `Esc` closes it, the clock keys, the
  prompts and quit work, and the map keys wait. The log opens pinned to its newest line and sticks
  there when scrolled back to the end. The status line shows `LOG` or `KEYS`.
- **Prompts.** `R` asks to start a new map (the next seed, the same nation, as the client does) and
  `1` `2` `3` `4` ask to restart the same seed as Air, Water, Earth or Fire (the client's order).
  Only a plain `y` confirms. A restart is a new `Session` and `UiState` on a map of the same size:
  a running clock, no selection, no log. The interactive loop rebuilds the palette for the new
  nation, and `--frame` draws the last frame in the nation it ended on.
- **Numpad.** The number row's digits now switch nation. A terminal that does not report keypad keys
  sends the numpad's digits as the number row's, so there they ask to switch nation too (the prompt
  makes that harmless); the Home/End/arrow forms still move.
- **Goldens.** Re-blessed: every nation's first frame, the ASCII frame and the paused, dragging and
  designated frames (the side panel gained Research and Modifiers, and People lists the units). New:
  `earth_seed7_selected` (and its ASCII, no-colour twin), `fire_seed1_log` and `earth_seed7_help`.
  `tests/panels.rs` covers the rest cell by cell and in text.

## What #100 built

The research screen, the last of the chain. Where it differs from the proposal above:

- **`T`** (`input::Action::ShowResearch`, in the key table and so in the help) opens the tree over
  the map region, and `T` or `Esc` closes it; it switches to and from the log and the help like
  they do. The side panel stays, the status line shows `RESEARCH`, and the map keys wait (the
  clock, the prompts and quit still work). `research.rs` draws it and moves its cursor;
  `screens::draw_screen` and `App::act_in_screen` dispatch to it.
- **What shows** is `fn_ui::research::screen_view` as the client uses it: columns by depth (`TIER
  n`), shared techs and the player's own branch only (`Defs::techs_for`). A card is three rows: a
  state mark and the name; the state word on the left and the cost on the right; the unlock
  summary. The marks (`✓ ◆ ◇ ·`, ASCII `+ * o -`) and the words (`Known`, `Queued`, `Available`,
  `Locked`) carry the state without colour. A queued prereq is drawn the same as the target, as in
  the client. Under the grid a rule and three rows tell about the tech under the cursor: name,
  state and cost; its description; its unlocks and the prereqs still unknown (`Needs:`).
- **Fitting.** A column is at least 18 cells wide. A terminal too narrow for every column shows as
  many as fit and slides along with the cursor (`◂` and `▸` on the frame's edge say there are more),
  and a column with more cards than rows slides the same way (`▲` and `▼` after its title). The
  window is a pure function of the cursor and the size, so a frame stays a function of the state.
  At 80x24 that is two columns of four cards; at 120x40 four columns of up to eight.
- **Keys.** The arrows and `W` `A` `S` `D` move the cursor one card (Shift makes no difference; it
  stops at the edges, and a step sideways keeps the row where the next column has it). `PgUp` and
  `PgDn` move a page of cards. `Enter` issues `Command::SetResearch` for the card; on a locked tech
  the sim queues its whole prereq chain, on a known one it refuses and nothing changes. The cursor
  starts on the current target, or else the first tech that can be researched.
- **Status line.** The Select-mode keys now teach `T research` in place of `+/- speed` (the help
  has it), so `q quit` still fits at 120 columns.
- **Side fix.** `draw_frame` resets the cells it covers. It only restyled them, so with no colour
  the map cursor's reverse video showed through the log, the help and the tree.
- **Goldens.** New: `earth_seed7_research` (a target set), `earth_seed7_research_small` (80x24,
  scrolled to the deep end) and `fire_seed3_research_ascii`. Re-blessed: the status line of every
  Select-mode frame, and the help (it gained the `T` row).

## Owner decisions (2026-10-24)

The four open questions on #93, as the owner answered them.

1. **Glyphs: Unicode by default**, CP437-style (`♣ ≈ ▲ ☺`), with `--ascii` falling back to the
   mapgen ASCII glyphs. As recommended.
2. **Parity: the TUI trails the Bevy client.** Bevy stays the main frontend. When triage files
   an issue that adds player-facing client UI, it also files a sibling `feat:tui` issue, and
   `fn_ui` keeps that sibling small. As recommended.
3. **Priority: #96 and #97 are p2**, because `fn_ui` tidies the client too and `--frame` lets
   agents play-test later features without the GUI. #98 to #100 stay p3. As recommended.
4. **Movement: arrows and `W` `A` `S` `D`.** Not the recommendation (arrows and numpad only).
   `W` `A` `S` `D` move the cursor 1 tile exactly like the arrows, and Shift moves 10. They no
   longer scroll the view. Scrolling follows the cursor at the edges, and Ctrl+arrows and
   PageUp/PageDown jump a quarter screen ("Input" above). Numpad diagonals stay.
