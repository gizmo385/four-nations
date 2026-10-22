# Tech stack options

Status: **decided** 2026-10-21: Rust + Bevy, merged to `main`.

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
5. ~~Web builds~~: no longer needed, since the game is personal-only (see vision.md).

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
- `fn_client` (Bevy 0.20) renders it with Ninja Adventure terrain and characters; units wander at nation-specific
  speeds; pan, zoom, regenerate, and switch nation.
- Bevy was built with `default-features = false`, plus 2D, UI, and X11 only, to avoid needing
  alsa, udev, or wayland dev headers on this machine (Nix toolchain on Ubuntu/WSL2). Audio and
  gamepad support will need those system libraries later.

## Dev environment (2026-10-22)

`flake.nix` provides a devShell with Rust (pinned via rust-overlay, kept in sync with CI's
version), pkg-config, and Bevy's native libraries, including alsa, udev, and wayland, so enabling
those Bevy features later only needs a Cargo.toml change. It points the Vulkan loader at nixpkgs
Mesa's drivers, because Ubuntu's drivers can't load into Nix-built binaries. CI doesn't use the
flake.
