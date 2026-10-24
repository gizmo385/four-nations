# Claude-authored pixel art

Status: **proposed** (#122). Waiting on the owner's answers under "Open questions for the owner".
Follow-ups: #128 (tool), #129 (UI frame and emblems), #130 (icons), #131 (map gap fillers).

The owner wants Claude to make the game's pixel art, starting with the UI, so it can replace the
third-party packs: Ninja Adventure (`docs/design/art.md`) and the code-drawn Seal flair
(`docs/design/ui.md`). Original art makes the look our own, fills the holes the pack audit found
(no cabbage, no inner cliff corners, a thin Air Nomad cast), and lets an agent draw art for a new
feature instead of hunting for a pack that has it.

Claude can't paint, but it can write text. So art is **authored as palette-indexed character
grids**, and a repo tool renders them to PNG. Every sample below was made that way.

## Decisions in brief

1. **Source is text.** One TOML file per sprite sheet under `art/`, one character per pixel,
   every character a name from one shared palette (`art/palette.toml`).
2. **Our own renderer.** `cargo xtask art` turns `art/*.toml` into `assets/original/*.png` and
   their manifest entries. `cargo xtask assets`, already part of `cargo xtask ci`, re-renders in
   memory and fails if a PNG is stale or holds an off-palette colour. No new tools and no Python.
3. **Pack conventions, our palette.** 16px cells, a 1px near-black outline, 3 to 5 tone ramps lit
   from the top left, no anti-aliasing. That's what Ninja Adventure does, so the two can share
   the screen during the transition.
4. **Nations through data.** Accent pixels are a 3-tone ramp mixed from the nation's `color` in
   `data/nations.toml`. UI tokens (`panel`, `inset`, `accent_soft`) come from `fn_ui::theme`, so a
   pixel frame matches the code-drawn widgets exactly.
5. **UI first, then gaps, then (maybe) everything.** Order: the tool, the Seal frame and emblems,
   item and tool icons, the pack's stand-ins. Terrain and units only after a one-biome trial and
   an owner decision.

## Samples

Each sample shows 1x on top and 4x below. Sources: `mockups/122/src/*.toml`, rendered by
`mockups/122/render.py` (the Python prototype of #128) and laid out by `mockups/122/mock.py`.

![Seal frame and seal, per nation](https://github.com/gizmo385/four-nations/blob/de2082f277dbcf42abd5364d9c7fb0a84aa9ec08/mockups/122/sample-frame.png?raw=true)
![Nation emblems](https://github.com/gizmo385/four-nations/blob/de2082f277dbcf42abd5364d9c7fb0a84aa9ec08/mockups/122/sample-emblems.png?raw=true)
![Item icons](https://github.com/gizmo385/four-nations/blob/de2082f277dbcf42abd5364d9c7fb0a84aa9ec08/mockups/122/sample-icons.png?raw=true)
![Tool icons](https://github.com/gizmo385/four-nations/blob/de2082f277dbcf42abd5364d9c7fb0a84aa9ec08/mockups/122/sample-tools.png?raw=true)

**Side by side with what ships today.** Left is a crop of the #39 reference render (the
code-drawn Seal panel over Ninja Adventure). Right is the same panel built from the samples at 2x,
with a two-column Stores grid and a tool row:

![Seal today vs Claude-authored](https://github.com/gizmo385/four-nations/blob/de2082f277dbcf42abd5364d9c7fb0a84aa9ec08/mockups/122/compare-ui.png?raw=true)
![The other three nations](https://github.com/gizmo385/four-nations/blob/de2082f277dbcf42abd5364d9c7fb0a84aa9ec08/mockups/122/panel-nations.png?raw=true)

**Map sprites next to the pack**, on the pack's grass, using the in-repo Ninja Adventure cells the
client ships (`sprites::RESOURCES`): the cabbage stand-in (8,11), stone (6,14) and gold vein
(4,14):

![Ninja Adventure vs Claude-authored map sprites](https://github.com/gizmo385/four-nations/blob/de2082f277dbcf42abd5364d9c7fb0a84aa9ec08/mockups/122/compare-map.png?raw=true)

What the comparison shows: at UI scale the hand-authored art holds up well. The emblems and
icons read at 2x, and the frame looks like a sprite rather than a box. On the map, the
cabbage is a clear improvement on the stand-in. Our rocks are smoother and cleaner than the
pack's mottled ones, which would stand out next to pack boulders, so map art should either
copy the pack's texture or replace whole families at once (open question 1).

## Authoring format

```toml
# art/icons.toml -> assets/original/icons.png
cell = [16, 16]          # every sprite in the sheet is one cell; the sheet packs them in order
columns = 8              # optional; default is one row
outline = true           # optional: 1px `ink` outline around every filled pixel (4-neighbour)
per_nation = false       # true: one PNG per nation, `icons.<nation>.png`, accent keys resolved per nation

[legend]                 # optional: adds to palette.toml's [legend]
g = "gold2"

[[sprite]]
name = "stone"
legend = { "1" = "stone0", "2" = "stone1", "3" = "stone2", "4" = "stone3", "5" = "stone4" }
grid = """
................
......55444.....
....554444443...
...
"""
```

- **Palette** (`art/palette.toml`): about 32 named colours in ramps (`stone0`-`stone4`, `wood0`-`wood3`,
  `leaf0`-`leaf4`, `gold`, `water`, `red`, `cream`), plus `ink` (the outline) and `white`. An
  `[accent]` table defines `accent_dark` and `accent_light` as mixes of the nation colour toward
  `ink` and `white`. A shared `[legend]` gives every sheet the same keys for the UI: `.` clear,
  `o` ink, `i` inset, `p` panel, `s` accent_soft, `D`/`a`/`L` the accent ramp.
- **Digits are ramps.** A sprite binds `1`-`5` to one ramp, darkest first. Grids stay readable
  ("light top-left, dark bottom-right" is visible in the text), and recolouring a sprite is a
  one-line legend change.
- **Outlines are explicit or automatic.** Map sprites and icons draw fills only and set
  `outline = true`. The frame and medallions draw their outlines by hand because the outline is
  part of the shape.
- **A sprite can pin a nation** (`nation = "fire"`): the emblems sheet renders each emblem in its
  own nation's ramp.
- **Sheets, not loose files.** One PNG per sheet keeps the manifest short. The client names cells
  by `(column, row)` constants in `sprites.rs`, as it does for the pack. A client test reads the
  TOML and checks each constant points at the sprite name it claims.

Why not keep the art as code (like `ui_style.rs` draws Seal today)? Text grids are something an
agent can diff and review a pixel at a time, and the owner can edit them in any editor. Shapes in
code stop being reviewable past a rectangle.

## Render pipeline

| Step | Where | What |
|---|---|---|
| Author | `art/*.toml` | Edit the grid; the palette is the only colour source |
| Render | `cargo xtask art` | Writes `assets/original/<sheet>[.<nation>].png` and adds or updates the `[[file]]` entries (`source = "original"`, `generated_from`, sha256) |
| Check | `cargo xtask assets` (in `cargo xtask ci`) | For each `generated_from` file, re-renders in memory and compares pixels: a mismatch is "stale: run cargo xtask art". Every pixel of an `original` PNG must be a palette colour |
| Load | client | `ImagePlugin::default_nearest()` (already set), UI art at 2x, map art at 1:1 |

- **Rust in xtask, with the `png` crate.** I tried it hands-on: a 16x16 grid becomes a 266-byte
  PNG in under a millisecond, byte-identical across runs. `png` 0.18 is already in `Cargo.lock`
  through Bevy, so it adds no new download; `fn_ui` supplies the theme maths. The xtask stays in
  the `sim` CI scope with no Bevy.
- **Comparing pixels, not bytes,** keeps a future `png` upgrade from failing CI on a re-encode.
- **The PNGs are committed.** The client loads plain PNGs, and anyone can open them. The source
  stays the truth, and CI makes sure the two agree.
- `assets/original/` gets one `[[source]]`: `id = "original"`, author "Claude (agents) for the
  owner", `license = "Original work, owner's"`, `release_ok = true`.

## Style rules

- **Grid.** 16px cells. Map sprites are drawn at 1:1 and 2x2 sprites are bottom-aligned, as the
  pack does. UI art is drawn at **2x only**: one UI pixel is always 2 screen pixels, so no two
  pixel sizes meet in a panel. A 16px icon is 32px on screen.
- **Outline.** 1px `ink` (`#161821`), close to the pack's near-black, on every map sprite and
  icon. Inside a shape, edges use the ramp's darkest tone (selective outline) rather than more
  `ink`.
- **Light.** From the top left. Ramps go from 3 tones for small things to 5 for big ones. No
  anti-aliasing, no dithering at 16px, no pure black or pure white fills.
- **Nation colour.** Only UI flair and emblems use the accent ramp. Map sprites and icons stay
  neutral, so a stone looks the same in every nation and status colours keep their meaning.
- **Seal.** Keep Seal's layout, tokens and flat panel body. Only the flair becomes art: the
  frame is a 32x32 9-slice (14px corners: an outlined L bracket outside a 1px `accent_soft` line,
  set 4 art pixels out, which is 8 on screen against today's 6), the seal a 16px diamond, the section bullet a
  4px diamond. Text stays Bevy's default font.
- **Icons sit in wells.** An `inset` square behind each icon, so it reads on any panel.

## Scope and order

1. **#128 Tool:** `cargo xtask art`, the palette, the CI check; port the samples to `art/`.
2. **#129 UI kit:** the Seal 9-slice, seal, bullet and nation emblems in the client.
3. **#130 Icons:** Stores as a two-column icon grid; tool icons on the toolbar buttons.
4. **#131 Pack gaps:** cabbage, fruit tree, fish spot and inner cliff corners, drawn to the pack's
   conventions.
5. **Owner-gated, not filed yet:** a one-biome trial (plains ground, water autotiles, oak) next
   to the pack in a screenshot. If the owner likes it, terrain goes biome by biome, then units and
   facesets last. Units are the biggest job (12 characters x 4 directions x animation frames,
   plus 38x38 facesets), and they're where text authoring is weakest. #78 (canon characters)
   stays in the Ninja Adventure style until the owner decides.

## How a pack leaves the manifest

Removal is per file, never a big switch:

- The PR that removes the **last code reference** to a pack file deletes the file and its
  `[[file]]` entry in the same commit. `cargo xtask assets` already fails on a listed file that's
  missing and on an unlisted file that's present, so the two can't drift.
- When a source's last file goes, its `[[source]]` entry and `LICENSE.txt` go too, and
  `docs/design/art.md` gets a line saying what replaced it.
- The Seal flair isn't an asset (it's code in `ui_style.rs`), so #129 just deletes the builder
  code it replaces.

## Tools considered

The owner asked for editors, loaders and helpers to be checked. Network access here allowed
crates.io; GitHub release downloads of third-party repos and lospec.com were blocked by the proxy
(403), and I didn't work around that, so those editors weren't run here.

| Tool | What it is | Licence | Headless or agent use | Fit | Verdict |
|---|---|---|---|---|---|
| **Own xtask + `png` crate** | A few hundred lines of Rust: TOML grids to PNG | MIT/Apache (`png`) | Fully scripted; **tried**: deterministic, sub-millisecond | Already in `Cargo.lock`; lives in `cargo xtask`; checks run in CI | **Recommended** |
| Pillow | Python imaging | MIT-CMU | Used for this doc's prototype | Adds Python to the toolchain and CI | Mockups only |
| ImageMagick + XPM | XPM is a standard palette-indexed text image format | ImageMagick licence | **Tried**: ImageMagick 6.9 renders an XPM pixel-identical to the Rust output. Pillow's XPM reader crashed on the same file | No sheets, nation accents, ramps or auto-outline; a system dependency in the Nix devShell | No |
| Aseprite | The standard pixel editor; `aseprite -b` batch export and Lua scripting | Source-available EULA: you can build it for personal use, not redistribute (from memory; the repo was blocked here) | Batch export works headless, but `.aseprite` is binary, so an agent can only drive it through Lua scripts | Best for the owner hand-editing; agents can't diff the source | Alternative (question 4) |
| LibreSprite | GPL-2 fork of old Aseprite | GPL-2 | Has a CLI and scripting, but it's behind Aseprite | Same binary-source problem | No |
| Pixelorama | Godot-based editor | MIT | GUI first; I couldn't check its command-line export here | Own `.pxo` format | No |
| `asefile` crate | Reads `.aseprite` files in Rust | MIT | Fetched and compiled 0.3.8. It only reads, and it pulls `image` 0.24 next to the workspace's 0.25 | Only useful with Aseprite sources | Only with Aseprite |
| `bevy_aseprite_ultra` | Bevy plugin that loads `.aseprite` with slices and animation tags | MIT | 0.10.0 targets Bevy 0.20, our version | Would skip PNGs, but makes the client depend on a binary source format | Only with Aseprite |
| Lospec palettes | Curated 16-64 colour palettes | Free to use | Site blocked here | A palette is a list of hex values, so it drops straight into `palette.toml` | Optional (question 4) |
| Tiled, LDtk | Tile-map editors with autotile rules | GPL-2 (Tiled), MIT (LDtk) | Tiled can export from the command line | Our maps are generated, and autotiling is code in `sprites.rs`. Nothing for them to edit | No |
| Procedural sprite generators (e.g. mask-based "pixel-sprite-generator") | Random sprites from a mask and a seed | Mostly MIT | Scriptable | They make random creatures and ships, not a specific cabbage | No, for now |

**Why our own xtask.** It's the only option where the source is text an agent can write and
review, and where the check runs in the existing `cargo xtask ci`. Its only dependency is already
in the lockfile, so the PR stays small, with no new system tools for the Nix devShell. An editor
can be added later without changing the pipeline: an Aseprite export script can write our TOML.

## Open questions for the owner

1. **How far does the replacement go?** (a) UI only, (b) UI plus the pack's stand-ins and gaps,
   (c) everything, including terrain, units and portraits, replacing Ninja Adventure.
   **Recommendation: (b) now, and decide on (c) after a one-biome trial screenshot.** The UI is
   where hand-authored art clearly wins. Units are the most work and the weakest fit for text
   authoring. #78 stays in the pack style until this is decided.
2. **Style: follow Ninja Adventure's conventions, or a look of our own?** Following them means
   outlined, saturated, lit from the top left, in our palette, as in the samples. Our own look
   could be flatter, unoutlined or a limited palette. **Recommendation: follow the pack's
   conventions,** so original and pack art can share the screen while the replacement goes
   piece by piece.
3. **Seal: should the pixel 9-slice frame replace the code-drawn brackets and seal?** The other
   choice keeps the code flair and adds only the emblem and the icons. **Recommendation: replace
   it** (#129). The mockup shows it matching the map's sprites better, with the same layout and
   colours. The Stores section also becomes a two-column icon grid (#130).
4. **Tooling and palette.** (a) Our own text-to-PNG xtask with our own ~32-colour palette, (b) the
   same xtask with a Lospec palette (for example Endesga 32), or (c) Aseprite sources loaded with
   `bevy_aseprite_ultra`, which is better if you want to edit in Aseprite yourself.
   **Recommendation: (a).** It's the only one agents can author and review as text, and it adds
   no dependency. Switching to (b) later is a palette-file change.
