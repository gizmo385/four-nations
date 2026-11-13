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
   conversion moves them into `fn_ui` first. The side panel goes first (#873).
3. **Input goes through keys.** Every mouse action is a key press the shell already handles, or
   a `UiState` method the key handler also calls (`point_at`, `select`). `fn_tui::pointer`
   already works this way. A click's command log is the keyboard's, so saves, replays and agents
   can't tell which one was used.
4. **Convert a screen at a time and paint the rest as cells.** Our own cell painter (#872)
   replaces `egui_ratatui`. It is cheap enough to keep the unconverted screens, and the
   terminal-only tools (watcher, studio, run browser) inside the window indefinitely.
5. **Order: map frame, side panel, inspector, mouse, boxes, title, embark, then the full
   screens.** That way the 90% of play time spent looking at the map and panel is native first.
6. **The Seal look in egui, dark only.** Visuals come from `fn_ui::theme` (no new data). Square
   corners, accent frames, the `◆ NATION ◆` seal, `◇` headings. One bundled monospace family:
   Hack plus DejaVu Sans Mono for the ten glyphs Hack lacks (#869).
7. **CI tests `fn_gui` without a GPU.** `egui_kittest` drives the UI and queries its AccessKit
   tree by label, which needs no renderer. Pixel snapshots stay out of the gate. A
   `cargo xtask gui-shot` writes PNGs for a reviewer to look at.

## What exists (crates/gui at 108f4f9)

| Module | What it does | Fate |
|---|---|---|
| `main.rs` | Native `eframe::run_native`; wasm `WebRunner` at the title screen, or a game from a `?seed=` query (#866), with Quit hidden | Stays |
| `app.rs` (`GuiApp`) | Reads the clock, translates keys (F12 is the window's own: the timing overlay), hands each frame to `Frame` | Stays: shell, keys and clock only (#876) |
| `frame.rs`, `hints.rs` | `surface` (the exhaustive native/cells match), the native layout (hint bar, side panel slot, map, banner), the box painter, the timing overlay; the hint bar from `fn_ui::keys` | New in #876. Arms flip to native as screens convert |
| `side.rs` | The side panel: `fn_ui::panel::SidePanel` as egui widgets (title, header with pause and speed buttons, Settlement, Alerts, Tool section and list), monospace, tones coloured by `fn_tui::widgets::ToneStyle`. A click returns the keys it stands for (`side::Clicks`), which `GuiApp` types into the shell after the frame; an alert is `L`, a `Down` per line above it and `Enter` (`side::alert_keys`). The inspector (`inspector.rs`) is its last section: `fn_ui::inspector::Inspected` as egui widgets, meters as bars, a pinned heading over a body that scrolls in at most two fifths of the panel (#882) | New in #877 |
| `mapview.rs`, `camera.rs` | Glyph map from `fn_tui::map::cell_look` + `style_cell`, pan, zoom 1–48 px, block overview below 9 px | Stays. Square tiles (#868's default), click semantics from `fn_ui::pointer`, a hover tooltip and a cached overview texture, `overview.rs` (#878; done) |
| `notices.rs` | The corner notices (#945): `fn_ui::notice::Notices` as boxes (340 points) stacked in the map's top right, newest on top, kept until dismissed, each with a heading, a line or two, its `NoticeAction` buttons and a close button; past four a "+N more" row. Clicks come back as `notices::Click` for the frame to carry out. The window turns notices on (`Notices::set_on`), and so does the terminal (#962, `fn_tui::notices`) | New in #945 |
| `keys.rs` | egui `Event` to `fn_tui::keyev::KeyEvent` | Stays, retargeted to #857's frontend-neutral key type |
| `fonts.rs` | Loads DejaVu Sans Mono from the system path | Bundles it with `include_bytes!` (#859) |
| `color.rs` | ratatui `Color` to `Color32` | Stays (the map look and the cell painter both need it) |
| `tests/` | Key translation, camera maths, glyph coverage, one wgpu snapshot that panics without an adapter | Snapshot moves behind a feature; the rest join the gate (#875) |

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
| `modal`/`ask` boxes | `egui::Modal`, centred, dims what is behind (#879, `fn_gui::boxes`) | Takes every click, as `crate::modal` takes every key. Its rows are buttons that press the box's own keys and hold no focus, so `Enter` reaches the shell once. A click outside is `Esc` |

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
- **Modal** (#879): the game menu, the confirm boxes and embark's exit prompt are `egui::Modal`s
  over whatever is showing, native or cells (`frame::modal`, beside `frame::surface`). Over a
  cells screen the terminal's own frame is drawn without the box (`shell::draw_under_boxes`),
  so a prompt over the Saves screen needs nothing more. This replaced painting one box's rect
  out of the terminal frame.

The shell is always sized to the window in cells (`Shell::resize`), so pagination and box sizes
match what the cell painter shows.

**The cell painter (#872, done)** replaced `egui_ratatui` + `soft_ratatui`. The spike measured
them at 5–7 ms per frame in release because they CPU-rasterise every cell and upload a texture
each frame. On the web they cost 6 ms of the 14 ms frame. `fn_gui::cells` instead walks the
`ratatui::buffer::Buffer` once (`CellLayout`: one background run per stretch of equal
background in a row, one glyph per cell that shows something) and `cells::paint` turns that into
one mesh of run rectangles plus one text shape per glyph, in the bundled monospace font (Hack,
then DejaVu Sans Mono, then egui's monochrome emoji fonts), colours through `fn_gui::color`. The
grid is 8 by 16 points a cell (14 pt font, whole physical pixels), so the native window's
1600 by 960 shows 200 by 60 cells. `REVERSED`, `DIM`, `UNDERLINED` and `CROSSED_OUT` are drawn;
bold is not (egui has no bold monospace). Measured in release (layout plus shapes, egui's
tessellation on top): a full-screen page 0.3 ms (+0.1 ms), the map's own 10 000 glyphs 1.8 ms
(+1.5 ms). `tests/cells.rs` checks the glyph grid against `fn_tui --frame`'s text for every
screen and has the measurement (`cell_paint_cost`, ignored). Dropping those two crates also lifted
the egui 0.34 pin: eframe, egui and egui_kittest are on 0.37.

**The native frame (#876, done)** is `fn_gui::frame`. Its `surface` match gives every state one
of the three answers: the map (with a tool, a corner or a settler selected) is native; the game
menu and the confirm boxes are modals (#879); the title, embark and every screen and
the Build picker are cells-full, each arm flipping when its painter lands. A question asked
over a full screen stays in that screen's cells. The native frame never draws the terminal's own
map: before, every game frame built the 10 000 glyphs under `MapView` and threw them away.
Measured in release at 1600 by 960 (`cell_paint_cost`, layout plus shapes, tessellation on top):
a game frame's cells 1.5 ms (+0.8 ms) before; the side panel's slot, which is all of a native
game frame's cells before #877, 0.06 ms (+0.03 ms); the menu box 0.015 ms (+0.008 ms). About
2.2 ms a frame saved. The slot was `fn_tui::panel::draw_panel` at its own size in cells (37
columns at 300 points) until #877 made it native; `F12` shows a timing overlay (surface, map, cells, sim and
frame ms). Under kittest the root ui keeps an 8-point margin, so a 1600-point harness is 198
columns where a window is 200.

## Screen-by-screen order

One row is roughly one agent PR (the #842 estimates). "Words" means a row-model move into
`fn_ui` is part of the job.

| # | What | Native as | Words to move first | Est |
|---|---|---|---|---|
| 1 | Cell painter, frame skeleton, hint bar, tool banner (#872, #876; done) | `fn_gui::cells`, `fn_gui::frame`, `fn_gui::hints` | none (`fn_ui::keys` already) | 2 |
| 2 | Side panel: header, Settlement glance, Alerts, Tool section (#873, #877) | `fn_gui::side` | `fn_tui::panel`'s `Data` rows and `widgets::{Row, Tone}` | 2.5 |
| 3 | Inspector (#882; done) | `fn_gui::inspector` | `fn_ui::inspector` (moved from `fn_tui::inspector`: settler, tile and placement rows) | 1 |
| 4 | Map mouse: click, select, one-tile apply, hover tooltip, overview cache (#878; done) | `mapview.rs` | none (`fn_ui::inspect::tooltip_lines`) | 1 |
| 5 | Game menu, Quit and confirm boxes, prompts, embark exit prompt (#879) | `fn_gui::boxes` | `fn_tui::menu` items, `fn_tui::ask` | 1 |
| 6 | Title, nation picker, seed step, Load (#880; done) | `fn_gui::title` | `fn_tui::title` steps | 1 |
| 7 | Embark (#883) | `fn_gui::embark`, reusing `MapView` | `fn_tui::embark` report | 1.5 |
| 8 | Shared page frame and the Build screen (#881) | `fn_gui::page`, `fn_gui::screens::build` | (`fn_ui::build` has them) | 1.5 |
| 9 | Research tree and the school box (#884) | `fn_gui::screens::research` | (`fn_ui::research` has most) | 2 |
| 10 | Saves and naming (#885; done) | `fn_gui::screens::saves` | (`fn_ui::saves`) | 1 |
| 11 | Traders and Abilities (#886) | `screens::{traders, abilities}` | (`fn_ui::trade`, `fn_ui::bend`) | 2 |
| 12 | Nation roster and Endeavor (#887; done) | `fn_gui::screens::{nation, endeavor}`, `fn_gui::gauge` (meters as bars) | (`fn_ui::screens::{nation, endeavor}` have them; the roster groups moved in #857) | 2 |
| 13 | Message log, key help and legend (#888) | `screens::{log, help}` | (`fn_ui::msglog`, `fn_ui::keys`), `fn_tui::legend` rows | 2 |

**Stays cells:** the watcher (`fn_tui::watch`), the studio (`fn_tui::studio`), the run browser
(`fn_tui::browser`, `runs`) and spectate. They are agent and dev tools, desktop only (#866 hides
Replays on the web), and they change often. Cells cost almost nothing once #872 lands (#871). Since #994 a replay's game pane is the native frame, read only (`fn_gui::replay`, which is
also the web replayer); its code pane, feed and strip are still cells.

Rows 2–4 are the milestone that matters. After them, an ordinary session (map, panel, cursor,
tools) is native, and only screens you open and close are cells. The menus and the title screen
come next because they are the first and last thing a browser visitor sees.

**The pattern for every conversion PR.** Add the egui painter. Flip its arm in `frame`'s match
from cells to native. Add a kittest test that the screen shows its key strings and reacts to its
keys and clicks (#875). Attach a `gui-shot` PNG to the PR. The TUI's goldens must pass without
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
| Text (the non-interactive widget stroke; `override_text_color` stays unset so a button's text can be `on_accent`), weak text | `text`, `dim` | |
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

**Fonts.** One monospace family for everything (#869): egui's bundled Hack, with DejaVu Sans Mono
(about 340 KB, Bitstream Vera licence) as fallback for the ten `data/tui.toml` glyphs Hack lacks
(`✤ ✢ ✣ Ϟ ⌂ ♆ ✦ ɱ ☼ ✶`). It is bundled with `include_bytes!` on every platform (#859), with a test
that every glyph resolves. Monospace keeps the terminal's identity, lines numbers up in the
panel and costs no third font. It also makes cell-painted screens sit beside native ones without
a jump in typeface.

**Map tiles.** The map is the glyph map at any zoom: no sprites, no art pipeline (vision.md).
`camera::CELL_ASPECT` (0.6) copies the terminal's stretched cell. The window has no reason to
keep it: a 5×5 building should look square. Recommended: square tiles with the glyph centred
(#868). A 1600 px-wide map region then shows about 40% fewer tiles across at the same height,
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
| F12 | Toggles the dev overlay (frame timings); there is no panel toggle any more (#877) | Left to the browser (devtools) |
| Ctrl+= / Ctrl+- | egui's UI zoom | The browser's zoom |

### Mouse

The TUI has no mouse (tui.md "Input": "Mouse is later"). Everything below is new and optional:
nothing needs the mouse, and the hint bar keeps naming keys.

| Where | Action | As which key or method |
|---|---|---|
| Map, Select | Left click | Cursor to the tile (`point_at`), then `Enter` if a unit can be selected there, else `Esc` if one is selected (deselect). With nothing selected a click on bare ground only moves the cursor: `Esc` would open the game menu |
| Map, Select | Left drag, right or middle drag | Pan the camera (widget-only state, no game effect) |
| Map, a tool | Left click | One-tile apply: `Enter` `Enter` on that tile (with a corner set, the click is the second corner: one `Enter`; the Gate and a chute's foot take one) |
| Map, a tool | Left drag | The rectangle: `Enter` at the start, cursor, `Enter` on release (`fn_tui::pointer` today) |
| Map, a tool | Right click | `Esc` (drop the corner, else leave the tool). In Select it does nothing |
| Map, tool list open | Any press | `Esc`: it closes the list and goes no further |
| Map | Wheel | Zoom about the pointer |
| Map | Hover for 0.4 s | Tooltip from `fn_ui::inspect::tooltip_lines`. It does not move the cursor, so the inspector stays on what the keyboard points at. This is vision.md's "hover anything and see why" |
| Side panel | Click a section's `[N]`/`[T]`/`[G]`/`[L]` | That key |
| Side panel | Click an alert | The message log's `Enter` on that entry (cursor to its place, or select its settler) |
| Side panel | Pause and speed buttons in the header | `Space`, `+`, `-` |
| Side panel | Click an alert's action button ("Open research") | `App::do_notice_action`: the jump or the screen it names (#945) |
| Map, corner notices | Click a notice's action button ("Jump to caravan", "Open research") | `App::do_notice_action`; the notice stays until dismissed (#945) |
| Map, corner notices | Click a notice's `✕` / "Dismiss all" on the "+N more" row | Dismisses it / every notice. `Del`, or `Esc` in Select with nothing selected, dismisses the top one (before `Esc` would open the menu) |
| Map, corner notices | `Enter` in Select with nothing selected | The top notice's first action, and the notice is done with |
| Full screens | Click a row or card / double-click | Move the highlight there / then `Enter` |
| Full screens | Wheel | The screen's own scroll keys |
| Boxes | Buttons | The box's own keys (`Enter`, `Esc`, the item's letter) |

Every row is a key or a `UiState` call the keys make, so tests compare command logs: a click
path and its keyboard path must produce the same `World` command log (`tests/pointer.rs`
already does this for drags). `fn_ui::inspect::Hint` already lets wording differ by input
device. v1 leaves it on keyboard wording.

## Testing in CI

`fn_gui` is in `cargo xtask ci` (#875). Its one pixel test needs a GPU or lavapipe ("No adapter
found" otherwise), so it sits behind the `shots` feature, which the gate leaves off.

**Semantic UI tests, no pixels (#875).**

- The gate builds, lints and tests `fn_gui`. Building needs no native libraries (they are
  `dlopen`ed at run time), and no test opens a window.
- `egui_kittest` without its `wgpu` feature: a `Harness` runs `GuiApp` frames headlessly, and
  tests find widgets through the AccessKit tree (`get_by_label("People 6")`,
  `.click()`, key presses). That covers "the panel shows X", "clicking the alert puts the
  cursor there" and "Esc opens the menu box". The cost is a software tree walk, not a renderer.
  Cell-painted text is invisible to AccessKit, so `fn_gui::cells::expose` labels each non-blank
  row of a region with its text (the side panel and hint bar while the map widget shows, the
  whole frame on a page or the title). `tests/semantic.rs` finds `Running x1` that way, presses
  Space, and finds `Paused x1`. Native widgets (#877) carry their own labels; the same words
  keep the tests valid.
- Logic tests stay as they are: key translation, camera maths, glyph coverage, and click-vs-key
  command logs.
- Clippy for `fn_gui` on `wasm32-unknown-unknown` is in `WASM_CRATES`. #859 adds the full web
  build to CI.
- The pixel snapshot and the `gui-shot` example are behind the `shots` cargo feature, off in the
  gate (so the gate does not compile wgpu, and does not lint `examples/shot.rs`).

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
download-size and GPU frame-time measurement), #860 (a save store behind an interface; done
with `localStorage`, deflated and base64'd, not IndexedDB: the store is synchronous, a small-map
save is about 190 KB stored against a 5 MiB quota, and `fn_gui::browser_saves` documents the
numbers) and #866 (Pages at `/play/`, title screen first, Quit and Replays
hidden, Playwright; done: `docs/design/agent-play.md`, "The hosted replayer", has the URL and the
build; `Shell::with_quit(false)` is how Quit goes, and Replays never shows with no runs folder). This doc adds one dependency: **#866 should also be blocked by #872**. The
public page shouldn't ship the CPU-rasterised terminal path the spike measured at 6 ms of a
14 ms browser frame. The title screen is cells until #880, which is fine once cells are cheap.

#858 (an optional `serde` feature on `fn_ui` view models) was for the DOM panel #843 dropped.
egui reads the Rust structs directly, and the agent surface has its own `Serialize` views
(`fn_tui::surface`). **It gates nothing here.** Keep it at p3 and close it if no consumer
appears.

## Follow-up issues

Filed from this doc as sub-issues of #856, alongside the existing #857, #858,
#859, #860 and #866:

| Id | Title | Blocked by |
|---|---|---|
| #872 | fn_gui: paint terminal cells ourselves; drop egui_ratatui and lift the egui pin | #857 |
| #875 | fn_gui in the gate: kittest semantic tests without a GPU; `cargo xtask gui-shot` | #872 |
| #874 | fn_gui: the Seal theme from `fn_ui::theme` | #859, #869 |
| #876 | fn_gui: the native frame (regions, hint bar, tool banner, native/cells dispatch) | #872 |
| #873 | Side panel rows into `fn_ui` (Row, Tone, section builders) | #857 |
| #877 | fn_gui: native side panel (header, glance, alerts, tool section) | #876, #873 |
| #882 | fn_gui: native inspector | #877 |
| #878 | fn_gui: mouse on the map (click, select, one-tile apply, tooltip, overview cache) | #876, #868 |
| #879 | fn_gui: menus, confirm boxes and prompts as egui modals | #876 |
| #880 | fn_gui: title, nation, seed and load screens | #876 |
| #883 | fn_gui: embark | #880 |
| #881 | fn_gui: the page frame and the Build screen | #876 |
| #884 | fn_gui: the Research screen and the school box | #881 |
| #885 | fn_gui: Saves and naming | #881, #860 |
| #886 | fn_gui: Traders and Abilities | #881 |
| #887 | fn_gui: Nation roster and Endeavor | #881 |
| #888 | fn_gui: message log, key help and legend | #881 |

Decisions: #868 (map tile shape), #869 (typeface), #870 (what `cargo run` opens), #871 (dev tools stay
cells).

## Open questions for the owner

1. **#868. Square map tiles in the window, or the terminal's tall cells?** Recommended: square.
   Shapes read true and drags cover what they look like they cover. The cost is fewer tiles
   across at a given zoom, and the window stops looking exactly like the terminal.
2. **#869. One monospace family throughout, or a proportional face for panel text?**
   Recommended: monospace throughout (Hack + DejaVu Sans Mono). It keeps the terminal identity,
   aligns numbers, adds no download and matches cell-painted screens. A proportional face reads
   better in long text (the log, tech descriptions) and looks less like a tool.
3. **#870. What does `cargo run` open?** Recommended: the TUI until rows 1–7 of the order are
   native, then the window, with `cargo run -p fn_tui` for the terminal. Switching earlier puts
   a half-converted window in front of every session. Never switching keeps the terminal as the
   front door after the window is the better game.
4. **#871. Do the watcher, studio and run browser stay cell-painted for good?** Recommended: yes.
   They are agent and dev tools, desktop only, and change often. Converting them is about 6 more
   PRs for screens only developers open.

## Not doing

- Converting anything in this issue, or changing `fn_tui`'s behaviour (#857 moves code, with
  goldens unchanged).
- A DOM web panel (#843 dropped it), sprites or pixel art, a light theme.
- Mouse-only features: everything stays reachable by key, and agents keep playing through keys.
