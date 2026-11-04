# The Four Nations

A colony/settlement management game set in the world of *Avatar: The Last Airbender*, where each
nation plays differently. See [docs/design/vision.md](docs/design/vision.md).

## Playing

`cargo run` opens the terminal UI on the title screen: pick New game (a nation and a seed),
Continue, Load or Quit. In the game, arrows or WASD move the cursor (Shift: 10 tiles), Enter
selects a unit, `H` `X` `P` `B` `U` pick the harvest, cancel, stockpile, bend and build tools,
Space pauses, `+`/`-` change speed, `T` opens research, `L` the message log, `?` the key help,
`F5`/`F9` quicksave/load, `F6` the save list, and `q` quits. Full list: `cargo run -- --help`.

With Nix, `nix develop` (see [flake.nix](flake.nix)) gives you the pinned Rust toolchain. For
direnv, put `use flake` in an `.envrc`.

```sh
cargo run                                                    # the title screen
cargo run -- SEED NATION                                     # play in the terminal, skipping it
cargo run -- SEED NATION --frame                             # print one frame, no TTY needed
cargo run -p fn_sim --bin mapgen -- [seed] [width] [height]  # ASCII map
gh issue list --label ready                                  # what to work on
cargo xtask ci --fast                                        # fmt, clippy, tests (the gate)
```

Contributors and agents: read [AGENTS.md](AGENTS.md).

Earlier versions live in history: the `archive/cljs` tag (ClojureScript, 2022), the
`archive/clojure-jvm` tag (Clojure, 2021), and the
[TheFourNations](https://github.com/gizmo385/TheFourNations) repo (Java, 2013).
