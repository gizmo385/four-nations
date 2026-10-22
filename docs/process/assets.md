# Assets and provenance

Every file under `assets/` must be listed in `assets/manifest.toml` with its source, license
and sha256. `cargo xtask assets` enforces this and runs as part of `cargo xtask ci`.

## Adding an asset

1. Prefer, in order: **CC0** (Kenney, OpenGameArt CC0), then **CC-BY** (credit required), then
   **CC-BY-SA / GPL** (share-alike; ask first). Never: "free for personal use", ripped game
   sprites, AI-generated images of canon characters, or anything without a stated license.
2. Put it under `assets/third_party/<source-id>/` (or `assets/original/` for our own art) and
   keep the upstream license file next to it.
3. Add a `[[source]]` entry (url, download_url, author, license, `release_ok`, retrieved date,
   who retrieved it) and a `[[file]]` entry per file. Note any modifications: crops,
   re-encodes, recolors.
4. Run `cargo xtask assets`.

## `release_ok = false`

This marks assets we can use locally while prototyping but must not publish. Today, that's all
of `thefournations-2013`: the original artists are unknown and the unit sprites depict canon
characters. `cargo xtask assets --release` fails while any of these are present, and the web
build (FN-0014) is gated on it.
