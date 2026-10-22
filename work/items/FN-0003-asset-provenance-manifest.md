+++
id = "FN-0003"
title = "Asset provenance manifest and checker"
status = "done"
priority = "p0"
area = "tooling"
depends_on = []
owner = ""
created = 2026-10-21
+++

## Acceptance criteria
- [x] `assets/manifest.toml` lists every file with source, license, sha256.
- [x] `cargo xtask assets` fails on unlisted/changed/missing files; `--release` also fails on
      sources with `release_ok = false`.
