# Agent rules

These rules are for any coding agent (Claude Code, Codex, and others) and for humans. If a rule
gets in your way, stop and say so in the work item's Notes instead of working around it.

## Orientation (read first)

- `docs/design/vision.md`: what the game is.
- `docs/design/tech-stack.md`: why the code is shaped this way.
- `docs/process/work-tracking.md`: GitHub Issues, labels, claiming, and direct-to-main vs. PR.
- `gh issue list --label ready`: what to work on.

## Layout

| Path | What | Rules |
|---|---|---|
| `crates/sim` (`fn_sim`) | All game rules and state | Headless and deterministic (see below) |
| `crates/client` (`fn_client`) | Bevy rendering, input, UI | No game rules. Reads `World`, changes it only via sim APIs |
| `data/*.toml` | Nations, biomes, resources, ... | Balance numbers go here, not in code |
| `assets/` | Art and audio | Every file in `assets/manifest.toml` |
| `xtask/` | Repo tooling | `cargo xtask ci` is the gate |
| `scripts/` | Dev helpers (e.g. `run-client.sh` for Nix + WSL) | |

## Hard rules

1. **Work from an unblocked GitHub issue labelled `ready`**, and claim it first (`in-progress`
   label plus a claim comment). One issue at a time. Only the owner or the orchestrator's triage
   pass (docs/process/work-tracking.md) adds `ready`.
2. **Choose the landing flow** from docs/process/work-tracking.md. Small or medium work, with
   you as the only active agent: commit straight to `main` with `Fixes #N`. Parallel agents or
   `large` work: branch `issue/N-slug` and a PR with `Closes #N`.
3. **`cargo xtask ci` must pass** before every push: fmt, clippy with `-D warnings`, tests and
   assets.
4. **The sim stays pure.** `fn_sim` must not depend on Bevy or any engine. It does no file or
   network I/O (data is embedded or passed in), reads no wall-clock time, and uses no
   thread or OS RNG. All randomness comes from `fn_sim::rng::stream(seed, "subsystem.name")`
   with a new stream name per subsystem.
5. **Determinism is tested.** New sim behavior needs a test. If you change map generation on
   purpose, re-bless the golden files (`FN_BLESS=1 cargo test -p fn_sim`) and say so in the
   commit message.
6. **No unrecorded assets.** Don't add, download or modify files in `assets/` without updating
   `assets/manifest.toml` and following `docs/process/assets.md`.
7. **Nations differ through data first.** Read `Modifiers` and nation definitions; don't
   write `if nation == Fire` in systems unless the feature truly is unique to one nation
   (bending abilities), and then keep it in one obvious place.
8. **Git safety and PR ownership.** Never force-push `main`, rewrite published history, or
   delete branches you didn't create. You own the PRs you open: turn on auto-merge
   (`gh pr merge --auto --squash`), fix CI failures, keep the branch rebased (`--force-with-lease`
   only on your own PR branch), and resolve conflicts until it merges. Don't auto-merge changes to
   AGENTS.md, docs/process/, or CI/workflows; leave those for the owner. Pushing to `main` is allowed
   only through the direct-to-main flow, after `git pull --rebase` and a passing `cargo xtask ci`.
9. **Stay in scope.** Found something else that needs doing? File an issue
   (`gh issue create --template work-item.md`, without `ready`) and keep going.

## Conventions

- Rust 2024 edition, `rustfmt.toml` (100 columns). Module doc comments explain *why*.
- Prefer plain structs and functions in `fn_sim`. Reach for `bevy_ecs` there only via a work item.
- Tests: unit tests next to code, cross-module behavior in `crates/sim/tests/`.
- Commit messages: imperative subject that starts with the issue number, e.g.
  `#2: Add A* pathfinding`; `Fixes #N` / `Closes #N` in the body or PR.
- **Don't open the GUI unless a human asks.** It pops a window and steals focus. Verify with
  tests and the ASCII renderer first. If you need pixels, use
  `cargo xtask screenshot SEED NATION OUT.png [WIDTH HEIGHT]`: it renders headless (no window at
  all) to a PNG and exits. Write the PNG under `target/`, not into the repo tree.
- Visual changes: the sim has an ASCII renderer (`cargo run -p fn_sim --bin mapgen -- SEED W H`).
  Paste relevant output into the issue or PR when it helps the reviewer.

## Handoff

Before you stop, even if you're unfinished, comment on the issue with what's done, what's next,
and anything surprising. If you're stopping unfinished, remove `in-progress`. The next agent
starts from that comment.
