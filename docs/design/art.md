# Art direction

Status: **draft, waiting on the owner's pick** (#1). Three rendered options below. Once the owner
chooses, this doc records the decision (and vision.md links to it). Follow-up issues come after
that, and nothing is added to `assets/` before then.

## The problem

Today the map mixes two styles that don't belong together:

- **Kenney Tiny Town** (CC0, flat 16px pixel art): grass, broadleaf and pine trees.
- **The 2013 textures** (unknown artists, `release_ok = false`): water, coast, desert, snow,
  stone, gold and cabbage, which look photographic and noisy at 16px. The unit sprites are chibi
  fan-art of canon characters, about 32×30 px and drawn at 0.6 scale.

The result: water and mountains look like a different game from the grass and trees, and units
don't share a pixel scale with the tiles.

![Today, seed 7](https://github.com/gizmo385/four-nations/blob/2b41f02f7fed3698855af7c2f8986dbd0392c8b8/mockups/1/today-map.png?raw=true)

## Constraints every option meets

- **16px tiles stay.** Nothing below needs a different `TILE`. A 96×64 map is 1536×1024 px at
  1:1, and the camera zooms in from there.
- **One primary source**, so outlines, palette and lighting match. A second source only fills
  gaps, and only if it shares the primary's style.
- **Licence:** the game is personal-only (vision.md), so any licence works. CC0 is still
  preferred because it keeps `release_ok = true` and needs no attribution bookkeeping.
- **Coverage we need now:** 4 terrains plus rivers and lakes, 6 biomes (plains, forest, taiga,
  swamp, desert, tundra), 5 resources (oak, fir, stone, gold vein, cabbage), and units for 4
  nations × 3 variants, plus a portrait for the selection bar (#39). **Soon:** buildings
  (study halls, #21), crops, item and tech icons (#23), and bending effects (#6).
- **UI fit:** it has to work with the #39 panel styles (A Scroll, B Seal, C Parchment).

## How the mockups were made

Each option was rendered headless with `cargo xtask screenshot` on seed 7, Earth Kingdom. Every
option has one whole-map shot and one zoomed shot with a #39 UI style. The candidate sheets were
downloaded to `target/` only and staged under `target/art/stage`. The client points at them with
`FN_ART=<option>` and `FN_ASSET_DIR`. Code and screenshots are on the unmerged reference branch
`issue/1-art-mockups` (`crates/client/src/artset.rs`, `mockups/1/`). Tile picks are quick
approximations to judge the look, not final mappings.

## Option A: Ninja Adventure (outlined, East Asian, 16px). Recommended

![Ninja Adventure with the Scroll UI](https://github.com/gizmo385/four-nations/blob/2b41f02f7fed3698855af7c2f8986dbd0392c8b8/mockups/1/ninja-ui.png?raw=true)
![Ninja Adventure, whole map](https://github.com/gizmo385/four-nations/blob/2b41f02f7fed3698855af7c2f8986dbd0392c8b8/mockups/1/ninja-map.png?raw=true)

- **Style:** warm, saturated 16px pixel art with dark outlines, in the style of a SNES/GBA
  action RPG. The pack's setting is a stylised feudal East Asia (ninja, samurai, pagodas,
  torii, elemental icons for fire, water, wind and earth), which is the closest off-the-shelf
  match to the Avatar world we found.
- **Source:** [Ninja Adventure Asset Pack](https://pixel-boy.itch.io/ninja-adventure-asset-pack)
  by Pixel-Boy and AAA. **CC0**. Actively updated (update #8 landed in March 2026). The full zip
  (89 MB) lists 50+ characters with animations **and facesets** (portraits), 30+ monsters,
  autotiling floor tilesets, exterior and interior elements, 60+ items, UI elements, 2 fonts,
  100+ sound effects and 37 music tracks. itch only serves the full zip through a browser
  download ("name your own price", $0 is allowed). The mockup uses the subset in the author's
  Godot repo, [pixel-boy/NinjaAdventure](https://github.com/pixel-boy/NinjaAdventure).
- **Coverage:**
  - **Seen in the subset:** two grass shades, sand, snow, mud and ice floors with autotile
    edges; broadleaf trees (2×2 tiles); bushes; stones; ninja and samurai characters with
    4-direction walk cycles.
  - **Listed but not yet checked:** water (the mockup uses tinted ice), conifers, cliffs for
    mountains, ore, crops, and monk or villager characters to dress each nation.
  - The **first follow-up audits coverage** against the list above.
  - Air and Fire units in the mockup are hue-shifted ninjas; the full pack's 50+ characters
    should give each nation its own outfit.
- **Today's mix:** Tiny Town and the 2013 textures are both replaced, and the 2013 fan-art units
  go too. Kenney stays only as a fallback for gaps, though the style clash (Kenney has no
  outlines) makes that a last resort.
- **Design notes:**
  - The 2×2 trees read as a dense canopy, which looks great but can hide units in forests. A
    follow-up can fade canopies near the cursor or the selection.
  - The facesets fill the #39 selection-bar portrait at native resolution.
- **UI fit:** best with **A Scroll**: the warm wood and dark ink match its palette and outlines.
  B Seal works. C Parchment is too close to its sand tones.

## Option B: the whole Kenney family (flat, clean, 16px). Least change

![Kenney family with the Scroll UI](https://github.com/gizmo385/four-nations/blob/2b41f02f7fed3698855af7c2f8986dbd0392c8b8/mockups/1/kenney-ui.png?raw=true)
![Kenney family, whole map](https://github.com/gizmo385/four-nations/blob/2b41f02f7fed3698855af7c2f8986dbd0392c8b8/mockups/1/kenney-map.png?raw=true)

- **Style:** soft, flat colours without outlines: what Tiny Town already looks like, applied to
  everything.
- **Sources:** all from [Kenney](https://kenney.nl), all **CC0** with direct downloads. Tiny
  Town is already in the repo. [Roguelike RPG Pack](https://kenney.nl/assets/roguelike-rpg-pack)
  (about 1,700 tiles) covers water, sand, stone, dirt, trees, cacti, rocks and gold.
  [Roguelike Characters](https://kenney.nl/assets/roguelike-characters) provides paper-doll
  layers (bodies, shirts, trousers, hair, hats).
- **Coverage:**
  - **Good:** water, coast, plains, forest, desert, mountains, all five resources.
  - **Weak:** tundra. There's no snow tile, so the mockup uses a plain white panel tile and
    tinted pines, and swamp and taiga are tinted grass.
  - **Units:** composed from layers, with one outfit colour per nation (Air: bald, cream and
    orange; Water: teal; Earth: green; Fire: dark and red). They're generic western-RPG figures,
    and portraits would be upscaled 16px sprites.
  - **Missing:** anything East Asian (architecture, clothing).
  - Tiny Town grass has a faint diagonal dotted pattern that shows as stripes at map scale.
- **Today's mix:** Tiny Town stays. Every 2013 texture and unit sprite is replaced.
- **UI fit:** neutral; works with **A Scroll** or **B Seal**. Of the three options, this one has
  the least Avatar flavour.

## Option C: 1-bit glyphs (minimal, tinted, 16px)

![1-bit with the Seal UI](https://github.com/gizmo385/four-nations/blob/2b41f02f7fed3698855af7c2f8986dbd0392c8b8/mockups/1/onebit-ui.png?raw=true)
![1-bit, whole map](https://github.com/gizmo385/four-nations/blob/2b41f02f7fed3698855af7c2f8986dbd0392c8b8/mockups/1/onebit-map.png?raw=true)

- **Style:** single-colour glyphs on dark ground, in the spirit of Dwarf Fortress tilesets or
  Caves of Qud. Colour carries the meaning: each biome, resource and nation gets a tint.
- **Source:** [Kenney 1-Bit Pack](https://kenney.nl/assets/1-bit-pack) (1,078 tiles), **CC0**.
  The glyphs are white, and every tint comes from code or data (a `color` per biome in
  `data/biomes.toml` would fit the existing `glyph` field there).
- **Coverage:**
  - **The most complete of the three:** trees, rocks, mountains, crops, ore, people, buildings,
    tools, items and UI icons are all present, in one consistent style.
  - **Weak spots:** Avatar flavour comes only through colour and the UI. Units differ only by
    glyph and tint, and a nation colour can disappear against similar ground (Earth's green
    units on grass, shown). The mockup lightens unit tints; a final version would want an
    outline or a neutral body with an accent marker.
- **Today's mix:** everything is replaced.
- **Design notes:**
  - It suits pillar 4 (readable simulation) and mirrors the sim's ASCII renderer.
  - It's the cheapest to extend: a new building or item is usually just a glyph and a colour.
- **UI fit:** best with **B Seal** (crisp, dark, square). Scroll's wooden rods feel heavy over it.

## Also considered (not rendered)

- **LPC (Liberated Pixel Cup), 32px.** CC-BY-SA 3.0 / GPL 3.0, with a large
  character-generator ecosystem. Rejected for now: it's western-medieval, the base terrain has no
  sand or snow, and moving to 32px tiles quadruples texture memory and the effort for every new
  sprite.
- **Original or commissioned Avatar art.** The best possible fit, but a big time and cost
  commitment. Any option above can be replaced piece by piece later.
- **Keeping the canon fan-art units** (the 2013 chibi Aang, Katara and others). It's allowed
  (personal project), but they clash with every option's pixel scale. If the owner wants canon
  characters, the better route is named special sprites drawn in the chosen style.

## Comparison

| | A Ninja Adventure | B Kenney family | C 1-bit |
|---|---|---|---|
| Avatar feel | **Strong** (East Asian setting, elemental icons) | Weak | Through colour only |
| Coherence | One artist, one palette | Same studio; snow is improvised | Perfect |
| Coverage today | Likely good; needs an audit of the full pack | Good except tundra | Complete |
| Units | 50+ characters with portraits | Paper-doll recolours | Tinted glyphs |
| Licence | CC0 | CC0 | CC0 |
| Getting it | Browser download from itch (or the GitHub subset) | Direct download | Direct download |
| Best #39 UI | A Scroll | A Scroll or B Seal | B Seal |

## Recommendation

**Option A, Ninja Adventure, at 16px, with the Scroll UI.** It's the only option that looks like
the Avatar world without original art. It's CC0, so nothing needs replacing later. It's one
artist's work, so it stays coherent as buildings, crops and icons arrive. Its facesets and
elemental icons cover needs we already have (#39's portrait, #23's tech icons). The risk is
coverage we haven't seen yet (water, conifers, mountains, desert). The first follow-up checks
that, and B remains the fallback if the audit finds big gaps.

## Follow-up issues (after the owner picks)

Not filed yet. For option A they would be:

1. Import the pack: record it in `assets/manifest.toml` per docs/process/assets.md, and audit
   its coverage against the list above.
2. Replace the terrain and resource sprites, and retire the 2013 textures.
3. Replace the units with per-nation characters, and wire the facesets into the selection bar.
4. Forest canopies: fade near the cursor and the selection (only if the 2×2 trees stay).

Autotiling (#8) and the river/lake art in #31 would then target this pack.

## Open questions for the owner

1. **Which direction: A (Ninja Adventure), B (Kenney family) or C (1-bit)?**
   *Recommendation: A.*
2. **Can you download the full Ninja Adventure zip once** from itch.io (browser only, $0 is
   fine) and drop it somewhere an agent can read? Agents can't complete itch's download flow.
   Without it we're limited to the GitHub subset. *Recommendation: yes. One download unlocks the
   whole pack, and the agent then records it in the manifest.*
3. **Units: generic pack characters dressed per nation, or canon characters?**
   *Recommendation: generic characters with nation colours now; canon characters later only as
   special sprites in the same style.*
