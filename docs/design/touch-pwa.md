# Touch play and an installable web app (landscape phones and tablets)

Doc status: current

Status: **accepted** (#971). The owner chose the direction on 2026-11-12 in the project thread;
see "Owner decisions" at the end. Mock-ups:
[Touch layout mock-ups](https://claude.ai/artifact/H8nH1PgJL977UY6FXuJFhX) (a Claude artifact,
private to the owner).

## The problem

The browser build (`fn_gui` compiled to wasm, published at `/play/`, docs/design/egui-frontend.md)
already loads on a phone: the page has a viewport tag, saves go to `localStorage` (#860) and a tap
is a click. But the game is built around the keyboard. Tools are letters, speed is `+`/`-`, saves
are F5/F6/F9 and leaving a tool is `Esc`. The window assumes a wide screen with a mouse: a 300-point
side panel, a hint bar of key names and hover tooltips. In the owner's first phone test, the side
panel and some menus ran off the bottom of the screen.

The goal: play on an iPhone or iPad held sideways, installed to the home screen, with no keyboard.

## The rule

**Landscape only, touch as a mouse.** Every touch control stands for the keys or clicks the game
already understands (egui-frontend.md, "Mouse"): the shell never learns that a finger was
involved. The layout gets one new arrangement for narrow or short windows. There is no portrait
layout.

## 1. The installable shell

- `web/play/manifest.webmanifest`: `name` "Four Nations", `display: standalone`,
  `orientation: landscape`, `start_url` and `scope` `/play/`, `background_color` and
  `theme_color` the window's ground (`#121212`). Icons: a 16-pixel-grid pixel-art mark scaled to
  192 and 512 (plus a maskable 512 with padding), and `apple-touch-icon` 180 in `index.html`.
- A service worker (`web/play/sw.js`) that caches `index.html`, `fn_gui.js` and the wasm, so an
  installed game opens offline. The cache name carries the build's hash, which `cargo xtask
  web-gui` writes in, so a new deploy replaces the cache instead of serving a stale wasm. Use a
  network-first strategy for `index.html` and cache-first for the hashed files.
- **Turn sideways.** iOS ignores the manifest's `orientation` and `screen.orientation.lock`, and
  Android browser tabs (not installed) do too. When the window is taller than it is wide, the page
  pauses the game and shows the turn-sideways screen (mock-up board 3: a pixel-art phone turning,
  "Turn your phone sideways", "Your settlement is paused until you turn back"). Turning back hides
  it. The game resumes only if the screen paused it: a game the player had paused stays paused.
  The screen is drawn by egui (its own `Surface`), not by HTML, so it follows the theme and the
  shell knows the game is paused.
- `index.html` gets `viewport-fit=cover`, and the egui frame keeps its controls inside
  `env(safe-area-inset-*)` (the notch and the home indicator), which the page passes to the wasm.
- The canvas gets `touch-action: none`, `-webkit-touch-callout: none` and `user-select: none`,
  so the browser doesn't scroll the page, zoom it, or show a callout on a long press.

## 2. Save export and import

An installed iOS web app keeps storage separate from Safari's, and iOS may clear it if the app
goes unused. The saves screen (F6, #885) and the More sheet (below) get **Export to file**, which
downloads the highlighted save as a `.fnsave` file (the same bytes `localStorage` holds), and
**Import file**, a file picker that adds a slot. This is useful on desktop web too. Natively,
export and import are not needed: saves are already files.

On touch, new saves are **named for you** ("Fire · Autumn Y4"; a number is added if the name is
taken), because egui's text field and iOS's on-screen keyboard don't work together reliably. A
keyboard still types a name as today. Renaming on touch waits until the keyboard is shown to work.

## 3. Gestures on the map

A tap is a click, and the map's mouse rules (`fn_ui::pointer`, `crates/gui/src/mapview.rs`) carry
over:

| Gesture | Select | A tool picked | Mouse today |
|---|---|---|---|
| Tap | point at the tile, then select or deselect | one-tile apply | left click |
| One-finger drag | pan | the rectangle | left drag |
| Two-finger drag | pan | pan (the rectangle in progress is cancelled) | right or middle drag |
| Pinch | zoom about the fingers' midpoint | zoom | wheel |
| Long-press (0.5 s, < 10 pt of movement) | the tile's tooltip, until the finger lifts | the same | hover |

- A second finger landing during a one-finger rectangle drag cancels the rectangle: no command is
  issued for it.
- A long press shows the tooltip without moving the cursor, as hover does, and lifting dismisses
  it. A long press never applies a tool.
- egui reports touch through pointer events and `zoom_delta`/`multi_touch`. Whether two-finger pan
  arrives as `multi_touch().translation_delta` on iOS Safari is the first thing the gesture issue
  checks, on the owner's iPhone.

## 4. The tool rail and the bottom bar

- **The tool rail** runs down the left edge (mock-up boards 1 and 4): Select, Harvest, Build,
  Stockpile, Bend, Sow, Clear, Remove, the order of `Tool::ALL` with Select first. Each button is
  the tool's key (`Esc` for Select). The active tool is filled with its tint. Buttons are 44×44 pt
  with icons only on a phone; on a tablet the rail is 76 pt wide with the tool's name and key
  letter under the icon. Picking Build or Bend opens its list, as the letter does
  (tool-pickers.md). The rail always shows on touch devices and is a setting (default off) on
  desktop, where the owner may want it as a mouse alternative to the letters.
- **The bottom bar** is the hint bar as buttons. On the left are the mode badge and the shortest
  hint. On the right are two to four buttons for that mode: in Select, `‹ Units ›` (Tab and
  Shift-Tab), Research (`T`), Nation (`N`) and Log (`L`); with a tool, Repeat for Harvest (`O`)
  and **Done** (`Esc`).
- **Time and More.** Pause (`Space`), speed (cycling 1, 2, 3, 5 with `+`) and a More button float
  at the map's top-right corner while the panel is a closed drawer. With the panel docked or open,
  they sit in its header. **More** opens a sheet (board 6) with the speed row (Pause, x1, x2, x3,
  x5), Quicksave (F5), Quickload (F9), All saves (F6), Export, Import and the game menu.

## 5. The compact layout

| Window (points) | Rail | Side panel | Time controls |
|---|---|---|---|
| ≥ 1000 wide and ≥ 600 tall (iPad, desktop) | 76 pt with names (touch or setting) | docked, 300 pt, as today | panel header |
| smaller (a phone sideways) | 56 pt, icons | a drawer from the right, closed by default | float on the map |

- **The drawer** (board 2) slides over the map from the right, the panel's normal 300 pt, and
  closes with its chevron or a tap on the map. Closed, it leaves a tab on the right edge with the
  open-alert count. Opening it doesn't pause the game.
- **Everything scrolls.** The side panel's sections, the inspector, every full-screen page (Build,
  Research, Abilities, Traders, Saves, Nation, Endeavor, the log, the key help), the title and
  embark steps, and the boxes scroll when they are taller than the window, by drag on touch and
  by wheel with a mouse. Parts a page pins today (the Nation screen's header and stores, the
  inspector's heading) give way to the scroll when pinning them would leave the scrolling part
  less than a third of the window.
- **Tap targets** in lists (Build, Research cards, Abilities, Saves, the panel's section links and
  alerts) are at least 44 pt tall on touch.
- The breakpoints are in egui points, which are CSS pixels in the browser (eframe sets
  `pixels_per_point` from `devicePixelRatio`).

## 6. Performance

The sim runs on the browser's main thread. Before the compact layout is called done, measure a
late-game save (the harness's year-8 champion is a good one) on the owner's iPhone: frame time
and sim ticks per second at each speed. If speed 5 can't keep up, the fix (moving the sim to a Web
Worker, or capping speed on touch) is its own design question.

## Follow-up issues

1. #973 Scrollable side panel, pages, boxes, title and embark (desktop too)
2. #974 PWA shell: manifest, icons, service worker, safe areas, touch CSS
3. #975 Turn-sideways screen (after #974)
4. #976 Save export and import, plus automatic save names on touch
5. #977 Touch gestures on the map
6. #978 Tool rail, bottom-bar buttons and the More sheet
7. #979 Compact layout: the panel drawer, floating time controls, 44 pt targets (after #978)
8. #980 Performance spike on iPhone (needs the owner's phone)

## Not chosen

- **A portrait layout.** The map and the panel don't share a tall, narrow screen well, and the
  owner prefers landscape.
- **An HTML overlay for the touch controls.** It would split the UI between DOM and egui and need a
  second input path. The rail and buttons are egui widgets, so desktop gets them too.
- **Tablet first.** The owner wants phones from the start.

## Owner decisions

Recorded from the project thread, 2026-11-12:

- Phones from the start, landscape only, tested on an iPhone.
- A tool rail on the left; if it's good, it may also be offered on desktop as an alternative to
  key presses.
- The gesture table above, including one-finger drag meaning pan in Select and the rectangle with
  a tool.
- Touch saves are auto-named.
- The side panel and menus must scroll: in the first phone test they were too long.
