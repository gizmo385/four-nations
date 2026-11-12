# The egui frontend (desktop and browser)

Doc status: current

Status: **proposed** (#856). Owner questions are at the end, each filed as a `decision` issue;
the doc's recommendation is the working default until the owner answers.

The owner chose egui for desktop and browser play and kept the terminal (#843, accepted). Browser
play goes on the public Pages site (#866, 2026-11-11). Two spikes show the approach works:
`fn_gui` (#842, #847) opens a window on the title screen, drives the same `fn_tui::shell::Shell`
the terminal drives, paints the map as a pan/zoom glyph widget and paints everything else as
the ratatui frame through `egui_ratatui`. #843 (#852) ran the same app in the browser. This doc
covers how to get from that spike to a real game window without forking the game logic or
changing the TUI.

## Decisions in brief

1. **One state machine, two painters.** `fn_gui` keeps driving `Shell`, with `App`/`UiState`
   moving into `fn_ui` (#857). egui gets no game state of its own. A screen converted to egui is
   a new *painter* for a state that already exists. It never adds a parallel state.
2. **Words come from `fn_ui`.** A converted screen prints the same strings as the terminal by
   reading the same row models. When `fn_tui` builds a screen's words inline, that screen's
   conversion moves them into `fn_ui` first. The side panel goes first (F5).
3. **Input goes through keys.** Every mouse action is a key press the shell already handles, or
   a `UiState` method the key handler also calls (`point_at`, `select`). `fn_tui::pointer`
   already works this way. A click's command log is the keyboard's, so saves, replays and agents
   can't tell which one was used.
4. **Convert a screen at a time and paint the rest as cells.** Our own cell painter (F1)
   replaces `egui_ratatui`. It is cheap enough to keep the unconverted screens, and the
   terminal-only tools (watcher, studio, run browser) inside the window indefinitely.
5. **Order: map frame, side panel, inspector, mouse, boxes, title, embark, then the full
   screens.** That way the 90% of play time spent looking at the map and panel is native first.
6. **The Seal look in egui, dark only.** Visuals come from `fn_ui::theme` (no new data). Square
   corners, accent frames, the `◆ NATION ◆` seal, `◇` headings. One bundled monospace family:
   Hack plus DejaVu Sans Mono for the ten glyphs Hack lacks (D2).
7. **CI tests `fn_gui` without a GPU.** `egui_kittest` drives the UI and queries its AccessKit
   tree by label, which needs no renderer. Pixel snapshots stay out of the gate. A
   `cargo xtask gui-shot` writes PNGs for a reviewer to look at.

## What exists (crates/gui at 108f4f9)

| Module | What it does | Fate |
|---|---|---|
| `main.rs` | Native `eframe::run_native`; wasm `WebRunner` starting at embark from a query string | Web entry goes to the title screen (#866) |
| `app.rs` (`GuiApp`) | Reads the clock, translates keys, paints the whole ratatui frame via `egui_ratatui`, puts `MapView` over the map region, shows the timing overlay | Becomes the frame (F4). `egui_ratatui`/`soft_ratatui` go (F1) |
| `mapview.rs`, `camera.rs` | Glyph map from `fn_tui::map::cell_look` + `style_cell`, pan, zoom 1–48 px, block overview below 9 px | Stays. Gains click semantics, a hover tooltip and a cached overview (F8) |
| `keys.rs` | egui `Event` to `fn_tui::keyev::KeyEvent` | Stays, retargeted to #857's frontend-neutral key type |
| `panel.rs` | Spike sample: header, People rows, settler list, with its own strings | Replaced by the native side panel (F6) |
| `fonts.rs` | Loads DejaVu Sans Mono from the system path | Bundles it with `include_bytes!` (#859) |
| `color.rs` | ratatui `Color` to `Color32` | Stays (the map look and the cell painter both need it) |
| `tests/` | Key translation, camera maths, glyph coverage, one wgpu snapshot that panics without an adapter | Snapshot moves behind a feature; the rest join the gate (F2) |

`fn_gui` keeps depending on `fn_tui` for good. `fn_tui` holds `Shell`, the map look
(`fn_tui::map`), the `Palette` built from `data/tui.toml`, and the reference renderer the cell
painter falls back to. Dropping that dependency is not a goal. #857 only takes ratatui types
out of the *state* API, so `fn_gui` stops needing ratatui to read the game.

## The window

```
┌───────────────────────────────────────────────────────────────────────────┬── ◆ EARTH KINGDOM ◆ ──┐
│ ━ HARVEST ━ Enter: designate 5x3 · Esc: drop the corner          (banner) │ ▶ Running  x1   ‖ ▸ » │
│                                                                           │ Day 1 · 4:30 left     │
│                                                                           ├ ◇ SETTLEMENT ─── [N] ─┤
│              map widget (mapview.rs)                                      │ People 6       Idle 6 │
│              glyphs per tile, pan/zoom, live rectangle,                   │ Food 0 · 0 each       │
│              hover tooltip after 0.4 s                                    │ Research: none set [T]│
│                                                                           ├ ◇ ALERTS ─────── 1 ───┤
│                           ┌──────────────────┐                            │ No research target... │
│                           │ Lake  (98, 64)   │ (tooltip)                  ├ ◇ HARVEST ─────── [H]─┤
│                           │ Elevation 144    │                            │ Chop, mine or gather  │
│                           └──────────────────┘                            │ 5 x 3 tiles           │
│                                                                           ├ ◇ LAKE ──── (98, 64) ─┤
│                                                                           │ Elevation 144         │
├───────────────────────────────────────────────────────────────────────────┴───────────────────────┤
│ HARVEST  Arrows move · Enter apply · Esc drop corner · O repeat                            ? keys │
└───────────────────────────────────────────────────────────────────────────────────────────────────┘
```

That is the TUI's own layout (tui.md "Screen layout") in egui panels. The terminal frame it
replaces, from `fn_tui 7 earth --frame --size 100x30 --keys "h <enter> <right*4> <down*2>
<tick 300>"`:

```
━ HARVEST ━ Enter: designate 5x3  ·  Esc: drop the corner ━━━━━━━━━━┏───── ◆ EARTH KINGDOM ◆ ──────┓
"♠♠""",,,,,,,,,,,,τ,,▲▲▲,♣,,♣,,,,,..≈≈≈≈≈≈≈..,,,♣,,.≈..≈≈≈.",""""""♠│ ▶ Running x1                 │
♠""♠♠"♠,,,,,,,,,,,,,,▲▲$●♣,~♣,,,,,.≈≈≈≈≈≈≈ƒ≈..,,,♣,.....≈..",τ""♠"""│ Day 1 · 4:30 left            │
"""¥""♠¥""¥♣,♣,,,,,,●,•▲,,,,~,,,,..≈≈≈≈≈≈≈≈≈≈.,♣♣,,♣,,,.≈.,♠,,,"""""├─ SETTLEMENT ─────────── [N] ─┤
♠♠♠♠""♠""♠"♠,,,,,,,,,•▲▲●♣♣,,~,,..≈≈≈≈≈≈≈≈≈≈≈..♣,,,,,...≈.,,♠,"¥♠"♠♠│ People 6              Idle 6 │
♠♠¥♠♠"♠"""""",,,,,,,●▲▲▲,,,...~..≈≈≈≈≈≈≈≈≈≈≈≈≈..♣♣,...≈≈≈.¥♠"""♠"♠"♠│ Food 0 · 0 each              │
""♠♠"""""¥"♠♠♠",,,,,,▲▲●●,,.≈≈≈≈≈≈≈≈≈≈≈≈≈≈≈≈≈≈≈.,τ,.≈≈≈≈≈.♠♠♠"♠"""""│ Research: none set       [T] │
♠♠"""¥¥""""""ƒ,,,,,,,•,●♣♣♣.≈≈≈≈≈≈≈≈≈≈≈≈≈≈≈≈≈≈≈.,,,.ƒ≈≈≈≈~♠♠♠""♠"♠♠"│ Gate 0 of 9 signs        [G] │
♠♠♠"""""♠"""~♠,,¥,,,●,●,♣,♣...≈≈≈≈≈≈≈≈≈≈≈≈≈≈≈≈≈.♣,,.≈≈.≈≈.~♠♠"♠"♠♠♠♠│ Needs 4 more people          │
♠♠"♠"""""""~"",,,,,,,,,,,,,,,....≈≈≈≈≈≈≈≈≈≈≈≈≈..♣,,...≈≈≈.♠~♠""♠♠"♠"├─ ALERTS ─────────── 1 · [L] ─┤
.~..♠"♠♠""~"",,¥,,,,,♣¥,,,♣,,♣,♣..≈≈≈≈≈≈≈≈≈≈≈≈.,,,¥♣,.....♠♠~♠♠♠♠♠♠"│ No research target is set. 1 │
≈≈≈...""""~",,¥,,,,,,,,,,,,,,,,♣♣.≈≈≈≈≈≈≈≈≈≈.≈.,,♣♣♣♣♣,,♣♣♠♠♠~~"♠♠""│ point is waiting.            │
≈≈≈≈≈..""~τ",♣,,τ,,,,,,,,,,,♣,♣♣☺..≈≈≈≈≈≈≈≈....,,,♣,♣♣,,,♣♠♠♠♠♠ƒ~~~♠├─ HARVEST ────────────── [H] ─┤
≈≈≈≈≈≈..~""♠,♣,,,,,¥,,,,,,,♣,♣,♣,,.≈≈≈≈≈≈≈≈≈..♣,♣♣♣,,,,τ,♣,♠♠♠♠♠"♠"~│ Chop, mine or gather what is │
≈≈≈≈≈≈≈~."""♣,,,,,,,,,,,,¥,τ,♣♣♣,,.≈≈≈≈≈≈≈ƒ.≈.,,,,♣,,,,♣♣♣♣♣♠♠♠"♠♠"♠│ inside the rectangle.        │
≈≈≈≈≈≈≈≈..",♣♣,,,,,♣,♣,♣,,¥,,,,♣,¥☺≈≈≈≈≈≈≈≈≈..,,,,,,,♣♣,,♣,,♠"♠♠♠♠"♠│                              │
≈≈≈≈≈≈≈≈≈.",,♣,♣♣,,,,,♣,,♣,,,,,,,,☺≈≈≈≈≈≈≈≈≈≈..,♣♣,♣,♣♣♣,♣♣,,♣,¥"♠♠♠│ 5 x 3 tiles                  │
≈≈≈≈≈≈≈≈≈."♣,♣,,,♣,,♣,,,♣,♣♣♣,♣♣,,☺.≈≈≈≈≈≈≈≈≈≈..,,,♣,,♣,♣♣♣,♣♣♠♣♣,♠♠│ Nothing to harvest           │
 ...
 HARVEST  Arrows move · Enter apply · Esc drop corner · O repeat                             ? keys
```

The regions map one to one:

| TUI (`fn_tui::layout::Regions`) | egui | Notes |
|---|---|---|
| `map` | `CentralPanel` holding `MapView`, with the tool banner (`fn_tui::banner`) as a strip on its top edge | The map widget has its own camera. It follows the keyboard cursor as it does today (`MapView::last_cursor`) |
| `panel` | `Panel::right`, 300 px default, resizable from 260 to 440 | Sections in side-panel-v2.md order. egui scrolls the inspector instead of trimming it |
| `status` | `Panel::bottom`, one row: the hint bar from `fn_ui::keys::hints` | Keys stay named (keyboard-first). No mouse wording in v1 |
| `page` (full screens) | The whole window above the status bar; the side panel hides | Same rule as tui.md's #652: a full screen covers map and panel |
| `modal`/`ask` boxes | `egui::Modal`, centred, dims what is behind | Captures every key, as `crate::modal` does |

Window: 1600×960 by default, minimum 1024×640 logical pixels. In the browser the canvas fills
the page and egui follows `devicePixelRatio`.

## Native and cell-painted surfaces

During the conversion (and for good, for the dev tools), each frame `fn_gui::frame` asks one
question: is what the shell shows now native or cells? It asks with an exhaustive `match` over
`shell::Mode`, `state::Screen`, `state::Modal` and `state::Prompt`, so a new screen added to
the TUI fails to compile in `fn_gui` until someone says which it is.

- **Native**: egui widgets.
- **Cells, full**: the whole terminal frame, at the window's size in cells (as the spike does
  today). Used for unconverted full screens, the title screen and embark until converted, and
  the watcher, studio and run browser.
- **Cells, box**: only the box's rect from the terminal frame (`fn_tui::modal`'s area), painted
  over the native map and panel. This avoids swapping the whole window to cells when the game
  menu opens.

The shell is always sized to the window in cells (`Shell::resize`), so pagination and box sizes
match what the cell painter shows.

**The cell painter (F1)** replaces `egui_ratatui` + `soft_ratatui`. The spike measured them at
5–7 ms per frame in release because they CPU-rasterise every cell and upload a texture each
frame. On the web they cost 6 ms of the 14 ms frame. The painter instead walks
`ratatui::buffer::Buffer` and emits one background rect per run of equal background and one
text shape per cell. That is `mapview.rs`'s own glyph code, run over a buffer. Expected cost is
about the map widget's 1.4 ms at 18 px. Dropping those two crates also lifts the egui ~0.34 pin,
so eframe can move to the current release.

## Screen-by-screen order

One row is roughly one agent PR (the #842 estimates). "Words" means a row-model move into
`fn_ui` is part of the job.

| # | What | Native as | Words to move first | Est |
|---|---|---|---|---|
| 1 | Cell painter, frame skeleton, hint bar, tool banner (F1, F4) | `fn_gui::cells`, `fn_gui::frame`, `fn_gui::hints` | none (`fn_ui::keys` already) | 2 |
| 2 | Side panel: header, Settlement glance, Alerts, Tool section (F5, F6) | `fn_gui::side` | `fn_tui::panel`'s `Data` rows and `widgets::{Row, Tone}` | 2.5 |
| 3 | Inspector (F7) | `fn_gui::inspector` | `fn_tui::inspector::settler_view`, tile and placement rows | 1 |
| 4 | Map mouse: click, select, one-tile apply, hover tooltip, overview cache (F8) | `mapview.rs` | none (`fn_ui::inspect::tooltip_lines`) | 1 |
| 5 | Game menu, Quit and confirm boxes, prompts, embark exit prompt (F9) | `fn_gui::boxes` | `fn_tui::menu` items, `fn_tui::ask` | 1 |
| 6 | Title, nation picker, seed step, Load (F10) | `fn_gui::title` | `fn_tui::title` steps | 1 |
| 7 | Embark (F11) | `fn_gui::embark`, reusing `MapView` | `fn_tui::embark` report | 1.5 |
| 8 | Shared page frame and the Build screen (F12) | `fn_gui::page`, `fn_gui::screens::build` | (`fn_ui::build` has them) | 1.5 |
| 9 | Research tree and the school box (F13) | `fn_gui::screens::research` | (`fn_ui::research` has most) | 2 |
| 10 | Saves and naming (F14) | `fn_gui::screens::saves` | (`fn_ui::saves`) | 1 |
| 11 | Traders and Abilities (F15) | `screens::{traders, abilities}` | (`fn_ui::trade`, `fn_ui::bend`) | 2 |
| 12 | Nation roster and Endeavor (F16) | `screens::{nation, endeavor}` | `fn_tui::nation` groups | 2 |
| 13 | Message log, key help and legend (F17) | `screens::{log, help}` | (`fn_ui::msglog`, `fn_ui::keys`), `fn_tui::legend` rows | 2 |

**Stays cells:** the watcher (`fn_tui::watch`), the studio (`fn_tui::studio`), the run browser
(`fn_tui::browser`, `runs`) and spectate. They are agent and dev tools, desktop only (#866 hides
Replays on the web), and they change often. Cells cost almost nothing once F1 lands (D4).

Rows 2–4 are the milestone that matters. After them, an ordinary session (map, panel, cursor,
tools) is native, and only screens you open and close are cells. The menus and the title screen
come next because they are the first and last thing a browser visitor sees.

**The pattern for every conversion PR.** Add the egui painter. Flip its arm in `frame`'s match
from cells to native. Add a kittest test that the screen shows its key strings and reacts to its
keys and clicks (F2). Attach a `gui-shot` PNG to the PR. The TUI's goldens must pass without
re-blessing, which proves a row-model move changed no words.

## The look

**Dark only.** Force `egui::Theme::Dark` and ignore the OS preference. A light theme is not
planned: the map's terrain backgrounds in `data/tui.toml` are dark on purpose (units must reach
3:1 on them), and a light panel beside a dark map reads as two apps. #859 does the forcing.

**Seal, not dev tool.** egui's default look (rounded grey buttons, blue selection, drop shadows)
is a debugger's. `fn_gui::style` builds `egui::Visuals` and `Spacing` from
`fn_ui::theme::Theme::for_nation`, the same tokens the terminal's `Palette` uses, so nothing
new goes in data:

| egui | Token | Seal rule (ui.md, tui.md) |
|---|---|---|
| `panel_fill`, `window_fill` | `panel` | Opaque slate |
| `extreme_bg_color` (bar tracks, text edits) | `inset` | |
| `override_text_color`, weak text | `text`, `dim` | |
| Widget strokes, panel frames, separators | `accent_soft`, 1 px | Square corners: every `corner_radius` 0 |
| `selection.bg_fill` | `select` | Quiet background, bold text, never reversed |
| Active and hovered widget fill | `accent`, text `on_accent` | |
| Headings, keys, chi | `accent_text` | |
| Meters, alert tones | `ok`, `warn`, `bad` | Same for every nation |
| Shadows | none | Flat panels |

Frames get the Seal flair the terminal draws in box glyphs: a `◆ NATION ◆` seal centred on the
side panel's top edge, `◇` before each section heading with the heading's right-hand key
(`[N]`, `[T]`) flush right, and accent corner brackets on boxes and full screens. Switching
nation rebuilds the style, as `GuiApp::step` already rebuilds the `Palette`.

**Type and spacing.** The ui.md scale: 20 px names, 18 px panel title, 14 px body, 12 px labels
and upper-case section headings. Spacing steps 4/6/8/12/16. The map has its own size, the
camera's cell (18 px to start, 1–48).

**Fonts.** One monospace family for everything (D2): egui's bundled Hack, with DejaVu Sans Mono
(about 340 KB, Bitstream Vera licence) as fallback for the ten `data/tui.toml` glyphs Hack lacks
(`✤ ✢ ✣ Ϟ ⌂ ♆ ✦ ɱ ☼ ✶`). It is bundled with `include_bytes!` on every platform (#859), with a test
that every glyph resolves. Monospace keeps the terminal's identity, lines numbers up in the
panel and costs no third font. It also makes cell-painted screens sit beside native ones without
a jump in typeface.

**Map tiles.** The map is the glyph map at any zoom: no sprites, no art pipeline (vision.md).
`camera::CELL_ASPECT` (0.6) copies the terminal's stretched cell. The window has no reason to
keep it: a 5×5 building should look square. Recommended: square tiles with the glyph centred
(D1). A 1600 px-wide map region then shows about 40% fewer tiles across at the same height,
which the zoom recovers.

## Input parity

### Keys

Every key the TUI answers works the same in the window. `fn_gui::keys` translates egui events
to the shell's key type, and the shell maps them to `fn_ui::keys::Action` as it does for the
terminal. `fn_tui::input`'s test that every handled key is in `fn_ui::keys`' table covers both
frontends. The window adds no game keys of its own.

| Key | Desktop | Browser |
|---|---|---|
| Letters, digits, arrows, Shift and Ctrl+arrows, Enter, Esc, Tab, Space, `+ - . / ?` | As in the TUI | As in the TUI; the page `preventDefault`s Tab, Esc, Backspace, Space and arrows |
| F5 / F6 / F9 (quicksave, Saves, quickload) | As in the TUI | Prevented, so F5 doesn't reload the page. macOS may still take some F-keys; the game menu's Save…/Load… cover them |
| Ctrl+C (quit) | As in the TUI | Not delivered (the browser owns Ctrl+letter); Quit is hidden on the web anyway (#866) |
| F12 | Toggles the dev overlay (frame timings); the spike's panel toggle goes away with F6 | Left to the browser (devtools) |
| Ctrl+= / Ctrl+- | egui's UI zoom | The browser's zoom |

### Mouse

The TUI has no mouse (tui.md "Input": "Mouse is later"). Everything below is new and optional:
nothing needs the mouse, and the hint bar keeps naming keys.

| Where | Action | As which key or method |
|---|---|---|
| Map, Select | Left click | Cursor to the tile (`point_at`), then `Enter` if a unit stands there (select), else `Esc` (deselect) |
| Map, Select | Left drag, right or middle drag | Pan the camera (widget-only state, no game effect) |
| Map, a tool | Left click | One-tile apply: `Enter` `Enter` on that tile |
| Map, a tool | Left drag | The rectangle: `Enter` at the start, cursor, `Enter` on release (`fn_tui::pointer` today) |
| Map, a tool | Right click | `Esc` (drop the corner, else leave the tool) |
| Map | Wheel | Zoom about the pointer |
| Map | Hover for 0.4 s | Tooltip from `fn_ui::inspect::tooltip_lines`. It does not move the cursor, so the inspector stays on what the keyboard points at. This is vision.md's "hover anything and see why" |
| Side panel | Click a section's `[N]`/`[T]`/`[G]`/`[L]` | That key |
| Side panel | Click an alert | The message log's `Enter` on that entry (cursor to its place, or select its settler) |
| Side panel | Pause and speed buttons in the header | `Space`, `+`, `-` |
| Full screens | Click a row or card / double-click | Move the highlight there / then `Enter` |
| Full screens | Wheel | The screen's own scroll keys |
| Boxes | Buttons | The box's own keys (`Enter`, `Esc`, the item's letter) |

Every row is a key or a `UiState` call the keys make, so tests compare command logs: a click
path and its keyboard path must produce the same `World` command log (`tests/pointer.rs`
already does this for drags). `fn_ui::inspect::Hint` already lets wording differ by input
device. v1 leaves it on keyboard wording.

## Testing in CI

Today `xtask` excludes `fn_gui` from `cargo xtask ci` (`EXCLUDED` in `xtask/src/main.rs`). Its
one snapshot test panics with "No adapter found" without a GPU or lavapipe.

**Recommended: semantic UI tests, no pixels (F2).**

- Remove `fn_gui` from `EXCLUDED`. The gate builds, lints and tests it. Building needs no native
  libraries (they are `dlopen`ed at run time), and no test opens a window.
- `egui_kittest` without its `wgpu` feature: a `Harness` runs `GuiApp` frames headlessly, and
  tests find widgets through the AccessKit tree (`get_by_label("People 6")`,
  `.click()`, key presses). That covers "the panel shows X", "clicking the alert puts the
  cursor there" and "Esc opens the menu box". The cost is a software tree walk, not a renderer.
- Logic tests stay as they are: key translation, camera maths, glyph coverage, and click-vs-key
  command logs.
- Clippy for `fn_gui` on `wasm32-unknown-unknown` joins `WASM_CRATES`. #859 adds the full web
  build to CI.
- The pixel snapshot moves behind a `shots` cargo feature, off in the gate.

**Not in the gate: pixel snapshots.** They would need `mesa-vulkan-drivers` on the runner,
committed PNGs with a tolerance, and agents reviewing image diffs on every palette tweak. The
TUI already chose `contains` assertions over colour goldens for the same reason ("Verifying
headlessly"). Instead, **`cargo xtask gui-shot SEED NATION [--keys SCRIPT] [--size WxH] --out
FILE.png`** renders one frame through kittest's wgpu path. It plays the role `--frame` plays
for the terminal: an agent or reviewer looks at it, it is attached to conversion PRs, and
nothing compares it. It needs lavapipe or a GPU locally, and it fails with a clear message
without one.

**Browser smoke:** #866's Playwright check (page loads, small map draws, no console errors).

## Web

Already filed: #859 (`cargo xtask web` builds and binds `fn_gui`, bundled font, dark theme, a
download-size and GPU frame-time measurement), #860 (a save store behind an interface, with
IndexedDB in the browser) and #866 (Pages at `/play/`, title screen first, Quit and Replays
hidden, Playwright). This doc adds one dependency: **#866 should also be blocked by F1**. The
public page shouldn't ship the CPU-rasterised terminal path the spike measured at 6 ms of a
14 ms browser frame. The title screen is cells until F10, which is fine once cells are cheap.

#858 (an optional `serde` feature on `fn_ui` view models) was for the DOM panel #843 dropped.
egui reads the Rust structs directly, and the agent surface has its own `Serialize` views
(`fn_tui::surface`). **It gates nothing here.** Keep it at p3 and close it if no consumer
appears.

## Follow-up issues

Filed from this doc (numbers to be filled in when filed), alongside the existing #857, #858,
#859, #860 and #866:

| Id | Title | Blocked by |
|---|---|---|
| F1 | fn_gui: paint terminal cells ourselves; drop egui_ratatui and lift the egui pin | #857 |
| F2 | fn_gui in the gate: kittest semantic tests without a GPU; `cargo xtask gui-shot` | F1 |
| F3 | fn_gui: the Seal theme from `fn_ui::theme` | #859, D2 |
| F4 | fn_gui: the native frame (regions, hint bar, tool banner, native/cells dispatch) | F1 |
| F5 | Side panel rows into `fn_ui` (Row, Tone, section builders) | #857 |
| F6 | fn_gui: native side panel (header, glance, alerts, tool section) | F4, F5 |
| F7 | fn_gui: native inspector | F6 |
| F8 | fn_gui: mouse on the map (click, select, one-tile apply, tooltip, overview cache) | F4, D1 |
| F9 | fn_gui: menus, confirm boxes and prompts as egui modals | F4 |
| F10 | fn_gui: title, nation, seed and load screens | F4 |
| F11 | fn_gui: embark | F10 |
| F12 | fn_gui: the page frame and the Build screen | F4 |
| F13 | fn_gui: the Research screen and the school box | F12 |
| F14 | fn_gui: Saves and naming | F12, #860 |
| F15 | fn_gui: Traders and Abilities | F12 |
| F16 | fn_gui: Nation roster and Endeavor | F12 |
| F17 | fn_gui: message log, key help and legend | F12 |

Decisions: D1 (map tile shape), D2 (typeface), D3 (what `cargo run` opens), D4 (dev tools stay
cells).

## Open questions for the owner

1. **D1. Square map tiles in the window, or the terminal's tall cells?** Recommended: square.
   Shapes read true and drags cover what they look like they cover. The cost is fewer tiles
   across at a given zoom, and the window stops looking exactly like the terminal.
2. **D2. One monospace family throughout, or a proportional face for panel text?**
   Recommended: monospace throughout (Hack + DejaVu Sans Mono). It keeps the terminal identity,
   aligns numbers, adds no download and matches cell-painted screens. A proportional face reads
   better in long text (the log, tech descriptions) and looks less like a tool.
3. **D3. What does `cargo run` open?** Recommended: the TUI until rows 1–7 of the order are
   native, then the window, with `cargo run -p fn_tui` for the terminal. Switching earlier puts
   a half-converted window in front of every session. Never switching keeps the terminal as the
   front door after the window is the better game.
4. **D4. Do the watcher, studio and run browser stay cell-painted for good?** Recommended: yes.
   They are agent and dev tools, desktop only, and change often. Converting them is about 6 more
   PRs for screens only developers open.

## Not doing

- Converting anything in this issue, or changing `fn_tui`'s behaviour (#857 moves code, with
  goldens unchanged).
- A DOM web panel (#843 dropped it), sprites or pixel art, a light theme.
- Mouse-only features: everything stays reachable by key, and agents keep playing through keys.
