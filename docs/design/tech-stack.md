# Tech stack options

Doc status: current (the Saves section is the live rule; the Bevy engine choice below is historical)

Status: **decided** 2026-10-21: Rust + Bevy, merged to `main`. **Superseded** 2026-11-03
(#93, #280, #281): the owner retired the Bevy client, its art pipeline and its CI cost
entirely, once a terminal frontend (`fn_tui`, docs/design/tui.md) reached parity with it. The
sim/frontend split this doc argues for is why that swap cost one crate, not a rewrite; the rest
of this doc is the record of the original choice and stays accurate for `fn_sim`.

## History

| Attempt | Years | Stack | Got as far as |
|---|---|---|---|
| [TheFourNations](https://github.com/gizmo385/TheFourNations) | 2013 | Java + Swing, mutable OO singletons | Playable: 4 tribes, units with hunger/energy/skills, task queue (collect, store, build, plant, eat, drink, sleep), A* paths, structures, save/load, win/lose |
| `archive/clojure-jvm` tag | 2021 | Clojure (JVM), EDN data files | Map gen, data-driven unit types and attributes, Swing-ish GUI, unit tests |
| `archive/cljs` tag | 2022 | ClojureScript + Reagent, deployed to GitHub Pages | Map gen with biomes, coast and resources, rendered as DOM tiles |

Each rewrite got the world generator working, but the colony sim itself (jobs, needs, AI) was
never rebuilt after 2013. The next stack should make **the simulation** easy to grow and test,
not just the map.

## What this game needs from a stack

1. **Thousands of entities ticking cheaply**: a colony sim is mostly a big update loop. ECS or
   data-oriented design helps; immutable persistent maps (cljs) don't.
2. **A deterministic, headless sim** that can be unit-tested and replayed without a GPU. This
   matters doubly for agent-driven development, because agents can't look at a window.
3. **Lots of UI** eventually: inventories, job lists, tooltips, menus.
4. **Data-driven content** (nations, biomes, resources, jobs) that people can edit without
   touching code.
5. **Web builds**: needed again (2026-11-11). The replayer is already hosted on the public
   Pages site (#596), and browser play will be hosted there too (see vision.md, "Audience and
   IP"). From 2026-10-21 until then this requirement was struck out as "no longer needed",
   because the game was meant to be personal-only.

## Options

| | Rust + Bevy | Rust + macroquad | Godot 4 (GDScript, optional Rust via gdext) | TypeScript + PixiJS | Stay on ClojureScript |
|---|---|---|---|---|---|
| Sim performance | Excellent (ECS, parallel systems) | Excellent (bring your own structure) | Fine in C#/Rust, slow in GDScript for big sims | OK | Weak for big grids |
| Headless testability | Excellent if the sim is a plain crate | Same | Possible but engine-centric | Good | Good |
| UI tooling | Weakest point: bevy_ui is serviceable, egui is good for debug UI | Minimal | **Best in class**: editor, themes, containers | DOM/React: very good | DOM: very good |
| Editor / level tools | None (code-first) | None | Full editor | None | None |
| Web build | Yes (wasm, WebGL2/WebGPU) | Yes, tiny binaries | Yes (large download) | Native | Native |
| Churn risk | High: breaking release about every 3–4 months (0.20 today) | Low | Low | Low | Low (ecosystem stagnant) |
| Agent-friendliness | Strong types + compiler catch most agent mistakes; long compiles | Same, faster compiles | Scene files (`.tscn`) are text but awkward for agents to edit; needs the editor | Good, but weaker guarantees | Good REPL, small ecosystem |
| Fit with "learn a modern language" goal | Yes | Yes | Partially | Not really | No |

## Recommendation: Rust, with the sim as a plain crate and Bevy as one frontend

```
crates/sim     fn_sim     pure Rust, deterministic, no engine deps — ALL game rules live here
crates/client  fn_client  Bevy app: rendering, input, UI. Reads sim state, sends commands.
xtask/                    repo tooling (asset checks, work tracker, CI entry point)
data/                     TOML game data loaded by the sim
assets/                   art/audio with a provenance manifest
```

(As planned at the time; `crates/client` and `assets/` are gone now. `crates/client`'s place in
the diagram was taken by `crates/tui` (`fn_tui`) plus `crates/ui` (`fn_ui`, the frontend logic
the two UIs shared before the first was retired); see "Superseded" above and tui.md.)

Why:

- **Splitting the sim from the frontend is the important decision**, more than the engine
  choice. It means the sim is unit-testable without a GPU, agents can verify their work with
  `cargo test`, and if Bevy's UI or churn turns out painful, the client can be swapped (to
  Godot via `gdext`, or to macroquad) without touching game rules.
- Bevy's ECS is a natural fit for the client side. If the sim outgrows plain structs, the sim
  can adopt `bevy_ecs` alone (it works standalone, with no renderer).
- Rust's compiler is a strong guard-rail for agent-written code.

Costs we accept:

- Bevy upgrades every few months. Mitigation: pin a version and upgrade deliberately as a work
  item. Keep the client thin.
- Compile times: the first build of the client takes minutes, though incremental builds are
  fine. The sim crate builds in seconds.
- UI will need investment. Use `bevy_egui` for debug and dev tools early, and decide on the
  player-facing UI later.

**Runner-up: Godot 4 + Rust (gdext)**, if UI and editor tooling turn out to matter more than
code-first workflows. Because of the crate split above, that switch would replace only
`crates/client`.

## Spike results (2026-10-21)

- `fn_sim` ports the cljs map generator pass for pass: deterministic, 13 tests, ASCII renderer.
- `fn_client` (Bevy 0.20) renders it with original pixel-art terrain and characters; units wander at nation-specific
  speeds; pan, zoom, regenerate, and switch nation.
- Bevy was built with `default-features = false`, plus 2D, UI, and X11 only, to avoid needing
  alsa, udev, or wayland dev headers on this machine (Nix toolchain on Ubuntu/WSL2). Audio and
  gamepad support will need those system libraries later.

## Dev environment (2026-10-22, trimmed 2026-11-03 in #281)

`flake.nix` provides a devShell with Rust (pinned via rust-overlay, kept in sync with CI's
version) and pkg-config. It used to add Bevy's native libraries too (alsa, udev, wayland, and
pointing the Vulkan loader at nixpkgs Mesa's drivers, because Ubuntu's drivers can't load into
Nix-built binaries); those left with the client in #281, since `fn_tui` needs no graphics stack.
CI doesn't use the flake.

## Saves (#7)

A save is a snapshot of the whole `World` as RON text (`World::save` / `World::load`, in
`crates/sim/src/save.rs`), not a seed plus command log. Replaying a log re-simulates every tick
played and silently becomes a different game when any rule or balance number changes; a snapshot
loads in about a tenth of a second and survives retuning. The command log is saved inside the
snapshot, so replay still works for tests and debugging. A save carries a format version
(`SAVE_VERSION`), and `load` refuses a version it doesn't know with a clear error. The sim does
no I/O: it produces and consumes the text, and the frontend decides where it lives
(`fn_ui::saves`, `FN_SAVE_DIR` or the platform's data directory).

Adding state to `World` needs no save code: the derive makes a field either serialize or fail to
compile. If you change the serialized shape of existing state so that old files can't be read,
bump `SAVE_VERSION` and add an upgrade step in `load`.

**Saves survive content changes (#602, owner decision 2026-11-09).** In memory, content ids
(`BiomeId`, `ResourceId`, `TechId`, `ItemId`, `BuildingId`, `AbilityId`) are table indices. A
save also writes, on its own line, the id strings each index meant. `load` maps every saved
index to today's index by name. The plan is in save-compat.md. For content in those six
tables:

- **May:** append, insert anywhere, reorder, and retune anything. A save made before any of
  these loads and plays on.
- **May remove** an entry that a given save doesn't use. A save that does use it fails with
  `SaveError::MissingContent` naming the id ("this save uses the building …").
- **May not rename** an id: to an old save, a rename is a removal plus an addition. A rename
  table can come later. Until then, a rename breaks every save that uses the id.
- **Not covered yet:** in-progress state that runs parallel to one definition's own lists, such
  as a build site's deliveries against `BuildingDef::cost` or a batch against
  `RecipeDef::inputs`. Append to those lists freely. Reordering or shortening them can
  misattribute what an old save had delivered.

What this asks of code:

- Never store state indexed by position in a content table. Key it by id instead, in a
  `BTreeMap`/`BTreeSet`. `Research::known` is the one exception, and `load` remaps it by hand.
- A new content-id type must use the remapping `Deserialize` macro and get its own table in
  `SaveIds`.

`Defs::id_hash` remains in the save header as information only. It is still the
`data_digest` of a run log, because a keystroke replay only holds for the same id order.

## RNG streams and content (#225)

All randomness comes from `fn_sim::rng::stream(seed, name)`, one named stream per subsystem. For
content that lives in `data/*.toml`, the stream is also keyed by the content's **id**, never by
its index or its position in the file: map resources roll from `mapgen.resources.<id>` and draw
cluster noise from `mapgen.resource_clusters.<id>`. Sharing one stream across a list makes every
entry's draws depend on how many entries came before it, so adding a resource would move every
tree and rock on every seed (and every map golden and seed-dependent test with them).

With per-id streams, adding, removing or reordering a resource leaves every other resource's
candidate tiles identical. The one thing list order still decides is priority where two resources
want the same tile: the earlier one in `data/resources.toml` wins it. Spacing (`min_spacing`) is
checked against a resource's own candidates, so a winner elsewhere never changes where the others
may grow. New data-driven content with its own randomness should follow the same rule.
