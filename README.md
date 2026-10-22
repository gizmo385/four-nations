# The Four Nations

A colony/settlement management game set in the world of *Avatar: The Last Airbender*, where each
nation plays differently. See [docs/design/vision.md](docs/design/vision.md).

## Rust prototype (this branch)

```sh
cargo run -p fn_client -- [seed] [air|water|earth|fire]   # Bevy client (first build takes minutes)
cargo run -p fn_sim --bin mapgen -- [seed] [width] [height] # ASCII map, no GPU needed
cargo xtask work next                                       # what to work on
cargo xtask ci                                              # fmt, clippy, tests, asset + work checks
```

Contributors and agents: read [AGENTS.md](AGENTS.md). Asset credits and licenses:
[assets/manifest.toml](assets/manifest.toml).

---

# Legacy ClojureScript version (below)


FIXME: Write a one-line description of your library/project.

## Overview

FIXME: Write a paragraph about the library/project and highlight its goals.

## Setup

To get an interactive development environment run:


    lein figwheel

and open your browser at [localhost:3449](http://localhost:3449/).
This will auto compile and send all changes to the browser without the
need to reload. After the compilation process is complete, you will
get a Browser Connected REPL. An easy way to try it is:

    (js/alert "Am I connected?")

and you should see an alert in the browser window.

To clean all compiled files:

    lein clean

To create a production build run:

    lein do clean, cljsbuild once min

And open your browser in `resources/public/index.html`. You will not
get live reloading, nor a REPL. 

## License

Copyright © 2014 FIXME

Distributed under the Eclipse Public License either version 1.0 or (at your option) any later version.
