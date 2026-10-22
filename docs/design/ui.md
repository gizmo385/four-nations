# UI visual direction

Status: **draft, waiting on the owner's pick** (#39). Three mockups below. Once the owner chooses,
this doc gets the final tokens and follow-up issue numbers. The shared `fn_client` style module
waits until then too.

Today the UI is three debug text boxes (hover, selection with #3's needs, HUD). Research (#23),
jobs and commands (#5) and bending (#6) will all add widgets. This doc fixes one look
and one layout before they land.

## What the owner already decided (issue #39)

- **Feel:** clean, flat panels, with Avatar-themed flair *around the edges* and a light accent
  colour per nation. The comment trails off after "e.g.". From the options listed in the issue
  body, I read it as: scroll and parchment motifs and element-coloured accents are welcome as
  trim, but panel bodies stay flat (no textures, gradients or bevels behind text). That's the
  constraint every variant below follows.
- **Typography:** Bevy's default font (Fira Mono subset, monospaced). No font asset to record.
- **Layout:** persistent info in a **side panel**. Selecting something opens a **bottom bar**
  with its actions.
- **Nation colouring:** yes. The accent is the nation's `color` from `data/nations.toml`, so it
  stays data-driven and needs no new schema.

## Fixed across all variants

The layout and the palette *structure* are the same in every mockup, so the owner compares only
the styling.

### Layout zones (1280×720 reference)

| Zone | Where | Contents | Behaviour |
|---|---|---|---|
| **Side panel** | Right edge, full height, 272 px | Nation emblem and name; date and speed controls; People; Stores; Research (#23); Alerts | Always visible. Sections are added in this order. A section with nothing to show hides itself |
| **Selection bar** | Bottom, from the left edge to the side panel, 128 px | Portrait; name, role and current activity; needs bars (#3); action buttons with hotkeys | Present only while something is selected. Esc closes it. Units, tiles and (later) buildings each define their own content and actions |
| **Hover tooltip** | Next to the cursor | Up to 4 lines: name, context, a yield or reason, then an action hint in accent colour | Pillar 4 (readable simulation): hover anything to see why. Tooltips never hold buttons |
| **Screens** | Centred over a dimmed map | Research tree (#23), later logs and overviews | Use the same panel style. The side panel stays visible; the selection bar hides |

Map framing: the camera centres in the area the panels leave free, so the selected unit isn't
hidden behind the bar.

### Palette tokens

One `Theme` per (variant, nation) pair, computed at startup from the nation's colour:

| Token | Use |
|---|---|
| `panel` | Panel body. Opaque, so snow and sand can't wash it out |
| `inset` | Bar tracks, button bodies, portrait well |
| `text`, `dim` | Values and labels |
| `accent` | Nation colour, unchanged from `nations.toml`: progress fills, active button, emblem, flair |
| `accent_text` | The accent lightened or darkened until it reads as text on `panel` |
| `accent_soft` | The accent blended 50–60% into `panel`: borders, section rules |
| `on_accent` | Text on an accent fill |
| `ok`, `warn`, `bad` | Status colours. They are the same for every nation, so a hunger warning never changes colour with the nation |

### Type scale and spacing

Default font only: 20 px (selection name), 18 px (panel title), 14 px (body, rows, buttons),
12 px (section titles in upper case, small labels). Spacing steps: 4, 6, 8, 12, 16 px.

## The three variants

Each variant was mocked with `FN_UI_MOCKUP=<variant> cargo xtask screenshot 7 <nation> ...`
(headless, no window). Each variant is shown for Earth and for a second nation, to show the
accent. The needs bars use #3's real set (health, hunger, thirst, energy), but all the numbers
are placeholders: stores, roles and the research percentage don't exist in the sim yet. Mockup
code and PNGs are on the unmerged reference branch `issue/39-mockups`
(`crates/client/src/mockup.rs`, `mockups/39/`) and won't ship as is. For comparison, today's UI is
[here](https://github.com/gizmo385/four-nations/blob/c4c5ed64ffed0cb603192728dec9fb02b11e1e3a/mockups/39/baseline-earth.png?raw=true).

### A. Scroll

![Scroll, Earth Kingdom](https://github.com/gizmo385/four-nations/blob/c4c5ed64ffed0cb603192728dec9fb02b11e1e3a/mockups/39/scroll-earth.png?raw=true)
![Scroll, Fire Nation](https://github.com/gizmo385/four-nations/blob/c4c5ed64ffed0cb603192728dec9fb02b11e1e3a/mockups/39/scroll-fire.png?raw=true)

Dark, warm ink panels. The side panel is capped top and bottom by wooden scroll rods with
accent-coloured end caps, and the selection bar has the rods at its ends, like an unrolled
scroll. The motif comes straight from the show (scrolls, Wan Shi Tong's library) and lives
entirely outside the panel body. Rounded corners, 3 px.

- **For:** the most recognisably Avatar of the three while keeping the bodies flat. The nation
  accent appears in only a few places (knobs, emblem, fills), which keeps it "light".
- **Against:** the rods and caps stick out up to 20 px past the panels, which costs a little screen
  space and needs care with modal screens and small windows.

### B. Seal

![Seal, Earth Kingdom](https://github.com/gizmo385/four-nations/blob/c4c5ed64ffed0cb603192728dec9fb02b11e1e3a/mockups/39/seal-earth.png?raw=true)
![Seal, Water Tribes](https://github.com/gizmo385/four-nations/blob/c4c5ed64ffed0cb603192728dec9fb02b11e1e3a/mockups/39/seal-water.png?raw=true)

Cool, dark slate panels with a thin accent-tinted frame, square corners, accent corner brackets,
and a diamond "seal" centred on the top edge. Section titles get a small diamond bullet.

- **For:** the cleanest and most compact option. Nothing extends past the panels and it scales
  to dense screens (research tree, logs) without fuss.
- **Against:** the flair reads as "stylish UI" more than "Avatar". The nation accent is in the
  frame all the time, so it's the strongest accent of the three. Thin accent lines also vanish
  against terrain of a similar colour (Earth's green brackets on grassland).

### C. Parchment

![Parchment, Earth Kingdom](https://github.com/gizmo385/four-nations/blob/c4c5ed64ffed0cb603192728dec9fb02b11e1e3a/mockups/39/parchment-earth.png?raw=true)
![Parchment, Air Nomads](https://github.com/gizmo385/four-nations/blob/c4c5ed64ffed0cb603192728dec9fb02b11e1e3a/mockups/39/parchment-air.png?raw=true)

Light parchment panels with dark ink text. A solid accent band with scalloped trim runs along
the panel's inner edge: the left edge of the side panel and the top edge of the selection bar.

- **For:** the warmest and most "paper scroll" feel. Very readable at 14 px.
- **Against:** a big light area competes with the pixel-art map, especially on snow, desert and
  coast tiles. Some nation colours lose contrast on cream (Air's yellow most of all), so
  `accent_text` has to darken them a lot.

### Recommendation

**A, Scroll.** It's the only variant whose flair says "Avatar" at a glance, and it does it
entirely outside the panel body, which is exactly the owner's "flat panels, flair around the
edges". The accent stays light: a few caps, the emblem, fills and the active button. Dark panels
also sit better against the bright pixel-art map than parchment does. B (Seal) is a close second
if A's rods feel heavy. It's the better fit if the UI gets much denser later.

## Code shape once a variant is picked

- `crates/client/src/ui/style.rs`: `Theme` (the tokens above), built from `NationDef::color`, and
  the layout constants. Every screen reads colours from `Res<Theme>`, never literals.
- `crates/client/src/ui/widgets.rs`: `panel(kind)` (body plus edge flair), `section_title`,
  `row`, `bar`, `button`, `tooltip`. #23, #5 and #3's client parts build only from these.
- The palette stays in code, not in `data/`: it's presentation, not balance, and the only
  per-nation part (the accent) already comes from `nations.toml`. If one nation's colour reads
  badly as an accent, add an optional `ui_accent` to that nation in `nations.toml` instead of
  special-casing it in code.

## Follow-up issues

To be filed after the owner picks: the shared style module; moving the current debug panels into
the side panel, selection bar and tooltip; and constraining #23 (research screen) to the style.

## Open questions for the owner

1. **Which variant: A Scroll, B Seal or C Parchment?** A mix is fine too (for example Scroll's
   rods with Seal's slate colours). *Recommendation: A, Scroll.*
2. **Side panel on the right or the left?** *Recommendation: right.* The hover tooltip and the
   selection bar then grow from the left, where the eye starts, and the persistent numbers sit
   out of the way.
3. **Anything about tone or density to change** in the chosen variant: panel opacity, how
   strong the accent is, corner rounding? *Recommendation: keep it as mocked and adjust once
   real data fills the panels.*
