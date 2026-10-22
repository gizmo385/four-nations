# UI visual direction

Status: **decided** (#39, owner, 2026-10-22): the **Seal** style, with the side panel on the
right and no other changes to the mockup. The shared code lives in
`crates/client/src/ui_style.rs`. Follow-ups: #80 restyles the existing panels, and #23 (the
research screen) must build from the module.

Today the UI is three debug text boxes. Research (#23), jobs and commands (#5) and bending (#6)
will all add widgets. This doc fixes one look and one layout before they land.

## Decision

![Seal with Ninja Adventure art, Water Tribes](https://github.com/gizmo385/four-nations/blob/92ea0705fc25b2dc46ec2b8c1fdfd848d95ef781/mockups/1/ninja-seal-ui.png?raw=true)

- **Feel:** clean, flat panels with Avatar-themed flair only around the edges, and a light
  nation accent. Seal does it with cool, opaque slate panels, a 1px frame tinted with the
  accent, square corners, L-shaped accent **corner brackets**, and a **diamond seal** centred on
  the top edge. Section titles get a small diamond bullet. Panel bodies stay flat: no textures,
  gradients or bevels behind text.
- **Typography:** Bevy's default font (Fira Mono subset, monospaced). No font asset.
- **Layout:** persistent info lives in a **side panel on the right**. Selecting something
  opens a **bottom bar** with its actions.
- **Nation colouring:** the accent is the nation's `color` from `data/nations.toml`, so it stays
  data-driven with no new schema.
- **Art:** the map uses Ninja Adventure (`docs/design/art.md`). The render above shows the
  combination; see "Fitting Seal to Ninja Adventure" below.

## Layout zones (1280×720 reference)

| Zone | Where | Contents | Behaviour |
|---|---|---|---|
| **Side panel** | Right edge, full height, 272 px | Nation emblem and name; date and speed controls; People; Stores; Research (#23); Alerts | Always visible. Sections appear in this order. A section with nothing to show hides itself |
| **Selection bar** | Bottom, from the left edge to the side panel, 128 px | Portrait (Ninja Adventure faceset, #76); name, role and current activity; needs bars (#3); action buttons with hotkeys | Present only while something is selected. Esc closes it. Units, tiles and (later) buildings each define their own content and actions |
| **Hover tooltip** | Next to the cursor | Up to 4 lines: name, context, a yield or reason, then an action hint in `accent_text` | Pillar 4 (readable simulation): hover anything to see why. Tooltips never hold buttons. Small brackets, no seal |
| **Screens** | Centred over a dimmed map | Research tree (#23), later logs and overviews | Same panel style with the full frame. The side panel stays visible; the selection bar hides |

The camera centres the selected unit in the area the panels leave free, so the bar doesn't
hide it.

## Colour tokens

`Theme::for_nation(color)` in `ui_style.rs` builds one theme per nation. Every screen reads
colours from it and never uses literals.

| Token | Value | Use |
|---|---|---|
| `panel` | sRGB (0.07, 0.08, 0.10), opaque | Panel body, so snow and sand can't wash it out |
| `inset` | (0.04, 0.045, 0.06) | Bar tracks, button bodies, portrait well, flair outlines |
| `text` / `dim` | (0.93, 0.94, 0.96) / (0.58, 0.61, 0.67) | Values / labels |
| `accent` | The nation's `color`, unchanged | Progress fills, active button, emblem, brackets, seal |
| `accent_text` | Accent mixed 25% toward white | Accent-coloured text on `panel` |
| `accent_soft` | Accent mixed 55% into `panel` | Panel frame, section rules, idle button borders |
| `on_accent` | (0.05, 0.05, 0.07) | Text on an accent fill |
| `ok`, `warn`, `bad` | Green, amber, red | Status, the same for every nation, so a hunger warning never changes colour |

A unit test checks that `accent_text`, `text` and `dim` reach at least 4.5:1 contrast on
`panel` (WCAG AA) for every nation in `data/nations.toml`, and that `on_accent` reaches 3:1 on
`accent`. A new nation colour that would be unreadable fails CI.

## Type scale and spacing

Default font only, in `ui_style::font`:

| Constant | Size | Use |
|---|---|---|
| `NAME` | 20 px | Name in the selection bar |
| `TITLE` | 18 px | Panel title |
| `BODY` | 14 px | Rows, buttons, tooltips |
| `LABEL` | 12 px | Upper-case section titles, small labels |

Spacing steps in `ui_style::space`: 4, 6, 8, 12, 16 px. Layout constants are in
`ui_style::layout`: side panel width, bar height, screen margin and bracket offset.

## Builders

`ui_style.rs` provides:

- `panel(theme, node)`: the body, with 1px `accent_soft` border, square corners and a soft shadow.
- `frame(p, theme, Frame::Panel | Frame::Tooltip)`: the flair, added inside the panel's
  children. It's absolutely positioned, so it never affects layout.
- `corner_brackets` and `seal`: the flair parts on their own.
- `section_title`, `bar`, `button` and `text`.

New screens use only these. If a screen needs something new, add a builder here rather than
styling inline.

## Fitting Seal to Ninja Adventure

- **Contrast with the map.** Ninja Adventure is warm and saturated (yellow-green grass, orange
  sand, cyan water). Cool, opaque slate panels separate cleanly from it, which is part of why
  Seal works.
- **Outlines.** The pack outlines everything in near-black. The brackets and seal get the same
  1px `inset` outline, so they read like sprites and stay visible when the accent matches the
  terrain (Earth's green brackets on grass vanished without it in the mockup).
- **Portraits.** Facesets are 38×38. Draw them at an integer scale (2× = 76 px) with nearest
  sampling inside the 96 px portrait well, so pixels stay crisp.
- **Icons.** Tech and item icons (#23) come from the pack's items and elemental FX. Draw them at
  integer scale too, on `inset`.
- **Fonts.** The pack ships two pixel fonts, but the owner chose Bevy's default font. Revisit
  only if the owner asks.

## Considered (not chosen)

All three variants shared the layout above and differed only in edge flair. The mockups were
rendered headless (`FN_UI_MOCKUP=<variant> cargo xtask screenshot 7 <nation>`). The code is on
the unmerged reference branch `issue/39-mockups`, and won't ship.

- **A. Scroll** ([Earth](https://github.com/gizmo385/four-nations/blob/c4c5ed64ffed0cb603192728dec9fb02b11e1e3a/mockups/39/scroll-earth.png?raw=true),
  [Fire](https://github.com/gizmo385/four-nations/blob/c4c5ed64ffed0cb603192728dec9fb02b11e1e3a/mockups/39/scroll-fire.png?raw=true)):
  warm ink panels capped by wooden scroll rods with accent end caps. It was my recommendation;
  the owner preferred Seal's cleaner look.
- **B. Seal** ([Earth](https://github.com/gizmo385/four-nations/blob/c4c5ed64ffed0cb603192728dec9fb02b11e1e3a/mockups/39/seal-earth.png?raw=true),
  [Water](https://github.com/gizmo385/four-nations/blob/c4c5ed64ffed0cb603192728dec9fb02b11e1e3a/mockups/39/seal-water.png?raw=true)):
  **chosen**.
- **C. Parchment** ([Earth](https://github.com/gizmo385/four-nations/blob/c4c5ed64ffed0cb603192728dec9fb02b11e1e3a/mockups/39/parchment-earth.png?raw=true),
  [Air](https://github.com/gizmo385/four-nations/blob/c4c5ed64ffed0cb603192728dec9fb02b11e1e3a/mockups/39/parchment-air.png?raw=true)):
  light parchment panels with a scalloped accent band. It competed with the bright map, and
  Air's yellow lost contrast on cream.

For comparison, [the UI before #39](https://github.com/gizmo385/four-nations/blob/c4c5ed64ffed0cb603192728dec9fb02b11e1e3a/mockups/39/baseline-earth.png?raw=true).

## Follow-ups

- **#80:** restyle the HUD, the selection and hover info, and the job and designation lines into
  the side panel, selection bar and tooltip using `ui_style`; delete `panel_style`.
- **#23:** the research screen and HUD progress must use `ui_style` and this doc (commented on
  the issue, which is now blocked by #80).
- Portraits arrive with #76 (Ninja Adventure facesets).
