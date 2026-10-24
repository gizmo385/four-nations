# The Four Nations

A colony/settlement management game set in the world of *Avatar: The Last Airbender*, where each
nation plays differently. See [docs/design/vision.md](docs/design/vision.md).

## Running

With Nix, `nix develop` (see [flake.nix](flake.nix)) gives you the pinned Rust toolchain and the
native libraries Bevy needs, and is the supported way to run the client on Ubuntu and WSL2. For
direnv, put `use flake` in an `.envrc`. `scripts/run-client.sh` enters the shell on its own.

```sh
cargo run -p fn_client -- [seed] [air|water|earth|fire]   # Bevy client (first build takes minutes)
cargo run -p fn_sim --bin mapgen -- [seed] [width] [height] # ASCII map, no GPU needed
cargo run -p fn_tui -- SEED NATION                          # play in the terminal (--help)
cargo run -p fn_tui -- SEED NATION --frame                  # print one frame, no TTY needed
gh issue list --label ready                                 # what to work on
cargo xtask ci                                              # fmt, clippy, tests, asset check
cargo xtask ci sim                                          # same, minus the Bevy client (fast)
```

Contributors and agents: read [AGENTS.md](AGENTS.md). Asset credits and licenses:
[assets/manifest.toml](assets/manifest.toml).

Earlier versions live in history: the `archive/cljs` tag (ClojureScript, 2022), the
`archive/clojure-jvm` tag (Clojure, 2021), and the
[TheFourNations](https://github.com/gizmo385/TheFourNations) repo (Java, 2013).
