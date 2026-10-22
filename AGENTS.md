# Agent rules

These rules are for any coding agent (Claude Code, Codex, and others) and for humans. If a rule
gets in your way, stop and say so in the work item's Notes instead of working around it.

## Orientation (read first)

- `docs/design/vision.md`: what the game is.
- `docs/design/tech-stack.md`: why the code is shaped this way.
- `docs/process/work-tracking.md`: how work is picked up and handed off.
- `cargo xtask work next`: what to work on.

## Layout

| Path | What | Rules |
|---|---|---|
| `crates/sim` (`fn_sim`) | All game rules and state | Headless and deterministic (see below) |
| `crates/client` (`fn_client`) | Bevy rendering, input, UI | No game rules. Reads `World`, changes it only via sim APIs |
| `data/*.toml` | Nations, biomes, resources, ... | Balance numbers go here, not in code |
| `assets/` | Art and audio | Every file in `assets/manifest.toml` |
| `work/items/` | Work tracker | One item per branch |
| `xtask/` | Repo tooling | `cargo xtask ci` is the gate |
| `src/`, `resources/`, `project.clj` | Legacy ClojureScript version | Reference only. Don't modify |

## Hard rules

1. **One work item per branch**, named `work/FN-XXXX-slug`. Claim the item in your first commit
   (see work-tracking.md). Don't work on `proposed` items, and don't set items to `done`.
2. **`cargo xtask ci` must pass** before you set an item to `review`: fmt, clippy with
   `-D warnings`, tests, assets, work check.
3. **The sim stays pure.** `fn_sim` must not depend on Bevy or any engine. It does no file or
   network I/O (data is embedded or passed in), reads no wall-clock time, and uses no
   thread or OS RNG. All randomness comes from `fn_sim::rng::stream(seed, "subsystem.name")`
   with a new stream name per subsystem.
4. **Determinism is tested.** New sim behavior needs a test. If you change map generation on
   purpose, re-bless the golden files (`FN_BLESS=1 cargo test -p fn_sim`) and say so in the
   commit message.
5. **No unrecorded assets.** Don't add, download or modify files in `assets/` without updating
   `assets/manifest.toml` and following `docs/process/assets.md`. Never commit art with an
   unknown license as `release_ok = true`.
6. **Nations differ through data first.** Read `Modifiers` and nation definitions; don't
   write `if nation == Fire` in systems unless the feature truly is unique to one nation
   (bending abilities), and then keep it in one obvious place.
7. **Never push, merge, force-push, or touch `cljs`, `main` or `gh-pages`** unless a human
   asks you to in this session. The `cljs` branch auto-deploys to GitHub Pages on push.
8. **Stay in scope.** Found something else that needs doing? Create a `proposed` item with
   `cargo xtask work new` and keep going.

## Conventions

- Rust 2024 edition, `rustfmt.toml` (100 columns). Module doc comments explain *why*.
- Prefer plain structs and functions in `fn_sim`. Reach for `bevy_ecs` there only via a work item.
- Tests: unit tests next to code, cross-module behavior in `crates/sim/tests/`.
- Commit messages: imperative subject that starts with the item id, e.g.
  `FN-0006: Add A* pathfinding`.
- **Don't open the GUI unless a human asks.** It pops a window and steals focus. Verify with
  tests and the ASCII renderer first. If you truly need pixels, use one
  `FN_SCREENSHOT=out.png scripts/run-client.sh SEED NATION` run (it opens unfocused and behind other
  windows, then exits after about 4s), and say in the work item that you did.
- Visual changes: the sim has an ASCII renderer (`cargo run -p fn_sim --bin mapgen -- SEED W H`).
  Paste relevant output into the work item when it helps the reviewer.

## Handoff

Before you stop, even if you're unfinished, append a dated Notes entry to the work item with
what's done, what's next, and anything surprising. The next agent starts from that entry.
