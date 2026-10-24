# Art direction

Status: **decided** (#1, owner, 2026-10-22). The decision and the coverage audit come first. The
options that were considered follow for the record.

## Decision

- **Ninja Adventure** (Pixel-Boy and AAA, **CC0**) is the single primary art source, at **16px
  tiles**. `TILE` doesn't change.
- It **replaces both** Kenney Tiny Town and the 2013 textures. The 2013 fan-art units go too, so
  `thefournations-2013` source (`release_ok = false`) has left the manifest (#75, #76).
- **Units:** generic pack characters, three per nation (table below). Canon characters come later,
  as special sprites in the same style (#78).
- **UI:** used with #39's **Seal** panels (owner's pick, `docs/design/ui.md`). Cool, opaque
  slate panels separate cleanly from the pack's warm, saturated map. The brackets and seal get a
  1px dark outline to match the pack's outlined sprites, and facesets and icons are drawn at
  integer scale. Details are in ui.md under "Fitting Seal to Ninja Adventure".
- **Follow-ups:**
  - #74: import the pack and record it in the manifest (done).
  - #75: terrain and resources (done).
  - #76: units and portraits (done).
  - #77: fade forest canopies (p3, blocked by #75).
  - #78: canon characters (p3, blocked by #76).
- **Superseded (#122, owner, 2026-10-24):** Claude-authored pixel art, written as text and
  rendered by `cargo xtask art`, replaces this whole pack, one family per PR (#144 to #155). See
  [pixel-art.md](pixel-art.md). This doc stays as the record of the pack and its audit.

![Full pack, Water Tribes, Seal UI](https://github.com/gizmo385/four-nations/blob/92ea0705fc25b2dc46ec2b8c1fdfd848d95ef781/mockups/1/ninja-seal-ui.png?raw=true)
![Full pack, whole map](https://github.com/gizmo385/four-nations/blob/d4a70353b8cb52e2364f9b32a75b2a82751d1f2c/mockups/1/ninja-full-map.png?raw=true)

These two renders use the owner's full download, the first with the chosen Seal UI. They show
real water, conifers, grey boulders on mountains, and the Water Tribes cast
with an `Eskimo` faceset as the portrait. Water/coast edges are now bitmask-autotiled (#8), and
mountains are autotiled cliffs (#104).

## Coverage audit (full pack)

The audit was done on the owner's download, `Ninja Adventure - Asset Pack.zip` (2,674 entries,
1,915 PNGs, sha256 `95a06f4f…7b15`), unzipped under `target/art/` only.

| Need | In the pack | Verdict |
|---|---|---|
| Sea, rivers, lakes | `TilesetWater`: autotiled water on sand, grass and snow; ice; purple bog water (unused, #109); bridges, dock, boat. Animated `Water Ripples` and `Waterfall` | **Covered** |
| Plains, forest | `TilesetFloor`: light and dark grass with tuft variants | **Covered** |
| Taiga | Dark grass plus `TilesetNature` conifers and snowy conifers | **Covered** |
| Swamp | `TilesetFloor` mud, a rooted tree; its water is the blue grass-edged set (#109) | **Covered** |
| Desert | `TilesetFloor` sand; `TilesetDesert` palms, oasis water, desert houses | **Covered**, but no cactus (palms and dry shrubs stand in) |
| Tundra | `TilesetFloor` snow and ice, snowy trees, snow boulders, igloos in `TilesetHouse` | **Covered** |
| Mountains | No single mountain tile. `TilesetRelief` has cliff faces and tops, and `TilesetNature` has grey and brown boulders | **Covered** (#104): `TilesetRelief` plateau pieces for the rim, boulders on the interior. No inner-corner pieces |
| Oak, fir | `TilesetNature`: broadleaf, conifer, snowy conifer, cherry blossom, dead trees. All trees are **2×2** | **Covered** (2×2 sizing is why #77 exists) |
| Stone, gold vein | `TilesetNature` grey rocks and gem- or ore-studded rocks | **Covered** |
| Cabbage | No cabbage. Leafy plants in `TilesetNature`, sprouts and vegetables in `TilesetFloorDetail`, farmland in `TilesetField` | **Stand-in:** leafy plant |
| Units | 95 characters, each with a 4-direction sprite sheet and a 38×38 `Faceset.png` | **Covered.** Air is the thinnest (only `Monk2` is a bald monk) |
| Later | `TilesetHouse` (dojo, torii, pagoda roofs, tents, igloos, statues), `MillPropeller` and `WaterMill` animations (Air wind power), 60+ items with elemental FX, UI, 2 fonts, music and SFX | Good for #21, #22, #23 and #6 |

**Per-nation cast** (#76; first frame shown in the render above):

| Nation | Characters |
|---|---|
| Air Nomads | `Monk2`, `Master`, `NinjaYellow` |
| Water Tribes | `Eskimo`, `NinjaEskimo`, `NinjaWater` |
| Earth Kingdom | `Villager3`, `Samurai`, `NinjaGreen` |
| Fire Nation | `NinjaFire`, `SamuraiRed`, `NinjaRed` |

The cast lives in `data/nations.toml` as a `characters` list (rule 7: nations differ through
data). Units pick one by their existing `variant` (`NationDef::character`); the sim only checks
the list is non-empty. `sprites/characters.rs` turns a character id into its sheet and faceset paths
(`SamuraiRed`'s sheet is `redsamurai.png`) and names the frame drawn: the down-facing idle cell
(0,0), at 1:1. The selection panel shows the unit's faceset at 2x. Units, item markers and
resources share one depth band by row (`row_depth` in `main.rs`), so a unit passes behind the
canopy of a tree in the row below it and in front of one in its own row or above.

**Map mapping** (#75, implemented). Coordinates are (column, row) in 16px cells of each sheet.
The table lives in one place, `crates/client/src/sprites/` (`GROUND` and `RESOURCES`, one module per family); a new
biome or resource is one new row there, and its tests fail until every biome has one. Where the
cells differ from the first audit, the shipped cell is shown:

| Map element | Sprite |
|---|---|
| Plains floor | `TilesetFloor` light grass (0,12) with tufts (1-3,12) |
| Forest, taiga floor | dark grass (11,12) with tufts (12-13,12) |
| Desert, coast, ford | sand (1,1); desert has twig and pebble variants (0-1,4); tundra coast and ford use snow |
| Tundra | snow (1,15), flat: the (1-2,19) variants are lavender and read as boxes |
| Swamp | mud (12,15) with variants (13-14,19); water is the blue grass-edged set, not the purple bog block (#109) |
| Water, river, lake | bitmask-autotiled (#8, below) against which cardinal neighbors are dry land; interior piece `TilesetWater` (1,1) |
| Mountain | cliff-autotiled (#104, below). Interior: the plateau's flat top, `TilesetRelief` (2,6), plus a `TilesetNature` grey rock (18,9) on about two in three tiles with no resource; tundra: the snow top (2,1) and a snow-capped rock (4,12) |
| Oak / fir / tundra fir | `TilesetNature` 2x2 at (0,0) / (2,0) / (8,0), bottom-aligned to the tile |
| Stone / gold vein | `TilesetNature` plain grey rock (6,14) / the same rock with orange ore (4,14) |
| Cabbage | `TilesetNature` leafy head (8,11), a stand-in |
| Fruit tree (#56) | `TilesetNature` 2x2 cherry blossom (14,0), a stand-in: no tree in the pack carries fruit |
| Fish spot (#56) | `TilesetWater` fish shadows (11,4), a full cyan tile  |

**Water autotiling** (#8, implemented). `TilesetWater` turns out to carry its own "pond" demo:
four 4x4-cell blocks, one per land colour it blends into (sand, snow, grass, and purple bog
water on mud), each laid out the same way: a 3x3 of corner/edge/interior pieces, a
1-wide strait (for rivers, which have no separate shore tile) in the 4th column, and its closed
ends in the 4th row. `sprites::WaterSet` reads one block and `water_cell` picks a piece per tile
by which of its four cardinal neighbors are dry land (`Terrain::is_water`, so a shallow ford still
reads as water to its neighbors); `GroundRule::water` matches a biome to its block: desert→sand,
tundra→snow, everything else→grass, swamps included. The purple bog block went unused once
swamp lakes turned out pink in play (#109); it waits for a real bog terrain. A tile boxed in by land on all four sides (rare
on the mapgen's rounded water bodies) falls back to the open-water interior rather than guess,
since the pack's "isolated puddle" piece isn't present for every block. Shore (`Coast`/`Ford`)
tiles stay flat; only the water side of the boundary carries the transition art.

**Mountain cliffs** (#104, implemented). `TilesetRelief` is two raised-plateau sets (a grey-faced
cliff with a snow top at rows 0-2, a brown-faced one with a sand top at rows 5-7), each a 3x3 block
of corner/edge/top pieces plus a 1-wide ridge column at its left; the rest of the sheet (the tall
wall blocks, ice bridges and stairs) is unused. Unlike the water pieces, every cliff piece is still
one 16px tile: the face is painted into the lower two thirds of the south pieces, so it does not
overlap the row below and needs no extra depth banding (trees, resources and units in the row
below already draw over the floor layer). `sprites::CliffSet` reads one set and `cliff_cell` picks
a piece by which cardinal neighbors are lower ground (any non-mountain tile; off-map does not
count); `GroundRule::cliff` matches tundra→snow, everything else→sand. A mountain with no lower
neighbor is the set's flat top, and only that interior tile carries boulders. The top colours
differ from the old dirt floor, so the interior now uses the plateau top instead, or the rim would
show a seam. Gaps: no inner-corner pieces (a tile whose only lower neighbor is diagonal reads as
interior), no end caps for a horizontal ridge, and a ridge one tile tall drops its north rim.

## The problem (before the decision)

The map mixed two styles that don't belong together:

- **Kenney Tiny Town** (CC0, flat 16px pixel art): grass, broadleaf and pine trees.
- **The 2013 textures** (unknown artists, `release_ok = false`): water, coast, desert, snow,
  stone, gold and cabbage, which look photographic and noisy at 16px. The unit sprites are chibi
  fan-art of canon characters, about 32×30 px and drawn at 0.6 scale.

The result: water and mountains look like a different game from the grass and trees, and units
don't share a pixel scale with the tiles.

![Today, seed 7](https://github.com/gizmo385/four-nations/blob/2b41f02f7fed3698855af7c2f8986dbd0392c8b8/mockups/1/today-map.png?raw=true)

## Constraints every option met

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

## Option A: Ninja Adventure (outlined, East Asian, 16px). Chosen

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
- **Coverage:** these first renders used only the GitHub subset (tinted ice for water,
  hue-shifted ninjas for Air and Fire). The full-pack audit above replaces those caveats.
- **Today's mix:** Tiny Town and the 2013 textures are both replaced, and the 2013 fan-art units
  go too. Kenney stays only as a fallback for gaps, though the style clash (Kenney has no
  outlines) makes that a last resort.
- **Design notes:**
  - The 2×2 trees read as a dense canopy, which looks great but can hide units in forests. A
    follow-up can fade canopies near the cursor or the selection.
  - The facesets fill the #39 selection-bar portrait at native resolution.
- **UI fit (as proposed):** best with A Scroll, with B Seal workable. The owner then chose
  **Seal** for #39, and it fits well (see Decision). C Parchment is too close to its sand tones.

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
| Coverage | Good (full pack audited after the pick) | Good except tundra | Complete |
| Units | 50+ characters with portraits | Paper-doll recolours | Tinted glyphs |
| Licence | CC0 | CC0 | CC0 |
| Getting it | Browser download from itch (or the GitHub subset) | Direct download | Direct download |
| Best #39 UI | A Scroll | A Scroll or B Seal | B Seal |

## Recommendation (as proposed)

**Option A, at 16px, with the Scroll UI.** It was the only option that looks like the Avatar
world without original art. It's CC0, it's one artist's work, so it stays coherent as buildings
and icons arrive, and its facesets and elemental icons cover #39 and #23. The owner agreed, and
the audit above confirmed the coverage.

## Owner answers (2026-10-22)

1. Direction: **A, Ninja Adventure**.
2. The full zip was downloaded to `/Users/chris/workspace/Ninja Adventure - Asset Pack.zip`,
   outside the repo. #74 imports what we use.
3. Units: **generic pack characters dressed per nation** for now. Canon characters come later
   (#78).
