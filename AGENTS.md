# Agent rules

These rules are for any coding agent (Claude Code, Codex, and others) and for humans. If a rule
gets in your way, stop and say so in the work item's Notes instead of working around it.

## Orientation (read first)

- `docs/design/vision.md`: what the game is.
- `docs/design/tech-stack.md`: why the code is shaped this way.
- `docs/design/README.md`: which design docs are current, superseded or historical.
- `docs/process/work-tracking.md`: GitHub Issues, labels, claiming, and direct-to-main vs. PR.
- `gh issue list --label ready`: what to work on.

## Layout

| Path | What | Rules |
|---|---|---|
| `crates/sim` (`fn_sim`) | All game rules and state | Headless and deterministic (see below) |
| `crates/ui` (`fn_ui`) | Frontend logic shared by every UI (tools, text, view models, theme, clock) | Depends on `fn_sim` only. No wall-clock reads. No I/O except `fn_ui::saves`, the one module that reads and writes save files |
| `crates/tui` (`fn_tui`) | The terminal frontend (ratatui + crossterm); `cargo run` plays it | Reads `World`, changes it only via sim APIs. Headless by construction: see below |
| `crates/harness` (`fn_harness`) | The balance harness (docs/design/balance-harness.md): play styles so far; reads `data/harness.toml` | Depends on `fn_sim`. The only crate that reads `data/harness.toml`; `fn_sim` never does |
| `crates/agent` (`fn_agent`) | The agent surface: an MCP server over stdio (`fn_agent serve`), run logs and scorecards (docs/design/agent-play.md) | Depends on `fn_tui` and `fn_ui`; never names `fn_sim`, so an agent sees only what a player sees |
| `crates/web` (`fn_web`) | The web replayer: `fn_tui`'s run-log watcher compiled to wasm32 (#595) | Depends on `fn_sim` and `fn_tui`. No clock or file reads: the page hands it the log text |
| `data/*.toml` | Nations, biomes, resources, ... | Balance numbers go here, not in code |
| `xtask/` | Repo tooling (`cargo xtask`; also drives agent runs) | `cargo xtask ci --fast` is the gate |
| `scripts/` | Dev helpers | |

## Hard rules

1. **Work from an unblocked GitHub issue labelled `ready`**, and claim it first (`in-progress`
   label plus a claim comment). One issue at a time. Only the owner or the orchestrator's triage
   pass (docs/process/work-tracking.md) adds `ready`.
2. **Choose the landing flow** from docs/process/work-tracking.md. Small or medium work, with
   you as the only active agent: commit straight to `main` with `Fixes #N`. Parallel agents or
   `large` work: branch `issue/N-slug` and a PR with `Closes #N`.
3. **`cargo xtask ci --fast` must pass** before every push: fmt, clippy with `-D warnings` and
   tests for every crate.
4. **The sim stays pure.** `fn_sim` must not depend on Bevy or any engine. It does no file or
   network I/O (data is embedded or passed in), reads no wall-clock time, and uses no
   thread or OS RNG; clippy enforces this in `fn_sim` (`crates/sim/clippy.toml` bans
   `std::time::Instant`/`SystemTime`, `std::fs`, `std::env::var` and thread/OS RNG types).
   All randomness comes from `fn_sim::rng::stream(seed, "subsystem.name")`
   with a new stream name per subsystem.
5. **Determinism is tested.** New sim behavior needs a test. If you change map generation on
   purpose, re-bless the golden files (`FN_BLESS=1 cargo test -p fn_sim`) and say so in the
   commit message.
6. **Nations differ through data first.** Read `Modifiers` and nation definitions; don't
   write `if nation == Fire` in systems unless the feature truly is unique to one nation
   (bending abilities), and then keep it in one obvious place.
7. **Git safety and PR ownership.** Never force-push `main`, rewrite published history, or
   delete branches you didn't create. You own the PRs you open: turn on auto-merge
   (`gh pr merge --auto --squash`), fix CI failures, keep the branch rebased (`--force-with-lease`
   only on your own PR branch), and resolve conflicts until it merges. Don't auto-merge changes to
   AGENTS.md, docs/process/, or CI/workflows; leave those for the owner. Pushing to `main` is allowed
   only through the direct-to-main flow, after `git pull --rebase` and a passing `cargo xtask ci --fast`.
8. **Stay in scope.** Found something else that needs doing? File an issue
   (`gh issue create --template work-item.md`, without `ready`) and keep going.

## Conventions

- Rust 2024 edition, `rustfmt.toml` (100 columns). Module doc comments explain *why*.
- Prefer plain structs and functions in `fn_sim`. Reach for `bevy_ecs` there only via a work item.
- Tests: unit tests next to code, cross-module behavior in `crates/sim/tests/`.
- Commit messages: imperative subject that starts with the issue number, e.g.
  `#2: Add A* pathfinding`; `Fixes #N` / `Closes #N` in the body or PR.
- **Don't launch `fn_tui` interactively unless a human asks.** It takes over the terminal.
  Verify with tests and `cargo run -p fn_tui -- SEED NATION --frame [--keys SCRIPT]`: it renders
  one frame to stdout and exits, no TTY needed. `--keys` plays a key script first (see
  `fn_tui --help`), so an agent can "play" a scripted session and check the resulting frame.
- Player-facing text (TUI strings, `fn_ui` view text, descriptions and hints in `data/*.toml`)
  follows docs/design/text-voice.md.
- Visual changes: paste `--frame` output (or the sim's ASCII renderer,
  `cargo run -p fn_sim --bin mapgen -- SEED W H`) into the issue or PR when it helps the reviewer.

## Handoff

Before you stop, even if you're unfinished, comment on the issue with what's done, what's next,
and anything surprising. If you're stopping unfinished, remove `in-progress`. The next agent
starts from that comment.
