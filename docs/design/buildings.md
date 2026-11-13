# Buildings: multi-tile footprints

Doc status: current

Status: **decided** (#196; owner decisions of 2026-10-25 are listed at the end). Follow-up work
is listed at the end of this doc. Units go *inside* buildings through a door, on top of the solid
footprints ([Units inside buildings](#units-inside-buildings)), and the study halls get taller
art ([Larger art](#larger-art)). Which buildings each nation has, and what they cost, is in
[nation-buildings.md](nation-buildings.md) (#357).

Every building is 1x1 in the rules today. `BuildingDef` has no size, the sim stores one
`Building` per tile (`World::building_at`), and `passable` covers that one tile. The art is
32x32, two tiles by two (#149, #175 to #178), drawn bottom-centred on that one tile like a tree.
So a study hall or a forge looks 2x2 but uses only 1x1:

- units walk through the visible body of a building (or stand in its wall);
- two buildings on neighbouring tiles overlap;
- the build-site tint marks one tile under a 2x2 sprite.

(A before/after render lived at `mockups/196/`, removed with the client in #281 and only in git
history now: it showed a hall's build site with its one-tile tint under the 2x2 ghost sprite.)

## Decisions in brief

1. **A footprint is a rectangle in data.** `footprint = [w, h]` on a building in
   `data/buildings.toml`, default `[1, 1]`. Each side is 1 to 4 tiles. There are no masks or
   L-shapes, because all the art is rectangular.
2. **The anchor is the top-left tile.** `Command::DesignateBuild { pos }` names the anchor, and
   the footprint covers `pos.x .. pos.x + w` and `pos.y .. pos.y + h`. The frontends turn the
   cursor tile into the anchor, so the sim never needs to know where the pointer was.
3. **No rotation.** Every sprite faces the camera with its door at the bottom, so a rotated
   footprint would have no art to draw. Footprints are rectangles, so a 2x1 can be "rotated" by
   defining a 1x2 building if one is ever needed. (The owner agreed, 2026-10-25.)
4. **Every footprint tile carries the building.** The `Building` already baked onto
   `Tile::building` gains `part` (its offset from the anchor) and `size`. `building_at`,
   `passable`, pathfinding, regions and the bending checks keep working tile by tile and need no
   changes. The size is baked too, so a building keeps the size it was built with even if the
   data changes later. That is what makes old saves safe.
5. **Use from the edge, then go in.** A solid building is built and used from the ring around
   its footprint, and a walkable one from inside its footprint. A building with a capacity (a
   study hall) is then *entered* through its door tile: units step inside, leave the map and
   are listed in its panel ([Units inside buildings](#units-inside-buildings), a second stage
   on top of this one).
6. **2x2 for everything drawn 2x2.** The existing 32x32 art already *is* a 2x2 footprint
   (16 px tiles). Sprites whose opaque base is about 28 px wide become 2x2, and the 16 px ones
   stay 1x1. The current art works on the new footprints as it is. The four study halls then
   get taller 32x48 art ([Larger art](#larger-art)).
7. **Saves need no version bump.** The new fields use `serde(default)` and are skipped when they
   are 1x1, so a v1 save loads with every building as the 1x1 it was built as, and a world of
   1x1 buildings writes the same text as before.

## Footprint sizes

The owner agreed this table on 2026-10-25. It follows the art: the width of each sprite's opaque
base, measured from the `assets/original/` PNGs. Then role and cost decide the cases the art
leaves open.

| Building | Art base (px) | Footprint | Walkable | Why |
|---|---|---|---|---|
| bed | 16 | 1x1 | yes | One sleeper per bed, lies on its tile |
| campfire | 10 | 1x1 | yes | Cheap, placed often, near beds |
| earth_wall | 16 | 1x1 | no | Built in runs. The art fills its tile edge to edge (#175) |
| wind_turbine | 16 (blades 18) | 1x1 | no | A mast. The blades may overhang upward like a tree canopy |
| igloo | 30 x 13 | 1x1 | yes | A bed. The art is narrowed to fit its tile (#213) |
| study hall (all four) | 26 to 30 | **2x2** | **no** (was yes) | The art's footprint. Scholars study from the door side |
| orchard | 24 to 28 | **2x2** | yes | A planted plot. People walk among the trees and later tend them (#142) |
| windmill | 22 | **2x2** | no | The sails need the width, and 1x1 would overlap its neighbours |
| bison_stable | 30 | **2x2** | no | 3x3 would suit a sky bison, but stays 2x2 for now (owner, 2026-10-25) |
| forge, brick_kiln, smelter, steam_engine | 26 to 28 | **2x2** | no | The art's footprint |
| lightning_station | 26 | **2x2** | no | Same plinth as the others |

Costs and work times don't change. A 2x2 hall at 10 wood is cheap for its area, but balance is
out of scope here (#124 and #142 give these buildings a purpose and will retune them).

**Halls become solid.** Today the study hall is `passable = true` so that scholars can stand on
its tile. With a 2x2 body, walking through it is exactly the bug this design fixes, so halls
become `passable = false` and their scholars stand on the ring (see below). The pavilion is
open-sided, but it is still a raised platform. Scholars then study *inside* the hall, entering by
its door ([Units inside buildings](#units-inside-buildings)).

## Sim rules

### Data and types

```toml
[[building]]
id = "forge"
footprint = [2, 2]   # [width, height] in tiles; default [1, 1]
```

- `BuildingDef::footprint: (u8, u8)`. Load fails if a side is 0 or more than 4, or if a building
  with a `[building.bed]` isn't 1x1 (one sleeper per bed, lying on its tile). Bigger beds, such
  as a bunkhouse, would be a new design.
- `geom::Footprint { anchor: Pos, w: u8, h: u8 }` holds the geometry that everything shares:
  `tiles()` (row-major), `contains(p)`, `ring()` (the tiles around it, row-major, corners
  included, matching `NEIGHBORS` today) and `distance(p)` (Chebyshev distance to the rectangle,
  0 inside).
- `Building { id, passable, part: [u8; 2], size: [u8; 2] }`. Each footprint tile gets a copy
  with its own `part`, and the anchor is the tile whose `part` is `[0, 0]`.
- `Site` (keyed by anchor in `World::sites`) gains `size`. There are few sites at any time (tens),
  so the site covering a tile is found by scanning them, with no second index to keep in sync.
- Queries: `World::footprint_at(pos)` returns the finished building's or the site's footprint
  covering `pos`. `World::buildings()` yields each finished building once, as
  `(Footprint, BuildingId)` in anchor order, for the frontends. `build_sites()` yields a
  `Footprint` instead of a `Pos`.

### Placement

`World::check_build(anchor, building, defs) -> Result<(), BuildError>` is a pure function that
`designate_build` uses and the frontends call for the preview. It checks the tech first (as
today), then every footprint tile with `World::build_blocker(pos) -> Option<Blocker>`:

- **OffMap**: any tile is outside the map.
- **Terrain**: the tile isn't `Land` or `Coast`. That rules out water, mountains and **fords**.
  Mapgen places fords to keep landmasses connected (#30), so a solid building must never close
  one. A walkable bed on a ford is harmless, but one rule for every building is easier to
  explain. This is a small change for 1x1 buildings too.
- **Resource**: a tree, rock, bush and so on.
- **Building** or **Site**: no overlap with anything finished or designated.
- **Stockpile**: a stockpile zone tile. Today nothing stops a wall from going up on a stockpile
  tile and burying what's stored there.

A unit standing in the footprint does not block placement. `BuildError::CannotBuild` gains the
first blocked tile and the reason (`CannotBuild { at, why }`), so the UI can say "a tree is in
the way". `Command::DesignateBuild` is all-or-nothing: either the whole footprint is designated,
or nothing changes.

### Movement and where units stand

- **Blocking.** Every tile of a solid footprint blocks movement through `MoveRules::tile_cost`,
  which already reads the baked `passable`. A walkable footprint blocks nothing.
- **No door or entry tile.** A unit uses a building from the nearest tile it can reach (by flood
  cost, ties by `Pos`, as `World::stand` breaks them today). For a walkable building that is a
  footprint tile. For a solid one it is a ring tile, and this applies to the *site* too, so a
  builder never stands inside something that is about to become solid. A door would cost a data
  field, a per-building validity rule ("the door tile must stay free") and a failure mode (a
  blocked door) for very little gain at 2x2. The art's door is cosmetic. If production buildings
  (#124) later need a work spot, it can be added as data then.
- **Completion clears the footprint.** When a solid building finishes, each unit on its footprint
  moves to the nearest walkable tile outside it (Chebyshev rings, row-major, as `place_ground`
  searches) and drops its route, and ground stacks there move the same way through
  `place_ground`. Then `map_changed()` runs once. This also fixes a latent 1x1 bug: today the
  builder of an earth wall finishes it while standing on it.
- **Reachability isn't checked at placement.** As today, a site nobody can reach is abandoned
  by its builder and stays designated until it is cancelled.

### Sleeping, heat and study

- **Beds** stay 1x1 (enforced at load), so `bed_route` and `sleep_rate` don't change.
- **Heat.** `HeatSource` gains `size` (`serde(default)`, skipped when 1x1). `warms(p)` becomes
  `footprint.distance(p) <= radius`, so a 2x2 heat building warms `radius` tiles out from every
  side. `warm_sleep_route` scans the footprint grown by the radius. No building with `heat` is
  bigger than 1x1 today. This only keeps a future warm forge (#124) correct.
- **Study.** `JobKind::Study { pos }` names the anchor. `run_study` walks to the footprint's
  stand tile (the ring, now that halls are solid). Two scholars may share a ring tile, just as
  they share the hall's tile today.

### Cancel and deconstruct

- `Command::CancelBuild { pos }` accepts any tile of a site's footprint and cancels the whole
  site. Delivered materials drop near the anchor, as today.
- `Command::Deconstruct { pos }` (#342, `world/deconstruct.rs`) marks a finished building for
  taking down, and `CancelDeconstruct` takes the mark back. A footprint is atomic: any tile
  names the building, and the mark is one job on its anchor. A unit works it for
  `[deconstruct] work` times the building's own `work`; then `World::remove_building` clears the
  whole footprint in one tick, along with everything the building owned (seats, craft jobs, a
  plot's jobs, its fire, units inside, anyone bound for its bed). `[deconstruct] refund` of each
  cost item (half, rounded down) drops by the anchor, plus a production building's delivered
  batch inputs in full. The endeavor's gate can't be taken down. In the TUI it is the Remove
  tool (`X`, #953, docs/design/tool-pickers.md), which takes away everything the player made in
  its rectangle: marks, sites, stockpile tiles and finished buildings.

### Saves and determinism

- **No `SAVE_VERSION` bump.** `part`, `size` (on `Building`, `Site` and `HeatSource`) are
  `#[serde(default, skip_serializing_if = ...)]` with 1x1 defaults. A v1 save reads as before,
  and every building and site in it is the 1x1 it was built as, even where the data now says
  2x2. They are "grandfathered" and stay drawn and used as 1x1 until they are deconstructed. The
  sim never re-derives a placed building's size from the data (decision 4).
- `Defs::id_hash` is unaffected: a footprint is a number, not an id.
- A **v1 fixture** (a small saved world with a bed, a hall and a site, checked in before the
  format changes) gets a test showing it still loads and plays after the change.
- **Determinism.** No RNG is involved. Footprint tiles, rings and searches are walked row-major,
  and every choice breaks ties on `(cost, Pos)` as today. The golden snapshots are mapgen-only,
  so they don't change. Replaying a command log recorded before footprints, against footprint
  data, gives a different game, but replay already requires identical data (save.rs).

## Frontends

### Placing (shared in `fn_ui`)

- **Cursor to anchor.** `fn_ui::build::anchor_for(cursor, footprint)` returns
  `cursor - ((w - 1) / 2, (h - 1) / 2)`. Odd sizes centre on the cursor, and even sizes put the
  cursor in the top-left of the middle. For a 2x2 the ghost grows down and to the right from the
  pointer, so the pointer doesn't hide it.
- **Ghost.** While the Build tool is active, the hovered placement shows the building's sprite
  at 60% opacity. Each footprint tile is tinted green if `build_blocker` is `None` and red if
  not, and a red-tinted ghost means the click will be refused. The status line gives the first
  reason ("can't build here: a tree"). `fn_ui` exposes a
  `placement(session, cursor) -> Placement { footprint, tiles: Vec<(Pos, Option<Blocker>)> }`
  that both frontends draw.
- **Drag.** A Build drag stamps footprints on a lattice from the drag's start, stepping by
  `(w, h)`. Every stamp that fits inside the dragged rectangle is tried on its own, so a drag
  along a row lays halls side by side and skips the blocked ones ("2 done, 1 skipped (a
  tree)"). 1x1 drags (earth walls) behave as today.
- **Picker.** Each row shows the size after the cost ("10 Wood, 2x2"), but only for buildings
  bigger than 1x1.

### Drawing (historical: `crates/client/src/buildings.rs`, removed with the Bevy client in #281)

This is the record of how the Bevy client drew footprints; the TUI's own drawing is in the next
section, "TUI", below.

- **One sprite per building**, from `World::buildings()` and `build_sites()`, not one per tile.
- **Anchor.** The sprite's bottom-centre sits at the bottom-middle of the footprint: halfway
  across its `w` tiles, on the bottom edge of row `anchor.y + h - 1`. For a 1x1 that is today's
  position exactly.
- **Size.** A footprint of `w` tiles takes a sprite `w * 16` px wide. Height may exceed `h * 16`
  only to overhang *upward* (masts, sails, roofs), as trees do. 2x2 is the existing 32x32 cell
  (`big`). A 3x3 would be a 48x48 cell, which `Cell { w, h }` already supports. A client test
  checks each building's opaque width against its footprint (allowing 2 px), the same way
  `NO_ART_YET` is checked today.
- **`row_depth`.** `row_depth(anchor.y + h - 1)`, the footprint's front row. Units can't stand
  inside a solid footprint, so they are always fully in front or fully behind. A unit in the
  front row of a walkable footprint (the orchard) draws over it, and one further back draws
  behind it.
- **Fade (#77).** Building sprites join the canopy fade (`canopy.rs`'s `target_alpha`), but only
  where the sprite extends beyond its footprint or the footprint is walkable: the orchard, the
  turbine's blades, and grandfathered 1x1 halls. A 2x2 solid building that fills its footprint
  hides nothing, so it never fades.
- **Site overlay.** `designate.rs` tints every tile of a site's footprint, not just the anchor.
- **Hover and selection.** Hovering any footprint tile outlines the whole footprint, and the
  tooltip (`fn_ui::build::site_report`, `built_name`) is the same from every tile.
  `Command::CancelBuild` accepts any tile, so the Cancel tool works anywhere on the site.
- **Demo.** `FN_BUILD_DEMO=buildings` places through footprints, with each building's grid cell
  sized to its footprint plus a one-tile gap.

### TUI (`crates/tui`)

- Every footprint tile draws the building's glyph in its colours, so a forge reads as a 2x2 block
  of `Ω`. This needs no new data, a block is easy to read on a glyph map, and it matches the
  sprite. Box-drawing borders were considered, but they lose the building's identity on its
  inner tiles.
- A site is the same block, dimmed, as it is per tile today.
- The Build tool's cursor shows the footprint as the ghost block, green or red per tile (the
  same `fn_ui::placement`), and a drag stamps the same lattice.
- `--ascii` is the same with the `ascii` character.

## Migration

- **Behaviour.** Every building with no `footprint` is 1x1, and every 1x1 rule is today's rule,
  apart from the fords and stockpile placement rules, which are deliberate. The sim and frontend
  work can land with all data still 1x1, so nothing visible changes until the last issue turns
  the sizes on in `data/buildings.toml`.
- **Saves.** Old saves load as they are (above). Their halls stay 1x1, passable and overhanging
  until a future deconstruct command lets the player rebuild them. This is a personal project
  with few saves, so no automatic upgrade is worth its complexity.
- **Docs.** When the sizes turn on, buildings go from "stands on its tile like a tree" to "fills
  its footprint, and may overhang upward" (recorded, historically, in the now-removed
  `pixel-art.md`; the current rule is "Footprints and the placement ghost" in
  docs/design/tui.md).

## Units inside buildings

**Decision (owner, 2026-10-25): option (b), entering through the door**, built on top of the
solid footprints (#206 to #214 unchanged) and tracked in #226 to #230. The owner asked how hard
it would be for units to go *inside* buildings. The three options below are kept as the
rationale. They differ mostly in what "inside" means to the sim.

(A render comparing the three options lived at `mockups/196/`, removed with the client in #281
and only in git history now.)

### (a) A walkable footprint

The building is `passable = true`. Units walk onto its tiles, and "inside" just means standing on
one.

- **Sim: almost nothing.** The design already handles walkable buildings: a walkable building is
  used from inside its footprint (`World::stand`), so for the halls this is a data change.
  Pathing, regions, jobs, sleep, heat (distance 0 inside), study, saves and determinism don't
  change. The catch is that a walkable building can be crossed from any side, so it becomes a
  shortcut through its walls. A per-building move cost (`Building::cost`, baked like `passable`)
  would make units prefer to go around. That is small, but it is one more field.
- **Client: the hard part is how it looks.** The sprite is sorted by its front row (`row_depth`),
  so a unit in the back row hides behind the roof and a unit in the front row draws *over* the
  walls (first panel above). The #77 fade can make the whole building translucent while anyone
  is inside (second panel). It reads, but in a busy settlement the buildings will flicker
  between solid and ghost. A cleaner look splits every 2x2 sprite into a back layer (floor and
  back wall) and a front layer (front wall and roof), with units drawn between the two and the
  front layer faded while occupied. That doubles the art for those buildings. Smooth movement
  (#194) needs no change, because units walk in like they walk anywhere else. The TUI already
  draws a unit over a building, so units show up inside the glyph block.
- **Risk:** low in the sim, medium for looks. **Size:** none in the sim (a data line in #214),
  plus about one client PR for the occupied fade. Layered art would add about one art PR per
  nation sheet (4).

### (b) An entrance, with "inside" as a state

The footprint stays solid, as designed. The building has an entrance: the ring tile below the
middle of its bottom row (`anchor.x + (w - 1) / 2`, `anchor.y + h`), where the art's door is.
A unit that reaches the entrance for a task that uses the building steps *in*. From then on it is
off the map: it isn't drawn, and the building's panel lists it. It steps back out onto the
entrance when the task ends or a need pre-empts it.

- **Location model.** `Unit` gains `inside: Option<Pos>` (the building's anchor, `serde(default)`).
  While a unit is inside, its `pos` stays the entrance tile, so everything that floods or paths
  from a unit (`flood_from`, regions, `place_ground`) keeps working unchanged. Only the code that
  asks "who is standing on the map here" has to skip units that are inside: idle wander, bending
  targets, occupancy checks, and drawing.
- **Pathing and regions:** unchanged. The footprint is solid and the entrance is an ordinary
  tile. Placement gains one rule: no footprint may cover another building's entrance, and a
  building's own entrance must be walkable when it is placed (a new `Blocker::Entrance`). If an
  entrance is blocked anyway (terrain bending, say), units leave by the nearest walkable ring
  tile. Ties break on `Pos`, so this stays deterministic.
- **Jobs, sleep, heat and study:**
  - Study seats become places inside, so `seats` is also the building's capacity.
  - Builders still work from the ring, never inside.
  - A 1x1 bed is unaffected: walkable, one sleeper lying right on its tile. A bed bigger than
    1x1 needs a capacity too (#227), the same way a study hall's seats do: that many sleeping
    places inside it, reached through the door. A future house or bunkhouse fits naturally this
    way, as a building with N sleeping places inside; no such building ships yet.
  - Heat already reaches the entrance tile, which is within any radius of the footprint, so
    being inside a warm building counts as warm with no extra rule.
  - A unit that dies inside is recorded at the entrance.
- **Saves and determinism:** a single `serde(default)` field. Entering and leaving happen on the
  tick the unit arrives or its task ends, in unit-id order like everything else in the tick.
- **Client:**
  - Entering, the sprite walks onto the entrance tile and fades out over about 0.3 s; leaving,
    it fades back in there. Nothing teleports, because the unit's `pos` never jumps, so #194's
    `is_teleport` is never triggered.
  - A badge on the building shows how many are inside.
  - Selecting the building opens a panel that lists the occupants. Clicking a name selects that
    unit, and its selection ring moves to the building.
  - No canopy fade is needed, because a solid 2x2 hides nothing.
- **TUI:** the entrance tile is drawn like any tile. The front-middle footprint cell shows the
  occupant count while anyone is inside, and the side panel lists the occupants.
- **Risk: medium.** Every system that reads unit positions has to be checked for `inside`.
  Hidden units also cut against "readable simulation" (vision pillar 4), which the badge, the
  panel and the hover tooltip ("Mai: studying in the Academy") have to make up for.
  **Size:** about 4 to 5 PRs on top of the current plan:
  1. sim: entrance, `inside` state, enter and leave, the placement rule;
  2. sim: study (and later sleep) using places inside;
  3. client: hidden occupants, badge, building panel;
  4. TUI: occupants;
  5. possibly the placement ghost showing the entrance tile.

### (c) A real interior

Each building has an interior map (or a layer of the main map) that units walk into through a
portal, shown either by switching the view or as a cutaway.

- **Sim:**
  - `Pos` gains a map or layer id. Every structure keyed by position needs it too: jobs, ground
    and stored items, zones, sites, heat, regions, `Flood` and `path`.
  - Pathing crosses portals, and regions need portal edges.
  - Interiors need content (floors, seats, beds), which means a layout format in data.
  - Saves need a `SAVE_VERSION` bump.
- **Client and TUI:**
  - a view mode or a cutaway renderer;
  - interior art for every building;
  - smooth movement and selection across views.
- **Scale problem:** a 2x2 building has a 2x2 interior, which is four tiles. Interiors are only
  worth having for much bigger buildings, or for rooms the player builds from walls as in
  RimWorld. That is a different building model, not an extension of this one.
- **Risk: high. Size:** 10 to 15 PRs or more, plus an art pass.

### Why (b)

(b) is the only option where "inside" means something, with a capacity and a list of who is in
a building, without changing the map model. It builds on the solid footprints rather than
replacing them. The entrance is simply the ring tile below the art's door. (a) remains
available per building in data for open structures: the orchard is walkable. (c) is a different
building model.

### Rules for (b)

- **Entrance:** `Footprint::entrance()`, the ring tile below the middle of the bottom row,
  `(anchor.x + (w - 1) / 2, anchor.y + h)`. All doors face down (no rotation). Placement refuses
  a footprint that covers another building's entrance, or whose own entrance isn't walkable
  (`Blocker::Entrance`).
- **Capacity:** `BuildingDef::capacity`, which defaults to `seats`. Only buildings with a
  capacity above 0 can be entered.
- **State:** `Unit::inside: Option<Pos>` (the anchor, `serde(default)`). While a unit is inside,
  its `pos` stays on the entrance, so floods, regions and paths are unchanged. Every system that
  reads unit positions skips units that are inside: wander, bending targets, occupancy, drawing.
  A unit leaves when its task ends or a need pre-empts it, coming out on the entrance (or the
  nearest walkable ring tile if that is blocked).
- **Uses:**
  - Study happens inside, and seats are places inside.
  - A multi-tile `[building.bed]` with a capacity means sleeping places inside (for a future
    house).
  - Being inside a building that keeps a fire counts as heated.
  - Builders and scroll deliveries (#24) never enter.
- **Frontends:**
  - The client fades a unit out and in at the door; `pos` doesn't jump, so #194 sees no
    teleport. It shows a count badge, and the building panel lists occupants, each selectable.
    The selection ring shows on the building while its unit is inside. Hidden units never
    trigger the canopy fade.
  - The TUI shows a count in the building's front-middle cell and an occupant list.
  - The placement ghost marks the entrance tile.

## Larger art

**Historical (#231, done; removed with the client in #281).** The owner asked to see larger art
for the 2x2 buildings, compared with the Bevy client's 32x32 sprites on the new footprints. It
shipped for the four study halls and was later removed along with every other sprite, so this
section is the record of that decision, not a live design. The mockup sheets lived at
`mockups/196/`, only in git history now.

Each sprite was 32 px wide, so it still filled its 2x2 footprint, and 35 to 42 px tall (including
the outline) in a 32x48 cell. The roof rose up to one tile into the row behind, as tree canopies
did. A 3x3 sketch for the bison stable showed what the next size up would look like, but that
building stayed 2x2.

- **What it bought:** scale. A unit is 16 px tall, so a hall was only twice the height of the
  people using it. At 40 px, the halls read as the settlement's landmarks, and each nation's
  silhouette is stronger: the Fire pagoda gains a storey and the Earth library a second floor.
- **What it costs:**
  - Roofs overhang the row behind, so building sprites need the #77 fade wherever they extend
    past their footprint. #210 already includes this rule.
  - Each larger sprite is a redraw. The rest of the Fire set (kiln, smelter, engine, lightning
    station) and the orchard would follow the forge's treatment.
- **Format and loader changes:**
  - A sheet's `cell` is already per sheet in `xtask/src/art.rs`, so tall sprites go on sheets
    with `cell = [32, 48]`: one per nation, or each nation's sheet moves to 32x48 cells. 1x1
    buildings keep their 32x32 sheets.
  - On the client, `sprites::Cell` already has `w` and `h` in 16 px units, so a `tall()` helper
    (`w: 2, h: 3`) sits next to `big()`, and 3x3 is `w: 3, h: 3` on a `[48, 48]` sheet.
  - The anchor doesn't change: the bottom-centre of the sprite sits at the bottom-middle of the
    footprint, so taller art only extends upward.
  - The client test that expects a 32x32 rect and the art-width test (#210) read the cell size
    from the sheet instead.
- **Why the halls first:** they were the landmarks every nation builds. Production buildings
  were meant to follow once #124 and #142 gave them a purpose; neither got the treatment before
  the client (and the whole art pipeline) was removed in #281.

## Follow-up issues

Each is sized for one PR. Blocked-by links are recorded on the issues.

| # | Issue | Blocked by |
|---|---|---|
| #206 | Save: a checked-in v1 save fixture and a load test | none |
| #207 | Sim: building footprints in data, on tiles and in placement | #206 |
| #208 | Sim: using a multi-tile building (ring stand, clearing on completion, heat and study) | #207 |
| #209 | UI: shared footprint placement (cursor anchor, drag lattice, per-tile validity, any-tile cancel) | #207 |
| #210 | Client: draw buildings and sites by footprint | #207 |
| #211 | Client: placement ghost preview | #209, #210 |
| #212 | TUI: footprints as glyph blocks and the placement ghost | #209 |
| #213 | Art: the igloo fits its tile | none |
| #214 | Data: turn on the footprint sizes and make halls solid | #208, #210, #211, #212 |
| #226 | Sim: units enter buildings through the door (inside state, entrance, capacity) | #208 |
| #227 | Sim: study, sleep and heat inside buildings | #226 |
| #228 | Client: door fade, count badge, occupants panel, selection | #226, #210 |
| #229 | TUI: occupant count and list for buildings | #228, #212 |
| #230 | Placement: show the entrance tile in the build ghost (client and TUI) | #226, #211, #212 |
| #231 | Art: the four study halls at 32x48 (tall sheet and loader) | #210 |

## Owner decisions (2026-10-25)

1. **Sizes:** as in the table. The four study halls, orchard, windmill, bison stable and all five
   Fire buildings are 2x2. The bed, campfire, earth wall, wind turbine and igloo are 1x1.
2. **Units inside:** option (b). Units enter through the door and are hidden and listed while
   inside, with places inside as capacity. This is built on top of the solid footprints.
3. **Rotation:** not needed.
4. **Art:** the four study halls at 32x48 first, production buildings later, the bison stable
   stays 2x2, and the igloo is narrowed (#213).
