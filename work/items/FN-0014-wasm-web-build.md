+++
id = "FN-0014"
title = "Web (wasm) build deployed to GitHub Pages"
status = "proposed"
priority = "p3"
area = "tooling"
depends_on = ["FN-0005", "FN-0013"]
owner = ""
created = 2026-10-21
+++

## Why
The cljs version was playable in the browser; keep that ability.

## Acceptance criteria
- [ ] `fn_client` builds for wasm32-unknown-unknown (trunk or wasm-bindgen-cli).
- [ ] Gated on `cargo xtask assets --release` passing — no unknown-license art goes public.
