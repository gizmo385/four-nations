+++
id = "FN-0001"
title = "Port ClojureScript map generator to fn_sim"
status = "done"
priority = "p0"
area = "sim"
depends_on = []
owner = ""
created = 2026-10-21
+++

## Why
Map generation is the most complete system from the cljs branch; porting it proves out the Rust
sim crate and gives every later system a world to stand on.

## Acceptance criteria
- [x] Noise + smoothing, water/land split, biome region growth, water border, water spread,
      drowning of isolated land, coastline, resource rolls — all ported.
- [x] Deterministic per seed; per-pass RNG streams.
- [x] Invariant tests + golden ASCII snapshot. `cargo run -p fn_sim --bin mapgen` prints a map.

## Notes
- 2026-10-21: Added a mountain band (top 6% elevation) and a `stone` resource; fixed the cljs
  off-by-one that left some tiles without a biome.
